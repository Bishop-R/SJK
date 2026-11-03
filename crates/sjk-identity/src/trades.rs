//! Trades between players (`PROTOCOL.md` in Sol-Vulpes/SJK-hub, "Trades between players";
//! 22/10/2026): two players swap unopened holocrons and duplicate saber shaders through
//! the hub. The hub holds the trade and checks both sides twice; the client asks, shows
//! and answers.
//!
//! The identity worker keeps the player's trades in [`crate::Snapshot::trades`]: read when
//! the hub's feed tells of a change (`trades` in [`crate::Feed`]), after every step the
//! player takes, and while a trade is open every few seconds so its clocks stay true.
//! Every limit and worth comes from the hub ([`TradeRules`]): the client never holds
//! numbers of its own.

use crate::hub::HubError;
use crate::wire::Profile;
use serde::Deserialize;
use std::collections::BTreeMap;

/// A change to one of the reader's trades, as the feed relays it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct TradeEvent {
    /// Its id in the feed's sequence.
    pub id: u64,
    /// When it changed, unix seconds.
    #[serde(default)]
    pub at: i64,
    /// The trade's id.
    pub trade: i64,
    /// Its state after the change.
    #[serde(default)]
    pub state: String,
    /// Its revision after the change.
    #[serde(default)]
    pub rev: i64,
}

/// One item on a side of a trade.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct TradeItem {
    /// `holocron`, `shader` or (since 03/11/2026, hubs whose economy says
    /// [`crate::wire::TAG_COPIES_ABILITY`]) `tag`.
    #[serde(default)]
    pub kind: String,
    /// The holocron's id, for a holocron.
    #[serde(default)]
    pub holocron: Option<u64>,
    /// The shader's unlock id, for a shader.
    #[serde(default)]
    pub id: Option<String>,
    /// The shader's name, for a shader.
    #[serde(default)]
    pub name: Option<String>,
    /// The holocron's tier, or the shader's.
    #[serde(default)]
    pub tier: String,
    /// What it brings on average, in kyber shards.
    #[serde(default)]
    pub value: i64,
    /// A tag copy's finish (`normal`, `holo`, `solar`); `None` for other items and a
    /// Normal copy a hub left it out of.
    #[serde(default)]
    pub finish: Option<String>,
}

impl TradeItem {
    /// Whether it is a holocron.
    pub fn is_holocron(&self) -> bool {
        self.kind == "holocron"
    }

    /// Whether it is a tag copy.
    pub fn is_tag(&self) -> bool {
        self.kind == "tag"
    }

    /// The tag copy it is, for a tag.
    pub fn tag_copy(&self) -> Option<crate::wire::TagCopy> {
        if !self.is_tag() {
            return None;
        }
        Some(crate::wire::TagCopy::new(
            self.id.as_deref()?,
            self.finish.as_deref().unwrap_or("normal"),
        ))
    }
}

/// One side of a trade.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct TradeSide {
    /// What it offers.
    #[serde(default)]
    pub items: Vec<TradeItem>,
    /// What it is worth together, in kyber shards.
    #[serde(default)]
    pub value: i64,
    /// Whether this side said it is ready for the offers as they are.
    #[serde(default)]
    pub ready: bool,
    /// Whether this side confirmed.
    #[serde(default)]
    pub confirmed: bool,
}

impl TradeSide {
    /// The holocron ids it offers.
    pub fn holocrons(&self) -> Vec<u64> {
        self.items.iter().filter_map(|item| item.holocron).collect()
    }

    /// The shader ids it offers.
    pub fn shaders(&self) -> Vec<String> {
        self.items
            .iter()
            .filter(|item| !item.is_holocron() && !item.is_tag())
            .filter_map(|item| item.id.clone())
            .collect()
    }

    /// The tag copies it offers.
    pub fn tags(&self) -> Vec<crate::wire::TagCopy> {
        self.items.iter().filter_map(TradeItem::tag_copy).collect()
    }
}

/// The other person of a trade.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct TradePartner {
    /// Their id at the hub (their person's).
    pub key_id: String,
    /// Their display name, with colour codes. Show it only through
    /// [`crate::chat::for_display`].
    #[serde(default)]
    pub name: String,
    /// Whether the hub's operator vouches for them.
    #[serde(default)]
    pub verified: bool,
    /// Their picture's version; empty for none (and from hubs that do not send it).
    #[serde(default)]
    pub avatar: String,
}

/// A trade as the reader sees it.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct PlayerTrade {
    /// Its id at the hub.
    pub id: i64,
    /// `asked`, `open`, `done`, `declined`, `cancelled`, `expired` or `refused`.
    pub state: String,
    /// Whether the reader asked (else was asked).
    #[serde(default)]
    pub asker: bool,
    /// The other person.
    pub with: TradePartner,
    /// Rises with every change of either offer.
    #[serde(default)]
    pub rev: i64,
    /// The reader's side.
    #[serde(default)]
    pub mine: TradeSide,
    /// The other side.
    #[serde(default)]
    pub theirs: TradeSide,
    /// When it was asked, unix seconds.
    #[serde(default)]
    pub created: i64,
    /// Its last change, unix seconds.
    #[serde(default)]
    pub updated: i64,
    /// When it ends by itself if nothing happens, unix seconds (hub clock).
    #[serde(default)]
    pub expires: Option<i64>,
    /// From when the confirmations are taken, once both sides are ready (hub clock).
    #[serde(default)]
    pub confirm_from: Option<i64>,
    /// When it ended.
    #[serde(default)]
    pub ended: Option<i64>,
    /// Why it was refused, or ended otherwise; empty when it was not.
    #[serde(default)]
    pub reason: String,
    /// Whether one side is worth much more than the other (a warning only).
    #[serde(default)]
    pub uneven: bool,
}

impl PlayerTrade {
    /// Whether it waits for the asked player's answer.
    pub fn is_asked(&self) -> bool {
        self.state == "asked"
    }

    /// Whether both sides are choosing, or confirming.
    pub fn is_open(&self) -> bool {
        self.state == "open"
    }

    /// Whether it still runs (asked or open).
    pub fn is_live(&self) -> bool {
        self.is_asked() || self.is_open()
    }

    /// Whether it ended with the items swapped.
    pub fn is_done(&self) -> bool {
        self.state == "done"
    }

    /// How it ended, in the player's words, for an ended trade; empty while it runs. A
    /// finished trade says new items are in the collection only when the other side
    /// gave some.
    pub fn ending(&self) -> String {
        let name = crate::chat::name_for_display(&self.with.name);
        match self.state.as_str() {
            // What the reader received decides the words (hub report #208: a trade the
            // other side put nothing into said new items were in the collection).
            "done" => match (self.theirs.items.len(), self.mine.items.is_empty()) {
                (0, true) => "Trade complete. Nothing changed hands.".to_owned(),
                (0, false) => {
                    format!("Trade complete. You gave your offer to {name}^7 and received nothing.")
                }
                (1, _) => "Trade complete. Your new item is in your collection.".to_owned(),
                _ => "Trade complete. Your new items are in your collection.".to_owned(),
            },
            // The hub ends a request as `busy` when one of the two took another trade.
            "declined" if self.reason == "busy" && self.asker => {
                format!("{name}^7 is trading with someone else.")
            }
            "declined" if self.reason == "busy" => {
                "The request closed: you are trading with someone else.".to_owned()
            }
            "cancelled" if self.reason == "busy" => {
                "Your request closed: you are trading with someone else.".to_owned()
            }
            "cancelled" if self.reason == "removed" => {
                "The trade ended: an account was removed.".to_owned()
            }
            "declined" if self.asker => format!("{name}^7 declined the trade."),
            "declined" => "You declined the trade.".to_owned(),
            "cancelled" => "The trade was cancelled.".to_owned(),
            "expired" if self.updated == self.created => {
                "The trade request ran out of time.".to_owned()
            }
            "expired" => "The trade closed: nothing changed for too long.".to_owned(),
            "refused" => format!("The hub stopped the trade: {}", reason_words(&self.reason)),
            _ => String::new(),
        }
    }
}

/// What the reader traded in their trading day, against each limit.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct TradeToday {
    /// Trades completed.
    #[serde(default)]
    pub trades: i64,
    /// Holocrons received.
    #[serde(default)]
    pub holocrons_received: i64,
    /// Mythical holocrons received.
    #[serde(default)]
    pub mythical_received: i64,
    /// Holocrons given.
    #[serde(default)]
    pub holocrons_given: i64,
    /// Mythical holocrons given.
    #[serde(default)]
    pub mythical_given: i64,
    /// Duplicate shaders received.
    #[serde(default)]
    pub duplicates_received: i64,
    /// Duplicate shaders given.
    #[serde(default)]
    pub duplicates_given: i64,
    /// Holocrons received, by tier id (`rare`), the tiers that count.
    #[serde(default)]
    pub tiers_received: std::collections::BTreeMap<String, i64>,
    /// Holocrons given, by tier id.
    #[serde(default)]
    pub tiers_given: std::collections::BTreeMap<String, i64>,
    /// When the trading day ends and every count resets (hub unix seconds): 24 hours
    /// after the first trade completed in it. `None` while no trading day runs.
    #[serde(default)]
    pub resets_at: Option<i64>,
}

/// The hub's trade rules, as `GET /v1/trades` sends them.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct TradeRules {
    /// Most items a side offers.
    #[serde(default)]
    pub items_max: usize,
    /// How long a request waits, in seconds.
    #[serde(default)]
    pub ask_secs: i64,
    /// How long an open trade lives unchanged, in seconds.
    #[serde(default)]
    pub idle_secs: i64,
    /// Seconds between both being ready and the confirmations.
    #[serde(default)]
    pub confirm_delay: i64,
    /// Whether holocrons trade yet; duplicates come first. An older hub that does not
    /// say trades them.
    #[serde(default = "yes")]
    pub holocrons: bool,
    /// Holocrons of one tier one may receive, and give, in a trading day (0: no such
    /// limit).
    #[serde(default)]
    pub holocron_tier_day: i64,
    /// Holocrons one may receive in a trading day.
    #[serde(default)]
    pub holocrons_received_day: i64,
    /// Mythical holocrons one may receive in a trading day.
    #[serde(default)]
    pub mythical_received_day: i64,
    /// Mythical holocrons one may give in a trading day.
    #[serde(default)]
    pub mythical_given_day: i64,
    /// Holocrons one may give in a trading day.
    #[serde(default)]
    pub holocrons_given_day: i64,
    /// Duplicate shaders one may receive in a trading day.
    #[serde(default)]
    pub duplicates_received_day: i64,
    /// Duplicate shaders one may give in a trading day.
    #[serde(default)]
    pub duplicates_given_day: i64,
    /// Seconds a traded holocron waits before it trades again.
    #[serde(default)]
    pub resale_lock: i64,
    /// What each holocron tier is worth, in kyber shards.
    #[serde(default)]
    pub holocron_value: BTreeMap<String, i64>,
    /// What a duplicate of each shader tier is worth, lowest tier first.
    #[serde(default)]
    pub shader_value: Vec<i64>,
    /// Whether tag copies trade (since 03/11/2026); hubs before leave it out.
    #[serde(default)]
    pub tags: bool,
    /// What a tag copy is worth: its break-down yield (since 03/11/2026).
    #[serde(default)]
    pub tag_value: Option<crate::wire::TagTables>,
}

/// The hub's answer to `GET /v1/trades`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct TradesAnswer {
    /// Who may trade on this hub: `off`, `staff` or `everyone`.
    #[serde(default)]
    pub trading: String,
    /// The reader's live trades and those that ended in the last minute, newest first.
    #[serde(default)]
    pub trades: Vec<PlayerTrade>,
    /// What the reader traded today.
    #[serde(default)]
    pub today: TradeToday,
    /// The limits and worths.
    #[serde(default)]
    pub rules: TradeRules,
    /// The hub's clock when it answered, unix seconds (absent from hubs that do not
    /// say).
    #[serde(default)]
    pub now: Option<i64>,
}

impl TradesAnswer {
    /// Whether players may trade on this hub (for this player: `staff` lets staff).
    pub fn open(&self, staff: bool) -> bool {
        match self.trading.as_str() {
            "everyone" => true,
            "staff" => staff,
            _ => false,
        }
    }
}

/// The hub's answer to a step of a trade.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct TradeAnswer {
    /// The trade after the step.
    pub trade: PlayerTrade,
    /// The reader's profile, once the swap is done.
    #[serde(default)]
    pub profile: Option<Profile>,
}

/// A step the player takes in a trade (`POST /v1/trades/<id>/<action>`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TradeStep {
    /// The asked player opens the trade.
    Accept,
    /// The asked player says no.
    Decline,
    /// Either player ends it.
    Cancel,
    /// The player's side, whole.
    Offer {
        /// Holocron ids.
        holocrons: Vec<u64>,
        /// Shader unlock ids.
        shaders: Vec<String>,
        /// Tag copies (since 03/11/2026; left out of the body when empty, so older hubs
        /// read the same offer).
        tags: Vec<crate::wire::TagCopy>,
    },
    /// Ready (or not) for the offers as of revision `rev`.
    Ready {
        /// The revision seen.
        rev: i64,
        /// Ready, or taken back.
        ready: bool,
    },
    /// Confirm the swap as of revision `rev`.
    Confirm {
        /// The revision seen.
        rev: i64,
    },
}

impl TradeStep {
    /// The action's name in the path, and its body.
    pub fn call(&self) -> (&'static str, serde_json::Value) {
        use serde_json::json;
        match self {
            Self::Accept => ("accept", json!({})),
            Self::Decline => ("decline", json!({})),
            Self::Cancel => ("cancel", json!({})),
            Self::Offer {
                holocrons,
                shaders,
                tags,
            } => {
                let mut body = json!({ "holocrons": holocrons, "shaders": shaders });
                if !tags.is_empty() {
                    body["tags"] = json!(tags);
                }
                ("offer", body)
            }
            Self::Ready { rev, ready } => ("ready", json!({ "rev": rev, "ready": ready })),
            Self::Confirm { rev } => ("confirm", json!({ "rev": rev })),
        }
    }
}

/// Serde's default for a flag that is on unless said.
fn yes() -> bool {
    true
}

/// Why the hub ended a trade `refused` (its `reason`), in the player's words.
pub fn reason_words(reason: &str) -> &'static str {
    match reason {
        "not_tradable" => "an item changed hands or was opened first.",
        "holocrons_received_day" | "their_holocrons_received_day" => {
            "one of you reached the day's limit of holocrons received."
        }
        "holocrons_given_day" | "their_holocrons_given_day" => {
            "one of you reached the day's limit of holocrons given."
        }
        "mythical_received_day" | "their_mythical_received_day" => {
            "one of you reached the day's limit of Mythical holocrons received."
        }
        "mythical_given_day" | "their_mythical_given_day" => {
            "one of you reached the day's limit of Mythical holocrons given."
        }
        "duplicates_received_day" | "their_duplicates_received_day" => {
            "one of you reached the day's limit of duplicates received."
        }
        "duplicates_given_day" | "their_duplicates_given_day" => {
            "one of you reached the day's limit of duplicates given."
        }
        "holocrons_off" => "holocrons cannot be traded yet: duplicates only for now.",
        reason if tier_limit(reason).is_some() => {
            "one of you reached the day's limit for holocrons of that tier."
        }
        "merged" => "an account was merged.",
        "removed" => "an account was removed.",
        _ => "a check failed.",
    }
}

/// A daily limit refused as an item went in (`code`), in the player's words: theirs
/// ("You have given ...") or the other player's ("They have received ...").
pub fn limit_words(code: &str) -> Option<&'static str> {
    Some(match code {
        "holocrons_given_day" => "Daily limit: you have given all the holocrons you may today.",
        "mythical_given_day" => {
            "Daily limit: you have given all the Mythical holocrons you may today."
        }
        "duplicates_given_day" => "Daily limit: you have given all the duplicates you may today.",
        "holocrons_received_day" => {
            "Daily limit: you have received all the holocrons you may today."
        }
        "mythical_received_day" => {
            "Daily limit: you have received all the Mythical holocrons you may today."
        }
        "duplicates_received_day" => {
            "Daily limit: you have received all the duplicates you may today."
        }
        "their_holocrons_received_day" => {
            "Their daily limit: they cannot receive more holocrons today."
        }
        "their_mythical_received_day" => {
            "Their daily limit: they cannot receive another Mythical holocron today."
        }
        "their_duplicates_received_day" => {
            "Their daily limit: they cannot receive more duplicates today."
        }
        "their_holocrons_given_day" => "Their daily limit: they cannot give more holocrons today.",
        "their_mythical_given_day" => {
            "Their daily limit: they cannot give another Mythical holocron today."
        }
        "their_duplicates_given_day" => {
            "Their daily limit: they cannot give more duplicates today."
        }
        "holocrons_off" => "Holocrons cannot be traded yet: duplicates only for now.",
        code => tier_words(tier_limit(code)?),
    })
}

/// A one-of-each-tier limit's code: its tier, whether it is the received one, and whether
/// it is the other player's (`their_`). Mythical keeps its own older words.
fn tier_limit(code: &str) -> Option<(&'static str, bool, bool)> {
    let (theirs, code) = match code.strip_prefix("their_") {
        Some(rest) => (true, rest),
        None => (false, code),
    };
    let (tier, received) = if let Some(tier) = code.strip_suffix("_received_day") {
        (tier, true)
    } else {
        (code.strip_suffix("_given_day")?, false)
    };
    let tier = ["uncommon", "rare", "legendary"]
        .into_iter()
        .find(|known| *known == tier)?;
    Some((tier, received, theirs))
}

/// The words for a one-of-each-tier limit ([`tier_limit`]).
fn tier_words((tier, received, theirs): (&str, bool, bool)) -> &'static str {
    match (tier, received, theirs) {
        ("uncommon", false, false) => {
            "Daily limit: you have given all the Uncommon holocrons you may today."
        }
        ("uncommon", true, false) => {
            "Daily limit: you have received all the Uncommon holocrons you may today."
        }
        ("uncommon", false, true) => {
            "Their daily limit: they cannot give more Uncommon holocrons today."
        }
        ("uncommon", true, true) => {
            "Their daily limit: they cannot receive more Uncommon holocrons today."
        }
        ("rare", false, false) => {
            "Daily limit: you have given all the Rare holocrons you may today."
        }
        ("rare", true, false) => {
            "Daily limit: you have received all the Rare holocrons you may today."
        }
        ("rare", false, true) => "Their daily limit: they cannot give more Rare holocrons today.",
        ("rare", true, true) => "Their daily limit: they cannot receive more Rare holocrons today.",
        (_, false, false) => {
            "Daily limit: you have given all the Legendary holocrons you may today."
        }
        (_, true, false) => {
            "Daily limit: you have received all the Legendary holocrons you may today."
        }
        (_, false, true) => "Their daily limit: they cannot give more Legendary holocrons today.",
        (_, true, true) => "Their daily limit: they cannot receive more Legendary holocrons today.",
    }
}

/// Why a trade request failed, in the player's words.
pub fn refusal(error: &HubError) -> String {
    let words = match error {
        HubError::Rejected { code, message, .. } => match code.as_str() {
            "trading_off" => "Trading is not open on the SJK hub yet.",
            "trading_staff_only" => "Trading is being tested by the SJK team: soon.",
            "not_trusted" => {
                "You can trade once the hub trusts you (the trust mark on your Profile) or you are verified."
            }
            "they_cannot_trade" => "That player cannot trade yet.",
            "trade_self" => "That is you.",
            "same_address" => "Two accounts on one connection cannot trade with each other.",
            "trade_busy" => "You are in a trade already.",
            "they_are_busy" => "That player is in a trade already.",
            "asked_recently" => "You asked that player a moment ago: wait before asking again.",
            "asking_too_often" => "You sent many trade requests: wait a while before the next.",
            "trade_closed" => "That trade has ended.",
            "not_asked" => "Only the player asked can answer.",
            "trade_changed" => "The offer changed: look again before you are ready.",
            "trade_not_ready" => "Both players must be ready first.",
            "trade_too_soon" => "Wait a moment: the offer must settle first.",
            "not_tradable" => "That item cannot be traded.",
            "locked" => "That shader is locked: unlock it in the Collection to trade it.",
            "trade_full" => "That is the most items one side can offer.",
            "trade_empty" => "Put something in the trade first: one of you must give.",
            "holocrons_off" => "Holocrons cannot be traded yet: duplicates only for now.",
            "not_found" => "That trade is gone.",
            "not_registered" => "Your identity is not registered at the hub yet.",
            "client_update_required" => "Update SJK to trade: this version is too old.",
            limit if limit.ends_with("_day") => {
                return limit_words(limit).map_or_else(
                    || format!("Daily limit: {}", reason_words(limit)),
                    str::to_owned,
                );
            }
            _ => return message.clone(),
        },
        // A hub from before trades has no such route.
        HubError::Protocol(why)
            if why.starts_with("status 404") || why.starts_with("status 405") =>
        {
            "This SJK hub cannot trade yet."
        }
        other => return other.to_string(),
    };
    words.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trade_list_reads_as_the_hub_sends_it() {
        let answer: TradesAnswer = serde_json::from_str(
            r#"{"trading":"everyone","trades":[{"id":3,"state":"open","asker":false,
            "with":{"key_id":"ff","name":"^1Nib","verified":true},"rev":2,
            "mine":{"items":[{"kind":"holocron","holocron":7,"tier":"rare","value":8}],
            "value":8,"ready":true,"confirmed":false},
            "theirs":{"items":[{"kind":"shader","id":"saber_haze","name":"Haze","tier":"rare",
            "value":10}],"value":10,"ready":false,"confirmed":false},
            "created":100,"updated":120,"expires":420,"uneven":false}],
            "today":{"trades":1,"holocrons_received":0,"mythical_received":0,
            "holocrons_given":1,"mythical_given":0,"duplicates_received":0},
            "rules":{"items_max":6,"holocron_value":{"rare":8},"shader_value":[5,10]}}"#,
        )
        .unwrap();
        let trade = &answer.trades[0];
        assert!(trade.is_open() && trade.is_live() && !trade.asker);
        assert_eq!(trade.mine.holocrons(), [7]);
        assert_eq!(trade.theirs.shaders(), ["saber_haze"]);
        assert!(trade.theirs.tags().is_empty());
        let side: TradeSide = serde_json::from_str(
            r#"{"items":[{"kind":"tag","id":"tag_gonk","name":"Gonk tag","tier":"uncommon",
            "finish":"holo","value":25},{"kind":"shader","id":"saber_haze","tier":"rare"}]}"#,
        )
        .unwrap();
        assert_eq!(side.shaders(), ["saber_haze"]);
        assert_eq!(side.tags(), [crate::wire::TagCopy::new("tag_gonk", "holo")]);
        assert_eq!(trade.expires, Some(420));
        assert_eq!(answer.rules.holocron_value["rare"], 8);
        assert_eq!(answer.today.holocrons_given, 1);
        // An older hub that does not say trades holocrons.
        assert!(answer.rules.holocrons);
        let duplicates_first: TradesAnswer = serde_json::from_str(
            r#"{"trades":[],"today":{"tiers_given":{"rare":1},"resets_at":900},
            "rules":{"holocrons":false,"holocron_tier_day":1}}"#,
        )
        .unwrap();
        assert!(!duplicates_first.rules.holocrons);
        assert_eq!(duplicates_first.rules.holocron_tier_day, 1);
        assert_eq!(duplicates_first.today.tiers_given.get("rare"), Some(&1));
        assert_eq!(duplicates_first.today.resets_at, Some(900));
    }

    #[test]
    fn steps_carry_their_paths_and_bodies() {
        let (action, body) = TradeStep::Offer {
            holocrons: vec![7],
            shaders: vec!["saber_haze".to_owned()],
            tags: Vec::new(),
        }
        .call();
        assert_eq!(action, "offer");
        assert_eq!(
            body,
            serde_json::json!({"holocrons":[7],"shaders":["saber_haze"]})
        );
        // Tag copies go only when there are some, so older hubs read the same body.
        let (_, body) = TradeStep::Offer {
            holocrons: Vec::new(),
            shaders: Vec::new(),
            tags: vec![crate::wire::TagCopy::new("tag_gonk", "holo")],
        }
        .call();
        assert_eq!(
            body,
            serde_json::json!({"holocrons":[],"shaders":[],
                "tags":[{"id":"tag_gonk","finish":"holo"}]})
        );
        assert_eq!(
            TradeStep::Ready {
                rev: 4,
                ready: false
            }
            .call()
            .1,
            serde_json::json!({"rev":4,"ready":false})
        );
        assert_eq!(TradeStep::Accept.call(), ("accept", serde_json::json!({})));
    }

    #[test]
    fn refusals_read_in_the_players_words() {
        let rejected = |code: &str| HubError::Rejected {
            status: 409,
            code: code.to_owned(),
            message: "hub words".to_owned(),
        };
        assert_eq!(
            refusal(&rejected("they_are_busy")),
            "That player is in a trade already."
        );
        assert_eq!(
            refusal(&rejected("holocrons_given_day")),
            "Daily limit: you have given all the holocrons you may today."
        );
        assert_eq!(
            refusal(&rejected("their_holocrons_received_day")),
            "Their daily limit: they cannot receive more holocrons today."
        );
        assert_eq!(
            refusal(&rejected("rare_given_day")),
            "Daily limit: you have given all the Rare holocrons you may today."
        );
        assert_eq!(
            refusal(&rejected("their_uncommon_received_day")),
            "Their daily limit: they cannot receive more Uncommon holocrons today."
        );
        assert_eq!(
            refusal(&rejected("holocrons_off")),
            "Holocrons cannot be traded yet: duplicates only for now."
        );
        assert_eq!(
            refusal(&rejected("asked_recently")),
            "You asked that player a moment ago: wait before asking again."
        );
        assert_eq!(refusal(&rejected("something_new")), "hub words");
        // A request the hub closed because one of the two took another trade.
        let busy = |asker: bool, state: &str, reason: &str| PlayerTrade {
            state: state.to_owned(),
            reason: reason.to_owned(),
            asker,
            ..PlayerTrade::default()
        };
        assert_eq!(
            busy(true, "declined", "busy").ending(),
            "^7 is trading with someone else."
        );
        assert_eq!(
            busy(false, "declined", "busy").ending(),
            "The request closed: you are trading with someone else."
        );
        // The other's name stops its colours before the words, as the finished trade's does.
        let mut refused = busy(true, "declined", "");
        refused.with.name = "^1Zed".to_owned();
        assert_eq!(refused.ending(), "^1Zed^7 declined the trade.");
        assert_eq!(
            busy(true, "cancelled", "removed").ending(),
            "The trade ended: an account was removed."
        );
        let ended = PlayerTrade {
            state: "refused".to_owned(),
            reason: "their_mythical_received_day".to_owned(),
            ..PlayerTrade::default()
        };
        assert_eq!(
            ended.ending(),
            "The hub stopped the trade: one of you reached the day's limit of Mythical holocrons received."
        );
    }

    /// Hub report #208: a finished trade says new items are in the collection only when
    /// the other side gave some; one item reads in the singular; a trade they put nothing
    /// into says so, with their name as the other endings carry it.
    #[test]
    fn a_finished_trade_says_what_was_received() {
        let shader = |id: &str| TradeItem {
            kind: "shader".to_owned(),
            id: Some(id.to_owned()),
            ..TradeItem::default()
        };
        let done = |mine: Vec<TradeItem>, theirs: Vec<TradeItem>| PlayerTrade {
            state: "done".to_owned(),
            with: TradePartner {
                name: "^1Zed".to_owned(),
                ..TradePartner::default()
            },
            mine: TradeSide {
                items: mine,
                ..TradeSide::default()
            },
            theirs: TradeSide {
                items: theirs,
                ..TradeSide::default()
            },
            ..PlayerTrade::default()
        };
        assert_eq!(
            done(vec![shader("a")], Vec::new()).ending(),
            "Trade complete. You gave your offer to ^1Zed^7 and received nothing."
        );
        assert_eq!(
            done(Vec::new(), vec![shader("b")]).ending(),
            "Trade complete. Your new item is in your collection."
        );
        assert_eq!(
            done(vec![shader("a")], vec![shader("b"), shader("c")]).ending(),
            "Trade complete. Your new items are in your collection."
        );
        assert_eq!(
            done(Vec::new(), Vec::new()).ending(),
            "Trade complete. Nothing changed hands."
        );
        // A name's clan marks show as in the window's headline, not dropped.
        let mut marked = done(vec![shader("a")], Vec::new());
        marked.with.name = "jof\u{bb}jk\u{ab}belyash".to_owned();
        assert_eq!(
            marked.ending(),
            "Trade complete. You gave your offer to jof\u{bb}jk\u{ab}belyash^7 and received nothing."
        );
    }

    /// The hub owns the numbers (limits, waits, trust): no refusal or ending names one,
    /// so the hub can change them without a client update (Sol's review, 24/10/2026).
    #[test]
    fn no_words_name_a_number_the_hub_owns() {
        let mut codes: Vec<String> = [
            "trading_off",
            "trading_staff_only",
            "not_trusted",
            "not_tradable",
            "holocrons_off",
            "merged",
            "trade_busy",
            "they_are_busy",
            "asked_recently",
            "asking_too_often",
            "removed",
            "something_new",
        ]
        .map(str::to_owned)
        .to_vec();
        for kind in [
            "holocrons",
            "mythical",
            "duplicates",
            "uncommon",
            "rare",
            "legendary",
        ] {
            for side in ["received", "given"] {
                codes.push(format!("{kind}_{side}_day"));
                codes.push(format!("their_{kind}_{side}_day"));
            }
        }
        for code in &codes {
            let rejected = HubError::Rejected {
                status: 409,
                code: code.clone(),
                message: "hub words".to_owned(),
            };
            for words in [refusal(&rejected), reason_words(code).to_owned()] {
                assert!(
                    !words.chars().any(|c| c.is_ascii_digit()),
                    "{code}: {words}"
                );
            }
        }
        for state in ["done", "declined", "cancelled", "expired", "refused"] {
            let ended = PlayerTrade {
                state: state.to_owned(),
                updated: 5,
                ..PlayerTrade::default()
            };
            assert!(
                !ended.ending().chars().any(|c| c.is_ascii_digit()),
                "{state}"
            );
        }
    }
}
