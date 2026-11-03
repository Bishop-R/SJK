//! Local-player damage direction and timing from authoritative player state.
//!
//! `Feedback::observe` mirrors the damage-event transition in
//! `codemp/cgame/cg_playerstate.c:505-516`. Direction, health scaling, kick
//! clamping, and screen-coordinate clamping mirror `CG_DamageFeedback` at
//! lines 106-195. The 500ms lifetime is `DAMAGE_TIME` from
//! `codemp/cgame/cg_local.h:44`; the strict `0 < age < DAMAGE_TIME` gate and
//! linear decay mirror `CG_DamageBlendBlob` in `codemp/cgame/cg_view.c`
//! lines 1293-1339. Only the visual style is modernized. `view_kick` is the
//! damage view kick of `CG_OffsetFirstPersonView` (`cg_view.c:967-981`).

use sjk_protocol::PlayerState;

/// Codemp view-kick and damage-feedback lifetime (`cg_local.h:44`).
pub(crate) const DAMAGE_TIME_MILLIS: i32 = 500;

/// Shader-ready screen-space feedback at a presentation time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Sample {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) alpha: f32,
    pub(crate) strength: f32,
    /// The hit took the player's shield (armour) and `cg_hitMarkerShield` is on: the
    /// indicator shows green instead of red.
    pub(crate) shield: bool,
}

/// `cg_hitMarkerShield` (AngeloPapaya's idea, 03/11/2026): the damage indicator turns
/// green when the hit took armour, off by default (stock has only red).
pub(crate) const SHIELD_CVAR: &str = "cg_hitMarkerShield";

/// The green of a shield hit.
const SHIELD_GREEN: [f32; 3] = [0.20, 0.95, 0.35];

impl Sample {
    /// The HUD uniform's `damage_color`: green for a shield hit, else none (red).
    pub(crate) fn color(self) -> [f32; 4] {
        if self.shield {
            [SHIELD_GREEN[0], SHIELD_GREEN[1], SHIELD_GREEN[2], 1.0]
        } else {
            [0.0; 4]
        }
    }
}

/// `DAMAGE_DEFLECT_TIME` / `DAMAGE_RETURN_TIME` (`cg_local.h:42-43`).
const DEFLECT_MILLIS: i32 = 100;
const RETURN_MILLIS: i32 = 400;

#[derive(Clone, Copy, Debug)]
struct Event {
    x: f32,
    y: f32,
    kick: f32,
    /// `cg.v_dmg_pitch` / `cg.v_dmg_roll`, degrees.
    kick_pitch: f32,
    kick_roll: f32,
    server_time: i32,
    /// The player's armour fell with the hit: the shield took (some of) it.
    shield: bool,
}

/// Tracks `damageEvent` and exposes the active 500ms directional pulse.
#[derive(Default)]
pub(crate) struct Feedback {
    previous_event: Option<u8>,
    /// The armour at the last snapshot, to tell a hit the shield took.
    previous_armor: Option<i32>,
    active: Option<Event>,
}

impl Feedback {
    /// Sample only the damage marker; disabling it preserves event tracking and view kick.
    pub(crate) fn marker(
        &self,
        at_time: i32,
        console: Option<&crate::console::ViewerConsole>,
    ) -> Option<Sample> {
        let shield = console
            .and_then(|console| console.bool_cvar(SHIELD_CVAR))
            .unwrap_or(false);
        console
            .and_then(|console| console.bool_cvar("cg_hitmarker"))
            .unwrap_or(false)
            .then(|| self.sample(at_time))
            .flatten()
            .map(|sample| Sample {
                shield: shield && sample.shield,
                ..sample
            })
    }

    /// Whether damage feedback has established stock's `cg.attackerTime` gate.
    pub(crate) fn has_attacker(&self) -> bool {
        self.active.is_some_and(|event| event.server_time != 0)
    }

    /// Consume one authoritative snapshot. The initial state establishes the
    /// transition baseline, exactly as cgame copies its first playerState.
    pub(crate) fn observe(&mut self, player: &PlayerState, server_time: i32) {
        let damage_event = player.damage_event();
        let changed = self
            .previous_event
            .is_some_and(|previous| previous != damage_event);
        self.previous_event = Some(damage_event);
        let armor = player.armor();
        let shield = shield_hit(self.previous_armor.replace(armor), armor);
        if changed && player.damage_count() != 0 {
            self.active = Some(Event {
                shield,
                ..calculate(
                    player.damage_yaw(),
                    player.damage_pitch(),
                    player.damage_count(),
                    player.health(),
                    player.view_angles(),
                    server_time,
                )
            });
        }
    }

    pub(crate) fn sample(&self, at_time: i32) -> Option<Sample> {
        let event = self.active?;
        let age = at_time.wrapping_sub(event.server_time);
        if !(1..DAMAGE_TIME_MILLIS).contains(&age) {
            return None;
        }
        Some(Sample {
            x: event.x,
            y: event.y,
            alpha: 1.0 - age as f32 / DAMAGE_TIME_MILLIS as f32,
            strength: event.kick / 10.0,
            shield: event.shield,
        })
    }

    /// The view kick `[pitch, roll]` in Quake degrees at `at_time`: a linear
    /// deflection over `DAMAGE_DEFLECT_TIME`, then a linear return over
    /// `DAMAGE_RETURN_TIME` (`cg_view.c:967-981`). Presentation times before
    /// the damage snapshot yield nothing rather than a negative deflection.
    pub(crate) fn view_kick(&self, at_time: i32) -> Option<[f32; 2]> {
        let event = self.active?;
        let age = at_time.wrapping_sub(event.server_time);
        let ratio = if age < 0 {
            return None;
        } else if age < DEFLECT_MILLIS {
            age as f32 / DEFLECT_MILLIS as f32
        } else {
            1.0 - (age - DEFLECT_MILLIS) as f32 / RETURN_MILLIS as f32
        };
        (ratio > 0.0).then(|| [ratio * event.kick_pitch, ratio * event.kick_roll])
    }
}

/// Whether a hit taken with the armour going from `previous` to `now` hit the shield:
/// the armour fell (`G_Damage` takes from armour first, `CheckArmor`).
fn shield_hit(previous: Option<i32>, now: i32) -> bool {
    previous.is_some_and(|previous| now < previous)
}

fn calculate(
    yaw_byte: u8,
    pitch_byte: u8,
    damage: u8,
    health: i32,
    view_angles: [f32; 3],
    server_time: i32,
) -> Event {
    let scale = if health < 40 {
        1.0
    } else {
        40.0 / health as f32
    };
    let kick = (f32::from(damage) * scale).clamp(5.0, 10.0);
    let (x, y, kick_pitch, kick_roll) = if yaw_byte == 255 && pitch_byte == 255 {
        (0.0, 0.0, -kick, 0.0)
    } else {
        let pitch = f32::from(pitch_byte) / 255.0 * 360.0;
        let yaw = f32::from(yaw_byte) / 255.0 * 360.0;
        let damage_forward = angle_forward(pitch, yaw);
        let direction = damage_forward.map(|value| -value);
        let [view_forward, view_left, view_up] = view_axis(view_angles);
        let mut front = dot(direction, view_forward);
        let left = dot(direction, view_left);
        let up = dot(direction, view_up);
        let distance = (front * front + left * left).sqrt().max(0.1);
        let kick_roll = kick * left;
        let kick_pitch = -kick * front;
        if front <= 0.1 {
            front = 0.1;
        }
        (
            (-left / front).clamp(-1.0, 1.0),
            (up / distance).clamp(-1.0, 1.0),
            kick_pitch,
            kick_roll,
        )
    };
    Event {
        x,
        y,
        kick,
        kick_pitch,
        kick_roll,
        server_time,
        shield: false,
    }
}

fn angle_forward(pitch_degrees: f32, yaw_degrees: f32) -> [f32; 3] {
    let (pitch_sine, pitch_cosine) = pitch_degrees.to_radians().sin_cos();
    let (yaw_sine, yaw_cosine) = yaw_degrees.to_radians().sin_cos();
    [
        pitch_cosine * yaw_cosine,
        pitch_cosine * yaw_sine,
        -pitch_sine,
    ]
}

fn view_axis(angles: [f32; 3]) -> [[f32; 3]; 3] {
    let (pitch_sine, pitch_cosine) = angles[0].to_radians().sin_cos();
    let (yaw_sine, yaw_cosine) = angles[1].to_radians().sin_cos();
    let (roll_sine, roll_cosine) = angles[2].to_radians().sin_cos();
    let forward = [
        pitch_cosine * yaw_cosine,
        pitch_cosine * yaw_sine,
        -pitch_sine,
    ];
    let right = [
        -roll_sine * pitch_sine * yaw_cosine + roll_cosine * yaw_sine,
        -roll_sine * pitch_sine * yaw_sine - roll_cosine * yaw_cosine,
        -roll_sine * pitch_cosine,
    ];
    let up = [
        roll_cosine * pitch_sine * yaw_cosine + roll_sine * yaw_sine,
        roll_cosine * pitch_sine * yaw_sine - roll_sine * yaw_cosine,
        roll_cosine * pitch_cosine,
    ];
    [forward, right.map(|value| -value), up]
}

fn dot(left: [f32; 3], right: [f32; 3]) -> f32 {
    left.into_iter().zip(right).map(|(a, b)| a * b).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_defaults_off_can_toggle_live_and_persists_without_changing_kick() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.cfg");
        let mut console = crate::console::ViewerConsole::new(path.clone()).unwrap();
        let feedback = Feedback {
            active: Some(calculate(255, 255, 20, 100, [0.0; 3], 1000)),
            ..Default::default()
        };
        let kick = feedback.view_kick(1050);
        assert!(kick.is_some());
        assert_eq!(console.bool_cvar("cg_hitMarker"), Some(false));
        assert_eq!(feedback.marker(1050, Some(&console)), None);
        assert_eq!(feedback.marker(1050, None), None);
        assert!(console.set_cvar("cg_hitMarker", "1"));
        assert_eq!(feedback.marker(1050, Some(&console)), feedback.sample(1050));
        assert_eq!(feedback.view_kick(1050), kick);
        assert_eq!(feedback.marker(1500, Some(&console)), None);
        drop(console);
        let mut console = crate::console::ViewerConsole::new(path).unwrap();
        assert_eq!(console.bool_cvar("cg_hitMarker"), Some(true));
        assert!(console.set_cvar("cg_hitMarker", "0"));
        assert_eq!(feedback.marker(1050, Some(&console)), None);
        assert_eq!(feedback.view_kick(1050), kick);
    }

    #[test]
    fn a_hit_the_shield_took_shows_green_when_asked() {
        assert!(shield_hit(Some(50), 40));
        assert!(!shield_hit(Some(0), 0), "no armour left to lose");
        assert!(!shield_hit(Some(40), 60), "picked up a shield");
        assert!(!shield_hit(None, 10), "the first snapshot");
        let directory = tempfile::tempdir().unwrap();
        let mut console =
            crate::console::ViewerConsole::new(directory.path().join("config.cfg")).unwrap();
        assert!(console.set_cvar("cg_hitMarker", "1"));
        assert_eq!(
            console.bool_cvar(SHIELD_CVAR),
            Some(false),
            "off by default"
        );
        let feedback = Feedback {
            active: Some(Event {
                shield: true,
                ..calculate(255, 255, 20, 100, [0.0; 3], 1000)
            }),
            ..Default::default()
        };
        let sample = feedback.marker(1050, Some(&console)).unwrap();
        assert!(!sample.shield, "stock red while the option is off");
        assert_eq!(sample.color(), [0.0; 4]);
        assert!(console.set_cvar(SHIELD_CVAR, "1"));
        let sample = feedback.marker(1050, Some(&console)).unwrap();
        assert!(sample.shield);
        assert_eq!(sample.color()[3], 1.0);
        let red = Feedback {
            active: Some(calculate(255, 255, 20, 100, [0.0; 3], 1000)),
            ..Default::default()
        };
        assert!(!red.marker(1050, Some(&console)).unwrap().shield);
    }

    #[test]
    #[ignore = "needs a GPU and external JKA_GAME_DATA"]
    fn damage_marker_gpu_comparison() {
        use bytemuck::Zeroable;
        crate::world_shot::on_big_stack(|| {
            let (mut gpu, _profile) = crate::world_shot::open(
                "maps/mp/duel6.bsp",
                [640, 480],
                None,
                &[("r_hdr", "0"), ("cg_materialMaps", "0")],
            )
            .expect("a GPU adapter");
            assert!(matches!(
                gpu.render(&mut None),
                crate::gpu_context::FrameStatus::Rendered
            ));
            let feedback = Feedback {
                active: Some(calculate(255, 255, 20, 100, [0.0; 3], 1000)),
                ..Default::default()
            };
            let mut shots = Vec::new();
            for enabled in ["0", "1"] {
                let console = gpu.console.as_mut().unwrap();
                assert!(console.set_cvar("cg_hitMarker", enabled));
                let sample = feedback.marker(1050, Some(console));
                let uniform = crate::hud_runtime::HudUniform {
                    hud_visible: 1.0,
                    inverse_width: 1.0 / 640.0,
                    inverse_height: 1.0 / 480.0,
                    damage_alpha: sample.map_or(0.0, |s| s.alpha),
                    damage_strength: sample.map_or(0.0, |s| s.strength),
                    ..crate::hud_runtime::HudUniform::zeroed()
                };
                gpu.queue
                    .write_buffer(&gpu.hud_buffer, 0, bytemuck::bytes_of(&uniform));
                let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("damage marker comparison"),
                    size: wgpu::Extent3d {
                        width: 640,
                        height: 480,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: crate::ui_target::format(gpu.context.format),
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
                let view = texture.create_view(&Default::default());
                let mut encoder = gpu.device.create_command_encoder(&Default::default());
                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &gpu.depth.view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        ..Default::default()
                    });
                    pass.set_pipeline(&gpu.hud_pipeline);
                    pass.set_bind_group(0, &gpu.hud_bind_group, &[]);
                    pass.draw(0..3, 0..1);
                }
                gpu.queue.submit([encoder.finish()]);
                shots.push(crate::world_shot::read_back(
                    &gpu.device,
                    &gpu.queue,
                    &texture,
                ));
            }
            let red = |image: &image::RgbaImage| {
                image
                    .pixels()
                    .filter(|p| p[0] > 64 && p[0] > p[1].saturating_add(32))
                    .count()
            };
            assert_eq!(red(&shots[0]), 0);
            assert!(red(&shots[1]) > 50);
            println!(
                "damage marker red pixels: off={}, on={}",
                red(&shots[0]),
                red(&shots[1])
            );
            if let Some(path) = std::env::var_os("SJK_DAMAGE_MARKER_SHOT") {
                shots[1].save(path).unwrap();
            }
        });
    }
}
