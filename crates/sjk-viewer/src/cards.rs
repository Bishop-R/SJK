//! Cards and credits (`docs/cards.md`): what opening a holocron brings. The SJK hub rolls
//! a card's rarity, finish and edition and the credits, and keeps them; the client shows
//! them. Until the first set of cards is out every card is *sealed*: its rarity, finish
//! and edition are known, its face is chosen when it is revealed.
//!
//! This is the one place that says which rarities, finishes and editions the client knows
//! and how each reads ([`RARITIES`], [`FINISHES`], [`edition_name`]), what a holocron of
//! each tier holds ([`CREDITS`], [`inside`]) and how a profile's cards read ([`stacks`]).
//! An id the client does not know is left out.
//!
//! | piece | module |
//! | --- | --- |
//! | a card drawn from shapes, its finishes' light | [`look`] |
//! | revealed cards' faces, from the SJK packs | [`faces`] |
//! | the Cards tab's binder: its tiles, filter and sets' progress | [`binder`] |
//! | the credits' mark | [`mark`] |
//! | the loot table's odds, worked out | [`loot`] |
//! | `debug_open` | [`rehearsal`] |
//! | pictures the client draws from the game's models | [`scene`] |

pub(crate) mod binder;
pub(crate) mod faces;
pub(crate) mod look;
pub(crate) mod loot;
pub(crate) mod mark;
pub(crate) mod rehearsal;
pub(crate) mod scene;
pub(crate) mod shard;

use crate::holocrons::COUNT;
use sjk_identity::CardStack;
use sjk_ui::Color;

/// `0xRRGGBB` as an opaque colour.
const fn rgb(hex: u32) -> Color {
    Color::new(
        ((hex >> 16) & 0xff) as f32 / 255.0,
        ((hex >> 8) & 0xff) as f32 / 255.0,
        (hex & 0xff) as f32 / 255.0,
        1.0,
    )
}

/// A card's rarity, in the hub's order (the commonest first).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rarity {
    /// The hub's id (`rare`).
    pub(crate) id: &'static str,
    /// What players read ("Rare").
    pub(crate) name: &'static str,
    /// Its colour: silver for common, then the holocron tiers' colours (Uncommon's green
    /// to Mythical's).
    pub(crate) colour: Color,
    /// Position in [`RARITIES`].
    pub(crate) index: usize,
}

/// Every rarity the client knows.
pub(crate) const RARITIES: [Rarity; 5] = [
    Rarity {
        id: "common",
        name: "Common",
        colour: rgb(0xB4_C0D4),
        index: 0,
    },
    Rarity {
        id: "uncommon",
        name: "Uncommon",
        colour: crate::holocrons::TIERS[0].colour,
        index: 1,
    },
    Rarity {
        id: "rare",
        name: "Rare",
        colour: crate::holocrons::TIERS[1].colour,
        index: 2,
    },
    Rarity {
        id: "legendary",
        name: "Legendary",
        colour: crate::holocrons::TIERS[2].colour,
        index: 3,
    },
    Rarity {
        id: "mythical",
        name: "Mythical",
        colour: crate::holocrons::TIERS[3].colour,
        index: 4,
    },
];

impl Rarity {
    /// The rarity the hub's `id` names, if the client knows it.
    pub(crate) fn from_id(id: &str) -> Option<&'static Rarity> {
        RARITIES.iter().find(|rarity| rarity.id == id)
    }

    /// The colour at `alpha`.
    pub(crate) fn colour_alpha(&self, alpha: f32) -> Color {
        Color::new(self.colour.r, self.colour.g, self.colour.b, alpha)
    }

    /// How a card of it reads: its name, colour and mark.
    pub(crate) fn tone(&'static self) -> Tone<'static> {
        Tone {
            name: self.name,
            colour: self.colour,
            mark: Some(crate::symbols::card_rarity(self.index)),
        }
    }
}

/// How a card reads and is coloured wherever players see it: its face's rank when it has
/// one ([`faces::Rank`], 31/10/2026), else its rarity. A ranked card never shows its
/// rarity, which only sets its odds; a sealed copy, not a card yet, shows its rarity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Tone<'a> {
    /// What players read ("Mythical", "Leader").
    pub(crate) name: &'a str,
    pub(crate) colour: Color,
    /// The rarity's SJK Symbols mark; a rank has none.
    pub(crate) mark: Option<char>,
}

impl Tone<'_> {
    /// The tone of `face` when one is given, else of `rarity`.
    pub(crate) fn of<'a>(rarity: &'static Rarity, face: Option<&'a faces::Face>) -> Tone<'a> {
        face.map_or_else(|| rarity.tone(), faces::Face::tone)
    }
}

/// Its mark, a space and its name, or its name alone when it has no mark ("Leader").
impl std::fmt::Display for Tone<'_> {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.mark {
            Some(mark) => write!(out, "{mark} {}", self.name),
            None => out.write_str(self.name),
        }
    }
}

/// A copy's finish, in the hub's order (the plainest first).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Finish {
    /// The hub's id (`holo`).
    pub(crate) id: &'static str,
    /// What players read ("Holo").
    pub(crate) name: &'static str,
    /// What the finish looks like, one line.
    pub(crate) about: &'static str,
    /// Position in [`FINISHES`].
    pub(crate) index: usize,
}

/// Every finish the client knows.
pub(crate) const FINISHES: [Finish; 3] = [
    Finish {
        id: "normal",
        name: "Normal",
        about: "The plain print.",
        index: 0,
    },
    Finish {
        id: "holo",
        name: "Holo",
        about: "A rainbow foil shines across the card.",
        index: 1,
    },
    Finish {
        id: "solar",
        name: "Solar",
        about: "The rarest finish: a living solar corona burns along the card's edge.",
        index: 2,
    },
];

impl Finish {
    /// The finish the hub's `id` names, if the client knows it.
    pub(crate) fn from_id(id: &str) -> Option<&'static Finish> {
        FINISHES.iter().find(|finish| finish.id == id)
    }
}

/// What players read for edition `id`: "Alpha Edition"; an id the client does not know
/// reads as itself with "Edition" after it, and none as nothing.
pub(crate) fn edition_name(id: &str) -> String {
    crate::editions::name(id)
}

/// The credits a holocron of each tier holds, the least and the most, in
/// [`crate::holocrons::TIERS`] order: the hub's table (`PROTOCOL.md`, "Opening"), shown
/// on the Holocrons tab. The hub decides; this only says it.
pub(crate) const CREDITS: [(u32, u32); COUNT] = [(10, 20), (30, 50), (80, 120), (250, 350)];

/// The odds of the card inside a holocron of one tier, in parts per thousand: its rarity
/// (in [`RARITIES`] order) and its finish (in [`FINISHES`] order), rolled apart.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Odds {
    pub(crate) rarity: [u16; RARITIES.len()],
    pub(crate) finish: [u16; FINISHES.len()],
}

/// The card's odds inside a holocron of each tier, in [`crate::holocrons::TIERS`] order:
/// the hub's table (`src/cards.rs`, `TABLES`; `PROTOCOL.md`, "Opening"), shown by the loot
/// table. The hub decides; this only says it.
pub(crate) const ODDS: [Odds; COUNT] = [
    Odds {
        rarity: [700, 240, 55, 4, 1],
        finish: [970, 30, 0],
    },
    Odds {
        rarity: [500, 330, 150, 18, 2],
        finish: [900, 100, 0],
    },
    Odds {
        rarity: [300, 380, 260, 55, 5],
        finish: [700, 290, 10],
    },
    Odds {
        rarity: [100, 300, 450, 130, 20],
        finish: [400, 550, 50],
    },
];

/// What a holocron of tier `index` holds, one line: "A sealed card and 30 to 50 credits,
/// or a saber shader (40%) or kyber shards (10%)."; without the shards from a hub that
/// gives none.
pub(crate) fn inside(index: usize) -> String {
    let index = index.min(COUNT - 1);
    let odds = crate::loot_odds::current();
    let (least, most) = odds.credits[index];
    let percent = |per_mille: u32| crate::cards::loot::percent(f64::from(per_mille) / 1_000.0);
    let shader = percent(odds.shader[index]);
    if odds.tag[index] > 0 {
        let tag = percent(odds.tag[index]);
        let shards = percent(odds.shard_pile[index]);
        return format!(
            "Opening it gives a sealed card and {least} to {most} credits, or a saber shader \
             ({shader}), a tag ({tag}) or kyber shards ({shards})."
        );
    }
    if odds.shard_pile[index] == 0 {
        return format!(
            "Opening it gives a sealed card and {least} to {most} credits, or ({shader} of \
             the time) a saber shader."
        );
    }
    let shards = percent(odds.shard_pile[index]);
    format!(
        "Opening it gives a sealed card and {least} to {most} credits, or a saber shader \
         ({shader}) or kyber shards ({shards})."
    )
}

/// Copies of one kind the player holds, as the client shows them.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Stack {
    pub(crate) rarity: &'static Rarity,
    pub(crate) finish: &'static Finish,
    /// "Alpha Edition", or empty.
    pub(crate) edition: String,
    /// The card's id once revealed; `None` while sealed.
    pub(crate) card: Option<String>,
    pub(crate) count: u32,
    /// When the first of them was obtained, unix seconds.
    pub(crate) first: i64,
}

impl Stack {
    /// Whether its face is not chosen yet.
    pub(crate) fn sealed(&self) -> bool {
        self.card.is_none()
    }

    /// "Rare, Holo": the rarity and the finish (the finish left out when Normal).
    pub(crate) fn title(&self) -> String {
        if self.finish.index == 0 {
            format!("{} card", self.rarity.name)
        } else {
            format!("{} card, {}", self.rarity.name, self.finish.name)
        }
    }
}

/// A profile's cards the client knows, the best first: by finish, then rarity, then
/// edition (the hub's own order, kept whatever it sends).
pub(crate) fn stacks(list: &[CardStack]) -> Vec<Stack> {
    let mut stacks: Vec<Stack> = list
        .iter()
        .filter(|stack| stack.count > 0)
        .filter_map(|stack| {
            Some(Stack {
                rarity: Rarity::from_id(&stack.rarity)?,
                finish: Finish::from_id(&stack.finish)?,
                edition: edition_name(&stack.edition),
                card: stack.card.clone(),
                count: stack.count,
                first: stack.first,
            })
        })
        .collect();
    stacks.sort_by(|a, b| {
        (b.finish.index, b.rarity.index)
            .cmp(&(a.finish.index, a.rarity.index))
            .then_with(|| a.edition.cmp(&b.edition))
            .then_with(|| a.card.cmp(&b.card))
    });
    stacks
}

impl Stack {
    /// What players read it as: its face's name once revealed ("Unknown card <id>" when
    /// the packs have no face of it), else its sealed title.
    pub(crate) fn name_in(&self, faces: &faces::Faces) -> String {
        match &self.card {
            None => self.title(),
            Some(id) => faces.find(id).map_or_else(
                || format!("Unknown card {id}"),
                |(_, face)| face.name.clone(),
            ),
        }
    }

    /// How it reads and is coloured: its face's rank or rarity once revealed and in the
    /// packs, else its rarity's ([`Tone`]).
    pub(crate) fn tone_in<'a>(&self, faces: &'a faces::Faces) -> Tone<'a> {
        tone_in(faces, self.rarity, self.card.as_deref())
    }
}

/// The tone of a copy of `rarity` that is card `card` (`None` while sealed) ([`Tone`]).
pub(crate) fn tone_in<'a>(
    faces: &'a faces::Faces,
    rarity: &'static Rarity,
    card: Option<&str>,
) -> Tone<'a> {
    Tone::of(
        rarity,
        card.and_then(|id| faces.find(id)).map(|(_, face)| face),
    )
}

/// How a card `card` (its id, `None` while sealed) shows its face, when the packs have it.
pub(crate) fn shown<'a>(faces: &'a faces::Faces, card: Option<&str>) -> Option<look::Shown<'a>> {
    let (index, face) = faces.find(card?)?;
    Some(look::Shown { face, index })
}

/// How many sealed copies of `stacks` the hub could reveal now: those of a rarity the
/// packs have faces of.
pub(crate) fn revealable(stacks: &[Stack], faces: &faces::Faces) -> u32 {
    stacks
        .iter()
        .filter(|stack| stack.sealed() && faces.has_rarity(stack.rarity))
        .map(|stack| stack.count)
        .sum()
}

/// How many cards the stacks hold in all.
pub(crate) fn total(stacks: &[Stack]) -> u32 {
    stacks.iter().map(|stack| stack.count).sum()
}

/// `credits` as players read it: digits in groups of three, apart by a space ("12 340"),
/// as French and most of Europe write them.
pub(crate) fn credits_text(credits: u64) -> String {
    grouped(credits)
}

/// `number` in groups of three digits, apart by a space ("5 698").
pub(crate) fn grouped(number: u64) -> String {
    let digits = number.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(digit);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire(rarity: &str, finish: &str, edition: &str, count: u32) -> CardStack {
        CardStack {
            card: None,
            rarity: rarity.to_owned(),
            finish: finish.to_owned(),
            edition: edition.to_owned(),
            count,
            first: 1,
            last: 2,
        }
    }

    /// The lists are the hub's (`src/cards.rs` pins the same ids in the same order).
    #[test]
    fn the_lists_are_the_hubs() {
        let rarities: Vec<&str> = RARITIES.iter().map(|rarity| rarity.id).collect();
        assert_eq!(
            rarities,
            ["common", "uncommon", "rare", "legendary", "mythical"]
        );
        let finishes: Vec<&str> = FINISHES.iter().map(|finish| finish.id).collect();
        assert_eq!(finishes, ["normal", "holo", "solar"]);
        for (index, rarity) in RARITIES.iter().enumerate() {
            assert_eq!(rarity.index, index);
            assert_eq!(Rarity::from_id(rarity.id), Some(rarity));
        }
        for (index, finish) in FINISHES.iter().enumerate() {
            assert_eq!(finish.index, index);
        }
        assert_eq!(Rarity::from_id("ascendant_future"), None);
        assert_eq!(
            CREDITS,
            [(10, 20), (30, 50), (80, 120), (250, 350)],
            "the hub's ranges"
        );
        let odds: Vec<([u16; 5], [u16; 3])> =
            ODDS.iter().map(|odds| (odds.rarity, odds.finish)).collect();
        assert_eq!(
            odds,
            [
                ([700, 240, 55, 4, 1], [970, 30, 0]),
                ([500, 330, 150, 18, 2], [900, 100, 0]),
                ([300, 380, 260, 55, 5], [700, 290, 10]),
                ([100, 300, 450, 130, 20], [400, 550, 50]),
            ],
            "the hub's tables"
        );
        for odds in &ODDS {
            assert_eq!(
                odds.rarity.iter().map(|&p| u32::from(p)).sum::<u32>(),
                1_000
            );
            assert_eq!(
                odds.finish.iter().map(|&p| u32::from(p)).sum::<u32>(),
                1_000
            );
        }
    }

    #[test]
    fn stacks_are_the_known_ones_best_first() {
        let list = [
            wire("common", "normal", "alpha", 3),
            wire("rare", "holo", "alpha", 1),
            wire("mythic_future", "holo", "alpha", 1),
            wire("legendary", "normal", "alpha", 1),
            wire("uncommon", "solar", "alpha", 1),
            wire("rare", "glitter_future", "alpha", 1),
            wire("rare", "normal", "alpha", 0),
        ];
        let stacks = stacks(&list);
        let shown: Vec<(&str, &str, u32)> = stacks
            .iter()
            .map(|stack| (stack.rarity.id, stack.finish.id, stack.count))
            .collect();
        assert_eq!(
            shown,
            [
                ("uncommon", "solar", 1),
                ("rare", "holo", 1),
                ("legendary", "normal", 1),
                ("common", "normal", 3),
            ]
        );
        assert_eq!(total(&stacks), 6);
        assert!(stacks[0].sealed());
        assert_eq!(stacks[0].edition, "Alpha Edition");
        assert_eq!(stacks[0].title(), "Uncommon card, Solar");
        assert_eq!(stacks[2].title(), "Legendary card");
    }

    /// Revealed copies read as their face's name (or say they are unknown), and only
    /// sealed copies of a rarity the packs have faces of can be revealed.
    #[test]
    fn faces_name_copies_and_say_what_can_be_revealed() {
        let face = look::tests::face();
        let faces = faces::Faces::of_list(vec![face]);
        let mut known = wire("legendary", "holo", "alpha", 1);
        known.card = Some("sjk_alpha_010".to_owned());
        let mut unknown = wire("rare", "normal", "alpha", 1);
        unknown.card = Some("sjk_alpha_099".to_owned());
        let list = [
            known,
            unknown,
            wire("legendary", "normal", "alpha", 2),
            wire("rare", "normal", "alpha", 3),
        ];
        let stacks = stacks(&list);
        let names: Vec<String> = stacks.iter().map(|stack| stack.name_in(&faces)).collect();
        assert!(names.contains(&"Toxiee and Sol on JoF Memorial".to_owned()));
        assert!(names.contains(&"Unknown card sjk_alpha_099".to_owned()));
        assert!(names.contains(&"Legendary card".to_owned()));
        assert_eq!(
            revealable(&stacks, &faces),
            2,
            "the sealed Legendaries only"
        );
        let shown = shown(&faces, Some("sjk_alpha_010")).unwrap();
        assert_eq!(shown.index, 0);
        assert!(shown_none(&faces));
    }

    fn shown_none(faces: &faces::Faces) -> bool {
        shown(faces, None).is_none() && shown(faces, Some("sjk_alpha_099")).is_none()
    }

    #[test]
    fn editions_and_credits_read_as_words() {
        assert_eq!(edition_name("alpha"), "Alpha Edition");
        assert_eq!(edition_name("beta"), "Beta Edition");
        assert_eq!(edition_name(""), "");
        assert_eq!(credits_text(0), "0");
        assert_eq!(credits_text(999), "999");
        assert_eq!(credits_text(1_000), "1 000");
        assert_eq!(credits_text(12_340), "12 340");
        assert_eq!(credits_text(1_234_567), "1 234 567");
        assert_eq!(
            inside(1),
            "Opening it gives a sealed card and 30 to 50 credits, or a saber shader (40%) or kyber shards (10%)."
        );
        assert_eq!(inside(99), inside(COUNT - 1));
    }
}
