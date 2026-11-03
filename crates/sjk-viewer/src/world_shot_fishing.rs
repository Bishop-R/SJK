//! World shots of the Fishing toy ([`crate::fishing`]): two rods on the floor ahead of a
//! map's first spawn point, without a session (the shots place them): one fishing a
//! pond (or water, where the map has it ahead), one with a fish on the line: a sun carp
//! in the first bucket, Old Whiskers on the second line. Ignored like the other world
//! shots.
//!
//! `SJK_FISHING_MAPS` names the maps (`duel6,ffa3`; default duel6). Each map gives one
//! sheet of four views: from behind, from the side, close up, and from above.

use super::*;

const VIEWS: [(Vec3, Vec3); 4] = [
    // The camera from the first fisher in its yaw frame, and where it looks at.
    (Vec3::new(-110.0, -40.0, 50.0), Vec3::new(50.0, 0.0, -10.0)),
    (Vec3::new(30.0, -150.0, 30.0), Vec3::new(40.0, 0.0, -10.0)),
    (Vec3::new(20.0, -60.0, 16.0), Vec3::new(72.0, 0.0, -20.0)),
    (Vec3::new(10.0, 0.0, 170.0), Vec3::new(60.0, 0.0, -20.0)),
];

#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn rods_at_the_first_spawn() {
    on_big_stack(|| {
        let maps = std::env::var("SJK_FISHING_MAPS").unwrap_or_else(|_| "duel6".to_owned());
        for map in maps.split(',').map(str::trim).filter(|map| !map.is_empty()) {
            shoot_map(map);
        }
    });
}

/// The origin a player standing on the floor under `point` has (24 over the floor).
fn standing(gpu: &mut GpuState, point: Vec3) -> Vec3 {
    let trace = gpu.bsp.trace_box_with(
        &mut gpu.trace_scratch,
        point.to_array(),
        (point - Vec3::Z * 512.0).to_array(),
        sjk_bsp::Aabb::new([0.0; 3], [0.0; 3]).expect("point bounds are valid"),
        1,
    );
    Vec3::from_array(trace.end_position) + Vec3::Z * 24.0
}

fn shoot_map(map: &str) {
    let Some((mut gpu, _profile)) = open(&format!("maps/mp/{map}.bsp"), [1280, 720], None, &[])
    else {
        return;
    };
    let (spawn, yaw) = assets::initial_camera(&gpu.bsp).expect("a spawn point");
    let turn = glam::Quat::from_rotation_z(yaw);
    let eye = Vec3::from_array(spawn);
    let first = standing(&mut gpu, eye + turn * Vec3::new(120.0, 0.0, 0.0));
    let second = standing(&mut gpu, eye + turn * Vec3::new(120.0, 110.0, 0.0));
    gpu.third_person_choice = true;
    gpu.fishing.shot_rods = vec![
        (first.to_array(), yaw, false, 6),
        (second.to_array(), yaw + 0.3, true, 11),
    ];
    let mut images = Vec::new();
    for (index, (from, at)) in VIEWS.iter().enumerate() {
        let camera = first + turn * *from;
        let target = first + turn * *at;
        let (look_yaw, look_pitch) = look(camera.to_array(), target.to_array());
        aim(&mut gpu, camera.to_array(), look_yaw, look_pitch);
        images.push(frame(&mut gpu, if index == 0 { 40 } else { 12 }));
    }
    println!("{map}: rods at {first:?} and {second:?}");
    println!(
        "{}",
        sheet(&images, 2, 960, &format!("fishing-{map}")).display()
    );
}
