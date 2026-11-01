//! The box hunt's cheat watch (Sol, 01/11/2026): a player in a chase, the mark or a
//! hunter, who is teleported other than by the map or a spawn (JA+'s `amtele`, an admin's
//! teleport, any server command that moves them) or flies in noclip is out of it at once.
//! The hub sees neither, so the player's own client watches its own player state, as it
//! tells the hub who killed the mark.
//!
//! A teleport is the game's `EF_TELEPORT_BIT` flipping (`TeleportPlayer`, `ClientSpawn`)
//! while the player stays alive. The map's own teleporters (`trigger_teleport`,
//! `target_teleporter`) and spawns flip it too, and land the player on a point the map
//! names: a landing within [`NEAR`] units of one of those is not a cheat.

use glam::Vec3;
use sjk_protocol::PlayerState;

/// `EF_DEAD`, `EF_TELEPORT_BIT` (`bg_public.h`).
const EF_DEAD: u32 = 1 << 1;
const EF_TELEPORT_BIT: u32 = 1 << 3;
/// `PMF_FOLLOW`: the view is someone else's.
const PMF_FOLLOW: u16 = 0x1000;
/// `pm_type`: `PM_NOCLIP`, `PM_SPECTATOR`, `PM_DEAD`, `PM_INTERMISSION`,
/// `PM_SPINTERMISSION`.
const PM_NOCLIP: u8 = 3;
const PM_SPECTATOR: u8 = 4;
const PM_DEAD: u8 = 5;
const PM_INTERMISSION: u8 = 7;
const PM_SPINTERMISSION: u8 = 8;
/// How far from a map's landing point a teleport may land and be the map's: a snapshot
/// may come a frame or two after the teleport, which throws the player forward.
const NEAR: f32 = 128.0;

/// Where the map's own teleporters and spawns put a player.
pub(super) fn landings(entities: &[sjk_entity::Entity]) -> Vec<Vec3> {
    let targets: Vec<&str> = entities
        .iter()
        .filter(|entity| {
            entity.classname().is_some_and(|class| {
                class.eq_ignore_ascii_case("trigger_teleport")
                    || class.eq_ignore_ascii_case("target_teleporter")
            })
        })
        .filter_map(|entity| entity.get("target"))
        .collect();
    let mut points: Vec<Vec3> = entities
        .iter()
        .filter(|entity| {
            entity.get("targetname").is_some_and(|name| {
                targets
                    .iter()
                    .any(|target| target.eq_ignore_ascii_case(name))
            }) || entity
                .classname()
                .is_some_and(|class| class.eq_ignore_ascii_case("misc_teleporter_dest"))
        })
        .filter_map(|entity| entity.vector("origin").ok().flatten())
        .map(Vec3::from_array)
        .collect();
    points.extend(crate::banana::spawn_points(entities));
    points
}

/// What the watch reads of one snapshot of the player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Sample {
    /// The snapshot's server time.
    pub(super) time: i32,
    /// The player state is the player's own (not someone followed).
    pub(super) own: bool,
    /// Alive and in the game (not dead, spectating or at the intermission).
    pub(super) alive: bool,
    pub(super) noclip: bool,
    /// `EF_TELEPORT_BIT`.
    pub(super) teleport: bool,
    pub(super) origin: Vec3,
}

impl Sample {
    /// The sample of `player` (the snapshot at `time`), the player on slot `own`.
    pub(super) fn of(player: &PlayerState, time: i32, own: u8) -> Self {
        let kind = player.movement_type();
        let flags = player.entity_flags();
        Self {
            time,
            own: player.client_num() == u16::from(own) && player.movement_flags() & PMF_FOLLOW == 0,
            alive: player.health() > 0
                && flags & EF_DEAD == 0
                && !player.is_spectator()
                && ![PM_SPECTATOR, PM_DEAD, PM_INTERMISSION, PM_SPINTERMISSION].contains(&kind),
            noclip: kind == PM_NOCLIP,
            teleport: flags & EF_TELEPORT_BIT != 0,
            origin: Vec3::from_array(player.origin()),
        }
    }
}

/// The player's last sample, to compare the next with.
#[derive(Default)]
pub(super) struct Watch {
    last: Option<Sample>,
}

impl Watch {
    /// Take `sample` (`landings`, the map's, [`landings`]): whether it shows the player
    /// cheating since the last one.
    pub(super) fn step(&mut self, sample: Sample, landings: &[Vec3]) -> bool {
        let last = self.last.replace(sample);
        if !sample.own || !sample.alive {
            self.last = None;
            return false;
        }
        if sample.noclip {
            return true;
        }
        let Some(last) = last.filter(|last| last.time < sample.time) else {
            // The first sample, or a new map (its clock starts again).
            return false;
        };
        last.alive
            && last.teleport != sample.teleport
            && !landings
                .iter()
                .any(|point| point.distance(sample.origin) <= NEAR)
    }

    /// Forget the last sample (the map changed).
    pub(super) fn reset(&mut self) {
        self.last = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(time: i32, teleport: bool, origin: Vec3) -> Sample {
        Sample {
            time,
            own: true,
            alive: true,
            noclip: false,
            teleport,
            origin,
        }
    }

    #[test]
    fn a_teleport_off_the_map_points_is_a_cheat() {
        let landings = [Vec3::new(1000.0, 0.0, 0.0)];
        let mut watch = Watch::default();
        assert!(!watch.step(sample(100, false, Vec3::ZERO), &landings));
        // Walking: nothing.
        assert!(!watch.step(sample(150, false, Vec3::X * 300.0), &landings));
        // A map teleporter's landing, thrown forward a little: not a cheat.
        assert!(!watch.step(sample(200, true, Vec3::new(1060.0, 0.0, 30.0)), &landings));
        // Teleported elsewhere: a cheat.
        assert!(watch.step(sample(250, false, Vec3::new(-4000.0, 0.0, 0.0)), &landings));
    }

    #[test]
    fn a_spawn_after_death_or_spectating_is_no_cheat() {
        let mut watch = Watch::default();
        assert!(!watch.step(sample(100, false, Vec3::ZERO), &[]));
        let dead = Sample {
            alive: false,
            ..sample(150, false, Vec3::ZERO)
        };
        assert!(!watch.step(dead, &[]));
        assert!(!watch.step(sample(200, true, Vec3::X * 5000.0), &[]));
        // Someone followed, then the player's own view again.
        let followed = Sample {
            own: false,
            ..sample(250, false, Vec3::ZERO)
        };
        assert!(!watch.step(followed, &[]));
        assert!(!watch.step(sample(300, false, Vec3::Y * 5000.0), &[]));
    }

    #[test]
    fn noclip_is_a_cheat_and_a_new_map_starts_again() {
        let mut watch = Watch::default();
        let flying = Sample {
            noclip: true,
            ..sample(100, false, Vec3::ZERO)
        };
        assert!(watch.step(flying, &[]));
        let mut watch = Watch::default();
        assert!(!watch.step(sample(5000, false, Vec3::ZERO), &[]));
        // The clock went back: a new map, whose spawn flips the bit.
        assert!(!watch.step(sample(100, true, Vec3::X * 5000.0), &[]));
    }

    #[test]
    fn the_map_names_its_teleport_landings_and_spawns() {
        let lump = r#"{
"classname" "trigger_teleport"
"target" "pad"
"model" "*1"
}
{
"classname" "target_position"
"targetname" "pad"
"origin" "10 20 30"
}
{
"classname" "target_position"
"targetname" "elsewhere"
"origin" "99 99 99"
}
{
"classname" "info_player_deathmatch"
"origin" "-5 0 0"
}"#;
        let entities = sjk_entity::parse_entity_lump(lump.as_bytes()).unwrap();
        let points = landings(&entities);
        assert!(points.contains(&Vec3::new(10.0, 20.0, 30.0)));
        assert!(points.contains(&Vec3::new(-5.0, 0.0, 0.0)));
        assert!(!points.contains(&Vec3::new(99.0, 99.0, 99.0)));
    }
}
