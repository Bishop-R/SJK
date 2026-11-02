//! 3D sound's looking (`audio_spatial.rs`): the hearing point, occlusion from rays,
//! the room's reverb from its rays.

use super::*;

const NEAR: Attenuation = Attenuation::Linear {
    full_volume_distance: 256.0,
    falloff_per_unit: 0.0008,
};

fn request(source: u32, origin: [f32; 3]) -> PlayRequest {
    PlayRequest {
        origin: Some(origin),
        source: SourceId(source),
        channel: sjk_audio::ChannelId(1),
        volume: 1.0,
        attenuation: NEAR,
    }
}

#[test]
fn the_hearing_point_is_the_head_unless_the_camera_is_far() {
    let origin = Vec3::new(100.0, 0.0, 0.0);
    // First person: the eyes.
    let eyes = Vec3::new(100.0, 0.0, 36.0);
    assert_eq!(hearing_point(eyes, Some(origin)), eyes);
    // Third person, the camera behind and above: the head, as high as allowed.
    let behind = Vec3::new(20.0, 0.0, 70.0);
    assert_eq!(
        hearing_point(behind, Some(origin)),
        Vec3::new(100.0, 0.0, 36.0)
    );
    // A camera far off (spectating, the drone) and no player: the camera.
    let far = Vec3::new(600.0, 0.0, 36.0);
    assert_eq!(hearing_point(far, Some(origin)), far);
    assert_eq!(hearing_point(behind, None), behind);
}

/// A wall across x = 50 from y -1000 to 1000 and z -1000 to 1000, ending at the
/// sky above z 500.
fn wall(from: Vec3, to: Vec3) -> Hit {
    if (from.x < 50.0) == (to.x < 50.0) {
        return Hit::OPEN;
    }
    let fraction = (50.0 - from.x) / (to.x - from.x);
    let point = from + (to - from) * fraction;
    if point.z > 500.0 {
        return Hit {
            fraction,
            sky: true,
        };
    }
    Hit {
        fraction,
        sky: false,
    }
}

fn tick(tracker: &mut Tracker, now: Instant) -> Vec<(SourceId, f32)> {
    tracker.tick(now, Vec3::ZERO, Vec3::new(0.0, -1.0, 0.0), wall);
    tracker.occlusions().to_vec()
}

#[test]
fn a_sound_behind_a_wall_is_occluded_and_one_on_it_is_not() {
    let mut tracker = Tracker {
        enabled: true,
        ..Tracker::default()
    };
    let now = Instant::now();
    tracker.hear(&request(1, [300.0, 0.0, 0.0]), now);
    tracker.hear(&request(2, [-300.0, 0.0, 0.0]), now);
    // An impact on the wall's near face: the wall it is on hides nothing.
    tracker.hear(&request(3, [52.0, 0.0, 0.0]), now);
    let sent = tick(&mut tracker, now);
    assert_eq!(sent, vec![(SourceId(1), 1.0)], "only the change is sent");
    // Nothing changed: nothing more to send.
    assert!(tick(&mut tracker, now).is_empty());
}

#[test]
fn the_players_own_and_listener_relative_sounds_are_left_alone() {
    let mut tracker = Tracker {
        enabled: true,
        ..Tracker::default()
    };
    let now = Instant::now();
    tracker.observe(
        &crate::audio_output::AudioCommand::ListenerSource(Some(SourceId(4))),
        now,
    );
    tracker.hear(&request(4, [300.0, 0.0, 0.0]), now);
    tracker.hear(
        &PlayRequest {
            attenuation: Attenuation::None,
            ..request(5, [300.0, 0.0, 0.0])
        },
        now,
    );
    assert!(tick(&mut tracker, now).is_empty());
}

#[test]
fn a_source_moving_out_from_behind_the_wall_opens_and_old_ones_are_forgotten() {
    let mut tracker = Tracker {
        enabled: true,
        ..Tracker::default()
    };
    let now = Instant::now();
    tracker.hear(&request(1, [300.0, 0.0, 0.0]), now);
    assert_eq!(tick(&mut tracker, now), vec![(SourceId(1), 1.0)]);
    tracker.observe(
        &crate::audio_output::AudioCommand::SourcePosition(SourceId(1), [-200.0, 0.0, 0.0]),
        now,
    );
    assert_eq!(tick(&mut tracker, now), vec![(SourceId(1), 0.0)]);
    tracker.observe(
        &crate::audio_output::AudioCommand::SourcePosition(SourceId(1), [300.0, 0.0, 0.0]),
        now,
    );
    // Not heard for longer than FORGET_AFTER: no longer traced.
    assert!(tick(&mut tracker, now + FORGET_AFTER * 2).is_empty());
}

#[test]
fn switched_off_it_notes_nothing() {
    let mut tracker = Tracker::default();
    let now = Instant::now();
    tracker.observe(
        &crate::audio_output::AudioCommand::Play(
            sjk_audio::SoundHandle(0),
            request(1, [300.0, 0.0, 0.0]),
        ),
        now,
    );
    assert!(tick(&mut tracker, now).is_empty());
    assert_eq!(tracker.switch_changed(), None);
    tracker.enabled = true;
    assert_eq!(tracker.switch_changed(), Some(true));
    assert_eq!(tracker.switch_changed(), None);
}

fn rays_at(distance: f32, sky: bool, closed: usize) -> [Hit; ROOM_RAYS] {
    std::array::from_fn(|index| {
        if index < closed {
            Hit {
                fraction: distance / 2_048.0,
                sky,
            }
        } else {
            Hit::OPEN
        }
    })
}

#[test]
fn the_room_rays_size_the_reverb() {
    let cupboard = room_reverb(&rays_at(48.0, false, ROOM_RAYS));
    let hall = room_reverb(&rays_at(1_500.0, false, ROOM_RAYS));
    let courtyard = room_reverb(&rays_at(300.0, false, ROOM_RAYS / 2));
    assert_eq!(cupboard.size, 0.0);
    assert_eq!(hall.size, 1.0);
    assert!(
        hall.wet > cupboard.wet && cupboard.wet > 0.0,
        "{cupboard:?} {hall:?}"
    );
    assert!(courtyard.wet < room_reverb(&rays_at(300.0, false, ROOM_RAYS)).wet);
    // Open air and the sky do not ring.
    assert_eq!(
        room_reverb(&rays_at(0.0, false, 0)),
        ReverbParams::default()
    );
    assert_eq!(
        room_reverb(&rays_at(300.0, true, ROOM_RAYS)),
        ReverbParams::default()
    );
}

#[test]
fn a_full_round_of_room_rays_gives_one_reverb() {
    let mut tracker = Tracker {
        enabled: true,
        ..Tracker::default()
    };
    let now = Instant::now();
    let mut rooms = 0;
    for _ in 0..ROOM_RAYS / ROOM_RAYS_PER_TICK * 2 {
        tracker.tick(now, Vec3::ZERO, Vec3::Y, |_, _| Hit {
            fraction: 0.25,
            sky: false,
        });
        rooms += usize::from(tracker.take_reverb().is_some());
    }
    assert_eq!(rooms, 2);
}

#[test]
fn the_room_directions_cover_the_sphere() {
    let directions = room_directions();
    let sum = directions.iter().fold(Vec3::ZERO, |sum, d| sum + *d);
    assert!(sum.length() < 1.0, "lopsided: {sum}");
    assert!(directions.iter().all(|d| (d.length() - 1.0).abs() < 1.0e-4));
}

#[test]
fn a_segment_meets_a_box_only_through_it() {
    let (low, high) = (Vec3::splat(-10.0), Vec3::splat(10.0));
    assert!(segment_meets_box(
        Vec3::new(-50.0, 0.0, 0.0),
        Vec3::new(50.0, 0.0, 0.0),
        low,
        high
    ));
    assert!(!segment_meets_box(
        Vec3::new(-50.0, 20.0, 0.0),
        Vec3::new(50.0, 20.0, 0.0),
        low,
        high
    ));
    assert!(!segment_meets_box(
        Vec3::new(-50.0, 0.0, 0.0),
        Vec3::new(-20.0, 0.0, 0.0),
        low,
        high
    ));
}

/// Noting sounds and a tick's looking take no allocation.
#[cfg(debug_assertions)]
#[test]
fn a_tick_allocates_nothing() {
    let mut tracker = Tracker {
        enabled: true,
        ..Tracker::default()
    };
    let now = Instant::now();
    let before = assert_no_alloc::violation_count();
    assert_no_alloc::assert_no_alloc(|| {
        for round in 0..200_u32 {
            tracker.hear(&request(round % 60, [300.0, round as f32, 0.0]), now);
            tracker.tick(now, Vec3::ZERO, Vec3::Y, wall);
            let _ = tracker.take_reverb();
        }
    });
    assert_eq!(assert_no_alloc::violation_count(), before);
}
