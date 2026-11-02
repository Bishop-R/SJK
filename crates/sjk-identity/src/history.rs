//! The player's inventory history (the hub's `PROTOCOL.md`, "History", 31/10/2026): what
//! came to them (holocrons and kyber crystals), what they opened and what it held, the
//! credits and kyber shards they gained and spent, the mystery boxes they opened and the
//! trades they finished, newest first.
//!
//! `GET /v1/history` (signed) answers a page of [`HistoryLine`]s; while
//! [`HistoryPage::more`] says so, `GET /v1/history?before=<HistoryPage::before>` the
//! older ones. Lines of one second are never split across pages. A kind of line this
//! client does not know is skipped, and so is a line it cannot read, so a newer hub can
//! add kinds; unknown fields are ignored and missing ones take their defaults. The hub
//! sends no key id: none is ever shown in game.
//!
//! The identity worker keeps what was read in [`crate::Snapshot::history`]
//! ([`HistoryState`]), read when the viewer asks ([`crate::Service::read_history`]).

use crate::hub::HubError;
use crate::trades::TradeItem;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// One page of the history as the hub answers it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HistoryPage {
    /// The lines this client reads, newest first.
    pub lines: Vec<HistoryLine>,
    /// Whether older lines are left: ask again with `?before=` [`HistoryPage::before`].
    pub more: bool,
    /// What to send as `before` for the next page (unix seconds).
    pub before: i64,
}

impl<'de> Deserialize<'de> for HistoryPage {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            lines: Vec<Value>,
            #[serde(default)]
            more: bool,
            #[serde(default)]
            before: i64,
        }
        let raw = Raw::deserialize(deserializer)?;
        Ok(Self {
            lines: raw
                .lines
                .into_iter()
                .filter_map(HistoryLine::read)
                .collect(),
            more: raw.more,
            before: raw.before,
        })
    }
}

/// One line of the history.
#[derive(Clone, Debug, PartialEq)]
pub enum HistoryLine {
    /// A holocron or a kyber crystal came to the player (`came`).
    Came(Came),
    /// The player opened one (`opened`).
    Opened(OpenedLine),
    /// A line of the credits ledger no other kind covers (`credits`).
    Credits(Ledger),
    /// A line of the kyber shard ledger no other kind covers (`shards`).
    Shards(Ledger),
    /// A mystery box opened (`box`).
    Box(BoxLine),
    /// A finished trade (`trade`).
    Trade(TradeLine),
}

impl HistoryLine {
    /// The line `value` holds, if it is of a kind this client knows and reads.
    pub fn read(value: Value) -> Option<Self> {
        fn as_<T: DeserializeOwned>(value: Value) -> Option<T> {
            serde_json::from_value(value).ok()
        }
        let kind = value.get("kind")?.as_str()?.to_owned();
        match kind.as_str() {
            "came" => as_(value).map(Self::Came),
            "opened" => as_(value).map(Self::Opened),
            "credits" => as_(value).map(Self::Credits),
            "shards" => as_(value).map(Self::Shards),
            "box" => as_(value).map(Self::Box),
            "trade" => as_(value).map(Self::Trade),
            _ => None,
        }
    }

    /// When it happened, unix seconds.
    pub fn at(&self) -> i64 {
        match self {
            Self::Came(line) => line.at,
            Self::Opened(line) => line.at,
            Self::Credits(line) | Self::Shards(line) => line.at,
            Self::Box(line) => line.at,
            Self::Trade(line) => line.at,
        }
    }
}

/// `null` or a missing string read as empty.
fn text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

/// A holocron or a kyber crystal that came to the player.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct Came {
    /// When, unix seconds (the trade's time when it came through one).
    #[serde(default)]
    pub at: i64,
    /// Its id at the hub.
    #[serde(default)]
    pub holocron: Option<u64>,
    /// Its tier: a holocron tier (`rare`) or a crystal's (`kyber_rare`).
    #[serde(default, deserialize_with = "text")]
    pub tier: String,
    /// How it came: `play`, `staff`, `achievement`, `bought`, `box`, `reward`, or
    /// another a newer hub knows.
    #[serde(default, deserialize_with = "text")]
    pub source: String,
    /// The team's note (a gift's, a reward's), often empty.
    #[serde(default, deserialize_with = "text")]
    pub note: String,
    /// The credits a bought one cost (0 otherwise; the hub's credits line for it is
    /// this one's).
    #[serde(default)]
    pub price: i64,
    /// It arrived through a trade.
    #[serde(default)]
    pub traded: bool,
}

/// The card an opening brought.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct HistoryCard {
    /// The card it is; `None` while it is still sealed.
    #[serde(default)]
    pub card: Option<String>,
    /// Its rarity (`rare`).
    #[serde(default, deserialize_with = "text")]
    pub rarity: String,
    /// Its finish (`holo`).
    #[serde(default, deserialize_with = "text")]
    pub finish: String,
}

/// A holocron or kyber crystal the player opened, and what it held.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct OpenedLine {
    /// When, unix seconds.
    #[serde(default)]
    pub at: i64,
    /// Its id at the hub.
    #[serde(default)]
    pub holocron: Option<u64>,
    /// Its tier.
    #[serde(default, deserialize_with = "text")]
    pub tier: String,
    /// The credits it held.
    #[serde(default)]
    pub credits: i64,
    /// The kyber shards it held.
    #[serde(default)]
    pub shards: i64,
    /// The card it held, if one.
    #[serde(default)]
    pub card: Option<HistoryCard>,
    /// The unlock (a shader, a tag) it gave by its id; empty for none.
    #[serde(default, deserialize_with = "text")]
    pub unlock: String,
}

/// A line of the credits or kyber shard ledger.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct Ledger {
    /// When, unix seconds.
    #[serde(default)]
    pub at: i64,
    /// What it changed, signed: a gain is positive, a spend negative.
    #[serde(default)]
    pub delta: i64,
    /// Why: credits `holocron_buy`, `staff`, `reward`, `mystery_box`; shards `shatter`,
    /// `craft`, `shop`, `staff`, `reward`; or another a newer hub knows.
    #[serde(default, deserialize_with = "text")]
    pub reason: String,
    /// More of why: the shader's unlock id for `shatter`, `craft` and `shop`,
    /// `explode_gift` for `mystery_box`, the team's words for `staff` and `reward`.
    #[serde(default, deserialize_with = "text")]
    pub note: String,
}

/// A mystery box the player opened.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct BoxLine {
    /// When, unix seconds.
    #[serde(default)]
    pub at: i64,
    /// What came out (`crate::mystery_box`'s outcomes: `empty`, `double`, ...).
    #[serde(default, deserialize_with = "text")]
    pub outcome: String,
    /// The credits it cost; 0 for a free roll.
    #[serde(default)]
    pub price: i64,
    /// The credits it paid.
    #[serde(default)]
    pub won: i64,
    /// The kyber shards it paid.
    #[serde(default)]
    pub shards: i64,
}

/// The other player of a finished trade.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct HistoryPartner {
    /// Their display name, with colour codes. Show it only through
    /// [`crate::chat::name_for_display`].
    #[serde(default, deserialize_with = "text")]
    pub name: String,
    /// Whether the hub's operator vouches for them.
    #[serde(default)]
    pub verified: bool,
}

/// A finished trade.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct TradeLine {
    /// When it finished, unix seconds.
    #[serde(default)]
    pub at: i64,
    /// Its id at the hub.
    #[serde(default)]
    pub trade: i64,
    /// The other player.
    #[serde(default)]
    pub with: HistoryPartner,
    /// What the player gave.
    #[serde(default)]
    pub gave: Vec<TradeItem>,
    /// What the player got.
    #[serde(default)]
    pub got: Vec<TradeItem>,
}

/// The history as the client read it so far ([`crate::Snapshot::history`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HistoryState {
    /// Counts the reads answered (one for each [`crate::Service::read_history`], failed
    /// ones too): a reader that noted it when asking waits while it is unchanged.
    pub serial: u64,
    /// Every line read, newest first: the first page and the older ones asked since.
    pub lines: Vec<HistoryLine>,
    /// Whether older lines are left at the hub.
    pub more: bool,
    /// What to ask for the next page with.
    pub before: i64,
    /// Why the last read failed, in the player's words; empty when it did not.
    pub message: String,
}

impl HistoryState {
    /// Take the answer to a read of the page before `before` (`None`: the newest page,
    /// which replaces what was read). An older page is added only when it follows what
    /// was read, so a second ask for the same page adds nothing twice; a failed read
    /// keeps the lines read before it, except a failed read of the newest page.
    pub fn take(&mut self, before: Option<i64>, answer: Result<HistoryPage, HubError>) {
        self.serial += 1;
        match (before, answer) {
            (None, Ok(page)) => {
                self.lines = page.lines;
                self.more = page.more;
                self.before = page.before;
                self.message.clear();
            }
            (Some(before), Ok(page)) => {
                if self.more && self.before == before {
                    self.lines.extend(page.lines);
                    self.more = page.more;
                    self.before = page.before;
                }
                self.message.clear();
            }
            (None, Err(error)) => {
                self.lines.clear();
                self.more = false;
                self.message = refusal(&error);
            }
            (Some(_), Err(error)) => self.message = refusal(&error),
        }
    }
}

/// `GET /v1/history`'s path: the newest page, or the one before `before`.
pub fn path(before: Option<i64>) -> String {
    match before {
        Some(before) => format!("/v1/history?before={before}"),
        None => "/v1/history".to_owned(),
    }
}

/// What players read when the history cannot be read, for `error`.
pub fn refusal(error: &HubError) -> String {
    let words = match error {
        // A hub from before the history has no such route.
        HubError::Rejected { status: 404, .. } => "This SJK hub keeps no history yet.",
        HubError::Protocol(why)
            if why.starts_with("status 404") || why.starts_with("status 405") =>
        {
            "This SJK hub keeps no history yet."
        }
        HubError::Rejected { code, .. } if code == "not_registered" => {
            "Your SJK identity is not registered at the hub yet."
        }
        HubError::Rejected { code, .. } if code.ends_with("quota") || code == "rate_limited" => {
            "Too many requests: wait a moment."
        }
        HubError::Network(_) => "Cannot reach the SJK hub.",
        HubError::NotConnected(why) => return why.clone(),
        other => return other.to_string(),
    };
    words.to_owned()
}

/// Unix `seconds` in this PC's time as `dd/mm/yyyy HH:MM`; empty for a time before 1970
/// or one chrono cannot place.
pub fn local_date_time(seconds: i64) -> String {
    use chrono::{DateTime, Local};
    if seconds <= 0 {
        return String::new();
    }
    DateTime::from_timestamp(seconds, 0)
        .map(|time| date_time(&time.with_timezone(&Local)))
        .unwrap_or_default()
}

/// `time` as `dd/mm/yyyy HH:MM`.
fn date_time<T: chrono::TimeZone>(time: &chrono::DateTime<T>) -> String {
    use chrono::{Datelike, Timelike};
    format!(
        "{:02}/{:02}/{} {:02}:{:02}",
        time.day(),
        time.month(),
        time.year(),
        time.hour(),
        time.minute()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(json: &str) -> HistoryPage {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn every_kind_reads() {
        let page = page(
            r#"{"lines":[
            {"kind":"came","at":10,"holocron":7,"tier":"rare","source":"play","note":"","traded":false},
            {"kind":"opened","at":9,"holocron":6,"tier":"legendary","credits":120,"shards":8,
             "card":{"card":"sjk_alpha_000","rarity":"rare","finish":"holo"},"unlock":"saber_tidal"},
            {"kind":"credits","at":8,"delta":-400,"reason":"holocron_buy","note":""},
            {"kind":"shards","at":7,"delta":15,"reason":"shatter","note":"saber_tidal"},
            {"kind":"box","at":6,"outcome":"double","price":100,"won":200,"shards":0},
            {"kind":"trade","at":5,"trade":3,"with":{"name":"^1Nib","verified":true},
             "gave":[{"kind":"holocron","holocron":4,"tier":"rare"}],
             "got":[{"kind":"shader","id":"saber_sun","name":"Sun","tier":"mythical"}]}
            ],"more":true,"before":5}"#,
        );
        assert!(page.more);
        assert_eq!(page.before, 5);
        assert_eq!(page.lines.len(), 6);
        assert_eq!(
            page.lines[0],
            HistoryLine::Came(Came {
                at: 10,
                holocron: Some(7),
                tier: "rare".into(),
                source: "play".into(),
                note: String::new(),
                price: 0,
                traded: false,
            })
        );
        let HistoryLine::Opened(opened) = &page.lines[1] else {
            panic!("{:?}", page.lines[1]);
        };
        assert_eq!((opened.credits, opened.shards), (120, 8));
        assert_eq!(
            opened.card.as_ref().and_then(|card| card.card.as_deref()),
            Some("sjk_alpha_000")
        );
        assert_eq!(opened.unlock, "saber_tidal");
        assert_eq!(
            page.lines[2],
            HistoryLine::Credits(Ledger {
                at: 8,
                delta: -400,
                reason: "holocron_buy".into(),
                note: String::new(),
            })
        );
        assert!(matches!(&page.lines[3], HistoryLine::Shards(line) if line.delta == 15));
        assert!(
            matches!(&page.lines[4], HistoryLine::Box(line) if line.outcome == "double" && line.won == 200)
        );
        let HistoryLine::Trade(trade) = &page.lines[5] else {
            panic!("{:?}", page.lines[5]);
        };
        assert_eq!(trade.with.name, "^1Nib");
        assert!(trade.with.verified);
        assert_eq!(trade.gave[0].holocron, Some(4));
        assert_eq!(trade.got[0].id.as_deref(), Some("saber_sun"));
        assert_eq!(
            page.lines.iter().map(HistoryLine::at).collect::<Vec<_>>(),
            [10, 9, 8, 7, 6, 5]
        );
    }

    #[test]
    fn unknown_kinds_and_unreadable_lines_are_skipped() {
        let page = page(
            r#"{"lines":[
            {"kind":"medal","at":10,"medal":"tester"},
            {"at":9},
            "not a line",
            {"kind":"came","at":"yesterday"},
            {"kind":"shards","at":8,"delta":3,"reason":"staff","future":{"a":1}}
            ],"more":false,"before":0,"extra":1}"#,
        );
        assert_eq!(page.lines.len(), 1);
        assert!(matches!(&page.lines[0], HistoryLine::Shards(line) if line.at == 8));
    }

    #[test]
    fn missing_and_null_fields_take_their_defaults() {
        let page = page(
            r#"{"lines":[
            {"kind":"came"},
            {"kind":"opened","at":4,"card":null,"unlock":null},
            {"kind":"opened","at":4,"card":{"card":null,"rarity":"rare"}},
            {"kind":"trade","at":3},
            {"kind":"trade","at":2,"got":[{"tier":"rare"}]},
            {"kind":"box","at":1,"outcome":null}
            ]}"#,
        );
        assert!(!page.more);
        assert_eq!(page.lines.len(), 6);
        assert_eq!(page.lines[0], HistoryLine::Came(Came::default()));
        assert!(
            matches!(&page.lines[1], HistoryLine::Opened(line) if line.card.is_none() && line.unlock.is_empty())
        );
        assert!(
            matches!(&page.lines[2], HistoryLine::Opened(line) if line.card.as_ref().is_some_and(|card| card.card.is_none() && card.finish.is_empty()))
        );
        assert!(
            matches!(&page.lines[3], HistoryLine::Trade(line) if line.with.name.is_empty() && line.gave.is_empty())
        );
        assert!(matches!(&page.lines[4], HistoryLine::Trade(line) if line.got[0].kind.is_empty()));
        assert_eq!(
            page.lines[5],
            HistoryLine::Box(BoxLine {
                at: 1,
                ..BoxLine::default()
            })
        );
        // An empty answer is an empty page.
        assert_eq!(super::tests::page("{}"), HistoryPage::default());
    }

    #[test]
    fn paths_carry_the_page() {
        assert_eq!(path(None), "/v1/history");
        assert_eq!(path(Some(1_792_000_000)), "/v1/history?before=1792000000");
    }

    fn line(at: i64) -> HistoryLine {
        HistoryLine::Credits(Ledger {
            at,
            ..Ledger::default()
        })
    }

    #[test]
    fn pages_follow_on_and_a_refresh_starts_again() {
        let mut state = HistoryState::default();
        state.take(
            None,
            Ok(HistoryPage {
                lines: vec![line(30), line(20)],
                more: true,
                before: 20,
            }),
        );
        assert_eq!((state.serial, state.lines.len(), state.more), (1, 2, true));
        let older = HistoryPage {
            lines: vec![line(10)],
            more: false,
            before: 10,
        };
        state.take(Some(20), Ok(older.clone()));
        assert_eq!((state.serial, state.lines.len(), state.more), (2, 3, false));
        // The same page again adds nothing.
        state.take(Some(20), Ok(older));
        assert_eq!((state.serial, state.lines.len()), (3, 3));
        // A failed older page keeps the lines; a failed refresh does not.
        state.more = true;
        state.take(Some(10), Err(HubError::Network("down".into())));
        assert_eq!(state.lines.len(), 3);
        assert_eq!(state.message, "Cannot reach the SJK hub.");
        state.take(
            None,
            Err(HubError::Rejected {
                status: 404,
                code: "not_found".into(),
                message: String::new(),
            }),
        );
        assert!(state.lines.is_empty());
        assert_eq!(state.message, "This SJK hub keeps no history yet.");
        state.take(
            None,
            Ok(HistoryPage {
                lines: vec![line(40)],
                more: false,
                before: 40,
            }),
        );
        assert_eq!((state.lines.len(), state.message.as_str()), (1, ""));
    }

    #[test]
    fn refusals_are_in_the_players_words() {
        assert_eq!(
            refusal(&HubError::Protocol("status 404, not JSON".into())),
            "This SJK hub keeps no history yet."
        );
        assert_eq!(
            refusal(&HubError::Rejected {
                status: 403,
                code: "not_registered".into(),
                message: "unknown key".into(),
            }),
            "Your SJK identity is not registered at the hub yet."
        );
        assert_eq!(refusal(&HubError::NotConnected("off".into())), "off");
    }

    #[test]
    fn times_are_day_first_with_the_year_on_the_24_hour_clock() {
        // 17/10/2026 12:32 UTC is 14:32 in Paris's summer time.
        let paris = chrono::FixedOffset::east_opt(2 * 3600).unwrap();
        let time = chrono::DateTime::from_timestamp(1_792_240_320, 0)
            .unwrap()
            .with_timezone(&paris);
        assert_eq!(date_time(&time), "17/10/2026 14:32");
        assert_eq!(local_date_time(0), "");
    }
}
