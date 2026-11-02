//! The blade core's thickness (`cg_saberCoreSize`): the white line down the middle of a
//! lit blade drawn thinner or thicker, its glow unchanged.
//!
//! Retail draws the core at a third of the blade's radius (`CG_DoSaber`,
//! `cg_players.c:5359-5469`; [`crate::saber::Instance::pair_flickering`]). This setting
//! multiplies that radius, flicker included, for every blade drawn in the world, held or
//! thrown. 1 is retail's line.
//!
//! JoF EJK's nearest is `cg_shaderSaberCore` (`cg_xcvar.h`, not archived, 0.625), which
//! scales the core of its SFX blades only (`CG_DoSFXSaber`, `cg_players.c:7288`) on top of
//! a pulse; SJK draws no SFX blades, so it keeps a saved setting of its own for the
//! retail blade rather than giving EternalJK's name another meaning.

/// The setting: the core's radius times this, [`MIN`] to [`MAX`], 1 by default.
pub(crate) const CVAR: &str = "cg_saberCoreSize";
/// The thinnest core: half retail's.
pub(crate) const MIN: f32 = 0.5;
/// The thickest core: twice retail's, still within the glow (a third of the radius
/// twice over is two thirds of it).
pub(crate) const MAX: f32 = 2.0;

/// The setting's value as a scale of the core's radius: 1 without a console or an
/// unreadable value, otherwise clamped to [`MIN`]..[`MAX`].
pub(crate) fn read(console: Option<&crate::console::ViewerConsole>) -> f32 {
    scale(console.and_then(|console| console.float_cvar(CVAR)))
}

/// [`read`]'s rule for a value: 1 when unset or not a number, otherwise clamped.
pub(crate) fn scale(value: Option<f64>) -> f32 {
    match value {
        Some(value) if value.is_finite() => (value as f32).clamp(MIN, MAX),
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_or_nonsense_is_retail() {
        assert_eq!(scale(None), 1.0);
        assert_eq!(scale(Some(f64::NAN)), 1.0);
        assert_eq!(scale(Some(f64::INFINITY)), 1.0);
    }

    #[test]
    fn values_are_kept_within_the_slider() {
        assert_eq!(scale(Some(1.5)), 1.5);
        assert_eq!(scale(Some(0.0)), MIN);
        assert_eq!(scale(Some(-3.0)), MIN);
        assert_eq!(scale(Some(9.0)), MAX);
    }
}
