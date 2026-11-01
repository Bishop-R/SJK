//! The Toy Master's secrets (Sol's idea, 30/10/2026; `docs/toy-master.md`): toys SJK's
//! creator, the Toy Master, hides on maps through the hub, for players to find with
//! `+use`. This module knows no toy: the hub sends each one's model, sound and show with
//! the toys placed (`sjk_identity::toy_master`), so what is hidden stays a surprise.
//! It draws the toys standing on the loaded map file, counts the player's Use presses on
//! the one they face and sends them twice a second, and plays a toy's show when the hub
//! says the presses of every player there reached its number: the toy shrinks away,
//! pops back and spins to its sound. Other players' shows come through the hub's toy
//! event. The first press on a toy finds it: the hub unlocks its hidden achievement,
//! announced here. No game server knows of a toy: SJK clients alone draw it, and it has
//! no collision.

use crate::GpuState;
use crate::actor_instance::ActorInstance;
use glam::Vec3;
use sjk_identity::service::SecretDone;
use sjk_identity::toy_master::{self as wire, Placing, Show};
use sjk_protocol::UserCommand;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// `BUTTON_USE`, `+use`.
const BUTTON_USE: u16 = 32;
/// Farthest a player may stand from a toy to press it, across the floor.
const REACH: f32 = 80.0;
/// Farthest above or below the toy a player may stand to press it.
const REACH_UP: f32 = 96.0;
/// Widest angle between the player's facing and the toy, in radians (60 degrees).
const FACING: f32 = std::f32::consts::FRAC_PI_3;
/// Shortest time between two sendings of the presses; and most presses one carries (the
/// hub's `PRESSES_MAX`).
const SEND_EVERY: Duration = Duration::from_millis(500);
const PRESSES_MAX: u32 = 10;
/// Farthest a toy `toymaster_remove` takes without an id may stand.
const REMOVE_REACH: f32 = 256.0;
/// Played, with [`EFFECT`], as a toy shrinks away and as it pops back.
const VANISH_SOUND: &str = "sound/player/teleout.mp3";
const POP_SOUND: &str = "sound/player/telein.mp3";
const EFFECT: &str = "mp/spawn";
/// How long a spin takes to reach its full speed, in milliseconds; it slows to a stop
/// over its last quarter.
const SPIN_UP: f32 = 1_000.0;
/// How far the show's sound carries: full up to the first, gone at the second.
const SOUND_FULL: f32 = 300.0;
const SOUND_RANGE: f32 = 1_400.0;
/// The first of the toys' sound sources: past the game's entity numbers, the boombox's
/// (`0x5000_0000`), the Monitor's (`0x5100_0000`) and the mystery boxes'
/// (`0x5200_0000`).
const SOURCE_BASE: u32 = 0x5300_0000;
/// The channel a toy's show sound plays on (stopped when the show ends), and its other
/// sounds'.
const SHOW_CHANNEL: sjk_audio::ChannelId = sjk_audio::ChannelId(1);
const CUE_CHANNEL: sjk_audio::ChannelId = sjk_audio::ChannelId(0);

/// Toy `id`'s own sound source.
fn source(id: i64) -> sjk_audio::SourceId {
    sjk_audio::SourceId(SOURCE_BASE | (id as u32 & 0xFFFF))
}

/// The creator's console commands, with their help.
pub(crate) const COMMANDS: &[(&str, &str)] = &[
    (
        "toymaster_place",
        "SJK creator: hide a toy of the kind given where you aim, facing you ('server': for this server only)",
    ),
    (
        "toymaster_remove",
        "SJK creator: take away the hidden toy nearest you, or the one with the id given",
    ),
    (
        "toymaster_list",
        "SJK creator: list the toys hidden on this map and the kinds there are",
    ),
];

/// Whether `name` is one of [`COMMANDS`].
pub(crate) fn is_command(name: &str) -> bool {
    COMMANDS.iter().any(|(command, _)| *command == name)
}

/// Where a show is, `elapsed` into it: how much the toy is grown (0 while gone), how far
/// it turned (degrees) and how high it hops; `None` once it is over.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pose {
    scale: f32,
    turn: f32,
    lift: f32,
}

/// The phases of `show`, `elapsed` into it.
fn pose(show: &Show, elapsed: Duration) -> Option<Pose> {
    let t = elapsed.as_secs_f32() * 1_000.0;
    let (vanish, gone, pop, spin) = (
        show.vanish as f32,
        show.gone as f32,
        show.pop as f32,
        show.spin as f32,
    );
    let ease = |u: f32| {
        let u = u.clamp(0.0, 1.0);
        u * u * (3.0 - 2.0 * u)
    };
    if t < vanish {
        // Shrinking away, a quick turn as it goes.
        let u = t / vanish.max(1.0);
        return Some(Pose {
            scale: 1.0 - ease(u),
            turn: 180.0 * u * u,
            lift: 0.0,
        });
    }
    let t = t - vanish;
    if t < gone {
        return Some(Pose {
            scale: 0.0,
            turn: 0.0,
            lift: 0.0,
        });
    }
    let t = t - gone;
    if t < pop {
        // Back, overshooting (an ease out with a bounce past 1).
        let u = (t / pop.max(1.0)).clamp(0.0, 1.0) - 1.0;
        return Some(Pose {
            scale: 1.0 + 2.7 * u * u * u + 1.7 * u * u,
            turn: 0.0,
            lift: 0.0,
        });
    }
    let t = t - pop;
    if t >= spin {
        return None;
    }
    Some(Pose {
        scale: 1.0,
        turn: spin_turn(show.turns, spin, t),
        lift: ((t / 1_000.0) * std::f32::consts::TAU * show.turns.max(0.5))
            .sin()
            .abs()
            * 1.5,
    })
}

/// How far a spin of `spin` milliseconds at up to `turns` turns a second turned, `t`
/// milliseconds into it, in degrees: speeding up over [`SPIN_UP`], at full speed, then
/// slowing to a stop over its last quarter.
fn spin_turn(turns: f32, spin: f32, t: f32) -> f32 {
    let top = turns * 360.0 / 1_000.0;
    let up = SPIN_UP.min(spin / 4.0);
    let down_from = (spin * 0.75).max(up);
    let down = (spin - down_from).max(1.0);
    let t = t.clamp(0.0, spin);
    if t < up {
        return top * t * t / (2.0 * up);
    }
    let cruise = top * up / 2.0;
    if t < down_from {
        return cruise + top * (t - up);
    }
    let into = t - down_from;
    cruise + top * (down_from - up) + top * (into - into * into / (2.0 * down))
}

/// How long `show` lasts in all.
fn length(show: &Show) -> Duration {
    Duration::from_millis(u64::from(show.vanish + show.gone + show.pop + show.spin))
}

/// How fast a spin of `spin` milliseconds goes `t` milliseconds into it, from 0 to 1 (its
/// full speed), as [`spin_turn`] turns it.
fn spin_speed(spin: f32, t: f32) -> f32 {
    let up = SPIN_UP.min(spin / 4.0);
    let down_from = (spin * 0.75).max(up);
    let down = (spin - down_from).max(1.0);
    if t < 0.0 || t >= spin {
        0.0
    } else if t < up {
        t / up.max(1.0)
    } else if t < down_from {
        1.0
    } else {
        1.0 - (t - down_from) / down
    }
}

/// Players' models within this distance of a spinning toy spin and squish with it (Sol,
/// 01/11/2026), fading out to none at [`DANCE_FADE`].
const DANCE_FULL: f32 = 320.0;
const DANCE_FADE: f32 = 512.0;
/// How much a model squishes at full speed: this much shorter at the bottom of a hop,
/// as much wider as keeps it about its size.
const SQUISH: f32 = 0.3;

/// A toy's spin as the players around it dance it this frame: where it stands, how far
/// it turned (degrees) and how far its hop squishes them (0 to 1).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Dance {
    origin: Vec3,
    turn: f32,
    squish: f32,
}

/// What `show`, `elapsed` into it, has the players around it dance: only while it spins.
fn dance(show: &Show, elapsed: Duration) -> Option<(f32, f32)> {
    let t = elapsed.as_secs_f32() * 1_000.0 - (show.vanish + show.gone + show.pop) as f32;
    let spin = show.spin as f32;
    if t < 0.0 || t >= spin {
        return None;
    }
    let speed = spin_speed(spin, t);
    // In step with the toy's hops ([`pose`]'s lift).
    let hop = ((t / 1_000.0) * std::f32::consts::TAU * show.turns.max(0.5))
        .sin()
        .abs();
    Some((spin_turn(show.turns, spin, t), hop * speed))
}

/// This frame's dances, for the actors drawn after ([`dance_actor`]).
static DANCES: std::sync::Mutex<Vec<Dance>> = std::sync::Mutex::new(Vec::new());

fn dances() -> std::sync::MutexGuard<'static, Vec<Dance>> {
    DANCES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Turn and squish an actor's drawn `transform` (feet `foot` units under its origin) as
/// the spinning toy nearest it has the players around dance: its looks alone, never
/// where the game has it nor which way it faces.
pub(crate) fn dance_actor(transform: &mut sjk_runtime::Transform, foot: f32) {
    let dances = dances();
    if !dances.is_empty() {
        dance_with(transform, foot, &dances);
    }
}

/// [`dance_actor`] among `dances`.
fn dance_with(transform: &mut sjk_runtime::Transform, foot: f32, dances: &[Dance]) {
    let at = Vec3::from_array(transform.translation);
    let Some((dance, near)) = dances
        .iter()
        .map(|dance| (dance, dance.origin.distance(at)))
        .filter(|(_, near)| *near < DANCE_FADE)
        .min_by(|a, b| a.1.total_cmp(&b.1))
    else {
        return;
    };
    let strength = 1.0 - ((near - DANCE_FULL) / (DANCE_FADE - DANCE_FULL)).clamp(0.0, 1.0);
    let turn = glam::Quat::from_rotation_z((dance.turn * strength).to_radians());
    transform.rotation = (turn * glam::Quat::from_array(transform.rotation)).to_array();
    let squish = dance.squish * strength * SQUISH;
    if squish > 0.0 {
        let [x, y, z] = transform.scale;
        let wide = 1.0 + squish * 0.5;
        transform.scale = [x * wide, y * wide, z * (1.0 - squish)];
        // The feet stay on the floor.
        transform.translation[2] -= foot * z * squish;
    }
}

/// One toy standing on the loaded map.
#[derive(Clone, Debug, PartialEq)]
struct Here {
    id: i64,
    origin: Vec3,
    /// Facing, in degrees.
    yaw: f32,
    model: String,
    scale: f32,
    sound: String,
    show: Show,
    /// Its kind (the creator's list alone).
    kind: Option<String>,
    /// Its show under way, and the moments it played so far (0 nothing, 1 the vanish, 2
    /// the pop).
    playing: Option<(Instant, u8)>,
    /// It played its show on this map load: presses count no more.
    done: bool,
    /// The player's presses on it reached the hub on this map load (so it is found).
    told: bool,
}

/// A sound to play once, or the show's to stop.
struct Cue {
    path: Option<String>,
    origin: [f32; 3],
    source: sjk_audio::SourceId,
    /// The show's sound, carrying far, on [`SHOW_CHANNEL`].
    show: bool,
}

/// A toy the creator asked to hide, waiting for the map's checksum.
struct PendingPlace {
    kind: String,
    origin: Vec3,
    yaw: f32,
    server: String,
}

/// The toys of the loaded map, the presses waiting to be sent and the sounds to play.
#[derive(Default)]
pub(crate) struct ToyMaster {
    /// The hub list's revision the toys were taken from, and the map, checksum, server
    /// and map load they were taken for.
    taken: Option<(u64, String, i32, String, i32)>,
    here: Vec<Here>,
    /// The kinds of toy, as the hub tells the creator.
    kinds: Vec<String>,
    /// The meshes of the toys' models once loaded, and the mesh count they were looked
    /// up at.
    meshes: Vec<(String, Option<usize>)>,
    meshes_seen: usize,
    use_pressed: bool,
    use_held: bool,
    /// Presses on a toy not sent yet, and when the last were.
    presses: Option<(i64, u32)>,
    sent: Option<Instant>,
    /// The last hub answer read.
    answer_seen: u64,
    pending_place: Option<PendingPlace>,
    cues: Vec<Cue>,
    /// Where this frame's sparkles go.
    sparkles: Vec<Vec3>,
    /// The audio generation, the list revision and the game data (its address) the
    /// sounds were handed over for: a pack that came since (the Sith cloud's) is heard.
    sounds_registered: Option<(u64, u64, usize)>,
    /// A world shot's toys, kept without a session.
    #[cfg(test)]
    shot: bool,
}

impl ToyMaster {
    /// Note a `+use` press in `command` for the frame's logic.
    pub(crate) fn observe_use(&mut self, command: &UserCommand) {
        let using = command.buttons & BUTTON_USE != 0;
        if using && !self.use_held {
            self.use_pressed = true;
        }
        self.use_held = using;
    }

    /// The models to load: those of the toys here not found among the loaded ones.
    pub(crate) fn wanted_models(&self) -> impl Iterator<Item = &str> {
        self.meshes
            .iter()
            .filter(|(_, mesh)| mesh.is_none())
            .map(|(model, _)| model.as_str())
    }

    fn mesh_of(&self, model: &str) -> Option<usize> {
        self.meshes
            .iter()
            .find(|(known, _)| known == model)
            .and_then(|(_, mesh)| *mesh)
    }

    /// Leave the map: the toys go, and their sounds stop.
    fn clear(&mut self) {
        for here in self.here.drain(..) {
            if here.playing.is_some() {
                self.cues.push(Cue {
                    path: None,
                    origin: here.origin.to_array(),
                    source: source(here.id),
                    show: true,
                });
            }
        }
        self.taken = None;
        self.meshes.clear();
        self.presses = None;
        self.use_pressed = false;
        dances().clear();
    }

    /// Hand the actors drawn this frame the spinning toys' dances, as at `now`.
    fn publish_dances(&self, now: Instant) {
        let mut dances = dances();
        dances.clear();
        dances.extend(self.here.iter().filter_map(|here| {
            let (started, _) = here.playing?;
            let (turn, squish) = dance(&here.show, now.saturating_duration_since(started))?;
            Some(Dance {
                origin: here.origin,
                turn,
                squish,
            })
        }));
    }

    /// Take the toys standing on `map` of `checksum` on `server` from `secrets`, keeping
    /// what each was doing; `fresh_round` when the map load changed (shows may come
    /// again).
    fn take(
        &mut self,
        secrets: &wire::Secrets,
        map: &str,
        checksum: i32,
        server: &str,
        fresh_round: bool,
    ) {
        let old = std::mem::take(&mut self.here);
        self.kinds = secrets.kinds.clone();
        self.here = secrets
            .secrets
            .iter()
            .filter(|placed| placed.stands_on(map, checksum, Some(server)))
            .map(|placed| {
                let before = old.iter().find(|here| here.id == placed.id);
                Here {
                    id: placed.id,
                    origin: Vec3::from_array(placed.origin),
                    yaw: placed.yaw,
                    model: placed.model.clone(),
                    scale: placed.scale.clamp(0.05, 20.0),
                    sound: placed.sound.clone(),
                    show: placed.show,
                    kind: placed.kind.clone(),
                    playing: before.and_then(|here| here.playing),
                    done: !fresh_round && before.is_some_and(|here| here.done),
                    told: !fresh_round && before.is_some_and(|here| here.told),
                }
            })
            .collect();
        let mut meshes: Vec<(String, Option<usize>)> = Vec::new();
        for here in &self.here {
            if !here.model.is_empty() && !meshes.iter().any(|(model, _)| *model == here.model) {
                meshes.push((here.model.clone(), None));
            }
        }
        self.meshes = meshes;
        self.meshes_seen = usize::MAX;
    }

    /// Toy `id` starts its show at `now`, unless it is playing one.
    fn start(&mut self, id: i64, now: Instant) {
        if let Some(here) = self.here.iter_mut().find(|here| here.id == id)
            && here.playing.is_none()
        {
            here.playing = Some((now, 0));
            here.done = true;
        }
    }

    /// Move the shows on to `now`: the vanish's and the pop's sounds and sparkles, the
    /// show's sound with the pop, and its end.
    fn tick(&mut self, now: Instant) {
        for here in &mut self.here {
            let Some((started, played)) = here.playing.as_mut() else {
                continue;
            };
            let elapsed = now.saturating_duration_since(*started);
            let origin = here.origin.to_array();
            if *played == 0 {
                *played = 1;
                self.cues.push(Cue {
                    path: Some(VANISH_SOUND.to_owned()),
                    origin,
                    source: source(here.id),
                    show: false,
                });
                self.sparkles.push(here.origin);
            }
            let back = Duration::from_millis(u64::from(here.show.vanish + here.show.gone));
            if *played == 1 && elapsed >= back {
                *played = 2;
                self.cues.push(Cue {
                    path: Some(POP_SOUND.to_owned()),
                    origin,
                    source: source(here.id),
                    show: false,
                });
                if !here.sound.is_empty() {
                    self.cues.push(Cue {
                        path: Some(here.sound.clone()),
                        origin,
                        source: source(here.id),
                        show: true,
                    });
                }
                self.sparkles.push(here.origin);
            }
            if elapsed >= length(&here.show) {
                here.playing = None;
                self.cues.push(Cue {
                    path: None,
                    origin,
                    source: source(here.id),
                    show: true,
                });
            }
        }
    }

    /// Count a press on toy `id`: the presses on another toy not sent yet are dropped.
    fn press(&mut self, id: i64) {
        self.presses = match self.presses {
            Some((toy, count)) if toy == id => Some((toy, (count + 1).min(PRESSES_MAX))),
            _ => Some((id, 1)),
        };
    }

    /// The presses to send at `now`, when it is time.
    fn due(&mut self, now: Instant) -> Option<(i64, u32)> {
        if self
            .sent
            .is_some_and(|sent| now.saturating_duration_since(sent) < SEND_EVERY)
        {
            return None;
        }
        let presses = self.presses.take()?;
        self.sent = Some(now);
        Some(presses)
    }
}

/// What the Use prompt says at a toy: what it does is the secret.
const PROMPT: &str = "???";

/// Whether `here` stands to be seen at `now`: not while its show has it shrunk away.
fn shown(here: &Here, now: Instant) -> bool {
    here.playing
        .and_then(|(started, _)| pose(&here.show, now.saturating_duration_since(started)))
        .is_none_or(|pose| pose.scale > 0.01)
}

/// The toy among `toys` the player at `from` facing `yaw` (radians) may press: within
/// [`REACH`] across the floor and [`REACH_UP`] up or down, within [`FACING`] of their
/// facing; the nearest of those.
fn pick<'a>(from: Vec3, yaw: f32, toys: impl IntoIterator<Item = &'a Here>) -> Option<i64> {
    let facing = glam::Vec2::from_angle(yaw);
    toys.into_iter()
        .filter_map(|here| {
            let way = (here.origin - from).truncate();
            let across = way.length();
            let up = (here.origin.z - from.z).abs();
            let faced = across < 1.0 || facing.angle_to(way).abs() <= FACING;
            (across <= REACH && up <= REACH_UP && faced).then_some((here.id, across))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

impl GpuState {
    fn toy_master_print(&mut self, text: String) {
        self.chat.receive(
            sjk_client::ServerEventKind::Print,
            text,
            None,
            Instant::now(),
        );
    }

    /// Find the toys' models among the loaded ones, when more have loaded.
    fn find_toy_master_meshes(&mut self) {
        if self.toy_master.meshes_seen == self.object_meshes.len() {
            return;
        }
        self.toy_master.meshes_seen = self.object_meshes.len();
        for (model, mesh) in &mut self.toy_master.meshes {
            *mesh = self.object_meshes.iter().position(|loaded| {
                loaded.appearance.variant.is_empty() && loaded.appearance.model == *model
            });
        }
    }

    /// The frame's Toy Master logic, after the looks: the toys of the loaded map, the
    /// hub's answers, the shows (`events`, this server's toy events) and the player's
    /// presses.
    pub(crate) fn update_toy_master(&mut self, events: &[sjk_identity::ToyEvent]) {
        #[cfg(test)]
        if self.toy_master.shot {
            self.find_toy_master_meshes();
            return;
        }
        let now = Instant::now();
        self.read_toy_master_answer(now);
        let Some(session) = self
            .live_session
            .as_ref()
            .filter(|session| !session.is_local())
        else {
            self.toy_master.clear();
            return;
        };
        let server = session.server().to_string();
        let round = session.server_id();
        let map_name = crate::remap_blocked_maps::map_name(&self.world_load_map);
        let (revision, on_map) = crate::player_identity::with_snapshot(|snapshot| {
            (
                snapshot.secrets_revision,
                snapshot
                    .secrets
                    .as_ref()
                    .is_some_and(|secrets| secrets.on_map(map_name)),
            )
        })
        .unwrap_or_default();
        // The map's checksum is read when a toy may stand on it (`map_edits`).
        if on_map || self.toy_master.pending_place.is_some() {
            self.map_edits.want_checksum(map_name);
        }
        let Some((map, checksum)) = self.map_edits.map().map(|(map, c)| (map.to_owned(), c)) else {
            self.toy_master.clear();
            return;
        };
        self.place_pending_toy(&map, checksum);
        let taken = self.toy_master.taken.as_ref();
        let fresh_round = taken.is_none_or(|taken| taken.4 != round);
        let fresh = taken.is_some_and(|(taken, taken_map, taken_checksum, taken_server, _)| {
            *taken == revision
                && *taken_checksum == checksum
                && *taken_map == map
                && *taken_server == server
        });
        if !fresh || fresh_round {
            let secrets = crate::player_identity::with_snapshot(|snapshot| {
                snapshot.secrets.clone().unwrap_or_default()
            })
            .unwrap_or_default();
            self.toy_master
                .take(&secrets, &map, checksum, &server, fresh_round);
            self.toy_master.taken = Some((revision, map, checksum, server.clone(), round));
        }
        self.find_toy_master_meshes();
        for event in events {
            if event.toy != wire::TOY {
                continue;
            }
            if let Some(id) = wire::parse_event(&event.event) {
                self.toy_master.start(id, now);
            }
        }
        self.toy_master.tick(now);
        self.toy_master.publish_dances(now);
        if self.toy_master.here.is_empty() {
            self.toy_master.use_pressed = false;
            return;
        }
        let Some(session) = self.live_session.as_ref() else {
            return;
        };
        let player = &session.latest_snapshot().player;
        let pressed = std::mem::take(&mut self.toy_master.use_pressed);
        let faced = crate::mystery_box::plays(player)
            .then(|| {
                let from = self
                    .local_prediction
                    .predicted_state()
                    .map_or_else(|| player.origin(), |state| state.origin);
                pick(
                    Vec3::from_array(from),
                    self.camera_yaw,
                    &self.toy_master.here,
                )
            })
            .flatten()
            .and_then(|id| self.toy_master.here.iter().find(|here| here.id == id))
            .filter(|here| shown(here, now));
        // A toy that played its show here, once the player found it, takes no more.
        let counts = faced.map(|here| (here.id, !(here.done && here.told)));
        if let Some((id, counts)) = counts {
            // What Use does at it, by the crosshair as at a mystery box (`use_prompt`): a
            // secret, so "???" (Sol, 01/11/2026), greyed once it takes no more.
            self.want_use_prompt(crate::use_prompt::Prompt {
                action: PROMPT.to_owned(),
                line: None,
                ready: counts,
            });
            if pressed && counts {
                self.toy_master.press(id);
            }
        }
        if let Some((id, presses)) = self.toy_master.due(now) {
            crate::player_identity::with_service(|service| {
                service.use_secret(id, server, round, presses);
            });
        }
    }

    /// Act on the hub's last answer about the toys, once.
    fn read_toy_master_answer(&mut self, now: Instant) {
        let seen = self.toy_master.answer_seen;
        let Some((serial, outcome)) = crate::player_identity::with_snapshot(|snapshot| {
            snapshot
                .secret
                .as_ref()
                .filter(|(serial, _)| *serial != seen)
                .cloned()
        })
        .flatten() else {
            return;
        };
        self.toy_master.answer_seen = serial;
        match outcome {
            Ok(SecretDone::Used {
                id,
                show,
                done,
                found,
            }) => {
                if show {
                    self.toy_master.start(id, now);
                }
                if let Some(here) = self.toy_master.here.iter_mut().find(|here| here.id == id) {
                    here.told = true;
                    here.done |= done;
                }
                if found {
                    self.announce_toy_found();
                }
            }
            Ok(SecretDone::Placed(id)) => self.toy_master_print(format!("Toy {id} hidden")),
            Ok(SecretDone::Removed(id)) => self.toy_master_print(format!("Toy {id} taken away")),
            // Presses on a toy are cosmetic: a refusal tells the creator alone.
            Err(why) if crate::player_identity::is_creator() => {
                self.toy_master_print(format!("Toy Master: {why}"));
            }
            Err(_) => {}
        }
    }

    /// The player found a toy: its achievement, the newest of the Toy Master's they hold,
    /// is announced as any other (the hub granted its kyber crystal).
    fn announce_toy_found(&mut self) {
        let held = crate::player_identity::own_achievements().unwrap_or_default();
        let Some(kind) = held
            .iter()
            .filter(|held| held.unlocked > 0)
            .filter_map(|held| {
                crate::achievements::secret_kind(held).map(|kind| (kind, held.unlocked))
            })
            .max_by_key(|(_, unlocked)| *unlocked)
            .map(|(kind, _)| kind)
        else {
            return;
        };
        if let Some(console) = self.console.as_mut() {
            console.push_log(format!(
                "^3Achievement unlocked: ^7{} ^5({})",
                kind.name, kind.description
            ));
        }
        self.unlock_toast
            .push(crate::unlock_toast::Unlock::Achievement {
                kind,
                crystal: true,
            });
    }

    /// Send the toy the creator asked to hide once the map's checksum is known.
    fn place_pending_toy(&mut self, map: &str, checksum: i32) {
        let Some(pending) = self.toy_master.pending_place.take() else {
            return;
        };
        let placing = Placing {
            kind: pending.kind,
            map: map.to_owned(),
            checksum,
            server: pending.server,
            origin: pending.origin.to_array(),
            yaw: pending.yaw,
        };
        if crate::player_identity::with_service(|service| service.place_secret(placing)).is_none() {
            self.toy_master_print("Toy Master: not connected to the SJK hub".to_owned());
        }
    }

    /// `toymaster_place`, `toymaster_remove` and `toymaster_list`: SJK's creator only.
    pub(crate) fn toy_master_command(
        &mut self,
        name: &str,
        args: &[String],
    ) -> Result<Vec<String>, String> {
        if !crate::player_identity::is_creator() {
            return Err("Only the Toy Master hides toys".into());
        }
        if name == "toymaster_list" {
            return Ok(self.toy_master_list());
        }
        let session = self
            .live_session
            .as_ref()
            .filter(|session| !session.is_local())
            .ok_or("Join a game server first: toys stand on a server's map")?;
        let server = session.server().to_string();
        match name {
            "toymaster_place" => {
                let kind = args
                    .first()
                    .ok_or("toymaster_place <kind> [server] (toymaster_list names the kinds)")?
                    .to_owned();
                let one_server = match args.get(1).map(String::as_str) {
                    None => false,
                    Some("server") => true,
                    Some(other) => {
                        return Err(format!("toymaster_place <kind> [server], not {other}"));
                    }
                };
                let (eye, forward) = self.world_notes.view().ok_or("No view to aim from")?;
                let hit = self
                    .decal_surfaces
                    .ray_hit(eye, forward, 4096.0)
                    .ok_or("Aim at the floor where the toy should stand")?;
                let yaw = forward.y.atan2(forward.x).to_degrees() + 180.0;
                self.toy_master.pending_place = Some(PendingPlace {
                    kind: kind.clone(),
                    origin: eye + forward * hit.distance,
                    yaw: yaw.rem_euclid(360.0),
                    server: if one_server { server } else { String::new() },
                });
                let map = crate::remap_blocked_maps::map_name(&self.world_load_map).to_owned();
                self.map_edits.want_checksum(&map);
                Ok(vec![format!(
                    "Hiding a {kind} on {map}{}...",
                    if one_server { " for this server" } else { "" }
                )])
            }
            "toymaster_remove" => {
                let id = match args.first() {
                    Some(id) => id.parse().map_err(|_| format!("Not a toy id: {id}"))?,
                    None => {
                        let from = Vec3::from_array(session.latest_snapshot().player.origin());
                        self.toy_master
                            .here
                            .iter()
                            .map(|here| (here.id, here.origin.distance(from)))
                            .filter(|(_, distance)| *distance <= REMOVE_REACH)
                            .min_by(|a, b| a.1.total_cmp(&b.1))
                            .map(|(id, _)| id)
                            .ok_or("No hidden toy near you: give its id (toymaster_list)")?
                    }
                };
                crate::player_identity::with_service(|service| service.remove_secret(id))
                    .ok_or("Not connected to the SJK hub")?;
                Ok(vec![format!("Taking toy {id} away...")])
            }
            _ => Err(format!("Unknown command {name}")),
        }
    }

    fn toy_master_list(&self) -> Vec<String> {
        let toys = &self.toy_master;
        let mut lines = vec![if toys.kinds.is_empty() {
            "Toy kinds: none read from the hub yet".to_owned()
        } else {
            format!("Toy kinds: {}", toys.kinds.join(", "))
        }];
        if toys.here.is_empty() {
            lines.push("No toy hidden on this map".to_owned());
            return lines;
        }
        let from = self
            .live_session
            .as_ref()
            .map(|session| Vec3::from_array(session.latest_snapshot().player.origin()));
        lines.push(format!("{} toys hidden here:", toys.here.len()));
        for here in &toys.here {
            let away = from.map_or(String::new(), |from| {
                format!(", {:.0} units away", here.origin.distance(from))
            });
            let done = if here.done { ", played its show" } else { "" };
            lines.push(format!(
                "  {}: {} at {:.0} {:.0} {:.0}{away}{done}",
                here.id,
                here.kind.as_deref().unwrap_or("?"),
                here.origin.x,
                here.origin.y,
                here.origin.z
            ));
        }
        lines
    }

    /// Add this frame's toys to the scene, each as its show poses it (none while gone).
    pub(crate) fn append_toy_master_instances(&mut self) {
        let now = Instant::now();
        for here in &self.toy_master.here {
            let Some(mesh) = self.toy_master.mesh_of(&here.model) else {
                continue;
            };
            let pose = here
                .playing
                .and_then(|(started, _)| pose(&here.show, now.saturating_duration_since(started)))
                .unwrap_or(Pose {
                    scale: 1.0,
                    turn: 0.0,
                    lift: 0.0,
                });
            if pose.scale <= 0.01 {
                continue;
            }
            let scale = here.scale * pose.scale;
            self.object_groups[mesh].push(ActorInstance::new(
                (here.origin + Vec3::Z * pose.lift).to_array(),
                sjk_client::legacy_angles_to_quaternion([0.0, here.yaw + pose.turn, 0.0]),
                [scale; 3],
            ));
        }
    }

    /// Play the toys' sounds and sparkles; their sounds are handed to the audio when the
    /// toys or the audio change.
    pub(crate) fn play_toy_master_sounds(&mut self, audio: &mut Option<crate::GameAudio>) {
        let Some(audio) = audio.as_mut() else {
            self.toy_master.cues.clear();
            self.toy_master.sparkles.clear();
            return;
        };
        let revision = self
            .toy_master
            .taken
            .as_ref()
            .map_or(0, |(revision, ..)| *revision);
        let data = self.vfs.as_ref().map_or(0, |vfs| Arc::as_ptr(vfs) as usize);
        if self.toy_master.sounds_registered != Some((audio.generation(), revision, data))
            && !self.toy_master.here.is_empty()
            && let Some(vfs) = self.vfs.as_deref()
        {
            let shows = self
                .toy_master
                .here
                .iter()
                .map(|here| here.sound.as_str())
                .filter(|sound| !sound.is_empty());
            audio.register_effect_paths(vfs, [VANISH_SOUND, POP_SOUND].into_iter().chain(shows));
            self.toy_master.sounds_registered = Some((audio.generation(), revision, data));
        }
        if let Some(vfs) = self.vfs.clone() {
            for at in self.toy_master.sparkles.drain(..) {
                crate::effect_runtime::spawn_effect(
                    &mut self.particles,
                    &mut self.effect_aux,
                    &mut self.effects,
                    &vfs,
                    EFFECT,
                    at + Vec3::Z * 8.0,
                    Instant::now(),
                    0,
                    0,
                    &mut None,
                    crate::combat_effects::rotation_from_direction([0.0, 0.0, 1.0]),
                );
            }
        }
        for cue in self.toy_master.cues.drain(..) {
            let Some(path) = cue.path else {
                audio.stop_toy(cue.source, SHOW_CHANNEL);
                continue;
            };
            if cue.show {
                // Not every player has the show's sound: without it the toy spins silent.
                if !audio.has_sound(&path) {
                    crate::log::progress(format_args!(
                        "toy master: the show's sound {path} is not in the game data (an \
                         optional pack such as the Sith cloud's has it): the toy spins silent"
                    ));
                    continue;
                }
                audio.play_toy_reaching(
                    crate::toy_sounds::Toy::Master,
                    &path,
                    1.0,
                    cue.origin,
                    cue.source,
                    SHOW_CHANNEL,
                    sjk_audio::Attenuation::Linear {
                        full_volume_distance: SOUND_FULL,
                        falloff_per_unit: 1.0 / (SOUND_RANGE - SOUND_FULL),
                    },
                );
            } else {
                audio.play_toy(
                    crate::toy_sounds::Toy::Master,
                    &path,
                    1.0,
                    Some(cue.origin),
                    cue.source,
                    CUE_CHANNEL,
                );
            }
        }
    }
}

#[cfg(test)]
impl GpuState {
    /// For the world shots: toys of `model` at each of `toys` (origin and yaw), each
    /// `since` into its show when given (`show` playing), else standing still.
    pub(crate) fn shot_toys(
        &mut self,
        model: &str,
        show: Show,
        toys: &[([f32; 3], f32, Option<Duration>)],
    ) {
        let now = Instant::now();
        self.toy_master.shot = true;
        self.toy_master.here = toys
            .iter()
            .enumerate()
            .map(|(index, (origin, yaw, since))| Here {
                id: index as i64 + 1,
                origin: Vec3::from_array(*origin),
                yaw: *yaw,
                model: model.to_owned(),
                scale: 1.0,
                sound: String::new(),
                show,
                kind: None,
                playing: since.map(|since| (now - since, 2)),
                done: false,
                told: false,
            })
            .collect();
        self.toy_master.meshes = vec![(model.to_owned(), None)];
        self.toy_master.meshes_seen = usize::MAX;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHOW: Show = Show {
        vanish: 200,
        gone: 1_000,
        pop: 400,
        spin: 8_000,
        turns: 2.0,
    };

    fn at(ms: u64) -> Option<Pose> {
        pose(&SHOW, Duration::from_millis(ms))
    }

    #[test]
    fn a_show_shrinks_away_stays_gone_pops_back_and_spins_to_a_stop() {
        assert_eq!(at(0).unwrap().scale, 1.0);
        assert!(at(100).unwrap().scale < 1.0);
        assert_eq!(at(700).unwrap().scale, 0.0);
        // The pop overshoots, then settles.
        let peak = (0..400)
            .map(|ms| at(1_200 + ms).unwrap().scale)
            .fold(0.0, f32::max);
        assert!(peak > 1.05, "{peak}");
        assert!((at(1_600).unwrap().scale - 1.0).abs() < 1e-4);
        // The spin speeds up, runs at full speed and slows to a stop.
        let turn = |ms: u64| at(1_600 + ms).unwrap().turn;
        let speed = |ms: u64| turn(ms + 10) - turn(ms);
        assert!(speed(100) < speed(3_000));
        assert!((speed(3_000) - 2.0 * 360.0 / 100.0).abs() < 0.1);
        assert!(speed(7_950) < speed(6_500) && speed(7_950) < 0.5);
        assert_eq!(at(1_600 + 8_000), None);
        assert_eq!(length(&SHOW), Duration::from_millis(9_600));
    }

    /// The players around dance only while the toy spins, turning as it turns, squishing
    /// in step with its hops and harder as it speeds up.
    #[test]
    fn players_dance_only_while_the_toy_spins() {
        let dance_at = |ms: u64| dance(&SHOW, Duration::from_millis(ms));
        assert_eq!(dance_at(0), None);
        assert_eq!(dance_at(1_000), None);
        assert_eq!(dance_at(1_599), None);
        assert_eq!(dance_at(1_600 + 8_000), None);
        let (turn, _) = dance_at(1_600 + 3_000).unwrap();
        assert_eq!(turn, at(1_600 + 3_000).unwrap().turn);
        // At the top of a hop it squishes most, on landing not at all.
        let squish = |ms: u64| dance_at(1_600 + ms).unwrap().1;
        assert!(squish(3_125) > 0.99, "{}", squish(3_125));
        assert!(squish(3_250) < 0.01, "{}", squish(3_250));
        // Speeding up, it squishes less.
        assert!(squish(125) < 0.2);
        assert!(spin_speed(8_000.0, 7_999.0) < 0.01);
    }

    /// A model near a spinning toy turns and squishes with its feet on the floor; one far
    /// off is left as it was, one between fades.
    #[test]
    fn a_dance_turns_and_squishes_the_model_feet_down() {
        let dances = [Dance {
            origin: Vec3::ZERO,
            turn: 90.0,
            squish: 1.0,
        }];
        let start = sjk_runtime::Transform {
            translation: [100.0, 0.0, 24.0],
            rotation: glam::Quat::IDENTITY.to_array(),
            scale: [1.0; 3],
        };
        let mut near = start;
        dance_with(&mut near, 24.0, &dances);
        let turned = glam::Quat::from_array(near.rotation) * Vec3::X;
        assert!((turned - Vec3::Y).length() < 1e-4, "{turned}");
        assert!((near.scale[2] - (1.0 - SQUISH)).abs() < 1e-5);
        assert!(near.scale[0] > 1.0 && near.scale[0] == near.scale[1]);
        // The feet, 24 under the origin times the height, stay at 0.
        assert!((near.translation[2] - 24.0 * near.scale[2]).abs() < 1e-4);
        let mut far = start;
        far.translation[0] = DANCE_FADE + 1.0;
        dance_with(&mut far, 24.0, &dances);
        assert_eq!(far, start_at(&start, DANCE_FADE + 1.0));
        let mut between = start;
        between.translation[0] = (DANCE_FULL + DANCE_FADE) / 2.0;
        dance_with(&mut between, 24.0, &dances);
        let squished = 1.0 - between.scale[2];
        assert!(squished > 0.0 && squished < SQUISH, "{squished}");
    }

    fn start_at(start: &sjk_runtime::Transform, x: f32) -> sjk_runtime::Transform {
        let mut moved = *start;
        moved.translation[0] = x;
        moved
    }

    /// A toy shrunk away in its show is not shown, so no prompt stands over it.
    #[test]
    fn a_toy_gone_in_its_show_is_not_shown() {
        let mut toys = ToyMaster::default();
        toys.take(
            &wire::Secrets {
                secrets: vec![toy(1, [0.0; 3])],
                ..Default::default()
            },
            "mp/ffa3",
            -5,
            "",
            true,
        );
        let now = Instant::now();
        assert!(shown(&toys.here[0], now));
        toys.start(1, now);
        assert!(shown(&toys.here[0], now));
        assert!(!shown(&toys.here[0], now + Duration::from_millis(700)));
        assert!(shown(&toys.here[0], now + Duration::from_millis(2_000)));
    }

    #[test]
    fn the_spin_never_turns_back() {
        let mut last = 0.0;
        for ms in (0..=8_000).step_by(50) {
            let turn = spin_turn(3.0, 8_000.0, ms as f32);
            assert!(turn >= last, "{ms}: {turn} < {last}");
            last = turn;
        }
    }

    fn toy(id: i64, origin: [f32; 3]) -> wire::Secret {
        wire::Secret {
            id,
            map: "mp/ffa3".to_owned(),
            checksum: -5,
            server: String::new(),
            origin,
            yaw: 0.0,
            model: "models/a.md3".to_owned(),
            scale: 1.0,
            sound: "sound/a.mp3".to_owned(),
            show: SHOW,
            kind: None,
        }
    }

    #[test]
    fn the_toys_of_the_map_are_taken_and_each_model_wanted_once() {
        let mut toys = ToyMaster::default();
        let mut other = toy(3, [0.0; 3]);
        other.map = "mp/ffa1".to_owned();
        let secrets = wire::Secrets {
            secrets: vec![toy(1, [0.0; 3]), toy(2, [100.0, 0.0, 0.0]), other],
            kinds: Vec::new(),
        };
        toys.take(&secrets, "mp/ffa3", -5, "1.2.3.4:29070", true);
        assert_eq!(toys.here.len(), 2);
        assert_eq!(toys.wanted_models().collect::<Vec<_>>(), ["models/a.md3"]);
        // A show played on this map load stays played when the list is read again, and
        // comes back with a new map load.
        toys.start(1, Instant::now());
        toys.take(&secrets, "mp/ffa3", -5, "1.2.3.4:29070", false);
        assert!(toys.here[0].done && toys.here[0].playing.is_some());
        toys.take(&secrets, "mp/ffa3", -5, "1.2.3.4:29070", true);
        assert!(!toys.here[0].done);
    }

    #[test]
    fn presses_gather_and_go_twice_a_second() {
        let mut toys = ToyMaster::default();
        let now = Instant::now();
        assert_eq!(toys.due(now), None);
        for _ in 0..3 {
            toys.press(4);
        }
        assert_eq!(toys.due(now), Some((4, 3)));
        toys.press(4);
        assert_eq!(toys.due(now + Duration::from_millis(100)), None);
        // Presses on another toy replace them; never more than the hub takes.
        for _ in 0..30 {
            toys.press(5);
        }
        assert_eq!(toys.due(now + SEND_EVERY), Some((5, PRESSES_MAX)));
    }

    #[test]
    fn the_faced_toy_within_reach_is_pressed() {
        let mut toys = ToyMaster::default();
        let secrets = wire::Secrets {
            secrets: vec![toy(1, [60.0, 0.0, 0.0]), toy(2, [-60.0, 0.0, 0.0])],
            kinds: Vec::new(),
        };
        toys.take(&secrets, "mp/ffa3", -5, "1.2.3.4:29070", true);
        assert_eq!(pick(Vec3::ZERO, 0.0, &toys.here), Some(1));
        assert_eq!(pick(Vec3::ZERO, std::f32::consts::PI, &toys.here), Some(2));
        assert_eq!(pick(Vec3::new(0.0, 500.0, 0.0), 0.0, &toys.here), None);
    }

    #[test]
    fn a_show_plays_its_moments_once_then_stops_its_sound() {
        let mut toys = ToyMaster::default();
        let secrets = wire::Secrets {
            secrets: vec![toy(1, [0.0; 3])],
            kinds: Vec::new(),
        };
        toys.take(&secrets, "mp/ffa3", -5, "1.2.3.4:29070", true);
        let start = Instant::now();
        toys.start(1, start);
        toys.tick(start);
        toys.tick(start + Duration::from_millis(100));
        assert_eq!(toys.cues.len(), 1);
        toys.tick(start + Duration::from_millis(1_300));
        let paths: Vec<Option<&str>> = toys.cues.iter().map(|cue| cue.path.as_deref()).collect();
        assert_eq!(
            paths,
            [Some(VANISH_SOUND), Some(POP_SOUND), Some("sound/a.mp3")]
        );
        // Starting again while it plays changes nothing.
        toys.start(1, start + Duration::from_millis(1_400));
        toys.tick(start + length(&SHOW));
        assert!(toys.here[0].playing.is_none() && toys.here[0].done);
        assert!(
            toys.cues
                .last()
                .is_some_and(|cue| cue.path.is_none() && cue.show)
        );
        assert_eq!(toys.sparkles.len(), 2);
    }
}
