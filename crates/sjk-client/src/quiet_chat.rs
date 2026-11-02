//! What quiet mode needs to know of a chat line (the viewer's `chat/quiet.rs`): who
//! sent it, told apart from a line nobody or several players could have sent, whether
//! it is a genuine whisper, and the private duel partner whose chat still passes. The
//! chat feed and the chat beeps (`local_sounds.rs`) share these rules, so a hidden
//! line never beeps and a shown one always may.

use crate::chat::{ChatRoster, ChatTarget, chat_body};
use sjk_protocol::PlayerState;

const MAX_CLIENTS: u16 = 32;

/// How long a private duel partner's chat still passes after the duel ended
/// (`duelInProgress` clears at the kill, before the "gg").
pub const PARTNER_GRACE_MS: i64 = 10_000;

/// Who sent a chat line the server did not attribute, by the names that start it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineSender {
    /// No player's name starts it: the server's own line.
    Nobody,
    /// Exactly this player's.
    One(ChatTarget),
    /// Several players' names fit (two "Padawan", or a copied name): a player's line,
    /// but not whose.
    Ambiguous,
}

impl LineSender {
    /// The one sender, when there is one.
    pub fn target(self) -> Option<ChatTarget> {
        match self {
            Self::One(target) => Some(target),
            _ => None,
        }
    }
}

/// The sender of a line as the feed attributes it: the slot the server named (a slot
/// with nobody in it still names a player), else the one player whose name, as the
/// game shows it, starts `display` (colour codes and the team's or a whisper's
/// brackets aside, [`chat_body`]).
pub fn line_sender(display: &str, sender: Option<u16>, roster: &ChatRoster) -> LineSender {
    if sender.is_some() {
        return roster
            .target(sender)
            .map_or(LineSender::Ambiguous, LineSender::One);
    }
    let mut found = LineSender::Nobody;
    for target in (0..MAX_CLIENTS).filter_map(|slot| roster.target(Some(slot))) {
        let Some(name) = roster.display_name(target).filter(|name| !name.is_empty()) else {
            continue;
        };
        if chat_body(display, name).0.len() < display.len() {
            if found != LineSender::Nobody {
                return LineSender::Ambiguous;
            }
            found = LineSender::One(target);
        }
    }
    found
}

/// Whether `text` (as the server sent it, with its 0x19 separators) only looks like a
/// tell: it starts with the private marker `\x19[` because a player's *name* does, and
/// it is that player's public line (`<name>^7\x19: `) rather than a tell from anyone
/// (`\x19[<name>^7\x19]`). The candidates are the slot the server named, else every
/// player. An unknown roster leaves the marker trusted.
pub fn forged_tell(text: &str, sender: Option<u16>, roster: &ChatRoster) -> bool {
    if !text.starts_with("\u{19}[") {
        return false;
    }
    let slots = match sender {
        Some(slot) => slot..slot.saturating_add(1),
        None => 0..MAX_CLIENTS,
    };
    let mut public = false;
    for slot in slots {
        let Some(name) = roster
            .target(Some(slot))
            .and_then(|target| roster.display_name(target))
            .filter(|name| !name.is_empty())
        else {
            continue;
        };
        if tell_of(text, name) {
            return false;
        }
        public |= said_by(text, name);
    }
    public
}

/// The rest of `text` after one colour code (`^7`), if it starts with one.
fn after_colour(text: &str) -> &str {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some('^'), Some(_)) => chars.as_str(),
        _ => text,
    }
}

/// `G_Say`'s tell from `name`: `\x19[<name>^7\x19]`.
fn tell_of(text: &str, name: &str) -> bool {
    text.strip_prefix("\u{19}[")
        .and_then(|rest| rest.strip_prefix(name))
        .map(after_colour)
        .is_some_and(|rest| rest.starts_with("\u{19}]") || rest.starts_with(']'))
}

/// `G_Say`'s public line from `name`: `<name>^7\x19: `.
fn said_by(text: &str, name: &str) -> bool {
    text.strip_prefix(name)
        .map(after_colour)
        .is_some_and(|rest| rest.starts_with("\u{19}:") || rest.starts_with(':'))
}

/// Whether a line passes quiet mode: a whisper, the server's own line (nobody's name
/// starts it), or one from the player (`own`) or their duel partner. A line several
/// players could have sent is a player's, so it is hidden.
pub fn quiet_passes(
    private: bool,
    sender: LineSender,
    own: Option<u16>,
    partner: Option<ChatTarget>,
) -> bool {
    match sender {
        _ if private => true,
        LineSender::Nobody => true,
        LineSender::Ambiguous => false,
        LineSender::One(target) => Some(target.slot()) == own || Some(target) == partner,
    }
}

/// The private duel partner, kept for [`PARTNER_GRACE_MS`] after the duel ends.
#[derive(Clone, Copy, Debug, Default)]
pub struct DuelPartner {
    target: Option<ChatTarget>,
    /// When the duel was last seen over (milliseconds on the caller's clock).
    ended_at: Option<i64>,
}

impl DuelPartner {
    /// Follow the latest snapshot's `player` at `now`: only the player's own state
    /// (`own`, the gamestate's client number) counts, not a followed player's. A new
    /// duel takes its partner; the last one is let go when they leave or rename (their
    /// roster observation changes) or the grace has passed.
    pub fn observe(
        &mut self,
        roster: &ChatRoster,
        own: Option<u16>,
        player: &PlayerState,
        now: i64,
    ) {
        let own_view = own == Some(player.client_num());
        if own_view && player.duel_in_progress() {
            self.target = roster.target(Some(player.duel_index()));
            self.ended_at = None;
            return;
        }
        let Some(target) = self.target else {
            return;
        };
        let ended = *self.ended_at.get_or_insert(now);
        if roster.display_name(target).is_none() || now.saturating_sub(ended) > PARTNER_GRACE_MS {
            *self = Self::default();
        }
    }

    /// The partner whose chat passes now.
    pub fn target(&self) -> Option<ChatTarget> {
        self.target
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sjk_protocol::GameState;

    fn roster(players: &[(usize, &str)]) -> (ChatRoster, GameState) {
        let mut game = GameState::empty_local(0);
        for (slot, name) in players {
            game.replace_config_string(1_131 + slot, format!("n\\{name}\\t\\0").into_bytes())
                .expect("a player");
        }
        let mut roster = ChatRoster::default();
        roster.update(&game);
        (roster, game)
    }

    #[test]
    fn a_line_two_names_fit_is_ambiguous_and_one_nobodys_the_servers() {
        let (roster, _) = roster(&[(1, "Padawan"), (2, "Padawan"), (3, "Fox")]);
        assert_eq!(
            line_sender("Padawan: hi", None, &roster),
            LineSender::Ambiguous
        );
        assert!(matches!(
            line_sender("Fox: hi", None, &roster),
            LineSender::One(target) if target.slot() == 3
        ));
        assert_eq!(
            line_sender("Server: restart soon", None, &roster),
            LineSender::Nobody
        );
        assert!(matches!(
            line_sender("Padawan: hi", Some(2), &roster),
            LineSender::One(target) if target.slot() == 2
        ));
        assert_eq!(line_sender("hi", Some(9), &roster), LineSender::Ambiguous);
        assert!(!quiet_passes(false, LineSender::Ambiguous, Some(0), None));
        assert!(quiet_passes(false, LineSender::Nobody, Some(0), None));
        assert!(quiet_passes(true, LineSender::Ambiguous, Some(0), None));
    }

    #[test]
    fn a_name_starting_with_the_marker_makes_no_tell() {
        let (roster, _) = roster(&[(1, "\u{19}[Bob"), (2, "Fox")]);
        // Bob's public line, as G_Say writes it.
        let public = "\u{19}[Bob^7\u{19}: psst";
        assert!(forged_tell(public, Some(1), &roster));
        assert!(forged_tell(public, None, &roster));
        // Fox's real tell, and one from an unknown roster.
        let tell = "\u{19}[Fox^7\u{19}]\u{19}: psst";
        assert!(!forged_tell(tell, Some(2), &roster));
        assert!(!forged_tell(tell, None, &roster));
        assert!(!forged_tell(tell, None, &ChatRoster::default()));
        assert!(!forged_tell("Fox^7\u{19}: hi", Some(2), &roster));
    }

    #[test]
    fn the_partner_stays_a_while_after_the_duel_and_only_our_own_duel_counts() {
        let (mut roster, mut game) = roster(&[(3, "Duelist"), (4, "Fox")]);
        let mut player = PlayerState::default();
        player.set_client_num(0);
        player.set_raw_field(119, 1);
        player.set_raw_field(44, 3);
        let mut partner = DuelPartner::default();
        partner.observe(&roster, Some(0), &player, 0);
        let duelist = roster.target(Some(3));
        assert_eq!(partner.target(), duelist);
        // The kill ends the duel: they still pass for the grace.
        player.set_raw_field(119, 0);
        partner.observe(&roster, Some(0), &player, 1_000);
        partner.observe(&roster, Some(0), &player, 1_000 + PARTNER_GRACE_MS);
        assert_eq!(partner.target(), duelist);
        partner.observe(&roster, Some(0), &player, 2_001 + PARTNER_GRACE_MS);
        assert_eq!(partner.target(), None);
        // Following another player in a duel names no partner of ours.
        let mut followed = player.clone();
        followed.set_client_num(4);
        followed.set_raw_field(119, 1);
        partner.observe(&roster, Some(0), &followed, 0);
        assert_eq!(partner.target(), None);
        // A partner who renames (or whose slot another takes) is let go.
        player.set_raw_field(119, 1);
        partner.observe(&roster, Some(0), &player, 0);
        player.set_raw_field(119, 0);
        game.replace_config_string(1_131 + 3, b"n\\Other\\t\\0".to_vec())
            .expect("a player");
        roster.update(&game);
        partner.observe(&roster, Some(0), &player, 10);
        assert_eq!(partner.target(), None);
    }
}
