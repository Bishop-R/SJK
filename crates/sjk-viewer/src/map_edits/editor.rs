//! The in-game map editor, for staff: console commands that act on what the world-note
//! key selected (or, without a selection, on what is under the crosshair), saved to the
//! map's document at once and sent to the hub with `mapedit_publish`.
//!
//! While `mapedit` is on, the world-note key only selects (its outline shows what an
//! edit will touch); it opens no note. Every command that changes the map's edits is one
//! step of [`History`], which `mapedit_undo` and `mapedit_redo` walk.

use super::document::{Edit, Kind, Scope};
use crate::GpuState;
use crate::world_notes::{EditTarget, Scene};
use glam::Vec3;

/// How many surfaces `mapedit_hide group` takes at most.
const GROUP_LIMIT: usize = 256;
/// Degrees `mapedit_turn` turns by without a number, and a notch of the wheel turns a
/// grabbed model by.
const TURN_STEP: f32 = 15.0;
/// Steps [`History`] keeps each way.
const HISTORY_STEPS: usize = 100;
/// The commands that may change the map's edits: each one that does is a step to undo.
const CHANGES: &[&str] = &[
    "mapedit_hide",
    "mapedit_place",
    "mapedit_turn",
    "mapedit_move",
    "mapedit_scale",
    "mapedit_grab",
    "mapedit_remap",
    "mapedit_delete",
];

/// The editor's steps on one map: the edits as they were before each change, to undo,
/// and as they were before each undo, to redo. Cleared when the map changes or the hub
/// brings another staff member's version.
#[derive(Default)]
pub(crate) struct History {
    undo: Vec<Vec<Edit>>,
    redo: Vec<Vec<Edit>>,
}

impl History {
    /// A change made the edits differ from `before`: a new step, and nothing to redo.
    fn changed(&mut self, before: Vec<Edit>) {
        if self.undo.len() == HISTORY_STEPS {
            self.undo.remove(0);
        }
        self.undo.push(before);
        self.redo.clear();
    }

    pub(crate) fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    /// Whether there is a step to undo, and one to redo.
    pub(crate) fn can(&self) -> (bool, bool) {
        (!self.undo.is_empty(), !self.redo.is_empty())
    }
}

/// The edit a step changes between `from` and `to`, as `to` has it when it is there: what
/// an undo or a redo names.
fn stepped<'a>(from: &'a [Edit], to: &'a [Edit]) -> Option<&'a Edit> {
    to.iter()
        .find(|edit| !from.contains(edit))
        .or_else(|| from.iter().find(|edit| !to.contains(edit)))
}

/// A path's last part: `models/a/door.md3` is `door.md3`.
pub(crate) fn short_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The editor's state in one world.
pub(crate) struct Editor {
    pub(crate) active: bool,
    /// Where new edits apply.
    scope: Scope,
    /// The placed model `mapedit_turn`, `_move`, `_scale` and `_grab` act on.
    current: Option<u32>,
    /// `mapedit_grab`: the current placed model follows the crosshair.
    grabbing: bool,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            active: false,
            scope: Scope::AllServers,
            current: None,
            grabbing: false,
        }
    }
}

impl Editor {
    /// The placed model the panel's controls act on.
    pub(crate) fn current(&self) -> Option<u32> {
        self.current
    }

    /// Whether the current placed model follows the aim.
    pub(crate) fn grabbing(&self) -> bool {
        self.grabbing
    }

    /// Where new edits apply.
    pub(crate) fn scope(&self) -> Scope {
        self.scope
    }

    /// Make placed edit `id` the one the placed model's controls act on.
    pub(crate) fn set_current(&mut self, id: u32) {
        if self.current != Some(id) {
            self.current = Some(id);
            self.grabbing = false;
        }
    }
}

/// The console commands, with their help.
pub(crate) const COMMANDS: &[(&str, &str)] = &[
    ("mapedit", "Staff: turn the map editor on or off"),
    (
        "mapedit_panel",
        "Map editor: open its panel (the world-note key does, on what you aim at)",
    ),
    (
        "mapedit_hide",
        "Map editor: hide the selected prop or surface ('group' takes the whole compiled model), or remove a placed model",
    ),
    (
        "mapedit_place",
        "Map editor: place a model (a path, or a copy of the selected one) where you aim",
    ),
    (
        "mapedit_turn",
        "Map editor: turn the current placed model (degrees, 15 by default)",
    ),
    (
        "mapedit_move",
        "Map editor: move the current placed model (x y z, or one number up)",
    ),
    (
        "mapedit_scale",
        "Map editor: scale the current placed model by a factor",
    ),
    (
        "mapedit_grab",
        "Map editor: the current placed model follows your aim until mapedit_grab again",
    ),
    (
        "mapedit_remap",
        "Map editor: draw a shader instead of the selected one ('-' removes the remap)",
    ),
    (
        "mapedit_scope",
        "Map editor: new edits for all servers or only this server",
    ),
    (
        "mapedit_undo",
        "Map editor: undo the last change to this map's edits",
    ),
    (
        "mapedit_redo",
        "Map editor: redo the change mapedit_undo took back",
    ),
    ("mapedit_list", "Map editor: list this map's edits"),
    ("mapedit_delete", "Map editor: remove the edit of an id"),
    (
        "mapedit_publish",
        "Map editor: send this map's edits to the hub for every SJK player",
    ),
];

/// Whether `name` (lower case) is a map editor command.
pub(crate) fn is_command(name: &str) -> bool {
    COMMANDS.iter().any(|(command, _)| *command == name)
}

/// What an edit is aimed at.
struct Aim {
    target: EditTarget,
    hit: Vec3,
    summary: String,
}

fn yaw_of(forward: Vec3) -> f32 {
    forward.y.atan2(forward.x).to_degrees()
}

fn number(text: &str) -> Result<f32, String> {
    text.parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("not a number: {text}"))
}

impl GpuState {
    /// A `mapedit*` command.
    pub(crate) fn map_edit_command(
        &mut self,
        name: &str,
        args: &[String],
    ) -> Result<Vec<String>, String> {
        if !crate::player_identity::is_staff() {
            return Err("The map editor is for SJK staff".into());
        }
        if name == "mapedit" {
            return self.map_edit_toggle(args);
        }
        if name == "mapedit_panel" {
            if !self.map_edits.editor.active {
                self.map_edit_toggle(&[])?;
            }
            self.open_map_edit_panel();
            return Ok(Vec::new());
        }
        if !self.map_edits.editor.active {
            return Err("Turn the map editor on first: mapedit".into());
        }
        let before = CHANGES.contains(&name).then(|| self.map_edit_edits());
        let result = self.map_edit_change(name, args);
        if let Some(before) = before
            && result.is_ok()
            && before != self.map_edit_edits()
        {
            self.map_edits.history.changed(before);
        }
        result
    }

    /// This map's edits as they stand (none without a document).
    fn map_edit_edits(&self) -> Vec<Edit> {
        self.map_edits
            .document
            .as_ref()
            .map_or_else(Vec::new, |document| document.edits.clone())
    }

    /// A command of the editor while it is on.
    fn map_edit_change(&mut self, name: &str, args: &[String]) -> Result<Vec<String>, String> {
        match name {
            "mapedit_hide" => self.map_edit_hide(args),
            "mapedit_place" => self.map_edit_place(args),
            "mapedit_turn" => {
                let degrees = match args {
                    [] => TURN_STEP,
                    [degrees] => number(degrees)?,
                    _ => return Err("usage: mapedit_turn [degrees]".into()),
                };
                self.map_edit_transform(|_, angles, _| angles[1] = (angles[1] + degrees) % 360.0)
            }
            "mapedit_move" => {
                let offset = match args {
                    [up] => [0.0, 0.0, number(up)?],
                    [x, y, z] => [number(x)?, number(y)?, number(z)?],
                    _ => return Err("usage: mapedit_move <up> | <x> <y> <z>".into()),
                };
                self.map_edit_transform(|origin, _, _| {
                    for axis in 0..3 {
                        origin[axis] += offset[axis];
                    }
                })
            }
            "mapedit_scale" => {
                let [factor] = args else {
                    return Err("usage: mapedit_scale <factor>".into());
                };
                let factor = number(factor)?;
                if factor <= 0.0 {
                    return Err("the factor must be above 0".into());
                }
                self.map_edit_transform(|_, _, scale| {
                    for axis in scale.iter_mut() {
                        *axis = (*axis * factor).clamp(0.01, 64.0);
                    }
                })
            }
            "mapedit_grab" => self.map_edit_grab(),
            "mapedit_remap" => self.map_edit_remap(args),
            "mapedit_scope" => self.map_edit_scope(args),
            "mapedit_undo" => self.map_edit_step(true),
            "mapedit_redo" => self.map_edit_step(false),
            "mapedit_delete" => {
                let [id] = args else {
                    return Err("usage: mapedit_delete <id> (mapedit_list shows them)".into());
                };
                let id = id.parse().map_err(|_| format!("not an id: {id}"))?;
                self.map_edit_delete(id)
            }
            "mapedit_list" => Ok(self.map_edit_list()),
            "mapedit_publish" => self.map_edit_publish(),
            _ => Err(format!("unknown map editor command {name}")),
        }
    }

    fn map_edit_toggle(&mut self, args: &[String]) -> Result<Vec<String>, String> {
        let active = match args {
            [] => !self.map_edits.editor.active,
            [value] if matches!(value.as_str(), "1" | "on") => true,
            [value] if matches!(value.as_str(), "0" | "off") => false,
            _ => return Err("usage: mapedit [on|off]".into()),
        };
        let editor = &mut self.map_edits.editor;
        editor.active = active;
        editor.grabbing = false;
        self.world_notes.hold_selection(active);
        if !active {
            self.world_notes.cancel_selection();
            self.close_map_edit_panel();
            return Ok(vec!["Map editor off".into()]);
        }
        // Start from the hub's latest copies.
        crate::player_identity::refresh_map_edits();
        // A map without a document has no checksum read yet: read it now.
        if self.map_edits.map().is_none() {
            self.map_edits.lookup = super::Lookup::Idle;
        }
        // In a world, its panel opens on what is under the crosshair.
        if self.live_session.is_some() || self.resident.exploring() {
            self.map_edit_pick();
            self.open_map_edit_panel();
        }
        Ok(vec![
            format!(
                "Map editor on ({}). The world-note key selects and opens its panel; or:",
                self.map_edit_scope_text()
            ),
            "  mapedit_hide [group], mapedit_place [model], mapedit_remap <shader|->".into(),
            "  mapedit_turn [deg], mapedit_move <up|x y z>, mapedit_scale <f>, mapedit_grab".into(),
            "  mapedit_scope [all|server], mapedit_list, mapedit_delete <id>".into(),
            "  mapedit_undo, mapedit_redo step through this map's changes".into(),
            "  mapedit_publish sends the map's edits to every SJK player".into(),
        ])
    }

    fn map_edit_scope_text(&self) -> String {
        match (self.map_edits.editor.scope, self.map_edits_server()) {
            (Scope::AllServers, _) => "new edits for all servers".into(),
            (Scope::ThisServer, Some(server)) => format!("new edits for {server} only"),
            (Scope::ThisServer, None) => "new edits for this server, but not on one".into(),
        }
    }

    fn map_edit_scope(&mut self, args: &[String]) -> Result<Vec<String>, String> {
        let editor = &mut self.map_edits.editor;
        editor.scope = match args {
            [] => match editor.scope {
                Scope::AllServers => Scope::ThisServer,
                Scope::ThisServer => Scope::AllServers,
            },
            [value] if value == "all" => Scope::AllServers,
            [value] if value == "server" => Scope::ThisServer,
            _ => return Err("usage: mapedit_scope [all|server]".into()),
        };
        Ok(vec![self.map_edit_scope_text()])
    }

    /// The server new edits are for, by the scope.
    fn map_edit_server(&self) -> Result<Option<String>, String> {
        match self.map_edits.editor.scope {
            Scope::AllServers => Ok(None),
            Scope::ThisServer => self
                .map_edits_server()
                .map(Some)
                .ok_or_else(|| "Not on a server: mapedit_scope all".into()),
        }
    }

    /// The world-note key while the editor is on: a grabbed model is put down where it is
    /// drawn; else what is under the crosshair is selected. Either way the panel opens.
    pub(crate) fn map_edit_select(&mut self) {
        if self.map_edits.editor.grabbing {
            let result = self.map_edit_command("mapedit_grab", &[]);
            self.map_edits.panel.answer(result);
        } else {
            let picked = self.map_edit_pick().is_some();
            self.map_edit_picked(picked, "Nothing under the crosshair");
        }
        self.open_map_edit_panel();
    }

    pub(super) fn map_edit_pick(&mut self) -> Option<String> {
        let map = crate::remap_blocked_maps::map_name(&self.world_load_map).to_owned();
        let remaps = Vec::new();
        self.world_notes.select_for_edit(Scene {
            bsp: &self.bsp,
            surfaces: &self.decal_surfaces,
            scratch: &mut self.trace_scratch,
            vfs: self.vfs.as_deref(),
            map: &map,
            remaps,
            markers: Vec::new(),
            entities: self.map_edits.pick_entities(&self.bsp),
        })
    }

    /// The selection the world-note key made, or what is under the crosshair now.
    fn map_edit_aim(&mut self) -> Result<Aim, String> {
        if self.world_notes.waiting_selection().is_none() {
            self.map_edit_pick();
        }
        let selection = self
            .world_notes
            .waiting_selection()
            .ok_or("Nothing under the crosshair")?;
        let (hit, _) = selection.aim();
        Ok(Aim {
            target: selection.edit_target(),
            hit,
            summary: selection.summary(),
        })
    }

    /// The loaded map's document, begun when it has none.
    fn map_edit_document(&mut self) -> Result<&mut super::Document, String> {
        let (map, checksum) = self
            .map_edits
            .map()
            .map(|(map, checksum)| (map.to_owned(), checksum))
            .ok_or("The map's checksum is still being read: try again in a moment")?;
        Ok(self
            .map_edits
            .document
            .get_or_insert_with(|| super::Document::new(&map, checksum)))
    }

    /// Save the document after an edit and answer `line`.
    fn map_edit_saved(&mut self, line: String) -> Result<Vec<String>, String> {
        let folder = self.map_edits_folder();
        self.map_edits.changed(folder.as_deref())?;
        Ok(vec![line])
    }

    fn lump_len(&self) -> usize {
        sjk_entity::parse_entity_lump(self.bsp.entities()).map_or(0, |entities| entities.len())
    }

    fn map_edit_hide(&mut self, args: &[String]) -> Result<Vec<String>, String> {
        let group = match args {
            [] => false,
            [word] if word == "group" => true,
            _ => return Err("usage: mapedit_hide [group]".into()),
        };
        let aim = self.map_edit_aim()?;
        let server = self.map_edit_server()?;
        self.world_notes.cancel_selection();
        match aim.target {
            EditTarget::Model { number, model, .. } => {
                if let Some(id) = self.map_edits.placed_edit(number, self.lump_len()) {
                    return self.map_edit_delete(id);
                }
                if !self.static_models.is_prop(number as u32) {
                    return Err(format!(
                        "{model} is a server entity: the game needs it, it is not hidden"
                    ));
                }
                let id = self.map_edit_document()?.add(
                    Kind::HideProp {
                        entity: number as u32,
                        model: model.clone(),
                    },
                    server,
                );
                self.map_edit_saved(format!("Edit {id}: {model} hidden"))
            }
            EditTarget::Surface { index, shader, .. } => {
                let surfaces = if group {
                    let render = self.bsp.render();
                    let soup = |other: usize| {
                        render.surfaces().get(other).is_some_and(|surface| {
                            surface.kind == sjk_bsp::SurfaceKind::TriangleSoup
                        })
                    };
                    self.decal_surfaces.connected(index, soup, GROUP_LIMIT)
                } else {
                    vec![index]
                };
                let shader_of = |surface: usize| {
                    self.bsp
                        .render()
                        .surfaces()
                        .get(surface)
                        .and_then(|surface| self.bsp.shaders().get(surface.shader))
                        .map_or_else(|| shader.clone(), |s| s.name_lossy().into_owned())
                };
                let named: Vec<(u32, String)> = surfaces
                    .iter()
                    .map(|&surface| (surface as u32, shader_of(surface)))
                    .collect();
                let document = self.map_edit_document()?;
                let ids: Vec<u32> = named
                    .into_iter()
                    .map(|(surface, shader)| {
                        document.add(Kind::HideSurface { surface, shader }, server.clone())
                    })
                    .collect();
                self.map_edit_saved(match ids.as_slice() {
                    [id] => format!("Edit {id}: surface {index} ({shader}) hidden"),
                    _ => format!(
                        "Edits {}-{}: {} surfaces hidden from surface {index}",
                        ids.iter().min().unwrap_or(&0),
                        ids.iter().max().unwrap_or(&0),
                        ids.len()
                    ),
                })
            }
            EditTarget::Other => Err(format!("{}: not something edits hide", aim.summary)),
        }
    }

    fn map_edit_place(&mut self, args: &[String]) -> Result<Vec<String>, String> {
        let aim = self.map_edit_aim()?;
        let server = self.map_edit_server()?;
        let (model, scale) = match (args, &aim.target) {
            ([model], _) => (model.replace('\\', "/"), [1.0; 3]),
            ([], EditTarget::Model { model, .. }) => (model.clone(), [1.0; 3]),
            ([], _) => {
                return Err("usage: mapedit_place <model.md3>, or select a model to copy".into());
            }
            _ => return Err("usage: mapedit_place [model.md3]".into()),
        };
        if !model.to_ascii_lowercase().ends_with(".md3") {
            return Err(format!("{model}: only MD3 models can be placed"));
        }
        let found = self
            .vfs
            .as_ref()
            .and_then(|vfs| vfs.read(&model).ok().flatten())
            .is_some();
        if !found {
            return Err(format!("{model} is not in the game data"));
        }
        let yaw = self
            .world_notes
            .view()
            .map_or(0.0, |(_, forward)| yaw_of(forward) + 180.0);
        self.world_notes.cancel_selection();
        let kind = Kind::Place {
            model: model.clone(),
            origin: aim.hit.to_array(),
            angles: [0.0, yaw.rem_euclid(360.0), 0.0],
            scale,
        };
        let id = self.map_edit_document()?.add(kind, server);
        self.map_edits.editor.current = Some(id);
        self.map_edit_saved(format!(
            "Edit {id}: {} placed at {:.0} {:.0} {:.0} (mapedit_turn, _move, _grab move it)",
            short_name(&model),
            aim.hit.x,
            aim.hit.y,
            aim.hit.z
        ))
    }

    /// The placed model edits act on: the selected one, else the last placed.
    fn map_edit_current(&mut self) -> Result<u32, String> {
        if let Some(selection) = self.world_notes.waiting_selection()
            && let EditTarget::Model { number, .. } = selection.edit_target()
            && let Some(id) = self.map_edits.placed_edit(number, self.lump_len())
        {
            self.map_edits.editor.current = Some(id);
        }
        let id = self
            .map_edits
            .editor
            .current
            .ok_or("No placed model: mapedit_place one, or select one")?;
        let placed = self
            .map_edits
            .document
            .as_ref()
            .and_then(|document| document.edits.iter().find(|edit| edit.id == id))
            .is_some_and(|edit| matches!(edit.kind, Kind::Place { .. }));
        if !placed {
            self.map_edits.editor.current = None;
            return Err("The placed model is gone".into());
        }
        Ok(id)
    }

    fn map_edit_transform(
        &mut self,
        change: impl FnOnce(&mut [f32; 3], &mut [f32; 3], &mut [f32; 3]),
    ) -> Result<Vec<String>, String> {
        let id = self.map_edit_current()?;
        let Some(edit) = self.map_edit_document()?.get_mut(id) else {
            return Err("The placed model is gone".into());
        };
        let Kind::Place {
            origin,
            angles,
            scale,
            ..
        } = &mut edit.kind
        else {
            return Err("Not a placed model".into());
        };
        change(origin, angles, scale);
        let line = edit.describe();
        if let Some(document) = &mut self.map_edits.document {
            document.unpublished = true;
        }
        self.map_edit_saved(line)
    }

    fn map_edit_grab(&mut self) -> Result<Vec<String>, String> {
        let id = self.map_edit_current()?;
        let editor = &mut self.map_edits.editor;
        editor.grabbing = !editor.grabbing;
        if editor.grabbing {
            return Ok(vec![format!(
                "Edit {id} follows your aim: mapedit_grab again to put it down"
            )]);
        }
        // Put down where it was last drawn.
        let Some((origin, angles)) = self
            .map_edits
            .placed
            .iter()
            .find(|placed| placed.id == id)
            .map(|placed| (placed.origin, placed.angles))
        else {
            return Err("The placed model is gone".into());
        };
        self.map_edit_transform(|o, a, _| {
            *o = origin;
            *a = angles;
        })
    }

    /// Once a frame while the editor is on: a grabbed model follows the aim.
    pub(super) fn editor_frame(&mut self) {
        let editor = &self.map_edits.editor;
        if !editor.active || !editor.grabbing {
            return;
        }
        let (Some(id), Some((eye, forward))) = (editor.current, self.world_notes.view()) else {
            return;
        };
        let Some(hit) = self.decal_surfaces.ray_hit(eye, forward, 4096.0) else {
            return;
        };
        let point = eye + forward * hit.distance;
        let angles = self
            .map_edits
            .placed
            .iter()
            .find(|placed| placed.id == id)
            .map_or([0.0; 3], |placed| placed.angles);
        self.map_edits
            .preview_placement(id, point.to_array(), angles);
    }

    fn map_edit_remap(&mut self, args: &[String]) -> Result<Vec<String>, String> {
        let [target] = args else {
            return Err("usage: mapedit_remap <shader> | - (removes the remap)".into());
        };
        let aim = self.map_edit_aim()?;
        let server = self.map_edit_server()?;
        let from = match &aim.target {
            EditTarget::Surface { shader, .. } => shader.clone(),
            EditTarget::Model {
                shader: Some(shader),
                ..
            } => shader.clone(),
            _ => return Err(format!("{}: no shader to remap", aim.summary)),
        };
        self.world_notes.cancel_selection();
        if target == "-" {
            let document = self.map_edit_document()?;
            let ids: Vec<u32> = document
                .edits
                .iter()
                .filter(|edit| {
                    edit.server == server
                        && matches!(&edit.kind, Kind::Remap { from: f, .. } if f.eq_ignore_ascii_case(&from))
                })
                .map(|edit| edit.id)
                .collect();
            if ids.is_empty() {
                return Err(format!("No remap edit of {from} for these servers"));
            }
            for id in &ids {
                document.remove(*id);
            }
            return self.map_edit_saved(format!("{from}: remap edit removed"));
        }
        let (vfs, shaders) = (
            self.vfs.clone().ok_or("No loaded map")?,
            self.shaders.clone(),
        );
        let (from, to) =
            crate::world_materials::map_remaps::checked(&vfs, &shaders, &from, target)?;
        let id = self.map_edit_document()?.add(
            Kind::Remap {
                from: from.clone(),
                to: to.clone(),
            },
            server,
        );
        self.map_edit_saved(format!("Edit {id}: {from} drawn as {to}"))
    }

    /// Undo the last change to this map's edits (`undo`), or redo the last undone.
    fn map_edit_step(&mut self, undo: bool) -> Result<Vec<String>, String> {
        // The map's checksum first: a step is taken only when it can be saved.
        self.map_edit_document()?;
        let history = &mut self.map_edits.history;
        let from = if undo {
            &mut history.undo
        } else {
            &mut history.redo
        };
        let Some(edits) = from.pop() else {
            return Err(if undo {
                "Nothing to undo".into()
            } else {
                "Nothing to redo".into()
            });
        };
        let document = self.map_edit_document()?;
        let now = std::mem::replace(&mut document.edits, edits);
        document.unpublished = true;
        let line = stepped(&now, &document.edits).map_or_else(
            || "the edits".to_owned(),
            |edit| match document.edits.iter().find(|kept| kept.id == edit.id) {
                Some(kept) => kept.label(),
                None => format!("{} taken away", edit.label()),
            },
        );
        let present = document
            .edits
            .iter()
            .map(|edit| edit.id)
            .collect::<Vec<_>>();
        let history = &mut self.map_edits.history;
        if undo {
            history.redo.push(now);
        } else {
            history.undo.push(now);
        }
        // The placed model the controls act on may be gone.
        let editor = &mut self.map_edits.editor;
        if editor.current.is_some_and(|id| !present.contains(&id)) {
            editor.current = None;
            editor.grabbing = false;
        }
        self.map_edit_saved(format!(
            "{}: {line}",
            if undo { "Undone" } else { "Redone" }
        ))
    }

    fn map_edit_delete(&mut self, id: u32) -> Result<Vec<String>, String> {
        let removed = self
            .map_edit_document()?
            .remove(id)
            .ok_or_else(|| format!("No edit {id}"))?;
        if self.map_edits.editor.current == Some(id) {
            self.map_edits.editor.current = None;
            self.map_edits.editor.grabbing = false;
        }
        self.map_edit_saved(format!("Removed {}", removed.describe()))
    }

    fn map_edit_list(&self) -> Vec<String> {
        let Some(document) = &self.map_edits.document else {
            return vec!["No edits on this map".into()];
        };
        let server = self.map_edits_server();
        let mut lines = vec![format!(
            "{} edits on {} (hub revision {}{}):",
            document.edits.len(),
            document.map,
            document.revision,
            if document.unpublished {
                ", changed here, not published"
            } else {
                ""
            }
        )];
        lines.extend(document.edits.iter().map(|edit| {
            let shown = if edit.applies_on(server.as_deref()) {
                ""
            } else {
                " (not on this server)"
            };
            format!("  {}{shown}", edit.describe())
        }));
        lines
    }

    fn map_edit_publish(&mut self) -> Result<Vec<String>, String> {
        let document = self
            .map_edits
            .document
            .as_ref()
            .ok_or("No edits on this map")?;
        document.check()?;
        let edits = serde_json::to_value(&document.edits).map_err(|error| error.to_string())?;
        crate::player_identity::publish_map_edits(document.map.clone(), document.checksum, edits)?;
        Ok(vec![format!(
            "Sending {} edits of {} to the hub...",
            document.edits.len(),
            document.map
        )])
    }

    /// Once a frame: the hub's answer to `mapedit_publish`.
    pub(super) fn poll_map_edit_publish(&mut self) {
        let Some(outcome) = crate::player_identity::map_edits_published() else {
            return;
        };
        let result = match outcome {
            Ok((map, checksum, revision)) => {
                let folder = self.map_edits_folder();
                if let Some(document) = self
                    .map_edits
                    .document
                    .as_mut()
                    .filter(|document| document.map == map && document.checksum == checksum)
                {
                    document.revision = revision;
                    document.unpublished = false;
                    if let Some(folder) = folder
                        && let Err(error) = super::save(&folder, document)
                    {
                        crate::log::progress(format_args!("map edits not saved: {error}"));
                    }
                }
                Ok(format!(
                    "Published: every SJK player gets {map}'s edits (hub revision {revision})"
                ))
            }
            Err(error) => Err(format!("Not published: {error}")),
        };
        let line = match &result {
            Ok(line) => line.clone(),
            Err(error) => format!("^1Map edits {}", error.replacen("Not", "not", 1)),
        };
        crate::log::progress(format_args!("map edits: {line}"));
        self.chat.receive(
            sjk_client::ServerEventKind::Print,
            line,
            None,
            std::time::Instant::now(),
        );
        let panel = &mut self.map_edits.panel;
        panel.answer(result.map(|line| vec![line]));
        panel.stale();
    }

    /// The mouse wheel while a placed model is grabbed: it turns the model (by a degree
    /// with `fine`, Shift held) instead of changing weapon. Whether it took the wheel.
    pub(crate) fn map_edit_wheel(&mut self, up: bool, fine: bool) -> bool {
        let editor = &self.map_edits.editor;
        if !editor.active || !editor.grabbing {
            return false;
        }
        let Some(id) = editor.current else {
            return false;
        };
        let Some((origin, mut angles)) = self
            .map_edits
            .placed
            .iter()
            .find(|placed| placed.id == id)
            .map(|placed| (placed.origin, placed.angles))
        else {
            return false;
        };
        let step = if fine { 1.0 } else { TURN_STEP };
        angles[1] = (angles[1] + if up { step } else { -step }).rem_euclid(360.0);
        self.map_edits.preview_placement(id, origin, angles);
        true
    }
}
