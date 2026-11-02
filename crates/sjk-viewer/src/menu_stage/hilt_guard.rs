//! Keeps a saved hilt that closes the game from closing it at every start (#214).
//!
//! The player stage shows the hilts the profile saves (`saber1`, `saber2`) from the
//! first menu on, so a hilt whose loading ends the process would end every later start
//! too. Before a hilt is loaded its name goes in `crashes/stage-hilt.txt`; it is removed
//! when the load returns, failed or not. A file still there at the next start names the
//! hilt the game closed on: that session leaves it out (the hand holds
//! [`DEFAULT_HILT`]) and logs why. The file is read and removed once a session, so
//! the hilt is tried again at the start after.

use std::path::Path;
use std::sync::OnceLock;

/// The file, in the crash reports' folder.
const FILE: &str = "stage-hilt.txt";
/// The hilt a hand holds when its own cannot be shown: retail's first single saber.
pub(super) const DEFAULT_HILT: &str = "single_1";

/// The hilt the last session closed while loading, read once a session.
static CRASHED: OnceLock<Option<String>> = OnceLock::new();

/// Whether `name` is the hilt the last session closed while loading. The first call
/// reads (and removes) what that session left in `directory`, and logs it.
pub(super) fn crashed_on(directory: Option<&Path>, name: &str) -> bool {
    CRASHED
        .get_or_init(|| {
            let left = directory.and_then(take);
            if let Some(hilt) = &left {
                crate::log::progress(format_args!(
                    "player stage: the game closed while loading saber {hilt} last time; \
                     showing {DEFAULT_HILT} this session"
                ));
            }
            left
        })
        .as_deref()
        .is_some_and(|hilt| hilt.eq_ignore_ascii_case(name))
}

/// Note that `name` is being loaded.
pub(super) fn begin(directory: Option<&Path>, name: &str) {
    if let Some(directory) = directory {
        let _ = std::fs::write(directory.join(FILE), name);
    }
}

/// Note that the load returned.
pub(super) fn end(directory: Option<&Path>) {
    if let Some(directory) = directory {
        let _ = std::fs::remove_file(directory.join(FILE));
    }
}

/// The hilt named in `directory`'s file, removing the file.
fn take(directory: &Path) -> Option<String> {
    let path = directory.join(FILE);
    let text = std::fs::read_to_string(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    let name = text.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hilt_left_loading_is_named_once_and_a_finished_one_not_at_all() {
        let directory = std::env::temp_dir().join(format!("sjk-hilt-guard-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        begin(Some(&directory), "empty_stance");
        end(Some(&directory));
        assert_eq!(
            take(&directory),
            None,
            "a load that returned leaves nothing"
        );
        begin(Some(&directory), "empty_stance");
        // The process ended here; the next start finds the name, once.
        assert_eq!(take(&directory).as_deref(), Some("empty_stance"));
        assert_eq!(take(&directory), None);
        let _ = std::fs::remove_dir_all(&directory);
    }
}
