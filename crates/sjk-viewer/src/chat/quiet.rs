//! Quiet mode (Lex Deifaber's idea, 30/10/2026): until it is turned off or the map
//! changes, the chat feed hides public, team, SJK and admin chat and their sounds, but
//! for the player's private duel partner, whispers (server tells and encrypted SJK
//! whispers) and the player's own lines. What no player sent (the server's own chat
//! lines, the SJK team's announcements, refusals) and centre prints still show; the
//! console keeps every line, as it does under `pmonly`.
//!
//! Whether a line passes is decided when it arrives (`ChatLine::loud`), from
//! the duel partner of that moment, so a duel partner's lines stay after the duel and
//! turning quiet mode on hides the history it would have hidden; turning it off shows
//! that history again.
use crate::console::ViewerConsole;

/// `cg_quietMode`: quiet mode on. Not archived: every map starts with it off.
pub(crate) const CVAR: &str = "cg_quietMode";
/// The console command that toggles it (bindable as any command).
pub(crate) const COMMAND: &str = "quiet";

/// `quiet [on|off]`: toggle quiet mode, or set it; says ON or OFF.
pub(crate) fn command(console: &mut ViewerConsole, args: &[String]) -> Result<Vec<String>, String> {
    let value = match args {
        [] => !console.bool_cvar(CVAR).unwrap_or(false),
        [arg] if arg == "1" || arg.eq_ignore_ascii_case("on") => true,
        [arg] if arg == "0" || arg.eq_ignore_ascii_case("off") => false,
        _ => return Err("Usage: quiet [on|off]".into()),
    };
    console.set_cvar(CVAR, if value { "1" } else { "0" });
    Ok(vec![
        if value {
            "Quiet mode ON: only your duel partner and whispers until the map changes."
        } else {
            "Quiet mode OFF."
        }
        .to_owned(),
    ])
}

/// A new map: quiet mode ends. Graphics reloads keep it.
pub(crate) fn reset(console: &mut ViewerConsole) {
    console.set_cvar(CVAR, "0");
}

impl super::ChatOverlay {
    /// Follow the latest snapshot's `player` at `now` (server milliseconds) for the
    /// private duel partner ([`sjk_client::DuelPartner`]): the player's own duel only,
    /// its partner kept a few seconds after the kill for the "gg".
    pub(crate) fn observe_duel(&mut self, player: &sjk_protocol::PlayerState, now: i64) {
        self.duel_partner
            .observe(&self.roster, self.own_slot, player, now);
    }

    /// The player's own SJK identity key (`player_identity::own_key_id`).
    pub(crate) fn set_sjk_own_key(&mut self, key: Option<&str>) {
        if self.sjk_own_key.as_deref() != key {
            self.sjk_own_key = key.map(str::to_owned);
        }
    }

    /// While quiet mode is on, the identity keys whose SJK chat still passes: the
    /// player's own and the duel partner's (as the hub knows their slot); `None` while
    /// it is off.
    pub(crate) fn quiet_keys(&self) -> Option<[String; 2]> {
        self.options.quiet.then(|| {
            [
                self.sjk_own_key.clone().unwrap_or_default(),
                self.partner_key().to_owned(),
            ]
        })
    }

    /// The identity key the hub knows in the duel partner's slot, empty when none.
    fn partner_key(&self) -> &str {
        self.duel_partner
            .target()
            .and_then(|target| self.slot_marks.get(usize::from(target.slot())))
            .map_or("", |mark| mark.key_id.as_str())
    }

    /// Whether an SJK chat line from `key` passes quiet mode: the player's own or the
    /// duel partner's, or the SJK team's (`official`).
    pub(super) fn sjk_line_loud(&self, key: &str, official: bool) -> bool {
        official
            || (!key.is_empty()
                && (self.sjk_own_key.as_deref() == Some(key) || key == self.partner_key()))
    }

    /// Whether `line` shows under quiet mode.
    pub(super) fn quiet_visible(&self, line: &super::ChatLine) -> bool {
        !self.options.quiet || line.loud
    }

    /// Whether quiet mode hides server chat `text` (from `sender`, when the server named
    /// one; `marked` for the server's tell marker), decided as the feed decides it: the
    /// console's alert words leave such a line out.
    pub(crate) fn quiets(&self, text: &str, sender: Option<u16>, marked: bool) -> bool {
        if !self.options.quiet {
            return false;
        }
        let display = sjk_client::chat_display_text(&sjk_client::chat_unescape(text));
        let from = sjk_client::line_sender(&display, sender, &self.roster);
        let whisper = (marked && !sjk_client::forged_tell(text, sender, &self.roster))
            || from
                .target()
                .and_then(|target| self.roster.display_name(target))
                .is_some_and(|name| sjk_client::chat_body(&display, name).1);
        !sjk_client::quiet_passes(whisper, from, self.own_slot, self.duel_partner.target())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::ChatOverlay;
    use crate::chat::marks::SlotMark;
    use sjk_client::ServerEventKind;
    use std::time::Instant;

    #[test]
    fn the_command_toggles_parses_says_on_or_off_and_resets() {
        let directory = tempfile::tempdir().unwrap();
        let mut console = ViewerConsole::new(directory.path().join("config.cfg")).unwrap();
        assert_eq!(console.bool_cvar(CVAR), Some(false));
        let said = command(&mut console, &[]).unwrap();
        assert!(said[0].contains("ON"));
        assert_eq!(console.bool_cvar(CVAR), Some(true));
        assert!(command(&mut console, &["maybe".into()]).is_err());
        assert_eq!(console.bool_cvar(CVAR), Some(true));
        assert!(command(&mut console, &["off".into()]).unwrap()[0].contains("OFF"));
        assert_eq!(console.bool_cvar(CVAR), Some(false));
        command(&mut console, &["1".into()]).unwrap();
        reset(&mut console);
        assert_eq!(console.bool_cvar(CVAR), Some(false));
    }

    fn visible(chat: &ChatOverlay) -> Vec<String> {
        chat.lines
            .iter()
            .filter(|line| chat.shown(line))
            .map(|line| line.body.clone())
            .collect()
    }

    /// A chat overlay for slot 0 with these players in the roster.
    fn overlay(players: &[(usize, &str)]) -> ChatOverlay {
        let mut chat = ChatOverlay::new();
        let mut game = sjk_protocol::GameState::empty_local(0);
        for (slot, name) in players {
            game.replace_config_string(1_131 + slot, format!("n\\{name}\\t\\0").into_bytes())
                .expect("a player");
        }
        chat.update_roster(&game);
        chat
    }

    /// Our own player state, in a private duel with `partner` or in none.
    fn player(partner: Option<u16>) -> sjk_protocol::PlayerState {
        let mut player = sjk_protocol::PlayerState::default();
        player.set_client_num(0);
        player.set_raw_field(119, u32::from(partner.is_some()));
        player.set_raw_field(44, u32::from(partner.unwrap_or(0)));
        player
    }

    #[test]
    fn quiet_mode_hides_public_chat_but_the_partner_and_whispers() {
        let mut chat = overlay(&[
            (3, "Duelist"),
            (4, "Fox"),
            (5, "Padawan"),
            (6, "Padawan"),
            (7, "\u{19}[Bob"),
        ]);
        let now = Instant::now();
        chat.observe_duel(&player(Some(3)), 0);
        for (kind, text, sender, private) in [
            (ServerEventKind::Chat, "Fox: public", Some(4), false),
            (ServerEventKind::TeamChat, "(Fox): team", Some(4), false),
            (ServerEventKind::Chat, "Duelist: gg", Some(3), false),
            (ServerEventKind::Chat, "\u{19}[Fox]: tell", Some(4), true),
            (ServerEventKind::Chat, "Server notice", None, false),
            // Two players wear the name: certainly a player's, so hidden.
            (ServerEventKind::Chat, "Padawan: hi", None, false),
            // A name starting with the tell marker makes a public line look like one.
            (
                ServerEventKind::Chat,
                "\u{19}[Bob^7\u{19}: psst",
                Some(7),
                true,
            ),
        ] {
            chat.receive_classified(kind, text.into(), sender, private, now);
        }
        assert!(chat.receive_encrypted(None, (4, "Fox"), "sealed", false, now));
        assert_eq!(visible(&chat).len(), 8, "all show while it is off");
        chat.options.quiet = true;
        let shown = visible(&chat);
        assert_eq!(shown.len(), 4, "{shown:?}");
        assert!(!shown.iter().any(|body| {
            ["public", "team", "hi", "psst"]
                .iter()
                .any(|w| body.contains(w))
        }));
        assert!(chat.quiets("Padawan: hi", None, false));
        assert!(chat.quiets("\u{19}[Bob^7\u{19}: psst", Some(7), true));
        assert!(!chat.quiets("\u{19}[Fox]: tell", Some(4), true));
        // The kill ends the duel: the partner's "gg" still shows for a few seconds.
        chat.observe_duel(&player(None), 1_000);
        chat.receive_classified(
            ServerEventKind::Chat,
            "Duelist: gg2".into(),
            Some(3),
            false,
            now,
        );
        assert_eq!(visible(&chat).len(), 5);
        chat.observe_duel(&player(None), 1_001 + sjk_client::PARTNER_GRACE_MS);
        chat.receive_classified(
            ServerEventKind::Chat,
            "Duelist: again".into(),
            Some(3),
            false,
            now,
        );
        assert!(chat.quiets("Duelist: again", Some(3), false));
        assert_eq!(visible(&chat).len(), 5);
        chat.options.quiet = false;
        assert_eq!(visible(&chat).len(), 10, "history comes back");
    }

    #[test]
    fn a_followed_players_duel_is_not_ours() {
        let mut chat = overlay(&[(3, "Duelist"), (4, "Fox")]);
        let mut followed = player(Some(3));
        followed.set_client_num(4);
        chat.observe_duel(&followed, 0);
        chat.options.quiet = true;
        assert!(chat.quiets("Duelist: hi", Some(3), false));
        // Our own line is ours by the gamestate's slot, not the followed player's.
        assert!(chat.quiets("Fox: hi", Some(4), false));
    }

    #[test]
    fn sjk_lines_pass_for_the_partners_key_ours_and_the_teams() {
        let mut chat = overlay(&[(2, "Duelist")]);
        assert_eq!(chat.quiet_keys(), None);
        let mark = |key: &str| SlotMark {
            sjk: true,
            verified: false,
            tick: sjk_identity::Tick::None,
            trust: None,
            emoji: String::new(),
            name: None,
            key_id: key.to_owned(),
        };
        // Our own slot's mark is not there yet: our key still counts.
        chat.set_slot_marks(vec![mark(""), mark(""), mark("partner"), mark("other")]);
        chat.set_sjk_own_key(Some("me"));
        chat.observe_duel(&player(Some(2)), 0);
        assert!(chat.sjk_line_loud("me", false));
        assert!(chat.sjk_line_loud("partner", false));
        assert!(chat.sjk_line_loud("", true), "the SJK team's");
        assert!(!chat.sjk_line_loud("other", false));
        assert!(!chat.sjk_line_loud("", false));
        chat.options.quiet = true;
        assert_eq!(
            chat.quiet_keys(),
            Some(["me".to_owned(), "partner".to_owned()])
        );
    }
}
