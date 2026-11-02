//! GIFs the player chose to hide (`docs/hub-chat.md`, "Hidden GIFs"): ids kept on this
//! PC in `gif-hidden.txt` beside `config.cfg`, one a line, newest first, in the
//! favourites' format ([`super::favourites`]), read at start and written back at every
//! change. Nothing is sent anywhere.
//!
//! A hidden GIF never shows ([`super::shows`]): its line reads "GIF hidden"
//! ([`super::link::HIDDEN_LABEL`]) in place of the link, no picture is drawn and it is
//! never fetched, in the SJK chat page, the docks, the chat feed and the picker's other
//! tabs. The chosen message's "Hide this GIF" button adds one; the chosen message's
//! "Show this GIF again", the picker's Hidden tab (which alone shows their pictures: a
//! tile chosen, then its Show this GIF again) or `gif_unhideall` take them back.

use super::GifId;
use super::favourites::{Favourites, load_file, save_file};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// The list's file, beside `config.cfg`.
pub(crate) const FILE: &str = "gif-hidden.txt";
/// The console command showing every hidden GIF again.
pub(crate) const UNHIDE_ALL: &str = "gif_unhideall";
/// [`UNHIDE_ALL`]'s help line.
pub(crate) const UNHIDE_ALL_HELP: &str = "Show every chat GIF you hid again";

/// What [`UNHIDE_ALL`] says after showing `count` GIFs again.
pub(crate) fn unhide_all_reply(count: usize) -> String {
    match count {
        0 => "No GIFs were hidden.".to_owned(),
        1 => "1 hidden GIF shows again.".to_owned(),
        n => format!("{n} hidden GIFs show again."),
    }
}

struct State {
    list: Favourites,
    /// Where the list is kept, once read.
    path: Option<PathBuf>,
    /// Counts changes, so lines already worded are worded again.
    revision: u64,
}

static STATE: Mutex<State> = Mutex::new(State {
    list: Favourites::new(),
    path: None,
    revision: 0,
});

fn lock() -> MutexGuard<'static, State> {
    STATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Read the list from `directory` (the settings folder), once.
pub(crate) fn load(directory: &Path) {
    let mut state = lock();
    if state.path.is_some() {
        return;
    }
    let path = directory.join(FILE);
    match load_file(&path) {
        Ok(list) => {
            state.list = list;
            state.revision += 1;
        }
        Err(error) => crate::log::progress(format_args!("Hidden GIFs: {error}")),
    }
    state.path = Some(path);
}

/// The hidden GIFs, newest first (the GIF picker's Hidden tab).
pub(crate) fn list() -> Vec<GifId> {
    lock().list.ids().to_vec()
}

/// Whether the player hid `id`.
pub(crate) fn contains(id: &GifId) -> bool {
    lock().list.contains(id)
}

/// Hide `id` or show it again, and keep the list.
pub(crate) fn set(id: &GifId, hidden: bool) {
    let mut state = lock();
    if !state.list.set(id, hidden) {
        return;
    }
    state.revision += 1;
    save(&state);
}

/// Show every hidden GIF again; returns how many were hidden.
pub(crate) fn clear() -> usize {
    let mut state = lock();
    let count = state.list.ids().len();
    if count == 0 {
        return 0;
    }
    state.list = Favourites::new();
    state.revision += 1;
    save(&state);
    count
}

fn save(state: &State) {
    if let Some(path) = &state.path
        && let Err(error) = save_file(path, &state.list)
    {
        crate::log::progress(format_args!("Hidden GIFs: could not save: {error}"));
    }
}

/// Counts changes to the list.
pub(crate) fn revision() -> u64 {
    lock().revision
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_gifs_are_kept_in_their_own_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(FILE);
        let mut list = Favourites::new();
        list.set(&GifId::new("Hid0001xyz").unwrap(), true);
        save_file(&path, &list).unwrap();
        assert_eq!(load_file(&path).unwrap(), list);
        assert_ne!(FILE, super::super::favourites::FILE);
    }

    /// A hidden GIF reads "GIF hidden" and has no picture; shown again, it is "GIF" with
    /// its picture. Its buttons stay. The count's reply reads well.
    #[test]
    fn a_hidden_gif_reads_gif_hidden_and_is_never_shown() {
        let id = GifId::new("HiddenTest1xyz").unwrap();
        let text = "look https://giphy.com/gifs/HiddenTest1xyz";
        assert!(super::super::shows(&id));
        assert_eq!(super::super::link::with_label(text).1, Some(id.clone()));
        let before = super::super::shown_revision();
        set(&id, true);
        assert!(contains(&id));
        assert_ne!(
            super::super::shown_revision(),
            before,
            "lines are worded again"
        );
        assert!(!super::super::shows(&id));
        assert_eq!(
            super::super::link::with_label(text),
            ("look GIF hidden".to_owned(), None)
        );
        assert_eq!(
            super::super::chosen_gif(text, false),
            Some(id.clone()),
            "its buttons stay"
        );
        assert_eq!(
            super::super::chosen_gif(text, true),
            None,
            "not for a muted sender"
        );
        set(&id, false);
        assert!(super::super::shows(&id));
        assert_eq!(unhide_all_reply(0), "No GIFs were hidden.");
        assert_eq!(unhide_all_reply(1), "1 hidden GIF shows again.");
        assert_eq!(unhide_all_reply(3), "3 hidden GIFs show again.");
    }
}
