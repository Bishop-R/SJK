//! Mystery boxes (the hub's `PROTOCOL.md`, "Mystery boxes", since 27/10/2026): boxes
//! SJK's creator places on maps, which players open for credits. The hub keeps where
//! they are, takes the price and rolls what is inside; this crate reads the list, asks
//! to open one and carries the creator's placing. The viewer draws them
//! (`sjk-viewer`'s `mystery_box.rs`).

use crate::hub::HubError;
use crate::wire::Profile;
use serde::{Deserialize, Serialize};

/// The toy id of the feed event the hub makes when a box is opened.
pub const TOY: &str = "mystery_box";
/// The outcome that blows the opener up and hands what they paid to a random hunter on
/// their server (since 28/10/2026).
pub const EXPLODE: &str = "explode";
/// The outcome that gives a holocron (since 28/10/2026).
pub const HOLOCRON: &str = "holocron";
/// The outcome that gives a pile of kyber shards (since 29/10/2026).
pub const SHARD_PILE: &str = "shards";
/// The outcome that gives a free opening, spent on the next one (since 29/10/2026).
pub const AGAIN: &str = "again";
/// The outcome that gives the boxes' own saber shader, [`BLADE_ID`], or shards for a
/// player who holds it (since 29/10/2026).
pub const BLADE: &str = "blade";
/// The boxes' own saber shader, found nowhere else.
pub const BLADE_ID: &str = "saber_mystery";

/// One placed box.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MysteryBox {
    /// The hub's id, never given again.
    pub id: i64,
    /// The map's name as the game gives it (`mp/ffa3`).
    pub map: String,
    /// The map's BSP checksum: a box stands in one exact file.
    pub checksum: i32,
    /// `ip:port` for one server only, empty for every server.
    #[serde(default)]
    pub server: String,
    /// Where it stands, in game units.
    pub origin: [f32; 3],
    /// Facing, in degrees.
    pub yaw: f32,
}

impl MysteryBox {
    /// Whether the box stands on map `map` (any case) of checksum `checksum`, on `server`.
    pub fn stands_on(&self, map: &str, checksum: i32, server: Option<&str>) -> bool {
        self.checksum == checksum
            && self.map.eq_ignore_ascii_case(map)
            && (self.server.is_empty() || Some(self.server.as_str()) == server)
    }
}

/// One thing a box may hold, as the hub rolls it.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct Outcome {
    /// Its id (`empty`, `jackpot`, ...).
    pub id: String,
    /// How likely it is, per mille.
    pub per_mille: u32,
    /// The credits it pays.
    pub credits: u64,
}

/// `GET /v1/mystery-boxes`: every placed box, the price and the odds.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Boxes {
    /// Every placed box.
    pub boxes: Vec<MysteryBox>,
    /// What opening one costs, in credits.
    pub price: u64,
    /// Shortest time between two openings by one person, in seconds.
    #[serde(default)]
    pub cooldown: u64,
    /// What a box may hold.
    #[serde(default)]
    pub outcomes: Vec<Outcome>,
}

impl Boxes {
    /// Whether any box stands on a map called `map` (any checksum), so the viewer knows
    /// to read the loaded map's checksum.
    pub fn on_map(&self, map: &str) -> bool {
        self.boxes
            .iter()
            .any(|placed| placed.map.eq_ignore_ascii_case(map))
    }
}

/// One opening, as the opener is answered.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct Opened {
    /// The box opened.
    #[serde(rename = "box")]
    pub box_id: i64,
    /// What was inside (`empty`, `refund`, `double`, `jackpot`, or a newer hub's).
    pub outcome: String,
    /// What it cost.
    pub price: u64,
    /// The credits it paid.
    pub won: u64,
    /// The opener's credits after.
    pub balance: u64,
    /// A box that blew up: the hunter who got what the opener paid, when one was there.
    #[serde(default)]
    pub gift: Option<Gift>,
    /// The holocron the box gave.
    #[serde(default)]
    pub holocron: Option<BoxHolocron>,
    /// The kyber shards the box gave (since 29/10/2026).
    #[serde(default)]
    pub shards: Option<u64>,
    /// The saber shader the box gave, its unlock id (since 29/10/2026).
    #[serde(default)]
    pub blade: Option<String>,
    /// The free openings left after this one (since 29/10/2026; an older hub leaves it
    /// out).
    #[serde(default)]
    pub free: Option<u64>,
}

/// Where a blown up box's price went.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct Gift {
    /// The hunter's slot.
    pub slot: u8,
    /// The hunter's worn name.
    pub name: String,
    /// What they got (0 for a free opening).
    pub credits: u64,
}

/// The holocron a box gave: its id and tier.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct BoxHolocron {
    /// The holocron's id among the player's.
    pub id: i64,
    /// Its tier (`rare`, ...).
    pub tier: String,
}

/// `POST /v1/mystery-box/open`'s answer.
#[derive(Clone, Debug, Deserialize)]
pub struct OpenAnswer {
    /// What came out.
    pub opened: Opened,
    /// The opener's profile after.
    pub profile: Profile,
}

/// What a feed event of [`TOY`] says: box `box` gave `outcome` (`open_<box>_<outcome>`).
pub fn parse_event(event: &str) -> Option<(i64, &str)> {
    let rest = event.strip_prefix("open_")?;
    let (id, outcome) = rest.split_once('_')?;
    Some((id.parse().ok()?, outcome)).filter(|(_, outcome)| !outcome.is_empty())
}

/// The look toy of the box hunt (`POST /v1/box-hunt`): a player wearing it takes part.
pub const HUNT_TOY: &str = "box_hunt";

/// The box hunt's mark as the hub tells hunters.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct HuntMark {
    /// The mark's slot.
    pub slot: u8,
    /// The mark's worn name.
    pub name: String,
    /// When the hub picks another, Unix seconds.
    pub until: i64,
    /// Seconds from the read to `until`, by the hub's clock (a hub before 31/10/2026
    /// does not say).
    #[serde(default)]
    pub left: Option<u64>,
}

/// `POST /v1/box-hunt`: the hunt on a server as the player reads it.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct HuntState {
    /// The player wears the hunt's toy there: only then is the mark told.
    #[serde(default)]
    pub hunting: bool,
    /// The mark, told while the player hunts and a mark is picked.
    #[serde(default)]
    pub mark: Option<HuntMark>,
    /// Free box openings won and not spent.
    #[serde(default)]
    pub free: u64,
    /// How long a mark lasts, in seconds.
    #[serde(default)]
    pub every: u64,
}

/// What a feed event of the hunt says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HuntEvent {
    /// The player on this slot is the new mark (`mark_<slot>`).
    Mark(u8),
    /// The player on `slot` killed the mark, and whether it won them a free opening
    /// (`hunted_<slot>_<won>`).
    Hunted {
        /// The hunter's slot.
        slot: u8,
        /// Whether it won them a free opening.
        won: bool,
    },
    /// The box the player on `opener` opened blew up, and `credits` went to the hunter
    /// on `to` (`None`: nobody else hunted there, they were lost)
    /// (`laugh_<opener>_<to>_<credits>`, `to` 255 for nobody).
    Laugh {
        /// The opener's slot.
        opener: u8,
        /// The slot of the hunter who got the credits.
        to: Option<u8>,
        /// What they got.
        credits: u64,
    },
}

/// Read a feed event of [`HUNT_TOY`].
pub fn parse_hunt_event(event: &str) -> Option<HuntEvent> {
    if let Some(slot) = event.strip_prefix("mark_") {
        return Some(HuntEvent::Mark(slot.parse().ok()?));
    }
    if let Some(rest) = event.strip_prefix("laugh_") {
        let mut parts = rest.split('_');
        let opener = parts.next()?.parse().ok()?;
        let to: u8 = parts.next()?.parse().ok()?;
        let credits = parts.next()?.parse().ok()?;
        return parts.next().is_none().then_some(HuntEvent::Laugh {
            opener,
            to: (to != 255).then_some(to),
            credits,
        });
    }
    let (slot, won) = event.strip_prefix("hunted_")?.split_once('_')?;
    Some(HuntEvent::Hunted {
        slot: slot.parse().ok()?,
        won: match won {
            "1" => true,
            "0" => false,
            _ => return None,
        },
    })
}

/// Why an opening, a placing or a removal failed, in the player's words.
pub fn refusal(error: &HubError) -> String {
    let words = match error {
        HubError::Rejected { code, message, .. } => match code.as_str() {
            "not_enough_credits" => "Not enough credits.",
            "box_cooldown" => "You opened a box a moment ago: let someone else roll.",
            "no_such_box" => "That box is not here any more.",
            "not_on_server" => "The hub does not see you on this server yet.",
            "opening_staff_only" => "Mystery boxes open with holocrons: soon.",
            "too_many_boxes" => "As many boxes are placed as the hub keeps: remove one first.",
            "creator_only" => "Only SJK's creator places boxes.",
            _ => return message.clone(),
        },
        HubError::Protocol(why)
            if why.starts_with("status 404") || why.starts_with("status 405") =>
        {
            "This SJK hub has no mystery boxes yet."
        }
        other => return other.to_string(),
    };
    words.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunt_events_read_back() {
        assert_eq!(parse_hunt_event("mark_7"), Some(HuntEvent::Mark(7)));
        assert_eq!(
            parse_hunt_event("hunted_3_1"),
            Some(HuntEvent::Hunted { slot: 3, won: true })
        );
        assert_eq!(
            parse_hunt_event("hunted_3_0"),
            Some(HuntEvent::Hunted {
                slot: 3,
                won: false
            })
        );
        assert_eq!(
            parse_hunt_event("laugh_2_4_100"),
            Some(HuntEvent::Laugh {
                opener: 2,
                to: Some(4),
                credits: 100
            })
        );
        assert_eq!(
            parse_hunt_event("laugh_2_255_0"),
            Some(HuntEvent::Laugh {
                opener: 2,
                to: None,
                credits: 0
            })
        );
        for bad in [
            "mark_",
            "mark_x",
            "hunted_3",
            "hunted_3_2",
            "open_1_empty",
            "laugh_2_4",
            "laugh_2_4_100_1",
            "laugh_x_4_100",
        ] {
            assert_eq!(parse_hunt_event(bad), None, "{bad}");
        }
        let state: HuntState = serde_json::from_str(
            r#"{"hunting":true,"mark":{"slot":4,"name":"Kyle","until":99},"free":1,"every":180}"#,
        )
        .unwrap();
        assert_eq!(state.mark.map(|mark| mark.slot), Some(4));
        assert_eq!(state.free, 1);
    }

    #[test]
    fn the_list_reads_as_the_hub_sends_it() {
        let boxes: Boxes = serde_json::from_str(
            r#"{"boxes":[{"id":3,"map":"mp/ffa3","checksum":-1234,"server":"","origin":[1,2,3],
                "yaw":90,"placed":5}],"price":100,"cooldown":3,
                "outcomes":[{"id":"empty","per_mille":500,"credits":0}]}"#,
        )
        .unwrap();
        assert_eq!(boxes.price, 100);
        let placed = &boxes.boxes[0];
        assert!(placed.stands_on("MP/FFA3", -1234, Some("1.2.3.4:29070")));
        assert!(!placed.stands_on("mp/ffa3", 7, None));
        assert!(boxes.on_map("mp/ffa3") && !boxes.on_map("mp/ffa2"));
        let one = MysteryBox {
            server: "1.2.3.4:29070".to_owned(),
            ..placed.clone()
        };
        assert!(one.stands_on("mp/ffa3", -1234, Some("1.2.3.4:29070")));
        assert!(!one.stands_on("mp/ffa3", -1234, Some("5.6.7.8:29070")));
        assert!(!one.stands_on("mp/ffa3", -1234, None));
    }

    #[test]
    fn an_opening_reads_its_gift_and_holocron_when_the_hub_sends_them() {
        let plain: Opened = serde_json::from_str(
            r#"{"id":1,"box":3,"outcome":"double","price":100,"won":200,"balance":5}"#,
        )
        .unwrap();
        assert_eq!((plain.gift, plain.holocron), (None, None));
        let blown: Opened = serde_json::from_str(
            r#"{"id":2,"box":3,"outcome":"explode","price":100,"won":0,"balance":5,
                "gift":{"slot":4,"name":"Kyle","credits":100}}"#,
        )
        .unwrap();
        assert_eq!(
            blown.gift.map(|gift| (gift.slot, gift.credits)),
            Some((4, 100))
        );
        let found: Opened = serde_json::from_str(
            r#"{"id":3,"box":3,"outcome":"holocron","price":100,"won":0,"balance":5,
                "holocron":{"id":77,"tier":"rare"}}"#,
        )
        .unwrap();
        assert_eq!(found.holocron.map(|h| h.tier), Some("rare".to_owned()));
    }

    #[test]
    fn feed_events_name_the_box_and_what_came_out() {
        assert_eq!(parse_event("open_12_jackpot"), Some((12, "jackpot")));
        assert_eq!(parse_event("open_12_big_win"), Some((12, "big_win")));
        assert_eq!(parse_event("open_x_empty"), None);
        assert_eq!(parse_event("open_12_"), None);
        assert_eq!(parse_event("spotted"), None);
    }
}
