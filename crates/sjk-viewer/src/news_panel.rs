//! The News panel (`docs/sjk-ui.md`, "News panel"; Sol's request, 31/10/2026: the
//! what's-new card and the announcements "are kinda the same", one panel for both, with
//! working switches, pictures and links). It replaced the what's-new card and the
//! announcement card: one SJK pop-up over the darkened screen, the same look in every
//! menu style, on the 16:9 frame.
//!
//! - **Left, the entries** newest first: "Settings to review" when an update asks for
//!   it, the SJK team's announcements (the hub's newest ten) and the recaps of the
//!   recent releases (`CHANGELOG.md`). A release's announcement goes on its release's
//!   entry, above the recap. An entry the player has not read wears a gold dot.
//! - **Right, the entry in full:** its kicker and date, its title, then its text:
//!   recap headings with their icons over their sentences, paragraphs and gold bullets,
//!   commands and cvars in chips, links (`[Label](target)`, [`crate::ui_links`]),
//!   pictures (`[[image:<id>]]`, [`crate::news_images`]) and switches (`= <cvar>:` in the
//!   changelog, `[[switch:<cvar>]]` in an announcement), which flip the setting at once
//!   and keep the panel up. Up to fill the pane, the wheel and Page Up/Down scroll it.
//!
//! **When it opens on its own** (on the main menu, or over the game menu in a match,
//! never in the middle of play, after the other cards): after an update, on that
//! update's settings to review or its recap (`cl_whatsNewSeen` holds the version whose
//! panel was closed, as the card's did), and when an announcement came that the player
//! has not read (`announcements_seen.txt`, [`crate::official::seen`]). Asked for (the
//! chat line's "Read more", `news`, `whatsnew`, `announcement [n]`) it shows at once,
//! over play too, with the cursor free.
//!
//! The update's bookkeeping is the card's: a new installation writes the marker and
//! shows nothing, a local build no release lists shows nothing, and the settings an
//! update marks `reset` go back to their default once per version ([`RESET_CVAR`]).

use crate::console::changelog::{Digest, Review};
use crate::inline_code::{self, Action, Look, Measures, Place, Row};
use crate::menu::sjk::{Frame, TextTarget, color, key_hint, key_hint_width, kit, text};
use crate::menu_widgets::{MenuCanvas, TextFamily};
use sjk_identity::Announcement;
use sjk_identity::news_markup::{self, Line};
use sjk_ui::{Color, DrawCommand, FontWeight, InputEvent, TextAlign, UiEventKind};
use std::collections::BTreeMap;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// The version whose update the player closed the panel on.
pub(crate) const SEEN_CVAR: &str = "cl_whatsNewSeen";
/// The version whose settings to reset (`= <cvar> reset:`) were put back to their
/// default, so they are reset once, though the panel stays unanswered over several runs.
pub(crate) const RESET_CVAR: &str = "cl_whatsNewReset";
/// `whatsnew`: the panel on the newest release's recap.
pub(crate) const COMMAND: &str = "whatsnew";
pub(crate) const HELP: &str = "Show what the latest SJK update brought";
/// `news`: the panel on its newest entry.
pub(crate) const NEWS_COMMAND: &str = "news";
pub(crate) const NEWS_HELP: &str = "Open the News: SJK's releases and the SJK team's announcements";

/// How many releases' recaps the panel lists.
const RELEASES: usize = 6;

const CLOSE_TOKEN: u16 = 1;
const CHANGELOG_TOKEN: u16 = 2;
const UPDATE_TOKEN: u16 = 3;
const LIST_TOKEN: u16 = 4;
const BODY_TOKEN: u16 = 5;
/// The switches on show, `SWITCH_BASE + n`.
const SWITCH_BASE: u16 = 10;
const SWITCH_LIMIT: usize = 40;
/// The entries, `ENTRY_BASE + n`.
const ENTRY_BASE: u16 = 50;
const ENTRY_LIMIT: usize = 50;
/// The chips and links on show, in reading order.
const ACTION_BASE: u16 = 100;
const _: () = assert!(SWITCH_BASE as usize + SWITCH_LIMIT <= ENTRY_BASE as usize);
const _: () = assert!(ENTRY_BASE as usize + ENTRY_LIMIT <= ACTION_BASE as usize);

/// The panel, in frame pixels.
const PANEL_X: f32 = 150.0;
const PANEL_Y: f32 = 70.0;
const PANEL_WIDTH: f32 = 1620.0;
const PANEL_HEIGHT: f32 = 900.0;
/// The list of entries, down the panel's left.
const LIST_WIDTH: f32 = 420.0;
const LIST_PAD: f32 = 26.0;
const LIST_TOP: f32 = 92.0;
const ENTRY_HEIGHT: f32 = 76.0;
const ENTRIES_SHOWN: usize = 10;
/// The pane beside it.
const PANE_X: f32 = PANEL_X + LIST_WIDTH + 48.0;
const PANE_WIDTH: f32 = PANEL_X + PANEL_WIDTH - 48.0 - PANE_X;
const BODY_TOP: f32 = PANEL_Y + 146.0;
const BODY_BOTTOM: f32 = PANEL_Y + PANEL_HEIGHT - 100.0;
/// A recap line's icon, its heading's height and size, the indent of what comes under.
const ICON: f32 = 26.0;
const INDENT: f32 = ICON + 12.0;
const HEADING_HEIGHT: f32 = 30.0;
const HEADING_SIZE: f32 = 21.0;
const BLOCK_GAP: f32 = 16.0;
const BULLET_INDENT: f32 = 26.0;
/// The tallest a picture is drawn.
const IMAGE_MAX_HEIGHT: f32 = 470.0;
const LOOK: Look = Look {
    size: 18.0,
    line: 28.0,
    chip_size: 17.0,
    chip_pad: 7.0,
    chip_height: 25.0,
    chip_radius: 6.0,
};
const CREDIT_LOOK: Look = Look {
    size: 16.0,
    line: 24.0,
    ..LOOK
};
/// Introduction lines shown for a release without a recap.
const INTRO_LINES: usize = 6;
const BUTTON_HEIGHT: f32 = 46.0;
const BUTTON_GAP: f32 = 14.0;
const CLOSE_WIDTH: f32 = 150.0;
const CHANGELOG_WIDTH: f32 = 230.0;
const UPDATE_WIDTH: f32 = 150.0;
const KEYS_Y: f32 = 992.0;
const KEYS_RIGHT: f32 = 1824.0;

/// The keys that flip the switches on show, the first nine.
const DIGITS: [KeyCode; 9] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

/// The icon a recap heading wears: the first entry with a word that starts a word of
/// the heading (any case), else [`HEADING_ICON`]. Settings and quick wheel icons
/// (`settings_icons::texture`). A heading written for a new theme falls back until an
/// entry names it.
const HEADING_ICONS: &[(&[&str], &str)] = &[
    (
        &["server", "browser", "download", "network", "master"],
        "network",
    ),
    (&["scoreboard"], "scoreboard"),
    (&["hud", "frame rate", "clock", "kill feed"], "hud"),
    (&["saber", "shader", "blade", "mythical"], "saber_style"),
    (&["clan", "nameplate", "name"], "nameplates"),
    (
        &["holocron", "collection", "cosmetic", "hat", "cape", "toy"],
        "cosmetics",
    ),
    (
        &[
            "card",
            "profile",
            "identity",
            "medal",
            "team",
            "staff",
            "achievement",
        ],
        "identity",
    ),
    (&["update", "changelog", "release"], "update"),
    (&["weather", "rain", "snow", "fog", "cloud"], "weather"),
    (
        &["graphic", "render", "light", "shadow", "video", "effect"],
        "graphics",
    ),
    (&["sound", "audio", "music", "voice"], "sound"),
    (&["gameplay", "fps", "physics"], "gameplay"),
    (&["movement", "jetpack", "grapple", "strafe"], "movement"),
    (&["camera"], "camera"),
    (&["chat", "text", "font"], "text"),
    (&["setup", "import"], "import"),
    (&["showcase", "tool", "demo", "replay", "record"], "record"),
    (&["menu", "interface", "settings", "ui"], "interface"),
    (&["key", "bind", "control"], "key_bindings"),
    (&["fix", "crash", "stutter", "bug"], "options"),
];
/// The icon of a heading no entry names.
const HEADING_ICON: &str = "changelog";

/// The icon heading `heading` wears ([`HEADING_ICONS`]).
fn heading_icon(heading: &str) -> &'static str {
    let heading = heading.to_ascii_lowercase();
    let starts_word = |word: &str| {
        heading.match_indices(word).any(|(at, _)| {
            heading[..at]
                .chars()
                .next_back()
                .is_none_or(|before| !before.is_ascii_alphanumeric())
        })
    };
    HEADING_ICONS
        .iter()
        .find(|(words, _)| words.iter().any(|word| starts_word(word)))
        .map_or(HEADING_ICON, |(_, icon)| icon)
}

/// What the player chose on the panel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Choice {
    Close,
    Changelog,
    /// Install a release waiting, else open the Update page.
    Update,
    /// A chip: its text (`ViewerConsole::follow_chip`); the panel closes.
    Chip(String),
    /// A link: its target ([`crate::ui_links`]); the panel closes.
    Link(String),
    /// Flip a setting: its cvar. The panel stays.
    Switch(String),
}

/// Which entry it is, to find it again when the list is made anew.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Key {
    Review,
    /// A release, by its changelog title ("2026.1031.1 (Alpha)", "Unreleased").
    Release(String),
    /// An announcement, by its number at the hub.
    Announcement(u64),
}

/// One block of an entry, before it is wrapped to the pane.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Block {
    /// A recap line: its heading and its sentence, code in backticks.
    Line(String, String),
    /// A release's introduction, standing in for a recap it lacks.
    Intro(String),
    /// A switch: its cvar, its Settings name and why it is asked (may be empty).
    Setting(String, &'static str, String),
    /// A line of an announcement, links and code in it; `true` when it is a bullet.
    Text(String, bool),
    /// A picture: its id.
    Image(String),
    /// A paragraph break.
    Gap,
}

/// One entry of the list.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Entry {
    pub(crate) key: Key,
    kicker: &'static str,
    /// The pane's headline, and the list's name for it.
    title: String,
    name: String,
    /// dd/mm/yyyy (and HH:MM for an announcement), or empty.
    date: String,
    /// When, for the order (unix seconds).
    sort: i64,
    blocks: Vec<Block>,
    /// "By Sol and Creyon, after JoF EJK", or empty.
    credit: String,
    pub(crate) unread: bool,
    /// The pictures' sizes.
    images: BTreeMap<String, [u32; 2]>,
    /// A release newer than this build: the Update button shows.
    update: bool,
}

/// What the panel is given to list.
#[derive(Default)]
pub(crate) struct Sources<'a> {
    /// Releases' recaps, newest first.
    pub(crate) digests: &'a [Digest],
    /// The settings to review, as the changelog gives them.
    pub(crate) reviews: &'a [Review],
    /// The hub's announcements.
    pub(crate) announcements: &'a [Announcement],
    /// Releases the player has not seen (their titles).
    pub(crate) new_releases: &'a [String],
    /// The highest announcement number read (announcements above it are new).
    pub(crate) read: u64,
    /// This build's version (empty for a local build).
    pub(crate) installed: &'a str,
}

/// The release's version in its changelog title ("2026.1031.1 (Alpha)").
fn title_version(title: &str) -> &str {
    title.split_whitespace().next().unwrap_or_default()
}

/// dd/mm/yyyy as unix seconds at noon UTC; `None` when it does not read.
fn date_seconds(date: &str) -> Option<i64> {
    let mut parts = date.split('/').map(|part| part.parse::<i64>().ok());
    let (day, month, year) = (parts.next()??, parts.next()??, parts.next()??);
    if !(1..=31).contains(&day) || !(1..=12).contains(&month) {
        return None;
    }
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + 43_200)
}

/// An announcement's lines as blocks: its paragraphs, bullets, pictures and switches (a
/// switch Settings has no on/off row for is left out).
fn announcement_blocks(text: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    for raw in text.split('\n') {
        match news_markup::line(raw) {
            Line::Blank => {
                if blocks.last().is_some_and(|last| *last != Block::Gap) {
                    blocks.push(Block::Gap);
                }
            }
            Line::Image(id) => blocks.push(Block::Image(id.to_owned())),
            Line::Switch(cvar) => {
                if let Some(label) = crate::settings::switch_label(cvar) {
                    blocks.push(Block::Setting(
                        cvar.to_ascii_lowercase(),
                        label,
                        String::new(),
                    ));
                }
            }
            Line::Text { text, bullet } => blocks.push(Block::Text(text.to_owned(), bullet)),
        }
    }
    while blocks.last() == Some(&Block::Gap) {
        blocks.pop();
    }
    blocks
}

/// The entries `sources` make, in the list's order: the settings to review first, then
/// newest first.
pub(crate) fn entries(sources: &Sources<'_>) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    for digest in sources.digests.iter().take(RELEASES) {
        let blocks = if digest.lines.is_empty() {
            (!digest.intro.trim().is_empty())
                .then(|| Block::Intro(digest.intro.clone()))
                .into_iter()
                .collect()
        } else {
            digest
                .lines
                .iter()
                .map(|(heading, sentence)| Block::Line(heading.clone(), sentence.clone()))
                .collect()
        };
        let unreleased = digest.title == "Unreleased";
        entries.push(Entry {
            key: Key::Release(digest.title.clone()),
            kicker: if unreleased { "PREVIEW" } else { "SJK RELEASE" },
            title: if unreleased {
                "What's coming next".to_owned()
            } else {
                format!("What's new in {}", digest.title)
            },
            name: digest.title.clone(),
            date: digest.date.clone(),
            sort: if unreleased {
                i64::MAX - 1
            } else {
                date_seconds(&digest.date).unwrap_or_default()
            },
            blocks,
            credit: credit_line(&digest.people, &digest.after),
            unread: sources.new_releases.contains(&digest.title),
            images: BTreeMap::new(),
            update: false,
        });
    }
    for announcement in sources.announcements {
        let blocks = announcement_blocks(&announcement.text);
        let unread = announcement.id > sources.read;
        let release = announcement.is_release().then(|| {
            entries.iter_mut().find(|entry| {
                matches!(&entry.key, Key::Release(title) if title_version(title) == announcement.version)
            })
        });
        if let Some(Some(entry)) = release {
            // The release's own entry tells it, above its recap.
            let mut text = blocks;
            text.push(Block::Gap);
            text.append(&mut entry.blocks);
            entry.blocks = text;
            entry.unread |= unread;
            entry.images.extend(announcement.images.clone());
            continue;
        }
        let date = crate::holocrons::when_text(announcement.at);
        entries.push(Entry {
            key: Key::Announcement(announcement.id),
            kicker: if announcement.is_release() {
                "SJK RELEASE"
            } else {
                "FROM THE SJK TEAM"
            },
            title: announcement.title.clone(),
            name: announcement.title.clone(),
            date,
            sort: announcement.at,
            blocks,
            credit: String::new(),
            unread,
            images: announcement.images.clone(),
            update: announcement.is_release()
                && crate::official::older_than(sources.installed, &announcement.version),
        });
    }
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.sort));
    let rows: Vec<Block> = sources
        .reviews
        .iter()
        .filter_map(|review| {
            crate::settings::switch_label(&review.cvar).map(|label| {
                Block::Setting(review.cvar.to_ascii_lowercase(), label, review.text.clone())
            })
        })
        .take(SWITCH_LIMIT)
        .collect();
    if !rows.is_empty() {
        entries.insert(
            0,
            Entry {
                key: Key::Review,
                kicker: "SETTINGS TO REVIEW",
                title: "Settings to review".to_owned(),
                name: "Settings to review".to_owned(),
                date: String::new(),
                sort: i64::MAX,
                blocks: rows,
                credit: String::new(),
                unread: true,
                images: BTreeMap::new(),
                update: false,
            },
        );
    }
    entries.truncate(ENTRY_LIMIT);
    entries
}

/// One line of the pane, wrapped: the pane scrolls by these.
#[derive(Clone, Debug, PartialEq)]
enum Item {
    /// A recap heading with its icon.
    Heading(&'static str, String),
    /// A wrapped row, `indent` in, in `tone`; `bullet` draws a gold bullet before it.
    Row {
        row: Row,
        indent: f32,
        tone: Tone,
        bullet: bool,
    },
    /// A switch's row: its icon, its Settings name, its switch's index.
    Setting(&'static str, String, usize),
    /// A picture: its id and the size it is drawn at.
    Image(String, [f32; 2]),
    /// Room.
    Gap(f32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Tone {
    Text,
    Muted,
    Credit,
}

impl Item {
    fn height(&self) -> f32 {
        match self {
            Self::Heading(..) | Self::Setting(..) => HEADING_HEIGHT,
            Self::Row { tone, .. } => {
                if *tone == Tone::Credit {
                    CREDIT_LOOK.line
                } else {
                    LOOK.line
                }
            }
            Self::Image(_, [_, height]) => height + 12.0,
            Self::Gap(height) => *height,
        }
    }
}

/// An entry wrapped to the pane: its items and the cvars of its switches, in order.
#[derive(Debug, Default, PartialEq)]
struct Laid {
    items: Vec<Item>,
    switches: Vec<String>,
}

fn rows_of(text: &str, indent: f32, tone: Tone, measures: &Measures<'_>) -> Vec<Item> {
    let look = if tone == Tone::Credit {
        &CREDIT_LOOK
    } else {
        &LOOK
    };
    inline_code::flow_links(
        text,
        PANE_WIDTH - indent,
        measures,
        look,
        &crate::ui_links::known,
    )
    .into_iter()
    .map(|row| Item::Row {
        row,
        indent,
        tone,
        bullet: false,
    })
    .collect()
}

/// `entry` wrapped to the pane as `measures` draw it.
fn lay_out(entry: &Entry, measures: &Measures<'_>) -> Laid {
    let mut laid = Laid::default();
    for block in &entry.blocks {
        match block {
            Block::Line(heading, sentence) => {
                laid.items
                    .push(Item::Heading(heading_icon(heading), heading.clone()));
                laid.items
                    .extend(rows_of(sentence, INDENT, Tone::Text, measures));
                laid.items.push(Item::Gap(BLOCK_GAP));
            }
            Block::Intro(intro) => {
                let mut rows = rows_of(intro, 0.0, Tone::Muted, measures);
                if rows.len() > INTRO_LINES {
                    rows.truncate(INTRO_LINES);
                    if let Some(Item::Row { row, .. }) = rows.last_mut()
                        && let Some(run) = row.last_mut()
                        && !run.code
                        && run.link.is_none()
                    {
                        run.text.push_str("...");
                    }
                }
                laid.items.extend(rows);
                laid.items.push(Item::Gap(BLOCK_GAP));
            }
            Block::Setting(cvar, label, text) => {
                let index = laid.switches.len();
                laid.switches.push(cvar.clone());
                laid.items.push(Item::Setting(
                    heading_icon(label),
                    (*label).to_owned(),
                    index,
                ));
                if !text.is_empty() {
                    laid.items
                        .extend(rows_of(text, INDENT, Tone::Text, measures));
                }
                laid.items.push(Item::Gap(BLOCK_GAP));
            }
            Block::Text(text, bullet) => {
                let indent = if *bullet { BULLET_INDENT } else { 0.0 };
                let mut rows = rows_of(text, indent, Tone::Text, measures);
                if *bullet && let Some(Item::Row { bullet, .. }) = rows.first_mut() {
                    *bullet = true;
                }
                laid.items.extend(rows);
            }
            // A picture the hub took down is no longer among the entry's pictures.
            Block::Image(id) if !entry.images.contains_key(id) => {}
            Block::Image(id) => {
                let [width, height] = entry.images.get(id).copied().unwrap_or([16, 9]);
                let (width, height) = (width.max(1) as f32, height.max(1) as f32);
                let scale = (PANE_WIDTH / width).min(IMAGE_MAX_HEIGHT / height).min(1.0);
                laid.items
                    .push(Item::Image(id.clone(), [width * scale, height * scale]));
            }
            Block::Gap => laid.items.push(Item::Gap(BLOCK_GAP)),
        }
    }
    if laid.items.is_empty() {
        // A release without a recap or an introduction ("Unreleased" before it is cut).
        laid.items.extend(rows_of(
            "Its changes are listed in the full changelog.",
            0.0,
            Tone::Muted,
            measures,
        ));
    }
    if !entry.credit.is_empty() {
        laid.items.push(Item::Gap(6.0));
        laid.items
            .extend(rows_of(&entry.credit, 0.0, Tone::Credit, measures));
    }
    while matches!(laid.items.last(), Some(Item::Gap(_))) {
        laid.items.pop();
    }
    laid
}

/// The first item the pane may start at so the last one still shows: how far it
/// scrolls.
fn max_scroll(items: &[Item]) -> usize {
    let room = BODY_BOTTOM - BODY_TOP;
    let mut height = 0.0;
    for (index, item) in items.iter().enumerate().rev() {
        height += item.height();
        if height > room {
            return index + 1;
        }
    }
    0
}

pub(crate) struct Panel {
    /// The marker was read this run.
    checked: bool,
    /// The update the panel is about, written to the marker on Close; empty for none.
    version: String,
    entries: Vec<Entry>,
    selected: usize,
    /// The first entry the list shows.
    list_scroll: usize,
    /// The first item the pane shows.
    scroll: usize,
    /// The selected entry wrapped, and the measures it was wrapped with.
    laid: Laid,
    laid_for: Option<(usize, u32)>,
    /// Whether each switch of the selected entry is on, read this frame.
    on: Vec<bool>,
    /// The chips and links drawn this frame, by token.
    actions: Vec<Action>,
    /// The panel has something to show, and the player asked for it.
    wants: bool,
    asked: bool,
    /// The announcements on the panel, counted as read when it closes.
    listed: Vec<u64>,
    /// The highest announcement the panel opened on by itself this run.
    offered: u64,
    open: bool,
    ui: MenuCanvas,
}

impl Default for Panel {
    fn default() -> Self {
        Self {
            checked: false,
            version: String::new(),
            entries: Vec::new(),
            selected: 0,
            list_scroll: 0,
            scroll: 0,
            laid: Laid::default(),
            laid_for: None,
            on: Vec::new(),
            actions: Vec::new(),
            wants: false,
            asked: false,
            listed: Vec::new(),
            offered: 0,
            open: false,
            ui: MenuCanvas::with_capacities(256, 160, 480),
        }
    }
}

impl Panel {
    /// Whether the marker was read this run; reading it sets this.
    pub(crate) fn take_check(&mut self) -> bool {
        !std::mem::replace(&mut self.checked, true)
    }

    /// Show `entries` on the entry `on` (else the first). `asked` when the player asked
    /// for it (it then shows anywhere); `version` is the update it is about, or empty.
    pub(crate) fn offer(
        &mut self,
        entries: Vec<Entry>,
        on: Option<&Key>,
        asked: bool,
        version: &str,
    ) {
        if entries.is_empty() {
            return;
        }
        self.selected = on
            .and_then(|key| entries.iter().position(|entry| entry.key == *key))
            .unwrap_or(0);
        self.listed = entries
            .iter()
            .filter_map(|entry| match entry.key {
                Key::Announcement(id) => Some(id),
                _ => None,
            })
            .collect();
        self.entries = entries;
        self.list_scroll = self.selected.saturating_sub(ENTRIES_SHOWN - 1);
        self.scroll = 0;
        self.laid_for = None;
        self.wants = true;
        self.asked |= asked;
        if !version.is_empty() {
            version.clone_into(&mut self.version);
        }
        self.mark_selected_read();
    }

    fn mark_selected_read(&mut self) {
        if let Some(entry) = self.entries.get_mut(self.selected) {
            entry.unread = false;
        }
    }

    /// The highest announcement number on the panel, to count as read when it closes.
    pub(crate) fn highest_listed(&self) -> Option<u64> {
        self.listed.iter().copied().max()
    }

    /// Whether the panel has something to show.
    pub(crate) fn wants(&self) -> bool {
        self.wants
    }

    /// Whether the player asked for it (it shows over play too).
    pub(crate) fn asked(&self) -> bool {
        self.asked
    }

    pub(crate) fn set_open(&mut self, open: bool) {
        self.open = open;
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    /// The selected entry's key.
    #[cfg(test)]
    pub(crate) fn selected_key(&self) -> Option<&Key> {
        self.entries.get(self.selected).map(|entry| &entry.key)
    }

    fn put_away(&mut self) {
        self.wants = false;
        self.asked = false;
        self.entries.clear();
        self.laid = Laid::default();
        self.laid_for = None;
        self.on.clear();
        self.actions.clear();
    }

    /// Show entry `index`, its text from the top.
    fn select(&mut self, index: usize) {
        if index >= self.entries.len() || index == self.selected {
            return;
        }
        self.selected = index;
        self.scroll = 0;
        if index < self.list_scroll {
            self.list_scroll = index;
        } else if index >= self.list_scroll + ENTRIES_SHOWN {
            self.list_scroll = index + 1 - ENTRIES_SHOWN;
        }
        self.mark_selected_read();
    }

    fn scroll_by(&mut self, step: isize) {
        let last = max_scroll(&self.laid.items);
        self.scroll = self.scroll.saturating_add_signed(step).min(last);
    }

    /// Read whether each switch of the selected entry is on, with `on`.
    pub(crate) fn sync_switches(&mut self, on: impl Fn(&str) -> bool) {
        self.on.clear();
        self.on
            .extend(self.laid.switches.iter().map(|cvar| on(cvar.as_str())));
    }

    /// The cvars of the selected entry's switches (once it was laid out).
    #[cfg(test)]
    pub(crate) fn switch_cvars(&self) -> &[String] {
        &self.laid.switches
    }

    fn switch(&self, index: usize) -> Option<Choice> {
        self.laid
            .switches
            .get(index)
            .map(|cvar| Choice::Switch(cvar.clone()))
    }

    /// Esc, Enter or Backspace close, Up and Down change the entry, Page Up and Down
    /// scroll the pane, 1 to 9 flip the switches on show, C opens the changelog and U
    /// updates (on a newer release); every other key is swallowed while it shows.
    pub(crate) fn handle_key(&mut self, event: &KeyEvent) -> Option<Choice> {
        if event.state != ElementState::Pressed {
            return None;
        }
        let PhysicalKey::Code(code) = event.physical_key else {
            return None;
        };
        let held = matches!(
            code,
            KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::PageUp | KeyCode::PageDown
        );
        if event.repeat && !held {
            return None;
        }
        self.key(code)
    }

    fn key(&mut self, code: KeyCode) -> Option<Choice> {
        let choice = match code {
            KeyCode::Enter
            | KeyCode::NumpadEnter
            | KeyCode::Escape
            | KeyCode::Backspace
            | KeyCode::Space => Choice::Close,
            KeyCode::KeyC => Choice::Changelog,
            KeyCode::KeyU
                if self
                    .entries
                    .get(self.selected)
                    .is_some_and(|entry| entry.update) =>
            {
                Choice::Update
            }
            KeyCode::ArrowUp => {
                self.select(self.selected.saturating_sub(1));
                return None;
            }
            KeyCode::ArrowDown => {
                self.select(self.selected + 1);
                return None;
            }
            KeyCode::PageUp => {
                self.scroll_by(-6);
                return None;
            }
            KeyCode::PageDown => {
                self.scroll_by(6);
                return None;
            }
            _ => {
                let digit = DIGITS.iter().position(|digit| *digit == code)?;
                return self.switch(digit);
            }
        };
        self.put_away();
        Some(choice)
    }

    pub(crate) fn handle_pointer(&mut self, event: InputEvent) -> Option<Choice> {
        let event = self.ui.pointer(event)?;
        if event.kind == UiEventKind::Wheel {
            let down = event.delta.is_some_and(|delta| delta.y < 0.0);
            let step = if down { 1 } else { -1 };
            match event.token {
                Some(LIST_TOKEN) => {
                    let last = self.entries.len().saturating_sub(ENTRIES_SHOWN);
                    self.list_scroll = self.list_scroll.saturating_add_signed(step).min(last);
                }
                Some(token) if (ENTRY_BASE..ENTRY_BASE + ENTRY_LIMIT as u16).contains(&token) => {
                    let last = self.entries.len().saturating_sub(ENTRIES_SHOWN);
                    self.list_scroll = self.list_scroll.saturating_add_signed(step).min(last);
                }
                _ => self.scroll_by(step * 2),
            }
            return None;
        }
        if event.kind != UiEventKind::Activate {
            return None;
        }
        let choice = match event.token? {
            CLOSE_TOKEN => Choice::Close,
            CHANGELOG_TOKEN => Choice::Changelog,
            UPDATE_TOKEN => Choice::Update,
            token if (SWITCH_BASE..ENTRY_BASE).contains(&token) => {
                return self.switch(usize::from(token - SWITCH_BASE));
            }
            token if (ENTRY_BASE..ACTION_BASE).contains(&token) => {
                self.select(usize::from(token - ENTRY_BASE));
                return None;
            }
            token if token >= ACTION_BASE => {
                match self.actions.get(usize::from(token - ACTION_BASE))? {
                    Action::Chip(text) => Choice::Chip(text.clone()),
                    Action::Link(target) => Choice::Link(target.clone()),
                }
            }
            _ => return None,
        };
        self.put_away();
        Some(choice)
    }

    pub(crate) fn draw_list(&self) -> &sjk_ui::DrawList {
        self.ui.draw_list()
    }

    /// Wrap the selected entry for `measures`, once for each entry and measure.
    fn lay_out_selected(&mut self, measures: &Measures<'_>) {
        let key = (self.selected, measures.key(&LOOK));
        if self.laid_for == Some(key) {
            return;
        }
        self.laid_for = Some(key);
        self.laid = self
            .entries
            .get(self.selected)
            .map(|entry| lay_out(entry, measures))
            .unwrap_or_default();
        self.scroll = self.scroll.min(max_scroll(&self.laid.items));
    }

    /// Wrap the selected entry now (the switches it holds are then known).
    pub(crate) fn prepare(&mut self, measures: &Measures<'_>) {
        self.lay_out_selected(measures);
    }

    /// Draw the panel over the whole frame.
    pub(crate) fn append(&mut self, target: TextTarget<'_>, viewport: [f32; 2]) {
        self.lay_out_selected(&target.inline_measures());
        let frame = Frame::new(viewport);
        let s = frame.s;
        let Some(entry) = self.entries.get(self.selected) else {
            return;
        };
        let ui = &mut self.ui;
        ui.begin_transparent(viewport);
        kit::scrim(ui, viewport);
        kit::card(ui, &frame, [PANEL_X, PANEL_Y, PANEL_WIDTH, PANEL_HEIGHT]);
        // The list.
        ui.hit_region(
            LIST_TOKEN,
            frame.rect(PANEL_X, PANEL_Y, LIST_WIDTH, PANEL_HEIGHT),
        );
        let _ = ui.draw_list_mut().push(DrawCommand::SolidRect {
            rect: frame.rect(
                PANEL_X + LIST_WIDTH,
                PANEL_Y + 30.0,
                1.0,
                PANEL_HEIGHT - 60.0,
            ),
            color: color::alpha(color::HOLO, 0.25),
        });
        if let Some(texture) = crate::settings_icons::texture("changelog") {
            let _ = ui.draw_list_mut().push(DrawCommand::TexturedQuad {
                rect: frame.rect(PANEL_X + LIST_PAD, PANEL_Y + 30.0, 30.0, 30.0),
                texture,
                color: Color::new(1.0, 1.0, 1.0, 0.95),
            });
        }
        text(
            ui,
            TextFamily::Display,
            format_args!("News"),
            frame.rect(PANEL_X + LIST_PAD + 42.0, PANEL_Y + 26.0, 300.0, 38.0),
            30.0 * s,
            color::GOLD_BRIGHT,
            FontWeight::Semibold,
            TextAlign::Start,
        );
        let shown = self.entries.len().min(ENTRIES_SHOWN);
        for (offset, entry) in self
            .entries
            .iter()
            .enumerate()
            .skip(self.list_scroll)
            .take(shown)
            .map(|(index, entry)| (index - self.list_scroll, (index, entry)))
        {
            let (index, entry) = entry;
            let y = PANEL_Y + LIST_TOP + offset as f32 * ENTRY_HEIGHT;
            let row = [PANEL_X + 12.0, y, LIST_WIDTH - 24.0, ENTRY_HEIGHT - 6.0];
            let token = ENTRY_BASE + index as u16;
            let selected = index == self.selected;
            let hovered = ui.token_hovered(token);
            if selected || hovered {
                let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
                    rect: frame.rect(row[0], row[1], row[2], row[3]),
                    radius: 8.0 * s,
                    color: color::alpha(color::HOLO, if selected { 0.16 } else { 0.08 }),
                });
            }
            if selected {
                let _ = ui.draw_list_mut().push(DrawCommand::SolidRect {
                    rect: frame.rect(row[0], row[1] + 8.0, 3.0, row[3] - 16.0),
                    color: color::GOLD,
                });
            }
            ui.hit_region(token, frame.rect(row[0], row[1], row[2], row[3]));
            text(
                ui,
                TextFamily::Display,
                format_args!("{}", entry.kicker),
                frame.rect(row[0] + 16.0, y + 8.0, row[2] - 48.0, 22.0),
                14.0 * s,
                if entry.kicker == "SETTINGS TO REVIEW" {
                    color::GOLD
                } else {
                    color::MUTED
                },
                FontWeight::Semibold,
                TextAlign::Start,
            );
            text(
                ui,
                TextFamily::Display,
                format_args!("{}", entry.name),
                frame.rect(row[0] + 16.0, y + 30.0, row[2] - 48.0, 30.0),
                21.0 * s,
                if selected {
                    color::TEXT
                } else {
                    color::alpha(color::TEXT, 0.85)
                },
                FontWeight::Semibold,
                TextAlign::Start,
            );
            let day = entry.date.split(' ').next().unwrap_or_default();
            text(
                ui,
                TextFamily::Body,
                format_args!("{day}"),
                frame.rect(row[0] + 16.0, y + 8.0, row[2] - 32.0, 22.0),
                14.0 * s,
                color::MUTED,
                FontWeight::Regular,
                TextAlign::End,
            );
            if entry.unread {
                let dot = frame.rect(row[0] + row[2] - 22.0, y + 40.0, 10.0, 10.0);
                let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
                    rect: dot,
                    radius: dot.width * 0.5,
                    color: color::GOLD_BRIGHT,
                });
            }
        }
        if self.entries.len() > ENTRIES_SHOWN {
            let more = format!(
                "{}-{} of {}",
                self.list_scroll + 1,
                self.list_scroll + shown,
                self.entries.len()
            );
            text(
                ui,
                TextFamily::Body,
                format_args!("{more}"),
                frame.rect(
                    PANEL_X + LIST_PAD,
                    PANEL_Y + PANEL_HEIGHT - 44.0,
                    LIST_WIDTH - 2.0 * LIST_PAD,
                    24.0,
                ),
                15.0 * s,
                color::MUTED,
                FontWeight::Regular,
                TextAlign::Center,
            );
        }
        // The pane.
        ui.hit_region(
            BODY_TOKEN,
            frame.rect(PANE_X - 20.0, PANEL_Y, PANE_WIDTH + 40.0, PANEL_HEIGHT),
        );
        text(
            ui,
            TextFamily::Display,
            format_args!("{}", entry.kicker),
            frame.rect(PANE_X, PANEL_Y + 32.0, PANE_WIDTH, 28.0),
            19.0 * s,
            color::GOLD_BRIGHT,
            FontWeight::Semibold,
            TextAlign::Start,
        );
        let when = if entry.key == Key::Review {
            "Changed by this update: choose again".to_owned()
        } else {
            entry.date.clone()
        };
        text(
            ui,
            TextFamily::Display,
            format_args!("{when}"),
            frame.rect(PANE_X, PANEL_Y + 32.0, PANE_WIDTH, 28.0),
            18.0 * s,
            color::MUTED,
            FontWeight::Semibold,
            TextAlign::End,
        );
        text(
            ui,
            TextFamily::Display,
            format_args!("{}", entry.title),
            frame.rect(PANE_X, PANEL_Y + 66.0, PANE_WIDTH, 48.0),
            34.0 * s,
            color::TEXT,
            FontWeight::Semibold,
            TextAlign::Start,
        );
        self.actions.clear();
        let mut token = ACTION_BASE;
        let mut y = BODY_TOP;
        let items = &self.laid.items;
        let last = max_scroll(items);
        for item in items.iter().skip(self.scroll) {
            if y + item.height() > BODY_BOTTOM + 0.5 {
                break;
            }
            match item {
                Item::Heading(icon, heading) => {
                    if let Some(texture) = crate::settings_icons::texture(icon) {
                        let _ = ui.draw_list_mut().push(DrawCommand::TexturedQuad {
                            rect: frame.rect(PANE_X, y + (HEADING_HEIGHT - ICON) * 0.5, ICON, ICON),
                            texture,
                            color: Color::new(1.0, 1.0, 1.0, 0.9),
                        });
                    }
                    text(
                        ui,
                        TextFamily::Display,
                        format_args!("{heading}"),
                        frame.rect(PANE_X + INDENT, y, PANE_WIDTH - INDENT, HEADING_HEIGHT),
                        HEADING_SIZE * s,
                        color::GOLD_BRIGHT,
                        FontWeight::Semibold,
                        TextAlign::Start,
                    );
                }
                Item::Row {
                    row,
                    indent,
                    tone,
                    bullet,
                } => {
                    if *bullet {
                        let dot = frame.rect(PANE_X + 8.0, y + LOOK.line * 0.5 - 3.5, 7.0, 7.0);
                        let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
                            rect: dot,
                            radius: dot.width * 0.5,
                            color: color::GOLD,
                        });
                    }
                    let (look, colour) = match tone {
                        Tone::Text => (&LOOK, color::TEXT),
                        Tone::Muted => (&LOOK, color::MUTED),
                        Tone::Credit => (&CREDIT_LOOK, color::alpha(color::GOLD, 0.85)),
                    };
                    self.actions
                        .extend(inline_code::actions(std::slice::from_ref(row)));
                    inline_code::draw_rows(
                        ui,
                        &frame,
                        std::slice::from_ref(row),
                        look,
                        &Place {
                            x: PANE_X + indent,
                            y,
                            colour,
                            code_family: TextFamily::Code,
                        },
                        Some(&mut token),
                    );
                }
                Item::Setting(icon, label, index) => {
                    if let Some(texture) = crate::settings_icons::texture(icon) {
                        let _ = ui.draw_list_mut().push(DrawCommand::TexturedQuad {
                            rect: frame.rect(PANE_X, y + (HEADING_HEIGHT - ICON) * 0.5, ICON, ICON),
                            texture,
                            color: Color::new(1.0, 1.0, 1.0, 0.9),
                        });
                    }
                    let on = self.on.get(*index).copied().unwrap_or(false);
                    let number = if *index < DIGITS.len() {
                        format!("{}. ", index + 1)
                    } else {
                        String::new()
                    };
                    text(
                        ui,
                        TextFamily::Display,
                        format_args!("{number}{label}"),
                        frame.rect(
                            PANE_X + INDENT,
                            y,
                            PANE_WIDTH - INDENT - 120.0,
                            HEADING_HEIGHT,
                        ),
                        HEADING_SIZE * s,
                        color::GOLD_BRIGHT,
                        FontWeight::Semibold,
                        TextAlign::Start,
                    );
                    let right = PANE_X + PANE_WIDTH;
                    let left = kit::switch(ui, &frame, right, y + HEADING_HEIGHT * 0.5, on, false);
                    ui.hit_region(
                        SWITCH_BASE + *index as u16,
                        frame.rect(
                            left - 6.0,
                            y - 4.0,
                            right - left + 12.0,
                            HEADING_HEIGHT + 8.0,
                        ),
                    );
                }
                Item::Image(id, [width, height]) => {
                    let rect = frame.rect(PANE_X, y + 4.0, *width, *height);
                    match crate::news_images::texture(id) {
                        Some(texture) => {
                            let _ = ui.draw_list_mut().push(DrawCommand::TexturedQuad {
                                rect,
                                texture,
                                color: Color::new(1.0, 1.0, 1.0, 1.0),
                            });
                        }
                        None => {
                            let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
                                rect,
                                radius: 8.0 * s,
                                color: color::alpha(color::HOLO, 0.08),
                            });
                            text(
                                ui,
                                TextFamily::Body,
                                format_args!("Picture loading..."),
                                rect,
                                16.0 * s,
                                color::MUTED,
                                FontWeight::Regular,
                                TextAlign::Center,
                            );
                        }
                    }
                    let _ = ui.draw_list_mut().push(DrawCommand::Border {
                        rect,
                        radius: 8.0 * s,
                        width: s.max(1.0),
                        color: color::alpha(color::HOLO, 0.3),
                    });
                }
                Item::Gap(_) => {}
            }
            y += item.height();
        }
        if last > 0 {
            // How far down the pane is, on its right edge.
            let track = [
                PANE_X + PANE_WIDTH + 18.0,
                BODY_TOP,
                4.0,
                BODY_BOTTOM - BODY_TOP,
            ];
            let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
                rect: frame.rect(track[0], track[1], track[2], track[3]),
                radius: 2.0 * s,
                color: color::alpha(color::HOLO, 0.15),
            });
            let share = 1.0 / (last + 1) as f32;
            let thumb = (track[3] * share).max(40.0);
            let top = track[1] + (track[3] - thumb) * (self.scroll as f32 / last as f32);
            let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
                rect: frame.rect(track[0], top, track[2], thumb),
                radius: 2.0 * s,
                color: color::GOLD,
            });
        }
        // The foot.
        let buttons_y = PANEL_Y + PANEL_HEIGHT - 24.0 - BUTTON_HEIGHT;
        let right = PANE_X + PANE_WIDTH;
        kit::button(
            ui,
            &frame,
            [right - CLOSE_WIDTH, buttons_y, CLOSE_WIDTH, BUTTON_HEIGHT],
            "Close",
            !entry.update,
            true,
            false,
            CLOSE_TOKEN,
        );
        let mut x = right - CLOSE_WIDTH - BUTTON_GAP;
        if entry.update {
            kit::button(
                ui,
                &frame,
                [x - UPDATE_WIDTH, buttons_y, UPDATE_WIDTH, BUTTON_HEIGHT],
                "Update",
                true,
                true,
                false,
                UPDATE_TOKEN,
            );
            x -= UPDATE_WIDTH + BUTTON_GAP;
        }
        kit::button(
            ui,
            &frame,
            [
                x - CHANGELOG_WIDTH,
                buttons_y,
                CHANGELOG_WIDTH,
                BUTTON_HEIGHT,
            ],
            "Full changelog",
            false,
            true,
            false,
            CHANGELOG_TOKEN,
        );
        let [right, keys_y] = frame.point(KEYS_RIGHT, KEYS_Y);
        let mut hints: Vec<(&[&str], &str)> = Vec::with_capacity(6);
        if !self.laid.switches.is_empty() {
            hints.push((&["1-9"], "switch"));
        }
        if self.entries.len() > 1 {
            hints.push((&["Up", "Down"], "entry"));
        }
        if last > 0 {
            hints.push((&["PgUp", "PgDn"], "scroll"));
        }
        if entry.update {
            hints.push((&["U"], "update"));
        }
        hints.push((&["C"], "full changelog"));
        hints.push((&["Esc"], "close"));
        let gap = 18.0 * s;
        let width: f32 = hints
            .iter()
            .map(|(keys, action)| key_hint_width(keys, action, s))
            .sum::<f32>()
            + gap * (hints.len() - 1) as f32;
        let mut x = right - width;
        for (keys, action) in hints {
            x = key_hint(ui, keys, action, x, keys_y, s) + gap;
        }
        ui.finish(CLOSE_TOKEN);
        target.append(&self.ui, viewport);
    }
}

/// "By Sol and Creyon, after JoF EJK and OpenJK": who made a release's changes and the
/// clients they follow; empty when nobody is credited.
fn credit_line(people: &[String], after: &[String]) -> String {
    if people.is_empty() {
        return String::new();
    }
    let mut line = format!("By {}", and_list(people));
    if !after.is_empty() {
        line.push_str(", after ");
        line.push_str(&and_list(after));
    }
    line
}

/// "a", "a and b", "a, b and c".
fn and_list(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [first @ .., last] => format!("{} and {last}", first.join(", ")),
    }
}

/// Put each setting `reviews` marks `reset` back to its default, before the panel asks
/// the player to choose it again: a value saved before the change does not carry over.
fn reset_reviewed(console: &mut crate::console::ViewerConsole, reviews: &[Review]) {
    for review in reviews.iter().filter(|review| review.reset) {
        if let Some(default) = console
            .cvar_default(&review.cvar)
            .map(sjk_shell::CvarValue::as_text)
        {
            console.set_cvar(&review.cvar, &default);
        }
    }
}

/// Reset the settings `resets` names for reaching `installed` from `seen` (given the
/// version whose resets were applied), and record at once that `installed`'s were: a
/// player who joins a server from the command line, or quits before answering the
/// panel, keeps what they chose since at the next start.
fn reset_once(
    console: &mut crate::console::ViewerConsole,
    installed: &str,
    seen: &str,
    resets: impl FnOnce(&str, &str, &str) -> Vec<Review>,
) {
    let applied = console.text_cvar(RESET_CVAR).unwrap_or_default().to_owned();
    reset_reviewed(console, &resets(installed, seen, &applied));
    console.set_cvar(RESET_CVAR, installed);
}

/// Whether the setting `cvar` is on, as its Settings row reads it: a switch may be a
/// number cvar (`r_dynamicGlowNoSabers` is), which is on when nonzero.
fn switch_on(console: &crate::console::ViewerConsole, cvar: &str) -> bool {
    console.cvar(cvar).is_some_and(crate::settings::switch_on)
}

/// Flip the setting `cvar`.
fn flip(console: &mut crate::console::ViewerConsole, cvar: &str) {
    let on = switch_on(console, cvar);
    console.set_cvar(cvar, if on { "0" } else { "1" });
}

#[path = "news_panel_frame.rs"]
mod frame;

#[cfg(test)]
#[path = "news_panel_tests.rs"]
mod tests;
