//! A world shot of the Toy Master's toys ([`crate::toy_master`]) on duel6: a toy of the
//! model `SJK_TEST_TOY_MODEL` names, from the hub pack `SJK_TEST_TOY_PACK` names (mounted
//! as the game mounts the hub's packs), standing still, shrinking away, popping back and
//! spinning. The toys are the hub's secrets, so no model of theirs is in this
//! repository. Ignored like the other world shots.

use super::*;

#[test]
#[ignore = "renders with the GPU, the installed game data named by JKA_GAME_DATA and a hub pack"]
fn duel6_toy_master() {
    on_big_stack(|| {
        let (Ok(model), Some(pack)) = (
            std::env::var("SJK_TEST_TOY_MODEL"),
            std::env::var_os("SJK_TEST_TOY_PACK"),
        ) else {
            println!("SJK_TEST_TOY_MODEL and SJK_TEST_TOY_PACK are not set: no toy to draw");
            return;
        };
        // The pack in a settings folder's cache of its own, as the identity keeps it.
        let pack = std::path::Path::new(&pack);
        let settings = std::env::temp_dir().join("sjk-toy-master-shot");
        let cache = crate::sjk_packs::directory_in(&settings);
        std::fs::create_dir_all(&cache).expect("the cache folder");
        std::fs::copy(pack, cache.join(pack.file_name().expect("a pack file")))
            .expect("the pack copied");
        crate::sjk_packs::mount_at_start(&settings);
        let Some((mut gpu, _profile)) = open("maps/mp/duel6.bsp", [1600, 900], None, &[]) else {
            return;
        };
        let shots = crate::menu_backdrop::tour_for("Yavin Training Grounds").expect("duel6's tour");
        let (from, at) = (
            Vec3::from_array(shots[0].from),
            Vec3::from_array(shots[0].at),
        );
        // Four toys in a row across the view, 120 units ahead, facing the camera: from
        // the right, still, shrinking away, popping back, spinning.
        let ahead = (at - from).with_z(0.0).normalize_or(Vec3::X);
        let across = Vec3::new(-ahead.y, ahead.x, 0.0);
        let floor = from + ahead * 120.0 - Vec3::Z * 60.0;
        let yaw = ahead.y.atan2(ahead.x).to_degrees() + 180.0;
        let show = sjk_identity::toy_master::Show {
            vanish: 180,
            gone: 1_200,
            pop: 450,
            spin: 30_000,
            turns: 3.0,
            ..Default::default()
        };
        let moments = [
            None,
            Some(std::time::Duration::from_millis(90)),
            Some(std::time::Duration::from_millis(1_520)),
            Some(std::time::Duration::from_millis(1_830 + 400)),
        ];
        let toys: Vec<([f32; 3], f32, Option<std::time::Duration>)> = moments
            .iter()
            .enumerate()
            .map(|(index, since)| {
                (
                    (floor + across * (index as f32 - 1.5) * 40.0).to_array(),
                    yaw,
                    *since,
                )
            })
            .collect();
        gpu.shot_toys(&model, show, &toys);
        let (yaw, pitch) = look(from.to_array(), (floor + Vec3::Z * 15.0).to_array());
        aim(&mut gpu, from.to_array(), yaw, pitch);
        gpu.is_menu_world = false;
        println!("{}", shoot(&mut gpu, 90, "duel6-toy-master").display());
    });
}

/// A show in steps (02/11/2026) on duel6, the toy of `SJK_TEST_TOY_MODEL` from the pack
/// of `SJK_TEST_TOY_PACK` (which also holds the ring, `SJK_TEST_TOY_RING`): its ring on
/// the ground round it 40 seconds into its song, seen from above, and its drops thrown
/// and landed round it at the song's end, seen from a step back.
#[test]
#[ignore = "renders with the GPU, the installed game data named by JKA_GAME_DATA and a hub pack"]
fn duel6_toy_master_show() {
    on_big_stack(|| {
        let (Ok(model), Ok(ring), Some(pack)) = (
            std::env::var("SJK_TEST_TOY_MODEL"),
            std::env::var("SJK_TEST_TOY_RING"),
            std::env::var_os("SJK_TEST_TOY_PACK"),
        ) else {
            println!("SJK_TEST_TOY_MODEL, _RING and _PACK are not set: no toy to draw");
            return;
        };
        let pack = std::path::Path::new(&pack);
        let settings = std::env::temp_dir().join("sjk-toy-master-show-shot");
        let cache = crate::sjk_packs::directory_in(&settings);
        std::fs::create_dir_all(&cache).expect("the cache folder");
        std::fs::copy(pack, cache.join(pack.file_name().expect("a pack file")))
            .expect("the pack copied");
        crate::sjk_packs::mount_at_start(&settings);
        let Some((mut gpu, _profile)) = open("maps/mp/duel6.bsp", [1600, 900], None, &[]) else {
            return;
        };
        let shots = crate::menu_backdrop::tour_for("Yavin Training Grounds").expect("duel6's tour");
        let (from, at) = (
            Vec3::from_array(shots[0].from),
            Vec3::from_array(shots[0].at),
        );
        let ahead = (at - from).with_z(0.0).normalize_or(Vec3::X);
        let floor = from + ahead * 220.0 - Vec3::Z * 60.0;
        let yaw = ahead.y.atan2(ahead.x).to_degrees() + 180.0;
        let step = |at: u32| sjk_identity::toy_master::Step {
            at,
            ..Default::default()
        };
        let show = sjk_identity::toy_master::Show {
            vanish: 180,
            gone: 1_200,
            pop: 450,
            spin: 30_000,
            turns: 3.0,
            steps: vec![
                sjk_identity::toy_master::Step {
                    spin: 3.0,
                    dance: true,
                    ring: true,
                    ..step(0)
                },
                sjk_identity::toy_master::Step {
                    spin: 3.0,
                    spit: true,
                    ..step(60_000)
                },
                step(83_000),
            ],
            length: 90_000,
            reach: 400.0,
            ring,
            ring_radius: 100.0,
            voice: Vec::new(),
        };
        let lead = 1_830;
        let kinds = {
            use sjk_identity::toy_master::DropKind::{Credits, Holocron, Shards};
            [
                Credits, Shards, Credits, Holocron, Credits, Shards, Credits, Credits, Shards,
                Credits, Holocron, Credits, Shards, Credits, Credits,
            ]
        };
        let ms = |ms: u64| Some(std::time::Duration::from_millis(ms));
        // The ring 40 s in, from above.
        let floor = gpu.shot_show(
            &model,
            show.clone(),
            &[(floor.to_array(), yaw, ms(lead + 40_000))],
            7,
            &[],
        );
        let above = floor - ahead * 300.0 + Vec3::Z * 520.0;
        let (look_yaw, pitch) = look(above.to_array(), floor.to_array());
        aim(&mut gpu, above.to_array(), look_yaw, pitch);
        gpu.is_menu_world = false;
        println!("{}", shoot(&mut gpu, 90, "duel6-toy-master-ring").display());
        // The drops landed, at the song's end.
        let floor = gpu.shot_show(
            &model,
            show,
            &[(floor.to_array(), yaw, ms(lead + 86_000))],
            7,
            &kinds,
        );
        let back = floor - ahead * 220.0 + Vec3::Z * 150.0;
        let (look_yaw, pitch) = look(back.to_array(), floor.to_array());
        aim(&mut gpu, back.to_array(), look_yaw, pitch);
        println!(
            "{}",
            shoot(&mut gpu, 90, "duel6-toy-master-drops").display()
        );
    });
}
