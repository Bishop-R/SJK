//! The SJK team's chat lines (`docs/hub-chat.md`, "Announcements and reward rounds"): an
//! announcement or a reward round as the game's chat, the docked chat and the chat page
//! show it. The identity service relays each as a chat message with no text and
//! [`sjk_identity::ChatMessage::official`] set; this words it. Every view draws the same
//! parts: the sender "SJK" in the SJK UI's accent gold, the mark where a player's tick
//! goes (SJK's logo for an announcement, the "from the SJK team" gift box for a round),
//! then the words, the announcement's title in bold.

use sjk_identity::ChatMessage;
use sjk_identity::news::{Official, plain_line};

/// The longest first line a line shows, in characters, before "...": about two rows of
/// the docked chat.
const FIRST_LINE_MAX: usize = 110;

/// What a click on the line (or `announcement`, `rewards`) opens.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Target {
    /// The announcement with this hub number.
    Announcement(u64),
    /// The round list, on the round with this hub number.
    Round(u64),
}

/// How an SJK team's line looks in a view, beside its words.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Look {
    /// What a click opens.
    pub(crate) target: Target,
    /// How many bytes of the words are the title, drawn in bold; 0 for none.
    pub(crate) bold: usize,
}

impl Look {
    /// The mark is the gift box (a round), else SJK's logo.
    pub(crate) fn gift(self) -> bool {
        matches!(self.target, Target::Round(_))
    }
}

/// The sender every SJK team line shows.
pub(crate) const SENDER: &str = "SJK";

/// The words of an SJK team line and how it looks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Words {
    pub(crate) text: String,
    pub(crate) look: Look,
}

/// The words of `message` if it is the SJK team's, ending in `read_more` when the
/// announcement says more than its line shows ("Read more" where a click opens it, the
/// command in the game's chat).
pub(crate) fn words(message: &ChatMessage, read_more: &str) -> Option<Words> {
    match message.official.as_ref()? {
        Official::Announcement(announcement) => {
            let title = plain_line(&announcement.title, sjk_identity::news::TITLE_MAX);
            let first = announcement.first_line();
            let cut = first.chars().count() > FIRST_LINE_MAX;
            let mut shown: String = first.chars().take(FIRST_LINE_MAX).collect();
            if cut {
                shown = shown.trim_end().to_owned();
                shown.push_str("...");
            }
            let mut text = title.clone();
            if !shown.is_empty() {
                text.push(':');
            }
            let bold = text.len();
            if !shown.is_empty() {
                text.push(' ');
                text.push_str(&shown);
            }
            if cut || announcement.has_more() {
                text.push_str("  ");
                text.push_str(read_more);
            }
            Some(Words {
                text,
                look: Look {
                    target: Target::Announcement(announcement.id),
                    bold,
                },
            })
        }
        Official::Round(round) => Some(Words {
            text: round_text(&round.title, round.count),
            look: Look {
                target: Target::Round(round.round),
                bold: 0,
            },
        }),
    }
}

/// The words the game's chat shows for `message` if it is the SJK team's: it cannot be
/// clicked, so it names the console command that opens the announcement or the rounds,
/// and a round's line starts with the gift box.
pub(crate) fn game_words(message: &ChatMessage) -> Option<String> {
    let words = words(message, "Read more: /announcement")?;
    Some(match words.look.target {
        Target::Round(_) => format!("{} {}: /rewards", crate::symbols::GIFT, words.text),
        Target::Announcement(_) => words.text,
    })
}

/// A round's line: "Contributor rewards #1: 7 players rewarded. See the list".
pub(crate) fn round_text(title: &str, count: u32) -> String {
    let title = plain_line(title, sjk_identity::news::TITLE_MAX);
    let players = if count == 1 {
        "1 player rewarded".to_owned()
    } else {
        format!("{count} players rewarded")
    };
    format!("{title}: {players}. See the list")
}

#[cfg(test)]
mod tests {
    use super::*;
    use sjk_identity::news::{Announcement, FeedRound};

    pub(crate) fn line(official: Official) -> ChatMessage {
        ChatMessage {
            reactions: Default::default(),
            id: 1,
            at: 0,
            key_id: String::new(),
            name: SENDER.to_owned(),
            verified: false,
            tick: sjk_identity::Tick::None,
            staff: false,
            creator: false,
            trust: None,
            emoji: String::new(),
            client: String::new(),
            text: String::new(),
            holocron: None,
            official: Some(official),
        }
    }

    fn announcement(text: &str) -> Official {
        Official::Announcement(Announcement {
            id: 3,
            at: 0,
            kind: "news".to_owned(),
            title: "Big ^1news".to_owned(),
            text: text.to_owned(),
            version: String::new(),
            images: Default::default(),
        })
    }

    #[test]
    fn an_announcement_reads_its_title_in_bold_then_its_first_line() {
        let words = words(&line(announcement("Hello all")), "Read more").unwrap();
        assert_eq!(words.text, "Big 1news: Hello all");
        assert_eq!(&words.text[..words.look.bold], "Big 1news:");
        assert_eq!(words.look.target, Target::Announcement(3));
        assert!(!words.look.gift());
        let more = words_of("Hello all\n\n- a point");
        assert_eq!(more, "Big 1news: Hello all  Read more");
        let long = words_of(&"word ".repeat(40));
        assert!(long.ends_with("...  Read more"), "{long}");
    }

    fn words_of(text: &str) -> String {
        words(&line(announcement(text)), "Read more").unwrap().text
    }

    #[test]
    fn a_round_says_how_many_it_rewards() {
        let round = |count| {
            Official::Round(FeedRound {
                id: 9,
                round: 2,
                at: 0,
                title: "Contributor rewards #1".to_owned(),
                count,
            })
        };
        let seven = words(&line(round(7)), "").unwrap();
        assert_eq!(
            seven.text,
            "Contributor rewards #1: 7 players rewarded. See the list"
        );
        assert!(seven.look.gift() && seven.look.bold == 0);
        assert_eq!(seven.look.target, Target::Round(2));
        assert_eq!(
            words(&line(round(1)), "").unwrap().text,
            "Contributor rewards #1: 1 player rewarded. See the list"
        );
    }
}
