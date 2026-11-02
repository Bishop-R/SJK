//! Announcements and contributor reward rounds (`PROTOCOL.md` in Sol-Vulpes/SJK-hub,
//! "Announcements" and "Reward rounds", 25/10/2026): what the hub sends about them, the
//! creator's request to announce, and the rules an announcement keeps.
//!
//! An announcement is an official message from the SJK team, shown in the SJK chat only
//! (never in a game server's chat). A reward round is a list of contributors the SJK team
//! rewards: everyone reads who and why, and each person claims what they got, which only
//! they see. The feed relays both as they are made ([`crate::Feed::announcements`],
//! [`crate::Feed::rounds`]), and the chat shows each as a line marked [`Official`] (a
//! [`crate::ChatMessage`] whose [`crate::ChatMessage::official`] is set), so the game's
//! chat, the docked chat and the chat page show it where it came.

use crate::wire::Profile;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Longest title, in characters.
pub const TITLE_MAX: usize = 80;
/// Longest text, in characters.
pub const TEXT_MAX: usize = 2_000;
/// Most lines a text holds.
pub const LINES_MAX: usize = 30;

/// One announcement (`GET /v1/announcements`, the feed's `announcements`).
#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
pub struct Announcement {
    /// The hub's own number for it: later ones have higher numbers.
    pub id: u64,
    /// When it was made, unix seconds.
    pub at: i64,
    /// `news` or `release`.
    pub kind: String,
    /// One line, 1 to [`TITLE_MAX`] characters. Show it through [`plain_line`].
    pub title: String,
    /// The text, line breaks kept, blank lines between paragraphs. Show it through
    /// [`plain_text`].
    pub text: String,
    /// A release's version (`2026.1025.1`); empty for news.
    #[serde(default)]
    pub version: String,
    /// The news pictures the text names (`[[image:<id>]]`, [`crate::news_markup`]):
    /// each id's width and height, so a page lays them out before they arrive. Hubs
    /// before 31/10/2026 leave it out.
    #[serde(default)]
    pub images: BTreeMap<String, [u32; 2]>,
}

impl Announcement {
    /// Whether it announces a release.
    pub fn is_release(&self) -> bool {
        self.kind == "release"
    }

    /// The text's first line that holds something, as shown ([`plain_line`]), links as
    /// their labels and pictures left out ([`crate::news_markup::plain`]).
    pub fn first_line(&self) -> String {
        plain_text(&crate::news_markup::plain(&self.text))
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or_default()
            .to_owned()
    }

    /// Whether the text says more than its first line.
    pub fn has_more(&self) -> bool {
        if !crate::news_markup::images(&self.text).is_empty() {
            return true;
        }
        plain_text(&crate::news_markup::plain(&self.text))
            .lines()
            .filter(|line| !line.trim().is_empty())
            .nth(1)
            .is_some()
    }
}

/// The hub's answer to `GET /v1/announcements`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct Announcements {
    /// The newest (up to 10, at most 30 days old), newest first.
    #[serde(default)]
    pub announcements: Vec<Announcement>,
}

/// An announcement the feed relays as it is made.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct FeedAnnouncement {
    /// Its id in the feed's sequence.
    pub id: u64,
    /// The announcement.
    pub announcement: Announcement,
}

/// A reward round the feed relays as it is made (`rounds`): every reader is told, and
/// reads `GET /v1/rewards` again.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct FeedRound {
    /// Its id in the feed's sequence.
    pub id: u64,
    /// The round's number at the hub.
    pub round: u64,
    /// When the round was made, unix seconds.
    #[serde(default)]
    pub at: i64,
    /// Its title (`Contributor rewards #1`).
    pub title: String,
    /// How many people it rewards.
    #[serde(default)]
    pub count: u32,
}

/// One person's reward in a round (`GET /v1/rewards`): what it gives is private to them.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct Reward {
    /// Its number at the hub (what a claim names).
    pub id: u64,
    /// The round it belongs to.
    pub round: u64,
    /// The round's title.
    #[serde(default)]
    pub round_title: String,
    /// When the round was made, unix seconds.
    #[serde(default)]
    pub at: i64,
    /// Why: one line, which the round's public list shows too.
    #[serde(default)]
    pub reason: String,
    /// Holocrons it gives, by tier id (`rare`), each with its count.
    #[serde(default)]
    pub holocrons: BTreeMap<String, u32>,
    /// Credits it gives (0 for none).
    #[serde(default)]
    pub credits: u64,
    /// Kyber shards it gives (0 for none).
    #[serde(default)]
    pub shards: u64,
    /// When it was claimed, unix seconds; `None` while it waits.
    #[serde(default)]
    pub claimed: Option<i64>,
}

/// One person a round rewards, as everyone reads it: no amounts.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct RoundPerson {
    /// Their key id (their person's main key).
    pub key_id: String,
    /// Their display name now, with colour codes. Show it through
    /// [`crate::chat::name_for_display`].
    #[serde(default)]
    pub name: String,
    /// Whether the hub's operator vouches for them.
    #[serde(default)]
    pub verified: bool,
    /// Why they are rewarded.
    #[serde(default)]
    pub reason: String,
}

/// A reward round as everyone reads it.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct Round {
    /// Its number at the hub.
    pub id: u64,
    /// When it was made, unix seconds.
    #[serde(default)]
    pub at: i64,
    /// Its title.
    pub title: String,
    /// A line shown above the list; empty for none.
    #[serde(default)]
    pub note: String,
    /// Who it rewards and why, in the order the SJK team gave.
    #[serde(default)]
    pub people: Vec<RoundPerson>,
}

/// The hub's answer to `GET /v1/rewards`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct Rewards {
    /// The person's rewards not claimed yet, oldest first.
    #[serde(default)]
    pub pending: Vec<Reward>,
    /// Their newest claimed rewards (up to 20), newest first.
    #[serde(default)]
    pub claimed: Vec<Reward>,
    /// The newest rounds (up to 10), newest first.
    #[serde(default)]
    pub rounds: Vec<Round>,
}

/// The hub's answer to a claim (`POST /v1/rewards/claim`).
#[derive(Clone, Debug, Deserialize)]
pub struct ClaimAnswer {
    /// The person's profile after it: the gift holocrons it gave are in it.
    pub profile: Profile,
    /// The rewards after it.
    #[serde(default)]
    pub rewards: Rewards,
}

/// What marks a chat line as the SJK team's: an announcement or a reward round. Never
/// read from the wire: the feed (or the service, for announcements read at the
/// registration) sets it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Official {
    /// An announcement; the line shows its title and first line.
    Announcement(Announcement),
    /// A reward round; the line says how many it rewards.
    Round(FeedRound),
}

/// A news picture the hub stored (`PUT /v1/staff/news-image`, 31/10/2026): its id, which
/// an announcement names as `[[image:<id>]]`, and its size.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct NewsImage {
    /// Its id: 16 lowercase hex digits.
    pub image: String,
    /// Its width as the hub wrote it, in pixels.
    pub width: u32,
    /// Its height, in pixels.
    pub height: u32,
}

/// Largest news picture the hub sends (it writes them at most 1600 x 900).
pub const NEWS_IMAGE_MAX: u64 = 4 * 1024 * 1024;
/// Largest picture the SJK team may upload, as the hub takes it.
pub const NEWS_UPLOAD_MAX: usize = 3 * 1024 * 1024;

/// Where the hub serves news picture `id` (no signature).
pub fn news_image_path(id: &str) -> String {
    format!("/v1/news-image/{id}")
}

/// What the SJK team asks the hub to announce (`POST /v1/staff/announce`; hubs before
/// 31/10/2026 take only the creator's, at `POST /v1/creator/announce`).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AnnounceRequest {
    /// A release's announcement (`release`) or news (`news`).
    pub release: bool,
    /// Its title.
    pub title: String,
    /// Its text.
    pub text: String,
    /// The release's version, only for a release.
    pub version: String,
}

/// Which rule an announcement breaks, with the hub's code for it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnnounceError {
    /// The title is empty, longer than [`TITLE_MAX`], more than one line, or holds a
    /// character outside the chat's alphabet (or a `^`).
    Title,
    /// The text is empty, longer than [`TEXT_MAX`] or [`LINES_MAX`] lines, or holds a
    /// character outside the chat's alphabet (or a `^`).
    Text,
    /// A release without a version that reads as one (`2026.1025.1`).
    Version,
    /// A link, picture or switch written wrong ([`crate::news_markup::check`]).
    Markup,
}

impl AnnounceError {
    /// The hub's error code for the rule.
    pub fn code(self) -> &'static str {
        match self {
            Self::Title => "bad_title",
            Self::Text => "bad_text",
            Self::Version => "bad_version",
            Self::Markup => "bad_markup",
        }
    }

    /// What the creator is told.
    pub fn message(self) -> &'static str {
        match self {
            Self::Title => "The title is one line of 1 to 80 characters, without ^ codes.",
            Self::Text => "The text is 1 to 2000 characters on at most 30 lines, without ^ codes.",
            Self::Version => "A release needs its version, like 2026.1025.1.",
            Self::Markup => {
                "A link, picture or switch is not written as [Label](open:<page>),                              [[image:<id>]] or [[switch:<cvar>]]."
            }
        }
    }
}

impl std::fmt::Display for AnnounceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

/// Whether `c` may stand in an announcement: the chat's alphabet without colour codes,
/// and the backtick that marks code (drawn in chips, as in the changelog; 31/10/2026).
fn allowed(c: char) -> bool {
    c == '`' || (c != '^' && crate::bio::allowed(c))
}

/// The request ready to send (the title's spaces made one and trimmed, the text tidied as
/// the hub stores it: [`crate::bio::tidy`]), or the first rule it breaks, as the hub
/// checks it.
pub fn check(request: &AnnounceRequest) -> Result<AnnounceRequest, AnnounceError> {
    if request.title.contains(['\n', '\r']) {
        return Err(AnnounceError::Title);
    }
    let title = request
        .title
        .split([' ', '\t'])
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let count = title.chars().count();
    if count == 0 || count > TITLE_MAX || !title.chars().all(allowed) {
        return Err(AnnounceError::Title);
    }
    let text = crate::bio::tidy(&request.text);
    let count = text.chars().count();
    if count == 0
        || count > TEXT_MAX
        || text.lines().count() > LINES_MAX
        || !text.chars().all(|c| c == '\n' || allowed(c))
    {
        return Err(AnnounceError::Text);
    }
    // The hub alone knows which pictures it holds; it checks their ids.
    if crate::news_markup::check(&text, |_| true).is_err() {
        return Err(AnnounceError::Markup);
    }
    let version = request.version.trim().to_owned();
    if request.release && !valid_version(&version) {
        return Err(AnnounceError::Version);
    }
    Ok(AnnounceRequest {
        release: request.release,
        title,
        text,
        version: if request.release {
            version
        } else {
            String::new()
        },
    })
}

/// Whether `version` reads as a release's (`2026.1025.1`: three numbers).
pub fn valid_version(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty() && part.len() <= 9 && part.bytes().all(|b| b.is_ascii_digit())
        })
}

/// A one-line text from the hub (a title, a reason, a note) as a page may show it: only
/// the chat's alphabet, no colour codes, spaces made one, at most `max` characters.
pub fn plain_line(raw: &str, max: usize) -> String {
    raw.split(|c: char| c.is_whitespace())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| allowed(*c))
        .take(max)
        .collect::<String>()
        .trim()
        .to_owned()
}

/// An announcement's text from the hub as a page may show it: each line as
/// [`plain_line`] keeps it, blank lines kept as one, at most [`LINES_MAX`] lines and
/// [`TEXT_MAX`] characters.
pub fn plain_text(raw: &str) -> String {
    let lines: Vec<String> = raw
        .replace("\r\n", "\n")
        .split('\n')
        .map(|line| plain_line(line, TEXT_MAX))
        .collect();
    let tidied = crate::bio::tidy(&lines.join("\n"));
    tidied
        .lines()
        .take(LINES_MAX)
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(TEXT_MAX)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wire_reads_as_the_spec_writes_it() {
        let one: Announcement = serde_json::from_str(
            r#"{"id":3,"at":1761400000,"kind":"release","title":"SJK 2026.1025.1",
            "text":"Line one\n\n- a point\n- another","version":"2026.1025.1"}"#,
        )
        .unwrap();
        assert!(one.is_release());
        assert_eq!(one.version, "2026.1025.1");
        assert_eq!(one.first_line(), "Line one");
        assert!(one.has_more());
        let news: Announcement =
            serde_json::from_str(r#"{"id":4,"at":1,"kind":"news","title":"Hi","text":"Only"}"#)
                .unwrap();
        assert!(!news.is_release() && news.version.is_empty() && !news.has_more());
        let rewards: Rewards = serde_json::from_str(
            r#"{"pending":[{"id":7,"round":2,"round_title":"Contributor rewards #1","at":5,
            "reason":"Fixed the fog","holocrons":{"rare":2,"legendary":1},"credits":500,
            "shards":0,"claimed":null}],"claimed":[],
            "rounds":[{"id":2,"at":5,"title":"Contributor rewards #1","note":"Thanks!",
            "people":[{"key_id":"ab","name":"^2Sol","verified":true,"reason":"Fixed the fog"}]}]}"#,
        )
        .unwrap();
        let reward = &rewards.pending[0];
        assert_eq!(reward.holocrons.get("rare"), Some(&2));
        assert_eq!(
            (reward.credits, reward.shards, reward.claimed),
            (500, 0, None)
        );
        assert_eq!(rewards.rounds[0].people[0].reason, "Fixed the fog");
        let round: FeedRound = serde_json::from_str(
            r#"{"id":12,"round":2,"at":5,"title":"Contributor rewards #1","count":7}"#,
        )
        .unwrap();
        assert_eq!((round.round, round.count), (2, 7));
        let empty: Rewards = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, Rewards::default());
    }

    #[test]
    fn an_announcement_is_checked_as_the_hub_checks_it() {
        let request = |title: &str, text: &str| AnnounceRequest {
            release: false,
            title: title.to_owned(),
            text: text.to_owned(),
            version: "ignored".to_owned(),
        };
        let ok = check(&request("  Big   news ", "One  line\n\n\n\n- two  \n")).unwrap();
        assert_eq!(ok.title, "Big news");
        assert_eq!(ok.text, "One line\n\n- two");
        assert!(ok.version.is_empty(), "news has no version");
        assert_eq!(check(&request("", "x")), Err(AnnounceError::Title));
        assert_eq!(check(&request("a\nb", "x")), Err(AnnounceError::Title));
        assert_eq!(check(&request("^1Red", "x")), Err(AnnounceError::Title));
        assert_eq!(
            check(&request(&"t".repeat(81), "x")),
            Err(AnnounceError::Title)
        );
        assert_eq!(check(&request("t", "  ")), Err(AnnounceError::Text));
        assert_eq!(check(&request("t", "^2no")), Err(AnnounceError::Text));
        assert_eq!(
            check(&request("t", &"x".repeat(2_001))),
            Err(AnnounceError::Text)
        );
        assert_eq!(
            check(&request("t", &"x\n".repeat(31))),
            Err(AnnounceError::Text)
        );
        assert!(check(&request("t", &"x\n".repeat(30))).is_ok());
        let release = AnnounceRequest {
            release: true,
            version: " 2026.1025.1 ".to_owned(),
            ..request("SJK 2026.1025.1", "New")
        };
        assert_eq!(check(&release).unwrap().version, "2026.1025.1");
        for bad in ["", "2026.1025", "2026.x.1", "v2026.1025.1"] {
            let release = AnnounceRequest {
                version: bad.to_owned(),
                ..release.clone()
            };
            assert_eq!(check(&release), Err(AnnounceError::Version), "{bad:?}");
        }
        assert_eq!(AnnounceError::Text.code(), "bad_text");
    }

    #[test]
    fn hub_text_shows_without_codes_or_odd_characters() {
        assert_eq!(plain_line("  ^1Red \u{202E}title  ", 80), "1Red title");
        assert_eq!(plain_line("abcdef", 3), "abc");
        assert_eq!(plain_text("a\r\n\n\n\nb ^ c"), "a\n\nb c");
        assert_eq!(plain_text(&"x\n".repeat(40)).lines().count(), LINES_MAX);
    }
}
