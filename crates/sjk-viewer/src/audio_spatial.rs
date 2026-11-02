//! 3D sound (`s_spatial`, off by default; Sol's feature after VisionJK's `v_spatial`,
//! Blackwolf's idea): the sounds of the level heard from the player's head, quieter and
//! duller behind a wall or a shut door, and the room round the player ringing, a hall
//! longer than a corridor.
//!
//! The render thread does the looking: a few times a second it traces from the hearing
//! point to each world sound heard lately ([`Tracker`], fed where the audio output
//! sends them) and casts [`ROOM_RAYS`] rays round the player for the room's size. Only
//! plain numbers cross to the mixer, through the usual command queue
//! ([`sjk_audio::Mixer::set_source_occlusion`], [`sjk_audio::Mixer::set_reverb`]), which
//! glides to them. Every table is fixed: a frame allocates nothing. Off, nothing is
//! traced or sent and the mixer's output is sample for sample the plain one.

use glam::Vec3;
use sjk_audio::{Attenuation, PlayRequest, ReverbParams, SourceId};
use std::time::{Duration, Instant};

/// The switch (Settings > Sound, 3D sound).
pub(crate) const CVAR: &str = "s_spatial";

/// The player's head, between crouching and standing eye height above the origin
/// (`CROUCH_VIEWHEIGHT`, `DEFAULT_VIEWHEIGHT` of `bg_public.h`).
const EYE_LOW: f32 = 12.0;
const EYE_HIGH: f32 = 36.0;
/// A camera further than this from the head is not following the player (a
/// spectator, the drone, the free camera): it is where they hear from.
const EYE_FAR: f32 = 160.0;

/// World sounds remembered for occlusion.
const TRACKED: usize = 48;
/// A sound not heard again for this long is forgotten.
const FORGET_AFTER: Duration = Duration::from_secs(8);
/// How often occlusion and the room are looked at (about 15 times a second).
const TICK: Duration = Duration::from_millis(66);
/// Sources traced at most a tick (three rays each), round the table in turn.
const SOURCES_PER_TICK: usize = 16;
/// Sources further than this are past hearing (retail's falloff ends near 1300).
const TRACE_REACH: f32 = 2_000.0;
/// A wall hit this close before the sound is the wall the sound is on (an impact),
/// not one in the way.
const NEAR_SOURCE: f32 = 16.0;
/// How far off the straight line the two extra occlusion rays pass.
const RAY_SPREAD: f32 = 24.0;
/// A change of occlusion smaller than this is not sent.
const OCCLUSION_EPSILON: f32 = 0.02;

/// Rays cast round the hearing point for the room's size, a few each tick.
pub(crate) const ROOM_RAYS: usize = 32;
const ROOM_RAYS_PER_TICK: usize = 8;
/// A ray that meets nothing this close is open air.
const ROOM_REACH: f32 = 2_048.0;
/// The mean distance of a cupboard and of a hall.
const ROOM_SMALL: f32 = 64.0;
const ROOM_LARGE: f32 = 1_024.0;
/// Reverb loudness in the largest closed room.
const ROOM_WET: f32 = 0.6;

/// `CONTENTS_SOLID`: walls, not water, fog or players.
const CONTENTS_SOLID: u32 = 1;

/// Where the player hears from with `s_spatial` on: their head as high as the camera
/// (so in first person the eyes, in third person not the orbiting camera), else the
/// camera when it is far from them or there is no predicted player.
pub(crate) fn hearing_point(camera: Vec3, player_origin: Option<Vec3>) -> Vec3 {
    let Some(origin) = player_origin else {
        return camera;
    };
    let head = Vec3::new(
        origin.x,
        origin.y,
        camera.z.clamp(origin.z + EYE_LOW, origin.z + EYE_HIGH),
    );
    if head.distance_squared(camera) > EYE_FAR * EYE_FAR {
        camera
    } else {
        head
    }
}

/// What one ray met: how far along it stopped (1 nothing) and whether on the sky.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Hit {
    pub(crate) fraction: f32,
    pub(crate) sky: bool,
}

impl Hit {
    pub(crate) const OPEN: Self = Self {
        fraction: 1.0,
        sky: false,
    };
}

#[derive(Clone, Copy)]
struct Tracked {
    source: SourceId,
    origin: [f32; 3],
    heard: Instant,
    /// The occlusion last sent to the mixer.
    sent: f32,
}

/// The world sounds sent lately and the room round the player.
pub(crate) struct Tracker {
    pub(crate) enabled: bool,
    /// Whether the mixer was last told the effects are on.
    mixer_enabled: bool,
    listener_source: Option<SourceId>,
    sources: [Option<Tracked>; TRACKED],
    next_source: usize,
    next_tick: Option<Instant>,
    directions: [Vec3; ROOM_RAYS],
    room: [Hit; ROOM_RAYS],
    next_room_ray: usize,
    /// Results of the last tick for the caller to send.
    occlusions: [(SourceId, f32); SOURCES_PER_TICK],
    occlusion_count: usize,
    reverb: Option<ReverbParams>,
}

impl Default for Tracker {
    fn default() -> Self {
        Self {
            enabled: false,
            mixer_enabled: false,
            listener_source: None,
            sources: [None; TRACKED],
            next_source: 0,
            next_tick: None,
            directions: room_directions(),
            room: [Hit::OPEN; ROOM_RAYS],
            next_room_ray: 0,
            occlusions: [(SourceId(0), 0.0); SOURCES_PER_TICK],
            occlusion_count: 0,
            reverb: None,
        }
    }
}

/// [`ROOM_RAYS`] directions spread evenly over the sphere (a Fibonacci spiral).
fn room_directions() -> [Vec3; ROOM_RAYS] {
    let golden = std::f32::consts::PI * (3.0 - 5.0_f32.sqrt());
    std::array::from_fn(|index| {
        let z = 1.0 - (index as f32 + 0.5) * 2.0 / ROOM_RAYS as f32;
        let radius = (1.0 - z * z).max(0.0).sqrt();
        let angle = golden * index as f32;
        Vec3::new(radius * angle.cos(), radius * angle.sin(), z)
    })
}

impl Tracker {
    /// Note a command the audio output sends: world sounds and where they move.
    pub(crate) fn observe(&mut self, command: &crate::audio_output::AudioCommand, now: Instant) {
        use crate::audio_output::AudioCommand;
        if !self.enabled {
            return;
        }
        match command {
            AudioCommand::Play(_, request)
            | AudioCommand::PlayFrom(_, request, ..)
            | AudioCommand::PlayTrack(_, request, ..)
            | AudioCommand::SetLoop(_, request, _) => self.hear(request, now),
            AudioCommand::SourcePosition(source, origin) => {
                if let Some(tracked) = self
                    .sources
                    .iter_mut()
                    .flatten()
                    .find(|t| t.source == *source)
                {
                    tracked.origin = *origin;
                }
            }
            AudioCommand::ListenerSource(source) => self.listener_source = *source,
            AudioCommand::StopEffects | AudioCommand::StopAll => self.clear(),
            _ => {}
        }
    }

    /// Remember a world sound's source and place (not the player's own).
    pub(crate) fn hear(&mut self, request: &PlayRequest, now: Instant) {
        let Some(origin) = request.origin else {
            return;
        };
        if request.attenuation == Attenuation::None || Some(request.source) == self.listener_source
        {
            return;
        }
        if let Some(tracked) = self
            .sources
            .iter_mut()
            .flatten()
            .find(|tracked| tracked.source == request.source)
        {
            tracked.origin = origin;
            tracked.heard = now;
            return;
        }
        // A free slot, else the one heard longest ago.
        let slot = self
            .sources
            .iter()
            .position(Option::is_none)
            .or_else(|| {
                self.sources
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, tracked)| tracked.map(|t| t.heard))
                    .map(|(index, _)| index)
            })
            .unwrap_or(0);
        self.sources[slot] = Some(Tracked {
            source: request.source,
            origin,
            heard: now,
            sent: 0.0,
        });
    }

    /// Forget every source and the room (a new map, `soundstop`, switched off).
    pub(crate) fn clear(&mut self) {
        self.sources = [None; TRACKED];
        self.room = [Hit::OPEN; ROOM_RAYS];
        self.next_room_ray = 0;
        self.occlusion_count = 0;
        self.reverb = None;
    }

    /// Whether the mixer must be told the switch changed, and remember it was.
    pub(crate) fn switch_changed(&mut self) -> Option<bool> {
        (self.mixer_enabled != self.enabled).then(|| {
            self.mixer_enabled = self.enabled;
            if !self.enabled {
                self.clear();
            }
            self.enabled
        })
    }

    /// Whether a tick is due at `now` (and start the next one's wait).
    pub(crate) fn tick_due(&mut self, now: Instant) -> bool {
        if self.next_tick.is_some_and(|next| now < next) {
            return false;
        }
        self.next_tick = Some(now + TICK);
        true
    }

    /// One tick's looking from `hearing` (its right being `right`): occlusion of a
    /// share of the sources and a share of the room rays. `trace` answers one ray.
    /// The results wait in [`Self::occlusions`] and [`Self::take_reverb`].
    pub(crate) fn tick(
        &mut self,
        now: Instant,
        hearing: Vec3,
        right: Vec3,
        mut trace: impl FnMut(Vec3, Vec3) -> Hit,
    ) {
        self.occlusion_count = 0;
        for tracked in &mut self.sources {
            if tracked.is_some_and(|t| now.saturating_duration_since(t.heard) > FORGET_AFTER) {
                *tracked = None;
            }
        }
        let side = right.normalize_or_zero() * RAY_SPREAD;
        let mut visited = 0;
        while visited < TRACKED && self.occlusion_count < SOURCES_PER_TICK {
            let index = self.next_source;
            self.next_source = (self.next_source + 1) % TRACKED;
            visited += 1;
            let Some(tracked) = &mut self.sources[index] else {
                continue;
            };
            let target = Vec3::from_array(tracked.origin);
            if target.distance_squared(hearing) > TRACE_REACH * TRACE_REACH {
                continue;
            }
            let rays = [
                (hearing, target),
                (hearing, target + Vec3::Z * RAY_SPREAD),
                (hearing + side, target),
            ];
            let blocked = rays
                .iter()
                .filter(|(from, to)| blocks(trace(*from, *to), from.distance(*to)))
                .count();
            let amount = blocked as f32 / rays.len() as f32;
            if (amount - tracked.sent).abs() >= OCCLUSION_EPSILON {
                tracked.sent = amount;
                self.occlusions[self.occlusion_count] = (tracked.source, amount);
                self.occlusion_count += 1;
            }
        }
        for _ in 0..ROOM_RAYS_PER_TICK {
            let index = self.next_room_ray;
            let end = hearing + self.directions[index] * ROOM_REACH;
            self.room[index] = trace(hearing, end);
            self.next_room_ray = (index + 1) % ROOM_RAYS;
            if self.next_room_ray == 0 {
                self.reverb = Some(room_reverb(&self.room));
            }
        }
    }

    /// The occlusions the last tick changed, to send.
    pub(crate) fn occlusions(&self) -> &[(SourceId, f32)] {
        &self.occlusions[..self.occlusion_count]
    }

    /// The room's reverb when a full round of rays has just been cast.
    pub(crate) fn take_reverb(&mut self) -> Option<ReverbParams> {
        self.reverb.take()
    }
}

/// Whether a ray of `length` that met `hit` is blocked before its end (a wall right
/// at the end is the sound's own).
fn blocks(hit: Hit, length: f32) -> bool {
    hit.fraction < 1.0 && (1.0 - hit.fraction) * length > NEAR_SOURCE
}

/// The reverb of the room the rays found: the further the walls, the larger the room
/// and the longer its tail; the more rays reach the sky or nothing, the less it rings.
pub(crate) fn room_reverb(rays: &[Hit; ROOM_RAYS]) -> ReverbParams {
    let mut closed = 0;
    let mut distance = 0.0;
    for hit in rays {
        if hit.fraction < 1.0 && !hit.sky {
            closed += 1;
            distance += hit.fraction * ROOM_REACH;
        }
    }
    if closed == 0 {
        return ReverbParams::default();
    }
    let enclosed = closed as f32 / ROOM_RAYS as f32;
    let mean = distance / closed as f32;
    let size = ((mean - ROOM_SMALL) / (ROOM_LARGE - ROOM_SMALL)).clamp(0.0, 1.0);
    ReverbParams {
        wet: ROOM_WET * enclosed * (0.35 + 0.65 * size),
        size,
    }
}

/// Whether the segment from `start` to `end` passes through the box.
pub(crate) fn segment_meets_box(start: Vec3, end: Vec3, minimums: Vec3, maximums: Vec3) -> bool {
    let delta = end - start;
    let (mut enter, mut leave) = (0.0_f32, 1.0_f32);
    for axis in 0..3 {
        if delta[axis].abs() < f32::EPSILON {
            if start[axis] < minimums[axis] || start[axis] > maximums[axis] {
                return false;
            }
            continue;
        }
        let near = (minimums[axis] - start[axis]) / delta[axis];
        let far = (maximums[axis] - start[axis]) / delta[axis];
        enter = enter.max(near.min(far));
        leave = leave.min(near.max(far));
        if enter > leave {
            return false;
        }
    }
    true
}

impl super::GameAudio {
    /// Hand this frame's room acoustics to the mixer: the switch when it changed and,
    /// when a tick is due in a map (`world` traces one ray there), the occlusions and
    /// room it found. Out of a map the room is dry.
    pub(crate) fn spatial_frame(
        &mut self,
        now: Instant,
        hearing: Vec3,
        right: Vec3,
        world: Option<&mut dyn FnMut(Vec3, Vec3) -> Hit>,
    ) {
        use crate::audio_output::AudioCommand;
        if let Some(enabled) = self.output.spatial.switch_changed() {
            self.output.send(AudioCommand::SpatialEffects(enabled));
        }
        if !self.output.spatial.enabled || !self.output.spatial.tick_due(now) {
            return;
        }
        let Some(world) = world else {
            self.output.spatial.clear();
            self.output
                .send(AudioCommand::Reverb(ReverbParams::default()));
            return;
        };
        self.output.spatial.tick(now, hearing, right, world);
        for index in 0..self.output.spatial.occlusions().len() {
            let (source, amount) = self.output.spatial.occlusions()[index];
            self.output
                .send(AudioCommand::SourceOcclusion(source, amount));
        }
        if let Some(room) = self.output.spatial.take_reverb() {
            self.output.send(AudioCommand::Reverb(room));
        }
    }
}

impl crate::GpuState {
    /// Where the listener hears from this frame: the camera, or with `s_spatial` the
    /// player's head ([`hearing_point`]).
    pub(crate) fn audio_hearing_point(&self, spatial: bool) -> Vec3 {
        if !spatial {
            return self.camera_position;
        }
        let player = self
            .local_prediction
            .predicted_state()
            .filter(|_| !self.free_camera_active())
            .map(|state| Vec3::from_array(state.origin));
        hearing_point(self.camera_position, player)
    }

    /// The room acoustics' share of the frame ([`super::GameAudio::spatial_frame`]): in a
    /// map, its rays meet the world's walls and the doors and lifts where they are shown.
    pub(crate) fn spatial_audio_frame(
        &mut self,
        audio: &mut super::GameAudio,
        hearing: Vec3,
        right: Vec3,
    ) {
        let in_map = self.live_session.is_some() || self.demo_session.is_some();
        let (bsp, scratch) = (&self.bsp, &mut self.trace_scratch);
        let solids = self.local_prediction.presented_solids();
        let mut trace = |from: Vec3, to: Vec3| {
            let world = bsp.trace_box_with(
                scratch,
                from.to_array(),
                to.to_array(),
                sjk_bsp::Aabb::POINT,
                CONTENTS_SOLID,
            );
            if world.start_solid {
                return Hit::OPEN;
            }
            let mut hit = Hit {
                fraction: world.fraction,
                sky: world.surface_flags & crate::weather::cover::SURF_SKY != 0,
            };
            for solid in solids.iter().filter(|solid| solid.bounds.is_none()) {
                let Some(model) = bsp.render().models().get(solid.model) else {
                    continue;
                };
                let origin = Vec3::from_array(solid.origin);
                let (low, high) = if solid.rotated {
                    // Turned about its origin: the sphere its corners can sweep.
                    let reach = Vec3::from_array(model.minimums)
                        .abs()
                        .max(Vec3::from_array(model.maximums).abs())
                        .length();
                    (origin - Vec3::splat(reach), origin + Vec3::splat(reach))
                } else {
                    (
                        origin + Vec3::from_array(model.minimums),
                        origin + Vec3::from_array(model.maximums),
                    )
                };
                if !segment_meets_box(from, to, low, high) {
                    continue;
                }
                let door = crate::movement_collision::solid_trace(
                    bsp,
                    solid,
                    from.to_array(),
                    to.to_array(),
                    sjk_bsp::Aabb::POINT,
                    CONTENTS_SOLID,
                );
                if !door.start_solid && door.fraction < hit.fraction {
                    hit = Hit {
                        fraction: door.fraction,
                        sky: false,
                    };
                }
            }
            hit
        };
        let world: Option<&mut dyn FnMut(Vec3, Vec3) -> Hit> =
            if in_map { Some(&mut trace) } else { None };
        audio.spatial_frame(Instant::now(), hearing, right, world);
    }
}

#[cfg(test)]
#[path = "audio_spatial_tests.rs"]
mod tests;
