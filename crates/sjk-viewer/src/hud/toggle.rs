//! What the HUD switch hides (Blep's idea, 03/11/2026): with `cg_drawHud 0` (Settings >
//! HUD > HUD, the quick wheel's "HUD on/off", a `toggle cg_drawHud` bind) each part of
//! the screen below hides only while its own switch says so. The defaults are what the
//! HUD switch always hid (the HUD's elements, the crosshair, the names over players), and
//! the chat stays unless the player ticks it. `cg_draw2D 0` still hides everything.

use crate::console::ViewerConsole;

/// [`Part::Elements`]'s switch.
pub(crate) const ELEMENTS_CVAR: &str = "cg_hudHideElements";
/// [`Part::Crosshair`]'s switch.
pub(crate) const CROSSHAIR_CVAR: &str = "cg_hudHideCrosshair";
/// [`Part::Names`]'s switch.
pub(crate) const NAMES_CVAR: &str = "cg_hudHideNames";
/// [`Part::Chat`]'s switch.
pub(crate) const CHAT_CVAR: &str = "cg_hudHideChat";

/// A part of the screen the HUD switch may hide.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Part {
    /// The HUD's elements: health, shield, Force, ammo, the weapon and item rows, the
    /// timers, vote and warmup lines, the kill feed, the lagometer and the rest of the
    /// HUD's layout.
    Elements,
    /// The crosshair and the name under it.
    Crosshair,
    /// The nameplates, the names over players' heads and the team logos and outlines.
    Names,
    /// The chat box.
    Chat,
}

impl Part {
    /// Every part, in the Settings rows' order.
    #[cfg(test)]
    pub(crate) const ALL: [Self; 4] = [Self::Elements, Self::Crosshair, Self::Names, Self::Chat];

    /// Its switch: archived, on while the HUD switch hides the part.
    pub(crate) const fn cvar(self) -> &'static str {
        match self {
            Self::Elements => ELEMENTS_CVAR,
            Self::Crosshair => CROSSHAIR_CVAR,
            Self::Names => NAMES_CVAR,
            Self::Chat => CHAT_CVAR,
        }
    }

    /// Its switch's default: what `cg_drawHud 0` hid before the switches came.
    pub(crate) const fn hidden_by_default(self) -> bool {
        !matches!(self, Self::Chat)
    }
}

/// Whether `part` is hidden this frame by the HUD switch: `cg_drawHud` off and the
/// part's own switch on. `cg_draw2D` is not this switch's: callers check it apart.
pub(crate) fn hides(console: Option<&ViewerConsole>, part: Part) -> bool {
    let Some(console) = console else {
        return false;
    };
    hidden(
        console.bool_cvar("cg_drawHud").unwrap_or(true),
        console
            .bool_cvar(part.cvar())
            .unwrap_or(part.hidden_by_default()),
    )
}

/// The rule of [`hides`]: the HUD switch off (`draw_hud` false) and the part's switch on.
fn hidden(draw_hud: bool, part_switch: bool) -> bool {
    !draw_hud && part_switch
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hud_switch_hides_only_the_parts_ticked() {
        assert!(!hidden(true, true), "HUD on: nothing hides");
        assert!(hidden(false, true));
        assert!(!hidden(false, false));
    }

    /// The defaults keep what `cg_drawHud 0` always did: the chat stays, the rest goes.
    #[test]
    fn the_defaults_are_the_old_hud_switch() {
        let hidden: Vec<Part> = Part::ALL
            .into_iter()
            .filter(|part| part.hidden_by_default())
            .collect();
        assert_eq!(hidden, [Part::Elements, Part::Crosshair, Part::Names]);
        let mut cvars: Vec<&str> = Part::ALL.iter().map(|part| part.cvar()).collect();
        cvars.dedup();
        assert_eq!(cvars.len(), Part::ALL.len());
    }
}
