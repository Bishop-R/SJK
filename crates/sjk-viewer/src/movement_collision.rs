//! BSP-backed collision adapter used by client movement prediction.

use crate::local_prediction::movers::{ClipActor, Collider};
use glam::Vec3;
use sjk_bsp::{Aabb, Bsp};
use sjk_client::pmove::{ENTITY_NUMBER_WORLD, MovementCollision, MovementTrace};
use std::cell::RefCell;

#[path = "mover_collision.rs"]
mod inline;

/// Reuse transformed inline and packed-box collision for a crosshair entity trace.
pub(crate) fn target_trace(
    bsp: &Bsp,
    collider: &Collider,
    start: [f32; 3],
    end: [f32; 3],
) -> MovementTrace {
    inline::trace(
        bsp,
        collider,
        start,
        end,
        Aabb::new([0.0; 3], [0.0; 3]).unwrap(),
        1 | 0x100,
    )
}

/// A box swept through one solid (the Ragdoll's points against movers).
pub(crate) fn solid_trace(
    bsp: &Bsp,
    collider: &Collider,
    start: [f32; 3],
    end: [f32; 3],
    bounds: Aabb,
    mask: u32,
) -> MovementTrace {
    inline::trace(bsp, collider, start, end, bounds, mask)
}

pub(super) struct BspMovementCollision<'a> {
    bsp: &'a Bsp,
    scratch: RefCell<&'a mut sjk_bsp::TraceScratch>,
    movers: &'a [Collider],
    actor: Option<ClipActor>,
}

impl<'a> BspMovementCollision<'a> {
    pub(super) fn with_movers(
        bsp: &'a Bsp,
        scratch: &'a mut sjk_bsp::TraceScratch,
        movers: &'a [Collider],
    ) -> Self {
        Self {
            bsp,
            scratch: RefCell::new(scratch),
            movers,
            actor: None,
        }
    }

    pub(super) fn for_actor(
        bsp: &'a Bsp,
        scratch: &'a mut sjk_bsp::TraceScratch,
        movers: &'a [Collider],
        actor: ClipActor,
    ) -> Self {
        Self {
            actor: Some(actor),
            ..Self::with_movers(bsp, scratch, movers)
        }
    }

    pub(super) fn chassis(&self, actor: Option<ClipActor>) -> ChassisCollision<'_, 'a> {
        ChassisCollision { world: self, actor }
    }

    fn trace_for_actor(
        &self,
        actor: Option<ClipActor>,
        start: [f32; 3],
        minimums: [f32; 3],
        maximums: [f32; 3],
        end: [f32; 3],
        content_mask: u32,
    ) -> MovementTrace {
        trace_for_actor(self, actor, start, minimums, maximums, end, content_mask)
    }
}

/// Borrow the same scratch/world for chassis movement, without changing the pilot view.
pub(super) struct ChassisCollision<'w, 'b> {
    world: &'w BspMovementCollision<'b>,
    actor: Option<ClipActor>,
}

impl MovementCollision for ChassisCollision<'_, '_> {
    fn rocket_lock_target(&self, entity: u16) -> Option<sjk_game_jka::pmove::RocketLockTarget> {
        self.world.rocket_lock_target(entity)
    }
    fn point_contents(&self, point: [f32; 3]) -> u32 {
        self.world.point_contents(point)
    }
    fn trace(
        &self,
        start: [f32; 3],
        minimums: [f32; 3],
        maximums: [f32; 3],
        end: [f32; 3],
        content_mask: u32,
    ) -> MovementTrace {
        self.world
            .trace_for_actor(self.actor, start, minimums, maximums, end, content_mask)
    }
}

impl MovementCollision for BspMovementCollision<'_> {
    fn rocket_lock_target(&self, entity: u16) -> Option<sjk_game_jka::pmove::RocketLockTarget> {
        self.movers
            .iter()
            .find(|collider| collider.entity == entity)
            .map(|collider| collider.rocket_target)
    }
    fn point_contents(&self, point: [f32; 3]) -> u32 {
        // Water-jump ledge probes need SOLID/PLAYERCLIP/BODY as well as liquids.
        let mut contents = self.bsp.point_contents(point, u32::MAX);
        for mover in self.movers.iter().filter(|mover| !mover.unlaunched) {
            contents |= inline::contents(self.bsp, mover, point);
        }
        contents
    }
    fn trace(
        &self,
        start: [f32; 3],
        minimums: [f32; 3],
        maximums: [f32; 3],
        end: [f32; 3],
        content_mask: u32,
    ) -> MovementTrace {
        self.trace_for_actor(self.actor, start, minimums, maximums, end, content_mask)
    }
}

fn trace_for_actor(
    world: &BspMovementCollision<'_>,
    actor: Option<ClipActor>,
    start: [f32; 3],
    minimums: [f32; 3],
    maximums: [f32; 3],
    end: [f32; 3],
    content_mask: u32,
) -> MovementTrace {
    let bounds = Aabb::new(minimums, maximums).expect("pmove produces valid bounds");
    let trace = world.bsp.trace_box_with(
        &mut world.scratch.borrow_mut(),
        start,
        end,
        bounds,
        content_mask,
    );
    let mut result = MovementTrace {
        fraction: trace.fraction,
        end_position: trace.end_position,
        plane_normal: trace.plane.map_or([0.0; 3], |plane| plane.normal),
        surface_flags: trace.surface_flags,
        start_solid: trace.start_solid,
        all_solid: trace.all_solid,
        // CG_Trace, codemp/cgame/cg_predict.c:377-383: an escaping
        // startsolid trace with fraction 1 does not identify a world hit.
        entity_number: if trace.fraction < 1.0 {
            ENTITY_NUMBER_WORLD
        } else {
            sjk_protocol::ENTITY_NUMBER_NONE
        },
    };
    for mover in world.movers.iter().filter(|mover| {
        !mover.unlaunched && actor.is_none_or(|actor| actor.collides(mover.clip_actor))
    }) {
        let trace = inline::trace(world.bsp, mover, start, end, bounds, content_mask);
        // CG_ClipMoveToEntities also identifies an overlapping entity
        // when another surface supplied the nearer trace fraction.
        if trace.all_solid || trace.fraction < result.fraction {
            result = trace;
        } else if trace.start_solid {
            result.start_solid = true;
            result.entity_number = mover.entity;
        }
        if result.all_solid {
            break;
        }
    }
    result
}

/// CG_Trace camera hull, including presented inline doors/platforms and the
/// baseline-only permanent ones, such as a server's `misc_bsp` instances, which
/// `CG_BuildSolidList` adds from `cg_permanents` (`cg_predict.c`). BODY is
/// deliberately absent: the local rider and vehicle cannot occlude their own view.
pub(crate) fn camera_trace(
    bsp: &Bsp,
    scratch: &mut sjk_bsp::TraceScratch,
    solids: Option<(&sjk_protocol::GameState, &sjk_protocol::Snapshot)>,
    time: i32,
    start: Vec3,
    end: Vec3,
) -> Vec3 {
    match solids {
        Some((game, snapshot)) => camera_trace_through(
            bsp,
            scratch,
            sjk_client::legacy_scene_entities(game, snapshot),
            snapshot.player.client_num(),
            time,
            start,
            end,
        ),
        None => camera_trace_through(bsp, scratch, [], u16::MAX, time, start, end),
    }
}

/// A rain strike needs open sky and a dry point, including presented inline doors
/// and baseline-only permanent models. Packed player hulls do not stop rain.
pub(crate) fn rain_exposed(
    bsp: &Bsp,
    scratch: &mut sjk_bsp::TraceScratch,
    colliders: &[Collider],
    owner: u16,
    point: Vec3,
) -> bool {
    const REJECT: u32 = 1 | 0x1000_0000 | 0x20000 | 2 | 4;
    let mut contents = bsp.point_contents(point.to_array(), u32::MAX);
    if contents & REJECT != 0 {
        return false;
    }
    let end = point + Vec3::Z * 65536.0;
    let mut hit = bsp.trace_box_with(
        scratch,
        point.to_array(),
        end.to_array(),
        Aabb::POINT,
        0x1001,
    );
    for collider in colliders {
        if collider.entity == owner {
            continue;
        }
        contents |= inline::contents(bsp, collider, point.to_array());
        if contents & REJECT != 0 {
            return false;
        }
        let other = inline::trace(
            bsp,
            collider,
            point.to_array(),
            end.to_array(),
            Aabb::POINT,
            0x1001,
        );
        hit.start_solid |= other.start_solid;
        hit.all_solid |= other.all_solid;
        if other.fraction < hit.fraction {
            hit.fraction = other.fraction;
            hit.surface_flags = other.surface_flags;
        }
    }
    !hit.start_solid
        && !hit.all_solid
        && (hit.surface_flags & 0x2000 != 0 || (hit.fraction == 1.0 && contents & 0x10000 != 0))
}

/// [`camera_trace`] against the world and the inline-model solids among `entities`,
/// skipping the local client `local`.
fn camera_trace_through<'a>(
    bsp: &Bsp,
    scratch: &mut sjk_bsp::TraceScratch,
    entities: impl IntoIterator<Item = &'a sjk_protocol::EntityState>,
    local: u16,
    time: i32,
    start: Vec3,
    end: Vec3,
) -> Vec3 {
    // MASK_SOLID | CONTENTS_PLAYERCLIP (codemp/game/bg_public.h, surfaceflags.h).
    const MASK: u32 = 0x1001 | 0x10;
    let bounds = Aabb::new([-4.0; 3], [4.0; 3]).expect("valid camera hull");
    let mut best = bsp.trace_box_with(scratch, start.to_array(), end.to_array(), bounds, MASK);
    for entity in entities {
        if entity.solid() != 0x00ff_ffff || entity.number() == local {
            continue;
        }
        let Some(collider) = Collider::from_entity(entity, time, time, false) else {
            continue;
        };
        let hit = inline::trace(
            bsp,
            &collider,
            start.to_array(),
            end.to_array(),
            bounds,
            MASK,
        );
        if hit.all_solid || hit.fraction < best.fraction {
            best.fraction = hit.fraction;
            best.end_position = hit.end_position;
            best.all_solid = hit.all_solid;
        }
        if best.all_solid {
            break;
        }
    }
    Vec3::from_array(best.end_position)
}

/// A server pusher can leave groundEntityNum stale until the next Pmove.
/// Only reject carry when even the rider's foot footprint misses that mover.
/// This is a presentation guard, not a replacement for PM_GroundTrace.
pub(super) fn mover_supports_feet(bsp: &Bsp, mover: &Collider, origin: [f32; 3]) -> bool {
    let mut end = origin;
    end[2] -= 0.25;
    let feet = Aabb::new([-15.0, -15.0, -24.0], [15.0, 15.0, -24.0])
        .expect("constant player footprint is valid");
    let hit = inline::trace(
        bsp,
        mover,
        origin,
        end,
        feet,
        sjk_client::pmove::PLAYER_CONTENT_MASK,
    );
    hit.fraction != 1.0 || hit.start_solid
}

#[cfg(test)]
mod bsp_instance_tests {
    //! A server's `misc_bsp` (codemp `SP_misc_bsp`) is an `EF_PERMANENT` `ET_MOVER`
    //! whose model is a sub-BSP's world, sent only in the baselines. Prediction and
    //! `CG_Trace` clip against it through `CG_BuildSolidList`'s permanents.
    use super::*;
    use crate::local_prediction::movers::Movers;
    use sjk_bsp::{CollisionBrush, CollisionShader, box_brush, write_collision_map};
    use sjk_protocol::{EntityState, LEGACY_ENTITY_FIELDS, PlayerState, Snapshot};

    /// Where the instance stands: in the main map's void, as JoF places its arenas.
    const ORIGIN: [f32; 3] = [8000.0, 0.0, 0.0];
    const INSTANCE: u16 = 300;

    fn map(brushes: &[CollisionBrush]) -> Bsp {
        let shaders = [CollisionShader {
            name: "textures/test/solid".into(),
            surface_flags: 0,
            content_flags: 1,
        }];
        let entities = "{
\"classname\" \"worldspawn\"
}
";
        Bsp::parse(&write_collision_map(entities, &shaders, brushes)).expect("valid map")
    }

    /// A main map with a floor at its centre and, appended after it as `CM_LoadSubBSP`
    /// does, a sub-BSP with a floor and a wall at local x 200..256.
    fn world() -> (Bsp, usize) {
        let mut main = map(&[box_brush([-512.0, -512.0, -64.0], [512.0, 512.0, 0.0], 0)]);
        let arena = map(&[
            box_brush([-256.0, -256.0, -32.0], [256.0, 256.0, 0.0], 0),
            box_brush([200.0, -256.0, 0.0], [256.0, 256.0, 256.0], 0),
        ]);
        let models = main.append_model_asset(arena).expect("append the sub-BSP");
        (main, models.start)
    }

    fn instance(model: usize) -> EntityState {
        let mut state = EntityState::zero(INSTANCE, &LEGACY_ENTITY_FIELDS);
        state.set_raw_field(8, 6); // ET_MOVER
        state.set_raw_field(19, 1 << 7); // EF_PERMANENT
        state.set_raw_field(26, 0x00ff_ffff); // SOLID_BMODEL
        state.set_raw_field(46, model as u32);
        // `trajectory_base` reads fields 2, 1 and 4 as x, y and z.
        state.set_raw_field(2, ORIGIN[0].to_bits());
        state.set_raw_field(1, ORIGIN[1].to_bits());
        state.set_raw_field(4, ORIGIN[2].to_bits());
        state
    }

    #[test]
    fn moving_brush_contents_use_presentation_time_and_sweeps_keep_physics_time() {
        const WATER: u32 = 4;
        let shaders = [CollisionShader {
            name: "textures/test/water".into(),
            surface_flags: 0,
            content_flags: WATER,
        }];
        let water = Bsp::parse(&write_collision_map(
            "{\n\"classname\" \"worldspawn\"\n}\n",
            &shaders,
            &[box_brush([-1.0, -4.0, -4.0], [1.0, 4.0, 4.0], 0)],
        ))
        .unwrap();
        let mut bsp = map(&[]);
        let model = bsp.append_model_asset(water).unwrap().start;
        for step in [8, 7, 4, 3] {
            let mut state = instance(model);
            state.set_raw_field(19, 0);
            state.set_raw_field(2, 0.0_f32.to_bits());
            state.set_raw_field(23, 2); // TR_LINEAR
            state.set_raw_field(6, 1_000.0_f32.to_bits()); // pos.trDelta[0]
            let collider = Collider::from_entity(&state, step, 0, false).unwrap();
            assert!((collider.origin[0] - step as f32).abs() < 0.0001);
            assert_eq!(collider.contents_origin[0], 0.0);
            assert_eq!(inline::contents(&bsp, &collider, [0.0; 3]), WATER);
            assert_eq!(
                inline::contents(&bsp, &collider, [step as f32, 0.0, 0.0]),
                0
            );
            let mut scratch = bsp.trace_scratch();
            let collision = BspMovementCollision::with_movers(
                &bsp,
                &mut scratch,
                std::slice::from_ref(&collider),
            );
            let trace = collision.trace(
                [step as f32 - 2.0, 0.0, 0.0],
                [0.0; 3],
                [0.0; 3],
                [step as f32 + 2.0, 0.0, 0.0],
                WATER,
            );
            assert_eq!(trace.entity_number, INSTANCE);
            assert!(trace.fraction < 1.0);
        }
    }

    #[test]
    fn the_camera_stops_at_a_permanent_bsp_instance_wall() {
        let (bsp, model) = world();
        let mut scratch = bsp.trace_scratch();
        let state = instance(model);
        let start = Vec3::new(ORIGIN[0], 0.0, 64.0);
        let end = Vec3::new(ORIGIN[0] + 400.0, 0.0, 64.0);
        // A snapshot never carries the instance: on its own the camera crosses the wall.
        assert_eq!(
            camera_trace_through(&bsp, &mut scratch, [], 0, 0, start, end),
            end
        );
        let blocked = camera_trace_through(&bsp, &mut scratch, [&state], 0, 0, start, end);
        assert!(
            blocked.x > ORIGIN[0] + 190.0 && blocked.x < ORIGIN[0] + 200.0,
            "{blocked}"
        );
    }

    /// A rocket fired along +x at 900 units a second from x 100 at 1000 ms, a solid
    /// 3-unit box as the game makes it (`r.contents = MASK_SHOT`).
    fn rocket() -> EntityState {
        let mut state = EntityState::zero(INSTANCE, &LEGACY_ENTITY_FIELDS);
        state.set_raw_field(8, 3); // ET_MISSILE
        state.set_raw_field(26, 3 | 3 << 8 | 35 << 16);
        state.set_raw_field(0, 1_000); // pos.trTime
        state.set_raw_field(23, 2); // TR_LINEAR
        state.set_raw_field(2, 100.0_f32.to_bits());
        state.set_raw_field(4, 64.0_f32.to_bits());
        state.set_raw_field(6, 900.0_f32.to_bits());
        state
    }

    fn snapshot(entities: Vec<EntityState>) -> Snapshot {
        let mut player = PlayerState::zero();
        player.set_client_num(0);
        Snapshot {
            message_sequence: 1,
            reliable_acknowledge: 0,
            server_commands: Vec::new(),
            server_time: 1_000,
            delta_from: None,
            flags: 0,
            area_mask: Vec::new(),
            player,
            vehicle_player: None,
            entities,
            consumed_bits: 0,
        }
    }

    fn rider(number: u16, vehicle: u16, x: f32) -> EntityState {
        let mut state = EntityState::zero(number, &LEGACY_ENTITY_FIELDS);
        state.set_raw_field(8, 1);
        state.set_raw_field(93, u32::from(vehicle));
        state.set_raw_field(26, 10 | (10 << 8) | (42 << 16));
        state.set_raw_field(2, x.to_bits());
        state
    }

    #[test]
    fn vehicle_clip_keeps_pilot_and_chassis_contacts_separate() {
        let bsp = map(&[]);
        for step in [8, 7, 4, 3] {
            for attached in [true, false] {
                let mut cloaked_rider = rider(2, 101, 100.0);
                cloaked_rider.set_raw_field(77, 2);
                let passengers = snapshot(vec![
                    rider(1, if attached { 100 } else { 0 }, 0.0),
                    cloaked_rider,
                ]);
                let mut movers = Movers::new();
                movers.update(&passengers, 1_000 + step);
                let mut scratch = bsp.trace_scratch();
                let pilot = ClipActor::player(0, 100);
                let mut vehicle = rider(100, 1, 0.0);
                vehicle.set_raw_field(8, 13);
                vehicle.set_raw_field(88, 53);
                let world =
                    BspMovementCollision::for_actor(&bsp, &mut scratch, &movers.colliders, pilot);
                let first = world.trace([0.0; 3], [0.0; 3], [0.0; 3], [20.0, 0.0, 0.0], 0x100);
                assert!(
                    first.start_solid,
                    "pilot still collides with the other rider"
                );
                let chassis = world.chassis(Some(ClipActor::from_entity(&vehicle)));
                assert_eq!(chassis.rocket_lock_target(2).unwrap().powerups, 2);
                let chassis_hit =
                    chassis.trace([0.0; 3], [0.0; 3], [0.0; 3], [20.0, 0.0, 0.0], 0x100);
                assert_eq!(chassis_hit.start_solid, !attached);
                if attached {
                    assert_eq!(
                        chassis
                            .trace([0.0; 3], [0.0; 3], [0.0; 3], [200.0, 0.0, 0.0], 0x100)
                            .entity_number,
                        2,
                        "unrelated vehicle rider still blocks"
                    );
                }
                assert!(
                    world
                        .trace([0.0; 3], [0.0; 3], [0.0; 3], [20.0, 0.0, 0.0], 0x100)
                        .start_solid,
                    "chassis view must not change pilot context"
                );
            }
        }
    }

    #[test]
    fn vehicle_clip_metadata_covers_a_local_passenger_without_a_predicted_chassis() {
        let bsp = map(&[]);
        let mut vehicle = rider(100, 2, 0.0);
        vehicle.set_raw_field(8, 13);
        vehicle.set_raw_field(88, 53);
        let mut snap = snapshot(vec![vehicle]);
        snap.player.set_raw_field(84, 100);
        let mut movers = Movers::new();
        movers.update(&snap, 1_000);
        assert!(movers.vehicle_actor.is_none());
        let mut scratch = bsp.trace_scratch();
        let world = BspMovementCollision::for_actor(
            &bsp,
            &mut scratch,
            &movers.colliders,
            movers.local_actor,
        );
        let hit = world.trace([0.0; 3], [0.0; 3], [0.0; 3], [20.0, 0.0, 0.0], 0x100);
        assert!(!hit.start_solid && hit.fraction == 1.0);
    }

    fn crowded_vehicle_scene() -> (Bsp, Movers, ClipActor) {
        let entities = (32..160)
            .map(|number| {
                let mut entity = rider(
                    number,
                    if number % 2 == 0 { 900 } else { 901 },
                    f32::from(number - 32) * 30.0,
                );
                entity.set_raw_field(8, 13);
                entity.set_raw_field(88, 50);
                entity
            })
            .collect();
        let mut movers = Movers::new();
        movers.update(&snapshot(entities), 1_000);
        let mut vehicle = rider(900, 1, 0.0);
        vehicle.set_raw_field(8, 13);
        vehicle.set_raw_field(88, 53);
        (map(&[]), movers, ClipActor::from_entity(&vehicle))
    }

    #[cfg(debug_assertions)]
    #[test]
    fn vehicle_clip_views_allocate_nothing_after_warmup() {
        let (bsp, movers, vehicle) = crowded_vehicle_scene();
        let mut scratch = bsp.trace_scratch();
        let world = BspMovementCollision::for_actor(
            &bsp,
            &mut scratch,
            &movers.colliders,
            ClipActor::player(0, 900),
        );
        let chassis = world.chassis(Some(vehicle));
        let trace = || {
            chassis.trace(
                [0.0, 0.0, 100.0],
                [0.0; 3],
                [0.0; 3],
                [4096.0, 0.0, 100.0],
                0x100,
            )
        };
        let _ = trace();
        let before = assert_no_alloc::violation_count();
        assert_no_alloc::assert_no_alloc(|| {
            for _ in 0..100 {
                std::hint::black_box(trace());
            }
        });
        assert_eq!(assert_no_alloc::violation_count(), before);
    }

    #[test]
    #[ignore = "release-only actor collision filter measurement"]
    fn measure_vehicle_clip_views() {
        assert!(!cfg!(debug_assertions), "run with --release");
        let (bsp, movers, vehicle) = crowded_vehicle_scene();
        let mut scratch = bsp.trace_scratch();
        let world = BspMovementCollision::for_actor(
            &bsp,
            &mut scratch,
            &movers.colliders,
            ClipActor::player(0, 900),
        );
        let chassis = world.chassis(Some(vehicle));
        let unfiltered = world.chassis(None);
        for (name, collision) in [
            ("unfiltered", &unfiltered as &dyn MovementCollision),
            ("pilot", &world as &dyn MovementCollision),
            ("chassis", &chassis as &dyn MovementCollision),
        ] {
            let start = std::time::Instant::now();
            for _ in 0..10_000 {
                std::hint::black_box(std::hint::black_box(collision).trace(
                    [0.0, 0.0, 100.0],
                    [0.0; 3],
                    [0.0; 3],
                    [4096.0, 0.0, 100.0],
                    0x100,
                ));
            }
            println!(
                "128 actors, {name}: {:.2} ns per clear sweep",
                start.elapsed().as_nanos() as f64 / 10_000.0
            );
        }
    }

    /// A rocket never blocks the predicted player: before its launch its trajectory
    /// runs back into its shooter, and after it the server lets its owner through.
    #[test]
    fn a_rocket_never_blocks_the_runner() {
        let bsp = map(&[box_brush([-512.0, -512.0, -64.0], [512.0, 512.0, 0.0], 0)]);
        let mut scratch = bsp.trace_scratch();
        for time in [950, 1_010, 1_100] {
            let mut movers = Movers::new();
            movers.update(&snapshot(vec![rocket()]), time);
            assert!(movers.colliders.is_empty(), "{time}");
            let trace = BspMovementCollision::with_movers(&bsp, &mut scratch, &movers.colliders)
                .trace(
                    [0.0, 0.0, 64.0],
                    [-15.0, -15.0, -24.0],
                    [15.0, 15.0, 40.0],
                    [200.0, 0.0, 64.0],
                    sjk_client::pmove::PLAYER_CONTENT_MASK,
                );
            assert_eq!(trace.fraction, 1.0, "{time}");
        }
    }

    #[test]
    fn prediction_stands_on_a_permanent_bsp_instance_floor() {
        let (bsp, model) = world();
        let mut scratch = bsp.trace_scratch();
        let mut player = PlayerState::zero();
        player.set_client_num(0);
        player.set_origin([ORIGIN[0], 0.0, 64.0]);
        let snapshot = Snapshot {
            message_sequence: 1,
            reliable_acknowledge: 0,
            server_commands: Vec::new(),
            server_time: 1_000,
            delta_from: None,
            flags: 0,
            area_mask: Vec::new(),
            player,
            vehicle_player: None,
            entities: Vec::new(),
            consumed_bits: 0,
        };
        let mut movers = Movers::new();
        movers.set_permanent_states(vec![instance(model)]);
        movers.update(&snapshot, 1_000);
        let collision = BspMovementCollision::with_movers(&bsp, &mut scratch, &movers.colliders);
        let down = collision.trace(
            [ORIGIN[0], 0.0, 64.0],
            [-15.0, -15.0, -24.0],
            [15.0, 15.0, 40.0],
            [ORIGIN[0], 0.0, -64.0],
            sjk_client::pmove::PLAYER_CONTENT_MASK,
        );
        assert!(down.fraction < 1.0, "fell through the instance floor");
        assert_eq!(down.entity_number, INSTANCE);
        assert!(
            (down.end_position[2] - 24.0).abs() < 0.2,
            "{:?}",
            down.end_position
        );
    }
}
