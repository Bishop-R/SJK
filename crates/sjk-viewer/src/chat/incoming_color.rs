//! Local, display-only colour for public message bodies; special channels keep theirs.
use crate::console::ViewerConsole;
use sjk_ui::Color;
use std::fmt::{self, Write};

/// Zero keeps authored colours; 1-9 match the chat codes, 10 selects black.
pub(crate) const CVAR: &str = "cg_chatColor";
/// Settings labels indexed by the saved cvar value.
pub(crate) const LABELS: &[&str] = &[
    "Original", "Red", "Green", "Yellow", "Blue", "Cyan", "Magenta", "White", "Orange", "Grey",
    "Black",
];

pub(crate) const CHOICES: &[&str] = &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10"];

/// Resolve the local preference once while a view reads its settings.
pub(crate) fn read(console: Option<&ViewerConsole>) -> Option<Color> {
    from_value(console.and_then(|c| c.integer_cvar(CVAR)).unwrap_or(0))
}

fn from_value(value: i64) -> Option<Color> {
    let code = match value {
        1..=9 => value as u8,
        10 => 0,
        _ => return None,
    };
    let [r, g, b, _] = crate::text::quake_color(code);
    Some(Color::new(r, g, b, 1.0))
}

impl super::ChatLine {
    /// Only the game server's public All channel uses the local preference.
    /// Private/team/admin chat, the separate SJK channel and encrypted messages
    /// keep their established colours, even when displayed in the All feed.
    pub(super) fn incoming_colour(&self, preference: Option<Color>) -> Option<Color> {
        preference.filter(|_| {
            self.channel == super::Channel::Global && !self.server_private && !self.encrypted
        })
    }
}

/// Keep a message's fade while replacing only its RGB.
pub(crate) fn colour(original: Color, forced: Option<Color>) -> Color {
    forced.map_or(original, |c| Color::new(c.r, c.g, c.b, original.a))
}

/// Format body text without colour escapes when forced, without allocating or
/// changing the stored message, wrapping offsets, links or emoji markers.
pub(super) struct Body<'a>(pub &'a str, pub bool);

impl fmt::Display for Body<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.1 {
            return f.write_str(self.0);
        }
        let mut chars = self.0.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '^' && chars.peek().is_some_and(char::is_ascii_digit) {
                chars.next();
            } else {
                f.write_char(c)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_and_invalid_values_preserve_colour_and_markup() {
        for value in [i64::MIN, -1, 0, 11, i64::MAX] {
            assert_eq!(from_value(value), None);
        }
        assert_eq!(Body("^1red ^7white", false).to_string(), "^1red ^7white");
    }
    #[test]
    fn forcing_removes_only_colour_codes_and_keeps_fade() {
        assert_eq!(Body("^1Hi ^7é🙂 ^x ^", true).to_string(), "Hi é🙂 ^x ^");
        let original = Color::new(0.2, 0.3, 0.4, 0.25);
        let green = colour(original, from_value(2));
        assert_eq!(green, Color::new(0.0, 1.0, 0.0, 0.25));
        assert_eq!(colour(original, None), original);
        assert_eq!(from_value(10), Some(Color::new(0.0, 0.0, 0.0, 1.0)));
    }
}
