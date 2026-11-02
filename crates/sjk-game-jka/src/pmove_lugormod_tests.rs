use super::*;
use crate::pmove_lugormod::LugormodRules;
use sjk_protocol::{GameState, PlayerState};
include!("pmove_lugormod_fixtures.rs");

fn mover() -> Predictor {
    Predictor::from_state(
        MovementState {
            health: 100,
            gravity: 800.0,
            standing_height: 40.0,
            crouching_height: 16.0,
            view_height: 26,
            speed: 250.0,
            origin: [0.0, 0.0, 512.0],
            ground_entity_number: ENTITY_NUMBER_NONE,
            ..Default::default()
        },
        MovementConfig {
            lugormod: LugormodRules { enabled: true },
            ..Default::default()
        },
    )
}

#[test]
fn lugormod_bounds_match_compiled_default_build() {
    let poses = [
        0,
        crate::legacy_animation_index("BOTH_MEDITATE").unwrap() as u16,
        crate::legacy_animation_index("BOTH_CROUCH3").unwrap() as u16,
        crate::legacy_animation_index("BOTH_SIT6").unwrap() as u16,
    ];
    for &(input, mins, maxs, eye) in BOUNDS {
        let [scale, mode, flags, pose] = input;
        let mut p = mover();
        p.state.lugormod.model_scale_percent = scale;
        p.state.movement_type = mode as u8;
        p.state.movement_flags = flags as u16;
        p.state.legs_anim = poses[pose as usize];
        p.state.legs_timer = 100;
        let bounds = p.lugormod_bounds();
        p.lugormod_finish_slice();
        assert_eq!(bounds.minimums.map(f32::to_bits), mins, "{input:?}");
        assert_eq!(bounds.maximums.map(f32::to_bits), maxs, "{input:?}");
        assert_eq!(p.state.view_height, eye, "{input:?}");
    }
}

struct Distance(f32);
impl MovementCollision for Distance {
    fn trace(
        &self,
        start: [f32; 3],
        _: [f32; 3],
        _: [f32; 3],
        mut end: [f32; 3],
        _: u32,
    ) -> MovementTrace {
        end[2] = start[2] - self.0;
        MovementTrace {
            fraction: self.0 / 4096.0,
            ..MovementTrace::miss(end)
        }
    }
}

#[test]
fn lugormod_jetpack_matches_compiled_thrust_control_and_pose() {
    for &(input, velocity, flags, pose, wish) in JETPACK {
        let [distance, vz, admin, gametype, up, forward, right] = input;
        let mut p = mover();
        p.state.movement_type = 1;
        p.state.lugormod.admin_jetpack = admin != 0;
        p.gametype = gametype;
        p.state.velocity[2] = vz as f32;
        p.state.entity_flags = 1 << 30;
        let command = UserCommand {
            up_move: up as i8,
            forward_move: forward as i8,
            right_move: right as i8,
            ..Default::default()
        };
        let bounds = p.lugormod_bounds();
        p.jetpack_move(&command, bounds, &Distance(distance as f32));
        assert_eq!(p.state.velocity[2].to_bits(), velocity, "{input:?}");
        assert_eq!(p.state.entity_flags, flags, "{input:?}");
        assert_eq!(p.state.legs_anim, pose, "{input:?}");
        let factor = p.jetpack_air_control(&command);
        let wish_velocity =
            (Vec3::X * forward as f32 + Vec3::new(0.0, -1.0, 0.0) * right as f32) * factor;
        assert_eq!(
            [wish_velocity.x.to_bits(), wish_velocity.y.to_bits()],
            wish,
            "{input:?}"
        );
    }
}

struct Stand {
    fraction: f32,
    all: bool,
    start: bool,
}
impl MovementCollision for Stand {
    fn trace(
        &self,
        start: [f32; 3],
        mins: [f32; 3],
        maxs: [f32; 3],
        end: [f32; 3],
        _: u32,
    ) -> MovementTrace {
        assert_eq!(mins[2], -24.0);
        assert_eq!(maxs, [-mins[0], -mins[1], 0.0]);
        assert_eq!(end, [start[0], start[1], start[2] + 40.0]);
        MovementTrace {
            fraction: self.fraction,
            all_solid: self.all,
            start_solid: self.start,
            ..MovementTrace::miss(end)
        }
    }
}

#[test]
fn lugormod_stand_sweep_matches_compiled_bevel_rule() {
    for &(input, expected) in STAND {
        let [scale, all, start, clear] = input;
        let mut p = mover();
        p.state.lugormod.model_scale_percent = scale;
        let bounds = p.lugormod_bounds();
        assert_eq!(
            p.lugormod_can_stand(
                bounds,
                &Stand {
                    fraction: if clear != 0 { 1.0 } else { 0.5 },
                    all: all != 0,
                    start: start != 0
                }
            ),
            expected,
            "{input:?}"
        );
    }
}

struct Void;
impl MovementCollision for Void {
    fn trace(&self, _: [f32; 3], _: [f32; 3], _: [f32; 3], end: [f32; 3], _: u32) -> MovementTrace {
        MovementTrace::miss(end)
    }
}

fn game(name: &str) -> GameState {
    let mut game = GameState::empty_local(0);
    game.replace_config_string(
        0,
        format!("\\gamename\\{name}\\protocol\\26\\jcinfo\\2097184").into_bytes(),
    )
    .unwrap();
    game.replace_config_string(1, b"\\pmove_float\\1".to_vec())
        .unwrap();
    game
}

#[test]
fn lugormod_commands_follow_current_profile_and_reseed_at_each_step() {
    for step in [8, 7, 4, 3] {
        let mut p = mover();
        let mut config = MovementConfig::default();
        let mut seed = PlayerState::default();
        seed.stats[0] = 100;
        seed.set_gravity(800);
        seed.set_speed(250.0);
        seed.set_base_speed(250);
        seed.set_origin([0.0, 0.0, 512.0]);
        seed.set_velocity([0.0, 0.0, 20.0]);
        seed.set_ground_entity_num(ENTITY_NUMBER_NONE);
        seed.set_movement_type(1);
        seed.set_raw_field(123, 150);
        seed.set_raw_field(125, 1);
        for name in [
            "^5L^7ugormod ^5v3",
            "basejka",
            "JA+ Mod",
            "japro",
            "Lugormod U#",
            "lugormod",
            "Lugormod v3",
            "basejka",
        ] {
            config.refresh_game_state(&game(name));
            let enabled = sjk_protocol::is_lugormod_v3_game_name(name);
            assert_eq!(config.lugormod.enabled, enabled, "{name}");
            p.set_config(config);
            p = p.reseeded(&seed);
            let command = UserCommand {
                server_time: step,
                forward_move: 127,
                up_move: 127,
                ..Default::default()
            };
            p.predict_command(command, &Void);
            assert_eq!(
                p.box_bounds().0[0],
                if enabled { -22.5 } else { -15.0 },
                "{name}"
            );
            assert_eq!(p.state.view_height, if enabled { 54 } else { 36 }, "{name}");
            let seconds = frame_seconds(step);
            let expected_x = if enabled {
                (1250.0 * seconds).round_ties_even()
            } else {
                500.0 * seconds
            };
            let expected_z = if enabled {
                (22.0 - 200.0 * seconds).round_ties_even()
            } else {
                20.0 - 200.0 * seconds
            };
            assert!(
                (p.state.velocity[0] - expected_x).abs() < 0.0001,
                "{name}, {step}: {:?}",
                p.state.velocity
            );
            assert!(
                (p.state.velocity[2] - expected_z).abs() < 0.0001,
                "{name}, {step}: {:?}",
                p.state.velocity
            );

            let mut preview = p.clone();
            preview.predict_command(
                UserCommand {
                    server_time: step * 2,
                    ..command
                },
                &Void,
            );
            assert_eq!(preview.config.lugormod.enabled, enabled);
            assert_eq!(preview.state.view_height, if enabled { 54 } else { 36 });
            assert_eq!(config.snap_velocity, enabled, "{name}");
        }
    }
}

#[test]
fn lugormod_grapple_and_unimplemented_force_curves_use_snapshots_only_on_v3() {
    for name in ["Lugormod v3", "japro", "JA+ Mod", "basejka", "Lugormod U#"] {
        let rules = LugormodRules::from_game_state(&game(name));
        let mut p = PlayerState::default();
        p.set_movement_type(9);
        assert_eq!(rules.needs_snapshot(&p), name == "Lugormod v3");
        p.set_movement_type(0);
        assert!(!rules.needs_snapshot(&p));
        p.stats[11] = 2;
        assert_eq!(rules.needs_snapshot(&p), name == "Lugormod v3");
        p.stats[11] = 0;
        p.set_raw_field(123, 150);
        p.set_raw_field(82, 2);
        assert_eq!(rules.needs_snapshot(&p), name == "Lugormod v3");
    }
}

#[test]
#[ignore = "release-only Lugormod command-cost measurement"]
fn measure_lugormod_commands() {
    assert!(!cfg!(debug_assertions));
    for name in ["basejka", "Lugormod v3"] {
        let mut p = mover();
        p.set_config(MovementConfig::from_game_state(&game(name)));
        let start = std::time::Instant::now();
        for n in 1..=100_000 {
            p.predict_command(
                std::hint::black_box(UserCommand {
                    server_time: n * 8,
                    forward_move: 127,
                    right_move: 64,
                    ..Default::default()
                }),
                &Void,
            );
        }
        std::hint::black_box(p.state());
        println!(
            "{name}: {:.2} ns/command",
            start.elapsed().as_nanos() as f64 / 100_000.0
        );
    }
}

#[test]
fn lugormod_baton_roll_requires_current_dialect_and_public_force_eligibility() {
    for step in [8, 7, 4, 3] {
        for name in ["Lugormod v3", "basejka", "JA+ Mod", "japro"] {
            let rules = MovementConfig::from_game_state(&game(name)).roll_rules;
            for weapon in 0..8 {
                for eligible in [true, false] {
                    let mut p = mover();
                    p.state.weapon = weapon;
                    p.state.force_restricted = !eligible;
                    p.state.lugormod.model_scale_percent = 150;
                    let rolled = crate::pmove_roll::try_roll(
                        &mut p.state,
                        &UserCommand {
                            server_time: step,
                            forward_move: 127,
                            ..Default::default()
                        },
                        &Void,
                        rules,
                    );
                    assert_eq!(
                        rolled.is_some(),
                        eligible
                            && (matches!(weapon, 2 | 3) || name == "Lugormod v3" && weapon == 1),
                        "{name}, weapon {weapon}, {eligible}"
                    );
                }
            }
        }
    }
}
