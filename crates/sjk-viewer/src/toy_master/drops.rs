//! The drops a toy's show throws out (Sol, 02/11/2026: "random holocrons, shards, credits
//! that have physics and drop on the ground, people can walk to them and pick them up
//! with E when it's stationary"). The hub rolls what each one
//! is and tells every SJK player on the server with its show (`drops_<id>_<seed>_..`):
//! what it looks like, never what it gives. Each client throws them the same way from
//! the seed ([`crate::ragdoll::unit`]) at the same moments of the show
//! (`Show::drop_time`), and runs them in fixed steps against the map, so they land in the
//! same places for everyone. A drop at rest may be taken; the hub gives it to whoever
//! claims it first and tells the others (`taken_<id>_<drop>`).

use crate::ragdoll::{World, unit};
use glam::Vec3;
use sjk_identity::toy_master::{DropKind, Show};

/// One step, in milliseconds and in seconds.
const STEP_MS: f32 = 16.0;
const STEP: f32 = STEP_MS / 1_000.0;
/// `g_gravity`.
const GRAVITY: f32 = 800.0;
/// Half the size of a drop's box in the world.
const HALF: f32 = 4.0;
/// How much of its speed into a surface a drop bounces back with, and how much of its
/// slide it keeps each step on a floor.
const BOUNCE: f32 = 0.35;
const SLIDE_KEPT: f32 = 0.85;
/// A drop slower than this on a floor (units a second) for [`STILL_STEPS`] in a row
/// rests; one still moving after [`MAX_STEPS`] rests where it is.
const STILL_SPEED: f32 = 12.0;
const STILL_STEPS: u32 = 12;
const MAX_STEPS: u32 = 900;
/// A surface this upright or more is a floor.
const FLOOR: f32 = 0.7;
/// Where a drop leaves the toy: this far over where the toy stands (its mouth).
const MOUTH: f32 = 26.0;
/// How fast a drop is thrown: across, from the first to the first and second, and up.
const ACROSS: (f32, f32) = (60.0, 120.0);
const UP: (f32, f32) = (260.0, 120.0);
/// How fast a drop turns as it flies, and once at rest, in degrees a second.
const FLY_TURN: f32 = 400.0;
const REST_TURN: f32 = 60.0;
/// Farthest a player may stand from a drop to take it, across the floor and up or down,
/// and the widest angle between their facing and it (60 degrees).
pub(super) const REACH: f32 = 64.0;
pub(super) const REACH_UP: f32 = 80.0;

/// One drop of a show.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Drop {
    pub(crate) index: u32,
    pub(crate) kind: DropKind,
    /// When it comes out, in milliseconds after the show's start.
    thrown: f32,
    /// Where it is, and where it was a step ago (drawn between the two).
    at: Vec3,
    before: Vec3,
    speed: Vec3,
    steps: u32,
    still: u32,
    /// It lies still: it may be taken.
    pub(crate) resting: bool,
    /// Someone took it (it is no longer drawn).
    pub(crate) taken: bool,
    /// The player asked the hub for it at this moment, and waits.
    pub(crate) asked: Option<std::time::Instant>,
}

/// A show's drops.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Drops {
    pub(crate) list: Vec<Drop>,
}

impl Drops {
    /// The drops `kinds` of a show of `show` from `seed`, thrown from a toy standing at
    /// `origin`.
    pub(crate) fn new(show: &Show, seed: u32, kinds: &[DropKind], origin: Vec3) -> Self {
        let count = kinds.len() as u32;
        let list = kinds
            .iter()
            .enumerate()
            .filter_map(|(index, kind)| {
                let index = index as u32;
                let thrown = show.drop_time(index, count)? as f32;
                let salt = index * 3;
                let angle = unit(seed, salt) * std::f32::consts::TAU;
                let across = ACROSS.0 + ACROSS.1 * unit(seed, salt + 1);
                let up = UP.0 + UP.1 * unit(seed, salt + 2);
                let at = origin + Vec3::Z * MOUTH;
                Some(Drop {
                    index,
                    kind: *kind,
                    thrown,
                    at,
                    before: at,
                    speed: Vec3::new(angle.cos() * across, angle.sin() * across, up),
                    steps: 0,
                    still: 0,
                    resting: false,
                    taken: false,
                    asked: None,
                })
            })
            .collect();
        Self { list }
    }

    /// Run every drop thrown by `elapsed_ms` into the show on to then, against `world`.
    pub(crate) fn run(&mut self, elapsed_ms: f32, world: &mut impl World) {
        for drop in &mut self.list {
            if drop.taken || elapsed_ms < drop.thrown {
                continue;
            }
            let steps = ((elapsed_ms - drop.thrown) / STEP_MS) as u32;
            while drop.steps < steps && !drop.resting {
                drop.step(world);
            }
            if drop.resting {
                drop.before = drop.at;
            }
        }
    }

    /// The drops to draw `elapsed_ms` into the show: each one thrown and not taken,
    /// where it is (between its last two steps) and which way it faces (degrees).
    pub(crate) fn shown(&self, elapsed_ms: f32) -> impl Iterator<Item = (&Drop, Vec3, f32)> {
        self.list
            .iter()
            .filter(move |drop| !drop.taken && elapsed_ms >= drop.thrown)
            .map(move |drop| {
                let into = elapsed_ms - drop.thrown;
                let within = ((into - drop.steps as f32 * STEP_MS) / STEP_MS + 1.0).clamp(0.0, 1.0);
                let at = if drop.resting {
                    drop.at
                } else {
                    drop.before.lerp(drop.at, within)
                };
                let turn = if drop.resting { REST_TURN } else { FLY_TURN };
                let yaw = (into / 1_000.0 * turn + drop.index as f32 * 47.0) % 360.0;
                (drop, at, yaw)
            })
    }

    /// The resting drop not taken that the player at `from` facing `yaw` (radians) may
    /// take: within reach and faced, the nearest.
    pub(crate) fn faced(&self, from: Vec3, yaw: f32) -> Option<&Drop> {
        let facing = glam::Vec2::from_angle(yaw);
        self.list
            .iter()
            .filter(|drop| drop.resting && !drop.taken)
            .filter_map(|drop| {
                let way = (drop.at - from).truncate();
                let across = way.length();
                let faced = across < 8.0 || facing.angle_to(way).abs() <= super::FACING;
                (across <= REACH && (drop.at.z - from.z).abs() <= REACH_UP && faced)
                    .then_some((drop, across))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(drop, _)| drop)
    }

    pub(crate) fn get_mut(&mut self, index: u32) -> Option<&mut Drop> {
        self.list.iter_mut().find(|drop| drop.index == index)
    }
}

impl Drop {
    /// One step: gravity, the flight out of the world's way, a bounce off what it meets,
    /// its slide on a floor, and rest once still.
    fn step(&mut self, world: &mut impl World) {
        self.steps += 1;
        self.before = self.at;
        self.speed.z -= GRAVITY * STEP;
        let to = self.at + self.speed * STEP;
        let mut on_floor = false;
        match world.trace(self.at, to, HALF) {
            Some(hit) => {
                self.at = hit.stop + hit.normal * 0.125;
                let into = self.speed.dot(hit.normal);
                if into < 0.0 {
                    self.speed -= hit.normal * into * (1.0 + BOUNCE);
                }
                if hit.normal.z >= FLOOR {
                    on_floor = true;
                    let up = hit.normal * self.speed.dot(hit.normal);
                    self.speed = up + (self.speed - up) * SLIDE_KEPT;
                }
            }
            None => self.at = to,
        }
        if on_floor && self.speed.length() < STILL_SPEED {
            self.still += 1;
        } else {
            self.still = 0;
        }
        if self.still >= STILL_STEPS || self.steps >= MAX_STEPS {
            self.resting = true;
            self.speed = Vec3::ZERO;
        }
    }

    /// Where it lies (or flies).
    #[cfg(test)]
    pub(crate) fn at(&self) -> Vec3 {
        self.at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat floor at z = 0, and a wall at x = 300.
    fn room(from: Vec3, to: Vec3, half: f32) -> Option<(Vec3, Vec3)> {
        if to.z - half < 0.0 && from.z - half >= 0.0 {
            let u = (from.z - half) / (from.z - to.z);
            return Some((from.lerp(to, u), Vec3::Z));
        }
        if to.x + half > 300.0 && from.x + half <= 300.0 {
            let u = (300.0 - half - from.x) / (to.x - from.x);
            return Some((from.lerp(to, u), Vec3::NEG_X));
        }
        None
    }

    /// Thrown from the same seed, the drops fly the same way and come to rest on the
    /// floor, within reach of a walk, each at its own moment of the show.
    #[test]
    fn drops_fly_alike_from_the_seed_and_rest_on_the_floor() {
        let show = super::super::track::tests::stepped();
        let kinds = [DropKind::Credits, DropKind::Shards, DropKind::Holocron];
        let mut drops = Drops::new(&show, 1234, &kinds, Vec3::ZERO);
        let mut again = Drops::new(&show, 1234, &kinds, Vec3::ZERO);
        let end = 1_830.0 + 135_000.0;
        let thrown = drops.list[0].thrown;
        assert_eq!(drops.shown(thrown - 1.0).count(), 0);
        drops.run(thrown + 300.0, &mut room);
        assert_eq!(drops.shown(thrown + 300.0).count(), 1);
        assert!(drops.list[0].at().z > 20.0, "{:?}", drops.list[0].at());
        drops.run(end, &mut room);
        again.run(end, &mut room);
        assert_eq!(drops, again);
        for drop in &drops.list {
            assert!(drop.resting, "{drop:?}");
            assert!((drop.at().z - HALF).abs() < 1.0, "{drop:?}");
            assert!(drop.at().truncate().length() < 600.0 && drop.at().x < 300.0);
        }
        let other = Drops::new(&show, 99, &kinds, Vec3::ZERO);
        assert_ne!(other.list[0].speed, drops.list[0].speed);
    }

    /// Only a resting drop not taken, within reach and faced, may be taken.
    #[test]
    fn a_resting_drop_in_reach_and_faced_may_be_taken() {
        let show = super::super::track::tests::stepped();
        let mut drops = Drops::new(&show, 7, &[DropKind::Credits], Vec3::ZERO);
        let at = Vec3::new(40.0, 0.0, 4.0);
        drops.list[0].at = at;
        let player = Vec3::new(0.0, 0.0, 24.0);
        assert!(drops.faced(player, 0.0).is_none());
        drops.list[0].resting = true;
        assert_eq!(drops.faced(player, 0.0).map(|drop| drop.index), Some(0));
        assert!(drops.faced(player, std::f32::consts::PI).is_none());
        assert!(drops.faced(Vec3::new(-100.0, 0.0, 24.0), 0.0).is_none());
        drops.get_mut(0).unwrap().taken = true;
        assert!(drops.faced(player, 0.0).is_none());
    }
}
