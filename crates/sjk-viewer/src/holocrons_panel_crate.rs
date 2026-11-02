//! The opening's crate spin (`docs/holocrons.md`, "Opening"): before a holocron's burst, a
//! strip of tiles runs past a gold marker and slows to a stop on what the hub rolled, as a
//! loot crate does. The hub has rolled already: the strip only dresses it, so the winner
//! is the real prize, put at a fixed place of a strip of made-up neighbours, and the stop
//! lands somewhere inside its tile (never always dead centre).
//!
//! The neighbours are what the holocron's tier can drop and nothing else: the cards it can
//! hold (each rarity and finish with odds above zero), the droppable shaders it can bring
//! and its pile of kyber shards, each weighted by its chance, by the hub's odds
//! ([`crate::loot_odds`]); the hub decides, this only dresses it. A kyber crystal always
//! gives a shader, so its strip is shaders only, by the crystal's odds ([`crate::kyber`]). A card shows as its
//! credits, with the card tucked face down in a corner (18/10/2026): the prize's are what
//! the hub gave, a neighbour's a whole number in its tier's range.

use crate::cards::{FINISHES, RARITIES};
use crate::unlockables::{self, Rarity, Unlockable};

/// Tiles in a strip, and the one the stop lands on.
pub(crate) const TILES: usize = 58;
pub(crate) const WIN: usize = 46;
/// A tile's width and the gap after it (frame pixels).
pub(crate) const TILE: f32 = 176.0;
pub(crate) const GAP: f32 = 14.0;
/// How far the stop may land from the winner's middle, as a share of a tile's half: up
/// to near its edges, so the last tile is a close call, but always inside it (the hub
/// rolled it; the strip never shows a near miss that was not).
const JITTER: f32 = 0.9;

/// A strip's tiles: their width and the gap after each (frame pixels).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Geometry {
    pub(crate) tile: f32,
    pub(crate) gap: f32,
}

impl Geometry {
    /// From one tile's left edge to the next one's.
    pub(crate) fn pitch(self) -> f32 {
        self.tile + self.gap
    }
}

/// The strip of one holocron opened on its own.
pub(crate) const SINGLE: Geometry = Geometry {
    tile: TILE,
    gap: GAP,
};
/// One of the strips of an opening of many, five at once.
pub(crate) const STACKED: Geometry = Geometry {
    tile: 92.0,
    gap: 12.0,
};

/// What a tile shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Item {
    /// A card of rarity and finish (positions in [`RARITIES`] and [`FINISHES`]) out of a
    /// holocron of tier `tier`, with its credits: the strip shows it as its credits with
    /// the card tucked face down in a corner, its back in the holocron's colour
    /// (18/10/2026). `credits` is what the hub gave for the prize, and for a neighbour an
    /// amount in its tier's range (0 in [`pool`], filled in by [`strip`]).
    Card {
        rarity: usize,
        finish: usize,
        tier: usize,
        credits: u64,
    },
    /// A saber shader.
    Shader(&'static Unlockable),
    /// A saber shader newer than this client (only ever the prize).
    New,
    /// A tag (`tags.rs`, 27/10/2026; only ever the prize: the strip's neighbours are cards,
    /// shaders and shards).
    Tag(&'static crate::tags::Tag),
    /// A profile cover, shown as the selected prize.
    Banner(&'static crate::profile_card::banner::Banner),
    /// A pile of kyber shards (17/10/2026): `shards` of them for the prize, 0 for a
    /// neighbour (its tier's range is shown instead).
    Shards { tier: usize, shards: u64 },
}

impl Item {
    /// The colour it is shown in: a card's rarity's (gold for Solar), a shader's tier's.
    pub(crate) fn colour(self) -> sjk_ui::Color {
        match self {
            Self::Card { finish: 2, .. } => crate::menu::sjk::color::GOLD_BRIGHT,
            Self::Card { rarity, .. } => RARITIES[rarity].colour,
            Self::Shader(skin) => skin.tier.colour(),
            Self::Tag(tag) => tag.tier.colour(),
            Self::Banner(banner) => banner.tier.colour(),
            Self::New => crate::menu::sjk::color::GOLD,
            Self::Shards { .. } => crate::cards::shard::KYBER,
        }
    }

    /// How fine a find it is, for the order of an opening's finds (highest first): a
    /// shader's tier or a card's rarity (Common to Mythical), then a card's finish.
    pub(crate) fn grade(self) -> u32 {
        match self {
            Self::Card { rarity, finish, .. } => rarity as u32 * 10 + finish as u32 * 4,
            Self::Shader(skin) => skin.tier as u32 * 10 + 1,
            // A tag stands just under a shader of its tier.
            Self::Tag(tag) => tag.tier as u32 * 10,
            Self::Banner(banner) => banner.tier as u32 * 10,
            Self::New => 21,
            // A consolation: above a plain Common card, below everything rarer.
            Self::Shards { .. } => 2,
        }
    }

    /// A find worth the fanfare: a Legendary or Mythical card, a Solar one, a Legendary or
    /// Mythical shader.
    pub(crate) fn grand(self) -> bool {
        match self {
            Self::Card { rarity, finish, .. } => rarity >= 3 || finish == 2,
            Self::Shader(skin) => skin.tier >= Rarity::Legendary,
            Self::Tag(tag) => tag.tier >= Rarity::Legendary,
            Self::Banner(banner) => banner.tier >= Rarity::Legendary,
            Self::New | Self::Shards { .. } => false,
        }
    }
}

/// The shaders an opening can bring from shader tier `tier`: its droppable ones, else
/// those of the nearest commoner tier that has any, else the commonest above (the hub's
/// `loot::pick_shader`).
pub(crate) fn shaders_of(tier: Rarity) -> Vec<&'static Unlockable> {
    let of = |tier: Rarity| -> Vec<&'static Unlockable> {
        unlockables::blade_skins()
            .filter(|skin| skin.droppable && skin.tier == tier)
            .collect()
    };
    const ORDER: [Rarity; 5] = [
        Rarity::Common,
        Rarity::Uncommon,
        Rarity::Rare,
        Rarity::Legendary,
        Rarity::Mythical,
    ];
    let at = ORDER.iter().position(|&t| t == tier).unwrap_or(0);
    for below in (0..=at).rev() {
        let pool = of(ORDER[below]);
        if !pool.is_empty() {
            return pool;
        }
    }
    ORDER[at + 1..]
        .iter()
        .map(|&tier| of(tier))
        .find(|pool| !pool.is_empty())
        .unwrap_or_default()
}

/// The saber shader tiers a drop rolls among, commonest first.
const SHADER_RARITIES: [Rarity; 5] = crate::kyber::SHADER_RARITIES;

/// Add the shaders a drop brings `shader` times in a thousand, its tier by `shares`
/// (common to mythical, per mille), to `pool`: a tier's share split evenly among its
/// droppable shaders, a tier without any falling back as the hub does.
fn add_shaders(pool: &mut Vec<(Item, u64)>, shader: u64, shares: &[u32]) {
    for (at, share) in shares.iter().enumerate() {
        let skins = shaders_of(SHADER_RARITIES[at]);
        if *share == 0 || skins.is_empty() {
            continue;
        }
        let weight = shader * u64::from(*share) * 1_000 / skins.len() as u64;
        for skin in skins {
            // A shader reached by a fallback adds to its tier's weight.
            match pool
                .iter_mut()
                .find(|(item, _)| *item == Item::Shader(skin))
            {
                Some((_, sum)) => *sum += weight,
                None => pool.push((Item::Shader(skin), weight)),
            }
        }
    }
}

/// Everything a holocron (or kyber crystal) of the tier at slot `tier`
/// ([`crate::holocrons::slot`]) can bring, each with its weight (parts of a thousand
/// million). A crystal brings a shader and nothing else.
pub(crate) fn pool(tier: usize) -> Vec<(Item, u64)> {
    if let Some(crystal) = tier.checked_sub(crate::holocrons::COUNT) {
        let odds = crate::kyber::current();
        let mut pool = Vec::new();
        let shares = odds.shader_tier[crystal.min(crate::kyber::COUNT - 1)];
        add_shaders(&mut pool, 1_000, &shares);
        return pool;
    }
    let table = crate::loot_odds::current();
    let shader = u64::from(table.shader[tier]);
    let pile = u64::from(table.shard_pile[tier]);
    // A tag (27/10/2026) stands only as the prize: its share leaves the cards'.
    let tag = u64::from(table.tag[tier]);
    let odds = &table.card[tier];
    let mut pool = Vec::new();
    if pile > 0 {
        pool.push((Item::Shards { tier, shards: 0 }, pile * 1_000_000));
    }
    for rarity in 0..RARITIES.len() {
        for finish in 0..FINISHES.len() {
            let weight = (1_000 - shader - pile - tag)
                * u64::from(odds.rarity[rarity])
                * u64::from(odds.finish[finish]);
            if weight > 0 {
                pool.push((
                    Item::Card {
                        rarity,
                        finish,
                        tier,
                        credits: 0,
                    },
                    weight,
                ));
            }
        }
    }
    add_shaders(&mut pool, shader, &table.shader_tier[tier]);
    pool
}

/// A small deterministic generator (splitmix64): the same holocron always spins the same
/// strip.
struct Mix(u64);

impl Mix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// 0 up to but not including 1.
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// The strip of a holocron of tier `tier` numbered `holocron` that ends on `prize`: made-up
/// neighbours picked by the tier's odds and the prize in place [`WIN`]; a neighbouring
/// card's credits are a whole number in the tier's range (the hub's `credits`).
pub(crate) fn strip(tier: usize, holocron: u64, prize: Item) -> Vec<Item> {
    let pool = pool(tier);
    if pool.is_empty() {
        // Nothing this client knows can come out (no droppable shader at all): the prize
        // alone.
        return vec![prize; TILES];
    }
    let total: u64 = pool.iter().map(|(_, weight)| weight).sum();
    let mut mix = Mix(holocron ^ 0x5EED_C4A7_E000_0000);
    // Its own generator, so the credits do not change which neighbours come up.
    let mut amounts = Mix(holocron ^ 0xC4ED_175E_ED00_0000);
    let (least, most) = crate::loot_odds::current().credits[tier.min(crate::holocrons::COUNT - 1)];
    let (least, most) = (u64::from(least), u64::from(most.max(least)));
    let mut tiles: Vec<Item> = (0..TILES)
        .map(|_| {
            let mut roll = mix.next() % total.max(1);
            let mut picked = pool[0].0;
            for (item, weight) in &pool {
                if roll < *weight {
                    picked = *item;
                    break;
                }
                roll -= weight;
            }
            let credits = least + amounts.next() % (most - least + 1);
            match picked {
                Item::Card {
                    rarity,
                    finish,
                    tier,
                    ..
                } => Item::Card {
                    rarity,
                    finish,
                    tier,
                    credits,
                },
                other => other,
            }
        })
        .collect();
    tiles[WIN] = prize;
    tiles
}

/// Where the stop lands inside the winner's tile, as a share of the tile's width from its
/// middle (-0.45 to 0.45): the same for the same holocron.
pub(crate) fn landing(holocron: u64) -> f32 {
    let mut mix = Mix(holocron.wrapping_mul(31) ^ 0xA11C_E000);
    (mix.unit() * 2.0 - 1.0) * JITTER * 0.5
}

/// How far of the whole run the strip has gone at `t` (0 to 1 through the spin): quick
/// off the mark, then slowing without a lurch to a standstill.
pub(crate) fn travelled(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(4)
}

/// The distance the whole run covers (frame pixels), from tile 0 at the marker to the
/// stop inside the winner's tile, on a strip of `geometry`.
pub(crate) fn run(holocron: u64, geometry: Geometry) -> f32 {
    WIN as f32 * geometry.pitch() + landing(holocron) * geometry.tile
}

/// The tile under the marker after `distance` of the run.
pub(crate) fn under_marker(distance: f32, geometry: Geometry) -> usize {
    ((distance / geometry.pitch()) + 0.5).floor().max(0.0) as usize
}

/// How fast the strip runs at `t` (0 to 1 through the spin), as a share of the whole run
/// a second of a spin `seconds` long.
pub(crate) fn speed(t: f32, seconds: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    4.0 * (1.0 - t).powi(3) / seconds.max(1e-3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards;
    use crate::holocrons::{COUNT, TIERS};

    #[test]
    fn the_shader_odds_rise_with_the_tier() {
        let table = crate::loot_odds::current();
        assert!(table.shader.windows(2).all(|pair| pair[0] <= pair[1]));
        for (odds, tier) in table.shader_tier.iter().zip(&TIERS) {
            assert_eq!(odds.iter().sum::<u32>(), 1_000, "{}", tier.id);
        }
    }

    #[test]
    fn a_pool_holds_only_what_the_tier_can_drop() {
        for tier in 0..COUNT {
            let pool = pool(tier);
            assert!(!pool.is_empty());
            for (item, weight) in &pool {
                assert!(*weight > 0);
                match item {
                    Item::Card {
                        rarity,
                        finish,
                        tier: of,
                        credits,
                    } => {
                        assert_eq!((*of, *credits), (tier, 0), "a pool's card, no credits yet");
                        assert!(cards::ODDS[tier].rarity[*rarity] > 0);
                        assert!(cards::ODDS[tier].finish[*finish] > 0);
                    }
                    Item::Shader(skin) => assert!(skin.droppable, "{}", skin.id),
                    Item::New | Item::Tag(_) | Item::Banner(_) => {
                        panic!("only a prize is new or a tag")
                    }
                    Item::Shards { tier: of, shards } => {
                        assert_eq!((*of, *shards), (tier, 0), "a neighbour's pile");
                    }
                }
            }
        }
        // A Solar card cannot come out of an Uncommon holocron; a Common shader can.
        let uncommon = pool(0);
        assert!(
            !uncommon
                .iter()
                .any(|(item, _)| matches!(item, Item::Card { finish: 2, .. }))
        );
        assert!(
            uncommon
                .iter()
                .any(|(item, _)| matches!(item, Item::Shader(s) if s.id == "saber_tidal"))
        );
        // Nothing the holocrons never drop, whatever the tier: a medal's shader.
        for tier in 0..COUNT {
            assert!(
                !pool(tier)
                    .iter()
                    .any(|(item, _)| matches!(item, Item::Shader(s) if s.id == "saber_glitch"))
            );
        }
    }

    /// A kyber crystal's strip is shaders only (it always gives one), droppable ones, by
    /// the crystal's odds: a Mythical shader can come up in every tier, a Common never out
    /// of a Mythical crystal.
    #[test]
    fn a_kyber_crystals_strip_shows_only_shaders() {
        use crate::unlockables::Rarity;
        for crystal in &crate::kyber::TIERS {
            let pool = pool(crystal.slot);
            assert!(!pool.is_empty(), "{}", crystal.id);
            for (item, _) in &pool {
                assert!(
                    matches!(item, Item::Shader(skin) if skin.droppable),
                    "{}: {item:?}",
                    crystal.id
                );
            }
            let strip = strip(crystal.slot, 4_000_000_001, Item::New);
            assert_eq!(strip.len(), TILES);
            assert!(
                strip
                    .iter()
                    .enumerate()
                    .all(|(at, item)| at == WIN || matches!(item, Item::Shader(_)))
            );
        }
        let tiers_in = |slot: usize| -> Vec<Rarity> {
            pool(slot)
                .iter()
                .filter_map(|(item, _)| match item {
                    Item::Shader(skin) => Some(skin.tier),
                    _ => None,
                })
                .collect()
        };
        let mythical = crate::kyber::TIERS[3].slot;
        assert!(!tiers_in(mythical).contains(&Rarity::Common));
        for crystal in &crate::kyber::TIERS {
            assert!(
                tiers_in(crystal.slot).contains(&Rarity::Mythical),
                "{}",
                crystal.id
            );
        }
        // The weights are the crystal's odds: a Rare crystal's Legendary share.
        let rare = pool(crate::kyber::TIERS[1].slot);
        let all: u64 = rare.iter().map(|(_, weight)| weight).sum();
        let legendary: u64 = rare
            .iter()
            .filter(|(item, _)| matches!(item, Item::Shader(s) if s.tier == Rarity::Legendary))
            .map(|(_, weight)| weight)
            .sum();
        let share = legendary as f64 / all as f64 * 1_000.0;
        assert!((share - 85.0).abs() < 1.0, "{share}");
    }

    #[test]
    fn a_shader_chance_adds_up_to_its_tables_share() {
        for (tier, odds) in crate::loot_odds::current().shader.iter().enumerate() {
            let pool = pool(tier);
            let shaders: u64 = pool
                .iter()
                .filter(|(item, _)| matches!(item, Item::Shader(_)))
                .map(|(_, weight)| weight)
                .sum();
            let all: u64 = pool.iter().map(|(_, weight)| weight).sum();
            let share = shaders as f64 / all as f64 * 1_000.0;
            assert!((share - f64::from(*odds)).abs() < 1.0, "{share}");
        }
    }

    /// A pile of shards comes up as often as the hub's odds say, and the cards' share
    /// shrinks by it.
    #[test]
    fn a_pile_of_shards_comes_up_at_its_tables_share() {
        let table = crate::loot_odds::current();
        for tier in 0..COUNT {
            let pool = pool(tier);
            let all: u64 = pool.iter().map(|(_, weight)| weight).sum();
            let piles: u64 = pool
                .iter()
                .filter(|(item, _)| matches!(item, Item::Shards { .. }))
                .map(|(_, weight)| weight)
                .sum();
            let share = piles as f64 / all as f64 * 1_000.0;
            assert!(
                (share - f64::from(table.shard_pile[tier])).abs() < 1.0,
                "{share}"
            );
        }
        let pile = Item::Shards {
            tier: 3,
            shards: 140,
        };
        assert!(!pile.grand());
        assert!(
            pile.grade()
                > Item::Card {
                    rarity: 0,
                    finish: 0,
                    tier: 0,
                    credits: 0,
                }
                .grade()
        );
        assert!(
            pile.grade()
                < Item::Card {
                    rarity: 1,
                    finish: 0,
                    tier: 0,
                    credits: 0,
                }
                .grade()
        );
        assert_eq!(pile.colour(), crate::cards::shard::KYBER);
    }

    #[test]
    fn a_strip_ends_on_its_prize_and_is_the_same_each_time() {
        let prize = Item::Card {
            rarity: 3,
            finish: 1,
            tier: 2,
            credits: 97,
        };
        let strip_a = strip(2, 17, prize);
        assert_eq!(strip_a.len(), TILES);
        assert_eq!(strip_a[WIN], prize);
        assert_eq!(strip_a, strip(2, 17, prize));
        assert_ne!(strip_a, strip(2, 18, prize));
    }

    /// A card on the strip shows its credits: the prize what the hub gave, a neighbour a
    /// whole number in its tier's range, not all the same.
    #[test]
    fn a_strips_cards_carry_credits_in_their_tiers_range() {
        let table = crate::loot_odds::current();
        for (tier, (holocron_tier, (least, most))) in TIERS.iter().zip(table.credits).enumerate() {
            let prize = Item::Card {
                rarity: 0,
                finish: 0,
                tier,
                credits: 1_234,
            };
            let mut seen = Vec::new();
            for holocron in 0..40 {
                let tiles = strip(tier, holocron, prize);
                assert_eq!(tiles[WIN], prize);
                for (at, tile) in tiles.iter().enumerate() {
                    if let (false, Item::Card { credits, .. }) = (at == WIN, tile) {
                        assert!(
                            (u64::from(least)..=u64::from(most)).contains(credits),
                            "{} {credits}",
                            holocron_tier.id
                        );
                        seen.push(*credits);
                    }
                }
            }
            seen.sort_unstable();
            seen.dedup();
            assert!(seen.len() > 3, "{}: {seen:?}", holocron_tier.id);
        }
        // The hub's copy: Uncommon 10 to 20 credits, Mythical 250 to 350.
        assert_eq!(crate::loot_odds::BUILT_IN.credits[0], (10, 20));
        assert_eq!(crate::loot_odds::BUILT_IN.credits[3], (250, 350));
    }

    #[test]
    fn the_run_slows_to_a_stop_inside_the_winning_tile() {
        assert_eq!(travelled(0.0), 0.0);
        assert_eq!(travelled(1.0), 1.0);
        // Slowing: each step covers less than the one before.
        let steps: Vec<f32> = (0..10)
            .map(|i| travelled((i + 1) as f32 / 10.0) - travelled(i as f32 / 10.0))
            .collect();
        assert!(steps.windows(2).all(|pair| pair[0] > pair[1]));
        for holocron in 0..50 {
            let landing = landing(holocron);
            assert!(landing.abs() <= JITTER * 0.5 + 1e-6);
            for geometry in [SINGLE, STACKED] {
                assert_eq!(
                    under_marker(run(holocron, geometry), geometry),
                    WIN,
                    "holocron {holocron}"
                );
            }
        }
        // The speed is the run's slope: fast off the mark, still at the stop.
        assert!(speed(0.0, 5.0) > speed(0.5, 5.0));
        assert_eq!(speed(1.0, 5.0), 0.0);
        let (t, dt) = (0.3, 1e-3);
        let slope = (travelled(t + dt) - travelled(t)) / (dt * 5.0);
        assert!((slope - speed(t, 5.0)).abs() < 1e-2, "{slope}");
    }

    #[test]
    fn the_finest_finds_come_first_and_the_grand_ones_are_marked() {
        let card = |rarity, finish| Item::Card {
            rarity,
            finish,
            tier: 0,
            credits: 0,
        };
        let shader = |id| Item::Shader(unlockables::find(id).unwrap());
        assert!(card(3, 0).grand() && card(4, 0).grand() && card(0, 2).grand());
        assert!(!card(2, 1).grand());
        assert!(shader("saber_sun").grand());
        assert!(!shader("saber_tidal").grand());
        assert!(card(1, 1).grade() > card(1, 0).grade());
        assert!(card(2, 0).grade() > card(1, 2).grade());
        assert!(shader("saber_sun").grade() > card(3, 2).grade());
        assert_eq!(card(0, 2).colour(), crate::menu::sjk::color::GOLD_BRIGHT);
    }
}
