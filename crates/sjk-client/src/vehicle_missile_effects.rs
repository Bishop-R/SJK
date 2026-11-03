//! Reconstruct cgame's vehicle weapon indices in `CS_MODELS` registration order.
//! The wire carries a vehicle weapon index, not a `CS_EFFECTS` index.

use sjk_game_jka::vehicle_parms::{VehicleCapacity, VehicleFiles, VehicleRegistry, VehicleTable};
use sjk_game_jka::vehicle_presentation::WeaponPresentation;
use sjk_protocol::GameState;
use sjk_vfs::VirtualFileSystem;
use std::sync::Arc;

const CS_MODELS: usize = 298;
const MAX_MODELS: usize = 512;

pub(crate) struct VehicleMissiles {
    files: Arc<VehicleFiles>,
    table: VehicleTable,
    weapons: Vec<WeaponPresentation>,
    /// The vehicle each `CS_MODELS` slot names (`$name`), by model index.
    by_model: Vec<Option<usize>>,
}

impl VehicleMissiles {
    pub(crate) fn load(vfs: &VirtualFileSystem, game: &GameState) -> Self {
        let files = VehicleFiles::from_listing(
            |directory, extension| vfs.list_files(directory, extension),
            |path| vfs.read(path).ok().flatten().map(|asset| asset.bytes),
        );
        Self::from_files(files, game)
    }

    fn from_files(files: VehicleFiles, game: &GameState) -> Self {
        let files = Arc::new(files);
        let mut result = Self {
            table: VehicleTable::new(Arc::clone(&files), VehicleCapacity::REFERENCE),
            files,
            weapons: vec![WeaponPresentation::default()],
            by_model: vec![None; MAX_MODELS],
        };
        for index in CS_MODELS..CS_MODELS + MAX_MODELS {
            result.refresh(index, game);
        }
        result
    }

    pub(crate) fn refresh(&mut self, index: usize, game: &GameState) {
        if !(CS_MODELS..CS_MODELS + MAX_MODELS).contains(&index) {
            return;
        }
        let Some(name) = game
            .config_string(index)
            .and_then(|name| name.strip_prefix(b"$"))
        else {
            return;
        };
        self.by_model[index - CS_MODELS] = self.table.index_for_name(name, &mut Unregistered);
        for weapon in &self.table.weapons()[self.weapons.len()..] {
            self.weapons.push(
                weapon
                    .name
                    .as_deref()
                    .map_or_else(WeaponPresentation::default, |name| {
                        self.files.weapon_presentation(name)
                    }),
            );
        }
    }

    pub(crate) fn weapon(&self, index: usize) -> Option<&WeaponPresentation> {
        self.weapons.get(index)
    }

    pub(crate) fn weapons(&self) -> impl Iterator<Item = &WeaponPresentation> {
        self.weapons.iter()
    }

    pub(crate) fn effect_names(&self) -> impl Iterator<Item = &str> {
        self.weapons.iter().flat_map(|weapon| {
            [
                weapon.shot_effect.as_deref(),
                weapon.muzzle_effect.as_deref(),
                weapon.impact_effect.as_deref(),
            ]
            .into_iter()
            .flatten()
        })
    }

    /// `CG_VehMuzzleFireFX` (`cg_event.c:1391-1432`): the `muzzleFX` of the weapon on
    /// muzzle `muzzle` (from 0) of the vehicle whose model is `CS_MODELS` slot
    /// `model_index`: the muzzle's own weapon (`weapMuzzle`), else the turret's whose
    /// `iMuzzle` names it.
    pub(crate) fn muzzle_effect(&self, model_index: usize, muzzle: usize) -> Option<&str> {
        let vehicle = self
            .table
            .vehicle(self.by_model.get(model_index).copied().flatten()?)?;
        let mut weapon = *vehicle.muzzle_weapons.get(muzzle)?;
        if weapon == 0 {
            // Stock walks every turret and stops at the first muzzle that matches.
            weapon = vehicle
                .turrets
                .iter()
                .find(|turret| turret.muzzles.iter().any(|&m| m - 1 == muzzle as i32))?
                .weapon;
        }
        self.weapons
            .get(usize::try_from(weapon).ok()?)?
            .muzzle_effect
            .as_deref()
    }
}

struct Unregistered;
impl VehicleRegistry for Unregistered {
    fn model_index(&mut self, _: &[u8]) -> i32 {
        0
    }
    fn sound_index(&mut self, _: &[u8]) -> i32 {
        0
    }
    fn effect_index(&mut self, _: &[u8]) -> i32 {
        0
    }
    fn print(&mut self, _: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    const VEHICLES: &[u8] = b"fighter
{
	name	fighter
	type	VH_FIGHTER
	weapMuzzle1	laser
	weapMuzzle3	laser
	turret1Weap	turbo
	turret1Muzzle1	2
}
";
    const WEAPONS: &[u8] = b"laser
{
	name	laser
	muzzleFX	ships/laser_flash
	shotFX	ships/laser_shot
	impactFX	atst/wall_impact
}
turbo
{
	name	turbo
	muzzleFX	ships/turbo_flash
}
";

    #[test]
    fn each_muzzle_plays_its_weapons_flash_or_its_turrets() {
        let files = VehicleFiles::load([("f.veh", VEHICLES)], [("w.vwp", WEAPONS)]).unwrap();
        let mut game = GameState::empty_local(0);
        game.replace_config_string(CS_MODELS + 5, b"$fighter".to_vec())
            .unwrap();
        let missiles = VehicleMissiles::from_files(files, &game);
        // weapMuzzle1 and weapMuzzle3; muzzle 2 has no weapon of its own, so turret 1's,
        // whose iMuzzle is 2.
        assert_eq!(missiles.muzzle_effect(5, 0), Some("ships/laser_flash"));
        assert_eq!(missiles.muzzle_effect(5, 1), Some("ships/turbo_flash"));
        assert_eq!(missiles.muzzle_effect(5, 2), Some("ships/laser_flash"));
        assert_eq!(missiles.muzzle_effect(5, 3), None);
        // A model that is not a vehicle, or past the table.
        assert_eq!(missiles.muzzle_effect(6, 0), None);
        assert_eq!(missiles.muzzle_effect(MAX_MODELS, 0), None);
        // Loaded with the map, so their sounds are registered.
        let names: Vec<_> = missiles.effect_names().collect();
        assert!(names.contains(&"ships/laser_flash") && names.contains(&"ships/turbo_flash"));
        assert!(names.contains(&"atst/wall_impact"));
        // The laser names its impact; the turbo has none.
        let impacts: Vec<_> = missiles
            .weapons()
            .map(|w| (w.shot_effect.as_deref(), w.impact_effect.as_deref()))
            .collect();
        assert!(impacts.contains(&(Some("ships/laser_shot"), Some("atst/wall_impact"))));
        assert!(impacts.contains(&(None, None)));
    }
}
