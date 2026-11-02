//! Additive SJK hub chat-reaction contract; independent of game-server chat.
use serde::{Deserialize, Serialize};

/// One supported emoji, with its stable wire id and visible symbol.
pub const CATALOG: [(&str, &str); 6] = [
    ("like", "👍"),
    ("heart", "❤️"),
    ("laugh", "😂"),
    ("wow", "😮"),
    ("sad", "😢"),
    ("fire", "🔥"),
];
/// Maximum person/emoji pairs on one message, as enforced by the hub.
pub const MAX: usize = 128;

/// One authenticated person's public reaction, read from the hub.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct Reaction {
    /// Canonical person id, not a game-server slot.
    pub key_id: String,
    /// A stable id from [`CATALOG`].
    pub emoji: String,
}

/// Immutable shared reaction state. Cloning a displayed chat snapshot does not
/// allocate or copy the reaction list; validation happens on receipt.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct State(std::sync::Arc<[Reaction]>);
impl From<Vec<Reaction>> for State {
    fn from(mut values: Vec<Reaction>) -> Self {
        sanitize(&mut values);
        Self(values.into())
    }
}
impl std::ops::Deref for State {
    type Target = [Reaction];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'de> Deserialize<'de> for State {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Vec::<Reaction>::deserialize(deserializer)?.into())
    }
}
impl Serialize for State {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.as_ref().serialize(serializer)
    }
}

/// A complete replacement for a retained message's reaction state.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct Update {
    /// Normal feed sequence id.
    pub id: u64,
    /// The original chat message id.
    pub message: u64,
    /// Current reactions; an empty list clears them.
    pub reactions: State,
}

/// Whether an id is supported. Unknown future ids remain harmless to old clients.
pub fn valid(id: &str) -> bool {
    CATALOG.iter().any(|(known, _)| *known == id)
}

/// Bound and validate received state before it becomes UI history.
pub fn sanitize(reactions: &mut Vec<Reaction>) {
    reactions.truncate(MAX);
    reactions.retain(|r| {
        valid(&r.emoji)
            && r.key_id.len() == 16
            && r.key_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    });
    reactions.sort_unstable_by(|a, b| (&a.emoji, &a.key_id).cmp(&(&b.emoji, &b.key_id)));
    reactions.dedup();
}

/// Counts and the reader's selected emojis, without per-frame allocations.
pub fn counts(reactions: &[Reaction], own: Option<&str>) -> ([usize; 6], [bool; 6]) {
    let mut counts = [0; 6];
    let mut mine = [false; 6];
    for r in reactions.iter().take(MAX) {
        if let Some(at) = CATALOG.iter().position(|(id, _)| *id == r.emoji) {
            counts[at] += 1;
            mine[at] |= own == Some(r.key_id.as_str());
        }
    }
    (counts, mine)
}

/// Small display suffix for per-message counts; formats into the UI's existing arena.
pub struct Summary(pub [usize; 6]);
impl std::fmt::Display for Summary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (at, count) in self.0.iter().enumerate() {
            if *count != 0 {
                write!(f, "  {} {count}", CATALOG[at].1)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_hub_json_remains_readable_and_reaction_snapshots_share_storage() {
        let old: crate::wire::Feed = serde_json::from_str(r#"{"next":1,"chat":[{"id":1,"at":100,"key_id":"0123456789abcdef","name":"Player","text":"hello"}]}"#).unwrap();
        assert!(old.chat[0].reactions.is_empty());
        assert!(old.reaction_updates.is_empty());
        let state: State = vec![Reaction {
            key_id: "0123456789abcdef".into(),
            emoji: "heart".into(),
        }]
        .into();
        let clone = state.clone();
        assert!(std::ptr::eq(state.as_ptr(), clone.as_ptr()));
        let json = serde_json::to_string(&state).unwrap();
        let roundtrip: State = serde_json::from_str(&json).unwrap();
        assert_eq!(state, roundtrip);
    }

    #[test]
    fn old_messages_default_to_empty_and_updates_are_bounded() {
        let mut values = vec![
            Reaction {
                key_id: "0123456789abcdef".into(),
                emoji: "heart".into()
            };
            200
        ];
        values.push(Reaction {
            key_id: "bad".into(),
            emoji: "unknown".into(),
        });
        sanitize(&mut values);
        assert_eq!(values.len(), 1);
        let (counts, mine) = counts(&values, Some("0123456789abcdef"));
        assert_eq!(counts, [0, 1, 0, 0, 0, 0]);
        assert!(mine[1]);
        assert!(!valid("not_an_emoji"));
        assert_eq!(Summary(counts).to_string(), "  ❤️ 1");
    }
}
