//! Which of the SJK team's announcements the chat has shown (`docs/hub-chat.md`,
//! "Announcements and reward rounds"): `announcements_seen.txt` in the settings folder,
//! beside `identity.key`, as `holocrons_seen.txt` is kept. Announcement numbers at the hub
//! only rise, so the file keeps the highest one shown; one with a higher number is new.
//! The file names the key it belongs to, so a new identity sees its own. With no file for
//! the key (a new install, a new PC) only the newest announcement shows, so a fresh install
//! is not flooded.
//!
//! The News panel (31/10/2026) adds `read`: the highest announcement number the player
//! has seen on the panel, so one above it opens the panel on its own. A file from before
//! counts what the chat showed as read; with no file, what the hub lists when it is
//! first read counts as read, so a fresh install opens nothing.
//!
//! ```text
//! # Announcements SJK has shown this identity: the highest announcement number shown.
//! key 0123456789abcdef
//! highest 12
//! read 11
//! ```

use sjk_identity::Announcement;
use std::path::Path;

/// The file, in the settings folder.
pub(crate) const FILE: &str = "announcements_seen.txt";
const HEADER: &str =
    "# Announcements SJK has shown this identity: the highest announcement number shown.";
/// The most announcements one reading puts into the chat.
pub(crate) const AT_ONCE: usize = 3;

/// What has been shown to one key.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Seen {
    key_id: String,
    highest: u64,
    /// A file for this key was read, or an announcement was marked since.
    known: bool,
    /// The highest announcement read on the News panel; `None` until the file or the
    /// first listing sets it.
    read: Option<u64>,
}

impl Seen {
    /// What `text` says was shown to `key_id`; nothing, and not known, when it is another
    /// key's or unreadable.
    pub(crate) fn parse(text: &str, key_id: &str) -> Self {
        let mut seen = Self {
            key_id: key_id.to_owned(),
            ..Self::default()
        };
        let mut lines = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'));
        if lines.next().and_then(|line| line.strip_prefix("key ")) != Some(key_id) {
            return seen;
        }
        seen.known = true;
        for line in lines {
            if let Some(number) = line
                .strip_prefix("highest ")
                .and_then(|number| number.trim().parse::<u64>().ok())
            {
                seen.highest = number;
            }
            if let Some(number) = line
                .strip_prefix("read ")
                .and_then(|number| number.trim().parse::<u64>().ok())
            {
                seen.read = Some(number);
            }
        }
        // A file from before the panel: what the chat showed counts as read.
        if seen.read.is_none() {
            seen.read = Some(seen.highest);
        }
        seen
    }

    /// The file `directory` holds for `key_id`.
    pub(crate) fn load(directory: &Path, key_id: &str) -> Self {
        let text = std::fs::read_to_string(directory.join(FILE)).unwrap_or_default();
        Self::parse(&text, key_id)
    }

    /// The key these numbers belong to.
    pub(crate) fn key_id(&self) -> &str {
        &self.key_id
    }

    /// The file's text.
    pub(crate) fn text(&self) -> String {
        let mut text = format!("{HEADER}\nkey {}\nhighest {}\n", self.key_id, self.highest);
        if let Some(read) = self.read {
            text.push_str(&format!("read {read}\n"));
        }
        text
    }

    /// The highest announcement read on the News panel, given the hub's `list`: with
    /// nothing recorded (a new key), everything listed counts as read, so a fresh install
    /// opens nothing. The flag says it was decided just now (the file wants saving).
    pub(crate) fn read_given(&mut self, list: &[Announcement]) -> (u64, bool) {
        if let Some(read) = self.read {
            return (read, false);
        }
        // Nothing listed yet: it is decided when the list comes.
        let Some(newest) = list.iter().map(|announcement| announcement.id).max() else {
            return (u64::MAX, false);
        };
        self.read = Some(newest);
        self.known = true;
        (newest, true)
    }

    /// Count announcement `id`, and everything before it, as read on the News panel;
    /// true when that changed the file.
    pub(crate) fn mark_read(&mut self, id: u64) -> bool {
        let read = self.read.unwrap_or(0).max(id);
        let changed = self.read != Some(read);
        self.read = Some(read);
        self.known = true;
        changed
    }

    /// Write the file into `directory`.
    pub(crate) fn save(&self, directory: &Path) -> std::io::Result<()> {
        std::fs::write(directory.join(FILE), self.text())
    }

    /// Count announcement `id`, and everything before it, as shown.
    pub(crate) fn mark(&mut self, id: u64) {
        self.highest = self.highest.max(id);
        self.known = true;
    }

    /// Of `list` (newest first, as the hub lists them), the announcements to put into the
    /// chat now, oldest first: the newest [`AT_ONCE`] not shown yet, or with no file for
    /// this key only the newest one.
    pub(crate) fn to_show(&self, list: &[Announcement]) -> Vec<Announcement> {
        let most = if self.known { AT_ONCE } else { 1 };
        let mut new: Vec<Announcement> = list
            .iter()
            .filter(|announcement| announcement.id > self.highest)
            .take(most)
            .cloned()
            .collect();
        new.reverse();
        new
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(ids: &[u64]) -> Vec<Announcement> {
        ids.iter()
            .map(|id| Announcement {
                id: *id,
                ..Announcement::default()
            })
            .collect()
    }

    fn ids(shown: &[Announcement]) -> Vec<u64> {
        shown.iter().map(|announcement| announcement.id).collect()
    }

    #[test]
    fn a_fresh_install_shows_only_the_newest_then_up_to_three_new_ones() {
        let mut seen = Seen::parse("", "aa");
        assert_eq!(ids(&seen.to_show(&list(&[9, 7, 5, 2]))), [9]);
        seen.mark(9);
        assert!(seen.to_show(&list(&[9, 7])).is_empty());
        assert_eq!(
            ids(&seen.to_show(&list(&[14, 13, 12, 11, 10, 9]))),
            [12, 13, 14],
            "the newest three, oldest first"
        );
    }

    /// The News panel's read mark: a new key counts what the hub lists first as read, so
    /// a fresh install opens nothing; marking keeps the highest and is saved.
    #[test]
    fn the_read_mark_starts_at_the_first_listing_and_rises() {
        let directory = tempfile::tempdir().expect("a folder");
        let mut seen = Seen::load(directory.path(), "aa");
        assert_eq!(
            seen.read_given(&[]),
            (u64::MAX, false),
            "nothing to decide yet"
        );
        assert_eq!(seen.read_given(&list(&[7, 5])), (7, true));
        assert_eq!(seen.read_given(&list(&[9, 7])), (7, false));
        assert!(seen.mark_read(9));
        assert!(!seen.mark_read(8), "never goes down");
        seen.save(directory.path()).expect("saved");
        let loaded = Seen::load(directory.path(), "aa");
        assert_eq!(loaded.read, Some(9));
        assert_eq!(loaded, seen);
    }

    #[test]
    fn the_file_round_trips_and_belongs_to_one_key() {
        let directory = tempfile::tempdir().expect("a folder");
        let mut seen = Seen::load(directory.path(), "aa");
        seen.mark(12);
        seen.mark(3);
        seen.save(directory.path()).expect("saved");
        let loaded = Seen::load(directory.path(), "aa");
        assert_eq!(loaded.highest, seen.highest);
        // A file without `read` (from before the News panel) counts what was shown as read.
        assert_eq!(loaded.read, Some(12));
        assert_eq!(loaded.key_id(), "aa");
        assert_eq!(ids(&loaded.to_show(&list(&[13, 12]))), [13]);
        let other = Seen::load(directory.path(), "bb");
        assert_eq!(
            ids(&other.to_show(&list(&[13, 12]))),
            [13],
            "another key's file"
        );
        assert_eq!(
            Seen::parse("key aa\nhighest many\n", "aa")
                .to_show(&list(&[1]))
                .len(),
            1
        );
    }
}
