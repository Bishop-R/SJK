//! The last reliable commands this client released to the server, and when.
//!
//! The viewer's box hunt reads it to tell a teleport the player asked for (their own
//! `amtele`, `noclip` and the like) from one the map or an admin made
//! (`crates/sjk-viewer/src/mystery_box/hunt_watch.rs`): the server's snapshots show only
//! that the player moved, not who moved them.

use std::collections::VecDeque;
use std::time::Instant;

/// How many commands are kept: enough for a few seconds of a player's own commands.
const KEPT: usize = 16;

/// The newest [`KEPT`] commands released, oldest first, each with its number (counting
/// every command noted, so a reader knows which it has seen) and when it left.
#[derive(Debug, Default)]
pub struct SentCommands {
    recent: VecDeque<(u64, Instant, Vec<u8>)>,
    noted: u64,
}

impl SentCommands {
    /// Note `command`, released at `at`.
    pub(crate) fn note(&mut self, command: &[u8], at: Instant) {
        if self.recent.len() == KEPT {
            self.recent.pop_front();
        }
        self.noted += 1;
        self.recent.push_back((self.noted, at, command.to_vec()));
    }

    /// The commands kept, oldest first: each one's number (from 1, rising), when it left
    /// and the command.
    pub fn iter(&self) -> impl Iterator<Item = (u64, Instant, &[u8])> + Clone {
        self.recent
            .iter()
            .map(|(number, at, command)| (*number, *at, command.as_slice()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_newest() {
        let mut sent = SentCommands::default();
        let now = Instant::now();
        for index in 0..20u8 {
            sent.note(&[index], now);
        }
        let kept: Vec<(u64, u8)> = sent
            .iter()
            .map(|(number, _, command)| (number, command[0]))
            .collect();
        assert_eq!(
            kept,
            (4..20u8)
                .map(|index| (u64::from(index) + 1, index))
                .collect::<Vec<_>>()
        );
    }
}
