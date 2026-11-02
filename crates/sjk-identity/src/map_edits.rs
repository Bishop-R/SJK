//! Map edits (`PROTOCOL.md`, "Map edits"): SJK's client-side fixes to maps, which staff
//! publish to the hub and every SJK client draws. The identity worker keeps a copy of
//! each map's document in a folder beside `identity.key` ([`FOLDER`]), one
//! [`file_name`] per map and BSP checksum, and the viewer reads the loaded map's.
//!
//! The hub stores each document's `edits` as given; their meaning is the viewer's
//! (`map_edits::document`). A copy changed on this PC and not yet published
//! (`"unpublished": true`) is never overwritten or removed here.

use crate::hub::{Hub, HubError};
use crate::keys::Identity;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

/// The folder's name, in the settings folder beside `identity.key`.
pub const FOLDER: &str = "map_edits";

/// One document the hub lists (`GET /v1/map-edits`).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct Entry {
    /// The map, as `mp/ffa3`.
    pub map: String,
    /// The BSP's checksum the edits are for.
    pub checksum: i32,
    /// Counts the document's changes on the hub.
    pub revision: u64,
    /// When it last changed, in Unix seconds.
    #[serde(default)]
    pub updated: u64,
}

/// The hub's answer to `GET /v1/map-edits`.
#[derive(Deserialize)]
struct List {
    maps: Vec<Entry>,
}

/// Read the hub's list.
pub(crate) fn list(answer: Value) -> Result<Vec<Entry>, HubError> {
    serde_json::from_value::<List>(answer)
        .map(|list| list.maps)
        .map_err(|error| HubError::Protocol(format!("map edits list: {error}")))
}

/// The file name of `map`'s document for `checksum`: `mp/ffa3` becomes
/// `mp_ffa3_-1234.json`.
pub fn file_name(map: &str, checksum: i32) -> String {
    let flat: String = map
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{flat}_{checksum}.json")
}

/// Whether `map` is a map name the hub takes: 1 to 64 of `a` to `z`, `0` to `9`, `_`,
/// `-`, `.` and `/`, without a leading `/` or a `..`.
pub fn valid_map(map: &str) -> bool {
    (1..=64).contains(&map.len())
        && map.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-./".contains(&byte)
        })
        && !map.starts_with('/')
        && !map.contains("..")
}

/// Why a publish or a look at the hub's list failed, in staff's words.
pub fn refusal(error: &HubError) -> String {
    let words = match error {
        HubError::Rejected { code, message, .. } => match code.as_str() {
            "not_staff" => "Only SJK staff publish map edits.",
            "not_registered" => "Your identity is not registered at the SJK hub yet.",
            "staff_quota" | "rate_limited" => "Too many requests: wait a moment.",
            "edits_too_large" | "too_large" => "Too many edits for the hub: 2048 at most, 256 KiB.",
            "bad_map" => "The hub takes no edits for a map of that name.",
            _ => return format!("The hub refused it ({code}): {message}"),
        },
        // A hub from before map edits has no such route.
        HubError::Protocol(why)
            if why.starts_with("status 404") || why.starts_with("status 405") =>
        {
            "This SJK hub does not keep map edits yet: it needs its map edits update."
        }
        HubError::Network(_) => "Cannot reach the SJK hub.",
        other => return other.to_string(),
    };
    words.to_owned()
}

/// What one look at the hub's list did.
#[derive(Debug, Default, PartialEq)]
pub struct Refresh {
    /// Documents the hub lists.
    pub listed: usize,
    /// Documents written or removed.
    pub changed: usize,
    /// Why the look fell short, if it did.
    pub problem: Option<String>,
}

impl Refresh {
    /// One line for the log: what the hub lists and what changed, or why it fell short.
    pub fn summary(&self) -> String {
        match &self.problem {
            Some(problem) => format!("not in step with the hub: {problem}"),
            None => format!(
                "{} map{} on the hub, {} cop{} changed",
                self.listed,
                if self.listed == 1 { "" } else { "s" },
                self.changed,
                if self.changed == 1 { "y" } else { "ies" }
            ),
        }
    }
}

/// A copy in the folder: its revision, and whether it holds unpublished work.
fn copy(path: &Path) -> Option<(String, i32, u64, bool)> {
    let value: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    Some((
        value.get("map")?.as_str()?.to_owned(),
        i32::try_from(value.get("checksum")?.as_i64()?).ok()?,
        value.get("revision").and_then(Value::as_u64).unwrap_or(0),
        value
            .get("unpublished")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    ))
}

fn write(path: &Path, value: &Value) -> std::io::Result<()> {
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, serde_json::to_string_pretty(value)?)?;
    std::fs::rename(&temporary, path)
}

/// Bring `dir` up to the hub's list: download each document newer than its copy, and
/// remove the published copies of documents the hub no longer lists.
pub fn refresh(hub: &mut dyn Hub, identity: &Identity, dir: &Path) -> Refresh {
    let entries = match hub.map_edits(identity) {
        Ok(entries) => entries,
        Err(error) => {
            return Refresh {
                problem: Some(refusal(&error)),
                ..Refresh::default()
            };
        }
    };
    if let Err(error) = std::fs::create_dir_all(dir) {
        return Refresh {
            problem: Some(format!("{}: {error}", dir.display())),
            ..Refresh::default()
        };
    }
    let mut refresh = Refresh {
        listed: entries.len(),
        ..Refresh::default()
    };
    let mut listed = BTreeSet::new();
    for entry in entries.iter().filter(|entry| valid_map(&entry.map)) {
        let name = file_name(&entry.map, entry.checksum);
        listed.insert(name.clone());
        let path = dir.join(&name);
        if copy(&path)
            .is_some_and(|(_, _, revision, unpublished)| unpublished || revision >= entry.revision)
        {
            continue;
        }
        let document = hub
            .map_edits_doc(identity, &entry.map, entry.checksum)
            .and_then(|answer| {
                answer
                    .get("edits")
                    .filter(|edits| edits.is_array())
                    .cloned()
                    .ok_or_else(|| HubError::Protocol("map edits without edits".to_owned()))
            });
        match document {
            Ok(edits) => {
                let value = json!({
                    "map": entry.map,
                    "checksum": entry.checksum,
                    "revision": entry.revision,
                    "unpublished": false,
                    "edits": edits,
                });
                match write(&path, &value) {
                    Ok(()) => refresh.changed += 1,
                    Err(error) => {
                        refresh.problem = Some(format!("{}: {error}", path.display()));
                    }
                }
            }
            Err(error) => refresh.problem = Some(refusal(&error)),
        }
    }
    // A published copy the hub no longer lists was deleted there.
    for item in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = item.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.ends_with(".json") || listed.contains(name) {
            continue;
        }
        if copy(&path).is_some_and(|(_, _, revision, unpublished)| !unpublished && revision > 0)
            && std::fs::remove_file(&path).is_ok()
        {
            refresh.changed += 1;
        }
    }
    refresh
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Fake {
        documents: BTreeMap<(String, i32), (u64, Value)>,
        reads: usize,
    }

    impl Hub for Fake {
        fn register(
            &mut self,
            _identity: &Identity,
            _name: Option<&str>,
        ) -> Result<crate::wire::Profile, HubError> {
            Err(HubError::Network("offline".into()))
        }
        fn set_bio(&mut self, _: &Identity, _: &str) -> Result<crate::wire::Profile, HubError> {
            Err(HubError::Network("offline".into()))
        }
        fn profile(&mut self, _: &str) -> Result<crate::wire::Profile, HubError> {
            Err(HubError::Network("offline".into()))
        }
        fn claim(
            &mut self,
            _: &Identity,
            _: &str,
            _: u8,
            _: &str,
            _: bool,
        ) -> Result<(), HubError> {
            Err(HubError::Network("offline".into()))
        }
        fn release(&mut self, _: &Identity, _: &str) -> Result<(), HubError> {
            Err(HubError::Network("offline".into()))
        }
        fn presence(&mut self, _: &str) -> Result<Vec<crate::wire::Presence>, HubError> {
            Err(HubError::Network("offline".into()))
        }
        fn map_edits(&mut self, _identity: &Identity) -> Result<Vec<Entry>, HubError> {
            Ok(self
                .documents
                .iter()
                .map(|((map, checksum), (revision, _))| Entry {
                    map: map.clone(),
                    checksum: *checksum,
                    revision: *revision,
                    updated: 0,
                })
                .collect())
        }
        fn map_edits_doc(
            &mut self,
            _identity: &Identity,
            map: &str,
            checksum: i32,
        ) -> Result<Value, HubError> {
            self.reads += 1;
            let (revision, edits) = &self.documents[&(map.to_owned(), checksum)];
            Ok(json!({"map": map, "checksum": checksum, "revision": revision, "edits": edits}))
        }
    }

    fn folder(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("sjk-map-edits-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn documents_follow_the_hub_and_unpublished_work_stays() {
        let dir = folder("follow");
        let identity = Identity::generate().unwrap();
        let mut hub = Fake::default();
        hub.documents.insert(
            ("mp/ffa3".into(), -5),
            (
                1,
                json!([{"id": 1, "kind": "remap", "from": "a", "to": "b"}]),
            ),
        );
        assert_eq!(refresh(&mut hub, &identity, &dir).changed, 1);
        let path = dir.join(file_name("mp/ffa3", -5));
        assert_eq!(copy(&path), Some(("mp/ffa3".into(), -5, 1, false)));
        // Up to date: not read again.
        assert_eq!(refresh(&mut hub, &identity, &dir).changed, 0);
        assert_eq!(hub.reads, 1);
        hub.documents.get_mut(&("mp/ffa3".into(), -5)).unwrap().0 = 2;
        assert_eq!(refresh(&mut hub, &identity, &dir).changed, 1);
        assert_eq!(copy(&path).unwrap().2, 2);
        // Staff's work on this PC is kept over a newer hub copy.
        let mut local: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        local["unpublished"] = json!(true);
        write(&path, &local).unwrap();
        hub.documents.get_mut(&("mp/ffa3".into(), -5)).unwrap().0 = 3;
        assert_eq!(refresh(&mut hub, &identity, &dir).changed, 0);
        assert_eq!(copy(&path).unwrap().2, 2);
        // ... and when the hub deletes the document.
        hub.documents.clear();
        assert_eq!(refresh(&mut hub, &identity, &dir).changed, 0);
        assert!(path.exists());
        local["unpublished"] = json!(false);
        write(&path, &local).unwrap();
        assert_eq!(refresh(&mut hub, &identity, &dir).changed, 1);
        assert!(!path.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refusals_are_worded_for_staff() {
        let rejected = |code: &str| HubError::Rejected {
            status: 403,
            code: code.to_owned(),
            message: "why".to_owned(),
        };
        assert_eq!(
            refusal(&rejected("not_staff")),
            "Only SJK staff publish map edits."
        );
        assert_eq!(
            refusal(&rejected("odd")),
            "The hub refused it (odd): why",
            "an unknown code keeps the hub's words"
        );
        // A hub from before map edits answers its default 404 page.
        assert_eq!(
            refusal(&HubError::Protocol("status 404, not JSON".to_owned())),
            "This SJK hub does not keep map edits yet: it needs its map edits update."
        );
        assert_eq!(
            refusal(&HubError::Network("timed out".to_owned())),
            "Cannot reach the SJK hub."
        );
        let off = "the SJK identity is off: turn it on with cl_identity 1";
        assert_eq!(refusal(&HubError::NotConnected(off.to_owned())), off);
    }

    #[test]
    fn a_refresh_sums_itself_up_for_the_log() {
        let done = Refresh {
            listed: 3,
            changed: 1,
            problem: None,
        };
        assert_eq!(done.summary(), "3 maps on the hub, 1 copy changed");
        let failed = Refresh {
            problem: Some("Cannot reach the SJK hub.".to_owned()),
            ..Refresh::default()
        };
        assert_eq!(
            failed.summary(),
            "not in step with the hub: Cannot reach the SJK hub."
        );
    }

    #[test]
    fn names_are_checked_and_files_named() {
        assert!(valid_map("mp/ffa3"));
        assert!(valid_map("jof_temple.v2"));
        assert!(!valid_map("/mp/ffa3"));
        assert!(!valid_map("mp/../x"));
        assert!(!valid_map("MP/FFA3"));
        assert!(!valid_map(""));
        assert_eq!(file_name("mp/FFA3", -1234), "mp_ffa3_-1234.json");
    }
}
