use super::*;
use crate::pmove_japro::{JaProRules, JaProState};
include!("pmove_japro_jump_fixtures.rs");

fn mover(style: u32) -> Predictor {
    let mut player = PlayerState::default();
    player.stats[0] = 100;
    player.stats[11] = 1;
    player.stats[13] = style;
    player.set_origin([0.0, 0.0, 24.0]);
    player.set_ground_entity_num(ENTITY_NUMBER_WORLD);
    player.set_gravity(800);
    player.set_speed(320.0);
    player.set_base_speed(320);
    Predictor::from_player_state(
        &player,
        MovementConfig {
            snap_velocity: false,
            ja_pro: JaProRules {
                enabled: true,
                ..Default::default()
            },
            ..Default::default()
        },
    )
}

#[test]
fn japro_jump_impulses_match_compiled_taystjk() {
    for &(input, velocity, timer, last_speed) in TAKEOFF {
        let [style, sample, superjump] = input;
        let mut predictor = mover(style as u32);
        predictor.state.velocity = [400.0, 0.0, 15.0];
        predictor.state.ja_pro.jump_time = if sample == 0 { 0 } else { 50 };
        predictor.state.ja_pro.last_jump_speed = 77;
        predictor.state.ja_pro.restrictions = if superjump != 0 { 16 } else { 0 };
        let mut ground = GroundState::air(MovementTrace::miss([0.0; 3]));
        ground.trace.plane_normal = match sample {
            0 => [0.0, 0.0, 1.0],
            1 => [-0.6, 0.0, 0.8],
            _ => [0.6, 0.0, 0.8],
        };
        assert!(predictor.japro_jump_takeoff(&ground));
        assert!(
            (predictor.state.velocity[2] - f32::from_bits(velocity)).abs() < 0.0001,
            "{input:?}"
        );
        assert_eq!(
            (
                predictor.state.ja_pro.jump_time,
                predictor.state.ja_pro.last_jump_speed
            ),
            (timer, last_speed),
            "{input:?}"
        );
    }
}

#[test]
fn japro_jump_gates_match_compiled_taystjk() {
    for &(input, allowed, up) in GATE {
        let [style, sample] = input;
        let mut predictor = mover(style as u32);
        predictor.state.ja_pro.jump_time = if sample == 1 { 376 } else { 0 };
        predictor.state.ground_entity_number = if sample == 3 {
            ENTITY_NUMBER_NONE
        } else {
            ENTITY_NUMBER_WORLD
        };
        predictor.state.movement_flags = if matches!(sample, 0 | 3) {
            PMF_JUMP_HELD
        } else {
            0
        };
        let mut command = UserCommand {
            up_move: if sample >= 2 { 9 } else { 127 },
            ..Default::default()
        };
        assert_eq!(
            !predictor.japro_jump_blocked(&mut command),
            allowed,
            "{input:?}"
        );
        assert_eq!(i32::from(command.up_move), up, "{input:?}");
    }
}

fn rising(style: u32, height: f32, last_speed: i32) -> Predictor {
    let mut predictor = mover(style);
    predictor.state.levitation_level = 3;
    predictor.state.force_power = 100;
    predictor.state.force_jump_start_height = 1000.0;
    predictor.state.origin[2] = 1000.0 + height;
    predictor.state.ground_entity_number = ENTITY_NUMBER_NONE;
    predictor.state.movement_flags = PMF_JUMP_HELD;
    predictor.state.velocity[2] = 100.0;
    predictor.state.ja_pro.last_jump_speed = last_speed;
    predictor
}

#[test]
fn japro_force_rise_policy_and_ramp_curves_match_taystjk() {
    for &(style, allowed) in FORCE_RISE {
        let mut predictor = rising(style, 16.0, 225);
        let mut command = UserCommand {
            server_time: 8,
            up_move: 127,
            ..Default::default()
        };
        assert_eq!(
            predictor.check_force_jump(&mut command, true),
            allowed,
            "style {style}"
        );
    }
    for &(input, velocity) in CURVE {
        let [style, last_speed, height] = input;
        let mut predictor = rising(style as u32, height as f32, last_speed);
        let mut command = UserCommand {
            server_time: 8,
            up_move: 127,
            ..Default::default()
        };
        assert!(predictor.check_force_jump(&mut command, true));
        assert!(
            (predictor.state.velocity[2] - f32::from_bits(velocity)).abs() < 0.0001,
            "{input:?}"
        );
    }
}

#[test]
fn japro_only_bhop_and_timers_match_compiled_taystjk() {
    for &(input, expected) in BHOP {
        let [enabled, forced, optional, player, restricted] = input;
        let rules = JaProRules {
            enabled: enabled != 0,
            forced_bhop: forced != 0,
            optional_bhop: optional != 0,
            player_bhop: player != 0,
            ..Default::default()
        };
        let state = JaProState {
            restrictions: restricted as u32,
            ..Default::default()
        };
        assert_eq!(rules.only_bhop(state), expected, "{input:?}");
    }
    for &(input, expected) in TIMER {
        let [enabled, timer, millis] = input;
        let mut predictor = mover(1);
        predictor.config.ja_pro.enabled = enabled != 0;
        predictor.state.ja_pro.jump_time = timer;
        predictor.drop_timers(millis);
        assert_eq!(predictor.state.ja_pro.jump_time, expected, "{input:?}");
    }
}

#[test]
fn japro_only_bhop_preference_never_enables_it_on_a_foreign_server() {
    let mut game = sjk_protocol::GameState::empty_local(0);
    let mut config = MovementConfig::default();
    config.ja_pro.player_bhop = true;
    for (name, enabled) in [
        ("japro", true),
        ("basejka", false),
        ("JA+ Mod", false),
        ("lugormod", false),
        ("TaystJK", true),
    ] {
        game.replace_config_string(
            0,
            format!("\\gamename\\{name}\\jcinfo\\67108864").into_bytes(),
        )
        .unwrap();
        config.refresh_game_state(&game);
        assert!(config.ja_pro.player_bhop);
        assert_eq!(
            config.ja_pro.only_bhop(JaProState::default()),
            enabled,
            "{name}"
        );
        let mut predictor = rising(1, 64.0, 225);
        predictor.set_config(config);
        let mut command = UserCommand {
            server_time: 8,
            up_move: 127,
            ..Default::default()
        };
        assert!(predictor.check_force_jump(&mut command, true));
        assert_eq!(command.up_move == 0, enabled, "{name}");
        assert_eq!(
            predictor.state.force_powers_active & FORCE_LEVITATION_BIT == 0,
            enabled,
            "{name}"
        );
    }
}

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
        if end[2] + mins[2] < 0.0 {
            let fraction = ((start[2] + mins[2]) / (start[2] - end[2])).clamp(0.0, 1.0);
            MovementTrace {
                fraction,
                end_position: [end[0], end[1], -mins[2]],
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
fn japro_held_jump_repeats_only_for_the_selected_style_and_server() {
    for millis in [8, 7, 4, 3] {
        for (style, enabled, expected) in [
            (3, true, 2),
            (4, true, 2),
            (6, true, 2),
            (16, true, 1),
            (1, true, 1),
            (3, false, 1),
        ] {
            let mut predictor = mover(style);
            predictor.config.ja_pro.enabled = enabled;
            let mut jumps = 0;
            for time in (millis..=1000).step_by(millis as usize) {
                let grounded = predictor.state.ground_entity_number != ENTITY_NUMBER_NONE;
                predictor.predict_command(
                    UserCommand {
                        server_time: time,
                        up_move: 127,
                        ..Default::default()
                    },
                    &Floor,
                );
                if grounded && predictor.state.ground_entity_number == ENTITY_NUMBER_NONE {
                    jumps += 1;
                }
            }
            assert_eq!(jumps, expected, "{style}, enabled {enabled}, {millis} ms");
        }
    }
}

#[test]
fn japro_zero_gravity_does_not_add_automatic_push_impulses() {
    // Zero-gravity push-off remains unsupported for every style; a transient
    // zero-gravity spawn snapshot must not inject horizontal speed.
    for millis in [8, 7, 4, 3] {
        for style in 0..19 {
            for held in [false, true] {
                let mut predictor = mover(style);
                predictor.state.gravity = 0.0;
                predictor.state.origin[2] = 200.0;
                predictor.state.ground_entity_number = ENTITY_NUMBER_NONE;
                predictor.state.movement_flags = if held { PMF_JUMP_HELD } else { 0 };
                let mut command = UserCommand {
                    server_time: millis,
                    up_move: 127,
                    ..Default::default()
                };
                let mut ground = GroundState::air(MovementTrace::miss(predictor.state.origin));
                predictor.check_jump(
                    &mut command,
                    Bounds {
                        minimums: [-15.0, -15.0, -24.0],
                        maximums: [15.0, 15.0, 40.0],
                    },
                    &mut ground,
                    &Floor,
                    &MoveContext::CLIENT,
                );
                assert_eq!(
                    predictor.state.velocity[0], 0.0,
                    "style {style}, held {held}, {millis} ms"
                );
            }
        }
    }
}
