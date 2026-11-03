//! A destroyed vehicle's flames: the dead branch of `CG_VehicleEffects`
//! (`codemp/cgame/cg_players.c`).
//!
//! While a vehicle carries `EF_DEAD` (its death until it blows up, the AT-ST's
//! `explosionDelay` of 2.45 s), stock plays `effects/ships/dest_burning.efx` at the
//! vehicle's origin, facing up, every 50 ms (`m_iLastFXTime`'s debounce). Animals
//! give off no vehicle effects. The live vehicles' smoke of stock is a fighter's
//! damaged surfaces (`brokenLimbs` with the `.veh` file's `dmgFX` and `injureFX` at
//! `*nosedamage` and its kin), which this module does not draw.

use crate::vehicle_assets::VehicleKind;
use std::time::{Duration, Instant};

/// `cgs.effects.mShipDestBurning` (`cg_main.c`).
pub(crate) const EFFECT: &str = "ships/dest_burning";
/// `EF_DEAD` (`bg_public.h`).
const EF_DEAD: u32 = 1 << 1;
/// `nextFXDelay`: one play per 50 ms.
const INTERVAL: Duration = Duration::from_millis(50);

/// Whether a vehicle of `kind` with entity flags `e_flags` burns.
pub(crate) fn burns(kind: VehicleKind, e_flags: u32) -> bool {
    kind != VehicleKind::Animal && e_flags & EF_DEAD != 0
}

/// `MAX_GENTITIES`: one debounce per entity number.
const ENTITIES: usize = 1024;

/// Every vehicle's debounce (`m_iLastFXTime`), by entity number.
pub(crate) struct Flames {
    next: Box<[Option<Instant>; ENTITIES]>,
}

impl Default for Flames {
    fn default() -> Self {
        Self {
            next: Box::new([None; ENTITIES]),
        }
    }
}

impl Flames {
    /// Whether vehicle `number`'s flames play at `now`; a play holds the next one 50 ms.
    /// A vehicle that stops burning starts again at once the next time.
    pub(crate) fn due(&mut self, number: u16, burning: bool, now: Instant) -> bool {
        let Some(next) = self.next.get_mut(usize::from(number)) else {
            return false;
        };
        if !burning {
            *next = None;
            return false;
        }
        if next.is_some_and(|next| now < next) {
            return false;
        }
        *next = Some(now + INTERVAL);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dead_vehicles_burn_every_fifty_milliseconds_but_animals_never() {
        assert!(burns(VehicleKind::Walker, EF_DEAD));
        assert!(burns(VehicleKind::Speeder, EF_DEAD | 1));
        assert!(!burns(VehicleKind::Walker, 0));
        assert!(!burns(VehicleKind::Animal, EF_DEAD));
        let start = Instant::now();
        let mut flames = Flames::default();
        assert!(flames.due(7, true, start));
        assert!(!flames.due(7, true, start + Duration::from_millis(49)));
        // Another vehicle keeps its own pace.
        assert!(flames.due(8, true, start + Duration::from_millis(49)));
        assert!(flames.due(7, true, start + Duration::from_millis(50)));
        assert!(!flames.due(7, false, start + Duration::from_millis(60)));
        assert!(flames.due(7, true, start + Duration::from_millis(61)));
        assert!(!flames.due(u16::MAX, true, start));
    }
}
