//! A holocron drop as an SJK chat line (`docs/holocrons.md`, "In the chat"): the game's
//! chat, the docked chat and the chat page show it where it was said, in the tier's
//! colour with a small gem before it. Other players' legendary and mythical drops read
//! `Sol found a Legendary Holocron!`; the player's own read `You found an Uncommon
//! Holocron.` (with a `!` for the two grand tiers). The identity service relays a drop
//! as a chat message with no text and [`sjk_identity::ChatMessage::holocron`] set; this
//! words it from the tier the client knows, and leaves out a tier it does not. A kyber
//! crystal's grand find (its tier `kyber_rare` and so on, [`crate::kyber`]) reads the same
//! with the crystal's name: `Sol opened a Rare Kyber Crystal: Phoenix blade!`. A find's
//! prize ([`Prize`]) is drawn in its own rarity's colour, and a shader's blade beside it
//! (`chat/view/sjk_line.rs`, 01/11/2026, Sol's request).

use super::Tier;
use sjk_identity::ChatMessage;

/// The words of one drop line.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Words {
    /// The whole sentence, in the tier's colour and without colour codes. The line has
    /// no separate name: the sender is in the sentence.
    pub(crate) text: String,
    /// The sentence without its subject (`found a Legendary Holocron!`), for the chat
    /// page, which sets the sender's name in a column of its own.
    pub(crate) rest: String,
    pub(crate) tier: &'static Tier,
    /// A grand find's prize, which ends both sentences just before their `!`.
    pub(crate) prize: Option<Prize>,
}

/// What a grand find gave, as its line shows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Prize {
    /// The prize's words' length in bytes: they end the sentence, before its `!`
    /// ([`Prize::range`]).
    pub(crate) len: usize,
    /// The prize's rarity's colour (a shader's tier, a card's rarity or rank), which can
    /// differ from the holocron's: a Rare crystal can give a Legendary blade.
    pub(crate) colour: sjk_ui::Color,
    /// The blade skin a shader prize is, when the client knows it: the line draws it.
    pub(crate) skin: Option<&'static crate::unlockables::Unlockable>,
}

impl Prize {
    /// Where the prize's words are in `sentence` (the line's `text` or `rest`).
    pub(crate) fn range(&self, sentence: &str) -> std::ops::Range<usize> {
        let end = sentence.len().saturating_sub(1);
        end.saturating_sub(self.len)..end
    }
}

/// The longest shader name a find's line shows, in characters.
const PRIZE_NAME_MAX: usize = 40;

/// The words of `message` if it is a drop (or a grand find) of a tier the client knows.
pub(crate) fn words(message: &ChatMessage) -> Option<Words> {
    let mark = message.holocron.as_ref()?;
    let tier = super::any(&mark.tier)?;
    let end = if tier.is_announced() { '!' } else { '.' };
    let (rest, prize) = match &mark.find {
        Some(find) => {
            let words = prize(find);
            let rest = format!("opened {} {}: {words}!", tier.article(), tier.title());
            (rest, Some(prize_look(find, words.len(), tier)))
        }
        None => (
            format!("found {} {}{end}", tier.article(), tier.title()),
            None,
        ),
    };
    let text = if mark.own {
        format!("You {rest}")
    } else {
        let who =
            crate::text::Plain(&sjk_identity::chat::name_for_display(&message.name)).to_string();
        let who = who.trim();
        let who = if who.is_empty() { "Someone" } else { who };
        format!("{who} {rest}")
    };
    Some(Words {
        text,
        rest,
        tier,
        prize,
    })
}

/// How a find's prize of `len` bytes of words shows: its rarity's colour (the holocron
/// `tier`'s when the client does not know it) and, for a shader, its blade skin.
fn prize_look(find: &sjk_identity::FindMark, len: usize, tier: &'static Tier) -> Prize {
    if find.shader {
        let skin = crate::unlockables::blade_skin(&find.skin);
        let colour = skin.map_or_else(
            || super::Tier::from_id(&find.rarity).map_or(tier.colour, |tier| tier.colour),
            |skin| skin.tier.colour(),
        );
        return Prize { len, colour, skin };
    }
    let faces = crate::cards::faces::current();
    let card = Some(find.card.as_str()).filter(|card| !card.is_empty());
    let colour = crate::cards::Rarity::from_id(&find.rarity).map_or(tier.colour, |rarity| {
        crate::cards::tone_in(&faces, rarity, card).colour
    });
    Prize {
        len,
        colour,
        skin: None,
    }
}

/// What a grand find gave, in words: the shader's name (plain, cut short), or "a
/// Legendary card" with its finish when not plain ("an Uncommon card, Solar").
fn prize(find: &sjk_identity::FindMark) -> String {
    if find.shader {
        let name = crate::text::Plain(&find.name).to_string();
        let name: String = name.trim().chars().take(PRIZE_NAME_MAX).collect();
        if name.is_empty() {
            return "a saber shader".to_owned();
        }
        // A tag that came Holo or Solar (31/10/2026) says so: "Solar Gonk tag".
        return match crate::cards::Finish::from_id(&find.finish) {
            Some(finish) if finish.index > 0 => format!("{} {name}", finish.name),
            _ => name,
        };
    }
    // A ranked card says its rank, never its rarity.
    let faces = crate::cards::faces::current();
    let card = Some(find.card.as_str()).filter(|card| !card.is_empty());
    let rarity = crate::cards::Rarity::from_id(&find.rarity).map_or("rare", |rarity| {
        crate::cards::tone_in(&faces, rarity, card).name
    });
    let article = if rarity.starts_with(['A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    };
    match crate::cards::Finish::from_id(&find.finish) {
        Some(finish) if finish.index > 0 => format!("{article} {rarity} card, {}", finish.name),
        _ => format!("{article} {rarity} card"),
    }
}

/// Whether `message` is a drop the client cannot show (a tier it does not know): the
/// chat leaves it out.
pub(crate) fn unknown(message: &ChatMessage) -> bool {
    message.holocron.is_some() && words(message).is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sjk_identity::DropMark;

    fn drop(name: &str, tier: &str, own: bool) -> ChatMessage {
        ChatMessage {
            reactions: Default::default(),
            id: 5,
            at: 0,
            key_id: "aa".to_owned(),
            name: name.to_owned(),
            verified: true,
            tick: sjk_identity::Tick::None,
            staff: false,
            creator: false,
            trust: None,
            emoji: String::new(),
            client: String::new(),
            text: String::new(),
            holocron: Some(DropMark {
                tier: tier.to_owned(),
                own,
                find: None,
            }),
            official: None,
            channel: Default::default(),
        }
    }

    fn said(message: &ChatMessage) -> String {
        words(message).expect("a drop").text
    }

    fn find(name: &str, tier: &str, own: bool, mark: sjk_identity::FindMark) -> ChatMessage {
        let mut message = drop(name, tier, own);
        if let Some(drop) = message.holocron.as_mut() {
            drop.find = Some(mark);
        }
        message
    }

    /// A Solar tag's find says its finish (31/10/2026).
    #[test]
    fn a_solar_tag_find_says_its_finish() {
        let mark = sjk_identity::FindMark {
            finish: "solar".to_owned(),
            ..shader("Gonk tag")
        };
        assert_eq!(
            said(&find("Sol", "rare", false, mark)),
            "Sol opened a Rare Holocron: Solar Gonk tag!"
        );
    }

    fn shader(name: &str) -> sjk_identity::FindMark {
        sjk_identity::FindMark {
            shader: true,
            name: name.to_owned(),
            rarity: "mythical".to_owned(),
            finish: String::new(),
            card: String::new(),
            skin: String::new(),
        }
    }

    fn card(rarity: &str, finish: &str) -> sjk_identity::FindMark {
        sjk_identity::FindMark {
            shader: false,
            name: String::new(),
            rarity: rarity.to_owned(),
            finish: finish.to_owned(),
            card: String::new(),
            skin: String::new(),
        }
    }

    #[test]
    fn grand_finds_say_who_opened_what_and_what_it_gave() {
        assert_eq!(
            said(&find("^2Sol", "mythical", false, shader("Phoenix blade"))),
            "Sol opened a Mythical Holocron: Phoenix blade!"
        );
        assert_eq!(
            said(&find("Fox", "uncommon", false, card("legendary", "normal"))),
            "Fox opened an Uncommon Holocron: a Legendary card!"
        );
        assert_eq!(
            said(&find("Fox", "mythical", false, card("mythical", "holo"))),
            "Fox opened a Mythical Holocron: a Mythical card, Holo!"
        );
        assert_eq!(
            said(&find("Fox", "rare", true, card("uncommon", "solar"))),
            "You opened a Rare Holocron: an Uncommon card, Solar!"
        );
        let words = words(&find("Fox", "legendary", false, shader("^1Red ^7blade"))).unwrap();
        assert_eq!(words.rest, "opened a Legendary Holocron: Red blade!");
        assert_eq!(
            said(&find("Fox", "rare", false, shader("  "))),
            "Fox opened a Rare Holocron: a saber shader!"
        );
        let long = "x".repeat(90);
        assert!(said(&find("Fox", "rare", false, shader(&long))).len() < 90);
        // A kyber crystal's grand find says Kyber Crystal, in the crystal's colour.
        let crystal =
            super::words(&find("^2Sol", "kyber_rare", false, shader("Phoenix blade"))).unwrap();
        assert_eq!(
            crystal.text,
            "Sol opened a Rare Kyber Crystal: Phoenix blade!"
        );
        assert_eq!(crystal.tier.colour, crate::holocrons::TIERS[1].colour);
        assert_eq!(
            said(&find("Fox", "kyber_uncommon", true, shader("Tidal blade"))),
            "You opened an Uncommon Kyber Crystal: Tidal blade!"
        );
        // A tier the client does not know is left out, as a drop's.
        assert!(unknown(&find("Fox", "from_the_future", false, shader("A"))));
    }

    /// A find's prize is marked where it ends the sentence, in its own rarity's colour
    /// (not the holocron's), with its blade when the client knows the skin (01/11/2026).
    #[test]
    fn a_finds_prize_is_marked_in_its_rarity_colour_with_its_blade() {
        let mark = sjk_identity::FindMark {
            skin: "saber_phoenix".to_owned(),
            ..shader("Phoenix blade")
        };
        let phoenix = crate::unlockables::blade_skin("saber_phoenix").unwrap();
        let words = words(&find("Fox", "kyber_rare", false, mark)).unwrap();
        let prize = words.prize.expect("a prize");
        assert_eq!(&words.text[prize.range(&words.text)], "Phoenix blade");
        assert_eq!(&words.rest[prize.range(&words.rest)], "Phoenix blade");
        assert_eq!(prize.colour, phoenix.tier.colour());
        assert_eq!(prize.skin.map(|skin| skin.id), Some("saber_phoenix"));
        // Without the skin's id (an older hub), the shader's tier gives the colour.
        let words = words_of(&find("Fox", "rare", false, shader("Phoenix blade")));
        let prize = words.prize.unwrap();
        assert!(prize.skin.is_none());
        assert_eq!(prize.colour, crate::holocrons::TIERS[3].colour);
        // A card's prize is its rarity's colour; a plain drop has no prize.
        let words = words_of(&find("Fox", "uncommon", false, card("legendary", "holo")));
        let prize = words.prize.unwrap();
        assert_eq!(
            &words.text[prize.range(&words.text)],
            "a Legendary card, Holo"
        );
        assert_eq!(prize.colour, crate::holocrons::TIERS[2].colour);
        assert!(words_of(&drop("Fox", "legendary", false)).prize.is_none());
    }

    fn words_of(message: &ChatMessage) -> Words {
        words(message).unwrap()
    }

    #[test]
    fn other_players_grand_drops_and_the_players_own_are_worded_as_decided() {
        assert_eq!(
            said(&drop("^2Sol", "legendary", false)),
            "Sol found a Legendary Holocron!"
        );
        assert_eq!(
            said(&drop("Fox", "mythical", false)),
            "Fox found a Mythical Holocron!"
        );
        assert_eq!(
            said(&drop("^2Sol", "uncommon", true)),
            "You found an Uncommon Holocron."
        );
        assert_eq!(
            said(&drop("^2Sol", "rare", true)),
            "You found a Rare Holocron."
        );
        assert_eq!(
            said(&drop("^2Sol", "legendary", true)),
            "You found a Legendary Holocron!"
        );
    }

    #[test]
    fn the_page_gets_the_sentence_without_its_subject() {
        for own in [false, true] {
            let words = words(&drop("^2Sol", "legendary", own)).unwrap();
            assert_eq!(words.rest, "found a Legendary Holocron!");
        }
        assert_eq!(
            words(&drop("Sol", "uncommon", false)).unwrap().rest,
            "found an Uncommon Holocron."
        );
    }

    #[test]
    fn the_sender_is_plain_text_and_never_nothing() {
        assert_eq!(
            said(&drop("^1S^2o^3l \u{221E}", "legendary", false)),
            "Sol found a Legendary Holocron!"
        );
        assert_eq!(
            said(&drop("", "mythical", false)),
            "Someone found a Mythical Holocron!"
        );
        assert_eq!(
            said(&drop("^1^2", "mythical", false)),
            "Someone found a Mythical Holocron!"
        );
    }

    #[test]
    fn the_tier_colours_the_line() {
        let words = words(&drop("Sol", "rare", false)).unwrap();
        assert_eq!(words.tier.id, "rare");
        assert_eq!(words.tier.colour, crate::holocrons::TIERS[1].colour);
    }

    #[test]
    fn an_unknown_tier_or_a_plain_message_is_no_drop_line() {
        let future = drop("Sol", "from_the_future", false);
        assert!(words(&future).is_none() && unknown(&future));
        let mut plain = drop("Sol", "rare", false);
        plain.holocron = None;
        assert!(words(&plain).is_none() && !unknown(&plain));
    }
}
