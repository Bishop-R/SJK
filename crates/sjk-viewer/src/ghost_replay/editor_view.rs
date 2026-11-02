//! The ghost editor in the SJK UI's look (`docs/sjk-ui.md`, Ghost editor): the map
//! editor's column down the window's right edge over its dark fade, and the run's path
//! drawn over the world left of it, or over play with a strip at the top while the player
//! looks about. The state, tokens, keys and pointer are [`super::editor`]'s.
//!
//! Positions are pixels of a 1080-line frame whose right edge is the window's, as the map
//! editor's.

use super::edit::Motion;
use super::editor::*;
use crate::hud::identification::Camera;
use crate::menu::sjk::{Frame, color, fade_across, key_hint, key_hint_width, kit, text, wrap};
use crate::menu_widgets::{MenuCanvas, TextFamily};
use glam::Vec3;
use sjk_ui::{Color, DrawCommand, FontWeight, Rect, TextAlign};

const COLUMN_X: f32 = 1440.0;
const COLUMN_WIDTH: f32 = 416.0;
const FADE_START: f32 = 1200.0;
const FADE_DARK: f32 = 1400.0;
const TITLE_Y: f32 = 80.0;
const SUBTITLE_Y: f32 = 120.0;
const LIST_HEADING_Y: f32 = 166.0;
const LIST_Y: f32 = 182.0;
const LIST_ROW: f32 = 34.0;
const COPY_Y: f32 = 412.0;
const CUT_HEADING_Y: f32 = 494.0;
const TIMELINE_Y: f32 = 510.0;
const TIMELINE_HEIGHT: f32 = 36.0;
/// The strip under the timeline where the start can land cleanly.
const CLEAN_Y: f32 = 550.0;
const START_Y: f32 = 566.0;
const END_Y: f32 = 626.0;
const ROW: f32 = 56.0;
const MOVES_Y: f32 = 694.0;
const NOTE_Y: f32 = 744.0;
/// The foot: the bubble's switch, the answer, the closing buttons, the keys.
const BUBBLES_Y: f32 = 800.0;
const STATUS_Y: f32 = 840.0;
const CLOSE_Y: f32 = 898.0;
const KEYS_Y: f32 = 964.0;
const KEYS_PITCH: f32 = 32.0;
const BUTTON: f32 = 40.0;
const GAP: f32 = 8.0;
const FIELD: f32 = 44.0;
const LINE_CHARS: usize = 44;
/// The start's ring on the ground and its facing line, in world units.
const START_RING: f32 = 16.0;
const FACING: f32 = 40.0;
const END_RING: f32 = 10.0;
const RING_SIDES: usize = 16;

/// The SJK UI's 1080-line frame with its right edge on the window's.
fn frame(viewport: [f32; 2]) -> Frame {
    let s = crate::menu::sjk::scale(viewport);
    Frame {
        s,
        origin: [viewport[0] - 1920.0 * s, (viewport[1] - 1080.0 * s) * 0.5],
    }
}

fn push(ui: &mut MenuCanvas, command: DrawCommand) {
    let _ = ui.draw_list_mut().push(command);
}

/// Body text on a line of the column from `y`.
fn line(ui: &mut MenuCanvas, frame: &Frame, y: f32, value: std::fmt::Arguments<'_>, colour: Color) {
    text(
        ui,
        TextFamily::Body,
        value,
        frame.rect(COLUMN_X, y, COLUMN_WIDTH, 24.0),
        16.0 * frame.s,
        colour,
        FontWeight::Regular,
        TextAlign::Start,
    );
}

/// Seconds to a tenth.
fn seconds(millis: i32) -> f32 {
    millis as f32 / 1000.0
}

/// Draw `editor`'s column into `ui`, and keep where its list, timeline and tracks lie.
pub(super) fn build(editor: &mut Editor, ui: &mut MenuCanvas, viewport: [f32; 2]) {
    let frame = frame(viewport);
    ui.begin_transparent(viewport);
    scrim(ui, &frame, viewport);
    header(ui, &frame, editor);
    list(ui, &frame, editor);
    copy(ui, &frame, editor);
    cut(ui, &frame, editor);
    foot(ui, &frame, editor);
    keys(ui, &frame, editor);
    ui.finish(editor.focus);
}

/// The fade from clear to dark under the column, full height.
fn scrim(ui: &mut MenuCanvas, frame: &Frame, viewport: [f32; 2]) {
    let [width, height] = viewport;
    let x = |frame_x: f32| frame.point(frame_x, 0.0)[0];
    let space = |alpha| color::alpha(color::SPACE, alpha);
    let (start, dark) = (x(FADE_START).max(0.0), x(FADE_DARK));
    if dark > start {
        fade_across(
            ui,
            Rect::new(start, 0.0, dark - start, height),
            space(0.0),
            space(0.74),
        );
    }
    fade_across(
        ui,
        Rect::new(dark, 0.0, width - dark, height),
        space(0.74),
        space(0.88),
    );
}

fn header(ui: &mut MenuCanvas, frame: &Frame, editor: &Editor) {
    let s = frame.s;
    text(
        ui,
        TextFamily::Display,
        format_args!("Ghost editor"),
        frame.rect(COLUMN_X, TITLE_Y - 26.0, COLUMN_WIDTH, 52.0),
        42.0 * s,
        color::TEXT,
        FontWeight::Semibold,
        TextAlign::Start,
    );
    let here = editor
        .ghosts
        .iter()
        .filter(|ghost| {
            ghost
                .summary
                .as_ref()
                .is_ok_and(|(map, _)| map.eq_ignore_ascii_case(&editor.here))
        })
        .count();
    let map = if editor.here.is_empty() {
        "this map"
    } else {
        editor.here.as_str()
    };
    let rect = frame.rect(COLUMN_X, SUBTITLE_Y - 12.0, COLUMN_WIDTH, 24.0);
    let body = |ui: &mut MenuCanvas, value: std::fmt::Arguments<'_>| {
        text(
            ui,
            TextFamily::Body,
            value,
            rect,
            17.0 * s,
            color::MUTED,
            FontWeight::Regular,
            TextAlign::Start,
        );
    };
    match editor.ghosts.len() {
        0 => body(ui, format_args!("No ghosts yet: ghost record <name>")),
        count => body(
            ui,
            format_args!(
                "{count} ghost{}, {here} on {map}",
                if count == 1 { "" } else { "s" }
            ),
        ),
    }
}

fn ground(ui: &mut MenuCanvas, frame: &Frame, rect: [f32; 4], radius: f32) {
    let [x, y, width, height] = rect;
    let s = frame.s;
    push(
        ui,
        DrawCommand::RoundedRect {
            rect: frame.rect(x, y, width, height),
            radius: radius * s,
            color: color::alpha(color::SPACE, 0.6),
        },
    );
    push(
        ui,
        DrawCommand::Border {
            rect: frame.rect(x, y, width, height),
            radius: radius * s,
            width: 1.5 * s,
            color: color::alpha(color::HOLO, 0.35),
        },
    );
}

/// The saved runs: their names, maps and lengths, another map's quiet, the open one gold.
fn list(ui: &mut MenuCanvas, frame: &Frame, editor: &mut Editor) {
    let s = frame.s;
    kit::heading(ui, frame, COLUMN_X, LIST_HEADING_Y, COLUMN_WIDTH, "Ghosts");
    let height = LIST_ROW * GHOST_ROWS as f32 + 16.0;
    ground(ui, frame, [COLUMN_X, LIST_Y, COLUMN_WIDTH, height], 12.0);
    editor.list = Some(frame.rect(COLUMN_X, LIST_Y, COLUMN_WIDTH, height));
    if editor.ghosts.is_empty() {
        line(
            ui,
            frame,
            LIST_Y + 16.0,
            format_args!("Record one first: ghost record <name>"),
            color::MUTED,
        );
    }
    let open = editor.loaded.as_ref().map(|loaded| loaded.name.as_str());
    for slot in 0..GHOST_ROWS {
        let index = editor.list_first + slot;
        let Some(ghost) = editor.ghosts.get(index) else {
            break;
        };
        let top = LIST_Y + 8.0 + slot as f32 * LIST_ROW;
        let token = GHOST_ROW + slot as u16;
        let chosen = open.is_some_and(|open| open.eq_ignore_ascii_case(&ghost.name));
        let focused = editor.focus == token;
        if chosen || focused || ui.token_hovered(token) {
            push(
                ui,
                DrawCommand::RoundedRect {
                    rect: frame.rect(COLUMN_X + 6.0, top, COLUMN_WIDTH - 12.0, LIST_ROW),
                    radius: 8.0 * s,
                    color: color::alpha(color::HOLO, if focused { 0.2 } else { 0.1 }),
                },
            );
        }
        if chosen {
            push(
                ui,
                DrawCommand::RoundedRect {
                    rect: frame.rect(COLUMN_X + 6.0, top + 6.0, 3.0, LIST_ROW - 12.0),
                    radius: 1.5 * s,
                    color: color::GOLD_BRIGHT,
                },
            );
        }
        let here = ghost
            .summary
            .as_ref()
            .is_ok_and(|(map, _)| map.eq_ignore_ascii_case(&editor.here));
        let name_colour = match (chosen, here) {
            (true, _) => color::GOLD_BRIGHT,
            (false, true) => color::TEXT,
            (false, false) => color::QUIET,
        };
        let text_rect =
            |x: f32, width: f32| frame.rect(x, top + (LIST_ROW - 22.0) * 0.5, width, 22.0);
        text(
            ui,
            TextFamily::Body,
            format_args!("{}", ghost.name),
            text_rect(COLUMN_X + 18.0, 210.0),
            15.0 * s,
            name_colour,
            FontWeight::Regular,
            TextAlign::Start,
        );
        let detail_rect = text_rect(COLUMN_X + 228.0, COLUMN_WIDTH - 242.0);
        let detail = |ui: &mut MenuCanvas, value: std::fmt::Arguments<'_>, colour: Color| {
            text(
                ui,
                TextFamily::Body,
                value,
                detail_rect,
                14.0 * s,
                colour,
                FontWeight::Regular,
                TextAlign::End,
            );
        };
        match &ghost.summary {
            Ok((map, millis)) => detail(
                ui,
                format_args!("{map}  {:.1} s", seconds(*millis)),
                if here { color::MUTED } else { color::QUIET },
            ),
            Err(_) => detail(ui, format_args!("cannot be read"), color::EMBER),
        }
        ui.hit_region(
            token,
            frame.rect(COLUMN_X + 6.0, top, COLUMN_WIDTH - 12.0, LIST_ROW),
        );
    }
}

/// The copy's name and Duplicate.
fn copy(ui: &mut MenuCanvas, frame: &Frame, editor: &Editor) {
    let field = [COLUMN_X, COPY_Y, 260.0, FIELD];
    let focused = editor.focus == NAME_FIELD;
    let open = editor.loaded.is_some();
    if editor.copy_name.is_empty() && !focused {
        kit::field(ui, frame, field, format_args!("Copy's name"), false, false);
    } else {
        kit::field(
            ui,
            frame,
            field,
            format_args!("{}{}", editor.copy_name, if focused { "|" } else { "" }),
            focused,
            false,
        );
    }
    ui.hit_region(
        NAME_FIELD,
        frame.rect(field[0], field[1], field[2], field[3]),
    );
    kit::button(
        ui,
        frame,
        [COLUMN_X + 268.0, COPY_Y, COLUMN_WIDTH - 268.0, FIELD],
        "Duplicate",
        false,
        open,
        editor.focus == DUPLICATE,
        DUPLICATE,
    );
}

/// The cut: the timeline, the start's and the end's rows, the buttons that place them, and
/// a note on a start that is not clean.
fn cut(ui: &mut MenuCanvas, frame: &Frame, editor: &mut Editor) {
    let s = frame.s;
    // The rule stops short of the length on its right.
    kit::heading(
        ui,
        frame,
        COLUMN_X,
        CUT_HEADING_Y,
        COLUMN_WIDTH - 180.0,
        "Cut",
    );
    editor.timeline = None;
    editor.start_track = None;
    editor.end_track = None;
    let Some(loaded) = &editor.loaded else {
        line(
            ui,
            frame,
            TIMELINE_Y + 6.0,
            format_args!("Choose a ghost in the list to cut it."),
            color::MUTED,
        );
        return;
    };
    let kept = loaded.kept();
    let total = loaded.cut.duration();
    text(
        ui,
        TextFamily::Body,
        format_args!("keeps {:.1} s of {:.1} s", seconds(kept), seconds(total)),
        frame.rect(
            COLUMN_X + 120.0,
            CUT_HEADING_Y - 12.0,
            COLUMN_WIDTH - 120.0,
            24.0,
        ),
        15.0 * s,
        color::MUTED,
        FontWeight::Regular,
        TextAlign::End,
    );
    // The timeline: what each slice of the run does, the cut parts dimmed, the handles.
    ground(
        ui,
        frame,
        [COLUMN_X, TIMELINE_Y, COLUMN_WIDTH, TIMELINE_HEIGHT],
        8.0,
    );
    let inner_x = COLUMN_X + 6.0;
    let inner_width = COLUMN_WIDTH - 12.0;
    editor.timeline = Some(frame.rect(inner_x, TIMELINE_Y, inner_width, TIMELINE_HEIGHT));
    let columns = &loaded.columns;
    let slice = inner_width / columns.len() as f32;
    let mut at = 0;
    while at < columns.len() {
        let motion = columns[at].0;
        let run = columns[at..]
            .iter()
            .take_while(|column| column.0 == motion)
            .count();
        let colour = match motion {
            Motion::Still => color::alpha(color::HOLO, 0.14),
            Motion::Ground => color::alpha(color::HOLO, 0.5),
            Motion::Air => color::alpha(color::HOLO, 0.85),
            Motion::Talk if loaded.clear_talk => color::alpha(color::EMBER, 0.35),
            Motion::Talk => color::alpha(color::EMBER, 0.85),
        };
        push(
            ui,
            DrawCommand::SolidRect {
                rect: frame.rect(
                    inner_x + at as f32 * slice,
                    TIMELINE_Y + 7.0,
                    run as f32 * slice,
                    TIMELINE_HEIGHT - 14.0,
                ),
                color: colour,
            },
        );
        at += run;
    }
    // Where the start can land cleanly.
    let mut at = 0;
    while at < columns.len() {
        let clean = columns[at].1;
        let run = columns[at..]
            .iter()
            .take_while(|column| column.1 == clean)
            .count();
        if clean {
            push(
                ui,
                DrawCommand::SolidRect {
                    rect: frame.rect(
                        inner_x + at as f32 * slice,
                        CLEAN_Y,
                        run as f32 * slice,
                        3.0,
                    ),
                    color: color::alpha(color::TRUSTED, 0.8),
                },
            );
        }
        at += run;
    }
    let x_of =
        |index: usize| inner_x + inner_width * loaded.cut.time(index) as f32 / total.max(1) as f32;
    let (start_x, end_x) = (x_of(loaded.first), x_of(loaded.last));
    for (from, to) in [(inner_x, start_x), (end_x, inner_x + inner_width)] {
        if to > from {
            push(
                ui,
                DrawCommand::SolidRect {
                    rect: frame.rect(from, TIMELINE_Y + 3.0, to - from, TIMELINE_HEIGHT - 6.0),
                    color: color::alpha(color::SPACE, 0.7),
                },
            );
        }
    }
    let clean = loaded.cut.is_clean(loaded.first);
    for (x, colour) in [
        (
            start_x,
            if clean {
                color::GOLD_BRIGHT
            } else {
                color::EMBER
            },
        ),
        (end_x, color::GOLD),
    ] {
        push(
            ui,
            DrawCommand::RoundedRect {
                rect: frame.rect(x - 1.5, TIMELINE_Y - 3.0, 3.0, TIMELINE_HEIGHT + 6.0),
                radius: 1.5 * s,
                color: colour,
            },
        );
    }
    // The start's and the end's rows.
    let start = loaded.cut.time(loaded.first);
    let end = loaded.cut.time(loaded.last);
    let start_state = if clean { "standing" } else { "moving" };
    // Exo 2 at 19 for the names, Rajdhani at 21 for the values, as Camera control's rows.
    let ratio = |time: i32| time as f32 / total.max(1) as f32;
    editor.start_track = Some(handle_row(
        ui,
        frame,
        START_Y,
        START,
        "Start",
        format_args!("{:.2} s, {start_state}", seconds(start)),
        ratio(start),
        editor.focus == START,
    ));
    editor.end_track = Some(handle_row(
        ui,
        frame,
        END_Y,
        END,
        "End",
        format_args!("{:.2} s", seconds(end)),
        ratio(end),
        editor.focus == END,
    ));
    let width = (COLUMN_WIDTH - GAP * 2.0) / 3.0;
    for (index, (token, label)) in [
        (FIRST_MOVE, "First move"),
        (LAST_MOVE, "Last move"),
        (RESET, "Reset"),
    ]
    .into_iter()
    .enumerate()
    {
        kit::button(
            ui,
            frame,
            [
                COLUMN_X + index as f32 * (width + GAP),
                MOVES_Y,
                width,
                BUTTON,
            ],
            label,
            false,
            true,
            editor.focus == token,
            token,
        );
    }
    if !clean {
        line(
            ui,
            frame,
            NOTE_Y,
            format_args!("Starts mid-move: the replay may drift or get lost"),
            color::EMBER,
        );
    } else if !editor.here.is_empty() && !loaded.cut.map().eq_ignore_ascii_case(&editor.here) {
        line(
            ui,
            frame,
            NOTE_Y,
            format_args!("Recorded on {}: its path shows there", loaded.cut.map()),
            color::MUTED,
        );
    }
}

/// A handle's row of 56 from `y`: its name and value on one line, the kit's slider under
/// them; the slider's track (window pixels) for drags.
#[allow(clippy::too_many_arguments)]
fn handle_row(
    ui: &mut MenuCanvas,
    frame: &Frame,
    y: f32,
    token: u16,
    label: &str,
    value: std::fmt::Arguments<'_>,
    ratio: f32,
    focused: bool,
) -> Rect {
    let s = frame.s;
    if focused {
        kit::band(
            ui,
            frame,
            [COLUMN_X - 16.0, y - 2.0, COLUMN_WIDTH + 32.0, ROW],
        );
    }
    text(
        ui,
        TextFamily::Body,
        format_args!("{label}"),
        frame.rect(COLUMN_X, y + 2.0, 160.0, 26.0),
        19.0 * s,
        if focused {
            color::TEXT
        } else {
            color::alpha(color::TEXT, 0.88)
        },
        FontWeight::Regular,
        TextAlign::Start,
    );
    text(
        ui,
        TextFamily::Display,
        value,
        frame.rect(COLUMN_X + 120.0, y + 2.0, COLUMN_WIDTH - 120.0, 26.0),
        21.0 * s,
        color::TEXT,
        FontWeight::Regular,
        TextAlign::End,
    );
    let track_y = y + 40.0;
    kit::slider(
        ui,
        frame,
        COLUMN_X + 8.0,
        track_y,
        COLUMN_WIDTH - 16.0,
        ratio,
        focused,
    );
    ui.hit_region(token, frame.rect(COLUMN_X, y, COLUMN_WIDTH, ROW - 4.0));
    frame.rect(COLUMN_X + 8.0, track_y - 12.0, COLUMN_WIDTH - 16.0, 24.0)
}

/// The bubble's switch, the last answer, then Save, Play, Look and Close.
fn foot(ui: &mut MenuCanvas, frame: &Frame, editor: &Editor) {
    let s = frame.s;
    let loaded = editor.loaded.as_ref();
    let focused = editor.focus == BUBBLES;
    if focused {
        kit::band(
            ui,
            frame,
            [COLUMN_X - 16.0, BUBBLES_Y - 22.0, COLUMN_WIDTH + 32.0, 44.0],
        );
    }
    let bubbles = loaded.map_or(0, |loaded| loaded.bubbles());
    let label = |ui: &mut MenuCanvas, value: std::fmt::Arguments<'_>| {
        text(
            ui,
            TextFamily::Body,
            value,
            frame.rect(COLUMN_X, BUBBLES_Y - 14.0, 300.0, 28.0),
            19.0 * s,
            if focused {
                color::TEXT
            } else {
                color::alpha(color::TEXT, 0.88)
            },
            FontWeight::Regular,
            TextAlign::Start,
        );
    };
    match bubbles {
        0 => label(ui, format_args!("Clear the chat bubble")),
        1 => label(ui, format_args!("Clear the chat bubble (once)")),
        count => label(ui, format_args!("Clear the chat bubble ({count} times)")),
    }
    kit::switch(
        ui,
        frame,
        COLUMN_X + COLUMN_WIDTH,
        BUBBLES_Y,
        loaded.is_some_and(|loaded| loaded.clear_talk),
        focused,
    );
    ui.hit_region(
        BUBBLES,
        frame.rect(COLUMN_X, BUBBLES_Y - 20.0, COLUMN_WIDTH, 40.0),
    );
    let status = if !editor.status.is_empty() {
        Some((
            editor.status.as_str(),
            if editor.status_error {
                color::EMBER
            } else {
                color::TEXT
            },
        ))
    } else if loaded.is_some_and(Loaded::changed) {
        Some((
            "Not saved: Save overwrites it, Duplicate keeps it",
            color::MUTED,
        ))
    } else {
        None
    };
    if let Some((status, colour)) = status {
        for (index, part) in wrap(status, LINE_CHARS).take(2).enumerate() {
            line(
                ui,
                frame,
                STATUS_Y + index as f32 * 22.0,
                format_args!("{part}"),
                colour,
            );
        }
    }
    let changed = loaded.is_some_and(Loaded::changed);
    let width = (COLUMN_WIDTH - GAP * 3.0) / 4.0;
    for (index, (token, label, primary, enabled)) in [
        (SAVE, "Save", true, changed),
        (PLAY, "Play", false, loaded.is_some() && !changed),
        (LOOK, "Look", false, true),
        (CLOSE, "Close", false, true),
    ]
    .into_iter()
    .enumerate()
    {
        kit::button(
            ui,
            frame,
            [
                COLUMN_X + index as f32 * (width + GAP),
                CLOSE_Y,
                width,
                44.0,
            ],
            label,
            primary,
            enabled,
            editor.focus == token,
            token,
        );
    }
}

/// One key hint: its caps and what they do.
type Hint<'a> = (&'a [&'a str], &'a str);

/// Hints right-aligned on the line whose top is `y` (frame pixels), ending at `right`.
fn hint_line(ui: &mut MenuCanvas, frame: &Frame, right: f32, y: f32, hints: &[Hint<'_>]) {
    let s = frame.s;
    let gap = 20.0 * s;
    let total = hints
        .iter()
        .map(|(caps, action)| key_hint_width(caps, action, s))
        .sum::<f32>()
        + gap * hints.len().saturating_sub(1) as f32;
    let [right, y] = frame.point(right, y);
    let mut x = right - total;
    for (caps, action) in hints {
        x = key_hint(ui, caps, action, x, y, s) + gap;
    }
}

/// The keys of what has the keyboard, two lines at the column's foot.
fn keys(ui: &mut MenuCanvas, frame: &Frame, editor: &Editor) {
    let right = COLUMN_X + COLUMN_WIDTH;
    let first: &[Hint<'_>] = match editor.focus {
        START => &[(&["Left", "Right"], "snap"), (&["Shift"], "free")],
        END => &[(&["Left", "Right"], "move"), (&["Shift"], "one command")],
        NAME_FIELD => &[(&["Enter"], "duplicate")],
        _ => &[(&["Enter"], "act"), (&["Ctrl", "S"], "save")],
    };
    hint_line(ui, frame, right, KEYS_Y, first);
    let second: &[Hint<'_>] = if matches!(editor.focus, START | END) {
        &[(&["Page Up", "Page Down"], "a second"), (&["Esc"], "look")]
    } else {
        &[(&["Up", "Down"], "choose"), (&["Esc"], "look")]
    };
    hint_line(ui, frame, right, KEYS_Y + KEYS_PITCH, second);
}

/// The run's path over the world (under the column, or over play while looking about),
/// and the strip at the top while looking.
pub(super) fn build_overlay(editor: &mut Editor, camera: Option<Camera>, viewport: [f32; 2]) {
    let looking = !editor.panel_open;
    let here = editor.here.clone();
    let Editor {
        loaded, overlay, ..
    } = editor;
    overlay.begin_transparent(viewport);
    if let (Some(loaded), Some(camera)) = (loaded.as_ref(), camera)
        && (here.is_empty() || loaded.cut.map().eq_ignore_ascii_case(&here))
    {
        path(overlay, loaded, camera, viewport);
    }
    if looking {
        strip(
            overlay,
            loaded.as_ref().map(|loaded| loaded.name.as_str()),
            viewport,
        );
    }
    overlay.finish(0);
}

fn path(ui: &mut MenuCanvas, loaded: &Loaded, camera: Camera, viewport: [f32; 2]) {
    let s = crate::menu::sjk::scale(viewport);
    let project = |at: [f32; 3]| camera.project_within(Vec3::from_array(at), 3.0);
    let (first, last) = (loaded.first as u32, loaded.last as u32);
    let mut previous: Option<[f32; 2]> = None;
    for pair in loaded.path.windows(2) {
        let [from, to] = [pair[0], pair[1]];
        let start = previous.or_else(|| project(from.at));
        let end = project(to.at);
        previous = end;
        let (Some(start), Some(end)) = (start, end) else {
            continue;
        };
        let kept = to.index > first && from.index < last;
        let (width, colour) = match (kept, to.talk && !loaded.clear_talk) {
            (true, true) => (3.0, color::alpha(color::EMBER, 0.9)),
            (true, false) => (3.0, color::alpha(color::HOLO, 0.9)),
            (false, _) => (2.0, color::alpha(color::QUIET, 0.45)),
        };
        push(
            ui,
            DrawCommand::Line {
                from: start,
                to: end,
                width: width * s,
                color: colour,
            },
        );
    }
    // The new start: a ring where the replay walks to, and the way it faces.
    let feet = |origin: [f32; 3]| [origin[0], origin[1], origin[2] - 24.0];
    let start = loaded.cut.start_at(loaded.first);
    let centre = feet(start.origin);
    let colour = if loaded.cut.is_clean(loaded.first) {
        color::GOLD_BRIGHT
    } else {
        color::EMBER
    };
    ring(ui, &project, centre, START_RING, 3.0 * s, colour);
    let yaw = super::short_to_degrees(start.view[1]).to_radians();
    let ahead = [
        centre[0] + yaw.cos() * FACING,
        centre[1] + yaw.sin() * FACING,
        centre[2],
    ];
    if let (Some(from), Some(to)) = (project(centre), project(ahead)) {
        push(
            ui,
            DrawCommand::Line {
                from,
                to,
                width: 3.0 * s,
                color: colour,
            },
        );
    }
    ring(
        ui,
        &project,
        feet(loaded.cut.origin(loaded.last)),
        END_RING,
        2.0 * s,
        color::alpha(color::TEXT, 0.85),
    );
}

/// A flat ring of `radius` world units round `centre`.
fn ring(
    ui: &mut MenuCanvas,
    project: &impl Fn([f32; 3]) -> Option<[f32; 2]>,
    centre: [f32; 3],
    radius: f32,
    width: f32,
    colour: Color,
) {
    let point = |side: usize| {
        let angle = side as f32 / RING_SIDES as f32 * std::f32::consts::TAU;
        project([
            centre[0] + angle.cos() * radius,
            centre[1] + angle.sin() * radius,
            centre[2],
        ])
    };
    for side in 0..RING_SIDES {
        if let (Some(from), Some(to)) = (point(side), point(side + 1)) {
            push(
                ui,
                DrawCommand::Line {
                    from,
                    to,
                    width,
                    color: colour,
                },
            );
        }
    }
}

/// The strip at the top of the screen while the player looks about: what is on, and
/// Escape to bring the column back.
fn strip(ui: &mut MenuCanvas, name: Option<&str>, viewport: [f32; 2]) {
    let frame = Frame::new(viewport);
    let s = frame.s;
    let (x, y, width, height) = (660.0, 28.0, 600.0, 64.0);
    push(
        ui,
        DrawCommand::RoundedRect {
            rect: frame.rect(x, y, width, height),
            radius: 14.0 * s,
            color: color::alpha(color::SPACE, 0.78),
        },
    );
    push(
        ui,
        DrawCommand::Border {
            rect: frame.rect(x, y, width, height),
            radius: 14.0 * s,
            width: 1.5 * s,
            color: color::alpha(color::HOLO, 0.4),
        },
    );
    text(
        ui,
        TextFamily::Display,
        format_args!("Ghost editor"),
        frame.rect(x + 20.0, y + 8.0, 300.0, 24.0),
        20.0 * s,
        color::GOLD_BRIGHT,
        FontWeight::Semibold,
        TextAlign::Start,
    );
    let detail = |ui: &mut MenuCanvas, value: std::fmt::Arguments<'_>| {
        text(
            ui,
            TextFamily::Body,
            value,
            frame.rect(x + 20.0, y + 34.0, width - 40.0, 22.0),
            15.0 * s,
            color::MUTED,
            FontWeight::Regular,
            TextAlign::Start,
        );
    };
    match name {
        Some(name) => detail(
            ui,
            format_args!("The path of {name}, its start ringed in gold"),
        ),
        None => detail(ui, format_args!("No ghost open")),
    }
    let (caps, action) = (["Esc"], "back to the editor");
    let right = frame.point(x + width - 20.0, y + 8.0);
    let start = right[0] - key_hint_width(&caps, action, s);
    key_hint(ui, &caps, action, start, right[1], s);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const VIEWPORTS: [[f32; 2]; 4] = [
        [1_920.0, 1_080.0],
        [3_840.0, 2_160.0],
        [2_560.0, 1_080.0],
        [1_024.0, 768.0],
    ];

    fn states() -> Vec<(&'static str, Editor)> {
        let full = super::super::editor::tests::editor_with(PathBuf::new());
        let mut cut = super::super::editor::tests::editor_with(PathBuf::new());
        let _ = cut.activate(FIRST_MOVE);
        let _ = cut.activate(LAST_MOVE);
        cut.status =
            "Save again to overwrite pillars; Duplicate keeps the original, and wraps".into();
        let mut free = super::super::editor::tests::editor_with(PathBuf::new());
        free.focus = START;
        if let Some(loaded) = &mut free.loaded {
            loaded.first = 130;
        }
        let mut many = super::super::editor::tests::editor_with(PathBuf::new());
        many.ghosts = (0..20)
            .map(|index| Listed {
                name: format!("a_rather_long_ghost_name_{index}"),
                summary: if index == 3 {
                    Err("broken".into())
                } else {
                    Ok(("mp/ffa5".into(), 61_000))
                },
            })
            .collect();
        many.here = "mp/ffa3".into();
        let mut empty = Editor::new(PathBuf::new());
        empty.copy_name.clear();
        vec![
            ("full", full),
            ("cut", cut),
            ("free", free),
            ("many", many),
            ("empty", empty),
        ]
    }

    #[test]
    fn the_column_fits_every_window_and_its_controls_end_above_the_foot() {
        for viewport in VIEWPORTS {
            for (state, mut editor) in states() {
                let mut canvas = MenuCanvas::new();
                build(&mut editor, &mut canvas, viewport);
                assert!(!canvas.overflowed(), "{state} {viewport:?}");
                for token in [NAME_FIELD, DUPLICATE, BUBBLES, SAVE, PLAY, LOOK, CLOSE] {
                    assert!(
                        canvas.rect_for(token).is_some(),
                        "{state} {viewport:?}: {token}"
                    );
                }
                let foot = frame(viewport).point(0.0, BUBBLES_Y - 22.0)[1];
                let column = frame(viewport).point(COLUMN_X - 0.5, 0.0)[0];
                for token in canvas.widget_tokens().to_vec() {
                    let rect = canvas.rect_for(token).expect("a registered token");
                    if !(BUBBLES..GHOST_ROW).contains(&token) {
                        assert!(
                            rect.bottom() <= foot + 0.01,
                            "{state} {viewport:?}: {token}"
                        );
                    }
                    assert!(
                        rect.x >= column
                            && rect.y >= 0.0
                            && rect.right() <= viewport[0] + 0.01
                            && rect.bottom() <= viewport[1] + 0.01,
                        "{state} {viewport:?}: token {token} at {rect:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_run_shows_its_handles_and_a_mid_move_start_warns() {
        let text_of = |state: &str| {
            let (_, mut editor) = states()
                .into_iter()
                .find(|(name, _)| *name == state)
                .unwrap();
            let mut canvas = MenuCanvas::new();
            build(&mut editor, &mut canvas, [1_920.0, 1_080.0]);
            let runs: Vec<String> = canvas.text_runs().map(str::to_owned).collect();
            let tokens = canvas.widget_tokens().to_vec();
            (runs, tokens, editor)
        };
        let (runs, tokens, editor) = text_of("full");
        assert!(tokens.contains(&START) && tokens.contains(&END) && tokens.contains(&RESET));
        assert!(runs.iter().any(|run| run == "keeps 2.0 s of 2.0 s"));
        assert!(runs.iter().any(|run| run == "Clear the chat bubble (once)"));
        assert!(editor.timeline.is_some() && editor.start_track.is_some());
        let (runs, ..) = text_of("cut");
        assert!(runs.iter().any(|run| run == "1.00 s, standing"));
        assert!(runs.iter().any(|run| run == "keeps 0.6 s of 2.0 s"));
        let (runs, ..) = text_of("free");
        assert!(runs.iter().any(|run| run.starts_with("Starts mid-move")));
        let (runs, tokens, editor) = text_of("empty");
        assert!(!tokens.contains(&START));
        assert!(editor.timeline.is_none());
        assert!(runs.iter().any(|run| run.starts_with("No ghosts yet")));
        let (runs, tokens, _) = text_of("many");
        assert!(tokens.contains(&(GHOST_ROW + GHOST_ROWS as u16 - 1)));
        assert!(!tokens.contains(&(GHOST_ROW + GHOST_ROWS as u16)));
        assert!(runs.iter().any(|run| run == "cannot be read"));
    }

    #[test]
    fn a_press_on_the_timeline_takes_the_nearer_handle_and_drags_it() {
        use sjk_ui::{InputEvent, PointerButton, Vec2};
        let (_, mut editor) = states().remove(1);
        let mut canvas = MenuCanvas::new();
        build(&mut editor, &mut canvas, [1_920.0, 1_080.0]);
        let timeline = editor.timeline.expect("a timeline");
        let at = |ratio: f32| Vec2::new(timeline.x + timeline.width * ratio, timeline.y + 10.0);
        // The end is at 0.8 of the run: a press at 0.95 takes it to the end.
        let button = PointerButton::Primary;
        let _ = editor.pointer(
            &mut canvas,
            InputEvent::PointerPress {
                position: at(0.95),
                button,
            },
            false,
        );
        assert_eq!(editor.focus, END);
        let _ = editor.pointer(&mut canvas, InputEvent::PointerMove(at(1.0)), false);
        let _ = editor.pointer(
            &mut canvas,
            InputEvent::PointerRelease {
                position: at(1.0),
                button,
            },
            false,
        );
        let loaded = editor.loaded.as_ref().unwrap();
        assert_eq!(loaded.last, loaded.cut.len() - 1);
    }

    #[test]
    fn the_overlay_draws_the_path_and_the_strip_only_while_looking() {
        let (_, mut editor) = states().remove(1);
        let camera = Camera {
            eye: Vec3::new(1_000.0, -400.0, 200.0),
            target: Vec3::new(1_000.0, 0.0, 0.0),
            up: Vec3::Z,
            fov: 90.0,
            viewport: [1_920.0, 1_080.0],
        };
        editor.here = "mp/ffa3".into();
        editor.panel_open = true;
        build_overlay(&mut editor, Some(camera), [1_920.0, 1_080.0]);
        assert!(!editor.overlay.overflowed());
        let lines = editor.overlay.draw_list().len();
        assert!(lines > RING_SIDES, "the path and its rings: {lines}");
        assert_eq!(editor.overlay.text_runs().count(), 0);
        editor.panel_open = false;
        build_overlay(&mut editor, Some(camera), [1_920.0, 1_080.0]);
        assert!(editor.overlay.text_runs().any(|run| run == "Ghost editor"));
        // Another map's run draws no path.
        editor.here = "mp/ffa5".into();
        editor.panel_open = true;
        build_overlay(&mut editor, Some(camera), [1_920.0, 1_080.0]);
        assert_eq!(editor.overlay.draw_list().len(), 0);
    }
}
