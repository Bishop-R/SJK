//! `cg_predictLegacy`: each bit brings back 2026.1028.1's client prediction for one
//! change, and a server's own move never reads them.
use super::*;
use sjk_protocol::PlayerState;

const PMF_RESPAWNED: u16 = 512;

struct Floor;
impl MovementCollision for Floor {
    fn trace(
        &self,
        start: [f32; 3],
        _: [f32; 3],
        _: [f32; 3],
        end: [f32; 3],
        _: u32,
    ) -> MovementTrace {
        if end[2] < start[2] && start[2] <= 24.0 {
            MovementTrace {
                fraction: 0.0,
                end_position: start,
                plane_normal: [0.0, 0.0, 1.0],
                entity_number: ENTITY_NUMBER_WORLD,
                ..MovementTrace::miss(end)
            }
        } else {
            MovementTrace::miss(end)
        }
    }
}

struct Void;
impl MovementCollision for Void {
    fn trace(&self, _: [f32; 3], _: [f32; 3], _: [f32; 3], end: [f32; 3], _: u32) -> MovementTrace {
        MovementTrace::miss(end)
    }
}

fn respawned(config: MovementConfig) -> Predictor {
    Predictor::from_state(
        MovementState {
            health: 100,
            gravity: 800.0,
            origin: [0.0, 0.0, 24.0],
            standing_height: 40.0,
            crouching_height: 16.0,
            ground_entity_number: ENTITY_NUMBER_WORLD,
            movement_flags: PMF_RESPAWNED,
            ..MovementState::default()
        },
        config,
    )
}

fn jump_holding_attack(predictor: &mut Predictor, step: i32) -> f32 {
    predictor.predict_command(
        UserCommand {
            server_time: step,
            up_move: 127,
            buttons: 1,
            ..UserCommand::default()
        },
        &Floor,
    );
    predictor.state().velocity[2]
}

#[test]
fn the_respawn_jump_bit_jumps_before_attack_is_released_as_1028_did() {
    for step in [8, 7, 4, 3] {
        let current = jump_holding_attack(&mut respawned(MovementConfig::default()), step);
        assert_eq!(current, 0.0);
        let legacy = MovementConfig {
            predict_legacy: predict_legacy::RESPAWN_JUMP,
            ..MovementConfig::default()
        };
        assert!(jump_holding_attack(&mut respawned(legacy), step) > 0.0);
    }
}

#[test]
fn a_server_move_ignores_every_legacy_bit() {
    let server = MovementConfig {
        authoritative: true,
        predict_legacy: predict_legacy::ALL,
        ..MovementConfig::default()
    };
    assert_eq!(jump_holding_attack(&mut respawned(server), 8), 0.0);
}

#[test]
fn the_held_player_bit_lets_movement_through_while_held() {
    let moved = |bits| {
        let mut player = PlayerState::default();
        player.stats[0] = 100;
        player.set_raw_field(121, 2);
        let config = MovementConfig {
            predict_legacy: bits,
            ..MovementConfig::default()
        };
        let mut predictor = Predictor::from_player_state(&player, config);
        predictor.predict_command(
            UserCommand {
                server_time: 8,
                forward_move: 127,
                ..UserCommand::default()
            },
            &Void,
        );
        predictor.state().input_freeze_active
    };
    assert!(moved(0));
    assert!(!moved(predict_legacy::HELD_PLAYER));
}
