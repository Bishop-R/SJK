//! Shown faces: sides of the world's brushes that the map draws nothing on (caulk,
//! nodraw), drawn by a map edit with a shader staff choose (`Kind::ShowFace`).
//!
//! q3map2 makes no drawn surface for such a side, and a BSP keeps no texture projection
//! for brush sides, so the face is rebuilt from its brush's planes (as `r_showBrushes`
//! does) and textured the way Radiant textures a new brush: the axial projection at
//! scale 0.5, no shift, no turn. It is drawn as a small model with entity lighting.

use crate::brush_view::kinds::Kind as BrushKind;
use crate::scene_flatten::{ActorDraw, FlattenedScene, ViewerMaterial};
use glam::Vec3;
use sjk_bsp::Bsp;

/// A side smaller than this (square units) is a bevel or a sliver: not offered.
const MIN_AREA: f32 = 0.5;
/// Radiant's default texture scale: a texel covers half a unit.
const TEXTURE_SCALE: f32 = 0.5;
/// What a texture of unknown size is taken to measure.
const DEFAULT_TEXTURE_SIZE: [f32; 2] = [256.0; 2];

/// A world brush side the ray met that edits may show.
#[derive(Clone, Debug)]
pub(crate) struct Face {
    pub(crate) brush: u32,
    pub(crate) side: u32,
    /// The side's own shader (`textures/system/caulk`).
    pub(crate) shader: String,
    pub(crate) distance: f32,
    pub(crate) normal: Vec3,
    /// Its corners in order around it.
    pub(crate) polygon: Vec<[f32; 3]>,
}

/// Whether side `side` of brush `brush` is one the map draws nothing on and edits may
/// show: a solid brush's caulk or nodraw side, not a tool volume (clip, trigger, water).
fn showable(bsp: &Bsp, brush: usize, side: usize) -> Option<String> {
    let brush = bsp.brushes().get(brush)?;
    let side = bsp.brush_sides().get(side)?;
    let shader = bsp.shaders().get(side.shader)?;
    matches!(
        BrushKind::of_face(brush.content_flags, shader.surface_flags, shader.name()),
        Some(BrushKind::Caulk | BrushKind::Solid | BrushKind::Nodraw)
    )
    .then(|| shader.name_lossy().into_owned())
}

/// The world brush side the ray from `eye` along `direction` (normalized) enters first
/// within `reach`, when it is one edits may show.
pub(crate) fn pick(bsp: &Bsp, eye: Vec3, direction: Vec3, reach: f32) -> Option<Face> {
    let world = bsp.render().models().first()?;
    let mut nearest: Option<(f32, usize, usize)> = None;
    for brush_index in world.brushes.clone() {
        let Some(brush) = bsp.brushes().get(brush_index) else {
            continue;
        };
        let (mut enter, mut exit, mut entered) = (f32::NEG_INFINITY, reach, None);
        let mut missed = false;
        for side_index in brush.sides.clone() {
            let Some(plane) = bsp
                .brush_sides()
                .get(side_index)
                .and_then(|side| bsp.planes().get(side.plane))
            else {
                missed = true;
                break;
            };
            let normal = Vec3::from_array(plane.normal);
            let start = normal.dot(eye) - plane.distance;
            let along = normal.dot(direction);
            if along.abs() < 1e-6 {
                if start > 0.0 {
                    missed = true;
                    break;
                }
                continue;
            }
            let at = -start / along;
            if along < 0.0 {
                if at > enter {
                    enter = at;
                    entered = Some(side_index);
                }
            } else if at < exit {
                exit = at;
            }
            if enter > exit {
                missed = true;
                break;
            }
        }
        // Behind the eye, or the eye inside the brush: not a face it sees.
        let Some(side) = entered.filter(|_| !missed && enter >= 0.0) else {
            continue;
        };
        if nearest.is_none_or(|(distance, _, _)| enter < distance) {
            nearest = Some((enter, brush_index, side));
        }
    }
    let (distance, brush, side) = nearest?;
    let shader = showable(bsp, brush, side)?;
    let polygon = bsp.brush_side_polygon(brush, side);
    if area(&polygon) < MIN_AREA {
        return None;
    }
    let plane = bsp.planes()[bsp.brush_sides()[side].plane];
    Some(Face {
        brush: brush as u32,
        side: side as u32,
        shader,
        distance,
        normal: Vec3::from_array(plane.normal),
        polygon,
    })
}

/// Side `side` of brush `brush` as an edit names it, when it is still one edits may show
/// and still carries `shader`: its corners and its plane's normal.
pub(crate) fn polygon(
    bsp: &Bsp,
    brush: u32,
    side: u32,
    shader: &str,
) -> Option<(Vec<[f32; 3]>, [f32; 3])> {
    let (brush, side) = (brush as usize, side as usize);
    let model = bsp.render().models().first()?;
    if !model.brushes.contains(&brush) {
        return None;
    }
    if !showable(bsp, brush, side)?.eq_ignore_ascii_case(shader) {
        return None;
    }
    let polygon = bsp.brush_side_polygon(brush, side);
    if area(&polygon) < MIN_AREA {
        return None;
    }
    let normal = bsp.planes()[bsp.brush_sides()[side].plane].normal;
    Some((polygon, normal))
}

/// The middle of `polygon`'s corners.
pub(crate) fn centre(polygon: &[[f32; 3]]) -> [f32; 3] {
    let sum = polygon
        .iter()
        .fold(Vec3::ZERO, |sum, point| sum + Vec3::from_array(*point));
    (sum / polygon.len().max(1) as f32).to_array()
}

/// Append the face `polygon` (corners in order, `normal` out of its brush) drawn with
/// `shader` to `scene`, its positions relative to `origin` (where it is lit from), its
/// texture laid as Radiant lays it on a `size` (width, height) texture.
pub(crate) fn append(
    scene: &mut FlattenedScene,
    polygon: &[[f32; 3]],
    normal: [f32; 3],
    origin: [f32; 3],
    shader: &str,
    size: Option<[u32; 2]>,
) -> Result<Vec<ActorDraw>, Box<dyn std::error::Error>> {
    if polygon.len() < 3 {
        return Err("a face has three corners at least".into());
    }
    let size = size.map_or(DEFAULT_TEXTURE_SIZE, |[w, h]| {
        [w.max(1) as f32, h.max(1) as f32]
    });
    let (s, t) = texture_axes(normal);
    let vertex_base: u32 = scene.vertices.len().try_into()?;
    let index_start: u32 = scene.indices.len().try_into()?;
    scene.vertices.extend(polygon.iter().map(|point| {
        let world = Vec3::from_array(*point);
        crate::GpuVertex {
            position: (world - Vec3::from_array(origin)).to_array(),
            normal,
            color: [1.0; 4],
            texture_coordinates: [
                world.dot(s) / TEXTURE_SCALE / size[0],
                world.dot(t) / TEXTURE_SCALE / size[1],
            ],
            lightmap_coordinates: [0.0; 2],
        }
    }));
    // Wound as the map's drawn faces are (clockwise seen from the front), so a shader
    // culling back faces shows it on the side its normal points to.
    let corner = |i: usize| Vec3::from_array(polygon[i]);
    let reversed = (corner(1) - corner(0))
        .cross(corner(2) - corner(0))
        .dot(Vec3::from_array(normal))
        > 0.0;
    for corner in 2..polygon.len() as u32 {
        let (b, c) = if reversed {
            (corner, corner - 1)
        } else {
            (corner - 1, corner)
        };
        scene
            .indices
            .extend([vertex_base, vertex_base + b, vertex_base + c]);
    }
    let material = scene.materials.len();
    scene.materials.push(ViewerMaterial {
        shader: shader.to_owned(),
        lightmap: -1,
    });
    Ok(vec![ActorDraw {
        indices: index_start..scene.indices.len().try_into()?,
        material,
    }])
}

/// Radiant's texture axes for a face with `normal` (`TextureAxisFromPlane`): the world
/// axis nearest the normal picks the pair the texture is projected along.
fn texture_axes(normal: [f32; 3]) -> (Vec3, Vec3) {
    const BASE: [[[f32; 3]; 3]; 6] = [
        [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0]],
        [[0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0]],
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]],
        [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]],
        [[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]],
        [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]],
    ];
    let normal = Vec3::from_array(normal);
    let mut best = (0, 0.0);
    for (index, [axis, _, _]) in BASE.iter().enumerate() {
        let dot = normal.dot(Vec3::from_array(*axis));
        if dot > best.1 {
            best = (index, dot);
        }
    }
    let [_, s, t] = BASE[best.0];
    (Vec3::from_array(s), Vec3::from_array(t))
}

fn area(points: &[[f32; 3]]) -> f32 {
    let Some(first) = points.first().map(|p| Vec3::from_array(*p)) else {
        return 0.0;
    };
    let mut sum = Vec3::ZERO;
    for pair in points[1..].windows(2) {
        sum += (Vec3::from_array(pair[0]) - first).cross(Vec3::from_array(pair[1]) - first);
    }
    sum.length() * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_take_radiants_axial_projection() {
        let (s, t) = texture_axes([0.0, 0.0, 1.0]);
        assert_eq!((s, t), (Vec3::X, Vec3::NEG_Y));
        let (s, t) = texture_axes([0.0, -1.0, 0.0]);
        assert_eq!((s, t), (Vec3::X, Vec3::NEG_Z));
        // A slope nearer a wall than the floor takes the wall's axes.
        let (s, t) = texture_axes([0.9, 0.0, 0.43]);
        assert_eq!((s, t), (Vec3::Y, Vec3::NEG_Z));
    }

    #[test]
    fn a_face_is_lit_from_its_middle_and_tiles_at_half_scale() {
        let square = [
            [0.0, 0.0, 0.0],
            [128.0, 0.0, 0.0],
            [128.0, 128.0, 0.0],
            [0.0, 128.0, 0.0],
        ];
        let middle = centre(&square);
        assert_eq!(middle, [64.0, 64.0, 0.0]);
        let mut scene = FlattenedScene::default();
        let draws = append(
            &mut scene,
            &square,
            [0.0, 0.0, 1.0],
            middle,
            "textures/a/wall",
            Some([256, 256]),
        )
        .unwrap();
        assert_eq!(draws.len(), 1);
        // Counter-clockwise seen from above: turned over to the map's winding.
        assert_eq!(scene.indices, [0, 2, 1, 0, 3, 2]);
        assert_eq!(scene.vertices[0].position, [-64.0, -64.0, 0.0]);
        // 128 units of a 256 texel texture at scale 0.5: exactly one tile.
        assert_eq!(scene.vertices[1].texture_coordinates, [1.0, 0.0]);
        assert_eq!(scene.vertices[3].texture_coordinates, [0.0, -1.0]);
        assert_eq!(scene.materials[0].shader, "textures/a/wall");
        assert_eq!(area(&square), 128.0 * 128.0);
    }
}
