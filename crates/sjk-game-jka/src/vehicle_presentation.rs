//! Client-only vehicle weapon assets from `vehWeaponFields` in multiplayer
//! `bg_vehicleLoad.c`. These names do not register server configstrings.

use crate::text_parse::TextParser;

/// Authored projectile presentation. Empty fields retain stock's null handles.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WeaponPresentation {
    /// EFX trail emitted each presented frame.
    pub shot_effect: Option<String>,
    /// Optional rigid projectile model.
    pub model: Option<String>,
    /// Optional flight loop.
    pub loop_sound: Option<String>,
    /// `muzzleFX`: the flash `EV_VEH_FIRE` plays bolted to the muzzle that fired
    /// (`CG_VehMuzzleFireFX`). Its `Sound` primitive is the weapon's firing sound.
    pub muzzle_effect: Option<String>,
    /// `impactFX`: what the shot plays where it hits instead of its carrier weapon's
    /// impact (`CG_VehicleWeaponImpact`, `cg_weapons.c`): the AT-ST's laser flies as
    /// `WP_ROCKET_LAUNCHER` but bursts in `atst/wall_impact`, not a rocket explosion.
    pub impact_effect: Option<String>,
}

pub(crate) fn parse(text: &[u8], name: &[u8]) -> WeaponPresentation {
    let mut parser = TextParser::new(text);
    let mut result = WeaponPresentation::default();
    if !crate::vehicle_parms::find_block(&mut parser, name) {
        return result;
    }
    loop {
        parser.skip_rest_of_line();
        let key = parser.parse_ext(true);
        if key.is_empty() || key == b"}" {
            break;
        }
        let field = if key.eq_ignore_ascii_case(b"shotFX") {
            Some(&mut result.shot_effect)
        } else if key.eq_ignore_ascii_case(b"model") {
            Some(&mut result.model)
        } else if key.eq_ignore_ascii_case(b"loopSound") {
            Some(&mut result.loop_sound)
        } else if key.eq_ignore_ascii_case(b"muzzleFX") {
            Some(&mut result.muzzle_effect)
        } else if key.eq_ignore_ascii_case(b"impactFX") {
            Some(&mut result.impact_effect)
        } else {
            None
        };
        let value = parser.parse_ext(true);
        if let Some(field) = field {
            *field = std::str::from_utf8(value)
                .ok()
                .filter(|value| !value.is_empty())
                .map(str::to_owned);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_weapon_names_its_muzzle_flash() {
        let text = b"WeaponA
{
	muzzleFX	ships/reb_blastermuzzleflash
	shotFX	ships/reb_blastershot
	impactFX	ships/reb_blasterimpact
}
";
        let weapon = parse(text, b"WeaponA");
        assert_eq!(
            weapon.muzzle_effect.as_deref(),
            Some("ships/reb_blastermuzzleflash")
        );
        assert_eq!(weapon.shot_effect.as_deref(), Some("ships/reb_blastershot"));
        assert_eq!(
            weapon.impact_effect.as_deref(),
            Some("ships/reb_blasterimpact")
        );
        assert_eq!(parse(text, b"WeaponB"), WeaponPresentation::default());
    }
}
