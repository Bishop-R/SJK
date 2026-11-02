//! A world built during a map change, adopted after the window changed size: the
//! new map is built on a worker at the size the window had when the change began
//! (`session_transition.rs`), and the window can be resized during those seconds
//! (a player's 2026.1011.1 crash: its depth stayed 1280x720 under a 1920x1080
//! frame, and wgpu refused the first frame). Off-screen, ignored like the other
//! world shots: it needs a GPU adapter and the installed game data named by
//! `JKA_GAME_DATA`.
//!
//! Run: `JKA_GAME_DATA=<GameData> cargo test -p sjk-viewer map_change_after_resize --
//! --ignored --test-threads=1 --nocapture`.

use super::*;

#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn map_change_after_resize() {
    on_big_stack(|| {
        let screen = [1920, 1080];
        let Some((mut gpu, _profile)) = open("maps/mp/duel6.bsp", screen, None, &[]) else {
            return;
        };
        let _ = frame(&mut gpu, 4);

        // The next map, built on the same context at the window's earlier size, the
        // way the world-install worker builds it (no console, no menu: the running
        // world hands its own over).
        let (mut input, _next_profile) = world_input("maps/mp/duel6.bsp", None, &[]);
        input.console = None;
        let built = pollster::block_on(GpuState::new_with_context(
            Arc::clone(&gpu.context),
            [1280, 720],
            input,
            None,
        ))
        .expect("the next map is built");
        assert_eq!([built.size.width, built.size.height], [1280, 720]);

        let mut adopted = gpu.adopt_world(Box::new(built));
        assert_eq!(
            [adopted.size.width, adopted.size.height],
            screen,
            "the adopted world takes the window's present size"
        );
        // Frames now go to an image of the window's size, as they go to the swapchain.
        attach_target(&mut adopted, screen);
        let image = frame(&mut adopted, 4);
        assert_eq!(image.dimensions(), (screen[0], screen[1]));
    });
}

/// The next map's world takes the signature pipelines the last one built
/// (`saber_gpu::Signatures`) instead of building every loaded signature again, and draws a
/// signature blade with them at once. Needs `SJK_TEST_PACKS` (the hub's pack, e.g.
/// `GameData/SJK/assets`) for the blade skins.
#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn map_change_keeps_blade_signatures() {
    on_big_stack(|| {
        super::saber_skins::mount_test_packs();
        let screen = [1280, 720];
        let Some((mut gpu, _profile)) = open("maps/mp/duel6.bsp", screen, None, &[]) else {
            return;
        };
        let _ = frame(&mut gpu, 4);
        gpu.saber_gpu.build_every_signature();
        let started = std::time::Instant::now();
        let loaded = loop {
            let (built, loaded) = gpu.saber_gpu.signatures_built();
            if loaded > 0 && built == loaded {
                break loaded;
            }
            assert!(loaded > 0, "no signature skin loaded: set SJK_TEST_PACKS");
            assert!(
                started.elapsed().as_secs() < 600,
                "{built} of {loaded} built"
            );
            let _ = frame(&mut gpu, 10);
        };
        println!(
            "{loaded} signatures built in {:.1} s",
            started.elapsed().as_secs_f64()
        );

        let (mut input, _next_profile) = world_input("maps/mp/duel6.bsp", None, &[]);
        input.console = None;
        let built = pollster::block_on(GpuState::new_with_context(
            Arc::clone(&gpu.context),
            screen,
            input,
            None,
        ))
        .expect("the next map is built");
        let mut adopted = gpu.adopt_world(Box::new(built));
        attach_target(&mut adopted, screen);
        let (spawn, yaw) = assets::initial_camera(&adopted.bsp).expect("a spawn point");
        let turn = glam::Quat::from_rotation_z(yaw);
        let at = Vec3::from_array(spawn) + turn * Vec3::new(48.0, 0.0, -22.0);
        adopted.saber_skins.shot_blades = vec![(
            crate::saber::Blade {
                base: at.to_array(),
                direction: [0.0, 0.0, 1.0],
                length: 40.0,
                radius: 3.0,
            },
            crate::saber_skins::ShotColor::Skin("saber_sun"),
            2,
        )];
        let _ = frame(&mut adopted, 4);
        assert_eq!(
            adopted.saber_gpu.signatures_built(),
            (loaded, loaded),
            "the next map's world builds no signature again"
        );
    });
}
