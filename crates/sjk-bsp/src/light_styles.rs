//! Light styles of a map's baked light: what rd-vanilla adds over a surface's own
//! lightmap for every switchable or animated light that shines on it.
//!
//! q3map2 bakes each light that has a `style` into a lightmap of its own: a surface
//! lists up to four styles (`lightmapStyles`, 0 being the map's plain light) with a
//! lightmap page and coordinates for each. rd-vanilla's `FinishShader`
//! (`tr_shader.cpp:2905-2955`) copies the surface's `$lightmap` stage once per further
//! style, on that style's page, coloured by the style (`CGEN_LIGHTMAPSTYLE`,
//! `tr_shade.cpp:1295-1302`) and added (`GLS_SRCBLEND_ONE | GLS_DSTBLEND_ONE`), so the
//! light the textures are filtered by is `lightmap₀ + Σ colourₖ × lightmapₖ`. A
//! vertex-lit surface sums its vertex colours the same way (`ComputeFinalVertexColor`,
//! `tr_surface.cpp:273-310`).
//!
//! SJK's world draws one lightmap per surface, so [`crate::Bsp::compose_light_styles`]
//! gives every styled surface a lightmap block of its own holding that sum, on pages
//! added after the file's. q3map2 gives every style's block the size of the surface's
//! own, so the vertices give the whole-texel offset from one to the other. The file's
//! pages are never written: q3map2 shares one block between surfaces whose light is
//! alike (often a block black in style 0), so a sum written there would light the
//! others too. Surfaces in one q3map2 block share the new block as well.
//!
//! The style colours are the cgame's (`CG_RunLightStyles`, `cg_light.c:52-79`): a
//! style with no `CS_LIGHT_STYLES` pattern is white, which is every switchable style
//! (32 and up) of a map whose lights the server does not spawn.

use crate::render_data::{DrawVertex, Lightmap, Surface};
use std::collections::HashMap;

/// `MAX_LIGHT_STYLES` (`q_shared.h:249`).
pub const MAX_LIGHT_STYLES: usize = 64;
/// `LS_UNUSED`: a style slot no light uses.
const STYLE_UNUSED: u8 = 255;
/// How far (in texels) a style's block may sit from a whole-texel offset of the
/// surface's own block before that style is left out.
const OFFSET_TOLERANCE: f32 = 0.05;
/// Side of the pages the composed blocks are packed into.
const PAGE_SIDE: u32 = 512;
/// Texels repeated round each composed block, so filtering never reads a neighbour.
const BORDER: u32 = 1;

/// The RGB of every light style, as `R_SetLightStyle` receives them.
pub type StyleColors = [[u8; 3]; MAX_LIGHT_STYLES];

/// Every style at full strength: what the cgame gives styles without a pattern.
pub const WHITE_STYLES: StyleColors = [[255; 3]; MAX_LIGHT_STYLES];

/// A rectangle of texels on a page, inclusive of `first` and exclusive of `end`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct Rect {
    first: [i64; 2],
    end: [i64; 2],
}

impl Rect {
    fn size(&self) -> [u32; 2] {
        [
            (self.end[0] - self.first[0]) as u32,
            (self.end[1] - self.first[1]) as u32,
        ]
    }

    fn union(&self, other: &Rect) -> Rect {
        Rect {
            first: [
                self.first[0].min(other.first[0]),
                self.first[1].min(other.first[1]),
            ],
            end: [self.end[0].max(other.end[0]), self.end[1].max(other.end[1])],
        }
    }
}

/// One composed block: where it is drawn from and what it adds.
#[derive(Clone, Debug, PartialEq)]
pub struct StyledBlock {
    /// The added page and the corner of the block on it, its border included.
    pub page: usize,
    pub origin: [u32; 2],
    /// Width and height, its border included.
    pub size: [u32; 2],
    /// The surface's own light, border included, RGB.
    base: Box<[u8]>,
    /// Each style and its block's light, laid out as `base`.
    styles: Vec<(u8, Box<[u8]>)>,
}

impl StyledBlock {
    /// The block lit by `colors`, RGB rows of `size[0]` texels.
    pub fn compose(&self, colors: &StyleColors) -> Box<[u8]> {
        let mut sum: Vec<u32> = self.base.iter().map(|&value| u32::from(value)).collect();
        for (style, light) in &self.styles {
            let color = colors[usize::from(*style)];
            for (at, &texel) in light.iter().enumerate() {
                // The stage's texel times the style colour, added in the framebuffer.
                sum[at] += (u32::from(texel) * u32::from(color[at % 3]) + 127) / 255;
            }
        }
        sum.into_iter().map(|value| value.min(255) as u8).collect()
    }

    /// Whether `style` lights this block.
    pub fn uses(&self, style: usize) -> bool {
        self.styles
            .iter()
            .any(|(used, _)| usize::from(*used) == style)
    }
}

/// The blocks a map's styled light is composed into.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LightStyleLayers {
    blocks: Vec<StyledBlock>,
    /// The styles any block reads.
    used: u64,
    /// Styles of surfaces left out because their blocks did not line up.
    unmatched_styles: usize,
    styled_surfaces: usize,
}

impl LightStyleLayers {
    /// The composed blocks.
    pub fn blocks(&self) -> &[StyledBlock] {
        &self.blocks
    }

    /// Whether `style` lights any block.
    pub fn uses(&self, style: usize) -> bool {
        style < MAX_LIGHT_STYLES && self.used & (1 << style) != 0
    }

    /// Surfaces now drawn from a composed block.
    pub fn styled_surfaces(&self) -> usize {
        self.styled_surfaces
    }

    /// Texels in the composed blocks, borders included.
    pub fn texel_count(&self) -> usize {
        self.blocks
            .iter()
            .map(|block| (block.size[0] * block.size[1]) as usize)
            .sum()
    }

    /// Surface styles left out because their block did not line up with the surface's.
    pub fn unmatched_styles(&self) -> usize {
        self.unmatched_styles
    }

    /// Whether two colour sets light these blocks alike.
    pub fn same_light(&self, a: &StyleColors, b: &StyleColors) -> bool {
        (0..MAX_LIGHT_STYLES).all(|style| !self.uses(style) || a[style] == b[style])
    }

    /// Give every styled surface a composed block lit by `colors`, on pages appended
    /// to `lightmaps`, and point its first lightmap at it. `None` when no surface has
    /// a further style that lines up.
    pub(crate) fn build(
        surfaces: &mut [Surface],
        vertices: &mut [DrawVertex],
        lightmaps: &mut Vec<Option<Lightmap>>,
        colors: &StyleColors,
    ) -> Option<Self> {
        let mut layers = Self::default();
        // Surfaces that read the same light (one q3map2 block) share one group.
        type Key = (usize, Vec<(u8, usize, [i64; 2])>);
        let mut groups: HashMap<Key, (usize, Rect)> = HashMap::new();
        let mut order: Vec<Key> = Vec::new();
        let mut members: Vec<(usize, usize)> = Vec::new();
        for (surface_index, surface) in surfaces.iter().enumerate() {
            let Ok(own) = usize::try_from(surface.lightmaps[0]) else {
                continue;
            };
            let Some(own_page) = lightmaps.get(own).and_then(Option::as_ref) else {
                continue;
            };
            let Some(points) = vertices.get(surface.vertices.clone()) else {
                continue;
            };
            let (width, height) = (own_page.width, own_page.height);
            let Some(rect) = footprint(points, width, height) else {
                continue;
            };
            let mut styles = Vec::new();
            for slot in 1..4 {
                let style = surface.lightmap_styles[slot];
                // rd-vanilla stops at the first unused slot.
                if style == STYLE_UNUSED {
                    break;
                }
                let lined_up = usize::try_from(surface.lightmaps[slot])
                    .ok()
                    .filter(|_| usize::from(style) < MAX_LIGHT_STYLES)
                    .filter(|&page| {
                        lightmaps
                            .get(page)
                            .and_then(Option::as_ref)
                            .is_some_and(|other| other.width == width && other.height == height)
                    })
                    .and_then(|page| {
                        let offset = block_offset(points, slot, width, height)?;
                        let inside = (0..2).all(|axis| {
                            let side = i64::from([width, height][axis]);
                            rect.first[axis] + offset[axis] >= 0
                                && rect.end[axis] + offset[axis] <= side
                        });
                        inside.then_some((style, page, offset))
                    });
                match lined_up {
                    Some(entry) => styles.push(entry),
                    None => layers.unmatched_styles += 1,
                }
            }
            if styles.is_empty() {
                continue;
            }
            let key = (own, styles);
            let group = match groups.get_mut(&key) {
                Some((group, area)) => {
                    *area = area.union(&rect);
                    *group
                }
                None => {
                    let group = order.len();
                    groups.insert(key.clone(), (group, rect));
                    order.push(key);
                    group
                }
            };
            members.push((surface_index, group));
        }
        if order.is_empty() {
            return None;
        }

        // Pack the groups' blocks, tallest first, into shelves on new pages.
        let areas: Vec<Rect> = order.iter().map(|key| groups[key].1).collect();
        let mut by_height: Vec<usize> = (0..order.len()).collect();
        by_height.sort_by_key(|&group| std::cmp::Reverse(areas[group].size()[1]));
        let mut placed = vec![(0_usize, [0_u32; 2]); order.len()];
        let mut pages: Vec<Lightmap> = Vec::new();
        let mut shelf = [0_u32; 3]; // x, y, shelf height on the last page
        for &group in &by_height {
            let [w, h] = areas[group].size().map(|side| side + 2 * BORDER);
            let fits = |shelf: &[u32; 3], page: &Lightmap| {
                if shelf[0] + w <= page.width && shelf[1] + h <= page.height {
                    Some([shelf[0], shelf[1]])
                } else if shelf[1] + shelf[2] + h <= page.height && w <= page.width {
                    Some([0, shelf[1] + shelf[2]])
                } else {
                    None
                }
            };
            let spot = pages.last().and_then(|page| fits(&shelf, page));
            let at = match spot {
                Some(at) => {
                    if at[1] != shelf[1] {
                        shelf = [0, at[1], 0];
                    }
                    at
                }
                None => {
                    let (pw, ph) = (PAGE_SIDE.max(w), PAGE_SIDE.max(h));
                    pages.push(Lightmap {
                        width: pw,
                        height: ph,
                        rgb: vec![0; (pw * ph * 3) as usize].into_boxed_slice(),
                    });
                    shelf = [0, 0, 0];
                    [0, 0]
                }
            };
            shelf[0] = at[0] + w;
            shelf[2] = shelf[2].max(h);
            placed[group] = (pages.len() - 1, at);
        }

        // Read each block's light out of the file's pages and compose it.
        let first_new_page = lightmaps.len();
        for (group, key) in order.iter().enumerate() {
            let (own, styles) = key;
            let rect = areas[group];
            let read = |page: usize, offset: [i64; 2]| {
                let lightmap = lightmaps[page].as_ref().expect("checked above");
                block(lightmap, rect, offset)
            };
            let (page, origin) = placed[group];
            let block = StyledBlock {
                page: first_new_page + page,
                origin,
                size: rect.size().map(|side| side + 2 * BORDER),
                base: read(*own, [0, 0]),
                styles: styles
                    .iter()
                    .map(|&(style, page, offset)| (style, read(page, offset)))
                    .collect(),
            };
            write_block(&mut pages[page], &block, &block.compose(colors));
            for &(style, _, _) in styles {
                layers.used |= 1 << style;
            }
            layers.blocks.push(block);
        }

        // Point each member surface at its group's block.
        for &(surface_index, group) in &members {
            let surface = &mut surfaces[surface_index];
            let own = usize::try_from(surface.lightmaps[0]).expect("lightmapped");
            let lightmap = lightmaps[own].as_ref().expect("checked above");
            let (old, block) = ([lightmap.width, lightmap.height], &layers.blocks[group]);
            let new = [pages[placed[group].0].width, pages[placed[group].0].height];
            let rect = areas[group];
            for point in &mut vertices[surface.vertices.clone()] {
                for axis in 0..2 {
                    let texels = point.lightmap_coordinates[0][axis] * old[axis] as f32;
                    let moved =
                        texels - rect.first[axis] as f32 + (block.origin[axis] + BORDER) as f32;
                    point.lightmap_coordinates[0][axis] = moved / new[axis] as f32;
                }
            }
            surface.lightmaps[0] = block.page as i32;
            layers.styled_surfaces += 1;
        }
        lightmaps.extend(pages.into_iter().map(Some));
        Some(layers)
    }
}

/// The texels of `rect` moved by `offset` on `page`, with a border repeating the
/// edge, RGB.
fn block(page: &Lightmap, rect: Rect, offset: [i64; 2]) -> Box<[u8]> {
    let [w, h] = rect.size().map(|side| side + 2 * BORDER);
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for row in 0..h {
        for column in 0..w {
            let x = (rect.first[0] + i64::from(column) - i64::from(BORDER))
                .clamp(rect.first[0], rect.end[0] - 1)
                + offset[0];
            let y = (rect.first[1] + i64::from(row) - i64::from(BORDER))
                .clamp(rect.first[1], rect.end[1] - 1)
                + offset[1];
            let at = ((y * i64::from(page.width) + x) * 3) as usize;
            rgb.extend_from_slice(&page.rgb[at..at + 3]);
        }
    }
    rgb.into_boxed_slice()
}

/// Write a composed block into its page.
fn write_block(page: &mut Lightmap, block: &StyledBlock, rgb: &[u8]) {
    let row_bytes = (block.size[0] * 3) as usize;
    for row in 0..block.size[1] {
        let to = (((block.origin[1] + row) * page.width + block.origin[0]) * 3) as usize;
        let from = row as usize * row_bytes;
        page.rgb[to..to + row_bytes].copy_from_slice(&rgb[from..from + row_bytes]);
    }
}

/// The whole-texel offset from a surface's own block to the block of style `slot`,
/// when every vertex agrees on it.
fn block_offset(points: &[DrawVertex], slot: usize, width: u32, height: u32) -> Option<[i64; 2]> {
    let size = [width as f32, height as f32];
    let mut offset = None;
    for point in points {
        let own = point.lightmap_coordinates[0];
        let styled = point.lightmap_coordinates[slot];
        let mut whole = [0_i64; 2];
        for axis in 0..2 {
            let texels = (styled[axis] - own[axis]) * size[axis];
            if !texels.is_finite() || (texels - texels.round()).abs() > OFFSET_TOLERANCE {
                return None;
            }
            whole[axis] = texels.round() as i64;
        }
        match offset {
            None => offset = Some(whole),
            Some(first) if first != whole => return None,
            Some(_) => {}
        }
    }
    offset
}

/// The texels a bilinear lookup of the surface's own block can read: from the texel
/// under the lowest coordinate to the one over the highest. q3map2 puts a block's edge
/// vertices on its edge texels' centres, which read no further texel.
fn footprint(points: &[DrawVertex], width: u32, height: u32) -> Option<Rect> {
    let mut low = [f32::INFINITY; 2];
    let mut high = [f32::NEG_INFINITY; 2];
    let size = [width as f32, height as f32];
    for point in points {
        for axis in 0..2 {
            let texel = point.lightmap_coordinates[0][axis] * size[axis] - 0.5;
            if !texel.is_finite() {
                return None;
            }
            low[axis] = low[axis].min(texel);
            high[axis] = high[axis].max(texel);
        }
    }
    let mut rect = Rect {
        first: [0; 2],
        end: [0; 2],
    };
    for axis in 0..2 {
        if low[axis] > high[axis] {
            return None;
        }
        let side = i64::from([width, height][axis]);
        // A whisker under a centre is still that texel.
        rect.first[axis] = ((low[axis] + 1e-3).floor() as i64).max(0);
        rect.end[axis] = ((high[axis] - 1e-3).ceil() as i64).min(side - 1) + 1;
        if rect.first[axis] >= rect.end[axis] {
            return None;
        }
    }
    Some(rect)
}

/// `ComputeFinalVertexColor` for a vertex-lit surface: each style's colour times the
/// style colour, summed (`>> 8`) and clamped. `None` when the surface has no further
/// style, as its colours then stay as the file gave them.
pub(crate) fn vertex_color(
    colors: &[[u8; 4]; 4],
    styles: [u8; 4],
    style_colors: &StyleColors,
) -> Option<[u8; 4]> {
    if styles[1] == STYLE_UNUSED {
        return None;
    }
    let mut sum = [0_u32; 3];
    for (slot, &style) in styles.iter().enumerate() {
        if style == STYLE_UNUSED {
            break;
        }
        let Some(color) = style_colors.get(usize::from(style)) else {
            continue;
        };
        for channel in 0..3 {
            sum[channel] += u32::from(colors[slot][channel]) * u32::from(color[channel]);
        }
    }
    Some([
        (sum[0] >> 8).min(255) as u8,
        (sum[1] >> 8).min(255) as u8,
        (sum[2] >> 8).min(255) as u8,
        colors[0][3],
    ])
}

/// `CG_SetLightstyle` and `CG_RunLightStyles` (`cg_light.c:52-105`): the colour of
/// every style at `time` (ms) from its three `CS_LIGHT_STYLES` strings, `'a'` dark to
/// `'z'` full, one letter per 50 ms. `pattern(style, channel)` gives the string.
pub fn style_colors<'a>(pattern: impl Fn(usize, usize) -> &'a [u8], time: i32) -> StyleColors {
    let step = (time.max(0) / 50) as usize;
    let mut colors = WHITE_STYLES;
    for (style, color) in colors.iter_mut().enumerate() {
        let strings = [pattern(style, 0), pattern(style, 1), pattern(style, 2)];
        // A style's length is the last of its strings `CG_SetLightstyle` read.
        let length = strings[2].len();
        if length == 0 {
            continue;
        }
        let at = if length == 1 { 0 } else { step % length };
        for (channel, value) in color.iter_mut().enumerate() {
            *value = strings[channel]
                .get(at)
                .map_or(0, |&letter| letter_value(letter));
        }
    }
    colors
}

/// `(s[k] - 'a') / ('z' - 'a') * 255`, as the cgame stores it in a byte.
fn letter_value(letter: u8) -> u8 {
    let value = (f32::from(letter) - f32::from(b'a')) / 25.0 * 255.0;
    value.clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_data::SurfaceKind;

    const SIDE: u32 = 8;

    fn vertex(own: [f32; 2], styled: [f32; 2]) -> DrawVertex {
        DrawVertex {
            position: [0.0; 3],
            texture_coordinates: [0.0; 2],
            lightmap_coordinates: [own, styled, [0.0; 2], [0.0; 2]],
            normal: [0.0, 0.0, 1.0],
            colors: [[0; 4]; 4],
        }
    }

    fn surface(vertices: std::ops::Range<usize>, styles: [u8; 4], pages: [i32; 4]) -> Surface {
        Surface {
            shader: 0,
            fog: None,
            kind: SurfaceKind::Planar,
            vertices,
            indices: 0..0,
            lightmap_styles: styles,
            vertex_styles: [0, 255, 255, 255],
            lightmaps: pages,
            lightmap_rectangles: [[0; 4]; 4],
            lightmap_origin: [0.0; 3],
            lightmap_vectors: [[0.0; 3]; 3],
            patch_dimensions: None,
        }
    }

    fn page(fill: impl Fn(u32, u32) -> u8) -> Lightmap {
        let mut rgb = Vec::new();
        for y in 0..SIDE {
            for x in 0..SIDE {
                rgb.extend_from_slice(&[fill(x, y); 3]);
            }
        }
        Lightmap {
            width: SIDE,
            height: SIDE,
            rgb: rgb.into_boxed_slice(),
        }
    }

    fn at(x: f32, y: f32) -> [f32; 2] {
        [(x + 0.5) / SIDE as f32, (y + 0.5) / SIDE as f32]
    }

    /// The light a surface's first lightmap shows at texel-space `uv` of its old page.
    fn shown(lightmaps: &[Option<Lightmap>], surface: &Surface, point: &DrawVertex) -> u8 {
        let page = lightmaps[surface.lightmaps[0] as usize].as_ref().unwrap();
        let [u, v] = point.lightmap_coordinates[0];
        let x = (u * page.width as f32 - 0.5).round() as u32;
        let y = (v * page.height as f32 - 0.5).round() as u32;
        page.rgb[((y * page.width + x) * 3) as usize]
    }

    /// A wall whose own block is texels 0..=1 of page 0, dark, and whose style-32
    /// block sits four texels to the right on page 1. A plain floor shares the dark
    /// block, as q3map2 shares blocks alike.
    fn shuttle_wall() -> (Vec<Surface>, Vec<DrawVertex>, Vec<Option<Lightmap>>) {
        let shifted = |x: f32, y: f32| at(x + 4.0, y);
        let vertices = vec![
            vertex(at(0.0, 0.0), shifted(0.0, 0.0)),
            vertex(at(1.0, 0.0), shifted(1.0, 0.0)),
            vertex(at(1.0, 1.0), shifted(1.0, 1.0)),
            vertex(at(0.0, 0.0), [0.0; 2]),
            vertex(at(1.0, 1.0), [0.0; 2]),
        ];
        let surfaces = vec![
            surface(0..3, [0, 32, 255, 255], [0, 1, -3, -3]),
            surface(3..5, [0, 255, 255, 255], [0, -3, -3, -3]),
        ];
        let own = page(|_, _| 6);
        let styled = page(|x, _| if (4..6).contains(&x) { 160 } else { 99 });
        (surfaces, vertices, vec![Some(own), Some(styled)])
    }

    fn build(
        world: &mut (Vec<Surface>, Vec<DrawVertex>, Vec<Option<Lightmap>>),
        colors: &StyleColors,
    ) -> Option<LightStyleLayers> {
        LightStyleLayers::build(&mut world.0, &mut world.1, &mut world.2, colors)
    }

    #[test]
    fn a_styled_surface_gets_a_block_of_its_own_with_the_style_added() {
        let mut world = shuttle_wall();
        let layers = build(&mut world, &WHITE_STYLES).expect("styled");
        let (surfaces, vertices, lightmaps) = &world;
        assert!(layers.uses(32));
        assert_eq!(layers.styled_surfaces(), 1);
        assert_eq!(
            surfaces[0].lightmaps[0], 2,
            "a new page after the file's two"
        );
        for point in &vertices[0..3] {
            assert_eq!(shown(lightmaps, &surfaces[0], point), 166);
        }
        // The file's pages, and the plain floor sharing the dark block, are untouched.
        assert_eq!(surfaces[1].lightmaps[0], 0);
        assert_eq!(shown(lightmaps, &surfaces[1], &vertices[3]), 6);
        assert_eq!(lightmaps[0], shuttle_wall().2[0]);
    }

    #[test]
    fn a_style_switched_off_adds_nothing() {
        let mut world = shuttle_wall();
        let layers = build(&mut world, &WHITE_STYLES).expect("styled");
        let mut off = WHITE_STYLES;
        off[32] = [0; 3];
        let block = &layers.blocks()[0];
        assert!(block.compose(&off).iter().all(|&value| value == 6));
        assert!(!layers.same_light(&WHITE_STYLES, &off));
        let mut other = WHITE_STYLES;
        other[5] = [0; 3];
        assert!(
            layers.same_light(&WHITE_STYLES, &other),
            "style 5 lights nothing"
        );
    }

    #[test]
    fn blocks_that_do_not_line_up_are_left_alone() {
        let mut world = shuttle_wall();
        world.1[2].lightmap_coordinates[1][0] += 0.3 / SIDE as f32;
        assert!(build(&mut world, &WHITE_STYLES).is_none());
        assert_eq!(world.0[0].lightmaps[0], 0);
        assert_eq!(world.2.len(), 2);
    }

    #[test]
    fn a_surface_without_further_styles_builds_nothing() {
        let mut world = shuttle_wall();
        world.0[0].lightmap_styles = [0, 255, 255, 255];
        assert!(build(&mut world, &WHITE_STYLES).is_none());
    }

    #[test]
    fn surfaces_of_one_block_share_their_new_block() {
        let mut world = shuttle_wall();
        // A second wall in the same q3map2 block: the same pages and offset.
        let shifted = |x: f32, y: f32| at(x + 4.0, y);
        world.1.push(vertex(at(1.0, 0.0), shifted(1.0, 0.0)));
        world.1.push(vertex(at(1.0, 1.0), shifted(1.0, 1.0)));
        world
            .0
            .push(surface(5..7, [0, 32, 255, 255], [0, 1, -3, -3]));
        let layers = build(&mut world, &WHITE_STYLES).expect("styled");
        assert_eq!(layers.blocks().len(), 1);
        assert_eq!(world.0[2].lightmaps[0], world.0[0].lightmaps[0]);
        assert_eq!(shown(&world.2, &world.0[2], &world.1[5]), 166);
    }

    #[test]
    fn vertex_light_sums_its_styles() {
        let colors = [[10, 20, 30, 255], [200, 200, 200, 255], [0; 4], [0; 4]];
        let mut style_colors = WHITE_STYLES;
        assert_eq!(
            vertex_color(&colors, [0, 33, 255, 255], &style_colors),
            Some([209, 219, 229, 255])
        );
        style_colors[33] = [0; 3];
        assert_eq!(
            vertex_color(&colors, [0, 33, 255, 255], &style_colors),
            Some([9, 19, 29, 255])
        );
        assert_eq!(
            vertex_color(&colors, [0, 255, 255, 255], &style_colors),
            None
        );
    }

    #[test]
    fn style_patterns_follow_the_cgame() {
        let patterns = |style: usize, _channel: usize| -> &'static [u8] {
            match style {
                33 => b"a",
                1 => b"az",
                _ => b"",
            }
        };
        let colors = style_colors(patterns, 0);
        assert_eq!(colors[32], [255; 3], "no pattern is white");
        assert_eq!(colors[33], [0; 3], "a switched-off light");
        assert_eq!(colors[1], [0; 3]);
        assert_eq!(
            style_colors(patterns, 50)[1],
            [255; 3],
            "one letter per 50 ms"
        );
    }
}
