//! Snapshot-backed default-build Lugormod-v3 movement, reference 50f42520.
//! `PM_SetBounds`, `PM_CheckDuck`, `PM_CanStand` and default jetpack policy.
use super::*;

impl Predictor {
    pub(super) fn lugormod_player(&self) -> bool {
        self.config.lugormod.enabled
            && self.state.client_num < MAX_CLIENTS
            && self.npc.is_none()
            && !self.riding_move()
            && matches!(self.state.movement_type, 0 | 1 | 2 | 5)
    }

    /// Default v3's transmitted admin flag only has enhanced flight in FFA.
    pub(super) fn lugormod_admin_jetpack(&self) -> bool {
        self.lugormod_player() && self.state.lugormod.admin_jetpack && self.gametype == 0
    }

    /// PM_SetBounds scales horizontal dimensions and corpse top, not the feet
    /// or already server-scaled standing/crouching heights.
    pub(super) fn lugormod_bounds(&mut self) -> Bounds {
        let scale = self.state.lugormod.scale();
        let width = 15.0 * scale;
        let top = if self.state.movement_type == PM_DEAD {
            // Invalid authored corpse boxes cannot enter the engine's AABB API.
            (-8.0 * scale).max(-24.0)
        } else if self.state.movement_flags & PMF_DUCKED != 0 {
            self.state.view_height = CROUCH_VIEW_HEIGHT;
            self.state.crouching_height
        } else {
            self.state.view_height = STANDING_VIEW_HEIGHT;
            let sitting = self.state.legs_timer != 0
                && matches!(
                    crate::legacy_animation_name(self.state.legs_anim as usize),
                    Some("BOTH_MEDITATE" | "BOTH_CROUCH3" | "BOTH_SIT6")
                );
            if self.state.movement_flags & crate::pmove_roll::PMF_ROLLING != 0 || sitting {
                self.state.crouching_height
            } else {
                self.state.standing_height
            }
        };
        Bounds {
            minimums: [-width, -width, -24.0],
            maximums: [width, width, top],
        }
    }

    fn lugormod_can_stand(&self, bounds: Bounds, collision: &impl MovementCollision) -> bool {
        let mut end = self.state.origin;
        end[2] += self.state.standing_height;
        let trace = collision.trace(
            self.state.origin,
            bounds.minimums,
            [bounds.maximums[0], bounds.maximums[1], 0.0],
            end,
            PLAYER_CONTENT_MASK,
        );
        !trace.all_solid && trace.fraction >= 1.0
    }

    /// v3 sweeps the lower half of the box upward, rather than stock head probes.
    /// Ground is sampled with the previous posture before this method runs.
    pub(super) fn lugormod_check_duck(
        &mut self,
        command: &UserCommand,
        bounds: Bounds,
        collision: &impl MovementCollision,
    ) -> Bounds {
        if self.state.movement_type != PM_DEAD {
            if crate::pmove_roll::in_roll(&self.state)
                && !crate::pmove_input_freeze::kicking_animation(self.state.legs_anim)
            {
                self.state.movement_flags =
                    (self.state.movement_flags & !PMF_DUCKED) | crate::pmove_roll::PMF_ROLLING;
            } else if self.state.movement_flags & crate::pmove_roll::PMF_ROLLING != 0 {
                if self.lugormod_can_stand(bounds, collision) {
                    self.state.movement_flags &= !crate::pmove_roll::PMF_ROLLING;
                } else {
                    self.state.movement_flags |= PMF_DUCKED;
                }
            } else if command.up_move < 0 || matches!(self.state.force_hand_extend, 8 | 13 | 14) {
                self.state.movement_flags |= PMF_DUCKED;
            } else if self.state.movement_flags & PMF_DUCKED != 0
                && self.lugormod_can_stand(bounds, collision)
            {
                self.state.movement_flags &= !PMF_DUCKED;
            }
        }
        self.lugormod_bounds()
    }

    /// Pmove scales the view height after every single move, with integer truncation.
    pub(super) fn lugormod_finish_slice(&mut self) {
        if self.lugormod_player() {
            self.state.view_height =
                (self.state.view_height as f32 * self.state.lugormod.scale()) as i32;
        }
    }
}

#[cfg(test)]
#[path = "pmove_lugormod_tests.rs"]
mod tests;
