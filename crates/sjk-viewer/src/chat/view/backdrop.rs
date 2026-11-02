//! The dark band behind each row of a chat message (`cg_chatBoxBackdrop`; Creo's idea,
//! 29/10/2026: the chat is hard to read over a bright map, even with vanilla graphics).
//! Each drawn row gets its own band, as long as what the row shows (the sender's
//! emblems and name on a line's first row, its text and emoji pictures) and a little
//! more, with round ends, in the SJK UI's navy ([`crate::menu::sjk::color::SPACE`]) at
//! the setting's opacity times the line's own: it slides in, moves and fades out with
//! its line. Nothing else gets one: no empty space, no GIF under a line, nothing while
//! the feed shows no line. Bands are pushed before any line's content, so a picture
//! reaching over a neighbouring row (the SJK emblem is taller than a row) stays on top.

use super::*;

/// A band's height against the row pitch: the rest parts it from the next row's.
const HEIGHT: f32 = 0.9;

/// Push the bands behind `line`'s rows, its first row's top-left at `origin`, at
/// `opacity` (the setting's times the line's); `prefix` is what a line with a sender
/// draws before its message on the first row.
pub(super) fn draw(
    ui: &mut MenuCanvas,
    line: &ChatLine,
    prefix: Option<&sjk_line::Prefix<'_>>,
    font: &UiFont,
    g: &Geometry,
    origin: [f32; 2],
    opacity: f32,
) {
    let [x, y] = origin;
    let height = g.row * HEIGHT;
    let middle = ink_middle(font, g.font);
    let colour = crate::menu::sjk::color::alpha(crate::menu::sjk::color::SPACE, opacity);
    let rows = if prefix.is_some() && line.muted {
        1
    } else {
        line.wrap.len
    };
    for row in 0..rows {
        let mut width = if prefix.is_some() && line.muted {
            visible_text_width_face(
                font,
                sjk_line::MUTED_NOTE,
                sjk_line::muted_size(g) / font.height.max(1.0),
                TextFace::Regular,
            )
        } else {
            row_width(line, row, font, g)
        };
        if row == 0 {
            width += prefix.map_or(0.0, sjk_line::Prefix::width);
        }
        if width <= 0.0 {
            continue;
        }
        let top = y + row as f32 * g.row + middle - height * 0.5;
        let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
            rect: Rect::new(x - height * 0.5, top, width.min(g.width) + height, height),
            radius: height * 0.5,
            color: colour,
        });
    }
}

/// How wide body row `row` of `line` is drawn: its text and pictures, and the
/// ellipsis of a last row cut short.
fn row_width(line: &ChatLine, row: usize, font: &UiFont, g: &Geometry) -> f32 {
    let range = line.wrap.rows[row].clone();
    let truncated = row + 1 == line.wrap.len && range.end < line.body.len();
    let mut width = layout::drawn_width(&line.body[range], font, g.font, g.spacing);
    if truncated {
        width += visible_text_width_style(
            font,
            "...",
            g.font / font.height.max(1.0),
            TextFace::Regular,
            g.spacing,
        );
    }
    width
}

/// How far below a row's top the middle of its ink is at text `size`: half way from
/// the capitals' top to the descenders' foot.
fn ink_middle(font: &UiFont, size: f32) -> f32 {
    let capital = font.glyph(TextFace::Regular, b'H');
    let descender = font.glyph(TextFace::Regular, b'p');
    (capital.offset_y + descender.offset_y + descender.height) * 0.5 * size / font.height.max(1.0)
}

#[cfg(test)]
mod tests {
    use crate::chat::ChatOverlay;
    use crate::menu::sjk::color::SPACE;
    use crate::text::{TextFace, test_font, visible_text_width_style};
    use sjk_client::ServerEventKind;
    use sjk_identity::{ChatMessage, ChatState};
    use sjk_ui::{Color, DrawCommand, Rect};
    use std::time::Instant;

    const VIEWPORT: [f32; 2] = [1920.0, 1080.0];
    const LONG: &str = "the quick brown fox jumps over the lazy dog and keeps running far away";

    /// A feed of a short and a long game line (no sender, the server's words), the
    /// band at `backdrop` (0 to 1), built `ms` after they came.
    fn feed(backdrop: f32, ms: u64) -> ChatOverlay {
        let mut chat = ChatOverlay::new();
        chat.options.backdrop = backdrop;
        let now = Instant::now();
        for line in ["gg", LONG] {
            chat.receive(ServerEventKind::Chat, line.to_owned(), None, now);
        }
        let received = chat.lines[0].received_ms;
        chat.build(true, &test_font(), VIEWPORT, received + ms);
        chat
    }

    fn is_band(command: &DrawCommand) -> bool {
        matches!(command, DrawCommand::RoundedRect { color, .. }
            if [color.r, color.g, color.b] == [SPACE.r, SPACE.g, SPACE.b])
    }

    /// The bands drawn: the rounded boxes in the SJK UI's navy, and their colours.
    fn bands(chat: &ChatOverlay) -> Vec<(Rect, Color)> {
        chat.ui
            .draw_list()
            .commands()
            .iter()
            .filter_map(|command| match command {
                DrawCommand::RoundedRect { rect, color, .. } if is_band(command) => {
                    Some((*rect, *color))
                }
                _ => None,
            })
            .collect()
    }

    /// The band on the row drawn at `rect`.
    fn band_of(bands: &[(Rect, Color)], rect: Rect) -> (Rect, Color) {
        *bands
            .iter()
            .find(|(band, _)| band.y <= rect.y + 5.0 && band.bottom() > rect.y)
            .unwrap_or_else(|| panic!("no band at {rect:?} in {bands:?}"))
    }

    /// Every text run drawn, its rectangle and alpha.
    fn texts(chat: &ChatOverlay) -> Vec<(String, Rect, f32)> {
        chat.ui
            .draw_list()
            .commands()
            .iter()
            .filter_map(|command| match command {
                DrawCommand::Text {
                    rect, text, color, ..
                } => Some((chat.ui.stored_text(*text).to_owned(), *rect, color.a)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn off_by_default_and_on_it_only_adds_the_bands() {
        let defaults = crate::chat::options::Options::default();
        assert_eq!(defaults.backdrop, 0.0);
        assert!(!defaults.outline);
        let off = feed(defaults.backdrop, 1_000);
        assert!(bands(&off).is_empty());
        let on = feed(0.5, 1_000);
        assert!(!bands(&on).is_empty());
        // The same draw list, the bands taken out.
        let without: Vec<DrawCommand> = on
            .ui
            .draw_list()
            .commands()
            .iter()
            .filter(|command| !is_band(command))
            .copied()
            .collect();
        assert_eq!(off.ui.draw_list().commands(), without.as_slice());
    }

    /// One band behind each drawn row, from just before the row to just after its last
    /// letter: a longer row has a longer band, and rows' bands never overlap.
    #[test]
    fn each_row_has_a_band_as_long_as_it() {
        let chat = feed(0.5, 1_000);
        let font = test_font();
        let g = chat.options.geometry(VIEWPORT);
        let texts = texts(&chat);
        let bands = bands(&chat);
        assert!(texts.len() >= 3, "the long line wraps: {texts:?}");
        assert_eq!(bands.len(), texts.len(), "a band a row");
        let height = g.row * super::HEIGHT;
        for (text, rect, _) in &texts {
            let width = visible_text_width_style(
                &font,
                text,
                g.font / font.height,
                TextFace::Regular,
                g.spacing,
            );
            let (band, _) = band_of(&bands, *rect);
            assert!((band.x - (rect.x - height * 0.5)).abs() < 0.01, "{band:?}");
            assert!(
                (band.right() - (rect.x + width + height * 0.5)).abs() < 0.01,
                "{text:?}: {band:?}"
            );
            assert!((band.height - height).abs() < 0.01);
        }
        let widths = bands.iter().map(|(band, _)| band.width);
        let shortest = widths.clone().fold(f32::MAX, f32::min);
        let longest = widths.fold(0.0, f32::max);
        assert!(shortest < longest * 0.25, "{shortest} {longest}");
        for (index, (band, _)) in bands.iter().enumerate() {
            for (other, _) in &bands[index + 1..] {
                assert!(band.bottom() <= other.y || other.bottom() <= band.y);
            }
        }
    }

    /// A band is as opaque as the setting times its line, so it fades out with it, and
    /// goes when the line goes: nothing is drawn while the chat shows nothing.
    #[test]
    fn a_band_fades_and_goes_with_its_line() {
        let chat = feed(0.6, 9_000);
        let texts = texts(&chat);
        let (_, gg, alpha) = texts.iter().find(|(text, ..)| text == "gg").unwrap();
        assert!(*alpha > 0.3 && *alpha < 0.7, "fading: {alpha}");
        let (_, colour) = band_of(&bands(&chat), *gg);
        assert!((colour.a - 0.6 * alpha).abs() < 1e-4, "{colour:?} {alpha}");
        let gone = feed(0.6, 10_500);
        assert!(gone.ui.draw_list().commands().is_empty());
        let mut empty = ChatOverlay::new();
        empty.options.backdrop = 1.0;
        empty.build(true, &test_font(), VIEWPORT, 1_000);
        assert!(empty.ui.draw_list().commands().is_empty());
    }

    fn message(id: u64, name: &str, text: &str) -> ChatMessage {
        ChatMessage {
            reactions: Default::default(),
            id,
            at: 0,
            key_id: format!("{id:016x}"),
            name: name.to_owned(),
            verified: true,
            tick: sjk_identity::Tick::None,
            staff: false,
            creator: false,
            trust: None,
            emoji: String::new(),
            client: String::new(),
            text: text.to_owned(),
            holocron: None,
            official: None,
        }
    }

    /// An SJK line's band runs from its emblem past its message, under the emblem, and
    /// stops above the GIF shown under it; every band comes before any picture.
    #[test]
    fn an_sjk_lines_band_sits_under_its_marks_and_not_its_gif() {
        let mut chat = ChatOverlay::new();
        chat.options.backdrop = 0.5;
        let now = Instant::now();
        let state = |messages| ChatState {
            messages,
            loaded: Some(1),
            ..ChatState::default()
        };
        chat.sync_sjk(&state(Default::default()), |_| false, now);
        chat.sync_sjk(
            &state(
                [
                    message(1, "Sol", "look https://giphy.com/gifs/wave-BandGif01"),
                    message(2, "Fox", "gg"),
                ]
                .into_iter()
                .collect(),
            ),
            |_| false,
            now,
        );
        chat.build(true, &test_font(), VIEWPORT, 1_000);
        let commands = chat.ui.draw_list().commands();
        let bands = bands(&chat);
        assert_eq!(bands.len(), 2, "one row each, none for the GIF");
        let gif = commands
            .iter()
            .find_map(|command| match command {
                DrawCommand::RoundedRect { rect, .. } if !is_band(command) => Some(*rect),
                _ => None,
            })
            .expect("the GIF's box");
        for (band, _) in &bands {
            assert!(
                band.bottom() <= gif.y || band.y >= gif.bottom(),
                "{band:?} {gif:?}"
            );
        }
        let logos: Vec<Rect> = commands
            .iter()
            .filter_map(|command| match command {
                DrawCommand::TexturedQuad { rect, texture, .. }
                    if *texture == crate::ui_renderer::LOGO_TEXTURE =>
                {
                    Some(*rect)
                }
                _ => None,
            })
            .collect();
        assert_eq!(logos.len(), 2);
        let texts = texts(&chat);
        for (logo, words) in logos.iter().zip(["look GIF", "gg"]) {
            let (_, body, _) = texts.iter().find(|(text, ..)| text == words).unwrap();
            let (band, _) = band_of(&bands, *body);
            assert!(
                band.x < logo.x && band.right() > body.x,
                "{band:?} {logo:?}"
            );
        }
        assert!(commands[..bands.len()].iter().all(is_band), "bands first");
    }
}
