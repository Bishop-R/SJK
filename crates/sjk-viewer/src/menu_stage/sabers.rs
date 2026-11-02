//! Sabers in the stage model's hands. Each hilt is its own detached mesh
//! (the shared object buffers only exist inside a session) parked on the
//! hand bolt of the pose the stage was skinned with, and its blades go
//! through the same glow/core renderer as in-game sabers, so the menu shows
//! exactly the hilt, sockets and colours the player will carry. On
//! lightsaber creation's preview the hilts leave the hands for the
//! [`showcase`](super::showcase).

use super::hilt_guard;
use super::lone;
use super::showcase::{self, Line, View};
use super::throw::{Pose, Throw};
use super::*;
use crate::menu_backdrop::Focus;
use crate::player_menu::StageSabers;
use crate::saber::{self, Attachment, HAND_BOLT, LEFT_HAND_BOLT};
use crate::saber_hilts::{HiltBlade, hilt_blades, hilt_sockets};
use crate::saber_rgb::BladeColor;

/// The last request the hands were synced to, so a request that has not
/// changed costs a comparison and nothing else — including one whose hilt
/// failed to load (it is retried only when the request changes).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Request {
    hilts: [Option<String>; 2],
    colors: [[u8; 3]; 2],
    black: [bool; 2],
}

impl Request {
    pub(super) fn matches(&self, wanted: &StageSabers<'_>) -> bool {
        self.colors == wanted.colors
            && self.black == wanted.black
            && self
                .hilts
                .iter()
                .zip(wanted.hilts)
                .all(|(current, wanted)| current.as_deref() == wanted)
    }

    pub(super) fn from(wanted: &StageSabers<'_>) -> Self {
        Self {
            hilts: wanted.hilts.map(|name| name.map(str::to_owned)),
            colors: wanted.colors,
            black: wanted.black,
        }
    }
}

/// One loaded hilt in a hand.
pub(super) struct StageSaber {
    color: BladeColor,
    blades: [Option<HiltBlade>; 8],
    num_blades: u8,
    /// Where the hilt lies on the showcase; `None` without a blade.
    line: Option<Line>,
    draws: Vec<DetachedDraw>,
    materials: DetachedMaterials,
    vertex_buffer: wgpu::Buffer,
    geometry_binding: wgpu::BindGroup,
    index_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
}

impl StageSaber {
    /// Draw the hilt's opaque or blended surfaces.
    pub(super) fn draw<'pass>(
        &'pass self,
        pass: &mut wgpu::RenderPass<'pass>,
        world_materials: &'pass world_materials::Runtime,
        camera: &'pass wgpu::BindGroup,
        blended: bool,
    ) {
        world_materials.draw_detached(
            pass,
            camera,
            &self.geometry_binding,
            &self.vertex_buffer,
            &self.index_buffer,
            &self.instance_buffer,
            &self.materials,
            &self.draws,
            blended,
        );
    }
}

impl GpuState {
    /// Reload the hands for a changed request. Sockets and blade parameters
    /// come from the `.sab` definitions; a hilt that fails to load is logged and
    /// the hand holds the default hilt instead (empty only when that fails too).
    pub(super) fn sync_stage_sabers(&mut self, request: Request) {
        for hand in 0..2 {
            let color = if request.black[hand] {
                BladeColor::Black
            } else {
                BladeColor::from_rgb(request.colors[hand])
            };
            self.menu_stage.sabers[hand] = request.hilts[hand]
                .as_deref()
                .and_then(|name| self.stage_saber_or_default(name, color));
        }
        self.menu_stage.request = request;
        self.menu_stage.sabers_dirty = true;
    }

    /// `name`'s hilt, or [`hilt_guard::DEFAULT_HILT`]'s when it cannot be shown.
    fn stage_saber_or_default(&mut self, name: &str, color: BladeColor) -> Option<StageSaber> {
        let error = match self.guarded_stage_saber(name, color) {
            Ok(saber) => return Some(saber),
            Err(error) => error,
        };
        if name.eq_ignore_ascii_case(hilt_guard::DEFAULT_HILT) {
            crate::log::progress(format_args!(
                "player stage could not load saber {name}: {error}"
            ));
            return None;
        }
        crate::log::progress(format_args!(
            "player stage could not load saber {name}: {error}; showing {} instead",
            hilt_guard::DEFAULT_HILT
        ));
        self.guarded_stage_saber(hilt_guard::DEFAULT_HILT, color)
            .inspect_err(|error| {
                crate::log::progress(format_args!(
                    "player stage could not load saber {}: {error}",
                    hilt_guard::DEFAULT_HILT
                ));
            })
            .ok()
    }

    /// [`Self::build_stage_saber`] with wgpu's errors captured (an uncaptured one
    /// ends the game), and not tried again for a hilt the game closed while loading
    /// the last time ([`hilt_guard`]): a hilt saved in the profile must not take
    /// every later start down with it (#214).
    fn guarded_stage_saber(
        &mut self,
        name: &str,
        color: BladeColor,
    ) -> Result<StageSaber, Box<dyn Error>> {
        let directory = crate::crash_report::directory();
        if hilt_guard::crashed_on(directory.as_deref(), name) {
            return Err("the game closed while loading it last time".into());
        }
        hilt_guard::begin(directory.as_deref(), name);
        let kept = self.world_materials.texture_keys();
        let device = self.device.clone();
        let built = crate::gpu_guard::capture(&device, || self.build_stage_saber(name, color))
            .unwrap_or_else(|error| {
                self.world_materials.forget_textures_since(&kept);
                Err(format!("the graphics card: {error}").into())
            });
        hilt_guard::end(directory.as_deref());
        built
    }

    fn build_stage_saber(
        &mut self,
        name: &str,
        color: BladeColor,
    ) -> Result<StageSaber, Box<dyn Error>> {
        let vfs = self.vfs.clone().ok_or("player stage has no VFS")?;
        if self.menu_stage.saber_definitions.is_none() {
            self.menu_stage.saber_definitions = Some(crate::saber_defs::load(&vfs)?);
        }
        let definition = self
            .menu_stage
            .saber_definitions
            .as_ref()
            .and_then(|definitions| definitions.get(&name.to_ascii_lowercase()))
            .ok_or_else(|| format!("no saber definition named {name}"))?
            .clone();
        let asset = vfs
            .read(&definition.model)?
            .ok_or_else(|| format!("hilt model {} is missing", definition.model))?;
        let model = Glm::parse(&asset.bytes)?;
        let sockets = hilt_sockets(&model)?;
        let mut flattened = FlattenedScene::default();
        let mut draws = append_static_glm_mesh(&mut flattened, &model)?;
        let blades = hilt_blades(&definition, &sockets);
        let line = showcase_line(&blades, definition.num_blades, &flattened.vertices);
        pad_empty_mesh(&mut flattened, &mut draws);
        let materials = self.world_materials.compile_detached(
            &self.device,
            &self.queue,
            &vfs,
            &self.shaders,
            &flattened.materials,
        )?;
        let draws = draws
            .into_iter()
            .map(|draw| DetachedDraw {
                indices: draw.indices,
                material: draw.material,
            })
            .collect();
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SJK player stage hilt vertices"),
                contents: bytemuck::cast_slice(&flattened.vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
            });
        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SJK player stage hilt indices"),
                contents: bytemuck::cast_slice(&flattened.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        let instance = ActorInstance::new([0.0; 3], Quat::IDENTITY.to_array(), [1.0; 3]);
        let quads = crate::shared_geometry::quads::upload(
            &self.device,
            flattened.vertices.len(),
            &flattened.indices,
            0,
        );
        let geometry_binding =
            crate::shared_geometry::quads::bind(&self.device, &vertex_buffer, &quads);
        let instance_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SJK player stage hilt instance"),
                contents: bytemuck::bytes_of(&instance),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
        Ok(StageSaber {
            color,
            blades,
            num_blades: definition.num_blades,
            line,
            draws,
            materials,
            vertex_buffer,
            geometry_binding,
            index_buffer,
            instance_buffer,
        })
    }

    /// Step every hand's throw for this frame and remember where its hilt
    /// is while out of the hand. Two hilts float side by side across the
    /// focus, one alone on it. Without a hand, a hilt or a focus point a
    /// saber simply stays held.
    /// The Force page's Saber Throw: the first saber flies out of the hand and back
    /// on a loop, `seconds` into it ([`super::force_motion::throw_at`]). Run after
    /// [`Self::advance_stage_throw`], which leaves a preview's sabers in hand.
    pub(super) fn advance_preview_throw(&mut self, seconds: f32) {
        let stage = &mut self.menu_stage;
        let Some(actor) = stage.actor.as_ref() else {
            return;
        };
        let Some(bolt) = actor.hands[0] else {
            return;
        };
        let Some(blade) = stage.sabers[0]
            .as_ref()
            .and_then(|saber| saber.blades[0].as_ref())
            .map(|blade| Vec3::from_array(blade.socket.direction))
        else {
            return;
        };
        let (grip, rotation) = saber::world_attachment(actor.origin, actor.rotation, bolt);
        // Out to the model's side and towards the camera in front of it, a little up,
        // so the whole flight stays in the picture.
        let facing = actor.rotation * Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2) * Vec3::X;
        let side = Vec3::Z.cross(facing).normalize_or(Vec3::Y);
        let direction = (facing * 0.6 - side * 0.75 + Vec3::Z * 0.15).normalize_or(facing);
        let flying =
            super::force_motion::throw_at(Pose { grip, rotation }, blade, direction, seconds);
        if flying.is_some() || stage.flying[0].is_some() {
            stage.sabers_dirty = true;
        }
        stage.flying[0] = flying;
    }

    pub(super) fn advance_stage_throw(&mut self, thrown: bool, focus: Option<Focus>, now: Instant) {
        let stage = &mut self.menu_stage;
        let held = stage.sabers.iter().flatten().count();
        for hand in 0..2 {
            let attachment = stage.actor.as_ref().and_then(|actor| {
                let (grip, rotation) =
                    saber::world_attachment(actor.origin, actor.rotation, actor.hands[hand]?);
                Some(Pose { grip, rotation })
            });
            let blade = stage.sabers[hand]
                .as_ref()
                .and_then(|saber| saber.blades[0].as_ref())
                .map(|blade| {
                    let direction = Vec3::from_array(blade.socket.direction).normalize_or(Vec3::Y);
                    (
                        direction,
                        Vec3::from_array(blade.socket.origin).dot(direction),
                    )
                });
            let flying = match blade {
                Some((blade, root)) => {
                    let spot = focus.map(|focus| {
                        focus.point
                            + focus.right * float_offset(hand, held)
                            + Vec3::Z * (FLOAT_ROOT - root)
                    });
                    stage.throw[hand].advance(thrown, attachment, spot, blade, now)
                }
                None => {
                    stage.throw[hand] = Throw::Held;
                    None
                }
            };
            if flying.is_some() || stage.flying[hand].is_some() {
                stage.sabers_dirty = true;
            }
            stage.flying[hand] = flying;
        }
    }

    /// Start this frame's saber instance list: cleared, then seeded with the
    /// stage model's blades (session sabers are appended by entity
    /// submission afterwards). Hilt instances are rewritten only when the
    /// pose or the hilts changed.
    pub(crate) fn begin_saber_instances(&mut self) {
        self.saber_instances.clear();
        // The stage model is the local player: its blades wear the player's skin, or
        // the one the Collection's Shaders tab shows.
        // The shader the Collection shows goes on the side being dressed; the other side
        // shows what it wears now, so the pair is seen as Equip will leave it.
        // With the target switch on the second, the shader goes there and the first keeps
        // what it wears.
        let shown = match self.menu_stage.skin_override {
            Some(crate::console::collection_panel::PreviewSkin::Stock) => Some(None),
            Some(crate::console::collection_panel::PreviewSkin::Skin(id)) => Some(
                self.blade_skins
                    .color_with(id, self.looks.own_options_off(id))
                    .map(|skin| crate::saber_skins::SkinColor {
                        persona: self.saber_skins.local_persona(),
                        ..skin
                    }),
            ),
            _ => None,
        };
        let skins = crate::saber_skins::previewed(
            self.saber_skins.local_both(),
            shown,
            self.menu_stage.override_dressed,
        );
        let frame_view = self.frame_view;
        let stage = &mut self.menu_stage;
        stage.preview.blades.clear();
        // What the local player holds, for the Collection's second-saber button.
        crate::saber_skins::set_local_pair(stage.sabers.each_ref().map(|saber| {
            saber
                .as_ref()
                .map(|saber| crate::saber_hilts::has_other_half(&saber.blades, saber.num_blades))
        }));
        // Found again below while a lone staff is drawn whole.
        crate::saber_skins::set_staff_halves(None);
        let two_sabers = stage.sabers[1].is_some();
        // The classic preview's blades are drawn into it, not into the world.
        let preview = stage.preview_only;
        let Some(actor) = &stage.actor else {
            stage.preview.showcase = None;
            return;
        };
        // Lightsaber creation lays the sabers out on their own, turning, in
        // front of the preview camera (the model faces its yaw, as there).
        let showcase = (preview && (stage.showcase || stage.lone.is_some())).then(|| {
            let facing =
                actor.rotation * Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2) * Vec3::X;
            View::new(
                actor.origin + Vec3::Z * showcase::FOCUS_ABOVE_ORIGIN,
                facing,
                stage
                    .sabers
                    .each_ref()
                    .map(|saber| saber.as_ref().and_then(|saber| saber.line)),
            )
        });
        stage.preview.showcase = showcase;
        let shown = stage
            .sabers
            .iter()
            .flatten()
            .filter(|saber| saber.line.is_some())
            .count();
        // The Collection's Shaders tab lays them before the backdrop camera instead, as
        // long as the longest of them asks.
        let lone = stage
            .lone
            .filter(|_| !preview)
            .zip(frame_view)
            .map(|(turn, view)| {
                let longest = stage
                    .sabers
                    .iter()
                    .flatten()
                    .filter_map(|saber| saber.line)
                    .map(|line| line.span.length())
                    .fold(0.0, f32::max);
                (turn, lone::Place::new(&view, longest))
            });
        let seconds = crate::menu::art::motion::seconds();
        // On the live preview the Shaders tab's turn takes the place of retail's roll.
        let (roll, yaw) = match stage.lone {
            Some(turn) => (turn.roll.to_degrees(), turn.yaw),
            None => (showcase::roll_degrees(seconds as f32), 0.0),
        };
        // A jumping preview's blades drawn in (none at all once they are off).
        let out = 1.0 - stage.blades_in.clamp(0.0, 1.0);
        // Where a lone staff's two halves lie on screen, for a click on it to choose one.
        let mut halves = [None; 2];
        for (hand, saber) in stage.sabers.iter().enumerate() {
            let Some(saber) = saber else {
                continue;
            };
            let pose = match (&showcase, &lone, actor.hands[hand]) {
                (Some(view), _, _) => saber
                    .line
                    .map(|line| view.pose(&line, View::offset(hand, shown), roll, yaw)),
                (None, Some((turn, place)), _) => saber
                    .line
                    .map(|line| place.pose(&line, View::offset(hand, shown), *turn)),
                (None, None, Some(attachment)) => Some(match stage.flying[hand] {
                    Some(pose) => (pose.grip, pose.rotation),
                    None => saber::world_attachment(actor.origin, actor.rotation, attachment),
                }),
                (None, None, None) => None,
            };
            let Some((grip, rotation)) = pose else {
                continue;
            };
            // The showcase and the lone saber turn every frame.
            if stage.sabers_dirty || showcase.is_some() || lone.is_some() {
                // In the hand (and on the showcase) the hilt shares the
                // model's light sample; thrown, the grid is read where the
                // hilt actually is.
                // A preview's are lit as its actor is, the same on every map.
                let light = match stage.flying[hand]
                    .filter(|_| showcase.is_none() && !actor.preview_lit)
                {
                    Some(pose) => self
                        .entity_lighting
                        .sample(&self.bsp, pose.grip.to_array(), &[]),
                    None => actor.light,
                };
                let mut instance =
                    ActorInstance::new(grip.to_array(), rotation.to_array(), [1.0; 3]);
                instance.set_light(light);
                self.queue
                    .write_buffer(&saber.instance_buffer, 0, bytemuck::bytes_of(&instance));
            }
            for (index, blade) in saber
                .blades
                .iter()
                .take(usize::from(saber.num_blades))
                .enumerate()
                .filter_map(|(index, blade)| Some((index, blade.as_ref()?)))
                .filter(|_| out > 0.0)
            {
                let blade = saber::world_blade(
                    grip,
                    rotation,
                    blade.socket,
                    blade.length * out,
                    blade.radius,
                );
                let other_half =
                    crate::saber_hilts::other_half(&saber.blades, saber.num_blades, index);
                if lone.is_some() && !two_sabers && hand == 0 {
                    let middle = Vec3::from_array(blade.base)
                        + Vec3::from_array(blade.direction) * (blade.length * 0.5);
                    let half = &mut halves[usize::from(other_half)];
                    if half.is_none() {
                        *half = frame_view.and_then(|view| view.project_within(middle, 4.0));
                    }
                }
                let skin = crate::saber_skins::skin_of_blade(skins, two_sabers, hand, other_half);
                let color = skin.map_or(saber.color, |skin| {
                    BladeColor::Skin(skin.worn_with(saber.color))
                });
                let seed = (hand * 8 + index) as u32;
                let pair = saber::Instance::pair(blade, color)
                    .map(|i| i.with_animation(seconds, seed))
                    .into_iter()
                    .chain(saber::skin_extras(blade, color, seconds, seed));
                if preview {
                    stage.preview.blades.extend(pair);
                } else {
                    self.saber_instances.extend(pair);
                }
            }
        }
        crate::saber_skins::set_staff_halves(match halves {
            [Some(first), Some(other)] => Some([first, other]),
            _ => None,
        });
        stage.sabers_dirty = false;
    }
}

/// Give a hilt with nothing to draw one unused vertex and triangle, and no draws.
/// A community "empty hilt" (a model of only its `*blade1` tag, so the player holds
/// a blade with no hilt in a stance) flattens to no vertices; a zero-sized buffer bound
/// for storage fails wgpu's validation, and the uncaptured error closed the game, at
/// every start once the hilt was saved (#214). Its blades stay.
fn pad_empty_mesh(flattened: &mut FlattenedScene, draws: &mut Vec<ActorDraw>) {
    if !flattened.vertices.is_empty() && !flattened.indices.is_empty() {
        return;
    }
    draws.clear();
    if flattened.vertices.is_empty() {
        flattened
            .vertices
            .push(<GpuVertex as bytemuck::Zeroable>::zeroed());
    }
    flattened.indices = vec![0; 3];
}

/// Sideways distance between two floating hilts.
const FLOAT_SPACING: f32 = 17.0;
/// Height of a floating hilt's blade root above the focus point. Hilts
/// keep their model origin at different distances below the blade
/// (`single_1` 4.5 units, `single_9` 6.0, the staffs 11.6), so the blade,
/// not the origin, is what is placed on the focus: cycling hilts swaps the
/// hilt under a blade that stays put.
const FLOAT_ROOT: f32 = 4.5;

/// Where along the focus's right axis `hand`'s hilt floats when `held`
/// hands carry one: centred alone, the right hand's hilt to the viewer's
/// left of a pair.
fn float_offset(hand: usize, held: usize) -> f32 {
    if held < 2 {
        0.0
    } else {
        (hand as f32 - 0.5) * FLOAT_SPACING
    }
}

/// Where a hilt lies on the showcase: along its first used blade, its span
/// taken over the mesh and every used blade's root and tip.
fn showcase_line(blades: &[Option<HiltBlade>; 8], used: u8, mesh: &[GpuVertex]) -> Option<Line> {
    let used = blades.iter().take(usize::from(used)).flatten();
    let first = used.clone().next()?;
    let ends = used.flat_map(|blade| {
        let root = Vec3::from_array(blade.socket.origin);
        let direction = Vec3::from_array(blade.socket.direction).normalize_or_zero();
        [root, root + direction * blade.length]
    });
    let points = mesh
        .iter()
        .map(|vertex| Vec3::from_array(vertex.position))
        .chain(ends);
    Line::new(
        Vec3::from_array(first.socket.origin),
        Vec3::from_array(first.socket.direction),
        points,
    )
}

/// Both hand bolts of the whole-body frame the stage was skinned with.
pub(super) fn hand_attachments(preview: &PlayerPreview, frame: usize) -> [Option<Attachment>; 2] {
    let Ok(matrices) = preview.animation.frame_matrices(frame) else {
        return [None; 2];
    };
    hand_attachments_at(preview, &matrices)
}

/// Both hand bolts of the pose `matrices` the stage was skinned with.
pub(super) fn hand_attachments_at(
    preview: &PlayerPreview,
    matrices: &[[[f32; 4]; 3]],
) -> [Option<Attachment>; 2] {
    [HAND_BOLT, LEFT_HAND_BOLT].map(|bolt| saber::attachment_from_matrices(preview, matrices, bolt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_hilt_uploads_no_empty_buffer_and_draws_nothing() {
        let mut flattened = FlattenedScene::default();
        let mut draws = Vec::new();
        pad_empty_mesh(&mut flattened, &mut draws);
        assert_eq!(flattened.vertices.len(), 1);
        assert_eq!(flattened.indices, [0; 3]);
        assert!(draws.is_empty());

        // A surface with vertices but no triangles draws nothing either.
        let mut flattened = FlattenedScene::default();
        flattened
            .vertices
            .push(<GpuVertex as bytemuck::Zeroable>::zeroed());
        let mut draws = vec![ActorDraw {
            indices: 0..0,
            material: 0,
        }];
        pad_empty_mesh(&mut flattened, &mut draws);
        assert_eq!(flattened.vertices.len(), 1);
        assert_eq!(flattened.indices, [0; 3]);
        assert!(draws.is_empty());
    }

    #[test]
    fn a_hilt_with_a_mesh_is_left_as_it_is() {
        let mut flattened = FlattenedScene::default();
        flattened
            .vertices
            .extend([<GpuVertex as bytemuck::Zeroable>::zeroed(); 3]);
        flattened.indices = vec![0, 1, 2];
        let mut draws = vec![ActorDraw {
            indices: 0..3,
            material: 0,
        }];
        pad_empty_mesh(&mut flattened, &mut draws);
        assert_eq!(flattened.vertices.len(), 3);
        assert_eq!(draws.len(), 1);
    }

    /// The stage's hilt buffers and geometry binding, as `build_stage_saber` makes them.
    fn bind_hilt(device: &wgpu::Device, flattened: &FlattenedScene) -> wgpu::BindGroup {
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("hilt test vertices"),
            contents: bytemuck::cast_slice(&flattened.vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
        });
        let quads = crate::shared_geometry::quads::upload(
            device,
            flattened.vertices.len(),
            &flattened.indices,
            0,
        );
        crate::shared_geometry::quads::bind(device, &vertices, &quads)
    }

    #[test]
    #[ignore = "requires an available GPU; uses no installed game assets"]
    fn an_empty_hilt_binds_without_a_wgpu_error_once_padded() {
        let gpu = pollster::block_on(crate::gpu_context::Context::new(None, None)).unwrap();
        let device = &gpu.device;
        let empty = FlattenedScene::default();
        assert!(
            crate::gpu_guard::capture(device, || bind_hilt(device, &empty)).is_err(),
            "an empty hilt's zero-sized storage binding is what closed the game (#214)"
        );
        let mut padded = FlattenedScene::default();
        pad_empty_mesh(&mut padded, &mut Vec::new());
        assert!(crate::gpu_guard::capture(device, || bind_hilt(device, &padded)).is_ok());
    }
}
