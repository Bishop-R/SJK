//! The News panel's pictures (`docs/sjk-ui.md`, "News panel"): an announcement names
//! them (`[[image:<id>]]`, [`sjk_identity::news_markup`]) and the hub serves them
//! (`GET /v1/news-image/<id>`, no signature). A worker thread reads each from this PC
//! (`news/<id>` beside the settings) or downloads it from the hub and keeps it there,
//! then decodes it with its mip chain; [`service`], once a frame, uploads what the panel
//! asks for into one of [`NEWS_IMAGE_SLOTS`] textures of its own. Until a picture is
//! there the panel draws its frame. Nothing on the frame thread waits: it compares a few
//! strings and hands finished pixels over.
//!
//! At most [`DECODED`] pictures stay decoded in memory, the least used making room, and
//! a slot nobody asked for in [`IDLE_FRAMES`] frames is given back as a 1x1 texture, so
//! a closed panel holds no video memory.

use crate::menu::levelshot::LevelshotImage;
use crate::ui_renderer::{NEWS_IMAGE_SLOTS, news_image_texture};
use sjk_ui::TextureId;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// The folder beside the settings that keeps the pictures read from the hub.
pub(crate) const DIR: &str = "news";
/// Pictures kept decoded at once.
const DECODED: usize = 4;
/// Frames a slot may go unasked before its texture is given back.
const IDLE_FRAMES: u64 = 600;
/// How long a picture that could not be had waits before it is asked for again.
const RETRY_AFTER: Duration = Duration::from_secs(120);

enum State {
    Loading,
    Ready(Arc<LevelshotImage>),
    Missing(Instant),
}

struct Picture {
    id: String,
    state: State,
    used: u64,
}

/// A texture slot: the picture it holds (or is to hold) and the renderer it is in.
#[derive(Default)]
struct Slot {
    id: String,
    used: u64,
    uploaded_into: Option<u64>,
    /// Given back as a 1x1 texture, in that renderer.
    freed_in: Option<u64>,
}

/// What the worker answers: the picture's id and its pixels, or none.
type Answer = (String, Option<LevelshotImage>);

struct Request {
    id: String,
    hub_url: String,
    dir: Option<PathBuf>,
}

#[derive(Default)]
struct Cache {
    hub_url: String,
    dir: Option<PathBuf>,
    pictures: Vec<Picture>,
    slots: [Slot; NEWS_IMAGE_SLOTS],
    frame: u64,
    renderer: Option<u64>,
    worker: Option<(Sender<Request>, Receiver<Answer>)>,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);

fn lock() -> MutexGuard<'static, Option<Cache>> {
    CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Read pictures from the hub at `hub_url` (empty: none) and keep them in `dir`'s
/// [`DIR`] folder.
pub(crate) fn configure(config_directory: Option<&std::path::Path>, hub_url: &str) {
    let mut cache = lock();
    let cache = cache.get_or_insert_with(Cache::default);
    let dir = config_directory.map(|dir| dir.join(DIR));
    if cache.hub_url == hub_url && cache.dir == dir {
        return;
    }
    cache.hub_url = hub_url.to_owned();
    cache.dir = dir;
    cache
        .pictures
        .retain(|picture| !matches!(picture.state, State::Missing(_)));
}

impl Cache {
    fn texture(&mut self, id: &str) -> Option<TextureId> {
        let frame = self.frame;
        match self.pictures.iter_mut().find(|picture| picture.id == id) {
            Some(picture) => {
                picture.used = frame;
                if let State::Missing(at) = picture.state
                    && at.elapsed() >= RETRY_AFTER
                {
                    picture.state = State::Loading;
                    self.ask(id);
                }
            }
            None => {
                self.pictures.push(Picture {
                    id: id.to_owned(),
                    state: State::Loading,
                    used: frame,
                });
                self.ask(id);
            }
        }
        let ready = self
            .pictures
            .iter()
            .any(|picture| picture.id == id && matches!(picture.state, State::Ready(_)));
        if !ready {
            return None;
        }
        let index = match self.slots.iter().position(|slot| slot.id == id) {
            Some(index) => index,
            None => {
                // The slot used least long ago takes it.
                let index = (0..self.slots.len())
                    .min_by_key(|index| self.slots[*index].used)
                    .unwrap_or_default();
                self.slots[index] = Slot {
                    id: id.to_owned(),
                    ..Slot::default()
                };
                index
            }
        };
        let slot = &mut self.slots[index];
        slot.used = frame;
        (slot.uploaded_into.is_some() && slot.uploaded_into == self.renderer)
            .then(|| news_image_texture(index))
    }

    fn ask(&mut self, id: &str) {
        if self.hub_url.is_empty() && self.dir.is_none() {
            return;
        }
        let (requests, _) = self.worker.get_or_insert_with(spawn);
        let _ = requests.send(Request {
            id: id.to_owned(),
            hub_url: self.hub_url.clone(),
            dir: self.dir.clone(),
        });
    }

    fn take_answers(&mut self) {
        let Some((_, answers)) = &self.worker else {
            return;
        };
        let answers: Vec<_> = answers.try_iter().collect();
        for (id, image) in answers {
            if let Some(picture) = self.pictures.iter_mut().find(|picture| picture.id == id) {
                picture.state = match image {
                    Some(image) => State::Ready(Arc::new(image)),
                    None => State::Missing(Instant::now()),
                };
            }
        }
        // Keep the decoded pictures to the newest few.
        while self
            .pictures
            .iter()
            .filter(|picture| matches!(picture.state, State::Ready(_)))
            .count()
            > DECODED
        {
            let Some(oldest) = self
                .pictures
                .iter()
                .enumerate()
                .filter(|(_, picture)| matches!(picture.state, State::Ready(_)))
                .min_by_key(|(_, picture)| picture.used)
                .map(|(index, _)| index)
            else {
                break;
            };
            self.pictures.remove(oldest);
        }
    }
}

/// The texture of news picture `id` once it is uploaded, asking for it the first time;
/// `None` until then (the panel draws the picture's frame).
pub(crate) fn texture(id: &str) -> Option<TextureId> {
    lock().get_or_insert_with(Cache::default).texture(id)
}

/// Once a frame: take what the worker finished, upload the pictures asked for into their
/// slots, and give back the slots nobody asked for in a while.
pub(crate) fn service(
    renderer: &mut crate::ui_renderer::ShapeRenderer,
    device: &wgpu::Device,
    queue: &crate::frame_queue::FrameQueue,
) {
    let mut guard = lock();
    let Some(cache) = guard.as_mut() else {
        return;
    };
    cache.frame += 1;
    let id = renderer.id();
    cache.renderer = Some(id);
    cache.take_answers();
    let frame = cache.frame;
    for index in 0..cache.slots.len() {
        let slot = &cache.slots[index];
        if slot.id.is_empty() {
            continue;
        }
        if frame.saturating_sub(slot.used) > IDLE_FRAMES {
            if cache.slots[index].freed_in != Some(id) {
                renderer.upload_news_image(device, queue, index, &blank());
                cache.slots[index] = Slot {
                    freed_in: Some(id),
                    ..Slot::default()
                };
            }
            continue;
        }
        if slot.uploaded_into == Some(id) {
            continue;
        }
        let image = cache
            .pictures
            .iter()
            .find_map(|picture| match &picture.state {
                State::Ready(image) if picture.id == slot.id => Some(image.clone()),
                _ => None,
            });
        if let Some(image) = image {
            renderer.upload_news_image(device, queue, index, &image);
            cache.slots[index].uploaded_into = Some(id);
            cache.slots[index].freed_in = None;
        }
    }
}

/// A transparent 1x1 picture, which gives a slot's texture back.
fn blank() -> LevelshotImage {
    LevelshotImage {
        size: [1, 1],
        levels: vec![vec![0; 4]],
    }
}

fn spawn() -> (Sender<Request>, Receiver<Answer>) {
    let (requests, inbox) = channel::<Request>();
    let (outbox, answers) = channel();
    let spawned = std::thread::Builder::new()
        .name("sjk-news-images".to_owned())
        .spawn(move || {
            let mut hubs: Option<(String, sjk_identity::HttpHub)> = None;
            for request in inbox {
                let image = load(&request, &mut hubs);
                if outbox.send((request.id, image)).is_err() {
                    break;
                }
            }
        });
    if let Err(error) = spawned {
        crate::log::progress(format_args!(
            "news pictures: cannot start a worker: {error}"
        ));
    }
    (requests, answers)
}

/// Picture `request.id` from this PC, else from the hub (kept on this PC then), decoded.
fn load(
    request: &Request,
    hubs: &mut Option<(String, sjk_identity::HttpHub)>,
) -> Option<LevelshotImage> {
    if !sjk_identity::news_markup::image_id_ok(&request.id) {
        return None;
    }
    let path = request.dir.as_ref().map(|dir| dir.join(&request.id));
    let kept = path.as_ref().and_then(|path| std::fs::read(path).ok());
    let bytes = match kept {
        Some(bytes) => bytes,
        None => {
            if request.hub_url.is_empty() {
                return None;
            }
            if hubs.as_ref().is_none_or(|(url, _)| *url != request.hub_url) {
                let hub =
                    sjk_identity::HttpHub::new(&request.hub_url, crate::build_info::hub_agent())
                        .ok()?;
                *hubs = Some((request.hub_url.clone(), hub));
            }
            let (_, hub) = hubs.as_mut()?;
            let bytes = match sjk_identity::Hub::news_image(hub, &request.id) {
                Ok(bytes) => bytes,
                Err(error) => {
                    crate::log::progress(format_args!(
                        "news pictures: {} not read: {error}",
                        request.id
                    ));
                    return None;
                }
            };
            if let Some(path) = &path
                && let Some(dir) = path.parent()
                && std::fs::create_dir_all(dir).is_ok()
            {
                let _ = std::fs::write(path, &bytes);
            }
            bytes
        }
    };
    decode(&bytes)
}

/// A PNG's or JPEG's pixels with their mip chain.
fn decode(bytes: &[u8]) -> Option<LevelshotImage> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    Some(LevelshotImage::from_rgba(image))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_png_decodes_with_its_mips() {
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(8, 4, image::Rgba([10, 20, 30, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let picture = decode(&png).unwrap();
        assert_eq!(picture.size, [8, 4]);
        assert!(picture.levels.len() > 1);
        assert!(decode(b"not a picture").is_none());
    }

    #[test]
    fn a_picture_is_asked_for_once_and_kept_on_this_pc() {
        let directory = tempfile::tempdir().unwrap();
        let id = "0123456789abcdef";
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(directory.path().join(id), &png).unwrap();
        let request = Request {
            id: id.to_owned(),
            hub_url: String::new(),
            dir: Some(directory.path().to_path_buf()),
        };
        assert_eq!(
            load(&request, &mut None).map(|image| image.size),
            Some([2, 2])
        );
        let bad = Request {
            id: "../escape".to_owned(),
            ..request
        };
        assert!(load(&bad, &mut None).is_none(), "only real ids name a file");
    }
}
