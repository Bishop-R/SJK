//! A thrown monitor's flight: the old CRT as a solid of eight corners held by sticks
//! between every two of them (Verlet integration, as the Ragdoll's body,
//! [`crate::ragdoll`]), so that it tumbles, bounces off the world, topples over an edge
//! and comes to rest on a face.
//!
//! Every SJK client that hears of a throw runs this same simulation from the same
//! [`Throw`], in fixed steps of [`STEP_MS`] game server milliseconds from the throw's
//! moment, as the Ragdoll does: plain arithmetic from whole numbers, so every client puts
//! the monitor in the same place. A monitor that strikes a body is bounced back at that
//! step ([`Deflect`]), on every client alike; one heard of late runs again from its throw.

use crate::ragdoll::{STEP_MS, World, friction, unbounced, unit};
use glam::{Mat3, Quat, Vec3};

/// The corners.
pub(crate) const CORNERS: usize = 8;
/// The corners in the monitor's own frame (its screen facing +X, its left +Y, up +Z),
/// their centre at the origin: the screen's face, then the tube's back, smaller and
/// standing on the same bottom. `scripts/monitor_assets.py` builds the model on them.
const SHAPE: [[f32; 3]; CORNERS] = [
    [9.0, 10.0, 10.25],
    [9.0, -10.0, 10.25],
    [9.0, -10.0, -6.75],
    [9.0, 10.0, -6.75],
    [-9.0, 6.0, 3.25],
    [-9.0, -6.0, 3.25],
    [-9.0, -6.0, -6.75],
    [-9.0, 6.0, -6.75],
];
/// The corners of the screen's face, of the top and of the bottom, for its turn.
const FRONT: [usize; 4] = [0, 1, 2, 3];
const BACK: [usize; 4] = [4, 5, 6, 7];
const TOP: [usize; 4] = [0, 1, 4, 5];
const BOTTOM: [usize; 4] = [2, 3, 6, 7];
/// About how far its body reaches from its centre: a body this close is struck.
pub(crate) const RADIUS: f32 = 11.0;
/// Half the size of each corner's box in the world's collision.
const HALF: f32 = 1.5;
const STEP: f32 = STEP_MS as f32 / 1000.0;
/// `g_gravity`.
const GRAVITY: f32 = 800.0;
/// How much of its speed it keeps from one step to the next, in the air.
const DRAG: f32 = 0.998;
/// Rounds of the sticks per step.
const ROUNDS: usize = 4;
/// Its corners on a surface: Coulomb's friction, the share of the slide kept besides,
/// and how much of its speed into the surface it bounces back with (plastic and glass:
/// more than a body), faster than [`BOUNCE_SPEED`].
const FRICTION: f32 = 0.5;
const CONTACT_KEPT: f32 = 0.99;
const BOUNCE: f32 = 0.35;
const BOUNCE_SPEED: f32 = 80.0;
/// The fastest a corner bounces back off a surface, units a second.
const MAX_BOUNCE: f32 = 300.0;
/// A knock harder than this is heard (units a second).
pub(crate) const KNOCK_SPEED: f32 = 160.0;
/// The fastest a corner may go, in units a step.
const TOP_SPEED: f32 = 56.0;
/// A corner moving less than this a step is still; the monitor sleeps once all are, for
/// this many steps in a row, or after [`MAX_STEPS`] whatever it does (20 seconds).
const STILL: f32 = 0.15;
const SLEEP_STEPS: u32 = 30;
pub(crate) const MAX_STEPS: u32 = 1_250;
/// It stops this far below where it was thrown: the void under a map.
const FLOOR_DEPTH: f32 = 8_192.0;
/// How fast it tumbles from the hand (radians a second), the least and the most more.
const SPIN: f32 = 5.0;
const SPIN_MORE: f32 = 6.0;
/// How fast a bounce off a body sets it turning (radians a second).
const DEFLECT_SPIN: f32 = 9.0;

/// How a monitor is thrown: the moment (game server milliseconds), where it leaves the
/// hand (its centre, whole units) and how fast (units a second). It faces the way it
/// flies, and the moment sets its tumble.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Throw {
    pub(crate) moment: i32,
    pub(crate) origin: [i32; 3],
    pub(crate) velocity: [i32; 3],
}

/// A bounce off a body: at the start of step `step` the monitor gains `push` (units a
/// second) and a tumble.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Deflect {
    pub(crate) step: u32,
    pub(crate) push: [i32; 3],
}

/// The monitor, as it flies.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Flight {
    throw: Throw,
    points: [Vec3; CORNERS],
    previous: [Vec3; CORNERS],
    /// Each corner before the last step, where it was shown.
    earlier: [Vec3; CORNERS],
    lengths: [[f32; CORNERS]; CORNERS],
    deflects: Vec<Deflect>,
    steps: u32,
    still: u32,
    /// The hardest knock on the world since [`Flight::run_to`] began, units a second.
    pub(crate) knock: f32,
    lowest: f32,
    /// How far its centre was from where it was thrown after each step (the first at
    /// the hand, 0), for the hits heard of a ping after their step ([`Flight::reach_at`]).
    reach: Vec<f32>,
}

fn vector(v: [i32; 3]) -> Vec3 {
    Vec3::new(v[0] as f32, v[1] as f32, v[2] as f32)
}

impl Flight {
    /// The monitor leaving the hand.
    pub(crate) fn new(throw: &Throw) -> Self {
        let velocity = vector(throw.velocity);
        let centre = vector(throw.origin);
        let forward = velocity.try_normalize().unwrap_or(Vec3::X);
        let left = Vec3::Z.cross(forward).try_normalize().unwrap_or(Vec3::Y);
        let up = forward.cross(left);
        let seed = throw.moment as u32;
        // A tumble end over end (the screen dips), and some of a turn and a roll.
        let spin = left * (SPIN + SPIN_MORE * unit(seed, 11))
            + up * (3.0 * unit(seed, 12) - 1.5)
            + forward * (2.0 * unit(seed, 13) - 1.0);
        let mut points = [Vec3::ZERO; CORNERS];
        let mut previous = [Vec3::ZERO; CORNERS];
        for (corner, shape) in SHAPE.iter().enumerate() {
            let [x, y, z] = *shape;
            let r = forward * x + left * y + up * z;
            points[corner] = centre + r;
            previous[corner] = points[corner] - (velocity + spin.cross(r)) * STEP;
        }
        let lengths = std::array::from_fn(|a| {
            std::array::from_fn(|b| Vec3::from_array(SHAPE[a]).distance(Vec3::from_array(SHAPE[b])))
        });
        Self {
            throw: *throw,
            points,
            previous,
            earlier: points,
            lengths,
            deflects: Vec::new(),
            steps: 0,
            still: 0,
            knock: 0.0,
            lowest: centre.z - FLOOR_DEPTH,
            reach: vec![0.0],
        }
    }

    /// The throw it began with.
    pub(crate) fn throw(&self) -> &Throw {
        &self.throw
    }

    /// Steps taken since the throw.
    pub(crate) fn steps(&self) -> u32 {
        self.steps
    }

    /// Whether it lies still (or has flown for [`MAX_STEPS`]).
    pub(crate) fn asleep(&self) -> bool {
        self.still >= SLEEP_STEPS || self.steps >= MAX_STEPS
    }

    /// Its centre now.
    pub(crate) fn centre(&self) -> Vec3 {
        mean(&self.points, &[0, 1, 2, 3, 4, 5, 6, 7])
    }

    /// How far its centre was from where it was thrown after `step` steps since the
    /// throw (where a bounce at `step` met the body); `None` before it got there.
    pub(crate) fn reach_at(&self, step: u32) -> Option<f32> {
        self.reach.get(usize::try_from(step).ok()?).copied()
    }

    /// How fast its centre went over the last step, units a second.
    pub(crate) fn velocity(&self) -> Vec3 {
        (self.centre() - mean(&self.earlier, &[0, 1, 2, 3, 4, 5, 6, 7])) / STEP
    }

    /// Add `deflect`; false (and not added) when its step has gone by: the flight must
    /// then run again from its throw ([`Flight::rerun`]).
    pub(crate) fn deflect(&mut self, deflect: Deflect) -> bool {
        if deflect.step < self.steps {
            return false;
        }
        if !self.deflects.contains(&deflect) {
            let at = self.deflects.partition_point(|d| d.step <= deflect.step);
            self.deflects.insert(at, deflect);
        }
        true
    }

    /// The flight from its throw again, with every bounce it took and `deflect`, not
    /// yet run (the next [`Flight::run_to`] fast-forwards it).
    pub(crate) fn rerun(&self, deflect: Deflect) -> Self {
        let mut flight = Self::new(&self.throw);
        flight.deflects.clone_from(&self.deflects);
        flight.deflect(deflect);
        flight
    }

    /// The flight from its throw again, with every bounce it took but `deflect` (one
    /// this client foresaw that did not come so), not yet run.
    pub(crate) fn without(&self, deflect: &Deflect) -> Self {
        let mut flight = Self::new(&self.throw);
        flight.deflects = self
            .deflects
            .iter()
            .filter(|taken| *taken != deflect)
            .copied()
            .collect();
        flight
    }

    /// Where it is shown `fraction` of the way through its last step: its centre and
    /// its turn (the model's +X its screen).
    pub(crate) fn pose(&self, fraction: f32) -> (Vec3, Quat) {
        let points: [Vec3; CORNERS] =
            std::array::from_fn(|c| self.earlier[c].lerp(self.points[c], fraction));
        let centre = mean(&points, &[0, 1, 2, 3, 4, 5, 6, 7]);
        // The same two lines through the corners (face to back, bottom to top) in its
        // own frame and in the world, each made a frame: the turn takes one to the other.
        let shape = SHAPE.map(Vec3::from_array);
        let own = frame(
            mean(&shape, &FRONT) - mean(&shape, &BACK),
            mean(&shape, &TOP) - mean(&shape, &BOTTOM),
        );
        let world = frame(
            mean(&points, &FRONT) - mean(&points, &BACK),
            mean(&points, &TOP) - mean(&points, &BOTTOM),
        );
        (
            centre,
            Quat::from_mat3(&(world * own.transpose())).normalize(),
        )
    }

    /// Step on to `steps` steps since the throw, against `world`. Asleep, the steps go
    /// by with it lying still, unless a bounce comes.
    pub(crate) fn run_to(&mut self, steps: u32, world: &mut impl World) {
        self.knock = 0.0;
        while self.steps < steps {
            let deflected = self.deflects.iter().any(|d| d.step == self.steps);
            if self.asleep() && !deflected {
                self.steps += 1;
                self.earlier = self.points;
            } else {
                self.step(world);
            }
            let reach = self.centre().distance(vector(self.throw.origin));
            self.reach.push(reach);
        }
    }

    /// The bounces of the step about to begin: the push, and a tumble the step sets.
    fn take_deflects(&mut self) {
        let step = self.steps;
        let seed = (self.throw.moment as u32) ^ step.wrapping_mul(0x2545_f491);
        for index in 0..self.deflects.len() {
            let deflect = self.deflects[index];
            if deflect.step != step {
                continue;
            }
            let centre = self.centre();
            let spin = Vec3::new(
                unit(seed, 21) * 2.0 - 1.0,
                unit(seed, 22) * 2.0 - 1.0,
                unit(seed, 23) * 2.0 - 1.0,
            ) * DEFLECT_SPIN;
            let push = vector(deflect.push);
            for corner in 0..CORNERS {
                let r = self.points[corner] - centre;
                self.previous[corner] -= (push + spin.cross(r)) * STEP;
            }
            self.still = 0;
        }
    }

    /// One step: the bounces due, then every corner moves on by its speed and gravity,
    /// out of the world's way, and the sticks hold it solid.
    fn step(&mut self, world: &mut impl World) {
        self.take_deflects();
        self.steps += 1;
        let began = self.points;
        self.earlier = began;
        let (low, high) = self
            .points
            .iter()
            .fold((self.points[0], self.points[0]), |(l, h), p| {
                (l.min(*p), h.max(*p))
            });
        world.at(
            self.throw
                .moment
                .wrapping_add((self.steps as i32).wrapping_mul(STEP_MS)),
            low,
            high,
        );
        let fall = Vec3::new(0.0, 0.0, -GRAVITY * STEP * STEP);
        for corner in 0..CORNERS {
            let at = self.points[corner];
            let speed = ((at - self.previous[corner]) * DRAG + fall).clamp_length_max(TOP_SPEED);
            let mut to = at + speed;
            to.z = to.z.max(self.lowest);
            self.previous[corner] = at;
            self.points[corner] = self.collide(corner, at, to, world);
        }
        let moved_to = self.points;
        for _ in 0..ROUNDS {
            for a in 0..CORNERS {
                for b in a + 1..CORNERS {
                    self.keep(a, b);
                }
            }
        }
        // The sticks may have pushed corners into the world: what is left slides along.
        let mut moved = 0.0_f32;
        for (corner, &from) in moved_to.iter().enumerate() {
            let to = self.points[corner];
            if let Some(hit) = world.trace(from, to, HALF) {
                let rest = to - hit.stop;
                let slide = rest - hit.normal * rest.dot(hit.normal);
                let base = hit.stop + hit.normal * 0.125;
                self.points[corner] = match world.trace(base, base + slide, HALF) {
                    None => base + slide,
                    Some(hit) => hit.stop + hit.normal * 0.125,
                };
                // No speed away from the surface for the way out (it would bounce).
                self.previous[corner] = self.points[corner]
                    - unbounced(self.points[corner] - self.previous[corner], hit.normal);
            }
            moved = moved.max(self.points[corner].distance(began[corner]));
        }
        if moved < STILL {
            self.still += 1;
        } else {
            self.still = 0;
        }
    }

    /// Where a corner going from `from` to `to` ends up: where it meets the world, its
    /// speed off the surface kept in part.
    fn collide(&mut self, corner: usize, from: Vec3, to: Vec3, world: &mut impl World) -> Vec3 {
        let Some(hit) = world.trace(from, to, HALF) else {
            return to;
        };
        let normal = hit.normal;
        let speed = to - from;
        let into = speed.dot(normal);
        let along = speed - normal * into;
        let hard = -into.min(0.0) / STEP;
        let back = if hard > BOUNCE_SPEED {
            (hard * BOUNCE).min(MAX_BOUNCE) * STEP
        } else {
            0.0
        };
        let after = friction(along, -into.min(0.0), FRICTION) * CONTACT_KEPT + normal * back;
        let at = hit.stop + normal * 0.125;
        self.previous[corner] = at - after;
        self.knock = self.knock.max(hard);
        at
    }

    /// Hold corners `a` and `b` at their distance, each moved half the way.
    fn keep(&mut self, a: usize, b: usize) {
        let gap = self.points[b] - self.points[a];
        let length = gap.length();
        if length < 1e-4 {
            return;
        }
        let shift = gap * ((length - self.lengths[a][b]) / length * 0.5);
        self.points[a] += shift;
        self.points[b] -= shift;
    }
}

/// The frame of `ahead` and `rise` made square: its first axis along `ahead`, its third
/// toward `rise`.
fn frame(ahead: Vec3, rise: Vec3) -> Mat3 {
    let forward = ahead.try_normalize().unwrap_or(Vec3::X);
    let up = (rise - forward * rise.dot(forward))
        .try_normalize()
        .unwrap_or(Vec3::Z);
    Mat3::from_cols(forward, up.cross(forward), up)
}

/// The mean of `points` at `corners`.
fn mean(points: &[Vec3; CORNERS], corners: &[usize]) -> Vec3 {
    corners
        .iter()
        .fold(Vec3::ZERO, |sum, corner| sum + points[*corner])
        / corners.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A floor at height 0, and a wall at x = 300 if asked.
    fn floor(wall: bool) -> impl FnMut(Vec3, Vec3, f32) -> Option<(Vec3, Vec3)> {
        move |from: Vec3, to: Vec3, half: f32| {
            let low = half;
            let face = 300.0 - half;
            let mut best: Option<(f32, Vec3)> = None;
            if to.z < low {
                let t = if from.z >= low {
                    ((from.z - low) / (from.z - to.z)).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                best = Some((t, Vec3::Z));
            }
            if wall && to.x > face {
                let t = if from.x <= face {
                    ((face - from.x) / (to.x - from.x)).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                if best.is_none_or(|(bt, _)| t < bt) {
                    best = Some((t, -Vec3::X));
                }
            }
            best.map(|(t, normal)| {
                let mut at = from + (to - from) * t;
                at.z = at.z.max(low);
                if wall {
                    at.x = at.x.min(face);
                }
                (at, normal)
            })
        }
    }

    fn throw(moment: i32) -> Throw {
        Throw {
            moment,
            origin: [0, 0, 60],
            velocity: [900, 100, 200],
        }
    }

    #[test]
    fn a_thrown_monitor_flies_lands_and_rests_on_a_face_in_one_piece() {
        let mut flight = Flight::new(&throw(1_000));
        let mut world = floor(false);
        flight.run_to(10, &mut world);
        assert!(flight.centre().x > 100.0, "it flew: {}", flight.centre());
        flight.run_to(MAX_STEPS, &mut world);
        assert!(flight.still >= SLEEP_STEPS, "it came to rest");
        assert!(
            flight.centre().z < 14.0 && flight.centre().z > 3.0,
            "{}",
            flight.centre()
        );
        for a in 0..CORNERS {
            assert!(flight.points[a].z >= HALF - 0.01, "{}", flight.points[a]);
            for b in a + 1..CORNERS {
                let length = flight.points[a].distance(flight.points[b]);
                assert!(
                    (length - flight.lengths[a][b]).abs() < 0.5,
                    "{a} {b} {length}"
                );
            }
        }
        // On a face: the lowest four corners are level.
        let mut heights: Vec<f32> = flight.points.iter().map(|p| p.z).collect();
        heights.sort_by(f32::total_cmp);
        assert!(heights[3] - heights[0] < 1.0, "{heights:?}");
    }

    #[test]
    fn landing_and_hitting_a_wall_are_heard_as_knocks() {
        let knocks = |wall| {
            let mut flight = Flight::new(&throw(1_000));
            let mut world = floor(wall);
            (1..=MAX_STEPS)
                .map(|step| {
                    flight.run_to(step, &mut world);
                    flight.knock
                })
                .filter(|knock| *knock > KNOCK_SPEED)
                .count()
        };
        assert!(knocks(false) > 0);
        assert!(knocks(true) > knocks(false));
    }

    /// The same in a map's brushes, as the game steps it (`ragdoll::Scene`).
    #[test]
    fn knocks_are_heard_against_a_maps_floor_and_wall() {
        use sjk_bsp::{Bsp, CollisionShader, box_brush, write_collision_map};
        let shaders = [CollisionShader {
            name: "textures/test/solid".into(),
            surface_flags: 0,
            content_flags: 1,
        }];
        let bsp = Bsp::parse(&write_collision_map(
            "{
\"classname\" \"worldspawn\"
}
",
            &shaders,
            &[
                box_brush([-2048.0, -2048.0, -64.0], [2048.0, 2048.0, 0.0], 0),
                box_brush([400.0, -2048.0, 0.0], [464.0, 2048.0, 512.0], 0),
            ],
        ))
        .expect("valid map");
        let mut scratch = sjk_bsp::TraceScratch::default();
        let mut movers = crate::ragdoll::Movers::default();
        let mut world = crate::ragdoll::Scene::new(&bsp, &mut scratch, None, &mut movers);
        let mut flight = Flight::new(&throw(1_000));
        let mut knocks = Vec::new();
        for step in 1..=MAX_STEPS {
            flight.run_to(step, &mut world);
            if flight.knock > KNOCK_SPEED {
                knocks.push((step, flight.knock as i32));
            }
        }
        assert!(knocks.len() >= 2, "{knocks:?}");
    }

    #[test]
    fn the_same_throw_flies_the_same_way_however_the_steps_are_taken() {
        let mut world = floor(true);
        let mut once = Flight::new(&throw(77));
        once.run_to(300, &mut world);
        let mut piecemeal = Flight::new(&throw(77));
        for steps in (0..=300).step_by(9) {
            piecemeal.run_to(steps, &mut world);
        }
        piecemeal.run_to(300, &mut world);
        assert_eq!(once.points, piecemeal.points);
        for point in once.points {
            assert!(point.x <= 300.0, "{point}");
        }
    }

    #[test]
    fn a_bounce_heard_late_gives_the_flight_one_heard_in_time() {
        let mut world = floor(false);
        let deflect = Deflect {
            step: 20,
            push: [-600, 0, 300],
        };
        let mut in_time = Flight::new(&throw(5));
        assert!(in_time.deflect(deflect));
        in_time.run_to(200, &mut world);
        let mut late = Flight::new(&throw(5));
        late.run_to(60, &mut world);
        assert!(!late.deflect(deflect));
        let mut late = late.rerun(deflect);
        late.run_to(200, &mut world);
        assert_eq!(in_time.points, late.points);
        // It went back.
        let mut straight = Flight::new(&throw(5));
        straight.run_to(200, &mut world);
        assert!(in_time.centre().x < straight.centre().x - 50.0);
    }

    /// Hub reports #216, #220: a hit heard a ping after its step runs the flight again
    /// from the hand, which must still tell how far the monitor flew to the body.
    #[test]
    fn the_reach_at_a_bounces_step_outlives_running_the_flight_again() {
        let mut world = floor(false);
        let deflect = Deflect {
            step: 20,
            push: [-600, 0, 300],
        };
        let mut flight = Flight::new(&throw(5));
        assert_eq!(flight.reach_at(0), Some(0.0));
        assert_eq!(flight.reach_at(1), None);
        flight.run_to(60, &mut world);
        let reach = flight.reach_at(deflect.step).expect("flown past the step");
        let origin = vector(throw(5).origin);
        let mut to_step = Flight::new(&throw(5));
        to_step.run_to(deflect.step, &mut world);
        assert_eq!(reach, to_step.centre().distance(origin));
        assert!(reach > 200.0, "{reach}");
        assert!(!flight.deflect(deflect));
        // Run again from the hand, its centre is back there: the reach is read before.
        let rerun = flight.rerun(deflect);
        assert!(rerun.centre().distance(origin) < 0.01);
        assert_eq!(rerun.reach_at(deflect.step), None);
        // Steps asleep still count their reach.
        flight.run_to(MAX_STEPS, &mut world);
        assert_eq!(flight.reach.len(), MAX_STEPS as usize + 1);
    }

    #[test]
    fn its_pose_faces_the_way_it_was_thrown() {
        let flight = Flight::new(&Throw {
            moment: 0,
            origin: [0, 0, 100],
            velocity: [0, 500, 0],
        });
        let (centre, turn) = flight.pose(1.0);
        assert!(centre.distance(Vec3::new(0.0, 0.0, 100.0)) < 0.01);
        assert!((turn * Vec3::X).distance(Vec3::Y) < 1e-3);
        assert!((turn * Vec3::Z).distance(Vec3::Z) < 1e-3);
    }
}
