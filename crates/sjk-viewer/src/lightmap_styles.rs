//! Light styles folded into a map's lightmaps as it loads.
//!
//! Every switchable or animated light q3map2 baked into a style of its own is added
//! over the surfaces' own lightmaps at full strength, as the cgame shows a style the
//! server gave no pattern (`sjk_bsp::Bsp::compose_light_styles`). Without it a map lit
//! mostly by such lights, like JoF Shuttle with its `mainlights`, `left` and `right`
//! groups, drew almost black. While a game runs, the world's renderer follows the
//! server's `CS_LIGHT_STYLES` (`world_light_styles.rs`).
use sjk_bsp::{Bsp, WHITE_STYLES};

/// Fold `bsp`'s light styles into its lightmaps and vertex colours; under
/// `r_vertexLight 1` every surface is vertex lit first (`crate::vertex_light`).
pub(crate) fn compose(bsp: &mut Bsp) {
    if crate::vertex_light::enabled() {
        bsp.light_by_vertex();
        crate::log::progress(format_args!("r_vertexLight 1: world lit by vertex colours"));
    }
    let Some(layers) = bsp.compose_light_styles(&WHITE_STYLES) else {
        return;
    };
    let unmatched = layers.unmatched_styles();
    crate::log::progress(format_args!(
        "light styles: {} surfaces drawn from {} composed blocks ({} texels){}",
        layers.styled_surfaces(),
        layers.blocks().len(),
        layers.texel_count(),
        if unmatched == 0 {
            String::new()
        } else {
            format!("; {unmatched} surface styles left out (blocks do not line up)")
        }
    ));
}
