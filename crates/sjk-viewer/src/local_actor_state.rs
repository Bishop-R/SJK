//! Predicted local-player animation and held-equipment presentation.
//!
//! OpenJK replaces the local `centity_t` state from
//! `cg.predictedPlayerState` before `CG_AddCEntity`
//! (`codemp/cgame/cg_ents.c:3457`) through
//! `BG_PlayerStateToEntityState` (`codemp/game/bg_misc.c:2762-2830`).

use sjk_client::pmove::MovementState;
use sjk_runtime::{
    AnimationState, AnimationTrackInput, AnimationTrackState, AnimationTrackTransition, EntityId,
    HeldEquipment, PoseState,
};

/// A held animation's timer at or below this has run out, as far as a restart goes:
/// a frame lands a few milliseconds before the end as often as after it.
const RAN_OUT_MILLIS: i32 = 50;

/// Persistent no-allocation clock state for the predicted local actor.
#[derive(Default)]
pub(crate) struct Tracker {
    entity: Option<EntityId>,
    animation: Option<AnimationState>,
    /// The presented state's `legsTimer` and `torsoTimer`.
    timers: Option<[i32; 2]>,
}

impl Tracker {
    /// Resolve the animation presented for this frame.
    ///
    /// An ordinal or flip revision starts at the predicted command time;
    /// unchanged input preserves its running clock. Deferred saber branches
    /// return `None`, selecting the authoritative runtime-world animation.
    pub(crate) fn resolve(
        &mut self,
        entity: EntityId,
        predicted: Option<&MovementState>,
        authoritative: Option<AnimationState>,
        authoritative_saber_move: Option<u32>,
    ) -> Option<AnimationState> {
        let Some(predicted) =
            predicted.filter(|state| prediction_is_presentable(state, authoritative_saber_move))
        else {
            self.clear();
            return None;
        };
        if self.entity != Some(entity) {
            self.entity = Some(entity);
            self.animation = authoritative;
            self.timers = None;
        }
        let mut previous = self.animation.or(authoritative);
        let command_time = i64::from(predicted.command_time);
        let timers = [predicted.legs_timer, predicted.torso_timer];
        let [mut lower, mut upper] =
            sjk_client::legacy_predicted_animation_inputs(predicted, previous, command_time);
        if let (Some(state), Some(old)) = (previous.as_mut(), self.timers) {
            let legs = restart_unseen(&mut state.lower, &lower, old[0], timers[0], command_time);
            let torso = restart_unseen(&mut state.upper, &upper, old[1], timers[1], command_time);
            if legs || torso {
                // The other track's synchronized start reads the restarted clock.
                [lower, upper] = sjk_client::legacy_predicted_animation_inputs(
                    predicted,
                    previous,
                    command_time,
                );
            }
        }
        self.timers = Some(timers);
        let animation = AnimationState {
            lower: update_track(previous.map(|state| state.lower), lower, command_time),
            upper: update_track(previous.map(|state| state.upper), upper, command_time),
        };
        self.animation = Some(animation);
        Some(animation)
    }

    /// Whether the predicted animation is currently presented.
    pub(crate) fn is_active(&self) -> bool {
        self.animation.is_some()
    }

    fn clear(&mut self) {
        self.entity = None;
        self.animation = None;
        self.timers = None;
    }
}

/// Start `track` over when the clip it plays came back without a frame in between.
///
/// A restart of the clip already playing is presented through its flip, or through
/// another animation in between, such as the stand a held melee kick passes through
/// for one command before the next kick (`bg_pmove.c:7464-7581`). A frame shows only
/// the last command (and the frame's own part of the next one), so that command can go
/// unseen, and a JA+ server restarts the kick with neither: the clip and revision then
/// stay the same, the track kept the finished kick's clock and stood on its last
/// frame, and the legs, which start a kick on the torso's frame, joined it (Slash,
/// 30/10/2026). A held timer that ran out and came back up is that restart.
fn restart_unseen(
    track: &mut AnimationTrackState,
    input: &AnimationTrackInput,
    old_timer: i32,
    timer: i32,
    command_time: i64,
) -> bool {
    let restarted = track.clip == input.clip
        && track.revision == input.revision
        && track.forced_frame.is_none()
        && old_timer <= RAN_OUT_MILLIS
        && timer > old_timer + RAN_OUT_MILLIS;
    if restarted {
        track.started_at_millis = command_time;
        track.phase_millis = 0;
        track.transition = None;
    }
    restarted
}

/// Replace snapshot saber/weapon presentation fields for the local actor.
pub(crate) fn equipment(
    authoritative: Option<HeldEquipment>,
    predicted: Option<&MovementState>,
    authoritative_saber_move: Option<u32>,
) -> Option<HeldEquipment> {
    let Some(predicted) =
        predicted.filter(|state| prediction_is_presentable(state, authoritative_saber_move))
    else {
        return authoritative;
    };
    sjk_client::legacy_predicted_equipment(
        authoritative,
        predicted.weapon,
        predicted.saber_holstered,
        predicted.saber_move,
        predicted.health > 0,
    )
}

/// Pose the local actor from the predicted view angles so the model turns in
/// the same frame as the camera (`cg_ents.c:3454-3476`); the interpolated
/// snapshot pose is a network round-trip behind the mouse.
pub(crate) fn pose(
    authoritative: Option<PoseState>,
    predicted: Option<&MovementState>,
) -> Option<PoseState> {
    let Some(predicted) = predicted else {
        return authoritative;
    };
    // EternalJK remaps the animations before `CG_G2PlayerAngles` reads them.
    let (legs_animation, torso_animation) = sjk_client::legacy_ejk_animation_fixes(
        predicted.legs_anim,
        predicted.torso_anim,
        predicted.weapon,
        predicted.saber_in_flight,
    );
    sjk_client::legacy_predicted_pose(
        authoritative,
        sjk_client::PredictedPoseFields {
            view_angles_degrees: predicted.view_angles,
            velocity: predicted.velocity,
            ground_entity_num: predicted.ground_entity_number,
            movement_direction: predicted.movement_direction,
            legs_animation,
            torso_animation,
            vehicle_entity_num: predicted.vehicle_entity_num,
            weapon: predicted.weapon,
            entity_flags: predicted.entity_flags,
            saber_move: predicted.saber_move,
        },
    )
}

fn prediction_is_presentable(state: &MovementState, authoritative_saber_move: Option<u32>) -> bool {
    state.saber_special_deferred == 0
        && (state.weapon != 3 || sjk_client::legacy_saber_move_is_predicted(state.saber_move))
        && authoritative_saber_move.is_none_or(sjk_client::legacy_saber_move_is_predicted)
}

fn update_track(
    current: Option<AnimationTrackState>,
    input: AnimationTrackInput,
    command_time: i64,
) -> AnimationTrackState {
    if let Some(mut current) = current
        && current.clip == input.clip
        && current.revision == input.revision
        && current.forced_frame == input.forced_frame
    {
        if current.speed_milli != input.speed_milli {
            current.phase_millis = current.elapsed_millis(command_time);
            current.started_at_millis = command_time;
            current.speed_milli = input.speed_milli;
            current.transition = None;
        }
        return current;
    }
    AnimationTrackState {
        clip: input.clip,
        revision: input.revision,
        started_at_millis: command_time,
        phase_millis: input.resume_phase_millis.unwrap_or(0),
        speed_milli: input.speed_milli,
        forced_frame: input.forced_frame,
        transition: current.map(|previous| AnimationTrackTransition {
            clip: previous.clip,
            revision: previous.revision,
            started_at_millis: previous.started_at_millis,
            phase_millis: previous.phase_millis,
            speed_milli: previous.speed_milli,
            forced_frame: previous.forced_frame,
            blend_started_at_millis: command_time,
            blend_duration_millis: input.blend_millis,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WP_MELEE: u8 = 2;

    fn animation(name: &str) -> u16 {
        sjk_game_jka::legacy_animation_index(name).unwrap() as u16
    }

    fn kicker(command_time: i32, legs: &str, torso: &str, timer: i32) -> MovementState {
        MovementState {
            command_time,
            weapon: WP_MELEE,
            legs_anim: animation(legs),
            torso_anim: animation(torso),
            legs_timer: timer,
            torso_timer: timer,
            ..Default::default()
        }
    }

    #[test]
    fn a_held_kick_starts_over_when_its_stand_went_unseen() {
        for step in [8, 7, 4, 3] {
            let mut tracker = Tracker::default();
            let entity = EntityId::new(1);
            let kick = "BOTH_A7_KICK_F";
            let end = 1_000 + 690 - step;
            for state in [
                kicker(1_000, kick, kick, 690),
                kicker(end, kick, kick, step),
                // The torso's stand (one command) is not presented.
                kicker(end + step, "BOTH_STAND1", kick, 0),
            ] {
                tracker.resolve(entity, Some(&state), None, None);
            }
            let next = end + 3 * step;
            let animation = tracker
                .resolve(entity, Some(&kicker(next, kick, kick, 690)), None, None)
                .unwrap();
            let next = i64::from(next);
            assert_eq!(animation.upper.elapsed_millis(next), 0, "{step} ms");
            assert_eq!(animation.lower.elapsed_millis(next), 0, "{step} ms");
        }
    }

    #[test]
    fn a_held_animation_keeps_its_clock_while_it_runs() {
        let mut tracker = Tracker::default();
        let entity = EntityId::new(1);
        let kick = "BOTH_A7_KICK_F";
        tracker.resolve(entity, Some(&kicker(1_000, kick, kick, 690)), None, None);
        // A timer raised before it ran out is not a restart.
        let animation = tracker
            .resolve(entity, Some(&kicker(1_400, kick, kick, 800)), None, None)
            .unwrap();
        assert_eq!(animation.upper.elapsed_millis(1_400), 400);
        assert_eq!(animation.lower.elapsed_millis(1_400), 400);
    }
}
