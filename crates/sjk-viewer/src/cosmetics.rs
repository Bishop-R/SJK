//! JoF EJK's hats and capes: `.md3` models worn on a player's `*head_top`
//! and `*back` bolts, chosen by name after the saber colour digits of
//! `color1` and `color2` (see [`sjk_client::split_color_value`]).
//!
//! A piece is any model in `models/cosmetics/hats/` or
//! `models/cosmetics/capes/` (`models/players/hats/` and `capes/` when the
//! new folders are empty, as older packs used them), and JoF's catalogue
//! names are looked up in both. A piece may ship
//! `settings/cosmetics/<kind>/<name>.cosmetic`, JSON nudging it into place
//! per model and skin; the format is JoF EJK's and TaystJK's
//! (`CG_LoadCosmeticOffsets`). `cg_cosmetics` chooses whose pieces are
//! drawn. This module holds the data rules; the menu lists the
//! [`Catalog`], [`actors`] resolves and submits what players wear, and
//! [`command`] is JoF's `cosmetics` console command; [`typed_colour`] keeps the worn
//! piece when a colour number is typed on its own.

pub(crate) mod actors;
pub(crate) mod command;
pub(crate) mod typed_colour;

use crate::actor_instance::ActorInstance;
use crate::bolt::{self, BoltMatrix};
use glam::{Mat3, Quat, Vec3};
use sjk_client::CosmeticSlot;
use sjk_vfs::VirtualFileSystem;

/// `cg_cosmetics` (JoF EJK, archived, default 1).
pub(crate) const VISIBILITY_CVAR: &str = "cg_cosmetics";

/// Whose cosmetics are drawn (`JAPRO_COSMETICS_*`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Visibility {
    Off,
    On,
    OnlyMe,
}

impl Visibility {
    /// Read `cg_cosmetics`; JoF treats anything out of range as on.
    pub(crate) fn from_cvar(value: i64) -> Self {
        match value {
            0 => Self::Off,
            2 => Self::OnlyMe,
            _ => Self::On,
        }
    }

    pub(crate) fn cvar_value(self) -> &'static str {
        match self {
            Self::Off => "0",
            Self::On => "1",
            Self::OnlyMe => "2",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::On => "On",
            Self::OnlyMe => "Only Me",
        }
    }

    /// Whether a piece on the local player (`local`) or another is drawn.
    pub(crate) fn shows(self, local: bool) -> bool {
        match self {
            Self::Off => false,
            Self::On => true,
            Self::OnlyMe => local,
        }
    }
}

/// Folders a slot's models are listed from, the newer first.
fn folders(slot: CosmeticSlot) -> [&'static str; 2] {
    match slot {
        CosmeticSlot::Hat => ["models/cosmetics/hats", "models/players/hats"],
        CosmeticSlot::Cape => ["models/cosmetics/capes", "models/players/capes"],
    }
}

/// The folders JoF's pieces lived in before `models/cosmetics/`, each with its new one.
pub(crate) const MOVED_FOLDERS: [(&str, &str); 2] = [
    ("models/players/hats/", "models/cosmetics/hats/"),
    ("models/players/capes/", "models/cosmetics/capes/"),
];
/// The picture formats a shader name finds, in the loader's order (`R_LoadImage`).
const PICTURES: [&str; 4] = ["png", "tga", "jpg", "jpeg"];

/// A hat's or cape's model, read once per mount for [`old_folder_pictures`] and
/// [`inside_out_shaders`] (Sol's review of PR #236: each read every model itself).
pub(crate) struct PieceModel {
    /// Whether it is a hat (in `models/cosmetics/hats/` or `models/players/hats/`).
    hat: bool,
    /// Whether it is in a new folder (`models/cosmetics/`).
    new_folder: bool,
    md3: sjk_model::Md3,
}

/// The models of the hats and capes [`old_folder_pictures`] and [`inside_out_shaders`]
/// look at, each read and parsed once: every `.md3` in `models/cosmetics/hats/` and
/// `capes/` (one listing for both, as a listing walks every mounted file) and in
/// `models/players/hats/`.
pub(crate) fn piece_models(vfs: &VirtualFileSystem) -> Vec<PieceModel> {
    let mut models = Vec::new();
    for (folder, new_folder) in [("models/cosmetics", true), ("models/players/hats", false)] {
        for name in vfs.list_files(folder, ".md3") {
            let hat = if new_folder {
                match name.split_once('/') {
                    Some(("hats", _)) => true,
                    Some(("capes", _)) => false,
                    _ => continue,
                }
            } else if name.contains('/') {
                continue;
            } else {
                true
            };
            let Ok(Some(asset)) = vfs.read(&format!("{folder}/{name}")) else {
                continue;
            };
            if let Ok(md3) = sjk_model::Md3::parse(&asset.bytes) {
                models.push(PieceModel {
                    hat,
                    new_folder,
                    md3,
                });
            }
        }
    }
    models
}

/// The pictures to add under the old folders so that pieces whose models still name
/// them draw as JoF EJK draws them (hub report #192, 28/10/2026: the pumpkin hat drew
/// grey). Seven of JoF's hats (pumpkin, cap, cringe, fedora, santahat, sombrero,
/// tophat) moved to `models/cosmetics/hats/`, but their `.md3` files still name
/// `models/players/hats/<name>`. JoF EJK finds those in jaPRO's `japro-assets.pk3`
/// (its own copies of the pictures and `japro_hats.shader`, which SJK takes from
/// the EternalJK folder, [`crate::asset_search_paths`]); where that pack is not
/// installed, the same picture from the new folder is copied under the old name, so the
/// surface draws with its picture as EternalJK does (not with JoF's `jof_hats.shader`
/// in the new folder, which no model names: its pumpkin glows). `models` are
/// [`piece_models`]; `have` says whether a path is already given (jaPRO's copies).
pub(crate) fn old_folder_pictures(
    vfs: &VirtualFileSystem,
    models: &[PieceModel],
    have: impl Fn(&str) -> bool,
) -> Vec<(String, Vec<u8>)> {
    let mut added: Vec<(String, Vec<u8>)> = Vec::new();
    for model in models.iter().filter(|model| model.new_folder) {
        let (old, new) = MOVED_FOLDERS[usize::from(!model.hat)];
        for shader in model
            .md3
            .surfaces
            .iter()
            .flat_map(|surface| &surface.shaders)
        {
            let name = shader.to_ascii_lowercase();
            let name = name
                .rsplit_once('.')
                .filter(|(_, extension)| PICTURES.contains(extension))
                .map_or(name.as_str(), |(stem, _)| stem);
            let Some(rest) = name.strip_prefix(old) else {
                continue;
            };
            let found = |path: &str| have(path) || vfs.contains(path).unwrap_or(false);
            if PICTURES
                .iter()
                .any(|extension| found(&format!("{old}{rest}.{extension}")))
                || added
                    .iter()
                    .any(|(path, _)| path.starts_with(&format!("{old}{rest}.")))
            {
                continue;
            }
            let copy = PICTURES.iter().find_map(|extension| {
                let bytes = vfs.read(&format!("{new}{rest}.{extension}")).ok()??.bytes;
                Some((format!("{old}{rest}.{extension}"), bytes))
            });
            added.extend(copy);
        }
    }
    added
}

/// Where [`inside_out_shaders`] puts its file. A shader name defined twice takes the
/// definition read last, and scripts are read in path order
/// (`sjk_shader::ShaderCatalog`), so this name, which sorts before every other
/// script's, lets any script defining the same name keep its own culling, without
/// reading every script at each mount to find out.
pub(crate) const TWO_SIDED_SHADERS: &str = "shaders/!sjk_cosmetics_two_sided.shader";

/// A shader file drawing two-sided the hats built inside out, as JoF EJK shows them
/// (Dave's report, 28/10/2026: McFox and McSnowFox were see-through in SJK). Most of their
/// triangles are wound away from the model's middle where every other piece's face it
/// (82 of 120 in `mcfox.md3`), so culling the back faces drew their insides only. Each
/// such model's surface shaders get one: their picture as
/// [`crate::world_stage::material_stages`] draws an unscripted model surface (lit by
/// `lightingDiffuse`), with `cull twosided`; a script defining the same name wins
/// ([`TWO_SIDED_SHADERS`]). Capes are left out: a cape is a thin sheet, which the
/// middle-of-the-model test cannot judge (it took JoF's royal cape for inside out).
/// `models` are [`piece_models`]; `None` when none is inside out.
pub(crate) fn inside_out_shaders(models: &[PieceModel]) -> Option<(String, Vec<u8>)> {
    let mut names: Vec<String> = Vec::new();
    for model in models
        .iter()
        .filter(|model| model.hat && inside_out(&model.md3))
    {
        for shader in model
            .md3
            .surfaces
            .iter()
            .flat_map(|surface| &surface.shaders)
        {
            let name = shader.to_ascii_lowercase();
            if !name.is_empty() && !names.contains(&name) {
                names.push(name);
            }
        }
    }
    if names.is_empty() {
        return None;
    }
    let mut text =
        String::from("// SJK: JoF's pieces built inside out, drawn two-sided (cosmetics.rs)\n");
    for name in &names {
        text.push_str(&format!(
            "{name}\n{{\n\tcull twosided\n\t{{\n\t\tmap {name}\n\t\trgbGen lightingDiffuse\n\t}}\n}}\n"
        ));
    }
    Some((TWO_SIDED_SHADERS.to_owned(), text.into_bytes()))
}

/// Whether more of `md3`'s triangles are wound as seen from inside it than from outside
/// (the game's front faces turn clockwise as seen: their normal by the right-hand rule
/// points into the model), judged against each surface's middle; true only for a
/// clearly inside-out model, as McFox's.
fn inside_out(md3: &sjk_model::Md3) -> bool {
    let (mut outward, mut inward) = (0usize, 0usize);
    for surface in &md3.surfaces {
        let Some(frame) = surface.frames.first() else {
            continue;
        };
        if frame.is_empty() {
            continue;
        }
        let point = |index: u32| {
            frame
                .get(index as usize)
                .map_or(Vec3::ZERO, |vertex| Vec3::from_array(vertex.position))
        };
        let middle = frame
            .iter()
            .map(|vertex| Vec3::from_array(vertex.position))
            .sum::<Vec3>()
            / frame.len() as f32;
        for [a, b, c] in &surface.triangles {
            let (a, b, c) = (point(*a), point(*b), point(*c));
            let side = (b - a).cross(c - a).dot((a + b + c) / 3.0 - middle);
            if side > 1e-6 {
                outward += 1;
            } else if side < -1e-6 {
                inward += 1;
            }
        }
    }
    outward > inward * 3 / 2
}

/// Whether a `.shader` file's text defines only shaders in the old piece folders
/// ([`MOVED_FOLDERS`]), as jaPRO's `japro_hats.shader` does: such a file can be taken
/// from a pack without bringing anything else of it.
pub(crate) fn only_old_folder_shaders(text: &str) -> bool {
    let mut depth = 0usize;
    let mut names = 0usize;
    for line in text.lines() {
        let line = line.split("//").next().unwrap_or_default();
        for token in line.split_whitespace() {
            match token {
                "{" => depth += 1,
                "}" => depth = depth.saturating_sub(1),
                name if depth == 0 => {
                    let name = name.to_ascii_lowercase();
                    if !MOVED_FOLDERS.iter().any(|(old, _)| name.starts_with(old)) {
                        return false;
                    }
                    names += 1;
                }
                _ => {}
            }
        }
    }
    names > 0 && depth == 0
}

/// Folder of a slot's `.cosmetic` fitting files.
fn settings_folder(slot: CosmeticSlot) -> &'static str {
    match slot {
        CosmeticSlot::Hat => "settings/cosmetics/hats",
        CosmeticSlot::Cape => "settings/cosmetics/capes",
    }
}

/// The JoF launcher's catalogue (`uiKnownHats`, `uiKnownCapes`), listed
/// even when only some are installed so the menu can say where the rest are.
const KNOWN_HATS: [&str; 24] = [
    "afro",
    "beard",
    "bucket",
    "cap",
    "cringe",
    "crown",
    "fedora",
    "fedora2",
    "fedora3",
    "fedora4",
    "glasses",
    "gradcap",
    "headcrab",
    "horns",
    "mario",
    "mask",
    "metalhelm",
    "plaguemask",
    "predatorhelm",
    "pumpkin",
    "santahat",
    "sombrero",
    "supersaiyan",
    "tophat",
];
const KNOWN_CAPES: [&str; 8] = [
    "ak47",
    "crowbar",
    "goose",
    "grogucape",
    "royalcape",
    "rpg",
    "vadercape",
    "yodacape",
];

fn known(slot: CosmeticSlot) -> &'static [&'static str] {
    match slot {
        CosmeticSlot::Hat => &KNOWN_HATS,
        CosmeticSlot::Cape => &KNOWN_CAPES,
    }
}

/// One installed piece.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Piece {
    /// The name worn in `color1`/`color2`.
    pub(crate) name: String,
    /// What the menu lists it as.
    pub(crate) display: String,
}

/// The installed pieces of both slots, sorted by display name.
#[derive(Clone, Debug, Default)]
pub(crate) struct Catalog {
    pieces: [Vec<Piece>; 2],
    /// Whether JoF's catalogue has pieces of the slot that are not installed.
    missing: [bool; 2],
}

impl Catalog {
    /// List what `vfs` holds, as JoF EJK's `UI_LoadCosmetics` does.
    pub(crate) fn scan(vfs: &VirtualFileSystem) -> Self {
        let mut catalog = Self::default();
        for slot in CosmeticSlot::ALL {
            let [folder, legacy] = folders(slot);
            let mut names = models_in(vfs, folder);
            if names.is_empty() {
                names = models_in(vfs, legacy);
            }
            let mut missing = false;
            for name in known(slot) {
                if names.iter().any(|known| known.eq_ignore_ascii_case(name)) {
                    continue;
                }
                if model_path(vfs, slot, name).is_some() {
                    names.push((*name).to_owned());
                } else {
                    missing = true;
                }
            }
            let mut pieces: Vec<Piece> = names
                .into_iter()
                .map(|name| Piece {
                    display: display_name(&name),
                    name,
                })
                .collect();
            pieces.sort_by_cached_key(|piece| piece.display.to_ascii_lowercase());
            catalog.pieces[slot.index()] = pieces;
            catalog.missing[slot.index()] = missing;
        }
        catalog
    }

    pub(crate) fn pieces(&self, slot: CosmeticSlot) -> &[Piece] {
        &self.pieces[slot.index()]
    }

    /// Position of the piece called `name` (any case).
    pub(crate) fn position(&self, slot: CosmeticSlot, name: &str) -> Option<usize> {
        self.pieces(slot)
            .iter()
            .position(|piece| piece.name.eq_ignore_ascii_case(name))
    }

    /// Whether some of JoF's catalogue is not installed.
    pub(crate) fn has_missing(&self, slot: CosmeticSlot) -> bool {
        self.missing[slot.index()]
    }
}

/// Wearable model names in `folder`: `.md3` files directly in it whose name
/// can travel in `color1`/`color2`.
fn models_in(vfs: &VirtualFileSystem, folder: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for file in vfs.list_files(folder, ".md3") {
        if file.contains('/') {
            continue;
        }
        let name = &file[..file.len() - ".md3".len()];
        if sjk_client::valid_cosmetic_name(name)
            && !names.iter().any(|known| known.eq_ignore_ascii_case(name))
        {
            names.push(name.to_owned());
        }
    }
    names
}

/// The model a worn `name` draws, if installed.
pub(crate) fn model_path(
    vfs: &VirtualFileSystem,
    slot: CosmeticSlot,
    name: &str,
) -> Option<String> {
    if !sjk_client::valid_cosmetic_name(name) {
        return None;
    }
    folders(slot).into_iter().find_map(|folder| {
        let path = format!("{folder}/{}.md3", name.to_ascii_lowercase());
        vfs.contains(&path).ok()?.then_some(path)
    })
}

/// `UI_CosmeticDisplayName`: `_` and `-` become spaces, the first letter a
/// capital.
pub(crate) fn display_name(name: &str) -> String {
    let mut display: String = name
        .chars()
        .map(|c| if c == '_' || c == '-' { ' ' } else { c })
        .collect();
    if let Some(first) = display.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    display
}

/// The fitting offset of piece `name` on `model`/`skin`, from its
/// `.cosmetic` file; none when there is no file or nothing matches.
pub(crate) fn fitting_offset(
    vfs: &VirtualFileSystem,
    slot: CosmeticSlot,
    name: &str,
    model: &str,
    skin: &str,
) -> [f32; 3] {
    let path = format!("{}/{name}.cosmetic", settings_folder(slot));
    let Some(asset) = vfs.read(&path).ok().flatten() else {
        return [0.0; 3];
    };
    let text = String::from_utf8_lossy(&asset.bytes);
    match offset_from_json(&text, model, skin) {
        Ok(offset) => offset.unwrap_or_default(),
        Err(reason) => {
            crate::log::progress(format_args!(
                "warning: ignoring offsets for cosmetic ({name}): {reason}"
            ));
            [0.0; 3]
        }
    }
}

/// `CG_LoadCosmeticOffsets` over the file's text: the skin's entry under
/// the model's wins; with no skin match, the model's own offsets apply when
/// it says `"modelFallback": true`. Keys match exactly, else the wildcard key
/// (`"kyle*"`) with the longest matching prefix.
fn offset_from_json(text: &str, model: &str, skin: &str) -> Result<Option<[f32; 3]>, String> {
    let json: serde_json::Value =
        serde_json::from_str(text).map_err(|_| "not valid JSON".to_owned())?;
    let Some(model_entry) = json_match(&json, model) else {
        return Ok(None);
    };
    let fallback = model_entry
        .get("modelFallback")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if let Some(skin_entry) = json_match(model_entry, skin) {
        return offsets(skin_entry)
            .map(Some)
            .ok_or_else(|| format!("skin ({skin}) has no valid xOffset/yOffset/zOffset"));
    }
    if fallback {
        return offsets(model_entry)
            .map(Some)
            .ok_or_else(|| format!("model ({model}) has no valid xOffset/yOffset/zOffset"));
    }
    Ok(None)
}

/// `CG_CosmeticJSONMatch`.
fn json_match<'a>(object: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    let object = object.as_object()?;
    if let Some(exact) = object.get(name) {
        return Some(exact);
    }
    object
        .iter()
        .filter(|(_, entry)| entry.is_object())
        .filter_map(|(key, entry)| {
            let prefix = &key.as_bytes()[..key.find('*')?];
            // Compare bytes, as `Q_stricmpn` does: a player's model name may hold a
            // multi-byte character across the prefix length, and slicing the string
            // there panicked.
            name.as_bytes()
                .get(..prefix.len())
                .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
                .then_some((prefix.len(), entry))
        })
        .max_by_key(|(length, _)| *length)
        .map(|(_, entry)| entry)
}

/// `CG_CosmeticJSONOffsets`: all three numbers, or nothing.
fn offsets(node: &serde_json::Value) -> Option<[f32; 3]> {
    let read = |key: &str| node.get(key)?.as_f64().map(|value| value as f32);
    Some([read("xOffset")?, read("yOffset")?, read("zOffset")?])
}

/// Where a piece on raw bolt `raw` of an actor at `transform` is drawn:
/// `CG_DrawCosmeticOnPlayer`'s axes from `G2API_GetBoltMatrix`, its origin
/// two units down the bolt's up axis, then `offset` along the world axes.
pub(crate) fn placement(
    raw: BoltMatrix,
    transform: sjk_runtime::Transform,
    offset: [f32; 3],
) -> Option<ActorInstance> {
    placement_at(
        raw,
        Vec3::from_array(transform.translation),
        crate::weapon_view::actor_world_rotation(transform.rotation),
        Vec3::from_array(transform.scale),
        offset,
    )
}

/// [`placement`] for an actor whose world rotation already includes the
/// Ghoul2 facing turn ([`crate::weapon_view::actor_world_rotation`]), as the
/// menu stage keeps it.
pub(crate) fn placement_at(
    mut raw: BoltMatrix,
    origin: Vec3,
    rotation: Quat,
    scale: Vec3,
    offset: [f32; 3],
) -> Option<ActorInstance> {
    // G2API_GetBoltMatrix normalizes the three basis rows (as `flag_carrier`).
    for row in &mut raw {
        let length = Vec3::new(row[0], row[1], row[2]).length();
        if length != 0.0 {
            for component in &mut row[..3] {
                *component /= length;
            }
        }
    }
    let game = bolt::game_facing(raw);
    let axis = |index| rotation * Vec3::from_array(bolt::column(&game, index));
    let forward = axis(0).normalize_or_zero();
    let left = axis(1);
    let left = (left - forward * left.dot(forward)).normalize_or_zero();
    let up = forward.cross(left);
    if forward.length_squared() < 0.5 || left.length_squared() < 0.5 {
        return None;
    }
    let origin = origin + rotation * (Vec3::from_array(bolt::column(&game, 3)) * scale)
        - axis(2).normalize_or_zero() * 2.0
        + Vec3::from_array(offset);
    let quaternion = Quat::from_mat3(&Mat3::from_cols(forward, left, up));
    Some(ActorInstance::new(
        origin.to_array(),
        quaternion.to_array(),
        [1.0; 3],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md3_naming(shader: &str) -> Vec<u8> {
        md3_with(shader, &[], &[])
    }

    /// An MD3 of one frame and one surface with one shader, these vertices (units) and
    /// triangles.
    fn md3_with(shader: &str, positions: &[[f32; 3]], triangles: &[[i32; 3]]) -> Vec<u8> {
        let vertices = positions.len();
        let triangles_at = 108 + 68;
        let st_at = triangles_at + triangles.len() * 12;
        let xyz_at = st_at + vertices * 8;
        let end = xyz_at + vertices * 8;
        let mut surface = Vec::new();
        surface.extend_from_slice(b"IDP3");
        let mut name = [0u8; 64];
        name[..4].copy_from_slice(b"hat0");
        surface.extend_from_slice(&name);
        for value in [
            0,
            1,
            1,
            vertices,
            triangles.len(),
            triangles_at,
            108,
            st_at,
            xyz_at,
            end,
        ] {
            surface.extend_from_slice(&(value as i32).to_le_bytes());
        }
        let mut shader_name = [0u8; 68];
        shader_name[..shader.len()].copy_from_slice(shader.as_bytes());
        surface.extend_from_slice(&shader_name);
        for triangle in triangles {
            for index in triangle {
                surface.extend_from_slice(&index.to_le_bytes());
            }
        }
        surface.extend(std::iter::repeat_n(0u8, vertices * 8));
        for position in positions {
            for axis in position {
                surface.extend_from_slice(&((axis * 64.0) as i16).to_le_bytes());
            }
            surface.extend_from_slice(&[0, 0]);
        }
        let mut file = Vec::new();
        file.extend_from_slice(b"IDP3");
        file.extend_from_slice(&15i32.to_le_bytes());
        file.extend_from_slice(&[0u8; 64]);
        let frames_at = 108;
        let surfaces_at = frames_at + 56;
        let end = surfaces_at + surface.len();
        for value in [0, 1, 0, 1, 0, frames_at, frames_at, surfaces_at, end] {
            file.extend_from_slice(&(value as i32).to_le_bytes());
        }
        file.extend_from_slice(&[0u8; 56]);
        file.extend_from_slice(&surface);
        file
    }

    /// A tetrahedron wound as the game's front faces (clockwise seen from outside), or
    /// inside out.
    fn tetrahedron(shader: &str, inside_out: bool) -> Vec<u8> {
        let points = [
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [0.0, 10.0, 0.0],
            [0.0, 0.0, 10.0],
        ];
        // Counter-clockwise seen from outside (normals out by the right-hand rule).
        let outward = [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
        let faces: Vec<[i32; 3]> = outward
            .iter()
            .map(|&[a, b, c]| if inside_out { [a, b, c] } else { [a, c, b] })
            .collect();
        md3_with(shader, &points, &faces)
    }

    /// McFox and McSnowFox are wound inside out, and SJK drew their insides only
    /// (Dave's report, 28/10/2026): an unscripted hat built so is drawn two-sided; a
    /// hat wound as the game's, or one a script defines, is left alone.
    #[test]
    fn an_inside_out_hat_is_drawn_two_sided() {
        let mut vfs = VirtualFileSystem::new();
        vfs.mount_memory(
            "test",
            [
                (
                    "models/cosmetics/hats/mcfox.md3".to_owned(),
                    tetrahedron("models/cosmetics/hats/mcfox", true),
                ),
                (
                    "models/cosmetics/hats/crown.md3".to_owned(),
                    tetrahedron("models/cosmetics/hats/crown", false),
                ),
                (
                    "models/cosmetics/hats/mask.md3".to_owned(),
                    tetrahedron("models/cosmetics/hats/mask", true),
                ),
                (
                    "shaders/jof_hats.shader".to_owned(),
                    b"models/cosmetics/hats/mask\n{\n{\nmap models/cosmetics/hats/mask\n}\n}\n"
                        .to_vec(),
                ),
            ],
        )
        .expect("mount");
        let (path, text) = inside_out_shaders(&piece_models(&vfs)).expect("one inside-out hat");
        assert_eq!(path, TWO_SIDED_SHADERS);
        let written = String::from_utf8(text.clone()).expect("text");
        assert!(written.contains("models/cosmetics/hats/mcfox\n{\n\tcull twosided\n\t{\n\t\tmap models/cosmetics/hats/mcfox\n\t\trgbGen lightingDiffuse"));
        assert!(!written.contains("crown"), "wound as the game's");
        // Mounted as the game mounts it, over the rest: a script still wins.
        vfs.mount_memory("two-sided", [(path, text)])
            .expect("mount");
        let shaders = sjk_shader::ShaderCatalog::load_with_warnings(&vfs, |_, _| {});
        let mcfox = shaders.get("models/cosmetics/hats/mcfox").expect("defined");
        assert_eq!(mcfox.cull, sjk_shader::ShaderCull::TwoSided);
        let mask = shaders.get("models/cosmetics/hats/mask").expect("scripted");
        assert_ne!(
            mask.cull,
            sjk_shader::ShaderCull::TwoSided,
            "a script keeps its own culling"
        );
    }

    /// JoF's pumpkin: its model names `models/players/hats/pumpkin`, its picture is in
    /// `models/cosmetics/hats/` (hub report #192). Without jaPRO's copy the picture is
    /// copied under the old name; with it, or for a name found where it is, nothing is.
    #[test]
    fn a_hat_naming_the_old_folder_gets_its_picture_there() {
        let mut vfs = VirtualFileSystem::new();
        vfs.mount_memory(
            "test",
            [
                (
                    "models/cosmetics/hats/pumpkin.md3".to_owned(),
                    md3_naming("models/players/hats/pumpkin"),
                ),
                (
                    "models/cosmetics/hats/pumpkin.png".to_owned(),
                    b"orange".to_vec(),
                ),
                (
                    "models/cosmetics/hats/tophat.md3".to_owned(),
                    md3_naming("models/players/hats/tophat.tga"),
                ),
                (
                    "models/cosmetics/hats/tophat.jpg".to_owned(),
                    b"black".to_vec(),
                ),
                (
                    "models/players/hats/tophat.png".to_owned(),
                    b"there".to_vec(),
                ),
                (
                    "models/cosmetics/hats/crown.md3".to_owned(),
                    md3_naming("models/cosmetics/hats/crown"),
                ),
                (
                    "models/cosmetics/capes/cape.md3".to_owned(),
                    md3_naming("models/players/capes/cape"),
                ),
            ],
        )
        .expect("mount");
        let pieces = piece_models(&vfs);
        let added = old_folder_pictures(&vfs, &pieces, |_| false);
        assert_eq!(
            added,
            [(
                "models/players/hats/pumpkin.png".to_owned(),
                b"orange".to_vec()
            )],
            "the tophat is found where it is, the crown names the new folder, the cape has no picture"
        );
        assert!(
            old_folder_pictures(&vfs, &pieces, |path| path
                == "models/players/hats/pumpkin.png")
            .is_empty()
        );
    }

    #[test]
    fn only_a_shader_file_of_old_piece_folders_is_taken() {
        let japro = "models/players/hats/cap\n{\nq3map_nolightmap // two sided\ncull twosided\n{\nmap models/players/hats/cap\n}\n}\nmodels/players/capes/cape\n{\n}\n";
        assert!(only_old_folder_shaders(japro));
        assert!(!only_old_folder_shaders("gfx/2d/crosshaira\n{\n}\n"));
        assert!(!only_old_folder_shaders(&format!(
            "{japro}textures/x\n{{\n}}\n"
        )));
        assert!(!only_old_folder_shaders(""));
    }

    #[test]
    fn wildcard_keys_match_model_names_with_symbols() {
        let json = serde_json::json!({ "abc*": { "x": 1 } });
        // `×` covers bytes 2..4, across the three-byte prefix: this panicked.
        assert!(json_match(&json, "ab×").is_none());
        assert!(json_match(&json, "abc×").is_some());
        assert!(json_match(&json, "ABcd").is_some());
    }

    #[test]
    fn display_names_read_as_jof_lists_them() {
        assert_eq!(display_name("santahat"), "Santahat");
        assert_eq!(display_name("royal_cape"), "Royal cape");
        assert_eq!(display_name("mc-fox"), "Mc fox");
    }

    #[test]
    fn visibility_reads_and_cycles_as_jof_does() {
        assert_eq!(Visibility::from_cvar(0), Visibility::Off);
        assert_eq!(Visibility::from_cvar(2), Visibility::OnlyMe);
        assert_eq!(Visibility::from_cvar(7), Visibility::On);
        assert!(Visibility::OnlyMe.shows(true) && !Visibility::OnlyMe.shows(false));
        assert!(!Visibility::Off.shows(true));
        for visibility in [Visibility::On, Visibility::OnlyMe, Visibility::Off] {
            let value: i64 = visibility.cvar_value().parse().unwrap();
            assert_eq!(Visibility::from_cvar(value), visibility);
        }
    }

    #[test]
    fn offsets_pick_the_skin_then_the_model_fallback() {
        let text = r#"{
            "kyle": { "modelFallback": true, "xOffset": 0, "yOffset": 0, "zOffset": 2,
                      "red": { "xOffset": 0, "yOffset": 1, "zOffset": 3 } },
            "de*": { "xOffset": 1, "yOffset": 0, "zOffset": 0,
                     "def*": { "xOffset": 4, "yOffset": 4, "zOffset": 4 } },
            "desann*": { "modelFallback": true, "xOffset": 9, "yOffset": 9, "zOffset": 9 }
        }"#;
        assert_eq!(
            offset_from_json(text, "kyle", "red"),
            Ok(Some([0.0, 1.0, 3.0]))
        );
        assert_eq!(
            offset_from_json(text, "kyle", "blue"),
            Ok(Some([0.0, 0.0, 2.0]))
        );
        // `de*` has no fallback, but its `def*` skin key matches.
        assert_eq!(
            offset_from_json(text, "dendo", "default"),
            Ok(Some([4.0; 3]))
        );
        assert_eq!(offset_from_json(text, "dendo", "blue"), Ok(None));
        // The longest wildcard prefix wins.
        assert_eq!(offset_from_json(text, "desann", "blue"), Ok(Some([9.0; 3])));
        assert_eq!(offset_from_json(text, "luke", "default"), Ok(None));
        assert!(offset_from_json("{ nope", "kyle", "default").is_err());
        let partial = r#"{ "kyle": { "red": { "xOffset": 1 } } }"#;
        assert!(offset_from_json(partial, "kyle", "red").is_err());
    }

    #[test]
    fn a_piece_sits_two_units_down_its_bolt() {
        // An identity bolt at (0, 0, 60) on an unrotated, unscaled actor.
        let raw = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 60.0],
        ];
        let transform = sjk_runtime::Transform {
            translation: [100.0, 0.0, 0.0],
            rotation: Quat::IDENTITY.to_array(),
            scale: [1.0; 3],
        };
        let instance = placement(raw, transform, [0.0, 0.0, 1.0]).expect("placed");
        let position = Vec3::from_array(instance.position);
        assert!((position - Vec3::new(100.0, 0.0, 59.0)).length() < 1e-4);
        let rotation = Quat::from_array(instance.rotation);
        assert!((rotation * Vec3::Z - Vec3::Z).length() < 1e-4);
    }
}
