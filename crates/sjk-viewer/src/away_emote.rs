//! Away emote (`docs/hub-chat.md`, "Away emote"): while the player is away from the
//! game (its window alt-tabbed out or minimised, or no input for a while), the emote
//! they play goes quiet for the other SJK players, or one they picked in Settings >
//! Sound > Emotes starts: a JA+ emote on a JA+ server, else a looping SJK emote,
//! quiet from the start. Coming back undoes it: the music plays again from where it
//! is, the away emote ends (a JA+ one by crouching while it still holds the player,
//! as a player gets up from it: [`shape_command`]).
//!
//! The hub takes one emote every 2 seconds from a key, so a post waits for its turn
//! ([`player_identity::emote_ready`]); one still waiting when the player comes back
//! was never seen, and neither it nor its undoing is sent.

use crate::emotes::{self, EmoteDef};
use crate::player_identity;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The SJK emote to play while away, an emote id or [`NONE`].
pub(crate) const SJK_CVAR: &str = "cg_awayEmote";
/// The JA+ emote to play while away on a JA+ server, one of [`JAPLUS_EMOTES`].
/// Core-owned but JA+'s, so under the `japlus.` prefix (`docs/mods.md`).
pub(crate) const JAPLUS_CVAR: &str = "japlus.awayEmote";
/// Its name before the `japlus.` prefix, kept as an alias for saved profiles.
pub(crate) const JAPLUS_OLD_CVAR: &str = "cg_awayEmoteJaplus";
/// Whether an alt-tabbed out or minimised window counts as away.
pub(crate) const ALT_TAB_CVAR: &str = "cg_awayAltTab";
/// Seconds without input before the player is away; 0 never.
pub(crate) const IDLE_CVAR: &str = "cg_awayIdle";
/// [`IDLE_CVAR`]'s default: the scoreboard's away mark waits as long.
pub(crate) const DEFAULT_IDLE: i64 = 180;
/// No away emote; never an emote id.
pub(crate) const NONE: &str = "none";
/// The JA+ emotes that hold the player until they crouch: the ones worth leaving on
/// while away.
pub(crate) const JAPLUS_EMOTES: [&str; 11] = [
    NONE,
    "amsit",
    "amsit2",
    "amsit3",
    "amsit4",
    "amsit5",
    "amkneel",
    "amhips",
    "amwait",
    "amsurrender",
    "amneo",
];
/// `PM_FREEZE`: a JA+ emote holds the player.
const PM_FREEZE: u8 = 6;
/// The longest the player crouches to get up from a JA+ emote; it ends sooner,
/// once the emote lets go of them.
const CROUCH_FOR: Duration = Duration::from_secs(1);
/// `up_move` held down (`cardboard_box.rs`'s crouch).
const CROUCH: i8 = -127;

/// What one change of the player's presence does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// Silence the emote the player plays, for everyone.
    Quiet,
    /// Its sound again.
    Loud,
    /// Start this SJK emote, quiet.
    Start(String),
    /// End it.
    Stop,
    /// Send this JA+ emote to the server.
    Japlus(String),
    /// Crouch, which ends a JA+ emote.
    Crouch,
}

/// What going away did, undone on the way back.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Did {
    #[default]
    Nothing,
    Quieted,
    Started(String),
    Japlus(String),
}

/// The player's presence and the hub post waiting for its turn.
#[derive(Debug, Default)]
pub(crate) struct Away {
    away: bool,
    did: Did,
    /// An emote id to post and whether it is quiet.
    pending: Option<(String, bool)>,
}

impl Away {
    /// The player is `away` now, playing `own` (the id of their SJK emote, if any);
    /// `japlus` is the JA+ emote to play (on a JA+ server only), `sjk` the SJK one.
    /// `held` is whether a JA+ emote holds the player already (`PM_FREEZE`): one they
    /// chose themselves (`amsit5`, `ammeditate`), which going away leaves alone, as
    /// another emote over it, and the crouch that ends that one on the way back, would
    /// get them up (hub report #196). `riding` is whether the player rides a vehicle
    /// (`m_iVehicleNum` set): going away then starts neither away emote, as an emote
    /// over a rider breaks the vehicle's animations and attacks until they get off and
    /// on again (hub report #205, as `amwon` on a bike in EternalJK), and so the way
    /// back has nothing to undo.
    pub(crate) fn turn(
        &mut self,
        away: bool,
        own: Option<&str>,
        japlus: Option<&str>,
        sjk: Option<&str>,
        held: bool,
        riding: bool,
    ) -> Option<Step> {
        if away == self.away {
            return None;
        }
        self.away = away;
        if away {
            let (did, step) = if own.is_some() {
                (Did::Quieted, Step::Quiet)
            } else if held || riding {
                self.did = Did::Nothing;
                return None;
            } else if let Some(japlus) = japlus {
                (
                    Did::Japlus(japlus.to_owned()),
                    Step::Japlus(japlus.to_owned()),
                )
            } else {
                let sjk = sjk?;
                (Did::Started(sjk.to_owned()), Step::Start(sjk.to_owned()))
            };
            self.did = did;
            return Some(step);
        }
        match std::mem::take(&mut self.did) {
            Did::Nothing => None,
            Did::Quieted => own.is_some().then_some(Step::Loud),
            Did::Started(id) => (own == Some(id.as_str())).then_some(Step::Stop),
            Did::Japlus(_) => Some(Step::Crouch),
        }
    }

    /// Post `id` (quiet or not) at the hub's next turn; a post still waiting is undone
    /// by this one, as presence only ever goes away and back, so neither goes.
    pub(crate) fn post(&mut self, id: &str, quiet: bool) {
        if self.pending.take().is_none() {
            self.pending = Some((id.to_owned(), quiet));
        }
    }

    /// The post to send now that the hub takes one.
    fn due(&mut self, ready: bool) -> Option<(String, bool)> {
        ready.then(|| self.pending.take()).flatten()
    }
}

static AWAY: Mutex<Option<Away>> = Mutex::new(None);
/// Until when the commands sent crouch ([`Step::Crouch`]).
static CROUCH_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);

/// Crouch in `command` while the player gets up from a JA+ away emote.
pub(crate) fn shape_command(command: &mut sjk_protocol::UserCommand) {
    let until = *CROUCH_UNTIL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if until.is_some_and(|until| Instant::now() < until) {
        command.up_move = CROUCH;
    }
}

fn crouch_until(until: Option<Instant>) {
    *CROUCH_UNTIL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = until;
}

/// Whether the player's own client counts them away now (alt-tabbed, or idle past
/// `cg_awayEmoteIdle`), as the away emote reads it.
pub(crate) fn own_away() -> bool {
    AWAY.lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
        .is_some_and(|away| away.away)
}

/// Whether the player is away: their window is out of focus or minimised (with
/// `alt_tab`), or they gave no input for `idle` (zero never). Idle time does not make
/// a player away while their own looping emote plays (`dancing`), unless they are
/// away already (`was_away`, the away emote being that emote): a dance is left
/// running without a key pressed on purpose, and going away silenced its music for
/// everyone (hub report #107, Foshin).
pub(crate) fn is_away(
    unfocused: bool,
    alt_tab: bool,
    since_input: Option<Duration>,
    idle: Duration,
    dancing: bool,
    was_away: bool,
) -> bool {
    (alt_tab && unfocused)
        || ((!dancing || was_away)
            && !idle.is_zero()
            && since_input.is_some_and(|since| since >= idle))
}

/// Whether an alt-tabbed out or minimised window (`unfocused`) silences the emote the
/// player plays (`emoting`), for everyone and here: always, whatever [`ALT_TAB_CVAR`]
/// says (Sol's rule, 23/10/2026: nobody hears music a player left running behind
/// another window). [`ALT_TAB_CVAR`] only decides whether alt-tabbing out with no emote
/// playing starts the away emote.
pub(crate) fn alt_tab_quiets(unfocused: bool, emoting: bool) -> bool {
    unfocused && emoting
}

/// The SJK emotes [`SJK_CVAR`] may name: [`NONE`] then the catalogue's looping ones
/// sent to the hub, made once for each catalogue (the settings list wants them for
/// the program's life).
pub(crate) fn sjk_choices() -> &'static [&'static str] {
    static CHOICES: Mutex<Option<(usize, &'static [&'static str])>> = Mutex::new(None);
    let ids: Vec<String> = emotes::lock()
        .catalogue
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter(|def| fits(def))
        .map(|def| def.id.clone())
        .collect();
    let mut cached = CHOICES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let key = ids.iter().map(String::len).sum::<usize>() + ids.len() * 1_000;
    if let Some((cached_key, choices)) = *cached
        && cached_key == key
    {
        return choices;
    }
    let choices: Vec<&'static str> = std::iter::once(NONE)
        .chain(ids.into_iter().map(|id| &*Box::leak(id.into_boxed_str())))
        .collect();
    let choices: &'static [&'static str] = Box::leak(choices.into_boxed_slice());
    *cached = Some((key, choices));
    choices
}

/// Whether `def` may be the away emote: it loops, and the others see it.
fn fits(def: &EmoteDef) -> bool {
    def.looping && !def.local
}

/// What the settings show for choice `value` of `cvar`: an SJK emote's name, a JA+
/// emote's command.
pub(crate) fn choice_name(cvar: &str, value: &str) -> Option<&'static str> {
    if value.eq_ignore_ascii_case(NONE)
        && (cvar.eq_ignore_ascii_case(SJK_CVAR) || cvar.eq_ignore_ascii_case(JAPLUS_CVAR))
    {
        return Some("None");
    }
    if !cvar.eq_ignore_ascii_case(SJK_CVAR) {
        return None;
    }
    let name = emotes::lock()
        .catalogue
        .as_deref()?
        .iter()
        .find(|def| def.id == value)?
        .name
        .clone();
    static NAMES: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    let mut names = NAMES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(known) = names.iter().find(|known| **known == name) {
        return Some(known);
    }
    let name: &'static str = Box::leak(name.into_boxed_str());
    names.push(name);
    Some(name)
}

impl crate::GpuState {
    /// Follow the player's presence, and send the post waiting for the hub's turn.
    pub(crate) fn update_away_emote(&mut self) {
        let now = Instant::now();
        let mut slot = AWAY.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let (Some(console), Some(session)) = (self.console.as_ref(), self.live_session.as_mut())
        else {
            *slot = None;
            return;
        };
        let away_state = slot.get_or_insert_with(Away::default);
        let player = &session.latest_snapshot().player;
        let own = u8::try_from(player.client_num()).ok();
        let playing = !session.is_local() && !player.is_spectator();
        let frozen = player.movement_type() == PM_FREEZE;
        let riding = player.vehicle_entity_num() != 0;
        // Up from the JA+ emote: the crouch is over.
        if !frozen {
            crouch_until(None);
        }
        let unfocused = console.bool_cvar("com_unfocused") == Some(true)
            || console.bool_cvar("com_minimized") == Some(true);
        let idle = Duration::from_secs(
            console
                .integer_cvar(IDLE_CVAR)
                .unwrap_or(DEFAULT_IDLE)
                .clamp(0, 3_600) as u64,
        );
        let pick = |cvar: &str| {
            console
                .text_value(cvar)
                .map(str::trim)
                .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case(NONE))
                .map(str::to_ascii_lowercase)
        };
        let ja_plus = session
            .game_state()
            .config_string(0)
            .and_then(|info| sjk_client::LegacyClientInfo::new(info).text("gamename"))
            .is_some_and(sjk_protocol::is_ja_plus_game_name);
        let japlus = pick(JAPLUS_CVAR)
            .filter(|command| ja_plus && JAPLUS_EMOTES.contains(&command.as_str()));
        let mut state = emotes::lock();
        let def_of = |state: &emotes::State, id: &str| {
            state
                .catalogue
                .as_deref()
                .unwrap_or_default()
                .iter()
                .find(|def| def.id == id)
                .cloned()
        };
        let dancing = own
            .and_then(|own| state.active.get(own))
            .and_then(|active| def_of(&state, &active.emote))
            .is_some_and(|def| def.looping);
        let current = own
            .and_then(|own| state.active.get(own))
            .map(|active| active.emote.clone());
        let away = playing
            && (is_away(
                unfocused,
                console.bool_cvar(ALT_TAB_CVAR) != Some(false),
                crate::holocrons::activity::since_input(now),
                idle,
                dancing,
                away_state.away,
            ) || alt_tab_quiets(unfocused, current.is_some()));
        let sjk = pick(SJK_CVAR).filter(|id| def_of(&state, id).is_some_and(|def| fits(&def)));
        let step = away_state.turn(
            away,
            current.as_deref(),
            japlus.as_deref(),
            sjk.as_deref(),
            frozen,
            riding,
        );
        if let Some(step) = &step {
            crate::emote_end_debug::note_away_step(step);
        }
        if let (Some(step), Some(own)) = (step, own) {
            let def = current.as_deref().and_then(|id| def_of(&state, id));
            match step {
                Step::Quiet => {
                    state.active.set_quiet(own, true, def.as_ref(), now);
                    away_state.post(emotes::QUIET, false);
                }
                Step::Loud => {
                    state.active.set_quiet(own, false, def.as_ref(), now);
                    away_state.post(emotes::LOUD, false);
                }
                Step::Start(id) => {
                    let def = def_of(&state, &id);
                    let origin = self
                        .live_world
                        .entity(sjk_runtime::EntityId::new(u64::from(own) + 1))
                        .map(|entity| entity.current().transform.translation);
                    state.active.start(own, &id, def.as_ref(), origin, now);
                    state.active.set_quiet(own, true, def.as_ref(), now);
                    away_state.post(&id, true);
                }
                Step::Stop => {
                    state.active.stop(own);
                    away_state.post(emotes::STOP, false);
                }
                Step::Japlus(command) => {
                    if let Err(error) = session.send_reliable_command(command.as_bytes()) {
                        eprintln!("failed to send {command}: {error}");
                    }
                }
                // Only while the emote still holds the player: once it let go, a
                // crouch would only duck them.
                Step::Crouch if frozen => crouch_until(Some(now + CROUCH_FOR)),
                Step::Crouch => {}
            }
        }
        drop(state);
        if let Some((id, quiet)) = away_state.due(player_identity::emote_ready(now)) {
            if quiet {
                player_identity::emote_quiet(id);
            } else {
                player_identity::emote(id, None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn going_away_quiets_the_emote_playing_or_starts_the_pick() {
        let mut away = Away::default();
        assert_eq!(
            away.turn(false, None, None, Some("dance"), false, false),
            None,
            "not away"
        );
        assert_eq!(
            away.turn(true, Some("wave"), None, Some("dance"), false, false),
            Some(Step::Quiet)
        );
        assert_eq!(
            away.turn(true, Some("wave"), None, Some("dance"), false, false),
            None,
            "still away"
        );
        assert_eq!(
            away.turn(false, Some("wave"), None, None, false, false),
            Some(Step::Loud)
        );
        // A JA+ pick (on a JA+ server) comes before the SJK one.
        assert_eq!(
            away.turn(true, None, Some("amsit"), Some("dance"), false, false),
            Some(Step::Japlus("amsit".into()))
        );
        assert_eq!(
            away.turn(false, None, None, None, false, false),
            Some(Step::Crouch)
        );
        assert_eq!(
            away.turn(true, None, None, Some("dance"), false, false),
            Some(Step::Start("dance".into()))
        );
        assert_eq!(
            away.turn(false, Some("dance"), None, None, false, false),
            Some(Step::Stop)
        );
        assert_eq!(
            away.turn(true, None, None, None, false, false),
            None,
            "nothing picked"
        );
        assert_eq!(away.turn(false, None, None, None, false, false), None);
    }

    /// Hub report #196: a JA+ emote the player chose (`amsit5`, `ammeditate`) is left
    /// alone when they go away: no away emote over it, and no crouch on the way back.
    #[test]
    fn going_away_leaves_a_held_ja_plus_emote_alone() {
        let mut away = Away::default();
        assert_eq!(
            away.turn(true, None, Some("amsit"), Some("dance"), true, false),
            None
        );
        assert_eq!(
            away.turn(false, None, None, None, true, false),
            None,
            "no crouch"
        );
        // Not held, the away emote starts and is got up from as before.
        assert_eq!(
            away.turn(true, None, Some("amsit"), None, false, false),
            Some(Step::Japlus("amsit".into()))
        );
        assert_eq!(
            away.turn(false, None, None, None, true, false),
            Some(Step::Crouch),
            "held by the away emote itself"
        );
        // The player's own SJK emote still goes quiet, held or not.
        assert_eq!(
            away.turn(true, Some("wave"), None, None, true, false),
            Some(Step::Quiet)
        );
    }

    /// Hub report #205: going away on a vehicle starts neither away emote, which broke
    /// the vehicle's animations and attacks until the rider got off and on again, and
    /// the way back undoes nothing (no crouch, no stop).
    #[test]
    fn going_away_on_a_vehicle_starts_no_away_emote() {
        let mut away = Away::default();
        assert_eq!(
            away.turn(true, None, Some("amsit"), Some("dance"), false, true),
            None,
            "no JA+ away emote on a vehicle"
        );
        assert_eq!(
            away.turn(false, None, None, None, false, true),
            None,
            "no crouch on the way back"
        );
        assert_eq!(
            away.turn(true, None, None, Some("dance"), false, true),
            None,
            "no SJK away emote on a vehicle"
        );
        assert_eq!(
            away.turn(false, None, None, None, false, false),
            None,
            "off it since, still nothing to undo"
        );
        // The player's own emote still goes quiet and loud again on a vehicle.
        assert_eq!(
            away.turn(true, Some("wave"), Some("amsit"), None, false, true),
            Some(Step::Quiet)
        );
        assert_eq!(
            away.turn(false, Some("wave"), None, None, false, true),
            Some(Step::Loud)
        );
        // On foot again, the away emote starts as before.
        assert_eq!(
            away.turn(true, None, Some("amsit"), None, false, false),
            Some(Step::Japlus("amsit".into()))
        );
    }

    #[test]
    fn coming_back_leaves_alone_what_already_ended() {
        let mut away = Away::default();
        away.turn(true, None, None, Some("dance"), false, false);
        assert_eq!(
            away.turn(false, Some("wave"), None, None, false, false),
            None,
            "another emote now"
        );
        away.turn(true, Some("wave"), None, None, false, false);
        assert_eq!(
            away.turn(false, None, None, None, false, false),
            None,
            "it ended meanwhile"
        );
    }

    #[test]
    fn a_post_still_waiting_is_dropped_with_its_undoing() {
        let mut away = Away::default();
        away.post("dance", true);
        assert_eq!(away.due(false), None, "the hub's gap");
        away.post(emotes::STOP, false);
        assert_eq!(away.due(true), None, "never seen, never undone");
        away.post(emotes::QUIET, false);
        assert_eq!(away.due(true), Some((emotes::QUIET.to_owned(), false)));
        assert_eq!(away.due(true), None);
    }

    #[test]
    fn away_is_alt_tabbed_out_or_idle_long_enough() {
        let idle = Duration::from_secs(180);
        assert!(is_away(true, true, None, idle, false, false));
        assert!(
            !is_away(true, false, None, idle, false, false),
            "alt-tab not counted"
        );
        assert!(is_away(false, false, Some(idle), idle, false, false));
        assert!(!is_away(false, true, Some(idle / 2), idle, false, false));
        assert!(
            !is_away(false, true, Some(idle * 10), Duration::ZERO, false, false),
            "idle off"
        );
        assert!(
            !is_away(false, true, None, idle, false, false),
            "no input yet"
        );
    }

    /// Sol's rule (23/10/2026): alt-tabbing out always silences the emote playing, even
    /// with `cg_awayAltTab` off, which then starts no away emote; coming back plays it
    /// again.
    #[test]
    fn alt_tabbing_out_always_quiets_the_emote_playing() {
        let idle = Duration::from_secs(180);
        let alt_tab_off = |unfocused: bool, emoting: bool| {
            is_away(unfocused, false, None, idle, emoting, false)
                || alt_tab_quiets(unfocused, emoting)
        };
        let mut away = Away::default();
        assert!(alt_tab_off(true, true));
        assert_eq!(
            away.turn(
                alt_tab_off(true, true),
                Some("dance"),
                None,
                Some("sit"),
                false,
                false
            ),
            Some(Step::Quiet)
        );
        assert_eq!(
            away.turn(
                alt_tab_off(false, true),
                Some("dance"),
                None,
                Some("sit"),
                false,
                false
            ),
            Some(Step::Loud)
        );
        assert!(
            !alt_tab_off(true, false),
            "no emote and alt-tab off: no away emote starts"
        );
        assert!(!alt_tab_quiets(false, true), "in focus");
    }

    /// Hub report #107: a dance left running is not idle, an alt-tab still counts, and
    /// the away emote (a dance too) keeps an idle player away.
    #[test]
    fn a_dance_left_running_is_not_idle() {
        let idle = Duration::from_secs(180);
        assert!(!is_away(false, true, Some(idle * 2), idle, true, false));
        assert!(is_away(true, true, Some(idle * 2), idle, true, false));
        assert!(is_away(false, true, Some(idle * 2), idle, true, true));
    }
}
