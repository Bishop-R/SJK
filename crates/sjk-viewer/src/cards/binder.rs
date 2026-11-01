//! The Cards tab as a binder (`docs/cards.md`, "The Cards tab"; Sol's request of
//! 19/10/2026): every card the packs have a face of, found or not, one tile per card
//! whatever the copies held, so the collection reads as a binder with its gaps. Sealed
//! copies (no card yet, only a rarity) stand first, one tile per rarity, and copies of a
//! card whose face is not in the packs last. A tile knows the copies behind it, so the
//! tab draws the best finish held, counts them and breaks them down by finish and
//! edition; [`Filter`] picks the tiles shown and [`progress`] says how far each set is.
//!
//! Nothing here knows a set: the sets, their cards, rarities and kinds all come from the
//! faces the packs bring ([`super::faces`]).

use super::faces::{Face, Faces, Kind};
use super::{FINISHES, Finish, RARITIES, Rarity, Stack};

/// What a tile stands for.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Slot {
    /// The sealed copies of the tile's rarity.
    Sealed,
    /// The card of face `index` in [`Faces::list`].
    Card(usize),
    /// Copies of card `id`, whose face is not in the packs.
    Unknown(String),
}

/// One tile of the binder: a card (found or not), the sealed copies of a rarity, or the
/// copies of a card the packs do not have.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Tile {
    pub(crate) slot: Slot,
    pub(crate) rarity: &'static Rarity,
    /// The copies held, the best finish first; empty for a card not found yet.
    pub(crate) copies: Vec<Stack>,
}

impl Tile {
    /// Copies held in all.
    pub(crate) fn count(&self) -> u32 {
        self.copies.iter().map(|stack| stack.count).sum()
    }

    /// Whether the player holds a copy.
    pub(crate) fn owned(&self) -> bool {
        !self.copies.is_empty()
    }

    /// The best copy held (its finish is the tile's light, its edition the stamp).
    pub(crate) fn best(&self) -> Option<&Stack> {
        self.copies.first()
    }

    /// The finish the tile shows: the best held, Normal when none is.
    pub(crate) fn finish(&self) -> &'static Finish {
        self.best().map_or(&FINISHES[0], |stack| stack.finish)
    }

    /// The copy shown in the finish of index `finish` when one is held in it, else the
    /// best (01/11/2026, Sol's request: every finish held can be looked at).
    pub(crate) fn in_finish(&self, finish: Option<usize>) -> Option<&Stack> {
        finish
            .and_then(|index| self.copies.iter().find(|stack| stack.finish.index == index))
            .or_else(|| self.best())
    }

    /// The finishes held, by their index (Normal first).
    pub(crate) fn finishes_held(&self) -> Vec<usize> {
        let mut held: Vec<usize> = self.copies.iter().map(|stack| stack.finish.index).collect();
        held.sort_unstable();
        held.dedup();
        held
    }

    /// Whether a copy in `finish` is held.
    pub(crate) fn holds(&self, finish: &Finish) -> bool {
        self.copies.iter().any(|stack| stack.finish.id == finish.id)
    }

    /// When the first copy was found (unix seconds; 0 for none or not known).
    pub(crate) fn first(&self) -> i64 {
        self.copies
            .iter()
            .map(|stack| stack.first)
            .filter(|first| *first > 0)
            .min()
            .unwrap_or(0)
    }

    /// Its face, when it is a card the packs have.
    pub(crate) fn face<'a>(&self, faces: &'a Faces) -> Option<&'a Face> {
        match self.slot {
            Slot::Card(index) => faces.list.get(index),
            _ => None,
        }
    }

    /// How it reads and is coloured: its face's rank or rarity, else its rarity's
    /// ([`super::Tone`]).
    pub(crate) fn tone<'a>(&self, faces: &'a Faces) -> super::Tone<'a> {
        super::Tone::of(self.rarity, self.face(faces))
    }

    /// What players read it as: the card's name, "Sealed Legendary cards" or "Unknown
    /// card <id>".
    pub(crate) fn name(&self, faces: &Faces) -> String {
        match &self.slot {
            Slot::Sealed if self.count() == 1 => format!("Sealed {} card", self.rarity.name),
            Slot::Sealed => format!("Sealed {} cards", self.rarity.name),
            Slot::Card(_) => self
                .face(faces)
                .map_or_else(String::new, |face| face.name.clone()),
            Slot::Unknown(id) => format!("Unknown card {id}"),
        }
    }

    /// The copies held, by finish (the best first) and by edition when they are not all
    /// of one: "3 copies: 1 Solar, 2 Normal · Alpha Edition"; "1 copy, Holo · Alpha
    /// Edition"; empty for none.
    pub(crate) fn breakdown(&self) -> String {
        let count = self.count();
        if count == 0 {
            return String::new();
        }
        let mut finishes = Vec::new();
        for finish in FINISHES.iter().rev() {
            let held: u32 = self
                .copies
                .iter()
                .filter(|stack| stack.finish.id == finish.id)
                .map(|stack| stack.count)
                .sum();
            if held > 0 {
                finishes.push((held, finish.name));
            }
        }
        let mut editions: Vec<(u32, &str)> = Vec::new();
        for stack in &self.copies {
            match editions.iter_mut().find(|(_, name)| *name == stack.edition) {
                Some((held, _)) => *held += stack.count,
                None => editions.push((stack.count, &stack.edition)),
            }
        }
        editions.retain(|(_, name)| !name.is_empty());
        let mut words = if count == 1 {
            format!("1 copy, {}", finishes[0].1)
        } else {
            let list: Vec<String> = finishes
                .iter()
                .map(|(held, name)| format!("{held} {name}"))
                .collect();
            format!("{count} copies: {}", list.join(", "))
        };
        match editions.as_slice() {
            [] => {}
            [(_, name)] => {
                words.push_str(" · ");
                words.push_str(name);
            }
            several => {
                let list: Vec<String> = several
                    .iter()
                    .map(|(held, name)| format!("{held} {name}"))
                    .collect();
                words.push_str(" · ");
                words.push_str(&list.join(", "));
            }
        }
        words
    }
}

/// The binder's tiles for `stacks` (a profile's, [`super::stacks`]) and the faces the packs
/// have: the sealed copies first, a tile per rarity, the rarest first; then every card
/// of the packs, by set and number, with the copies held of it; then the copies of cards
/// the packs do not have, by id.
pub(crate) fn tiles(stacks: &[Stack], faces: &Faces) -> Vec<Tile> {
    let mut tiles = Vec::with_capacity(faces.list.len() + RARITIES.len());
    for rarity in RARITIES.iter().rev() {
        let copies: Vec<Stack> = stacks
            .iter()
            .filter(|stack| stack.sealed() && stack.rarity.id == rarity.id)
            .cloned()
            .collect();
        if !copies.is_empty() {
            tiles.push(Tile {
                slot: Slot::Sealed,
                rarity,
                copies: best_first(copies),
            });
        }
    }
    for (index, face) in faces.list.iter().enumerate() {
        let copies: Vec<Stack> = stacks
            .iter()
            .filter(|stack| stack.card.as_deref() == Some(face.id.as_str()))
            .cloned()
            .collect();
        tiles.push(Tile {
            slot: Slot::Card(index),
            rarity: face.rarity,
            copies: best_first(copies),
        });
    }
    let mut unknown: Vec<&str> = stacks
        .iter()
        .filter_map(|stack| stack.card.as_deref())
        .filter(|id| faces.find(id).is_none())
        .collect();
    unknown.sort_unstable();
    unknown.dedup();
    for id in unknown {
        let copies: Vec<Stack> = stacks
            .iter()
            .filter(|stack| stack.card.as_deref() == Some(id))
            .cloned()
            .collect();
        let copies = best_first(copies);
        tiles.push(Tile {
            slot: Slot::Unknown(id.to_owned()),
            rarity: copies[0].rarity,
            copies,
        });
    }
    tiles
}

/// `copies` with the best finish first, then the rarest, then by edition.
fn best_first(mut copies: Vec<Stack>) -> Vec<Stack> {
    copies.sort_by(|a, b| {
        (b.finish.index, b.rarity.index)
            .cmp(&(a.finish.index, a.rarity.index))
            .then_with(|| a.edition.cmp(&b.edition))
    });
    copies
}

/// Which tiles the binder shows by whether the player holds them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Show {
    #[default]
    All,
    Owned,
    Missing,
}

impl Show {
    /// The three, in the chips' order.
    pub(crate) const ALL: [Self; 3] = [Self::All, Self::Owned, Self::Missing];

    /// What its chip says.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Owned => "Owned",
            Self::Missing => "Missing",
        }
    }
}

/// What the binder shows: held or missing, one rarity, one kind, a finish held. They
/// combine; the default shows everything.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Filter {
    pub(crate) show: Show,
    pub(crate) rarity: Option<&'static Rarity>,
    pub(crate) kind: Option<Kind>,
    /// A finish a copy is held in (Holo or Solar).
    pub(crate) finish: Option<&'static Finish>,
}

impl Filter {
    /// Whether anything is filtered out.
    pub(crate) fn active(&self) -> bool {
        *self != Self::default()
    }

    /// Whether `tile` stays in the binder. Sealed copies and unknown cards have no kind,
    /// so a kind leaves them out; a ranked card shows no rarity, so a rarity leaves it out
    /// (its tier would show through).
    pub(crate) fn passes(&self, tile: &Tile, faces: &Faces) -> bool {
        let shown = match self.show {
            Show::All => true,
            Show::Owned => tile.owned(),
            Show::Missing => !tile.owned(),
        };
        shown
            && self.rarity.is_none_or(|rarity| {
                rarity.id == tile.rarity.id
                    && tile.face(faces).is_none_or(|face| face.rank.is_none())
            })
            && self
                .kind
                .is_none_or(|kind| tile.face(faces).is_some_and(|face| face.kind == kind))
            && self.finish.is_none_or(|finish| tile.holds(finish))
    }

    /// The next way to show (the previous with `back`), after Missing All again.
    pub(crate) fn cycle_show(&mut self, back: bool) {
        let at = Show::ALL
            .iter()
            .position(|show| *show == self.show)
            .unwrap_or(0);
        self.show = Show::ALL[step(at, Show::ALL.len(), back)];
    }

    /// The next rarity (the previous with `back`), any after the last.
    pub(crate) fn cycle_rarity(&mut self, back: bool) {
        self.rarity = cycle(self.rarity, &RARITIES.iter().collect::<Vec<_>>(), back);
    }

    /// The next kind of those `faces` have (the previous with `back`), any after the last.
    pub(crate) fn cycle_kind(&mut self, faces: &Faces, back: bool) {
        self.kind = cycle(self.kind, &kinds(faces), back);
    }

    /// The next finish held, past Normal (the previous with `back`), any after the last.
    pub(crate) fn cycle_finish(&mut self, back: bool) {
        self.finish = cycle(
            self.finish,
            &FINISHES.iter().skip(1).collect::<Vec<_>>(),
            back,
        );
    }
}

/// `at` one step on among `count` (one back with `back`), wrapping round.
fn step(at: usize, count: usize, back: bool) -> usize {
    if back {
        (at + count - 1) % count
    } else {
        (at + 1) % count
    }
}

/// The value after `now` in `none, values...` (the one before with `back`), wrapping.
fn cycle<T: Copy + PartialEq>(now: Option<T>, values: &[T], back: bool) -> Option<T> {
    let at = now
        .and_then(|now| values.iter().position(|value| *value == now))
        .map_or(0, |index| index + 1);
    step(at, values.len() + 1, back)
        .checked_sub(1)
        .map(|index| values[index])
}

/// The kinds `faces` have, in [`Kind::ALL`] order: what the kind filter walks.
pub(crate) fn kinds(faces: &Faces) -> Vec<Kind> {
    Kind::ALL
        .into_iter()
        .filter(|kind| faces.list.iter().any(|face| face.kind == *kind))
        .collect()
}

/// How far the player is in one set: cards found of those the packs have.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Progress {
    pub(crate) name: String,
    pub(crate) found: usize,
    pub(crate) of: usize,
}

/// Each set's progress, in the faces' order (by set id).
pub(crate) fn progress(tiles: &[Tile], faces: &Faces) -> Vec<Progress> {
    let mut sets: Vec<(&str, Progress)> = Vec::new();
    for tile in tiles {
        let Some(face) = tile.face(faces) else {
            continue;
        };
        let at = match sets.iter().position(|(set, _)| *set == face.set) {
            Some(at) => at,
            None => {
                sets.push((
                    &face.set,
                    Progress {
                        name: face.set_name.clone(),
                        found: 0,
                        of: 0,
                    },
                ));
                sets.len() - 1
            }
        };
        sets[at].1.of += 1;
        if tile.owned() {
            sets[at].1.found += 1;
        }
    }
    sets.into_iter().map(|(_, progress)| progress).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::stacks;
    use sjk_identity::CardStack;

    fn face(id: &str, set: &str, number: u16, rarity: usize, kind: Kind) -> Face {
        Face {
            id: id.to_owned(),
            name: format!("Card {id}"),
            set: set.to_owned(),
            set_name: set.to_uppercase(),
            number,
            kind,
            rarity: &RARITIES[rarity],
            lore: String::new(),
            artist: "Sol".to_owned(),
            art: String::new(),
            grants: None,
            blade: None,
            scene: None,
            rank: None,
        }
    }

    fn copy(
        card: Option<&str>,
        rarity: &str,
        finish: &str,
        edition: &str,
        count: u32,
    ) -> CardStack {
        CardStack {
            card: card.map(str::to_owned),
            rarity: rarity.to_owned(),
            finish: finish.to_owned(),
            edition: edition.to_owned(),
            count,
            first: 1_791_641_100 + i64::from(count),
            last: 1_791_641_100,
        }
    }

    fn faces() -> Faces {
        Faces::of_list(vec![
            face("a_000", "a", 0, 4, Kind::Credits),
            face("a_001", "a", 1, 3, Kind::Credits),
            face("a_002", "a", 2, 3, Kind::Meme),
            face("b_000", "b", 0, 2, Kind::Place),
        ])
    }

    fn held() -> Vec<Stack> {
        stacks(&[
            copy(Some("a_001"), "legendary", "normal", "alpha", 2),
            copy(Some("a_001"), "legendary", "solar", "alpha", 1),
            copy(Some("b_000"), "rare", "holo", "alpha", 1),
            copy(Some("b_000"), "rare", "holo", "beta", 1),
            copy(Some("z_404"), "rare", "normal", "alpha", 1),
            copy(None, "rare", "normal", "alpha", 3),
            copy(None, "mythical", "holo", "alpha", 1),
        ])
    }

    /// Sealed copies first by rarity, then every card by set and number, found or not,
    /// then the cards the packs lack; one tile per card whatever its copies.
    #[test]
    fn every_card_has_one_tile_in_binder_order() {
        let faces = faces();
        let tiles = tiles(&held(), &faces);
        let slots: Vec<(&Slot, &str, u32)> = tiles
            .iter()
            .map(|tile| (&tile.slot, tile.rarity.id, tile.count()))
            .collect();
        assert_eq!(
            slots,
            [
                (&Slot::Sealed, "mythical", 1),
                (&Slot::Sealed, "rare", 3),
                (&Slot::Card(0), "mythical", 0),
                (&Slot::Card(1), "legendary", 3),
                (&Slot::Card(2), "legendary", 0),
                (&Slot::Card(3), "rare", 2),
                (&Slot::Unknown("z_404".to_owned()), "rare", 1),
            ]
        );
        assert_eq!(tiles[3].finish().id, "solar", "the best finish held shows");
        assert_eq!(tiles[2].finish().id, "normal");
        assert_eq!(
            tiles[3].breakdown(),
            "3 copies: 1 Solar, 2 Normal · Alpha Edition"
        );
        assert_eq!(
            tiles[5].breakdown(),
            "2 copies: 2 Holo · 1 Alpha Edition, 1 Beta Edition"
        );
        assert_eq!(tiles[0].breakdown(), "1 copy, Holo · Alpha Edition");
        assert_eq!(tiles[2].breakdown(), "");
        assert_eq!(tiles[3].first(), 1_791_641_101);
        assert_eq!(tiles[1].name(&faces), "Sealed Rare cards");
        assert_eq!(tiles[0].name(&faces), "Sealed Mythical card");
        assert_eq!(tiles[6].name(&faces), "Unknown card z_404");
        assert_eq!(tiles[4].name(&faces), "Card a_002");
        assert_eq!(
            progress(&tiles, &faces),
            [
                Progress {
                    name: "A".to_owned(),
                    found: 1,
                    of: 3
                },
                Progress {
                    name: "B".to_owned(),
                    found: 1,
                    of: 1
                },
            ]
        );
    }

    /// The filters combine; a kind leaves out sealed and unknown tiles; the cycles walk
    /// round through any.
    #[test]
    fn filters_combine_and_cycle() {
        let faces = faces();
        let tiles = tiles(&held(), &faces);
        let shown = |filter: &Filter| -> Vec<usize> {
            tiles
                .iter()
                .enumerate()
                .filter(|(_, tile)| filter.passes(tile, &faces))
                .map(|(index, _)| index)
                .collect()
        };
        let mut filter = Filter::default();
        assert!(!filter.active());
        assert_eq!(shown(&filter).len(), tiles.len());
        filter.cycle_show(false);
        assert_eq!(filter.show, Show::Owned);
        assert_eq!(shown(&filter), [0, 1, 3, 5, 6]);
        filter.cycle_show(false);
        assert_eq!(shown(&filter), [2, 4]);
        filter.cycle_show(false);
        assert_eq!(filter.show, Show::All);
        filter.cycle_rarity(false);
        filter.cycle_rarity(false);
        filter.cycle_rarity(false);
        assert_eq!(filter.rarity.map(|rarity| rarity.id), Some("rare"));
        assert_eq!(shown(&filter), [1, 5, 6]);
        filter.cycle_finish(false);
        assert_eq!(filter.finish.map(|finish| finish.id), Some("holo"));
        assert_eq!(shown(&filter), [5]);
        filter.cycle_finish(true);
        filter.cycle_rarity(true);
        filter.cycle_rarity(true);
        filter.cycle_rarity(true);
        assert_eq!(filter, Filter::default());
        assert_eq!(kinds(&faces), [Kind::Place, Kind::Credits, Kind::Meme]);
        filter.cycle_kind(&faces, true);
        assert_eq!(filter.kind, Some(Kind::Meme));
        assert_eq!(shown(&filter), [4]);
        filter.cycle_kind(&faces, false);
        assert_eq!(filter.kind, None);
        filter.cycle_kind(&faces, false);
        assert_eq!(filter.kind, Some(Kind::Place));
        assert!(filter.active());
    }

    /// A ranked card shows its rank's name and colour, and a rarity filter leaves it out
    /// so its rarity (its odds) never shows through.
    #[test]
    fn a_ranked_card_shows_its_rank_and_no_rarity_filter_finds_it() {
        let rank = crate::cards::faces::Rank {
            name: "Leader".to_owned(),
            colour: sjk_ui::Color::new(0.1, 0.8, 0.9, 1.0),
        };
        let mut leader = face("r_001", "r", 1, 4, Kind::Community);
        leader.rank = Some(rank.clone());
        let faces = Faces::of_list(vec![face("a_000", "a", 0, 4, Kind::Credits), leader]);
        let tiles = tiles(&[], &faces);
        assert_eq!(tiles.len(), 2);
        let tone = tiles[1].tone(&faces);
        assert_eq!(
            (tone.name, tone.colour, tone.mark),
            ("Leader", rank.colour, None)
        );
        assert_eq!(tiles[0].tone(&faces).name, "Mythical");
        let filter = Filter {
            rarity: Some(&RARITIES[4]),
            ..Filter::default()
        };
        assert!(filter.passes(&tiles[0], &faces));
        assert!(
            !filter.passes(&tiles[1], &faces),
            "its tier would show through"
        );
        assert!(Filter::default().passes(&tiles[1], &faces));
    }
}
