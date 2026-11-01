//! The ghost editor (`ghost edit [name]`, `docs/client.md`, "Ghost editor"): the saved
//! runs as a list, a timeline to cut a run's start and end, and a switch that clears the
//! chat bubble, as an SJK UI column down the window's right edge (the map editor's), with
//! the run's path drawn over the world ([`super::editor_view`]).
//!
//! Nothing changes on disk until Save (which asks twice before it overwrites a run) or
//! Duplicate (which writes the run as cut under a new name, keeping the original). The
//! start snaps to the commands a replay can begin on cleanly, where the player stood still
//! on the ground ([`Cut::is_clean`]); Shift places it anywhere, with a warning. Look hides
//! the column to move about with the path still drawn, and Escape brings it back.
//!
//! It is a page of the in-game menu ([`crate::ingame_menu::Page::GhostEdit`]), which frees
//! the pointer and keeps clicks from the game; its state lives with the ghost.

use super::edit::{Cut, Motion, PathPoint};
use super::{Run, file, ghost_directory};
use crate::GpuState;
use crate::ingame_menu::Page;
use crate::menu_widgets::MenuCanvas;
use sjk_ui::{AbstractAction, InputEvent, PointerButton, Rect, UiEventKind, Vec2};
use std::path::PathBuf;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// The copy's name and Duplicate.
pub(crate) const NAME_FIELD: u16 = 10;
pub(crate) const DUPLICATE: u16 = 11;
/// The cut's two handles, and the buttons that place them.
pub(crate) const START: u16 = 20;
pub(crate) const END: u16 = 21;
pub(crate) const FIRST_MOVE: u16 = 22;
pub(crate) const LAST_MOVE: u16 = 23;
pub(crate) const RESET: u16 = 24;
/// Clear the chat bubble.
pub(crate) const BUBBLES: u16 = 30;
/// The foot.
pub(crate) const SAVE: u16 = 40;
pub(crate) const PLAY: u16 = 41;
pub(crate) const LOOK: u16 = 42;
pub(crate) const CLOSE: u16 = 43;
/// The list's rows on show are `GHOST_ROW..GHOST_ROW + GHOST_ROWS`.
pub(crate) const GHOST_ROW: u16 = 100;
pub(crate) const GHOST_ROWS: usize = 6;

/// What Left and Right move a handle by, and Page Up and Page Down, in milliseconds.
const STEP: i32 = 100;
const PAGE_STEP: i32 = 1_000;
/// Slices of the timeline ([`Cut::columns`]).
pub(crate) const COLUMNS: usize = 104;
/// The path drawn: points at least this far apart, and at most this many.
const PATH_SPACING: f32 = 12.0;
const PATH_POINTS: usize = 1_200;
/// Room for the path's lines, its markers and the strip.
const OVERLAY_DRAWS: usize = PATH_POINTS + 160;

/// A saved run in the list.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Listed {
    pub(crate) name: String,
    /// Its map and length, or why the file cannot be read.
    pub(crate) summary: Result<(String, i32), String>,
}

/// The run being edited.
pub(crate) struct Loaded {
    pub(crate) name: String,
    pub(crate) cut: Cut,
    /// The first and last commands kept.
    pub(crate) first: usize,
    pub(crate) last: usize,
    pub(crate) clear_talk: bool,
    /// Save was pressed once: the next press overwrites the file.
    armed: bool,
    /// What the timeline shows, per slice ([`Cut::columns`]).
    pub(crate) columns: Vec<(Motion, bool)>,
    pub(crate) path: Vec<PathPoint>,
}

impl Loaded {
    fn new(name: String, run: Run) -> Self {
        let cut = Cut::new(run);
        Self {
            name,
            first: 0,
            last: cut.len() - 1,
            clear_talk: true,
            armed: false,
            columns: cut.columns(COLUMNS),
            path: cut.path(PATH_SPACING, PATH_POINTS),
            cut,
        }
    }

    /// The chat bubble's stretches in the whole run.
    pub(crate) fn bubbles(&self) -> usize {
        self.cut.talk_stretches(0, self.cut.len() - 1)
    }

    /// Whether Save would write something new.
    pub(crate) fn changed(&self) -> bool {
        self.first != 0
            || self.last + 1 != self.cut.len()
            || (self.clear_talk && self.bubbles() > 0)
    }

    /// Milliseconds kept.
    pub(crate) fn kept(&self) -> i32 {
        self.cut.time(self.last) - self.cut.time(self.first)
    }

    fn edited(&self) -> Run {
        self.cut.edited(self.first, self.last, self.clear_talk)
    }
}

/// A handle of the cut.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Handle {
    Start,
    End,
}

/// How a moved start lands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Snap {
    /// On the nearest clean start (a drag).
    Nearest,
    /// On the first clean start that way (a key).
    Forward,
    Back,
    /// Where it was put (Shift).
    Free,
}

/// What the editor asks of the game.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Request {
    /// `ghost play <name>`.
    Play(String),
    /// Hide the column, the path still drawn.
    Look,
    Close,
}

/// The editor's state, while it is on.
pub(crate) struct Editor {
    directory: PathBuf,
    pub(crate) ghosts: Vec<Listed>,
    /// The list's row the keyboard is on, and the first row on show.
    pub(crate) row: usize,
    pub(crate) list_first: usize,
    pub(crate) loaded: Option<Loaded>,
    pub(crate) copy_name: String,
    pub(crate) focus: u16,
    /// The last action's answer, and whether it was a refusal.
    pub(crate) status: String,
    pub(crate) status_error: bool,
    /// The map the player is on (`mp/ffa3`), empty when unknown.
    pub(crate) here: String,
    /// The column is on show (else the player looks about with the path drawn).
    pub(crate) panel_open: bool,
    /// Built by the view, in window pixels: the list, the timeline and the handles'
    /// tracks, for the wheel and drags.
    pub(crate) list: Option<Rect>,
    pub(crate) timeline: Option<Rect>,
    pub(crate) start_track: Option<Rect>,
    pub(crate) end_track: Option<Rect>,
    /// The handle being dragged, along the rectangle the drag began on.
    drag: Option<(Handle, Rect)>,
    /// The path and the strip, drawn under the column.
    pub(crate) overlay: MenuCanvas,
}

/// Whether a list's name is `name` (file names ignore case on Windows).
fn same_name(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// `name` without a trailing `_<digits>`: the copies of `pillars_2` are `pillars_3` on.
fn copy_base(name: &str) -> &str {
    match name.rsplit_once('_') {
        Some((base, digits))
            if !base.is_empty()
                && !digits.is_empty()
                && digits.bytes().all(|b| b.is_ascii_digit()) =>
        {
            base
        }
        _ => name,
    }
}

impl Editor {
    pub(crate) fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            ghosts: Vec::new(),
            row: 0,
            list_first: 0,
            loaded: None,
            copy_name: String::new(),
            focus: GHOST_ROW,
            status: String::new(),
            status_error: false,
            here: String::new(),
            panel_open: false,
            list: None,
            timeline: None,
            start_track: None,
            end_track: None,
            drag: None,
            overlay: MenuCanvas::with_draw_capacity(OVERLAY_DRAWS),
        }
    }

    /// Lay the column out in the SJK UI's look.
    pub(crate) fn build(&mut self, canvas: &mut MenuCanvas, viewport: [f32; 2]) {
        super::editor_view::build(self, canvas, viewport);
    }

    fn answer(&mut self, result: Result<String, String>) {
        (self.status, self.status_error) = match result {
            Ok(line) => (line, false),
            Err(line) => (line, true),
        };
    }

    /// Read the ghosts' folder again: every `.ghost` file's map and length.
    pub(crate) fn refresh_list(&mut self) {
        let mut ghosts: Vec<Listed> = std::fs::read_dir(&self.directory)
            .map(|entries| {
                entries
                    .flatten()
                    .filter_map(|entry| {
                        let file = entry.file_name().to_string_lossy().into_owned();
                        let name = file.strip_suffix(".ghost")?.to_owned();
                        let summary = std::fs::read_to_string(entry.path())
                            .map_err(|error| error.to_string())
                            .and_then(|text| file::summary(&text));
                        Some(Listed { name, summary })
                    })
                    .collect()
            })
            .unwrap_or_default();
        ghosts.sort_by_key(|ghost| ghost.name.to_ascii_lowercase());
        self.ghosts = ghosts;
        if let Some(index) = self.loaded.as_ref().and_then(|loaded| {
            self.ghosts
                .iter()
                .position(|ghost| same_name(&ghost.name, &loaded.name))
        }) {
            self.row = index;
        }
        self.settle_list();
    }

    fn settle_list(&mut self) {
        let count = self.ghosts.len();
        self.row = self.row.min(count.saturating_sub(1));
        if self.row < self.list_first {
            self.list_first = self.row;
        } else if self.row >= self.list_first + GHOST_ROWS {
            self.list_first = self.row + 1 - GHOST_ROWS;
        }
        self.list_first = self.list_first.min(count.saturating_sub(GHOST_ROWS));
    }

    /// Open the list's ghost `index` for editing; a cut not saved is dropped.
    pub(crate) fn choose(&mut self, index: usize) {
        let Some(name) = self.ghosts.get(index).map(|ghost| ghost.name.clone()) else {
            return;
        };
        self.row = index;
        self.settle_list();
        if self
            .loaded
            .as_ref()
            .is_some_and(|loaded| same_name(&loaded.name, &name))
        {
            return;
        }
        let dropped = self
            .loaded
            .as_ref()
            .filter(|loaded| loaded.changed())
            .map(|loaded| loaded.name.clone());
        let path = self.directory.join(format!("{name}.ghost"));
        let run = std::fs::read_to_string(&path)
            .map_err(|error| error.to_string())
            .and_then(|text| Run::from_text(&text));
        match run {
            Ok(run) => {
                self.copy_name = self.free_copy_name(&name);
                self.loaded = Some(Loaded::new(name.clone(), run));
                self.answer(Ok(match dropped {
                    Some(old) => format!("Opened {name}; the cut of {old} was not saved"),
                    None => String::new(),
                }));
            }
            Err(error) => {
                self.loaded = None;
                self.answer(Err(format!("{name}: {error}")));
            }
        }
    }

    /// Open the ghost called `name`.
    pub(crate) fn choose_name(&mut self, name: &str) -> Result<(), String> {
        let name = name.strip_suffix(".ghost").unwrap_or(name);
        let index = self
            .ghosts
            .iter()
            .position(|ghost| same_name(&ghost.name, name))
            .ok_or_else(|| format!("ghost: no ghost called {name}"))?;
        self.choose(index);
        Ok(())
    }

    /// Open something when nothing is: the first ghost of this map, else the first.
    pub(crate) fn choose_default(&mut self) {
        if self.loaded.is_some() || self.ghosts.is_empty() {
            return;
        }
        let here = self.ghosts.iter().position(|ghost| {
            ghost
                .summary
                .as_ref()
                .is_ok_and(|(map, _)| same_name(map, &self.here))
        });
        self.choose(here.unwrap_or(0));
    }

    /// The first name `<base>_2`, `<base>_3`... no ghost has.
    fn free_copy_name(&self, name: &str) -> String {
        let base = copy_base(name);
        (2..)
            .map(|n| format!("{base}_{n}"))
            .find(|candidate| {
                candidate.len() > 64
                    || !self
                        .ghosts
                        .iter()
                        .any(|ghost| same_name(&ghost.name, candidate))
            })
            .filter(|candidate| candidate.len() <= 64)
            .unwrap_or_default()
    }

    fn write(&self, name: &str, run: &Run) -> Result<(), String> {
        std::fs::create_dir_all(&self.directory)
            .and_then(|()| {
                std::fs::write(self.directory.join(format!("{name}.ghost")), run.to_text())
            })
            .map_err(|error| format!("could not save {name}: {error}"))
    }

    /// Write the run as cut under the copy's name, and open the copy.
    fn duplicate(&mut self) {
        let Some(loaded) = &self.loaded else {
            return self.answer(Err("Choose a ghost to duplicate first".into()));
        };
        let name = self.copy_name.trim().to_owned();
        if let Err(error) = file::file_name(&name) {
            self.focus = NAME_FIELD;
            return self.answer(Err(format!("The copy's name: {error}")));
        }
        let name = name.strip_suffix(".ghost").unwrap_or(&name).to_owned();
        if self.directory.join(format!("{name}.ghost")).exists() {
            self.focus = NAME_FIELD;
            return self.answer(Err(format!("{name} exists already: choose another name")));
        }
        let run = loaded.edited();
        let what = if loaded.changed() {
            format!("cut to {:.1} s", run.seconds())
        } else {
            "unchanged".to_owned()
        };
        let original = loaded.name.clone();
        if let Err(error) = self.write(&name, &run) {
            return self.answer(Err(error));
        }
        self.refresh_list();
        let _ = self.choose_name(&name);
        self.answer(Ok(format!("{original} duplicated as {name} ({what})")));
    }

    /// Overwrite the run with its cut, after a second press.
    fn save(&mut self) {
        let Some(loaded) = &mut self.loaded else {
            return self.answer(Err("Choose a ghost first".into()));
        };
        if !loaded.changed() {
            return self.answer(Err(
                "Nothing to save: move a cut, or clear the chat bubble".into()
            ));
        }
        if !loaded.armed {
            loaded.armed = true;
            let name = loaded.name.clone();
            return self.answer(Ok(format!(
                "Save again to overwrite {name}; Duplicate keeps the original"
            )));
        }
        let name = loaded.name.clone();
        let run = loaded.edited();
        if let Err(error) = self.write(&name, &run) {
            return self.answer(Err(error));
        }
        let seconds = run.seconds();
        self.loaded = Some(Loaded::new(name.clone(), run));
        self.refresh_list();
        self.answer(Ok(format!("Saved {name} ({seconds:.1} s)")));
    }

    /// Move the start to command `wanted`, landing as `snap` says.
    fn set_start(&mut self, wanted: usize, snap: Snap) {
        let Some(loaded) = &mut self.loaded else {
            return;
        };
        let limit = loaded.last.saturating_sub(1);
        let wanted = wanted.min(limit);
        let cut = &loaded.cut;
        let index = match snap {
            Snap::Free => Some(wanted),
            Snap::Nearest => {
                let nearest = cut.nearest_clean(wanted);
                if nearest <= limit {
                    Some(nearest)
                } else {
                    cut.clean_from(limit, false)
                }
            }
            Snap::Forward => cut.clean_from(wanted, true).filter(|&index| index <= limit),
            Snap::Back => cut.clean_from(wanted, false),
        };
        match index {
            Some(index) => {
                if index != loaded.first {
                    loaded.first = index;
                    loaded.armed = false;
                }
            }
            None => self.answer(Err(
                "No clean start that way; Shift moves the start anywhere".into(),
            )),
        }
    }

    fn set_end(&mut self, wanted: usize) {
        let Some(loaded) = &mut self.loaded else {
            return;
        };
        let lowest = (loaded.first + 1).min(loaded.cut.len() - 1);
        let index = wanted.clamp(lowest, loaded.cut.len() - 1);
        if index != loaded.last {
            loaded.last = index;
            loaded.armed = false;
        }
    }

    /// Move `handle` by `millis`, or by one command when `fine` (Shift: the start lands
    /// anywhere).
    fn step(&mut self, handle: Handle, millis: i32, fine: bool) {
        let Some(loaded) = &self.loaded else {
            return;
        };
        let at = match handle {
            Handle::Start => loaded.first,
            Handle::End => loaded.last,
        };
        let forward = millis > 0;
        let wanted = if fine {
            if forward {
                at + 1
            } else {
                at.saturating_sub(1)
            }
        } else {
            let index = loaded.cut.index_at(loaded.cut.time(at) + millis);
            if forward {
                index.max(at + 1)
            } else {
                index.min(at.saturating_sub(1))
            }
        };
        match handle {
            Handle::Start => {
                let snap = match (fine, forward) {
                    (true, _) => Snap::Free,
                    (false, true) => Snap::Forward,
                    (false, false) => Snap::Back,
                };
                self.set_start(wanted, snap);
            }
            Handle::End => self.set_end(wanted),
        }
    }

    /// Put `handle` where `x` (window pixels) lies along `rect`.
    fn drag_to(&mut self, handle: Handle, rect: Rect, x: f32, free: bool) {
        let Some(loaded) = &self.loaded else {
            return;
        };
        let ratio = ((x - rect.x) / rect.width.max(1.0)).clamp(0.0, 1.0);
        let time = (ratio * loaded.cut.duration() as f32).round() as i32;
        let index = loaded.cut.index_at(time);
        match handle {
            Handle::Start => self.set_start(index, if free { Snap::Free } else { Snap::Nearest }),
            Handle::End => self.set_end(index),
        }
    }

    fn is_row(token: u16) -> bool {
        (GHOST_ROW..GHOST_ROW + GHOST_ROWS as u16).contains(&token)
    }

    /// Move along the list by `by`, opening the ghost landed on; whether the keyboard was
    /// in the list.
    fn move_in_list(&mut self, by: isize) -> bool {
        if !Self::is_row(self.focus) {
            return false;
        }
        let count = self.ghosts.len();
        let wanted = self.row.saturating_add_signed(by);
        if count == 0 || wanted >= count || (by < 0 && self.row == 0) {
            // Off either end of the list: the keyboard leaves it.
            return false;
        }
        self.choose(wanted);
        self.focus = GHOST_ROW + (self.row - self.list_first) as u16;
        true
    }

    /// Up, Down or Tab: along the list, else the next control in the drawn order.
    pub(crate) fn step_focus(&mut self, canvas: &mut MenuCanvas, forward: bool, along: bool) {
        if along && self.move_in_list(if forward { 1 } else { -1 }) {
            return;
        }
        let direction = if forward {
            AbstractAction::Next
        } else {
            AbstractAction::Previous
        };
        if let Some(token) = canvas.action(direction) {
            self.focus = if Self::is_row(token) {
                GHOST_ROW + self.row.saturating_sub(self.list_first) as u16
            } else {
                token
            };
        }
    }

    /// Enter, Space or a click on `token`.
    pub(crate) fn activate(&mut self, token: u16) -> Option<Request> {
        match token {
            DUPLICATE | NAME_FIELD => self.duplicate(),
            FIRST_MOVE => {
                let found = self
                    .loaded
                    .as_ref()
                    .and_then(|loaded| loaded.cut.first_move(loaded.first));
                match found {
                    Some(index) => self.set_start(index, Snap::Free),
                    None => self.answer(Err("Nothing moves after the start".into())),
                }
            }
            LAST_MOVE => {
                let found = self
                    .loaded
                    .as_ref()
                    .and_then(|loaded| loaded.cut.last_move(loaded.last));
                match found {
                    Some(index) => self.set_end(index),
                    None => self.answer(Err("Nothing moves before the end".into())),
                }
            }
            RESET => {
                if let Some(loaded) = &mut self.loaded {
                    loaded.first = 0;
                    loaded.last = loaded.cut.len() - 1;
                    loaded.clear_talk = true;
                    loaded.armed = false;
                }
            }
            BUBBLES => {
                if let Some(loaded) = &mut self.loaded {
                    loaded.clear_talk = !loaded.clear_talk;
                    loaded.armed = false;
                }
            }
            SAVE => self.save(),
            PLAY => match &self.loaded {
                Some(loaded) if loaded.changed() => {
                    self.answer(Err("Save the cut first: Play plays the saved run".into()));
                }
                Some(loaded) => return Some(Request::Play(loaded.name.clone())),
                None => self.answer(Err("Choose a ghost first".into())),
            },
            LOOK => return Some(Request::Look),
            CLOSE => return Some(Request::Close),
            row if Self::is_row(row) => {
                let index = self.list_first + usize::from(row - GHOST_ROW);
                self.choose(index);
            }
            _ => {}
        }
        None
    }

    /// A key while the column is open (the caller has the modifiers).
    pub(crate) fn key(
        &mut self,
        canvas: &mut MenuCanvas,
        event: &KeyEvent,
        shift: bool,
        control: bool,
    ) -> Option<Request> {
        if event.state != ElementState::Pressed {
            return None;
        }
        let PhysicalKey::Code(key) = event.physical_key else {
            return None;
        };
        let handle = match self.focus {
            START => Some(Handle::Start),
            END => Some(Handle::End),
            _ => None,
        };
        match key {
            KeyCode::Escape if !event.repeat => return Some(Request::Look),
            KeyCode::KeyS if control && !event.repeat => self.save(),
            KeyCode::ArrowUp => self.step_focus(canvas, false, true),
            KeyCode::ArrowDown => self.step_focus(canvas, true, true),
            KeyCode::Tab if !event.repeat => {
                let typing = self.focus == NAME_FIELD;
                self.step_focus(canvas, !shift, !typing);
            }
            KeyCode::ArrowLeft | KeyCode::ArrowRight if handle.is_some() => {
                let sign = if key == KeyCode::ArrowRight { 1 } else { -1 };
                self.step(handle.unwrap_or(Handle::Start), sign * STEP, shift);
            }
            KeyCode::PageUp | KeyCode::PageDown if handle.is_some() => {
                let sign = if key == KeyCode::PageDown { 1 } else { -1 };
                self.step(handle.unwrap_or(Handle::Start), sign * PAGE_STEP, false);
            }
            KeyCode::PageUp => {
                let _ = self.move_in_list(-(GHOST_ROWS as isize));
            }
            KeyCode::PageDown => {
                let _ = self.move_in_list(GHOST_ROWS as isize);
            }
            KeyCode::ArrowLeft | KeyCode::ArrowRight if self.focus == BUBBLES && !event.repeat => {
                if let Some(loaded) = &mut self.loaded {
                    let on = key == KeyCode::ArrowRight;
                    if loaded.clear_talk != on {
                        loaded.clear_talk = on;
                        loaded.armed = false;
                    }
                }
            }
            KeyCode::Enter | KeyCode::NumpadEnter if !event.repeat => {
                return self.activate(self.focus);
            }
            _ => {
                if self.focus == NAME_FIELD {
                    let text = event.text.as_deref();
                    let _ = crate::text_entry::edit(
                        &mut self.copy_name,
                        key,
                        text,
                        shift,
                        control,
                        64,
                        true,
                    );
                } else if key == KeyCode::Space && !event.repeat {
                    return self.activate(self.focus);
                }
            }
        }
        None
    }

    /// The wheel over the list scrolls it; whether it was over it.
    fn scroll(&mut self, position: Vec2, delta: Vec2) -> bool {
        if !self.list.is_some_and(|rect| inside(rect, position)) {
            return false;
        }
        let lines = if delta.y > 0.0 { -2 } else { 2 };
        self.list_first = self
            .list_first
            .saturating_add_signed(lines)
            .min(self.ghosts.len().saturating_sub(GHOST_ROWS));
        true
    }

    /// The handle a press at `position` grabs: on a handle's track, that one; on the
    /// timeline, the nearer.
    fn grab(&self, position: Vec2) -> Option<(Handle, Rect)> {
        let loaded = self.loaded.as_ref()?;
        if let Some(rect) = self.start_track.filter(|rect| inside(*rect, position)) {
            return Some((Handle::Start, rect));
        }
        if let Some(rect) = self.end_track.filter(|rect| inside(*rect, position)) {
            return Some((Handle::End, rect));
        }
        let rect = self.timeline.filter(|rect| inside(*rect, position))?;
        let duration = loaded.cut.duration().max(1) as f32;
        let x_of = |index: usize| rect.x + rect.width * loaded.cut.time(index) as f32 / duration;
        let to_start = (position.x - x_of(loaded.first)).abs();
        let to_end = (position.x - x_of(loaded.last)).abs();
        Some((
            if to_end < to_start {
                Handle::End
            } else {
                Handle::Start
            },
            rect,
        ))
    }

    /// A pointer event on the column (`shift`: the start lands anywhere).
    pub(crate) fn pointer(
        &mut self,
        canvas: &mut MenuCanvas,
        event: InputEvent,
        shift: bool,
    ) -> Option<Request> {
        match event {
            InputEvent::PointerWheel { position, delta } if self.scroll(position, delta) => {
                return None;
            }
            InputEvent::PointerPress {
                position,
                button: PointerButton::Primary,
            } => {
                if let Some((handle, rect)) = self.grab(position) {
                    self.drag = Some((handle, rect));
                    self.focus = match handle {
                        Handle::Start => START,
                        Handle::End => END,
                    };
                    self.drag_to(handle, rect, position.x, shift);
                    return None;
                }
            }
            InputEvent::PointerMove(position) => {
                if let Some((handle, rect)) = self.drag {
                    self.drag_to(handle, rect, position.x, shift);
                    return None;
                }
            }
            InputEvent::PointerRelease { .. } if self.drag.take().is_some() => return None,
            _ => {}
        }
        let event = canvas.pointer(event)?;
        let token = event.token?;
        if matches!(event.kind, UiEventKind::Hover | UiEventKind::HoverEnter) {
            // Hovering a row does not open it; a click does.
            if !Self::is_row(token) {
                self.focus = token;
            }
            return None;
        }
        if event.kind != UiEventKind::Activate {
            return None;
        }
        self.focus = token;
        self.activate(token)
    }
}

fn inside(rect: Rect, position: Vec2) -> bool {
    position.x >= rect.x
        && position.x <= rect.right()
        && position.y >= rect.y
        && position.y <= rect.bottom()
}

impl GpuState {
    /// Whether the editor's column is on show.
    pub(crate) fn ghost_panel_open(&self) -> bool {
        self.game_menu && self.game_menu_page == Page::GhostEdit
    }

    /// Whether the editor is on with its column hidden: Escape brings it back.
    pub(crate) fn ghost_editor_looking(&self) -> bool {
        self.ghost.editor.is_some() && !self.ghost_panel_open()
    }

    /// `ghost edit [name]`: turn the editor on (or bring its column back), on `name`.
    pub(crate) fn open_ghost_editor(&mut self, name: Option<&str>) -> Result<Vec<String>, String> {
        if self.live_session.is_none() && !self.resident.exploring() {
            return Err("ghost: the editor opens in a game".into());
        }
        let directory = self
            .console
            .as_ref()
            .ok_or("Console unavailable")?
            .config_directory()
            .to_owned();
        let here = self.ghost_editor_map();
        let editor = self
            .ghost
            .editor
            .get_or_insert_with(|| Box::new(Editor::new(ghost_directory(&directory))));
        editor.here = here;
        editor.refresh_list();
        match name {
            Some(name) => editor.choose_name(name)?,
            None => editor.choose_default(),
        }
        // The keyboard starts on the open ghost.
        if Editor::is_row(editor.focus) {
            editor.focus = GHOST_ROW + editor.row.saturating_sub(editor.list_first) as u16;
        }
        self.open_ghost_panel();
        Ok(Vec::new())
    }

    /// The map the player is on, as runs name it (`mp/ffa3`).
    fn ghost_editor_map(&self) -> String {
        self.live_session
            .as_ref()
            .and_then(|session| super::current_map(session.game_state()))
            .unwrap_or_else(|| {
                crate::remap_blocked_maps::map_name(&self.world_load_map)
                    .trim_end_matches(".bsp")
                    .to_ascii_lowercase()
            })
    }

    /// Show the column over the live map, the pointer freed for it.
    pub(crate) fn open_ghost_panel(&mut self) {
        let Some(editor) = self.ghost.editor.as_deref_mut() else {
            return;
        };
        editor.panel_open = true;
        if editor.focus == 0 {
            editor.focus = GHOST_ROW;
        }
        self.gameplay_input.release_keys();
        self.game_menu = true;
        self.game_menu_page = Page::GhostEdit;
        self.release_pointer();
    }

    /// Hide the column to move about; the path stays drawn.
    fn ghost_editor_look(&mut self) {
        if let Some(editor) = self.ghost.editor.as_deref_mut() {
            editor.panel_open = false;
            editor.drag = None;
        }
        if self.ghost_panel_open() {
            self.game_menu = false;
            self.game_menu_page = Page::Main;
            self.capture_pointer();
        }
    }

    /// Turn the editor off.
    pub(crate) fn close_ghost_editor(&mut self) {
        if self.ghost_panel_open() {
            self.game_menu = false;
            self.game_menu_page = Page::Main;
            self.capture_pointer();
        }
        self.ghost.editor = None;
    }

    /// A key while the column is open: all of them are its own. False when it is not.
    pub(crate) fn ghost_edit_key(&mut self, event: &KeyEvent) -> bool {
        if !self.ghost_panel_open() {
            return false;
        }
        let (shift, control) = self.console.as_ref().map_or((false, false), |console| {
            (console.shift_held(), console.control_held())
        });
        let request = match self.ghost.editor.as_deref_mut() {
            Some(editor) => editor.key(self.in_game_menu.canvas_mut(), event, shift, control),
            None => Some(Request::Close),
        };
        if let Some(request) = request {
            self.apply_ghost_request(request);
        }
        true
    }

    /// A pointer event while the column is open.
    pub(crate) fn ghost_edit_pointer(&mut self, event: InputEvent) {
        let shift = self
            .console
            .as_ref()
            .is_some_and(|console| console.shift_held());
        let request = match self.ghost.editor.as_deref_mut() {
            Some(editor) => editor.pointer(self.in_game_menu.canvas_mut(), event, shift),
            None => Some(Request::Close),
        };
        if let Some(request) = request {
            self.apply_ghost_request(request);
        }
    }

    fn apply_ghost_request(&mut self, request: Request) {
        match request {
            Request::Look => self.ghost_editor_look(),
            Request::Close => self.close_ghost_editor(),
            Request::Play(name) => match self.ghost_command(&["play".to_owned(), name]) {
                Ok(lines) => {
                    self.close_ghost_editor();
                    if let Some(console) = &mut self.console {
                        for line in lines {
                            console.push_log(line);
                        }
                    }
                }
                Err(error) => {
                    if let Some(editor) = self.ghost.editor.as_deref_mut() {
                        editor.answer(Err(error));
                    }
                }
            },
        }
    }

    /// The path under the column (or, while looking about, over play with the strip);
    /// whether anything was drawn.
    pub(crate) fn append_ghost_overlay(&mut self, viewport: [f32; 2]) -> bool {
        let Some(editor) = self.ghost.editor.as_deref_mut() else {
            return false;
        };
        let camera = self.frame_view;
        super::editor_view::build_overlay(editor, camera, viewport);
        crate::ingame_menu::sjk_view::text_target(
            &mut self.game_fonts,
            &mut self.text_vertices,
            &self.ui_font,
        )
        .append(&editor.overlay, viewport);
        true
    }
}

/// The world shots (`world_shot_ghost_editor.rs`).
#[cfg(test)]
impl GpuState {
    /// Write a run on `map` into `directory` (a second of standing with the console open,
    /// then a curving run from `origin` along `yaw` degrees, then a stop), two more ghosts
    /// beside it, and open the editor on it.
    pub(crate) fn shot_ghost_editor(
        &mut self,
        directory: PathBuf,
        map: &str,
        origin: [f32; 3],
        yaw: f32,
    ) {
        use super::{Frame, Start};
        let talk = sjk_game_jka::pmove_talk::BUTTON_TALK;
        let still = |time: i32, at: [f32; 3], buttons: u16| Frame {
            time,
            view: [0, (yaw / 360.0 * 65_536.0) as i32 & 0xFFFF, 0],
            forward: 0,
            right: 0,
            up: 0,
            buttons,
            weapon: 3,
            force_selection: 0,
            inventory_selection: 255,
            generic_command: 0,
            origin: at,
            velocity: [0.0; 3],
            ground: true,
            confirmed: true,
        };
        let mut frames: Vec<Frame> = (0..125).map(|step| still(step * 8, origin, talk)).collect();
        let mut at = origin;
        for step in 0..250 {
            let time = 1_000 + step * 8;
            let heading = (yaw + 50.0 * (step as f32 / 80.0).sin()).to_radians();
            let velocity = [heading.cos() * 250.0, heading.sin() * 250.0, 0.0];
            at = [
                at[0] + velocity[0] * 0.008,
                at[1] + velocity[1] * 0.008,
                at[2],
            ];
            // A jump in the middle: a quarter-second arc.
            let air = (100..130).contains(&step);
            let lift = if air {
                ((step - 100) as f32 / 30.0 * std::f32::consts::PI).sin() * 40.0
            } else {
                0.0
            };
            frames.push(Frame {
                forward: 127,
                origin: [at[0], at[1], at[2] + lift],
                velocity,
                ground: !air,
                ..still(time, at, 0)
            });
        }
        for step in 0..125 {
            frames.push(still(
                3_000 + step * 8,
                at,
                if step > 60 { talk } else { 0 },
            ));
        }
        let run = Run {
            map: map.to_owned(),
            start: Start {
                origin,
                view: frames[0].view,
                velocity: [0.0; 3],
                weapon: 3,
            },
            frames,
        };
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("the shot's ghost folder");
        let mut editor = Editor::new(directory);
        editor.write("pillars", &run).expect("a ghost");
        let short = Run {
            frames: run.frames[..400].to_vec(),
            ..run.clone()
        };
        editor.write("pillars_2", &short).expect("a ghost");
        let other = Run {
            map: "mp/ffa3".to_owned(),
            ..short
        };
        editor.write("ffa3_warmup", &other).expect("a ghost");
        editor.here = map.to_owned();
        editor.refresh_list();
        editor.choose_name("pillars").expect("the run");
        editor.focus = GHOST_ROW + editor.row as u16;
        self.ghost.showcase_for_shot = true;
        self.ghost.editor = Some(Box::new(editor));
        self.open_ghost_panel();
    }

    /// The world shots: cut the open run at its first and last moves, the start focused.
    pub(crate) fn shot_ghost_editor_cut(&mut self) {
        if let Some(editor) = self.ghost.editor.as_deref_mut() {
            let _ = editor.activate(FIRST_MOVE);
            let _ = editor.activate(LAST_MOVE);
            editor.focus = START;
        }
    }

    /// The world shots: hide the column, the path still drawn.
    pub(crate) fn shot_ghost_editor_look(&mut self) {
        self.ghost_editor_look();
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    fn frame(time: i32, forward: i8, speed: f32, ground: bool) -> super::super::Frame {
        super::super::Frame {
            time,
            view: [0; 3],
            forward,
            right: 0,
            up: 0,
            buttons: if time < 16 {
                sjk_game_jka::pmove_talk::BUTTON_TALK
            } else {
                0
            },
            weapon: 3,
            force_selection: 0,
            inventory_selection: 255,
            generic_command: 0,
            origin: [time as f32, 0.0, 24.0],
            velocity: [speed, 0.0, 0.0],
            ground,
            confirmed: true,
        }
    }

    /// 2 s of commands: standing with the console open, then running from 1 s, then
    /// standing again from 1.6 s.
    fn run() -> Run {
        Run {
            map: "mp/ffa3".into(),
            start: super::super::Start::default(),
            frames: (0..250)
                .map(|step| {
                    let time = step * 8;
                    let moving = (1_000..1_600).contains(&time);
                    frame(
                        time,
                        if moving { 127 } else { 0 },
                        if moving { 250.0 } else { 0.0 },
                        true,
                    )
                })
                .collect(),
        }
    }

    pub(crate) fn editor_with(directory: PathBuf) -> Editor {
        let mut editor = Editor::new(directory);
        editor.ghosts = vec![
            Listed {
                name: "pillars".into(),
                summary: Ok(("mp/ffa3".into(), 1_992)),
            },
            Listed {
                name: "pillars_2".into(),
                summary: Ok(("mp/ffa3".into(), 1_200)),
            },
        ];
        editor.loaded = Some(Loaded::new("pillars".into(), run()));
        editor.copy_name = editor.free_copy_name("pillars");
        editor
    }

    fn loaded(editor: &Editor) -> &Loaded {
        editor.loaded.as_ref().expect("a run")
    }

    #[test]
    fn copies_take_the_next_free_number() {
        let editor = editor_with(PathBuf::new());
        assert_eq!(editor.copy_name, "pillars_3");
        assert_eq!(editor.free_copy_name("pillars_2"), "pillars_3");
        assert_eq!(editor.free_copy_name("run"), "run_2");
        assert_eq!(copy_base("a_b"), "a_b");
        assert_eq!(copy_base("_2"), "_2");
    }

    #[test]
    fn keys_move_the_start_from_clean_start_to_clean_start() {
        let mut editor = editor_with(PathBuf::new());
        // Standing still until 1 s: a step forward is a tenth of a second.
        editor.step(Handle::Start, STEP, false);
        assert_eq!(loaded(&editor).cut.time(loaded(&editor).first), 96);
        // From 0.9 s, the next clean start is 1 s (the run starts there); from 1 s, the
        // next is after the run stops, 1.6 s and a command.
        editor.set_start(loaded(&editor).cut.index_at(904), Snap::Free);
        editor.step(Handle::Start, STEP, false);
        assert_eq!(loaded(&editor).cut.time(loaded(&editor).first), 1_000);
        editor.step(Handle::Start, STEP, false);
        assert_eq!(loaded(&editor).cut.time(loaded(&editor).first), 1_608);
        // Back to the end of the standing before the run.
        editor.step(Handle::Start, -STEP, false);
        assert_eq!(loaded(&editor).cut.time(loaded(&editor).first), 1_000);
        // Shift: one command, anywhere.
        editor.step(Handle::Start, STEP, true);
        assert_eq!(loaded(&editor).cut.time(loaded(&editor).first), 1_008);
        assert!(!loaded(&editor).cut.is_clean(loaded(&editor).first));
    }

    #[test]
    fn a_drag_snaps_the_start_unless_shift_is_held() {
        let mut editor = editor_with(PathBuf::new());
        let rect = Rect::new(0.0, 0.0, 1_992.0, 10.0);
        // 1.3 s is mid-run: the nearest clean start is 1 s.
        editor.drag_to(Handle::Start, rect, 1_300.0, false);
        assert_eq!(loaded(&editor).cut.time(loaded(&editor).first), 1_000);
        editor.drag_to(Handle::Start, rect, 1_300.0, true);
        assert_eq!(loaded(&editor).cut.time(loaded(&editor).first), 1_296);
        // The end never goes before the start.
        editor.drag_to(Handle::End, rect, 100.0, false);
        assert_eq!(loaded(&editor).last, loaded(&editor).first + 1);
    }

    #[test]
    fn first_and_last_move_cut_both_waits_and_save_asks_twice() {
        let directory =
            std::env::temp_dir().join(format!("sjk-ghost-editor-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        let mut editor = editor_with(directory.clone());
        assert!(
            loaded(&editor).changed(),
            "the bubble is cleared by default"
        );
        let _ = editor.activate(FIRST_MOVE);
        let _ = editor.activate(LAST_MOVE);
        let (first, last) = (loaded(&editor).first, loaded(&editor).last);
        assert_eq!(loaded(&editor).cut.time(first), 1_000);
        assert_eq!(loaded(&editor).cut.time(last), 1_600);
        assert_eq!(loaded(&editor).kept(), 600);
        // Play refuses a cut not saved.
        assert_eq!(editor.activate(PLAY), None);
        assert!(editor.status_error);
        // Duplicate writes the cut under the copy's name and opens it.
        let _ = editor.activate(DUPLICATE);
        assert!(!editor.status_error, "{}", editor.status);
        assert_eq!(loaded(&editor).name, "pillars_3");
        assert_eq!(loaded(&editor).cut.len(), 76);
        assert!(!loaded(&editor).changed());
        assert_eq!(
            editor.activate(PLAY),
            Some(Request::Play("pillars_3".into()))
        );
        // Save asks again before it overwrites.
        editor.step(Handle::End, -STEP, false);
        let _ = editor.activate(SAVE);
        assert_eq!(loaded(&editor).cut.len(), 76, "not yet");
        let _ = editor.activate(SAVE);
        assert!(!editor.status_error, "{}", editor.status);
        assert_eq!(loaded(&editor).cut.len(), 63);
        let text = std::fs::read_to_string(directory.join("pillars_3.ghost")).unwrap();
        let saved = Run::from_text(&text).unwrap();
        assert_eq!(saved.frames.len(), 63);
        assert_eq!(saved.frames[0].time, 0);
        assert!(
            saved
                .frames
                .iter()
                .all(|frame| frame.buttons & sjk_game_jka::pmove_talk::BUTTON_TALK == 0)
        );
        // An existing name is refused.
        editor.copy_name = "pillars_3".into();
        let _ = editor.activate(DUPLICATE);
        assert!(editor.status_error);
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn the_bubble_switch_decides_whether_the_balloon_stays() {
        let mut editor = editor_with(PathBuf::new());
        assert_eq!(loaded(&editor).bubbles(), 1);
        let _ = editor.activate(BUBBLES);
        assert!(!loaded(&editor).changed());
        assert!(loaded(&editor).edited().frames[0].buttons != 0);
        let _ = editor.activate(BUBBLES);
        assert_eq!(loaded(&editor).edited().frames[0].buttons, 0);
    }
}
