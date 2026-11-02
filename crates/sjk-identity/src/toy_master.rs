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
///
/// Since 02/11/2026 a show may go on in [`Step`]s instead, from the moment its sound
/// starts (as the toy pops back) for `length` in all: then `spin` and `turns` are only for
/// clients that do not know steps. `reach` is how far round the toy the players take
/// part, `ring` the model drawn on the ground to show it and `ring_radius` that model's
/// own radius.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
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
    /// Its steps, in order (none: it spins for `spin`).
    #[serde(default)]
    pub steps: Vec<Step>,
    /// How long its steps last in all, from the moment its sound starts.
    #[serde(default)]
    pub length: u32,
    /// How far round the toy the players take part in its steps, in units.
    #[serde(default)]
    pub reach: f32,
    /// The model drawn on the ground to show the reach (empty for none).
    #[serde(default)]
    pub ring: String,
    /// That model's own radius, in units.
    #[serde(default)]
    pub ring_radius: f32,
}

/// One step of a show, from `at` (milliseconds after its sound starts) to the next: the
/// toy spins at `spin` turns a second (0 stands still); with `dance` the players within
/// reach spin and squish with it; with `ring` the ring shows; with `float` the players
/// within reach and the toy float weightless (drawn so only); with `spit` the toy throws
/// out the show's drops, spread over the step.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Step {
    /// When it starts.
    pub at: u32,
    /// The toy's spin, in turns a second.
    #[serde(default)]
    pub spin: f32,
    /// The players within reach dance.
    #[serde(default)]
    pub dance: bool,
    /// The ring shows.
    #[serde(default)]
    pub ring: bool,
    /// The players within reach and the toy float.
    #[serde(default)]
    pub float: bool,
    /// The toy throws out its drops.
    #[serde(default)]
    pub spit: bool,
}

impl Show {
    /// How long before its sound starts the toy is back (it vanished, stayed gone and
    /// popped back), in milliseconds.
    pub fn lead(&self) -> u32 {
        self.vanish + self.gone + self.pop
    }

    /// When drop `index` of `count` comes out, in milliseconds after the show's start:
    /// spread evenly over its first `spit` step (the hub reckons it the same way).
    pub fn drop_time(&self, index: u32, count: u32) -> Option<u32> {
        let at = self.steps.iter().position(|step| step.spit)?;
        let from = u64::from(self.steps[at].at);
        let to = self
            .steps
            .get(at + 1)
            .map_or(u64::from(self.length), |next| u64::from(next.at));
        let into =
            to.saturating_sub(from) * (2 * u64::from(index) + 1) / (2 * u64::from(count.max(1)));
        Some(self.lead() + (from + into) as u32)
    }
}

/// What one of a show's drops is, as the toy throws it (what it gives is told only to
/// whoever takes it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropKind {
    /// Credits.
    Credits,
    /// Kyber shards.
    Shards,
    /// A holocron.
    Holocron,
}

impl DropKind {
    fn from_letter(letter: char) -> Option<Self> {
        match letter {
            'c' => Some(Self::Credits),
            's' => Some(Self::Shards),
            'h' => Some(Self::Holocron),
            _ => None,
        }
    }
}

/// A feed event of [`TOY`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// Toy `id` plays its show (`show_<id>`).
    Show(i64),
    /// Toy `id`'s show throws these drops, from `seed` (`drops_<id>_<seed>_<letters>`).
    Drops {
        /// The toy.
        id: i64,
        /// How the drops are thrown, the same for every client.
        seed: u32,
        /// What each drop is.
        kinds: Vec<DropKind>,
    },
    /// Drop `drop` of toy `id`'s show was taken (`taken_<id>_<drop>`).
    Taken {
        /// The toy.
        id: i64,
        /// The drop.
        drop: u32,
    },
}

impl Event {
    /// The event in `event`, if one of these.
    pub fn parse(event: &str) -> Option<Self> {
        if let Some(id) = event.strip_prefix("show_") {
            return id.parse().ok().map(Self::Show);
        }
        if let Some(rest) = event.strip_prefix("drops_") {
            let mut parts = rest.splitn(3, '_');
            let id = parts.next()?.parse().ok()?;
            let seed = parts.next()?.parse().ok()?;
            let kinds = parts
                .next()
                .unwrap_or_default()
                .chars()
                .map(DropKind::from_letter)
                .collect::<Option<Vec<_>>>()?;
            return (kinds.len() <= 32).then_some(Self::Drops { id, seed, kinds });
        }
        let rest = event.strip_prefix("taken_")?;
        let (id, drop) = rest.split_once('_')?;
        Some(Self::Taken {
            id: id.parse().ok()?,
            drop: drop.parse().ok().filter(|drop| *drop < 32)?,
        })
    }
}

/// `POST /v1/secrets/take`'s answer: what the drop gave, and the profile after.
#[derive(Clone, Debug, Deserialize)]
pub struct Took {
    /// `credits`, `shards` or `holocron`.
    pub kind: String,
    /// How many credits or shards.
    #[serde(default)]
    pub amount: i64,
    /// The holocron's tier.
    #[serde(default)]
    pub tier: Option<String>,
    /// The profile after.
    #[serde(default)]
    pub profile: Option<Profile>,
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
    match Event::parse(event)? {
        Event::Show(id) => Some(id),
        _ => None,
    }
}

/// A refused request in the player's words.
pub fn refusal(error: &HubError) -> String {
    let words = match error {
        HubError::Rejected { code, message, .. } => match code.as_str() {
            "no_such_secret" => "That toy is not here any more.",
            "no_such_drop" => "That is not here any more.",
            "drop_taken" => "Someone took that one first.",
            "drop_not_yet" => "Wait for it to land.",
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

    /// A show's drops, a drop taken, and its steps read as the hub sends them.
    #[test]
    fn steps_drops_and_takes_read_as_the_hub_sends_them() {
        assert_eq!(
            Event::parse("drops_3_77_csh"),
            Some(Event::Drops {
                id: 3,
                seed: 77,
                kinds: vec![DropKind::Credits, DropKind::Shards, DropKind::Holocron],
            })
        );
        assert_eq!(Event::parse("drops_3_77_cx"), None);
        assert_eq!(
            Event::parse("taken_3_12"),
            Some(Event::Taken { id: 3, drop: 12 })
        );
        assert_eq!(Event::parse("taken_3_40"), None);
        assert_eq!(parse_event("drops_3_77_c"), None);
        let show: Show = serde_json::from_str(
            r#"{"vanish":180,"gone":1200,"pop":450,"spin":30000,"turns":3.0,
            "steps":[{"at":0,"spin":3.0},{"at":108000,"spin":3.0,"spit":true},
            {"at":131000}],"length":135000,"reach":400.0,
            "ring":"models/r.md3","ring_radius":100.0}"#,
        )
        .unwrap();
        assert_eq!(show.steps.len(), 3);
        assert!(show.steps[1].spit && !show.steps[1].dance);
        assert_eq!(show.lead(), 1_830);
        // As the hub reckons them: 13 drops over 23 s.
        assert_eq!(show.drop_time(0, 2), Some(1_830 + 108_000 + 5_750));
        assert_eq!(show.drop_time(1, 2), Some(1_830 + 108_000 + 17_250));
        let took: Took = serde_json::from_str(r#"{"kind":"holocron","tier":"uncommon"}"#).unwrap();
        assert_eq!(
            (took.kind.as_str(), took.tier.as_deref()),
            ("holocron", Some("uncommon"))
        );
    }

    #[test]
    fn a_use_reads_its_show_and_its_find() {
        let used: Used = serde_json::from_str(r#"{"show":true,"done":true}"#).unwrap();
        assert!(used.show && used.done && used.profile.is_none());
    }
}
