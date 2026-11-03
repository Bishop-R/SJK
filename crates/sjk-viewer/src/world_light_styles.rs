//! Light styles on the world's lightmap pages while the map runs.
//!
//! The map loads with every styled surface drawn from a composed block of its own, its
//! styles at full strength (`sjk_bsp::Bsp::compose_light_styles`,
//! [`crate::lightmap_styles`]). The server then says what each style is
//! (`CS_LIGHT_STYLES`): a switchable light that starts off is `"a"`, one switched on
//! again is `"z"` or its pattern, and patterns such as flicker step every 50 ms
//! (`CG_RunLightStyles`, `cg_light.c:52-79`). Each frame the colours are worked out
//! again and, when one a block uses changed, that block is composed again and uploaded
//! over its rectangle. Vertex-lit surfaces keep the light they loaded with.

use sjk_bsp::{Bsp, LightStyleLayers, StyleColors, WHITE_STYLES};
use sjk_game_jka::worldspawn::CS_LIGHT_STYLES;
use sjk_protocol::GameState;
use std::collections::HashMap;

/// The composed blocks of the loaded world and the colours they show.
pub(crate) struct Live {
    layers: LightStyleLayers,
    /// The uploaded page of each block, in block order.
    views: Vec<Option<wgpu::TextureView>>,
    applied: StyleColors,
    /// Reused RGBA for an upload.
    rgba: Vec<u8>,
}

impl Live {
    /// The uploaded pages the composed blocks are on; `None` for a map without styles.
    pub(super) fn new(bsp: &Bsp, lightmaps: &HashMap<i32, wgpu::TextureView>) -> Option<Self> {
        let layers = bsp.render().light_style_layers()?.clone();
        let views = layers
            .blocks()
            .iter()
            .map(|block| lightmaps.get(&i32::try_from(block.page).ok()?).cloned())
            .collect();
        Some(Self {
            layers,
            views,
            applied: WHITE_STYLES,
            rgba: Vec::new(),
        })
    }

    /// Show `colors`, uploading again only the blocks a changed colour lights.
    fn show(&mut self, queue: &crate::frame_queue::FrameQueue, colors: &StyleColors) {
        if self.layers.same_light(&self.applied, colors) {
            return;
        }
        let changed: Vec<usize> = (0..sjk_bsp::MAX_LIGHT_STYLES)
            .filter(|&style| self.applied[style] != colors[style])
            .collect();
        self.applied = *colors;
        for (block, view) in self.layers.blocks().iter().zip(&self.views) {
            let Some(view) = view else {
                continue;
            };
            if !changed.iter().any(|&style| block.uses(style)) {
                continue;
            }
            let rgb = block.compose(colors);
            self.rgba.clear();
            self.rgba.extend(
                rgb.as_chunks::<3>()
                    .0
                    .iter()
                    .flat_map(|&[r, g, b]| [r, g, b, 255]),
            );
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: view.texture(),
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: block.origin[0],
                        y: block.origin[1],
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &self.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(block.size[0] * 4),
                    rows_per_image: Some(block.size[1]),
                },
                wgpu::Extent3d {
                    width: block.size[0],
                    height: block.size[1],
                    depth_or_array_layers: 1,
                },
            );
        }
    }
}

/// The style colours `game` gives at `time` (ms).
fn colors(game: &GameState, time: i32) -> StyleColors {
    sjk_bsp::style_colors(
        |style, channel| {
            game.config_string(CS_LIGHT_STYLES + style * 3 + channel)
                .unwrap_or_default()
        },
        time,
    )
}

impl super::Runtime {
    /// Light the composed lightmap blocks as `game` says at `time` (ms).
    pub(crate) fn update_light_styles(
        &mut self,
        queue: &crate::frame_queue::FrameQueue,
        game: &GameState,
        time: i32,
    ) {
        if let Some(live) = self.light_styles.as_mut() {
            live.show(queue, &colors(game, time));
        }
    }
}
