//! SJK chat said on the game server too (`docs/hub-chat.md`, "Said on the game server
//! too"). On JoF servers only, JoF tools say what the player says on the SJK channel
//! during a match in the server's chat as well, once the hub took it, ended with the
//! hub's id for it (`sjk_client::sjk_copy_marker`; `sjk-mod-jof`, `sjk_chat.rs`), as
//! JoF EternalJK does on its side. The server's copy of a message, the player's own or
//! another SJK chat player's, waits here for the message itself, which shows in its
//! place marked ALL/SJK: the line is shown, logged and sounded once. A copy is hidden
//! only when it comes from the message's sender (when the hub knows who is in its slot)
//! and holds the message's words; any other shows as it came, so a copy may show twice
//! but never vanishes.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::ChatOverlay;
use super::marks::SlotMark;
use sjk_client::{ServerEvent, ServerEventKind};
use sjk_identity::{ChatMessage, ReportOutcome};

/// How long the server's copy of a message waits for the message before it shows.
pub(crate) const HOLD: Duration = Duration::from_millis(2_500);
/// How long a message that showed keeps the server's later copies of it out.
const SEEN: Duration = Duration::from_secs(15);
/// How long a message said in a match waits for the hub; past it, or refused, it never
/// reached SJK chat and nothing goes to the server.
#[cfg_attr(not(feature = "mod-jof"), allow(dead_code))]
const ANSWER: Duration = Duration::from_secs(10);
/// Held copies and shown messages followed at most.
const KEPT: usize = 16;

/// What was handed to the SJK chat since the frame last looked, from any of its fields
/// (`player_identity::chat`): the text and the chat outcome's serial then.
static SAID: Mutex<Vec<(String, u64)>> = Mutex::new(Vec::new());

/// Note a message handed to the SJK chat; `outcome` is the chat outcome's serial then.
pub(crate) fn note_said(text: &str, outcome: u64) {
    if let Ok(mut said) = SAID.lock() {
        said.push((text.to_owned(), outcome));
    }
}

/// The messages noted since the last call ([`note_said`]), oldest first.
pub(crate) fn take_said() -> Vec<(String, u64)> {
    SAID.lock()
        .map(|mut said| std::mem::take(&mut *said))
        .unwrap_or_default()
}

/// What a mod says while it says SJK chat in this server's All chat: the composer's red
/// note, and the SJK chat page's Send only in SJK button with its hover card.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SaidInAll {
    pub(crate) note: &'static str,
    pub(crate) only: &'static str,
    pub(crate) only_title: &'static str,
    pub(crate) only_why: &'static [&'static str],
}

/// [`SaidInAll`] for the SJK chat page, which the console draws without the frame's
/// chat; set when it changes ([`ChatOverlay::set_said_in_all`]).
static SAID_IN_ALL: Mutex<Option<SaidInAll>> = Mutex::new(None);

/// Whether SJK chat is said in this server's All chat now, and the words for it.
pub(crate) fn said_in_all() -> Option<SaidInAll> {
    SAID_IN_ALL.lock().ok().and_then(|said| *said)
}

/// A message the hub took, ready to be said on the server: its text and id.
pub(crate) type Copy = (String, u64);

#[derive(Debug, Default)]
pub(super) struct ServerCopies {
    held: Vec<Held>,
    seen: VecDeque<Seen>,
    waiting: Option<Waiting>,
    /// Messages ready for the server's chat, with the match they are for.
    ready: Vec<(i32, Copy)>,
}

/// A server chat line that is the copy of an SJK chat message, waiting for it.
#[derive(Debug)]
struct Held {
    id: u64,
    until: Instant,
    event: ServerEvent,
}

/// A message that showed, which keeps its server's later copies out.
#[derive(Debug)]
struct Seen {
    id: u64,
    until: Instant,
    key_id: String,
    /// Its words as [`words`] reads them.
    words: String,
}

/// A message said in a match, waiting for the hub's answer.
#[derive(Debug)]
struct Waiting {
    /// The text as the hub takes it (`sjk_identity::chat::check`), to know the
    /// player's own message when the feed brings it back with its id.
    text: String,
    /// The match it was said in (`sv_serverid`).
    server: i32,
    /// The chat outcome's serial when it was sent: a later refusal is this message's.
    outcome: u64,
    #[cfg_attr(not(feature = "mod-jof"), allow(dead_code))]
    since: Instant,
}

/// A text's letters and digits, lower case, without colour codes or EternalJK's chat
/// escapes: what survives the trip through the game server, whose old code pages drop
/// emojis and other scripts.
fn words(text: &str) -> String {
    sjk_client::chat_plain_text(&sjk_client::chat_unescape(text))
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// What a copy's line says: after the SJK tag (else after the sender's name), without
/// the id at its end.
fn copy_body(text: &str) -> &str {
    let text = text.trim_end();
    let body = match text.find(sjk_client::SJK_COPY_TAG) {
        Some(at) => &text[at + sjk_client::SJK_COPY_TAG.len()..],
        None => text.split_once(": ").map_or(text, |(_, body)| body),
    };
    body.rfind("^9^").map_or(body, |end| &body[..end])
}

/// Whether a copy is the message's: from its sender when the hub knows who is in the
/// copy's slot, and its words are among the message's (a piece of a long one).
fn is_copy_of(event: &ServerEvent, key_id: &str, message_words: &str, slots: &[SlotMark]) -> bool {
    let sender = event
        .sender
        .and_then(|slot| slots.get(usize::from(slot)))
        .filter(|mark| mark.sjk && !mark.key_id.is_empty());
    sender.is_none_or(|mark| mark.key_id == key_id)
        && message_words.contains(&words(copy_body(&event.text)))
}

impl ChatOverlay {
    /// A line from the game server: true when it is the server's copy of an SJK chat
    /// message, held for the message or dropped as the message already showed.
    pub(crate) fn hold_server_copy(&mut self, event: &ServerEvent, now: Instant) -> bool {
        if event.kind != ServerEventKind::Chat || self.server_copies.held.len() >= KEPT {
            return false;
        }
        let Some(id) = sjk_client::sjk_copy_id(&event.text) else {
            return false;
        };
        if let Some(seen) = self
            .server_copies
            .seen
            .iter()
            .find(|seen| seen.id == id && now < seen.until)
        {
            if !is_copy_of(event, &seen.key_id, &seen.words, &self.slot_marks) {
                return false;
            }
            self.mark_said_in_all(id);
            return true;
        }
        self.server_copies.held.push(Held {
            id,
            until: now + HOLD,
            event: event.clone(),
        });
        true
    }

    /// The held copies whose message did not come in time, to show as they came.
    pub(crate) fn released_server_copies(&mut self, now: Instant) -> Vec<ServerEvent> {
        let held = std::mem::take(&mut self.server_copies.held);
        let (due, kept): (Vec<_>, Vec<_>) = held.into_iter().partition(|held| now >= held.until);
        self.server_copies.held = kept;
        due.into_iter().map(|held| held.event).collect()
    }

    /// Follow the SJK chat for the server's copies: `messages` just joined, so the
    /// server's copies of each go unshown and its line reads ALL/SJK, and the player's
    /// own (`own_key`), back with its id, is ready to be said; a refusal (`outcome`)
    /// drops it, as it never reached SJK chat.
    pub(crate) fn follow_server_copies<'m>(
        &mut self,
        messages: impl IntoIterator<Item = &'m ChatMessage>,
        own_key: Option<&str>,
        outcome: Option<&ReportOutcome>,
        now: Instant,
    ) {
        for message in messages {
            let message_words = words(&message.text);
            let slots = &self.slot_marks;
            let copies = &mut self.server_copies;
            let before = copies.held.len();
            copies.held.retain(|held| {
                held.id != message.id
                    || !is_copy_of(&held.event, &message.key_id, &message_words, slots)
            });
            let copied = copies.held.len() != before;
            if copies.seen.len() == KEPT {
                copies.seen.pop_front();
            }
            copies.seen.push_back(Seen {
                id: message.id,
                until: now + SEEN,
                key_id: message.key_id.clone(),
                words: message_words,
            });
            let own = own_key == Some(message.key_id.as_str());
            let said = own
                && copies
                    .waiting
                    .take_if(|waiting| waiting.text == message.text)
                    .map(|waiting| {
                        copies
                            .ready
                            .push((waiting.server, (waiting.text, message.id)));
                    })
                    .is_some();
            if copied || said {
                self.mark_said_in_all(message.id);
            }
        }
        if let Some(outcome) = outcome.filter(|outcome| !outcome.sent) {
            self.server_copies
                .waiting
                .take_if(|waiting| outcome.serial > waiting.outcome);
        }
    }

    /// The messages to say in the server's chat that are due in the match `server`
    /// (`None` outside a match on a game server): what was `said` there
    /// ([`take_said`]), once the hub took it. Called only while JoF tools say SJK chat
    /// on this server (`sjk_chat_frame.rs`). Copies for another match are dropped, and
    /// so is a message the hub did not answer within [`ANSWER`].
    #[cfg_attr(not(feature = "mod-jof"), allow(dead_code))]
    pub(crate) fn due_server_copies(
        &mut self,
        said: Vec<(String, u64)>,
        server: Option<i32>,
        now: Instant,
    ) -> Vec<Copy> {
        let copies = &mut self.server_copies;
        if let Some(server) = server {
            for (text, outcome) in said {
                copies.waiting = Some(Waiting {
                    text: sjk_identity::chat::check(&text).unwrap_or(text),
                    server,
                    outcome,
                    since: now,
                });
            }
        }
        copies
            .waiting
            .take_if(|waiting| now.saturating_duration_since(waiting.since) >= ANSWER);
        std::mem::take(&mut copies.ready)
            .into_iter()
            .filter(|(at, _)| Some(*at) == server)
            .map(|(_, copy)| copy)
            .collect()
    }

    /// Whether a mod says SJK chat in this server's All chat, with the words the field
    /// and the SJK chat page show then (`None`: not said there).
    pub(crate) fn set_said_in_all(&mut self, words: Option<SaidInAll>) {
        // A world shot sets the note itself, with no session to say it on.
        if (words.is_none() && self.shown_for_shot()) || self.said_in_all == words {
            return;
        }
        self.said_in_all = words;
        if let Ok(mut said) = SAID_IN_ALL.lock() {
            *said = words;
        }
    }

    /// Nothing is said on this server: forget a message waiting for the hub.
    pub(crate) fn stop_server_copies(&mut self) {
        self.server_copies.waiting = None;
        self.server_copies.ready.clear();
    }

    /// The SJK chat line of message `id` was said in All chat too: it reads ALL/SJK.
    fn mark_said_in_all(&mut self, id: u64) {
        for line in &mut self.lines {
            if let Some(hub) = line.hub.as_mut().filter(|hub| hub.id == id) {
                hub.in_all = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str) -> ServerEvent {
        ServerEvent {
            kind: ServerEventKind::Chat,
            text: text.to_owned(),
            sender: Some(3),
            is_private: false,
        }
    }

    fn message(id: u64, key_id: &str, text: &str) -> ChatMessage {
        ChatMessage {
            reactions: Default::default(),
            id,
            at: 0,
            key_id: key_id.to_owned(),
            name: "Sol".to_owned(),
            verified: false,
            tick: sjk_identity::Tick::None,
            staff: false,
            creator: false,
            trust: None,
            emoji: String::new(),
            client: "ejk".to_owned(),
            text: text.to_owned(),
            holocron: None,
            official: None,
        }
    }

    /// Slot 3 is the hub's key `key`.
    fn claim(chat: &mut ChatOverlay, key: &str) {
        chat.slot_marks = vec![SlotMark::default(); crate::chat_mutes::SLOTS];
        chat.slot_marks[3] = SlotMark {
            sjk: true,
            key_id: key.to_owned(),
            ..SlotMark::default()
        };
    }

    #[test]
    fn a_servers_copy_waits_for_its_message_and_goes_unshown() {
        let mut chat = ChatOverlay::new();
        let now = Instant::now();
        assert!(chat.hold_server_copy(&line("Sol^7: ^2^3SJK^2 hi^9^2^1^2"), now));
        assert!(
            !chat.hold_server_copy(&line("Sol^7: ^2hi"), now),
            "a plain line shows"
        );
        let mut team = line("Sol^7: ^2hi^9^2^1^2");
        team.kind = ServerEventKind::TeamChat;
        assert!(
            !chat.hold_server_copy(&team, now),
            "team chat is never a copy"
        );
        chat.follow_server_copies([&message(12, "a", "hi")], None, None, now);
        assert!(
            chat.released_server_copies(now + HOLD).is_empty(),
            "the message showed"
        );
        // A copy that comes after its message is dropped too, and so is one without the
        // tag (another client's), read after the sender's name.
        assert!(chat.hold_server_copy(&line("Sol^7: ^2^3SJK^2 hi^9^2^1^2"), now));
        assert!(chat.hold_server_copy(&line("Sol^7: ^2hi^9^2^1^2"), now));
        assert!(chat.released_server_copies(now + HOLD).is_empty());
    }

    #[test]
    fn a_copy_whose_message_does_not_come_shows_as_it_came() {
        let mut chat = ChatOverlay::new();
        let now = Instant::now();
        let copy = line("Sol^7: ^2hi^9^1^7");
        assert!(chat.hold_server_copy(&copy, now));
        assert!(chat.released_server_copies(now).is_empty(), "not yet");
        assert_eq!(chat.released_server_copies(now + HOLD), [copy]);
    }

    #[test]
    fn a_copy_hides_only_with_its_senders_slot_and_the_messages_words() {
        let mut chat = ChatOverlay::new();
        let now = Instant::now();
        claim(&mut chat, "b");
        // Other words under the message's id show.
        let other = line("Sol^7: ^2^3SJK^2 something else^9^2^1^2");
        assert!(chat.hold_server_copy(&other, now));
        // So does a copy from a slot the hub knows as someone else.
        let elsewhere = line("Sol^7: ^2^3SJK^2 hi there^9^2^1^2");
        assert!(chat.hold_server_copy(&elsewhere, now));
        chat.follow_server_copies([&message(12, "a", "hi there")], None, None, now);
        assert_eq!(
            chat.released_server_copies(now + HOLD),
            [other, elsewhere.clone()]
        );
        // Late, the same.
        assert!(!chat.hold_server_copy(&elsewhere, now));
        // From its sender, a piece of the message, its colours and emojis lost: hidden.
        claim(&mut chat, "a");
        let piece = line("Sol^7: ^2^3SJK^2 ^1There!^9^2^1^2");
        assert!(chat.hold_server_copy(&piece, now));
        assert!(chat.released_server_copies(now + HOLD).is_empty());
    }

    #[test]
    fn a_line_said_in_both_chats_reads_all_sjk() {
        let mut chat = ChatOverlay::new();
        let now = Instant::now();
        let hub = message(12, "a", "hi");
        chat.push_sjk(&hub, false, now);
        assert!(!chat.lines.back().unwrap().hub.as_ref().unwrap().in_all);
        assert!(chat.hold_server_copy(&line("Sol^7: ^2^3SJK^2 hi^9^2^1^2"), now));
        chat.follow_server_copies([&hub], None, None, now);
        assert!(chat.lines.back().unwrap().hub.as_ref().unwrap().in_all);
    }

    #[test]
    fn the_players_line_said_in_a_match_goes_to_the_server_with_its_id() {
        let mut chat = ChatOverlay::new();
        let now = Instant::now();
        let said = |text: &str, outcome| vec![(text.to_owned(), outcome)];
        assert!(
            chat.due_server_copies(said("  hello   there ", 4), Some(77), now)
                .is_empty(),
            "waits for the hub"
        );
        // Another player's message with the same words is not the player's own.
        chat.follow_server_copies([&message(11, "b", "hello there")], Some("a"), None, now);
        assert!(chat.due_server_copies(Vec::new(), Some(77), now).is_empty());
        chat.follow_server_copies([&message(12, "a", "hello there")], Some("a"), None, now);
        assert_eq!(
            chat.due_server_copies(Vec::new(), Some(77), now),
            [("hello there".to_owned(), 12)]
        );
        // In the menus nothing is said anywhere.
        assert!(
            chat.due_server_copies(said("from the menus", 5), None, now + ANSWER)
                .is_empty()
        );
    }

    #[test]
    fn a_refused_or_unanswered_line_never_goes_to_the_server() {
        let mut chat = ChatOverlay::new();
        let now = Instant::now();
        let said = |text: &str, outcome| vec![(text.to_owned(), outcome)];
        assert!(
            chat.due_server_copies(said("refused", 4), Some(77), now)
                .is_empty()
        );
        let refused = ReportOutcome {
            serial: 4,
            sent: false,
            message: "too fast".to_owned(),
        };
        chat.follow_server_copies([], Some("a"), Some(&refused), now);
        assert!(
            chat.server_copies.waiting.is_some(),
            "an earlier outcome is not this line's"
        );
        let refused = ReportOutcome {
            serial: 5,
            ..refused
        };
        chat.follow_server_copies([], Some("a"), Some(&refused), now);
        assert!(chat.server_copies.waiting.is_none());
        chat.follow_server_copies([&message(12, "a", "refused")], Some("a"), None, now);
        assert!(chat.due_server_copies(Vec::new(), Some(77), now).is_empty());
        // Unanswered.
        chat.due_server_copies(said("unanswered", 5), Some(77), now);
        assert!(
            chat.due_server_copies(Vec::new(), Some(77), now + ANSWER)
                .is_empty()
        );
        assert!(chat.server_copies.waiting.is_none());
        // A copy for a match that ended is dropped.
        chat.due_server_copies(said("left", 5), Some(77), now);
        chat.follow_server_copies([&message(13, "a", "left")], Some("a"), None, now);
        assert!(chat.due_server_copies(Vec::new(), Some(78), now).is_empty());
    }
}
