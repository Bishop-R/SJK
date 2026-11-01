//! World shots of the ghost editor (`ghost edit`): duel6 with a run written from its
//! first spawn point, the editor's column over it with the whole run, then cut at its
//! first and last moves, then hidden to look about with the path drawn; and a 4:3 window.
//! Ignored like the other world shots.

use super::*;

#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn duel6_ghost_editor() {
    on_big_stack(|| {
        for (size, prefix) in [
            ([1920, 1080], "ghost-editor"),
            ([1440, 1080], "ghost-editor-4x3"),
        ] {
            let Some((mut gpu, profile)) = open("maps/mp/duel6.bsp", size, None, &[]) else {
                return;
            };
            let (eye, yaw) = assets::initial_camera(&gpu.bsp).expect("a spawn point");
            // The spawn's eye is the view height over the origin.
            let origin = Vec3::from_array(eye) - Vec3::Z * 36.0;
            let ahead = Vec3::new(yaw.to_radians().cos(), yaw.to_radians().sin(), 0.0);
            let camera = origin - ahead * 140.0 + Vec3::Z * 150.0;
            let (look_yaw, pitch) = look(
                camera.to_array(),
                (origin + ahead * 260.0 - Vec3::Z * 24.0).to_array(),
            );
            aim(&mut gpu, camera.to_array(), look_yaw, pitch);
            if let Some(console) = gpu.console.as_mut() {
                console.close_for_connection();
            }
            let _ = frame(&mut gpu, 40);
            gpu.shot_ghost_editor(
                profile.path().join("ghosts"),
                "mp/duel6",
                origin.to_array(),
                yaw,
            );
            println!(
                "{}",
                shoot(&mut gpu, 8, &format!("{prefix}-whole")).display()
            );
            if prefix != "ghost-editor" {
                continue;
            }
            gpu.shot_ghost_editor_cut();
            println!("{}", shoot(&mut gpu, 4, "ghost-editor-cut").display());
            gpu.shot_ghost_editor_look();
            println!("{}", shoot(&mut gpu, 4, "ghost-editor-look").display());
        }
    });
}
