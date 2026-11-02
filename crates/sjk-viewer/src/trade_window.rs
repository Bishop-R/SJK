//! The trade window and the trade request toast (`docs/sjk-ui.md`, "Trading"): two SJK
//! players swap unopened holocrons and duplicate saber shaders through the SJK hub
//! (`sjk_identity::trades`). Both are drawn in the SJK UI's look in every menu style.
//!
//! - **The toast** ([`Toast`]) sits at the top centre over play while a trade runs and the
//!   window is closed: who, what they want, the request's clock, and how to answer. It
//!   takes no input: play goes on under it.
//! - **The window** ([`Window`]) is a card in the middle of the screen over a light
//!   shade, so the match still shows round it. In game the pointer is free only in the
//!   menus, so it shows over the game menu: Esc opens it while a trade runs (the menus
//!   under it are not drawn and take no input, as under the other cards), and Esc closes
//!   it again. A request reads Accept or Decline; an open trade shows both sides, the
//!   player's own holocrons and duplicates to add with a click, what each side is worth on
//!   average, today's limits, Ready and then Confirm; an ended one says how it ended.
//!
//! Every number shown comes from the hub (its rules, today's counts, each item's worth):
//! nothing here decides what may trade. The window builds a step and the frame sends it
//! (`trades.rs`).
//!
//! Since 29/10/2026 the player's shaders carry their collection marks
//! (`crate::shader_marks`): a New sticker, the star of a favourite (listed first; a
//! Favourites chip shows them alone) and the lightsaber of one worn; a locked one is shown
//! dimmed with its padlock and answers no click, since the hub would refuse it.

use crate::menu::sjk::{Frame, TextTarget, color, key_hint, key_hint_width, kit, text, wrap};
use crate::menu_widgets::{MenuCanvas, TextFamily};
use sjk_identity::{PlayerTrade, TradeStep};
use sjk_ui::{Color, DrawCommand, FontWeight, InputEvent, Rect, TextAlign, UiEventKind};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// The gold button (Accept, Ready, Confirm, Close), the outlined one (Decline, Not
/// ready) and Cancel trade.
const MAIN_TOKEN: u16 = 1;
const OTHER_TOKEN: u16 = 2;
const CANCEL_TOKEN: u16 = 3;
/// The Favourites chip over the player's own items (29/10/2026).
const FAVOURITES_TOKEN: u16 = 4;
/// The player's offered items (a click takes one back), then their own items (a click
/// offers one).
const OFFERED_FIRST: u16 = 10;
const OWN_FIRST: u16 = 40;

/// The open trade's card (frame pixels): centred, with room round it for the match.
const WIN_X: f32 = 380.0;
const WIN_Y: f32 = 150.0;
const WIN_W: f32 = 1160.0;
const WIN_H: f32 = 780.0;
const PAD: f32 = 40.0;
/// The two columns: the player's side and the other's.
const LEFT_X: f32 = WIN_X + PAD;
const RIGHT_X: f32 = WIN_X + WIN_W * 0.5 + 20.0;
const COLUMN_W: f32 = WIN_W * 0.5 - PAD - 20.0;
/// A tile and the gap between two; six make a side.
const TILE: f32 = 78.0;
const TILE_GAP: f32 = 10.0;
const TILES_ROW: usize = 6;
/// Rows of the player's own items shown.
const OWN_ROWS: usize = 3;
/// The small card of a request or an ended trade.
const SMALL_X: f32 = 560.0;
const SMALL_Y: f32 = 300.0;
const SMALL_W: f32 = 800.0;
/// Characters a line of running text.
const LINE_CHARS: usize = 74;

/// One item on a side or among the player's own, as the window draws it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Item {
    pub(crate) what: What,
    /// "Rare Holocron", "Haze".
    pub(crate) name: String,
    /// The tier's name: "Rare".
    pub(crate) tier: String,
    /// The holocron tier's slot, for its picture; `None` for a shader.
    pub(crate) holocron_slot: Option<usize>,
    pub(crate) colour: Color,
    /// What it brings on average, in kyber shards, as the hub says.
    pub(crate) value: i64,
    /// Seconds until a holocron received in a trade may be traded again (the hub's
    /// resale lock); `None` when it may now.
    pub(crate) locked_for: Option<i64>,
    /// A shader's collection marks (29/10/2026): a favourite, locked by the player (it
    /// may not go in), New, worn.
    pub(crate) favourite: bool,
    pub(crate) locked: bool,
    pub(crate) new: bool,
    pub(crate) worn: bool,
}

/// What an item is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum What {
    Holocron(u64),
    Shader(String),
}

impl Item {
    /// An item of a side, as the hub sent it.
    pub(crate) fn from_wire(item: &sjk_identity::TradeItem) -> Self {
        match (item.holocron, &item.id) {
            (Some(id), _) => Self::holocron(id, &item.tier, item.value),
            (None, Some(id)) => Self::shader(id, item.name.as_deref(), item.value),
            _ => Self::shader("", item.name.as_deref(), item.value),
        }
    }

    /// Holocron `id` of `tier`, worth `value`.
    pub(crate) fn holocron(id: u64, tier: &str, value: i64) -> Self {
        let found = crate::holocrons::by_id(tier);
        Self {
            what: What::Holocron(id),
            name: found.map_or_else(
                || crate::terms::say("Holocron").into_owned(),
                |tier| tier.title().into_owned(),
            ),
            tier: found.map_or_else(String::new, |tier| tier.label.to_owned()),
            holocron_slot: found.map(|tier| tier.index),
            colour: found.map_or(color::HOLO, |tier| tier.colour),
            value,
            locked_for: None,
            favourite: false,
            locked: false,
            new: false,
            worn: false,
        }
    }

    /// A copy of shader `id` (its name `named` when this client does not know it).
    pub(crate) fn shader(id: &str, named: Option<&str>, value: i64) -> Self {
        let found = crate::unlockables::find(id);
        Self {
            what: What::Shader(id.to_owned()),
            name: found
                .map(|skin| skin.name.to_owned())
                .or_else(|| named.map(str::to_owned))
                .unwrap_or_else(|| id.to_owned()),
            tier: found.map_or_else(String::new, |skin| skin.tier.label().to_owned()),
            holocron_slot: None,
            colour: found.map_or(color::HOLO, |skin| skin.tier.colour()),
            value,
            locked_for: None,
            favourite: false,
            locked: false,
            new: false,
            worn: false,
        }
    }

    /// Whether the player may put it in now: not waiting for the hub's resale lock, not
    /// locked by the player.
    pub(crate) fn offerable(&self) -> bool {
        self.locked_for.is_none() && !self.locked
    }
}

/// What the window shows this frame, read from the identity service's snapshot.
#[derive(Clone, Debug, Default)]
pub(crate) struct View {
    pub(crate) trade: PlayerTrade,
    /// The other player's name, colour codes taken out (for running text).
    pub(crate) partner: String,
    /// Their name as they wear it, colours kept and white after ("^1Nib^7").
    pub(crate) partner_shown: String,
    /// Their SJK key and picture's version (empty for none), for their picture.
    pub(crate) partner_key: String,
    pub(crate) partner_avatar: String,
    pub(crate) mine: Vec<Item>,
    pub(crate) theirs: Vec<Item>,
    /// The player's items that may still go in (not offered yet), most worth first.
    pub(crate) own: Vec<Item>,
    /// The most items a side holds.
    pub(crate) items_max: usize,
    /// Seconds until the trade ends by itself (a request's answer, an idle trade).
    pub(crate) seconds_left: u64,
    /// The same, to the fraction: the bars run down smoothly with it.
    pub(crate) left_exact: f32,
    /// The request's whole wait, for its bar.
    pub(crate) ask_secs: u64,
    /// Seconds until a confirmation is taken, while both are ready.
    pub(crate) confirm_in: Option<u64>,
    /// The day's limits, a row a kind: received and given, each as done and most.
    pub(crate) limits: Vec<Limit>,
    /// Whether holocrons trade yet (duplicates come first): without, only duplicates show.
    pub(crate) holocrons: bool,
    /// Seconds until the player's trading day ends and every count resets; `None` while
    /// none runs (it starts with their first completed trade).
    pub(crate) resets_in: Option<u64>,
    /// How many more the player may still give today, past what they offer now.
    pub(crate) room: Room,
    /// Why the last step failed, in the player's words.
    pub(crate) problem: String,
    /// The player pressed Ready on a gift: the window asks once more (the window sets
    /// it).
    pub(crate) confirming_gift: bool,
    /// The hub keeps collection marks: the Favourites chip shows (29/10/2026).
    pub(crate) marks_kept: bool,
    /// Only the favourites among the player's own items show (the window sets it).
    pub(crate) favourites_only: bool,
}

/// One kind's daily limits: (done, most) received and given.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Limit {
    pub(crate) label: &'static str,
    pub(crate) received: (i64, i64),
    pub(crate) given: (i64, i64),
    /// The colour its name reads in: a holocron tier's own, else muted.
    pub(crate) colour: Option<Color>,
}

/// How many more of each kind the player may still give today, past their offer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Room {
    pub(crate) holocrons: i64,
    /// Holocrons of each tier, by its slot (Uncommon 0 to Mythical 3), as the hub limits them.
    pub(crate) tiers: [i64; 4],
    pub(crate) duplicates: i64,
}

impl Room {
    /// Plenty of room: nothing is held back (a hub that sends no limits).
    #[cfg(test)]
    pub(crate) const OPEN: Self = Self {
        holocrons: i64::MAX,
        tiers: [i64::MAX; 4],
        duplicates: i64::MAX,
    };

    /// Whether `item` may still go in today.
    pub(crate) fn takes(&self, item: &Item) -> bool {
        if item.locked_for.is_some() {
            return false;
        }
        match item.what {
            What::Holocron(_) => {
                self.holocrons > 0
                    && item
                        .holocron_slot
                        .and_then(|slot| self.tiers.get(slot))
                        .is_none_or(|left| *left > 0)
            }
            What::Shader(_) => self.duplicates > 0,
        }
    }

    /// Why some of the player's items wait today, if the day's limits are why.
    fn note(&self, own: &[Item]) -> Option<String> {
        let held = |item: &Item| item.locked_for.is_none() && !self.takes(item);
        let holocron = own
            .iter()
            .any(|item| item.holocron_slot.is_some() && held(item));
        let shader = own
            .iter()
            .any(|item| item.holocron_slot.is_none() && held(item));
        // The tiers held back, rarest last: "Rare and Mythical".
        let mut tiers: Vec<(usize, &str)> = own
            .iter()
            .filter(|item| held(item))
            .filter_map(|item| Some((item.holocron_slot?, item.tier.as_str())))
            .collect();
        tiers.sort_unstable();
        tiers.dedup();
        let names: Vec<&str> = tiers.iter().map(|(_, name)| *name).collect();
        let tiers = match names.as_slice() {
            [] => String::new(),
            [one] => format!("today's {one} holocrons"),
            [rest @ .., last] => format!("today's {} and {last} holocrons", rest.join(", ")),
        };
        let limit = match (holocron, shader) {
            (true, _) if self.holocrons <= 0 && shader => {
                "You have given today's holocrons and duplicates.".to_owned()
            }
            (true, false) if self.holocrons <= 0 => "Today's holocrons are given.".to_owned(),
            (true, true) => format!("You have given {tiers} and today's duplicates."),
            (true, false) => format!("You have given {tiers}."),
            (false, true) => "Today's duplicates are given.".to_owned(),
            (false, false) => return None,
        };
        Some(crate::terms::say_owned(format!("{limit} More tomorrow.")))
    }
}

/// The warning over the player's items while any waits after a trade; each such tile
/// shows its own countdown. How long is the hub's (`resale_lock`), so it is not named.
const LOCK_WARNING: &str =
    "Holocrons received in a trade wait before they can be traded again: each shows how long.";

impl View {
    /// The player gives and gets nothing back.
    fn gives_gift(&self) -> bool {
        !self.mine.is_empty() && self.theirs.is_empty()
    }

    /// The other player gives and asks nothing back.
    fn receives_gift(&self) -> bool {
        self.mine.is_empty() && !self.theirs.is_empty()
    }

    /// The side with one item more or one fewer: the step that offers it.
    fn offer_with(&self, add: Option<&Item>, remove: Option<usize>) -> TradeStep {
        let (mut holocrons, mut shaders) = (Vec::new(), Vec::new());
        for (index, item) in self.mine.iter().chain(add).enumerate() {
            if Some(index) == remove {
                continue;
            }
            match &item.what {
                What::Holocron(id) => holocrons.push(*id),
                What::Shader(id) => shaders.push(id.clone()),
            }
        }
        TradeStep::Offer { holocrons, shaders }
    }
}

/// What the player chose in the window.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Choice {
    /// Send this step for the trade.
    Step(i64, TradeStep),
    /// Close the window: the trade goes on, the toast says so.
    Close,
}

/// The trade window.
pub(crate) struct Window {
    ui: MenuCanvas,
    view: Option<View>,
    open: bool,
    /// The trade and revision a gift's Ready was asked for: the window asks once more
    /// until the offers change.
    gift_asked: Option<(i64, i64)>,
    /// How big the window draws ([`Size`], the player's setting).
    size: Size,
    /// The Favourites chip is on: only the favourites among the player's items show.
    favourites_only: bool,
}

/// The window's size, the player's choice (`cl_tradeWindowSize`): the card scales about
/// the frame's middle.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Size {
    Small,
    #[default]
    Normal,
    Large,
}

impl Size {
    /// The setting's value read: 0 small, 1 normal, 2 large.
    pub(crate) fn from_value(value: i64) -> Self {
        match value {
            0 => Self::Small,
            2 => Self::Large,
            _ => Self::Normal,
        }
    }

    /// How much of the card's full design size it draws at.
    fn scale(self) -> f32 {
        match self {
            Self::Small => 0.8,
            Self::Normal => 0.9,
            Self::Large => 1.0,
        }
    }

    /// `frame` scaled about its middle.
    fn frame(self, frame: &Frame) -> Frame {
        let k = self.scale();
        let [cx, cy] = frame.point(960.0, 540.0);
        Frame {
            s: frame.s * k,
            origin: [cx - 960.0 * frame.s * k, cy - 540.0 * frame.s * k],
        }
    }
}

impl Default for Window {
    fn default() -> Self {
        Self {
            // The marks (29/10/2026) add a sticker and a run of marks to a tile at most.
            ui: MenuCanvas::with_capacities(240, 160, 1_000),
            view: None,
            open: false,
            gift_asked: None,
            size: Size::default(),
            favourites_only: false,
        }
    }
}

impl Window {
    /// What to show; `None` hides it.
    pub(crate) fn set_view(&mut self, view: Option<View>) {
        self.view = view.map(|mut view| {
            view.confirming_gift = self.gift_asked == Some((view.trade.id, view.trade.rev))
                && view.gives_gift()
                && !view.trade.mine.ready;
            view.favourites_only = self.favourites_only && view.marks_kept;
            view
        });
        if !self.view.as_ref().is_some_and(|view| view.confirming_gift) {
            self.gift_asked = None;
        }
    }

    /// Change the view shown in place (its clocks, its refusal), keeping what the window
    /// set on it; nothing without one.
    pub(crate) fn update_view(&mut self, change: impl FnOnce(&mut View)) {
        if let Some(view) = self.view.as_mut() {
            change(view);
        }
    }

    /// A world shot's gift question: the window asks once more for the view it gets.
    #[cfg(test)]
    pub(crate) fn ask_gift_for_shot(&mut self, trade: i64, rev: i64) {
        self.gift_asked = Some((trade, rev));
    }

    pub(crate) fn has_view(&self) -> bool {
        self.view.is_some()
    }

    pub(crate) fn set_open(&mut self, open: bool) {
        self.open = open;
    }

    /// How big the window draws.
    pub(crate) fn set_size(&mut self, size: Size) {
        self.size = size;
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    /// Enter takes the gold button (Accept, Ready, Confirm, Close), Escape closes the
    /// window and leaves the trade as it is; every other key is swallowed while it shows.
    pub(crate) fn handle_key(&mut self, event: &KeyEvent) -> Option<Choice> {
        if event.state != ElementState::Pressed || event.repeat {
            return None;
        }
        let PhysicalKey::Code(code) = event.physical_key else {
            return None;
        };
        self.key(code)
    }

    fn key(&mut self, code: KeyCode) -> Option<Choice> {
        match code {
            KeyCode::Enter | KeyCode::NumpadEnter => self.main(),
            KeyCode::Escape => Some(Choice::Close),
            _ => None,
        }
    }

    pub(crate) fn handle_pointer(&mut self, event: InputEvent) -> Option<Choice> {
        let event = self.ui.pointer(event)?;
        if event.kind != UiEventKind::Activate {
            return None;
        }
        self.press(event.token?)
    }

    fn press(&mut self, token: u16) -> Option<Choice> {
        let view = self.view.as_ref()?;
        let id = view.trade.id;
        match token {
            MAIN_TOKEN => self.main(),
            OTHER_TOKEN => match buttons(view).1 {
                Some((_, action)) => self.act(id, action),
                None => None,
            },
            CANCEL_TOKEN if view.trade.is_live() => Some(Choice::Step(id, TradeStep::Cancel)),
            token if (OFFERED_FIRST..OWN_FIRST).contains(&token) && view.trade.is_open() => {
                let index = usize::from(token - OFFERED_FIRST);
                (index < view.mine.len())
                    .then(|| Choice::Step(id, view.offer_with(None, Some(index))))
            }
            FAVOURITES_TOKEN if view.marks_kept => {
                self.favourites_only = !view.favourites_only;
                let on = self.favourites_only;
                if let Some(view) = self.view.as_mut() {
                    view.favourites_only = on;
                }
                None
            }
            token if token >= OWN_FIRST && view.trade.is_open() => {
                let item = view.own.get(usize::from(token - OWN_FIRST))?;
                (view.mine.len() < view.items_max && view.room.takes(item) && item.offerable())
                    .then(|| Choice::Step(id, view.offer_with(Some(item), None)))
            }
            _ => None,
        }
    }

    /// The gold button's choice.
    fn main(&mut self) -> Option<Choice> {
        let view = self.view.as_ref()?;
        let id = view.trade.id;
        match buttons(view).0 {
            Some((_, action, true)) => self.act(id, action),
            _ => None,
        }
    }

    /// What `action` chooses for trade `id`; the gift's question is the window's own.
    fn act(&mut self, id: i64, action: Do) -> Option<Choice> {
        match action {
            Do::Step(step) => {
                self.gift_asked = None;
                Some(Choice::Step(id, step))
            }
            Do::Close => Some(Choice::Close),
            Do::AskGift => {
                if let Some(view) = self.view.as_mut() {
                    self.gift_asked = Some((view.trade.id, view.trade.rev));
                    view.confirming_gift = true;
                }
                None
            }
            Do::BackFromGift => {
                self.gift_asked = None;
                if let Some(view) = self.view.as_mut() {
                    view.confirming_gift = false;
                }
                None
            }
            Do::Nothing => None,
        }
    }

    pub(crate) fn draw_list(&self) -> &sjk_ui::DrawList {
        self.ui.draw_list()
    }

    /// Draw the window over the whole frame.
    pub(crate) fn append(&mut self, target: TextTarget<'_>, viewport: [f32; 2]) {
        let Self { ui, view, size, .. } = self;
        let Some(view) = view.as_ref() else {
            return;
        };
        let frame = size.frame(&Frame::new(viewport));
        ui.begin_transparent(viewport);
        // A light shade only: the match still shows round the window.
        let _ = ui.draw_list_mut().push(DrawCommand::SolidRect {
            rect: Rect::new(0.0, 0.0, viewport[0], viewport[1]),
            color: color::alpha(color::SPACE, 0.38),
        });
        if view.trade.is_open() {
            draw_open(ui, &frame, view);
        } else {
            draw_small(ui, &frame, view);
        }
        ui.finish(OTHER_TOKEN);
        target.append(&self.ui, viewport);
    }
}

/// What the gold button does.
#[derive(Clone, Debug, PartialEq)]
enum Do {
    Step(TradeStep),
    /// Close the window (an ended trade).
    Close,
    /// Ask once more before a gift is ready.
    AskGift,
    /// Leave the gift's question.
    BackFromGift,
    /// Nothing (a waiting button).
    Nothing,
}

/// The gold button (label, action, enabled) and the outlined one (label, action) for
/// `view`.
type Main = Option<(String, Do, bool)>;
type Other = Option<(String, Do)>;

fn buttons(view: &View) -> (Main, Other) {
    let trade = &view.trade;
    let rev = trade.rev;
    if trade.is_asked() {
        return if trade.asker {
            (None, None)
        } else {
            (
                Some(("Accept".to_owned(), Do::Step(TradeStep::Accept), true)),
                Some(("Decline".to_owned(), Do::Step(TradeStep::Decline))),
            )
        };
    }
    if !trade.is_open() {
        return (Some(("Close".to_owned(), Do::Close, true)), None);
    }
    let empty = view.mine.is_empty() && view.theirs.is_empty();
    let ready = Do::Step(TradeStep::Ready { rev, ready: true });
    let not_ready = Some((
        "Not ready".to_owned(),
        Do::Step(TradeStep::Ready { rev, ready: false }),
    ));
    match (trade.mine.ready, trade.theirs.ready, trade.mine.confirmed) {
        (false, _, _) if view.confirming_gift => (
            Some(("Yes, give it".to_owned(), ready, true)),
            Some(("Go back".to_owned(), Do::BackFromGift)),
        ),
        (false, _, _) if view.gives_gift() => (Some(("Ready".to_owned(), Do::AskGift, true)), None),
        (false, _, _) => (Some(("Ready".to_owned(), ready, !empty)), None),
        (true, false, _) => (
            Some(("Waiting for them".to_owned(), Do::Nothing, false)),
            not_ready,
        ),
        (true, true, false) => {
            let label = match view.confirm_in {
                Some(seconds) if seconds > 0 => format!("Confirm in {seconds}"),
                _ => "Confirm trade".to_owned(),
            };
            (
                Some((
                    label,
                    Do::Step(TradeStep::Confirm { rev }),
                    view.confirm_in.unwrap_or(0) == 0,
                )),
                not_ready,
            )
        }
        (true, true, true) => (Some(("Confirmed".to_owned(), Do::Nothing, false)), None),
    }
}

/// A countdown of `seconds` for a tile: "6d 23h", "5h 12m", "12m", "<1m".
pub(crate) fn countdown(seconds: i64) -> String {
    let minutes = (seconds.max(0) + 59) / 60;
    let (days, hours, minutes) = (minutes / 1_440, minutes / 60 % 24, minutes % 60);
    match (days, hours, minutes) {
        (0, 0, 0) => "<1m".to_owned(),
        (0, 0, m) => format!("{m}m"),
        (0, h, m) => format!("{h}h {m:02}m"),
        (d, h, _) => format!("{d}d {h}h"),
    }
}

/// How `trade` ended, in words; a refusal's reason goes by the JoF names
/// ([`crate::terms`]), the lines with the partner's name stay as they are.
fn ending(trade: &PlayerTrade) -> String {
    let words = trade.ending();
    if trade.state == "refused" {
        crate::terms::say_owned(words)
    } else {
        words
    }
}

/// `seconds` as a clock: "4:05", or "24s" under a minute.
pub(crate) fn clock(seconds: u64) -> String {
    if seconds >= 60 {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

/// The request or the ended trade, in a card of its own.
fn draw_small(ui: &mut MenuCanvas, frame: &Frame, view: &View) {
    let s = frame.s;
    let trade = &view.trade;
    let name = &view.partner;
    // The headline wears their colours; the running text their plain name.
    let shown = &view.partner_shown;
    let (kicker, headline, words) = if trade.is_asked() && trade.asker {
        (
            "Trade request sent".to_owned(),
            format!("Waiting for {shown}"),
            format!(
                "{name} sees your request at the top of their screen and answers it from their \
                 menu. Once they accept, you both choose what to give."
            ),
        )
    } else if trade.is_asked() {
        (
            "Trade request".to_owned(),
            format!("{shown} wants to trade"),
            format!(
                "{} {name}. Nothing changes hands until you both press Ready and then Confirm.",
                crate::terms::say("Swap unopened holocrons and duplicate saber shaders with")
            ),
        )
    } else {
        (
            "Trade".to_owned(),
            if trade.is_done() {
                format!("Traded with {shown}")
            } else {
                format!("Trade with {shown} ended")
            },
            ending(trade),
        )
    };
    let lines: Vec<&str> = wrap(&words, LINE_CHARS).take(4).collect();
    let body_top = SMALL_Y + 128.0;
    let bar_y = body_top + lines.len() as f32 * 28.0 + 18.0;
    let buttons_y = bar_y + if trade.is_asked() { 30.0 } else { 6.0 };
    let problem_room = if view.problem.is_empty() { 0.0 } else { 30.0 };
    let height = buttons_y + 46.0 + problem_room + 64.0 - SMALL_Y;
    kit::card(ui, frame, [SMALL_X, SMALL_Y, SMALL_W, height]);
    accent(ui, frame, [SMALL_X, SMALL_Y, SMALL_W]);
    let x = SMALL_X + PAD;
    let width = SMALL_W - PAD * 2.0;
    // The SJK logo in its glow, the kicker and headline beside it.
    logo(ui, frame, [x + 30.0, SMALL_Y + 70.0]);
    let head_x = x + 80.0;
    text(
        ui,
        TextFamily::Display,
        format_args!("{kicker}"),
        frame.rect(head_x, SMALL_Y + 30.0, width - 80.0, 30.0),
        22.0 * s,
        color::GOLD_BRIGHT,
        FontWeight::Semibold,
        TextAlign::Start,
    );
    text(
        ui,
        TextFamily::Display,
        format_args!("{headline}"),
        frame.rect(head_x, SMALL_Y + 64.0, width - 200.0, 48.0),
        36.0 * s,
        color::TEXT,
        FontWeight::Semibold,
        TextAlign::Start,
    );
    if trade.is_asked() {
        text(
            ui,
            TextFamily::Display,
            format_args!("{}", clock(view.seconds_left)),
            frame.rect(x, SMALL_Y + 66.0, width, 44.0),
            32.0 * s,
            color::GOLD_BRIGHT,
            FontWeight::Semibold,
            TextAlign::End,
        );
    }
    for (index, line) in lines.iter().enumerate() {
        text(
            ui,
            TextFamily::Body,
            format_args!("{line}"),
            frame.rect(x, body_top + index as f32 * 28.0, width, 28.0),
            18.0 * s,
            color::MUTED,
            FontWeight::Regular,
            TextAlign::Start,
        );
    }
    if trade.is_asked() {
        let share = if view.ask_secs == 0 {
            0.0
        } else {
            (view.left_exact / view.ask_secs as f32).clamp(0.0, 1.0)
        };
        let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
            rect: frame.rect(x, bar_y, width, 4.0),
            radius: 2.0 * s,
            color: color::alpha(color::HOLO, 0.16),
        });
        let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
            rect: frame.rect(x, bar_y, width * share, 4.0),
            radius: 2.0 * s,
            color: color::GOLD,
        });
    }
    let right = SMALL_X + SMALL_W - PAD;
    let (main, other) = buttons(view);
    let mut left = right;
    if let Some((label, _, enabled)) = &main {
        left -= 220.0;
        kit::button(
            ui,
            frame,
            [left, buttons_y, 220.0, 46.0],
            label,
            true,
            *enabled,
            false,
            MAIN_TOKEN,
        );
    }
    if let Some((label, _)) = &other {
        left -= 14.0 + 180.0;
        kit::button(
            ui,
            frame,
            [left, buttons_y, 180.0, 46.0],
            label,
            false,
            true,
            false,
            OTHER_TOKEN,
        );
    }
    if trade.is_asked() && trade.asker {
        kit::button(
            ui,
            frame,
            [right - 220.0, buttons_y, 220.0, 46.0],
            "Cancel request",
            false,
            true,
            false,
            CANCEL_TOKEN,
        );
    }
    let mut foot = buttons_y + 46.0 + 20.0;
    if !view.problem.is_empty() {
        line(ui, frame, x, foot, width, &view.problem, color::EMBER);
        foot += problem_room;
    }
    hints(
        ui,
        frame,
        right,
        foot + 6.0,
        trade.is_asked() && !trade.asker,
    );
}

/// The open trade: both sides, the player's own items, today's limits, the buttons.
fn draw_open(ui: &mut MenuCanvas, frame: &Frame, view: &View) {
    let s = frame.s;
    let trade = &view.trade;
    kit::card(ui, frame, [WIN_X, WIN_Y, WIN_W, WIN_H]);
    accent(ui, frame, [WIN_X, WIN_Y, WIN_W]);
    let inner = WIN_W - PAD * 2.0;
    header(ui, frame, view, "Trading with");
    text(
        ui,
        TextFamily::Body,
        format_args!("Closes in {} without a change", clock(view.seconds_left)),
        frame.rect(LEFT_X, WIN_Y + 66.0, inner, 24.0),
        15.0 * s,
        color::QUIET,
        FontWeight::Regular,
        TextAlign::End,
    );
    // The two sides, each with its worth and whether it is ready.
    let sides_y = WIN_Y + 128.0;
    side(
        ui,
        frame,
        LEFT_X,
        sides_y,
        "You give",
        &view.mine,
        trade.mine.value,
        (trade.mine.ready, trade.mine.confirmed),
        view.items_max,
        Some(OFFERED_FIRST),
    );
    side(
        ui,
        frame,
        RIGHT_X,
        sides_y,
        &format!("{} gives", view.partner_shown),
        &view.theirs,
        trade.theirs.value,
        (trade.theirs.ready, trade.theirs.confirmed),
        view.items_max,
        None,
    );
    // The arrows between the sides.
    swap_badge(
        ui,
        frame,
        [WIN_X + WIN_W * 0.5, sides_y + 38.0 + TILE * 0.5],
    );
    rule(ui, frame, sides_y + 196.0);
    let own_y = sides_y + 216.0;
    let buttons_y = WIN_Y + WIN_H - 112.0;
    if view.confirming_gift {
        gift_question(ui, frame, view, own_y, buttons_y - 24.0);
    } else {
        own_items(ui, frame, view, own_y);
        daily_limits(ui, frame, view, own_y);
    }
    // What to know before Ready: the hub's refusal, a gift, or an uneven trade.
    let say_y = buttons_y - 40.0;
    if !view.problem.is_empty() {
        line(ui, frame, LEFT_X, say_y, inner, &view.problem, color::EMBER);
    } else if view.receives_gift() {
        line(
            ui,
            frame,
            LEFT_X,
            say_y,
            inner,
            &format!(
                "{} is giving this as a gift: nothing is asked in return.",
                view.partner
            ),
            color::TRUSTED,
        );
    } else if trade.uneven && !view.gives_gift() && !view.confirming_gift {
        line(
            ui,
            frame,
            LEFT_X,
            say_y,
            inner,
            "Uneven: one side is worth more than twice the other. Check before you confirm.",
            color::GOLD_BRIGHT,
        );
    }
    kit::button(
        ui,
        frame,
        [LEFT_X, buttons_y, 200.0, 50.0],
        "Cancel trade",
        false,
        true,
        false,
        CANCEL_TOKEN,
    );
    let right = WIN_X + WIN_W - PAD;
    let (main, other) = buttons(view);
    if let Some((label, _, enabled)) = &main {
        kit::button(
            ui,
            frame,
            [right - 260.0, buttons_y, 260.0, 50.0],
            label,
            true,
            *enabled,
            false,
            MAIN_TOKEN,
        );
    }
    if let Some((label, _)) = &other {
        kit::button(
            ui,
            frame,
            [right - 260.0 - 14.0 - 190.0, buttons_y, 190.0, 50.0],
            label,
            false,
            true,
            false,
            OTHER_TOKEN,
        );
    }
    hints(ui, frame, right, buttons_y + 68.0, true);
}

/// The player's own items to put in, under the sides: a click puts one in. Those the
/// day's limits hold back are dimmed, with why under them.
fn own_items(ui: &mut MenuCanvas, frame: &Frame, view: &View, own_y: f32) {
    let s = frame.s;
    text(
        ui,
        TextFamily::Display,
        format_args!(
            "{}",
            crate::terms::say(if view.holocrons {
                "Your holocrons and duplicates"
            } else {
                "Your duplicates"
            })
        ),
        frame.rect(LEFT_X, own_y, COLUMN_W, 26.0),
        19.0 * s,
        color::TEXT,
        FontWeight::Semibold,
        TextAlign::Start,
    );
    if view.marks_kept {
        favourites_chip(
            ui,
            frame,
            view.favourites_only,
            [LEFT_X + COLUMN_W, own_y - 2.0],
        );
    }
    let tiles_y = own_y + 36.0;
    if view.own.is_empty() {
        let words = "Nothing to offer yet: unopened holocrons you found by playing, and \
                     second copies of saber shaders, can be traded.";
        for (index, part) in wrap(&crate::terms::say(words), 52).enumerate() {
            line(
                ui,
                frame,
                LEFT_X,
                tiles_y + index as f32 * 24.0,
                COLUMN_W,
                part,
                color::QUIET,
            );
        }
    }
    let full = view.mine.len() >= view.items_max;
    // The Favourites chip shows the favourites alone; each tile keeps its own token.
    let listed = || {
        view.own
            .iter()
            .enumerate()
            .filter(|(_, item)| !view.favourites_only || item.favourite)
    };
    let count = listed().count();
    let shown = count.min(TILES_ROW * OWN_ROWS);
    for (slot, (index, item)) in listed().take(shown).enumerate() {
        let (column, row) = (slot % TILES_ROW, slot / TILES_ROW);
        let x = LEFT_X + column as f32 * (TILE + TILE_GAP);
        let y = tiles_y + row as f32 * (TILE + TILE_GAP);
        let live = !full && view.room.takes(item) && !item.locked;
        tile(
            ui,
            frame,
            [x, y],
            item,
            live,
            Some(OWN_FIRST + index as u16),
        );
    }
    if count == 0 && !view.own.is_empty() {
        line(
            ui,
            frame,
            LEFT_X,
            tiles_y,
            COLUMN_W,
            "No favourite to offer: star shaders in the Collection.",
            color::QUIET,
        );
    }
    let rows = shown.div_ceil(TILES_ROW).max(1) as f32;
    let mut under = tiles_y + rows * (TILE + TILE_GAP) + 2.0;
    if count > shown {
        line(
            ui,
            frame,
            LEFT_X,
            under,
            COLUMN_W,
            &format!(
                "{} more not shown: the most worth come first",
                count - shown
            ),
            color::QUIET,
        );
        under += 24.0;
    }
    if listed().any(|(_, item)| item.locked) {
        line(
            ui,
            frame,
            LEFT_X,
            under,
            COLUMN_W,
            "Locked shaders stay out of trades: unlock them in the Collection.",
            color::GOLD_BRIGHT,
        );
        under += 24.0;
    }
    if view.own.iter().any(|item| item.locked_for.is_some()) {
        under += warning(
            ui,
            frame,
            LEFT_X,
            under,
            COLUMN_W,
            &crate::terms::say(LOCK_WARNING),
        ) + 8.0;
    }
    if let Some(note) = view.room.note(&view.own) {
        for (index, part) in wrap(&note, 58).enumerate() {
            line(
                ui,
                frame,
                LEFT_X,
                under + index as f32 * 22.0,
                COLUMN_W,
                part,
                color::GOLD_BRIGHT,
            );
        }
    } else if full && !view.own.is_empty() {
        line(
            ui,
            frame,
            LEFT_X,
            under,
            COLUMN_W,
            &format!("A side holds {} items at most.", view.items_max),
            color::QUIET,
        );
    }
}

/// The day's limits, beside the player's items: a row a kind, received and given, each
/// as done of most (ember once reached), and a line on how they count.
fn daily_limits(ui: &mut MenuCanvas, frame: &Frame, view: &View, own_y: f32) {
    let s = frame.s;
    text(
        ui,
        TextFamily::Display,
        format_args!("Daily limits"),
        frame.rect(RIGHT_X, own_y, COLUMN_W, 26.0),
        19.0 * s,
        color::TEXT,
        FontWeight::Semibold,
        TextAlign::Start,
    );
    let given_right = RIGHT_X + COLUMN_W;
    let received_right = given_right - 150.0;
    let head_y = own_y + 38.0;
    for (title, right) in [("Received", received_right), ("Given", given_right)] {
        text(
            ui,
            TextFamily::Body,
            format_args!("{title}"),
            frame.rect(right - 140.0, head_y, 140.0, 20.0),
            13.5 * s,
            color::QUIET,
            FontWeight::Semibold,
            TextAlign::End,
        );
    }
    let _ = ui.draw_list_mut().push(DrawCommand::SolidRect {
        rect: frame.rect(RIGHT_X, head_y + 24.0, COLUMN_W, 1.0),
        color: color::alpha(color::HOLO, 0.12),
    });
    let mut y = head_y + 34.0;
    for limit in &view.limits {
        let ink = limit.colour.unwrap_or(color::MUTED);
        line(
            ui,
            frame,
            RIGHT_X,
            y,
            COLUMN_W,
            &crate::terms::say(limit.label),
            ink,
        );
        for ((done, most), right) in [(limit.received, received_right), (limit.given, given_right)]
        {
            text(
                ui,
                TextFamily::Body,
                format_args!("{done} / {most}"),
                frame.rect(right - 140.0, y, 140.0, 24.0),
                16.0 * s,
                if most > 0 && done >= most {
                    color::EMBER
                } else {
                    color::TEXT
                },
                FontWeight::Semibold,
                TextAlign::End,
            );
        }
        y += 30.0;
    }
    // The trading day: 24 hours from the player's first trade, then every count resets.
    let Some(left) = view.resets_in else {
        return;
    };
    let _ = ui.draw_list_mut().push(DrawCommand::SolidRect {
        rect: frame.rect(RIGHT_X, y + 2.0, COLUMN_W, 1.0),
        color: color::alpha(color::HOLO, 0.12),
    });
    y += 14.0;
    line(
        ui,
        frame,
        RIGHT_X,
        y,
        COLUMN_W,
        "Daily reset in",
        color::MUTED,
    );
    text(
        ui,
        TextFamily::Body,
        format_args!("{}", hms(left)),
        frame.rect(RIGHT_X, y, COLUMN_W, 24.0),
        16.0 * s,
        color::GOLD_BRIGHT,
        FontWeight::Semibold,
        TextAlign::End,
    );
}

/// `seconds` as hours, minutes and seconds: "23:59:59", "04:05:09".
pub(crate) fn hms(seconds: u64) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3_600,
        seconds / 60 % 60,
        seconds % 60
    )
}

/// The gift's question, in place of the player's items and the limits: what they give,
/// that nothing comes back and that it cannot be undone. The buttons under it answer.
fn gift_question(ui: &mut MenuCanvas, frame: &Frame, view: &View, top: f32, bottom: f32) {
    let s = frame.s;
    let width = WIN_W - PAD * 2.0;
    // A compact card, centred in the room the items and limits leave.
    let height = 180.0_f32.min(bottom - top);
    let top = top + ((bottom - top) - height) * 0.5;
    let panel = frame.rect(LEFT_X, top, width, height);
    let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
        rect: panel,
        radius: 14.0 * s,
        color: color::alpha(color::GOLD, 0.07),
    });
    let _ = ui.draw_list_mut().push(DrawCommand::Border {
        rect: panel,
        radius: 14.0 * s,
        width: 1.5 * s,
        color: color::alpha(color::GOLD, 0.7),
    });
    // A gold gift mark: a ring with an exclamation, plain ASCII.
    let [cx, cy] = [LEFT_X + 60.0, top + height * 0.5];
    let ring = frame.rect(cx - 28.0, cy - 28.0, 56.0, 56.0);
    let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
        rect: ring,
        radius: 28.0 * s,
        color: color::alpha(color::GOLD, 0.16),
    });
    let _ = ui.draw_list_mut().push(DrawCommand::Border {
        rect: ring,
        radius: 28.0 * s,
        width: 2.0 * s,
        color: color::GOLD,
    });
    text(
        ui,
        TextFamily::Display,
        format_args!("!"),
        frame.rect(cx - 28.0, cy - 18.0, 56.0, 36.0),
        30.0 * s,
        color::GOLD_BRIGHT,
        FontWeight::Semibold,
        TextAlign::Center,
    );
    let x = LEFT_X + 110.0;
    let words_width = width - 140.0;
    text(
        ui,
        TextFamily::Display,
        format_args!("Give it as a gift?"),
        frame.rect(x, top + 34.0, words_width, 40.0),
        30.0 * s,
        color::TEXT,
        FontWeight::Semibold,
        TextAlign::Start,
    );
    let names: Vec<String> = view
        .mine
        .iter()
        .map(|item| match item.holocron_slot {
            Some(_) => crate::terms::say_owned(format!("a {} Holocron", item.tier)),
            None => format!("a {} blade", item.name),
        })
        .collect();
    let gives = match names.len() {
        0 => String::new(),
        1 => names[0].clone(),
        count => format!("{} and {}", names[..count - 1].join(", "), names[count - 1]),
    };
    let words = format!(
        "You give {gives} and get nothing back from {name}. Once you both confirm it is \
         theirs to keep: a gift cannot be undone.",
        name = view.partner
    );
    for (index, part) in wrap(&words, 92).take(4).enumerate() {
        text(
            ui,
            TextFamily::Body,
            format_args!("{part}"),
            frame.rect(x, top + 86.0 + index as f32 * 28.0, words_width, 28.0),
            18.0 * s,
            color::MUTED,
            FontWeight::Regular,
            TextAlign::Start,
        );
    }
}

/// One side: its title and worth, its slots, and how far it is.
#[allow(clippy::too_many_arguments)]
fn side(
    ui: &mut MenuCanvas,
    frame: &Frame,
    x: f32,
    y: f32,
    title: &str,
    items: &[Item],
    value: i64,
    (ready, confirmed): (bool, bool),
    most: usize,
    tokens: Option<u16>,
) {
    let s = frame.s;
    text(
        ui,
        TextFamily::Display,
        format_args!("{title}"),
        frame.rect(x, y, COLUMN_W - 150.0, 28.0),
        21.0 * s,
        color::TEXT,
        FontWeight::Semibold,
        TextAlign::Start,
    );
    text(
        ui,
        TextFamily::Body,
        format_args!("worth about {value} shards"),
        frame.rect(x, y + 2.0, COLUMN_W, 24.0),
        15.0 * s,
        color::MUTED,
        FontWeight::Regular,
        TextAlign::End,
    );
    let slots_y = y + 38.0;
    for index in 0..most.clamp(1, TILES_ROW) {
        let at = [x + index as f32 * (TILE + TILE_GAP), slots_y];
        match items.get(index) {
            Some(item) => tile(
                ui,
                frame,
                at,
                item,
                tokens.is_some(),
                tokens.map(|first| first + index as u16),
            ),
            None => {
                let _ = ui.draw_list_mut().push(DrawCommand::Border {
                    rect: frame.rect(at[0], at[1], TILE, TILE),
                    radius: 10.0 * s,
                    width: 1.0 * s,
                    color: color::alpha(color::HOLO, 0.16),
                });
            }
        }
    }
    let (label, colour) = match (ready, confirmed) {
        (_, true) => ("Confirmed", color::TRUSTED),
        (true, false) => ("Ready", color::TRUSTED),
        (false, false) => ("Choosing...", color::QUIET),
    };
    let chip = frame.rect(x, slots_y + TILE + 14.0, 130.0, 28.0);
    let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
        rect: chip,
        radius: 14.0 * s,
        color: color::alpha(colour, 0.14),
    });
    let _ = ui.draw_list_mut().push(DrawCommand::Border {
        rect: chip,
        radius: 14.0 * s,
        width: 1.0 * s,
        color: color::alpha(colour, 0.7),
    });
    text(
        ui,
        TextFamily::Body,
        format_args!("{label}"),
        frame.rect(x, slots_y + TILE + 17.0, 130.0, 22.0),
        14.0 * s,
        colour,
        FontWeight::Semibold,
        TextAlign::Center,
    );
}

/// One item's tile at `at` (frame pixels): its picture (the holocron's, or the blade in
/// its tier's colour), its name and tier, ringed in the tier's colour. `live` tiles
/// answer to `token`.
fn tile(
    ui: &mut MenuCanvas,
    frame: &Frame,
    at: [f32; 2],
    item: &Item,
    live: bool,
    token: Option<u16>,
) {
    let s = frame.s;
    let [x, y] = at;
    let rect = frame.rect(x, y, TILE, TILE);
    let lit = live && token.is_some_and(|token| ui.token_hovered(token));
    let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
        rect,
        radius: 10.0 * s,
        color: if lit {
            color::alpha(item.colour, 0.22)
        } else {
            Color::new(0.06, 0.09, 0.17, 0.95)
        },
    });
    let _ = ui.draw_list_mut().push(DrawCommand::Border {
        rect,
        radius: 10.0 * s,
        width: if lit { 2.0 * s } else { 1.0 * s },
        color: color::alpha(item.colour, if live { 0.85 } else { 0.45 }),
    });
    let (cx, cy) = (x + TILE * 0.5, y + 30.0);
    match item.holocron_slot {
        Some(slot) if crate::holocrons::icons::is_loaded(slot) => {
            let _ = ui.draw_list_mut().push(DrawCommand::TexturedQuad {
                rect: frame.rect(cx - 24.0, cy - 24.0, 48.0, 48.0),
                texture: crate::holocrons::icons::texture(slot),
                color: Color::new(1.0, 1.0, 1.0, 1.0),
            });
        }
        Some(_) => {
            let [gx, gy] = frame.point(cx, cy);
            crate::holocrons::gem::draw(
                ui.draw_list_mut(),
                [gx, gy],
                20.0 * s,
                item.colour,
                1.0,
                crate::holocrons::gem::ROWS,
            );
        }
        None => {
            // A blade: a bright core in a glow of the tier's colour.
            let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
                rect: frame.rect(cx - 6.0, cy - 22.0, 12.0, 44.0),
                radius: 6.0 * s,
                color: color::alpha(item.colour, 0.45),
            });
            let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
                rect: frame.rect(cx - 2.5, cy - 20.0, 5.0, 40.0),
                radius: 2.5 * s,
                color: Color::new(1.0, 1.0, 1.0, 0.95),
            });
        }
    }
    if let Some(left) = item.locked_for {
        // Waiting its week: a gold timer pill where the name goes.
        let pill = frame.rect(x + 6.0, y + TILE - 23.0, TILE - 12.0, 18.0);
        let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
            rect: pill,
            radius: 9.0 * s,
            color: color::alpha(color::GOLD, 0.18),
        });
        let _ = ui.draw_list_mut().push(DrawCommand::Border {
            rect: pill,
            radius: 9.0 * s,
            width: 1.0 * s,
            color: color::alpha(color::GOLD, 0.7),
        });
        text(
            ui,
            TextFamily::Body,
            format_args!("{}", countdown(left)),
            frame.rect(x + 6.0, y + TILE - 22.0, TILE - 12.0, 16.0),
            11.5 * s,
            color::GOLD_BRIGHT,
            FontWeight::Semibold,
            TextAlign::Center,
        );
        return;
    }
    item_marks(ui, frame, [x, y], item);
    let label = match item.holocron_slot {
        Some(_) => item.tier.clone(),
        None => item.name.clone(),
    };
    text(
        ui,
        TextFamily::Body,
        format_args!("{label}"),
        frame.rect(x + 3.0, y + TILE - 22.0, TILE - 6.0, 18.0),
        12.0 * s,
        color::TEXT,
        FontWeight::Semibold,
        TextAlign::Center,
    );
    if let Some(token) = token.filter(|_| live) {
        ui.hit_region(token, rect);
    }
}

/// A shader tile's collection marks at its top (29/10/2026), drawn from shapes and plain
/// ASCII (the window's font draws bytes): the star of a favourite and the padlock of a
/// locked one at the left, "Worn" at the right, the New sticker over the top edge.
fn item_marks(ui: &mut MenuCanvas, frame: &Frame, [x, y]: [f32; 2], item: &Item) {
    let s = frame.s;
    let mut left = x + 11.0;
    if item.favourite {
        star(ui, frame, [left, y + 11.0], 6.5, color::GOLD_BRIGHT);
        left += 15.0;
    }
    if item.locked {
        padlock(ui, frame, [left, y + 11.0], color::GOLD_BRIGHT);
    }
    if item.worn {
        text(
            ui,
            TextFamily::Body,
            format_args!("Worn"),
            frame.rect(x + 4.0, y + 4.0, TILE - 9.0, 14.0),
            10.5 * s,
            color::GOLD_BRIGHT,
            FontWeight::Semibold,
            TextAlign::End,
        );
    }
    if item.new {
        let rect = frame.rect(x + TILE * 0.5 - 19.0, y - 8.0, 38.0, 15.0);
        let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
            rect,
            radius: 7.5 * s,
            color: color::GOLD_BRIGHT,
        });
        text(
            ui,
            TextFamily::Display,
            format_args!("New"),
            frame.rect(x + TILE * 0.5 - 19.0, y - 7.0, 38.0, 13.0),
            10.5 * s,
            Color::new(0.078, 0.063, 0.02, 1.0),
            FontWeight::Semibold,
            TextAlign::Center,
        );
    }
}

/// A five-pointed star centred on `[cx, cy]` (frame pixels), its points `radius` out: a
/// pentagon and a triangle a point, as the renderer fills convex shapes.
fn star(ui: &mut MenuCanvas, frame: &Frame, [cx, cy]: [f32; 2], radius: f32, colour: Color) {
    let inner = radius * 0.42;
    let at = |r: f32, step: usize| {
        let angle = -std::f32::consts::FRAC_PI_2 + step as f32 * std::f32::consts::PI / 5.0;
        frame.point(cx + r * angle.cos(), cy + r * angle.sin())
    };
    let mut push = |points: &[[f32; 2]]| {
        let mut corners = [[0.0; 2]; sjk_ui::POLYGON_POINTS];
        for (corner, point) in corners.iter_mut().zip(points) {
            *corner = *point;
        }
        let _ = ui.draw_list_mut().push(DrawCommand::Polygon {
            points: corners,
            count: points.len() as u8,
            color: colour,
            texture: None,
        });
    };
    let middle: [[f32; 2]; 5] = std::array::from_fn(|n| at(inner, n * 2 + 1));
    push(&middle);
    for n in 0..5 {
        push(&[
            at(inner, (n * 2 + 9) % 10),
            at(radius, n * 2),
            at(inner, n * 2 + 1),
        ]);
    }
}

/// A small padlock whose body is centred on `[cx, cy]` (frame pixels): a rounded body and
/// its shackle over it.
fn padlock(ui: &mut MenuCanvas, frame: &Frame, [cx, cy]: [f32; 2], colour: Color) {
    let s = frame.s;
    let _ = ui.draw_list_mut().push(DrawCommand::Border {
        rect: frame.rect(cx - 3.5, cy - 7.5, 7.0, 9.0),
        radius: 3.5 * s,
        width: 1.6 * s,
        color: colour,
    });
    let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
        rect: frame.rect(cx - 5.5, cy - 2.0, 11.0, 8.0),
        radius: 1.8 * s,
        color: colour,
    });
}

/// The Favourites chip, its right edge at `right` (frame pixels, from `y`): its star and
/// word, lit gold while `on`.
fn favourites_chip(ui: &mut MenuCanvas, frame: &Frame, on: bool, [right, y]: [f32; 2]) {
    let s = frame.s;
    let words = "Favourites";
    let width = crate::text::display_width(words, 15.0) + 50.0;
    let rect = frame.rect(right - width, y, width, 28.0);
    let hovered = ui.token_hovered(FAVOURITES_TOKEN);
    if on || hovered {
        let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
            rect,
            radius: 14.0 * s,
            color: color::alpha(color::GOLD, if on { 0.22 } else { 0.09 }),
        });
    }
    let _ = ui.draw_list_mut().push(DrawCommand::Border {
        rect,
        radius: 14.0 * s,
        width: 1.2 * s,
        color: color::alpha(color::GOLD, if on { 1.0 } else { 0.45 }),
    });
    let ink = if on { color::GOLD_BRIGHT } else { color::MUTED };
    star(ui, frame, [right - width + 17.0, y + 14.0], 6.5, ink);
    text(
        ui,
        TextFamily::Display,
        format_args!("{words}"),
        frame.rect(right - width + 28.0, y + 4.0, width - 34.0, 20.0),
        15.0 * s,
        ink,
        FontWeight::Semibold,
        TextAlign::Center,
    );
    ui.hit_region(FAVOURITES_TOKEN, rect);
}

/// The badge between the two sides: a gold ring with one chevron each way, drawn
/// with plain ASCII so every font has it.
fn swap_badge(ui: &mut MenuCanvas, frame: &Frame, [cx, cy]: [f32; 2]) {
    let s = frame.s;
    let radius = 21.0;
    let ring = frame.rect(cx - radius, cy - radius, radius * 2.0, radius * 2.0);
    let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
        rect: ring,
        radius: radius * s,
        color: color::alpha(color::GOLD, 0.12),
    });
    let _ = ui.draw_list_mut().push(DrawCommand::Border {
        rect: ring,
        radius: radius * s,
        width: 1.5 * s,
        color: color::alpha(color::GOLD, 0.8),
    });
    for (dy, chevron) in [(-15.0, ">"), (-1.0, "<")] {
        text(
            ui,
            TextFamily::Display,
            format_args!("{chevron}"),
            frame.rect(cx - radius, cy + dy, radius * 2.0, 16.0),
            17.0 * s,
            color::GOLD_BRIGHT,
            FontWeight::Semibold,
            TextAlign::Center,
        );
    }
}

/// The SJK logo centred on `at` (frame pixels), on its own: no halo, so only the
/// logo shows (Creyon 23/10/2026).
fn logo(ui: &mut MenuCanvas, frame: &Frame, [lx, ly]: [f32; 2]) {
    const SIZE: f32 = 64.0;
    let _ = ui.draw_list_mut().push(DrawCommand::TexturedQuad {
        rect: frame.rect(lx - SIZE * 0.5, ly - SIZE * 0.5, SIZE, SIZE),
        texture: crate::ui_renderer::LOGO_TEXTURE,
        color: Color::new(1.0, 1.0, 1.0, 1.0),
    });
}

/// The card's head: the SJK logo in its gold glow, the kicker (`kicker`, gold) and the
/// other player's picture and name as they wear it, colours and all.
fn header(ui: &mut MenuCanvas, frame: &Frame, view: &View, kicker: &str) {
    let s = frame.s;
    logo(ui, frame, [LEFT_X + 30.0, WIN_Y + 66.0]);
    let x = LEFT_X + 80.0;
    text(
        ui,
        TextFamily::Display,
        format_args!("{kicker}"),
        frame.rect(x, WIN_Y + 26.0, 400.0, 26.0),
        19.0 * s,
        color::GOLD_BRIGHT,
        FontWeight::Semibold,
        TextAlign::Start,
    );
    // Their picture (or their initial), then their name.
    let radius = 19.0;
    crate::profile_card::avatar(
        ui,
        frame.point(x + radius, WIN_Y + 76.0),
        radius * s,
        &crate::profile_card::Avatar {
            key_id: &view.partner_key,
            version: &view.partner_avatar,
            name: &view.partner_shown,
            verified: view.trade.with.verified,
            tick: sjk_identity::Tick::None,
            trust: None,
            preview: false,
            lit: false,
        },
    );
    text(
        ui,
        TextFamily::Display,
        format_args!("{}", view.partner_shown),
        frame.rect(x + radius * 2.0 + 14.0, WIN_Y + 56.0, 620.0, 42.0),
        32.0 * s,
        color::TEXT,
        FontWeight::Semibold,
        TextAlign::Start,
    );
}

/// The gold line along a card's top.
fn accent(ui: &mut MenuCanvas, frame: &Frame, [x, y, width]: [f32; 3]) {
    let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
        rect: frame.rect(x + 24.0, y, width - 48.0, 3.0),
        radius: 1.5 * frame.s,
        color: color::alpha(color::GOLD, 0.8),
    });
}

/// A hairline across the open trade's card at `y`.
fn rule(ui: &mut MenuCanvas, frame: &Frame, y: f32) {
    let _ = ui.draw_list_mut().push(DrawCommand::SolidRect {
        rect: frame.rect(LEFT_X, y, WIN_W - PAD * 2.0, 1.0),
        color: color::alpha(color::HOLO, 0.14),
    });
}

/// A line of body text.
fn line(ui: &mut MenuCanvas, frame: &Frame, x: f32, y: f32, width: f32, words: &str, ink: Color) {
    text(
        ui,
        TextFamily::Body,
        format_args!("{words}"),
        frame.rect(x, y, width, 24.0),
        16.0 * frame.s,
        ink,
        FontWeight::Regular,
        TextAlign::Start,
    );
}

/// A warning box at `x`, `y` (frame pixels), `width` wide: a gold "!" and `words`
/// wrapped beside it, on a soft gold card. Its height back.
fn warning(ui: &mut MenuCanvas, frame: &Frame, x: f32, y: f32, width: f32, words: &str) -> f32 {
    let s = frame.s;
    let parts: Vec<&str> = wrap(words, 66).collect();
    let height = 14.0 + parts.len() as f32 * 22.0;
    let card = frame.rect(x, y, width, height);
    let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
        rect: card,
        radius: 8.0 * s,
        color: color::alpha(color::GOLD, 0.10),
    });
    let _ = ui.draw_list_mut().push(DrawCommand::Border {
        rect: card,
        radius: 8.0 * s,
        width: 1.0 * s,
        color: color::alpha(color::GOLD, 0.55),
    });
    let ring = frame.rect(x + 10.0, y + height * 0.5 - 11.0, 22.0, 22.0);
    let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
        rect: ring,
        radius: 11.0 * s,
        color: color::alpha(color::GOLD, 0.9),
    });
    text(
        ui,
        TextFamily::Display,
        format_args!("!"),
        frame.rect(x + 10.0, y + height * 0.5 - 10.0, 22.0, 20.0),
        15.0 * s,
        Color::new(0.06, 0.09, 0.17, 1.0),
        FontWeight::Semibold,
        TextAlign::Center,
    );
    for (index, part) in parts.iter().enumerate() {
        line(
            ui,
            frame,
            x + 42.0,
            y + 7.0 + index as f32 * 22.0,
            width - 50.0,
            part,
            color::GOLD_BRIGHT,
        );
    }
    height
}

/// The keys, right-aligned to `right` at `y`: Enter for the gold button when it does
/// something, Esc to close.
fn hints(ui: &mut MenuCanvas, frame: &Frame, right: f32, y: f32, enter: bool) {
    let s = frame.s;
    let [right, y] = frame.point(right, y);
    let close = key_hint_width(&["Esc"], "close, the trade stays", s);
    if enter {
        let main = key_hint_width(&["Enter"], "main button", s);
        let x = right - close - main - 18.0 * s;
        let x = key_hint(ui, &["Enter"], "main button", x, y, s);
        key_hint(ui, &["Esc"], "close, the trade stays", x + 18.0 * s, y, s);
    } else {
        key_hint(ui, &["Esc"], "close, the trade stays", right - close, y, s);
    }
}

/// The toast over play: a trade runs and the window is closed.
pub(crate) struct Toast {
    ui: MenuCanvas,
}

/// The toast's Open button, when the pointer is free (the game menu or the chat's
/// composer open), and the toast itself.
const TOAST_OPEN_TOKEN: u16 = 1;
const TOAST_TOKEN: u16 = 2;
/// The toast's usual width (frame pixels); it grows for longer words, up to
/// [`TOAST_MAX_W`], so its text stays on one line and is never cut.
const TOAST_W: f32 = 660.0;
const TOAST_MAX_W: f32 = 1800.0;
/// The room left of the words (their picture) and right of them.
const TOAST_LEFT: f32 = 92.0;
const TOAST_RIGHT: f32 = 24.0;

/// How wide the toast's words are, in frame pixels: the headline (Rajdhani 24,
/// semibold) or the line (15), whichever is wider.
fn toast_words(
    said: &Said,
    display: &crate::sjk_chat_look::Measure<'_>,
    body: &crate::sjk_chat_look::Measure<'_>,
) -> f32 {
    let headline = display.width(&said.headline, 24.0, crate::text::TextFace::Semibold);
    let line = body.width(&said.line, 15.0, crate::text::TextFace::Regular);
    (headline + toast_beside(said)).max(line)
}

/// The room the clock, the key hint or the Open trade button take right of the headline.
fn toast_beside(said: &Said) -> f32 {
    if said.clickable && !said.key.0.is_empty() {
        190.0
    } else if said.clock.is_some() || !said.key.0.is_empty() {
        70.0
    } else {
        0.0
    }
}

/// The toast's width for `words` frame pixels of text: [`TOAST_W`], or wider so they fit
/// on one line (with a little to spare), up to [`TOAST_MAX_W`].
fn toast_width(words: f32) -> f32 {
    (TOAST_LEFT + words + 8.0 + TOAST_RIGHT).clamp(TOAST_W, TOAST_MAX_W)
}

impl Default for Toast {
    fn default() -> Self {
        Self {
            ui: MenuCanvas::with_capacities(32, 120, 64),
        }
    }
}

/// What the toast says.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Said {
    pub(crate) headline: String,
    pub(crate) line: String,
    /// A request's clock and its whole wait, for the bar.
    pub(crate) clock: Option<(u64, u64)>,
    /// How much of the request's wait is left (1 to 0, smoothly), for its bar.
    pub(crate) share: Option<f32>,
    /// The key that opens the window ("Esc"), and what it does.
    pub(crate) key: (&'static str, &'static str),
    /// The first letter of the other player's name, in its disc.
    pub(crate) initial: char,
    /// The other player's key and picture version (empty for none), for their picture.
    pub(crate) who: (String, String),
    /// The pointer is free: the toast is a button (Open trade) instead of a key hint.
    pub(crate) clickable: bool,
}

impl Said {
    /// What the toast says of `view`.
    pub(crate) fn of(view: &View) -> Self {
        let trade = &view.trade;
        let name = &view.partner_shown;
        let initial = view
            .partner
            .chars()
            .find(|c| c.is_alphanumeric())
            .map_or('?', |c| c.to_ascii_uppercase());
        let (headline, line, clock) = if trade.is_asked() && !trade.asker {
            (
                format!("{name} wants to trade"),
                if view.holocrons {
                    crate::terms::say("Holocrons and duplicate shaders. You keep playing.")
                        .into_owned()
                } else {
                    "Duplicate shaders. You keep playing.".to_owned()
                },
                Some((view.seconds_left, view.ask_secs)),
            )
        } else if trade.is_asked() {
            (
                format!("Asking {name} to trade"),
                "Waiting for their answer.".to_owned(),
                Some((view.seconds_left, view.ask_secs)),
            )
        } else if trade.is_open() {
            (
                format!("Trading with {name}"),
                match (trade.mine.ready, trade.theirs.ready) {
                    (_, true) if !trade.mine.ready => "They are ready: your turn.".to_owned(),
                    (true, true) => "Both ready: confirm to swap.".to_owned(),
                    _ => format!("Closes in {} without a change.", clock(view.seconds_left)),
                },
                None,
            )
        } else {
            (ending(trade), String::new(), None)
        };
        let share = clock.filter(|(_, whole)| *whole > 0).map(|(_, whole)| {
            let exact = if view.left_exact > 0.0 {
                view.left_exact
            } else {
                view.seconds_left as f32
            };
            (exact / whole as f32).clamp(0.0, 1.0)
        });
        Self {
            headline,
            line,
            clock,
            share,
            key: if trade.is_live() {
                (
                    "Esc",
                    if trade.is_asked() && !trade.asker {
                        "answer"
                    } else {
                        "open"
                    },
                )
            } else {
                ("", "")
            },
            initial,
            who: (view.partner_key.clone(), view.partner_avatar.clone()),
            clickable: false,
        }
    }
}

impl Toast {
    /// A pointer event over the toast: true when it was a click on it (open the window).
    pub(crate) fn handle_pointer(&mut self, event: InputEvent) -> bool {
        self.ui.pointer(event).is_some_and(|event| {
            event.kind == UiEventKind::Activate
                && matches!(event.token, Some(TOAST_OPEN_TOKEN | TOAST_TOKEN))
        })
    }

    /// Whether the pointer rests on the toast, so a click there is the toast's.
    pub(crate) fn hovered(&self) -> bool {
        self.ui.token_hovered(TOAST_OPEN_TOKEN) || self.ui.token_hovered(TOAST_TOKEN)
    }

    pub(crate) fn draw_list(&self) -> &sjk_ui::DrawList {
        self.ui.draw_list()
    }

    /// Draw `said` at the top centre of the frame.
    pub(crate) fn append(&mut self, said: &Said, target: TextTarget<'_>, viewport: [f32; 2]) {
        let words = toast_words(said, &target.display_measure(), &target.body_measure());
        self.build(said, viewport, words);
        target.append(&self.ui, viewport);
    }

    /// Lay `said` out at the top centre of the frame, `words` wide (frame pixels: the
    /// headline or the line, whichever is wider, see [`toast_words`]).
    fn build(&mut self, said: &Said, viewport: [f32; 2], words: f32) {
        let frame = Frame::new(viewport);
        let s = frame.s;
        let ui = &mut self.ui;
        ui.begin_transparent(viewport);
        let width = toast_width(words);
        let (x, y) = (960.0 - width * 0.5, 64.0);
        let height = if said.line.is_empty() { 78.0 } else { 104.0 };
        let card = frame.rect(x, y, width, height);
        let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
            rect: frame.rect(x + 2.0, y + 8.0, width, height),
            radius: 16.0 * s,
            color: Color::new(0.0, 0.0, 0.0, 0.4),
        });
        let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
            rect: card,
            radius: 16.0 * s,
            color: color::alpha(color::SPACE, 0.92),
        });
        let _ = ui.draw_list_mut().push(DrawCommand::Border {
            rect: card,
            radius: 16.0 * s,
            width: 1.0 * s,
            color: color::alpha(color::GOLD, 0.65),
        });
        // Their picture (or their initial) in a gold ring: the gold of a trade.
        let [cx, cy] = frame.point(x + 50.0, y + height * 0.5);
        let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
            rect: Rect::new(cx - 27.0 * s, cy - 27.0 * s, 54.0 * s, 54.0 * s),
            radius: 27.0 * s,
            color: color::alpha(color::GOLD, 0.85),
        });
        let (key_id, version) = &said.who;
        let initial = said.initial.to_string();
        crate::profile_card::avatar(
            ui,
            [cx, cy],
            24.0 * s,
            &crate::profile_card::Avatar {
                key_id,
                version,
                name: &initial,
                verified: false,
                tick: sjk_identity::Tick::None,
                trust: None,
                preview: false,
                lit: false,
            },
        );
        let text_x = x + 92.0;
        let text_w = width - 92.0 - 24.0;
        text(
            ui,
            TextFamily::Display,
            format_args!("{}", said.headline),
            frame.rect(text_x, y + 16.0, text_w - toast_beside(said), 32.0),
            24.0 * s,
            color::TEXT,
            FontWeight::Semibold,
            TextAlign::Start,
        );
        if let Some((left, _)) = said.clock {
            text(
                ui,
                TextFamily::Display,
                format_args!("{}", clock(left)),
                frame.rect(text_x, y + 16.0, text_w, 32.0),
                24.0 * s,
                color::GOLD_BRIGHT,
                FontWeight::Semibold,
                TextAlign::End,
            );
        }
        if !said.line.is_empty() {
            text(
                ui,
                TextFamily::Body,
                format_args!("{}", said.line),
                frame.rect(text_x, y + 50.0, text_w, 22.0),
                15.0 * s,
                color::MUTED,
                FontWeight::Regular,
                TextAlign::Start,
            );
        }
        let (key, does) = said.key;
        if said.clickable && !key.is_empty() {
            // The pointer is free: a gold button opens the window, and so does the card.
            let button = [
                x + width - 24.0 - 170.0,
                y + (height - 40.0) * 0.5,
                170.0,
                40.0,
            ];
            kit::button(
                ui,
                &frame,
                button,
                "Open trade",
                true,
                true,
                false,
                TOAST_OPEN_TOKEN,
            );
            ui.hit_region(TOAST_TOKEN, card);
        } else if !key.is_empty() {
            let hint = key_hint_width(&[key], does, s);
            // Clear of the request's bar along the bottom, at the card's right edge.
            let [right, keys_y] = frame.point(x + width - 24.0, y + height - 40.0);
            key_hint(ui, &[key], does, right - hint, keys_y, s);
        }
        if let Some(share) = said.share {
            let _ = ui.draw_list_mut().push(DrawCommand::RoundedRect {
                rect: frame.rect(x + 24.0, y + height - 6.0, (width - 48.0) * share, 3.0),
                radius: 1.5 * s,
                color: color::GOLD,
            });
        }
        ui.finish(0);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use sjk_identity::{TradePartner, TradeSide};

    /// An open trade with Nib: the player offers a Rare holocron, Nib a Legendary one, and
    /// the player holds an Uncommon one and a second Haze to add.
    pub(crate) fn open_view() -> View {
        View {
            trade: PlayerTrade {
                id: 3,
                state: "open".to_owned(),
                asker: true,
                with: TradePartner {
                    key_id: "ff".to_owned(),
                    name: "^1Nib".to_owned(),
                    verified: true,
                    avatar: String::new(),
                },
                rev: 4,
                mine: TradeSide {
                    value: 8,
                    ..TradeSide::default()
                },
                theirs: TradeSide {
                    value: 18,
                    ..TradeSide::default()
                },
                ..PlayerTrade::default()
            },
            partner: "Nib".to_owned(),
            partner_shown: "^1Nib^7".to_owned(),
            partner_key: "ff".to_owned(),
            partner_avatar: String::new(),
            mine: vec![Item::holocron(7, "rare", 8)],
            theirs: vec![Item::holocron(9, "legendary", 18)],
            own: vec![
                Item::holocron(11, "uncommon", 4),
                Item::shader("saber_haze", Some("Haze"), 10),
            ],
            items_max: 6,
            seconds_left: 272,
            left_exact: 272.0,
            ask_secs: 30,
            confirm_in: None,
            limits: vec![Limit {
                label: "Holocrons",
                received: (1, 4),
                given: (0, 4),
                colour: None,
            }],
            holocrons: true,
            resets_in: None,
            room: Room::OPEN,
            problem: String::new(),
            confirming_gift: false,
            marks_kept: true,
            favourites_only: false,
        }
    }

    fn window(view: View) -> Window {
        let mut window = Window::default();
        window.set_view(Some(view));
        window
    }

    fn drawn(window: &mut Window) -> Vec<String> {
        window.ui.begin_transparent([1920.0, 1080.0]);
        let view = window.view.clone().unwrap();
        let frame = Frame::new([1920.0, 1080.0]);
        if view.trade.is_open() {
            draw_open(&mut window.ui, &frame, &view);
        } else {
            draw_small(&mut window.ui, &frame, &view);
        }
        window.ui.finish(OTHER_TOKEN);
        window.ui.text_runs().map(str::to_owned).collect()
    }

    #[test]
    fn a_request_reads_accept_or_decline_and_enter_accepts() {
        let mut view = open_view();
        view.trade.state = "asked".to_owned();
        view.trade.asker = false;
        view.seconds_left = 24;
        view.left_exact = 24.0;
        let mut window = window(view.clone());
        let texts = drawn(&mut window);
        for wanted in ["^1Nib^7 wants to trade", "Accept", "Decline", "24s"] {
            assert!(
                texts.iter().any(|text| text == wanted),
                "{wanted}: {texts:?}"
            );
        }
        assert_eq!(
            window.key(KeyCode::Enter),
            Some(Choice::Step(3, TradeStep::Accept))
        );
        assert_eq!(
            window.press(OTHER_TOKEN),
            Some(Choice::Step(3, TradeStep::Decline))
        );
        assert_eq!(window.key(KeyCode::Escape), Some(Choice::Close));
        assert_eq!(window.key(KeyCode::Space), None);
        // The asker waits, and may only cancel.
        view.trade.asker = true;
        let mut window = super::tests::window(view);
        let texts = drawn(&mut window);
        assert!(
            texts.iter().any(|text| text == "Waiting for ^1Nib^7"),
            "{texts:?}"
        );
        assert_eq!(window.key(KeyCode::Enter), None);
        assert_eq!(
            window.press(CANCEL_TOKEN),
            Some(Choice::Step(3, TradeStep::Cancel))
        );
    }

    #[test]
    fn clicking_items_offers_the_side_whole() {
        let mut window = window(open_view());
        let texts = drawn(&mut window);
        for wanted in [
            "You give",
            "^1Nib^7 gives",
            "worth about 8 shards",
            "Ready",
            "Daily limits",
            "Received",
            "Given",
            "1 / 4",
        ] {
            assert!(
                texts.iter().any(|text| text == wanted),
                "{wanted}: {texts:?}"
            );
        }
        // The Haze among the player's items goes in beside the Rare holocron.
        assert!(window.ui.rect_for(OWN_FIRST + 1).is_some());
        assert_eq!(
            window.press(OWN_FIRST + 1),
            Some(Choice::Step(
                3,
                TradeStep::Offer {
                    holocrons: vec![7],
                    shaders: vec!["saber_haze".to_owned()],
                }
            ))
        );
        // The offered holocron comes back out; Nib's items answer no click.
        assert_eq!(
            window.press(OFFERED_FIRST),
            Some(Choice::Step(
                3,
                TradeStep::Offer {
                    holocrons: vec![],
                    shaders: vec![],
                }
            ))
        );
        assert_eq!(window.press(OFFERED_FIRST + 1), None);
        assert_eq!(
            window.key(KeyCode::Enter),
            Some(Choice::Step(
                3,
                TradeStep::Ready {
                    rev: 4,
                    ready: true
                }
            ))
        );
    }

    /// Collection marks (29/10/2026): a locked shader answers no click and says why, the
    /// Favourites chip shows the favourites alone (each keeping its own token), and a hub
    /// without marks shows no chip.
    #[test]
    fn a_locked_shader_waits_and_the_favourites_chip_filters() {
        let mut view = open_view();
        let mut star = Item::shader("saber_tidal", Some("Tidal"), 5);
        star.favourite = true;
        star.new = true;
        view.own[1].locked = true;
        view.own.push(star);
        let mut window = window(view.clone());
        let texts = drawn(&mut window);
        assert!(
            texts
                .iter()
                .any(|text| text.contains("Locked shaders stay out")),
            "{texts:?}"
        );
        assert!(texts.iter().any(|text| text == "New"));
        assert!(
            window.ui.rect_for(OWN_FIRST + 1).is_none(),
            "the locked Haze"
        );
        assert_eq!(window.press(OWN_FIRST + 1), None);
        assert!(window.press(OWN_FIRST + 2).is_some(), "the starred Tidal");
        // The chip: only Tidal shows, under its own token.
        assert_eq!(window.press(FAVOURITES_TOKEN), None);
        let texts = drawn(&mut window);
        assert!(
            window.ui.rect_for(OWN_FIRST).is_none(),
            "the holocron is hidden"
        );
        assert!(window.ui.rect_for(OWN_FIRST + 2).is_some());
        assert!(
            !texts
                .iter()
                .any(|text| text.contains("Locked shaders stay out"))
        );
        // It stays on for the next view, as long as the hub keeps marks.
        window.set_view(Some(view.clone()));
        assert!(window.view.as_ref().unwrap().favourites_only);
        view.marks_kept = false;
        let mut window = super::tests::window(view);
        let _ = drawn(&mut window);
        assert!(window.ui.rect_for(FAVOURITES_TOKEN).is_none());
    }

    #[test]
    fn confirm_waits_for_both_and_its_delay() {
        let mut view = open_view();
        view.trade.mine.ready = true;
        let mut window = window(view.clone());
        assert_eq!(window.key(KeyCode::Enter), None, "waiting for them");
        assert_eq!(
            window.press(OTHER_TOKEN),
            Some(Choice::Step(
                3,
                TradeStep::Ready {
                    rev: 4,
                    ready: false
                }
            ))
        );
        view.trade.theirs.ready = true;
        view.confirm_in = Some(2);
        let mut window = super::tests::window(view.clone());
        let texts = drawn(&mut window);
        assert!(texts.iter().any(|text| text == "Confirm in 2"), "{texts:?}");
        assert_eq!(window.key(KeyCode::Enter), None);
        view.confirm_in = Some(0);
        let mut window = super::tests::window(view);
        assert_eq!(
            window.key(KeyCode::Enter),
            Some(Choice::Step(3, TradeStep::Confirm { rev: 4 }))
        );
    }

    #[test]
    fn a_full_side_takes_no_more_and_an_ended_trade_closes() {
        let mut view = open_view();
        view.items_max = 1;
        let mut window = window(view.clone());
        drawn(&mut window);
        assert_eq!(window.press(OWN_FIRST), None);
        view.trade.state = "done".to_owned();
        let mut window = super::tests::window(view);
        let texts = drawn(&mut window);
        assert!(
            texts.iter().any(|text| text == "Traded with ^1Nib^7"),
            "{texts:?}"
        );
        assert_eq!(window.key(KeyCode::Enter), Some(Choice::Close));
        assert_eq!(window.press(CANCEL_TOKEN), None);
    }

    /// The window's draw list as an SVG, for a look at its layout without the game
    /// (shapes and text; pictures as their outline).
    fn svg(ui: &MenuCanvas) -> String {
        use std::fmt::Write as _;
        let colour = |c: Color| {
            format!(
                "rgba({},{},{},{:.3})",
                (c.r * 255.0) as u8,
                (c.g * 255.0) as u8,
                (c.b * 255.0) as u8,
                c.a
            )
        };
        let mut out = String::from(
            "<svg xmlns='http://www.w3.org/2000/svg' width='1920' height='1080'>\
             <rect width='1920' height='1080' fill='#3a3f4a'/>",
        );
        for command in ui.draw_list().commands() {
            match command {
                DrawCommand::SolidRect { rect, color } => {
                    let _ = write!(
                        out,
                        "<rect x='{}' y='{}' width='{}' height='{}' fill='{}'/>",
                        rect.x,
                        rect.y,
                        rect.width,
                        rect.height,
                        colour(*color)
                    );
                }
                DrawCommand::RoundedRect {
                    rect,
                    radius,
                    color,
                } => {
                    let _ = write!(
                        out,
                        "<rect x='{}' y='{}' width='{}' height='{}' rx='{}' fill='{}'/>",
                        rect.x,
                        rect.y,
                        rect.width,
                        rect.height,
                        radius,
                        colour(*color)
                    );
                }
                DrawCommand::Border {
                    rect,
                    radius,
                    width,
                    color,
                } => {
                    let _ = write!(
                        out,
                        "<rect x='{}' y='{}' width='{}' height='{}' rx='{}' fill='none' stroke='{}' stroke-width='{}'/>",
                        rect.x,
                        rect.y,
                        rect.width,
                        rect.height,
                        radius,
                        colour(*color),
                        width
                    );
                }
                DrawCommand::TexturedQuad { rect, .. } => {
                    let _ = write!(
                        out,
                        "<rect x='{}' y='{}' width='{}' height='{}' fill='#2fc77a' opacity='0.6'/>",
                        rect.x, rect.y, rect.width, rect.height
                    );
                }
                DrawCommand::Text {
                    rect,
                    text,
                    size,
                    color,
                    align,
                    weight,
                    ..
                } => {
                    // Colour codes as the game draws them: ^1 red, ^5 cyan, ^7 white ...
                    let raw = ui.stored_text(*text);
                    let palette = [
                        "#000", "#f33", "#3f3", "#ff3", "#33f", "#3ff", "#f3f", "#fff",
                    ];
                    let mut words = String::from("<tspan>");
                    let mut chars = raw.chars().peekable();
                    while let Some(c) = chars.next() {
                        if c == '^' && chars.peek().is_some_and(|d| d.is_ascii_digit()) {
                            let digit = chars.next().unwrap().to_digit(10).unwrap() as usize;
                            let _ = write!(words, "</tspan><tspan fill='{}'>", palette[digit % 8]);
                        } else {
                            match c {
                                '&' => words.push_str("&amp;"),
                                '<' => words.push_str("&lt;"),
                                _ => words.push(c),
                            }
                        }
                    }
                    words.push_str("</tspan>");
                    let (x, anchor) = match align {
                        TextAlign::Center => (rect.x + rect.width * 0.5, "middle"),
                        TextAlign::End => (rect.x + rect.width, "end"),
                        _ => (rect.x, "start"),
                    };
                    let bold = if matches!(weight, FontWeight::Semibold) {
                        600
                    } else {
                        400
                    };
                    let _ = write!(
                        out,
                        "<text x='{}' y='{}' font-family='Exo 2, Arial' font-size='{}' font-weight='{}' fill='{}' text-anchor='{}'>{}</text>",
                        x,
                        rect.y + rect.height * 0.5 + size * 0.35,
                        size,
                        bold,
                        colour(*color),
                        anchor,
                        words
                    );
                }
                _ => {}
            }
        }
        out.push_str("</svg>");
        out
    }

    /// Writes the window in its states as SVGs to `SJK_TRADE_SVG` (a folder), for a look.
    #[test]
    #[ignore = "writes pictures for a look at the layout"]
    fn trade_window_pictures() {
        let Some(folder) = std::env::var_os("SJK_TRADE_SVG") else {
            return;
        };
        let folder = std::path::PathBuf::from(folder);
        let mut open = open_view();
        open.own.push(Item::holocron(12, "mythical", 51));
        open.own.push(Item::holocron(13, "rare", 8));
        let mut waiting = Item::holocron(14, "legendary", 18);
        waiting.locked_for = Some(5 * 86_400 + 3_000);
        open.own.push(waiting);
        open.resets_in = Some(14 * 3_600 + 7 * 60 + 33);
        open.limits = vec![
            Limit {
                label: "Holocrons",
                received: (1, 4),
                given: (2, 4),
                colour: None,
            },
            Limit {
                label: "Duplicates",
                received: (0, 10),
                given: (0, 10),
                colour: None,
            },
        ];
        let mut limited = open.clone();
        limited.room = Room {
            holocrons: 2,
            tiers: [1, 0, 1, 0],
            duplicates: 10,
        };
        let mut gift = open.clone();
        gift.theirs.clear();
        gift.trade.theirs.value = 0;
        gift.confirming_gift = true;
        let mut received = open.clone();
        received.mine.clear();
        received.trade.mine.value = 0;
        let mut ready = open.clone();
        ready.trade.mine.ready = true;
        ready.trade.theirs.ready = true;
        ready.confirm_in = Some(2);
        let mut asked = open.clone();
        asked.trade.state = "asked".to_owned();
        asked.trade.asker = false;
        asked.seconds_left = 24;
        asked.left_exact = 24.0;
        for (name, view) in [
            ("open", open),
            ("limited", limited),
            ("gift", gift),
            ("gift-received", received),
            ("both-ready", ready),
            ("request", asked),
        ] {
            let asks = view.confirming_gift;
            let mut window = window(view);
            if asks {
                let _ = window.key(KeyCode::Enter);
            }
            drawn(&mut window);
            std::fs::write(folder.join(format!("{name}.svg")), svg(&window.ui)).unwrap();
        }
        // The toast, and the chat card with Trade beside Block.
        let mut toast = Toast::default();
        let mut said = Said::of(&open_view());
        said.clickable = true;
        toast.build(&said, [1920.0, 1080.0], 0.0);
        std::fs::write(folder.join("toast.svg"), svg(&toast.ui)).unwrap();
        let mut ended = open_view();
        ended.trade.state = "done".to_owned();
        ended.trade.mine.items = vec![sjk_identity::TradeItem::default()];
        ended.trade.theirs.items.clear();
        ended.trade.with.name = "Rollembones".to_owned();
        let said = Said::of(&ended);
        let font = crate::text::test_font();
        let measure = crate::sjk_chat_look::Measure::new(&font, font.style());
        toast.build(
            &said,
            [1920.0, 1080.0],
            toast_words(&said, &measure, &measure),
        );
        std::fs::write(folder.join("toast-ended.svg"), svg(&toast.ui)).unwrap();
    }

    /// Giving and getting nothing back asks once more: Ready opens the question, Go
    /// back leaves it, Yes, give it is the Ready; a change of the offers drops it.
    #[test]
    fn a_gift_asks_once_more_before_it_is_ready() {
        let mut gift = open_view();
        gift.theirs.clear();
        let mut window = window(gift.clone());
        assert_eq!(window.key(KeyCode::Enter), None, "Ready asks first");
        let texts = drawn(&mut window);
        for wanted in ["Give it as a gift?", "Yes, give it", "Go back"] {
            assert!(
                texts.iter().any(|text| text == wanted),
                "{wanted}: {texts:?}"
            );
        }
        assert!(
            !texts
                .iter()
                .any(|text| text == "Your holocrons and duplicates"),
            "the question takes the items' place: {texts:?}"
        );
        assert_eq!(window.press(OTHER_TOKEN), None);
        assert!(!window.view.as_ref().unwrap().confirming_gift, "back");
        assert_eq!(window.key(KeyCode::Enter), None);
        assert_eq!(
            window.key(KeyCode::Enter),
            Some(Choice::Step(
                3,
                TradeStep::Ready {
                    rev: 4,
                    ready: true
                }
            ))
        );
        // The hub's next list keeps the question while the offers stand, not after.
        let mut window = super::tests::window(gift.clone());
        let _ = window.key(KeyCode::Enter);
        window.set_view(Some(gift.clone()));
        assert!(window.view.as_ref().unwrap().confirming_gift);
        let mut changed = gift;
        changed.trade.rev += 1;
        window.set_view(Some(changed));
        assert!(!window.view.as_ref().unwrap().confirming_gift);
    }

    /// Receiving a gift needs no question: Ready is ready, and the window says so.
    #[test]
    fn a_gift_received_reads_as_one() {
        let mut view = open_view();
        view.mine.clear();
        let mut window = window(view);
        let texts = drawn(&mut window);
        assert!(
            texts
                .iter()
                .any(|t| t == "Nib is giving this as a gift: nothing is asked in return."),
            "{texts:?}"
        );
        assert_eq!(
            window.key(KeyCode::Enter),
            Some(Choice::Step(
                3,
                TradeStep::Ready {
                    rev: 4,
                    ready: true
                }
            ))
        );
    }

    /// Items the day's limits hold back are dimmed and answer no click, with why.
    #[test]
    fn items_past_the_daily_limit_wait_for_tomorrow() {
        let mut view = open_view();
        view.room = Room {
            holocrons: 0,
            tiers: [1; 4],
            duplicates: 10,
        };
        let mut window = window(view.clone());
        let texts = drawn(&mut window);
        assert!(
            texts
                .iter()
                .any(|t| t == "Today's holocrons are given. More tomorrow."),
            "{texts:?}"
        );
        // The Uncommon holocron waits; the Haze duplicate goes in.
        assert_eq!(window.press(OWN_FIRST), None);
        assert!(matches!(
            window.press(OWN_FIRST + 1),
            Some(Choice::Step(_, _))
        ));
        // One of each tier: today's Uncommon is given, so that one waits and says so.
        view.room = Room {
            holocrons: 3,
            tiers: [0, 1, 1, 1],
            duplicates: 10,
        };
        let mut tiered = super::tests::window(view.clone());
        let texts = drawn(&mut tiered);
        assert!(
            texts
                .iter()
                .any(|t| t.contains("today's Uncommon holocron")),
            "{texts:?}"
        );
        assert_eq!(tiered.press(OWN_FIRST), None);
        // Before holocrons trade, the list says duplicates.
        view.holocrons = false;
        let mut duplicates = super::tests::window(view);
        assert!(
            drawn(&mut duplicates)
                .iter()
                .any(|t| t == "Your duplicates")
        );
    }

    /// The menu font draws bytes: every word the window and the toast show is ASCII,
    /// or it would draw as "?" (the swap arrow did, Creyon, 22/10/2026).
    #[test]
    fn every_word_drawn_is_plain_ascii() {
        let mut open = open_view();
        open.trade.uneven = true;
        let mut asked = open_view();
        asked.trade.state = "asked".to_owned();
        let mut done = open_view();
        done.trade.state = "done".to_owned();
        let mut gift = open_view();
        gift.theirs.clear();
        for (view, asks) in [(open, false), (asked, false), (done, false), (gift, true)] {
            let mut window = window(view);
            if asks {
                let _ = window.key(KeyCode::Enter);
                assert!(window.view.as_ref().unwrap().confirming_gift);
            }
            for text in drawn(&mut window) {
                assert!(text.is_ascii(), "{text:?}");
            }
        }
    }

    /// With the pointer free the toast is a button: Open trade, and a click anywhere on
    /// it, opens the window.
    #[test]
    fn a_clickable_toast_opens_the_window() {
        use sjk_ui::{PointerButton, Vec2};
        let mut said = Said::of(&open_view());
        said.clickable = true;
        let mut toast = Toast::default();
        toast.build(&said, [1920.0, 1080.0], 0.0);
        let texts: Vec<String> = toast.ui.text_runs().map(str::to_owned).collect();
        assert!(texts.iter().any(|t| t == "Open trade"), "{texts:?}");
        let button = toast.ui.rect_for(TOAST_OPEN_TOKEN).expect("Open trade");
        let position = Vec2::new(button.x + 4.0, button.y + 4.0);
        let button_kind = PointerButton::Primary;
        assert!(!toast.handle_pointer(InputEvent::PointerMove(position)));
        assert!(!toast.handle_pointer(InputEvent::PointerPress {
            position,
            button: button_kind
        }));
        assert!(toast.handle_pointer(InputEvent::PointerRelease {
            position,
            button: button_kind
        }));
        // Not clickable: the key hint, no button.
        said.clickable = false;
        toast.build(&said, [1920.0, 1080.0], 0.0);
        assert!(toast.ui.rect_for(TOAST_OPEN_TOKEN).is_none());
    }

    /// In play the toast's key hint sits at the card's right edge, wide card or not
    /// (Sol's review of #321: a local `width` once put it at the left edge).
    #[test]
    fn the_key_hint_sits_at_the_cards_right_edge() {
        let mut view = open_view();
        view.trade.state = "asked".to_owned();
        view.trade.asker = false;
        let said = Said::of(&view);
        assert!(!said.clickable && !said.key.0.is_empty());
        for words in [0.0, 900.0] {
            let width = toast_width(words);
            let (left, right) = (960.0 - width * 0.5, 960.0 + width * 0.5);
            let mut toast = Toast::default();
            toast.build(&said, [1920.0, 1080.0], words);
            // The key's cap is the one small rounded border on the card.
            let cap = toast
                .ui
                .draw_list()
                .commands()
                .iter()
                .find_map(|command| match command {
                    DrawCommand::Border { rect, radius, .. } if *radius < 8.0 => Some(*rect),
                    _ => None,
                })
                .expect("the key's cap");
            assert!(cap.x > left + width * 0.5, "{cap:?} on a card from {left}");
            assert!(
                cap.x + cap.width <= right - 24.0 + 0.5,
                "{cap:?} to {right}"
            );
        }
    }

    #[test]
    fn clocks_and_countdowns_read_as_shown() {
        assert_eq!(hms(86_399), "23:59:59");
        assert_eq!(hms(4 * 3_600 + 5 * 60 + 9), "04:05:09");
        assert_eq!(countdown(0), "<1m");
        assert_eq!(countdown(61), "2m");
        assert_eq!(countdown(5 * 3_600 + 720), "5h 12m");
        assert_eq!(countdown(7 * 86_400 - 30), "7d 0h");
        assert_eq!(countdown(6 * 86_400 + 23 * 3_600 + 5), "6d 23h");
    }

    #[test]
    fn the_toast_says_who_and_how_to_answer() {
        let mut view = open_view();
        view.trade.state = "asked".to_owned();
        view.trade.asker = false;
        view.seconds_left = 21;
        view.left_exact = 21.0;
        let said = Said::of(&view);
        assert_eq!(said.headline, "^1Nib^7 wants to trade");
        assert_eq!(said.key, ("Esc", "answer"));
        assert_eq!((said.clock, said.initial), (Some((21, 30)), 'N'));
        view.trade.state = "open".to_owned();
        view.trade.theirs.ready = true;
        assert_eq!(Said::of(&view).line, "They are ready: your turn.");
        assert_eq!(clock(272), "4:32");
    }

    /// An ended trade's toast keeps its words on one line, never cut: the card grows to
    /// fit them, centred, and stays the usual width for short ones.
    #[test]
    fn an_ended_trade_toast_fits_its_words_on_one_line() {
        let font = crate::text::test_font();
        let measure = crate::sjk_chat_look::Measure::new(&font, font.style());
        let mut view = open_view();
        view.trade.state = "done".to_owned();
        view.trade.mine.items = vec![sjk_identity::TradeItem::default()];
        view.trade.theirs.items.clear();
        view.trade.with.name = "^5jof^7\u{bb}jk\u{ab}^5bel^3y^5ash".to_owned();
        let said = Said::of(&view);
        let words = toast_words(&said, &measure, &measure);
        let width = toast_width(words);
        assert!(width > TOAST_W && width <= TOAST_MAX_W, "{width}");
        assert!(width - TOAST_LEFT - TOAST_RIGHT >= words, "the words fit");
        let mut toast = Toast::default();
        toast.build(&said, [1920.0, 1080.0], words);
        let texts: Vec<String> = toast.ui.text_runs().map(str::to_owned).collect();
        assert_eq!(
            texts,
            ["N".to_owned(), said.headline.clone()],
            "one line, whole"
        );
        assert!(said.headline.ends_with("and received nothing."));
        // Short words keep the usual card; the widest stays on the frame.
        assert_eq!(toast_width(300.0), TOAST_W);
        assert_eq!(toast_width(5_000.0), TOAST_MAX_W);
    }
}
