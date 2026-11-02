//! Optional spatial fragment sampling of the existing map light grid, and toon models
//! (`r_toonModels`, Sol's request, 02/11/2026): banded light with a rim on lit models.

use sjk_shell::{CvarDefinition, CvarError, CvarFlags, CvarRegistry, CvarValue};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

/// Banded, rim-lit model shading; 0, stock's smooth shading, by default.
pub(crate) const TOON_CVAR: &str = "r_toonModels";

/// Retained spatial model diffuse policy (zero restores legacy entity-origin lighting)
/// and the toon mode.
#[derive(Clone, Default)]
pub(crate) struct Settings(Arc<AtomicBool>, Arc<AtomicU32>);

impl Settings {
    /// Register and seed from the actual registry before subscribing.
    pub(crate) fn bind(cvars: &mut CvarRegistry) -> Result<Self, CvarError> {
        // Default on: spatial grid sampling makes lighting vary across a body, which single-point
        // sampling cannot. Measured 0.018 ms with three characters at 2560x1080 on an
        // RX 9060 XT (0.0427 off against 0.0610 on). `r_modelPixelLight 0` opts out.
        cvars.register(CvarDefinition::new("r_modelPixelLight", 1_i64, CvarFlags::ARCHIVE,
            "Sample model light grid at each pixel's world position (0 legacy); applies immediately"))?;
        cvars.register(CvarDefinition::new(TOON_CVAR, 0_i64, CvarFlags::ARCHIVE,
            "Toon shading on players and other lit models: light in soft bands with a              rim on the silhouette (0 stock smooth shading); applies immediately"))?;
        Self::from_registered(cvars)
    }

    fn from_registered(cvars: &mut CvarRegistry) -> Result<Self, CvarError> {
        let settings = Self::default();
        settings.set(&cvars.get("r_modelPixelLight").unwrap().value);
        let changed = settings.clone();
        cvars.on_change("r_modelPixelLight", move |change| {
            changed.set(&change.current)
        })?;
        settings.set_toon(&cvars.get(TOON_CVAR).unwrap().value);
        let changed = settings.clone();
        cvars.on_change(TOON_CVAR, move |change| changed.set_toon(&change.current))?;
        Ok(settings)
    }

    fn set(&self, value: &CvarValue) {
        if let CvarValue::Integer(value) = value {
            self.0.store(*value != 0, Ordering::Relaxed);
        }
    }

    fn set_toon(&self, value: &CvarValue) {
        let mode = match value {
            CvarValue::Integer(value) => u32::from(*value != 0),
            CvarValue::Bool(on) => u32::from(*on),
            _ => return,
        };
        self.1.store(mode, Ordering::Relaxed);
    }

    /// The model word of the scene-light block (`point_lights.metadata.y`): bit 0 per-pixel
    /// grid sampling, bits 1..3 the toon mode. Read once at the existing scene-light
    /// upload, never once per pixel or actor.
    pub(crate) fn bits(&self) -> u32 {
        u32::from(self.0.load(Ordering::Relaxed)) | self.1.load(Ordering::Relaxed) << 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toon_models_are_off_by_default_and_take_their_own_bits() {
        let mut cvars = CvarRegistry::new();
        let settings = Settings::bind(&mut cvars).expect("the settings bind");
        assert_eq!(settings.bits(), 1);
        cvars.set_text(TOON_CVAR, "1").expect("set");
        assert_eq!(settings.bits(), 1 | 2);
        cvars.set_text("r_modelPixelLight", "0").expect("set");
        assert_eq!(settings.bits(), 2);
        cvars.set_text(TOON_CVAR, "0").expect("set");
        assert_eq!(settings.bits(), 0);
    }
}
