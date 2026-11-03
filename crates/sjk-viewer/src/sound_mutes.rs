//! Muting one player's sounds (akr's and Neffie's request, 03/11/2026): `mutesounds
//! <player>` silences everything the player causes, as a block does (`audio_mute.rs`:
//! their voice, footsteps, saber, Force, emotes' and toys' sounds), without hiding their
//! chat or drawing them as Kyle. It lasts until the map changes or they leave, as a
//! block for the map does ([`crate::map_blocks::MapBlocks`], the same bookkeeping);
//! `mutesounds` alone lists them and `unmutesounds <player|all>` lifts it.
//!
//! With `cl_ignoreMutesSounds` (Settings > Chat "Ignore mutes sounds", off by default) a
//! JA+ or jaPRO `ignore <player>` typed by the player (a server command, which hides
//! that player's chat) also mutes their sounds, and `ignore` again (the servers' toggle)
//! lifts it: the client reads the commands it released to the server
//! ([`sjk_client::sent_commands`]), so nothing is sent twice.

use crate::map_blocks::MapBlocks;

/// Console command names.
pub(crate) const COMMAND: &str = "mutesounds";
pub(crate) const UNMUTE_COMMAND: &str = "unmutesounds";
/// Help texts for completion and `cmdlist`.
pub(crate) const HELP: &str = "Mute a player's sounds until the map changes (voice, saber, steps, emotes, toys): mutesounds <player|target>; alone, list them";
pub(crate) const UNMUTE_HELP: &str = "Hear a player's sounds again: unmutesounds <player|all>";
/// The setting tying the servers' `ignore` to it.
pub(crate) const IGNORE_CVAR: &str = "cl_ignoreMutesSounds";

/// The players whose sounds are muted, and the last sent command read.
#[derive(Debug, Default)]
pub(crate) struct SoundMutes {
    pub(crate) players: MapBlocks,
    /// The number of the last released command looked at for `ignore`.
    seen: u64,
    /// The commands were looked at once: the ones sent before are not read.
    started: bool,
}

/// The player an `ignore` command names: the words after it, when `command` is one.
pub(crate) fn ignored(command: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(command).ok()?.trim();
    let (name, rest) = text.split_once(char::is_whitespace)?;
    let rest = rest.trim().trim_matches('"').trim();
    (name.eq_ignore_ascii_case("ignore") && !rest.is_empty()).then(|| rest.to_owned())
}

impl SoundMutes {
    /// The players named by `ignore` commands released since the last call, oldest
    /// first; the first call after a start only notes where the commands are. A new
    /// connection numbers its commands from 1 again.
    pub(crate) fn new_ignores<'a>(
        &mut self,
        commands: impl Iterator<Item = (u64, &'a [u8])> + Clone,
    ) -> Vec<String> {
        let mut named = Vec::new();
        let first = !std::mem::replace(&mut self.started, true);
        if commands
            .clone()
            .last()
            .is_some_and(|(newest, _)| newest < self.seen)
        {
            self.seen = 0;
        }
        for (number, command) in commands {
            if number <= self.seen {
                continue;
            }
            self.seen = number;
            if !first && let Some(player) = ignored(command) {
                named.push(player);
            }
        }
        named
    }
}

impl crate::GpuState {
    /// The console's lines for `mutesounds` alone.
    fn sound_mute_listing(&self) -> Vec<String> {
        let slots = self.sound_mutes.players.own_slots();
        if slots == 0 {
            return vec!["Nobody's sounds are muted".to_owned()];
        }
        std::iter::once(format!(
            "Sounds muted until the map changes ({}):",
            slots.count_ones()
        ))
        .chain(
            (0..crate::chat_mutes::SLOTS as u16)
                .filter(|slot| slots & 1 << slot != 0)
                .map(|slot| format!("  {}^7", self.chat.player_label(slot))),
        )
        .collect()
    }

    /// The player `word` names here: their slot, plain name and the name shown.
    fn sound_mutable(&self, word: &str) -> Result<(u8, String, String), String> {
        let Some(session) = self.live_session.as_ref() else {
            return Err("Join a server to mute a player's sounds there".to_owned());
        };
        let own = u8::try_from(session.latest_snapshot().player.client_num()).ok();
        let (slot, plain) = self.emote_player(own, word)?;
        let shown = crate::player_identity::shown_name(session.game_state(), usize::from(slot))
            .unwrap_or_else(|| plain.clone());
        Ok((slot, plain, shown))
    }

    /// `mutesounds [player]`.
    pub(crate) fn mute_sounds_command(&mut self, args: &[String]) -> Result<Vec<String>, String> {
        let word = args.get(1..).unwrap_or_default().join(" ");
        let word = word.trim();
        if word.is_empty() {
            return Ok(self.sound_mute_listing());
        }
        let (slot, plain, shown) = self.sound_mutable(word)?;
        if self.sound_mutes.players.own_slots() & (1 << slot) != 0 {
            return Err(format!("{plain}'s sounds are already muted"));
        }
        self.sound_mutes.players.block(usize::from(slot), &shown);
        Ok(vec![format!(
            "{plain}'s sounds are muted until the map changes (unmutesounds {plain})"
        )])
    }

    /// `unmutesounds <player|all>`.
    pub(crate) fn unmute_sounds_command(&mut self, args: &[String]) -> Result<Vec<String>, String> {
        let word = args.get(1..).unwrap_or_default().join(" ");
        let word = word.trim();
        if word.is_empty() {
            return Err(UNMUTE_HELP.to_owned());
        }
        if word.eq_ignore_ascii_case(crate::emotes::ALL) {
            let count = self.sound_mutes.players.clear();
            return Ok(vec![format!("Sounds of {count} player(s) heard again")]);
        }
        let (slot, plain, _) = self.sound_mutable(word)?;
        if !self.sound_mutes.players.unblock(usize::from(slot)) {
            return Err(format!("{plain}'s sounds are not muted"));
        }
        Ok(vec![format!("{plain}'s sounds are heard again")])
    }

    /// Follow the `ignore` commands the player sent: with `cl_ignoreMutesSounds`, each
    /// mutes the named player's sounds, or lifts that (the servers' ignore toggles).
    pub(crate) fn follow_ignores(&mut self) {
        let Some(session) = self.live_session.as_ref() else {
            return;
        };
        let named = self.sound_mutes.new_ignores(
            session
                .sent_commands()
                .iter()
                .map(|(number, _, command)| (number, command)),
        );
        let on = self
            .console
            .as_ref()
            .and_then(|console| console.bool_cvar(IGNORE_CVAR))
            == Some(true);
        if !on {
            return;
        }
        for word in named {
            let Ok((slot, _, shown)) = self.sound_mutable(&word) else {
                continue;
            };
            let players = &mut self.sound_mutes.players;
            if players.own_slots() & (1 << slot) != 0 {
                players.unblock(usize::from(slot));
            } else {
                players.block(usize::from(slot), &shown);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ignore_command_names_its_player() {
        assert_eq!(ignored(b"ignore Troll"), Some("Troll".to_owned()));
        assert_eq!(
            ignored(b"IGNORE \"Big Troll\""),
            Some("Big Troll".to_owned())
        );
        assert_eq!(ignored(b"ignore 4"), Some("4".to_owned()));
        assert_eq!(ignored(b"ignore"), None);
        assert_eq!(ignored(b"ignorelist"), None);
        assert_eq!(ignored(b"say ignore him"), None);
    }

    #[test]
    fn only_new_ignores_count_and_none_from_before_the_start() {
        let mut mutes = SoundMutes::default();
        let old: [(u64, &[u8]); 2] = [(1, b"ignore Old"), (2, b"say hi")];
        assert!(mutes.new_ignores(old.into_iter()).is_empty(), "sent before");
        let new: [(u64, &[u8]); 3] = [(2, b"say hi"), (3, b"ignore Troll"), (4, b"kill")];
        assert_eq!(mutes.new_ignores(new.into_iter()), ["Troll"]);
        assert!(mutes.new_ignores(new.into_iter()).is_empty(), "read once");
        // A new connection counts from 1 again.
        let again: [(u64, &[u8]); 1] = [(1, b"ignore Sol")];
        assert_eq!(mutes.new_ignores(again.into_iter()), ["Sol"]);
    }

    #[test]
    fn the_first_command_of_a_start_counts_once_looked_at() {
        let mut mutes = SoundMutes::default();
        assert!(mutes.new_ignores(std::iter::empty()).is_empty());
        let first: [(u64, &[u8]); 1] = [(1, b"ignore Troll")];
        assert_eq!(mutes.new_ignores(first.into_iter()), ["Troll"]);
    }
}
