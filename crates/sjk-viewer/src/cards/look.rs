//! A card drawn from shapes (`docs/cards.md`, "In the client"): a sealed card shows its
//! back, a dark plate edged in its rarity's colour with the holocron's gem at its heart,
//! its rarity above, "Sealed" under the gem and its edition stamped at the foot. A
//! revealed card shows its face ([`super::faces`]), laid out as a trading card: its name
//! and a dot of its rarity's colour on top, its picture in a gold-edged 6:5 window, a
//! strip with its kind and set and its edition and finish, its lore in a box, and at the
//! foot its artist and its rarity (no number: it stays the card's id). The finish lies over it all,
//! shaded per pixel by the renderer ([`sjk_ui::CardShine`]): Holo a smooth rainbow foil sweeping
//! across the card with a glint, Solar the Sun blade's living corona burning along the
//! card's edge with flares running round it and sparks, a gold edge and a faint band of
//! sunlight over the face. One drawing serves the Cards tab, the opening's
//! ceremony and the card on show. A small card shows a face's picture without its words
//! ([`WORDS`], [`TITLE`]), or with its name under it on the Cards tab's tiles
//! ([`Card::label`]); a face's lore grows to fill its box when it is short ([`lore_fit`]).
//!
//! A card is a fixed number of commands for its finish ([`commands`]): nothing is
//! allocated, and the canvas's room is counted by the tests.

use super::faces::{ART_ASPECT, Face};
use super::{Finish, Rarity};
use crate::holocrons::gem;
use crate::menu::sjk::{Frame, color, text};
use crate::menu_widgets::{MenuCanvas, TextFamily};
use sjk_ui::{Color, DrawCommand, FontWeight, Gradient, ShineKind, TextAlign};

/// A card's height against its width.
pub(crate) const ASPECT: f32 = 1.4;
/// Gem rows for a card this wide (frame pixels) or wider, and fewer below it.
const BIG: f32 = 200.0;
const BIG_ROWS: usize = gem::ROWS;
const SMALL_ROWS: usize = 9;
/// The Holo rainbow's hues, red to violet, for the drawings that band it (the opening's
/// strip); the card itself shades a continuous rainbow ([`sjk_ui::CardShine`]).
pub(crate) const HOLO_BANDS: [[f32; 3]; 6] = [
    [1.0, 0.35, 0.45],
    [1.0, 0.75, 0.3],
    [0.55, 1.0, 0.45],
    [0.3, 0.9, 1.0],
    [0.45, 0.5, 1.0],
    [0.85, 0.45, 1.0],
];
/// A card's corner radius against its width.
const CORNER: f32 = 0.07;
/// How far a card turns towards the pointer at its edge, radians (about 9 degrees), and
/// the eye's distance for the perspective, in card heights.
const TILT_MAX: f32 = 0.16;
const EYE: f32 = 2.6;
/// A face's name shows on a card this wide (frame pixels) or wider, and all its words
/// from [`WORDS`]: below, they would be too small to read.
const TITLE: f32 = 200.0;
const WORDS: f32 = 300.0;
/// The face's lore lines at most, at its smallest size.
const LORE_LINES: usize = 7;
/// A short lore's text grows up to this many times its smallest size, tried in this
/// many steps down to it, to fill its box ([`lore_fit`]); one of this many lines or
/// fewer is centred.
const LORE_GROW: f32 = 1.25;
const LORE_STEPS: usize = 8;
const LORE_CENTRED: usize = 2;
/// A small card's name ([`Card::label`]): its text size against the card's width, and
/// the characters a line holds at the player's text size of 1.
const LABEL_SIZE: f32 = 0.13;
const LABEL_CHARS: f32 = 14.0;
/// A card not found yet: its plate's colour, its picture's tint and the grey veil over
/// the picture that takes most of its colour away.
const DIM_PLATE: Color = Color::new(0.42, 0.45, 0.52, 1.0);
const DIM_TINT: Color = Color::new(0.5, 0.5, 0.54, 1.0);
const DIM_VEIL: Color = Color::new(0.16, 0.17, 0.2, 0.55);
/// Solar's gold, its edge's.
const SUN: Color = Color::new(1.0, 0.8, 0.32, 1.0);

/// What a card shows.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Card<'a> {
    pub(crate) rarity: &'static Rarity,
    pub(crate) finish: &'static Finish,
    /// "Alpha Edition", or empty for no stamp.
    pub(crate) edition: &'a str,
    /// Its face, once revealed and in the packs; `None` draws the sealed back.
    pub(crate) face: Option<Shown<'a>>,
    /// Where the pointer rests on it, when it does: the card tilts towards it and its
    /// light follows it.
    pub(crate) tilt: Option<Tilt>,
    /// A card the player has not found yet (the Cards tab's binder): its plate grey, its
    /// picture darkened and greyed, its words quiet. Drawn with the Normal finish.
    pub(crate) dim: bool,
    /// A card narrower than [`TITLE`] names itself under its picture, with its rarity at
    /// its foot (the Cards tab's tiles; Sol's request of 20/10/2026, so
    /// that tiles with like pictures tell apart). Without it a small card shows its
    /// picture alone, where its name is written beside it (the opening's finds).
    pub(crate) label: bool,
}

/// Where the pointer rests on a card, eased in and out ([`crate::tile_shine::HoverEase`]).
pub(crate) use crate::tile_shine::Hover as Tilt;

/// A revealed card's face as a card draws it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Shown<'a> {
    pub(crate) face: &'a Face,
    /// Its position among the faces, for its picture's texture.
    pub(crate) index: usize,
}

impl<'a> Card<'a> {
    /// How it reads and is coloured: its face's rank or rarity, else (sealed) its own
    /// rarity's ([`super::Tone`]).
    pub(crate) fn tone(&self) -> super::Tone<'a> {
        super::Tone::of(self.rarity, self.face.map(|shown| shown.face))
    }
}

/// How many shape commands a card of `finish` that wide (frame pixels) takes, its words
/// apart.
#[cfg(test)]
pub(crate) fn commands(finish: &Finish, width: f32) -> usize {
    let rows = if width >= BIG { BIG_ROWS } else { SMALL_ROWS };
    let finish = usize::from(finish.index > 0);
    // Opacity, plate, wash, edges, ring, gem, stamp, clip.
    2 + 1 + 1 + 2 + 1 + gem::commands(rows) + 1 + 2 + finish
}

/// How many shape commands a revealed card's face takes, its words apart.
#[cfg(test)]
pub(crate) fn face_commands(finish: &Finish, width: f32) -> usize {
    let finish = usize::from(finish.index > 0);
    let boxes = if width >= WORDS { 2 } else { 0 };
    // Opacity, plate, wash, edge, the window's dark, picture, veil and gold edge, the
    // lore box, clip.
    2 + 1 + 1 + 1 + 4 + boxes + 2 + finish
}

/// `colour` at `alpha` times its own.
fn faded(colour: Color, alpha: f32) -> Color {
    Color::new(colour.r, colour.g, colour.b, colour.a * alpha)
}

/// Draw `card` over `rect` (frame pixels: x, y, width; its height follows [`ASPECT`]) at
/// `alpha`, `seconds` into the page's clock for its finish's motion. `turn` (0 to 1) is
/// how far it has turned to face the player: it narrows about its middle as it turns,
/// and its words show only once it faces.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    canvas: &mut MenuCanvas,
    frame: &Frame,
    [x, y, width]: [f32; 3],
    card: Card<'_>,
    seconds: f32,
    alpha: f32,
    turn: f32,
) {
    let Some(tilt) = card.tilt else {
        draw_flat(canvas, frame, [x, y, width], card, seconds, alpha, turn);
        return;
    };
    let height = width * ASPECT;
    let centre = frame.point(x + width * 0.5, y + height * 0.5);
    let _ = canvas
        .draw_list_mut()
        .push(DrawCommand::PushTransform(tilt_matrix(
            centre,
            height * frame.s,
            tilt,
        )));
    draw_flat(canvas, frame, [x, y, width], card, seconds, alpha, turn);
    let _ = canvas.draw_list_mut().push(DrawCommand::PopTransform);
}

/// The perspective that turns a card about `centre` (window pixels) towards the pointer
/// at `tilt`, the eye [`EYE`] times its `height` away: the side under the pointer
/// recedes, as if pressed.
pub(crate) fn tilt_matrix(centre: [f32; 2], height: f32, tilt: Tilt) -> [f32; 9] {
    let [lean_x, lean_y] = tilt.lean();
    let (yaw, pitch) = (lean_x * TILT_MAX, -lean_y * TILT_MAX);
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    let eye = height * EYE;
    let [ox, oy] = centre;
    // A point (u, v) of the card from its middle turns to x = u cos(yaw),
    // y = u sin(yaw) sin(pitch) + v cos(pitch), z = v sin(pitch) - u sin(yaw) cos(pitch),
    // and shows at the middle plus (x, y) times eye / (eye - z).
    let (zu, zv) = (-sy * cp, sp);
    let local = [
        eye * cy - ox * zu,
        -ox * zv,
        ox * eye,
        eye * sy * sp - oy * zu,
        eye * cp - oy * zv,
        oy * eye,
        -zu,
        -zv,
        eye,
    ];
    // Then from window points: (u, v) = (x - ox, y - oy).
    let mut out = [0.0; 9];
    for row in 0..3 {
        let [a, b, c] = [local[row * 3], local[row * 3 + 1], local[row * 3 + 2]];
        out[row * 3] = a;
        out[row * 3 + 1] = b;
        out[row * 3 + 2] = c - a * ox - b * oy;
    }
    out
}

/// [`draw`] without the tilt: the card as laid out, upright.
#[allow(clippy::too_many_arguments)]
fn draw_flat(
    canvas: &mut MenuCanvas,
    frame: &Frame,
    [x, y, width]: [f32; 3],
    card: Card<'_>,
    seconds: f32,
    alpha: f32,
    turn: f32,
) {
    let s = frame.s;
    let height = width * ASPECT;
    let turn = turn.clamp(0.0, 1.0);
    let shown = (width * turn).max(1.0);
    let left = x + (width - shown) * 0.5;
    let rect = frame.rect(left, y, shown, height);
    let radius = width * CORNER * s;
    let unit = width / 220.0;
    let rarity = card.rarity;
    let plate = if card.dim {
        DIM_PLATE
    } else {
        card.tone().colour
    };
    let edge = if card.finish.index == 2 { SUN } else { plate };
    let list = canvas.draw_list_mut();
    let _ = list.push(DrawCommand::PushOpacity(alpha.clamp(0.0, 1.0)));
    let _ = list.push(DrawCommand::RoundedRect {
        rect,
        radius,
        color: Color::new(0.035, 0.05, 0.1, 0.97),
    });
    let _ = list.push(DrawCommand::GradientRect {
        rect,
        radius,
        gradient: Gradient {
            start: faded(plate, 0.3),
            end: faded(plate, 0.04),
            vertical: true,
        },
    });
    let _ = list.push(DrawCommand::Border {
        rect,
        radius,
        width: (2.6 * unit * s).max(1.2),
        color: faded(edge, if card.dim { 0.6 } else { 0.95 }),
    });
    if let Some(shown_face) = card.face {
        face(
            canvas,
            frame,
            [x, y, width],
            [left, shown],
            card,
            shown_face,
            seconds,
            alpha,
            turn,
        );
        return;
    }
    let inset = 9.0 * unit * turn;
    let _ = list.push(DrawCommand::Border {
        rect: frame.rect(
            left + inset,
            y + 9.0 * unit,
            (shown - inset * 2.0).max(1.0),
            height - 18.0 * unit,
        ),
        radius: (radius - 6.0 * unit * s).max(1.0),
        width: (1.0 * s).max(1.0),
        color: Color::new(1.0, 1.0, 1.0, 0.12),
    });
    let [cx, cy] = frame.point(x + width * 0.5, y + height * 0.47);
    let _ = list.push(DrawCommand::Arc {
        center: [cx, cy],
        radius: width * 0.31 * s * turn.max(0.01),
        width: (1.6 * unit * s).max(1.0),
        start: 0.0,
        sweep: std::f32::consts::TAU,
        color: faded(edge, 0.4),
        knockout: None,
    });
    let rows = if width >= BIG { BIG_ROWS } else { SMALL_ROWS };
    gem::draw(
        list,
        [cx, cy],
        width * 0.2 * s * turn.max(0.01),
        rarity.colour,
        1.0,
        rows,
    );
    // The edition's stamp at the foot.
    let stamp = [
        x + width * 0.22,
        y + height - 40.0 * unit,
        width * 0.56,
        22.0 * unit,
    ];
    let _ = list.push(DrawCommand::RoundedRect {
        rect: frame.rect(
            stamp[0] + (stamp[2] - stamp[2] * turn) * 0.5,
            stamp[1],
            (stamp[2] * turn).max(1.0),
            stamp[3],
        ),
        radius: 11.0 * unit * s,
        color: if card.edition.is_empty() {
            Color::new(0.0, 0.0, 0.0, 0.0)
        } else {
            color::alpha(color::GOLD, 0.16)
        },
    });
    // The finish over everything, inside the card.
    finish_light(list, frame, card, [left, y, shown, height], width, seconds);
    let _ = list.push(DrawCommand::PopOpacity);
    if turn < 0.92 {
        return;
    }
    // The words, once it faces the player (text draws over every shape).
    let ink = alpha.clamp(0.0, 1.0);
    text(
        canvas,
        TextFamily::Display,
        format_args!(
            "{} {}",
            crate::symbols::card_rarity(rarity.index),
            rarity.name.to_uppercase()
        ),
        frame.rect(x, y + 16.0 * unit, width, 28.0 * unit),
        19.0 * unit * s,
        faded(rarity.colour, ink),
        FontWeight::Semibold,
        TextAlign::Center,
    );
    text(
        canvas,
        TextFamily::Body,
        format_args!("Sealed"),
        frame.rect(x, y + height * 0.47 + width * 0.33, width, 22.0 * unit),
        14.0 * unit * s,
        faded(color::MUTED, ink),
        FontWeight::Regular,
        TextAlign::Center,
    );
    if !card.edition.is_empty() {
        text(
            canvas,
            TextFamily::Body,
            format_args!("{}", card.edition),
            frame.rect(stamp[0], stamp[1] + 2.0 * unit, stamp[2], 18.0 * unit),
            12.0 * unit * s,
            faded(color::GOLD_BRIGHT, ink),
            FontWeight::Semibold,
            TextAlign::Center,
        );
    }
}

/// A revealed card's face over its plate (`[left, shown]` its turned span), then the
/// finish, then its words once it faces the player and is wide enough to read them.
#[allow(clippy::too_many_arguments)]
fn face(
    canvas: &mut MenuCanvas,
    frame: &Frame,
    [x, y, width]: [f32; 3],
    [left, shown]: [f32; 2],
    card: Card<'_>,
    face: Shown<'_>,
    seconds: f32,
    alpha: f32,
    turn: f32,
) {
    let s = frame.s;
    let height = width * ASPECT;
    let tone = face.face.tone();
    // Across the card as it turns: a share of the width from its left.
    let across = |share: f32| left + share * shown;
    let list = canvas.draw_list_mut();
    // The picture's window: dark until its picture is uploaded, then the picture.
    let art = [
        across(0.0413),
        y + width * 0.119,
        (0.917 * shown).max(1.0),
        0.917 * width / ART_ASPECT,
    ];
    let window = frame.rect(art[0], art[1], art[2], art[3]);
    let _ = list.push(DrawCommand::RoundedRect {
        rect: window,
        radius: width * 0.012 * s,
        color: Color::new(0.02, 0.03, 0.06, 1.0),
    });
    let _ = list.push(DrawCommand::TexturedQuad {
        rect: window,
        texture: super::faces::texture(face.index),
        color: if card.dim {
            DIM_TINT
        } else {
            Color::new(1.0, 1.0, 1.0, 1.0)
        },
    });
    // A card not found yet: a grey veil over the picture (none otherwise, so the card
    // keeps its number of commands).
    let _ = list.push(DrawCommand::RoundedRect {
        rect: window,
        radius: width * 0.012 * s,
        color: if card.dim {
            DIM_VEIL
        } else {
            Color::new(0.0, 0.0, 0.0, 0.0)
        },
    });
    let _ = list.push(DrawCommand::Border {
        rect: frame.rect(
            art[0] - width * 0.003,
            art[1] - width * 0.003,
            art[2] + width * 0.006,
            art[3] + width * 0.006,
        ),
        radius: width * 0.016 * s,
        width: (width * 0.0032 * s).max(1.0),
        color: if card.dim {
            color::alpha(color::MUTED, 0.5)
        } else {
            color::GOLD_BRIGHT
        },
    });
    let lore_box = [
        across(0.0413),
        y + width * 0.945,
        (0.917 * shown).max(1.0),
        height - width * 1.065,
    ];
    if width >= WORDS {
        let rect = frame.rect(lore_box[0], lore_box[1], lore_box[2], lore_box[3]);
        let _ = list.push(DrawCommand::RoundedRect {
            rect,
            radius: width * 0.018 * s,
            color: Color::new(0.067, 0.1, 0.19, 0.92),
        });
        let _ = list.push(DrawCommand::Border {
            rect,
            radius: width * 0.018 * s,
            width: (width * 0.0016 * s).max(1.0),
            color: Color::new(0.23, 0.28, 0.43, 0.9),
        });
    }
    // The finish over everything, inside the card.
    finish_light(list, frame, card, [left, y, shown, height], width, seconds);
    let _ = list.push(DrawCommand::PopOpacity);
    if turn < 0.92 {
        return;
    }
    if width < TITLE {
        if card.label {
            label(canvas, frame, [x, y, width], card, face, alpha);
        }
        return;
    }
    let ink = alpha.clamp(0.0, 1.0) * if card.dim { 0.6 } else { 1.0 };
    text(
        canvas,
        TextFamily::Display,
        format_args!("{}", face.face.name),
        frame.rect(
            x + width * 0.049,
            y + width * 0.03,
            width * 0.82,
            width * 0.08,
        ),
        width * 0.052 * s,
        faded(color::TEXT, ink),
        FontWeight::Semibold,
        TextAlign::Start,
    );
    // The rarity's symbol in the corner, in its colour (a rank has none).
    if let Some(mark) = tone.mark {
        text(
            canvas,
            TextFamily::Display,
            format_args!("{mark}"),
            frame.rect(
                x + width * 0.82,
                y + width * 0.03,
                width * 0.13,
                width * 0.08,
            ),
            width * 0.06 * s,
            faded(tone.colour, ink),
            FontWeight::Semibold,
            TextAlign::End,
        );
    }
    if width < WORDS {
        return;
    }
    let small = width * 0.025;
    let strip_y = art[1] + art[3] + width * 0.016;
    text(
        canvas,
        TextFamily::Body,
        format_args!("{}  ·  {}", face.face.kind.name(), face.face.set_name),
        frame.rect(x + width * 0.049, strip_y, width * 0.5, width * 0.036),
        small * s,
        faded(color::MUTED, ink),
        FontWeight::Semibold,
        TextAlign::Start,
    );
    let finish = if card.finish.index == 0 {
        String::new()
    } else {
        format!("  ·  {}", card.finish.name)
    };
    text(
        canvas,
        TextFamily::Body,
        format_args!("{}{finish}", card.edition),
        frame.rect(x + width * 0.45, strip_y, width * 0.501, width * 0.036),
        small * s,
        faded(color::GOLD_BRIGHT, ink),
        FontWeight::Semibold,
        TextAlign::End,
    );
    // The lore, cut into lines at spaces for the box's width at the player's text size,
    // as large as fits the box ([`lore_fit`]) and centred in it.
    let scale = crate::text::style::current().scale.max(0.5);
    let fit = lore_fit(&face.face.lore, scale);
    let step = width * 0.046 * fit.grow;
    let top = lore_box[1] + (lore_box[3] - step * fit.lines as f32) * 0.5;
    for (index, line) in crate::menu::sjk::wrap(&face.face.lore, fit.chars)
        .take(LORE_LINES)
        .enumerate()
    {
        text(
            canvas,
            TextFamily::Body,
            format_args!("{line}"),
            frame.rect(
                lore_box[0] + width * 0.028,
                top + index as f32 * step,
                lore_box[2] - width * 0.056,
                step,
            ),
            width * 0.033 * fit.grow * s,
            faded(color::TEXT, ink),
            FontWeight::Regular,
            fit.align,
        );
    }
    // At the foot its artist and its rarity; no number (Sol, 20/10/2026: it stays the
    // card's id and the binder's order).
    let foot_y = y + height - width * 0.094;
    text(
        canvas,
        TextFamily::Body,
        format_args!("Art: {}", face.face.artist),
        frame.rect(x + width * 0.049, foot_y, width * 0.55, width * 0.04),
        small * s,
        faded(color::MUTED, ink),
        FontWeight::Semibold,
        TextAlign::Start,
    );
    text(
        canvas,
        TextFamily::Body,
        format_args!("{tone}"),
        frame.rect(x + width * 0.6, foot_y, width * 0.351, width * 0.04),
        small * s,
        faded(tone.colour, ink),
        FontWeight::Semibold,
        TextAlign::End,
    );
}

/// A small card's words ([`Card::label`]): its name under its picture, centred on up to
/// two lines (the second ending in an ellipsis when it runs on), and at its foot its
/// rarity, its symbol and name. A card not found yet reads in grey, still legible.
fn label(
    canvas: &mut MenuCanvas,
    frame: &Frame,
    [x, y, width]: [f32; 3],
    card: Card<'_>,
    face: Shown<'_>,
    alpha: f32,
) {
    let s = frame.s;
    let ink = alpha.clamp(0.0, 1.0);
    let name_colour = if card.dim { color::MUTED } else { color::TEXT };
    let size = width * LABEL_SIZE;
    let step = size * 1.18;
    let scale = crate::text::style::current().scale.max(0.5);
    let chars = (LABEL_CHARS / scale).floor().max(6.0) as usize;
    let name = &face.face.name;
    let all = crate::menu::sjk::wrap(name, chars).count();
    let lines = all.min(2);
    // Between the picture's foot and the rarity's line.
    let band = [y + width * 0.895, width * 0.355];
    let top = band[0] + (band[1] - step * lines as f32) * 0.5;
    for (index, line) in crate::menu::sjk::wrap(name, chars).take(lines).enumerate() {
        let more = index + 1 == lines && all > lines;
        text(
            canvas,
            TextFamily::Display,
            format_args!("{line}{}", if more { "…" } else { "" }),
            frame.rect(
                x + width * 0.05,
                top + index as f32 * step,
                width * 0.9,
                step,
            ),
            size * s,
            faded(name_colour, ink),
            FontWeight::Semibold,
            TextAlign::Center,
        );
    }
    let foot = [y + width * (ASPECT - 0.14), width * 0.09];
    let quiet = faded(color::MUTED, ink * if card.dim { 0.75 } else { 1.0 });
    text(
        canvas,
        TextFamily::Body,
        format_args!("{}", face.face.tone()),
        frame.rect(x + width * 0.05, foot[0], width * 0.9, foot[1]),
        width * 0.078 * s,
        if card.dim {
            quiet
        } else {
            faded(face.face.tone().colour, ink)
        },
        FontWeight::Semibold,
        TextAlign::Center,
    );
}

/// How a face's lore fills its box: its text size against the smallest, the characters
/// a line holds at it, its lines and how they align.
#[derive(Clone, Copy, Debug, PartialEq)]
struct LoreFit {
    grow: f32,
    chars: usize,
    lines: usize,
    align: TextAlign,
}

/// The lore as large as its box holds it (Sol's request of 20/10/2026: a short lore
/// left most of the box empty): from [`LORE_GROW`] times the smallest size down to it,
/// the first size whose lines fit the room [`LORE_LINES`] smallest lines take. A lore of
/// [`LORE_CENTRED`] lines or fewer is centred, a longer one reads from the left. `scale`
/// is the player's text size, which widens the letters. Nothing is allocated.
fn lore_fit(lore: &str, scale: f32) -> LoreFit {
    // At the smallest size a line holds 62 characters at the player's text size of 1.
    let chars_at = |grow: f32| (62.0 / (scale * grow)).floor().max(12.0) as usize;
    let lines_at = |chars: usize| crate::menu::sjk::wrap(lore, chars).count();
    let mut fit = LoreFit {
        grow: 1.0,
        chars: chars_at(1.0),
        lines: 0,
        align: TextAlign::Start,
    };
    for step in 0..=LORE_STEPS {
        let grow = LORE_GROW - (LORE_GROW - 1.0) * step as f32 / LORE_STEPS as f32;
        let chars = chars_at(grow);
        let lines = lines_at(chars);
        if lines as f32 * grow <= LORE_LINES as f32 + 0.01 {
            fit = LoreFit {
                grow,
                chars,
                lines,
                align: fit.align,
            };
            break;
        }
    }
    if fit.lines == 0 {
        fit.lines = lines_at(fit.chars);
    }
    fit.lines = fit.lines.min(LORE_LINES);
    if fit.lines <= LORE_CENTRED {
        fit.align = TextAlign::Center;
    }
    fit
}

/// The finish's light over a card at `[x, y, width, height]` (frame pixels, `full` its
/// unturned width), inside its border and rounded corners: Holo's rainbow foil or
/// Solar's corona, their light following the pointer while it rests on the card
/// ([`Card::tilt`]), and a soft glare under the pointer whatever the finish
/// ([`crate::tile_shine`]).
fn finish_light(
    list: &mut sjk_ui::DrawList,
    frame: &Frame,
    card: Card<'_>,
    [x, y, width, height]: [f32; 4],
    full: f32,
    seconds: f32,
) {
    let s = frame.s;
    let unit = full / 220.0;
    // Inside the rarity's edge, so the light never lies over it.
    let inset = 3.5 * unit;
    let rect = frame.rect(
        x + inset,
        y + inset,
        (width - inset * 2.0).max(1.0),
        height - inset * 2.0,
    );
    let radius = ((full * CORNER - inset) * s).max(0.0);
    let kind = match card.finish.index {
        0 => None,
        1 => Some(ShineKind::Holo),
        _ => Some(ShineKind::Solar),
    };
    let _ = list.push(DrawCommand::PushRoundedClip { rect, radius });
    crate::tile_shine::push(list, rect, radius, kind, card.tilt, seconds);
    let _ = list.push(DrawCommand::PopClip);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Each finish takes the commands it says, at either gem size, turned or not.
    #[test]
    fn a_card_takes_a_fixed_number_of_commands() {
        let frame = Frame::new([1920.0, 1080.0]);
        for finish in &super::super::FINISHES {
            for width in [120.0, 320.0] {
                for turn in [0.3, 1.0] {
                    let mut canvas = MenuCanvas::with_capacities(16, 16, 512);
                    canvas.begin_transparent([1920.0, 1080.0]);
                    draw(
                        &mut canvas,
                        &frame,
                        [100.0, 100.0, width],
                        Card {
                            rarity: &super::super::RARITIES[2],
                            finish,
                            edition: "Alpha Edition",
                            face: None,
                            tilt: None,
                            dim: false,
                            label: false,
                        },
                        1.5,
                        1.0,
                        turn,
                    );
                    let shapes = canvas
                        .draw_list()
                        .commands()
                        .iter()
                        .filter(|command| !matches!(command, DrawCommand::Text { .. }))
                        .count();
                    assert_eq!(shapes, commands(finish, width), "{} {width}", finish.id);
                }
            }
        }
    }

    fn map(m: [f32; 9], [x, y]: [f32; 2]) -> [f32; 2] {
        let w = m[6] * x + m[7] * y + m[8];
        [
            (m[0] * x + m[1] * y + m[2]) / w,
            (m[3] * x + m[4] * y + m[5]) / w,
        ]
    }

    /// Upright, a card maps to itself; tilted towards the pointer at its right edge, the
    /// right side recedes (narrower and nearer the middle) and the left comes forward,
    /// the middle staying where it is.
    #[test]
    fn a_tilt_turns_the_side_under_the_pointer_away() {
        let centre = [960.0, 540.0];
        let flat = tilt_matrix(centre, 600.0, Tilt::default());
        for point in [[700.0, 300.0], [1_200.0, 800.0], centre] {
            let [x, y] = map(flat, point);
            assert!((x - point[0]).abs() < 1e-3 && (y - point[1]).abs() < 1e-3);
        }
        let right = tilt_matrix(centre, 600.0, Tilt::held([1.0, 0.0]));
        let middle = map(right, centre);
        assert!((middle[0] - centre[0]).abs() < 1e-3 && (middle[1] - centre[1]).abs() < 1e-3);
        let near_right = map(right, [1_160.0, 540.0]);
        let near_left = map(right, [760.0, 540.0]);
        assert!(near_right[0] - centre[0] < 200.0, "the right side recedes");
        assert!(
            centre[0] - near_left[0] > near_right[0] - centre[0],
            "the left comes forward"
        );
        // The receding side is shorter: its corners come together.
        let top = map(right, [1_160.0, 240.0]);
        let bottom = map(right, [1_160.0, 840.0]);
        assert!(bottom[1] - top[1] < 600.0);
        // Pointer at the foot: the foot recedes.
        let foot = tilt_matrix(centre, 600.0, Tilt::held([0.0, 1.0]));
        let low = map(foot, [960.0, 840.0]);
        assert!(low[1] - centre[1] < 300.0);
    }

    pub(crate) fn face() -> Face {
        Face {
            id: "sjk_alpha_010".to_owned(),
            name: "Toxiee and Sol on JoF Memorial".to_owned(),
            set: "sjk_alpha".to_owned(),
            set_name: "SJK Alpha".to_owned(),
            number: 10,
            kind: super::super::faces::Kind::Meme,
            rarity: &super::super::RARITIES[3],
            lore: "Tox was simply chilling when a pestering fox started taking pictures and                    turning them into art for its card collection, who knows why."
                .to_owned(),
            artist: "Sol".to_owned(),
            art: "cards/sjk_alpha_010.jpg".to_owned(),
            grants: None,
            blade: None,
            scene: None,
            rank: None,
        }
    }

    /// A lore fills its box (20/10/2026): a short one at the largest size, centred; a
    /// longer one smaller, its lines always within the room seven smallest ones take;
    /// one too long for that at the smallest, cut at seven lines and read from the left.
    #[test]
    fn a_lore_grows_to_fill_its_box() {
        let short = lore_fit("A walking battery that says its own name. Gonk.", 1.0);
        assert_eq!(short.grow, LORE_GROW);
        assert_eq!((short.lines, short.align), (1, TextAlign::Center));
        let lore = format!("{0} {0}", face().lore);
        let medium = lore_fit(&lore, 1.0);
        assert!(medium.grow > 1.0 && medium.grow < LORE_GROW, "{medium:?}");
        assert!(medium.lines as f32 * medium.grow <= LORE_LINES as f32 + 0.01);
        assert_eq!(medium.align, TextAlign::Start);
        let long = "word ".repeat(200);
        let long = lore_fit(&long, 1.0);
        assert_eq!((long.grow, long.lines), (1.0, LORE_LINES));
        // A larger text size holds fewer characters a line, so the lore grows less.
        let big = lore_fit(&lore, 1.3);
        assert!(
            big.grow < medium.grow && big.chars < medium.chars,
            "{big:?}"
        );
        assert_eq!(lore_fit("", 1.0).lines, 0);
    }

    /// A small card names itself only when asked (the binder's tiles): its name on up
    /// to two lines, an ellipsis when it runs on, its number and its rarity's symbol.
    #[test]
    fn a_small_card_names_itself_when_labelled() {
        let frame = Frame::new([1920.0, 1080.0]);
        let mut face = face();
        face.name = "Jedi Master Kyle Katarn".to_owned();
        for (label, name) in [(false, None), (true, Some(face.name.clone()))] {
            let mut canvas = MenuCanvas::with_capacities(32, 128, 512);
            canvas.begin_transparent([1920.0, 1080.0]);
            draw(
                &mut canvas,
                &frame,
                [100.0, 100.0, 140.0],
                Card {
                    rarity: face.rarity,
                    finish: &super::super::FINISHES[0],
                    edition: "",
                    face: Some(Shown {
                        face: &face,
                        index: 0,
                    }),
                    tilt: None,
                    dim: true,
                    label,
                },
                1.5,
                1.0,
                1.0,
            );
            let runs: Vec<&str> = canvas.text_runs().collect();
            let Some(name) = name else {
                assert!(runs.is_empty(), "{runs:?}");
                continue;
            };
            assert_eq!(runs.len(), 3, "two lines and the rarity: {runs:?}");
            assert_eq!(format!("{} {}", runs[0], runs[1]), name);
            assert!(runs[2].ends_with("Legendary"), "no number: {runs:?}");
        }
        face.name = "A name far too long to be held on two lines of a tile".to_owned();
        let mut canvas = MenuCanvas::with_capacities(32, 128, 512);
        canvas.begin_transparent([1920.0, 1080.0]);
        draw(
            &mut canvas,
            &frame,
            [100.0, 100.0, 140.0],
            Card {
                rarity: face.rarity,
                finish: &super::super::FINISHES[0],
                edition: "",
                face: Some(Shown {
                    face: &face,
                    index: 0,
                }),
                tilt: None,
                dim: false,
                label: true,
            },
            1.5,
            1.0,
            1.0,
        );
        let runs: Vec<&str> = canvas.text_runs().collect();
        assert!(runs[1].ends_with('…'), "{runs:?}");
    }

    /// A face takes the commands it says in each finish, with its words from the width
    /// that reads them: none on a small card, all of them on a big one.
    #[test]
    fn a_face_takes_a_fixed_number_of_commands_and_its_words_by_width() {
        let frame = Frame::new([1920.0, 1080.0]);
        let face = face();
        for finish in &super::super::FINISHES {
            for (width, words) in [(128.0, 0), (240.0, 1), (500.0, 7)] {
                let mut canvas = MenuCanvas::with_capacities(32, 128, 512);
                canvas.begin_transparent([1920.0, 1080.0]);
                draw(
                    &mut canvas,
                    &frame,
                    [100.0, 100.0, width],
                    Card {
                        rarity: face.rarity,
                        finish,
                        edition: "Alpha Edition",
                        face: Some(Shown {
                            face: &face,
                            index: 0,
                        }),
                        tilt: None,
                        dim: false,
                        label: false,
                    },
                    1.5,
                    1.0,
                    1.0,
                );
                let commands = canvas.draw_list().commands();
                let shapes = commands
                    .iter()
                    .filter(|command| !matches!(command, DrawCommand::Text { .. }))
                    .count();
                assert_eq!(
                    shapes,
                    face_commands(finish, width),
                    "{} {width}",
                    finish.id
                );
                let runs: Vec<&str> = canvas.text_runs().collect();
                assert!(runs.len() >= words, "{width}: {runs:?}");
                if words == 0 {
                    assert!(runs.is_empty(), "{runs:?}");
                } else {
                    assert_eq!(runs[0], "Toxiee and Sol on JoF Memorial");
                }
                if width >= WORDS {
                    assert!(
                        !runs.iter().any(|run| run.contains("010")),
                        "no number: {runs:?}"
                    );
                    assert!(runs.contains(&"Art: Sol"), "{runs:?}");
                }
            }
        }
    }
}
