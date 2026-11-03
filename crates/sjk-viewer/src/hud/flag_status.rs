//! Capture-the-flag status on the HUD, in every HUD style: the flag icons at the left
//! (`CG_DrawFlagStatus`, `cg_draw.c`), a note when the local player carries a flag, and
//! the team scores at the top centre (`CG_DrawMiniScoreboard`'s red and blue counts).
//!
//! The flags' state is `CS_FLAGSTATUS` (config string 23, one character per team: `0` at
//! base, `1` taken, `2` dropped; `ctfFlagStatusRemap` in `cg_servercmds.c`), the team
//! scores `CS_SCORES1` and `CS_SCORES2`, and carrying is the local player state's
//! `PW_REDFLAG`, `PW_BLUEFLAG` or `PW_NEUTRALFLAG` timer, as `CG_DrawPowerupIcons` tests.
//!
//! A taken flag's caption names its carrier when the client knows who it is, from the
//! freshest source, as the scoreboard's flags do (`scoreboard/live_flags.rs`): the
//! viewed player's own state, a player entity in the snapshot, a teammate's `tinfo`
//! row, a `scores` row that arrived after the flag was taken, then the last carrier
//! seen while the flag stays taken. `scores` is the only source naming an enemy
//! carrier out of sight; it comes with each scoreboard press, and rows from before
//! the take could name an earlier carrier, so they are left out.
//!
//! With the team scores on, the scores stand in two team boxes under the clock, in
//! the SJK UI's team colours, each with its flag and where the flag is ("At base",
//! the carrier, "18 s" before a dropped one goes home); in capture games the boxes then replace the left
//! icons. `cg_teamScoresBoxes 0` brings back the classic "RED 3" and "BLUE 5" plates
//! beside the left icons; `cg_drawTeamScores 0` leaves the left icons alone.
use super::icons::assets;
use super::*;
use crate::menu::sjk::color;
use sjk_client::{ScoreEntry, TeamInfo};
use sjk_ui::{Color, DrawCommand, FontWeight, Rect, TextAlign, TextOverflow};
use std::fmt::Write;

/// Text ids of the flag captions (two rows), the two team scores, the two boxes'
/// flag lines and the line under the boxes.
pub(super) const TEXT_FIRST: u32 = 340;
/// Last text id this module resolves.
pub(super) const TEXT_LAST: u32 = 348;
/// The team scores in their boxes.
const TEXT_SCORES: u32 = 344;
/// Where each team's flag is, in its box.
const TEXT_STATUS: u32 = 346;
/// The line under the boxes while the local player carries a flag.
const TEXT_HINT: u32 = 348;
/// How long a dropped flag lies before it goes home by itself: `LaunchItem` gives a
/// dropped CTF flag `nextthink = level.time + 30000` and `Team_DroppedFlagThink`
/// returns it (OpenJK `codemp/game/g_items.c`; JoF EJK and jaPRO keep it, but for
/// jaPRO's Tribes weapons, 45 s, which the client cannot tell: its count reaches zero
/// early and the box then just says "Dropped").
const FLAG_RETURN_MS: i64 = 30_000;

const CS_SCORES1: usize = 6;
const CS_SCORES2: usize = 7;
const CS_FLAGSTATUS: usize = 23;
/// `SCORE_NOT_PRESENT` (`bg_public.h`): a team score the server has not sent.
const SCORE_NOT_PRESENT: i32 = -9999;
/// `gametype_t`: team free for all, siege, capture the flag, capture the ysalamiri.
const GT_TEAM: i32 = 6;
const GT_SIEGE: i32 = 7;
const GT_CTF: i32 = 8;
const GT_CTY: i32 = 9;
const TEAM_RED: u8 = 1;
const TEAM_BLUE: u8 = 2;
/// `PW_REDFLAG`, `PW_BLUEFLAG`, `PW_NEUTRALFLAG`.
const FLAG_POWERUPS: [usize; 3] = [4, 5, 6];
/// `CS_PLAYERS`: one client-info config string per client from here.
const CS_PLAYERS: usize = 1_131;
/// `entityType_t::ET_PLAYER` and `ET_EVENTS`.
const ET_PLAYER: u8 = 1;
const ET_EVENTS: u8 = 18;
/// `EV_EVENT_BITS`: the sequence bits over an entity's `event`.
const EV_EVENT_BITS: u16 = 0x300;
/// `EV_CTFMESSAGE` and `CTFMESSAGE_PLAYER_GOT_FLAG` (`bg_public.h`, `ctfMessages_t`).
const EV_CTFMESSAGE: u16 = 99;
const CTFMESSAGE_PLAYER_GOT_FLAG: u32 = 4;
/// `PMF_FOLLOW`: the player state is the followed player's, not the local player's.
const PMF_FOLLOW: u16 = sjk_game_jka::follow::PMF_FOLLOW;
/// `MAX_CLIENTS`.
const MAX_CLIENTS: usize = 32;
/// How long a CTF message naming who took a flag stays good for, in server time:
/// the config string saying the flag is taken may come a little before or after it.
const TAKER_MS: i64 = 3_000;

/// Where a team's flag is (`flagStatus_t`, the states the server sends).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Flag {
    /// On its stand.
    #[default]
    AtBase,
    /// Held by a player.
    Taken,
    /// Lying where its carrier died.
    Dropped,
}

/// The red and blue flags' states from a `CS_FLAGSTATUS` string; an unknown character
/// reads as at base, as `CG_SetConfigValues` ignores it.
pub(crate) fn parse_status(status: &[u8]) -> [Flag; 2] {
    let flag = |byte: Option<&u8>| match byte {
        Some(b'1') => Flag::Taken,
        Some(b'2') => Flag::Dropped,
        _ => Flag::AtBase,
    };
    [flag(status.first()), flag(status.get(1))]
}

/// One flag icon of the left stack.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Row {
    /// Whose flag it is: 0 red, 1 blue.
    pub(crate) side: usize,
    /// The icon's cell ([`assets`]).
    pub(crate) slot: usize,
    /// The note beside it.
    pub(crate) caption: &'static str,
    /// The local player carries this flag: the icon stands on a gold plate.
    pub(crate) carried: bool,
}

/// The flag rows for a player of `team`: first the enemy flag when the team holds it
/// (taken, dropped, or carried by the local player whatever the status says), then the
/// team's own flag when the enemy holds it (`CG_YourTeamHasFlag`, `CG_OtherTeamHasFlag`).
pub(crate) fn rows(team: u8, flags: [Flag; 2], carried: bool, ysalamiri: bool) -> [Option<Row>; 2] {
    let (own, enemy) = match team {
        TEAM_RED => (flags[0], flags[1]),
        TEAM_BLUE => (flags[1], flags[0]),
        _ => return [None, None],
    };
    let caption = |flag| match flag {
        Flag::Dropped => "DROPPED",
        _ => "TAKEN",
    };
    let red = team == TEAM_RED;
    let (own_side, enemy_side) = if red { (0, 1) } else { (1, 0) };
    let enemy_slot = match (red, ysalamiri) {
        (true, false) => assets::BLUE_FLAG,
        (true, true) => assets::BLUE_FLAG_YSALAMIRI,
        (false, false) => assets::RED_FLAG,
        (false, true) => assets::RED_FLAG_YSALAMIRI,
    };
    let held = enemy != Flag::AtBase || carried;
    let first = held.then_some(Row {
        side: enemy_side,
        slot: enemy_slot,
        caption: if carried {
            "YOU HAVE THE FLAG"
        } else {
            caption(enemy)
        },
        carried,
    });
    let second = (own != Flag::AtBase).then_some(Row {
        side: own_side,
        slot: if red {
            assets::RED_FLAG_TAKEN
        } else {
            assets::BLUE_FLAG_TAKEN
        },
        caption: caption(own),
        carried: false,
    });
    [first, second]
}

/// Sampled once per HUD update; the strings keep their storage.
#[derive(Default)]
pub(crate) struct State {
    team: u8,
    ctf: bool,
    ysalamiri: bool,
    flags: [Flag; 2],
    carried: bool,
    /// The player state is a followed player's: their flag is not "You".
    following: bool,
    /// `sv_serverid` at the last update: a new gamestate (map change, restart or
    /// another server) forgets the drops and takers seen in the last one.
    server_id: Option<i32>,
    show_flags: bool,
    show_scores: bool,
    /// The scores stand in SJK team boxes (`cg_teamScoresBoxes`), else on the classic
    /// plates beside the left flag icons.
    boxes: bool,
    /// The boxes' size over the HUD's (`cg_teamScoresScale`, 0.5 to 2; 0 before the
    /// first update reads as 1).
    box_scale: f32,
    scores: Option<[i32; 2]>,
    labels: [String; 2],
    /// The red and blue flags' carriers by name ([`Carrier`]).
    carriers: [Carrier; 2],
    /// Who the last CTF message said took each flag, and when ([`ctf_takers`]).
    takers: [Option<(u16, i64)>; 2],
    /// The snapshot's server time at the last update.
    now: i64,
    /// When each flag was seen dropping; `None` while it is not dropped, or when it was
    /// already lying when this client first saw the game (its count is unknown).
    dropped_at: [Option<i64>; 2],
    /// The flags were read in a capture game at the last update, so a change since is a
    /// change seen happening.
    seen: bool,
    /// Where each flag is, as its box says it, and how.
    statuses: [String; 2],
    tones: [Tone; 2],
    /// The line under the boxes, empty when none.
    hint: &'static str,
    /// A made-up CTF state for world shots.
    #[cfg(test)]
    staged: Option<Staged>,
}

/// How a box's flag line reads.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Tone {
    /// At base: quiet.
    #[default]
    Quiet,
    /// Taken or dropped.
    Plain,
    /// Carried by the local player: gold.
    Own,
}

/// Who carries one flag, and the name the caption shows. The name is formatted again
/// only when the carrier, their client-info config string or the colour setting
/// changes, so a steady carrier costs a byte comparison a frame and no allocation.
#[derive(Default)]
struct Carrier {
    client: Option<u16>,
    /// The `scores` revision when the flag was first seen taken; `None` while it is not.
    taken_at: Option<u32>,
    colors: bool,
    /// The client-info config string `name` was read from.
    source: Vec<u8>,
    name: String,
}

impl Carrier {
    fn set(&mut self, client: Option<u16>, game: &GameState, colors: bool) {
        let Some(number) = client else {
            self.client = None;
            self.name.clear();
            return;
        };
        let info = game
            .config_string(CS_PLAYERS + usize::from(number))
            .unwrap_or_default();
        if self.client == client && self.colors == colors && self.source == info {
            return;
        }
        self.client = client;
        self.colors = colors;
        self.source.clear();
        self.source.extend_from_slice(info);
        self.name.clear();
        if let Some((name, _)) = data_source::parse_client_info(info) {
            family::name(&mut self.name, &name, colors);
        }
    }
}

/// The latest `scores` rows and their revision (`ClientSession::scores_revision`).
#[derive(Clone, Copy, Default)]
pub(crate) struct ScoreRows<'a> {
    pub(crate) rows: &'a [ScoreEntry],
    pub(crate) revision: u32,
}

/// The players a snapshot's CTF messages say just took a flag, by side (red, blue):
/// `PrintCTFMessage(player, flag team, CTFMESSAGE_PLAYER_GOT_FLAG)` from
/// `Team_TouchEnemyFlag` (OpenJK `codemp/game/g_team.c`) raises a broadcast
/// `EV_CTFMESSAGE` with the player in `trickedentindex` and the flag's team in
/// `trickedentindex2`, so every client learns who took a flag, in sight or not.
fn ctf_takers(snapshot: &sjk_protocol::Snapshot) -> [Option<u16>; 2] {
    let mut takers = [None; 2];
    for state in &snapshot.entities {
        let event = if state.entity_type() >= ET_EVENTS {
            u16::from(state.entity_type() - ET_EVENTS)
        } else {
            state.event() & !EV_EVENT_BITS
        };
        if event != EV_CTFMESSAGE
            || u32::from(state.event_parameter()) != CTFMESSAGE_PLAYER_GOT_FLAG
        {
            continue;
        }
        let player = state.tracked_entity_num();
        // `trickedentindex2`, which the protocol's accessor names for its mute use.
        let side = match state.mute_entity_num() {
            1 => 0,
            2 => 1,
            _ => continue,
        };
        if usize::from(player) < MAX_CLIENTS {
            takers[side] = Some(player);
        }
    }
    takers
}

/// The client carrying flag powerup `power`, from the viewed player's state, a player
/// entity in the snapshot or a teammate's `tinfo` row.
fn find_carrier(power: usize, snapshot: &sjk_protocol::Snapshot, team: &[TeamInfo]) -> Option<u16> {
    let bit = 1_u32 << power;
    if i64::from(snapshot.player.powerups[power]) > i64::from(snapshot.server_time) {
        return Some(snapshot.player.client_num());
    }
    if let Some(state) = snapshot
        .entities
        .iter()
        .find(|state| state.entity_type() == ET_PLAYER && state.powerups() & bit != 0)
    {
        return Some(state.number());
    }
    team.iter()
        .find(|info| info.powerups as u32 & bit != 0)
        .map(|info| u16::from(info.client_num))
}

/// What a world shot stages over a recorded game.
#[cfg(test)]
#[derive(Clone, Copy)]
pub(crate) struct Staged {
    pub(crate) team: u8,
    pub(crate) flags: [Flag; 2],
    pub(crate) carried: bool,
    pub(crate) scores: [i32; 2],
    pub(crate) ysalamiri: bool,
    /// A capture game; off stages team FFA (scores only).
    pub(crate) ctf: bool,
    /// The view follows the carrier rather than being theirs.
    pub(crate) following: bool,
}

impl State {
    /// Whether the team score plates are on this frame (the HUD's score line then
    /// leaves the team scores out).
    pub(super) fn scores_shown(&self) -> bool {
        self.show_scores && self.scores.is_some()
    }

    pub(super) fn update(
        &mut self,
        snapshot: &sjk_protocol::Snapshot,
        game: &GameState,
        console: Option<&ViewerConsole>,
    ) {
        let previous = self.flags;
        self.show_flags = family::integer(console, "cg_drawFlagStatus", 1) != 0;
        self.show_scores = family::integer(console, "cg_drawTeamScores", 1) != 0;
        self.boxes = family::integer(console, "cg_teamScoresBoxes", 1) != 0;
        self.box_scale = console
            .and_then(|console| console.float_cvar("cg_teamScoresScale"))
            .map_or(1.0, |scale| scale as f32)
            .clamp(0.5, 2.0);
        let gametype = game
            .config_string(0)
            .and_then(|s| sjk_client::LegacyClientInfo::new(s).integer("g_gametype"))
            .unwrap_or(0);
        self.team = snapshot.player.team();
        self.following = snapshot.player.movement_flags() & PMF_FOLLOW != 0;
        let server_id = game
            .config_string(1)
            .and_then(|s| sjk_client::LegacyClientInfo::new(s).integer("sv_serverid"));
        self.new_game(server_id, i64::from(snapshot.server_time));
        self.ctf = matches!(gametype, GT_CTF | GT_CTY);
        self.ysalamiri = gametype == GT_CTY;
        self.flags = if self.ctf {
            game.config_string(CS_FLAGSTATUS)
                .map_or([Flag::AtBase; 2], parse_status)
        } else {
            [Flag::AtBase; 2]
        };
        self.carried = self.ctf
            && FLAG_POWERUPS.iter().any(|&power| {
                i64::from(snapshot.player.powerups[power]) > i64::from(snapshot.server_time)
            });
        self.scores = (gametype >= GT_TEAM && gametype != GT_SIEGE).then(|| {
            [CS_SCORES1, CS_SCORES2].map(|index| {
                game.config_string(index)
                    .and_then(|s| std::str::from_utf8(s).ok())
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0)
            })
        });
        #[cfg(test)]
        if let Some(staged) = self.staged {
            self.team = staged.team;
            self.ctf = staged.ctf;
            self.following = staged.following;
            self.ysalamiri = staged.ysalamiri;
            self.flags = staged.flags;
            self.carried = staged.carried;
            self.scores = Some(staged.scores);
        }
        self.track_drops(previous, i64::from(snapshot.server_time));
        // A box shows the bare score, a classic plate its team's name before it.
        for (label, (name, score)) in self.labels.iter_mut().zip(
            ["RED ", "BLUE "]
                .into_iter()
                .zip(self.scores.unwrap_or([0; 2])),
        ) {
            label.clear();
            if !self.boxes {
                label.push_str(name);
            }
            if score == SCORE_NOT_PRESENT {
                label.push('-');
            } else {
                let _ = write!(label, "{score}");
            }
        }
        self.format_statuses();
    }

    /// Forget the drops and CTF messages of the last game when the gamestate changes
    /// (`server_id`) or the server's clock goes back: their times are another game's.
    fn new_game(&mut self, server_id: Option<i32>, now: i64) {
        if self.server_id != server_id || now < self.now {
            self.seen = false;
            self.dropped_at = [None; 2];
            self.takers = [None; 2];
        }
        self.server_id = server_id;
    }

    /// The side of the flag the local player carries (not a followed player's).
    fn own_carry(&self) -> Option<usize> {
        self.enemy_side()
            .filter(|_| self.carried && !self.following)
    }

    /// Note when a flag is seen dropping at server time `now`, the flags having been
    /// `previous` at the last update. A flag already lying when the client first reads
    /// a capture game has no known count.
    fn track_drops(&mut self, previous: [Flag; 2], now: i64) {
        self.now = now;
        for ((dropped_at, flag), before) in self.dropped_at.iter_mut().zip(self.flags).zip(previous)
        {
            *dropped_at = match flag {
                Flag::Dropped if !self.ctf => None,
                Flag::Dropped if self.seen && before != Flag::Dropped => Some(now),
                Flag::Dropped => *dropped_at,
                _ => None,
            };
        }
        self.seen = self.ctf;
    }

    /// The milliseconds before flag `side` goes home by itself, when known.
    fn return_left(&self, side: usize) -> Option<i64> {
        let left = FLAG_RETURN_MS - (self.now - self.dropped_at[side]?);
        (left > 0).then_some(left.min(FLAG_RETURN_MS))
    }

    /// The side of the flag the local player's team goes for, or `None` off a team.
    fn enemy_side(&self) -> Option<usize> {
        match self.team {
            TEAM_RED => Some(1),
            TEAM_BLUE => Some(0),
            _ => None,
        }
    }

    /// Word where each flag is for its box, and the line under the boxes, in their
    /// kept storage.
    fn format_statuses(&mut self) {
        let enemy = self.enemy_side();
        let mine = self.own_carry();
        for side in 0..2 {
            let own = mine == Some(side);
            // A followed carrier's flag reads as taken even before the status catches up.
            let flag = if self.carried && enemy == Some(side) {
                Flag::Taken
            } else {
                self.flags[side]
            };
            let left = self.return_left(side);
            let status = &mut self.statuses[side];
            status.clear();
            self.tones[side] = if own {
                status.push_str("You");
                Tone::Own
            } else {
                match flag {
                    Flag::AtBase => {
                        status.push_str("At base");
                        Tone::Quiet
                    }
                    Flag::Taken => {
                        let name = &self.carriers[side].name;
                        status.push_str(if name.is_empty() { "Taken" } else { name });
                        Tone::Plain
                    }
                    Flag::Dropped => {
                        match left {
                            Some(left) => {
                                let _ = write!(status, "{} s", (left + 999) / 1_000);
                            }
                            None => status.push_str("Dropped"),
                        }
                        Tone::Plain
                    }
                }
            };
        }
        self.hint = match mine {
            Some(enemy) if self.ctf => {
                if self.flags[1 - enemy] == Flag::AtBase {
                    "Take it home"
                } else {
                    "Your flag is away"
                }
            }
            _ => "",
        };
    }

    /// Note who carries each taken flag, from `snapshot`, the teammates' `team` rows
    /// and `scores` rows newer than the take; a carrier out of sight stays named while
    /// the flag stays taken.
    pub(super) fn update_carriers(
        &mut self,
        snapshot: &sjk_protocol::Snapshot,
        game: &GameState,
        team: &[TeamInfo],
        scores: ScoreRows,
        colors: bool,
    ) {
        let now = i64::from(snapshot.server_time);
        for (taker, seen) in self.takers.iter_mut().zip(ctf_takers(snapshot)) {
            if let Some(player) = seen {
                *taker = Some((player, now));
            } else if taker.is_some_and(|(_, at)| now - at > TAKER_MS) {
                *taker = None;
            }
        }
        for (side, carrier) in self.carriers.iter_mut().enumerate() {
            if !(self.ctf && self.flags[side] == Flag::Taken) {
                carrier.taken_at = None;
                carrier.set(None, game, colors);
                continue;
            }
            let power = FLAG_POWERUPS[side];
            let since_take = *carrier.taken_at.get_or_insert(scores.revision) != scores.revision;
            let taker = self.takers[side].map(|(player, _)| player);
            let client = find_carrier(power, snapshot, team)
                .or(taker)
                .or_else(|| {
                    let bit = 1_u32 << power;
                    since_take.then_some(scores.rows).and_then(|rows| {
                        rows.iter()
                            .find(|row| row.powerups & bit != 0)
                            .map(|row| u16::from(row.client_num))
                    })
                })
                .or(carrier.client);
            carrier.set(client, game, colors);
        }
        self.format_statuses();
    }

    /// Play a made-up CTF state over a recorded game, for world shots.
    #[cfg(test)]
    pub(crate) fn stage_for_shot(&mut self, staged: Staged) {
        self.staged = Some(staged);
    }

    /// The caption or label behind text id `id`.
    pub(super) fn text(&self, id: u32) -> &str {
        match id {
            TEXT_FIRST | 343 => self.caption(usize::from(id != TEXT_FIRST)),
            TEXT_SCORES => &self.labels[0],
            345 => &self.labels[1],
            TEXT_STATUS => &self.statuses[0],
            347 => &self.statuses[1],
            TEXT_HINT => self.hint,
            _ => "",
        }
    }

    /// The caption of the `row`th drawn icon: a taken flag's carrier by name when known.
    fn caption(&self, row: usize) -> &str {
        let Some(row) = rows(self.team, self.flags, self.carried, self.ysalamiri)
            .into_iter()
            .flatten()
            .nth(row)
        else {
            return "";
        };
        let carrier = &self.carriers[row.side].name;
        if !row.carried && self.flags[row.side] == Flag::Taken && !carrier.is_empty() {
            carrier
        } else {
            row.caption
        }
    }

    /// Whether the team boxes show the flags this frame, so the other flag pictures
    /// (the left icons, the carried flag's powerup pictures) stay out.
    pub(super) fn flags_in_boxes(&self, visibility: HudVisibility) -> bool {
        self.boxes
            && self.show_flags
            && self.ctf
            && self.scores_shown()
            && visibility.elements
            && visibility.status
    }

    /// Append the icons, their captions and the team score plates.
    pub(super) fn emit(
        &self,
        list: &mut DrawList,
        viewport: [f32; 2],
        icons: &icons::Icons,
        visibility: HudVisibility,
    ) {
        let [w, h] = viewport;
        // Under the status gate, as JoF's team scores in `CG_DrawHUD`: an open classic
        // board keeps the HUD but not these boxes over its header.
        let scores = self.scores_shown() && visibility.elements && visibility.status;
        let boxes = scores && self.boxes;
        if boxes {
            self.emit_boxes(list, w, h, icons);
        } else if scores {
            self.emit_plates(list, w, h);
        }
        if self.show_flags && self.ctf && visibility.status && !boxes {
            // CG_DrawFlagStatus: 32-unit icons at the left edge of the 640x480 screen,
            // the first at y 328 and the next one icon up.
            let unit = h / 480.0;
            let size = 32.0 * unit;
            let rows = rows(self.team, self.flags, self.carried, self.ysalamiri);
            for (index, row) in rows.iter().flatten().enumerate() {
                let x = 6.0 * unit;
                let y = (328.0 - 36.0 * index as f32) * unit;
                if row.carried {
                    let _ = list.push(DrawCommand::SolidRect {
                        rect: Rect::new(
                            x - 3.0 * unit,
                            y - 3.0 * unit,
                            size + 6.0 * unit,
                            size + 6.0 * unit,
                        ),
                        color: Color::new(1.0, 0.78, 0.2, 0.6),
                    });
                }
                let texture = icons.cell(row.slot).or_else(|| {
                    icons.cell(match row.slot {
                        assets::RED_FLAG_TAKEN => assets::RED_FLAG,
                        assets::BLUE_FLAG_TAKEN => assets::BLUE_FLAG,
                        other => other,
                    })
                });
                if let Some(texture) = texture {
                    let _ = list.push(DrawCommand::TexturedQuad {
                        rect: Rect::new(x, y, size, size),
                        texture,
                        color: Color::new(1.0, 1.0, 1.0, 1.0),
                    });
                }
                let _ = list.push(DrawCommand::Text {
                    rect: Rect::new(
                        x + size + 6.0 * unit,
                        y + 6.0 * unit,
                        200.0 * unit,
                        22.0 * unit,
                    ),
                    text: TextId(TEXT_FIRST + 3 * index as u32),
                    size: 13.0 * unit,
                    color: if row.carried {
                        Color::new(1.0, 0.85, 0.35, 1.0)
                    } else {
                        Color::new(1.0, 1.0, 1.0, 1.0)
                    },
                    align: TextAlign::Start,
                    overflow: TextOverflow::Clip,
                    weight: FontWeight::Semibold,
                    letter_spacing: 0.0,
                });
            }
        }
    }

    /// The classic team score plates under the clock: "RED 3" and "BLUE 5".
    fn emit_plates(&self, list: &mut DrawList, w: f32, h: f32) {
        let s = crate::ui_scale::height_scale(h);
        let (plate_w, plate_h, gap) = (130.0 * s, 30.0 * s, 8.0 * s);
        let top = 76.0 * s;
        for (index, tint) in [
            Color::new(0.75, 0.12, 0.12, 0.55),
            Color::new(0.14, 0.3, 0.85, 0.55),
        ]
        .into_iter()
        .enumerate()
        {
            let x = if index == 0 {
                w * 0.5 - gap * 0.5 - plate_w
            } else {
                w * 0.5 + gap * 0.5
            };
            let rect = Rect::new(x, top, plate_w, plate_h);
            let _ = list.push(DrawCommand::SolidRect { rect, color: tint });
            let _ = list.push(DrawCommand::Text {
                rect,
                text: TextId(TEXT_SCORES + index as u32),
                size: 20.0 * s,
                color: Color::new(1.0, 1.0, 1.0, 1.0),
                align: TextAlign::Center,
                overflow: TextOverflow::Clip,
                weight: FontWeight::Semibold,
                letter_spacing: 0.0,
            });
        }
    }

    /// The two team boxes under the clock, red left and blue right, in a `w` by `h`
    /// viewport: the UI's navy under the team's colour, a rule of it along the bottom;
    /// the score at the inner
    /// edge and, in capture games with `cg_drawFlagStatus`, the flag at the outer edge
    /// (its taken picture while away, a gold arc round it counting down a drop) and
    /// where it is between them; then, while the local player carries a flag, one line
    /// under the boxes.
    fn emit_boxes(&self, list: &mut DrawList, w: f32, h: f32, icons: &icons::Icons) {
        // The boxes stand where the plates did, under the clock, at any size.
        let top = 76.0 * crate::ui_scale::height_scale(h);
        let scale = if self.box_scale > 0.0 {
            self.box_scale
        } else {
            1.0
        };
        let s = crate::ui_scale::height_scale(h) * scale;
        let (box_w, box_h, gap, pad) = (168.0 * s, 42.0 * s, 6.0 * s, 9.0 * s);
        let (icon, rule) = (22.0 * s, 2.0 * s);
        // HUD text sets its line box from the rectangle's top: give each run a line
        // about its size, centred on the box.
        let line = |size: f32| (top + (box_h - size * 1.25) * 0.5, size * 1.25);
        let flags = self.show_flags && self.ctf;
        let own = self.enemy_side().map(|enemy| 1 - enemy);
        for (side, team) in [color::RED_TEAM, color::BLUE_TEAM].into_iter().enumerate() {
            let red = side == 0;
            let x = if red {
                w * 0.5 - gap * 0.5 - box_w
            } else {
                w * 0.5 + gap * 0.5
            };
            let rect = Rect::new(x, top, box_w, box_h);
            for color in [color::alpha(color::SPACE, 0.72), color::alpha(team, 0.22)] {
                let _ = list.push(DrawCommand::SolidRect { rect, color });
            }
            let _ = list.push(DrawCommand::SolidRect {
                rect: Rect::new(x, top + box_h - rule, box_w, rule),
                color: team,
            });
            // Without flags the score has the whole box, centred; beside a flag, the
            // inner edge, as wide as its digits ("-12" or "300" fit, smaller if need be).
            let (score_w, score_size) = score_fit(
                self.labels[side].chars().count(),
                if flags {
                    SCORE_MAX_W * s
                } else {
                    box_w - 2.0 * pad
                },
                36.0 * s,
            );
            let score_w = if flags { score_w } else { box_w - 2.0 * pad };
            let (score_y, score_h) = line(score_size);
            let inner = Rect::new(
                if !flags {
                    x + pad
                } else if red {
                    x + box_w - pad - score_w
                } else {
                    x + pad
                },
                score_y,
                score_w,
                score_h,
            );
            let _ = list.push(DrawCommand::Text {
                rect: inner,
                text: TextId(TEXT_SCORES + side as u32),
                size: score_size,
                color: color::TEXT,
                align: if !flags {
                    TextAlign::Center
                } else if red {
                    TextAlign::End
                } else {
                    TextAlign::Start
                },
                overflow: TextOverflow::Clip,
                weight: FontWeight::Semibold,
                letter_spacing: 0.0,
            });
            if !flags {
                continue;
            }
            let icon_x = if red { x + pad } else { x + box_w - pad - icon };
            let icon_y = top + (box_h - icon) * 0.5;
            let away = self.flags[side] != Flag::AtBase
                || (self.carried && self.enemy_side() == Some(side));
            let slot = match (red, self.ysalamiri, away) {
                (true, _, true) => assets::RED_FLAG_TAKEN,
                (false, _, true) => assets::BLUE_FLAG_TAKEN,
                (true, false, false) => assets::RED_FLAG,
                (true, true, false) => assets::RED_FLAG_YSALAMIRI,
                (false, false, false) => assets::BLUE_FLAG,
                (false, true, false) => assets::BLUE_FLAG_YSALAMIRI,
            };
            // A pack without the taken pictures shows the flag faint while it is away.
            let picture = icons.cell(slot).map(|texture| (texture, 1.0)).or_else(|| {
                let home = if red {
                    assets::RED_FLAG
                } else {
                    assets::BLUE_FLAG
                };
                icons.cell(home).map(|texture| (texture, 0.45))
            });
            if let Some((texture, alpha)) = picture {
                let _ = list.push(DrawCommand::TexturedQuad {
                    rect: Rect::new(icon_x, icon_y, icon, icon),
                    texture,
                    color: Color::new(1.0, 1.0, 1.0, alpha),
                });
            }
            if let Some(left) = self.return_left(side) {
                let _ = list.push(DrawCommand::Arc {
                    center: [icon_x + icon * 0.5, icon_y + icon * 0.5],
                    radius: icon * 0.5 + 3.0 * s,
                    width: 2.0 * s,
                    start: -std::f32::consts::FRAC_PI_2,
                    sweep: std::f32::consts::TAU * left as f32 / FLAG_RETURN_MS as f32,
                    color: color::GOLD,
                    knockout: None,
                });
            }
            let words_x = if red {
                icon_x + icon + 7.0 * s
            } else {
                x + pad + score_w
            };
            let words_w = box_w - 2.0 * pad - icon - 7.0 * s - score_w;
            let (words_y, words_h) = line(13.0 * s);
            let _ = list.push(DrawCommand::Text {
                rect: Rect::new(words_x, words_y, words_w, words_h),
                text: TextId(TEXT_STATUS + side as u32),
                size: 13.0 * s,
                color: match self.tones[side] {
                    Tone::Quiet => color::MUTED,
                    Tone::Plain => color::TEXT,
                    Tone::Own => color::GOLD_BRIGHT,
                },
                align: if red {
                    TextAlign::Start
                } else {
                    TextAlign::End
                },
                overflow: TextOverflow::Ellipsis,
                weight: FontWeight::Semibold,
                letter_spacing: 0.0,
            });
        }
        if flags && !self.hint.is_empty() {
            let (hint_w, hint_h, size) = (150.0 * s, 20.0 * s, 13.0 * s);
            let (hint_x, hint_y) = (w * 0.5 - hint_w * 0.5, top + box_h + gap);
            let _ = list.push(DrawCommand::SolidRect {
                rect: Rect::new(hint_x, hint_y, hint_w, hint_h),
                color: color::alpha(color::SPACE, 0.78),
            });
            let _ = list.push(DrawCommand::Text {
                rect: Rect::new(
                    hint_x,
                    hint_y + (hint_h - size * 1.25) * 0.5,
                    hint_w,
                    size * 1.25,
                ),
                text: TextId(TEXT_HINT),
                size,
                color: if self.flags[own.unwrap_or(0)] == Flag::AtBase {
                    color::GOLD_BRIGHT
                } else {
                    color::TEXT
                },
                align: TextAlign::Center,
                overflow: TextOverflow::Clip,
                weight: FontWeight::Regular,
                letter_spacing: 0.0,
            });
        }
    }
}

/// The widest a box's score stands beside its flag, in 1080-line pixels.
const SCORE_MAX_W: f32 = 66.0;
/// A HUD digit's width over its size (the semibold figures, a little generous).
const DIGIT_EM: f32 = 0.62;

/// The width and size a score of `chars` characters takes at `size`, at most `max_w`
/// wide: as wide as its digits, then smaller once they no longer fit.
fn score_fit(chars: usize, max_w: f32, size: f32) -> (f32, f32) {
    let chars = chars.max(1) as f32;
    let need = chars * DIGIT_EM * size;
    if need <= max_w {
        (need.max(size * 0.7), size)
    } else {
        (max_w, max_w / (chars * DIGIT_EM))
    }
}

impl HudOverlay {
    /// Name the flag carriers after [`HudOverlay::update_family`] has read the flags;
    /// `team` is the teammates' `tinfo` and `scores` the last scoreboard rows (both
    /// empty for a demo, which keeps none).
    pub(crate) fn update_flag_carriers(
        &mut self,
        snapshot: &sjk_protocol::Snapshot,
        game: &GameState,
        team: &[TeamInfo],
        scores: ScoreRows,
    ) {
        self.flag_status
            .update_carriers(snapshot, game, team, scores, self.family.name_colors);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_characters_map_to_states() {
        assert_eq!(parse_status(b"12"), [Flag::Taken, Flag::Dropped]);
        assert_eq!(parse_status(b"00"), [Flag::AtBase; 2]);
        assert_eq!(parse_status(b"*"), [Flag::AtBase; 2]);
        assert_eq!(parse_status(b""), [Flag::AtBase; 2]);
    }

    #[test]
    fn a_red_player_sees_the_blue_flag_taken_and_the_own_flag_gone() {
        let [first, second] = rows(TEAM_RED, [Flag::Taken, Flag::Taken], false, false);
        assert_eq!(first.unwrap().slot, assets::BLUE_FLAG);
        assert_eq!(first.unwrap().caption, "TAKEN");
        assert_eq!(second.unwrap().slot, assets::RED_FLAG_TAKEN);
    }

    #[test]
    fn a_blue_player_sees_the_red_flag_and_a_dropped_caption() {
        let [first, second] = rows(TEAM_BLUE, [Flag::Dropped, Flag::AtBase], false, false);
        assert_eq!(first.unwrap().slot, assets::RED_FLAG);
        assert_eq!(first.unwrap().caption, "DROPPED");
        assert!(second.is_none());
    }

    #[test]
    fn carrying_marks_the_first_row_even_if_the_status_says_at_base() {
        let [first, second] = rows(TEAM_BLUE, [Flag::AtBase; 2], true, false);
        let first = first.unwrap();
        assert!(first.carried);
        assert_eq!(first.caption, "YOU HAVE THE FLAG");
        assert!(second.is_none());
    }

    #[test]
    fn ysalamiri_games_use_the_ys_icons_and_spectators_get_nothing() {
        let [first, _] = rows(TEAM_RED, [Flag::AtBase, Flag::Taken], false, true);
        assert_eq!(first.unwrap().slot, assets::BLUE_FLAG_YSALAMIRI);
        assert_eq!(rows(3, [Flag::Taken; 2], true, false), [None, None]);
    }

    #[test]
    fn a_taken_flag_names_its_carrier_and_keeps_the_name_cached() {
        let mut game = GameState::empty_local(0);
        game.replace_config_string(CS_PLAYERS + 3, b"\\n\\^1Slash\\t\\1".to_vec())
            .unwrap();
        let mut state = State {
            team: TEAM_RED,
            ctf: true,
            flags: [Flag::Taken, Flag::Taken],
            ..State::default()
        };
        state.carriers[1].set(Some(3), &game, false);
        // The blue flag (held by red) first, then red's own flag, no carrier known.
        assert_eq!(state.caption(0), "Slash");
        assert_eq!(state.caption(1), "TAKEN");
        let capacity = state.carriers[1].name.capacity();
        state.carriers[1].set(Some(3), &game, false);
        assert_eq!(state.carriers[1].name.capacity(), capacity);
        state.carriers[1].set(Some(3), &game, true);
        assert_eq!(state.caption(0), "^1Slash");
        state.carriers[1].set(None, &game, true);
        assert_eq!(state.caption(0), "TAKEN");
    }

    #[test]
    fn an_enemy_carrier_out_of_sight_is_named_from_scores_sent_after_the_take() {
        let mut game = GameState::empty_local(0);
        for (client, name) in [(3, "Slash"), (5, "Sol")] {
            let info = format!("\\n\\{name}\\t\\1").into_bytes();
            game.replace_config_string(CS_PLAYERS + client, info)
                .unwrap();
        }
        let snapshot = sjk_protocol::Snapshot {
            message_sequence: 1,
            reliable_acknowledge: 0,
            server_commands: vec![],
            server_time: 1_000,
            delta_from: None,
            flags: 0,
            area_mask: vec![],
            player: sjk_protocol::PlayerState::zero(),
            vehicle_player: None,
            entities: vec![],
            consumed_bits: 0,
        };
        let row = |client_num, powerups| ScoreEntry {
            client_num,
            score: 0,
            ping: 0,
            time_minutes: 0,
            flags: 0,
            powerups,
            defends: 0,
            assists: 0,
            captures: 0,
            deaths: None,
        };
        let blue_flag = 1 << FLAG_POWERUPS[1];
        // A blue player's own flag, taken by a red player nobody on blue can see.
        let mut state = State {
            team: TEAM_BLUE,
            ctf: true,
            flags: [Flag::AtBase, Flag::Taken],
            ..State::default()
        };
        let update = |state: &mut State, rows: &[ScoreEntry], revision| {
            state.update_carriers(&snapshot, &game, &[], ScoreRows { rows, revision }, false);
        };
        // Rows from before the take name an earlier carrier: left out.
        let stale = [row(5, blue_flag), row(3, 0)];
        update(&mut state, &stale, 1);
        assert_eq!(state.caption(0), "TAKEN");
        // A scoreboard press after the take names the carrier, who stays named.
        let fresh = [row(5, 0), row(3, blue_flag)];
        update(&mut state, &fresh, 2);
        assert_eq!(state.caption(0), "Slash");
        update(&mut state, &[], 3);
        assert_eq!(state.caption(0), "Slash");
        // Returned and taken again: the rows from the last take no longer count.
        state.flags[1] = Flag::AtBase;
        update(&mut state, &fresh, 3);
        state.flags[1] = Flag::Taken;
        update(&mut state, &fresh, 3);
        assert_eq!(state.caption(0), "TAKEN");
    }

    #[test]
    fn a_ctf_message_names_who_took_a_flag_out_of_sight() {
        let mut game = GameState::empty_local(0);
        game.replace_config_string(CS_PLAYERS + 7, b"\\n\\Torren\\t\\1".to_vec())
            .unwrap();
        let mut event = sjk_protocol::EntityState::zero(40, &sjk_protocol::LEGACY_ENTITY_FIELDS);
        event.set_raw_field(8, u32::from(ET_EVENTS) + u32::from(EV_CTFMESSAGE));
        event.set_raw_field(42, CTFMESSAGE_PLAYER_GOT_FLAG);
        event.set_raw_field(58, 7);
        // `trickedentindex2` (netfield 74): the flag's team.
        event.set_raw_field(74, 2);
        let mut snapshot = sjk_protocol::Snapshot {
            message_sequence: 1,
            reliable_acknowledge: 0,
            server_commands: vec![],
            server_time: 1_000,
            delta_from: None,
            flags: 0,
            area_mask: vec![],
            player: sjk_protocol::PlayerState::zero(),
            vehicle_player: None,
            entities: vec![event],
            consumed_bits: 0,
        };
        assert_eq!(ctf_takers(&snapshot), [None, Some(7)]);
        // The message comes a moment before the status says taken: still good.
        let mut state = ctf_state(TEAM_BLUE, [Flag::AtBase; 2]);
        state.update_carriers(&snapshot, &game, &[], ScoreRows::default(), false);
        snapshot.entities.clear();
        snapshot.server_time = 1_200;
        state.flags[1] = Flag::Taken;
        state.update_carriers(&snapshot, &game, &[], ScoreRows::default(), false);
        assert_eq!(state.text(347), "Torren");
    }

    #[test]
    fn the_own_flag_alone_keeps_its_caption() {
        let state = State {
            team: TEAM_BLUE,
            ctf: true,
            flags: [Flag::AtBase, Flag::Dropped],
            ..State::default()
        };
        assert_eq!(state.caption(0), "DROPPED");
        assert_eq!(state.caption(1), "");
    }

    fn ctf_state(team: u8, flags: [Flag; 2]) -> State {
        State {
            team,
            ctf: true,
            flags,
            ..State::default()
        }
    }

    #[test]
    fn the_boxes_say_where_each_flag_is() {
        let mut state = ctf_state(TEAM_BLUE, [Flag::AtBase, Flag::Taken]);
        state.format_statuses();
        assert_eq!(state.text(TEXT_STATUS), "At base");
        assert_eq!(state.tones[0], Tone::Quiet);
        // No carrier known yet: just taken.
        assert_eq!(state.text(347), "Taken");
        state.carriers[1].name.push_str("Torren");
        state.format_statuses();
        assert_eq!(state.text(347), "Torren");
        assert_eq!(state.tones[1], Tone::Plain);
        assert_eq!(state.text(TEXT_HINT), "");
    }

    #[test]
    fn a_carrier_reads_you_and_a_line_on_what_to_do() {
        // Blue carries red's flag; the status may not have caught up yet.
        let mut state = ctf_state(TEAM_BLUE, [Flag::AtBase, Flag::AtBase]);
        state.carried = true;
        state.format_statuses();
        assert_eq!(state.text(TEXT_STATUS), "You");
        assert_eq!(state.tones[0], Tone::Own);
        assert_eq!(state.text(TEXT_HINT), "Take it home");
        // Blue's own flag away: no capture until it is back.
        state.flags = [Flag::Taken, Flag::Dropped];
        state.format_statuses();
        assert_eq!(state.text(TEXT_HINT), "Your flag is away");
    }

    #[test]
    fn a_drop_seen_happening_counts_down_to_its_return() {
        let mut state = ctf_state(TEAM_RED, [Flag::AtBase; 2]);
        state.track_drops([Flag::AtBase; 2], 1_000);
        state.flags = [Flag::AtBase, Flag::Dropped];
        state.track_drops([Flag::AtBase; 2], 2_000);
        state.format_statuses();
        assert_eq!(state.text(347), "30 s");
        state.track_drops(state.flags, 14_500);
        state.format_statuses();
        assert_eq!(state.text(347), "18 s");
        assert_eq!(state.return_left(1), Some(17_500));
        // Past the stock return time (a mod that waits longer): just dropped.
        state.track_drops(state.flags, 32_000);
        state.format_statuses();
        assert_eq!(state.text(347), "Dropped");
        // Home again, then dropped again: a new count.
        state.flags = [Flag::AtBase; 2];
        state.track_drops([Flag::AtBase, Flag::Dropped], 33_000);
        assert_eq!(state.dropped_at, [None, None]);
    }

    #[test]
    fn a_flag_already_lying_when_first_seen_has_no_count() {
        let mut state = ctf_state(TEAM_RED, [Flag::Dropped, Flag::AtBase]);
        state.track_drops([Flag::AtBase; 2], 5_000);
        state.format_statuses();
        assert_eq!(state.text(TEXT_STATUS), "Dropped");
        state.track_drops(state.flags, 6_000);
        assert_eq!(state.return_left(0), None);
    }

    #[test]
    fn the_boxes_replace_the_left_icons_and_draw_the_count() {
        let mut state = ctf_state(TEAM_RED, [Flag::AtBase; 2]);
        state.show_flags = true;
        state.show_scores = true;
        state.boxes = true;
        state.scores = Some([3, 5]);
        state.track_drops([Flag::AtBase; 2], 0);
        state.flags = [Flag::Dropped, Flag::Taken];
        state.track_drops([Flag::AtBase; 2], 1_000);
        state.format_statuses();
        let visibility = HudVisibility::from_console(None);
        let icons = icons::Icons::default();
        let mut list = DrawList::new(64);
        state.emit(&mut list, [1920.0, 1080.0], &icons, visibility);
        let texts: Vec<u32> = list
            .commands()
            .iter()
            .filter_map(|command| match command {
                DrawCommand::Text { text, .. } => Some(text.0),
                _ => None,
            })
            .collect();
        assert_eq!(texts, [344, TEXT_STATUS, 345, 347]);
        assert!(
            list.commands()
                .iter()
                .any(|command| matches!(command, DrawCommand::Arc { .. }))
        );
        // The classic look: the plates and the left icons with their captions.
        state.boxes = false;
        let mut list = DrawList::new(64);
        state.emit(&mut list, [1920.0, 1080.0], &icons, visibility);
        let texts: Vec<u32> = list
            .commands()
            .iter()
            .filter_map(|command| match command {
                DrawCommand::Text { text, .. } => Some(text.0),
                _ => None,
            })
            .collect();
        assert_eq!(texts, [344, 345, TEXT_FIRST, 343]);
        // Team scores off: only the left icons.
        state.show_scores = false;
        let mut list = DrawList::new(64);
        state.emit(&mut list, [1920.0, 1080.0], &icons, visibility);
        assert!(list.commands().iter().all(|command| match command {
            DrawCommand::Text { text, .. } => [TEXT_FIRST, 343].contains(&text.0),
            _ => true,
        }));
    }

    #[test]
    fn a_followed_carrier_is_named_not_you() {
        // Spectating a red carrier who holds blue's flag: no "You", no line below.
        let mut state = ctf_state(TEAM_RED, [Flag::AtBase; 2]);
        state.carried = true;
        state.following = true;
        state.carriers[1].name.push_str("Torren");
        state.format_statuses();
        assert_eq!(state.text(347), "Torren");
        assert_eq!(state.tones[1], Tone::Plain);
        assert_eq!(state.text(TEXT_HINT), "");
        // Unnamed yet: taken, even before the status catches up.
        state.carriers[1].name.clear();
        state.format_statuses();
        assert_eq!(state.text(347), "Taken");
    }

    #[test]
    fn a_new_gamestate_forgets_the_last_games_drops() {
        let mut state = ctf_state(TEAM_RED, [Flag::AtBase; 2]);
        state.new_game(Some(1), 1_000);
        state.track_drops([Flag::AtBase; 2], 1_000);
        state.flags = [Flag::AtBase, Flag::Dropped];
        state.track_drops([Flag::AtBase; 2], 2_000);
        state.takers[0] = Some((4, 2_000));
        assert!(state.dropped_at[1].is_some());
        // Another server, where a flag already lies: no count, no taker.
        state.new_game(Some(2), 500_000);
        state.flags = [Flag::Dropped, Flag::AtBase];
        state.track_drops([Flag::AtBase; 2], 500_000);
        assert_eq!(state.dropped_at, [None, None]);
        assert_eq!(state.takers, [None, None]);
        state.format_statuses();
        assert_eq!(state.text(TEXT_STATUS), "Dropped");
        // The same game's clock going back counts as a new game too.
        state.flags = [Flag::AtBase; 2];
        state.track_drops([Flag::Dropped, Flag::AtBase], 501_000);
        state.flags = [Flag::Dropped, Flag::AtBase];
        state.track_drops([Flag::AtBase; 2], 502_000);
        assert!(state.dropped_at[0].is_some());
        state.new_game(Some(2), 100);
        assert_eq!(state.dropped_at, [None, None]);
        assert!(!state.seen);
    }

    #[test]
    fn long_scores_fit_their_box() {
        // Two digits beside a flag: full size, narrower than the old fixed 48.
        let (w, size) = score_fit(2, SCORE_MAX_W, 36.0);
        assert_eq!(size, 36.0);
        assert!(w <= 48.0);
        // Three digits or a minus: still inside the widest score, smaller if need be.
        for chars in [3, 4] {
            let (w, size) = score_fit(chars, SCORE_MAX_W, 36.0);
            assert!(w <= SCORE_MAX_W && chars as f32 * DIGIT_EM * size <= w + 0.01);
        }
        // Team FFA: the whole box, so "300" keeps the full size.
        let (_, size) = score_fit(3, 150.0, 36.0);
        assert_eq!(size, 36.0);
        // A team FFA box centres the score over the box's width.
        let mut state = State {
            team: TEAM_RED,
            show_scores: true,
            boxes: true,
            scores: Some([300, -12]),
            ..State::default()
        };
        state.labels = ["300".into(), "-12".into()];
        let mut list = DrawList::new(64);
        let visibility = HudVisibility::from_console(None);
        state.emit(
            &mut list,
            [1920.0, 1080.0],
            &icons::Icons::default(),
            visibility,
        );
        let scores: Vec<_> = list
            .commands()
            .iter()
            .filter_map(|command| match command {
                DrawCommand::Text {
                    rect, size, align, ..
                } => Some((*rect, *size, *align)),
                _ => None,
            })
            .collect();
        assert_eq!(scores.len(), 2);
        for (rect, size, align) in scores {
            assert_eq!(align, TextAlign::Center);
            assert_eq!(size, 36.0);
            assert!(rect.width > 140.0);
        }
    }

    #[test]
    fn at_base_flags_show_nothing() {
        assert_eq!(
            rows(TEAM_RED, [Flag::AtBase; 2], false, false),
            [None, None]
        );
    }
}
