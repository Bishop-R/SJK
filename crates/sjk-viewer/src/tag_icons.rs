//! The tags' pictures in the UI atlas (`tags.rs`): each tag's `gfx/tags/<id>.png`, from
//! the hub's `sjk_tags` pack, fitted into its own cell ([`crate::ui_renderer::tag_icon`],
//! the catalogue's order) for the Collection's Tags tab and the holocron prizes. They are
//! read from the hub's packs as last mounted (`sjk_packs.rs`, so a pack downloaded during
//! play shows at once) and go in again into each new renderer's atlas (a map change, a
//! connection), as the emotes' do (`emote_icons.rs`). A tag whose picture is missing (no
//! pack yet) has no cell and is drawn as its name.

use crate::ui_renderer::{ShapeRenderer, TAG_ICON_CELLS, tag_icon};
use sjk_ui::TextureId;
use std::sync::{Mutex, PoisonError};

const _: () = assert!(crate::tags::CATALOGUE.len() <= TAG_ICON_CELLS);

#[derive(Default)]
struct Icons {
    /// The renderer ([`ShapeRenderer::id`]) the pictures went into, and the packs'
    /// generation then: a new mount (the tags' pack, downloaded late) tries again.
    served: Option<(u64, u64)>,
    /// Which tags (by catalogue place) have their picture in the atlas.
    loaded: Vec<bool>,
}

static ICONS: Mutex<Icons> = Mutex::new(Icons {
    served: None,
    loaded: Vec::new(),
});

fn lock() -> std::sync::MutexGuard<'static, Icons> {
    ICONS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The picture of tag `id`, when it is in the atlas.
pub(crate) fn texture(id: &str) -> Option<TextureId> {
    let place = crate::tags::CATALOGUE.iter().position(|tag| tag.id == id)?;
    lock()
        .loaded
        .get(place)
        .copied()
        .unwrap_or(false)
        .then(|| tag_icon(place))
}

/// Put the tags' pictures into `renderer`'s atlas; nothing happens while they are there
/// already and no pack came since.
pub(crate) fn service(renderer: &ShapeRenderer, queue: &crate::frame_queue::FrameQueue) {
    let generation = crate::sjk_packs::generation();
    let mut icons = lock();
    if icons.served == Some((renderer.id(), generation)) {
        return;
    }
    let skins = crate::sjk_packs::skins();
    let vfs = skins.packs();
    icons.loaded = crate::tags::CATALOGUE
        .iter()
        .enumerate()
        .map(|(place, tag)| {
            let path = tag.picture();
            let Ok(Some(asset)) = vfs.read(&path) else {
                return false;
            };
            match crate::decode_image(&asset.bytes, &path) {
                Ok(image) => {
                    let cell = crate::emote_icons::fit(&image.into_rgba8());
                    renderer.upload_icon(queue, tag_icon(place), cell.as_raw());
                    true
                }
                Err(why) => {
                    crate::log::progress(format_args!("tags: {path}: {why}"));
                    false
                }
            }
        })
        .collect();
    icons.served = Some((renderer.id(), generation));
}
