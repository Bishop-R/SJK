//! A toy's show over time: where it is at each moment, for the toy, the players round
//! it and the ring. A show from a hub of 30/10/2026 shrinks away, pops back and spins
//! for `spin` ([`legacy`]); one with steps (since 02/11/2026, `Show::steps`) goes on from
//! step to step after the pop, each turning the toy at its own speed and turning on or
//! off the players' dance, the ring and the floating ([`Track`]). Changes are eased: a
//! spin speeds up or slows over [`RAMP`], the dance and the ring come and go over
//! [`FADE`], the toy rises and lands over [`RISE`] and [`LAND`]. A show with a voice
//! (`Show::voice`, since 03/11/2026) spins only while its sound sings, starting and
//! stopping over [`VOICE_RAMP`].
//!
//! The turns are added up once per show, every [`SAMPLE`] milliseconds, and read back
//! between two samples, so a frame costs a lookup.

use glam::Vec3;
use sjk_identity::toy_master::Show;

/// How often the turns are added up, in milliseconds.
const SAMPLE: u32 = 20;
/// How long a change of spin takes, in milliseconds.
const RAMP: f32 = 700.0;
/// How long the toy takes to start or stop spinning as its sound starts or stops singing.
const VOICE_RAMP: f32 = 90.0;
/// How long the dance and the ring take to come or go, in milliseconds.
const FADE: f32 = 500.0;
/// How long the toy takes to rise as it floats, and to land when it stops.
const RISE: f32 = 1_500.0;
const LAND: f32 = 900.0;
/// How long a spin takes to reach its full speed in a show without steps; it slows to a
/// stop over its last quarter.
pub(super) const SPIN_UP: f32 = 1_000.0;
/// How high the toy hops as it spins.
const HOP: f32 = 1.5;
/// How high the toy floats, and how far it drifts and tumbles about.
const FLOAT_UP: f32 = 70.0;
const FLOAT_DRIFT: f32 = 60.0;
const FLOAT_TUMBLE: f32 = 40.0;
/// The ring turns this share of the toy's speed.
const RING_SHARE: f32 = 0.25;

/// What a show does at one moment.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Moment {
    /// How much the toy is grown (0 while it is gone).
    pub(crate) scale: f32,
    /// How far it turned (degrees), and its pitch and roll while it floats.
    pub(crate) turn: f32,
    pub(crate) pitch: f32,
    pub(crate) roll: f32,
    /// Where it is drawn from where it stands: its hop, and its drift as it floats.
    pub(crate) offset: Vec3,
    /// The players' dance: how far they turn (degrees) and how far they squish (0 to 1).
    pub(crate) dance: Option<(f32, f32)>,
    /// The ring: how far it has grown (0 to 1) and how far it turned (degrees).
    pub(crate) ring: Option<(f32, f32)>,
    /// The players float (the step says so).
    pub(crate) floating: bool,
}

/// A show's turns, added up, and its steps' flags, for [`Track::at`].
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Track {
    show: Show,
    /// The toy's, the dance's and the ring's turns every [`SAMPLE`] ms after the lead.
    turns: Vec<[f32; 3]>,
}

/// `u` eased in and out, from 0 to 1.
fn ease(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

impl Track {
    /// The track of `show`.
    pub(crate) fn new(show: &Show) -> Self {
        let mut track = Self {
            show: show.clone(),
            turns: Vec::new(),
        };
        if !show.steps.is_empty() {
            let samples = show.length / SAMPLE + 2;
            let mut sum = [0.0_f32; 3];
            track.turns.reserve(samples as usize);
            for index in 0..samples {
                track.turns.push(sum);
                let t = (index * SAMPLE) as f32 + SAMPLE as f32 / 2.0;
                let speed = track.speed(t) * 0.36 * SAMPLE as f32;
                sum[0] += speed;
                sum[1] += speed * track.level(t, |step| step.dance);
                sum[2] += speed * RING_SHARE;
            }
        }
        track
    }

    pub(crate) fn show(&self) -> &Show {
        &self.show
    }

    /// The step under way `t` ms after the lead, and the one before it.
    fn steps_at(&self, t: f32) -> (usize, Option<usize>) {
        let steps = &self.show.steps;
        let at = steps
            .iter()
            .rposition(|step| step.at as f32 <= t)
            .unwrap_or(0);
        (at, at.checked_sub(1))
    }

    /// `t` ms after the lead as its sound's time: the sound starts as the toy pops back,
    /// so the pop's length earlier, and the steps and the voice are timed on it.
    fn heard(&self, t: f32) -> f32 {
        t + self.show.pop as f32
    }

    /// The toy's spin `t` ms after the lead, in turns a second, eased from the last step's.
    fn speed(&self, t: f32) -> f32 {
        let t = self.heard(t);
        let (at, before) = self.steps_at(t);
        let step = &self.show.steps[at];
        let from = before.map_or(0.0, |before| self.show.steps[before].spin);
        if t < step.at as f32 {
            return 0.0;
        }
        let speed = from + (step.spin - from) * ease((t - step.at as f32) / RAMP);
        speed * self.singing(t)
    }

    /// How far the toy's sound sings `t` ms into it, from 0 to 1, eased in and out over
    /// [`VOICE_RAMP`]; always 1 for a show without a voice.
    fn singing(&self, t: f32) -> f32 {
        let voice = &self.show.voice;
        if voice.is_empty() {
            return 1.0;
        }
        let Some(&[from, to]) = voice.iter().rev().find(|span| span[0] as f32 <= t) else {
            return 0.0;
        };
        let (from, to) = (from as f32, to as f32);
        let up = ease((t.min(to) - from) / VOICE_RAMP);
        if t <= to {
            up
        } else {
            up * (1.0 - ease((t - to) / VOICE_RAMP))
        }
    }

    /// How far on `flag` is `t` ms after the lead, from 0 to 1, coming and going over
    /// [`FADE`].
    fn level(&self, t: f32, flag: impl Fn(&sjk_identity::toy_master::Step) -> bool) -> f32 {
        self.level_over(t, flag, FADE, FADE)
    }

    fn level_over(
        &self,
        t: f32,
        flag: impl Fn(&sjk_identity::toy_master::Step) -> bool,
        on: f32,
        off: f32,
    ) -> f32 {
        let t = self.heard(t);
        let (at, before) = self.steps_at(t);
        let step = &self.show.steps[at];
        if t < step.at as f32 {
            return 0.0;
        }
        let now = flag(step);
        let was = before.is_some_and(|before| flag(&self.show.steps[before]));
        let into = t - step.at as f32;
        match (was, now) {
            (true, true) => 1.0,
            (false, true) => ease(into / on),
            (true, false) => 1.0 - ease(into / off),
            (false, false) => 0.0,
        }
    }

    /// The turns `t` ms after the lead, read between two samples.
    fn turns_at(&self, t: f32) -> [f32; 3] {
        let place = (t / SAMPLE as f32).max(0.0);
        let index = (place as usize).min(self.turns.len().saturating_sub(2));
        let within = (place - index as f32).clamp(0.0, 1.0);
        let (a, b) = (self.turns[index], self.turns[index + 1]);
        std::array::from_fn(|n| a[n] + (b[n] - a[n]) * within)
    }

    /// The show `elapsed_ms` after its start; `None` once it is over.
    pub(crate) fn at(&self, elapsed_ms: f32) -> Option<Moment> {
        if self.show.steps.is_empty() || self.turns.len() < 2 {
            return legacy(&self.show, elapsed_ms);
        }
        let lead = self.show.lead() as f32;
        if elapsed_ms < lead {
            let pose = legacy(&self.show, elapsed_ms)?;
            return Some(Moment {
                turn: pose.turn,
                scale: pose.scale,
                ..Moment::default()
            });
        }
        let t = elapsed_ms - lead;
        if t >= self.show.length as f32 {
            return None;
        }
        let [turn, dance_turn, ring_turn] = self.turns_at(t);
        let speed = self.speed(t);
        let top = self
            .show
            .steps
            .iter()
            .map(|step| step.spin)
            .fold(0.5_f32, f32::max);
        // Two hops a turn, as high as the spin is fast.
        let hop = turn.to_radians().sin().abs() * (speed / top).clamp(0.0, 1.0);
        let float = self.level_over(t, |step| step.float, RISE, LAND);
        let seconds = t / 1_000.0;
        let drift = Vec3::new(
            (seconds * 0.37).sin() * FLOAT_DRIFT,
            (seconds * 0.29 + 1.0).cos() * FLOAT_DRIFT,
            FLOAT_UP + (seconds * 0.8).sin() * 15.0,
        ) * float;
        let dance = self.level(t, |step| step.dance);
        let ring = self.level(t, |step| step.ring);
        let (at, _) = self.steps_at(self.heard(t));
        Some(Moment {
            scale: 1.0,
            turn,
            pitch: (seconds * 0.5).sin() * FLOAT_TUMBLE * float,
            roll: (seconds * 0.43).cos() * FLOAT_TUMBLE * float,
            offset: drift + Vec3::Z * hop * HOP * (1.0 - float),
            dance: (dance > 0.0).then_some((dance_turn, hop * dance)),
            ring: (ring > 0.0).then_some((ring, ring_turn)),
            floating: self.show.steps[at].float,
        })
    }

    /// How long the show lasts in all, in milliseconds.
    pub(crate) fn length(&self) -> u32 {
        self.show.lead()
            + if self.show.steps.is_empty() {
                self.show.spin
            } else {
                self.show.length
            }
    }
}

/// A show without steps, `elapsed_ms` after its start: it shrinks away, stays gone, pops
/// back past its size and spins for `spin`, speeding up and slowing to a stop; the
/// players round it dance the whole spin (01/11/2026).
fn legacy(show: &Show, elapsed_ms: f32) -> Option<Moment> {
    let t = elapsed_ms;
    let (vanish, gone, pop, spin) = (
        show.vanish as f32,
        show.gone as f32,
        show.pop as f32,
        show.spin as f32,
    );
    let still = |scale: f32, turn: f32| {
        Some(Moment {
            scale,
            turn,
            ..Moment::default()
        })
    };
    if t < vanish {
        // Shrinking away, a quick turn as it goes.
        let u = t / vanish.max(1.0);
        return still(1.0 - ease(u), 180.0 * u * u);
    }
    let t = t - vanish;
    if t < gone {
        return still(0.0, 0.0);
    }
    let t = t - gone;
    if t < pop {
        // Back, overshooting (an ease out with a bounce past 1).
        let u = (t / pop.max(1.0)).clamp(0.0, 1.0) - 1.0;
        return still(1.0 + 2.7 * u * u * u + 1.7 * u * u, 0.0);
    }
    let t = t - pop;
    if t >= spin {
        return None;
    }
    let turn = spin_turn(show.turns, spin, t);
    let hop = ((t / 1_000.0) * std::f32::consts::TAU * show.turns.max(0.5))
        .sin()
        .abs();
    Some(Moment {
        scale: 1.0,
        turn,
        offset: Vec3::Z * hop * HOP,
        dance: Some((turn, hop * spin_speed(spin, t))),
        ..Moment::default()
    })
}

/// How far a spin of `spin` milliseconds at up to `turns` turns a second turned, `t`
/// milliseconds into it, in degrees: speeding up over [`SPIN_UP`], at full speed, then
/// slowing to a stop over its last quarter.
pub(crate) fn spin_turn(turns: f32, spin: f32, t: f32) -> f32 {
    let top = turns * 360.0 / 1_000.0;
    let up = SPIN_UP.min(spin / 4.0);
    let down_from = (spin * 0.75).max(up);
    let down = (spin - down_from).max(1.0);
    let t = t.clamp(0.0, spin);
    if t < up {
        return top * t * t / (2.0 * up);
    }
    let cruise = top * up / 2.0;
    if t < down_from {
        return cruise + top * (t - up);
    }
    let into = t - down_from;
    cruise + top * (down_from - up) + top * (into - into * into / (2.0 * down))
}

/// How fast a spin of `spin` milliseconds goes `t` milliseconds into it, from 0 to 1 (its
/// full speed), as [`spin_turn`] turns it.
pub(crate) fn spin_speed(spin: f32, t: f32) -> f32 {
    let up = SPIN_UP.min(spin / 4.0);
    let down_from = (spin * 0.75).max(up);
    let down = (spin - down_from).max(1.0);
    if t < 0.0 || t >= spin {
        0.0
    } else if t < up {
        t / up.max(1.0)
    } else if t < down_from {
        1.0
    } else {
        1.0 - (t - down_from) / down
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use sjk_identity::toy_master::Step;

    /// A show in steps, as a hub sends one: every kind of step, timed on a sound.
    pub(crate) fn stepped() -> Show {
        let step = |at: u32| Step {
            at,
            ..Step::default()
        };
        Show {
            vanish: 180,
            gone: 1_200,
            pop: 450,
            spin: 30_000,
            turns: 3.0,
            steps: vec![
                Step {
                    spin: 3.0,
                    ..step(0)
                },
                Step {
                    spin: 1.5,
                    ..step(28_500)
                },
                Step {
                    spin: 3.0,
                    ..step(30_000)
                },
                Step {
                    spin: 3.0,
                    dance: true,
                    ring: true,
                    ..step(34_000)
                },
                Step {
                    ring: true,
                    ..step(60_000)
                },
                Step {
                    ring: true,
                    float: true,
                    ..step(62_000)
                },
                Step {
                    spin: 3.0,
                    dance: true,
                    ring: true,
                    float: true,
                    ..step(89_000)
                },
                step(100_000),
                Step {
                    spin: 3.0,
                    ..step(104_000)
                },
                Step {
                    spin: 3.0,
                    spit: true,
                    ..step(108_000)
                },
                Step {
                    spin: 1.5,
                    ..step(131_000)
                },
                step(133_000),
            ],
            length: 135_000,
            reach: 400.0,
            ring: "models/r.md3".to_owned(),
            ring_radius: 100.0,
            voice: Vec::new(),
        }
    }

    const LEAD: f32 = 1_830.0;

    /// The toy spins alone at first, the players dance and the ring shows from the drop,
    /// all hold at 60 s, the players float from 62 s, and the show ends with the song.
    #[test]
    fn a_show_goes_step_by_step() {
        let track = Track::new(&stepped());
        let at = |seconds: f32| track.at(LEAD + seconds * 1_000.0);
        let first = at(10.0).unwrap();
        assert!(first.dance.is_none() && first.ring.is_none() && !first.floating);
        let speed = |seconds: f32| at(seconds + 0.01).unwrap().turn - at(seconds).unwrap().turn;
        assert!((speed(10.0) - 3.0 * 3.6).abs() < 0.05, "{}", speed(10.0));
        let drop = at(40.0).unwrap();
        let (grown, _) = drop.ring.unwrap();
        assert_eq!(grown, 1.0);
        assert!(drop.dance.is_some());
        // The players turn as fast as the toy once the dance is on.
        let dance = |seconds: f32| at(seconds).unwrap().dance.unwrap().0;
        assert!((dance(40.01) - dance(40.0) - speed(40.0)).abs() < 0.05);
        // The hold: nothing turns, the ring stays.
        assert!(speed(61.0).abs() < 0.01);
        assert!(at(61.0).unwrap().ring.is_some() && at(61.0).unwrap().dance.is_none());
        let floating = at(70.0).unwrap();
        assert!(floating.floating && floating.offset.z > 40.0);
        assert!(at(95.0).unwrap().dance.is_some() && at(95.0).unwrap().floating);
        // Landed and alone again.
        let alone = at(105.0).unwrap();
        assert!(!alone.floating && alone.offset.z < 2.0 && alone.ring.is_none());
        assert!(at(134.9).is_some() && at(135.0).is_none());
        assert_eq!(track.length(), 1_830 + 135_000);
    }

    /// A spin's change of speed is eased, never a jump, and the turn never goes back.
    #[test]
    fn the_turn_eases_and_never_goes_back() {
        let track = Track::new(&stepped());
        let mut last = 0.0;
        for ms in (0..135_000).step_by(37) {
            let turn = track.at(LEAD + ms as f32).unwrap().turn;
            assert!(turn >= last - 1e-3, "{ms}");
            assert!(turn - last < 3.0 * 0.36 * 37.0 + 0.5, "{ms}");
            last = turn;
        }
    }

    /// With a voice, the toy spins only while its sound sings: still between, easing in
    /// and out quickly, and at its step's speed within.
    #[test]
    fn a_show_with_a_voice_spins_only_while_it_sings() {
        let show = Show {
            voice: vec![[10_000, 11_000], [11_300, 12_000]],
            ..stepped()
        };
        let track = Track::new(&show);
        // Times in the sound, which starts as the toy pops back (450 ms before the lead).
        let turn = |ms: f32| track.at(LEAD - 450.0 + ms).unwrap().turn;
        let speed = |ms: f32| (turn(ms + 10.0) - turn(ms)) / 10.0;
        assert!(speed(9_000.0).abs() < 1e-4);
        assert!(
            (speed(10_500.0) - 3.0 * 0.36).abs() < 0.01,
            "{}",
            speed(10_500.0)
        );
        assert!(speed(11_200.0).abs() < 1e-4);
        assert!(speed(11_600.0) > 1.0);
        assert!(speed(13_000.0).abs() < 1e-4);
        // Starting and stopping are eased, not jumps.
        assert!(speed(10_030.0) > 0.0 && speed(10_030.0) < 1.0);
        assert!(speed(11_030.0) > 0.0 && speed(11_030.0) < 1.0);
        // No voice: it spins all through its steps, as before.
        let plain = Track::new(&stepped());
        assert!(plain.at(LEAD + 9_000.0).unwrap().turn > 0.0);
    }

    /// A show without steps plays as before: gone, back, spinning for `spin`, dancing.
    #[test]
    fn a_show_without_steps_spins_for_its_spin() {
        let show = Show {
            steps: Vec::new(),
            ..stepped()
        };
        let track = Track::new(&show);
        assert_eq!(track.at(700.0).unwrap().scale, 0.0);
        assert!(track.at(LEAD + 5_000.0).unwrap().dance.is_some());
        assert_eq!(track.at(LEAD + 30_000.0), None);
        assert_eq!(track.length(), 1_830 + 30_000);
    }
}
