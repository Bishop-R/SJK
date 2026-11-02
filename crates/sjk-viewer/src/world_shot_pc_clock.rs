//! World shots of the PC clock (`r_dayClock`) in the SJK (recommended) preset, which
//! keeps Sun and sky off: ffa3 from its start camera with the clock off, with it on from
//! the map load, and with it switched on live. Each shot prints what the renderer read
//! (the day settings, the night factor) and how bright the sky and the ground came out.
//! Ignored like the other world shots: run on the PC at night, the shots are in
//! `target/world-shots`.
//!
//! Run: `JKA_GAME_DATA=<GameData> cargo test -p sjk-viewer pc_clock_night -- --ignored
//! --test-threads=1 --nocapture`.
//!
//! `pc_clock_every_preset` loads a map (`PC_CLOCK_MAP`, default the retail
//! `maps/t1_fatal.bsp` of hub report #219) in each graphics preset with the clock on and
//! off and draws it, so every pipeline a map load builds, the day tint's among them,
//! compiles: run it with `WGPU_BACKEND=dx12` too, with and without `dxcompiler.dll` on
//! the `PATH` (DXC, else FXC); `PC_CLOCK_CASES=Sjk:1,Experimental:1` picks the cases.

use super::*;

/// Sun and sky off as in the SJK and Vanilla presets, with the clock on or off.
fn cvars(clock: &'static str) -> [(&'static str, &'static str); 5] {
    cvars_sun("0", clock)
}

/// [`cvars`] with Sun and sky as given.
fn cvars_sun(sun: &'static str, clock: &'static str) -> [(&'static str, &'static str); 5] {
    [
        ("r_dayNight", sun),
        ("r_worldSunShadows", "0"),
        ("r_actorSunShadows", "0"),
        ("r_volumetrics", "0"),
        (
            crate::world_materials::shadows::day::live::PC_CLOCK_CVAR,
            clock,
        ),
    ]
}

/// Mean brightness (0..255) of the top quarter (sky) and the bottom quarter (ground).
fn brightness(image: &image::RgbaImage) -> (f32, f32) {
    let mean = |rows: std::ops::Range<u32>| {
        let mut sum = 0.0;
        let mut count = 0.0;
        for y in rows {
            for x in 0..image.width() {
                let p = image.get_pixel(x, y).0;
                sum +=
                    0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2]);
                count += 1.0;
            }
        }
        sum / count
    };
    let h = image.height();
    (mean(0..h / 4), mean(h * 3 / 4..h))
}

fn report(gpu: &GpuState, name: &str, image: &image::RgbaImage) {
    let clock = gpu.context.dynamic_light_settings.day_clock();
    let (sky, ground) = brightness(image);
    eprintln!(
        "pc clock {name}: day {:?}; clock values {:?}; night factor {:.2}; sky {sky:.1}, ground {ground:.1}",
        gpu.context.sun_shadows.day,
        clock.values(),
        clock.night_tint(),
    );
}

#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn pc_clock_night() {
    on_big_stack(|| {
        let size = [1280, 720];
        let map = "maps/mp/ffa3.bsp";
        let mut shots = Vec::new();

        let Some((mut gpu, _off)) = open(map, size, None, &cvars("0")) else {
            return;
        };
        let off = frame(&mut gpu, 30);
        report(&gpu, "off", &off);
        shots.push(off);

        // Switched on live: the world darkens at once, the sky waits for a map load.
        if let Some(console) = gpu.console.as_mut() {
            console.set_cvar(
                crate::world_materials::shadows::day::live::PC_CLOCK_CVAR,
                "1",
            );
        }
        let live = frame(&mut gpu, 30);
        report(&gpu, "on live", &live);
        shots.push(live);
        drop(gpu);

        let Some((mut gpu, _on)) = open(map, size, None, &cvars("1")) else {
            return;
        };
        let loaded = frame(&mut gpu, 30);
        report(&gpu, "on from load", &loaded);
        shots.push(loaded);
        drop(gpu);

        // For comparison: Sun and sky on, the clock on.
        let Some((mut gpu, _sun)) = open(map, size, None, &cvars_sun("1", "1")) else {
            return;
        };
        let sun = frame(&mut gpu, 30);
        report(&gpu, "with sun and sky", &sun);
        shots.push(sun);

        let path = sheet(&shots, 2, 640, "pc-clock-night");
        eprintln!("pc clock sheet: {}", path.display());
    });
}

#[test]
#[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
fn pc_clock_every_preset() {
    on_big_stack(|| {
        let map = std::env::var("PC_CLOCK_MAP").unwrap_or_else(|_| "maps/t1_fatal.bsp".into());
        // `PC_CLOCK_CASES` (e.g. `Sjk:1,Experimental:1`) narrows the sweep: a DX12 load
        // under FXC takes minutes.
        let cases = std::env::var("PC_CLOCK_CASES").unwrap_or_default();
        for level in crate::graphics_quality::Level::ALL {
            for clock in ["1", "0"] {
                if !cases.is_empty() && !cases.split(',').any(|c| c == format!("{level:?}:{clock}"))
                {
                    continue;
                }
                let mut cvars = crate::graphics_quality::preset_cvars(level);
                cvars.push((
                    crate::world_materials::shadows::day::live::PC_CLOCK_CVAR,
                    clock,
                ));
                let Some((mut gpu, _profile)) = open(&map, [640, 360], None, &cvars) else {
                    return;
                };
                let shot = frame(&mut gpu, 10);
                report(&gpu, &format!("{map} {level:?} clock {clock}"), &shot);
            }
        }
    });
}
