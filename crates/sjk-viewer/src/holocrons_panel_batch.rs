//! Opening many holocrons at once (`docs/cards.md`, "The ceremony"): more than
//! [`super::opening::ONE_BY_ONE`] would take minutes one by one, so the first cracks open
//! ([`super::unseal`]), then their strips run five at a time, stacked, each stopping a
//! beat after the one above, the next five following on their own; then everything found
//! is laid out, the finest first, with the credits it brought. Enter skips to the stops,
//! then to the next five, then ends it. All of it is worked out from one clock
//! ([`moment`]), so a world shot draws any moment.

use super::opening::{
    BAND, BAND_GAP, Prize, UNSEAL, credits_line, marker, prize_light, shards_line, smooth,
    strip_ends,
};
use super::*;
use crate::audio::ui_cues::{self, Cue};
use crate::cards;
use crate::menu::sjk::{Frame, color, kit, text};
use crate::menu_widgets::TextFamily;
use sjk_ui::{DrawCommand, FontWeight, TextAlign};

/// Strips at a time.
pub(super) const ROWS: usize = 5;
/// The top strip's run (seconds); each below stops [`STAGGER`] later.
const RUN: f32 = 3.6;
const STAGGER: f32 = 0.35;
/// When the last strip of a round stops, and the round's length with its rest.
const STOP: f32 = RUN + STAGGER * (ROWS - 1) as f32;
const ROUND: f32 = STOP + 0.9;
/// The strips' band (frame pixels): left and right, the top one's top, the gap between.
const LEFT: f32 = 230.0;
const RIGHT: f32 = 1_720.0;
const MIDDLE_X: f32 = 975.0;
const TOP: f32 = 160.0;
const ROW_GAP: f32 = 22.0;
/// The finds laid out at the end: at most this many a row, this wide, from this high.
const PER_ROW: usize = 10;
const FOUND_WIDTH: f32 = 140.0;
const FOUND_GAP: f32 = 24.0;
const FOUND_TOP: f32 = 230.0;
const FOUND_ROW: f32 = FOUND_WIDTH * cards::look::ASPECT + 92.0;
/// How long each find takes to come in, and the beat between them.
const FOUND_IN: f32 = 0.3;
const FOUND_BEAT: f32 = 0.05;

/// The strip's height for its tiles, and the step from one strip to the next.
fn strip_height() -> f32 {
    strip::STACKED.tile * cards::look::ASPECT
}

fn row_step() -> f32 {
    strip_height() + BAND_GAP + BAND + ROW_GAP
}

/// Where the batch of `count` is `elapsed` seconds in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Moment {
    /// The first holocron cracking open, seconds in.
    Unseal(f32),
    /// Strips of round `round` (from 0), seconds into it.
    Round { round: usize, within: f32 },
    /// Everything found laid out, seconds since.
    Found(f32),
}

/// The rounds of strips `count` holocrons take.
fn rounds(count: usize) -> usize {
    count.div_ceil(ROWS)
}

pub(super) fn moment(elapsed: f32, count: usize) -> Moment {
    if elapsed < UNSEAL {
        return Moment::Unseal(elapsed.max(0.0));
    }
    let after = elapsed - UNSEAL;
    let round = (after / ROUND).floor() as usize;
    if round < rounds(count) {
        Moment::Round {
            round,
            within: after - round as f32 * ROUND,
        }
    } else {
        Moment::Found(after - rounds(count) as f32 * ROUND)
    }
}

/// Whether the finds are laid out (Enter is Done).
pub(super) fn summing_up(elapsed: f32, count: usize) -> bool {
    matches!(moment(elapsed, count), Moment::Found(_))
}

/// Where Enter takes the batch of `count` from `elapsed`: out of the unseal to the first
/// round's stops, a running round to its stops, a round at rest to the next (or to the
/// finds); `None` ends it.
pub(super) fn skip(elapsed: f32, count: usize) -> Option<f32> {
    match moment(elapsed, count) {
        Moment::Unseal(_) => Some(UNSEAL + STOP),
        Moment::Round { round, within } if within < STOP => {
            Some(UNSEAL + round as f32 * ROUND + STOP)
        }
        Moment::Round { round, .. } => Some(UNSEAL + (round + 1) as f32 * ROUND),
        // Every find in place.
        Moment::Found(since) if since < found_settled(count) => {
            Some(UNSEAL + rounds(count) as f32 * ROUND + found_settled(count))
        }
        Moment::Found(_) => None,
    }
}

/// Seconds until the last of `count` finds is in.
fn found_settled(count: usize) -> f32 {
    FOUND_IN + FOUND_BEAT * count.saturating_sub(1) as f32
}

/// The order the finds are laid out in: the finest first, else as opened.
pub(super) fn finest_first(shown: &[Prize]) -> [usize; sjk_identity::OPEN_MAX] {
    let mut order = [0; sjk_identity::OPEN_MAX];
    let count = shown.len().min(order.len());
    for (at, slot) in order.iter_mut().enumerate().take(count) {
        *slot = at;
    }
    order[..count].sort_by_key(|&at| std::cmp::Reverse(shown[at].item().grade()));
    order
}

fn bonus_batch(shown: &[Prize]) -> bool {
    shown
        .iter()
        .all(|prize| matches!(prize.item(), strip::Item::Banner(_)))
}

fn batch_label(shown: &[Prize], opened: bool) -> String {
    if bonus_batch(shown) {
        format!("{} bonus banners", shown.len())
    } else {
        crate::terms::say_owned(format!(
            "{} holocrons{}",
            shown.len(),
            if opened { " opened" } else { "" }
        ))
    }
}

impl Panel {
    /// The batch `elapsed` seconds in.
    pub(super) fn draw_batch(
        &mut self,
        frame: &Frame,
        viewport: [f32; 2],
        shown: &[Prize],
        elapsed: f32,
        seconds: f32,
    ) {
        match moment(elapsed, shown.len()) {
            Moment::Unseal(within) => self.draw_unseal(
                frame,
                viewport,
                shown[0].tier(),
                within,
                format_args!("{}", batch_label(shown, false)),
                None,
            ),
            Moment::Round { round, within } => {
                if self.played.round != round {
                    self.played.round = round;
                    self.played.tick = [0; ROWS];
                    self.played.landed = 0;
                }
                self.draw_round(frame, viewport, shown, round, within, seconds);
            }
            Moment::Found(since) => self.draw_found(frame, shown, since, seconds),
        }
    }

    /// Round `round`'s strips, `within` seconds in.
    fn draw_round(
        &mut self,
        frame: &Frame,
        viewport: [f32; 2],
        shown: &[Prize],
        round: usize,
        within: f32,
        seconds: f32,
    ) {
        let s = frame.s;
        let first = round * ROWS;
        let these = &shown[first..(first + ROWS).min(shown.len())];
        text(
            &mut self.ui,
            TextFamily::Display,
            format_args!(
                "{}",
                crate::terms::say(&format!(
                    "{} {} to {} of {}",
                    if bonus_batch(shown) {
                        "Bonus banners"
                    } else {
                        "Holocrons"
                    },
                    first + 1,
                    first + these.len(),
                    shown.len()
                ))
            ),
            frame.rect(MIDDLE_X - 500.0, 92.0, 1_000.0, 44.0),
            32.0 * s,
            color::TEXT,
            FontWeight::Semibold,
            TextAlign::Center,
        );
        let geometry = strip::STACKED;
        let height = strip_height();
        for (row, prize) in these.iter().enumerate() {
            let top = TOP + row as f32 * row_step();
            let tier = holocrons::slot(prize.tier());
            let item = prize.item();
            let running = RUN + STAGGER * row as f32;
            let t = within / running;
            let run = strip::run(prize.holocron(), geometry);
            let distance = run * strip::travelled(t);
            let speed = run * strip::speed(t, running);
            let under = strip::under_marker(distance, geometry);
            if under != self.played.tick[row] {
                self.played.tick[row] = under;
                ui_cues::post(Cue::CrateTick);
            }
            let settled = within >= running;
            if settled && self.played.landed & (1 << row) == 0 {
                self.played.landed |= 1 << row;
                ui_cues::post(if item.grand() {
                    Cue::Medal
                } else {
                    Cue::CrateLand
                });
            }
            let lit = if settled {
                smooth((within - running) / 0.3)
            } else {
                0.0
            };
            // The holocron's tier at the strip's left.
            text(
                &mut self.ui,
                TextFamily::Body,
                format_args!("{}", tier.label),
                frame.rect(40.0, top + height * 0.5 - 14.0, LEFT - 60.0, 28.0),
                19.0 * s,
                tier.colour,
                FontWeight::Semibold,
                TextAlign::End,
            );
            if lit > 0.0 {
                prize_light(
                    &mut self.ui,
                    frame,
                    [MIDDLE_X, top + height * 0.5],
                    item,
                    lit,
                    seconds,
                    0.45,
                );
            }
            let tiles = strip::strip(prize.tier(), prize.holocron(), item);
            let _ = self
                .ui
                .draw_list_mut()
                .push(DrawCommand::PushClip(frame.rect(
                    LEFT,
                    top - 20.0,
                    RIGHT - LEFT,
                    height + 40.0,
                )));
            for (at, tile) in tiles.iter().enumerate() {
                let x = MIDDLE_X - geometry.tile * 0.5 + at as f32 * geometry.pitch() - distance;
                if x + geometry.tile * 2.0 < LEFT || x > RIGHT {
                    continue;
                }
                let won = at == strip::WIN;
                let lift = if won { 8.0 * lit } else { 0.0 };
                let dim = if won { 1.0 } else { 1.0 - 0.65 * lit };
                self.strip_tile(
                    frame,
                    *tile,
                    [x, top - lift, geometry.tile],
                    if won { lit } else { 0.0 },
                    dim,
                    speed,
                    seconds,
                );
            }
            let _ = self.ui.draw_list_mut().push(DrawCommand::PopClip);
            strip_ends(
                &mut self.ui,
                frame,
                [LEFT, RIGHT],
                top - 20.0,
                height + 40.0,
            );
            marker(
                &mut self.ui,
                frame,
                MIDDLE_X,
                // Its points stay clear of the strips above and below.
                [top - 4.0, top + height + BAND_GAP + BAND + 4.0],
                color::alpha(tier.colour, 0.95),
                0.4,
            );
        }
        // The first holocron's flash fades over the first round.
        if round == 0 {
            super::unseal::flash(
                &mut self.ui,
                viewport,
                holocrons::slot(shown[0].tier()).colour,
                1.0 - within / super::unseal::FLASH_OUT,
            );
        }
    }

    /// Everything found, `since` seconds after the last round: the finest first, each
    /// coming in on its beat, a grand one lit; the credits they brought; Done.
    fn draw_found(&mut self, frame: &Frame, shown: &[Prize], since: f32, seconds: f32) {
        let s = frame.s;
        if !self.played.entered {
            self.played.entered = true;
            ui_cues::post(if shown.iter().any(|prize| prize.item().grand()) {
                Cue::Medal
            } else {
                Cue::Achievement
            });
        }
        text(
            &mut self.ui,
            TextFamily::Display,
            format_args!("{}", batch_label(shown, true)),
            frame.rect(MIDDLE_X - 500.0, 80.0, 1_000.0, 50.0),
            40.0 * s,
            color::TEXT,
            FontWeight::Semibold,
            TextAlign::Center,
        );
        let credits: u64 = shown
            .iter()
            .map(|prize| match prize {
                Prize::Card(card) => card.credits,
                Prize::Shader(_) | Prize::Shards(_) => 0,
            })
            .sum();
        let shards: u64 = shown
            .iter()
            .map(|prize| match prize {
                Prize::Shards(pile) => pile.shards,
                Prize::Card(card) => card.shards,
                Prize::Shader(_) => 0,
            })
            .sum();
        // Side by side when both came.
        let apart = if credits > 0 && shards > 0 {
            170.0
        } else {
            0.0
        };
        if credits > 0 {
            credits_line(
                &mut self.ui,
                frame,
                MIDDLE_X - apart,
                142.0,
                &format!("+{}", cards::credits_text(credits)),
                32.0,
                (since / 0.4).min(1.0),
            );
        }
        if shards > 0 {
            shards_line(
                &mut self.ui,
                frame,
                MIDDLE_X + apart,
                142.0,
                &format!("+{}", crate::economy::shards_text(shards)),
                32.0,
                (since / 0.4).min(1.0),
            );
        }
        let faces = cards::faces::current();
        let order = finest_first(shown);
        let count = shown.len().min(order.len());
        for (slot, &at) in order[..count].iter().enumerate() {
            let prize = &shown[at];
            let item = prize.item();
            let row = slot / PER_ROW;
            let in_row = (count - row * PER_ROW).min(PER_ROW);
            let row_width = in_row as f32 * (FOUND_WIDTH + FOUND_GAP) - FOUND_GAP;
            let x =
                MIDDLE_X - row_width * 0.5 + (slot % PER_ROW) as f32 * (FOUND_WIDTH + FOUND_GAP);
            let y = FOUND_TOP + row as f32 * FOUND_ROW;
            let come = smooth((since - slot as f32 * FOUND_BEAT) / FOUND_IN);
            if come <= 0.0 {
                continue;
            }
            let height = FOUND_WIDTH * cards::look::ASPECT;
            if item.grand() {
                prize_light(
                    &mut self.ui,
                    frame,
                    [x + FOUND_WIDTH * 0.5, y + height * 0.5],
                    item,
                    come,
                    seconds,
                    0.5,
                );
            }
            let _ = self.ui.draw_list_mut().push(DrawCommand::PushOpacity(come));
            let rise = 24.0 * (1.0 - come);
            match prize {
                Prize::Card(card) => cards::look::draw(
                    &mut self.ui,
                    frame,
                    [x, y + rise, FOUND_WIDTH],
                    cards::look::Card {
                        rarity: card.rarity,
                        finish: card.finish,
                        edition: "",
                        face: cards::shown(&faces, card.card.as_deref()),
                        tilt: None,
                        dim: false,
                        label: false,
                    },
                    seconds,
                    1.0,
                    1.0,
                ),
                Prize::Shader(_) | Prize::Shards(_) => {
                    self.tile(frame, item, [x, y + rise, FOUND_WIDTH], 1.0, seconds);
                }
            }
            // A card found in the packs takes its rank's or rarity's colour.
            let colour = match prize {
                Prize::Card(card) => {
                    cards::tone_in(&faces, card.rarity, card.card.as_deref()).colour
                }
                Prize::Shader(_) | Prize::Shards(_) => item.colour(),
            };
            let _ = self.ui.draw_list_mut().push(DrawCommand::RoundedRect {
                rect: frame.rect(x, y + rise + height + BAND_GAP, FOUND_WIDTH, BAND),
                radius: BAND * 0.5 * s,
                color: color::alpha(colour, 0.9),
            });
            let words_y = y + rise + height + BAND_GAP + BAND + 8.0;
            match prize {
                Prize::Card(card) => {
                    match cards::shown(&faces, card.card.as_deref()) {
                        Some(face) => found_line(
                            self,
                            frame,
                            format_args!("{}", face.face.name),
                            [x, words_y],
                            colour,
                        ),
                        None if card.finish.index > 0 => found_line(
                            self,
                            frame,
                            format_args!("{} card, {}", card.rarity.name, card.finish.name),
                            [x, words_y],
                            item.colour(),
                        ),
                        None => found_line(
                            self,
                            frame,
                            format_args!("{} card", card.rarity.name),
                            [x, words_y],
                            item.colour(),
                        ),
                    }
                    found_note(
                        self,
                        frame,
                        format_args!(
                            "+{} credits{}",
                            card.credits,
                            if card.shards > 0 {
                                format!(
                                    ", +{} shard{}",
                                    crate::economy::shards_text(card.shards),
                                    if card.shards == 1 { "" } else { "s" }
                                )
                            } else {
                                String::new()
                            }
                        ),
                        [x, words_y + 22.0],
                    );
                    // A Solar blade card's blade, under it in its colour (20/10/2026).
                    if let Some(found) = &card.bonus {
                        found_line(
                            self,
                            frame,
                            format_args!("+{}", found.name),
                            [x, words_y + 42.0],
                            found.colour(),
                        );
                    }
                }
                Prize::Shards(pile) => {
                    found_line(
                        self,
                        frame,
                        format_args!("{} kyber shards", crate::economy::shards_text(pile.shards)),
                        [x, words_y],
                        item.colour(),
                    );
                    found_note(
                        self,
                        frame,
                        format_args!("For crafting"),
                        [x, words_y + 22.0],
                    );
                }
                Prize::Shader(found) => {
                    found_line(
                        self,
                        frame,
                        format_args!("{}", found.name),
                        [x, words_y],
                        item.colour(),
                    );
                    if found.count > 1 {
                        found_note(
                            self,
                            frame,
                            format_args!("Shader, x{}", found.count),
                            [x, words_y + 22.0],
                        );
                    } else {
                        found_note(
                            self,
                            frame,
                            format_args!("Shader, new"),
                            [x, words_y + 22.0],
                        );
                    }
                }
            }
            let _ = self.ui.draw_list_mut().push(DrawCommand::PopOpacity);
        }
        let done = smooth((since - found_settled(count)) / 0.3);
        text(
            &mut self.ui,
            TextFamily::Body,
            format_args!(
                "{}",
                if bonus_batch(shown) {
                    "Bonus covers are on the Collection's Banners tab."
                } else {
                    "Cards are on the Collection's Cards tab, shaders on its Shaders tab; kyber shards craft shaders."
                }
            ),
            frame.rect(MIDDLE_X - 600.0, 836.0, 1_200.0, 26.0),
            17.0 * s,
            color::alpha(color::QUIET, done),
            FontWeight::Regular,
            TextAlign::Center,
        );
        let done_label = self.done_label();
        kit::button(
            &mut self.ui,
            frame,
            [MIDDLE_X - 110.0, 880.0, 220.0, 48.0],
            done_label,
            true,
            true,
            true,
            super::opening::NEXT_TOKEN,
        );
    }
}

/// A find's name under it at `[x, y]` (frame pixels), cut to its width.
fn found_line(
    panel: &mut Panel,
    frame: &Frame,
    words: std::fmt::Arguments<'_>,
    [x, y]: [f32; 2],
    colour: sjk_ui::Color,
) {
    text(
        &mut panel.ui,
        TextFamily::Body,
        words,
        frame.rect(x - 8.0, y, FOUND_WIDTH + 16.0, 22.0),
        15.0 * frame.s,
        colour,
        FontWeight::Semibold,
        TextAlign::Center,
    );
}

/// The quiet line under a find's name.
fn found_note(panel: &mut Panel, frame: &Frame, words: std::fmt::Arguments<'_>, [x, y]: [f32; 2]) {
    text(
        &mut panel.ui,
        TextFamily::Body,
        words,
        frame.rect(x - 8.0, y, FOUND_WIDTH + 16.0, 20.0),
        14.0 * frame.s,
        color::MUTED,
        FontWeight::Regular,
        TextAlign::Center,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_batch_cracks_one_runs_five_at_a_time_then_lays_them_out() {
        assert_eq!(moment(0.5, 12), Moment::Unseal(0.5));
        assert_eq!(
            moment(UNSEAL + 1.0, 12),
            Moment::Round {
                round: 0,
                within: 1.0
            }
        );
        assert!(matches!(
            moment(UNSEAL + ROUND * 2.0 + 0.5, 12),
            Moment::Round { round: 2, .. }
        ));
        assert!(matches!(
            moment(UNSEAL + ROUND * 3.0 + 0.1, 12),
            Moment::Found(_)
        ));
        assert!(summing_up(UNSEAL + ROUND * 3.0 + 0.1, 12));
        // Twenty: four rounds.
        assert!(matches!(
            moment(UNSEAL + ROUND * 3.5, 20),
            Moment::Round { round: 3, .. }
        ));
    }

    #[test]
    fn enter_skips_to_the_stops_then_on_then_ends_it() {
        let count = 7;
        let to = skip(0.3, count).unwrap();
        assert!(
            matches!(moment(to, count), Moment::Round { round: 0, within } if (within - STOP).abs() < 1e-4)
        );
        let to = skip(to, count).unwrap();
        assert!(
            matches!(moment(to, count), Moment::Round { round: 1, within } if within.abs() < 1e-4)
        );
        let to = skip(to, count).unwrap();
        let to = skip(to, count).unwrap();
        assert!(matches!(moment(to, count), Moment::Found(since) if since.abs() < 1e-4));
        let to = skip(to, count).unwrap();
        assert!(
            matches!(moment(to, count), Moment::Found(since) if since >= found_settled(count) - 1e-4)
        );
        assert_eq!(skip(to, count), None, "Done");
    }

    #[test]
    fn every_strip_stops_inside_its_round_and_the_rows_fit_the_frame() {
        let bottom = TOP + (ROWS - 1) as f32 * row_step() + strip_height() + BAND_GAP + BAND;
        assert!(bottom < super::view::KEYS_Y - 20.0, "{bottom}");
        let found_bottom = FOUND_TOP + FOUND_ROW * 2.0;
        assert!(found_bottom < 836.0, "{found_bottom}");
        let widest = PER_ROW as f32 * (FOUND_WIDTH + FOUND_GAP) - FOUND_GAP;
        assert!(widest < 1_840.0);
    }
}
