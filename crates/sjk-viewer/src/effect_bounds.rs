//! Screen rectangle that holds every blended effect of the main view, so the legacy effect
//! layer (`effect_layer.rs`) encodes and merges only where effects can draw.
//!
//! Conservative: a world sphere is bounded by the eight corners of its cube, a swept
//! segment by the corners at both ends (perspective keeps convex hulls convex), and
//! anything that reaches the camera plane or is drawn in clip space yields the whole
//! screen. CPU work is a few matrix products per effect; nothing is allocated.

use glam::{Mat4, Vec3, Vec4};

/// Pixels added on every side: bilinear taps and rasterisation rounding.
const MARGIN: f32 = 2.0;

/// How far, as a share of the target's longer side, a blade's heat haze may bend the
/// picture it samples (`saber.wgsl` `fragment_haze` caps its shift there). The haze and a
/// black hole's lens read the layer's `original` image that far outside the effects' own
/// pixels, and outside the rectangle it holds an older frame: without this much more room
/// the bent picture showed a stale strip, cut at the rectangle's straight edges (round 23,
/// Obsidian Edge's heat seen in front of a wall in Creyon's game). The room is added only on
/// frames that draw a haze or a lens ([`Bounds::bend`]).
pub(crate) const HAZE_REACH: f32 = 0.02;

/// The same for a black hole's lens (`saber.wgsl` `fragment_lens`).
pub(crate) const LENS_REACH: f32 = 0.06;

/// Accumulates the screen extent of world-space effect primitives.
pub(crate) struct Bounds {
    view_projection: Mat4,
    min: [f32; 2],
    max: [f32; 2],
    whole_screen: bool,
    /// Extra room on every side for a bent picture, as a share of the longer side.
    bent: f32,
}

impl Bounds {
    /// Start an empty extent for one view.
    pub(crate) fn new(view_projection: Mat4) -> Self {
        Self {
            view_projection,
            min: [f32::MAX; 2],
            max: [f32::MIN; 2],
            whole_screen: false,
            bent: 0.0,
        }
    }

    /// Leave `reach` (a share of the target's longer side, [`HAZE_REACH`] or
    /// [`LENS_REACH`]) more room on every side: this frame draws something that samples the
    /// picture that far from its own pixels.
    pub(crate) fn bend(&mut self, reach: f32) {
        self.bent = self.bent.max(reach);
    }

    /// Give up on a tight rectangle: the effects may cover any pixel.
    pub(crate) fn whole_screen(&mut self) {
        self.whole_screen = true;
    }

    /// A sphere of `radius` around `centre`.
    pub(crate) fn sphere(&mut self, centre: Vec3, radius: f32) {
        self.segment(centre, centre, radius);
    }

    /// A segment swept by a sphere of `radius`.
    pub(crate) fn segment(&mut self, start: Vec3, end: Vec3, radius: f32) {
        if self.whole_screen {
            return;
        }
        let radius = radius.abs().max(0.0);
        if !radius.is_finite() || !start.is_finite() || !end.is_finite() {
            self.whole_screen = true;
            return;
        }
        for point in [start, end] {
            for corner in 0..8 {
                let offset = Vec3::new(
                    if corner & 1 == 0 { -radius } else { radius },
                    if corner & 2 == 0 { -radius } else { radius },
                    if corner & 4 == 0 { -radius } else { radius },
                );
                let clip = self.view_projection * Vec4::from((point + offset, 1.0));
                if clip.w <= 1.0e-3 {
                    self.whole_screen = true;
                    return;
                }
                let ndc = [clip.x / clip.w, clip.y / clip.w];
                for axis in 0..2 {
                    self.min[axis] = self.min[axis].min(ndc[axis]);
                    self.max[axis] = self.max[axis].max(ndc[axis]);
                }
            }
        }
    }

    /// `None` for the whole screen, else `[x, y, width, height]` in pixels of a `size`
    /// target; a zero-area rectangle means nothing is on screen.
    pub(crate) fn rectangle(&self, size: [u32; 2]) -> Option<[u32; 4]> {
        if self.whole_screen {
            return None;
        }
        if self.min[0] > self.max[0] {
            return Some([0, 0, 0, 0]);
        }
        let [w, h] = size.map(|n| n as f32);
        let margin = MARGIN + (self.bent * w.max(h)).ceil();
        let left = ((self.min[0] * 0.5 + 0.5) * w - margin)
            .floor()
            .clamp(0.0, w);
        let right = ((self.max[0] * 0.5 + 0.5) * w + margin)
            .ceil()
            .clamp(0.0, w);
        let top = ((0.5 - self.max[1] * 0.5) * h - margin)
            .floor()
            .clamp(0.0, h);
        let bottom = ((0.5 - self.min[1] * 0.5) * h + margin)
            .ceil()
            .clamp(0.0, h);
        Some([
            left as u32,
            top as u32,
            (right - left) as u32,
            (bottom - top) as u32,
        ])
    }
}

impl crate::GpuState {
    /// Pixels of a `size` main-view target that this frame's blended effects can touch:
    /// billboards, effect geometry, saber blades and trails. `None` is the whole target.
    pub(crate) fn effect_region(
        &self,
        view_projection: Mat4,
        ranges: &crate::effect_submission::Ranges,
        size: [u32; 2],
    ) -> Option<[u32; 4]> {
        let mut bounds = Bounds::new(view_projection);
        for range in ranges.blended() {
            for sprite in self
                .entity_instances
                .get(range.start as usize..range.end as usize)
                .unwrap_or_default()
            {
                let position = Vec3::from_array(sprite.position);
                let direction = Vec3::from_array(sprite.direction);
                // entity.wgsl: 3 billboard, 4 line along `direction`, 5 oriented quad,
                // 6 clip-space overlay, 7 world icon; rotated quads reach √2 of their size.
                match sprite.kind {
                    4 => bounds.segment(position, position + direction, sprite.size.abs()),
                    3 | 5 | 7 => bounds.sphere(
                        position,
                        sprite.size.abs() * std::f32::consts::SQRT_2 + 0.04,
                    ),
                    _ => bounds.whole_screen(),
                }
            }
        }
        for blade in &self.saber_instances {
            let (start, end, radius) = blade.extent();
            bounds.segment(Vec3::from_array(start), Vec3::from_array(end), radius);
        }
        self.saber_gpu.bound_trails(&mut bounds);
        bounds.bend(self.saber_gpu.bent_reach());
        self.effect_geometry.bound(&mut bounds);
        self.tag_render.bound(&mut bounds);
        bounds.rectangle(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rectangle_leaves_room_for_the_bent_picture() {
        // A tiny effect in the middle of a 1920x1080 target.
        let mut bounds = Bounds::new(Mat4::IDENTITY);
        bounds.sphere(Vec3::new(0.0, 0.0, 0.5), 0.001);
        // Nothing bends the picture: the rectangle stays tight.
        let [x, y, w, h] = bounds.rectangle([1920, 1080]).unwrap();
        assert!(
            x >= 960 - 4 && w <= 8 && y >= 540 - 4 && h <= 8,
            "{x} {y} {w} {h}"
        );
        // The haze, then a lens, may sample this far from the pixels they draw.
        for reach in [HAZE_REACH, LENS_REACH] {
            bounds.bend(reach);
            let [x, y, w, h] = bounds.rectangle([1920, 1080]).unwrap();
            let room = (reach * 1920.0) as u32;
            assert!(x <= 960 - room && x + w >= 960 + room, "{x} {w}");
            assert!(y <= 540 - room && y + h >= 540 + room, "{y} {h}");
        }
        // A smaller reach after a larger one keeps the larger.
        bounds.bend(HAZE_REACH);
        let [x, ..] = bounds.rectangle([1920, 1080]).unwrap();
        assert!(x <= 960 - (LENS_REACH * 1920.0) as u32, "{x}");
        // The caps in the shader are what the reaches cover.
        let shader = include_str!("saber.wgsl");
        assert!(shader.contains("to_pixels(shift, dx, dy, 0.02 * max(size.x, size.y))"));
        assert!(shader.contains("to_pixels(shift, dx, dy, 0.06 * max(size.x, size.y))"));
        const { assert!(HAZE_REACH >= 0.02 && LENS_REACH >= 0.06) };
    }
}
