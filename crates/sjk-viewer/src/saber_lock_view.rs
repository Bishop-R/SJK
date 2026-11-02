//! CG_PredictPlayerState's opponent-facing step during a saber lock.

use sjk_client::pmove::MovementState;
use sjk_protocol::Snapshot;

/// The displayed snapshot supplies the anchor; the opponent uses its presented origin.
#[derive(Default)]
pub(super) struct LockView {
    until: i32,
    enemy: u16,
    client: u16,
    origin: [f32; 3],
    /// `cg_predictLegacy` [`SABER_LOCK_FACING`](sjk_client::pmove::predict_legacy): no facing.
    pub(super) off: bool,
}

impl LockView {
    pub(super) fn new(snapshot: Option<&Snapshot>) -> Self {
        let mut result = Self::default();
        if let Some(snapshot) = snapshot {
            result.update(snapshot);
        }
        result
    }

    pub(super) fn update(&mut self, snapshot: &Snapshot) {
        self.until = snapshot.player.saber_lock_time();
        self.enemy = snapshot.player.raw_field(110).unwrap_or(0) as u16;
        self.client = snapshot.player.client_num();
        self.origin = snapshot.player.origin();
    }

    pub(super) fn enemy(&self) -> u16 {
        self.enemy
    }

    /// Before Pmove, its input freeze absorbs these angles into delta_angles.
    /// On display-only frames, the existing view-lock flag also cancels fresh mouse input.
    pub(super) fn face(&self, state: &mut MovementState, time: i32, target: Option<[f32; 3]>) {
        if self.off
            || self.until <= time
            || state.saber_lock_time <= time
            || state.saber_lock_time < state.command_time
            || self.client != state.client_num
        {
            return;
        }
        let Some(target) = target else {
            return;
        };
        let direction = std::array::from_fn(|axis| target[axis] - self.origin[axis]);
        let (pitch, yaw) = sjk_game_jka::damage::vector_to_angles(direction);
        // A display-only frame still needs PM_SetPMViewAngle/PM_UpdateViewAngles'
        // short-angle wrap and pitch clamp (vectoangles can return -315 degrees).
        let short = |angle: f32| (angle * 65536.0 / 360.0) as i32 as i16;
        let pitch = short(pitch).clamp(-16000, 16000);
        state.view_angles = [
            f32::from(pitch) * (360.0 / 65536.0),
            f32::from(short(yaw)) * (360.0 / 65536.0),
            0.0,
        ];
        state.wall_view_locked = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lock_faces_the_presented_opponent_even_without_another_command() {
        // codemp/cgame/cg_predict.c:1252-1263 uses cg.snap->ps.origin, not predicted origin.
        for step in [8, 7, 4, 3] {
            let lock = LockView {
                until: 1000,
                enemy: 2,
                client: 1,
                origin: [10.0, 20.0, 30.0],
                off: false,
            };
            let mut state = MovementState {
                client_num: 1,
                command_time: 100 + step,
                saber_lock_time: 1000,
                origin: [500.0, 500.0, 500.0],
                ..Default::default()
            };
            lock.face(&mut state, 100, Some([10.0, 120.0, 30.0]));
            assert_eq!(state.view_angles, [-0.0, 90.0, 0.0]);
            assert!(state.wall_view_locked);
            state.view_angles = [30.0, 0.0, 0.0]; // fresh mouse on the same command endpoint
            lock.face(&mut state, 101, Some([110.0, 20.0, 130.0]));
            assert_eq!(state.view_angles, [-45.0, 0.0, 0.0]);
            lock.face(&mut state, 102, Some([110.0, 20.0, -70.0]));
            assert_eq!(state.view_angles, [45.0, 0.0, 0.0]);
            lock.face(&mut state, 103, Some([10.0, 20.0, 130.0]));
            assert_eq!(state.view_angles[0], -16000.0 * (360.0 / 65536.0));
            assert_eq!(state.command_time, 100 + step);
            assert_eq!(state.origin, [500.0; 3]);
        }
    }

    #[test]
    fn expired_missing_or_changed_client_locks_do_not_take_the_view() {
        let lock = LockView {
            until: 1000,
            enemy: 2,
            client: 1,
            origin: [0.0; 3],
            off: false,
        };
        for (time, client, command, target) in [
            (1000, 1, 100, Some([1.0; 3])),
            (100, 3, 100, Some([1.0; 3])),
            (100, 1, 100, None),
            (100, 1, 1001, Some([1.0; 3])),
        ] {
            let mut state = MovementState {
                client_num: client,
                command_time: command,
                saber_lock_time: 1000,
                view_angles: [12.0, 34.0, 0.0],
                ..Default::default()
            };
            lock.face(&mut state, time, target);
            assert_eq!(state.view_angles, [12.0, 34.0, 0.0]);
            assert!(!state.wall_view_locked);
        }
    }
}
