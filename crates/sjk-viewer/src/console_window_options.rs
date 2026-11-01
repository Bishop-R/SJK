//! Window events, cached frame-cap settings (the focus-dependent caps, the menus' cap
//! and whether AUTO follows the monitor) and the away talk balloon; no simulation policy.
use super::console_cvars::IntegerSetting;
use super::*;

/// `com_maxfpsUnfocused` default: a window without focus draws at 30 FPS, so an
/// alt-tabbed client does not keep a 4K frame rate. EternalJK leaves it at 0.
const UNFOCUSED_CAP: i64 = 30;

/// Retired archived window preferences, discarded after loading old profiles.
const RETIRED_CVARS: &[&str] = &[
    // #331, 01/11/2026: the requested single unfocused switch covers both away
    // states, so an old minimized value must not survive as a phantom setting.
    "cl_minimizedChatbox",
];

/// Remove retired user-created entries without changing the surviving preference.
pub(super) fn discard_retired(cvars: &mut CvarRegistry) {
    for name in RETIRED_CVARS {
        let _ = cvars.unset(name);
    }
}

/// Window preferences retained across world installs with the console.
pub(super) struct Options {
    unfocused_cap: IntegerSetting,
    minimized_cap: IntegerSetting,
    /// [`crate::runtime_settings::MONITOR_CAP_CVAR`], read every frame.
    monitor_cap: IntegerSetting,
    /// [`crate::runtime_settings::MENU_CAP_CVAR`], read every frame.
    menu_cap: IntegerSetting,
    unfocused_chatbox: IntegerSetting,
    mute_away: IntegerSetting,
    unfocused: bool,
    minimized: bool,
    alt: bool,
    applied: Option<(bool, bool)>,
    attention: bool,
}

impl Options {
    /// Register state separately from archived preferences before config loading.
    pub(super) fn register(cvars: &mut CvarRegistry) -> Result<Self, sjk_shell::CvarError> {
        for (name, default, help) in [
            (
                "com_maxfpsUnfocused",
                UNFOCUSED_CAP,
                "Unfocused frame cap; zero uses the normal cap",
            ),
            (
                "com_maxfpsMinimized",
                50,
                "Minimized frame cap; zero uses the normal cap",
            ),
            (
                crate::runtime_settings::MONITOR_CAP_CVAR,
                0,
                "1: com_maxfps -1 (AUTO) follows the monitor's refresh rate (125 when \
                 unknown); 0: AUTO caps at 125 without reading the monitor",
            ),
            (
                crate::runtime_settings::MENU_CAP_CVAR,
                0,
                "Menu frame cap while no game or demo runs, never above com_maxfps; zero uses the normal cap",
            ),
            (
                "cl_unfocusedChatbox",
                1,
                "Show your automatic chat balloon while the game window is unfocused or minimized",
            ),
            (
                "snd_mute_losefocus",
                1,
                "Mute all sound while the game window is unfocused or minimised",
            ),
        ] {
            cvars.register(CvarDefinition::new(name, default, CvarFlags::ARCHIVE, help))?;
        }
        for (name, default, help) in [
            (
                "com_unfocused",
                false,
                "Native window has no keyboard focus",
            ),
            ("com_minimized", false, "Native window is minimized"),
        ] {
            cvars.register(CvarDefinition::new(
                name,
                default,
                CvarFlags::READ_ONLY,
                help,
            ))?;
        }
        for (name, default, help) in [
            ("cl_allowAltEnter", true, "Alt+Enter toggles fullscreen"),
            ("r_noborder", false, "Hide window decorations"),
            ("r_centerWindow", false, "Center window on its monitor"),
            (
                "con_notifyconnect",
                false,
                "Request attention for server connection notices",
            ),
            ("con_notifyvote", true, "Request attention for vote notices"),
        ] {
            cvars.register(CvarDefinition::new(name, default, CvarFlags::ARCHIVE, help))?;
        }
        cvars.register(CvarDefinition::new(
            "con_notifywords",
            "0",
            CvarFlags::ARCHIVE,
            "Attention words (space separated); -1 any chat, 0 disabled",
        ))?;
        Ok(Self {
            unfocused_cap: IntegerSetting::bind(cvars, "com_maxfpsUnfocused", UNFOCUSED_CAP)?,
            minimized_cap: IntegerSetting::bind(cvars, "com_maxfpsMinimized", 50)?,
            monitor_cap: IntegerSetting::bind(cvars, crate::runtime_settings::MONITOR_CAP_CVAR, 0)?,
            menu_cap: IntegerSetting::bind(cvars, crate::runtime_settings::MENU_CAP_CVAR, 0)?,
            unfocused_chatbox: IntegerSetting::bind(cvars, "cl_unfocusedChatbox", 1)?,
            mute_away: IntegerSetting::bind(cvars, "snd_mute_losefocus", 1)?,
            unfocused: false,
            minimized: false,
            alt: false,
            applied: None,
            attention: false,
        })
    }
}

impl ViewerConsole {
    /// Publish focus/minimize events without persisting engine state.
    pub(crate) fn window_state(&mut self, focused: Option<bool>, minimized: Option<bool>) {
        if let Some(focused) = focused {
            self.window_options.unfocused = !focused;
            if !focused {
                self.window_options.alt = false;
            }
            self.window_options.attention = false;
            let _ = self
                .shell
                .cvars
                .restore_text("com_unfocused", if focused { "0" } else { "1" });
        }
        if let Some(minimized) = minimized {
            self.window_options.minimized = minimized;
            let _ = self
                .shell
                .cvars
                .restore_text("com_minimized", if minimized { "1" } else { "0" });
        }
    }

    /// Whether `com_maxfps -1` (AUTO) follows the monitor's refresh rate
    /// ([`crate::runtime_settings::MONITOR_CAP_CVAR`]), without a name lookup.
    pub(crate) fn auto_fps_follows_monitor(&self) -> bool {
        self.window_options.monitor_cap.enabled()
    }

    /// The cap while only the menus run (`menus`: no game, demo or server connection,
    /// `GpuState::menus_only`): `normal`, lowered to `com_maxfpsMenu` when that is set
    /// ([`menu_cap`]). Read from a change callback, no name lookup.
    pub(crate) fn menu_fps(&self, normal: u32, menus: bool) -> u32 {
        menu_cap(normal, self.window_options.menu_cap.value(), menus)
    }

    /// Stock common.cpp:1528-1533 priority, using callbacks rather than frame lookups.
    pub(crate) fn window_fps(&self, normal: u32) -> u32 {
        let options = &self.window_options;
        let cap = if options.minimized && options.minimized_cap.value() > 0 {
            options.minimized_cap.value()
        } else if options.unfocused && options.unfocused_cap.value() > 0 {
            options.unfocused_cap.value()
        } else {
            return normal;
        };
        cap.min(i64::from(u32::MAX)) as u32
    }

    /// Whether commands carry the automatic away talk flag. One archived switch,
    /// `cl_unfocusedChatbox` (default 1), covers unfocused and minimized windows.
    pub(crate) fn window_talk(&self) -> bool {
        let options = &self.window_options;
        away_talk(
            options.unfocused,
            options.minimized,
            options.unfocused_chatbox.value(),
        )
    }

    /// Whether every sound is silenced because the window is away: EternalJK's
    /// `snd_mute_losefocus` (on by default), which pauses the sound device while
    /// the window is unfocused or minimised (`codemp/client/snd_dma.cpp`).
    pub(crate) fn window_muted(&self) -> bool {
        let options = &self.window_options;
        options.mute_away.enabled() && (options.unfocused || options.minimized)
    }

    /// Whether the window is away: unfocused (alt-tabbed out) or minimised. The unlock
    /// cards and the large pop-ups' entrances wait for the player to come back.
    pub(crate) fn window_away(&self) -> bool {
        self.window_options.unfocused || self.window_options.minimized
    }

    /// A private message arrived (a server's tell or an encrypted whisper, not from a
    /// player muted here): flash the taskbar while the window is away, unless
    /// `cl_privateChatFlash` is off (`chat_sounds.rs`). Coming back stops it.
    pub(crate) fn note_private_message(&mut self) {
        let setting = self.bool_cvar(crate::chat_sounds::FLASH_CVAR);
        self.window_options.attention |= private_flash(self.window_away(), setting);
    }

    /// Update modifier state from the same native event used by console shortcuts.
    pub(crate) fn window_alt(&mut self, alt: bool) {
        self.window_options.alt = alt;
    }

    /// Consume Alt+Enter before it reaches gameplay bindings, ignoring key repeat.
    pub(crate) fn window_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        use winit::event::ElementState;
        use winit::keyboard::{KeyCode, PhysicalKey};
        self.alt_enter(
            event.physical_key == PhysicalKey::Code(KeyCode::Enter),
            event.state == ElementState::Pressed,
            event.repeat,
        )
    }

    fn alt_enter(&mut self, enter: bool, pressed: bool, repeat: bool) -> bool {
        if !enter || !self.window_options.alt || !self.bool_cvar("cl_allowaltenter").unwrap_or(true)
        {
            return false;
        }
        if pressed && !repeat {
            let fullscreen = self.bool_cvar("r_fullscreen").unwrap_or(false);
            self.set_cvar("r_fullscreen", if fullscreen { "0" } else { "1" });
        }
        true
    }

    /// Apply changed decoration/centering preferences, never reposition every frame.
    pub(crate) fn apply_window_options(&mut self, window: &winit::window::Window) {
        if std::mem::take(&mut self.window_options.attention) {
            window.request_user_attention(Some(winit::window::UserAttentionType::Informational));
        }
        let (borderless, center) = self.window_preferences();
        if self.window_options.applied == Some((borderless, center)) {
            return;
        }
        window.set_decorations(!borderless);
        if center && window.fullscreen().is_none() {
            if let Some(monitor) = window.current_monitor() {
                let origin = monitor.position();
                let screen = monitor.size();
                let size = window.outer_size();
                window.set_outer_position(winit::dpi::PhysicalPosition::new(
                    origin.x + (i64::from(screen.width) - i64::from(size.width)) as i32 / 2,
                    origin.y + (i64::from(screen.height) - i64::from(size.height)) as i32 / 2,
                ));
            }
        }
        self.window_options.applied = Some((borderless, center));
    }

    fn window_preferences(&self) -> (bool, bool) {
        (
            self.bool_cvar("r_noborder").unwrap_or(false),
            self.bool_cvar("r_centerwindow").unwrap_or(false),
        )
    }

    /// Consume raw server text before localization removes notification tokens.
    pub(crate) fn notify_server_text(&mut self, kind: &sjk_client::ServerEventKind, text: &str) {
        use sjk_client::ServerEventKind;
        let alert = match kind {
            ServerEventKind::Print => {
                (self.bool_cvar("con_notifyconnect").unwrap_or(false)
                    && [
                        "@@@PLCONNECT",
                        "@@@DISCONNECT",
                        "@@@WAS_KICKED",
                        "timed out",
                    ]
                    .iter()
                    .any(|token| contains_ascii(text, token)))
                    || (self.bool_cvar("con_notifyvote").unwrap_or(true)
                        && contains_ascii(text, "@@@PLCALLEDVOTE"))
            }
            ServerEventKind::Chat | ServerEventKind::TeamChat => {
                let words = self.text_value("con_notifywords").unwrap_or("0");
                words == "-1"
                    || (words != "0"
                        && text.rsplit_once(':').is_some_and(|(_, body)| {
                            words
                                .split_ascii_whitespace()
                                .take(8)
                                .any(|word| contains_ascii(body, word))
                        }))
            }
            _ => false,
        };
        self.window_options.attention |= alert && self.window_options.unfocused;
    }
}

fn contains_ascii(text: &str, word: &str) -> bool {
    !word.is_empty()
        && text
            .as_bytes()
            .windows(word.len())
            .any(|part| part.eq_ignore_ascii_case(word.as_bytes()))
}

/// The menus' cap (`com_maxfpsMenu`, SJK's own, for weak PCs): while only the menus
/// run (`menus`), `menu` frames a second when it is above 0, never more than the
/// `normal` cap (0 is uncapped, so any menu cap lowers it); else, or at 0 (the default),
/// `normal` unchanged.
fn menu_cap(normal: u32, menu: i64, menus: bool) -> u32 {
    if !menus || menu <= 0 {
        return normal;
    }
    let menu = menu.min(i64::from(u32::MAX)) as u32;
    if normal == 0 { menu } else { normal.min(menu) }
}

/// Whether a private message flashes the taskbar: the window away (unfocused or
/// minimised) and the flash not switched off.
fn private_flash(away: bool, setting: Option<bool>) -> bool {
    away && setting != Some(false)
}

/// One switch covers either away state, including a minimized event before focus loss.
fn away_talk(unfocused: bool, minimized: bool, enabled: i64) -> bool {
    (unfocused || minimized) && enabled != 0
}

#[cfg(test)]
mod private_flash_tests {
    use super::private_flash;
    use crate::console::ViewerConsole;

    #[test]
    fn a_private_message_flashes_only_an_away_window_with_the_flash_on() {
        assert!(private_flash(true, Some(true)));
        assert!(private_flash(true, None));
        assert!(!private_flash(true, Some(false)));
        assert!(!private_flash(false, Some(true)));
        let directory = tempfile::tempdir().unwrap();
        let mut console = ViewerConsole::new(directory.path().join("config.cfg")).unwrap();
        console.note_private_message();
        assert!(!console.window_options.attention, "focused: no flash");
        console.window_state(Some(false), None);
        console.note_private_message();
        assert!(console.window_options.attention, "on by default");
        // Coming back clears it; with the flash off nothing asks again.
        console.window_state(Some(true), None);
        assert!(!console.window_options.attention);
        console.window_state(Some(false), None);
        assert!(console.set_cvar(crate::chat_sounds::FLASH_CVAR, "0"));
        console.note_private_message();
        assert!(!console.window_options.attention);
    }
}

#[cfg(test)]
mod menu_cap_tests {
    use super::menu_cap;
    use crate::console::ViewerConsole;

    #[test]
    fn the_menu_cap_lowers_the_normal_cap_only_in_the_menus() {
        // Off (0, the default) or in a game: the normal cap, uncapped included.
        for normal in [0, 60, 125, 240] {
            assert_eq!(menu_cap(normal, 0, true), normal);
            assert_eq!(menu_cap(normal, 30, false), normal);
            assert_eq!(menu_cap(normal, -5, true), normal);
        }
        // In the menus: the menu cap, never above the normal one.
        assert_eq!(menu_cap(125, 30, true), 30);
        assert_eq!(menu_cap(125, 60, true), 60);
        assert_eq!(menu_cap(60, 144, true), 60);
        assert_eq!(menu_cap(0, 60, true), 60, "uncapped is lowered too");
        assert_eq!(menu_cap(0, i64::MAX, true), u32::MAX);
    }

    /// New profiles get 0, so nothing changes; the setting is archived, applies in the
    /// menus only, and the unfocused and minimized caps still win when they apply.
    #[test]
    fn off_by_default_archived_and_under_the_window_caps() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.cfg");
        let mut console = ViewerConsole::new(path.clone()).unwrap();
        let name = crate::runtime_settings::MENU_CAP_CVAR;
        assert_eq!(
            console.cvar_default(name),
            Some(&sjk_shell::CvarValue::Integer(0))
        );
        assert_eq!(console.menu_fps(125, true), 125);
        assert!(console.set_cvar(name, "30"));
        assert_eq!(console.menu_fps(125, true), 30);
        assert_eq!(console.menu_fps(125, false), 125);
        console.window_state(Some(false), None);
        assert!(console.set_cvar("com_maxfpsUnfocused", "20"));
        assert_eq!(console.window_fps(console.menu_fps(125, true)), 20);
        assert!(console.set_cvar("com_maxfpsUnfocused", "0"));
        assert_eq!(console.window_fps(console.menu_fps(125, true)), 30);
        drop(console);
        let console = ViewerConsole::new(path).unwrap();
        assert_eq!(console.menu_fps(125, true), 30);
    }
}

#[cfg(test)]
mod away_talk_tests {
    use super::{ViewerConsole, away_talk};

    #[test]
    fn one_switch_controls_every_away_state() {
        for unfocused in [false, true] {
            for minimized in [false, true] {
                assert!(!away_talk(unfocused, minimized, 0));
                for enabled in [-1, 1, 2] {
                    assert_eq!(
                        away_talk(unfocused, minimized, enabled),
                        unfocused || minimized
                    );
                }
            }
        }
    }

    #[test]
    fn the_shared_switch_changes_live_and_the_old_setting_has_no_default() {
        let directory = tempfile::tempdir().unwrap();
        let mut console = ViewerConsole::new(directory.path().join("config.cfg")).unwrap();
        assert!(console.cvar_default("cl_minimizedChatbox").is_none());
        console.window_state(None, Some(true));
        assert!(
            console.window_talk(),
            "a minimized event can arrive before focus loss"
        );
        assert!(console.set_cvar("cl_unfocusedChatbox", "0"));
        assert!(!console.window_talk());
        console.window_state(Some(false), None);
        assert!(!console.window_talk());
        console.window_state(None, Some(false));
        assert!(!console.window_talk(), "unfocused is off too");
        assert!(console.set_cvar("cl_unfocusedChatbox", "2"));
        assert!(console.window_talk());
        console.window_state(Some(true), None);
        assert!(!console.window_talk());
    }

    #[test]
    fn old_profiles_follow_the_saved_unfocused_choice_in_both_states() {
        for (unfocused, old_minimized) in [(0, 1), (1, 0)] {
            let directory = tempfile::tempdir().unwrap();
            let config = directory.path().join("config.cfg");
            std::fs::write(&config, format!("seta cl_unfocusedChatbox {unfocused}\nseta cl_minimizedChatbox {old_minimized}\n")).unwrap();
            let mut console = ViewerConsole::new(config).unwrap();
            assert!(console.cvar("cl_minimizedChatbox").is_none());
            console.window_state(Some(false), Some(true));
            assert_eq!(console.window_talk(), unfocused != 0);
            console.window_state(None, Some(false));
            assert_eq!(console.window_talk(), unfocused != 0);
            assert!(console.set_cvar(
                "cl_unfocusedChatbox",
                if unfocused == 0 { "1" } else { "0" }
            ));
            let saved = std::fs::read_to_string(directory.path().join("config.cfg")).unwrap();
            assert!(!saved.to_ascii_lowercase().contains("cl_minimizedchatbox"));
        }
    }
}
