//! Noticing a config file changed by someone other than this shell.
//!
//! The shell holds every archived cvar and binding in memory and writes the whole
//! table back, so a `config.cfg` edited, replaced or deleted while the game runs
//! used to be overwritten with the values from before: the player's edit was lost
//! and an old name or camera came back at the next start. [`ConfigWatch`]
//! remembers the texts this shell loaded or wrote; a file on disk that is none of
//! them was changed from outside, and the shell then reads it rather than writing
//! over it ([`crate::Shell::autosave`]).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

/// How often the file's size and time are looked at while nothing is saved.
pub(crate) const CHECK_INTERVAL: Duration = Duration::from_secs(1);

/// Texts written in a row the background writer may still be landing: the disk
/// can hold any of them for a moment.
const KNOWN_TEXTS: usize = 8;

/// What happened to the file since this shell last saw its own text there.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum External {
    /// Another program (an editor, a copy) wrote it.
    Edited,
    /// The file was deleted.
    Removed,
}

/// Size and modification time: unchanged means the file was not rewritten.
type Stamp = (u64, SystemTime);

#[derive(Default)]
pub(crate) struct ConfigWatch {
    /// Hashes of the texts loaded or written, newest last.
    known: Vec<u64>,
    /// The file's stamp when it last held a known text; `None` while it is
    /// absent (a new profile) or before the first load.
    stamp: Option<Stamp>,
    /// Whether the file existed when last seen holding a known text.
    present: bool,
    /// Whether a load or save happened: until then nothing is watched.
    started: bool,
    checked: Option<Instant>,
}

impl ConfigWatch {
    /// The file now holds what the shell loaded from it (`bytes`, or nothing).
    pub(crate) fn loaded(&mut self, path: &Path, bytes: Option<&[u8]>) {
        self.known.clear();
        if let Some(bytes) = bytes {
            self.known.push(hash(bytes));
        }
        self.present = bytes.is_some();
        self.stamp = stamp(path);
        self.started = true;
    }

    /// The shell is writing `contents` to the file (now or on the writer thread).
    pub(crate) fn writing(&mut self, contents: &str) {
        let hash = hash(contents.as_bytes());
        self.known.retain(|known| *known != hash);
        self.known.push(hash);
        if self.known.len() > KNOWN_TEXTS {
            self.known.remove(0);
        }
        self.present = true;
        self.started = true;
    }

    /// Look at the file at most every [`CHECK_INTERVAL`] (`now`), or at once
    /// with `None`, and say whether someone else changed it.
    pub(crate) fn check(&mut self, path: &Path, now: Option<Instant>) -> Option<External> {
        if !self.started {
            return None;
        }
        if let Some(now) = now {
            if self
                .checked
                .is_some_and(|checked| now.saturating_duration_since(checked) < CHECK_INTERVAL)
            {
                return None;
            }
            self.checked = Some(now);
        }
        let current = stamp(path);
        if current.is_some() && current == self.stamp {
            return None;
        }
        let Ok(bytes) = std::fs::read(path) else {
            if !self.present {
                return None;
            }
            // Only once: a profile removed stays removed until something saves.
            self.present = false;
            self.stamp = None;
            return Some(External::Removed);
        };
        self.stamp = current;
        let text = hash(&bytes);
        if self.known.contains(&text) {
            return None;
        }
        self.present = true;
        self.known.clear();
        self.known.push(text);
        Some(External::Edited)
    }
}

fn stamp(path: &Path) -> Option<Stamp> {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.len(), metadata.modified().ok()?))
}

fn hash(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

/// A folder of its own under the system's temporary one, removed when dropped.
#[cfg(test)]
pub(crate) struct TestFolder(std::path::PathBuf);

#[cfg(test)]
impl TestFolder {
    pub(crate) fn new() -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "sjk-shell-config-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

#[cfg(test)]
impl Drop for TestFolder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_text_the_shell_did_not_write_is_external() {
        let directory = crate::config_watch::TestFolder::new();
        let path = directory.path().join("config.cfg");
        std::fs::write(&path, "seta name \"A\"\n").unwrap();
        let mut watch = ConfigWatch::default();
        assert_eq!(watch.check(&path, None), None, "nothing loaded yet");
        watch.loaded(&path, Some(b"seta name \"A\"\n"));
        assert_eq!(watch.check(&path, None), None);

        watch.writing("seta name \"B\"\n");
        std::fs::write(&path, "seta name \"B\"\n").unwrap();
        assert_eq!(watch.check(&path, None), None, "its own save");

        std::fs::write(&path, "seta name \"Edited\"\n").unwrap();
        assert_eq!(watch.check(&path, None), Some(External::Edited));
        assert_eq!(watch.check(&path, None), None, "said once");

        std::fs::remove_file(&path).unwrap();
        assert_eq!(watch.check(&path, None), Some(External::Removed));
        assert_eq!(watch.check(&path, None), None, "said once");
    }

    #[test]
    fn a_profile_without_a_file_is_not_removed() {
        let directory = crate::config_watch::TestFolder::new();
        let path = directory.path().join("config.cfg");
        let mut watch = ConfigWatch::default();
        watch.loaded(&path, None);
        assert_eq!(watch.check(&path, None), None);
    }

    #[test]
    fn checks_wait_for_the_interval() {
        let directory = crate::config_watch::TestFolder::new();
        let path = directory.path().join("config.cfg");
        std::fs::write(&path, "a").unwrap();
        let mut watch = ConfigWatch::default();
        watch.loaded(&path, Some(b"a"));
        let start = Instant::now();
        assert_eq!(watch.check(&path, Some(start)), None);
        std::fs::write(&path, "edited").unwrap();
        assert_eq!(watch.check(&path, Some(start)), None, "too soon");
        assert!(matches!(
            watch.check(&path, Some(start + CHECK_INTERVAL)),
            Some(External::Edited)
        ));
    }
}
