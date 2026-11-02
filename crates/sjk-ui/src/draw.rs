//! Renderer-neutral retained draw commands.

use serde::{Deserialize, Serialize};

use crate::{Color, Rect};

/// Stable identifier for text owned by the application.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TextId(pub u32);

/// Stable identifier for a texture owned by the renderer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TextureId(pub u32);

/// Horizontal text alignment inside its rectangle.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    /// Align to the leading edge.
    #[default]
    Start,
    /// Center the run.
    Center,
    /// Align to the trailing edge.
    End,
}

/// Handling for text that exceeds its rectangle.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextOverflow {
    /// Clip at the rectangle boundary.
    #[default]
    Clip,
    /// Replace the tail with an ellipsis.
    Ellipsis,
    /// Wrap onto subsequent lines.
    Wrap,
}

/// Renderer-independent font-weight request for a text run.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FontWeight {
    /// Normal body/caption weight.
    #[default]
    Regular,
    /// Strong emphasis without changing the type size.
    Semibold,
}

/// Linear two-color gradient description.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gradient {
    /// Color at the start edge.
    pub start: Color,
    /// Color at the end edge.
    pub end: Color,
    /// `true` for a vertical gradient, `false` for horizontal.
    pub vertical: bool,
}

/// The most corners a [`DrawCommand::Polygon`] has.
pub const POLYGON_POINTS: usize = 6;

/// One renderer-independent drawing operation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DrawCommand {
    /// Filled rectangle.
    SolidRect { rect: Rect, color: Color },
    /// Filled rounded rectangle.
    RoundedRect {
        rect: Rect,
        radius: f32,
        color: Color,
    },
    /// Filled linear gradient.
    GradientRect {
        rect: Rect,
        radius: f32,
        gradient: Gradient,
    },
    /// Rectangle outline.
    Border {
        rect: Rect,
        radius: f32,
        width: f32,
        color: Color,
    },
    /// Text resolved by the application at submission time.
    Text {
        rect: Rect,
        text: TextId,
        size: f32,
        color: Color,
        align: TextAlign,
        overflow: TextOverflow,
        /// Requested font weight.
        weight: FontWeight,
        /// Additional logical pixels between glyph advances.
        letter_spacing: f32,
    },
    /// Textured rectangle.
    TexturedQuad {
        rect: Rect,
        texture: TextureId,
        color: Color,
    },
    /// Textured rectangle with explicit texture coordinates at its corners
    /// (top-left, top-right, bottom-right, bottom-left), for rotated or
    /// scrolling images.
    TexturedQuadUv {
        rect: Rect,
        texture: TextureId,
        color: Color,
        uv: [[f32; 2]; 4],
    },
    /// Textured rectangle turned `degrees` about `pivot` (positive turns clockwise, as
    /// y grows downwards), as the classic HUD's `R_DrawRotatePic` turns a picture.
    TexturedQuadTurned {
        rect: Rect,
        texture: TextureId,
        color: Color,
        pivot: [f32; 2],
        degrees: f32,
    },
    /// Circular arc stroke with round caps, anti-aliased by the renderer. Angles are
    /// radians in screen space: 0 points right and positive turns clockwise (y grows
    /// downwards). `radius` is the centre line, `width` the stroke thickness.
    ///
    /// The stroke is left out inside `knockout`, a rectangle centred vertically on `center`
    /// (see [`crate::knockout_coverage`]): a translucent shadow there would stack on the
    /// translucent panel already drawn, and the two look like one shape only when the
    /// shadow adds just what the panel leaves ([`crate::knockout_remainder`]), the panel
    /// being the stroke's own colour and alpha.
    Arc {
        center: [f32; 2],
        radius: f32,
        width: f32,
        start: f32,
        sweep: f32,
        color: Color,
        knockout: Option<Rect>,
    },
    /// SJK's strafe aura centred on `center`, shaded per pixel by the renderer
    /// ([`crate::strafe_aura_shade`]).
    StrafeAura {
        center: [f32; 2],
        aura: crate::StrafeAura,
    },
    /// A card's finish over `rect` (the card as laid out), shaded per pixel by the
    /// renderer ([`crate::card_shine_shade`]) and cut to the card's rounded corners
    /// itself. A clip cuts it like any rectangle, keeping its pattern in place.
    CardShine { rect: Rect, shine: crate::CardShine },
    /// Straight stroke from `from` to `to` with square ends, anti-aliased along its
    /// sides by the renderer. `width` is the stroke thickness in pixels.
    Line {
        from: [f32; 2],
        to: [f32; 2],
        width: f32,
        color: Color,
    },
    /// Filled convex polygon: the first `count` (3 to [`POLYGON_POINTS`]) of `points`,
    /// in order round it, its edges anti-aliased by the renderer. With `texture`, the
    /// texture is spread over the polygon's bounding box, the box's corners taking the
    /// texture coordinates `uv` (top-left, bottom-right), and tinted by `color`. Like
    /// explicit coordinates, a polygon is not clipped.
    Polygon {
        points: [[f32; 2]; POLYGON_POINTS],
        count: u8,
        color: Color,
        texture: Option<(TextureId, [[f32; 2]; 2])>,
    },
    /// Begin clipping descendants.
    PushClip(Rect),
    /// Begin clipping descendants to `rect` with its corners rounded by `radius`: strokes
    /// ([`DrawCommand::Line`]) are cut to the rounded shape, everything else to the
    /// rectangle. Ended by [`DrawCommand::PopClip`] like a plain clip.
    PushRoundedClip { rect: Rect, radius: f32 },
    /// End the most recent clip.
    PopClip,
    /// Draw descendants through `matrix`, a projective transform of window points
    /// (row-major 3x3, applied to `(x, y, 1)` and divided by the third coordinate), for
    /// a card tilted in perspective: shapes, pictures and text are laid out as usual,
    /// then every corner is mapped. Clips are taken before the mapping. Ended by
    /// [`DrawCommand::PopTransform`]; nested ones compose.
    PushTransform([f32; 9]),
    /// End the most recent transform.
    PopTransform,
    /// Multiply descendant opacity.
    PushOpacity(f32),
    /// End the most recent opacity group.
    PopOpacity,
}

impl DrawCommand {
    /// Move what the command draws by `dx`, `dy` pixels: its rectangles, points,
    /// centres and pivots. A transform is conjugated, so what it maps moves too; opacity
    /// and the ends of groups have nothing to move.
    pub fn translate(&mut self, dx: f32, dy: f32) {
        let point = |p: &mut [f32; 2]| {
            p[0] += dx;
            p[1] += dy;
        };
        match self {
            Self::SolidRect { rect, .. }
            | Self::RoundedRect { rect, .. }
            | Self::GradientRect { rect, .. }
            | Self::Border { rect, .. }
            | Self::Text { rect, .. }
            | Self::TexturedQuad { rect, .. }
            | Self::TexturedQuadUv { rect, .. }
            | Self::CardShine { rect, .. }
            | Self::PushClip(rect)
            | Self::PushRoundedClip { rect, .. } => *rect = rect.translated(dx, dy),
            Self::TexturedQuadTurned { rect, pivot, .. } => {
                *rect = rect.translated(dx, dy);
                point(pivot);
            }
            Self::Arc {
                center, knockout, ..
            } => {
                point(center);
                if let Some(knockout) = knockout {
                    *knockout = knockout.translated(dx, dy);
                }
            }
            Self::StrafeAura { center, .. } => point(center),
            Self::Line { from, to, .. } => {
                point(from);
                point(to);
            }
            Self::Polygon { points, .. } => points.iter_mut().for_each(point),
            Self::PushTransform(matrix) => *matrix = conjugate_translation(*matrix, dx, dy),
            Self::PopClip | Self::PopTransform | Self::PushOpacity(_) | Self::PopOpacity => {}
        }
    }

    /// The pixels the command covers, roughly but never less: an arc's box is its
    /// stroke's, sampled along its sweep. `None` for commands that draw nothing
    /// themselves (clips, transforms, opacity groups); a transformed command's box is
    /// the one before its transform.
    pub fn bounds(&self) -> Option<Rect> {
        match *self {
            Self::SolidRect { rect, .. }
            | Self::RoundedRect { rect, .. }
            | Self::GradientRect { rect, .. }
            | Self::Border { rect, .. }
            | Self::Text { rect, .. }
            | Self::TexturedQuad { rect, .. }
            | Self::TexturedQuadUv { rect, .. }
            | Self::TexturedQuadTurned { rect, .. }
            | Self::CardShine { rect, .. } => Some(rect),
            Self::Arc {
                center,
                radius,
                width,
                start,
                sweep,
                ..
            } => {
                const STEPS: usize = 16;
                let half = width * 0.5;
                let mut bounds: Option<Rect> = None;
                for step in 0..=STEPS {
                    let angle = start + sweep * step as f32 / STEPS as f32;
                    let x = center[0] + radius * angle.cos();
                    let y = center[1] + radius * angle.sin();
                    let dot = Rect::new(x - half, y - half, width, width);
                    bounds = Some(bounds.map_or(dot, |bounds| bounds.union(dot)));
                }
                bounds
            }
            Self::StrafeAura { center, aura } => {
                let reach = aura.radius + aura.width;
                Some(Rect::new(
                    center[0] - reach,
                    center[1] - reach,
                    reach * 2.0,
                    reach * 2.0,
                ))
            }
            Self::Line {
                from, to, width, ..
            } => {
                let half = width * 0.5;
                let x = from[0].min(to[0]) - half;
                let y = from[1].min(to[1]) - half;
                Some(Rect::new(
                    x,
                    y,
                    from[0].max(to[0]) + half - x,
                    from[1].max(to[1]) + half - y,
                ))
            }
            Self::Polygon { points, count, .. } => {
                let points = &points[..usize::from(count).min(POLYGON_POINTS)];
                let first = points.first()?;
                let (mut low, mut high) = (*first, *first);
                for point in points {
                    low = [low[0].min(point[0]), low[1].min(point[1])];
                    high = [high[0].max(point[0]), high[1].max(point[1])];
                }
                Some(Rect::new(
                    low[0],
                    low[1],
                    high[0] - low[0],
                    high[1] - low[1],
                ))
            }
            Self::PushClip(_)
            | Self::PushRoundedClip { .. }
            | Self::PopClip
            | Self::PushTransform(_)
            | Self::PopTransform
            | Self::PushOpacity(_)
            | Self::PopOpacity => None,
        }
    }
}

/// `matrix` for content moved by (`dx`, `dy`): the move back, then `matrix`, then the
/// move, so moved content lands where the original landed, moved.
fn conjugate_translation(matrix: [f32; 9], dx: f32, dy: f32) -> [f32; 9] {
    let multiply = |a: [f32; 9], b: [f32; 9]| {
        let mut out = [0.0; 9];
        for row in 0..3 {
            for column in 0..3 {
                out[row * 3 + column] = (0..3).map(|k| a[row * 3 + k] * b[k * 3 + column]).sum();
            }
        }
        out
    };
    let to = [1.0, 0.0, dx, 0.0, 1.0, dy, 0.0, 0.0, 1.0];
    let back = [1.0, 0.0, -dx, 0.0, 1.0, -dy, 0.0, 0.0, 1.0];
    multiply(to, multiply(matrix, back))
}

/// Fixed-capacity command buffer reused between frames.
#[derive(Debug)]
pub struct DrawList {
    commands: Vec<DrawCommand>,
    capacity: usize,
}

impl DrawList {
    /// Allocate storage once for at most `capacity` commands.
    pub fn new(capacity: usize) -> Self {
        Self {
            commands: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Discard commands without releasing storage.
    pub fn clear(&mut self) {
        self.commands.clear();
    }

    /// Append a command, returning `false` if fixed storage is full.
    pub fn push(&mut self, command: DrawCommand) -> bool {
        if self.commands.len() == self.capacity {
            return false;
        }
        self.commands.push(command);
        true
    }

    /// Keep only the commands `keep` accepts, in their order, without releasing
    /// storage: for a screen that leaves out what a card drawn later covers.
    pub fn retain(&mut self, keep: impl FnMut(&DrawCommand) -> bool) {
        self.commands.retain(keep);
    }

    /// Commands in submission order.
    pub fn commands(&self) -> &[DrawCommand] {
        &self.commands
    }

    /// Number of retained commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Whether no commands are retained.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Fixed command limit.
    pub const fn limit(&self) -> usize {
        self.capacity
    }

    /// Move every command from index `start` on by `dx`, `dy` pixels
    /// ([`DrawCommand::translate`]): to place what one part of a screen drew.
    pub fn translate_from(&mut self, start: usize, dx: f32, dy: f32) {
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        for command in self.commands.iter_mut().skip(start) {
            command.translate(dx, dy);
        }
    }

    /// The box round what the commands from index `start` on draw
    /// ([`DrawCommand::bounds`]), `None` when they draw nothing.
    pub fn bounds_from(&self, start: usize) -> Option<Rect> {
        self.bounds_between(start, self.commands.len())
    }

    /// The box round what the commands from index `start` to `end` (excluded) draw.
    pub fn bounds_between(&self, start: usize, end: usize) -> Option<Rect> {
        self.commands
            .get(start..end.min(self.commands.len()))?
            .iter()
            .filter_map(DrawCommand::bounds)
            .reduce(Rect::union)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: Color = Color::new(1.0, 1.0, 1.0, 1.0);

    /// Moving the commands from an index moves only those, and their box with them.
    #[test]
    fn translating_from_an_index_moves_only_the_later_commands() {
        let mut list = DrawList::new(8);
        let first = Rect::new(0.0, 0.0, 10.0, 10.0);
        list.push(DrawCommand::SolidRect {
            rect: first,
            color: WHITE,
        });
        list.push(DrawCommand::Line {
            from: [20.0, 20.0],
            to: [40.0, 30.0],
            width: 2.0,
            color: WHITE,
        });
        list.push(DrawCommand::TexturedQuadTurned {
            rect: Rect::new(50.0, 50.0, 4.0, 4.0),
            texture: TextureId(1),
            color: WHITE,
            pivot: [52.0, 52.0],
            degrees: 30.0,
        });
        assert_eq!(list.bounds_from(1), Some(Rect::new(19.0, 19.0, 35.0, 35.0)));
        list.translate_from(1, 5.0, -10.0);
        assert_eq!(
            list.commands()[0],
            DrawCommand::SolidRect {
                rect: first,
                color: WHITE
            }
        );
        assert_eq!(list.bounds_from(1), Some(Rect::new(24.0, 9.0, 35.0, 35.0)));
        match list.commands()[2] {
            DrawCommand::TexturedQuadTurned { pivot, .. } => assert_eq!(pivot, [57.0, 42.0]),
            _ => unreachable!(),
        }
        assert_eq!(list.bounds_from(3), None);
        assert_eq!(list.bounds_between(0, 1), Some(first));
    }

    /// An arc's box is its stroke's: a quarter turn of radius 10 from the right to the
    /// bottom stays in the lower right quarter.
    #[test]
    fn an_arcs_box_follows_its_sweep() {
        let arc = DrawCommand::Arc {
            center: [100.0, 100.0],
            radius: 10.0,
            width: 2.0,
            start: 0.0,
            sweep: std::f32::consts::FRAC_PI_2,
            color: WHITE,
            knockout: None,
        };
        let bounds = arc.bounds().unwrap();
        assert!((bounds.x - 99.0).abs() < 1e-3, "{bounds:?}");
        assert!((bounds.y - 99.0).abs() < 1e-3, "{bounds:?}");
        assert!((bounds.right() - 111.0).abs() < 1e-3, "{bounds:?}");
        assert!((bounds.bottom() - 111.0).abs() < 1e-3, "{bounds:?}");
    }

    /// A transform moved with its content maps the moved content where the original
    /// mapped, moved.
    #[test]
    fn a_transform_moves_with_its_content() {
        // Twice the size about the origin.
        let matrix = [2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 1.0];
        let mut command = DrawCommand::PushTransform(matrix);
        command.translate(3.0, 4.0);
        let DrawCommand::PushTransform(moved) = command else {
            unreachable!()
        };
        let map = |m: [f32; 9], p: [f32; 2]| {
            [
                m[0] * p[0] + m[1] * p[1] + m[2],
                m[3] * p[0] + m[4] * p[1] + m[5],
            ]
        };
        let original = map(matrix, [5.0, 6.0]);
        assert_eq!(
            map(moved, [8.0, 10.0]),
            [original[0] + 3.0, original[1] + 4.0]
        );
    }
}
