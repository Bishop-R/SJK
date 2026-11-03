//! Cosmetic loadouts (Lex's idea, 03/11/2026): the player's look saved under a name in
//! one of [`SLOTS`] slots and worn again in one click, from the Collection's Loadouts card
//! (`docs/sjk-ui.md`, Collection). A loadout keeps the cvars of [`CVARS`]: the hilts, the
//! saber shaders of each blade and their parts, the colours (with the hat and cape JoF
//! EJK names after them, [`crate::cosmetics`]), the nameplate style and the tag. Wearing
//! one sets those cvars back as they were saved; a shader the player no longer holds is
//! set all the same and shows as the stock blade until held, as `saberskin` does.
//!
//! The slots are kept on this PC, in `loadouts.json` in the profile folder
//! (`GameData/SJK/`), never at the hub:
//!
//! ```json
//! { "version": 1, "slots": [
//!   { "name": "Duels", "look": { "saber1": "single_1", "cg_saberSkin": "sun" } },
//!   null
//! ] }
//! ```

use crate::menu::sjk::{Frame, color, kit, text};
use crate::menu_widgets::{MenuCanvas, TextFamily};
use serde_json::{Value, json};
use sjk_ui::{FontWeight, TextAlign};
use std::path::Path;
use winit::keyboard::KeyCode;

/// The file, in the profile folder.
pub(crate) const FILE: &str = "loadouts.json";
/// How many loadouts can be kept.
pub(crate) const SLOTS: usize = 5;
/// Most characters of a loadout's name.
pub(crate) const NAME_CHARS: usize = 24;
/// Longest value a loadout keeps for one cvar (a config line's).
const VALUE_CHARS: usize = 256;

/// What a loadout keeps, in the order wearing sets them back: the hilts, the RGB colours
/// before the colour numbers that name them (as the Collection sets them), the shaders and
/// their parts, the nameplate style and the tag.
pub(crate) const CVARS: [&str; 12] = [
    "saber1",
    "saber2",
    "cp_sbRGB1",
    "cp_sbRGB2",
    "color1",
    "color2",
    crate::unlockables::SABER_SKIN_CVAR,
    crate::looks::SECOND_CVAR,
    crate::saber_skin_options::CVAR,
    crate::hud::nameplate::STYLE_CVAR,
    crate::tags::CVAR,
    crate::tags::FINISH_CVAR,
];

/// One saved look.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Loadout {
    pub(crate) name: String,
    /// The value of each of [`CVARS`] when it was saved, in that order.
    pub(crate) values: Vec<String>,
}

impl Loadout {
    /// The look `value` reads now (each of [`CVARS`]), named `name`.
    pub(crate) fn capture(name: &str, value: impl Fn(&str) -> String) -> Self {
        Self {
            name: clean(name, NAME_CHARS),
            values: CVARS
                .iter()
                .map(|cvar| clean(&value(cvar), VALUE_CHARS))
                .collect(),
        }
    }

    /// The value it keeps for `cvar`, if it keeps one.
    pub(crate) fn value(&self, cvar: &str) -> Option<&str> {
        let index = CVARS
            .iter()
            .position(|known| known.eq_ignore_ascii_case(cvar))?;
        self.values.get(index).map(String::as_str)
    }

    /// The cvars wearing it changes, each with its value, from `current`'s values: only
    /// those that differ, in [`CVARS`]' order.
    pub(crate) fn changes<'a>(
        &'a self,
        current: impl Fn(&str) -> String + 'a,
    ) -> impl Iterator<Item = (&'static str, &'a str)> + 'a {
        CVARS
            .iter()
            .zip(&self.values)
            .filter(move |(cvar, value)| current(cvar) != **value)
            .map(|(cvar, value)| (*cvar, value.as_str()))
    }

    /// A line of what it is: its hilts, shaders and tag, as players read them.
    pub(crate) fn summary(&self) -> String {
        let value = |cvar: &str| self.value(cvar).unwrap_or_default();
        let mut parts: Vec<String> = Vec::new();
        let hilt = |name: &str| {
            (!name.is_empty() && !name.eq_ignore_ascii_case("none")).then(|| name.to_owned())
        };
        match (hilt(value("saber1")), hilt(value("saber2"))) {
            (Some(first), Some(second)) => parts.push(format!("{first} + {second}")),
            (Some(first), None) => parts.push(first),
            (None, Some(second)) => parts.push(second),
            (None, None) => {}
        }
        let shader = |id: &str| {
            crate::unlockables::blade_skin(id)
                .map_or_else(|| id.to_owned(), |skin| skin.name.to_owned())
        };
        let first = value(crate::unlockables::SABER_SKIN_CVAR);
        parts.push(if first.is_empty() {
            "Stock blade".to_owned()
        } else {
            shader(first)
        });
        let second = value(crate::looks::SECOND_CVAR);
        if !second.is_empty() && !second.eq_ignore_ascii_case(crate::looks::STOCK_SECOND) {
            parts.push(shader(second));
        }
        let tag = value(crate::tags::CVAR);
        if !tag.is_empty() {
            parts.push(format!("tag {tag}"));
        }
        parts.join("  ·  ")
    }
}

/// `text` without control characters, trimmed, at most `chars` characters.
fn clean(text: &str, chars: usize) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(chars)
        .collect::<String>()
        .trim()
        .to_owned()
}

/// The slots and their file.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Store {
    pub(crate) slots: [Option<Loadout>; SLOTS],
}

impl Store {
    /// The slots kept in `directory` (the profile folder); empty ones without the file or
    /// with one that cannot be read.
    pub(crate) fn load(directory: &Path) -> Self {
        std::fs::read(directory.join(FILE))
            .ok()
            .and_then(|bytes| parse(&bytes))
            .unwrap_or_default()
    }

    /// Keep the slots in `directory`; the log says why when that fails.
    pub(crate) fn save(&self, directory: &Path) {
        let slots: Vec<Value> = self
            .slots
            .iter()
            .map(|slot| match slot {
                None => Value::Null,
                Some(loadout) => {
                    let look: serde_json::Map<String, Value> = CVARS
                        .iter()
                        .zip(&loadout.values)
                        .map(|(cvar, value)| ((*cvar).to_owned(), json!(value)))
                        .collect();
                    json!({"name": loadout.name, "look": look})
                }
            })
            .collect();
        let text = serde_json::to_string_pretty(&json!({"version": 1, "slots": slots}))
            .unwrap_or_default();
        let path = directory.join(FILE);
        if let Err(error) = std::fs::write(&path, text) {
            crate::log::progress(format_args!("loadouts: {}: {error}", path.display()));
        }
    }

    /// Carry out `request` on the slots, `current` reading the look worn now (for a
    /// save); the cvars to set for a wear, in order. Whether the slots changed comes
    /// with them.
    pub(crate) fn apply(
        &mut self,
        request: &Request,
        current: impl Fn(&str) -> String,
    ) -> (bool, Vec<(&'static str, String)>) {
        match request {
            Request::Wear(slot) => {
                let Some(Some(loadout)) = self.slots.get(*slot) else {
                    return (false, Vec::new());
                };
                let changes = loadout
                    .changes(current)
                    .map(|(cvar, value)| (cvar, value.to_owned()))
                    .collect();
                (false, changes)
            }
            Request::Save(slot) => {
                let Some(place) = self.slots.get_mut(*slot) else {
                    return (false, Vec::new());
                };
                let name = place
                    .as_ref()
                    .map_or_else(|| format!("Loadout {}", slot + 1), |kept| kept.name.clone());
                *place = Some(Loadout::capture(&name, current));
                (true, Vec::new())
            }
            Request::Rename(slot, name) => {
                let name = clean(name, NAME_CHARS);
                match self.slots.get_mut(*slot) {
                    Some(Some(loadout)) if !name.is_empty() && loadout.name != name => {
                        loadout.name = name;
                        (true, Vec::new())
                    }
                    _ => (false, Vec::new()),
                }
            }
            Request::Clear(slot) => match self.slots.get_mut(*slot) {
                Some(place @ Some(_)) => {
                    *place = None;
                    (true, Vec::new())
                }
                _ => (false, Vec::new()),
            },
        }
    }
}

/// The slots a `loadouts.json` holds; `None` when it is not one.
fn parse(bytes: &[u8]) -> Option<Store> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    let slots = value.get("slots")?.as_array()?;
    let mut store = Store::default();
    for (place, slot) in store.slots.iter_mut().zip(slots) {
        let Some(name) = slot.get("name").and_then(Value::as_str) else {
            continue;
        };
        let look = slot.get("look").and_then(Value::as_object);
        let values = CVARS
            .iter()
            .map(|cvar| {
                look.and_then(|look| look.get(*cvar))
                    .and_then(Value::as_str)
                    .map(|value| clean(value, VALUE_CHARS))
                    .unwrap_or_default()
            })
            .collect();
        let name = clean(name, NAME_CHARS);
        *place = Some(Loadout {
            name: if name.is_empty() {
                "Loadout".to_owned()
            } else {
                name
            },
            values,
        });
    }
    Some(store)
}

/// What the card asks of the console.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Request {
    /// Wear the loadout in this slot.
    Wear(usize),
    /// Save the look worn now in this slot (its name kept, or "Loadout N").
    Save(usize),
    /// Rename the loadout in this slot.
    Rename(usize, String),
    /// Empty this slot.
    Clear(usize),
}

/// What the card answers to a key or a click.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Reply {
    /// Nothing more to do.
    None,
    /// The card closed.
    Closed,
    /// Carry this out.
    Ask(Request),
}

/// A press waiting for its second one: replacing a saved loadout, emptying a slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Pending {
    Replace(usize),
    Clear(usize),
}

/// The tokens of the card: the rows', each row's buttons, Close.
const TOKEN: u16 = 5_000;
const ROW_BASE: u16 = TOKEN;
const WEAR_BASE: u16 = TOKEN + 10;
const SAVE_BASE: u16 = TOKEN + 20;
const RENAME_BASE: u16 = TOKEN + 30;
const CLEAR_BASE: u16 = TOKEN + 40;
const CLOSE_TOKEN: u16 = TOKEN + 50;
/// The Collection's button that opens the card.
pub(crate) const OPEN_TOKEN: u16 = TOKEN + 51;

/// The card's place on the 1080-line frame, and a row's height.
const CARD: [f32; 4] = [440.0, 170.0, 1_040.0, 740.0];
const ROWS_Y: f32 = 330.0;
const ROW_HEIGHT: f32 = 86.0;

/// The Loadouts card's state.
#[derive(Clone, Debug, Default)]
pub(crate) struct Card {
    open: bool,
    row: usize,
    /// The name being typed for the chosen row's loadout.
    renaming: Option<String>,
    pending: Option<Pending>,
    /// What the last request did, said under the rows.
    note: String,
}

impl Card {
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    pub(crate) fn open(&mut self) {
        *self = Self {
            open: true,
            ..Self::default()
        };
    }

    pub(crate) fn close(&mut self) {
        self.open = false;
        self.renaming = None;
        self.pending = None;
    }

    /// Whether a name is being typed (the field takes every key and its repeats).
    pub(crate) fn typing(&self) -> bool {
        self.renaming.is_some()
    }

    /// Say what a request did (the console's answer), under the rows.
    pub(crate) fn told(&mut self, note: impl Into<String>) {
        self.note = note.into();
    }

    /// A pressed key, the field's editing done by [`crate::text_entry::edit`]: `slots`
    /// are the loadouts kept.
    pub(crate) fn key(
        &mut self,
        key: KeyCode,
        text: Option<&str>,
        shift: bool,
        control: bool,
        slots: &[Option<Loadout>],
    ) -> Reply {
        if let Some(name) = &mut self.renaming {
            match key {
                KeyCode::Escape => self.renaming = None,
                KeyCode::Enter | KeyCode::NumpadEnter => {
                    let name = std::mem::take(name);
                    self.renaming = None;
                    return Reply::Ask(Request::Rename(self.row, name));
                }
                _ => {
                    let _ =
                        crate::text_entry::edit(name, key, text, shift, control, NAME_CHARS, true);
                }
            }
            return Reply::None;
        }
        let filled = slots.get(self.row).is_some_and(Option::is_some);
        let pending = self.pending.take();
        match key {
            KeyCode::Escape => {
                self.close();
                return Reply::Closed;
            }
            KeyCode::ArrowUp => self.row = self.row.checked_sub(1).unwrap_or(SLOTS - 1),
            KeyCode::ArrowDown => self.row = (self.row + 1) % SLOTS,
            KeyCode::Home => self.row = 0,
            KeyCode::End => self.row = SLOTS - 1,
            KeyCode::Digit1
            | KeyCode::Digit2
            | KeyCode::Digit3
            | KeyCode::Digit4
            | KeyCode::Digit5 => {
                self.row = digit(key);
            }
            KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space if filled => {
                return Reply::Ask(Request::Wear(self.row));
            }
            KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space | KeyCode::KeyS => {
                return self.save(slots, pending);
            }
            KeyCode::KeyR | KeyCode::F2 if filled => self.start_rename(slots),
            KeyCode::Delete | KeyCode::Backspace if filled => {
                return self.clear(pending);
            }
            _ => {}
        }
        Reply::None
    }

    /// A click (`activate`) or the pointer resting on `token`.
    pub(crate) fn token(&mut self, token: u16, activate: bool, slots: &[Option<Loadout>]) -> Reply {
        let in_range = |base: u16| {
            token
                .checked_sub(base)
                .map(usize::from)
                .filter(|row| *row < SLOTS)
        };
        if !activate {
            if let Some(row) = in_range(ROW_BASE).or_else(|| in_range(WEAR_BASE)) {
                if self.renaming.is_none() {
                    self.row = row;
                }
            }
            return Reply::None;
        }
        if token == CLOSE_TOKEN {
            self.close();
            return Reply::Closed;
        }
        // A click while a name is typed keeps what was typed, and does only that.
        if let Some(name) = self.renaming.take() {
            self.pending = None;
            return Reply::Ask(Request::Rename(self.row, name));
        }
        let pending = self.pending.take();
        if let Some(row) = in_range(ROW_BASE) {
            self.row = row;
            return Reply::None;
        }
        if let Some(row) = in_range(WEAR_BASE) {
            self.row = row;
            return if slots.get(row).is_some_and(Option::is_some) {
                Reply::Ask(Request::Wear(row))
            } else {
                Reply::None
            };
        }
        if let Some(row) = in_range(SAVE_BASE) {
            self.row = row;
            return self.save(slots, pending);
        }
        if let Some(row) = in_range(RENAME_BASE) {
            self.row = row;
            self.start_rename(slots);
            return Reply::None;
        }
        if let Some(row) = in_range(CLEAR_BASE) {
            self.row = row;
            return self.clear(pending);
        }
        Reply::None
    }

    /// Save here: at once in an empty slot, on a second press over a loadout.
    fn save(&mut self, slots: &[Option<Loadout>], pending: Option<Pending>) -> Reply {
        let row = self.row;
        if slots.get(row).is_some_and(Option::is_some) && pending != Some(Pending::Replace(row)) {
            self.pending = Some(Pending::Replace(row));
            return Reply::None;
        }
        Reply::Ask(Request::Save(row))
    }

    /// Empty the slot, on a second press.
    fn clear(&mut self, pending: Option<Pending>) -> Reply {
        let row = self.row;
        if pending != Some(Pending::Clear(row)) {
            self.pending = Some(Pending::Clear(row));
            return Reply::None;
        }
        Reply::Ask(Request::Clear(row))
    }

    fn start_rename(&mut self, slots: &[Option<Loadout>]) {
        if let Some(Some(loadout)) = slots.get(self.row) {
            self.renaming = Some(loadout.name.clone());
        }
    }

    /// The card over the Collection, which leaves its page out under it: `slots` are the
    /// loadouts kept.
    pub(crate) fn draw(
        &self,
        ui: &mut MenuCanvas,
        frame: &Frame,
        viewport: [f32; 2],
        slots: &[Option<Loadout>],
    ) {
        let s = frame.s;
        kit::scrim(ui, viewport);
        kit::card(ui, frame, CARD);
        let [x, y, width, height] = CARD;
        let inner = [x + 40.0, width - 80.0];
        let [left, inner_width] = inner;
        text(
            ui,
            TextFamily::Display,
            format_args!("Loadouts"),
            frame.rect(left, y + 28.0, 400.0, 28.0),
            22.0 * s,
            color::GOLD,
            FontWeight::Semibold,
            TextAlign::Start,
        );
        text(
            ui,
            TextFamily::Body,
            format_args!("Kept on this PC"),
            frame.rect(left, y + 28.0, inner_width, 28.0),
            16.0 * s,
            color::MUTED,
            FontWeight::Regular,
            TextAlign::End,
        );
        text(
            ui,
            TextFamily::Display,
            format_args!("Save your look, wear it again in one click"),
            frame.rect(left, y + 64.0, inner_width, 48.0),
            34.0 * s,
            color::TEXT,
            FontWeight::Semibold,
            TextAlign::Start,
        );
        text(
            ui,
            TextFamily::Body,
            format_args!(
                "A loadout keeps your hilts, each blade's shader and its parts, your colours with your hat and cape, your nameplate style and your tag."
            ),
            frame.rect(left, y + 118.0, inner_width, 24.0),
            16.0 * s,
            color::MUTED,
            FontWeight::Regular,
            TextAlign::Start,
        );
        for (row, slot) in slots.iter().enumerate().take(SLOTS) {
            self.row_view(ui, frame, row, slot.as_ref(), [left, inner_width]);
        }
        // Under the rows: a press waiting for its second one, else the last outcome.
        let under = y + height - 92.0;
        let (line, warm) = match self.pending {
            Some(Pending::Replace(row)) => (
                format!("Save again to replace {}", name_of(slots, row)),
                true,
            ),
            Some(Pending::Clear(row)) => (
                format!("Press again to empty the slot of {}", name_of(slots, row)),
                true,
            ),
            None => (self.note.clone(), false),
        };
        if !line.is_empty() {
            text(
                ui,
                TextFamily::Body,
                format_args!("{line}"),
                frame.rect(left, under, inner_width - 200.0, 28.0),
                17.0 * s,
                if warm { color::EMBER } else { color::HOLO },
                FontWeight::Semibold,
                TextAlign::Start,
            );
        }
        kit::button(
            ui,
            frame,
            [
                left + inner_width - 160.0,
                y + height - 70.0,
                160.0,
                kit::CONTROL_HEIGHT,
            ],
            "Close",
            false,
            true,
            false,
            CLOSE_TOKEN,
        );
        let keys = if self.renaming.is_some() {
            "Enter: keep the name    Esc: cancel    Ctrl V: paste"
        } else {
            "Up Down: choose    Enter: wear (or save in an empty slot)    S: save here    R: rename    Delete: empty    Esc: close"
        };
        text(
            ui,
            TextFamily::Body,
            format_args!("{keys}"),
            frame.rect(x, y + height + 18.0, width, 24.0),
            15.0 * s,
            color::MUTED,
            FontWeight::Regular,
            TextAlign::End,
        );
    }

    /// Row `row`: its number, the loadout's name (or the field while it is renamed) and
    /// what it keeps, and its buttons.
    fn row_view(
        &self,
        ui: &mut MenuCanvas,
        frame: &Frame,
        row: usize,
        loadout: Option<&Loadout>,
        [left, width]: [f32; 2],
    ) {
        let s = frame.s;
        let top = ROWS_Y + row as f32 * ROW_HEIGHT;
        let chosen = row == self.row;
        let height = ROW_HEIGHT - 8.0;
        if chosen {
            kit::band(ui, frame, [left - 12.0, top, width + 24.0, height]);
        }
        ui.hit_region(
            ROW_BASE + row as u16,
            frame.rect(left - 12.0, top, width - 420.0, height),
        );
        text(
            ui,
            TextFamily::Display,
            format_args!("{}", row + 1),
            frame.rect(left + 6.0, top + 12.0, 30.0, 30.0),
            26.0 * s,
            if chosen {
                color::GOLD_BRIGHT
            } else {
                color::HOLO
            },
            FontWeight::Semibold,
            TextAlign::Start,
        );
        let words = left + 44.0;
        let words_width = width - 44.0 - 440.0;
        match (loadout, &self.renaming) {
            (Some(_), Some(name)) if chosen => kit::field(
                ui,
                frame,
                [words, top + 4.0, words_width, kit::CONTROL_HEIGHT],
                format_args!("{name}|"),
                true,
                false,
            ),
            (Some(loadout), _) => text(
                ui,
                TextFamily::Display,
                format_args!("{}", loadout.name),
                frame.rect(words, top + 8.0, words_width, 30.0),
                24.0 * s,
                if chosen {
                    color::GOLD_BRIGHT
                } else {
                    color::TEXT
                },
                FontWeight::Semibold,
                TextAlign::Start,
            ),
            (None, _) => text(
                ui,
                TextFamily::Display,
                format_args!("Empty slot"),
                frame.rect(words, top + 8.0, words_width, 30.0),
                24.0 * s,
                color::QUIET,
                FontWeight::Regular,
                TextAlign::Start,
            ),
        }
        let line = loadout.map_or_else(
            || "Save here keeps the look you wear now".to_owned(),
            |loadout| ellipsis(&loadout.summary(), 58),
        );
        text(
            ui,
            TextFamily::Body,
            format_args!("{line}"),
            frame.rect(words, top + 46.0, words_width, 22.0),
            15.0 * s,
            color::MUTED,
            FontWeight::Regular,
            TextAlign::Start,
        );
        let middle = top + (height - kit::CONTROL_HEIGHT) * 0.5;
        let filled = loadout.is_some();
        let right = left + width;
        let buttons: [(f32, &str, bool, bool, u16); 4] = [
            (110.0, "Wear", true, filled, WEAR_BASE),
            (110.0, "Save here", false, true, SAVE_BASE),
            (110.0, "Rename", false, filled, RENAME_BASE),
            (80.0, "Empty", false, filled, CLEAR_BASE),
        ];
        let mut x = right - buttons.iter().map(|(w, ..)| w + 10.0).sum::<f32>() + 10.0;
        for (button_width, label, primary, enabled, base) in buttons {
            kit::button(
                ui,
                frame,
                [x, middle, button_width, kit::CONTROL_HEIGHT],
                label,
                primary,
                enabled,
                false,
                base + row as u16,
            );
            x += button_width + 10.0;
        }
    }
}

/// The loadout in `row`, or "this slot".
fn name_of(slots: &[Option<Loadout>], row: usize) -> String {
    slots
        .get(row)
        .and_then(Option::as_ref)
        .map_or_else(|| "this slot".to_owned(), |loadout| loadout.name.clone())
}

/// `text` cut at `chars` characters with an ellipsis.
fn ellipsis(text: &str, chars: usize) -> String {
    if text.chars().count() <= chars {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(chars.saturating_sub(3)).collect();
    cut.push_str("...");
    cut
}

/// The slot a digit key names (1 to 5).
fn digit(key: KeyCode) -> usize {
    match key {
        KeyCode::Digit2 => 1,
        KeyCode::Digit3 => 2,
        KeyCode::Digit4 => 3,
        KeyCode::Digit5 => 4,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look(cvar: &str) -> String {
        match cvar {
            "saber1" => "single_1",
            "saber2" => "none",
            "color1" => "6",
            "cp_sbRGB1" => "255",
            "cg_saberSkin" => "sun",
            "cg_tag" => "gonk",
            _ => "",
        }
        .to_owned()
    }

    #[test]
    fn a_saved_look_is_worn_again_and_kept_in_its_file() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = Store::load(directory.path());
        assert_eq!(store, Store::default(), "no file: every slot empty");
        let (changed, _) = store.apply(&Request::Save(1), look);
        assert!(changed);
        let saved = store.slots[1].clone().expect("saved");
        assert_eq!(saved.name, "Loadout 2");
        assert_eq!(saved.value("saber1"), Some("single_1"));
        assert_eq!(saved.value("cg_saberSkin"), Some("sun"));
        store.apply(&Request::Rename(1, "  Duels\u{7}  ".to_owned()), look);
        store.save(directory.path());
        let loaded = Store::load(directory.path());
        assert_eq!(
            loaded.slots[1].as_ref().map(|l| l.name.as_str()),
            Some("Duels")
        );
        assert_eq!(loaded, store);
        // Wearing sets back only what differs, in the list's order.
        let now = |cvar: &str| match cvar {
            "cg_saberSkin" => "glitch".to_owned(),
            "color1" => "4".to_owned(),
            other => look(other),
        };
        let (_, set) = store.apply(&Request::Wear(1), now);
        assert_eq!(
            set,
            [
                ("color1", "6".to_owned()),
                ("cg_saberSkin", "sun".to_owned())
            ]
        );
        // An empty slot wears nothing; a cleared slot is empty.
        assert!(store.apply(&Request::Wear(0), now).1.is_empty());
        assert!(store.apply(&Request::Clear(1), now).0);
        assert!(store.slots[1].is_none());
    }

    #[test]
    fn a_broken_file_reads_as_empty_slots() {
        assert!(parse(b"not json").is_none());
        let store =
            parse(br#"{"slots": [null, {"name": "", "look": {"saber1": 3}}, {"look": {}}]}"#)
                .expect("slots");
        assert!(store.slots[0].is_none());
        let named = store.slots[1].as_ref().expect("a loadout");
        assert_eq!(named.name, "Loadout");
        assert_eq!(named.value("saber1"), Some(""));
        assert!(store.slots[2].is_none(), "no name: not a loadout");
    }

    #[test]
    fn replacing_and_emptying_ask_twice() {
        let mut store = Store::default();
        store.apply(&Request::Save(0), look);
        let mut card = Card::default();
        card.open();
        let press =
            |card: &mut Card, store: &Store, key| card.key(key, None, false, false, &store.slots);
        // Enter on a kept loadout wears it; S over it waits for a second S.
        assert_eq!(
            press(&mut card, &store, KeyCode::Enter),
            Reply::Ask(Request::Wear(0))
        );
        assert_eq!(press(&mut card, &store, KeyCode::KeyS), Reply::None);
        assert_eq!(
            press(&mut card, &store, KeyCode::KeyS),
            Reply::Ask(Request::Save(0))
        );
        assert_eq!(press(&mut card, &store, KeyCode::Delete), Reply::None);
        assert_eq!(press(&mut card, &store, KeyCode::ArrowDown), Reply::None);
        // The second Delete on another row is a first one there: nothing kept, nothing done.
        assert_eq!(press(&mut card, &store, KeyCode::Delete), Reply::None);
        assert_eq!(press(&mut card, &store, KeyCode::ArrowUp), Reply::None);
        assert_eq!(press(&mut card, &store, KeyCode::Delete), Reply::None);
        assert_eq!(
            press(&mut card, &store, KeyCode::Delete),
            Reply::Ask(Request::Clear(0))
        );
        // An empty slot saves at once.
        assert_eq!(press(&mut card, &store, KeyCode::Digit3), Reply::None);
        assert_eq!(
            press(&mut card, &store, KeyCode::Enter),
            Reply::Ask(Request::Save(2))
        );
        assert_eq!(press(&mut card, &store, KeyCode::Escape), Reply::Closed);
        assert!(!card.is_open());
    }

    #[test]
    fn a_loadout_is_renamed_in_its_field() {
        let mut store = Store::default();
        store.apply(&Request::Save(0), look);
        let mut card = Card::default();
        card.open();
        assert_eq!(
            card.key(KeyCode::KeyR, None, false, false, &store.slots),
            Reply::None
        );
        assert!(card.typing());
        // Ctrl+A then typing replaces the name, as every field does.
        let _ = card.key(KeyCode::KeyA, Some("a"), false, true, &store.slots);
        let _ = card.key(KeyCode::KeyD, Some("D"), false, false, &store.slots);
        let _ = card.key(KeyCode::KeyU, Some("u"), false, false, &store.slots);
        assert_eq!(
            card.key(KeyCode::Enter, None, false, false, &store.slots),
            Reply::Ask(Request::Rename(0, "Du".to_owned()))
        );
        assert!(!card.typing());
    }

    #[test]
    fn the_card_fits_and_answers_the_pointer() {
        let mut store = Store::default();
        store.apply(&Request::Save(0), look);
        let mut card = Card::default();
        card.open();
        for viewport in [[1_920.0, 1_080.0], [3_840.0, 2_160.0], [1_440.0, 1_080.0]] {
            let mut ui = MenuCanvas::new();
            ui.begin_transparent(viewport);
            let frame = Frame::new(viewport);
            card.draw(&mut ui, &frame, viewport, &store.slots);
            ui.finish(u16::MAX);
            assert!(!ui.overflowed(), "{viewport:?}");
            for token in [
                WEAR_BASE,
                SAVE_BASE + 4,
                RENAME_BASE,
                CLEAR_BASE,
                CLOSE_TOKEN,
            ] {
                let rect = ui.rect_for(token).expect("a button");
                assert!(rect.right() <= viewport[0] && rect.bottom() <= viewport[1]);
            }
        }
        assert_eq!(
            card.token(WEAR_BASE, true, &store.slots),
            Reply::Ask(Request::Wear(0))
        );
        assert_eq!(
            card.token(WEAR_BASE + 1, true, &store.slots),
            Reply::None,
            "empty"
        );
        assert_eq!(
            card.token(SAVE_BASE + 1, true, &store.slots),
            Reply::Ask(Request::Save(1))
        );
        assert_eq!(card.token(CLOSE_TOKEN, true, &store.slots), Reply::Closed);
    }

    #[test]
    fn the_summary_names_hilts_shaders_and_tag() {
        let loadout = Loadout::capture("Duels", look);
        let line = loadout.summary();
        assert!(line.starts_with("single_1"), "{line}");
        assert!(line.contains("tag gonk"), "{line}");
        assert!(
            Loadout::capture("x", |_| String::new())
                .summary()
                .contains("Stock blade")
        );
    }
}
