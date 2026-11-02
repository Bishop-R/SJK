//! Fonts installed on the PC, for chat (`cg_chatFont`) and the console (`con_font`).
//!
//! The player names a family the system has (Saturn's idea, 29/10/2026); empty keeps
//! SJK's own font. Discovery reads only each font file's table directory and `name`
//! table ([`read_faces`]), on a worker the first time the list is wanted, from the
//! system's font folders ([`font_directories`]). Picking a family rasterizes its
//! 256 Windows-1252 slots once, on a worker, into a coverage atlas like the bundled
//! fonts' ([`load`]): chat as a proportional face (its bold face as the second weight
//! when the family has one), the console in its fixed 8 by 16 cell, each glyph centred
//! in the cell and the em sized so the widest ASCII character fits. A slot the font
//! lacks comes from SJK's default for the scope (Inter, JetBrains Mono); characters
//! beyond the code page keep the default's wide-glyph sources (Cyrillic, CJK, Hangul,
//! SJK symbols) and emoji, so they draw as before. A missing or unreadable font keeps
//! the default and logs why. Glyphs are outlined with `ab_glyph`, which reads outlines
//! on demand, so a large CJK system font costs only the 256 glyphs drawn.

use super::{FontAtlas, FontGlyph, GLYPH_COUNT, RasterizedGlyph, TextStyle, UiFont};
use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use fontdue::Metrics;
use std::error::Error;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once, OnceLock};

/// The chat box's and chat field's font family; empty for SJK's own.
pub(crate) const CHAT_CVAR: &str = "cg_chatFont";
/// The console's font family; empty for SJK's own (JetBrains Mono).
pub(crate) const CONSOLE_CVAR: &str = "con_font";
/// What the settings call the empty value.
pub(crate) const DEFAULT_NAME: &str = "SJK default";

/// Raster size of a chat glyph's em in pixels, as the SJK UI's families (1.5x the
/// 96-pixel modern raster); layout units are a third of it, as theirs are.
const CHAT_RASTER_EM: f32 = 144.0;
const CHAT_UNITS: f32 = 3.0;
/// Raster em of a console glyph, as JetBrains Mono's.
const CONSOLE_RASTER_EM: f32 = 96.0;
/// Largest `name` table read; real ones are a few kilobytes.
const MAX_NAME_TABLE: u32 = 1 << 20;
/// Most faces read from one collection file.
const MAX_COLLECTION: u32 = 64;
/// How deep the font folders are walked (Linux keeps fonts in subfolders).
const MAX_DEPTH: usize = 4;
/// Largest glyph (ink or advance), in ems, taken from a system font; a larger one is
/// treated as missing.
const MAX_GLYPH_EMS: f32 = 4.0;
/// The line height a system font is held to, in ems, and its largest line gap.
const LINE_EMS: (f32, f32) = (1.0, 1.6);
const MAX_GAP_EMS: f32 = 0.5;
/// Tallest atlas a system font may make: wgpu's default `max_texture_dimension_2d`.
const MAX_ATLAS_HEIGHT: u32 = 8_192;

/// Which surfaces a system font replaces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Scope {
    Chat,
    Console,
}

impl Scope {
    pub(crate) const ALL: [Self; 2] = [Self::Chat, Self::Console];

    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Chat => 0,
            Self::Console => 1,
        }
    }

    pub(crate) const fn cvar(self) -> &'static str {
        match self {
            Self::Chat => CHAT_CVAR,
            Self::Console => CONSOLE_CVAR,
        }
    }
}

/// A face's family and style names, from its `name` table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FaceName {
    pub(crate) family: String,
    pub(crate) style: String,
}

/// One face of a font file: the file and the face's index in a collection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FaceFile {
    pub(crate) path: PathBuf,
    pub(crate) index: u32,
}

/// A family installed on the PC, with the faces SJK draws.
#[derive(Clone, Debug)]
pub(crate) struct InstalledFamily {
    pub(crate) name: String,
    pub(crate) regular: FaceFile,
    pub(crate) bold: Option<FaceFile>,
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn read_exact_at(file: &mut (impl Read + Seek), at: u64, length: usize) -> io::Result<Vec<u8>> {
    file.seek(SeekFrom::Start(at))?;
    let mut bytes = vec![0; length];
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

/// The family and style of every face in a TrueType, OpenType or collection file,
/// in face order, reading only the headers and `name` tables. A face whose names
/// cannot be read is `None`.
pub(crate) fn read_faces(file: &mut (impl Read + Seek)) -> io::Result<Vec<Option<FaceName>>> {
    let header = read_exact_at(file, 0, 12)?;
    let offsets = if &header[..4] == b"ttcf" {
        let count = u32_at(&header, 8).unwrap_or(0).min(MAX_COLLECTION);
        let table = read_exact_at(file, 12, count as usize * 4)?;
        (0..count as usize)
            .filter_map(|face| u32_at(&table, face * 4))
            .collect()
    } else if matches!(&header[..4], [0, 1, 0, 0] | b"OTTO" | b"true") {
        vec![0]
    } else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "not a font"));
    };
    Ok(offsets
        .into_iter()
        .map(|offset| face_name(file, u64::from(offset)).ok().flatten())
        .collect())
}

fn face_name(file: &mut (impl Read + Seek), offset: u64) -> io::Result<Option<FaceName>> {
    let directory = read_exact_at(file, offset, 12)?;
    let tables = u16_at(&directory, 4).unwrap_or(0) as usize;
    let records = read_exact_at(file, offset + 12, tables * 16)?;
    let Some(record) = records
        .as_chunks::<16>()
        .0
        .iter()
        .find(|record| &record[..4] == b"name")
    else {
        return Ok(None);
    };
    let (Some(at), Some(length)) = (u32_at(record, 8), u32_at(record, 12)) else {
        return Ok(None);
    };
    let table = read_exact_at(file, u64::from(at), length.min(MAX_NAME_TABLE) as usize)?;
    Ok(parse_names(&table))
}

/// The family (typographic family, ID 16, else ID 1) and style (ID 17, else 2) of a
/// `name` table, preferring Windows English, then any Windows or Unicode name, then
/// a Mac Roman one.
pub(crate) fn parse_names(table: &[u8]) -> Option<FaceName> {
    let count = u16_at(table, 2)? as usize;
    let strings = u16_at(table, 4)? as usize;
    let lookup = |wanted: u16| -> Option<String> {
        let mut best: Option<(u8, String)> = None;
        for record in 0..count {
            let at = 6 + record * 12;
            let Some(record) = table.get(at..at + 12) else {
                // The table is shorter than its count: the records stop here.
                break;
            };
            let field = |offset| u16::from_be_bytes([record[offset], record[offset + 1]]);
            let (platform, encoding, language, id) = (field(0), field(2), field(4), field(6));
            if id != wanted {
                continue;
            }
            let (length, start) = (field(8) as usize, strings + field(10) as usize);
            let Some(bytes) = table.get(start..start + length) else {
                continue;
            };
            let (rank, text) = match platform {
                3 | 0 => {
                    let units: Vec<u16> = bytes
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|pair| u16::from_be_bytes(*pair))
                        .collect();
                    let rank = match (platform, language) {
                        (3, 0x0409) => 0,
                        (3, _) => 1,
                        _ => 2,
                    };
                    (rank, String::from_utf16_lossy(&units))
                }
                1 if encoding == 0 => (3, bytes.iter().map(|&byte| char::from(byte)).collect()),
                _ => continue,
            };
            let text = text.trim().to_owned();
            if !text.is_empty() && best.as_ref().is_none_or(|(held, _)| rank < *held) {
                best = Some((rank, text));
            }
        }
        best.map(|(_, text)| text)
    };
    let family = lookup(16).or_else(|| lookup(1))?;
    let style = lookup(17)
        .or_else(|| lookup(2))
        .unwrap_or_else(|| "Regular".to_owned());
    Some(FaceName { family, style })
}

/// How far a style is from the upright regular face (0 best).
fn regular_rank(style: &str) -> u8 {
    let style = style.to_ascii_lowercase();
    let slanted = style.contains("italic") || style.contains("oblique");
    let base = match style.replace("italic", "").replace("oblique", "").trim() {
        "" | "regular" | "normal" | "book" | "roman" | "plain" => 0,
        "medium" => 1,
        "light" | "semilight" => 2,
        "semibold" | "demibold" | "bold" => 4,
        _ => 3,
    };
    base + if slanted { 10 } else { 0 }
}

/// How far a style is from the upright bold face; `None` for one that is not bold.
fn bold_rank(style: &str) -> Option<u8> {
    let style = style.to_ascii_lowercase();
    if style.contains("italic") || style.contains("oblique") {
        return None;
    }
    match style.trim() {
        "bold" => Some(0),
        "semibold" | "demibold" | "semi bold" | "demi bold" => Some(1),
        _ => None,
    }
}

/// Families of `faces`, one each (names compared without case), with their regular and
/// bold faces, sorted by name. Hidden families (a leading `.`, macOS's system UI ones)
/// are left out.
pub(crate) fn families(
    faces: impl IntoIterator<Item = (FaceName, FaceFile)>,
) -> Vec<InstalledFamily> {
    let mut grouped: std::collections::BTreeMap<String, Vec<(FaceName, FaceFile)>> =
        std::collections::BTreeMap::new();
    for (name, file) in faces {
        if name.family.starts_with('.') || name.family.is_empty() {
            continue;
        }
        grouped
            .entry(name.family.to_lowercase())
            .or_default()
            .push((name, file));
    }
    grouped
        .into_values()
        .filter_map(|faces| {
            let (name, regular) = faces
                .iter()
                .min_by_key(|(name, _)| regular_rank(&name.style))?
                .clone();
            let bold = faces
                .iter()
                .filter_map(|(name, file)| Some((bold_rank(&name.style)?, file)))
                .min_by_key(|(rank, _)| *rank)
                .map(|(_, file)| file.clone())
                .filter(|file| *file != regular);
            Some(InstalledFamily {
                name: name.family,
                regular,
                bold,
            })
        })
        .collect()
}

/// The folders the system keeps fonts in, the user's own included.
pub(crate) fn font_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    let env = |name: &str| std::env::var_os(name).map(PathBuf::from);
    if cfg!(windows) {
        directories.extend(env("WINDIR").map(|windows| windows.join("Fonts")));
        directories.extend(
            env("LOCALAPPDATA").map(|local| local.join("Microsoft").join("Windows").join("Fonts")),
        );
    } else if cfg!(target_os = "macos") {
        directories.extend(["/System/Library/Fonts", "/Library/Fonts"].map(PathBuf::from));
        directories.extend(env("HOME").map(|home| home.join("Library").join("Fonts")));
    } else {
        let data =
            env("XDG_DATA_HOME").or_else(|| env("HOME").map(|home| home.join(".local/share")));
        directories.extend(data.map(|data| data.join("fonts")));
        directories.extend(env("HOME").map(|home| home.join(".fonts")));
        directories.extend(
            xdg_data_dirs(std::env::var_os("XDG_DATA_DIRS"))
                .into_iter()
                .map(|data| data.join("fonts")),
        );
    }
    let mut seen = std::collections::HashSet::new();
    directories.retain(|directory| seen.insert(directory.clone()));
    directories
}

/// The system data folders of `XDG_DATA_DIRS` (NixOS and Flatpak keep their
/// fonts there), or the spec's default when it is unset or empty.
fn xdg_data_dirs(value: Option<std::ffi::OsString>) -> Vec<PathBuf> {
    let value = value
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    std::env::split_paths(&value)
        .filter(|path| path.is_absolute())
        .collect()
}

fn is_font_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["ttf", "otf", "ttc", "otc"]
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
}

/// The font files under `directory`, at most [`MAX_DEPTH`] folders down.
/// Symlinked folders are followed (Linux distributions link font packages
/// into `/usr/share/fonts`); `visited` keeps each real folder to one visit, so
/// a link loop or two links to one folder add nothing twice.
fn collect_files(
    directory: &Path,
    depth: usize,
    visited: &mut std::collections::HashSet<PathBuf>,
    files: &mut Vec<PathBuf>,
) {
    let real = std::fs::canonicalize(directory).unwrap_or_else(|_| directory.to_path_buf());
    if !visited.insert(real) {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_dir = match entry.file_type() {
            Ok(kind) if kind.is_symlink() => path.is_dir(),
            Ok(kind) => kind.is_dir(),
            Err(_) => false,
        };
        if is_dir {
            if depth < MAX_DEPTH {
                collect_files(&path, depth + 1, visited, files);
            }
        } else if is_font_file(&path) {
            files.push(path);
        }
    }
}

/// Every family in `directories`.
pub(crate) fn scan(directories: &[PathBuf]) -> Vec<InstalledFamily> {
    let mut files = Vec::new();
    let mut visited = std::collections::HashSet::new();
    for directory in directories {
        collect_files(directory, 0, &mut visited, &mut files);
    }
    let faces = files.into_iter().flat_map(|path| {
        let faces = std::fs::File::open(&path)
            .and_then(|file| read_faces(&mut io::BufReader::new(file)))
            .unwrap_or_default();
        faces
            .into_iter()
            .enumerate()
            .filter_map(move |(index, name)| {
                Some((
                    name?,
                    FaceFile {
                        path: path.clone(),
                        index: index as u32,
                    },
                ))
            })
    });
    families(faces)
}

static CATALOGUE: OnceLock<Vec<InstalledFamily>> = OnceLock::new();
static SCAN: Once = Once::new();

fn installed_blocking() -> &'static [InstalledFamily] {
    CATALOGUE.get_or_init(|| scan(&font_directories()))
}

/// The installed families once they are read; the first call starts reading them on
/// a worker and returns `None` until it is done.
pub(crate) fn installed() -> Option<&'static [InstalledFamily]> {
    if let Some(families) = CATALOGUE.get() {
        return Some(families);
    }
    SCAN.call_once(|| {
        let _ = std::thread::Builder::new()
            .name("installed fonts".to_owned())
            .spawn(|| {
                installed_blocking();
            });
    });
    None
}

/// The values the font rows offer: empty (SJK's own) then every installed family.
pub(crate) fn choices() -> &'static [&'static str] {
    static CHOICES: OnceLock<Vec<&'static str>> = OnceLock::new();
    let Some(families) = installed() else {
        return &[""];
    };
    CHOICES.get_or_init(|| {
        std::iter::once("")
            .chain(families.iter().map(|family| family.name.as_str()))
            .collect()
    })
}

/// What the settings show for `value` of a font row: [`DEFAULT_NAME`] for empty.
pub(crate) fn choice_name(cvar: &str, value: &str) -> Option<&'static str> {
    let font_row = cvar.eq_ignore_ascii_case(CHAT_CVAR) || cvar.eq_ignore_ascii_case(CONSOLE_CVAR);
    (font_row && value.trim().is_empty()).then_some(DEFAULT_NAME)
}

/// `name`'s family in `families`, compared without case.
pub(crate) fn find<'a>(families: &'a [InstalledFamily], name: &str) -> Option<&'a InstalledFamily> {
    let name = name.trim();
    families
        .iter()
        .find(|family| family.name.eq_ignore_ascii_case(name))
}

/// The last atlas made for each scope, by family, so the next world takes it without
/// reading the font again.
static LOADED: Mutex<[Option<Loaded>; 2]> = Mutex::new([None, None]);

/// A family's name and the atlas made from it.
pub(crate) type Loaded = (String, Arc<FontAtlas>);

/// The atlas already made for `family` in `scope`, if any, with the family's name as
/// the system spells it.
pub(crate) fn cached(scope: Scope, family: &str) -> Option<Loaded> {
    let loaded = LOADED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    loaded[scope.index()]
        .as_ref()
        .filter(|(name, _)| name.eq_ignore_ascii_case(family.trim()))
        .cloned()
}

/// Keep `atlas` of family `name` as the one [`cached`] gives for `scope`: called for the
/// atlas taken into use, never for a load that was asked for and then replaced.
pub(crate) fn remember(scope: Scope, name: &str, atlas: &Arc<FontAtlas>) {
    let mut loaded = LOADED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    loaded[scope.index()] = Some((name.to_owned(), Arc::clone(atlas)));
}

/// Find `family` on the PC and rasterize it for `scope` (blocking: run on a worker),
/// with the family's name as the system spells it. Nothing is cached or logged here:
/// the caller does that for the load it takes into use.
pub(crate) fn load_installed(scope: Scope, family: &str) -> Result<Loaded, String> {
    if let Some(loaded) = cached(scope, family) {
        return Ok(loaded);
    }
    let found = find(installed_blocking(), family)
        .ok_or_else(|| format!("no font named \"{}\" is installed", family.trim()))?;
    let read = |face: &FaceFile| std::fs::read(&face.path).map(|bytes| (bytes, face.index));
    let regular = read(&found.regular).map_err(|error| format!("{}: {error}", found.name))?;
    // Only chat draws a second weight.
    let bold = match scope {
        Scope::Chat => found.bold.as_ref().and_then(|face| read(face).ok()),
        Scope::Console => None,
    };
    let atlas = load(
        scope,
        (&regular.0, regular.1),
        bold.as_ref()
            .map(|(bytes, index)| (bytes.as_slice(), *index)),
    )
    .map_err(|error| format!("{}: {error}", found.name))?;
    Ok((found.name.clone(), Arc::new(atlas)))
}

/// Whether `value` is a usable size: finite and above zero (NaN is not).
fn positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

/// The pixel scale that sets `font`'s em to `em` pixels; `None` for a font whose
/// header gives no usable em or height.
fn em_scale(font: &FontRef<'_>, em: f32) -> Option<PxScale> {
    let units = font.units_per_em().unwrap_or(1_000.0);
    let height = font.height_unscaled();
    let scale = em * height / units;
    (positive(units) && positive(height) && positive(scale)).then(|| PxScale::from(scale))
}

/// `character` of `font` at `em` pixels, as the fontdue metrics and coverage the
/// atlas packer takes; `None` when the font has no glyph for it or the glyph is
/// broken: bounds or advance that are not finite, or larger than [`MAX_GLYPH_EMS`]
/// ems (a damaged or crafted font must not make the atlas allocate gigabytes).
fn rasterize(font: &FontRef<'_>, em: f32, character: char) -> Option<(Metrics, Vec<u8>)> {
    let scale = em_scale(font, em)?;
    let id = font.glyph_id(character);
    let advance_width = font.as_scaled(scale).h_advance(id);
    let limit = em * MAX_GLYPH_EMS;
    if !advance_width.is_finite() || advance_width.abs() > limit {
        return None;
    }
    let advance_width = advance_width.max(0.0);
    let Some(outlined) = font.outline_glyph(id.with_scale_and_position(scale, point(0.0, 0.0)))
    else {
        // No outline: a space, an advance and no ink.
        return Some((
            Metrics {
                advance_width,
                ..Metrics::default()
            },
            Vec::new(),
        ));
    };
    let bounds = outlined.px_bounds();
    let corners = [bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y];
    if corners
        .iter()
        .any(|corner| !corner.is_finite() || corner.abs() > limit)
        || bounds.width() > limit
        || bounds.height() > limit
    {
        return None;
    }
    let (width, height) = (
        bounds.width().max(0.0) as usize,
        bounds.height().max(0.0) as usize,
    );
    let mut pixels = vec![0; width * height];
    outlined.draw(|x, y, coverage| {
        let (x, y) = (x as usize, y as usize);
        if x < width && y < height {
            pixels[y * width + x] = (coverage.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    });
    Some((
        Metrics {
            xmin: bounds.min.x as i32,
            ymin: -(bounds.max.y as i32),
            width,
            height,
            advance_width,
            ..Metrics::default()
        },
        pixels,
    ))
}

/// Slot `byte`'s glyph: `font`'s own when it has a sound one, else `fallback`'s
/// (SJK's default for the scope), else `.`, else nothing.
fn rasterize_slot(
    font: &FontRef<'_>,
    fallback: &FontRef<'_>,
    byte: u8,
    em: f32,
) -> (Metrics, Vec<u8>) {
    let character = sjk_protocol::windows_1252_char(byte);
    let own = matches!(byte, b'\n' | b'\r' | b' ') || font.glyph_id(character).0 != 0;
    own.then(|| rasterize(font, em, character))
        .flatten()
        .or_else(|| {
            (fallback.glyph_id(character).0 != 0)
                .then(|| rasterize(fallback, em, character))
                .flatten()
        })
        .or_else(|| rasterize(font, em, '.'))
        .or_else(|| rasterize(fallback, em, '.'))
        .unwrap_or_default()
}

/// A font's line at `em` pixels: ascent, descent (negative) and gap, `None` when its
/// metrics are unusable. A line outside [`LINE_EMS`] is scaled into it, keeping where
/// the baseline sits, so a font with huge or tiny `hhea` metrics still draws its
/// letters at their size.
fn line(font: &FontRef<'_>, em: f32) -> Option<(f32, f32, f32)> {
    let scaled = font.as_scaled(em_scale(font, em)?);
    let (ascent, descent, gap) = (scaled.ascent(), scaled.descent(), scaled.line_gap());
    let height = ascent - descent;
    if !positive(height) || !ascent.is_finite() || !descent.is_finite() {
        return None;
    }
    let fit = height.clamp(em * LINE_EMS.0, em * LINE_EMS.1) / height;
    let gap = if gap.is_finite() {
        gap.clamp(0.0, em * MAX_GAP_EMS)
    } else {
        0.0
    };
    Some((ascent * fit, descent * fit, gap))
}

/// Rasterize a system font for `scope`: `regular` (font bytes and face index) and, for
/// chat, its `bold` face when the family has one.
pub(crate) fn load(
    scope: Scope,
    regular: (&[u8], u32),
    bold: Option<(&[u8], u32)>,
) -> Result<FontAtlas, Box<dyn Error>> {
    let font = FontRef::try_from_slice_and_index(regular.0, regular.1)?;
    match scope {
        Scope::Chat => {
            let bold = bold
                .and_then(|(bytes, index)| FontRef::try_from_slice_and_index(bytes, index).ok())
                .filter(|bold| line(bold, CHAT_RASTER_EM).is_some());
            load_chat(&font, bold.as_ref())
        }
        Scope::Console => load_console(&font),
    }
}

/// A packed atlas and each glyph's rectangle in it.
type Packed = (image::GrayImage, Vec<[f32; 4]>);

/// Pack the glyphs, or fail when the atlas would be taller than a texture may be.
fn pack(rasterized: &[RasterizedGlyph]) -> Result<Packed, Box<dyn Error>> {
    super::paint_atlas_within(rasterized, MAX_ATLAS_HEIGHT)
        .ok_or_else(|| "its glyphs do not fit one atlas".into())
}

fn load_chat(font: &FontRef<'_>, bold: Option<&FontRef<'_>>) -> Result<FontAtlas, Box<dyn Error>> {
    let fallbacks = [
        FontRef::try_from_slice(super::INTER_REGULAR)?,
        FontRef::try_from_slice(super::INTER_SEMIBOLD)?,
    ];
    let faces = [font, bold.unwrap_or(font)];
    let (ascent, descent, gap) =
        line(font, CHAT_RASTER_EM).ok_or("the font has no usable line metrics")?;
    let face_count = if bold.is_some() { 2 } else { 1 };
    let mut rasterized = Vec::with_capacity(GLYPH_COUNT * face_count);
    for (face, font) in faces.iter().take(face_count).enumerate() {
        for byte in 0..GLYPH_COUNT {
            let (metrics, pixels) =
                rasterize_slot(font, &fallbacks[face], byte as u8, CHAT_RASTER_EM);
            rasterized.push(RasterizedGlyph {
                face,
                byte,
                metrics,
                pixels,
            });
        }
    }
    let (image, rectangles) = pack(&rasterized)?;
    let mut glyphs = [[FontGlyph::default(); GLYPH_COUNT]; 2];
    for (glyph, uv) in rasterized.iter().zip(rectangles) {
        let metrics = glyph.metrics;
        let entry = FontGlyph {
            width: metrics.width as f32 / CHAT_UNITS,
            height: metrics.height as f32 / CHAT_UNITS,
            advance: metrics.advance_width / CHAT_UNITS,
            offset_x: metrics.xmin as f32 / CHAT_UNITS,
            offset_y: (ascent - (metrics.ymin as f32 + metrics.height as f32)) / CHAT_UNITS,
            uv,
        };
        glyphs[glyph.face][glyph.byte] = entry;
        if face_count == 1 {
            glyphs[1][glyph.byte] = entry;
        }
    }
    Ok(FontAtlas {
        font: UiFont {
            glyphs,
            wide: super::wide::INTER,
            height: (ascent - descent + gap) / CHAT_UNITS,
            modern: true,
            style: TextStyle::NEUTRAL,
        },
        image,
        distance_field: false,
    })
}

fn load_console(font: &FontRef<'_>) -> Result<FontAtlas, Box<dyn Error>> {
    use super::console_font::{HEIGHT, WIDTH};
    let fallback = FontRef::try_from_slice(super::console_font::JETBRAINS_MONO)?;
    let em = CONSOLE_RASTER_EM;
    let (ascent, descent, _) = line(font, em).ok_or("the font has no usable line metrics")?;
    let rasterized: Vec<RasterizedGlyph> = (0..GLYPH_COUNT)
        .map(|byte| {
            let (metrics, pixels) = rasterize_slot(font, &fallback, byte as u8, em);
            RasterizedGlyph {
                face: 0,
                byte,
                metrics,
                pixels,
            }
        })
        .collect();
    // The widest printable ASCII character fills one cell.
    let widest = rasterized[0x21..=0x7E]
        .iter()
        .map(|glyph| glyph.metrics.advance_width.max(glyph.metrics.width as f32))
        .fold(0.0_f32, f32::max);
    if !positive(widest) {
        return Err("the font has no printable ASCII characters".into());
    }
    let unit = (WIDTH / widest).min(HEIGHT / (ascent - descent));
    let baseline = (HEIGHT + (ascent + descent) * unit) * 0.5;
    let (image, rectangles) = pack(&rasterized)?;
    let mut glyphs = [[FontGlyph::default(); GLYPH_COUNT]; 2];
    for (glyph, uv) in rasterized.iter().zip(rectangles) {
        let metrics = glyph.metrics;
        let ink_top = metrics.ymin as f32 + metrics.height as f32;
        // Centred in the cell: proportional fonts keep the console a grid.
        let centring = (WIDTH - metrics.advance_width * unit) * 0.5;
        let entry = FontGlyph {
            width: metrics.width as f32 * unit,
            height: metrics.height as f32 * unit,
            advance: WIDTH,
            offset_x: centring + metrics.xmin as f32 * unit,
            offset_y: baseline - ink_top * unit,
            uv,
        };
        glyphs[0][glyph.byte] = entry;
        glyphs[1][glyph.byte] = entry;
    }
    Ok(FontAtlas {
        font: UiFont {
            glyphs,
            wide: [super::wide::MONO; 2],
            height: HEIGHT,
            modern: false,
            style: TextStyle::NEUTRAL,
        },
        image,
        distance_field: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::TextFace;
    use std::io::Cursor;

    /// A `name` table with Windows English names `(id, text)`, and a Mac Roman family.
    fn name_table(names: &[(u16, &str)], mac_family: Option<&str>) -> Vec<u8> {
        let mut records = Vec::new();
        let mut strings = Vec::new();
        let mut push = |platform: u16, encoding: u16, language: u16, id: u16, bytes: Vec<u8>| {
            for value in [
                platform,
                encoding,
                language,
                id,
                bytes.len() as u16,
                strings.len() as u16,
            ] {
                records.extend(value.to_be_bytes());
            }
            strings.extend(bytes);
        };
        if let Some(family) = mac_family {
            push(1, 0, 0, 1, family.bytes().collect());
        }
        for &(id, text) in names {
            push(
                3,
                1,
                0x0409,
                id,
                text.encode_utf16().flat_map(u16::to_be_bytes).collect(),
            );
        }
        let count = records.len() / 12;
        let mut table = Vec::new();
        table.extend(0_u16.to_be_bytes());
        table.extend((count as u16).to_be_bytes());
        table.extend((6 + records.len() as u16).to_be_bytes());
        table.extend(records);
        table.extend(strings);
        table
    }

    /// A font file of one face holding only `name` (at `base` in a collection).
    fn sfnt(name: &[u8], base: usize) -> Vec<u8> {
        let mut face = Vec::new();
        face.extend([0, 1, 0, 0]);
        face.extend(1_u16.to_be_bytes());
        face.extend([0; 6]);
        face.extend(b"name");
        face.extend(0_u32.to_be_bytes());
        face.extend(((base + 28) as u32).to_be_bytes());
        face.extend((name.len() as u32).to_be_bytes());
        face.extend(name);
        face
    }

    #[test]
    fn names_prefer_the_typographic_family_and_windows_english() {
        let table = name_table(
            &[
                (1, "Segoe UI Semibold"),
                (2, "Regular"),
                (16, "Segoe UI"),
                (17, "Semibold"),
            ],
            Some("Mac Name"),
        );
        let name = parse_names(&table).unwrap();
        assert_eq!(name.family, "Segoe UI");
        assert_eq!(name.style, "Semibold");
        // Only a Mac Roman family: taken, the style defaulting to Regular.
        let mac = parse_names(&name_table(&[], Some("Chicago"))).unwrap();
        assert_eq!(
            (mac.family.as_str(), mac.style.as_str()),
            ("Chicago", "Regular")
        );
        assert!(parse_names(&[0, 0]).is_none());
    }

    #[test]
    fn single_fonts_and_collections_list_their_faces() {
        let table = name_table(&[(1, "Arial"), (2, "Bold")], None);
        let faces = read_faces(&mut Cursor::new(sfnt(&table, 0))).unwrap();
        assert_eq!(
            faces,
            [Some(FaceName {
                family: "Arial".into(),
                style: "Bold".into()
            })]
        );
        // A collection of two faces.
        let first = sfnt(&name_table(&[(1, "MS Gothic")], None), 20);
        let second_at = 20 + first.len();
        let second = sfnt(&name_table(&[(1, "MS PGothic")], None), second_at);
        let mut file = Vec::new();
        file.extend(b"ttcf");
        file.extend([0, 1, 0, 0]);
        file.extend(2_u32.to_be_bytes());
        file.extend(20_u32.to_be_bytes());
        file.extend((second_at as u32).to_be_bytes());
        file.extend(first);
        file.extend(second);
        let faces = read_faces(&mut Cursor::new(file)).unwrap();
        let names: Vec<_> = faces
            .iter()
            .map(|face| face.as_ref().unwrap().family.as_str())
            .collect();
        assert_eq!(names, ["MS Gothic", "MS PGothic"]);
        assert!(read_faces(&mut Cursor::new(b"PK\x03\x04 not a font".to_vec())).is_err());
    }

    #[test]
    fn families_pick_their_regular_and_bold_faces() {
        let face = |family: &str, style: &str, file: &str| {
            (
                FaceName {
                    family: family.into(),
                    style: style.into(),
                },
                FaceFile {
                    path: file.into(),
                    index: 0,
                },
            )
        };
        let found = families([
            face("Verdana", "Bold Italic", "verdanaz.ttf"),
            face("Verdana", "Bold", "verdanab.ttf"),
            face("verdana", "Regular", "verdana.ttf"),
            face("Verdana", "Italic", "verdanai.ttf"),
            face("Bahnschrift", "Light", "bahn-light.ttf"),
            face(".SF NS", "Regular", "sf.ttf"),
        ]);
        let names: Vec<_> = found.iter().map(|family| family.name.as_str()).collect();
        assert_eq!(names, ["Bahnschrift", "verdana"]);
        assert_eq!(found[1].regular.path, PathBuf::from("verdana.ttf"));
        assert_eq!(
            found[1].bold.as_ref().unwrap().path,
            PathBuf::from("verdanab.ttf")
        );
        assert!(found[0].bold.is_none());
        assert!(find(&found, " VERDANA ").is_some());
        assert!(find(&found, "Comic Sans MS").is_none());
    }

    #[test]
    fn the_rows_name_the_default() {
        assert_eq!(choice_name(CHAT_CVAR, ""), Some(DEFAULT_NAME));
        assert_eq!(choice_name("con_font", " "), Some(DEFAULT_NAME));
        assert_eq!(choice_name(CHAT_CVAR, "Arial"), None);
        assert_eq!(choice_name("cg_drawChat", ""), None);
    }

    #[test]
    fn missing_glyphs_fall_back_and_wide_characters_keep_their_sources() {
        // SJK Symbols stands in for a system font: it has capitals, but not `é` or `€`.
        let symbols = include_bytes!("../../assets/fonts/SJKSymbols.ttf");
        let font = FontRef::try_from_slice(symbols).unwrap();
        let inter = FontRef::try_from_slice(super::super::INTER_REGULAR).unwrap();
        assert_eq!(font.glyph_id('€').0, 0);
        let (metrics, _) = rasterize_slot(&font, &inter, 0x80, 96.0);
        let (inter_euro, _) = rasterize(&inter, 96.0, '€').unwrap();
        assert_eq!(
            metrics.width, inter_euro.width,
            "a missing slot comes from Inter"
        );
        let atlas = load(Scope::Chat, (symbols, 0), None).unwrap();
        let chat = &atlas.font;
        assert!(chat.is_modern());
        assert!(
            chat.glyph(TextFace::Regular, 0x80).width > 0.0,
            "€ from Inter"
        );
        assert!(chat.glyph(TextFace::Semibold, b'H').width > 0.0);
        // Cyrillic, kanji and emoji go to Inter's wide sources as before.
        assert_eq!(chat.wide[0], crate::text::wide::INTER[0]);
        let console = load(Scope::Console, (symbols, 0), None).unwrap().font;
        for byte in 0..=255_u8 {
            let glyph = console.glyph(TextFace::Regular, byte);
            assert_eq!(glyph.advance, super::super::console_font::WIDTH);
        }
        assert_eq!(console.wide[0], crate::text::wide::MONO);
    }

    #[test]
    fn a_proportional_console_font_keeps_its_ink_in_the_cell() {
        let font = load(Scope::Console, (super::super::INTER_REGULAR, 0), None)
            .unwrap()
            .font;
        let width = super::super::console_font::WIDTH;
        for byte in 0x21..=0x7E_u8 {
            let glyph = font.glyph(TextFace::Regular, byte);
            assert!(glyph.offset_x >= -0.6, "{byte} {glyph:?}");
            assert!(
                glyph.offset_x + glyph.width <= width + 0.6,
                "{byte} {glyph:?}"
            );
        }
    }

    #[test]
    fn an_unreadable_font_is_an_error() {
        assert!(load(Scope::Chat, (b"not a font", 0), None).is_err());
        assert!(load_installed(Scope::Chat, "\u{1}no such family\u{1}").is_err());
    }

    /// Malformed headers and tables are errors or `None`, never a panic or a huge read.
    #[test]
    fn malformed_files_and_tables_are_refused() {
        // Truncated header.
        assert!(read_faces(&mut Cursor::new(vec![0, 1, 0])).is_err());
        // A collection claiming 0xFFFFFFFF faces with no offsets after it.
        let mut huge = b"ttcf".to_vec();
        huge.extend([0, 1, 0, 0]);
        huge.extend(u32::MAX.to_be_bytes());
        assert!(read_faces(&mut Cursor::new(huge)).is_err());
        // A collection whose face offset lies past the end of the file.
        let mut past = b"ttcf".to_vec();
        past.extend([0, 1, 0, 0]);
        past.extend(1_u32.to_be_bytes());
        past.extend(1_000_000_u32.to_be_bytes());
        assert_eq!(read_faces(&mut Cursor::new(past)).unwrap(), [None]);
        // A `name` record pointing past the end of the file.
        let mut face = sfnt(&[], 0);
        face.truncate(20);
        face.extend(u32::MAX.to_be_bytes());
        face.extend(64_u32.to_be_bytes());
        assert_eq!(read_faces(&mut Cursor::new(face)).unwrap(), [None]);
        // More records than the table holds: the ones present still count.
        let mut table = name_table(&[(1, "Tahoma")], None);
        table[2..4].copy_from_slice(&900_u16.to_be_bytes());
        assert_eq!(parse_names(&table).unwrap().family, "Tahoma");
        // A string offset out of range is skipped, not read.
        let mut table = name_table(&[(1, "Tahoma")], None);
        table[6 + 10..6 + 12].copy_from_slice(&u16::MAX.to_be_bytes());
        assert!(parse_names(&table).is_none());
        // A bad record after a good one keeps the good one.
        let mut table = name_table(&[(1, "Tahoma"), (1, "Other")], None);
        table[6 + 12 + 10..6 + 12 + 12].copy_from_slice(&u16::MAX.to_be_bytes());
        assert_eq!(parse_names(&table).unwrap().family, "Tahoma");
    }

    /// A glyph the packer cannot place (wider than the atlas, or whose pixels do not
    /// match its size) draws nothing instead of panicking, and an atlas taller than a
    /// texture may be is refused before it is allocated.
    #[test]
    fn degenerate_glyphs_do_not_break_the_atlas() {
        let glyph = |width: usize, height: usize, pixels: usize| RasterizedGlyph {
            face: 0,
            byte: 0,
            metrics: Metrics {
                width,
                height,
                ..Metrics::default()
            },
            pixels: vec![255; pixels],
        };
        let glyphs = [
            glyph(10, 10, 100),
            glyph(100_000, 2, 200_000),
            glyph(4, 4, 3),
        ];
        let (image, rectangles) = super::super::paint_atlas_within(&glyphs, 8_192).unwrap();
        assert!(image.height() <= 64);
        assert_eq!(
            rectangles[1][0], rectangles[1][2],
            "the wide glyph has no ink"
        );
        assert_eq!(rectangles[2][1], rectangles[2][3], "the short one neither");
        let tall: Vec<_> = (0..21).map(|_| glyph(4_000, 400, 1_600_000)).collect();
        assert!(super::super::paint_atlas_within(&tall, MAX_ATLAS_HEIGHT).is_none());
        assert!(pack(&tall).is_err());
    }

    /// Unusable sizes are caught, NaN included.
    #[test]
    fn unusable_sizes_are_not_positive() {
        assert!(positive(1.0));
        for value in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(!positive(value), "{value}");
        }
        // A sane font's line stays within the band it is held to.
        let inter = FontRef::try_from_slice(super::super::INTER_REGULAR).unwrap();
        let (ascent, descent, gap) = line(&inter, 100.0).unwrap();
        assert!((100.0..=160.0).contains(&(ascent - descent)));
        assert!((0.0..=50.0).contains(&gap));
        // Every glyph of a sane font passes the size check.
        assert!(rasterize(&inter, 100.0, 'W').is_some());
        assert!(rasterize(&inter, 100.0, ' ').is_some());
    }

    /// Font folders are searched below, each real folder once, and only font
    /// files are kept.
    #[test]
    fn font_folders_are_searched_once_each() {
        let root = std::env::temp_dir().join(format!("sjk-font-scan-{}", std::process::id()));
        let nested = root.join("truetype").join("dejavu");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("DejaVuSans.ttf"), b"").unwrap();
        std::fs::write(nested.join("README"), b"").unwrap();
        let mut files = Vec::new();
        let mut visited = std::collections::HashSet::new();
        collect_files(&root, 0, &mut visited, &mut files);
        collect_files(&root.join("truetype"), 0, &mut visited, &mut files);
        assert_eq!(files, vec![nested.join("DejaVuSans.ttf")]);
        // A symlinked folder (as distributions link font packages) is
        // followed, and a link back to its parent adds nothing twice.
        #[cfg(unix)]
        {
            let linked = std::env::temp_dir().join(format!("sjk-font-link-{}", std::process::id()));
            std::fs::create_dir_all(&linked).unwrap();
            std::os::unix::fs::symlink(&root, linked.join("packages")).unwrap();
            std::os::unix::fs::symlink(&linked, linked.join("loop")).unwrap();
            let mut files = Vec::new();
            collect_files(&linked, 0, &mut Default::default(), &mut files);
            assert_eq!(files.len(), 1);
            let _ = std::fs::remove_dir_all(&linked);
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// `XDG_DATA_DIRS` names the system data folders; unset or empty, the
    /// spec's default holds.
    #[cfg(unix)]
    #[test]
    fn xdg_data_dirs_follow_the_spec() {
        let default = vec![
            PathBuf::from("/usr/local/share"),
            PathBuf::from("/usr/share"),
        ];
        assert_eq!(xdg_data_dirs(None), default);
        assert_eq!(xdg_data_dirs(Some("".into())), default);
        assert_eq!(
            xdg_data_dirs(Some(
                "/run/current-system/sw/share:relative:/app/share".into()
            )),
            vec![
                PathBuf::from("/run/current-system/sw/share"),
                PathBuf::from("/app/share")
            ]
        );
    }
}
