//! End-to-end encrypted whispers (`docs/hub-chat.md`, "Encrypted whispers"): a
//! whisper to another SJK player goes through the SJK hub sealed to their identity key
//! (`sjk_identity::whisper`), so only the two of them can read it, while
//! `cl_e2eWhispers` is on (the default).
//!
//! A `tell` (the composer's whisper, `tell`, `reply`) to a player whose claim on the
//! server carries a whisper key is sealed and handed to the hub instead of the game
//! server ([`GpuState::route_encrypted_whisper`]). When the hub takes it, the chat feed
//! and the console show it in its pink, labelled ENCRYPTED; when it does not, the card
//! ([`e2e_whisper_card`](crate::e2e_whisper_card)) says why and asks before sending it
//! unencrypted through the game server. Whispers that come in are shown the same way,
//! with their own sound, and `reply` answers them.
//!
//! Each player's keys are remembered in `whisper-keys.txt` beside `config.cfg`
//! ([`sjk_identity::whisper::KeyPins`]): a key not seen before for a player is said in
//! the console, since a lying hub could hand out its own.
//!
//! Optionally, a player compares the safety code of their two keys with the other
//! outside the game (the chat's player card, `whisper_verify`) and marks them verified
//! (`whisper-verified.txt`, [`sjk_identity::whisper::Verified`]): their whispers then say
//! VERIFIED, and once their key changes nothing goes encrypted to it without the card
//! asking, and what comes from it is flagged.
//!
//! Only a player the hub vouches for (verified or staff) starts an encrypted whisper
//! (Sol, 25/10/2026: no advertising by strangers where no server can see it); another
//! answers one, to its sender only, for [`ANSWER_WINDOW`] after it came, as the hub
//! allows. Any other whisper goes through the game server as before, which the
//! composer's "Not private" note says. A whisper that came can be reported with the
//! sender's proof inside it (the chat card's Report, `player_report.rs`).
//!
//! Servers whose operators moderate whispers ([`PLAIN_FAMILIES`]) keep them on the game
//! server: there the client neither takes nor sends encrypted whispers unless the
//! player turns that family's setting on.

use crate::e2e_whisper_card::{Choice, Failure, Safety};
use crate::{GpuState, player_identity};
use sjk_identity::whisper::{
    Checked, KeyPins, OpenError, Outgoing, PinCheck, Received, Verified, parse_key, safety_code,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// `cl_e2eWhispers`: whether whispers to SJK players go encrypted through the hub.
pub(crate) const CVAR: &str = "cl_e2eWhispers";
/// The remembered keys, in the settings folder.
const PINS_FILE: &str = "whisper-keys.txt";
/// The players checked with the safety code, in the settings folder.
const VERIFIED_FILE: &str = "whisper-verified.txt";
/// `whisper_verify`: the safety code with a player.
pub(crate) const VERIFY_COMMAND: &str = "whisper_verify";
/// How long a sent whisper may wait for the hub's answer before it counts as failed.
const ANSWER_WAIT: Duration = Duration::from_secs(10);
/// How long a player the hub does not vouch for may answer an encrypted whisper after
/// it came: the hub's 30 minutes (`PROTOCOL.md`, "Whispers") less one, so an answer
/// does not race its end.
const ANSWER_WINDOW: Duration = Duration::from_secs(29 * 60);
/// How long the last encrypted whisper from a player may be reported: the hub takes
/// one up to a day old.
const REPORT_WINDOW: Duration = Duration::from_secs(23 * 60 * 60);

/// A server family whose operators asked that whispers on their servers stay on the
/// game server, where they moderate them (AGENTS.md, no hardcoded special cases; the
/// families are `sjk_mod::known_servers`'), and the player's setting that turns
/// encrypted whispers back on there.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PlainFamily {
    /// The family's id in `sjk_mod::known_servers`.
    pub(crate) family: &'static str,
    /// Its name, as the family's.
    pub(crate) name: &'static str,
    /// The setting, off by default.
    pub(crate) cvar: &'static str,
}

/// The families that keep whispers on the game server.
pub(crate) const PLAIN_FAMILIES: &[PlainFamily] = &[
    // JoF asked to moderate whispers on their servers, against advertising (Sol,
    // 25/10/2026). The setting is the JoF tools' (`sjk_mod_jof::E2E_WHISPERS`).
    PlainFamily {
        family: "jof",
        name: "JoF",
        cvar: "jof.e2eWhispers",
    },
];

/// The family of [`PLAIN_FAMILIES`] `family` is, if any.
fn plain_family(family: Option<&sjk_mod::KnownServer>) -> Option<&'static PlainFamily> {
    let family = family?;
    PLAIN_FAMILIES
        .iter()
        .find(|plain| plain.family == family.id)
}

/// The last encrypted whisper from a player, kept for a report.
struct Reportable {
    server: SocketAddr,
    came: Instant,
    whisper: sjk_identity::ReportedWhisper,
}

/// A whisper handed to the hub, until it answers.
struct Pending {
    serial: u64,
    server: SocketAddr,
    /// The recipient's slot.
    slot: u16,
    /// The recipient's name as the game showed it.
    name: String,
    text: String,
    /// The `tell` the game server would have had, for sending it unencrypted.
    command: String,
    sent: Instant,
    /// The player was checked with the safety code, with this key.
    verified: bool,
}

#[derive(Default)]
pub(crate) struct State {
    serial: u64,
    pending: Vec<Pending>,
    pins: Option<(PathBuf, KeyPins)>,
    verified: Option<(PathBuf, Verified)>,
    pub(crate) card: crate::e2e_whisper_card::Card,
    /// The card opened the game menu, which goes with it.
    opened_menu: bool,
    /// The server, identity revision and [`State::generation`] the slots that take
    /// whispers were found for.
    slots_for: Option<(SocketAddr, u64, u64)>,
    /// When each player (key id) last whispered to us: a player the hub does not vouch
    /// for answers them within [`ANSWER_WINDOW`].
    answers: HashMap<String, Instant>,
    /// The last encrypted whisper from each player (key id), for a report.
    reportable: HashMap<String, Reportable>,
    /// Bumped when whom we may answer or report changes.
    generation: u64,
}

impl State {
    /// Whether `key` is one seen for `key_id` (remembering it), reading and writing the
    /// file in `directory`. A key seen before, or the first for a player, is no news.
    fn pin(&mut self, directory: &Path, key_id: &str, key: &str) -> PinCheck {
        let path = directory.join(PINS_FILE);
        if self.pins.as_ref().is_none_or(|(at, _)| *at != path) {
            let pins = std::fs::read_to_string(&path)
                .map(|text| KeyPins::parse(&text))
                .unwrap_or_default();
            self.pins = Some((path.clone(), pins));
        }
        let (_, pins) = self.pins.as_mut().expect("loaded above");
        let check = pins.check(key_id, key);
        if check != PinCheck::Known {
            let _ = std::fs::write(&path, pins.to_text());
        }
        check
    }

    /// The players checked with the safety code, read from `directory` once.
    fn verified(&mut self, directory: &Path) -> &mut Verified {
        let path = directory.join(VERIFIED_FILE);
        if self.verified.as_ref().is_none_or(|(at, _)| *at != path) {
            let verified = std::fs::read_to_string(&path)
                .map(|text| Verified::parse(&text))
                .unwrap_or_default();
            self.verified = Some((path, verified));
        }
        &mut self.verified.as_mut().expect("loaded above").1
    }

    /// Whether `key` is the one checked for `key_id`.
    fn checked(&mut self, directory: &Path, key_id: &str, key: &str) -> Checked {
        self.verified(directory).check(key_id, key)
    }

    /// Whether we may start or answer an encrypted whisper to `key_id`: always when the
    /// hub `vouched` for us, else within [`ANSWER_WINDOW`] of their last one.
    fn may_whisper(&self, key_id: &str, vouched: bool, now: Instant) -> bool {
        vouched
            || self
                .answers
                .get(key_id)
                .is_some_and(|came| now.duration_since(*came) < ANSWER_WINDOW)
    }

    /// Remember a whisper that came from `key_id` on `server`: we may answer it, and
    /// report it.
    fn came(
        &mut self,
        key_id: &str,
        server: SocketAddr,
        whisper: sjk_identity::ReportedWhisper,
        now: Instant,
    ) {
        self.answers.insert(key_id.to_owned(), now);
        self.reportable.insert(
            key_id.to_owned(),
            Reportable {
                server,
                came: now,
                whisper,
            },
        );
        self.generation += 1;
    }

    /// Forget the answers and reports out of their windows.
    fn expire(&mut self, now: Instant) {
        let (answers, reportable) = (self.answers.len(), self.reportable.len());
        self.answers
            .retain(|_, came| now.duration_since(*came) < ANSWER_WINDOW);
        self.reportable
            .retain(|_, kept| now.duration_since(kept.came) < REPORT_WINDOW);
        if (answers, reportable) != (self.answers.len(), self.reportable.len()) {
            self.generation += 1;
        }
    }

    /// The last encrypted whisper `key_id` sent us on `server`, for a report.
    pub(crate) fn reportable(
        &self,
        key_id: &str,
        server: SocketAddr,
    ) -> Option<&sjk_identity::ReportedWhisper> {
        self.reportable
            .get(key_id)
            .filter(|kept| kept.server == server)
            .map(|kept| &kept.whisper)
    }

    /// The players (key ids) whose last encrypted whisper on `server` can be reported.
    pub(crate) fn reportable_ids(&self, server: SocketAddr) -> Vec<String> {
        self.reportable
            .iter()
            .filter(|(_, kept)| kept.server == server)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Mark `key` as checked for `key_id`, or forget the check, and write the file.
    fn set_checked(&mut self, directory: &Path, key_id: &str, key: Option<&str>) {
        let verified = self.verified(directory);
        verified.set(key_id, key);
        let text = verified.to_text();
        let _ = std::fs::write(directory.join(VERIFIED_FILE), text);
    }
}

/// The console line telling that `name`'s key is new on this PC.
fn key_changed_line(name: &str) -> String {
    format!(
        "^3Encrypted whispers: {name}^3 uses a key this PC has not seen for them before (another \
         PC of theirs, a new key, or the SJK hub handing out its own)."
    )
}

/// How a whisper's text shows: EternalJK's escapes for `%` and `"` as what was typed,
/// as for every chat line.
fn shown_text(text: &str) -> String {
    sjk_client::chat_display_text(&sjk_client::chat_unescape(text))
}

/// The whisper's text: what follows `tell <slot>`, unquoted as the server reads it.
fn whisper_text(command: &str) -> Option<(u16, String)> {
    let tokens = server_tokens(command);
    let [verb, slot, body @ ..] = tokens.as_slice() else {
        return None;
    };
    let text = body.join(" ");
    (verb.eq_ignore_ascii_case("tell") && !text.trim().is_empty())
        .then(|| Some((slot.parse().ok()?, text)))
        .flatten()
}

/// `command`'s arguments as the game server splits them (`Cmd_TokenizeString`): a
/// quote runs to the next quote or the end, and a backslash is plain text. The
/// console's escapes would turn a whisper ending in `\` into an unclosed quote and
/// send it unencrypted.
fn server_tokens(command: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut rest = command;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            return tokens;
        }
        let (token, after) = if let Some(quoted) = rest.strip_prefix('"') {
            quoted.split_once('"').unwrap_or((quoted, ""))
        } else {
            let end = rest
                .find(|c: char| c.is_whitespace() || c == '"')
                .unwrap_or(rest.len());
            rest.split_at(end)
        };
        tokens.push(token);
        rest = after;
    }
}

/// Whether encrypted whispers are on for `console`'s player here: `cl_e2eWhispers` and
/// the identity on, and on a server of [`PLAIN_FAMILIES`] its setting too.
fn on_here(console: &crate::console::ViewerConsole) -> bool {
    console.bool_cvar(CVAR) != Some(false)
        && console.bool_cvar("cl_identity") == Some(true)
        && plain_family(console.server_family())
            .is_none_or(|plain| console.bool_cvar(plain.cvar) == Some(true))
}

impl GpuState {
    /// Send `command`, a `tell`, encrypted through the hub when its recipient takes
    /// encrypted whispers, we may whisper them and they are on here ([`on_here`]): true
    /// when it went that way, and the game server must not have it.
    pub(crate) fn route_encrypted_whisper(&mut self, command: &str) -> bool {
        let Some(console) = self.console.as_ref() else {
            return false;
        };
        if !on_here(console) {
            return false;
        }
        let directory = console.config_directory().to_owned();
        let Some((slot, text)) = whisper_text(command) else {
            return false;
        };
        let Some(session) = self
            .live_session
            .as_ref()
            .filter(|session| !session.is_local())
        else {
            return false;
        };
        let game_state = session.game_state();
        if u16::try_from(game_state.client_num).ok() == Some(slot)
            || text.len() > sjk_identity::whisper::TEXT_MAX
        {
            return false;
        }
        let server = session.server();
        let Some(shown) = player_identity::shown_name(game_state, usize::from(slot)) else {
            return false;
        };
        let Ok(target) = u8::try_from(slot) else {
            return false;
        };
        let Some((key_id, key)) = player_identity::with_snapshot(|snapshot| {
            let player = snapshot.badge(target, &shown)?;
            Some((player.key_id.clone(), player.whisper_key.clone()?))
        })
        .flatten() else {
            return false;
        };
        // Not ours to start: it goes through the game server, as the composer said.
        if !self
            .e2e_whispers
            .may_whisper(&key_id, player_identity::vouched(), Instant::now())
        {
            return false;
        }
        let checked = self.e2e_whispers.checked(&directory, &key_id, &key);
        // A player checked with the safety code whose key changed: nothing goes to the
        // new key without asking; the card offers the game server instead.
        if checked == Checked::Changed {
            self.e2e_whispers.card.offer(Failure {
                server,
                name: shown,
                text: shown_text(&text),
                command: command.to_owned(),
                why: "their key changed since you verified them; compare the safety code \
                      again (whisper_verify)"
                    .to_owned(),
            });
            self.open_e2e_whisper_card();
            return true;
        }
        self.e2e_whispers.serial += 1;
        let serial = self.e2e_whispers.serial;
        let sent = player_identity::send_whisper(Outgoing {
            serial,
            server: server.to_string(),
            target,
            to: key.clone(),
            text: text.clone(),
        });
        if !sent {
            return false;
        }
        if self.e2e_whispers.pin(&directory, &key_id, &key) == PinCheck::Changed
            && let Some(console) = self.console.as_mut()
        {
            console.push_log(key_changed_line(&shown));
        }
        self.e2e_whispers.pending.push(Pending {
            serial,
            server,
            slot,
            name: shown,
            text: shown_text(&text),
            command: command.to_owned(),
            sent: Instant::now(),
            verified: checked == Checked::Yes,
        });
        true
    }

    /// Show the safety code with the player in `slot` (`whisper_verify`, the chat's
    /// player card); why not, when they do not take encrypted whispers here.
    pub(crate) fn show_safety_code(&mut self, slot: u16) -> Result<(), String> {
        let not_here = || "they do not take encrypted whispers here".to_owned();
        let Some(console) = self.console.as_ref() else {
            return Err(not_here());
        };
        let directory = console.config_directory().to_owned();
        let session = self
            .live_session
            .as_ref()
            .filter(|session| !session.is_local())
            .ok_or_else(|| "only on a server".to_owned())?;
        let game_state = session.game_state();
        if u16::try_from(game_state.client_num).ok() == Some(slot) {
            return Err("that is you".to_owned());
        }
        let server = session.server();
        let target = u8::try_from(slot).map_err(|_| not_here())?;
        let shown =
            player_identity::shown_name(game_state, usize::from(slot)).ok_or_else(not_here)?;
        let (key_id, key, own) = player_identity::with_snapshot(|snapshot| {
            let player = snapshot.badge(target, &shown)?;
            Some((
                player.key_id.clone(),
                player.whisper_key.clone()?,
                snapshot.public_key.clone(),
            ))
        })
        .flatten()
        .ok_or_else(not_here)?;
        let (Some(theirs), Some(own)) = (parse_key(&key), parse_key(&own)) else {
            return Err(not_here());
        };
        let checked = self.e2e_whispers.checked(&directory, &key_id, &key);
        self.e2e_whispers.card.show_safety(Safety {
            server,
            name: shown,
            key_id,
            key,
            code: safety_code(&own, &theirs),
            checked,
        });
        self.open_e2e_whisper_card();
        Ok(())
    }

    /// `whisper_verify <player>`: a slot number or (part of) a name, as `tell` takes.
    pub(crate) fn whisper_verify_command(
        &mut self,
        args: &[String],
    ) -> Result<Vec<String>, String> {
        let usage = || format!("usage: {VERIFY_COMMAND} <player>");
        let wanted = args.join(" ");
        if wanted.trim().is_empty() {
            return Err(usage());
        }
        let session = self
            .live_session
            .as_ref()
            .ok_or_else(|| format!("{VERIFY_COMMAND}: only on a server"))?;
        let mut roster = sjk_client::ChatRoster::default();
        roster.update(session.game_state());
        let slot = match wanted.trim().parse::<u16>() {
            Ok(slot) => slot,
            Err(_) => match roster.lookup(wanted.trim()) {
                sjk_client::PlayerLookup::Found(slot) => slot,
                sjk_client::PlayerLookup::NotFound => {
                    return Err(format!("{VERIFY_COMMAND}: no player matches \"{wanted}\""));
                }
                sjk_client::PlayerLookup::Ambiguous(_) => {
                    return Err(format!(
                        "{VERIFY_COMMAND}: \"{wanted}\" matches several players; use a number"
                    ));
                }
            },
        };
        if let Some(console) = self.console.as_mut() {
            console.set_open(false);
        }
        self.show_safety_code(slot)
            .map(|()| Vec::new())
            .map_err(|why| format!("{VERIFY_COMMAND}: {why}"))
    }

    /// Once a frame: listen for encrypted whispers while on a server with them on, show
    /// what the hub did with ours and the ones that came, and tell the chat which slots
    /// take them.
    pub(crate) fn update_e2e_whispers(&mut self) {
        let on = self.console.as_ref().is_some_and(on_here);
        self.e2e_whispers.expire(Instant::now());
        let server = self
            .live_session
            .as_ref()
            .filter(|session| !session.is_local())
            .map(|session| session.server());
        player_identity::set_whisper_listen(on && server.is_some());
        self.update_encrypted_slots(server.filter(|_| on));
        if let Some(slot) = self.chat.take_safety_request()
            && let Err(why) = self.show_safety_code(slot)
            && let Some(console) = self.console.as_mut()
        {
            console.push_log(format!("^3Safety code: {why}."));
        }
        if let Some(slot) = self.chat.take_report_request() {
            if self.chat.is_typing() {
                self.chat.close_composer();
            }
            self.release_pointer();
            self.game_menu = true;
            self.open_whisper_report(slot);
            self.sync_cursor_policy();
        }
        self.e2e_whisper_answers(server);
        let received = player_identity::take_whispers(server);
        for whisper in received {
            self.show_received_whisper(whisper);
        }
        // A failure for a server the player left cannot be sent unencrypted there.
        self.e2e_whispers.card.keep_for(server);
    }

    /// Tell the chat which slots' players we may whisper encrypted, and whose last
    /// whisper can be reported, when the known players, the server or our answers
    /// changed.
    fn update_encrypted_slots(&mut self, server: Option<SocketAddr>) {
        let Some(server) = server else {
            if self.e2e_whispers.slots_for.take().is_some() {
                self.chat.set_encrypted_slots(0);
                self.chat.set_reportable_slots(0);
            }
            return;
        };
        let revision = player_identity::revision();
        let key = (server, revision, self.e2e_whispers.generation);
        if self.e2e_whispers.slots_for == Some(key) {
            return;
        }
        let Some(session) = self.live_session.as_ref() else {
            return;
        };
        let game_state = session.game_state();
        let vouched = player_identity::vouched();
        let now = Instant::now();
        let state = &self.e2e_whispers;
        let (slots, reportable) = player_identity::with_snapshot(|snapshot| {
            let (mut slots, mut reportable) = (0_u64, 0_u64);
            for player in &snapshot.players {
                let shown = player_identity::shown_name(game_state, usize::from(player.slot));
                if player.slot >= 64
                    || !shown.is_some_and(|shown| {
                        sjk_identity::names_match(&player.claimed_name, &shown)
                    })
                {
                    continue;
                }
                if player.whisper_key.is_some() && state.may_whisper(&player.key_id, vouched, now) {
                    slots |= 1 << player.slot;
                }
                if state.reportable(&player.key_id, server).is_some() {
                    reportable |= 1 << player.slot;
                }
            }
            (slots, reportable)
        })
        .unwrap_or((0, 0));
        self.chat.set_encrypted_slots(slots);
        self.chat.set_reportable_slots(reportable);
        self.e2e_whispers.slots_for = Some(key);
    }

    /// What the hub did with the whispers handed to it: shown when taken, else the card.
    fn e2e_whisper_answers(&mut self, server: Option<SocketAddr>) {
        let answers = player_identity::take_whispers_sent();
        let now = Instant::now();
        let mut done: Vec<(Pending, Result<(), String>)> = Vec::new();
        for answer in answers {
            if let Some(index) = self
                .e2e_whispers
                .pending
                .iter()
                .position(|pending| pending.serial == answer.serial)
            {
                done.push((self.e2e_whispers.pending.remove(index), answer.result));
            }
        }
        // No answer in time (the service stopped): failed.
        while let Some(index) = self
            .e2e_whispers
            .pending
            .iter()
            .position(|pending| now.duration_since(pending.sent) >= ANSWER_WAIT)
        {
            done.push((
                self.e2e_whispers.pending.remove(index),
                Err("the SJK hub did not answer".to_owned()),
            ));
        }
        for (pending, result) in done {
            match result {
                Ok(()) => {
                    self.chat.receive_encrypted(
                        None,
                        (pending.slot, &pending.name),
                        &pending.text,
                        pending.verified,
                        now,
                    );
                    // Sent: its sound, as the SJK chat plays one for the player's own.
                    if crate::chat_sounds::enabled(
                        self.console.as_ref(),
                        crate::chat_sounds::Kind::Private,
                    ) {
                        crate::audio::ui_cues::post(crate::audio::ui_cues::Cue::EncryptedWhisper);
                    }
                    if let Some(console) = self.console.as_mut() {
                        let line = format!("^6To {}^6 (encrypted): {}", pending.name, pending.text);
                        console.push_chat_line(line.clone(), &line, None);
                    }
                }
                Err(why) if Some(pending.server) == server => {
                    self.e2e_whispers.card.offer(Failure {
                        server: pending.server,
                        name: pending.name,
                        text: pending.text,
                        command: pending.command,
                        why,
                    });
                    self.open_e2e_whisper_card();
                }
                // Written on a server the player has left: said, never sent.
                Err(why) => {
                    if let Some(console) = self.console.as_mut() {
                        console.push_log(format!(
                            "^3Your encrypted whisper to {}^3 did not go ({why}).",
                            pending.name
                        ));
                    }
                }
            }
        }
    }

    /// Show a whisper that came in: from the player the game shows in its slot under
    /// the name its claim was made with, never from one blocked here.
    fn show_received_whisper(&mut self, whisper: Received) {
        let Some(session) = self.live_session.as_ref() else {
            return;
        };
        let game_state = session.game_state();
        let own = u16::try_from(game_state.client_num).ok();
        let slot = u16::from(whisper.slot);
        let Some(shown) = player_identity::shown_name(game_state, usize::from(whisper.slot))
            .filter(|shown| sjk_identity::names_match(&whisper.claimed_name, shown))
        else {
            return;
        };
        // Blocked here for the map or by staff (`muted_players.rs`) as well as muted on
        // this PC: no console line, no taskbar flash and no sound, as for game chat.
        if Some(slot) == own
            || crate::player_mutes::is_muted(Some(&whisper.key_id), &shown)
            || self.muted_players.contains(slot)
        {
            return;
        }
        let server = session.server();
        let Some(console) = self.console.as_ref() else {
            return;
        };
        let directory = console.config_directory().to_owned();
        let sound = crate::chat_sounds::enabled(Some(console), crate::chat_sounds::Kind::Private);
        let text = match whisper.text {
            Ok(opened) => {
                let text = shown_text(&opened.text);
                self.e2e_whispers.came(
                    &whisper.key_id,
                    server,
                    sjk_identity::ReportedWhisper {
                        from: whisper.from.clone(),
                        time: opened.time,
                        text: opened.text,
                        signature: opened.signature,
                    },
                    Instant::now(),
                );
                text
            }
            Err(failure) => {
                let why = match failure {
                    OpenError::Unreadable => "it was not sealed to your key",
                    OpenError::Forged => "it was not signed by them",
                    OpenError::Stale => "it is too old, or a clock is wrong",
                };
                if let Some(console) = self.console.as_mut() {
                    console.push_log(format!(
                        "^3An encrypted whisper from {shown}^3 was thrown away: {why}."
                    ));
                }
                return;
            }
        };
        let changed = self
            .e2e_whispers
            .pin(&directory, &whisper.key_id, &whisper.from)
            == PinCheck::Changed;
        let checked = self
            .e2e_whispers
            .checked(&directory, &whisper.key_id, &whisper.from);
        if !self.chat.receive_encrypted(
            Some(slot),
            (slot, ""),
            &text,
            checked == Checked::Yes,
            Instant::now(),
        ) {
            return;
        }
        if let Some(console) = self.console.as_mut() {
            if checked == Checked::Changed {
                console.push_log(format!(
                    "^1Encrypted whispers: {shown}^1's key changed since you verified them. \
                     Compare the safety code again ({VERIFY_COMMAND}) before trusting it."
                ));
            } else if changed {
                console.push_log(key_changed_line(&shown));
            }
            let line = format!("^6{shown}^6 (encrypted): {text}");
            console.push_chat_line(line.clone(), &line, None);
            console.note_private_message();
        }
        if sound {
            crate::audio::ui_cues::post(crate::audio::ui_cues::Cue::EncryptedWhisper);
        }
    }

    /// Show the card over the game menu, opening it when it is not.
    fn open_e2e_whisper_card(&mut self) {
        let menu_visible = self
            .client_menu
            .as_ref()
            .is_some_and(crate::menu::ClientMenu::is_visible);
        if self.chat.is_typing() {
            self.chat.close_composer();
        }
        if !menu_visible && self.live_session.is_some() && !self.game_menu {
            self.release_pointer();
            self.game_menu = true;
            self.game_menu_page = crate::ingame_menu::Page::Main;
            self.game_menu_row = 0;
            self.e2e_whispers.opened_menu = true;
        }
        self.sync_cursor_policy();
    }

    /// Whether the card shows this frame: a failure to answer, and the player on the
    /// game menu with nothing else over it. `covered` is another card or the console.
    pub(crate) fn prepare_e2e_whisper_card(&mut self, covered: bool) -> bool {
        let console_open = self
            .console
            .as_ref()
            .is_some_and(crate::console::ViewerConsole::is_open);
        let menu_visible = self
            .client_menu
            .as_ref()
            .is_some_and(crate::menu::ClientMenu::is_visible);
        let open = self.e2e_whispers.card.wants()
            && (self.game_menu || menu_visible)
            && !covered
            && !console_open
            && !self.text_dialog.is_open()
            && self.world_load_task.is_none()
            && self.world_install_task.is_none()
            && !self.pending_map_reload;
        self.e2e_whispers.card.set_open(open);
        open
    }

    /// Draw the card. It darkens everything, so every font batch appended before it is
    /// dropped (text draws above all shapes).
    pub(crate) fn append_e2e_whisper_card(&mut self, viewport: [f32; 2]) {
        self.text_vertices.clear();
        self.classic_text_vertices.clear();
        self.game_fonts.clear_text();
        let target = crate::ingame_menu::sjk_view::text_target(
            &mut self.game_fonts,
            &mut self.text_vertices,
            &self.ui_font,
        );
        self.e2e_whispers.card.append(target, viewport);
    }

    /// Act on the player's choice: send the whisper unencrypted or not at all, or mark a
    /// player verified or not. The game menu the card opened closes once nothing is left.
    pub(crate) fn e2e_whisper_card_choice(&mut self, choice: Option<Choice>) {
        let Some(choice) = choice else {
            return;
        };
        let directory = self
            .console
            .as_ref()
            .map(|console| console.config_directory().to_owned());
        match (choice, directory) {
            (Choice::SendPlain(failure), _) => {
                self.send_plain_chat_command(&failure.command, Some(failure.name));
            }
            (Choice::Verify(safety), Some(directory)) => {
                self.e2e_whispers
                    .set_checked(&directory, &safety.key_id, Some(&safety.key));
                if let Some(console) = self.console.as_mut() {
                    console.push_log(format!(
                        "Encrypted whispers: {}^7 verified on this PC.",
                        safety.name
                    ));
                }
            }
            (Choice::Unverify(safety), Some(directory)) => {
                self.e2e_whispers
                    .set_checked(&directory, &safety.key_id, None);
            }
            _ => {}
        }
        if !self.e2e_whispers.card.wants() && std::mem::take(&mut self.e2e_whispers.opened_menu) {
            self.game_menu = false;
            self.sync_cursor_policy();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proof(text: &str) -> sjk_identity::ReportedWhisper {
        sjk_identity::ReportedWhisper {
            from: "a2V5".to_owned(),
            time: 1_000,
            text: text.to_owned(),
            signature: [1; 64],
        }
    }

    #[test]
    fn a_player_not_verified_answers_for_a_while_and_reports_for_a_day() {
        let server: SocketAddr = "1.2.3.4:29070".parse().unwrap();
        let elsewhere: SocketAddr = "1.2.3.4:29071".parse().unwrap();
        let mut state = State::default();
        let start = Instant::now();
        assert!(!state.may_whisper("fox", false, start), "never whispered");
        assert!(state.may_whisper("fox", true, start), "verified: anyone");
        state.came("fox", server, proof("buy credits"), start);
        assert!(state.may_whisper("fox", false, start));
        assert!(!state.may_whisper("wolf", false, start));
        assert_eq!(state.reportable_ids(server), ["fox"]);
        assert!(state.reportable_ids(elsewhere).is_empty());
        assert_eq!(
            state.reportable("fox", server).map(|w| w.text.as_str()),
            Some("buy credits")
        );
        let generation = state.generation;
        let later = start + ANSWER_WINDOW;
        state.expire(later);
        assert!(!state.may_whisper("fox", false, later), "the window closed");
        assert!(
            state.reportable("fox", server).is_some(),
            "still reportable"
        );
        assert_ne!(
            state.generation, generation,
            "the chat's slots are found again"
        );
        state.expire(start + REPORT_WINDOW);
        assert!(state.reportable("fox", server).is_none());
    }

    #[test]
    fn jof_servers_keep_whispers_unless_the_player_says_otherwise() {
        let jof = sjk_mod::Server {
            kind: sjk_mod::ServerKind::JaPlus,
            address: "127.0.0.1:29070".into(),
            info: r"\V\2.5B0".into(),
        };
        let plain = plain_family(jof.family()).expect("JoF");
        assert_eq!(plain.cvar, sjk_mod_jof::E2E_WHISPERS);
        let other = sjk_mod::Server {
            info: r"\V\2.4B7".into(),
            ..jof
        };
        assert!(plain_family(other.family()).is_none());
        for plain in PLAIN_FAMILIES {
            let family = sjk_mod::KNOWN_SERVERS
                .iter()
                .find(|family| family.id == plain.family)
                .unwrap_or_else(|| panic!("{} is no known family", plain.family));
            assert_eq!(family.name, plain.name);
        }
    }

    #[test]
    fn the_text_is_what_follows_tell_and_its_slot() {
        assert_eq!(
            whisper_text("tell 7 \"^7hello there\""),
            Some((7, "^7hello there".to_owned()))
        );
        assert_eq!(
            whisper_text("tell 7 hello there"),
            Some((7, "hello there".to_owned()))
        );
        assert_eq!(whisper_text("TELL 12 x"), Some((12, "x".to_owned())));
        // A backslash is text, as for the server, so these stay encrypted.
        assert_eq!(
            whisper_text("tell 7 \"hi\\\""),
            Some((7, "hi\\".to_owned()))
        );
        assert_eq!(
            whisper_text("tell 7 \"a\\nb \\\\ c\""),
            Some((7, "a\\nb \\\\ c".to_owned()))
        );
        for other in ["say hi", "tell bob hi", "tell 7", "tell 7 \"  \""] {
            assert_eq!(whisper_text(other), None, "{other}");
        }
    }

    /// Encrypted whispers among the game's chat lines, and the card when one fails, over
    /// duel6, rendered off screen at 1080p: `JKA_GAME_DATA=<GameData> cargo test -p
    /// sjk-viewer encrypted_whisper_shots -- --ignored --nocapture`.
    #[test]
    #[ignore = "renders with the GPU and the installed game data named by JKA_GAME_DATA"]
    fn encrypted_whisper_shots() {
        crate::world_shot::on_big_stack(|| {
            let menu = crate::menu::ClientMenu::new(true, String::new());
            let cvars = [
                ("ui_menuStyle", "sjk"),
                (crate::settings::quick::HIDE_CVAR, "1"),
                ("cg_chatbox", "60000"),
                ("cg_chatboxlines", "8"),
            ];
            let Some((mut gpu, _profile)) =
                crate::world_shot::open("maps/mp/duel6.bsp", [1920, 1080], Some(menu), &cvars)
            else {
                return;
            };
            let _ = crate::world_shot::frame(&mut gpu, 10);
            // The card, when the hub could not take a whisper.
            gpu.e2e_whispers.card.preview = true;
            gpu.e2e_whispers.card.offer(Failure {
                server: "127.0.0.1:29070".parse().unwrap(),
                name: "^5Creyon".to_owned(),
                text: "on my way, only you can read this".to_owned(),
                command: "tell 4 \"on my way\"".to_owned(),
                why: "the SJK hub could not be reached".to_owned(),
            });
            let path = crate::world_shot::shoot(&mut gpu, 6, "encrypted-whisper-card");
            println!("{}", path.display());
            assert!(gpu.e2e_whispers.card.is_open(), "the card shows");
            // A safety code to compare, from the chat's player card.
            gpu.e2e_whispers.card.clear();
            let safety = |checked| Safety {
                server: "127.0.0.1:29070".parse().unwrap(),
                name: "^5Creyon".to_owned(),
                key_id: "c0ffee00c0ffee01".to_owned(),
                key: String::new(),
                code: safety_code(&[1; 32], &[2; 32]),
                checked,
            };
            gpu.e2e_whispers.card.show_safety(safety(Checked::No));
            let path = crate::world_shot::shoot(&mut gpu, 6, "encrypted-whisper-safety");
            println!("{}", path.display());
            gpu.e2e_whispers.card.clear();
            gpu.e2e_whispers.card.show_safety(safety(Checked::Changed));
            let path = crate::world_shot::shoot(&mut gpu, 6, "encrypted-whisper-key-changed");
            println!("{}", path.display());
        });
        crate::world_shot::on_big_stack(|| {
            let Some((mut gpu, _profile)) = crate::world_shot::open(
                "maps/mp/duel6.bsp",
                [1920, 1080],
                None,
                &[("cg_chatbox", "60000"), ("cg_chatboxlines", "8")],
            ) else {
                return;
            };
            gpu.chat.for_shot = true;
            let mut game = sjk_protocol::GameState::empty_local(0);
            for (slot, name) in [(0, "^1Sol^7Vulpes"), (4, "^5Creyon"), (6, "Padawan^1Fox")] {
                game.replace_config_string(1131 + slot, format!("n\\{name}\\t\\0").into_bytes())
                    .unwrap();
            }
            gpu.chat.update_roster(&game);
            // The hub knows Creyon (verified, an emoji) and Fox (new).
            let mut marks = vec![crate::chat::marks::SlotMark::default(); crate::chat_mutes::SLOTS];
            marks[4] = crate::chat::marks::SlotMark {
                sjk: true,
                verified: true,
                emoji: "🦊".to_owned(),
                ..Default::default()
            };
            marks[6] = crate::chat::marks::SlotMark {
                sjk: true,
                trust: Some(sjk_identity::Trust::New),
                ..Default::default()
            };
            gpu.chat.set_slot_marks(marks);
            let now = Instant::now();
            gpu.chat.receive(
                sjk_client::ServerEventKind::Chat,
                "Padawan^1Fox^7: ^2gg".to_owned(),
                Some(6),
                now,
            );
            gpu.chat.receive(
                sjk_client::ServerEventKind::Chat,
                "\u{19}[Padawan^1Fox^7\u{19}]: ^6a plain whisper, through the server".to_owned(),
                Some(6),
                now,
            );
            gpu.chat.receive_encrypted(
                Some(4),
                (4, ""),
                "meet me at the bridge, bring the hilt",
                true,
                now,
            );
            gpu.chat.receive_encrypted(
                None,
                (4, "^5Creyon"),
                "on my way, only you can read this",
                false,
                now,
            );
            let path = crate::world_shot::shoot(&mut gpu, 10, "encrypted-whispers-chat");
            println!("{}", path.display());
            // A whisper to Fox, who does not take encrypted ones: the red note.
            gpu.chat.set_encrypted_slots(1 << 4);
            assert!(gpu.chat.whisper_to(Some(6)));
            let path = crate::world_shot::shoot(&mut gpu, 10, "encrypted-whispers-plain-note");
            println!("{}", path.display());
        });
    }

    #[test]
    fn keys_are_remembered_in_the_settings_folder() {
        let directory = tempfile::tempdir().unwrap();
        let key = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8";
        let other = "HxweHRwbGhkYFxYVFBMSERAPDg0MCwoJCAcGBQQDAgE";
        let mut state = State::default();
        assert_eq!(state.pin(directory.path(), "fox", key), PinCheck::First);
        // Read back by another run.
        let mut later = State::default();
        assert_eq!(later.pin(directory.path(), "fox", key), PinCheck::Known);
        assert_eq!(later.pin(directory.path(), "fox", other), PinCheck::Changed);
        assert!(
            std::fs::read_to_string(directory.path().join(PINS_FILE))
                .unwrap()
                .contains(other)
        );
    }
}
