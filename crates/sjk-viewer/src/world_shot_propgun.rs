//! World shots of the Prop gun ([`crate::propgun`]) on a recorded game: the demo
//! `SJK_TEST_DEMO` names is played without a server, with the gun `SJK_TEST_PROPGUN`
//! names (a `propguns/<id>.gun` in the game data; SJK ships none) held by every player,
//! seen around a few of them and in first person. Ignored like the other world shots;
//! skipped when either is not set.

use super::*;

#[test]
#[ignore = "renders with the GPU, the installed game data, a prop gun in it and the demo SJK_TEST_DEMO names"]
fn propgun_on_a_demo() {
    on_big_stack(|| {
        let Some(demo) = std::env::var_os("SJK_TEST_DEMO").map(PathBuf::from) else {
            println!("SJK_TEST_DEMO is not set: skipped");
            return;
        };
        let Ok(gun) = std::env::var("SJK_TEST_PROPGUN") else {
            println!("SJK_TEST_PROPGUN is not set: skipped");
            return;
        };
        let probe = demo_playback::Session::open(&demo, demo_playback::Camera::FirstPerson, 60)
            .expect("the demo");
        let own = probe.latest_snapshot().player.client_num();
        let targets: Vec<u16> = probe
            .latest_snapshot()
            .entities
            .iter()
            .map(|entity| entity.number())
            .filter(|number| *number < 32 && *number != own)
            .take(3)
            .collect();
        let mut shots = Vec::new();
        for camera in std::iter::once(demo_playback::Camera::FirstPerson)
            .chain(targets.into_iter().map(demo_playback::Camera::Orbit))
        {
            let Some(mut gpu) = card_outline_shots::open_demo(&demo, camera, [1200, 900]) else {
                return;
            };
            gpu.shot_propgun(&gun, true);
            if camera == demo_playback::Camera::FirstPerson {
                // Two bolts fired ahead, from by the eyes: the shot's last frame comes about
                // 1.37 s of demo time after its first snapshot, so these have flown about
                // 0.1 and 0.25 s by then.
                let player = &probe.latest_snapshot().player;
                let eye = Vec3::from_array(player.origin()) + Vec3::Z * player.view_height() as f32;
                let [pitch, yaw, _] = player.view_angles();
                let way = crate::propgun::looking(pitch, yaw);
                let right = Vec3::new(yaw.to_radians().sin(), -yaw.to_radians().cos(), 0.0);
                let time = i64::from(probe.latest_snapshot().server_time);
                for age in [-1_266, -1_116] {
                    gpu.shot_propgun_bolt(
                        eye + way * 16.0 + right * 5.0 - Vec3::Z * 5.0,
                        way,
                        time,
                        age,
                    );
                }
            }
            let shot = frame(&mut gpu, 30);
            if matches!(camera, demo_playback::Camera::Orbit(_)) {
                // The player framed whole stands in the middle: close on them.
                shots.push(image::imageops::crop_imm(&shot, 380, 390, 400, 300).to_image());
            } else {
                println!(
                    "{}",
                    sheet(
                        std::slice::from_ref(&shot),
                        1,
                        1200,
                        &format!("propgun_{gun}_view")
                    )
                    .display()
                );
                shots.push(shot);
            }
            let models: Vec<_> = gpu
                .object_meshes
                .iter()
                .map(|mesh| mesh.appearance.model.clone())
                .filter(|model| model.contains("propguns"))
                .collect();
            println!(
                "{camera:?}: view {:?}, meshes {models:?}",
                gpu.propgun_view().map(|view| view.mesh)
            );
        }
        println!(
            "{}",
            sheet(&shots, 2, 800, &format!("propgun_{gun}")).display()
        );
    });
}
