//! Sound sonar (`cg_soundSonar`, Dawson's idea, 30/10/2026): for deaf and hard-of-hearing
//! players, short arcs round the crosshair point toward the world sounds the client plays
//! (steps, gunfire and swings, voices, explosions and the rest), tinted by kind and
//! fading over a second.
//!
//! It shows only what the mixer is asked to play: every positional one-shot goes
//! through [`Sonar::hear`] as the audio output sends it, so a sound the player could not
//! hear either (muted player, `cg_footsteps 0`, too far for its falloff) is not shown.
//! Listener-relative sounds (announcer, menu, music, the player's own) and per-frame
//! loops are left out. The volume settings (`s_volume`) do not hide it: a player who
//! turned the sound down is the one it is for. A small fixed ring, no allocation.
use sjk_audio::{Attenuation, ChannelId, PlayRequest, SourceId};
use std::time::{Duration, Instant};

/// The on/off setting, off by default (Settings > Sound > Sound sonar).
pub(crate) const CVAR: &str = "cg_soundSonar";
/// Sounds remembered at once; a new one replaces the oldest.
pub(crate) const CAPACITY: usize = 24;
/// How long a sound's arc shows, fading out.
pub(crate) const LIFETIME: Duration = Duration::from_millis(1_000);
/// A sound from the same source and of the same kind this soon after another refreshes
/// its arc instead of taking another (a player's footsteps are one arc).
const MERGE: Duration = Duration::from_millis(250);
/// Sounds fainter than this at the listener are not shown.
const MIN_STRENGTH: f32 = 0.05;
/// Sounds closer than this (game units, on the floor's plane) have no direction.
const NEAR: f32 = 40.0;

/// What a sound is, from the game channel it plays on (`codemp` `q_shared.h`
/// `soundChannel_t`): its arc's tint.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Kind {
    /// `CHAN_BODY`: footsteps, jumps, landings.
    Steps,
    /// `CHAN_WEAPON`: gunfire, saber swings, weapon switches.
    Combat,
    /// `CHAN_VOICE`, `CHAN_VOICE_ATTEN`: pain, taunts, voice commands.
    Voice,
    /// `CHAN_AUTO`, `CHAN_ITEM`, `CHAN_LESS_ATTEN` and SJK's toys: explosions, clashes,
    /// Force, pickups, doors.
    #[default]
    Other,
}

impl Kind {
    /// The kind of a sound on `channel`, `None` for the channels the sonar leaves out:
    /// local, ambient, announcer, menu, global voice, music and emotes, the boombox and
    /// the interface's own.
    pub(crate) fn of_channel(channel: ChannelId) -> Option<Self> {
        Some(match channel.0 {
            6 => Self::Steps,
            2 => Self::Combat,
            3 | 4 => Self::Voice,
            0 | 5 | 10 => Self::Other,
            c if c == crate::audio::TOY_CHANNEL.0 => Self::Other,
            _ => return None,
        })
    }
}

/// One sound heard.
#[derive(Clone, Copy, Debug)]
struct Heard {
    origin: [f32; 3],
    source: SourceId,
    kind: Kind,
    volume: f32,
    attenuation: Attenuation,
    at: Instant,
}

/// The sounds heard lately, newest replacing oldest.
#[derive(Debug, Default)]
pub(crate) struct Sonar {
    /// Whether sounds are remembered at all (`cg_soundSonar`).
    pub(crate) enabled: bool,
    /// The listener's own entity, whose sounds are not shown.
    pub(crate) listener_source: Option<SourceId>,
    heard: [Option<Heard>; CAPACITY],
}

/// One arc to draw: `angle` in radians clockwise from straight ahead (positive to the
/// right), `strength` 0 to 1 from the sound's loudness at the listener, `alpha` its fade.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Marker {
    pub(crate) angle: f32,
    pub(crate) strength: f32,
    pub(crate) alpha: f32,
    pub(crate) kind: Kind,
}

/// The markers of a frame: a fixed array and how many are used.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Markers {
    pub(crate) list: [Marker; CAPACITY],
    pub(crate) count: usize,
}

impl Markers {
    pub(crate) fn iter(&self) -> impl Iterator<Item = &Marker> {
        self.list[..self.count].iter()
    }
}

impl Sonar {
    /// Remember `request` started at `at`, when it is a world sound the sonar shows.
    pub(crate) fn hear(&mut self, request: &PlayRequest, at: Instant) {
        if !self.enabled || request.volume <= 0.0 || Some(request.source) == self.listener_source {
            return;
        }
        let (Some(origin), Some(kind)) = (request.origin, Kind::of_channel(request.channel)) else {
            return;
        };
        if request.attenuation == Attenuation::None {
            return;
        }
        let entry = Heard {
            origin,
            source: request.source,
            kind,
            volume: request.volume,
            attenuation: request.attenuation,
            at,
        };
        let same = self.heard.iter_mut().flatten().find(|heard| {
            heard.source == entry.source
                && heard.kind == entry.kind
                && at.saturating_duration_since(heard.at) < MERGE
        });
        if let Some(heard) = same {
            *heard = entry;
            return;
        }
        // An empty or expired slot first, else the sound heard longest ago.
        let slot = self
            .heard
            .iter()
            .position(|heard| {
                heard.is_none_or(|heard| at.saturating_duration_since(heard.at) >= LIFETIME)
            })
            .unwrap_or_else(|| {
                self.heard
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, heard)| heard.map(|heard| heard.at))
                    .map_or(0, |(index, _)| index)
            });
        self.heard[slot] = Some(entry);
    }

    /// Forget every sound: the setting turned off, or the output told to stop the
    /// effects (a new map, `install_gamestate`) or everything (`soundstop`).
    pub(crate) fn clear(&mut self) {
        self.heard = [None; CAPACITY];
    }

    /// The arcs for a listener at `origin` facing `yaw` (radians, Quake's: 0 along +X,
    /// counter-clockwise seen from above) at `now`.
    pub(crate) fn markers(&self, now: Instant, origin: [f32; 3], yaw: f32) -> Markers {
        let mut markers = Markers::default();
        for heard in self.heard.iter().flatten() {
            let age = now.saturating_duration_since(heard.at);
            if age >= LIFETIME {
                continue;
            }
            let delta = [
                heard.origin[0] - origin[0],
                heard.origin[1] - origin[1],
                heard.origin[2] - origin[2],
            ];
            let distance = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
            let strength = (heard.volume * distance_gain(heard.attenuation, distance)).min(1.0);
            let Some(angle) = bearing(delta, yaw) else {
                continue;
            };
            if strength < MIN_STRENGTH {
                continue;
            }
            let fade = 1.0 - age.as_secs_f32() / LIFETIME.as_secs_f32();
            markers.list[markers.count] = Marker {
                angle,
                strength,
                alpha: fade * fade,
                kind: heard.kind,
            };
            markers.count += 1;
        }
        markers
    }
}

/// The game's linear falloff (`sjk_audio::LegacyQuakeSpatial`): full volume to the full
/// distance, then down by `falloff_per_unit` a unit.
fn distance_gain(attenuation: Attenuation, distance: f32) -> f32 {
    match attenuation {
        Attenuation::None => 1.0,
        Attenuation::Linear {
            full_volume_distance,
            falloff_per_unit,
        } => (1.0 - (distance - full_volume_distance).max(0.0) * falloff_per_unit).max(0.0),
    }
}

/// Where `delta` (from the listener to the sound) lies for a listener facing `yaw`, on
/// the floor's plane: radians clockwise from ahead, in (-pi, pi]; `None` when the sound
/// is (nearly) straight above, below or at the listener.
pub(crate) fn bearing(delta: [f32; 3], yaw: f32) -> Option<f32> {
    let (sin, cos) = yaw.sin_cos();
    let ahead = delta[0] * cos + delta[1] * sin;
    // Quake's right axis: (sin yaw, -cos yaw).
    let right = delta[0] * sin - delta[1] * cos;
    (ahead * ahead + right * right >= NEAR * NEAR).then(|| right.atan2(ahead))
}

impl crate::GpuState {
    /// Hand the HUD this frame's sonar arcs, none without a game, audio or the setting.
    pub(crate) fn update_sound_sonar(&mut self, audio: Option<&crate::audio::GameAudio>) {
        let in_game = self.live_session.is_some() || self.demo_session.is_some();
        let markers = match audio.map(crate::audio::GameAudio::sonar) {
            Some(sonar) if in_game && sonar.enabled => sonar.markers(
                Instant::now(),
                self.camera_position.to_array(),
                self.camera_yaw,
            ),
            _ => Markers::default(),
        };
        self.hud.set_sound_sonar(markers);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    fn request(source: u32, channel: u32, origin: [f32; 3]) -> PlayRequest {
        PlayRequest {
            origin: Some(origin),
            source: SourceId(source),
            channel: ChannelId(channel),
            volume: 1.0,
            attenuation: sjk_client::legacy_sound_attenuation(channel),
        }
    }

    fn sonar() -> Sonar {
        Sonar {
            enabled: true,
            listener_source: Some(SourceId(0)),
            ..Sonar::default()
        }
    }

    #[test]
    fn bearings_are_clockwise_from_ahead_whatever_the_yaw() {
        let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
        // Facing +X: ahead, right (-Y), behind, left (+Y).
        assert!(close(bearing([100.0, 0.0, 0.0], 0.0).unwrap(), 0.0));
        assert!(close(bearing([0.0, -100.0, 0.0], 0.0).unwrap(), FRAC_PI_2));
        assert!(close(bearing([-100.0, 0.0, 0.0], 0.0).unwrap().abs(), PI));
        assert!(close(bearing([0.0, 100.0, 0.0], 0.0).unwrap(), -FRAC_PI_2));
        // Facing +Y, a sound along +X is on the right.
        assert!(close(
            bearing([100.0, 0.0, 0.0], FRAC_PI_2).unwrap(),
            FRAC_PI_2
        ));
        // Height does not count; right overhead has no direction.
        assert!(close(bearing([100.0, 0.0, 500.0], 0.0).unwrap(), 0.0));
        assert_eq!(bearing([5.0, 5.0, 300.0], 0.0), None);
    }

    #[test]
    fn only_world_sounds_of_others_are_heard() {
        let mut sonar = sonar();
        let now = Instant::now();
        sonar.hear(&request(0, 2, [200.0, 0.0, 0.0]), now); // the listener's own
        sonar.hear(&request(3, 9, [200.0, 0.0, 0.0]), now); // announcer
        sonar.hear(&request(3, 7, [200.0, 0.0, 0.0]), now); // map ambience
        let mut local = request(3, 2, [200.0, 0.0, 0.0]);
        local.origin = None;
        sonar.hear(&local, now);
        assert_eq!(sonar.markers(now, [0.0; 3], 0.0).count, 0);
        sonar.hear(&request(3, 2, [200.0, 0.0, 0.0]), now);
        let markers = sonar.markers(now, [0.0; 3], 0.0);
        assert_eq!(markers.count, 1);
        assert_eq!(markers.list[0].kind, Kind::Combat);
        assert_eq!(markers.list[0].alpha, 1.0);
    }

    #[test]
    fn nothing_is_heard_with_the_setting_off() {
        let mut sonar = Sonar::default();
        let now = Instant::now();
        sonar.hear(&request(3, 2, [200.0, 0.0, 0.0]), now);
        assert_eq!(sonar.markers(now, [0.0; 3], 0.0).count, 0);
    }

    #[test]
    fn a_source_s_quick_sounds_share_an_arc_and_the_ring_keeps_the_newest() {
        let mut sonar = sonar();
        let now = Instant::now();
        sonar.hear(&request(3, 6, [200.0, 0.0, 0.0]), now);
        sonar.hear(
            &request(3, 6, [0.0, 200.0, 0.0]),
            now + Duration::from_millis(100),
        );
        let markers = sonar.markers(now + Duration::from_millis(100), [0.0; 3], 0.0);
        assert_eq!(markers.count, 1);
        assert!(markers.list[0].angle < 0.0, "moved to the left");
        for source in 0..CAPACITY as u32 + 5 {
            sonar.hear(&request(10 + source, 0, [300.0, 0.0, 0.0]), now);
        }
        let markers = sonar.markers(now, [0.0; 3], 0.0);
        assert_eq!(markers.count, CAPACITY);
    }

    #[test]
    fn a_full_ring_drops_the_oldest_and_keeps_a_refreshed_sound() {
        let mut sonar = sonar();
        let now = Instant::now();
        // Source 3's steps, first of all, then refreshed after the ring filled.
        sonar.hear(&request(3, 6, [200.0, 0.0, 0.0]), now);
        for source in 0..CAPACITY as u32 - 1 {
            let at = now + Duration::from_millis(10 + u64::from(source));
            sonar.hear(&request(100 + source, 0, [300.0, 0.0, 0.0]), at);
        }
        let refreshed = now + Duration::from_millis(100);
        sonar.hear(&request(3, 6, [0.0, 200.0, 0.0]), refreshed);
        // Five more, within the lifetime: each replaces the oldest, never the refreshed.
        for source in 0..5 {
            let at = refreshed + Duration::from_millis(1 + u64::from(source));
            sonar.hear(&request(200 + source, 0, [300.0, 0.0, 0.0]), at);
        }
        let later = refreshed + Duration::from_millis(10);
        let markers = sonar.markers(later, [0.0; 3], 0.0);
        assert_eq!(markers.count, CAPACITY);
        assert_eq!(
            markers
                .iter()
                .filter(|marker| marker.kind == Kind::Steps)
                .count(),
            1,
            "the refreshed steps survive"
        );
        // An expired sound's slot is taken before any live one.
        sonar.clear();
        sonar.hear(&request(3, 6, [200.0, 0.0, 0.0]), now);
        for source in 0..CAPACITY as u32 - 1 {
            sonar.hear(&request(100 + source, 0, [300.0, 0.0, 0.0]), now + LIFETIME);
        }
        sonar.hear(&request(4, 2, [300.0, 0.0, 0.0]), now + LIFETIME);
        let markers = sonar.markers(now + LIFETIME, [0.0; 3], 0.0);
        assert_eq!(markers.count, CAPACITY);
        assert!(markers.iter().any(|marker| marker.kind == Kind::Combat));
    }

    #[test]
    fn arcs_fade_with_age_and_weaken_with_distance() {
        let mut sonar = sonar();
        let now = Instant::now();
        sonar.hear(&request(3, 0, [200.0, 0.0, 0.0]), now);
        sonar.hear(&request(4, 0, [800.0, 0.0, 0.0]), now);
        sonar.hear(&request(5, 0, [5_000.0, 0.0, 0.0]), now); // beyond its falloff
        let half = sonar.markers(now + LIFETIME / 2, [0.0; 3], 0.0);
        assert_eq!(half.count, 2);
        assert!(half.list[0].alpha > 0.0 && half.list[0].alpha < 1.0);
        assert_eq!(half.list[0].strength, 1.0);
        assert!(half.list[1].strength < half.list[0].strength);
        assert_eq!(sonar.markers(now + LIFETIME, [0.0; 3], 0.0).count, 0);
    }
}
