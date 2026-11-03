//! The quick wheel's own pictures (Nib's idea, 03/11/2026): a PNG or JPEG from the
//! player's PC shown as a wheel choice's icon, chosen in the custom choice's form
//! (Settings > Interface > Quick wheel). The picture is checked ([`MAX_BYTES`],
//! [`MAX_SIDE`]), fitted into one atlas cell as the emotes' are
//! ([`crate::emote_icons::fit`]) and kept as a small PNG in `wheel_icons/` in the profile
//! folder (`GameData/SJK/`), named by its content, so the original can move or go. Which
//! command shows which picture is kept in `wheel_icons.json` beside `wheel.json`:
//!
//! ```json
//! { "version": 1, "icons": { "say gg": "3f2a9c0d81b7e655.png" } }
//! ```
//!
//! A choice is matched by its command, so the same command on two pages shows the same
//! picture. The pictures go into each renderer's atlas ([`service`], the emotes' rule:
//! every new world has an empty one), [`CELLS`] different pictures at most (the form
//! refuses another); one that does not decode is reported to the log and its choice keeps
//! the stock picture.

use crate::ui_renderer::{CUSTOM_WHEEL_ICON_CELLS, ShapeRenderer, custom_wheel_icon};
use serde_json::{Value, json};
use sjk_ui::TextureId;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};

/// The pictures' folder, in the profile folder.
pub(crate) const DIR: &str = "wheel_icons";
/// Which command shows which picture, in the profile folder.
pub(crate) const FILE: &str = "wheel_icons.json";
/// Largest picture file taken, in bytes.
pub(crate) const MAX_BYTES: u64 = 4 * 1024 * 1024;
/// Longest side, in pixels, of a picture taken.
pub(crate) const MAX_SIDE: u32 = 4_096;
/// Most pictures the atlas shows at once.
pub(crate) const CELLS: usize = CUSTOM_WHEEL_ICON_CELLS;

/// The commands' pictures, and where they are kept.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Pictures {
    /// Each command (trimmed) and its picture's file name in [`DIR`].
    by_command: BTreeMap<String, String>,
    /// The profile folder; `None` for pictures never saved (tests, defaults).
    directory: Option<PathBuf>,
}

impl Pictures {
    /// The pictures kept in `directory`; none without the file.
    pub(crate) fn load(directory: &Path) -> Self {
        let by_command = std::fs::read(directory.join(FILE))
            .ok()
            .and_then(|bytes| parse(&bytes))
            .unwrap_or_default();
        let pictures = Self {
            by_command,
            directory: Some(directory.to_path_buf()),
        };
        pictures.publish();
        pictures
    }

    /// The picture file of `command`, if it has one.
    pub(crate) fn get(&self, command: &str) -> Option<&str> {
        self.by_command.get(command.trim()).map(String::as_str)
    }

    /// Give `command` the picture `file` (a name [`import`] returned), or none; kept at
    /// once and shown from the next frame.
    pub(crate) fn set(&mut self, command: &str, file: Option<&str>) {
        let command = command.trim();
        if command.is_empty() {
            return;
        }
        let changed = match file {
            Some(file) => {
                self.by_command.insert(command.to_owned(), file.to_owned()) != Some(file.to_owned())
            }
            None => self.by_command.remove(command).is_some(),
        };
        if changed {
            self.save();
            self.publish();
        }
    }

    /// Whether `file` may be given to `command`: it is one of the pictures in use, or
    /// fewer than [`CELLS`] different ones are (`command`'s own picture, which `file`
    /// would replace, aside).
    pub(crate) fn has_room_for(&self, command: &str, file: &str) -> bool {
        let command = command.trim();
        let mut files: Vec<&str> = self
            .by_command
            .iter()
            .filter(|(owner, _)| owner.as_str() != command)
            .map(|(_, file)| file.as_str())
            .collect();
        files.sort_unstable();
        files.dedup();
        files.contains(&file) || files.len() < CELLS
    }

    /// The profile folder the pictures are kept in.
    pub(crate) fn directory(&self) -> Option<&Path> {
        self.directory.as_deref()
    }

    fn save(&self) {
        let Some(directory) = &self.directory else {
            return;
        };
        let text = serde_json::to_string_pretty(&json!({"version": 1, "icons": self.by_command}))
            .unwrap_or_default();
        let path = directory.join(FILE);
        if let Err(error) = std::fs::write(&path, text) {
            crate::log::progress(format_args!("wheel pictures: {}: {error}", path.display()));
        }
    }

    /// Hand the pictures to the atlas ([`service`]).
    fn publish(&self) {
        let mut state = lock();
        state.directory = self.directory.clone();
        state.by_command = self
            .by_command
            .iter()
            .map(|(command, file)| (command.clone(), file.clone()))
            .collect();
    }
}

/// The table a `wheel_icons.json` holds; `None` when it is not one. Only plain file
/// names of [`file_name`]'s form are taken, so the file cannot point outside [`DIR`].
fn parse(bytes: &[u8]) -> Option<BTreeMap<String, String>> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    let icons = value.get("icons")?.as_object()?;
    Some(
        icons
            .iter()
            .filter_map(|(command, file)| {
                let file = file.as_str()?;
                (is_picture_name(file) && !command.trim().is_empty())
                    .then(|| (command.trim().to_owned(), file.to_owned()))
            })
            .collect(),
    )
}

/// The name a kept picture of `bytes` gets: 16 hex digits of its FNV-1a hash and `.png`.
fn file_name(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("{hash:016x}.png")
}

/// Whether `name` is a kept picture's name ([`file_name`]).
fn is_picture_name(name: &str) -> bool {
    name.strip_suffix(".png")
        .is_some_and(|stem| stem.len() == 16 && stem.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Why a picture is not taken, as the form says it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Refused {
    /// Not a `.png`, `.jpg` or `.jpeg` file.
    Kind,
    /// Larger than [`MAX_BYTES`].
    TooLarge,
    /// Wider or taller than [`MAX_SIDE`].
    TooBig,
    /// It could not be read or decoded, or the copy not written.
    Unreadable(String),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Kind => write!(formatter, "Only PNG and JPEG pictures"),
            Self::TooLarge => write!(formatter, "Too large: 4 MB at most"),
            Self::TooBig => write!(formatter, "Too big: {MAX_SIDE} pixels a side at most"),
            Self::Unreadable(why) => write!(formatter, "Not read: {why}"),
        }
    }
}

/// Take the picture at `source`: check it, fit it into an icon and keep it in `directory`'s
/// [`DIR`]; its file name there.
pub(crate) fn import(directory: &Path, source: &Path) -> Result<String, Refused> {
    let extension = source
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    if !matches!(extension.as_deref(), Some("png" | "jpg" | "jpeg")) {
        return Err(Refused::Kind);
    }
    let size = std::fs::metadata(source)
        .map_err(|error| Refused::Unreadable(error.to_string()))?
        .len();
    if size > MAX_BYTES {
        return Err(Refused::TooLarge);
    }
    let bytes = std::fs::read(source).map_err(|error| Refused::Unreadable(error.to_string()))?;
    let name = source.to_string_lossy();
    let image = crate::decode_image(&bytes, &name)
        .map_err(|error| Refused::Unreadable(error.to_string()))?;
    if image.width() > MAX_SIDE || image.height() > MAX_SIDE {
        return Err(Refused::TooBig);
    }
    let icon = crate::emote_icons::fit(&image.into_rgba8());
    let mut png = Vec::new();
    icon.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|error| Refused::Unreadable(error.to_string()))?;
    let folder = directory.join(DIR);
    std::fs::create_dir_all(&folder).map_err(|error| Refused::Unreadable(error.to_string()))?;
    let file = file_name(&png);
    std::fs::write(folder.join(&file), &png)
        .map_err(|error| Refused::Unreadable(error.to_string()))?;
    Ok(file)
}

/// What the atlas shows.
#[derive(Default)]
struct State {
    directory: Option<PathBuf>,
    by_command: HashMap<String, String>,
    /// A picture the custom choice's form shows before it is saved.
    preview: Option<String>,
    /// The files last put into the atlas, in cell order, and the renderer they went into.
    served: Vec<String>,
    renderer: Option<u64>,
    /// The cell of each file in the atlas.
    cells: HashMap<String, usize>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn lock() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The picture of the choice running `command`, when it has one in the atlas.
pub(crate) fn for_command(command: &str) -> Option<TextureId> {
    let state = lock();
    let file = state.by_command.get(command.trim())?;
    state.cells.get(file).copied().map(custom_wheel_icon)
}

/// The picture kept as `file`, when it is in the atlas.
pub(crate) fn for_file(file: &str) -> Option<TextureId> {
    lock().cells.get(file).copied().map(custom_wheel_icon)
}

/// Show `file` too (the form's picture before it is saved), or stop.
pub(crate) fn set_preview(file: Option<&str>) {
    lock().preview = file.map(str::to_owned);
}

/// Put the pictures into `renderer`'s atlas; nothing happens while they are already
/// there.
pub(crate) fn service(renderer: &ShapeRenderer, queue: &crate::frame_queue::FrameQueue) {
    let mut state = lock();
    let mut wanted: Vec<String> = state.preview.iter().cloned().collect();
    let mut files: Vec<&String> = state.by_command.values().collect();
    files.sort();
    files.dedup();
    for file in files {
        if !wanted.contains(file) {
            wanted.push(file.clone());
        }
    }
    wanted.truncate(CELLS);
    if state.served == wanted && state.renderer == Some(renderer.id()) {
        return;
    }
    state.cells.clear();
    if let Some(directory) = state.directory.clone() {
        for (cell, file) in wanted.iter().enumerate() {
            let path = directory.join(DIR).join(file);
            let decoded = std::fs::read(&path)
                .map_err(|error| error.to_string())
                .and_then(|bytes| {
                    crate::decode_image(&bytes, file).map_err(|error| error.to_string())
                });
            match decoded {
                Ok(image) => {
                    let icon = crate::emote_icons::fit(&image.into_rgba8());
                    renderer.upload_icon(queue, custom_wheel_icon(cell), icon.as_raw());
                    state.cells.insert(file.clone(), cell);
                }
                Err(why) => {
                    crate::log::progress(format_args!("wheel pictures: {}: {why}", path.display()));
                }
            }
        }
    }
    state.served = wanted;
    state.renderer = Some(renderer.id());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(directory: &Path, name: &str, width: u32, height: u32) -> PathBuf {
        let path = directory.join(name);
        image::RgbaImage::from_pixel(width, height, image::Rgba([200, 40, 40, 255]))
            .save(&path)
            .unwrap();
        path
    }

    #[test]
    fn a_picture_is_fitted_kept_and_named_by_its_content() {
        let directory = tempfile::tempdir().unwrap();
        let source = picture(directory.path(), "gg.png", 300, 150);
        let file = import(directory.path(), &source).expect("taken");
        assert!(is_picture_name(&file), "{file}");
        let kept = image::open(directory.path().join(DIR).join(&file)).unwrap();
        assert_eq!(
            (kept.width(), kept.height()),
            (crate::ui_renderer::ICON_SIZE, crate::ui_renderer::ICON_SIZE)
        );
        // The same picture again is the same file.
        assert_eq!(import(directory.path(), &source), Ok(file.clone()));
        // A JPEG is taken too.
        let jpeg = directory.path().join("gg.jpg");
        image::RgbImage::from_pixel(64, 64, image::Rgb([10, 200, 10]))
            .save(&jpeg)
            .unwrap();
        assert!(import(directory.path(), &jpeg).is_ok());
    }

    #[test]
    fn other_files_and_huge_pictures_are_refused() {
        let directory = tempfile::tempdir().unwrap();
        let text = directory.path().join("notes.txt");
        std::fs::write(&text, "hello").unwrap();
        assert_eq!(import(directory.path(), &text), Err(Refused::Kind));
        let fake = directory.path().join("fake.png");
        std::fs::write(&fake, "not a picture").unwrap();
        assert!(matches!(
            import(directory.path(), &fake),
            Err(Refused::Unreadable(_))
        ));
        let wide = picture(directory.path(), "wide.png", MAX_SIDE + 1, 1);
        assert_eq!(import(directory.path(), &wide), Err(Refused::TooBig));
        let large = directory.path().join("large.png");
        std::fs::write(&large, vec![0_u8; MAX_BYTES as usize + 1]).unwrap();
        assert_eq!(import(directory.path(), &large), Err(Refused::TooLarge));
    }

    #[test]
    fn the_table_is_kept_by_command_and_only_takes_its_own_names() {
        let directory = tempfile::tempdir().unwrap();
        let mut pictures = Pictures::load(directory.path());
        assert_eq!(pictures.get("say gg"), None);
        pictures.set(" say gg ", Some("0123456789abcdef.png"));
        let loaded = Pictures::load(directory.path());
        assert_eq!(loaded.get("say gg"), Some("0123456789abcdef.png"));
        // Room for [`CELLS`] different pictures; one in use is always taken again.
        let mut full = Pictures::default();
        for index in 0..CELLS {
            full.by_command
                .insert(format!("say {index}"), format!("{index:016x}.png"));
        }
        assert!(!full.has_room_for("say new", "ffffffffffffffff.png"));
        assert!(full.has_room_for("say new", &format!("{:016x}.png", 0)));
        assert!(
            full.has_room_for("say 3", "ffffffffffffffff.png"),
            "its own is replaced"
        );
        let mut loaded = loaded;
        loaded.set("say gg", None);
        assert_eq!(Pictures::load(directory.path()).get("say gg"), None);
        let table = parse(br#"{"icons": {"a": "../../evil.png", "b": "0123456789ABCDEF.png", " ": "0123456789abcdef.png"}}"#)
            .expect("a table");
        assert_eq!(table.len(), 1, "{table:?}");
        assert!(table.contains_key("b"));
    }
}
