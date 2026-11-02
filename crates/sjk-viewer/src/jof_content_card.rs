//! The content card, for JoF content (`jof.downloads`, Settings > General or Mods >
//! JoF content) and the Sith cloud (`sith.downloads`, `sith_cloud.rs`), each with its
//! own [`Words`]: an SJK UI pop-up in every menu style. The switch that loads the
//! folder, the repository's groups with a tick each and what is here of them,
//! each group's files to pick one by one, what a download will do, its
//! progress, and Check again and Download.
//!
//! The job runs on its own thread ([`sjk_jof_content::job`]); a download goes
//! on while the card is closed, and the card shows it again when reopened.
//!
//! Automatic updates (opt in, `docs/jof-content.md`): the second switch and an
//! Auto tick on each group say which groups the client keeps up to date by
//! itself, [`AUTO_FIRST`] after the start and then every [`AUTO_EVERY`], with the
//! card's own job ([`Card::auto_due`], [`Card::start_auto`]), so one job at a
//! time runs per collection.

use crate::menu::sjk::{Frame, TextTarget, color, kit, text};
use crate::menu_widgets::{BACK_TOKEN, MenuCanvas, TextFamily};
use sjk_jof_content::job::{Job, Outcome, Phase, Settings};
use sjk_jof_content::{AssetState, AutoGroups, Plan, Selection, Snapshot, Target, human_size};
use sjk_ui::{Color, DrawCommand, FontWeight, InputEvent, TextAlign, UiEventKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// Group rows shown at once.
const ROWS: usize = 6;
const ROW_HEIGHT: f32 = 62.0;
/// File rows shown at once, under the group's own row.
const FILE_ROWS: usize = 7;
const FILE_ROW_HEIGHT: f32 = 46.0;
const SWITCH_TOKEN: u16 = 800;
const CHECK_TOKEN: u16 = 801;
const MAIN_TOKEN: u16 = 802;
/// The "Groups" way back over a group's files.
const GROUPS_TOKEN: u16 = 803;
/// A group's own row over its files.
const HEADER_TOKEN: u16 = 804;
/// The automatic updates' switch.
const AUTO_TOKEN: u16 = 805;
/// The open group's Auto tick, over its files.
const HEADER_AUTO_TOKEN: u16 = 806;
/// A group row's tick box, by slot.
const TICK_BASE: u16 = 100;
/// A group row's Auto tick, by slot.
const AUTO_TICK_BASE: u16 = 200;
/// The first automatic check after the start (the client is up, the hub can be
/// asked), and then how often.
pub(crate) const AUTO_FIRST: Duration = Duration::from_secs(30);
pub(crate) const AUTO_EVERY: Duration = Duration::from_secs(60 * 60);
/// Whose the files are (Sol's request, 16/10/2026): JoF chooses them, SJK
/// only fetches what the player picks.
pub(crate) const NOTICE: &str = "JoF chooses and publishes these files and is responsible for them; SJK only downloads what you pick and is not responsible for their content.";
/// What a card says about its collection: the only part that differs between
/// JoF content and the Sith cloud.
pub(crate) struct Words {
    pub(crate) title: &'static str,
    pub(crate) about: &'static str,
    /// Whose the files are.
    pub(crate) notice: &'static str,
    /// The switch's name, also in the hints to turn it on.
    pub(crate) switch: &'static str,
    pub(crate) on: &'static str,
    pub(crate) off: &'static str,
    /// After the folder, on the line under the switch.
    pub(crate) folder_note: &'static str,
    /// Where the list comes from, as a sentence's subject.
    pub(crate) repository: &'static str,
    /// The automatic updates' switch's line, when on.
    pub(crate) auto_on: &'static str,
}

/// The automatic updates' switch, the same for every collection.
const AUTO_SWITCH: &str = "Update automatically";
/// Its line while off.
const AUTO_OFF: &str = "Off by default. On: SJK checks at start and every hour, and downloads only the groups marked Auto.";

/// A collection's settings: the same code reads both collections through
/// these ([`crate::jof_content::NAMES`], [`crate::sith_cloud::NAMES`]).
pub(crate) struct Names {
    /// On and off (`jof.content`).
    pub(crate) switch: &'static str,
    /// The groups chosen ([`Selection`]).
    pub(crate) groups: &'static str,
    /// Single files picked against their group.
    pub(crate) files: &'static str,
    /// Automatic updates on and off.
    pub(crate) auto: &'static str,
    /// The groups updated automatically ([`AutoGroups`]).
    pub(crate) auto_groups: &'static str,
}

/// JoF content's words.
pub(crate) const JOF: Words = Words {
    title: "JoF content",
    about: "JoF's maps, skins, hilts and packs, from JoF's repository as JoF's launcher installs them.",
    notice: NOTICE,
    switch: "Use JoF content",
    on: "On: SJK loads these files from its JoF folder, from the next map. Off leaves them on disk unused.",
    off: "Off by default. Turn it on to download and load JoF's files; nothing is fetched until you press Download.",
    folder_note: "files JoF's launcher put in your game folder are used as they are",
    repository: "JoF's repository",
    auto_on: "On: checks at start and every hour, also in a match; new files of the groups marked Auto load from the next map.",
};

/// The card, in frame pixels.
const CARD: [f32; 4] = [360.0, 96.0, 1200.0, 888.0];
const LEFT: f32 = 404.0;
const RIGHT: f32 = 1516.0;
/// The automatic updates' row, under the switch's.
const AUTO_TOP: f32 = 338.0;
const AUTO_HEIGHT: f32 = 60.0;
/// The list's heading, and its first row.
const HEADING: f32 = 412.0;
const GROUPS_TOP: f32 = HEADING + 26.0;
/// A group row's Auto tick, from the left.
const AUTO_X: f32 = LEFT + 500.0;

/// What the console does after an input.
pub(crate) enum Action {
    None,
    Close,
    /// Store `jof.content`.
    Switch(bool),
    /// Store `jof.contentAuto`.
    Auto(bool),
    /// Store `jof.contentAutoGroups`.
    AutoGroups(String),
    /// Store `jof.contentGroups` and `jof.contentFiles`.
    Choice {
        groups: String,
        files: String,
    },
    /// Start a refresh with the settings.
    Check,
    /// Start a sync with the settings.
    Download,
}

/// One group's line, from the latest snapshot and the choice.
#[derive(Clone, Copy, Default)]
struct Summary {
    /// Files SJK would fetch, and how many of them are here.
    files: usize,
    here: usize,
    size: u64,
    /// Files wanted.
    wanted: usize,
    /// Bytes of wanted files not here yet.
    missing: u64,
    /// Bytes of SJK's own copies of files not wanted.
    ours: u64,
    /// Files only for JoF's own client.
    elsewhere: usize,
}

pub(crate) struct Card {
    words: &'static Words,
    pub(crate) open: bool,
    pub(crate) owns_console: bool,
    game_data: PathBuf,
    job: Job,
    on: bool,
    selection: Selection,
    /// The automatic updates' switch and groups.
    auto: bool,
    auto_groups: AutoGroups,
    /// When the next automatic check is due, and its time on the clock (24 h).
    next_check: Option<Instant>,
    next_clock: Option<(u8, u8)>,
    /// The job running or last run is an automatic update not yet reported.
    auto_running: bool,
    folder: String,
    /// The group whose files are shown, or None for the groups.
    view: Option<usize>,
    /// In the groups: 0 the switch, 1 the automatic switch, then the groups. In
    /// a group: 0 the group's own row, then its files. Then Check again and the
    /// main button.
    focus: usize,
    first: usize,
    /// The groups' focus and scroll, given back when a group's files close.
    back: (usize, usize),
    /// The snapshot and selection the plan and summaries were made from.
    made_from: Option<(usize, Selection)>,
    snapshot: Option<Arc<Snapshot>>,
    plan: Plan,
    summaries: Vec<Summary>,
    phase: Phase,
    ui: MenuCanvas,
}

impl Default for Card {
    fn default() -> Self {
        Self::new(&JOF)
    }
}

impl Card {
    pub(crate) fn new(words: &'static Words) -> Self {
        Self {
            words,
            open: false,
            owns_console: false,
            game_data: PathBuf::new(),
            job: Job::default(),
            on: false,
            selection: Selection::default(),
            auto: false,
            auto_groups: AutoGroups::default(),
            next_check: None,
            next_clock: None,
            auto_running: false,
            folder: String::new(),
            view: None,
            focus: 0,
            first: 0,
            back: (0, 0),
            made_from: None,
            snapshot: None,
            plan: Plan::default(),
            summaries: Vec::new(),
            phase: Phase::Idle,
            ui: MenuCanvas::with_text_capacity(128),
        }
    }

    /// Show the card with the settings' values. The returned flag asks for a
    /// refresh (none is running and nothing was looked at yet).
    pub(crate) fn open(
        &mut self,
        game_data: &Path,
        on: bool,
        selection: Selection,
        folder: &Path,
    ) -> bool {
        self.game_data = game_data.to_path_buf();
        self.on = on;
        self.selection = selection;
        self.folder = folder.display().to_string();
        self.open = true;
        self.view = None;
        self.focus = 0;
        self.first = 0;
        self.update();
        !self.job.busy() && self.snapshot.is_none()
    }

    pub(crate) fn game_data(&self) -> &Path {
        &self.game_data
    }

    /// The automatic updates' settings, as the card shows them.
    pub(crate) fn set_auto(&mut self, auto: bool, groups: AutoGroups) {
        self.auto = auto;
        self.auto_groups = groups;
    }

    /// Whether the automatic check is due at `now`; when it is, the next one is
    /// set an hour on. The first call sets the first check [`AUTO_FIRST`] on.
    /// Called every frame: a comparison while nothing is due.
    pub(crate) fn auto_due(&mut self, now: Instant) -> bool {
        match self.next_check {
            Some(next) if now < next => false,
            Some(_) => {
                self.schedule(now, AUTO_EVERY);
                true
            }
            None => {
                self.schedule(now, AUTO_FIRST);
                false
            }
        }
    }

    /// Set the next automatic check `after` from `now`.
    pub(crate) fn schedule(&mut self, now: Instant, after: Duration) {
        self.next_check = Some(now + after);
        let time = sjk_shell::local_time::LocalTime::now();
        self.next_clock = Some(clock_after(time.hour, time.minute, time.second, after));
    }

    /// Start an automatic update; false when a job is at work already.
    pub(crate) fn start_auto(
        &mut self,
        settings: Settings,
        selection: Selection,
        auto: AutoGroups,
    ) -> bool {
        let started = self.job.auto(settings, selection, auto);
        self.auto_running |= started;
        started
    }

    /// An automatic update that just ended: its outcome, once, when it
    /// finished (stopped or failed ones come back as None, after a log line).
    /// A flag test while none runs.
    pub(crate) fn auto_finished(&mut self) -> Option<Outcome> {
        if !self.auto_running || self.job.busy() {
            return None;
        }
        self.auto_running = false;
        match self.job.progress().phase {
            Phase::Finished(outcome) if outcome.automatic => Some(outcome),
            Phase::Failed(reason) => {
                crate::log::progress(format_args!(
                    "{}: automatic update skipped ({reason}); next try in an hour",
                    self.words.title
                ));
                None
            }
            _ => None,
        }
    }

    pub(crate) fn title(&self) -> &'static str {
        self.words.title
    }

    /// The player's own look; the job is no longer an automatic update's.
    pub(crate) fn refresh(&mut self, settings: Settings) {
        if self.job.refresh(settings) {
            self.auto_running = false;
        }
    }

    pub(crate) fn download(&mut self, settings: Settings) {
        if self.job.sync(settings, self.selection.clone()) {
            self.auto_running = false;
        }
    }

    /// Whether a refresh or download is still at work.
    pub(crate) fn busy(&self) -> bool {
        self.job.busy()
    }

    pub(crate) fn draw_list(&self) -> &sjk_ui::DrawList {
        self.ui.draw_list()
    }

    /// Rows of the list on show: the groups, or the open group's files.
    fn items(&self) -> usize {
        let Some(snapshot) = &self.snapshot else {
            return 0;
        };
        match self.view {
            None => snapshot.manifest.groups.len(),
            Some(group) => snapshot
                .manifest
                .groups
                .get(group)
                .map_or(0, |group| group.assets.len()),
        }
    }

    fn rows(&self) -> usize {
        if self.view.is_some() { FILE_ROWS } else { ROWS }
    }

    /// The focus of the list's first row: after the two switches over the
    /// groups, after the group's own row over its files.
    fn head(&self) -> usize {
        if self.view.is_some() { 1 } else { 2 }
    }

    fn check_focus(&self) -> usize {
        self.items() + self.head()
    }

    fn main_focus(&self) -> usize {
        self.items() + self.head() + 1
    }

    /// The list row under the focus, if it is on one.
    fn focused_row(&self) -> Option<usize> {
        let head = self.head();
        (head..head + self.items())
            .contains(&self.focus)
            .then(|| self.focus - head)
    }

    /// Take the job's latest state; the plan is made again only when the
    /// snapshot or the choice changed.
    fn update(&mut self) {
        let progress = self.job.progress();
        self.phase = progress.phase;
        if let Some(snapshot) = progress.snapshot {
            self.snapshot = Some(snapshot);
        }
        let Some(snapshot) = self.snapshot.clone() else {
            return;
        };
        let key = (Arc::as_ptr(&snapshot) as usize, self.selection.clone());
        if self.made_from.as_ref() == Some(&key) {
            return;
        }
        self.plan = Plan::new(&snapshot, &self.selection);
        self.summaries = summaries(&snapshot, &self.selection);
        self.made_from = Some(key);
        if self
            .view
            .is_some_and(|group| group >= snapshot.manifest.groups.len())
        {
            self.view = None;
        }
        self.focus = self.focus.min(self.main_focus());
    }

    fn set_focus(&mut self, focus: usize) {
        self.focus = focus.min(self.main_focus());
        let rows = self.rows();
        if let Some(row) = self.focused_row() {
            if row < self.first {
                self.first = row;
            }
            if row >= self.first + rows {
                self.first = row + 1 - rows;
            }
        }
    }

    fn running(&self) -> bool {
        self.phase.running() || self.job.busy()
    }

    /// What the main button does now, its label and whether it can.
    fn main_button(&self) -> (String, bool) {
        if self.running() {
            return ("Stop".into(), true);
        }
        if self.snapshot.is_none() {
            return ("Download".into(), false);
        }
        if !self.on {
            return ("Download".into(), false);
        }
        if !self.plan.downloads.is_empty() {
            return (
                format!("Download {}", human_size(self.plan.download_bytes)),
                true,
            );
        }
        if !self.plan.removals.is_empty() {
            return (
                format!("Remove {}", human_size(self.plan.removal_bytes)),
                true,
            );
        }
        ("Up to date".into(), false)
    }

    /// Enter: switch, open a group, tick a file, or press a button.
    fn activate(&mut self) -> Action {
        match (self.focus, self.view, self.focused_row()) {
            (0, None, _) => {
                self.on = !self.on;
                Action::Switch(self.on)
            }
            (1, None, _) => self.toggle_auto(),
            (0, Some(group), _) => self.toggle_group(group),
            (_, None, Some(row)) => {
                self.open_group(row);
                Action::None
            }
            (_, Some(group), Some(row)) => self.toggle_file(group, row),
            (focus, _, _) if focus == self.check_focus() => self.check(),
            _ => self.main(),
        }
    }

    /// Space: tick the focused group or file; elsewhere as Enter.
    fn tick(&mut self) -> Action {
        match (self.view, self.focused_row()) {
            (None, Some(row)) => self.toggle_group(row),
            _ => self.activate(),
        }
    }

    fn toggle_auto(&mut self) -> Action {
        self.auto = !self.auto;
        Action::Auto(self.auto)
    }

    /// A: mark or unmark the focused group (or the open one) as Auto.
    fn auto_key(&mut self) -> Action {
        match (self.view, self.focused_row()) {
            (Some(group), _) | (None, Some(group)) => self.toggle_auto_group(group),
            _ => Action::None,
        }
    }

    /// Whether the group at `index` shows its Auto tick ticked.
    #[cfg(test)]
    fn auto_group_marked(&self, index: usize) -> bool {
        self.snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.manifest.groups.get(index))
            .is_some_and(|group| self.auto_groups.includes(&self.selection, group))
    }

    fn toggle_auto_group(&mut self, index: usize) -> Action {
        let Some(snapshot) = self.snapshot.clone() else {
            return Action::None;
        };
        let Some(group) = snapshot.manifest.groups.get(index) else {
            return Action::None;
        };
        if group.size() == 0 {
            return Action::None;
        }
        self.auto_groups
            .toggle(&self.selection, &snapshot.manifest, group.id);
        Action::AutoGroups(self.auto_groups.saved())
    }

    /// Show one group's files.
    fn open_group(&mut self, index: usize) {
        let Some(group) = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.manifest.groups.get(index))
        else {
            return;
        };
        if group.assets.is_empty() {
            return;
        }
        self.back = (self.focus, self.first);
        self.view = Some(index);
        self.focus = self.head();
        self.first = 0;
    }

    /// Back to the groups, where the player was.
    fn close_group(&mut self) {
        if self.view.take().is_some() {
            (self.focus, self.first) = self.back;
        }
    }

    fn choice(&self) -> Action {
        Action::Choice {
            groups: self.selection.saved(),
            files: self.selection.saved_files(),
        }
    }

    fn toggle_group(&mut self, index: usize) -> Action {
        let Some(snapshot) = self.snapshot.clone() else {
            return Action::None;
        };
        let Some(group) = snapshot.manifest.groups.get(index) else {
            return Action::None;
        };
        if group.size() == 0 {
            // Only files for JoF's own client: nothing to choose.
            return Action::None;
        }
        self.selection.toggle(&snapshot.manifest, group.id);
        self.update();
        self.choice()
    }

    fn toggle_file(&mut self, group: usize, asset: usize) -> Action {
        let Some(snapshot) = self.snapshot.clone() else {
            return Action::None;
        };
        let Some(group) = snapshot.manifest.groups.get(group) else {
            return Action::None;
        };
        let Some(asset) = group.assets.get(asset) else {
            return Action::None;
        };
        if asset.target != Target::Base {
            return Action::None;
        }
        self.selection.toggle_file(group, asset);
        self.update();
        self.choice()
    }

    fn check(&mut self) -> Action {
        if self.running() {
            Action::None
        } else {
            Action::Check
        }
    }

    fn main(&mut self) -> Action {
        if self.running() {
            self.job.cancel();
            return Action::None;
        }
        if self.main_button().1 {
            Action::Download
        } else {
            Action::None
        }
    }

    pub(crate) fn key(&mut self, event: &KeyEvent) -> Action {
        if event.state != ElementState::Pressed || event.repeat {
            return Action::None;
        }
        let PhysicalKey::Code(key) = event.physical_key else {
            return Action::None;
        };
        self.key_code(key)
    }

    /// One key pressed.
    fn key_code(&mut self, key: KeyCode) -> Action {
        self.update();
        let items = self.items();
        let rows = self.rows();
        let head = self.head();
        let row = self.focused_row();
        let last = (items + head).saturating_sub(1).max(head);
        match key {
            KeyCode::Escape if self.view.is_some() => self.close_group(),
            KeyCode::Escape => return Action::Close,
            KeyCode::Backspace if self.view.is_some() => self.close_group(),
            KeyCode::Enter | KeyCode::NumpadEnter => return self.activate(),
            KeyCode::Space => return self.tick(),
            KeyCode::KeyA => return self.auto_key(),
            KeyCode::F5 => return self.check(),
            KeyCode::ArrowUp => self.set_focus(self.focus.saturating_sub(1)),
            KeyCode::ArrowDown => self.set_focus(self.focus + 1),
            KeyCode::ArrowLeft if self.focus == self.main_focus() => {
                self.set_focus(self.check_focus())
            }
            KeyCode::ArrowRight if self.focus == self.check_focus() => {
                self.set_focus(self.main_focus())
            }
            KeyCode::ArrowLeft if self.view.is_some() => self.close_group(),
            KeyCode::ArrowRight if self.view.is_none() && row.is_some() => {
                self.open_group(row.unwrap_or_default())
            }
            KeyCode::PageUp => self.set_focus(self.focus.saturating_sub(rows).max(head).min(last)),
            KeyCode::PageDown => self.set_focus((self.focus + rows).min(last)),
            KeyCode::Home if self.view.is_some() => self.set_focus(head),
            KeyCode::End if self.view.is_some() => self.set_focus(last),
            _ => {}
        }
        Action::None
    }

    pub(crate) fn pointer(&mut self, event: InputEvent) -> Action {
        let Some(event) = self.ui.pointer(event) else {
            return Action::None;
        };
        self.update();
        if event.kind == UiEventKind::Wheel {
            let direction = event.delta.map_or(0, |delta| -delta.y.signum() as isize);
            self.first = self
                .first
                .saturating_add_signed(direction * if self.view.is_some() { 3 } else { 1 })
                .min(self.items().saturating_sub(self.rows()));
            return Action::None;
        }
        let Some(token) = event.token else {
            return Action::None;
        };
        let activate = event.kind == UiEventKind::Activate;
        match token {
            BACK_TOKEN if activate => return Action::Close,
            GROUPS_TOKEN if activate => {
                self.close_group();
                return Action::None;
            }
            // The switch answers from either list.
            SWITCH_TOKEN if activate => {
                self.on = !self.on;
                return Action::Switch(self.on);
            }
            AUTO_TOKEN if activate => return self.toggle_auto(),
            HEADER_AUTO_TOKEN if activate => {
                return self
                    .view
                    .map_or(Action::None, |group| self.toggle_auto_group(group));
            }
            tick if (TICK_BASE..TICK_BASE + ROWS as u16).contains(&tick)
                && activate
                && self.view.is_none() =>
            {
                let index = self.first + usize::from(tick - TICK_BASE);
                self.focus = index + self.head();
                return self.toggle_group(index);
            }
            tick if (AUTO_TICK_BASE..AUTO_TICK_BASE + ROWS as u16).contains(&tick)
                && activate
                && self.view.is_none() =>
            {
                let index = self.first + usize::from(tick - AUTO_TICK_BASE);
                self.focus = index + self.head();
                return self.toggle_auto_group(index);
            }
            _ => {}
        }
        let head = self.head();
        let focus = match token {
            SWITCH_TOKEN if self.view.is_none() => 0,
            AUTO_TOKEN if self.view.is_none() => 1,
            HEADER_TOKEN | HEADER_AUTO_TOKEN if self.view.is_some() => 0,
            CHECK_TOKEN => self.check_focus(),
            MAIN_TOKEN => self.main_focus(),
            row if usize::from(row) < self.rows() => self.first + usize::from(row) + head,
            tick if (TICK_BASE..TICK_BASE + ROWS as u16).contains(&tick) => {
                self.first + usize::from(tick - TICK_BASE) + head
            }
            tick if (AUTO_TICK_BASE..AUTO_TICK_BASE + ROWS as u16).contains(&tick) => {
                self.first + usize::from(tick - AUTO_TICK_BASE) + head
            }
            _ => return Action::None,
        };
        let focus = focus.min(self.main_focus());
        match event.kind {
            UiEventKind::HoverEnter => {
                self.focus = focus;
                Action::None
            }
            UiEventKind::Activate => {
                self.focus = focus;
                self.activate()
            }
            _ => Action::None,
        }
    }

    pub(crate) fn append(&mut self, target: TextTarget<'_>, viewport: [f32; 2]) {
        self.update();
        let frame = Frame::new(viewport);
        let s = frame.s;
        let (status, status_colour, ratio) = self.status();
        let (label, enabled) = self.main_button();
        let running = self.running();
        let check_focus = self.check_focus();
        let main_focus = self.main_focus();
        let rows = self.rows();
        let items = self.items();
        let words = self.words;
        let head = self.head();
        self.ui.begin_transparent(viewport);
        kit::scrim(&mut self.ui, viewport);
        kit::card(&mut self.ui, &frame, CARD);
        let ui = &mut self.ui;
        text(
            ui,
            TextFamily::Display,
            format_args!("{}", words.title),
            frame.rect(LEFT, 126.0, 700.0, 50.0),
            36.0 * s,
            color::GOLD_BRIGHT,
            FontWeight::Semibold,
            TextAlign::Start,
        );
        text(
            ui,
            TextFamily::Body,
            format_args!("{}", words.about),
            frame.rect(LEFT, 174.0, 1100.0, 26.0),
            18.0 * s,
            color::MUTED,
            FontWeight::Regular,
            TextAlign::Start,
        );
        text(
            ui,
            TextFamily::Body,
            format_args!("{}", words.notice),
            frame.rect(LEFT, 198.0, 1100.0, 22.0),
            15.0 * s,
            color::QUIET,
            FontWeight::Regular,
            TextAlign::Start,
        );
        ui.hit_region(BACK_TOKEN, frame.rect(1420.0, 124.0, 100.0, 44.0));
        text(
            ui,
            TextFamily::Body,
            format_args!("Close"),
            frame.rect(1420.0, 132.0, 96.0, 28.0),
            17.0 * s,
            if ui.token_hovered(BACK_TOKEN) {
                color::TEXT
            } else {
                color::MUTED
            },
            FontWeight::Regular,
            TextAlign::End,
        );

        // The switch: the opt-in, said plainly.
        let top = 222.0;
        let switch_focused = self.focus == 0 && self.view.is_none();
        if switch_focused {
            kit::band(ui, &frame, [LEFT - 16.0, top, RIGHT - LEFT + 32.0, 116.0]);
        }
        text(
            ui,
            TextFamily::Body,
            format_args!("{}", words.switch),
            frame.rect(LEFT, top + 12.0, 700.0, 30.0),
            21.0 * s,
            color::TEXT,
            FontWeight::Semibold,
            TextAlign::Start,
        );
        kit::switch(ui, &frame, RIGHT, top + 27.0, self.on, switch_focused);
        let explained = if self.on { words.on } else { words.off };
        text(
            ui,
            TextFamily::Body,
            format_args!("{explained}"),
            frame.rect(LEFT, top + 50.0, 1100.0, 24.0),
            16.0 * s,
            color::MUTED,
            FontWeight::Regular,
            TextAlign::Start,
        );
        text(
            ui,
            TextFamily::Body,
            format_args!(
                "Folder: {}   ·   {}",
                shorten(&self.folder, 70),
                words.folder_note
            ),
            frame.rect(LEFT, top + 78.0, 1100.0, 24.0),
            15.0 * s,
            color::QUIET,
            FontWeight::Regular,
            TextAlign::Start,
        );
        ui.hit_region(
            SWITCH_TOKEN,
            frame.rect(LEFT - 16.0, top, RIGHT - LEFT + 32.0, 116.0),
        );

        // Automatic updates: the second opt-in, under the first.
        let auto_focused = self.focus == 1 && self.view.is_none();
        if auto_focused {
            kit::band(
                ui,
                &frame,
                [LEFT - 16.0, AUTO_TOP, RIGHT - LEFT + 32.0, AUTO_HEIGHT],
            );
        }
        text(
            ui,
            TextFamily::Body,
            format_args!("{AUTO_SWITCH}"),
            frame.rect(LEFT, AUTO_TOP + 6.0, 700.0, 28.0),
            19.0 * s,
            color::TEXT,
            FontWeight::Semibold,
            TextAlign::Start,
        );
        kit::switch(ui, &frame, RIGHT, AUTO_TOP + 24.0, self.auto, auto_focused);
        text(
            ui,
            TextFamily::Body,
            format_args!("{}", if self.auto { words.auto_on } else { AUTO_OFF }),
            frame.rect(LEFT, AUTO_TOP + 33.0, 1000.0, 22.0),
            15.0 * s,
            color::MUTED,
            FontWeight::Regular,
            TextAlign::Start,
        );
        ui.hit_region(
            AUTO_TOKEN,
            frame.rect(LEFT - 16.0, AUTO_TOP, RIGHT - LEFT + 32.0, AUTO_HEIGHT),
        );
        let auto = self.auto;

        match (&self.snapshot, self.view) {
            (None, _) => {
                kit::heading(ui, &frame, LEFT, HEADING, RIGHT - LEFT, "Groups");
                let waiting = match &self.phase {
                    Phase::Failed(_) => {
                        "The list could not be fetched. Check again to retry.".to_owned()
                    }
                    _ => format!("Fetching the list from {}...", words.repository),
                };
                text(
                    ui,
                    TextFamily::Body,
                    format_args!("{waiting}"),
                    frame.rect(LEFT, GROUPS_TOP + 10.0, 1100.0, 28.0),
                    18.0 * s,
                    color::MUTED,
                    FontWeight::Regular,
                    TextAlign::Start,
                );
            }
            (Some(snapshot), None) => {
                kit::heading(ui, &frame, LEFT, HEADING, RIGHT - LEFT, "Groups");
                let groups = &snapshot.manifest.groups;
                for (slot, (index, group)) in groups
                    .iter()
                    .enumerate()
                    .skip(self.first)
                    .take(ROWS)
                    .enumerate()
                {
                    let top = GROUPS_TOP + slot as f32 * ROW_HEIGHT;
                    let focused = self.focus == index + head;
                    let summary = self.summaries.get(index).copied().unwrap_or_default();
                    let fetchable = summary.files > 0;
                    if focused {
                        kit::band(
                            ui,
                            &frame,
                            [LEFT - 16.0, top, RIGHT - LEFT + 32.0, ROW_HEIGHT - 4.0],
                        );
                    }
                    ui.hit_region(
                        slot as u16,
                        frame.rect(LEFT + 36.0, top, RIGHT - LEFT - 20.0, ROW_HEIGHT - 4.0),
                    );
                    if fetchable {
                        kit::tick(ui, &frame, LEFT, top + 29.0, summary.wanted > 0, focused);
                        ui.hit_region(
                            TICK_BASE + slot as u16,
                            frame.rect(LEFT - 16.0, top, 52.0, ROW_HEIGHT - 4.0),
                        );
                    }
                    if fetchable {
                        let marked = self.auto_groups.includes(&self.selection, group);
                        auto_tick(ui, &frame, top + 29.0, marked, auto, focused);
                        ui.hit_region(
                            AUTO_TICK_BASE + slot as u16,
                            frame.rect(AUTO_X - 12.0, top, 104.0, ROW_HEIGHT - 4.0),
                        );
                    }
                    text(
                        ui,
                        TextFamily::Body,
                        format_args!("{}", cut(&group.name, 36)),
                        frame.rect(LEFT + 42.0, top + 6.0, 440.0, 28.0),
                        20.0 * s,
                        if fetchable { color::TEXT } else { color::QUIET },
                        FontWeight::Semibold,
                        TextAlign::Start,
                    );
                    text(
                        ui,
                        TextFamily::Body,
                        format_args!("{}  ·  {}", group.kind.label(), cut(&group.description, 48)),
                        frame.rect(LEFT + 42.0, top + 33.0, 440.0, 22.0),
                        15.0 * s,
                        color::MUTED,
                        FontWeight::Regular,
                        TextAlign::Start,
                    );
                    let (state, colour) = group_state(&summary);
                    text(
                        ui,
                        TextFamily::Body,
                        format_args!("{state}"),
                        frame.rect(RIGHT - 500.0, top + 6.0, 500.0, 28.0),
                        18.0 * s,
                        colour,
                        FontWeight::Regular,
                        TextAlign::End,
                    );
                    let detail = if fetchable {
                        format!(
                            "{} files  ·  {}  ·  Enter to pick files  ›",
                            summary.files,
                            human_size(summary.size)
                        )
                    } else {
                        format!(
                            "{} for JoF's own client, not used by SJK",
                            summary.elsewhere
                        )
                    };
                    text(
                        ui,
                        TextFamily::Body,
                        format_args!("{detail}"),
                        frame.rect(RIGHT - 500.0, top + 33.0, 500.0, 22.0),
                        15.0 * s,
                        if focused { color::MUTED } else { color::QUIET },
                        FontWeight::Regular,
                        TextAlign::End,
                    );
                }
                if groups.len() > ROWS {
                    text(
                        ui,
                        TextFamily::Body,
                        format_args!(
                            "{}-{} of {} groups",
                            self.first + 1,
                            (self.first + ROWS).min(groups.len()),
                            groups.len()
                        ),
                        frame.rect(RIGHT - 300.0, HEADING - 8.0, 300.0, 22.0),
                        14.0 * s,
                        color::QUIET,
                        FontWeight::Regular,
                        TextAlign::End,
                    );
                }
            }
            (Some(snapshot), Some(index)) => {
                let group = &snapshot.manifest.groups[index];
                let summary = self.summaries.get(index).copied().unwrap_or_default();
                // The way back, over the files.
                let back = format!("‹ Groups  /  {}", group.name);
                kit::heading_in(
                    ui,
                    &frame,
                    LEFT,
                    HEADING,
                    RIGHT - LEFT,
                    &back,
                    if ui.token_hovered(GROUPS_TOKEN) {
                        color::GOLD_BRIGHT
                    } else {
                        color::HOLO
                    },
                );
                ui.hit_region(
                    GROUPS_TOKEN,
                    frame.rect(LEFT - 8.0, HEADING - 16.0, 560.0, 32.0),
                );
                if group.assets.len() > FILE_ROWS {
                    text(
                        ui,
                        TextFamily::Body,
                        format_args!(
                            "{}-{} of {} files",
                            self.first + 1,
                            (self.first + FILE_ROWS).min(group.assets.len()),
                            group.assets.len()
                        ),
                        frame.rect(RIGHT - 300.0, HEADING - 22.0, 300.0, 22.0),
                        14.0 * s,
                        color::QUIET,
                        FontWeight::Regular,
                        TextAlign::End,
                    );
                }
                // The group's own row: its tick takes or leaves every file.
                let header_focused = self.focus == 0;
                if header_focused {
                    kit::band(
                        ui,
                        &frame,
                        [LEFT - 16.0, GROUPS_TOP, RIGHT - LEFT + 32.0, 54.0],
                    );
                }
                if summary.files > 0 {
                    kit::tick(
                        ui,
                        &frame,
                        LEFT,
                        GROUPS_TOP + 27.0,
                        summary.wanted == summary.files,
                        header_focused,
                    );
                }
                text(
                    ui,
                    TextFamily::Body,
                    format_args!("All of {}", cut(&group.name, 32)),
                    frame.rect(LEFT + 42.0, GROUPS_TOP + 4.0, 440.0, 26.0),
                    19.0 * s,
                    color::TEXT,
                    FontWeight::Semibold,
                    TextAlign::Start,
                );
                text(
                    ui,
                    TextFamily::Body,
                    format_args!(
                        "{} of {} chosen  ·  {}",
                        summary.wanted,
                        summary.files,
                        human_size(summary.size)
                    ),
                    frame.rect(LEFT + 42.0, GROUPS_TOP + 30.0, 440.0, 22.0),
                    15.0 * s,
                    color::MUTED,
                    FontWeight::Regular,
                    TextAlign::Start,
                );
                let (state, colour) = group_state(&summary);
                text(
                    ui,
                    TextFamily::Body,
                    format_args!("{state}"),
                    frame.rect(RIGHT - 500.0, GROUPS_TOP + 14.0, 500.0, 28.0),
                    18.0 * s,
                    colour,
                    FontWeight::Regular,
                    TextAlign::End,
                );
                ui.hit_region(
                    HEADER_TOKEN,
                    frame.rect(LEFT - 16.0, GROUPS_TOP, RIGHT - LEFT + 32.0, 54.0),
                );
                if summary.files > 0 {
                    let marked = self.auto_groups.includes(&self.selection, group);
                    auto_tick(ui, &frame, GROUPS_TOP + 27.0, marked, auto, header_focused);
                    ui.hit_region(
                        HEADER_AUTO_TOKEN,
                        frame.rect(AUTO_X - 12.0, GROUPS_TOP, 104.0, 54.0),
                    );
                }
                // The files.
                let files_top = GROUPS_TOP + 64.0;
                for (slot, (asset_index, asset)) in group
                    .assets
                    .iter()
                    .enumerate()
                    .skip(self.first)
                    .take(FILE_ROWS)
                    .enumerate()
                {
                    let top = files_top + slot as f32 * FILE_ROW_HEIGHT;
                    let focused = self.focus == asset_index + 1;
                    let for_sjk = asset.target == Target::Base;
                    let wanted = for_sjk && self.selection.wants(group, asset);
                    if focused {
                        kit::band(
                            ui,
                            &frame,
                            [LEFT - 16.0, top, RIGHT - LEFT + 32.0, FILE_ROW_HEIGHT - 4.0],
                        );
                    }
                    if for_sjk {
                        kit::tick(ui, &frame, LEFT, top + 21.0, wanted, focused);
                    }
                    text(
                        ui,
                        TextFamily::Body,
                        format_args!("{}", shorten(&asset.name, 44)),
                        frame.rect(LEFT + 42.0, top + 9.0, 470.0, 26.0),
                        18.0 * s,
                        if for_sjk { color::TEXT } else { color::QUIET },
                        FontWeight::Regular,
                        TextAlign::Start,
                    );
                    text(
                        ui,
                        TextFamily::Body,
                        format_args!("{}", cut(&asset.description, 40)),
                        frame.rect(LEFT + 530.0, top + 12.0, 330.0, 22.0),
                        15.0 * s,
                        color::MUTED,
                        FontWeight::Regular,
                        TextAlign::Start,
                    );
                    let name = asset.name.to_ascii_lowercase();
                    let (state, colour) = file_state(
                        snapshot.state(index, asset_index),
                        wanted,
                        for_sjk,
                        snapshot.downloaded.contains(&name),
                    );
                    text(
                        ui,
                        TextFamily::Body,
                        format_args!("{state}"),
                        frame.rect(RIGHT - 270.0, top + 10.0, 160.0, 24.0),
                        16.0 * s,
                        colour,
                        FontWeight::Regular,
                        TextAlign::End,
                    );
                    text(
                        ui,
                        TextFamily::Body,
                        format_args!("{}", human_size(asset.size)),
                        frame.rect(RIGHT - 100.0, top + 10.0, 100.0, 24.0),
                        16.0 * s,
                        color::QUIET,
                        FontWeight::Regular,
                        TextAlign::End,
                    );
                    ui.hit_region(
                        slot as u16,
                        frame.rect(LEFT - 16.0, top, RIGHT - LEFT + 32.0, FILE_ROW_HEIGHT - 4.0),
                    );
                }
            }
        }

        // What happens, or is happening.
        let status_top = GROUPS_TOP + ROWS as f32 * ROW_HEIGHT + 14.0;
        text(
            ui,
            TextFamily::Body,
            format_args!("{status}"),
            frame.rect(LEFT, status_top, 1110.0, 26.0),
            17.0 * s,
            status_colour,
            FontWeight::Regular,
            TextAlign::Start,
        );
        if let Some(ratio) = ratio {
            bar(
                ui,
                &frame,
                [LEFT, status_top + 36.0, RIGHT - LEFT, 8.0],
                ratio,
            );
        }
        let buttons = status_top + 62.0;
        kit::button(
            ui,
            &frame,
            [LEFT, buttons, 200.0, 46.0],
            "Check again",
            false,
            !running,
            self.focus == check_focus,
            CHECK_TOKEN,
        );
        kit::button(
            ui,
            &frame,
            [RIGHT - 280.0, buttons, 280.0, 46.0],
            &label,
            !running,
            enabled,
            self.focus == main_focus,
            MAIN_TOKEN,
        );
        let hints = if self.view.is_some() {
            "Space pick  ·  A auto  ·  Esc back to groups  ·  F5 check"
        } else {
            "Up Down choose  ·  Space tick  ·  A auto  ·  Enter files  ·  Esc close"
        };
        text(
            ui,
            TextFamily::Body,
            format_args!("{hints}"),
            frame.rect(LEFT + 215.0, buttons + 12.0, 600.0, 24.0),
            15.0 * s,
            color::QUIET,
            FontWeight::Regular,
            TextAlign::Center,
        );
        // When the next automatic check runs, while it will.
        if let (true, true, Some((hour, minute))) = (self.on, self.auto, self.next_clock) {
            text(
                ui,
                TextFamily::Body,
                format_args!("Next automatic check {hour:02}:{minute:02}"),
                frame.rect(LEFT, buttons + 60.0, RIGHT - LEFT, 22.0),
                14.0 * s,
                color::QUIET,
                FontWeight::Regular,
                TextAlign::Center,
            );
        }
        let focused_token = match self.focus {
            0 if self.view.is_some() => HEADER_TOKEN,
            0 => SWITCH_TOKEN,
            1 if self.view.is_none() => AUTO_TOKEN,
            focus if focus == check_focus => CHECK_TOKEN,
            focus if focus == main_focus => MAIN_TOKEN,
            focus if focus < items + head => {
                (focus - head).saturating_sub(self.first).min(rows) as u16
            }
            _ => SWITCH_TOKEN,
        };
        self.ui.finish(focused_token);
        target.append(&self.ui, viewport);
    }

    /// The status line, its colour and the progress bar's fill.
    fn status(&self) -> (String, Color, Option<f32>) {
        let ratio = |done: u64, total: u64| {
            Some(if total == 0 {
                0.0
            } else {
                done as f32 / total as f32
            })
        };
        match &self.phase {
            Phase::Manifest => (
                format!("Asking {} for its list...", self.words.repository),
                color::MUTED,
                None,
            ),
            Phase::Checking { done, total } => (
                format!(
                    "Checking the files already here ({} of {})...",
                    human_size(*done),
                    human_size(*total)
                ),
                color::MUTED,
                ratio(*done, *total),
            ),
            Phase::Removing { done, total } => (
                format!("Removing files no longer chosen ({done} of {total})..."),
                color::MUTED,
                ratio(*done as u64, *total as u64),
            ),
            Phase::Downloading {
                file,
                files,
                name,
                done,
                total,
                ..
            } => (
                format!(
                    "Downloading {file} of {files}: {}  ·  {} of {}  ·  goes on while you play",
                    shorten(name, 40),
                    human_size(*done),
                    human_size(*total)
                ),
                color::GOLD_BRIGHT,
                ratio(*done, *total),
            ),
            // An automatic check that found nothing to do says what is here.
            Phase::Finished(outcome)
                if outcome.synced
                    && !(outcome.automatic
                        && !outcome.changed()
                        && outcome.failures.is_empty()) =>
            {
                self.finished(outcome)
            }
            Phase::Failed(reason) => (
                format!(
                    "{} could not be reached: {}",
                    capital(self.words.repository),
                    shorten(reason, 80)
                ),
                color::EMBER,
                None,
            ),
            Phase::Cancelled => (
                "Stopped. Download again to carry on where it stopped.".into(),
                color::MUTED,
                None,
            ),
            Phase::Idle | Phase::Finished(_) => (self.ready(), color::MUTED, None),
        }
    }

    fn finished(&self, outcome: &Outcome) -> (String, Color, Option<f32>) {
        if let Some(first) = outcome.failures.first() {
            return (
                format!(
                    "{} downloaded, {} failed: {}",
                    outcome.downloaded,
                    outcome.failures.len(),
                    shorten(first, 70)
                ),
                color::EMBER,
                None,
            );
        }
        let mut done = format!(
            "Done: {} files downloaded ({}), {} removed.",
            outcome.downloaded,
            human_size(outcome.downloaded_bytes),
            outcome.removed
        );
        if self.on {
            done.push_str(" They load from the next map.");
        } else {
            done.push_str(&format!(" Turn on {} to load them.", self.words.switch));
        }
        (done, color::GOLD_BRIGHT, None)
    }

    fn ready(&self) -> String {
        if self.snapshot.is_none() {
            return "Not checked yet.".into();
        }
        let plan = &self.plan;
        let what = match (plan.downloads.is_empty(), plan.removals.is_empty()) {
            (true, true) => return "Everything chosen is here.".into(),
            (false, true) => format!(
                "{} files to download ({}).",
                plan.downloads.len(),
                human_size(plan.download_bytes)
            ),
            (true, false) => format!(
                "{} files no longer chosen to remove ({}).",
                plan.removals.len(),
                human_size(plan.removal_bytes)
            ),
            (false, false) => format!(
                "{} files to download ({}), {} to remove ({}).",
                plan.downloads.len(),
                human_size(plan.download_bytes),
                plan.removals.len(),
                human_size(plan.removal_bytes)
            ),
        };
        if self.on {
            what
        } else {
            format!("{what} Turn on {} first.", self.words.switch)
        }
    }
}

/// A group's Auto tick and its word at [`AUTO_X`], dimmed while the automatic
/// updates are off (`live`).
fn auto_tick(
    canvas: &mut MenuCanvas,
    frame: &Frame,
    y: f32,
    marked: bool,
    live: bool,
    focused: bool,
) {
    if !live {
        canvas.push_opacity(0.4);
    }
    kit::tick(canvas, frame, AUTO_X, y, marked, focused && live);
    text(
        canvas,
        TextFamily::Body,
        format_args!("Auto"),
        frame.rect(AUTO_X + 34.0, y - 12.0, 60.0, 24.0),
        16.0 * frame.s,
        if marked { color::TEXT } else { color::MUTED },
        FontWeight::Regular,
        TextAlign::Start,
    );
    if !live {
        canvas.pop_opacity();
    }
}

/// The 24-hour clock `after` from `hour:minute:second`, to the minute (a
/// started minute counts whole, so the check is never before the time shown).
fn clock_after(hour: u8, minute: u8, second: u8, after: Duration) -> (u8, u8) {
    let now = u64::from(hour) * 3600 + u64::from(minute) * 60 + u64::from(second);
    let then = (now + after.as_secs()).div_ceil(60) % (24 * 60);
    ((then / 60) as u8, (then % 60) as u8)
}

/// The text with its first letter in capitals, for a name starting a sentence.
fn capital(text: &str) -> String {
    let mut letters = text.chars();
    letters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(letters).collect()
    })
}

/// Each group's counts, in the manifest's order.
fn summaries(snapshot: &Snapshot, selection: &Selection) -> Vec<Summary> {
    snapshot
        .manifest
        .groups
        .iter()
        .enumerate()
        .map(|(group_index, group)| {
            let mut summary = Summary::default();
            for (asset_index, asset) in group.assets.iter().enumerate() {
                if asset.target != Target::Base {
                    summary.elsewhere += 1;
                    continue;
                }
                summary.files += 1;
                summary.size += asset.size;
                let wanted = selection.wants(group, asset);
                let state = snapshot.state(group_index, asset_index);
                if wanted {
                    summary.wanted += 1;
                }
                if state.present() {
                    summary.here += 1;
                } else if wanted {
                    summary.missing += asset.size;
                }
                if !wanted && matches!(state, AssetState::Installed | AssetState::Outdated) {
                    summary.ours += asset.size;
                }
            }
            summary
        })
        .collect()
}

/// The group's state on the right of its row.
fn group_state(summary: &Summary) -> (String, Color) {
    if summary.files == 0 {
        return ("Not for SJK".into(), color::QUIET);
    }
    if summary.wanted == 0 {
        return if summary.ours > 0 {
            (
                format!("Not chosen  ·  {} to remove", human_size(summary.ours)),
                color::MUTED,
            )
        } else if summary.here > 0 {
            (
                format!(
                    "Not chosen  ·  {} of {} in game folder",
                    summary.here, summary.files
                ),
                color::QUIET,
            )
        } else {
            ("Not chosen".into(), color::QUIET)
        };
    }
    let whole = summary.wanted == summary.files;
    match (whole, summary.missing) {
        (true, 0) if summary.here == summary.files => {
            (format!("All {} here", summary.files), color::GOLD_BRIGHT)
        }
        (true, missing) => (
            format!(
                "{} of {} here  ·  {} to download",
                summary.here,
                summary.files,
                human_size(missing)
            ),
            color::TEXT,
        ),
        (false, 0) => (
            format!(
                "{} of {} chosen  ·  all here",
                summary.wanted, summary.files
            ),
            color::GOLD_BRIGHT,
        ),
        (false, missing) => (
            format!(
                "{} of {} chosen  ·  {} to download",
                summary.wanted,
                summary.files,
                human_size(missing)
            ),
            color::TEXT,
        ),
    }
}

/// One file's state on the right of its row.
fn file_state(
    state: AssetState,
    wanted: bool,
    for_sjk: bool,
    downloaded: bool,
) -> (&'static str, Color) {
    if !for_sjk {
        return ("JoF's client only", color::QUIET);
    }
    match (state, wanted) {
        (AssetState::Installed, true) => ("Here", color::GOLD_BRIGHT),
        (AssetState::InGameFolder, true) => ("In game folder", color::GOLD_BRIGHT),
        (AssetState::InGameFolder, false) => ("In game folder", color::MUTED),
        (AssetState::Missing, true) => ("To download", color::TEXT),
        (AssetState::Outdated, true) => ("To update", color::TEXT),
        (AssetState::Installed | AssetState::Outdated, false) if downloaded => {
            ("To remove", color::EMBER)
        }
        (AssetState::Installed | AssetState::Outdated, false) => ("Here, not chosen", color::MUTED),
        _ => ("Not chosen", color::QUIET),
    }
}

/// A thin track with its gold fill.
fn bar(canvas: &mut MenuCanvas, frame: &Frame, rect: [f32; 4], ratio: f32) {
    let [x, y, width, height] = rect;
    let radius = height * 0.5 * frame.s;
    let _ = canvas.draw_list_mut().push(DrawCommand::RoundedRect {
        rect: frame.rect(x, y, width, height),
        radius,
        color: color::alpha(color::HOLO, 0.12),
    });
    let fill = width * ratio.clamp(0.0, 1.0);
    if fill > 0.0 {
        let _ = canvas.draw_list_mut().push(DrawCommand::RoundedRect {
            rect: frame.rect(x, y, fill.max(height), height),
            radius,
            color: color::GOLD,
        });
    }
}

/// `text` cut to `most` characters with an ellipsis at the end.
fn cut(text: &str, most: usize) -> String {
    if text.chars().count() <= most {
        return text.to_owned();
    }
    let kept: String = text.chars().take(most - 1).collect();
    format!("{}…", kept.trim_end())
}

/// `text` cut to `most` characters with an ellipsis in the middle.
fn shorten(text: &str, most: usize) -> String {
    let count = text.chars().count();
    if count <= most {
        return text.to_owned();
    }
    let head = most / 2;
    let tail = most - head - 1;
    let start: String = text.chars().take(head).collect();
    let end: String = text.chars().skip(count - tail).collect();
    format!("{start}…{end}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_paths_keep_both_ends() {
        assert_eq!(shorten("short", 10), "short");
        let cut = shorten(
            "C:/Program Files (x86)/Steam/steamapps/common/Jedi Academy/GameData/SJK/jof",
            30,
        );
        assert_eq!(cut.chars().count(), 30);
        assert!(cut.starts_with("C:/Program"));
        assert!(cut.ends_with("SJK/jof"));
    }

    #[test]
    fn a_group_says_what_is_here_and_what_a_sync_does() {
        let summary = Summary {
            files: 35,
            here: 12,
            size: 4_600_000_000,
            wanted: 35,
            missing: 1_200_000_000,
            ours: 0,
            elsewhere: 0,
        };
        assert_eq!(
            group_state(&summary).0,
            "12 of 35 here  ·  1.2 GB to download"
        );
        let none = Summary {
            wanted: 0,
            missing: 0,
            ..summary
        };
        assert_eq!(
            group_state(&none).0,
            "Not chosen  ·  12 of 35 in game folder"
        );
        let all = Summary {
            here: 35,
            missing: 0,
            ..summary
        };
        assert_eq!(group_state(&all).0, "All 35 here");
        let ours = Summary {
            ours: 200_000_000,
            ..none
        };
        assert_eq!(group_state(&ours).0, "Not chosen  ·  200 MB to remove");
        let some = Summary {
            wanted: 3,
            missing: 75_000_000,
            ..summary
        };
        assert_eq!(group_state(&some).0, "3 of 35 chosen  ·  75 MB to download");
    }

    #[test]
    fn a_file_says_what_a_sync_does_with_it() {
        use AssetState::*;
        assert_eq!(file_state(Missing, true, true, false).0, "To download");
        assert_eq!(file_state(Missing, false, true, false).0, "Not chosen");
        assert_eq!(file_state(Installed, false, true, true).0, "To remove");
        assert_eq!(
            file_state(Installed, false, true, false).0,
            "Here, not chosen"
        );
        assert_eq!(
            file_state(InGameFolder, true, true, false).0,
            "In game folder"
        );
        assert_eq!(
            file_state(NotForSjk, false, false, false).0,
            "JoF's client only"
        );
    }

    /// A card over a two-group manifest, looked at, with the switch on.
    fn looked_at() -> Card {
        let json = r#"{"data":{"assetgroups":[
            {"id":1,"name":"Maps","type":"RECOMMENDED","assets":[
                {"id":10,"name":"a.pk3","size":3,"hash":"47bce5c74f589f4867dbd57e9ca9f808","targetfolder":"Base","localfolder":"assets"},
                {"id":11,"name":"b.pk3","size":4,"hash":"47bce5c74f589f4867dbd57e9ca9f808","targetfolder":"Base","localfolder":"assets"}]},
            {"id":2,"name":"Skins","type":"OPTIONAL","assets":[
                {"id":20,"name":"c.pk3","size":2,"hash":"47bce5c74f589f4867dbd57e9ca9f808","targetfolder":"Base","localfolder":"assets"}]}]}}"#;
        let manifest = sjk_jof_content::Manifest::parse(json.as_bytes()).unwrap();
        let states = vec![
            vec![AssetState::Missing, AssetState::Missing],
            vec![AssetState::Missing],
        ];
        let mut card = Card {
            on: true,
            snapshot: Some(Arc::new(Snapshot {
                manifest,
                states,
                ..Snapshot::default()
            })),
            ..Card::default()
        };
        card.update();
        card
    }

    fn press(card: &mut Card, focus: usize, space: bool) -> Action {
        card.set_focus(focus);
        if space { card.tick() } else { card.activate() }
    }

    #[test]
    fn a_group_opens_on_its_files_and_single_files_are_picked() {
        let mut card = looked_at();
        assert_eq!(card.plan.download_bytes, 7, "Maps by default");
        // Enter on Skins shows its files; Space on c.pk3 picks it alone.
        assert!(matches!(press(&mut card, 3, false), Action::None));
        assert_eq!(card.view, Some(1));
        let Action::Choice { files, .. } = press(&mut card, 1, true) else {
            panic!("no choice")
        };
        assert_eq!(files, "+20");
        assert_eq!(card.plan.download_bytes, 9);
        // Back to the groups where the player was; leave b.pk3 out of Maps.
        card.close_group();
        assert_eq!((card.view, card.focus), (None, 3));
        press(&mut card, 2, false);
        let Action::Choice { files, groups } = press(&mut card, 2, true) else {
            panic!("no choice")
        };
        assert_eq!((groups.as_str(), files.as_str()), ("default", "-11 +20"));
        assert_eq!(card.plan.download_bytes, 5);
        // The group's own row takes every file of it again.
        let Action::Choice { files, .. } = press(&mut card, 0, true) else {
            panic!("no choice")
        };
        assert_eq!(files, "+20");
        assert_eq!(card.plan.download_bytes, 9);
    }

    fn key(card: &mut Card, code: KeyCode) -> Action {
        card.key_code(code)
    }

    #[test]
    fn the_automatic_switch_and_auto_ticks_answer_keys() {
        let mut card = looked_at();
        // Under the switch, the automatic updates' switch.
        card.set_focus(0);
        key(&mut card, KeyCode::ArrowDown);
        assert!(matches!(card.activate(), Action::Auto(true)));
        assert!(card.auto);
        // A on a group marks it Auto; all (the default) means the chosen groups.
        assert!(card.auto_group_marked(0) && !card.auto_group_marked(1));
        key(&mut card, KeyCode::ArrowDown);
        let Action::AutoGroups(groups) = key(&mut card, KeyCode::KeyA) else {
            panic!("no automatic groups")
        };
        assert_eq!(groups, "");
        card.set_focus(3);
        let Action::AutoGroups(groups) = key(&mut card, KeyCode::KeyA) else {
            panic!("no automatic groups")
        };
        assert_eq!(groups, "2");
        // In a group's files, A marks the group itself.
        key(&mut card, KeyCode::Enter);
        assert_eq!(card.view, Some(1));
        let Action::AutoGroups(groups) = key(&mut card, KeyCode::KeyA) else {
            panic!("no automatic groups")
        };
        assert_eq!(groups, "");
        // On the buttons A does nothing.
        card.close_group();
        card.set_focus(card.check_focus());
        assert!(matches!(key(&mut card, KeyCode::KeyA), Action::None));
    }

    #[test]
    fn the_automatic_check_comes_soon_after_the_start_then_hourly() {
        let mut card = Card::default();
        let start = Instant::now();
        assert!(!card.auto_due(start), "the first call sets the first check");
        assert!(!card.auto_due(start + AUTO_FIRST - Duration::from_secs(1)));
        assert!(card.auto_due(start + AUTO_FIRST));
        let then = start + AUTO_FIRST;
        assert!(!card.auto_due(then + AUTO_EVERY - Duration::from_secs(1)));
        assert!(card.auto_due(then + AUTO_EVERY));
        assert!(card.next_clock.is_some());
        // The clock shown: a started minute counts whole, past midnight wraps.
        assert_eq!(clock_after(13, 5, 0, AUTO_EVERY), (14, 5));
        assert_eq!(clock_after(13, 5, 1, AUTO_EVERY), (14, 6));
        assert_eq!(clock_after(23, 40, 0, AUTO_EVERY), (0, 40));
        assert_eq!(clock_after(9, 59, 50, AUTO_FIRST), (10, 1));
    }

    #[test]
    fn a_quiet_automatic_check_says_what_is_here() {
        let mut card = looked_at();
        card.phase = Phase::Finished(Outcome {
            synced: true,
            automatic: true,
            ..Outcome::default()
        });
        assert_eq!(card.status().0, "2 files to download (7 B).");
        card.phase = Phase::Finished(Outcome {
            synced: true,
            automatic: true,
            downloaded: 1,
            downloaded_bytes: 3,
            ..Outcome::default()
        });
        assert!(card.status().0.starts_with("Done: 1 files downloaded"));
    }

    #[test]
    #[ignore = "needs a GPU, JKA_GAME_DATA and the network"]
    fn the_jof_content_card_renders_on_a_gpu() {
        crate::world_shot::on_big_stack(|| {
            let (mut gpu, _profile) = crate::world_shot::open(
                "maps/mp/duel6.bsp",
                [1920, 1080],
                None,
                &[("r_hdr", "0"), ("cg_materialMaps", "0")],
            )
            .expect("a GPU adapter");
            let game_data = gpu.game_data.clone();
            let console = gpu.console.as_mut().unwrap();
            // SJK_CONTENT_CLOUD=sith: the Sith cloud's card instead.
            let sith = std::env::var("SJK_CONTENT_CLOUD").is_ok_and(|cloud| cloud == "sith");
            if std::env::var_os("SJK_JOF_CONTENT_ON").is_some() {
                let switch = if sith {
                    crate::sith_cloud::SWITCH
                } else {
                    crate::jof_content::SWITCH
                };
                console.set_cvar(switch, "1");
            }
            // SJK_JOF_CONTENT_AUTO: automatic updates on, with their next check shown.
            let auto = std::env::var_os("SJK_JOF_CONTENT_AUTO").is_some();
            if auto {
                let names = if sith {
                    &crate::sith_cloud::NAMES
                } else {
                    &crate::jof_content::NAMES
                };
                console.set_cvar(names.auto, "1");
            }
            if sith {
                console.open_sith_cloud(&game_data);
            } else {
                console.open_jof_content(&game_data);
            }
            let card = if sith {
                &mut console.sith_cloud
            } else {
                &mut console.jof_content
            };
            let start = std::time::Instant::now();
            while card.busy() && start.elapsed().as_secs() < 300 {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            if auto {
                card.schedule(Instant::now(), AUTO_EVERY);
            }
            // SJK_JOF_CONTENT_GROUP=<n>: show the n-th group's files.
            if let Some(group) = std::env::var("SJK_JOF_CONTENT_GROUP")
                .ok()
                .and_then(|group| group.parse::<usize>().ok())
            {
                card.update();
                card.open_group(group);
                card.set_focus(3);
            }
            assert!(matches!(
                gpu.render(&mut None),
                crate::gpu_context::FrameStatus::Rendered
            ));
            let console = gpu.console.as_ref().unwrap();
            assert!(console.covers_frame());
            assert!(console.sjk_page_open());
            let card = if sith {
                &console.sith_cloud
            } else {
                &console.jof_content
            };
            // The automatic updates' switch, and an Auto tick on each group row
            // SJK can fetch from (or on the open group's own row).
            assert!(card.ui.rect_for(AUTO_TOKEN).is_some());
            let fetchable = |slot: usize| {
                card.summaries
                    .get(card.first + slot)
                    .is_some_and(|summary| summary.files > 0)
            };
            if card.view.is_some() {
                let open = card.view.and_then(|group| card.summaries.get(group));
                assert_eq!(
                    card.ui.rect_for(HEADER_AUTO_TOKEN).is_some(),
                    open.is_some_and(|summary| summary.files > 0)
                );
            } else {
                for slot in 0..ROWS.min(card.summaries.len()) {
                    assert_eq!(
                        card.ui.rect_for(AUTO_TICK_BASE + slot as u16).is_some(),
                        fetchable(slot),
                        "slot {slot}"
                    );
                }
            }
            if let Some(path) = std::env::var_os("SJK_JOF_CONTENT_SHOT") {
                crate::world_shot::frame(&mut gpu, 2).save(path).unwrap();
            }
        });
    }

    #[test]
    fn download_waits_for_the_switch_and_a_look_at_the_disk() {
        let mut card = Card::default();
        assert_eq!(card.main_button(), ("Download".into(), false));
        card.snapshot = Some(Arc::new(Snapshot::default()));
        card.update();
        assert_eq!(card.main_button(), ("Download".into(), false), "switch off");
        card.on = true;
        assert_eq!(card.main_button(), ("Up to date".into(), false));
        assert!(
            matches!(card.activate(), Action::Switch(false)),
            "focus starts on the switch"
        );
    }
}
