//! The channels the chat composer offers (29/10/2026): All always,
//! then Team, SJK, Admin and PM, each behind its own switch, so every player sets up the
//! row they use. Admin is JA+'s and jaPRO's `amsay`, a message only the server's admins
//! read (JoF EJK `Cmd_Amsay_f`, `SAY_ADMIN`); it is offered only on a server whose mod
//! has the command, and has its own key ([`ADMIN_COMMAND`]). PM whispers to the last
//! player who whispered you (`chat/reply.rs`). A hidden channel is only left off the row
//! and out of Tab: its key still opens it, and a whisper still opens from a name.
//!
//! Each button is also a tab, as in other games' chat windows (Dave, 29/10/2026): while
//! the composer is on Team, SJK, Admin or a whisper, the feed shows only that channel's
//! messages (whispers both ways, encrypted ones too), and All shows everything; the
//! feed shows everything again once the composer closes. [`FILTER_CVAR`] turns it off.
use super::*;
use crate::console::ViewerConsole;
use crate::text::UiFont;

/// Team chat in the composer's row.
pub(crate) const TEAM_CVAR: &str = "cg_chatTabTeam";
/// SJK chat in the composer's row.
pub(crate) const SJK_CVAR: &str = "cg_chatTabSjk";
/// Admin chat (`amsay`) in the composer's row, on servers that have it.
pub(crate) const ADMIN_CVAR: &str = "cg_chatTabAdmin";
/// A PM button (a whisper to the last whisperer) in the composer's row.
pub(crate) const PM_CVAR: &str = "cg_chatTabPm";
/// The key command that opens the composer on Admin chat, beside `messagemode5` (SJK).
pub(crate) const ADMIN_COMMAND: &str = "messagemode6";
/// Room between two of the row's words, in 1080p pixels at the chat's scale (58 px apart
/// whatever the word before 29/10/2026, which left wide holes after the short ones).
pub(super) const ROW_GAP: f32 = 22.0;
/// The row's text size, in 1080p pixels at the chat's scale.
const LABEL_SIZE: f32 = 14.0;

/// A channel's word in the row: All is the game's say chat.
pub(super) fn label(channel: Channel) -> &'static str {
    match channel {
        Channel::Global => "All",
        Channel::Team => "Team",
        Channel::Sjk => "SJK",
        Channel::Admin => "Admin",
        Channel::Whisper => "PM",
    }
}

/// A channel's button in the row.
pub(super) fn token(channel: Channel) -> u16 {
    match channel {
        Channel::Global => interaction::GLOBAL,
        Channel::Team => interaction::TEAM,
        Channel::Sjk => interaction::SJK,
        Channel::Admin => interaction::ADMIN,
        Channel::Whisper => interaction::PM,
    }
}

/// The composer's channel also filters the feed while it is open.
pub(crate) const FILTER_CVAR: &str = "cg_chatTabFilter";
/// What the composer says when PM is chosen and no one has whispered yet.
pub(super) const NO_PM: &str =
    "No one has whispered you yet: click a player's name to whisper them.";
/// What the composer says when the Admin key is used on a server without admin chat.
pub(super) const NO_ADMIN: &str = "This server has no admin chat (amsay is JA+ and jaPRO's).";

/// Which of the switchable channels the player keeps in the row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Tabs {
    pub(super) team: bool,
    pub(super) sjk: bool,
    pub(super) admin: bool,
    pub(super) pm: bool,
    /// The composer's channel filters the feed ([`FILTER_CVAR`]).
    pub(super) filter: bool,
}

impl Tabs {
    pub(super) fn read(console: Option<&ViewerConsole>) -> Self {
        let on = |name, default| {
            console
                .and_then(|console| console.integer_cvar(name))
                .map_or(default, |value| value != 0)
        };
        Self {
            team: on(TEAM_CVAR, true),
            sjk: on(SJK_CVAR, true),
            admin: on(ADMIN_CVAR, true),
            pm: on(PM_CVAR, false),
            filter: on(FILTER_CVAR, true),
        }
    }
}

/// Whether a server of this kind takes `amsay`: JA+ and jaPRO (and its TaystJK lineage,
/// JoF's servers among them).
pub(crate) fn server_has_admin_chat(kind: sjk_mod::ServerKind) -> bool {
    matches!(
        kind,
        sjk_mod::ServerKind::JaPlus | sjk_mod::ServerKind::JaPro
    )
}

/// Whether a game chat line is jaPRO's admin chat: its name starts `<Admin>` (JoF EJK
/// `G_Say`, `SAY_ADMIN`: `^3<Admin>^7(name)`), colour codes aside.
pub(super) fn is_admin_line(display: &str) -> bool {
    super::options::clean_body(display, 1)
        .trim_start()
        .starts_with("<Admin>(")
}

impl ChatOverlay {
    /// Whether the feed shows `line` now: the PM-only switch (`chat/pm_only.rs`), then
    /// the open composer's tab.
    pub(super) fn shown(&self, line: &ChatLine) -> bool {
        if !self.pm_visible(line) || !self.quiet_visible(line) {
            return false;
        }
        let Some(input) = self.input.as_ref().filter(|_| self.options.tabs.filter) else {
            return true;
        };
        match input.channel {
            Channel::Global => true,
            Channel::Whisper => {
                line.channel == Channel::Whisper || line.encrypted || line.server_private
            }
            channel => line.channel == channel,
        }
    }

    /// The channels the composer's row offers now, in order.
    pub(super) fn offered(&self) -> impl Iterator<Item = Channel> + '_ {
        [
            Channel::Global,
            Channel::Team,
            Channel::Sjk,
            Channel::Admin,
            Channel::Whisper,
        ]
        .into_iter()
        .filter(|channel| self.offers(*channel))
    }

    /// Whether the row offers `channel`.
    pub(super) fn offers(&self, channel: Channel) -> bool {
        let tabs = self.options.tabs;
        match channel {
            Channel::Global => true,
            Channel::Team => tabs.team,
            Channel::Sjk => tabs.sjk,
            Channel::Admin => tabs.admin && self.admin_server,
            Channel::Whisper => tabs.pm,
        }
    }

    /// Say whether the server being played takes `amsay`. Leaving one that does moves a
    /// composer on Admin to All.
    pub(crate) fn set_admin_server(&mut self, admin: bool) {
        self.admin_server = admin;
        if !admin
            && let Some(input) = self.input.as_mut()
            && input.channel == Channel::Admin
        {
            input.channel = Channel::Global;
        }
    }

    /// The channel Tab moves to from `channel`: the next one in the row, then the last
    /// whisperer (`chat/reply.rs`) when the row has no PM button, then All again. `None`
    /// is the last whisperer.
    pub(super) fn next_channel(&self, channel: Channel) -> Option<Channel> {
        let mut row = self.offered().skip_while(|offered| *offered != channel);
        row.next();
        for next in row {
            match next {
                // The PM button stands for the last whisperer, skipped when there is none.
                Channel::Whisper if self.reply_target().is_ok() => return None,
                Channel::Whisper => {}
                next => return Some(next),
            }
        }
        if channel != Channel::Whisper
            && !self.offers(Channel::Whisper)
            && self.reply_target().is_ok()
        {
            return None;
        }
        Some(Channel::Global)
    }

    /// The players who whispered you, newest first, each once, while they are still
    /// in the game: the PM button's choices.
    pub(super) fn whisperers(&self) -> Vec<ChatTarget> {
        let mut found: Vec<ChatTarget> = Vec::new();
        let senders = self.last_whisperer.into_iter().chain(
            self.lines
                .iter()
                .rev()
                .filter(|line| line.channel == Channel::Whisper)
                .filter_map(|line| line.sender),
        );
        for sender in senders {
            if Some(sender.slot()) != self.own_slot
                && self.roster.name(sender).is_some()
                && !found.contains(&sender)
            {
                found.push(sender);
            }
        }
        found
    }

    /// The PM button: a whisper to the last whisperer. Left-clicked again while on one
    /// (`step` 1), the next player who whispered you, round to the first; right-clicked
    /// (`step` -1), the one before, round to the last. With no whisperer, a word on how
    /// to whisper.
    pub(super) fn step_pm(&mut self, step: isize) {
        let whisperers = self.whisperers();
        if whisperers.is_empty() {
            self.notice = NO_PM;
            return;
        }
        let Some(input) = self.input.as_mut() else {
            return;
        };
        let current = match (input.channel, input.recipient) {
            (Channel::Whisper, Some(current)) => {
                whisperers.iter().position(|target| *target == current)
            }
            _ => None,
        };
        let count = whisperers.len() as isize;
        let at = current.map_or(0, |at| (at as isize + step).rem_euclid(count) as usize);
        input.channel = Channel::Whisper;
        input.recipient = Some(whisperers[at]);
        self.notice = "";
    }

    /// Where the composer's whisper stands among the players who whispered you, as
    /// "2/3", when more than one did and it is one of them.
    pub(super) fn pm_place(&self) -> Option<(usize, usize)> {
        let recipient = self.input.as_ref()?.recipient?;
        let whisperers = self.whisperers();
        (whisperers.len() > 1)
            .then(|| whisperers.iter().position(|target| *target == recipient))
            .flatten()
            .map(|at| (at + 1, whisperers.len()))
    }

    /// Open the composer on Admin chat for its key, whether or not its button is in the
    /// row; on a server without it, on All with a word why.
    pub(crate) fn open_admin(&mut self) {
        self.open(false);
        if self.admin_server {
            if let Some(input) = self.input.as_mut() {
                input.channel = Channel::Admin;
            }
        } else {
            self.notice = NO_ADMIN;
        }
    }

    /// The row as drawn: each offered channel with its left edge and its word's width,
    /// in 1080p pixels at the chat's scale, and where the row ends (the whisper's
    /// "To ..." and the SJK chat's red note start there).
    pub(super) fn row_layout(&self, font: &UiFont) -> ([(Channel, f32, f32); 5], usize, f32) {
        let mut row = [(Channel::Global, 0.0, 0.0); 5];
        let mut count = 0;
        let mut x = 0.0;
        for channel in self.offered() {
            let width = crate::text::visible_text_width_face(
                font,
                label(channel),
                LABEL_SIZE / font.height,
                crate::text::TextFace::Semibold,
            );
            row[count] = (channel, x, width);
            count += 1;
            x += width + ROW_GAP;
        }
        (row, count, x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sjk_ui::{InputEvent, PointerButton};

    fn chat(team: bool, sjk: bool, admin: bool, server: bool) -> ChatOverlay {
        let mut chat = ChatOverlay::new();
        chat.options.tabs = Tabs {
            team,
            sjk,
            admin,
            pm: false,
            filter: true,
        };
        chat.admin_server = server;
        chat
    }

    /// The labels the open composer draws in its row, after a frame.
    fn drawn(chat: &mut ChatOverlay) -> Vec<String> {
        chat.build(true, &crate::text::test_font(), [1920.0, 1080.0], 1_000);
        ["All", "Team", "SJK", "Admin", "PM"]
            .into_iter()
            .filter(|label| chat.ui.text_runs().any(|text| text == *label))
            .map(str::to_owned)
            .collect()
    }

    fn click(chat: &mut ChatOverlay, token: u16) {
        press(chat, token, PointerButton::Primary);
    }

    fn press(chat: &mut ChatOverlay, token: u16, button: PointerButton) {
        chat.build(true, &crate::text::test_font(), [1920.0, 1080.0], 1_000);
        let rect = chat.ui.rect_for(token).expect("a button");
        let at = sjk_ui::Vec2::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5);
        chat.pointer(InputEvent::PointerMove(at));
        chat.pointer(InputEvent::PointerPress {
            position: at,
            button,
        });
        chat.pointer(InputEvent::PointerRelease {
            position: at,
            button,
        });
    }

    #[test]
    fn the_row_keeps_all_and_the_channels_switched_on() {
        let every = chat(true, true, true, true);
        assert_eq!(
            every.offered().collect::<Vec<_>>(),
            [Channel::Global, Channel::Team, Channel::Sjk, Channel::Admin]
        );
        // A row without SJK or Team chat.
        let lean = chat(false, false, true, true);
        assert_eq!(
            lean.offered().collect::<Vec<_>>(),
            [Channel::Global, Channel::Admin]
        );
        let font = crate::text::test_font();
        let (_, _, lean_end) = lean.row_layout(&font);
        let (row, count, every_end) = every.row_layout(&font);
        assert!(lean_end < every_end);
        // An even gap between the words, whatever their length.
        for pair in row[..count].windows(2) {
            assert!((pair[1].1 - (pair[0].1 + pair[0].2) - ROW_GAP).abs() < 0.01);
        }
        // Admin only where the server takes amsay.
        let base = chat(true, true, true, false);
        assert!(!base.offers(Channel::Admin));
        assert_eq!(base.row_layout(&font).1, 3);
    }

    #[test]
    fn the_settings_switch_the_drawn_buttons() {
        let directory = tempfile::tempdir().unwrap();
        let mut console = ViewerConsole::new(directory.path().join("config.cfg")).unwrap();
        let mut chat = ChatOverlay::new();
        chat.set_admin_server(true);
        chat.configure(Some(&console));
        chat.open(false);
        assert_eq!(drawn(&mut chat), ["All", "Team", "SJK", "Admin"]);
        assert!(console.set_cvar(TEAM_CVAR, "0"));
        assert!(console.set_cvar(SJK_CVAR, "0"));
        assert!(console.set_cvar(PM_CVAR, "1"));
        chat.configure(Some(&console));
        assert_eq!(drawn(&mut chat), ["All", "Admin", "PM"]);
        assert!(console.set_cvar(ADMIN_CVAR, "0"));
        chat.configure(Some(&console));
        assert_eq!(drawn(&mut chat), ["All", "PM"]);
    }

    #[test]
    fn the_admin_button_can_be_clicked() {
        let mut chat = chat(true, true, true, true);
        chat.open(false);
        click(&mut chat, interaction::ADMIN);
        assert_eq!(chat.input.as_ref().unwrap().channel, Channel::Admin);
        click(&mut chat, interaction::TEAM);
        assert_eq!(chat.input.as_ref().unwrap().channel, Channel::Team);
    }

    #[test]
    fn the_pm_button_whispers_the_last_whisperer_or_says_how() {
        let mut chat = chat(true, true, true, true);
        chat.options.tabs.pm = true;
        chat.open(false);
        click(&mut chat, interaction::PM);
        assert_eq!(chat.input.as_ref().unwrap().channel, Channel::Global);
        assert_eq!(chat.notice, NO_PM);
    }

    #[test]
    fn tab_walks_only_the_offered_channels() {
        let lean = chat(false, true, true, true);
        assert_eq!(lean.next_channel(Channel::Global), Some(Channel::Sjk));
        assert_eq!(lean.next_channel(Channel::Sjk), Some(Channel::Admin));
        assert_eq!(lean.next_channel(Channel::Admin), Some(Channel::Global));
        // The team key opened Team though it is hidden: Tab goes back to All.
        assert_eq!(lean.next_channel(Channel::Team), Some(Channel::Global));
        assert_eq!(lean.next_channel(Channel::Whisper), Some(Channel::Global));
    }

    #[test]
    fn the_admin_key_opens_admin_chat_even_with_its_button_hidden() {
        let mut hidden = chat(true, true, false, true);
        hidden.open_admin();
        assert_eq!(hidden.input.as_ref().unwrap().channel, Channel::Admin);
        hidden.set_admin_server(true);
        assert_eq!(hidden.input.as_ref().unwrap().channel, Channel::Admin);
        // Not on a server without it.
        let mut base = chat(true, true, true, false);
        base.open_admin();
        assert_eq!(base.input.as_ref().unwrap().channel, Channel::Global);
        assert_eq!(base.notice, NO_ADMIN);
    }

    #[test]
    fn the_admin_key_is_a_bindable_control() {
        assert!(crate::input::GameplayInput::recognizes(ADMIN_COMMAND));
    }

    #[test]
    fn the_switches_default_on_but_pm_and_read_zero_as_off() {
        assert_eq!(
            Tabs::read(None),
            Tabs {
                team: true,
                sjk: true,
                admin: true,
                pm: false,
                filter: true
            }
        );
    }

    #[test]
    fn admin_chat_is_offered_on_ja_plus_and_japro_servers() {
        use sjk_mod::ServerKind;
        assert!(server_has_admin_chat(ServerKind::JaPlus));
        assert!(server_has_admin_chat(ServerKind::JaPro));
        assert!(!server_has_admin_chat(ServerKind::BaseJka));
        assert!(!server_has_admin_chat(ServerKind::Other));
    }

    #[test]
    fn leaving_an_admin_server_moves_the_composer_to_all() {
        let mut chat = chat(true, true, true, true);
        chat.open(false);
        chat.input.as_mut().unwrap().channel = Channel::Admin;
        chat.set_admin_server(false);
        assert_eq!(chat.input.as_ref().unwrap().channel, Channel::Global);
    }

    #[test]
    fn admin_chat_goes_out_as_amsay() {
        assert_eq!(
            sjk_client::chat_command(sjk_client::ChatDestination::Admin, "need help \"here\"")
                .as_deref(),
            Some("amsay \"need help ''here''\"")
        );
        let mut chat = chat(true, true, true, true);
        chat.open(false);
        chat.choose(Channel::Admin);
        chat.input.as_mut().unwrap().text = "hi admins".to_owned();
        assert!(matches!(
            chat.submit(),
            ChatInputResult::Submit(command) if command == "amsay \"hi admins\""
        ));
    }

    fn feed(chat: &mut ChatOverlay) -> Vec<String> {
        chat.build(true, &crate::text::test_font(), [1920.0, 1080.0], 1_000);
        ["public", "teamwords", "adminwords", "whisperwords"]
            .into_iter()
            .filter(|word| chat.ui.text_runs().any(|text| text.contains(word)))
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn each_tab_shows_only_its_own_messages_while_typing() {
        let mut chat = chat(true, true, true, true);
        chat.options.lines = 10;
        let now = std::time::Instant::now();
        for (kind, text, private) in [
            (sjk_client::ServerEventKind::Chat, "Fox: public", false),
            (
                sjk_client::ServerEventKind::TeamChat,
                "(Fox): teamwords",
                false,
            ),
            (
                sjk_client::ServerEventKind::Chat,
                "^3<Admin>^7(Fox^7)^7: ^2adminwords",
                false,
            ),
            (
                sjk_client::ServerEventKind::Chat,
                "\u{19}[Fox]: whisperwords",
                true,
            ),
        ] {
            chat.receive_classified(kind, text.to_owned(), None, private, now);
        }
        assert!(chat.lines.iter().any(|line| line.channel == Channel::Admin));
        // Closed: everything, as before.
        assert_eq!(feed(&mut chat).len(), 4);
        chat.open(false);
        assert_eq!(feed(&mut chat).len(), 4, "All shows everything");
        chat.choose(Channel::Team);
        assert_eq!(feed(&mut chat), ["teamwords"]);
        chat.choose(Channel::Admin);
        assert_eq!(feed(&mut chat), ["adminwords"]);
        chat.input.as_mut().unwrap().channel = Channel::Whisper;
        assert_eq!(feed(&mut chat), ["whisperwords"]);
        // The switch off: the tabs only choose where a message goes.
        chat.options.tabs.filter = false;
        assert_eq!(feed(&mut chat).len(), 4);
    }

    #[test]
    fn admin_lines_are_told_from_their_admin_name() {
        assert!(is_admin_line("^3<Admin>^7(Fox^7)^7: ^2hi"));
        assert!(!is_admin_line("Fox: <Admin>(nope)"));
        assert!(!is_admin_line("Fox: hi"));
    }

    #[test]
    fn the_pm_button_steps_through_everyone_who_whispered() {
        let mut chat = chat(true, true, true, true);
        chat.options.tabs.pm = true;
        let mut game = sjk_protocol::GameState::empty_local(0);
        for (slot, name) in [(3usize, "Fox"), (4, "Wolf"), (5, "Bear")] {
            game.replace_config_string(1_131 + slot, format!("n\\{name}\\t\\0").into_bytes())
                .expect("a player");
        }
        chat.update_roster(&game);
        let now = std::time::Instant::now();
        for (name, slot) in [("Fox", 3u16), ("Wolf", 4), ("Bear", 5)] {
            chat.receive_classified(
                sjk_client::ServerEventKind::Chat,
                format!("\u{19}[{name}\u{19}]: hi"),
                Some(slot),
                true,
                now,
            );
        }
        let names = |chat: &ChatOverlay| {
            chat.whisperers()
                .iter()
                .map(|target| chat.roster.name(*target).unwrap().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&chat), ["Bear", "Wolf", "Fox"], "newest first");
        chat.open(false);
        let recipient = |chat: &ChatOverlay| {
            let target = chat.input.as_ref().unwrap().recipient.unwrap();
            chat.roster.name(target).unwrap().to_owned()
        };
        chat.step_pm(1);
        assert_eq!(recipient(&chat), "Bear");
        assert_eq!(chat.pm_place(), Some((1, 3)));
        chat.step_pm(1);
        assert_eq!(recipient(&chat), "Wolf");
        chat.step_pm(1);
        assert_eq!(recipient(&chat), "Fox");
        chat.step_pm(1);
        assert_eq!(recipient(&chat), "Bear", "round to the first");
        // A right click goes back, round to the last.
        chat.step_pm(-1);
        assert_eq!(recipient(&chat), "Fox");
        chat.step_pm(-1);
        assert_eq!(recipient(&chat), "Wolf");
        // The same with the mouse on the PM button.
        click(&mut chat, interaction::PM);
        assert_eq!(recipient(&chat), "Fox");
        press(&mut chat, interaction::PM, PointerButton::Secondary);
        assert_eq!(recipient(&chat), "Wolf");
    }

    #[test]
    fn choosing_a_tab_with_the_pointer_still_on_it_opens_no_card() {
        let mut chat = chat(true, true, true, true);
        chat.options.lines = 10;
        let mut game = sjk_protocol::GameState::empty_local(0);
        for (slot, name) in [(1usize, "ArdanHett"), (2, "Padawan")] {
            game.replace_config_string(1_131 + slot, format!("n\\{name}\\t\\0").into_bytes())
                .expect("a player");
        }
        chat.update_roster(&game);
        let now = std::time::Instant::now();
        for text in ["ArdanHett: hi", "Padawan: yo", "Padawan: there"] {
            chat.receive_classified(
                sjk_client::ServerEventKind::Chat,
                text.to_owned(),
                None,
                false,
                now,
            );
        }
        chat.open(false);
        // Team hides them all; then All, clicked where the pointer rests.
        click(&mut chat, interaction::TEAM);
        click(&mut chat, interaction::GLOBAL);
        let font = crate::text::test_font();
        for ms in [1_100, 1_200, 1_500] {
            chat.build(true, &font, [1920.0, 1080.0], ms);
            assert!(chat.card.is_none(), "no player's card at {ms}");
        }
    }
}
