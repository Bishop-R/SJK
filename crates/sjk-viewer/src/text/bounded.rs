//! Retained text stays inside its layout rectangle, including glyph shadows.
use super::*;
use sjk_ui::{Rect, TextOverflow};

/// Submit bounded text without letting the legacy viewport wrapping escape a row.
#[allow(clippy::too_many_arguments)]
pub(crate) fn append_bounded(
    vertices: &mut Vec<TextVertex>,
    font: &UiFont,
    value: &str,
    origin: [f32; 2],
    rect: Rect,
    clip: Rect,
    scale: f32,
    viewport: [f32; 2],
    face: TextFace,
    base_color: [f32; 4],
    spacing: f32,
    overflow: TextOverflow,
    palette: CodePalette,
) {
    let mut cursor = origin;
    let mut color = base_color;
    let bytes = value.as_bytes();
    let mut index = 0;
    let ellipsis_width = 3.0 * (font.glyph(face, b'.').advance * scale + spacing);
    let truncate = overflow == TextOverflow::Ellipsis
        && visible_text_width_style(font, value, scale, face, spacing) > rect.width;
    let right = rect.right() - if truncate { ellipsis_width } else { 0.0 };
    while index < bytes.len() && cursor[1] < clip.bottom() {
        if bytes[index] == b'^' && index + 1 < bytes.len() && bytes[index + 1].is_ascii_digit() {
            color = palette.colour(bytes[index + 1] - b'0');
            color[3] *= base_color[3];
            index += 2;
            continue;
        }
        let (key, step) = super::glyph_at(value, index);
        index += step;
        let glyph = font.glyph_of(face, key);
        let advance = glyph.advance * scale + spacing;
        let newline = key == GlyphKey::Slot(b'\n');
        if newline || cursor[0] + advance > right {
            if overflow == TextOverflow::Wrap {
                cursor = [rect.x, cursor[1] + font.height * scale * 1.18];
                if newline {
                    continue;
                }
            } else if newline || truncate {
                if truncate {
                    for _ in 0..3 {
                        glyph_quad(
                            vertices,
                            font.glyph(face, b'.'),
                            cursor,
                            scale,
                            color,
                            palette.edge(color),
                            clip,
                            viewport,
                        );
                        cursor[0] += ellipsis_width / 3.0;
                    }
                }
                break;
            }
        }
        let edge = palette.edge(color);
        glyph_quad(vertices, glyph, cursor, scale, color, edge, clip, viewport);
        cursor[0] += advance;
        if overflow == TextOverflow::Clip && cursor[0] >= clip.right() {
            break;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn glyph_quad(
    vertices: &mut Vec<TextVertex>,
    glyph: FontGlyph,
    cursor: [f32; 2],
    scale: f32,
    color: [f32; 4],
    edge: super::Edge,
    clip: Rect,
    viewport: [f32; 2],
) {
    let rect = Rect::new(
        cursor[0] + glyph.offset_x * scale,
        cursor[1] + glyph.offset_y * scale,
        glyph.width * scale,
        glyph.height * scale,
    );
    let step = scale.max(1.0);
    if let super::Edge::Ring(tint) = edge {
        // The same glyph in the ring's tint a step out all round, the diagonals too so
        // the ring stays even ([`CodePalette::Haloed`]): the copies overlap at the
        // glyph's edge into a soft rim.
        let d = step * std::f32::consts::FRAC_1_SQRT_2;
        for [dx, dy] in [
            [-step, 0.0],
            [step, 0.0],
            [0.0, -step],
            [0.0, step],
            [-d, -d],
            [d, -d],
            [-d, d],
            [d, d],
        ] {
            let ring = Rect::new(rect.x + dx, rect.y + dy, rect.width, rect.height);
            clipped_quad(vertices, ring, glyph.uv, tint, clip, viewport);
        }
    } else {
        let shadow = Rect::new(rect.x + step, rect.y + step, rect.width, rect.height);
        clipped_quad(
            vertices,
            shadow,
            glyph.uv,
            [0.0, 0.0, 0.0, 0.55 * color[3]],
            clip,
            viewport,
        );
    }
    clipped_quad(vertices, rect, glyph.uv, color, clip, viewport);
}

fn clipped_quad(
    vertices: &mut Vec<TextVertex>,
    rect: Rect,
    uv: [f32; 4],
    color: [f32; 4],
    clip: Rect,
    viewport: [f32; 2],
) {
    let left = rect.x.max(clip.x);
    let top = rect.y.max(clip.y);
    let right = rect.right().min(clip.right());
    let bottom = rect.bottom().min(clip.bottom());
    if right <= left || bottom <= top {
        return;
    }
    let u = |x| uv[0] + (uv[2] - uv[0]) * (x - rect.x) / rect.width;
    let v = |y| uv[1] + (uv[3] - uv[1]) * (y - rect.y) / rect.height;
    push_quad(
        vertices,
        [left, top, right - left, bottom - top],
        [u(left), v(top), u(right), v(bottom)],
        color,
        viewport,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dark_glyph_gets_a_halo_in_the_haloed_palette_only() {
        let font = test_font();
        let rect = Rect::new(0.0, 0.0, 200.0, 20.0);
        let draw = |palette| {
            let mut vertices = Vec::new();
            append_bounded(
                &mut vertices,
                &font,
                "^0a^7b",
                [0.0, 0.0],
                rect,
                rect,
                1.0,
                [640.0, 480.0],
                TextFace::Regular,
                [1.0; 4],
                0.0,
                TextOverflow::Clip,
                palette,
            );
            vertices
        };
        // Game colours: a shadow and a glyph each.
        assert_eq!(draw(CodePalette::Game).len(), 2 * 2 * 6);
        // Haloed: the black `a` edged by eight pale copies instead of its shadow, the
        // white `b` as before.
        let haloed = draw(CodePalette::Haloed);
        assert_eq!(haloed.len(), (9 + 2) * 6);
        assert!(haloed[..8 * 6].iter().all(|vertex| vertex.color[0] > 0.8));
        assert_eq!(haloed[8 * 6].color, [0.0, 0.0, 0.0, 1.0]);
        assert!(!CodePalette::Haloed.halo(quake_color(7)));
        assert!(!CodePalette::Game.halo(quake_color(0)));
        // Outlined: the black `a` in a stronger pale ring, the white `b` in a dark one.
        let outlined = draw(CodePalette::Outlined);
        assert_eq!(outlined.len(), (9 + 9) * 6);
        assert!(outlined[0].color[3] > haloed[0].color[3]);
        assert!(
            outlined[9 * 6..17 * 6]
                .iter()
                .all(|vertex| vertex.color[0] < 0.1)
        );
        assert_eq!(outlined[17 * 6].color, [1.0; 4]);
    }

    /// The chat's outline (`cg_chatBoxOutline`): a dark ring round every letter, red
    /// and blue ones too, in place of its shadow; a black letter keeps the faint light
    /// halo, as a ring as dark as it would hide it.
    #[test]
    fn the_dark_outline_rings_every_letter_but_black() {
        let font = test_font();
        let rect = Rect::new(0.0, 0.0, 200.0, 20.0);
        let mut vertices = Vec::new();
        append_bounded(
            &mut vertices,
            &font,
            "^0a^1b^4c^7d",
            [0.0, 0.0],
            rect,
            rect,
            1.0,
            [640.0, 480.0],
            TextFace::Regular,
            [1.0; 4],
            0.0,
            TextOverflow::Clip,
            CodePalette::DarkOutlined,
        );
        assert_eq!(
            vertices.len(),
            4 * 9 * 6,
            "eight copies and the letter each"
        );
        let ring = |letter: usize| &vertices[letter * 9 * 6..(letter * 9 + 8) * 6];
        assert!(ring(0).iter().all(|vertex| vertex.color[0] > 0.8));
        assert!(ring(0).iter().all(|vertex| vertex.color[3] < 0.2), "faint");
        for letter in 1..4 {
            assert!(
                ring(letter)
                    .iter()
                    .all(|vertex| vertex.color[..3] == [0.04, 0.04, 0.06]),
                "letter {letter}"
            );
        }
        assert!(!CodePalette::DarkOutlined.halo(quake_color(1)));
        assert!(CodePalette::DarkOutlined.halo(quake_color(0)));
    }

    /// `cg_darkTextOutline`: a dark letter gets a crisp white ring instead of the faint
    /// halo; a light one keeps its shadow, or the dark outline where that is on.
    #[test]
    fn the_white_outline_rings_dark_letters_in_white() {
        let font = test_font();
        let rect = Rect::new(0.0, 0.0, 200.0, 20.0);
        let draw = |palette| {
            let mut vertices = Vec::new();
            append_bounded(
                &mut vertices,
                &font,
                "^0a^7b",
                [0.0, 0.0],
                rect,
                rect,
                1.0,
                [640.0, 480.0],
                TextFace::Regular,
                [1.0; 4],
                0.0,
                TextOverflow::Clip,
                palette,
            );
            vertices
        };
        let white = draw(CodePalette::Haloed.white_outlined(true));
        assert_eq!(white.len(), (9 + 2) * 6);
        assert!(
            white[..8 * 6]
                .iter()
                .all(|v| v.color == [1.0, 1.0, 1.0, 0.85])
        );
        let two_tone = draw(CodePalette::Outlined.white_outlined(true));
        assert_eq!(two_tone.len(), (9 + 9) * 6);
        assert!(two_tone[..8 * 6].iter().all(|v| v.color[..3] == [1.0; 3]));
        assert!(two_tone[9 * 6..17 * 6].iter().all(|v| v.color[0] < 0.1));
        assert_eq!(
            CodePalette::DarkOutlined.white_outlined(true),
            CodePalette::TwoTone
        );
        assert_eq!(CodePalette::Game.white_outlined(true), CodePalette::Game);
        assert_eq!(
            CodePalette::Haloed.white_outlined(false),
            CodePalette::Haloed
        );
    }

    /// Hub #211 (Creo): outlined, a letter is nine quads instead of two, and a chat
    /// full of long lines ran past the old 32 768-vertex batch, which left the last
    /// lines out whole. A thousand outlined letters (a busy chat and its composer) fit.
    #[test]
    fn a_full_outlined_chat_fits_one_batch() {
        let font = test_font();
        let rect = Rect::new(0.0, 0.0, 100_000.0, 20.0);
        let mut vertices = Vec::new();
        append_bounded(
            &mut vertices,
            &font,
            &"^1a".repeat(1_000),
            [0.0, 0.0],
            rect,
            rect,
            1.0,
            [640.0, 480.0],
            TextFace::Regular,
            [1.0; 4],
            0.0,
            TextOverflow::Clip,
            CodePalette::DarkOutlined,
        );
        assert_eq!(vertices.len(), 1_000 * 9 * 6);
        assert!(vertices.len() > 32_768 && vertices.len() <= MAX_TEXT_VERTICES);
    }

    #[test]
    fn the_outline_setting_picks_the_palette() {
        assert_eq!(CodePalette::nameplate(0), CodePalette::Game);
        assert_eq!(CodePalette::nameplate(1), CodePalette::Haloed);
        assert_eq!(CodePalette::nameplate(2), CodePalette::Outlined);
        assert_eq!(CodePalette::nameplate(-3), CodePalette::Game);
        assert_eq!(CodePalette::nameplate(9), CodePalette::Outlined);
    }
}
