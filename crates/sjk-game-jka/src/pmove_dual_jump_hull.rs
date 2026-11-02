//! `PM_CheckDualForwardJumpDuck` and `PM_CheckFixMins` (codemp `bg_pmove.c:4290-4376`) as a
//! JA+ server runs them.
//!
//! Stock raises the feet of the dual forward jump attack (`BOTH_JUMPATTACK6`) to 0 in two
//! windows of the animation, so the attacker sails over heads. The JA+ 2.4 B7 game module
//! (`PM_CheckDualForwardJumpDuck` of `jampgamei386.so`) does that only when a box trace
//! 70 units along the view hits another player, unless the server's `dmflags` has 256
//! ([`JaPlusRules::old_collision`](crate::pmove_japlus::JaPlusRules::old_collision)).
//! Without anyone in front the hull stays whole and the attacker lands on the floor,
//! which is what the original JA+ captures of #307 showed. Other servers keep the fixed
//! hull: the stock windows were rolled back in #307 and have no runtime evidence yet.

use super::{MAX_CLIENTS, MovementCollision, PLAYER_CONTENT_MASK, PMF_DUCKED, Predictor};

const PMF_FIX_MINS: u16 = 128;
const BOTH_JUMPATTACK6: u16 = 861;
const MINS_Z: f32 = -24.0;
/// How far ahead JA+ looks for a player to sail over.
const SAIL_OVER_REACH: f32 = 70.0;
/// The box of that trace. Its feet are twice as low as a player's. Its top is 0 because
/// the server's `pmove_t` is zeroed for every command and only the sides are set before
/// `PM_CheckDuck` asks (`g_active.c` `ClientThink_real`).
const SAIL_OVER_MINS: [f32; 3] = [-15.0, -15.0, -48.0];
const SAIL_OVER_MAXS: [f32; 3] = [15.0, 15.0, 0.0];

impl Predictor {
    /// The foot height of a player's hull this command, and the `PMF_FIX_MINS` and
    /// `PMF_DUCKED` bookkeeping that goes with it. Only a JA+ server resizes; elsewhere
    /// the hull keeps its -24 feet.
    pub(super) fn dual_jump_minimum(&mut self, collision: &impl MovementCollision) -> f32 {
        let ja_plus = self.config.ja_plus;
        if !ja_plus.enabled || self.state.client_num >= MAX_CLIENTS {
            return MINS_Z;
        }
        if self.in_sail_over_window() && (ja_plus.old_collision || self.player_ahead(collision)) {
            self.state.movement_flags |= PMF_FIX_MINS;
            return 0.0;
        }
        self.fix_minimum(collision);
        // The server's minimum starts at 0 every command, and `PM_CheckDuck` puts a
        // zero one back to -24 whether or not the fix above succeeded.
        MINS_Z
    }

    /// The two parts of `BOTH_JUMPATTACK6` in which the attacker lies sideways.
    fn in_sail_over_window(&self) -> bool {
        let state = &self.state;
        if state.legs_anim != BOTH_JUMPATTACK6 {
            return false;
        }
        let Some(length) = self
            .animation_lengths
            .as_deref()
            .and_then(|lengths| lengths.length_ms(BOTH_JUMPATTACK6))
        else {
            return false;
        };
        let elapsed = length - state.legs_timer;
        (state.legs_timer >= 1_450 && elapsed >= 400)
            || (state.legs_timer >= 400 && elapsed >= 1_100)
    }

    /// Whether the JA+ trace along the view (pitch included) stops on another player.
    fn player_ahead(&self, collision: &impl MovementCollision) -> bool {
        let state = &self.state;
        let forward = super::flight::angles_to_axis(state.view_angles)[0];
        let end = std::array::from_fn(|axis| state.origin[axis] + SAIL_OVER_REACH * forward[axis]);
        let hit = collision.trace(
            state.origin,
            SAIL_OVER_MINS,
            SAIL_OVER_MAXS,
            end,
            PLAYER_CONTENT_MASK,
        );
        hit.entity_number < MAX_CLIENTS && hit.entity_number != state.client_num
    }

    /// `PM_CheckFixMins`: put raised feet back down, moving up if the floor is in the way,
    /// or crouch at the end of the attack when there is no room above either.
    fn fix_minimum(&mut self, collision: &impl MovementCollision) {
        let state = &mut self.state;
        if state.movement_flags & PMF_FIX_MINS == 0 {
            return;
        }
        let origin = state.origin;
        let minimums = [-15.0, -15.0, 0.0];
        let maximums = [15.0, 15.0, state.standing_height];
        let mut end = origin;
        end[2] += MINS_Z;
        let down = collision.trace(origin, minimums, maximums, end, PLAYER_CONTENT_MASK);
        if down.all_solid || down.start_solid {
            return;
        }
        if down.fraction >= 1.0 {
            state.movement_flags &= !PMF_FIX_MINS;
            return;
        }
        let up_distance = (1.0 - down.fraction) * -MINS_Z;
        end[2] = origin[2] + up_distance;
        let up = collision.trace(origin, minimums, maximums, end, PLAYER_CONTENT_MASK);
        if up.all_solid || up.start_solid {
            return;
        }
        if up.fraction >= 1.0 {
            state.origin[2] += up_distance;
            state.movement_flags &= !PMF_FIX_MINS;
        } else if state.legs_anim != BOTH_JUMPATTACK6 || state.legs_timer <= 200 {
            state.origin[2] -= MINS_Z;
            if state.legs_anim == BOTH_JUMPATTACK6 {
                state.legs_timer = 0;
                state.torso_timer = 0;
            }
            state.movement_flags = (state.movement_flags | PMF_DUCKED) & !PMF_FIX_MINS;
        }
    }
}
