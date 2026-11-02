//! Legacy chat options mapped onto the floating, retained conversation layout.
use super::*;
use crate::{cgame_options::scalar, console::ViewerConsole};

/// Range of `cg_chatBoxFontSize`. 1 is EternalJK's size ([`super::layout::FONT_1080`]);
/// the ends hold every value saved under the smaller pre-19/10/2026 scale (0.25 to 3,
/// [`super::text_sizes`]).
pub(crate) const CHAT_SIZE: (f64, f64) = (0.15, 2.0);
/// Default `cg_chatBoxFontSize`: 0.85 of EternalJK's size, a 26 px line at 1080p,
/// about 1.45 on the pre-19/10/2026 scale (Sol's choice, 19/10/2026).
pub(crate) const CHAT_SIZE_DEFAULT: f64 = 0.85;
/// `cg_chatBoxLineSpacing`: room between chat messages in 1080p pixels at text size
/// 1, its range and default (8 before 19/10/2026).
pub(crate) const LINE_SPACING_CVAR: &str = "cg_chatBoxLineSpacing";
pub(crate) const LINE_SPACING: (f64, f64) = (0.0, 16.0);
pub(crate) const LINE_SPACING_DEFAULT: f64 = 2.0;
/// Largest `cg_centerSize`: EternalJK stops at 1; SJK allows twice its size, which
/// holds the old scale's largest value (4).
pub(crate) const CENTER_SIZE_MAX: f32 = 2.0;
/// The size of the sender card a chat name shows ([`super::card`]), over the chat's
/// own scale.
pub(crate) const CARD_SCALE_CVAR: &str = "cg_chatCardScale";
/// Range of [`CARD_SCALE_CVAR`].
pub(crate) const CARD_SCALE: (f32, f32) = (0.5, 2.0);
/// `cg_chatBoxBackdrop`: how opaque, in per cent, the dark band behind each row of a
/// chat message is (`chat/view/backdrop.rs`); 0, the default, draws none. Creo's idea
/// (29/10/2026): the chat is hard to read over a bright map.
pub(crate) const BACKDROP_CVAR: &str = "cg_chatBoxBackdrop";
/// `cg_chatBoxOutline`: chat text edged in a dark outline instead of its drop shadow
/// ([`crate::text::CodePalette::DarkOutlined`]); off by default.
pub(crate) const OUTLINE_CVAR: &str = "cg_chatBoxOutline";

/// `cg_centerSize` as drawn: zero, which would hide the text, is 1 as in
/// EternalJK's `CG_DrawCenterString`.
fn center_size(value: f32) -> f32 {
    if value == 0.0 {
        1.0
    } else {
        value.clamp(0.0, CENTER_SIZE_MAX)
    }
}

#[derive(Clone, Copy, PartialEq)]
/// Bounded presentation settings read from the archived cvars.
pub(super) struct Options {
    pub(super) pm_only: bool,
    /// Milliseconds before a message expires.
    pub(super) lifetime: u64,
    /// Maximum retained conversation entries displayed together.
    pub(super) lines: usize,
    /// Multiplier for the modern conversation font.
    pub(super) font: f32,
    /// Multiplier for the sender card, over the chat's scale ([`CARD_SCALE_CVAR`]).
    pub(super) card: f32,
    /// Extra room after each message character, in 1080p pixels at font scale 1.
    pub(super) spacing: f32,
    /// Room between messages, in 1080p pixels at font scale 1.
    pub(super) gap: f32,
    /// Left edge in virtual 640-wide coordinates.
    pub(super) x: f32,
    /// Bottom edge in virtual 480-high coordinates.
    pub(super) height: f32,
    /// Wrapping width in virtual coordinates.
    pub(super) width: f32,
    /// Expired history requested while the console catcher is active.
    pub(super) history: bool,
    /// Incoming body colour-cleaning mode.
    pub(super) clean: i64,
    /// Local display-only message colour.
    pub(super) incoming: Option<sjk_ui::Color>,
    /// Centre-print lifetime in milliseconds.
    pub(super) center_time: u64,
    /// Centre-print size multiplier.
    pub(super) center_size: f32,
    /// Centre-print virtual Y, with zero selecting the hero default.
    pub(super) center_height: f32,
    /// Arriving messages show emoji pictures ([`super::emoji::CVAR`]).
    pub(super) emojis: bool,
    /// An SJK chat line's GIPHY link shows its GIF under it ([`crate::chat_gifs::CVAR`]).
    pub(super) gifs: bool,
    /// The chat key opens on the last channel sent on ([`super::reply::REMEMBER_CVAR`]).
    pub(super) remember: bool,
    /// The channels kept in the composer's row ([`super::tabs`]).
    pub(super) tabs: super::tabs::Tabs,
    /// Opacity of the band behind each chat row, 0 (none) to 1 ([`BACKDROP_CVAR`]).
    pub(super) backdrop: f32,
    /// Chat text in a dark outline instead of its shadow ([`OUTLINE_CVAR`]).
    pub(super) outline: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self::read(None)
    }
}

impl Options {
    fn read(console: Option<&ViewerConsole>) -> Self {
        let integer = |name, default| {
            console
                .and_then(|c| c.integer_cvar(name))
                .unwrap_or(default)
        };
        Self {
            pm_only: console
                .and_then(|c| c.bool_cvar("cg_pmonlychat"))
                .unwrap_or(false),
            lifetime: integer("cg_chatbox", 10000).max(0) as u64,
            lines: integer("cg_chatboxlines", 5).clamp(1, MAX_VISIBLE as i64) as usize,
            font: scalar(console, "cg_chatboxfontsize", CHAT_SIZE_DEFAULT)
                .clamp(CHAT_SIZE.0 as f32, CHAT_SIZE.1 as f32),
            card: scalar(console, CARD_SCALE_CVAR, 1.0).clamp(CARD_SCALE.0, CARD_SCALE.1),
            spacing: scalar(console, "cg_chatboxletterspacing", -0.5).clamp(-2.0, 8.0),
            gap: scalar(console, LINE_SPACING_CVAR, LINE_SPACING_DEFAULT)
                .clamp(LINE_SPACING.0 as f32, LINE_SPACING.1 as f32),
            x: scalar(console, "cg_chatboxx", 30.0).clamp(0.0, 600.0),
            height: scalar(console, "cg_chatboxheight", 350.0).clamp(0.0, 480.0),
            width: scalar(console, "cg_chatboxcutofflength", 350.0).clamp(60.0, 640.0),
            history: integer("cg_chatboxshowhistory", 0) != 0
                && console.is_some_and(ViewerConsole::is_open),
            clean: integer("cg_cleanchatbox", 0),
            incoming: super::incoming_color::read(console),
            center_time: (scalar(console, "cg_centertime", 3.0).clamp(0.0, 3600.0) * 1000.0) as u64,
            center_size: center_size(scalar(console, "cg_centersize", 1.0)),
            center_height: scalar(console, "cg_centerheight", 0.0).clamp(0.0, 480.0),
            emojis: integer("cg_chatboxemojis", 0) != 0,
            gifs: console.is_none_or(|c| c.bool_cvar(crate::chat_gifs::CVAR) != Some(false)),
            // An integer cvar (`cgame_options`): read as a switch it was never off, so
            // turning it off kept the last channel (a player's bug report, 12/10/2026).
            remember: integer(super::reply::REMEMBER_CVAR, 0) != 0,
            tabs: super::tabs::Tabs::read(console),
            backdrop: integer(BACKDROP_CVAR, 0).clamp(0, 100) as f32 / 100.0,
            outline: integer(OUTLINE_CVAR, 0) != 0,
        }
    }

    /// Transform legacy coordinate controls into the retained modern layout.
    pub(super) fn geometry(self, viewport: [f32; 2]) -> layout::Geometry {
        let mut g = layout::Geometry::new(viewport);
        g.left = self.x / 640.0 * viewport[0];
        g.bottom = self.height / 480.0 * viewport[1];
        g.top = 0.0;
        g.width = (self.width / 640.0 * viewport[0])
            .min(viewport[0] - g.left)
            .max(1.0);
        g.font *= self.font;
        g.row *= self.font;
        g.scale *= self.font;
        g.spacing = self.spacing * g.scale;
        g.gap = self.gap * g.scale;
        g
    }
}

/// Where the chat's messages show, in pixels, with as many lines as `cg_chatBoxLines`
/// keeps: the box the HUD edit mode drags ([`crate::hud_edit`]) whether messages show
/// or not.
pub(crate) fn box_rect(console: Option<&ViewerConsole>, viewport: [f32; 2]) -> sjk_ui::Rect {
    let options = Options::read(console);
    let geometry = options.geometry(viewport);
    let lines = options.lines.max(1) as f32;
    let height = geometry.row * lines + geometry.gap * (lines - 1.0);
    sjk_ui::Rect::new(
        geometry.left,
        geometry.bottom - height,
        geometry.width,
        height,
    )
}

#[cfg(test)]
mod remember_tests {
    use super::*;

    #[test]
    fn remember_channel_is_off_by_default_and_follows_the_console() {
        let directory = tempfile::tempdir().unwrap();
        let mut console = ViewerConsole::new(directory.path().join("config.cfg")).unwrap();
        assert!(!Options::read(Some(&console)).remember, "off by default");
        assert!(!Options::default().remember, "off without a console");
        assert!(console.set_cvar(super::super::reply::REMEMBER_CVAR, "1"));
        assert!(Options::read(Some(&console)).remember);
        assert!(console.set_cvar(super::super::reply::REMEMBER_CVAR, "0"));
        assert!(!Options::read(Some(&console)).remember);
    }

    /// `cg_chatBoxBackdrop` is a share of full opacity, held to 0-100 per cent;
    /// `cg_chatBoxOutline` a switch. Both off in a new profile.
    #[test]
    fn backdrop_and_outline_are_off_by_default_and_follow_the_console() {
        let directory = tempfile::tempdir().unwrap();
        let mut console = ViewerConsole::new(directory.path().join("config.cfg")).unwrap();
        let read = |console: &ViewerConsole| {
            let options = Options::read(Some(console));
            (options.backdrop, options.outline)
        };
        assert_eq!(read(&console), (0.0, false));
        assert_eq!(
            (Options::default().backdrop, Options::default().outline),
            (0.0, false)
        );
        for (value, share) in [("50", 0.5), ("100", 1.0), ("250", 1.0), ("-5", 0.0)] {
            assert!(console.set_cvar(BACKDROP_CVAR, value));
            assert_eq!(read(&console).0, share, "{value}");
        }
        assert!(console.set_cvar(OUTLINE_CVAR, "1"));
        assert!(read(&console).1);
    }
}

/// Creyon's report #129: `cg_chatBoxLines` above 8 did not show more messages.
#[cfg(test)]
mod lines_tests {
    use super::*;

    fn shown(console: &ViewerConsole, sent: usize) -> usize {
        let mut chat = ChatOverlay::new();
        chat.configure(Some(console));
        let now = Instant::now();
        for n in 0..sent {
            chat.receive_classified(
                sjk_client::ServerEventKind::Chat,
                format!("Fox: line{n:02}"),
                None,
                false,
                now,
            );
        }
        chat.build(true, &crate::text::test_font(), [1920.0, 1080.0], 1_000);
        (0..sent)
            .filter(|n| {
                let wanted = format!("line{n:02}");
                chat.ui.text_runs().any(|text| text.contains(&wanted))
            })
            .count()
    }

    #[test]
    fn chat_box_lines_reach_eternaljks_twenty_four() {
        let directory = tempfile::tempdir().unwrap();
        let mut console = ViewerConsole::new(directory.path().join("config.cfg")).unwrap();
        assert_eq!(
            Options::read(Some(&console)).lines,
            5,
            "EternalJK's default"
        );
        for (value, lines) in [("0", 1), ("3", 3), ("12", 12), ("24", 24), ("100", 24)] {
            assert!(console.set_cvar("cg_chatBoxLines", value));
            assert_eq!(Options::read(Some(&console)).lines, lines, "{value}");
        }
        assert!(console.set_cvar("cg_chatBoxLines", "12"));
        assert_eq!(shown(&console, 20), 12);
        assert!(console.set_cvar("cg_chatBoxLines", "5"));
        assert_eq!(shown(&console, 20), 5);
    }
}

/// Clean message colour markup on arrival, leaving the separately rendered name intact.
pub(super) fn clean_body(body: &str, mode: i64) -> String {
    if mode == 1 {
        let mut output = String::with_capacity(body.len());
        let mut chars = body.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '^' && chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                chars.next();
            } else {
                output.push(ch);
            }
        }
        output
    } else if mode > 1 {
        let mut rest = body;
        while rest.as_bytes().first() == Some(&b'^')
            && rest.as_bytes().get(1).is_some_and(u8::is_ascii_digit)
        {
            rest = &rest[2..];
        }
        rest.to_owned()
    } else {
        body.to_owned()
    }
}

impl ChatOverlay {
    /// Console history can draw while the rest of the gameplay HUD is suppressed.
    pub(crate) fn wants_history(&self, console: Option<&ViewerConsole>) -> bool {
        self.shown_for_shot()
            || console.is_some_and(|c| {
                c.is_open() && c.integer_cvar("cg_chatboxshowhistory").unwrap_or(0) != 0
            })
    }
    /// Apply cvars without allocating; wrapping invalidates through width/font keys.
    pub(crate) fn configure(&mut self, console: Option<&ViewerConsole>) {
        let options = Options::read(console);
        if self.options != options {
            if self.options.pm_only != options.pm_only {
                self.scroll = 0;
                self.unread = 0;
            }
            for line in &mut self.lines {
                line.y = None;
            }
            self.options = options;
        }
    }
}
