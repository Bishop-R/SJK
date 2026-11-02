//! The boombox, SJK's toy (`toy_boombox`; Sol's idea, 25/10/2026): a big 80s stereo the
//! player carries on the shoulder ([`MODEL`]), playing music from the player's own
//! pk3s, which the SJK players standing close hear.
//!
//! - **Tracks** are files in the game data, as emotes are: `sound/boombox/<id>.mp3` (or
//!   `.wav`) in any pk3 or the game folder ([`load`]). `<id>` is 1 to 24 of `a` to `z`,
//!   `0` to `9` and `_`; the title shown is the id with spaces for its `_`.
//! - **`toy_boombox`** puts the stereo on the shoulder and plays the chosen track (the
//!   first one at first), or takes it off. `boombox` ([`TRACK_COMMAND`]) lists the
//!   tracks, and `boombox <id>`, `next`, `prev`, `play` and `stop` choose and play them.
//! - **Verified players only** (Sol's rule, 25/10/2026): on a server, only a player
//!   whose key the SJK hub verified may turn it on, and every SJK client ignores the
//!   boombox of a player the hub does not show as verified. In a local game anyone may.
//! - **Heard close by:** the player sets how far it carries ([`RANGE_CVAR`], from
//!   [`MIN_RANGE`] to [`MAX_RANGE`] units). It plays at full volume within a third of
//!   that and fades to nothing at the edge; a listener further away does not play it
//!   at all.
//!
//! No game server knows of it, and the hub never carries sound. The stereo is a toy in
//! the player's look (`looks.rs`, [`TOY`]), so every SJK player on the server draws it on
//! their shoulder; what plays travels through the toy relay (`PROTOCOL.md`, "Toy
//! relay"): `play_<moment>_<bytes>_<range>_<id>` names the track, its file's size and
//! the game server's time it started at, `stop` ends it. The carrier's client tells each
//! SJK player who comes within reach, one event every [`SEND_GAP`], and again every
//! [`RETELL`]. A listener plays the track from where it is now (the server's time minus
//! the start) when they have a file of that id and size, and says once in the console
//! which track they lack otherwise; a track not told again for [`LOST`] stops.

use crate::GpuState;
use crate::actor_instance::ActorInstance;
use glam::{Quat, Vec3};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// The toy's console command: put the stereo on, or take it off.
pub(crate) const COMMAND: &str = "toy_boombox";
/// The tracks' command (`boombox [id|next|prev|play|stop]`).
pub(crate) const TRACK_COMMAND: &str = "boombox";
/// [`TRACK_COMMAND`]'s console help.
pub(crate) const HELP: &str = "Boombox toy: list your tracks (sound/boombox/<id>.mp3), or play one: boombox <id>|next|prev|play|stop";
/// The toy's id in a look's `toys` and in relayed toy events.
pub(crate) const TOY: &str = "boombox";
/// The toy's icon (quick wheel, Collection, key bindings).
pub(crate) const ICON: &str = "gfx/sjk/toy_boombox.png";
/// The stereo on the shoulder.
pub(crate) const MODEL: &str = "models/sjk/boombox.md3";
/// Where the tracks are.
pub(crate) const FOLDER: &str = "sound/boombox";
/// How far the player's boombox carries, in units (archived).
pub(crate) const RANGE_CVAR: &str = "cg_toyBoomboxRange";
pub(crate) const DEFAULT_RANGE: i64 = 400;
pub(crate) const MIN_RANGE: i64 = 128;
pub(crate) const MAX_RANGE: i64 = 1_000;
/// The longest track id.
const MAX_ID: usize = 24;
/// The share of the range heard at full volume; past it the music fades out.
pub(crate) const FULL_SHARE: f32 = 0.35;
/// A listener playing a track stops it this far past the range (no flutter at the edge);
/// the carrier tells the players this far past it, so they know before they hear it.
const MARGIN: f32 = 64.0;
/// Shortest time between two events sent (the relay takes 40 a second from a player,
/// which the other toys share).
const SEND_GAP: Duration = Duration::from_millis(50);
/// A player told of the track is told again this often.
const RETELL: Duration = Duration::from_secs(15);
/// A track not told again this long stops.
const LOST: Duration = Duration::from_secs(40);
/// The pause between the end of a track and the next.
const END_PAUSE: Duration = Duration::from_millis(1_500);
/// The stereo rides on this bolt of the body, the upper back.
const BOLT: &str = "*back";
/// Its place from the bolt, in the body's frame (ahead, to the left, up): its bottom on the
/// right shoulder, its side against the head.
const SEAT: [f32; 3] = [5.0, -10.0, 1.0];
/// Its size against the model's.
const SCALE: f32 = 0.85;
const SLOTS: usize = crate::looks::SLOTS;
/// The boomboxes' sound sources: their own, so an emote's sound never cuts the music.
const SOURCE_BASE: u32 = 0x5000_0000;
const ET_PLAYER: u8 = 1;
const EF_DEAD: u32 = 1 << 1;
const EF_NODRAW: u32 = 1 << 8;
/// `PMF_FOLLOW`.
const PMF_FOLLOW: u16 = 4096;

/// A bundled file's bytes, by its name in `assets/boombox`.
macro_rules! bundled {
    ($name:literal) => {
        include_bytes!(concat!("../assets/boombox/", $name))
    };
}

/// The bundled files at their game paths (made by `scripts/boombox_assets.py`).
const FILES: [(&str, &[u8]); 4] = [
    (MODEL, bundled!("boombox.md3")),
    ("models/sjk/boombox.jpg", bundled!("boombox.jpg")),
    ("shaders/sjk_boombox.shader", bundled!("boombox.shader")),
    (ICON, bundled!("toy_boombox.png")),
];

/// Mount the bundled files.
pub(crate) fn mount(vfs: &mut sjk_vfs::VirtualFileSystem) -> Result<(), sjk_vfs::VfsError> {
    vfs.mount_memory("SJK boombox", FILES.iter().copied())?;
    Ok(())
}

/// The sound source a player's boombox plays on.
pub(crate) fn source(slot: u8) -> sjk_audio::SourceId {
    sjk_audio::SourceId(SOURCE_BASE + u32::from(slot))
}

/// The range `value` stands for: a whole number of units within the bounds.
pub(crate) fn clamp_range(value: i64) -> u16 {
    value.clamp(MIN_RANGE, MAX_RANGE) as u16
}

/// One track in the game data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Track {
    pub(crate) id: String,
    pub(crate) path: String,
    /// Its file's size, which tells two tracks of the same id apart.
    pub(crate) bytes: u64,
}

impl Track {
    pub(crate) fn title(&self) -> String {
        title(&self.id)
    }
}

/// How a track of `id` is named on screen.
fn title(id: &str) -> String {
    id.replace('_', " ")
}

/// Whether `id` may name a track: 1 to [`MAX_ID`] of `a` to `z`, `0` to `9` and `_`.
pub(crate) fn valid_id(id: &str) -> bool {
    (1..=MAX_ID).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// The tracks in `vfs`: every `sound/boombox/<id>.mp3` or `.wav` whose id is valid, by
/// id (an `.mp3` before a `.wav` of the same id).
pub(crate) fn load(vfs: &sjk_vfs::VirtualFileSystem) -> Vec<Track> {
    let mut tracks = Vec::new();
    for extension in [".mp3", ".wav"] {
        for name in vfs.list_files(FOLDER, extension) {
            let Some(id) = name.strip_suffix(extension) else {
                continue;
            };
            if !valid_id(id) || tracks.iter().any(|track: &Track| track.id == id) {
                continue;
            }
            let path = format!("{FOLDER}/{name}");
            let Some(bytes) = vfs.size(&path).ok().flatten() else {
                continue;
            };
            tracks.push(Track {
                id: id.to_owned(),
                path,
                bytes,
            });
        }
    }
    tracks.sort_by(|a, b| a.id.cmp(&b.id));
    tracks
}

/// A track as it plays: which, the game server's time it began at, and how far it
/// carries.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Song {
    moment: i32,
    bytes: u64,
    range: u16,
    id: String,
}

/// A relayed event of the toy.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Play(Song),
    Stop,
}

const PLAY_PREFIX: &str = "play_";
const STOP: &str = "stop";

impl Event {
    fn text(&self) -> String {
        match self {
            Self::Play(song) => format!(
                "{PLAY_PREFIX}{}_{}_{}_{}",
                song.moment, song.bytes, song.range, song.id
            ),
            Self::Stop => STOP.to_owned(),
        }
    }

    fn parse(event: &str) -> Option<Self> {
        if event == STOP {
            return Some(Self::Stop);
        }
        let mut words = event.strip_prefix(PLAY_PREFIX)?.splitn(4, '_');
        let moment = words.next()?.parse().ok()?;
        let bytes = words.next()?.parse().ok()?;
        let range = clamp_range(words.next()?.parse().ok()?);
        let id = words.next()?;
        valid_id(id).then(|| {
            Self::Play(Song {
                moment,
                bytes,
                range,
                id: id.to_owned(),
            })
        })
    }
}

/// Another player's track as heard, and when.
#[derive(Clone, Debug)]
struct Heard {
    song: Song,
    at: Instant,
}

/// What this client plays for a slot: the file and the start it plays from.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Sounding {
    path: String,
    moment: i32,
}

/// A change to what plays, for the audio.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Cue {
    /// Play `path` for `slot` from `offset` in, at `origin` (`None`: the player's own,
    /// in both ears), heard up to `range` units away.
    Start {
        slot: u8,
        path: String,
        origin: Option<[f32; 3]>,
        offset: Duration,
        range: f32,
    },
    Stop {
        slot: u8,
    },
}

/// The toy's state: the player's own stereo and track, who was told of it, the other
/// players' tracks and what plays here.
pub(crate) struct Boombox {
    on: bool,
    /// The tracks in the game data, read when first needed on a server.
    tracks: Option<Vec<Track>>,
    /// The chosen track's id.
    chosen: Option<String>,
    /// The player's own track playing, if any.
    playing: Option<Song>,
    server: Option<SocketAddr>,
    /// Who was told of the own track (bits) and when.
    told: u64,
    told_at: [Option<Instant>; SLOTS],
    send_at: Option<Instant>,
    heard: [Option<Heard>; SLOTS],
    sounding: [Option<Sounding>; SLOTS],
    /// Where each other player's music plays from this frame.
    placed: [Option<[f32; 3]>; SLOTS],
    /// The tracks a listener lacks, said once in the console: by slot, the id.
    lacking: [Option<String>; SLOTS],
    /// Who carries a stereo drawn this frame (bits by slot).
    carriers: u64,
    /// The SJK players within the own track's range this frame (bits by slot): its
    /// listeners, for DJ, Block Party and Tough Crowd.
    listeners: u64,
    cues: Vec<Cue>,
    /// The tracks given to the audio, freed when nothing plays them.
    loaded: Vec<String>,
    /// Slots that carry one in a world shot, without a server.
    #[cfg(test)]
    pub(crate) shot_carriers: u64,
}

impl Default for Boombox {
    fn default() -> Self {
        Self {
            on: false,
            tracks: None,
            chosen: None,
            playing: None,
            server: None,
            told: 0,
            told_at: [None; SLOTS],
            send_at: None,
            heard: std::array::from_fn(|_| None),
            sounding: std::array::from_fn(|_| None),
            placed: [None; SLOTS],
            lacking: std::array::from_fn(|_| None),
            carriers: 0,
            listeners: 0,
            cues: Vec::with_capacity(8),
            loaded: Vec::new(),
            #[cfg(test)]
            shot_carriers: 0,
        }
    }
}

impl Boombox {
    /// The SJK players within the own track's range while it plays (bits by slot).
    pub(crate) fn listeners(&self) -> u64 {
        self.listeners
    }

    /// Whether the player carries the stereo.
    pub(crate) fn on(&self) -> bool {
        self.on
    }

    /// Who carries a stereo drawn this frame (bits by slot).
    pub(crate) fn carriers(&self) -> u64 {
        #[cfg(test)]
        if self.shot_carriers != 0 {
            return self.shot_carriers;
        }
        self.carriers
    }

    /// The changes to what plays since the last call.
    #[cfg(test)]
    fn take_cues(&mut self) -> std::vec::Drain<'_, Cue> {
        self.cues.drain(..)
    }

    /// Where each other player's music plays from this frame, by slot.
    pub(crate) fn placed(&self) -> impl Iterator<Item = (u8, [f32; 3])> + '_ {
        self.placed
            .iter()
            .enumerate()
            .filter_map(|(slot, at)| Some((slot as u8, (*at)?)))
    }

    /// The tracks, read from `vfs` the first time.
    fn tracks(&mut self, vfs: Option<&sjk_vfs::VirtualFileSystem>) -> &[Track] {
        if self.tracks.is_none()
            && let Some(vfs) = vfs
        {
            self.tracks = Some(load(vfs));
        }
        self.tracks.as_deref().unwrap_or_default()
    }

    /// The id of the track after `id`, the first after the last.
    fn after(&self, id: &str) -> Option<String> {
        let tracks = self.tracks.as_deref()?;
        let at = tracks.iter().position(|track| track.id == id)?;
        Some(tracks[(at + 1) % tracks.len()].id.clone())
    }

    /// The installed track that plays `song`: the same id and size.
    fn installed(&self, song: &Song) -> Option<&Track> {
        installed(&self.tracks, song)
    }

    /// Forget everything of the server: no game, or another one. The stereo comes off
    /// (on the next server the player may not be verified); the chosen track is kept,
    /// the tracks are read again (another server may bring other pk3s).
    fn leave(&mut self) {
        self.stop_own();
        let chosen = self.chosen.take();
        for slot in 0..SLOTS {
            if self.sounding[slot].is_some() {
                self.cues.push(Cue::Stop { slot: slot as u8 });
            }
        }
        let cues = std::mem::take(&mut self.cues);
        let loaded = std::mem::take(&mut self.loaded);
        #[cfg(test)]
        let shot_carriers = self.shot_carriers;
        *self = Self {
            chosen,
            cues,
            loaded,
            #[cfg(test)]
            shot_carriers,
            ..Self::default()
        };
    }

    /// Set what plays for `slot`: `want` (the file, the start, where and how far), or
    /// nothing; a change becomes a cue. `now` is the game server's time.
    #[cfg(test)]
    fn sound(&mut self, slot: u8, want: Option<(&str, &Song, Option<[f32; 3]>)>, now: i32) {
        sound(
            &mut self.sounding[usize::from(slot)],
            &mut self.cues,
            slot,
            want,
            now,
        );
    }

    /// Take in the relayed events (`heard`, by sender slot) at `now`.
    fn hear(&mut self, heard: impl IntoIterator<Item = (u8, Event)>, now: Instant) {
        for (slot, event) in heard {
            let Some(entry) = self.heard.get_mut(usize::from(slot)) else {
                continue;
            };
            *entry = match event {
                Event::Play(song) => Some(Heard { song, at: now }),
                Event::Stop => None,
            };
        }
        for entry in &mut self.heard {
            if entry
                .as_ref()
                .is_some_and(|heard| now.saturating_duration_since(heard.at) > LOST)
            {
                *entry = None;
            }
        }
    }

    /// The players to tell of the own track now: `near` (bits) are the SJK players
    /// within reach. One who left reach is told again on coming back, and everyone again
    /// after [`RETELL`]. At most one a [`SEND_GAP`].
    fn next_to_tell(&mut self, near: u64, now: Instant) -> Option<u8> {
        self.told &= near;
        for (slot, at) in self.told_at.iter_mut().enumerate() {
            if self.told & 1 << slot == 0 {
                *at = None;
            } else if at.is_some_and(|at| now.saturating_duration_since(at) >= RETELL) {
                self.told &= !(1 << slot);
                *at = None;
            }
        }
        if self.send_at.is_some_and(|at| now < at) {
            return None;
        }
        let untold = near & !self.told;
        if untold == 0 {
            return None;
        }
        let slot = untold.trailing_zeros() as u8;
        self.told |= 1 << slot;
        self.told_at[usize::from(slot)] = Some(now);
        self.send_at = Some(now + SEND_GAP);
        Some(slot)
    }

    /// The own track ends: everyone told hears it.
    fn stop_own(&mut self) {
        if self.playing.take().is_some() {
            for slot in 0..SLOTS {
                if self.told & 1 << slot != 0 {
                    relay(&Event::Stop, slot as u8);
                }
            }
        }
        self.told = 0;
        self.told_at = [None; SLOTS];
    }
}

/// Set what plays for `slot` (`sounding`): `want` (the file, the start, where), or
/// nothing; a change becomes a cue in `cues`. `now` is the game server's time. Nothing is
/// allocated while what plays stays the same.
fn sound(
    sounding: &mut Option<Sounding>,
    cues: &mut Vec<Cue>,
    slot: u8,
    want: Option<(&str, &Song, Option<[f32; 3]>)>,
    now: i32,
) {
    let same = match (&*sounding, want) {
        (Some(playing), Some((path, song, _))) => {
            playing.path == path && playing.moment == song.moment
        }
        (None, None) => true,
        _ => false,
    };
    if same {
        return;
    }
    match want {
        Some((path, song, origin)) => {
            cues.push(Cue::Start {
                slot,
                path: path.to_owned(),
                origin,
                offset: Duration::from_millis(u64::try_from(now - song.moment).unwrap_or(0)),
                range: f32::from(song.range),
            });
            *sounding = Some(Sounding {
                path: path.to_owned(),
                moment: song.moment,
            });
        }
        None => {
            cues.push(Cue::Stop { slot });
            *sounding = None;
        }
    }
}

/// The installed track of `tracks` that plays `song`: the same id and size.
fn installed<'a>(tracks: &'a Option<Vec<Track>>, song: &Song) -> Option<&'a Track> {
    tracks
        .as_deref()?
        .iter()
        .find(|track| track.id == song.id && track.bytes == song.bytes)
}

/// Send `event` to the player in `slot`.
fn relay(event: &Event, slot: u8) {
    crate::player_identity::relay_toy_event(TOY, event.text(), slot, false);
}

/// The stereo on a body posed with `matrices`, placed at `transform`: from the bolt
/// [`BOLT`] (it follows the body's sway and crouch) to [`SEAT`] in the body's frame,
/// upright, facing where the body faces.
pub(crate) fn instance(
    preview: &crate::PlayerPreview,
    matrices: &[[[f32; 4]; 3]],
    transform: sjk_runtime::Transform,
) -> Option<ActorInstance> {
    let back = crate::saber::attachment_from_matrices(preview, matrices, BOLT)?;
    let rotation = crate::weapon_view::actor_world_rotation(transform.rotation);
    let (origin, _) =
        crate::saber::world_attachment(Vec3::from_array(transform.translation), rotation, back);
    let forward = Quat::from_array(transform.rotation) * Vec3::X;
    let yaw = forward.y.atan2(forward.x);
    let facing = Quat::from_rotation_z(yaw);
    let [ahead, left, up] = SEAT;
    let seat = origin + facing * Vec3::new(ahead, left, 0.0) + Vec3::Z * up;
    Some(ActorInstance::new(
        seat.to_array(),
        facing.to_array(),
        [SCALE; 3],
    ))
}

/// Whether the game draws a player entity: alive and not hidden.
fn drawn(entity: &sjk_protocol::EntityState) -> bool {
    entity.entity_type() == ET_PLAYER && entity.e_flags() & (EF_DEAD | EF_NODRAW) == 0
}

impl GpuState {
    /// Say `text` on the centre of the screen.
    fn boombox_centre(&mut self, text: &str) {
        self.chat.receive(
            sjk_client::ServerEventKind::CenterPrint,
            text.to_owned(),
            None,
            Instant::now(),
        );
    }

    /// Whether the player may carry the stereo here: anyone in a local game, a player
    /// the hub verified on a server. Says why not.
    fn boombox_allowed(&mut self) -> bool {
        let remote = self
            .live_session
            .as_ref()
            .is_some_and(|session| !session.is_local());
        if !remote || crate::player_identity::verified() {
            return true;
        }
        self.boombox_centre("The boombox is for verified SJK players");
        false
    }

    /// `toy_boombox`: put the stereo on and play the chosen track, or take it off.
    pub(crate) fn toggle_boombox(&mut self) {
        if self.boombox.on {
            self.boombox.on = false;
            self.boombox.stop_own();
            self.boombox_centre(&crate::toy_switch_words::switched("Boombox", false));
            return;
        }
        if !self.boombox_allowed() {
            return;
        }
        self.boombox.on = true;
        let chosen = self.boombox.chosen.clone();
        let first = self
            .boombox
            .tracks(self.vfs.as_deref())
            .first()
            .map(|track| track.id.clone());
        match chosen.or(first) {
            Some(id) => self.boombox_play(&id),
            None => self.boombox_centre("No boombox tracks: add sound/boombox/<name>.mp3 in a pk3"),
        }
    }

    /// Play the track `id` from its start, putting the stereo on.
    fn boombox_play(&mut self, id: &str) {
        let Some(track) = self
            .boombox
            .tracks(self.vfs.as_deref())
            .iter()
            .find(|track| track.id == id)
            .cloned()
        else {
            return;
        };
        let moment = self
            .live_session
            .as_ref()
            .map_or(0, |session| session.latest_snapshot().server_time);
        let range = clamp_range(
            self.console
                .as_ref()
                .and_then(|console| console.integer_cvar(RANGE_CVAR))
                .unwrap_or(DEFAULT_RANGE),
        );
        self.boombox.stop_own();
        self.boombox.on = true;
        self.boombox.chosen = Some(track.id.clone());
        self.boombox.playing = Some(Song {
            moment,
            bytes: track.bytes,
            range,
            id: track.id.clone(),
        });
        self.boombox_centre(&format!("^3\u{266a}^7 {}", track.title()));
    }

    /// `boombox [id|next|prev|play|stop]`: list the tracks, or choose and play one.
    pub(crate) fn boombox_command(&mut self, args: &[String]) -> Result<Vec<String>, String> {
        let word = args.first().map(|word| word.to_ascii_lowercase());
        let tracks = self.boombox.tracks(self.vfs.as_deref()).to_vec();
        let chosen = self.boombox.chosen.clone();
        let Some(word) = word else {
            if tracks.is_empty() {
                return Ok(vec![format!(
                    "No boombox tracks. Put <name>.mp3 (or .wav) in {FOLDER}/ of a pk3: <name> is 1 to {MAX_ID} of a to z, 0 to 9 and _"
                )]);
            }
            let playing = self.boombox.playing.as_ref().map(|song| song.id.as_str());
            let mut lines = vec![format!("{} boombox tracks:", tracks.len())];
            lines.extend(tracks.iter().map(|track| {
                let mark = if Some(track.id.as_str()) == playing {
                    " (playing)"
                } else {
                    ""
                };
                format!("  {}{mark}", track.id)
            }));
            return Ok(lines);
        };
        if word == STOP {
            self.boombox.stop_own();
            return Ok(Vec::new());
        }
        let at = chosen
            .as_ref()
            .and_then(|id| tracks.iter().position(|track| &track.id == id));
        let id = match word.as_str() {
            "next" | "prev" if tracks.is_empty() => {
                return Err("No boombox tracks".to_owned());
            }
            "next" => tracks[at.map_or(0, |at| (at + 1) % tracks.len())]
                .id
                .clone(),
            "prev" => tracks[at.map_or(tracks.len() - 1, |at| {
                (at + tracks.len() - 1) % tracks.len()
            })]
            .id
            .clone(),
            "play" => match chosen.or_else(|| tracks.first().map(|track| track.id.clone())) {
                Some(id) => id,
                None => return Err("No boombox tracks".to_owned()),
            },
            id if tracks.iter().any(|track| track.id == id) => id.to_owned(),
            id => return Err(format!("No boombox track {id} (boombox lists them)")),
        };
        if !self.boombox.on && !self.boombox_allowed() {
            return Ok(Vec::new());
        }
        self.boombox_play(&id);
        Ok(Vec::new())
    }

    /// The frame's toy logic: the relay's events, what plays here and who to tell of the
    /// own track.
    pub(crate) fn update_boombox(&mut self, relay_events: &[sjk_identity::ToyEvent]) {
        let Some(session) = self.live_session.as_ref() else {
            self.boombox.leave();
            self.boombox.carriers = 0;
            self.boombox.placed = [None; SLOTS];
            return;
        };
        let server = session.server();
        if self.boombox.server != Some(server) {
            self.boombox.leave();
            self.boombox.server = Some(server);
        }
        let remote = !session.is_local();
        let snapshot = session.latest_snapshot();
        let game_state = session.game_state();
        let own = u8::try_from(game_state.client_num).ok();
        let server_time = snapshot.server_time;
        let now = Instant::now();
        let verified = if remote {
            crate::player_identity::verified_slots(game_state)
        } else {
            0
        };
        // The relay's events: a sender counts only when the game shows the name they
        // claimed, the hub verified them, and never one blocked on this PC.
        let heard: Vec<(u8, Event)> = relay_events
            .iter()
            .filter(|event| {
                event.toy == TOY
                    && Some(event.slot) != own
                    && usize::from(event.slot) < SLOTS
                    && verified & 1 << event.slot != 0
                    && !self.muted_players.contains(u16::from(event.slot))
                    && crate::player_identity::shown_name(game_state, usize::from(event.slot))
                        .is_some_and(|shown| sjk_identity::names_match(&event.claimed_name, &shown))
            })
            .filter_map(|event| Some((event.slot, Event::parse(&event.event)?)))
            .collect();
        self.boombox.hear(heard, now);
        // A verified player no longer, one who left or one blocked here: their track stops.
        let sjk = self.looks.sjk_slots();
        for (slot, heard) in self.boombox.heard.iter_mut().enumerate() {
            if (verified & sjk) & 1 << slot == 0 || self.muted_players.contains(slot as u16) {
                *heard = None;
            }
        }
        // Where the players stand: the drawn ones of the snapshot, and the followed one.
        let presentation = i64::from(server_time);
        let world = &self.live_world;
        let mut origins = [None::<[f32; 3]>; SLOTS];
        for entity in &snapshot.entities {
            let slot = usize::from(entity.number());
            if slot < SLOTS && drawn(entity) {
                origins[slot] = world
                    .entity(sjk_runtime::EntityId::new(slot as u64 + 1))
                    .map(|entity| entity.sample(presentation).translation);
            }
        }
        let followed = snapshot.player.movement_flags() & PMF_FOLLOW != 0;
        if followed
            && let slot = usize::from(snapshot.player.client_num())
            && slot < SLOTS
            && snapshot.player.health() > 0
        {
            origins[slot] = Some(snapshot.player.origin());
        }
        let own_origin = self
            .local_prediction
            .predicted_state()
            .map_or_else(|| snapshot.player.origin(), |state| state.origin);
        let listener = self.camera_position;
        // Make sure the tracks are known once anyone plays one here.
        if self.boombox.heard.iter().any(Option::is_some) {
            let _ = self.boombox.tracks(self.vfs.as_deref());
        }
        // Other players' tracks: played while within range and installed.
        let mut lacking = Vec::new();
        let Boombox {
            tracks,
            heard,
            sounding,
            placed,
            lacking: said,
            cues,
            ..
        } = &mut self.boombox;
        *placed = [None; SLOTS];
        for (slot, origin) in origins.iter().copied().enumerate() {
            if Some(slot as u8) == own {
                continue;
            }
            let Some(song) = heard[slot].as_ref().map(|heard| &heard.song) else {
                said[slot] = None;
                sound(&mut sounding[slot], cues, slot as u8, None, server_time);
                continue;
            };
            let Some(track) = installed(tracks, song) else {
                if said[slot].as_deref() != Some(song.id.as_str()) {
                    said[slot] = Some(song.id.clone());
                    lacking.push((slot, song.id.clone()));
                }
                sound(&mut sounding[slot], cues, slot as u8, None, server_time);
                continue;
            };
            let reach = f32::from(song.range)
                + if sounding[slot].is_some() {
                    MARGIN
                } else {
                    0.0
                };
            let near = origin.filter(|at| Vec3::from_array(*at).distance(listener) <= reach);
            placed[slot] = near;
            let want = near.map(|at| (track.path.as_str(), song, Some(at)));
            sound(&mut sounding[slot], cues, slot as u8, want, server_time);
        }
        for (slot, id) in lacking {
            let name = crate::player_identity::shown_name(game_state, slot)
                .unwrap_or_else(|| "A player".to_owned());
            say_lacking(&mut self.chat, &name, &id);
        }
        // The player's own track, in both ears.
        if let Some(own) = own {
            let Boombox {
                on,
                tracks,
                playing,
                sounding,
                cues,
                ..
            } = &mut self.boombox;
            let want = playing
                .as_ref()
                .filter(|_| *on)
                .and_then(|song| Some((installed(tracks, song)?.path.as_str(), song, None)));
            sound(
                &mut sounding[usize::from(own)],
                cues,
                own,
                want,
                server_time,
            );
            if !self.boombox.on {
                self.boombox.stop_own();
            }
        }
        // Tell the SJK players within reach of the own track.
        self.boombox.listeners = 0;
        if remote && let (Some(own), Some(song)) = (own, self.boombox.playing.clone()) {
            self.boombox.listeners = (0..SLOTS)
                .filter(|&slot| slot != usize::from(own) && sjk & 1 << slot != 0)
                .filter(|&slot| {
                    origins[slot].is_some_and(|at| {
                        Vec3::from_array(at).distance(Vec3::from_array(own_origin))
                            <= f32::from(song.range)
                    })
                })
                .fold(0_u64, |bits, slot| bits | 1 << slot);
            let reach = f32::from(song.range) + MARGIN * 2.0;
            let near = (0..SLOTS)
                .filter(|&slot| slot != usize::from(own) && sjk & 1 << slot != 0)
                .filter(|&slot| {
                    origins[slot].is_some_and(|at| {
                        Vec3::from_array(at).distance(Vec3::from_array(own_origin)) <= reach
                    })
                })
                .fold(0_u64, |bits, slot| bits | 1 << slot);
            if let Some(slot) = self.boombox.next_to_tell(near, now) {
                relay(&Event::Play(song), slot);
            }
        }
        // Who carries one drawn: the player, and the verified players whose look has it.
        let mut carriers = 0_u64;
        if let Some(own) = own
            && self.boombox.on
        {
            carriers |= 1 << own;
        }
        for slot in 0..SLOTS {
            if Some(slot as u8) != own
                && self.looks.toy(slot, TOY)
                && (!remote || verified & 1 << slot != 0)
            {
                carriers |= 1 << slot;
            }
        }
        self.boombox.carriers = carriers;
    }

    /// Play and stop the boombox tracks, move the others' music with them, and free the
    /// tracks nothing plays any more.
    pub(crate) fn play_boombox_sounds(&mut self, audio: &mut Option<crate::GameAudio>) {
        let Some(audio) = audio.as_mut() else {
            self.boombox.cues.clear();
            return;
        };
        let Some(vfs) = self.vfs.clone() else {
            return;
        };
        for cue in self.boombox.cues.drain(..) {
            match cue {
                Cue::Start {
                    slot,
                    path,
                    origin,
                    offset,
                    range,
                } => {
                    if !self.boombox.loaded.contains(&path) {
                        self.boombox.loaded.push(path.clone());
                    }
                    audio.play_boombox(&vfs, &path, slot, origin, offset, range);
                }
                Cue::Stop { slot } => audio.stop_boombox(slot),
            }
        }
        for (slot, at) in self.boombox.placed() {
            audio.move_boombox(slot, at);
        }
        // The own track over: the next one plays (the same again when it is the only one).
        let ended = self.boombox.playing.as_ref().and_then(|song| {
            let track = self.boombox.installed(song)?;
            let length = audio.sound_length(&track.path)?;
            let now = self.live_session.as_ref()?.latest_snapshot().server_time;
            let played = Duration::from_millis(u64::try_from(now - song.moment).ok()?);
            (played > length + END_PAUSE).then(|| self.boombox.after(&song.id))
        });
        if let Some(Some(next)) = ended {
            self.boombox_play(&next);
        }
        let Boombox {
            loaded, sounding, ..
        } = &mut self.boombox;
        loaded.retain(|path| {
            let used = sounding
                .iter()
                .flatten()
                .any(|playing| playing.path == *path);
            if !used {
                audio.forget_sound(path);
            }
            used
        });
    }
}

/// Say once in the console that `name` plays a track `id` this PC does not have.
fn say_lacking(chat: &mut crate::chat::ChatOverlay, name: &str, id: &str) {
    chat.receive(
        sjk_client::ServerEventKind::Print,
        format!("{name}^7's boombox plays {id}, which you do not have ({FOLDER}/{id}.mp3)\n"),
        None,
        Instant::now(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(moment: i32, id: &str) -> Song {
        Song {
            moment,
            bytes: 4_321_987,
            range: 400,
            id: id.to_owned(),
        }
    }

    #[test]
    fn the_bundled_files_mount_and_the_model_stands_on_its_bottom_facing_forward() {
        let mut vfs = sjk_vfs::VirtualFileSystem::new();
        mount(&mut vfs).unwrap();
        for (path, _) in FILES {
            assert!(vfs.read(path).unwrap().is_some(), "{path}");
        }
        let model = sjk_model::Md3::parse(&vfs.read(MODEL).unwrap().unwrap().bytes).unwrap();
        let surface = &model.surfaces[0];
        assert_eq!(surface.shaders, ["models/sjk/boombox"]);
        let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for vertex in &surface.frames[0] {
            let position = Vec3::from_array(vertex.position);
            low = low.min(position);
            high = high.max(position);
        }
        // Wider than deep, from its bottom up, the handle over the body.
        assert!(low.z.abs() < 1e-3 && high.z > 12.0, "{low} {high}");
        assert!(high.y - low.y > 3.0 * (high.x - low.x), "{low} {high}");
        // Every face clockwise seen from outside (stock's front side).
        let triangles = &surface.triangles;
        for quad in triangles.chunks(2) {
            let [a, b, c] =
                quad[0].map(|i| Vec3::from_array(surface.frames[0][i as usize].position));
            let normal = surface.frames[0][quad[0][0] as usize].normal;
            let facing = (b - a).cross(c - a);
            assert!(facing.dot(Vec3::from_array(normal)) < 0.0);
        }
        let icon = image::load_from_memory(FILES[3].1).unwrap();
        assert_eq!((icon.width(), icon.height()), (128, 128));
    }

    #[test]
    fn events_read_back_as_written_and_fit_the_relay() {
        let longest = Song {
            moment: i32::MAX,
            bytes: 9_999_999_999,
            range: 1_000,
            id: "a".repeat(MAX_ID),
        };
        let text = Event::Play(longest.clone()).text();
        assert!(text.len() <= 64, "{text}");
        assert!(
            text.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        );
        assert_eq!(Event::parse(&text), Some(Event::Play(longest)));
        let with_underscores = Event::Play(song(5, "eye_of_the_tiger"));
        assert_eq!(
            Event::parse(&with_underscores.text()),
            Some(with_underscores)
        );
        assert_eq!(Event::parse(STOP), Some(Event::Stop));
        // A range outside the bounds is brought within them; a bad id is refused.
        assert_eq!(
            Event::parse("play_1_2_99999_song"),
            Some(Event::Play(Song {
                moment: 1,
                bytes: 2,
                range: 1_000,
                id: "song".to_owned()
            }))
        );
        assert_eq!(Event::parse("play_1_2_400_Song"), None);
        assert_eq!(Event::parse("play_1_2_400_"), None);
        assert_eq!(Event::parse("play_x_2_400_song"), None);
    }

    #[test]
    fn track_ids_are_short_lowercase_words() {
        assert!(valid_id("eye_of_the_tiger"));
        assert!(valid_id("track01"));
        assert!(!valid_id(""));
        assert!(!valid_id("Track"));
        assert!(!valid_id("a-b"));
        assert!(!valid_id(&"a".repeat(MAX_ID + 1)));
        assert_eq!(title("eye_of_the_tiger"), "eye of the tiger");
    }

    #[test]
    fn tracks_are_listed_from_the_folder_by_id_mp3_first() {
        let mut vfs = sjk_vfs::VirtualFileSystem::new();
        vfs.mount_memory(
            "test",
            [
                ("sound/boombox/zulu.wav", vec![0_u8; 10]),
                ("sound/boombox/alpha.mp3", vec![0_u8; 20]),
                ("sound/boombox/alpha.wav", vec![0_u8; 30]),
                ("sound/boombox/Bad Name.mp3", vec![0_u8; 5]),
                ("sound/boombox/deeper/inside.mp3", vec![0_u8; 5]),
                ("sound/other/song.mp3", vec![0_u8; 5]),
            ],
        )
        .unwrap();
        let tracks = load(&vfs);
        let listed: Vec<_> = tracks
            .iter()
            .map(|track| (track.id.as_str(), track.path.as_str(), track.bytes))
            .collect();
        assert_eq!(
            listed,
            [
                ("alpha", "sound/boombox/alpha.mp3", 20),
                ("zulu", "sound/boombox/zulu.wav", 10),
            ]
        );
    }

    #[test]
    fn a_heard_track_holds_until_stopped_or_lost() {
        let start = Instant::now();
        let mut toy = Boombox::default();
        toy.hear([(3, Event::Play(song(100, "a")))], start);
        assert!(toy.heard[3].is_some());
        toy.hear([], start + LOST);
        assert!(toy.heard[3].is_some(), "told again in time");
        toy.hear([], start + LOST + Duration::from_secs(1));
        assert!(toy.heard[3].is_none(), "lost");
        toy.hear([(3, Event::Play(song(100, "a")))], start);
        toy.hear([(3, Event::Stop)], start);
        assert!(toy.heard[3].is_none(), "stopped");
    }

    #[test]
    fn what_plays_changes_only_when_the_track_or_its_start_does() {
        let mut toy = Boombox::default();
        let first = song(1_000, "a");
        toy.sound(
            2,
            Some(("sound/boombox/a.mp3", &first, Some([1.0; 3]))),
            4_500,
        );
        let cues: Vec<_> = toy.take_cues().collect();
        assert_eq!(
            cues,
            [Cue::Start {
                slot: 2,
                path: "sound/boombox/a.mp3".to_owned(),
                origin: Some([1.0; 3]),
                offset: Duration::from_millis(3_500),
                range: 400.0,
            }]
        );
        // The same, moved: nothing new to play.
        toy.sound(
            2,
            Some(("sound/boombox/a.mp3", &first, Some([9.0; 3]))),
            5_000,
        );
        assert_eq!(toy.take_cues().count(), 0);
        // Started again (a new moment): played again, from the new start.
        let again = song(6_000, "a");
        toy.sound(2, Some(("sound/boombox/a.mp3", &again, None)), 6_000);
        assert!(matches!(
            toy.take_cues().next(),
            Some(Cue::Start { offset, .. }) if offset.is_zero()
        ));
        toy.sound(2, None, 7_000);
        assert_eq!(toy.take_cues().collect::<Vec<_>>(), [Cue::Stop { slot: 2 }]);
        toy.sound(2, None, 7_000);
        assert_eq!(toy.take_cues().count(), 0, "already silent");
    }

    #[test]
    fn players_within_reach_are_told_one_at_a_time_and_again_later() {
        let start = Instant::now();
        let mut toy = Boombox::default();
        let near = 0b1010;
        assert_eq!(toy.next_to_tell(near, start), Some(1));
        assert_eq!(toy.next_to_tell(near, start), None, "one a gap");
        assert_eq!(toy.next_to_tell(near, start + SEND_GAP), Some(3));
        assert_eq!(
            toy.next_to_tell(near, start + SEND_GAP * 2),
            None,
            "all told"
        );
        // Slot 1 walks away and comes back: told again.
        assert_eq!(toy.next_to_tell(0b1000, start + SEND_GAP * 3), None);
        assert_eq!(toy.next_to_tell(near, start + SEND_GAP * 4), Some(1));
        // Everyone again after the retell.
        let later = start + SEND_GAP * 4 + RETELL;
        assert_eq!(toy.next_to_tell(near, later), Some(1));
        assert_eq!(toy.next_to_tell(near, later + SEND_GAP), Some(3));
    }
}
