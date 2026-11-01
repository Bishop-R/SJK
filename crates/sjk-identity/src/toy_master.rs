//! The Toy Master's secrets (the hub's `PROTOCOL.md`, "Secrets", since 30/10/2026): toys
//! SJK's creator hides on maps for players to find. The hub keeps where they stand and
//! everything that makes each one what it is (its model, its sound, its show), and sends
//! a toy's look only with the toys placed; this crate reads the list, says how many times
//! the player pressed Use on one, and carries the creator's placing. The viewer draws
//! them (`sjk-viewer`'s `toy_master.rs`).

use crate::hub::HubError;
use crate::wire::Profile;
use serde::{Deserialize, Serialize};

/// The toy id of the feed event the hub makes when a toy plays its show.
pub const TOY: &str = "secrets";

/// How a toy's show plays, in milliseconds from its start: it shrinks away over
/// `vanish`, stays gone for `gone`, pops back over `pop` and spins for `spin`, at up to
/// `turns` turns a second.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Show {
    /// How long it shrinks away.
    pub vanish: u32,
    /// How long it stays gone.
    pub gone: u32,
    /// How long it takes to pop back.
    pub pop: u32,
    /// How long it spins.
    pub spin: u32,
    /// Its fastest spin, in turns a second.
    pub turns: f32,
}

/// One placed toy, with how it looks and plays.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Secret {
    /// The hub's id, never given again.
    pub id: i64,
    /// The map's name as the game gives it (`mp/ffa3`).
    pub map: String,
    /// The map's BSP checksum: a toy stands in one exact file.
    pub checksum: i32,
    /// `ip:port` for one server only, empty for every server.
    #[serde(default)]
    pub server: String,
    /// Where it stands, in game units.
    pub origin: [f32; 3],
    /// Facing, in degrees.
    pub yaw: f32,
    /// The model drawn (from the hub's asset packs).
    #[serde(default)]
    pub model: String,
    /// How much the model is grown.
    #[serde(default = "one")]
    pub scale: f32,
    /// The sound its show plays (empty for none; not every player has it).
    #[serde(default)]
    pub sound: String,
    /// How its show plays.
    #[serde(default)]
    pub show: Show,
    /// The kind it is, sent to SJK's creator only.
    #[serde(default)]
    pub kind: Option<String>,
}

fn one() -> f32 {
    1.0
}

impl Secret {
    /// Whether the toy stands on map `map` (any case) of checksum `checksum`, on `server`.
    pub fn stands_on(&self, map: &str, checksum: i32, server: Option<&str>) -> bool {
        self.checksum == checksum
            && self.map.eq_ignore_ascii_case(map)
            && (self.server.is_empty() || Some(self.server.as_str()) == server)
    }
}

/// `GET /v1/secrets`: every placed toy; for SJK's creator also the kinds there are.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Secrets {
    /// Every placed toy.
    pub secrets: Vec<Secret>,
    /// The kinds of toy the creator may place (left out for anyone else).
    #[serde(default)]
    pub kinds: Vec<String>,
}

impl Secrets {
    /// Whether any toy stands on a map called `map` (any checksum), so the viewer knows
    /// to read the loaded map's checksum.
    pub fn on_map(&self, map: &str) -> bool {
        self.secrets
            .iter()
            .any(|placed| placed.map.eq_ignore_ascii_case(map))
    }
}

/// `POST /v1/secrets/use`'s answer.
#[derive(Clone, Debug, Deserialize)]
pub struct Used {
    /// The presses reached the number the toy waited for: its show starts.
    #[serde(default)]
    pub show: bool,
    /// The toy played its show on this map load already: presses count no more.
    #[serde(default)]
    pub done: bool,
    /// The player found the toy with these presses: their profile after, the toy's
    /// achievement unlocked in it.
    #[serde(default)]
    pub profile: Option<Profile>,
}

/// A toy to place, as SJK's creator asks for it.
#[derive(Clone, Debug, PartialEq)]
pub struct Placing {
    /// The kind of toy (the hub's list names them to the creator).
    pub kind: String,
    /// The map's name as the game gives it.
    pub map: String,
    /// The map's BSP checksum.
    pub checksum: i32,
    /// `ip:port` for this server only, empty for every server.
    pub server: String,
    /// Where it stands.
    pub origin: [f32; 3],
    /// Facing, in degrees.
    pub yaw: f32,
}

/// What a feed event of [`TOY`] says: toy `id` plays its show (`show_<id>`).
pub fn parse_event(event: &str) -> Option<i64> {
    event.strip_prefix("show_")?.parse().ok()
}

/// A refused request in the player's words.
pub fn refusal(error: &HubError) -> String {
    let words = match error {
        HubError::Rejected { code, message, .. } => match code.as_str() {
            "no_such_secret" => "That toy is not here any more.",
            "not_on_server" => "The hub does not see you on this server yet.",
            "too_many_secrets" => "As many toys are hidden as the hub keeps: remove one first.",
            "creator_only" => "Only SJK's creator hides toys.",
            "bad_kind" => "No such toy.",
            _ => return message.clone(),
        },
        HubError::Protocol(why)
            if why.starts_with("status 404") || why.starts_with("status 405") =>
        {
            "This SJK hub has no Toy Master yet."
        }
        other => return other.to_string(),
    };
    words.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_reads_as_the_hub_sends_it() {
        let text = r#"{"secrets":[{"id":3,"map":"mp/ffa3","checksum":-1234,"server":"",
            "origin":[1,2,3],"yaw":90,"model":"models/x.md3","scale":1.5,
            "sound":"sound/x.mp3","show":{"vanish":180,"gone":1200,"pop":450,
            "spin":30000,"turns":3.0}}]}"#;
        let secrets: Secrets = serde_json::from_str(text).unwrap();
        let toy = &secrets.secrets[0];
        assert_eq!((toy.id, toy.scale, toy.show.spin), (3, 1.5, 30_000));
        assert_eq!(toy.kind, None);
        assert!(secrets.kinds.is_empty());
        assert!(secrets.on_map("MP/FFA3") && !secrets.on_map("mp/ffa1"));
        assert!(toy.stands_on("mp/ffa3", -1234, Some("1.2.3.4:29070")));
        assert!(!toy.stands_on("mp/ffa3", 5, None));
        // The creator reads each toy's kind and the kinds there are.
        let text = r#"{"secrets":[{"id":3,"map":"m","checksum":1,"origin":[0,0,0],
            "yaw":0,"kind":"k"}],"kinds":["k"]}"#;
        let secrets: Secrets = serde_json::from_str(text).unwrap();
        assert_eq!(secrets.secrets[0].kind.as_deref(), Some("k"));
        assert_eq!(secrets.secrets[0].scale, 1.0);
        assert_eq!(secrets.kinds, ["k"]);
    }

    #[test]
    fn feed_events_name_the_toy() {
        assert_eq!(parse_event("show_12"), Some(12));
        assert_eq!(parse_event("show_"), None);
        assert_eq!(parse_event("open_12"), None);
    }

    #[test]
    fn a_use_reads_its_show_and_its_find() {
        let used: Used = serde_json::from_str(r#"{"show":true,"done":true}"#).unwrap();
        assert!(used.show && used.done && used.profile.is_none());
    }
}
