//! Listener-local announcer and chat sounds of BaseJKA (`CG_CheckLocalSounds`,
//! `CG_DrawWarmup`, `CG_MapRestart`, `CG_ServerCommand`, `CG_PlayBufferedSounds`).
//!
//! These sounds are never carried by entity events: they are derived on the
//! client from config strings (limits, scores, warmup, level start), from the
//! local player state and from reliable server commands. Every rule below cites
//! `codemp/cgame`:
//!
//! - `cg_playerstate.c:528-531`: `CG_CheckLocalSounds` runs only while the
//!   snapshot is not in `PM_INTERMISSION` and the player is not a spectator;
//!   `cg_playerstate.c:326-328`: it returns whenever the team just changed.
//!   Hit sounds, reward sounds (`JK2AWARDS` undefined) and rank changes are
//!   all compiled out or commented out in the multiplayer game.
//! - `cg_playerstate.c:452-470`: timelimit warnings play directly on
//!   `CHAN_ANNOUNCER` and stamp `cgAnnouncerTime = cg.time + 3000`; the
//!   "sudden death" branch two seconds past the limit only sets the bits.
//! - `cg_playerstate.c:473-496`: fraglimit warnings enqueue into the 20-slot
//!   `cg.soundBuffer` ring (`cg_view.c:1839-1866`), drained one sound per
//!   750 ms.
//! - `cg_draw.c:6785-6890`: warmup countdown speaks 3/2/1 whenever the
//!   remaining whole second changes, never in Siege; a negative warmup means
//!   "waiting for players"; `CG_ParseWarmup` (`cg_servercmds.c:226-236`) resets
//!   the count on every `CS_WARMUP` change.
//! - `cg_draw.c:8168-8226`: Siege speaks its own 3/2/1 while `CS_SIEGE_STATE` counts
//!   the round in (`2|time`, `SIEGE_ROUND_BEGIN_TIME`), once per remaining whole
//!   second from three; `cgSiegeRoundCountTime` is never reset, so a count that ended
//!   on one speaks three again at the next round.
//! - `cg_servercmds.c:1070-1090`: `map_restart` clears both warning masks and
//!   plays the "fight" line when there is no warmup outside Siege/Power Duel.
//! - `cg_servercmds.c:1509-1572`: `chat`/`lchat` beep unless `cg_teamChatsOnly`
//!   (default 0) or `cg_chatBeep 0`; `tchat`/`ltchat` always beep.
//!
//! Deviation (measured, see `KNOWN_ISSUES.md`): the countdown, the announcer
//! cooldown and the buffered ring advance at snapshot cadence on
//! `snapshot.server_time`, not per rendered frame on `cg.time`, so a sound may
//! start up to one snapshot interval later than in the reference client.

use crate::sound_events::{CHAN_ANNOUNCER, LegacySoundEvent};
use sjk_protocol::{GameState, Snapshot};

#[path = "chat_sound_filter.rs"]
mod chat_filter;
#[path = "team_announcer.rs"]
mod team_announcer;

const CS_SERVERINFO: usize = 0;
const CS_WARMUP: usize = 5;
const CS_SCORES1: usize = 6;
const CS_SCORES2: usize = 7;
const CS_LEVEL_START_TIME: usize = 21;
const CS_SIEGE_STATE: usize = sjk_game_jka::siege_class::CS_SIEGE_STATE;
const GT_DUEL: i32 = 3;
const GT_POWERDUEL: i32 = 4;
const GT_TEAM: i32 = 6;
const GT_SIEGE: i32 = 7;
const GT_CTF: i32 = 8;
const PM_INTERMISSION: u8 = 7;
const TEAM_SPECTATOR: u8 = 3;
/// `CHAN_LOCAL_SOUND`: chat beeps are unspatialized listener sounds.
pub(crate) const CHAN_LOCAL_SOUND: u32 = 8;
const MAX_SOUNDBUFFER: usize = 20;
const ANNOUNCER_COOLDOWN_MILLIS: i32 = 3_000;
const BUFFERED_SPACING_MILLIS: i32 = 750;
const MAX_LOCAL_SOUNDS_PER_SNAPSHOT: usize = 8;
/// Protocol 26's player slots.
const MAX_CLIENTS: usize = 32;
/// `cg_chatBeep` and `cg_teamChatsOnly` at their BaseJKA defaults.
const CG_CHAT_BEEP: bool = true;
const CG_TEAM_CHATS_ONLY: bool = false;

/// One listener-local playback requested by a snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LegacyLocalSound {
    pub(crate) event: LegacySoundEvent,
    pub(crate) sound: Option<u16>,
    pub(crate) channel: u32,
    /// The sender of the chat message a beep is for, when the server names them
    /// ([`crate::LegacySoundDecision::cause`]).
    pub(crate) cause: Option<u16>,
}

/// `cgs.media` handles this module needs, interned into the adapter's bank.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct LocalSoundMedia {
    /// TaystJK cg_main.c:863 team notification.
    pub(crate) team_chat: Option<u16>,
    /// TaystJK cg_main.c:864-866 private notification, with base-asset fallback.
    pub(crate) private_chat: Option<u16>,
    pub(crate) talk: Option<u16>,
    pub(crate) one_minute: Option<u16>,
    pub(crate) five_minute: Option<u16>,
    pub(crate) one_frag: Option<u16>,
    pub(crate) two_frag: Option<u16>,
    pub(crate) three_frag: Option<u16>,
    /// `count1Sound`, `count2Sound`, `count3Sound` in remaining-second order.
    pub(crate) count: [Option<u16>; 3],
    pub(crate) count_fight: Option<u16>,
    team: [Option<u16>; 13],
}

impl LocalSoundMedia {
    /// `cg_main.c:617-625` media paths registered through `intern`.
    pub(crate) fn register(mut intern: impl FnMut(&str) -> u16) -> Self {
        let protocol = |name: &str| format!("sound/chars/protocol/misc/{name}");
        Self {
            talk: Some(intern("sound/player/talk.wav")),
            one_minute: Some(intern(&protocol("40MOM004"))),
            five_minute: Some(intern(&protocol("40MOM005"))),
            one_frag: Some(intern(&protocol("40MOM001"))),
            two_frag: Some(intern(&protocol("40MOM002"))),
            three_frag: Some(intern(&protocol("40MOM003"))),
            count: ["40MOM037", "40MOM036", "40MOM035"].map(|name| Some(intern(&protocol(name)))),
            count_fight: Some(intern(&protocol("40MOM038"))),
            team: team_announcer::register(&mut intern),
            team_chat: Some(intern("sound/movers/switches/button_11.mp3")),
            private_chat: Some(intern("sound/movers/switches/button_15.mp3")),
        }
    }
}

/// The `cg`/`cgs` state behind the local announcer, updated per snapshot.
pub(crate) struct LegacyLocalSounds {
    /// TaystJK cg_chatSounds mode; final muting remains in the output consumer.
    pub(crate) chat_mode: i64,
    pub(crate) pm_only: bool,
    /// Quiet mode (the viewer's `chat/quiet.rs`): chat beeps follow its rule.
    pub(crate) quiet: bool,
    /// The player's own slot, the gamestate's client number (not a followed player's).
    own_slot: Option<u16>,
    /// The private duel partner whose chat still beeps in quiet mode.
    partner: crate::quiet_chat::DuelPartner,
    /// Fixed-capacity filtering shared with chat presentation policy.
    pub(crate) chat_filter: chat_filter::Filter,
    media: LocalSoundMedia,
    /// `cgAnnouncerTime`: no announcer line starts before this server time.
    announcer_time: i32,
    /// `cg.soundTime`: next server time the buffered ring may play.
    sound_time: i32,
    buffer: [Option<(LegacySoundEvent, u16)>; MAX_SOUNDBUFFER],
    buffer_in: usize,
    buffer_out: usize,
    timelimit_warnings: u8,
    fraglimit_warnings: u8,
    /// `cg.warmup`: server time the match starts, negative while waiting.
    warmup: i32,
    warmup_count: i32,
    warmup_hash: u64,
    /// `cgSiegeRoundState` and `cgSiegeRoundTime` (`CG_ParseSiegeState`); a state
    /// with no time takes the time of the snapshot that sees it (`cg.time`).
    siege_round_state: i32,
    siege_round_time: Option<i32>,
    siege_state_hash: u64,
    /// `cgSiegeRoundCountTime`: the round count last spoken.
    siege_round_count: i32,
    serverinfo_hash: u64,
    timelimit: i32,
    fraglimit: i32,
    gametype: i32,
    scores: [i32; 2],
    level_start_time: i32,
    previous_team: Option<u8>,
    last_command_sequence: i32,
    pending: Vec<LegacyLocalSound>,
    /// The players' names, to tell who sent a chat line the server does not attribute
    /// (JA+ and JoF send none), as the chat feed does.
    roster: crate::chat::ChatRoster,
}

impl LegacyLocalSounds {
    /// Queue EV_GLOBAL_TEAM_SOUND through cgame's bounded announcer ring.
    pub(crate) fn team_event(&mut self, parameter: u8) {
        if let Some(index) = team_announcer::index(parameter, self.gametype) {
            self.enqueue(LegacySoundEvent::GlobalTeam, self.media.team[index]);
        }
    }
    pub(crate) fn new(game_state: &GameState, media: LocalSoundMedia) -> Self {
        let mut local = Self {
            media,
            announcer_time: 0,
            sound_time: 0,
            buffer: [None; MAX_SOUNDBUFFER],
            buffer_in: 0,
            buffer_out: 0,
            timelimit_warnings: 0,
            fraglimit_warnings: 0,
            warmup: 0,
            warmup_count: -1,
            warmup_hash: 0,
            siege_round_state: 0,
            siege_round_time: None,
            siege_state_hash: 0,
            siege_round_count: 0,
            serverinfo_hash: 0,
            timelimit: 0,
            fraglimit: 0,
            gametype: 0,
            scores: [0; 2],
            level_start_time: 0,
            previous_team: None,
            last_command_sequence: game_state.server_command_sequence,
            chat_mode: 1,
            pm_only: false,
            quiet: false,
            own_slot: u16::try_from(game_state.client_num).ok(),
            partner: crate::quiet_chat::DuelPartner::default(),
            chat_filter: chat_filter::Filter::default(),
            pending: Vec::with_capacity(MAX_LOCAL_SOUNDS_PER_SNAPSHOT),
            roster: crate::chat::ChatRoster::default(),
        };
        local.observe_config(game_state);
        local.observe_players(game_state);
        local
    }

    /// `CG_SetConfigValues` / `CG_ConfigStringModified`: mirror the limits,
    /// scores, warmup and intermission config strings. Allocation-free; the
    /// serverinfo and warmup strings are only re-parsed when their bytes change.
    pub(crate) fn observe_config(&mut self, game_state: &GameState) {
        let serverinfo = game_state.config_string(CS_SERVERINFO).unwrap_or_default();
        let hash = fnv1a(serverinfo);
        if hash != self.serverinfo_hash {
            self.serverinfo_hash = hash;
            self.timelimit = info_int(serverinfo, "timelimit");
            self.fraglimit = info_int(serverinfo, "fraglimit");
            self.gametype = info_int(serverinfo, "g_gametype");
        }
        let warmup = game_state.config_string(CS_WARMUP).unwrap_or_default();
        let hash = fnv1a(warmup);
        if hash != self.warmup_hash {
            self.warmup_hash = hash;
            self.warmup = atoi(warmup);
            self.warmup_count = -1;
        }
        let siege_state = game_state.config_string(CS_SIEGE_STATE).unwrap_or_default();
        let hash = fnv1a(siege_state);
        if hash != self.siege_state_hash {
            self.siege_state_hash = hash;
            // An empty string leaves the state as it was, as in the reference.
            if let Some((state, time)) = crate::match_info::parse_siege_state(siege_state) {
                self.siege_round_state = state;
                self.siege_round_time = time;
            }
        }
        self.scores = [CS_SCORES1, CS_SCORES2].map(|index| config_int(game_state, index));
        self.level_start_time = config_int(game_state, CS_LEVEL_START_TIME);
    }

    /// A player's `CS_PLAYERS` string changed: read the names again.
    pub(crate) fn observe_players(&mut self, game_state: &GameState) {
        self.roster.update(game_state);
        self.own_slot = u16::try_from(game_state.client_num).ok();
    }

    /// Run one snapshot through the reference frame order: new reliable
    /// commands, `CG_CheckLocalSounds`, the buffered ring, then the warmup
    /// countdown. The requested sounds are readable via [`Self::pending`].
    pub(crate) fn observe_snapshot(&mut self, snapshot: &Snapshot) {
        self.pending.clear();
        let time = snapshot.server_time;
        self.partner.observe(
            &self.roster,
            self.own_slot,
            &snapshot.player,
            i64::from(time),
        );
        self.execute_server_commands(snapshot);
        let team = snapshot.player.team();
        let team_changed = self.previous_team.is_some_and(|previous| previous != team);
        self.previous_team = Some(team);
        let in_play = snapshot.player.movement_type() != PM_INTERMISSION
            && team != TEAM_SPECTATOR
            && !team_changed;
        if in_play {
            self.check_limit_warnings(time);
        }
        self.play_buffered(time);
        self.draw_warmup(time);
        // `CG_Draw2D` returns before its siege count during intermission.
        if snapshot.player.movement_type() != PM_INTERMISSION {
            self.draw_siege_round_count(time);
        }
    }

    /// Sounds requested by the latest [`Self::observe_snapshot`].
    pub(crate) fn pending(&self) -> &[LegacyLocalSound] {
        &self.pending
    }

    /// `CG_ServerCommand` for the commands that make a local sound; each
    /// reliable command is handled once even when a snapshot repeats it.
    fn execute_server_commands(&mut self, snapshot: &Snapshot) {
        for command in &snapshot.server_commands {
            if command.sequence <= self.last_command_sequence {
                continue;
            }
            self.last_command_sequence = command.sequence;
            // Commands such as `map_restart\n` carry a trailing newline.
            let verb = command
                .command
                .split(|byte| matches!(byte, b' ' | b'\n' | b'\r'))
                .next()
                .unwrap_or_default();
            // Who sent a chat line and whether it is a genuine whisper, for PM-only
            // and quiet mode (`quiet_chat.rs`), worked out only while one is on.
            let facts = ((self.pm_only || self.quiet)
                && matches!(verb, b"chat" | b"lchat" | b"tchat" | b"ltchat"))
            .then(|| self.chat_facts(&command.command))
            .flatten();
            if self.pm_only && !facts.is_some_and(|(_, private)| private) {
                continue;
            }
            if self.quiet
                && facts.is_some_and(|(sender, private)| {
                    !crate::quiet_chat::quiet_passes(
                        private,
                        sender,
                        self.own_slot,
                        self.partner.target(),
                    )
                })
            {
                continue;
            }
            match verb {
                b"chat" | b"tchat"
                    if self.chat_filter.suppress(&command.command, verb == b"chat") =>
                {
                    continue;
                }
                // The server's copy of an SJK chat message: the SJK chat's own sound
                // announces the message (`docs/hub-chat.md`, "Said on the game server too").
                b"chat" if crate::chat::sjk_copy_command(&command.command) => continue,
                b"chat" | b"lchat" if !CG_TEAM_CHATS_ONLY && CG_CHAT_BEEP => {
                    // A tell is told apart at every mode, so the viewer can switch and
                    // voice it on its own; its default sound is EJK's private one only
                    // under cg_chatSounds 2 (JoF EJK `cg_servercmds.c:1814`).
                    let private = crate::chat::private_chat_command(&command.command);
                    if self.push(
                        if private {
                            LegacySoundEvent::PrivateChatBeep
                        } else {
                            LegacySoundEvent::ChatBeep
                        },
                        if private && self.chat_mode == 2 {
                            self.media.private_chat
                        } else {
                            self.media.talk
                        },
                        CHAN_LOCAL_SOUND,
                    ) {
                        self.name_sender(&command.command);
                    }
                }
                b"tchat" | b"ltchat" => {
                    if self.push(
                        LegacySoundEvent::TeamChatBeep,
                        if self.chat_mode == 2 && verb == b"tchat" {
                            self.media.team_chat
                        } else {
                            self.media.talk
                        },
                        CHAN_LOCAL_SOUND,
                    ) {
                        self.name_sender(&command.command);
                    }
                }
                b"map_restart" => self.map_restart(),
                _ => {}
            }
        }
    }

    /// `CG_MapRestart` (`cg_servercmds.c:1070-1090`).
    fn map_restart(&mut self) {
        self.fraglimit_warnings = 0;
        self.timelimit_warnings = 0;
        if self.warmup == 0 && self.gametype != GT_SIEGE && self.gametype != GT_POWERDUEL {
            self.push(
                LegacySoundEvent::RestartFight,
                self.media.count_fight,
                CHAN_ANNOUNCER,
            );
        }
    }

    /// The timelimit and fraglimit halves of `CG_CheckLocalSounds`.
    fn check_limit_warnings(&mut self, time: i32) {
        if self.timelimit > 0 && self.announcer_time < time {
            let msec = time.wrapping_sub(self.level_start_time);
            let warnings = self.timelimit_warnings;
            if warnings & 4 == 0 && msec > (self.timelimit * 60 + 2) * 1_000 {
                self.timelimit_warnings |= 1 | 2 | 4;
            } else if warnings & 2 == 0 && msec > (self.timelimit - 1) * 60_000 {
                self.timelimit_warnings |= 1 | 2;
                self.announce(
                    LegacySoundEvent::OneMinuteWarning,
                    self.media.one_minute,
                    time,
                );
            } else if self.timelimit > 5
                && warnings & 1 == 0
                && msec > (self.timelimit - 5) * 60_000
            {
                self.timelimit_warnings |= 1;
                self.announce(
                    LegacySoundEvent::FiveMinuteWarning,
                    self.media.five_minute,
                    time,
                );
            }
        }
        let frag_gametype = self.gametype < GT_CTF
            && self.gametype != GT_DUEL
            && self.gametype != GT_POWERDUEL
            && self.gametype != GT_SIEGE;
        if self.fraglimit > 0 && frag_gametype && self.announcer_time < time {
            let mut high_score = self.scores[0];
            if self.gametype == GT_TEAM {
                high_score = high_score.max(self.scores[1]);
            }
            let warnings = self.fraglimit_warnings;
            let remaining = self.fraglimit - high_score;
            if warnings & 4 == 0 && remaining == 1 {
                self.fraglimit_warnings |= 1 | 2 | 4;
                let sound = self.media.one_frag;
                self.buffer_announcement(LegacySoundEvent::OneFragWarning, sound, time);
            } else if self.fraglimit > 2 && warnings & 2 == 0 && remaining == 2 {
                self.fraglimit_warnings |= 1 | 2;
                let sound = self.media.two_frag;
                self.buffer_announcement(LegacySoundEvent::TwoFragWarning, sound, time);
            } else if self.fraglimit > 3 && warnings & 1 == 0 && remaining == 3 {
                self.fraglimit_warnings |= 1;
                let sound = self.media.three_frag;
                self.buffer_announcement(LegacySoundEvent::ThreeFragWarning, sound, time);
            }
        }
    }

    /// Direct `S_StartLocalSound(.., CHAN_ANNOUNCER)` plus the cooldown stamp.
    fn announce(&mut self, event: LegacySoundEvent, sound: Option<u16>, time: i32) {
        self.push(event, sound, CHAN_ANNOUNCER);
        self.announcer_time = time + ANNOUNCER_COOLDOWN_MILLIS;
    }

    /// `CG_AddBufferedSound` plus the cooldown stamp.
    fn buffer_announcement(&mut self, event: LegacySoundEvent, sound: Option<u16>, time: i32) {
        self.announcer_time = time + ANNOUNCER_COOLDOWN_MILLIS;
        self.enqueue(event, sound);
    }

    fn enqueue(&mut self, event: LegacySoundEvent, sound: Option<u16>) {
        let Some(sound) = sound else {
            return;
        };
        self.buffer[self.buffer_in] = Some((event, sound));
        self.buffer_in = (self.buffer_in + 1) % MAX_SOUNDBUFFER;
        if self.buffer_in == self.buffer_out {
            self.buffer_out = (self.buffer_out + 1) % MAX_SOUNDBUFFER;
        }
    }

    /// `CG_PlayBufferedSounds`: at most one ring entry every 750 ms.
    fn play_buffered(&mut self, time: i32) {
        if self.sound_time >= time || self.buffer_out == self.buffer_in {
            return;
        }
        let Some((event, sound)) = self.buffer[self.buffer_out].take() else {
            return;
        };
        self.buffer_out = (self.buffer_out + 1) % MAX_SOUNDBUFFER;
        self.sound_time = time + BUFFERED_SPACING_MILLIS;
        self.push(event, Some(sound), CHAN_ANNOUNCER);
    }

    /// The countdown half of `CG_DrawWarmup` (`cg_draw.c:6785-6890`).
    fn draw_warmup(&mut self, time: i32) {
        if self.warmup == 0 {
            return;
        }
        if self.warmup < 0 {
            self.warmup_count = 0;
            return;
        }
        let mut seconds = (self.warmup - time) / 1_000;
        if seconds < 0 {
            self.warmup = 0;
            seconds = 0;
        }
        if seconds == self.warmup_count {
            return;
        }
        self.warmup_count = seconds;
        if self.gametype == GT_SIEGE {
            return;
        }
        if let Some(sound) = self.media.count.get(seconds as usize) {
            self.push(LegacySoundEvent::WarmupCount, *sound, CHAN_ANNOUNCER);
        }
    }

    /// The countdown half of `CG_Draw2D`'s siege round state (`cg_draw.c:8168-8226`).
    fn draw_siege_round_count(&mut self, time: i32) {
        if self.siege_round_state != 2 {
            return;
        }
        let since = time.wrapping_sub(*self.siege_round_time.get_or_insert(time));
        let seconds = crate::match_info::siege_round_seconds(since);
        if seconds > 3 || seconds == self.siege_round_count {
            return;
        }
        self.siege_round_count = seconds;
        if let Some(sound) = self.media.count.get(seconds as usize - 1) {
            self.push(LegacySoundEvent::SiegeRoundCount, *sound, CHAN_ANNOUNCER);
        }
    }

    /// Queue a sound; false when the snapshot's room is full.
    fn push(&mut self, event: LegacySoundEvent, sound: Option<u16>, channel: u32) -> bool {
        if self.pending.len() >= MAX_LOCAL_SOUNDS_PER_SNAPSHOT {
            return false;
        }
        self.pending.push(LegacyLocalSound {
            event,
            sound,
            channel,
            cause: None,
        });
        true
    }

    /// The beep just pushed is for chat `command`: name its sender.
    /// The sender of chat `command` and whether it is a whisper: marked as a tell and
    /// not a public line of a player whose name starts with the marker, or in a tell's
    /// brackets after its sender's name, as the chat feed reads it.
    fn chat_facts(&self, command: &[u8]) -> Option<(crate::quiet_chat::LineSender, bool)> {
        let event = crate::chat::server_chat_event(&crate::tokenize_command(command))?;
        let display = crate::chat::chat_display_text(&crate::chat::chat_unescape(&event.text));
        let sender = crate::quiet_chat::line_sender(&display, event.sender, &self.roster);
        let private = (event.is_private
            && !crate::quiet_chat::forged_tell(&event.text, event.sender, &self.roster))
            || sender
                .target()
                .and_then(|target| self.roster.display_name(target))
                .is_some_and(|name| crate::chat::chat_body(&display, name).1);
        Some((sender, private))
    }

    fn name_sender(&mut self, command: &[u8]) {
        let sender = chat_sender(command, &self.roster);
        if let Some(beep) = self.pending.last_mut() {
            beep.cause = sender;
        }
    }
}

/// The sender of chat `command`: the slot `G_SayTo` appends, as the chat feed reads
/// it (`chat::server_chat_event`); when the server names none (JA+, JoF), the one
/// player whose name, as the game shows it, starts the line, then `: ` (or the team's
/// and a whisper's brackets), as the chat feed attributes such a line (the viewer's
/// `chat/marks.rs`, `line_sender`). `None` when nobody or two players fit, so a blocked
/// player's beep is silenced as their line is hidden (`docs/hub-chat.md`).
fn chat_sender(command: &[u8], roster: &crate::chat::ChatRoster) -> Option<u16> {
    let event = crate::chat::server_chat_event(&crate::tokenize_command(command))?;
    if event.sender.is_some() {
        return event.sender;
    }
    let display = crate::chat::chat_display_text(&crate::chat::chat_unescape(&event.text));
    let mut found = None;
    for slot in 0..MAX_CLIENTS as u16 {
        let Some(name) = roster
            .target(Some(slot))
            .and_then(|target| roster.display_name(target))
        else {
            continue;
        };
        if crate::chat::chat_body(&display, name).0.len() < display.len() {
            if found.is_some() {
                return None;
            }
            found = Some(slot);
        }
    }
    found
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}

/// C `atoi`: optional sign and leading digits, anything else yields zero.
pub(crate) fn atoi(bytes: &[u8]) -> i32 {
    let text = std::str::from_utf8(bytes).unwrap_or_default().trim_start();
    let end = text
        .char_indices()
        .find(|(index, ch)| !(ch.is_ascii_digit() || (*index == 0 && matches!(ch, '+' | '-'))))
        .map_or(text.len(), |(index, _)| index);
    text[..end].parse().unwrap_or(0)
}

fn config_int(game_state: &GameState, index: usize) -> i32 {
    atoi(game_state.config_string(index).unwrap_or_default())
}

fn info_int(info: &[u8], key: &str) -> i32 {
    crate::sound_events::config_info_value(info, key).map_or(0, |value| atoi(value.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::chat_sender;
    use crate::chat::ChatRoster;
    use sjk_protocol::GameState;

    #[test]
    fn a_chat_beep_names_the_slot_the_server_appends() {
        let roster = ChatRoster::default();
        assert_eq!(
            chat_sender(b"chat \"\x19^2Sol^7\x19: gg\" 5", &roster),
            Some(5)
        );
        assert_eq!(
            chat_sender(b"tchat \"\x19(Sol)\x19: go\" \"12\"", &roster),
            Some(12)
        );
        // Servers that send no slot, or one past the clients, name nobody here.
        assert_eq!(chat_sender(b"chat \"Sol: hi\"", &roster), None);
        assert_eq!(chat_sender(b"chat \"Sol: hi\" 40", &roster), None);
    }

    /// Report #147 (blec-bee): JA+ servers append no slot, so a blocked player's beep
    /// played while their line was hidden. The beep now names the one player whose
    /// name starts the line, as the chat feed does.
    #[test]
    fn a_chat_beep_without_a_slot_names_the_player_whose_name_starts_it() {
        let mut game = GameState::empty_local(0);
        for (slot, name) in [(2, "^1Fox"), (4, "^3D^5avy"), (6, "Twin"), (7, "Twin")] {
            game.replace_config_string(1131 + slot, format!("n\\{name}\\t\\0").into_bytes())
                .unwrap();
        }
        let mut roster = ChatRoster::default();
        roster.update(&game);
        assert_eq!(
            chat_sender(b"chat \"\x19^1Fox^7\x19: ^2hi\"", &roster),
            Some(2)
        );
        assert_eq!(
            chat_sender(b"tchat \"\x19(^3D^5avy^7)\x19: go\"", &roster),
            Some(4)
        );
        // A whisper's brackets.
        assert_eq!(
            chat_sender(b"chat \"\x19[^3D^5avy^7]\x19: psst\"", &roster),
            Some(4)
        );
        // Two players wearing one name, or nobody's name: nobody.
        assert_eq!(chat_sender(b"chat \"Twin: hi\"", &roster), None);
        assert_eq!(chat_sender(b"chat \"server: restart\"", &roster), None);
    }
}

/// An empty snapshot at `server_time`, for the tests below.
#[cfg(test)]
fn test_snapshot(server_time: i32) -> Snapshot {
    Snapshot {
        message_sequence: 1,
        reliable_acknowledge: 0,
        server_commands: vec![],
        server_time,
        delta_from: None,
        flags: 0,
        area_mask: vec![],
        player: sjk_protocol::PlayerState::zero(),
        vehicle_player: None,
        entities: vec![],
        consumed_bits: 0,
    }
}

#[cfg(test)]
mod pm_only_tests {
    use super::*;
    use sjk_protocol::ReliableServerCommand;
    #[test]
    fn pm_only_notifications_keep_only_marked_tells_and_resume_after_toggle() {
        let game = GameState::empty_local(0);
        let media = LocalSoundMedia::register(|_| 1);
        let mut local = LegacyLocalSounds::new(&game, media);
        local.pm_only = true;
        local.chat_mode = 2;
        let mut snapshot = test_snapshot(1000);
        for (sequence, command) in [
            b"chat \"Fox: public\" 2".as_slice(),
            b"tchat \"(Fox): team\" 2",
            b"chat \"Fox: [Fake^7]: ^6fake\" 2",
            b"chat \"\x19[Fox]: real PM\" 2",
            b"lchat Fox 0 7 hi 2",
        ]
        .iter()
        .enumerate()
        {
            snapshot.server_commands.push(ReliableServerCommand {
                sequence: sequence as i32 + 1,
                command: command.to_vec(),
            });
        }
        local.observe_snapshot(&snapshot);
        let beeps: Vec<_> = local
            .pending()
            .iter()
            .filter(|sound| {
                matches!(
                    sound.event,
                    LegacySoundEvent::ChatBeep
                        | LegacySoundEvent::TeamChatBeep
                        | LegacySoundEvent::PrivateChatBeep
                )
            })
            .collect();
        assert_eq!(beeps.len(), 1);
        assert_eq!(beeps[0].cause, Some(2));
        // Repeated snapshots do not replay that notification.
        local.observe_snapshot(&snapshot);
        assert!(local.pending().is_empty());
        local.pm_only = false;
        snapshot.server_commands = vec![ReliableServerCommand {
            sequence: 6,
            command: b"chat \"Fox: back\" 2".to_vec(),
        }];
        local.observe_snapshot(&snapshot);
        assert_eq!(
            local
                .pending()
                .iter()
                .filter(|sound| sound.event == LegacySoundEvent::ChatBeep)
                .count(),
            1
        );
    }
}

#[cfg(test)]
mod quiet_tests {
    use super::*;
    use sjk_protocol::ReliableServerCommand;

    /// The senders (`cause`) of the chat beeps `commands` make in quiet mode, in a duel
    /// with slot 3 while `duelling`.
    fn beeps(
        local: &mut LegacyLocalSounds,
        time: i32,
        duelling: bool,
        commands: &[&[u8]],
    ) -> usize {
        let mut snapshot = test_snapshot(time);
        snapshot.player.set_client_num(0);
        snapshot.player.set_raw_field(119, u32::from(duelling));
        snapshot.player.set_raw_field(44, 3);
        let first = local.last_command_sequence + 1;
        snapshot.server_commands = commands
            .iter()
            .enumerate()
            .map(|(n, command)| ReliableServerCommand {
                sequence: first + n as i32,
                command: command.to_vec(),
            })
            .collect();
        local.observe_snapshot(&snapshot);
        local
            .pending()
            .iter()
            .filter(|sound| {
                matches!(
                    sound.event,
                    LegacySoundEvent::ChatBeep
                        | LegacySoundEvent::TeamChatBeep
                        | LegacySoundEvent::PrivateChatBeep
                )
            })
            .count()
    }

    #[test]
    fn quiet_beeps_only_whispers_the_server_and_the_partner() {
        let mut game = GameState::empty_local(0);
        for (slot, name) in [(1, "Padawan"), (2, "Padawan"), (3, "Duelist"), (4, "Fox")] {
            game.replace_config_string(1_131 + slot, format!("n\\{name}\\t\\0").into_bytes())
                .expect("a player");
        }
        let mut local = LegacyLocalSounds::new(&game, LocalSoundMedia::register(|_| 1));
        local.quiet = true;
        for (command, beeps_wanted) in [
            (b"chat \"Fox^7\x19: hi\"".as_slice(), 0),
            (b"tchat \"(Fox^7)\x19: team\"", 0),
            (b"chat \"Padawan^7\x19: hi\"", 0),
            (b"chat \"Duelist^7\x19: hi\"", 1),
            (b"chat \"\x19[Fox^7\x19]\x19: psst\"", 1),
            (b"chat \"[Fox]: by its brackets\"", 1),
            (b"chat \"Server: restart\"", 1),
        ] {
            assert_eq!(
                beeps(&mut local, 1_000, true, &[command]),
                beeps_wanted,
                "{}",
                String::from_utf8_lossy(command)
            );
        }
        // The kill ends the duel: the partner's "gg" still beeps, later lines do not.
        assert_eq!(
            beeps(&mut local, 2_000, false, &[b"chat \"Duelist^7\x19: gg\""]),
            1
        );
        let late = 2_001 + crate::quiet_chat::PARTNER_GRACE_MS as i32;
        assert_eq!(
            beeps(&mut local, late, false, &[b"chat \"Duelist^7\x19: again\""]),
            0
        );
        local.quiet = false;
        assert_eq!(
            beeps(&mut local, late, false, &[b"chat \"Fox^7\x19: back\""]),
            1
        );
    }
}

#[cfg(test)]
mod chat_beep_tests {
    use super::*;
    use sjk_protocol::ReliableServerCommand;

    /// The chat beeps `commands` make at `mode`: each one's event and sound (talk 1,
    /// EJK team 2, EJK private 3).
    fn beeps(mode: i64, commands: &[&[u8]]) -> Vec<(LegacySoundEvent, Option<u16>)> {
        let media = LocalSoundMedia::register(|path| match path {
            "sound/player/talk.wav" => 1,
            "sound/movers/switches/button_11.mp3" => 2,
            "sound/movers/switches/button_15.mp3" => 3,
            _ => 9,
        });
        let mut local = LegacyLocalSounds::new(&GameState::empty_local(0), media);
        local.chat_mode = mode;
        let mut snapshot = test_snapshot(1000);
        snapshot.server_commands = commands
            .iter()
            .enumerate()
            .map(|(sequence, command)| ReliableServerCommand {
                sequence: sequence as i32 + 1,
                command: command.to_vec(),
            })
            .collect();
        local.observe_snapshot(&snapshot);
        local
            .pending()
            .iter()
            .map(|sound| (sound.event, sound.sound))
            .collect()
    }

    const COMMANDS: [&[u8]; 5] = [
        b"chat \"Fox: public\" 2",
        b"tchat \"(Fox): team\" 2",
        b"chat \"[Fox^7]: ^6a tell\" 2",
        // A tell to a teammate, with a location (G_Say's SAY_TELL in team games).
        b"lchat \"[Fox^7]: \" \"@Base\" \"6\" \"hello\" 2",
        // Typed to look like a tell: public chat.
        b"chat \"Fox: [Fake^7]: ^6fake\" 2",
    ];

    #[test]
    fn a_tell_is_its_own_event_at_every_mode_with_ejk_sounds_at_mode_2() {
        use LegacySoundEvent::{ChatBeep, PrivateChatBeep, TeamChatBeep};
        assert_eq!(
            beeps(1, &COMMANDS),
            [
                (ChatBeep, Some(1)),
                (TeamChatBeep, Some(1)),
                (PrivateChatBeep, Some(1)),
                (PrivateChatBeep, Some(1)),
                (ChatBeep, Some(1)),
            ]
        );
        assert_eq!(
            beeps(2, &COMMANDS),
            [
                (ChatBeep, Some(1)),
                (TeamChatBeep, Some(2)),
                (PrivateChatBeep, Some(3)),
                (PrivateChatBeep, Some(3)),
                (ChatBeep, Some(1)),
            ]
        );
    }
}

#[cfg(test)]
mod siege_round_count_tests {
    use super::*;

    fn set_state(game: &mut GameState, state: &str) {
        game.replace_config_string(CS_SIEGE_STATE, state.as_bytes().to_vec())
            .expect("a configstring");
    }

    /// The count sounds spoken by the latest snapshot, as 1, 2 or 3.
    fn counts(local: &LegacyLocalSounds) -> Vec<i32> {
        local
            .pending()
            .iter()
            .filter(|sound| sound.event == LegacySoundEvent::SiegeRoundCount)
            .filter_map(|sound| sound.sound.map(i32::from))
            .collect()
    }

    #[test]
    fn a_siege_round_counts_in_three_two_one_once_each() {
        let mut game = GameState::empty_local(0);
        set_state(&mut game, "2|10000");
        // count1Sound, count2Sound and count3Sound intern as 1, 2 and 3.
        let media = LocalSoundMedia::register(|path| match path.rsplit('/').next() {
            Some("40MOM037") => 1,
            Some("40MOM036") => 2,
            Some("40MOM035") => 3,
            _ => 99,
        });
        let mut local = LegacyLocalSounds::new(&game, media);
        let mut heard = Vec::new();
        for time in (10_000..16_000).step_by(50) {
            local.observe_snapshot(&test_snapshot(time));
            heard.extend(counts(&local));
        }
        assert_eq!(heard, [3, 2, 1]);

        // The round in play says nothing; the next one counts in from three again.
        set_state(&mut game, "0|15000");
        local.observe_config(&game);
        local.observe_snapshot(&test_snapshot(16_000));
        assert!(counts(&local).is_empty());
        set_state(&mut game, "2|20000");
        local.observe_config(&game);
        local.observe_snapshot(&test_snapshot(22_100));
        assert_eq!(counts(&local), [3]);
    }

    #[test]
    fn an_empty_state_keeps_the_count_and_intermission_silences_it() {
        let mut game = GameState::empty_local(0);
        set_state(&mut game, "2|10000");
        let media = LocalSoundMedia::register(|path| match path.rsplit('/').next() {
            Some("40MOM035") => 3,
            _ => 99,
        });
        let mut local = LegacyLocalSounds::new(&game, media);
        // A cleared string is ignored, as `CG_ConfigStringModified` ignores it.
        set_state(&mut game, "");
        local.observe_config(&game);
        let mut intermission = test_snapshot(12_100);
        intermission.player.set_movement_type(PM_INTERMISSION);
        local.observe_snapshot(&intermission);
        assert!(
            counts(&local).is_empty(),
            "nothing is counted in intermission"
        );
        local.observe_snapshot(&test_snapshot(12_150));
        assert_eq!(counts(&local), [3], "the round still counts in");
    }
}
