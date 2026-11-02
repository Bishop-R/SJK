//! Snapshot-owned actor lookups for codemp's PM_BGEntForNum checks.

use sjk_game_jka::pmove::{MoveContext, MovementCollision, Predictor};
use sjk_game_jka::vehicle_predict::RidePrediction;
use sjk_protocol::{EntityState, Snapshot, UserCommand};

#[derive(Clone, Copy)]
struct Body {
    number: u16,
    legs: u16,
    npc: bool,
}

/// Refreshed once per snapshot; command replay and frame previews only borrow it.
#[derive(Default)]
pub(super) struct Entities {
    bodies: Vec<Body>,
    /// `cg_predictLegacy` [`ACTOR_CONTEXT`](sjk_game_jka::pmove::predict_legacy): no
    /// actor is known, as in 2026.1028.1.
    pub(super) off: bool,
}

impl Entities {
    pub(super) fn new(snapshot: Option<&Snapshot>) -> Self {
        let mut result = Self::default();
        if let Some(snapshot) = snapshot {
            result.update(&snapshot.entities);
        }
        result
    }

    pub(super) fn update(&mut self, entities: &[EntityState]) {
        self.bodies.clear();
        if self.off {
            return;
        }
        self.bodies.extend(entities.iter().filter_map(|entity| {
            matches!(entity.entity_type(), 1 | 13).then(|| Body {
                number: entity.number(),
                legs: entity.raw_field(16).unwrap_or(0) as u16,
                // PM_CheckJump and PmoveSingle exclude CLASS_VEHICLE from NPC bounces.
                npc: entity.entity_type() == 13 && entity.npc_class() != 53,
            })
        }));
        self.bodies.sort_unstable_by_key(|body| body.number);
    }

    /// Turn the actor context off (clearing what is known) or back on from the next
    /// snapshot.
    pub(super) fn set_off(&mut self, off: bool) {
        self.off = off;
        if off {
            self.bodies.clear();
        }
    }

    fn body(&self, number: u16) -> Option<&Body> {
        self.bodies
            .binary_search_by_key(&number, |body| body.number)
            .ok()
            .map(|i| &self.bodies[i])
    }

    #[cfg(test)]
    pub(super) fn predict(
        &self,
        predictor: &mut Predictor,
        ride: Option<&mut RidePrediction>,
        command: UserCommand,
        collision: &impl MovementCollision,
    ) {
        self.predict_with_collisions(predictor, ride, command, collision, collision);
    }

    pub(super) fn predict_with_collisions(
        &self,
        predictor: &mut Predictor,
        ride: Option<&mut RidePrediction>,
        command: UserCommand,
        collision: &impl MovementCollision,
        vehicle_collision: &impl MovementCollision,
    ) {
        let bodies = |number| self.body(number).map(|body| body.legs);
        let npcs = |number| self.body(number).is_some_and(|body| body.npc);
        let context = MoveContext {
            bodies: &bodies,
            npcs: &npcs,
            ..MoveContext::CLIENT
        };
        super::ride::predict(
            predictor,
            ride,
            command,
            collision,
            vehicle_collision,
            &context,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sjk_game_jka::pmove::{MovementConfig, MovementState, MovementTrace};

    fn entity(number: u16, kind: u32, class: u32) -> EntityState {
        let mut e = EntityState::zero(number, &sjk_protocol::LEGACY_ENTITY_FIELDS);
        e.set_raw_field(8, kind);
        e.set_raw_field(88, class);
        e.set_raw_field(16, 1139);
        e
    }
    struct Floor(u16);
    impl MovementCollision for Floor {
        fn trace(
            &self,
            start: [f32; 3],
            mins: [f32; 3],
            _: [f32; 3],
            end: [f32; 3],
            _: u32,
        ) -> MovementTrace {
            if end[2] + mins[2] < 0.0 && start[2] + mins[2] >= 0.0 {
                MovementTrace {
                    fraction: (start[2] + mins[2]) / (start[2] - end[2]),
                    end_position: [end[0], end[1], -mins[2]],
                    plane_normal: [0.0, 0.0, 1.0],
                    entity_number: self.0,
                    ..MovementTrace::miss(end)
                }
            } else {
                MovementTrace::miss(end)
            }
        }
    }

    #[test]
    fn snapshot_npcs_bounce_the_player_but_vehicles_and_brushes_do_not() {
        // OpenJK bg_pmove.c:10880-10896, both cgame and game builds.
        for step in [8, 7, 4, 3] {
            for (kind, class, bounce) in [(13, 0, true), (13, 53, false), (6, 0, false)] {
                let mut entities = Entities::default();
                entities.update(&[entity(40, kind, class)]);
                let mut predictor = Predictor::from_state(
                    MovementState {
                        health: 100,
                        gravity: 800.0,
                        standing_height: 40.0,
                        crouching_height: 16.0,
                        origin: [0.0, 0.0, 24.0],
                        ground_entity_number: 40,
                        ..Default::default()
                    },
                    MovementConfig::default(),
                );
                entities.predict(
                    &mut predictor,
                    None,
                    UserCommand {
                        server_time: step,
                        ..Default::default()
                    },
                    &Floor(40),
                );
                assert_eq!(
                    predictor.state().velocity[2] > 200.0,
                    bounce,
                    "{step} ms, kind {kind}, class {class}"
                );
            }
        }
    }

    #[test]
    fn actor_legs_are_available_and_a_new_snapshot_removes_old_actors() {
        let mut entities = Entities::default();
        entities.update(&[entity(40, 13, 0), entity(2, 1, 0), entity(50, 6, 0)]);
        assert_eq!(entities.body(2).map(|b| b.legs), Some(1139));
        assert!(entities.body(40).unwrap().npc);
        assert!(entities.body(50).is_none());
        entities.update(&[]);
        assert!(entities.body(40).is_none());
    }
    #[test]
    #[cfg(debug_assertions)] // violation_count is provided by the warn_debug test feature.
    fn reading_actor_context_during_a_move_allocates_nothing() {
        let mut entities = Entities::default();
        entities.update(&[entity(40, 13, 0)]);
        let mut predictor = Predictor::from_state(
            MovementState {
                health: 100,
                gravity: 800.0,
                standing_height: 40.0,
                crouching_height: 16.0,
                origin: [0.0, 0.0, 24.0],
                ground_entity_number: 40,
                ..Default::default()
            },
            Default::default(),
        );
        let before = assert_no_alloc::violation_count();
        assert_no_alloc::assert_no_alloc(|| {
            entities.predict(
                &mut predictor,
                None,
                UserCommand {
                    server_time: 8,
                    ..Default::default()
                },
                &Floor(40),
            );
        });
        assert_eq!(assert_no_alloc::violation_count(), before);
    }

    #[test]
    #[cfg(not(debug_assertions))]
    #[ignore = "release-only snapshot actor lookup measurement"]
    fn measure_actor_context_lookups() {
        let mut entities = Entities::default();
        let snapshot: Vec<_> = (0..128).map(|number| entity(number, 13, 0)).collect();
        entities.update(&snapshot);
        let start = std::time::Instant::now();
        for n in 0..1_000_000 {
            std::hint::black_box(entities.body(std::hint::black_box((n % 128) as u16)));
        }
        println!(
            "128-actor snapshot: {:.2} ns/lookup (release, 1M binary-search lookups)",
            start.elapsed().as_nanos() as f64 / 1_000_000.0
        );
    }
}
