//! JoF content: the JoF community's maps, skins, hilts and packs, fetched from
//! JoF's own repository as JoF's launcher does (`docs/jof-content.md`).
//!
//! The repository publishes one manifest: groups of PK3 files, each with its
//! size and MD5 ([`manifest`]). SJK keeps what it downloads in a folder of its
//! own (`<root>/base`), never in the game's installation, and checks every file
//! against the manifest before it is used ([`download`]). A file the game folder
//! already holds with the same MD5 (JoF's launcher puts them in `GameData/base`)
//! is not fetched again ([`store`]).
//!
//! This crate knows nothing of the client: [`job`] runs a refresh or a sync on
//! a thread of its own and reports through a shared [`job::Progress`].

pub mod download;
pub mod job;
pub mod manifest;
pub mod plan;
pub mod source;
pub mod store;

pub use manifest::{Asset, Group, GroupKind, Manifest, Target};
pub use plan::{AutoGroups, Plan, Selection};
pub use source::{Directory, Source, SourceChoice};
pub use store::{AssetState, Snapshot, Store};

/// Lower-case hexadecimal of an MD5 digest, as the manifest writes it.
pub fn md5_hex(digest: &[u8; 16]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// An MD5 digest from its 32 hexadecimal digits (either case).
pub fn parse_md5(text: &str) -> Option<[u8; 16]> {
    let text = text.trim();
    if text.len() != 32 || !text.is_ascii() {
        return None;
    }
    let mut digest = [0_u8; 16];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(digest)
}

/// "1.4 GB", "75 MB", "820 KB": a size as the card shows it (decimal units, as
/// the launcher and file managers count downloads).
pub fn human_size(bytes: u64) -> String {
    const KB: f64 = 1000.0;
    let value = bytes as f64;
    if value >= KB * KB * KB {
        format!("{:.1} GB", value / (KB * KB * KB))
    } else if value >= KB * KB {
        format!("{:.0} MB", value / (KB * KB))
    } else if value >= KB {
        format!("{:.0} KB", value / KB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_text_round_trips_and_rejects_bad_digits() {
        let digest = parse_md5("7D26FB356D81EF10403F290D504517DF").unwrap();
        assert_eq!(md5_hex(&digest), "7d26fb356d81ef10403f290d504517df");
        assert!(parse_md5("7d26").is_none());
        assert!(parse_md5("zz26fb356d81ef10403f290d504517df").is_none());
    }

    #[test]
    fn sizes_read_as_the_card_shows_them() {
        assert_eq!(human_size(4_638_000_000), "4.6 GB");
        assert_eq!(human_size(75_400_000), "75 MB");
        assert_eq!(human_size(820_000), "820 KB");
        assert_eq!(human_size(12), "12 B");
    }
}
