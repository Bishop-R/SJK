//! The SJK team's windows (`docs/sjk-ui.md`, "Announcements and reward rounds"), one
//! SJK pop-up card over the darkened screen at a time, in the SJK UI's look in every menu
//! style:
//!
//! - the reward rounds ([`Page::Rounds`]): a round's title, date and note, then who it
//!   rewards and why, scrolled; older rounds with Left and Right. No amounts;
//! - a reward waiting for the player ([`Page::Claim`]): what it gives, why, and Claim;
//! - the SJK team's announcement ([`Page::Compose`]): Kind, Title and Text with a preview
//!   of the chat line; Send asks to confirm.
//!
//! Announcements are read on the News panel (`news_panel.rs`, 31/10/2026).
//!
//! The state and keys live here; [`super`] decides when a page opens and acts on the
//! player's [`Choice`].

use super::words::{self, SENDER};
use crate::menu::sjk::{Frame, TextTarget, color, key_hint, key_hint_width, kit, text, wrap};
use crate::menu_widgets::{MenuCanvas, TextFamily};
use sjk_identity::news::{AnnounceRequest, Announcement, Reward, Round, plain_line};
use sjk_ui::{Color, DrawCommand, FontWeight, InputEvent, Rect, TextAlign, UiEventKind};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

const CLOSE_TOKEN: u16 = 1;
const PRIMARY_TOKEN: u16 = 2;
const LATER_TOKEN: u16 = 3;
const PREVIOUS_TOKEN: u16 = 4;
const NEXT_TOKEN: u16 = 5;
const SCREEN_TOKEN: u16 = 6;
const CARD_TOKEN: u16 = 7;
const LIST_TOKEN: u16 = 8;
const NEWS_TOKEN: u16 = 10;
const RELEASE_TOKEN: u16 = 11;
const VERSION_TOKEN: u16 = 12;
const TITLE_TOKEN: u16 = 13;
const TEXT_TOKEN: u16 = 14;
const CONFIRM_TOKEN: u16 = 15;
const BACK_TOKEN: u16 = 16;

/// The card's left edge and width; it is as tall as what it holds, centred.
const CARD_X: f32 = 500.0;
const CARD_WIDTH: f32 = 920.0;
const MARGIN: f32 = 44.0;
const TEXT_X: f32 = CARD_X + MARGIN;
const TEXT_WIDTH: f32 = CARD_WIDTH - MARGIN * 2.0;
const RIGHT: f32 = CARD_X + CARD_WIDTH - MARGIN;
/// The head: the mark, the kicker and the date, then the title under them.
const HEAD: f32 = 34.0;
const MARK: f32 = 52.0;
const TITLE_TOP: f32 = 100.0;
const TITLE_LINE: f32 = 40.0;
const TITLE_CHARS: usize = 48;
/// Running text: its line, characters a line and the gap between paragraphs.
const LINE: f32 = 28.0;
const LINE_CHARS: usize = 86;
/// The round list: a person's row, how many show, and the name's column.
const ROW: f32 = 34.0;
const ROWS: usize = 11;
const NAME_WIDTH: f32 = 290.0;
/// The buttons.
const BUTTONS_GAP: f32 = 30.0;
const BUTTON_HEIGHT: f32 = 46.0;
const BUTTON_GAP: f32 = 14.0;
const BUTTON_WIDTH: f32 = 200.0;
const KEYS_Y: f32 = 1_020.0;
const KEYS_RIGHT: f32 = 1_824.0;
/// The composer's fields and how many lines of the text show.
const FIELD_HEIGHT: f32 = 44.0;
const TEXT_ROWS: usize = 9;
const VERSION_MAX: usize = 24;

/// What the player chose in a window.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Choice {
    /// The window closed.
    Close,
    /// Claim this reward (a rehearsal's is never sent).
    Claim { id: u64, rehearsal: bool },
    /// Later: the claim window waits for the next menu.
    Later,
    /// Send the SJK team's announcement.
    Send(AnnounceRequest),
    /// Pick a picture file for the announcement and upload it (Ctrl+I).
    PickImage,
}

/// Which field of the composer has the keyboard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Field {
    Kind,
    Version,
    Title,
    Text,
}

/// The creator's announcement being written.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Composer {
    pub(crate) release: bool,
    pub(crate) version: String,
    pub(crate) title: String,
    pub(crate) text: String,
    pub(crate) focus: Field,
    /// Send was asked: the card asks to confirm.
    pub(crate) confirm: bool,
    /// Sent, waiting for the hub's answer.
    pub(crate) busy: bool,
    /// Why the hub (or the check) refused it.
    pub(crate) message: String,
    /// The link picker (Ctrl+L with no setting typed): the place lit, of
    /// [`crate::ui_links::PAGES`].
    pub(crate) picker: Option<usize>,
    /// A picture is being picked or uploaded.
    pub(crate) uploading: bool,
}

impl Default for Composer {
    fn default() -> Self {
        Self {
            release: false,
            version: String::new(),
            title: String::new(),
            text: String::new(),
            focus: Field::Title,
            confirm: false,
            busy: false,
            message: String::new(),
            picker: None,
            uploading: false,
        }
    }
}

/// The last word of `text` (after its last space or line break) and where it starts.
fn last_word(text: &str) -> (usize, &str) {
    let start = text.rfind([' ', '\n']).map_or(0, |at| at + 1);
    (start, &text[start..])
}

/// Whether `text` with `insert` after it keeps to the hub's length and lines.
fn fits(text: &str, insert: &str) -> bool {
    let joined = format!("{text}{insert}");
    joined.chars().count() <= sjk_identity::news::TEXT_MAX
        && joined.split('\n').count() <= sjk_identity::news::LINES_MAX
}

impl Composer {
    /// Ctrl+L: the setting just typed (its cvar) becomes a link to it, named as Settings
    /// names it; with none, the picker of places opens.
    pub(crate) fn link(&mut self) {
        let (start, word) = last_word(&self.text);
        if crate::settings::has_row(word) {
            let label = crate::settings::row_label(word).unwrap_or(word);
            let link = format!("[{label}](setting:{word})");
            let kept = self.text[..start].to_owned();
            if fits(&kept, &link) {
                self.text = kept + &link;
                self.message.clear();
            } else {
                "No room left for the link.".clone_into(&mut self.message);
            }
            return;
        }
        self.picker = Some(0);
    }

    /// Ctrl+K: the setting just typed becomes its switch, on a line of its own.
    pub(crate) fn switch(&mut self) {
        let (start, word) = last_word(&self.text);
        if word.is_empty() || crate::settings::switch_label(word).is_none() {
            "Type an on/off setting's cvar (cg_nameplate), then Ctrl+K."
                .clone_into(&mut self.message);
            return;
        }
        let word = word.to_owned();
        let kept = self.text[..start].trim_end().to_owned();
        let gap = if kept.is_empty() { "" } else { "\n" };
        let line = format!("{gap}[[switch:{word}]]\n");
        if fits(&kept, &line) {
            self.text = kept + &line;
            self.message.clear();
        } else {
            "No room left for the switch.".clone_into(&mut self.message);
        }
    }

    /// The hub stored picture `id`: its line goes at the end of the text.
    pub(crate) fn add_image(&mut self, id: &str) {
        self.uploading = false;
        let kept = self.text.trim_end().to_owned();
        let gap = if kept.is_empty() { "" } else { "\n" };
        let line = format!("{gap}[[image:{id}]]\n");
        if fits(&kept, &line) {
            self.text = kept + &line;
            self.message.clear();
        } else {
            "No room left for the picture's line.".clone_into(&mut self.message);
        }
    }

    /// A key while the picker of places shows: Up and Down choose, Enter puts the link
    /// in, Esc closes it.
    fn picker_key(&mut self, key: KeyCode) {
        let Some(at) = self.picker else {
            return;
        };
        let pages = crate::ui_links::PAGES;
        match key {
            KeyCode::ArrowUp => self.picker = Some(at.saturating_sub(1)),
            KeyCode::ArrowDown => self.picker = Some((at + 1).min(pages.len() - 1)),
            KeyCode::Enter | KeyCode::NumpadEnter => {
                self.picker = None;
                let page = pages[at];
                let space = if self.text.is_empty() || self.text.ends_with([' ', '\n']) {
                    ""
                } else {
                    " "
                };
                let link = format!("{space}[{}](open:{})", page.name, page.id);
                if fits(&self.text, &link) {
                    self.text.push_str(&link);
                } else {
                    "No room left for the link.".clone_into(&mut self.message);
                }
            }
            KeyCode::Escape => self.picker = None,
            _ => {}
        }
    }
}

impl Composer {
    /// What would go to the hub.
    pub(crate) fn request(&self) -> AnnounceRequest {
        AnnounceRequest {
            release: self.release,
            title: self.title.clone(),
            text: self.text.clone(),
            version: self.version.clone(),
        }
    }

    /// The fields Tab visits, in order.
    fn fields(&self) -> Vec<Field> {
        if self.release {
            vec![Field::Kind, Field::Version, Field::Title, Field::Text]
        } else {
            vec![Field::Kind, Field::Title, Field::Text]
        }
    }

    fn step(&mut self, forward: bool) {
        let fields = self.fields();
        let at = fields
            .iter()
            .position(|field| *field == self.focus)
            .unwrap_or(0);
        let next = if forward {
            (at + 1) % fields.len()
        } else {
            (at + fields.len() - 1) % fields.len()
        };
        self.focus = fields[next];
    }

    /// The line the chat would show for it now.
    fn preview(&self) -> String {
        let message = sjk_identity::ChatMessage {
            channel: Default::default(),
            reactions: Default::default(),
            id: 0,
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
            official: Some(sjk_identity::Official::Announcement(Announcement {
                title: self.title.clone(),
                text: self.text.clone(),
                ..Announcement::default()
            })),
        };
        words::words(&message, "Read more").map_or_else(String::new, |words| words.text)
    }

    /// A key while a field has the keyboard (Ctrl+A, Ctrl+C and Ctrl+V and a held Backspace
    /// in every field, `crate::text_entry`): the one-line fields take one line, the text
    /// takes Enter as a new line; Ctrl+Enter sends from anywhere.
    fn key(
        &mut self,
        key: KeyCode,
        typed: Option<&str>,
        shift: bool,
        control: bool,
    ) -> Option<Choice> {
        if self.busy {
            return None;
        }
        if self.picker.is_some() {
            self.picker_key(key);
            return None;
        }
        if self.confirm {
            return match key {
                KeyCode::Enter | KeyCode::NumpadEnter => {
                    self.busy = true;
                    self.message.clear();
                    Some(Choice::Send(self.request()))
                }
                KeyCode::Escape | KeyCode::Backspace => {
                    self.confirm = false;
                    None
                }
                _ => None,
            };
        }
        match key {
            KeyCode::Escape => return Some(Choice::Close),
            KeyCode::Enter | KeyCode::NumpadEnter if control => {
                self.ask();
                return None;
            }
            KeyCode::Tab => {
                self.step(!shift);
                return None;
            }
            KeyCode::KeyL if control && self.focus == Field::Text => {
                self.link();
                return None;
            }
            KeyCode::KeyK if control && self.focus == Field::Text => {
                self.switch();
                return None;
            }
            KeyCode::KeyI if control && self.focus == Field::Text && !self.uploading => {
                self.uploading = true;
                "Choose a PNG or JPEG picture...".clone_into(&mut self.message);
                return Some(Choice::PickImage);
            }
            _ => {}
        }
        match self.focus {
            Field::Kind => match key {
                KeyCode::Space | KeyCode::ArrowLeft | KeyCode::ArrowRight => {
                    self.release = !self.release;
                }
                KeyCode::Enter | KeyCode::NumpadEnter => self.step(true),
                _ => {}
            },
            Field::Version | Field::Title => {
                if matches!(key, KeyCode::Enter | KeyCode::NumpadEnter) {
                    self.step(true);
                    return None;
                }
                let (field, max) = if self.focus == Field::Version {
                    (&mut self.version, VERSION_MAX)
                } else {
                    (&mut self.title, sjk_identity::news::TITLE_MAX)
                };
                crate::text_entry::edit(field, key, typed, shift, control, max, true);
            }
            Field::Text => edit_text(&mut self.text, key, typed, shift, control),
        }
        None
    }

    /// Send was asked: check it as the hub will, then ask to confirm.
    fn ask(&mut self) {
        match sjk_identity::news::check(&self.request()) {
            Ok(_) => {
                self.confirm = true;
                self.message.clear();
            }
            Err(error) => self.message = error.message().to_owned(),
        }
    }
}

/// A key into the composer's text: Enter makes a new line (up to the hub's lines), a paste
/// keeps its lines, Backspace (held, it repeats) takes the last character back, Ctrl+A and
/// Ctrl+C as in every field.
fn edit_text(text: &mut String, key: KeyCode, typed: Option<&str>, shift: bool, control: bool) {
    if crate::text_entry::before_edit(text, key, typed, shift, control) {
        return;
    }
    let room = |text: &str| sjk_identity::news::TEXT_MAX.saturating_sub(text.chars().count());
    let lines = |text: &str| text.split('\n').count();
    match key {
        KeyCode::Backspace => {
            text.pop();
        }
        KeyCode::Enter | KeyCode::NumpadEnter => {
            if lines(text) < sjk_identity::news::LINES_MAX && room(text) > 0 {
                text.push('\n');
            }
        }
        _ if crate::sjk_chat_field::is_paste(key, typed, shift, control) => {
            if let Some(clipboard) = crate::console::clipboard::paste() {
                let clipboard = clipboard.replace("\r\n", "\n").replace('\r', "\n");
                for c in clipboard.chars() {
                    if room(text) == 0
                        || (c == '\n' && lines(text) >= sjk_identity::news::LINES_MAX)
                    {
                        break;
                    }
                    if c == '\n' || !c.is_control() {
                        text.push(c);
                    }
                }
            }
        }
        _ if crate::text_entry::is_copy(key, typed, control) => {
            if !text.is_empty() {
                let _ = crate::console::clipboard::copy(text);
            }
        }
        _ => {
            if let Some(typed) = typed.filter(|_| !control) {
                let room = room(text);
                text.extend(typed.chars().filter(|c| !c.is_control()).take(room));
            }
        }
    }
}

/// What a window shows.
#[derive(Clone, Debug)]
pub(crate) enum Page {
    /// The reward rounds, newest first, on round `index`, its list `scroll` rows down.
    Rounds {
        rounds: Vec<Round>,
        index: usize,
        scroll: usize,
        /// The round asked for, while the rounds are still to be read.
        wanted: Option<u64>,
    },
    /// A reward waiting for the player.
    Claim {
        reward: Reward,
        rehearsal: bool,
        /// Sent, waiting for the hub's answer.
        busy: bool,
        /// Why the hub refused it.
        message: String,
    },
    /// The creator's announcement.
    Compose(Composer),
}

/// The window: the page on show and its canvas.
pub(crate) struct Window {
    pub(crate) page: Option<Page>,
    /// Shown this frame.
    open: bool,
    ui: MenuCanvas,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            page: None,
            open: false,
            ui: MenuCanvas::with_capacities(64, 512, 512),
        }
    }
}

impl Window {
    pub(crate) fn set_open(&mut self, open: bool) {
        self.open = open;
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    pub(crate) fn draw_list(&self) -> &sjk_ui::DrawList {
        self.ui.draw_list()
    }

    /// A key while the window shows; every key is taken. A typing page lets repeats
    /// through (a held Backspace), the others act on presses only.
    pub(crate) fn handle_key(
        &mut self,
        event: &KeyEvent,
        shift: bool,
        control: bool,
    ) -> Option<Choice> {
        if event.state != ElementState::Pressed {
            return None;
        }
        let PhysicalKey::Code(code) = event.physical_key else {
            return None;
        };
        let composing = matches!(self.page, Some(Page::Compose(_)));
        if event.repeat && !composing {
            return None;
        }
        self.key(code, event.text.as_deref(), shift, control)
    }

    /// [`Self::handle_key`] for a key code and what it typed.
    pub(crate) fn key(
        &mut self,
        code: KeyCode,
        typed: Option<&str>,
        shift: bool,
        control: bool,
    ) -> Option<Choice> {
        let choice = match self.page.as_mut()? {
            Page::Compose(composer) => composer.key(code, typed, shift, control),
            Page::Rounds {
                rounds,
                index,
                scroll,
                ..
            } => match code {
                KeyCode::Escape | KeyCode::Enter | KeyCode::NumpadEnter => Some(Choice::Close),
                KeyCode::ArrowLeft => {
                    if *index + 1 < rounds.len() {
                        *index += 1;
                        *scroll = 0;
                    }
                    None
                }
                KeyCode::ArrowRight => {
                    if *index > 0 {
                        *index -= 1;
                        *scroll = 0;
                    }
                    None
                }
                KeyCode::ArrowDown | KeyCode::PageDown => {
                    let people = rounds.get(*index).map_or(0, |round| round.people.len());
                    let step = if code == KeyCode::PageDown { ROWS } else { 1 };
                    *scroll = (*scroll + step).min(people.saturating_sub(ROWS));
                    None
                }
                KeyCode::ArrowUp | KeyCode::PageUp => {
                    let step = if code == KeyCode::PageUp { ROWS } else { 1 };
                    *scroll = scroll.saturating_sub(step);
                    None
                }
                _ => None,
            },
            Page::Claim {
                reward,
                rehearsal,
                busy,
                message,
            } => match code {
                KeyCode::Enter | KeyCode::NumpadEnter if !*busy => {
                    *busy = !*rehearsal;
                    message.clear();
                    Some(Choice::Claim {
                        id: reward.id,
                        rehearsal: *rehearsal,
                    })
                }
                KeyCode::Escape => Some(Choice::Later),
                _ => None,
            },
        };
        if matches!(choice, Some(Choice::Close)) {
            self.page = None;
        }
        choice
    }

    /// A click on the window.
    pub(crate) fn handle_pointer(&mut self, event: InputEvent) -> Option<Choice> {
        let event = self.ui.pointer(event)?;
        if event.kind == UiEventKind::Wheel {
            let down = event.delta.is_some_and(|delta| delta.y < 0.0);
            let code = if down {
                KeyCode::ArrowDown
            } else {
                KeyCode::ArrowUp
            };
            return match self.page {
                Some(Page::Rounds { .. }) => self.key(code, None, false, false),
                _ => None,
            };
        }
        if event.kind != UiEventKind::Activate {
            return None;
        }
        let token = event.token?;
        let choice = match (self.page.as_mut()?, token) {
            (_, CLOSE_TOKEN) => Some(Choice::Close),
            (Page::Rounds { .. }, SCREEN_TOKEN) => Some(Choice::Close),
            (Page::Rounds { .. }, PREVIOUS_TOKEN) => {
                return self.key(KeyCode::ArrowLeft, None, false, false);
            }
            (Page::Rounds { .. }, NEXT_TOKEN) => {
                return self.key(KeyCode::ArrowRight, None, false, false);
            }
            (Page::Claim { .. }, PRIMARY_TOKEN) => {
                return self.key(KeyCode::Enter, None, false, false);
            }
            (Page::Claim { .. }, LATER_TOKEN) => Some(Choice::Later),
            (Page::Compose(composer), token) => {
                if composer.busy {
                    return None;
                }
                match token {
                    NEWS_TOKEN => composer.release = false,
                    RELEASE_TOKEN => composer.release = true,
                    VERSION_TOKEN => composer.focus = Field::Version,
                    TITLE_TOKEN => composer.focus = Field::Title,
                    TEXT_TOKEN => composer.focus = Field::Text,
                    PRIMARY_TOKEN => composer.ask(),
                    LATER_TOKEN => return Some(Choice::Close),
                    CONFIRM_TOKEN => {
                        return composer.key(KeyCode::Enter, None, false, false);
                    }
                    BACK_TOKEN => composer.confirm = false,
                    _ => {}
                }
                None
            }
            _ => None,
        };
        if matches!(choice, Some(Choice::Close)) {
            self.page = None;
        }
        choice
    }

    /// Draw the window over the whole frame.
    pub(crate) fn append(&mut self, target: TextTarget<'_>, viewport: [f32; 2]) {
        let frame = Frame::new(viewport);
        let ui = &mut self.ui;
        ui.begin_transparent(viewport);
        kit::scrim(ui, viewport);
        ui.hit_region(SCREEN_TOKEN, Rect::new(0.0, 0.0, viewport[0], viewport[1]));
        let focus = match self.page.as_mut() {
            Some(Page::Rounds {
                rounds,
                index,
                scroll,
                wanted,
            }) => rounds_page(ui, &frame, rounds, *index, *scroll, wanted.is_some()),
            Some(Page::Claim {
                reward,
                busy,
                message,
                ..
            }) => claim_page(ui, &frame, reward, *busy, message),
            Some(Page::Compose(composer)) => compose_page(ui, &frame, composer),
            None => CLOSE_TOKEN,
        };
        ui.finish(focus);
        target.append(&self.ui, viewport);
    }
}

/// The card from `top`, `height` tall, with a hit region that keeps clicks on it from
/// closing it.
fn card(ui: &mut MenuCanvas, frame: &Frame, top: f32, height: f32) {
    kit::card(ui, frame, [CARD_X, top, CARD_WIDTH, height]);
    ui.hit_region(CARD_TOKEN, frame.rect(CARD_X, top, CARD_WIDTH, height));
}

/// The top of a card `height` tall, centred.
fn top_of(height: f32) -> f32 {
    ((1_080.0 - height) * 0.5).max(20.0)
}

/// The head: SJK's logo (or the gift box for rewards), the kicker in gold and what goes on
/// the right (the date).
fn head(ui: &mut MenuCanvas, frame: &Frame, top: f32, gift: bool, kicker: &str, right: &str) {
    let s = frame.s;
    if gift {
        text(
            ui,
            TextFamily::Body,
            format_args!("{}", crate::symbols::GIFT),
            frame.rect(TEXT_X, top + HEAD - 4.0, MARK, MARK),
            44.0 * s,
            color::GOLD,
            FontWeight::Regular,
            TextAlign::Start,
        );
    } else {
        let _ = ui.draw_list_mut().push(DrawCommand::TexturedQuad {
            rect: frame.rect(TEXT_X, top + HEAD - 6.0, MARK, MARK),
            texture: crate::ui_renderer::LOGO_TEXTURE,
            color: Color::new(1.0, 1.0, 1.0, 1.0),
        });
    }
    text(
        ui,
        TextFamily::Display,
        format_args!("{kicker}"),
        frame.rect(TEXT_X + MARK + 16.0, top + HEAD + 8.0, 420.0, 26.0),
        19.0 * s,
        color::GOLD_BRIGHT,
        FontWeight::Semibold,
        TextAlign::Start,
    );
    text(
        ui,
        TextFamily::Body,
        format_args!("{right}"),
        frame.rect(RIGHT - 360.0, top + HEAD + 8.0, 360.0, 26.0),
        16.0 * s,
        color::MUTED,
        FontWeight::Regular,
        TextAlign::End,
    );
}

/// The title's lines from `y`; returns the y under them.
fn title(ui: &mut MenuCanvas, frame: &Frame, value: &str, y: f32) -> f32 {
    let mut y = y;
    for line in wrap(value, TITLE_CHARS).take(2) {
        text(
            ui,
            TextFamily::Display,
            format_args!("{line}"),
            frame.rect(TEXT_X, y, TEXT_WIDTH, TITLE_LINE),
            32.0 * frame.s,
            color::TEXT,
            FontWeight::Semibold,
            TextAlign::Start,
        );
        y += TITLE_LINE;
    }
    y
}

/// A person of a round as the list shows them: the name (with its colour codes) or
/// "Someone" when the hub has none.
pub(crate) fn person_name(name: &str) -> String {
    let name = sjk_identity::chat::name_for_display(name);
    if crate::text::Plain(&name).to_string().trim().is_empty() {
        "Someone".to_owned()
    } else {
        name
    }
}

fn rounds_page(
    ui: &mut MenuCanvas,
    frame: &Frame,
    rounds: &[Round],
    index: usize,
    scroll: usize,
    waiting: bool,
) -> u16 {
    let s = frame.s;
    let round = rounds.get(index);
    let note = round
        .map(|round| plain_line(&round.note, 300))
        .unwrap_or_default();
    let note_lines: Vec<&str> = wrap(&note, LINE_CHARS).collect();
    let people = round.map_or(0, |round| round.people.len());
    let rows = people.clamp(1, ROWS);
    let list_top = TITLE_TOP + TITLE_LINE + 14.0 + note_lines.len() as f32 * LINE + 16.0;
    let foot = list_top + rows as f32 * ROW + 26.0 + BUTTONS_GAP;
    let height = foot + BUTTON_HEIGHT + MARGIN;
    let top = top_of(height);
    card(ui, frame, top, height);
    let place = if rounds.len() > 1 {
        format!("REWARD ROUND {} OF {}", index + 1, rounds.len())
    } else {
        "REWARD ROUND".to_owned()
    };
    let date = round.map_or_else(String::new, |round| crate::holocrons::when_text(round.at));
    head(ui, frame, top, true, &place, &date);
    let heading = match round {
        Some(round) => plain_line(&round.title, sjk_identity::news::TITLE_MAX),
        None if waiting => "Reading the rounds...".to_owned(),
        None => "No reward round yet".to_owned(),
    };
    title(ui, frame, &heading, top + TITLE_TOP);
    let mut y = top + TITLE_TOP + TITLE_LINE + 14.0;
    for line in &note_lines {
        text(
            ui,
            TextFamily::Body,
            format_args!("{line}"),
            frame.rect(TEXT_X, y, TEXT_WIDTH, LINE),
            18.0 * s,
            color::MUTED,
            FontWeight::Regular,
            TextAlign::Start,
        );
        y += LINE;
    }
    let list_y = top + list_top;
    ui.scroll_region(
        LIST_TOKEN,
        frame.rect(TEXT_X - 10.0, list_y, TEXT_WIDTH + 20.0, rows as f32 * ROW),
    );
    if let Some(round) = round {
        for (row, person) in round.people.iter().skip(scroll).take(ROWS).enumerate() {
            let y = list_y + row as f32 * ROW;
            if row % 2 == 0 {
                let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
                    rect: frame.rect(TEXT_X - 10.0, y, TEXT_WIDTH + 20.0, ROW - 2.0),
                    radius: 6.0 * s,
                    color: color::alpha(color::HOLO, 0.05),
                });
            }
            let name = person_name(&person.name);
            let mut x = TEXT_X;
            if let Some(clan) = crate::jof_tag::clan(&name) {
                let side = crate::jof_tag::side(18.0);
                let [left, middle] = frame.point(x, y + ROW * 0.5 - 1.0);
                crate::jof_tag::draw(clan, left, middle, side * s, 1.0, |command| {
                    let _ = ui.draw_list_mut().push(command);
                });
                x += crate::jof_tag::room(clan, side);
            }
            text(
                ui,
                TextFamily::Body,
                format_args!("{name}"),
                frame.rect(x, y + 3.0, NAME_WIDTH - (x - TEXT_X) - 30.0, ROW - 6.0),
                18.0 * s,
                color::TEXT,
                FontWeight::Semibold,
                TextAlign::Start,
            );
            if let Some(mark) =
                crate::ui_renderer::mark_texture(person.verified, sjk_identity::Tick::None, None)
            {
                let side = 16.0;
                let _ = ui.draw_list_mut().push(DrawCommand::TexturedQuad {
                    rect: frame.rect(
                        TEXT_X + NAME_WIDTH - 26.0,
                        y + (ROW - side) * 0.5 - 1.0,
                        side,
                        side,
                    ),
                    texture: mark,
                    color: Color::new(1.0, 1.0, 1.0, 1.0),
                });
            }
            let reason = plain_line(&person.reason, 200);
            let reason: String = if reason.chars().count() > 64 {
                let cut: String = reason.chars().take(62).collect();
                format!("{}...", cut.trim_end())
            } else {
                reason
            };
            text(
                ui,
                TextFamily::Body,
                format_args!("{reason}"),
                frame.rect(
                    TEXT_X + NAME_WIDTH,
                    y + 3.0,
                    TEXT_WIDTH - NAME_WIDTH,
                    ROW - 6.0,
                ),
                17.0 * s,
                color::MUTED,
                FontWeight::Regular,
                TextAlign::Start,
            );
        }
    }
    let below = list_y + rows as f32 * ROW + 2.0;
    if people > ROWS {
        text(
            ui,
            TextFamily::Body,
            format_args!(
                "{} to {} of {}: Up, Down or the wheel",
                scroll + 1,
                (scroll + ROWS).min(people),
                people
            ),
            frame.rect(TEXT_X, below, TEXT_WIDTH, 22.0),
            15.0 * s,
            color::QUIET,
            FontWeight::Regular,
            TextAlign::Start,
        );
    }
    let foot = top + foot;
    kit::button(
        ui,
        frame,
        [RIGHT - BUTTON_WIDTH, foot, BUTTON_WIDTH, BUTTON_HEIGHT],
        "Close",
        true,
        true,
        false,
        CLOSE_TOKEN,
    );
    let mut hints: Vec<(&[&str], &str)> = vec![(&["Esc"], "close")];
    if rounds.len() > 1 {
        kit::button(
            ui,
            frame,
            [TEXT_X, foot, 150.0, BUTTON_HEIGHT],
            "Older",
            false,
            index + 1 < rounds.len(),
            false,
            PREVIOUS_TOKEN,
        );
        kit::button(
            ui,
            frame,
            [TEXT_X + 150.0 + BUTTON_GAP, foot, 150.0, BUTTON_HEIGHT],
            "Newer",
            false,
            index > 0,
            false,
            NEXT_TOKEN,
        );
        hints.insert(0, (&["Left", "Right"], "older, newer"));
    }
    keys(ui, frame, &hints);
    CLOSE_TOKEN
}

/// What a reward gives, one part a line: holocrons by tier in the tiers' order (the hub
/// lists them by name), then credits and shards.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Item {
    Holocrons(&'static crate::holocrons::Tier, u32),
    Credits(u64),
    Shards(u64),
}

/// The items of `reward`, a tier the client does not know left out.
pub(crate) fn items(reward: &Reward) -> Vec<Item> {
    let mut items: Vec<Item> = crate::holocrons::TIERS
        .iter()
        .filter_map(|tier| {
            let count = *reward.holocrons.get(tier.id)?;
            (count > 0).then_some(Item::Holocrons(tier, count))
        })
        .collect();
    if reward.credits > 0 {
        items.push(Item::Credits(reward.credits));
    }
    if reward.shards > 0 {
        items.push(Item::Shards(reward.shards));
    }
    items
}

/// An item in words: "2 Rare Holocrons", "500 credits", "1 kyber shard".
pub(crate) fn item_words(item: &Item) -> String {
    let plural = |count: u64, one: &str, many: &str| {
        if count == 1 {
            format!("1 {one}")
        } else {
            format!("{count} {many}")
        }
    };
    match item {
        Item::Holocrons(tier, count) => crate::terms::say_owned(plural(
            u64::from(*count),
            tier.name,
            &format!("{}s", tier.name),
        )),
        Item::Credits(count) => plural(*count, "credit", "credits"),
        Item::Shards(count) => plural(*count, "kyber shard", "kyber shards"),
    }
}

const ITEM_ROW: f32 = 46.0;

fn claim_page(
    ui: &mut MenuCanvas,
    frame: &Frame,
    reward: &Reward,
    busy: bool,
    message: &str,
) -> u16 {
    let s = frame.s;
    let items = items(reward);
    let reason = plain_line(&reward.reason, 200);
    let why = format!("Why: {reason}");
    let why_lines: Vec<&str> = wrap(&why, LINE_CHARS).collect();
    let items_top = TITLE_TOP + TITLE_LINE + 52.0;
    let why_top = items_top + items.len().max(1) as f32 * ITEM_ROW + 12.0;
    let message_room = if message.is_empty() { 0.0 } else { LINE + 6.0 };
    let foot = why_top + why_lines.len() as f32 * LINE + message_room + BUTTONS_GAP;
    let height = foot + BUTTON_HEIGHT + MARGIN;
    let top = top_of(height);
    card(ui, frame, top, height);
    head(
        ui,
        frame,
        top,
        true,
        "CONTRIBUTOR REWARD",
        &plain_line(&reward.round_title, sjk_identity::news::TITLE_MAX),
    );
    title(ui, frame, "You received", top + TITLE_TOP);
    text(
        ui,
        TextFamily::Body,
        format_args!("From the SJK team, thank you."),
        frame.rect(TEXT_X, top + TITLE_TOP + TITLE_LINE + 6.0, TEXT_WIDTH, 24.0),
        17.0 * s,
        color::MUTED,
        FontWeight::Regular,
        TextAlign::Start,
    );
    let mut y = top + items_top;
    for item in &items {
        let middle = y + ITEM_ROW * 0.5;
        let icon = 34.0;
        match item {
            Item::Holocrons(tier, _) => {
                if crate::holocrons::icons::is_loaded(tier.index) {
                    let _ = ui.draw_list_mut().push(DrawCommand::TexturedQuad {
                        rect: frame.rect(TEXT_X, middle - icon * 0.5, icon, icon),
                        texture: crate::holocrons::icons::texture(tier.index),
                        color: Color::new(1.0, 1.0, 1.0, 1.0),
                    });
                } else {
                    crate::holocrons::gem::draw(
                        ui.draw_list_mut(),
                        frame.point(TEXT_X + icon * 0.5, middle),
                        icon * 0.5 * s,
                        tier.colour,
                        1.0,
                        8,
                    );
                }
            }
            Item::Credits(_) => {
                let height = 26.0 * s;
                let width = crate::cards::mark::width(height);
                let [x, y] = frame.point(TEXT_X + icon * 0.5, middle);
                crate::cards::mark::draw(
                    ui.draw_list_mut(),
                    [x - width * 0.5, y - height * 0.5],
                    height,
                    color::GOLD_BRIGHT,
                );
            }
            Item::Shards(_) => {
                let height = 30.0 * s;
                let width = crate::cards::shard::width(height);
                let [x, y] = frame.point(TEXT_X + icon * 0.5, middle);
                crate::cards::shard::draw(
                    ui.draw_list_mut(),
                    [x - width * 0.5, y - height * 0.5],
                    height,
                    color::HOLO,
                );
            }
        }
        let colour = match item {
            Item::Holocrons(tier, _) => tier.colour,
            _ => color::TEXT,
        };
        text(
            ui,
            TextFamily::Display,
            format_args!("{}", item_words(item)),
            frame.rect(
                TEXT_X + icon + 18.0,
                y + 6.0,
                TEXT_WIDTH - icon - 18.0,
                ITEM_ROW - 12.0,
            ),
            24.0 * s,
            colour,
            FontWeight::Semibold,
            TextAlign::Start,
        );
        y += ITEM_ROW;
    }
    if items.is_empty() {
        text(
            ui,
            TextFamily::Body,
            format_args!("Something this version of SJK cannot show yet."),
            frame.rect(TEXT_X, y + 8.0, TEXT_WIDTH, LINE),
            18.0 * s,
            color::MUTED,
            FontWeight::Regular,
            TextAlign::Start,
        );
    }
    let mut y = top + why_top;
    for line in &why_lines {
        text(
            ui,
            TextFamily::Body,
            format_args!("{line}"),
            frame.rect(TEXT_X, y, TEXT_WIDTH, LINE),
            18.0 * s,
            color::TEXT,
            FontWeight::Regular,
            TextAlign::Start,
        );
        y += LINE;
    }
    if !message.is_empty() {
        text(
            ui,
            TextFamily::Body,
            format_args!("{message}"),
            frame.rect(TEXT_X, y + 6.0, TEXT_WIDTH, LINE),
            17.0 * s,
            color::EMBER,
            FontWeight::Regular,
            TextAlign::Start,
        );
    }
    let foot = top + foot;
    kit::button(
        ui,
        frame,
        [RIGHT - BUTTON_WIDTH, foot, BUTTON_WIDTH, BUTTON_HEIGHT],
        if busy { "Claiming..." } else { "Claim" },
        true,
        !busy,
        false,
        PRIMARY_TOKEN,
    );
    kit::button(
        ui,
        frame,
        [
            RIGHT - BUTTON_WIDTH - BUTTON_GAP - 150.0,
            foot,
            150.0,
            BUTTON_HEIGHT,
        ],
        "Later",
        false,
        !busy,
        false,
        LATER_TOKEN,
    );
    keys(ui, frame, &[(&["Enter"], "claim"), (&["Esc"], "later")]);
    PRIMARY_TOKEN
}

fn compose_page(ui: &mut MenuCanvas, frame: &Frame, composer: &Composer) -> u16 {
    let s = frame.s;
    let label_width = 110.0;
    let field_x = TEXT_X + label_width;
    let field_width = TEXT_WIDTH - label_width;
    let mut y = TITLE_TOP + TITLE_LINE + 20.0;
    let kind_y = y;
    y += FIELD_HEIGHT + 14.0;
    let version_y = composer.release.then_some(y);
    if composer.release {
        y += FIELD_HEIGHT + 14.0;
    }
    let title_y = y;
    y += FIELD_HEIGHT + 14.0;
    let text_y = y;
    let text_height = TEXT_ROWS as f32 * 24.0 + 20.0;
    y += text_height + 18.0;
    let preview_y = y;
    y += 70.0;
    let message_y = y;
    if !composer.message.is_empty() || composer.confirm || composer.busy {
        y += LINE + 8.0;
    }
    let foot = y + BUTTONS_GAP - 16.0;
    let height = foot + BUTTON_HEIGHT + MARGIN;
    let top = top_of(height);
    card(ui, frame, top, height);
    head(
        ui,
        frame,
        top,
        false,
        "SJK TEAM ANNOUNCEMENT",
        "Every SJK player reads it",
    );
    title(ui, frame, "Announce", top + TITLE_TOP);
    let label = |ui: &mut MenuCanvas, value: &str, y: f32| {
        text(
            ui,
            TextFamily::Body,
            format_args!("{value}"),
            frame.rect(TEXT_X, top + y + 10.0, label_width, 24.0),
            17.0 * s,
            color::MUTED,
            FontWeight::Regular,
            TextAlign::Start,
        );
    };
    let focused = |field: Field| composer.focus == field && !composer.confirm;
    label(ui, "Kind", kind_y);
    let segments = [
        ("News", NEWS_TOKEN, !composer.release),
        ("Release", RELEASE_TOKEN, composer.release),
    ];
    let mut x = field_x;
    for (name, token, chosen) in segments {
        kit::button(
            ui,
            frame,
            [x, top + kind_y, 150.0, FIELD_HEIGHT],
            name,
            chosen,
            true,
            focused(Field::Kind) && chosen,
            token,
        );
        x += 150.0 + BUTTON_GAP;
    }
    let caret = |value: &str, field: Field| {
        if focused(field) {
            format!("{value}_")
        } else {
            value.to_owned()
        }
    };
    if let Some(version_y) = version_y {
        label(ui, "Version", version_y);
        let shown = caret(&composer.version, Field::Version);
        let shown = if shown.is_empty() {
            "2026.1025.1".to_owned()
        } else {
            shown
        };
        kit::field(
            ui,
            frame,
            [field_x, top + version_y, 300.0, FIELD_HEIGHT],
            format_args!("{shown}"),
            focused(Field::Version),
            false,
        );
        ui.hit_region(
            VERSION_TOKEN,
            frame.rect(field_x, top + version_y, 300.0, FIELD_HEIGHT),
        );
    }
    label(ui, "Title", title_y);
    let shown = caret(&composer.title, Field::Title);
    kit::field(
        ui,
        frame,
        [field_x, top + title_y, field_width, FIELD_HEIGHT],
        format_args!("{shown}"),
        focused(Field::Title),
        false,
    );
    ui.hit_region(
        TITLE_TOKEN,
        frame.rect(field_x, top + title_y, field_width, FIELD_HEIGHT),
    );
    label(ui, "Text", text_y);
    let area = [field_x, top + text_y, field_width, text_height];
    let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
        rect: frame.rect(area[0], area[1], area[2], area[3]),
        radius: 10.0 * s,
        color: color::alpha(color::SPACE, 0.6),
    });
    let _ = ui.draw_list_mut().push(DrawCommand::Border {
        rect: frame.rect(area[0], area[1], area[2], area[3]),
        radius: 10.0 * s,
        width: if focused(Field::Text) { 2.0 } else { 1.0 } * s,
        color: if focused(Field::Text) {
            color::TEXT
        } else {
            color::alpha(color::HOLO, 0.35)
        },
    });
    if focused(Field::Text) && crate::text_entry::is_selected(&composer.text) {
        let _ = ui.draw_list_mut().push(DrawCommand::SolidRect {
            rect: frame.rect(area[0] + 8.0, area[1] + 8.0, area[2] - 16.0, area[3] - 16.0),
            color: color::alpha(color::HOLO, 0.25),
        });
    }
    let mut rows: Vec<String> = Vec::new();
    for line in caret(&composer.text, Field::Text).split('\n') {
        if line.is_empty() {
            rows.push(String::new());
            continue;
        }
        rows.extend(wrap(line, 74).map(str::to_owned));
    }
    let first = rows.len().saturating_sub(TEXT_ROWS);
    for (index, row) in rows[first..].iter().enumerate() {
        text(
            ui,
            TextFamily::Body,
            format_args!("{row}"),
            frame.rect(
                area[0] + 14.0,
                area[1] + 10.0 + index as f32 * 24.0,
                area[2] - 28.0,
                24.0,
            ),
            16.0 * s,
            color::TEXT,
            FontWeight::Regular,
            TextAlign::Start,
        );
    }
    let count = composer.text.chars().count();
    text(
        ui,
        TextFamily::Body,
        format_args!(
            "{count} / {}, {} / {} lines",
            sjk_identity::news::TEXT_MAX,
            composer.text.split('\n').count(),
            sjk_identity::news::LINES_MAX
        ),
        frame.rect(area[0], area[1] + area[3] + 2.0, area[2], 18.0),
        13.0 * s,
        color::QUIET,
        FontWeight::Regular,
        TextAlign::End,
    );
    ui.hit_region(TEXT_TOKEN, frame.rect(area[0], area[1], area[2], area[3]));
    // The chat line as players will read it: SJK's logo, "SJK" in gold, the words.
    label(ui, "Preview", preview_y);
    let logo = 22.0;
    let _ = ui.draw_list_mut().push(DrawCommand::TexturedQuad {
        rect: frame.rect(field_x, top + preview_y + 11.0, logo, logo),
        texture: crate::ui_renderer::LOGO_TEXTURE,
        color: Color::new(1.0, 1.0, 1.0, 1.0),
    });
    text(
        ui,
        TextFamily::Body,
        format_args!("{SENDER}:"),
        frame.rect(field_x + logo + 8.0, top + preview_y + 10.0, 60.0, 24.0),
        17.0 * s,
        color::GOLD,
        FontWeight::Semibold,
        TextAlign::Start,
    );
    let preview = composer.preview();
    for (index, line) in wrap(&preview, 66).take(2).enumerate() {
        text(
            ui,
            TextFamily::Body,
            format_args!("{line}"),
            frame.rect(
                field_x + logo + 64.0,
                top + preview_y + 10.0 + index as f32 * 24.0,
                field_width - logo - 64.0,
                24.0,
            ),
            17.0 * s,
            crate::sjk_chat_look::GOLD,
            FontWeight::Regular,
            TextAlign::Start,
        );
    }
    let notice = if composer.busy {
        ("Sending...".to_owned(), color::MUTED)
    } else if composer.confirm {
        (
            "Send this to every SJK player? It cannot be taken back from screens that showed it."
                .to_owned(),
            color::GOLD_BRIGHT,
        )
    } else {
        (composer.message.clone(), color::EMBER)
    };
    if !notice.0.is_empty() {
        text(
            ui,
            TextFamily::Body,
            format_args!("{}", notice.0),
            frame.rect(TEXT_X, top + message_y, TEXT_WIDTH, LINE),
            17.0 * s,
            notice.1,
            FontWeight::Regular,
            TextAlign::Start,
        );
    }
    let foot = top + foot;
    if composer.confirm || composer.busy {
        kit::button(
            ui,
            frame,
            [RIGHT - BUTTON_WIDTH, foot, BUTTON_WIDTH, BUTTON_HEIGHT],
            "Send now",
            true,
            !composer.busy,
            false,
            CONFIRM_TOKEN,
        );
        kit::button(
            ui,
            frame,
            [
                RIGHT - BUTTON_WIDTH - BUTTON_GAP - 150.0,
                foot,
                150.0,
                BUTTON_HEIGHT,
            ],
            "Back",
            false,
            !composer.busy,
            false,
            BACK_TOKEN,
        );
        keys(ui, frame, &[(&["Enter"], "send now"), (&["Esc"], "back")]);
        return CONFIRM_TOKEN;
    }
    kit::button(
        ui,
        frame,
        [RIGHT - BUTTON_WIDTH, foot, BUTTON_WIDTH, BUTTON_HEIGHT],
        "Send",
        true,
        true,
        false,
        PRIMARY_TOKEN,
    );
    kit::button(
        ui,
        frame,
        [
            RIGHT - BUTTON_WIDTH - BUTTON_GAP - 150.0,
            foot,
            150.0,
            BUTTON_HEIGHT,
        ],
        "Cancel",
        false,
        true,
        false,
        LATER_TOKEN,
    );
    keys(
        ui,
        frame,
        &[
            (&["Tab"], "next field"),
            (&["Ctrl", "L"], "link"),
            (&["Ctrl", "K"], "switch"),
            (&["Ctrl", "I"], "picture"),
            (&["Ctrl", "Enter"], "send"),
            (&["Esc"], "cancel"),
        ],
    );
    if let Some(at) = composer.picker {
        picker(ui, frame, top, at);
    }
    match composer.focus {
        Field::Kind => NEWS_TOKEN,
        Field::Version => VERSION_TOKEN,
        Field::Title => TITLE_TOKEN,
        Field::Text => TEXT_TOKEN,
    }
}

/// The picker of places a link can open, over the composer from `top`, place `at` lit.
fn picker(ui: &mut MenuCanvas, frame: &Frame, top: f32, at: usize) {
    let pages = crate::ui_links::PAGES;
    let row = 30.0;
    let [x, y, width] = [TEXT_X + 110.0, top + 120.0, 440.0];
    let height = 60.0 + pages.len() as f32 * row;
    kit::card(ui, frame, [x, y, width, height]);
    text(
        ui,
        TextFamily::Display,
        format_args!("Link to (Up, Down, Enter; Esc closes)"),
        frame.rect(x + 20.0, y + 14.0, width - 40.0, 28.0),
        18.0 * frame.s,
        color::GOLD_BRIGHT,
        FontWeight::Semibold,
        TextAlign::Start,
    );
    for (index, page) in pages.iter().enumerate() {
        let row_y = y + 50.0 + index as f32 * row;
        if index == at {
            let _ = ui.draw_list_mut().push(sjk_ui::DrawCommand::RoundedRect {
                rect: frame.rect(x + 10.0, row_y, width - 20.0, row - 2.0),
                radius: 6.0 * frame.s,
                color: color::alpha(color::HOLO, 0.18),
            });
        }
        text(
            ui,
            TextFamily::Body,
            format_args!("{}   open:{}", page.name, page.id),
            frame.rect(x + 22.0, row_y, width - 44.0, row),
            17.0 * frame.s,
            if index == at {
                color::TEXT
            } else {
                color::MUTED
            },
            FontWeight::Regular,
            TextAlign::Start,
        );
    }
}

/// The key hints, right-aligned at the foot of the screen.
fn keys(ui: &mut MenuCanvas, frame: &Frame, hints: &[(&[&str], &str)]) {
    let s = frame.s;
    let [right, keys_y] = frame.point(KEYS_RIGHT, KEYS_Y);
    let width: f32 = hints
        .iter()
        .map(|(keys, label)| key_hint_width(keys, label, s) + 18.0 * s)
        .sum();
    let mut x = right - width;
    for (keys, label) in hints {
        x = key_hint(ui, keys, label, x, keys_y, s) + 18.0 * s;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn a_rewards_items_follow_the_tiers_order_then_credits_and_shards() {
        let reward = Reward {
            holocrons: BTreeMap::from([("rare".to_owned(), 2), ("legendary".to_owned(), 1)]),
            credits: 500,
            shards: 1,
            ..Reward::default()
        };
        let words: Vec<String> = items(&reward).iter().map(item_words).collect();
        assert_eq!(
            words,
            [
                "2 Rare Holocrons",
                "1 Legendary Holocron",
                "500 credits",
                "1 kyber shard"
            ]
        );
        let unknown = Reward {
            holocrons: BTreeMap::from([("cosmic".to_owned(), 3)]),
            ..Reward::default()
        };
        assert!(items(&unknown).is_empty());
    }

    #[test]
    fn a_person_without_a_name_is_someone() {
        assert_eq!(person_name(""), "Someone");
        assert_eq!(person_name("^7"), "Someone");
        assert_eq!(person_name("^2Sol"), "^2Sol");
    }

    fn page_of(window: &Window) -> &Page {
        window.page.as_ref().expect("a page")
    }

    /// Ctrl+L makes the setting just typed a link and otherwise opens the picker of
    /// places; Ctrl+K makes it a switch line; a picture's line goes at the end. What they
    /// write passes the markup check the hub makes.
    #[test]
    fn the_composer_writes_links_switches_and_pictures() {
        let mut composer = Composer {
            focus: Field::Text,
            text: "Try cg_nameplate".to_owned(),
            ..Composer::default()
        };
        assert_eq!(composer.key(KeyCode::KeyL, None, false, true), None);
        let label = crate::settings::row_label("cg_nameplate").unwrap();
        assert_eq!(
            composer.text,
            format!("Try [{label}](setting:cg_nameplate)")
        );
        // Nothing to link: the picker opens and Enter puts the place in.
        composer.key(KeyCode::KeyL, None, false, true);
        assert_eq!(composer.picker, Some(0));
        composer.key(KeyCode::ArrowDown, None, false, false);
        composer.key(KeyCode::Enter, None, false, false);
        assert_eq!(composer.picker, None);
        let page = crate::ui_links::PAGES[1];
        assert!(
            composer
                .text
                .ends_with(&format!(" [{}](open:{})", page.name, page.id)),
            "{}",
            composer.text
        );
        composer.text.push_str("\ncg_nameplate");
        composer.key(KeyCode::KeyK, None, false, true);
        assert!(
            composer.text.ends_with("\n[[switch:cg_nameplate]]\n"),
            "{:?}",
            composer.text
        );
        composer.uploading = true;
        composer.add_image("0123456789abcdef");
        assert!(!composer.uploading);
        assert!(composer.text.ends_with("[[image:0123456789abcdef]]\n"));
        assert_eq!(
            sjk_identity::news_markup::check(&composer.text, |_| true),
            Ok(())
        );
        // Ctrl+I asks for a file once at a time.
        assert_eq!(
            composer.key(KeyCode::KeyI, None, false, true),
            Some(Choice::PickImage)
        );
        assert_eq!(composer.key(KeyCode::KeyI, None, false, true), None);
        // Ctrl+K on a word that is no switch says what it wants.
        composer.text.push_str("hello");
        composer.key(KeyCode::KeyK, None, false, true);
        assert!(composer.text.ends_with("hello"));
        assert!(composer.message.contains("Ctrl+K"));
    }

    #[test]
    fn the_composer_types_new_lines_and_asks_before_it_sends() {
        let mut window = Window {
            page: Some(Page::Compose(Composer::default())),
            ..Window::default()
        };
        let type_text = |window: &mut Window, value: &str| {
            for c in value.chars() {
                let typed = c.to_string();
                window.key(KeyCode::KeyX, Some(&typed), false, false);
            }
        };
        type_text(&mut window, "Hi");
        window.key(KeyCode::Enter, None, false, false);
        type_text(&mut window, "Line one");
        window.key(KeyCode::Enter, None, false, false);
        type_text(&mut window, "two");
        window.key(KeyCode::Backspace, None, false, false);
        let Page::Compose(composer) = page_of(&window) else {
            panic!("the composer");
        };
        assert_eq!(
            (composer.title.as_str(), composer.text.as_str()),
            ("Hi", "Line one\ntw")
        );
        assert_eq!(composer.preview(), "Hi: Line one  Read more");
        // Ctrl+Enter asks; Enter then sends what was written.
        assert_eq!(window.key(KeyCode::Enter, None, false, true), None);
        let choice = window.key(KeyCode::Enter, None, false, false);
        let Some(Choice::Send(request)) = choice else {
            panic!("sent, not {choice:?}");
        };
        assert_eq!(request.text, "Line one\ntw");
        // A release needs its version: the check says so, nothing is asked.
        let mut window = Window {
            page: Some(Page::Compose(Composer {
                release: true,
                title: "SJK".to_owned(),
                text: "New".to_owned(),
                ..Composer::default()
            })),
            ..Window::default()
        };
        window.key(KeyCode::Enter, None, false, true);
        let Page::Compose(composer) = page_of(&window) else {
            panic!("the composer");
        };
        assert!(!composer.confirm && !composer.message.is_empty());
    }

    #[test]
    fn rounds_page_through_older_ones_and_a_claim_says_later() {
        let round = |id| Round {
            id,
            title: format!("Round {id}"),
            ..Round::default()
        };
        let mut window = Window {
            page: Some(Page::Rounds {
                rounds: vec![round(2), round(1)],
                index: 0,
                scroll: 0,
                wanted: None,
            }),
            ..Window::default()
        };
        window.key(KeyCode::ArrowLeft, None, false, false);
        window.key(KeyCode::ArrowLeft, None, false, false);
        assert!(matches!(page_of(&window), Page::Rounds { index: 1, .. }));
        assert_eq!(
            window.key(KeyCode::Escape, None, false, false),
            Some(Choice::Close)
        );
        assert!(window.page.is_none());
        let mut window = Window {
            page: Some(Page::Claim {
                reward: Reward {
                    id: 7,
                    ..Reward::default()
                },
                rehearsal: false,
                busy: false,
                message: String::new(),
            }),
            ..Window::default()
        };
        assert_eq!(
            window.key(KeyCode::Escape, None, false, false),
            Some(Choice::Later)
        );
        assert_eq!(
            window.key(KeyCode::Enter, None, false, false),
            Some(Choice::Claim {
                id: 7,
                rehearsal: false
            })
        );
        assert_eq!(window.key(KeyCode::Enter, None, false, false), None, "busy");
    }
}
