//! The teleport effect's floor and its `org2fromTrace` beam, in a made-up room.

use super::*;
use sjk_bsp::{CollisionShader, box_brush, write_collision_map};

/// Top of the floor and underside of the ceiling.
const FLOOR: f32 = 0.0;
const CEILING: f32 = 512.0;

/// A room 1024 units wide whose floor slab has `floor_contents`.
fn room(floor_contents: u32) -> sjk_bsp::Bsp {
    let shader = |name: &str, content_flags| CollisionShader {
        name: name.into(),
        surface_flags: 0,
        content_flags,
    };
    let shaders = [
        shader("textures/test/solid", 1),
        shader("textures/test/floor", floor_contents),
    ];
    let brushes = [
        box_brush([-512.0, -512.0, FLOOR - 64.0], [512.0, 512.0, FLOOR], 1),
        box_brush([-512.0, -512.0, CEILING], [512.0, 512.0, CEILING + 64.0], 0),
    ];
    let entities = "{\n\"classname\" \"worldspawn\"\n}\n";
    sjk_bsp::Bsp::parse(&write_collision_map(entities, &shaders, &brushes))
        .expect("synthetic map parses")
}

/// Two traced lines shaped like the beam of `mp/spawn` and `mp/jedispawn`: one from the
/// effect's origin, one from 20 units behind it along the effect's forward axis.
const BEAM: &str = "
Line
{
	spawnFlags	org2fromTrace
	life		500
	size { start 12 }
	shaders [ gfx/test/beam ]
}

Line
{
	spawnFlags	org2fromTrace
	life		500
	origin		-20 0 0
	size { start 40 }
	shaders [ gfx/test/beam ]
}
";

fn beam_vfs() -> VirtualFileSystem {
    let mut vfs = VirtualFileSystem::new();
    vfs.mount_memory("test", [("effects/test/beam.efx", BEAM)])
        .expect("mount the test effect");
    vfs
}

/// Play the test beam as `EV_PLAYER_TELEPORT_IN/OUT` play `mp/spawn`: at `origin`,
/// forward axis straight up.
fn play_beam(origin: Vec3) -> Vec<Particle> {
    let mut particles = Vec::new();
    spawn_effect(
        &mut particles,
        &mut crate::effect_aux::Runtime::default(),
        &mut EffectLibrary::default(),
        &beam_vfs(),
        "test/beam",
        origin,
        Instant::now(),
        64,
        0,
        &mut None,
        combat_effects::rotation_from_direction([0.0, 0.0, 1.0]),
    );
    particles
}

fn line_ends(particle: &Particle) -> (Vec3, Vec3) {
    let start = particle.motion.sample().origin;
    (start, start + particle.streak.expect("a line has a streak"))
}

#[test]
fn the_teleport_effect_stands_where_the_player_box_meets_the_floor() {
    let bsp = room(1);
    let mut scratch = bsp.trace_scratch();
    // A spawn point's origin is 9 units above its marker (`SelectSpawnPoint`); the box's
    // feet (mins z -16) rest on the floor, 16 units below the box origin.
    let floor = teleport_floor(&bsp, &mut scratch, Vec3::new(64.0, -32.0, FLOOR + 33.0))
        .expect("a floor below");
    assert!(
        (floor - Vec3::new(64.0, -32.0, FLOOR + 16.0)).length() < 0.2,
        "{floor}"
    );
    // Over a void the event plays no effect.
    assert_eq!(
        teleport_floor(&bsp, &mut scratch, Vec3::new(2_000.0, 0.0, 100.0)),
        None
    );
}

#[test]
fn the_teleport_floor_trace_stops_on_terrain_as_mask_solid_does() {
    const CONTENTS_TERRAIN: u32 = 0x1000;
    let bsp = room(CONTENTS_TERRAIN);
    let mut scratch = bsp.trace_scratch();
    let floor = teleport_floor(&bsp, &mut scratch, Vec3::new(0.0, 0.0, FLOOR + 33.0))
        .expect("a terrain floor below");
    assert!((floor.z - (FLOOR + 16.0)).abs() < 0.2, "{floor}");
}

#[test]
fn a_traced_line_runs_up_from_its_origin_to_the_ceiling() {
    let bsp = room(1);
    let mut scratch = bsp.trace_scratch();
    let origin = teleport_floor(&bsp, &mut scratch, Vec3::new(0.0, 0.0, FLOOR + 33.0))
        .expect("a floor below");
    let mut particles = play_beam(origin);
    assert_eq!(particles.len(), 2);
    for particle in &mut particles {
        assert!(particle.trace_streak, "traced when first drawn");
        crate::effect_geometry::resolve_traced_streak(particle, &bsp, &mut scratch);
        assert!(!particle.trace_streak);
    }
    // The first line starts at the effect's origin, the second 20 units below it (inside
    // the floor slab, which the trace leaves as stock's does); both reach the ceiling.
    let starts = [origin.z, origin.z - 20.0];
    for (particle, start_z) in particles.iter().zip(starts) {
        let (start, end) = line_ends(particle);
        assert!(
            (start - Vec3::new(origin.x, origin.y, start_z)).length() < 1e-3,
            "{start}"
        );
        assert!(end.x.abs() < 1e-3 && end.y.abs() < 1e-3, "{end}");
        assert!((end.z - CEILING).abs() < 0.2, "{end}");
    }
}

#[test]
fn a_traced_line_is_traced_once() {
    let bsp = room(1);
    let mut scratch = bsp.trace_scratch();
    let mut particles = play_beam(Vec3::new(0.0, 0.0, FLOOR + 16.0));
    let particle = &mut particles[0];
    crate::effect_geometry::resolve_traced_streak(particle, &bsp, &mut scratch);
    // Later draws keep the end, which a trace would cut at the ceiling; a trip mine
    // likewise gives its lines a kept trace's end and clears the flag.
    particle.streak = Some(Vec3::Z * 1_000.0);
    crate::effect_geometry::resolve_traced_streak(particle, &bsp, &mut scratch);
    assert_eq!(particle.streak, Some(Vec3::Z * 1_000.0));
}

#[test]
fn an_offset_traced_end_turns_with_the_effect() {
    // `org2isOffset` moves the untraced end by `origin2` in the effect's axes
    // (`FxScheduler.cpp:1402-1413`); `cheapOrg2Calc` leaves the offset in world axes.
    let mut component = sjk_effect::parse_effect(
        "Electricity { spawnFlags org2fromTrace org2isOffset origin2 0 -100 200 }",
    )
    .expect("parses")
    .components
    .remove(0);
    let up = combat_effects::rotation_from_direction([0.0, 0.0, 1.0]);
    let origin = Vec3::new(1.0, 2.0, 3.0);
    let offset = Vec3::new(0.0, -100.0, 200.0);
    let target = crate::effect_shapes::origin2_trace_target(&component, origin, up, offset);
    assert!((target - (origin + Vec3::Z * 16_384.0 + up * offset)).length() < 1e-2);
    component.spawn_flags.cheap_origin2 = true;
    let target = crate::effect_shapes::origin2_trace_target(&component, origin, up, offset);
    assert!((target - (origin + Vec3::Z * 16_384.0 + offset)).length() < 1e-2);
}

#[test]
fn the_spawn_beams_play_straight_up_whatever_the_player_faces() {
    // `cg_event.c` passes `ang = (0, 0, 1)` to `FX_PlayEffectID` for the teleport and
    // Jedi Master events, not the player's angles.
    let still = sjk_protocol::EntityState::zero(3, &sjk_protocol::LEGACY_ENTITY_FIELDS);
    let mut turned = still.clone();
    turned.set_raw_field(9, 90.0f32.to_bits()); // yaw
    for event in [34, 64, 65] {
        for player in [&still, &turned] {
            let direction = combat_effects::event_direction(event, player);
            assert_eq!(direction, [0.0, 0.0, 1.0], "event {event}");
        }
    }
}

/// `demp2/altdetonate`'s screen flash.
const FLASH: &str = "Flash
{
	life	120
	rgb { start 1 1 1 }
	size { start 2048 }
	shaders [ gfx/effects/whiteFlash ]
}
";

/// `demp2/altdetonate`'s flash colour as authored: white to a slight cyan, dimmed at
/// random every frame.
const RANDOM_FLASH: &str = "Flash
{
	spawnFlags	rgbComponentInterpolation
	life	120
	rgb
	{
		start	0.7843 1 1 1 1 1
		end	0 0 0
		parm	50
		flags	random linear nonlinear
	}
	size { start 2048 }
	shaders [ gfx/effects/whiteFlash ]
}
";

/// Play the test flash at the origin, seen from `eye` looking along +X.
fn play_flash(eye: Option<Vec3>) -> Vec<Particle> {
    play_flash_effect(FLASH, eye)
}

fn play_flash_effect(source: &str, eye: Option<Vec3>) -> Vec<Particle> {
    let mut vfs = VirtualFileSystem::new();
    vfs.mount_memory("test", [("effects/test/flash.efx", source)])
        .expect("mount the test effect");
    let mut auxiliary = crate::effect_aux::Runtime::default();
    if let Some(eye) = eye {
        auxiliary.set_viewer(eye, Vec3::X * 3.0);
    }
    let mut particles = Vec::new();
    spawn_effect(
        &mut particles,
        &mut auxiliary,
        &mut EffectLibrary::default(),
        &vfs,
        "test/flash",
        Vec3::ZERO,
        Instant::now(),
        64,
        0,
        &mut None,
        Quat::IDENTITY,
    );
    particles
}

#[test]
fn a_flash_lights_only_a_viewer_near_and_facing_it() {
    // Slash #125: a DEMP2 shock across the map whitened the screen.
    assert!(play_flash(Some(Vec3::new(-2_000.0, 0.0, 0.0))).is_empty());
    assert!(
        play_flash(Some(Vec3::new(300.0, 0.0, 0.0))).is_empty(),
        "behind"
    );
    assert!(play_flash(None).is_empty());
    let near = play_flash(Some(Vec3::new(-450.0, 0.0, 0.0)));
    assert_eq!(near.len(), 1);
    let (_, _, rgb) = near[0].sample_envelopes(0.0);
    for channel in rgb {
        assert!((channel - 0.75).abs() < 1e-4, "{rgb:?}");
    }
}

#[test]
fn a_flash_covers_the_view_instead_of_standing_where_it_went_off() {
    // `CFlash::Draw` puts the sprite 12 units in front of the eye: drawn at the shock,
    // it was hidden by nearer walls and tinted the sky behind it (Slash #125).
    let near = play_flash(Some(Vec3::new(-450.0, 0.0, 0.0)));
    assert!(matches!(
        near[0].shape,
        crate::particle_types::PrimitiveShape::ScreenFlash
    ));
}

#[test]
fn a_random_flash_colour_dims_as_one_and_never_changes_hue() {
    // `CParticle::UpdateRGB` draws one `flrand` a frame for the whole colour. Drawn per
    // channel, the white DEMP2 flash went magenta, yellow and purple frame to frame.
    let flash = play_flash_effect(RANDOM_FLASH, Some(Vec3::new(-50.0, 0.0, 0.0)));
    assert_eq!(flash.len(), 1);
    let mut ratio = None;
    let mut dimmed = false;
    for frame in 0..60 {
        let (_, _, [red, green, blue]) = flash[0].sample_envelopes(frame as f32 * 0.002);
        assert_eq!(green, blue, "frame {frame}");
        if green > 1e-3 {
            let red_over_green = red / green;
            assert!((0.78..=1.0001).contains(&red_over_green), "frame {frame}");
            let first = *ratio.get_or_insert(red_over_green);
            assert!((red_over_green - first).abs() < 1e-4, "frame {frame}");
        }
        let (_, _, [_, start_green, _]) = flash[0].sample_envelopes(0.0);
        dimmed |= (green - start_green).abs() > 1e-3;
    }
    assert!(dimmed, "the random flag still dims the flash");
}

/// `env/beam_lights`' runner: `count 0 0.6` rounds to one run about one time in six, and a
/// run plays one random `playFx` entry, never both.
#[test]
fn a_runner_rolls_its_count_and_plays_one_random_entry() {
    const RUNNER: &str = "FxRunner\n{\n\tcount 0 0.6\n\tplayFx [ test/a test/b ]\n}\n";
    const ONE: &str = "Particle\n{\n\tlife 100\n\tshaders [ gfx/test/dot ]\n}\n";
    let mut vfs = VirtualFileSystem::new();
    vfs.mount_memory(
        "test",
        [
            ("effects/test/runner.efx", RUNNER),
            ("effects/test/a.efx", ONE),
            ("effects/test/b.efx", ONE),
        ],
    )
    .expect("mount the test effects");
    let mut library = EffectLibrary::default();
    let (mut runs, plays) = (0, 600);
    for seed in 0..plays {
        let mut particles = Vec::new();
        spawn_effect(
            &mut particles,
            &mut crate::effect_aux::Runtime::default(),
            &mut library,
            &vfs,
            "test/runner",
            Vec3::ZERO,
            Instant::now(),
            seed,
            0,
            &mut None,
            Quat::IDENTITY,
        );
        assert!(
            particles.len() <= 1,
            "one entry per run, got {}",
            particles.len()
        );
        runs += particles.len();
    }
    // 1/6 of 600 is 100.
    assert!((60..=140).contains(&runs), "{runs} runs in {plays}");
}
