//! `r_vertexLight`: rd-vanilla's vertex lighting mode, the world lit by its vertex
//! colours instead of its lightmaps.
//!
//! As in rd-vanilla the cvar is read when a map loads (`CVAR_LATCH`): every surface then
//! takes `LIGHTMAP_BY_VERTEX` and its vertex styles (`sjk_bsp::Bsp::light_by_vertex`),
//! the worldspawn's `vertexremapshader` keys apply (`world_map_remaps.rs`), and every
//! shader's first `$lightmap` stage becomes vertex colour by `FinishShader`'s rule for
//! such surfaces (`world_stage::material_stages`). Dynamic lights stay as they are.
use sjk_shell::{CvarDefinition, CvarError, CvarFlags, CvarRegistry, CvarValue};
use std::sync::atomic::{AtomicBool, Ordering};

/// The cvar's name.
pub(crate) const CVAR: &str = "r_vertexLight";

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Whether the next map loads vertex lit.
pub(crate) fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

fn set(value: &CvarValue) {
    let on = match value {
        CvarValue::Integer(value) => *value != 0,
        CvarValue::Bool(on) => *on,
        CvarValue::Float(value) => *value != 0.,
        CvarValue::Text(_) => false,
    };
    ENABLED.store(on, Ordering::Relaxed);
}

/// Register `r_vertexLight` (archived, as EternalJK's) and follow it.
pub(crate) fn bind(cvars: &mut CvarRegistry) -> Result<(), CvarError> {
    cvars.register(CvarDefinition::new(
        CVAR,
        0_i64,
        CvarFlags::ARCHIVE | CvarFlags::OMIT_DEFAULT,
        "Light the world by its vertex colours instead of lightmaps, as the original \
         game's vertex lighting mode; applies at the next map load",
    ))?;
    set(&cvars.get(CVAR).unwrap().value);
    cvars.on_change(CVAR, |change| set(&change.current))?;
    Ok(())
}
