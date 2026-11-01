//! Regression for the fixed standing hull during dual forward jump attacks.
use super::*;
use crate::pmove_anim::{AnimationLengthTable, AnimationTiming};
use std::sync::Arc;

struct Floor;
impl MovementCollision for Floor {
    fn trace(
        &self,
        start: [f32; 3],
        mins: [f32; 3],
        _: [f32; 3],
        end: [f32; 3],
        _: u32,
    ) -> MovementTrace {
        let a = start[2] + mins[2] - 0.125;
        let b = end[2] + mins[2] - 0.125;
        if b < a && b < 0.0 {
            let fraction = (a / (a - b)).max(0.0);
            MovementTrace {
                fraction,
                end_position: std::array::from_fn(|i| start[i] + (end[i] - start[i]) * fraction),
                plane_normal: [0.0, 0.0, 1.0],
                entity_number: ENTITY_NUMBER_WORLD,
                ..MovementTrace::miss(end)
            }
        } else {
            MovementTrace::miss(end)
        }
    }
}

#[test]
fn dual_forward_jump_landing_keeps_feet_on_floor() {
    for step in [8, 7, 4, 3] {
        for timer in [500, 600, 1500] {
            let mut predictor = Predictor::from_state(
                MovementState {
                    health: 100,
                    weapon: 3,
                    legs_anim: 861,
                    legs_timer: timer,
                    torso_timer: timer,
                    origin: [0.0, 0.0, 24.125],
                    velocity: [0.0, 0.0, -254.0],
                    gravity: 800.0,
                    standing_height: 40.0,
                    crouching_height: 16.0,
                    ground_entity_number: ENTITY_NUMBER_NONE,
                    ..MovementState::default()
                },
                MovementConfig::default(),
            );
            // Synthetic lengths, not retail animation data.
            predictor.set_animation_lengths(Arc::new(AnimationLengthTable::new(vec![
                AnimationTiming { frame_count: 200, frame_lerp_ms: 10 }; 862
            ])));
            predictor.predict_command(
                UserCommand {
                    server_time: step,
                    weapon: 3,
                    ..UserCommand::default()
                },
                &Floor,
            );
            assert_eq!(
                predictor.state().origin[2],
                24.125,
                "{step} ms, timer {timer}"
            );
            assert_eq!(
                predictor.state().velocity[2],
                0.0,
                "{step} ms, timer {timer}"
            );
            assert_eq!(predictor.state().ground_entity_number, ENTITY_NUMBER_WORLD);
        }
    }
}

/// A JA+ server with `jp_cinfo` 196819, as JA+ 2.4 B7 ships it.
fn ja_plus(old_collision: bool) -> MovementConfig {
    MovementConfig {
        ja_plus: crate::pmove_japlus::JaPlusRules {
            enabled: true,
            flip_kick: true,
            yellow_dfa: true,
            old_collision,
            ..Default::default()
        },
        ..MovementConfig::default()
    }
}

/// Open space with, optionally, a body that only the JA+ forward trace (feet at -48)
/// reaches. The other traces miss.
struct Ahead(Option<u16>);
impl MovementCollision for Ahead {
    fn trace(
        &self,
        start: [f32; 3],
        mins: [f32; 3],
        _: [f32; 3],
        end: [f32; 3],
        _: u32,
    ) -> MovementTrace {
        match self.0 {
            Some(entity_number) if mins[2] == -48.0 => MovementTrace {
                fraction: 0.5,
                end_position: std::array::from_fn(|i| (start[i] + end[i]) / 2.0),
                entity_number,
                ..MovementTrace::miss(end)
            },
            _ => MovementTrace::miss(end),
        }
    }
}

fn attacker(config: MovementConfig, timer: i32) -> Predictor {
    let mut predictor = Predictor::from_state(
        MovementState {
            client_num: 3,
            legs_anim: 861,
            legs_timer: timer,
            torso_timer: timer,
            standing_height: 40.0,
            ..MovementState::default()
        },
        config,
    );
    predictor.set_animation_lengths(lengths());
    predictor
}

/// Synthetic lengths, not retail animation data: 2000 ms for BOTH_JUMPATTACK6.
fn lengths() -> Arc<AnimationLengthTable> {
    Arc::new(AnimationLengthTable::new(vec![
        AnimationTiming {
            frame_count: 200,
            frame_lerp_ms: 10
        };
        862
    ]))
}

/// The windows of a 2000 ms attack in which the feet may rise.
fn in_window(timer: i32) -> bool {
    (1_450..=1_600).contains(&timer) || (400..=900).contains(&timer)
}

const TIMERS: [i32; 6] = [1_600, 1_450, 1_449, 900, 400, 399];

#[test]
fn ja_plus_raises_the_feet_only_over_a_player_ahead() {
    for step in [8, 7, 4, 3] {
        for timer in TIMERS.map(|timer| timer - step + 8) {
            for (ahead, raised) in [
                (None, false),
                (Some(5), in_window(timer)),
                // The attacker himself, and an NPC, are not players to sail over.
                (Some(3), false),
                (Some(40), false),
            ] {
                let mut predictor = attacker(ja_plus(false), timer);
                let minimum = predictor.dual_jump_minimum(&Ahead(ahead));
                assert_eq!(
                    minimum,
                    if raised { 0.0 } else { -24.0 },
                    "{step} ms, timer {timer}, {ahead:?}"
                );
                assert_eq!(predictor.state().movement_flags & 128 != 0, raised);
            }
        }
    }
}

#[test]
fn ja_plus_old_collision_raises_the_feet_in_every_window() {
    for step in [8, 7, 4, 3] {
        for timer in TIMERS.map(|timer| timer - step + 8) {
            let mut predictor = attacker(ja_plus(true), timer);
            let raised = in_window(timer);
            assert_eq!(
                predictor.dual_jump_minimum(&Ahead(None)),
                if raised { 0.0 } else { -24.0 },
                "{step} ms, timer {timer}"
            );
        }
    }
}

#[test]
fn other_servers_keep_the_fixed_hull_over_a_player() {
    for timer in TIMERS {
        let mut predictor = attacker(MovementConfig::default(), timer);
        assert_eq!(predictor.dual_jump_minimum(&Ahead(Some(5))), -24.0);
        assert_eq!(predictor.state().movement_flags & 128, 0);
    }
}

#[test]
fn ja_plus_dual_forward_jump_without_a_player_lands_on_the_floor() {
    // #307's capture: nobody in front, so the JA+ server kept the feet down and landed.
    for step in [8, 7, 4, 3] {
        for timer in [500, 600, 1500] {
            let mut predictor = Predictor::from_state(
                MovementState {
                    health: 100,
                    weapon: 3,
                    legs_anim: 861,
                    legs_timer: timer,
                    torso_timer: timer,
                    origin: [0.0, 0.0, 24.125],
                    velocity: [0.0, 0.0, -254.0],
                    gravity: 800.0,
                    standing_height: 40.0,
                    crouching_height: 16.0,
                    ground_entity_number: ENTITY_NUMBER_NONE,
                    ..MovementState::default()
                },
                ja_plus(false),
            );
            predictor.set_animation_lengths(lengths());
            predictor.predict_command(
                UserCommand {
                    server_time: step,
                    weapon: 3,
                    ..UserCommand::default()
                },
                &Floor,
            );
            assert_eq!(
                predictor.state().origin[2],
                24.125,
                "{step} ms, timer {timer}"
            );
            assert_eq!(predictor.state().ground_entity_number, ENTITY_NUMBER_WORLD);
        }
    }
}

/// Floor 6 units below the raised feet; `up` is how far the trace up gets.
struct Ledge {
    up: f32,
}
impl MovementCollision for Ledge {
    fn trace(
        &self,
        start: [f32; 3],
        _: [f32; 3],
        _: [f32; 3],
        end: [f32; 3],
        _: u32,
    ) -> MovementTrace {
        MovementTrace {
            fraction: if end[2] < start[2] { 0.25 } else { self.up },
            ..MovementTrace::miss(end)
        }
    }
}

#[test]
fn ja_plus_puts_raised_feet_back_or_crouches_under_a_ceiling() {
    for step in [8, 7, 4, 3] {
        for up in [1.0, 0.5] {
            let mut predictor = attacker(ja_plus(false), 200 - step);
            predictor.state_mut().movement_flags |= 128;
            assert_eq!(predictor.dual_jump_minimum(&Ledge { up }), -24.0);
            let state = predictor.state();
            assert_eq!(state.origin[2], if up == 1.0 { 18.0 } else { 24.0 });
            assert_eq!(state.movement_flags & 128, 0);
            assert_eq!(state.movement_flags & 1 != 0, up < 1.0);
            if up < 1.0 {
                assert_eq!((state.legs_timer, state.torso_timer), (0, 0));
            }
        }
    }
}
