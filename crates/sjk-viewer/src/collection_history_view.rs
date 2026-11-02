//! The History tab's look, in the SJK UI's: its headline, the filter's chips with what the
//! read is doing beside them, then the list, a row to each line (its kind's chip in its
//! tier's colour, the sentence, the credits and shards it moved, gains green and spends
//! red, and when), Load older at its end while the hub has more, and the words of an empty
//! list, a read under way or one that failed.

use super::history::{FILTER_BASE, Filter, MORE_TOKEN, ROW_BASE, ROWS_SHOWN, Row};
use super::view::{LEFT_X, RIGHT_X};
use super::*;
use crate::menu::sjk::{Frame, color, kit, text};
use crate::menu_widgets::TextFamily;
use sjk_ui::{DrawCommand, FontWeight, TextAlign};

/// The filter's line (its middle), the list's top and a row's height.
const FILTER_Y: f32 = 320.0;
const LIST_Y: f32 = 352.0;
const ROW_HEIGHT: f32 = 41.0;
/// The columns: the chip's width, the date's and the amounts' (from the right).
const CHIP_WIDTH: f32 = 92.0;
const WHEN_WIDTH: f32 = 170.0;
const AMOUNT_WIDTH: f32 = 250.0;
/// How far a group's lines stand in.
const INDENT: f32 = 44.0;
/// The most characters of a sentence a row shows (about the text column at 17 pixels).
const TEXT_CHARS: usize = 118;

/// The gain's and the spend's colours.
const GAIN: sjk_ui::Color = color::TRUSTED;
const SPEND: sjk_ui::Color = color::RED_TEAM;

/// `text` cut to `chars` characters, "..." after a cut; no copy.
fn cut(text: &str, chars: usize) -> (&str, &'static str) {
    match text.char_indices().nth(chars) {
        Some((at, _)) => (&text[..at], "..."),
        None => (text, ""),
    }
}

impl Panel {
    /// The History tab.
    pub(super) fn history_tab(&mut self, frame: &Frame, inputs: &Inputs<'_>) {
        self.follow_history(inputs);
        self.lead(
            frame,
            "History",
            "What came to you, what you opened, spent and traded, newest first. Times are this PC's.",
        );
        let labels: Vec<&str> = Filter::ALL.iter().map(|filter| filter.label()).collect();
        let right = LEFT_X + kit::segments_width(&labels);
        kit::segments(
            &mut self.ui,
            frame,
            right,
            FILTER_Y,
            &labels,
            self.history.filter.index(),
            false,
            FILTER_BASE,
        );
        // A copy of the pointer, so the words stay while the page draws.
        let state = self.history.state.clone();
        let message = state.as_ref().map_or("", |state| state.message.as_str());
        let history = &self.history;
        let status = if history.loading() {
            "Reading your history..."
        } else if !history.rows.is_empty() {
            message
        } else {
            ""
        };
        if !status.is_empty() {
            text(
                &mut self.ui,
                TextFamily::Body,
                format_args!("{status}"),
                frame.rect(right + 24.0, FILTER_Y - 12.0, RIGHT_X - right - 24.0, 24.0),
                16.0 * frame.s,
                if history.loading() {
                    color::MUTED
                } else {
                    color::EMBER
                },
                FontWeight::Regular,
                TextAlign::End,
            );
        }
        if history.rows.is_empty() && history.more.is_none() {
            let (headline, line) = if history.loading() {
                ("Reading your history...", "")
            } else if !message.is_empty() {
                ("History unavailable", message)
            } else if inputs.snapshot.is_none() || !inputs.enabled {
                (
                    "History unavailable",
                    "The SJK identity is off: turn it on with cl_identity 1.",
                )
            } else if history.filter != Filter::All {
                ("Nothing yet", "Nothing of this kind in what was read.")
            } else {
                (
                    "Nothing yet",
                    "Holocrons you find, open, buy or trade will show here.",
                )
            };
            self.history_empty(frame, headline, line);
            return;
        }
        let shown = self
            .history
            .places()
            .saturating_sub(self.history.first)
            .min(ROWS_SHOWN);
        for slot in 0..shown {
            let index = self.history.first + slot;
            let y = LIST_Y + slot as f32 * ROW_HEIGHT;
            if self.history.more_row(index) {
                self.history_more(frame, y, index == self.history.chosen);
            } else {
                self.history_row(frame, slot, y, index);
            }
        }
        // Where the list stands, when it does not all show.
        let places = self.history.places();
        if places > ROWS_SHOWN {
            let track = ROWS_SHOWN as f32 * ROW_HEIGHT;
            let thumb = (track * ROWS_SHOWN as f32 / places as f32).max(24.0);
            let along = (track - thumb) * self.history.first as f32 / (places - ROWS_SHOWN) as f32;
            let _ = self.ui.draw_list_mut().push(DrawCommand::RoundedRect {
                rect: frame.rect(RIGHT_X + 10.0, LIST_Y + along, 4.0, thumb),
                radius: 2.0 * frame.s,
                color: color::alpha(color::HOLO, 0.35),
            });
        }
    }

    /// The words over an empty list.
    fn history_empty(&mut self, frame: &Frame, headline: &str, line: &str) {
        let s = frame.s;
        let width = RIGHT_X - LEFT_X;
        text(
            &mut self.ui,
            TextFamily::Display,
            format_args!("{headline}"),
            frame.rect(LEFT_X, 520.0, width, 40.0),
            30.0 * s,
            color::TEXT,
            FontWeight::Semibold,
            TextAlign::Center,
        );
        if !line.is_empty() {
            text(
                &mut self.ui,
                TextFamily::Body,
                format_args!("{line}"),
                frame.rect(LEFT_X, 566.0, width, 26.0),
                18.0 * s,
                color::MUTED,
                FontWeight::Regular,
                TextAlign::Center,
            );
        }
    }

    /// Row `index` of the list in place `slot`, its top at `y`.
    fn history_row(&mut self, frame: &Frame, slot: usize, y: f32, index: usize) {
        let s = frame.s;
        let token = ROW_BASE + slot as u16;
        let chosen = index == self.history.chosen;
        let hovered = self.ui.token_hovered(token);
        let Some(row) = self.history.rows.get(index) else {
            return;
        };
        let Row {
            chip,
            colour,
            text: sentence,
            amounts,
            when,
            group,
            inner,
        } = row;
        let x = if *inner { LEFT_X + INDENT } else { LEFT_X };
        let band = [x, y + 2.0, RIGHT_X - x, ROW_HEIGHT - 4.0];
        let _ = self.ui.draw_list_mut().push(DrawCommand::RoundedRect {
            rect: frame.rect(band[0], band[1], band[2], band[3]),
            radius: 8.0 * s,
            color: if chosen {
                color::alpha(color::GOLD, 0.12)
            } else if hovered {
                color::alpha(color::HOLO, 0.08)
            } else {
                color::alpha(color::SPACE, if *inner { 0.4 } else { 0.62 })
            },
        });
        if chosen {
            let _ = self.ui.draw_list_mut().push(DrawCommand::Border {
                rect: frame.rect(band[0], band[1], band[2], band[3]),
                radius: 8.0 * s,
                width: 1.2 * s,
                color: color::alpha(color::GOLD_BRIGHT, 0.7),
            });
        }
        let middle = y + ROW_HEIGHT * 0.5;
        // The kind's chip, in the tier's colour; a group's lines show a bar of it only.
        if *inner {
            let _ = self.ui.draw_list_mut().push(DrawCommand::RoundedRect {
                rect: frame.rect(x + 12.0, middle - 8.0, 4.0, 16.0),
                radius: 2.0 * s,
                color: *colour,
            });
        } else {
            let _ = self.ui.draw_list_mut().push(DrawCommand::RoundedRect {
                rect: frame.rect(x + 10.0, middle - 12.0, CHIP_WIDTH, 24.0),
                radius: 12.0 * s,
                color: color::alpha(*colour, 0.18),
            });
            let _ = self.ui.draw_list_mut().push(DrawCommand::Border {
                rect: frame.rect(x + 10.0, middle - 12.0, CHIP_WIDTH, 24.0),
                radius: 12.0 * s,
                width: s,
                color: color::alpha(*colour, 0.7),
            });
            text(
                &mut self.ui,
                TextFamily::Display,
                format_args!("{chip}"),
                frame.rect(x + 10.0, middle - 12.0, CHIP_WIDTH, 24.0),
                16.0 * s,
                *colour,
                FontWeight::Semibold,
                TextAlign::Center,
            );
        }
        let mut text_x = if *inner {
            x + 30.0
        } else {
            x + CHIP_WIDTH + 24.0
        };
        // A group opens and closes on its sign.
        if let Some((_, open, _)) = group {
            text(
                &mut self.ui,
                TextFamily::Display,
                format_args!("{}", if *open { "-" } else { "+" }),
                frame.rect(text_x, middle - 13.0, 20.0, 26.0),
                24.0 * s,
                color::GOLD_BRIGHT,
                FontWeight::Semibold,
                TextAlign::Center,
            );
            text_x += 26.0;
        }
        let amount_x = RIGHT_X - WHEN_WIDTH - AMOUNT_WIDTH;
        let (shown, more) = cut(sentence, TEXT_CHARS - if *inner { 6 } else { 0 });
        text(
            &mut self.ui,
            TextFamily::Body,
            format_args!("{shown}{more}"),
            frame.rect(text_x, middle - 12.0, amount_x - text_x - 12.0, 24.0),
            17.0 * s,
            color::TEXT,
            FontWeight::Regular,
            TextAlign::Start,
        );
        // The amounts, right-aligned, the last nearest the date.
        let mut right = RIGHT_X - WHEN_WIDTH - 12.0;
        for amount in amounts.iter().rev() {
            let width = 120.0;
            text(
                &mut self.ui,
                TextFamily::Display,
                format_args!("{}", amount.text),
                frame.rect(right - width, middle - 12.0, width, 24.0),
                18.0 * s,
                if amount.gain { GAIN } else { SPEND },
                FontWeight::Semibold,
                TextAlign::End,
            );
            right -= width + 8.0;
        }
        text(
            &mut self.ui,
            TextFamily::Body,
            format_args!("{when}"),
            frame.rect(RIGHT_X - WHEN_WIDTH, middle - 12.0, WHEN_WIDTH - 14.0, 24.0),
            15.0 * s,
            color::MUTED,
            FontWeight::Regular,
            TextAlign::End,
        );
        self.ui
            .hit_region(token, frame.rect(band[0], band[1], band[2], band[3]));
    }

    /// Load older, at the end of the list, its top at `y`.
    fn history_more(&mut self, frame: &Frame, y: f32, chosen: bool) {
        let loading = self.history.loading();
        let width = 240.0;
        kit::button(
            &mut self.ui,
            frame,
            [
                (LEFT_X + RIGHT_X - width) * 0.5,
                y + 2.0,
                width,
                ROW_HEIGHT - 4.0,
            ],
            if loading { "Reading..." } else { "Load older" },
            false,
            !loading,
            chosen,
            MORE_TOKEN,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_sentences_are_cut_without_a_copy() {
        assert_eq!(cut("short", 10), ("short", ""));
        assert_eq!(cut("abcdef", 3), ("abc", "..."));
        assert_eq!(cut("\u{e9}t\u{e9} x", 2), ("\u{e9}t", "..."));
    }
}
