//! JoF EJK #331: current-map private game chat, independent of hub chat and logs.
use crate::console::ViewerConsole;
pub(crate) const CVAR: &str = "cg_pmOnlyChat";

pub(crate) fn command(console: &mut ViewerConsole, args: &[String]) -> Result<Vec<String>, String> {
    let value = match args {
        [] => !console.bool_cvar(CVAR).unwrap_or(false),
        [arg] if arg == "1" || arg.eq_ignore_ascii_case("on") => true,
        [arg] if arg == "0" || arg.eq_ignore_ascii_case("off") => false,
        _ => return Err("Usage: pmonly [on|off]".into()),
    };
    console.set_cvar(CVAR, if value { "1" } else { "0" });
    Ok(vec![format!(
        "PM-only game chat {} for this map.",
        if value { "enabled" } else { "disabled" }
    )])
}

/// Reset on actual world adoption; graphics reloads keep the current-map choice.
pub(crate) fn reset(console: &mut ViewerConsole) {
    console.set_cvar(CVAR, "0");
}

impl super::ChatOverlay {
    pub(super) fn pm_visible(&self, line: &super::ChatLine) -> bool {
        !self.options.pm_only || line.channel.is_hub() || line.server_private
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::ChatOverlay;
    use sjk_client::ServerEventKind;
    use std::time::Instant;

    #[test]
    fn pm_only_command_toggles_parses_and_resets_without_archiving() {
        let directory = tempfile::tempdir().unwrap();
        let mut console = ViewerConsole::new(directory.path().join("config.cfg")).unwrap();
        assert_eq!(console.bool_cvar(CVAR), Some(false));
        command(&mut console, &[]).unwrap();
        assert_eq!(console.bool_cvar(CVAR), Some(true));
        assert!(command(&mut console, &["maybe".into()]).is_err());
        assert_eq!(console.bool_cvar(CVAR), Some(true));
        command(&mut console, &["OFF".into()]).unwrap();
        assert_eq!(console.bool_cvar(CVAR), Some(false));
        command(&mut console, &["1".into()]).unwrap();
        reset(&mut console);
        assert_eq!(console.bool_cvar(CVAR), Some(false));
    }

    #[test]
    fn pm_only_filters_existing_and_incoming_history_by_server_marker() {
        let mut chat = ChatOverlay::new();
        let now = Instant::now();
        for (kind, text, private) in [
            (ServerEventKind::Chat, "Fox: public", false),
            (ServerEventKind::TeamChat, "(Fox): team", false),
            (ServerEventKind::Chat, "Fox: [Fake^7]: ^6lookalike", false),
            (ServerEventKind::Chat, "\u{19}[Fox]: real PM", true),
            (ServerEventKind::Chat, "\u{19}[Fox]: sent echo", true),
            // A localized command can contain a lookalike name without being a tell.
            (ServerEventKind::Chat, "\u{19}[Fox]: localized", false),
        ] {
            chat.receive_classified(kind, text.into(), None, private, now);
        }
        assert_eq!(chat.lines.len(), 6);
        assert_eq!(
            chat.lines
                .iter()
                .filter(|line| chat.pm_visible(line))
                .count(),
            6
        );
        chat.options.pm_only = true;
        assert_eq!(
            chat.lines
                .iter()
                .filter(|line| chat.pm_visible(line))
                .count(),
            2
        );
        assert!(
            chat.lines
                .iter()
                .filter(|line| chat.pm_visible(line))
                .all(|line| line.server_private)
        );
        // Filtering never erases retained history or disables the composer.
        chat.open(false);
        assert!(chat.is_typing());
        chat.options.pm_only = false;
        assert_eq!(
            chat.lines
                .iter()
                .filter(|line| chat.pm_visible(line))
                .count(),
            6
        );
    }
}
