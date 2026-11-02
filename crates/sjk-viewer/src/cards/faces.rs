//! Card faces (`docs/cards.md`, "Faces"): what a revealed card shows, read from the SJK
//! packs the hub serves (`sjk_cards`, [`crate::sjk_packs`]). Each card is a file
//! `cards/<id>.card` (JSON: its name, set, number, kind, rarity, lore, artist and the game
//! path of its picture) beside its picture, a JPEG or PNG framed for the card's art
//! window ([`ART_ASPECT`]). The hub keeps which card each copy is; the pack says what it
//! looks like, so a client draws a card it has the face of and shows "Unknown card"
//! (its id) for one it has not.
//!
//! The faces are read whenever the packs are mounted ([`install`]), each picture kept as
//! the pack's file. A picture is decoded only when a screen draws its face ([`request`]):
//! on a worker thread, squeezed into a square mip chain the UI renderer uploads
//! (`ui_renderer/card_art.rs`) and drawn back into the 6:5 window, which undoes the
//! squeeze. Until then the window stays dark. Decoding and uploading every face at
//! start held 1.8 GB of video memory and as much system memory for the hub's 333
//! faces, more than a 3 GB card has (`docs/cards.md`, "Faces").

use super::Rarity;
use crate::menu::emblem::MipChain;
use serde::Deserialize;
use sjk_ui::{Color, TextureId};
use sjk_vfs::VirtualFileSystem;
use std::sync::{Arc, Mutex, MutexGuard};

/// Where the card files are in a pack, and their extension.
pub(crate) const FOLDER: &str = "cards";
pub(crate) const EXTENSION: &str = ".card";
/// The art window's width against its height: 6:5 (pictures are 1020 x 850).
pub(crate) const ART_ASPECT: f32 = 1.2;
/// The square texture a picture is squeezed into at full texture detail; from `r_picmip`
/// 1 on, half that. The inspected card's window is about 605 x 504 pixels at 1080p and
/// twice that at 4K.
const TEXTURE_EDGE: u32 = 1_024;
/// Decoded pictures waiting for the renderer at most; the worker stops at this many and
/// starts again as they are taken.
const READY_MAX: usize = 8;
/// Most faces the client draws pictures for; one texture each.
pub(crate) const MAX_FACES: usize = 512;
/// Card pictures' texture ids: this, plus the face's position in [`Faces::list`].
const TEXTURE_BASE: u32 = 0x6100_0000;
/// The longest name, set name and artist, and lore a face may have (as the hub's pack
/// build checks).
const NAME_MAX: usize = 40;
const LORE_MAX: usize = 400;

/// What a card is about. A face of a kind (or rarity) this client does not know is left
/// out on its own, named in the log; the pack's other faces still read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    Community,
    Place,
    Personality,
    Game,
    /// The people the project owes (19/10/2026): card 1 of SJK Alpha is Bishop.
    Credits,
    /// A meme of the community (19/10/2026).
    Meme,
    /// A droppable saber shader (20/10/2026, the SJK Blades collection): its Solar copies
    /// drop that blade too ([`Face::blade`]).
    Blade,
    /// A clan's card (01/11/2026): the clan is its set ("JoF"), so the strip reads
    /// "Clan · JoF" and each clan gets a set of its own.
    Clan,
}

impl Kind {
    /// Every kind the client knows, in the order the Cards tab's filter walks them.
    pub(crate) const ALL: [Self; 8] = [
        Self::Community,
        Self::Place,
        Self::Personality,
        Self::Game,
        Self::Credits,
        Self::Meme,
        Self::Blade,
        Self::Clan,
    ];

    fn from_id(id: &str) -> Option<Self> {
        match id {
            "community" => Some(Self::Community),
            "place" => Some(Self::Place),
            "personality" => Some(Self::Personality),
            "game" => Some(Self::Game),
            "credits" => Some(Self::Credits),
            "meme" => Some(Self::Meme),
            "blade" => Some(Self::Blade),
            "clan" => Some(Self::Clan),
            _ => None,
        }
    }

    /// The id a card file names it by ("credits"), as the hub's `tag_cards` lists it.
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Community => "community",
            Self::Place => "place",
            Self::Personality => "personality",
            Self::Game => "game",
            Self::Credits => "credits",
            Self::Meme => "meme",
            Self::Blade => "blade",
            Self::Clan => "clan",
        }
    }

    /// What players read ("Community").
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Community => "Community",
            Self::Place => "Place",
            Self::Personality => "Personality",
            Self::Game => "Jedi Academy",
            Self::Credits => "Credits",
            Self::Meme => "Meme",
            Self::Blade => "Blade",
            Self::Clan => "Clan",
        }
    }
}

/// One card's face.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Face {
    /// The card's id ("sjk_alpha_010"), the hub's.
    pub(crate) id: String,
    pub(crate) name: String,
    /// The set's id ("sjk_alpha") and the name players read ("SJK Alpha").
    pub(crate) set: String,
    pub(crate) set_name: String,
    /// Its place in its set, from 1.
    pub(crate) number: u16,
    pub(crate) kind: Kind,
    pub(crate) rarity: &'static Rarity,
    pub(crate) lore: String,
    pub(crate) artist: String,
    /// The game path of its picture in the pack.
    pub(crate) art: String,
    /// The unlockable a copy of it brings while held (`saber_sun`; 20/10/2026), as the
    /// hub grants it ([`crate::unlockables::CARD_BLADES`]); `None` for most cards.
    pub(crate) grants: Option<String>,
    /// The saber shader a blade card is of (`saber_sun`; 20/10/2026): the hub drops it
    /// to whoever finds or reveals a Solar copy. `None` for other cards.
    pub(crate) blade: Option<String>,
    /// What the client draws as its picture from the game's own models, in place of
    /// [`Self::art`] ([`super::scene`]; 30/10/2026); `None` for most cards.
    pub(crate) scene: Option<String>,
    /// The rank it shows in place of its rarity (`rank` and `rank_colour`; 31/10/2026,
    /// Sol's decision for the JoF collection): its rarity then only sets its odds.
    pub(crate) rank: Option<Rank>,
}

/// A card's rank: the name and colour it shows in place of its rarity's.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Rank {
    pub(crate) name: String,
    pub(crate) colour: Color,
}

/// The longest rank name, as the hub checks.
const RANK_MAX: usize = 24;

/// The rank a card file's `rank` and `rank_colour` (`#RRGGBB`) make, or why not.
fn rank(name: &str, colour: &str) -> Result<Rank, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > RANK_MAX {
        return Err(format!("rank must be 1 to {RANK_MAX} characters"));
    }
    let value = colour
        .strip_prefix('#')
        .filter(|hex| hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        .ok_or_else(|| format!("rank_colour {colour:?} is not #RRGGBB"))?;
    let channel = |shift: u32| ((value >> shift) & 0xff) as f32 / 255.0;
    Ok(Rank {
        name: name.to_owned(),
        colour: Color::new(channel(16), channel(8), channel(0), 1.0),
    })
}

impl Face {
    /// How it reads and is coloured: its rank's, else its rarity's ([`super::Tone`]).
    pub(crate) fn tone(&self) -> super::Tone<'_> {
        match &self.rank {
            Some(rank) => super::Tone {
                name: &rank.name,
                colour: rank.colour,
                mark: None,
            },
            None => self.rarity.tone(),
        }
    }

    /// What it brings, as its hover card and the Cards tab say it ("Brings the Sun blade
    /// while you hold it."), when it brings an unlockable this client knows.
    pub(crate) fn brings(&self) -> Option<String> {
        let unlockable = crate::unlockables::find(self.grants.as_deref()?)?;
        Some(format!("Brings the {} while you hold it.", unlockable.name))
    }

    /// What its Solar copies drop, as its hover card and the Cards tab say it ("Its Solar
    /// copies also drop the Sun blade."), when it is a blade card of an unlockable this
    /// client knows.
    pub(crate) fn solar_drop(&self) -> Option<String> {
        let unlockable = crate::unlockables::find(self.blade.as_deref()?)?;
        Some(format!(
            "Its Solar copies also drop the {}.",
            unlockable.name
        ))
    }

    /// What any copy of a blade card gives when crafting, as its hover card says it ("Any
    /// copy crafts the Sun blade for less."; 21/10/2026), for a shader this client knows.
    pub(crate) fn crafts_for_less(&self) -> Option<String> {
        let unlockable = crate::unlockables::find(self.blade.as_deref()?)?;
        Some(format!("Any copy crafts the {} for less.", unlockable.name))
    }

    /// What its copies bring ([`PERKS`], in that order; 21/10/2026, Sol's request: a mark
    /// on the card for each), as the tiles, the close-up and the hover card show them.
    pub(crate) fn perks(&self) -> Vec<Perk> {
        PERKS
            .iter()
            .filter_map(|(mark, words)| {
                Some(Perk {
                    mark: *mark,
                    words: words(self)?,
                })
            })
            .collect()
    }
}

/// A perk of a card: its SJK Symbols mark and its words ("Brings the Sun blade while you
/// hold it.").
pub(crate) struct Perk {
    pub(crate) mark: char,
    pub(crate) words: String,
}

/// Every perk a card can have and its mark, in the order they show: a new perk is a row
/// here. A lightsaber brings a blade while held, the Solar finish's sun drops one, the
/// smith's hammer crafts one for less.
const PERKS: [(char, fn(&Face) -> Option<String>); 3] = [
    (crate::symbols::SABER, Face::brings),
    (crate::symbols::FINISH[2], Face::solar_drop),
    (crate::symbols::SMITH, Face::crafts_for_less),
];

/// A card file as written.
#[derive(Deserialize)]
struct File {
    name: String,
    set: String,
    set_name: String,
    number: u16,
    kind: String,
    rarity: String,
    #[serde(default)]
    lore: String,
    art: String,
    #[serde(default)]
    artist: String,
    #[serde(default)]
    grants: Option<String>,
    #[serde(default)]
    blade: Option<String>,
    #[serde(default)]
    scene: Option<String>,
    #[serde(default)]
    rank: Option<String>,
    #[serde(default)]
    rank_colour: Option<String>,
}

/// The card file `text` of card `id`, read.
pub(crate) fn parse(id: &str, text: &str) -> Result<Face, String> {
    let file: File = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let within = |what: &str, value: &str, max: usize| {
        if value.trim().is_empty() || value.chars().count() > max {
            Err(format!("{what} must be 1 to {max} characters"))
        } else {
            Ok(())
        }
    };
    within("name", &file.name, NAME_MAX)?;
    within("set_name", &file.set_name, NAME_MAX)?;
    if file.lore.chars().count() > LORE_MAX {
        return Err(format!("lore is longer than {LORE_MAX} characters"));
    }
    // 0 is allowed: SJK Alpha's card 000 is Sol's (19/10/2026).
    if file.number > 999 {
        return Err("number must be 0 to 999".to_owned());
    }
    let kind = Kind::from_id(&file.kind).ok_or_else(|| format!("unknown kind {}", file.kind))?;
    let rarity =
        Rarity::from_id(&file.rarity).ok_or_else(|| format!("unknown rarity {}", file.rarity))?;
    // A scene this client cannot read leaves the card its pack's picture.
    let scene = file
        .scene
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
        .filter(|text| match super::scene::parse(text) {
            Ok(_) => true,
            Err(why) => {
                crate::log::progress(format_args!("cards: {id}: scene left out: {why}"));
                false
            }
        });
    // A rank comes with its colour; a bad pair leaves the card its rarity's look.
    let rank = match (file.rank, file.rank_colour) {
        (None, None) => None,
        (Some(name), Some(colour)) => rank(&name, &colour)
            .inspect_err(|why| {
                crate::log::progress(format_args!("cards: {id}: rank left out: {why}"));
            })
            .ok(),
        _ => {
            crate::log::progress(format_args!(
                "cards: {id}: rank left out: rank and rank_colour come together"
            ));
            None
        }
    };
    Ok(Face {
        id: id.to_owned(),
        name: file.name,
        set: file.set,
        set_name: file.set_name,
        number: file.number,
        kind,
        rarity,
        lore: file.lore,
        artist: if file.artist.trim().is_empty() {
            "Sol".to_owned()
        } else {
            file.artist
        },
        art: file.art,
        grants: file.grants.filter(|id| !id.trim().is_empty()),
        blade: file.blade.filter(|id| !id.trim().is_empty()),
        scene,
        rank,
    })
}

/// Every card face in `vfs`, by set and number; a file that cannot be read is named in
/// the log and left out.
pub(crate) fn load(vfs: &VirtualFileSystem) -> Vec<Face> {
    let mut faces = Vec::new();
    for listed in vfs.list_files(FOLDER, EXTENSION) {
        let path = format!("{FOLDER}/{listed}");
        let id = listed.trim_end_matches(EXTENSION);
        let read = vfs
            .read(&path)
            .map_err(|error| error.to_string())
            .and_then(|asset| asset.ok_or_else(|| "missing".to_owned()))
            .and_then(|asset| String::from_utf8(asset.bytes).map_err(|_| "not UTF-8".to_owned()))
            .and_then(|text| parse(id, &text));
        match read {
            Ok(face) => faces.push(face),
            Err(why) => crate::log::progress(format_args!("cards: {path}: {why}")),
        }
    }
    faces.sort_by(|a, b| (&a.set, a.number, &a.id).cmp(&(&b.set, b.number, &b.id)));
    faces.dedup_by(|a, b| a.id == b.id);
    faces.truncate(MAX_FACES);
    faces
}

/// The faces of the last mount, their pictures' files and the pictures decoded for the
/// renderer.
pub(crate) struct Faces {
    pub(crate) list: Vec<Face>,
    /// Each face's picture file as the pack holds it (`None` when it is missing).
    pictures: Vec<Option<Arc<[u8]>>>,
    /// The pictures asked for, being decoded, or decoded and not taken yet.
    art: Mutex<Art>,
    /// Counts mounts; the renderer uploads the pictures again when it changes.
    pub(crate) generation: u64,
}

/// What [`request`] and its worker share.
#[derive(Default)]
struct Art {
    /// Faces asked for and not decoded yet, the first first.
    asked: Vec<usize>,
    /// The face the worker is decoding.
    decoding: Option<usize>,
    /// Decoded pictures waiting for the renderer ([`Faces::take_art`]), at most
    /// [`READY_MAX`].
    ready: Vec<(usize, MipChain)>,
    /// Faces whose picture is missing or would not decode: never asked for again.
    failed: Vec<usize>,
    /// Whether a worker thread is decoding.
    working: bool,
}

impl Faces {
    fn new(list: Vec<Face>, pictures: Vec<Option<Arc<[u8]>>>, generation: u64) -> Self {
        Self {
            list,
            pictures,
            art: Mutex::new(Art::default()),
            generation,
        }
    }

    fn empty() -> Self {
        Self::new(Vec::new(), Vec::new(), 0)
    }

    fn lock_art(&self) -> MutexGuard<'_, Art> {
        self.art
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The face of card `id`, with its position in [`Self::list`].
    pub(crate) fn find(&self, id: &str) -> Option<(usize, &Face)> {
        self.list.iter().enumerate().find(|(_, face)| face.id == id)
    }

    /// Whether some face has rarity `rarity`: sealed copies of it can be revealed.
    pub(crate) fn has_rarity(&self, rarity: &Rarity) -> bool {
        self.list.iter().any(|face| face.rarity.id == rarity.id)
    }

    /// Faces `list` without pictures, for tests (not the current ones).
    #[cfg(test)]
    pub(crate) fn of_list(list: Vec<Face>) -> Self {
        let pictures = list.iter().map(|_| None).collect();
        Self::new(list, pictures, 1)
    }

    /// Let face `index`'s picture be asked for again after it failed: its scene was just
    /// drawn ([`super::scene`]).
    pub(crate) fn retry(&self, index: usize) {
        self.lock_art().failed.retain(|failed| *failed != index);
    }

    /// Face `index`'s picture file as its pack holds it, `None` when it is missing: what
    /// a card stuck as a tag is drawn from (`tag_render.rs`).
    pub(crate) fn picture(&self, index: usize) -> Option<Arc<[u8]>> {
        self.pictures.get(index).cloned().flatten()
    }

    /// The decoded picture of face `index`, taken for upload: `None` until a [`request`]
    /// for it has been decoded. The worker keeps no copy.
    pub(crate) fn take_art(&self, index: usize) -> Option<MipChain> {
        let mut art = self.lock_art();
        let at = art.ready.iter().position(|(ready, _)| *ready == index)?;
        Some(art.ready.remove(at).1)
    }
}

/// Ask for the pictures of faces `wanted`, the ones a screen draws that have no texture
/// yet, in that order: they replace the last asks, and decoded pictures no longer wanted
/// are dropped. A worker thread decodes them one by one ([`Faces::take_art`]); nothing
/// is decoded for a face no screen draws.
pub(crate) fn request(faces: &Arc<Faces>, wanted: &[usize]) {
    let mut art = faces.lock_art();
    art.ready.retain(|(index, _)| wanted.contains(index));
    art.asked.clear();
    for &index in wanted {
        let new = index < faces.list.len()
            && art.decoding != Some(index)
            && !art.failed.contains(&index)
            && !art.ready.iter().any(|(ready, _)| *ready == index)
            && !art.asked.contains(&index);
        if new {
            art.asked.push(index);
        }
    }
    if art.asked.is_empty() || art.working || art.ready.len() >= READY_MAX {
        return;
    }
    #[cfg(test)]
    if inline_for_shots() {
        drop(art);
        work(faces);
        return;
    }
    art.working = true;
    drop(art);
    let worker = Arc::clone(faces);
    let spawned = std::thread::Builder::new()
        .name("sjk-card-art".into())
        .spawn(move || work(&worker));
    if let Err(error) = spawned {
        faces.lock_art().working = false;
        crate::log::progress(format_args!(
            "warning: card pictures not decoded, drawing without them: {error}"
        ));
    }
}

/// Decode the asked pictures until none is left or [`READY_MAX`] wait for the renderer.
fn work(faces: &Faces) {
    loop {
        let index = {
            let mut art = faces.lock_art();
            if art.asked.is_empty() || art.ready.len() >= READY_MAX {
                art.working = false;
                return;
            }
            let index = art.asked.remove(0);
            art.decoding = Some(index);
            index
        };
        // The drawing of the card's scene when the client keeps one, else the pack's.
        let drawn =
            scene_picture(&faces.list[index]).map(|bytes| decode_guarded(&bytes, texture_edge()));
        let decoded = match drawn {
            Some(Ok(chain)) => Ok(chain),
            _ => match faces.pictures.get(index).and_then(Option::as_deref) {
                Some(bytes) => decode_guarded(bytes, texture_edge()),
                None => Err("missing".to_owned()),
            },
        };
        let mut art = faces.lock_art();
        art.decoding = None;
        match decoded {
            Ok(chain) => art.ready.push((index, chain)),
            Err(why) => {
                crate::log::progress(format_args!("cards: {}: {why}", faces.list[index].art));
                art.failed.push(index);
            }
        }
    }
}

/// The drawing of `face`'s scene when the client keeps one ([`super::scene`]). When it
/// does not, the drawing is asked for, and the pack's picture shows until it is ready.
fn scene_picture(face: &Face) -> Option<Vec<u8>> {
    let text = face.scene.as_deref()?;
    let path = super::scene::cache_path(&face.id, text)?;
    if let Ok(bytes) = std::fs::read(&path) {
        return Some(bytes);
    }
    let scene = super::scene::parse(text).ok()?;
    let colour = face.tone().colour;
    super::scene::want(super::scene::Job {
        id: face.id.clone(),
        scene,
        path,
        tint: [colour.r, colour.g, colour.b]
            .map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8),
    });
    None
}

/// [`decode`], a panic in it failing that picture alone: the worker goes on and clears
/// `working`, so later faces still load (after Slash's #206). The panic hook still
/// writes its crash report, which names the picture's decoder.
fn decode_guarded(bytes: &[u8], edge: u32) -> Result<MipChain, String> {
    std::panic::catch_unwind(|| {
        #[cfg(test)]
        if bytes == PANICS_FOR_TEST {
            panic!("a test picture the decoder trips on");
        }
        decode(bytes, edge)
    })
    .unwrap_or_else(|_| Err("the decoder panicked".to_owned()))
}

/// A picture file [`decode_guarded`] panics on, for tests.
#[cfg(test)]
const PANICS_FOR_TEST: &[u8] = b"sjk test: panic";

/// The edge pictures decode to at the current texture detail ([`TEXTURE_EDGE`]).
fn texture_edge() -> u32 {
    if crate::world_materials::filtering::mips::picmip() == 0 {
        TEXTURE_EDGE
    } else {
        TEXTURE_EDGE / 2
    }
}

/// Whether pictures decode on the asking thread rather than a worker, for world shots.
#[cfg(test)]
static INLINE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Decode pictures where they are asked for from now on, and upload every one a frame
/// asks for at once, so a face a shot draws in one frame shows in the next.
#[cfg(test)]
pub(crate) fn decode_inline_for_shots() {
    INLINE.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Whether [`decode_inline_for_shots`] was called.
#[cfg(test)]
pub(crate) fn inline_for_shots() -> bool {
    INLINE.load(std::sync::atomic::Ordering::Relaxed)
}

static FACES: Mutex<Option<Arc<Faces>>> = Mutex::new(None);

fn lock() -> MutexGuard<'static, Option<Arc<Faces>>> {
    FACES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The faces of the last mount (none before one).
pub(crate) fn current() -> Arc<Faces> {
    lock().clone().unwrap_or_else(|| Arc::new(Faces::empty()))
}

/// Make `list` the current faces, face `index`'s picture the current face
/// `pictures(index)`'s file: a made-up catalogue over the packs' pictures, for a world
/// shot.
#[cfg(test)]
pub(crate) fn install_for_shot(list: Vec<Face>, pictures: impl Fn(usize) -> usize) {
    let before = current();
    let files = (0..list.len())
        .map(|index| before.pictures.get(pictures(index)).cloned().flatten())
        .collect();
    *lock() = Some(Arc::new(Faces::new(list, files, before.generation + 1)));
}

/// Make `list` the current faces for a unit test, without pictures, until the returned
/// guard goes; tests that install faces wait for one another.
#[cfg(test)]
pub(crate) fn install_for_test(list: Vec<Face>) -> impl Drop {
    static TURN: Mutex<()> = Mutex::new(());
    struct Restore {
        before: Option<Arc<Faces>>,
        _turn: MutexGuard<'static, ()>,
    }
    impl Drop for Restore {
        fn drop(&mut self) {
            *lock() = self.before.take();
        }
    }
    let turn = TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let before = lock().clone();
    *lock() = Some(Arc::new(Faces::of_list(list)));
    Restore {
        before,
        _turn: turn,
    }
}

/// The name of card `id` as players read it: its face's name, or its id when the packs
/// have no face for it.
pub(crate) fn name_of(id: &str) -> String {
    current()
        .find(id)
        .map_or_else(|| id.to_owned(), |(_, face)| face.name.clone())
}

/// The texture of face `index`'s picture.
pub(crate) fn texture(index: usize) -> TextureId {
    TextureId(TEXTURE_BASE + index.min(MAX_FACES - 1) as u32)
}

/// The face position a texture id stands for, if it is a card picture's.
pub(crate) fn index_of(texture: TextureId) -> Option<usize> {
    let index = texture.0.checked_sub(TEXTURE_BASE)? as usize;
    (index < MAX_FACES).then_some(index)
}

/// Read the faces of the packs `vfs` (just mounted, under `generation`) and their
/// pictures' files, and make them the current ones; a picture is decoded when a screen
/// draws its face ([`request`]).
pub(crate) fn install(vfs: &VirtualFileSystem, generation: u64) {
    let list = load(vfs);
    let pictures = list
        .iter()
        .map(|face| {
            let asset = vfs.read(&face.art).ok().flatten();
            asset.map(|asset| Arc::from(asset.bytes))
        })
        .collect();
    let faces = Arc::new(Faces::new(list, pictures, generation));
    if !faces.list.is_empty() {
        crate::log::progress(format_args!(
            "SJK packs: card faces: {}",
            faces
                .list
                .iter()
                .map(|face| face.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    *lock() = Some(faces);
}

/// A picture's bytes as the square mip chain the renderer uploads: cut to the window's
/// 6:5 about its middle when it is not 6:5, then squeezed to `edge` square.
pub(crate) fn decode(bytes: &[u8], edge: u32) -> Result<MipChain, String> {
    let image = image::load_from_memory(bytes)
        .map_err(|error| error.to_string())?
        .into_rgba8();
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return Err("the picture is empty".to_owned());
    }
    let wide = width as f32 / height as f32;
    let (cut_w, cut_h) = if wide > ART_ASPECT {
        ((height as f32 * ART_ASPECT).round() as u32, height)
    } else {
        (width, (width as f32 / ART_ASPECT).round() as u32)
    };
    let cut = image::imageops::crop_imm(
        &image,
        (width - cut_w.min(width)) / 2,
        (height - cut_h.min(height)) / 2,
        cut_w.min(width),
        cut_h.min(height),
    )
    .to_image();
    let square = image::imageops::resize(&cut, edge, edge, image::imageops::FilterType::Triangle);
    Ok(MipChain::new(square))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = r#"{
  "name": "Toxiee and Sol on JoF Memorial",
  "set": "sjk_alpha",
  "set_name": "SJK Alpha",
  "number": 10,
  "kind": "meme",
  "rarity": "legendary",
  "lore": "Tox was simply chilling.",
  "art": "cards/sjk_alpha_010.jpg",
  "artist": "Sol"
}
"#;

    #[test]
    fn every_kind_reads_back_from_its_id() {
        for kind in Kind::ALL {
            assert_eq!(Kind::from_id(kind.id()), Some(kind));
        }
        assert_eq!(Kind::from_id("clan").map(Kind::name), Some("Clan"));
    }

    #[test]
    fn a_card_file_reads_and_bad_ones_are_refused() {
        let face = parse("sjk_alpha_010", FILE).unwrap();
        assert_eq!(face.name, "Toxiee and Sol on JoF Memorial");
        assert_eq!((face.set.as_str(), face.number), ("sjk_alpha", 10));
        assert_eq!(face.kind, Kind::Meme);
        assert_eq!(face.rarity.id, "legendary");
        assert_eq!(face.art, "cards/sjk_alpha_010.jpg");
        let bishop = FILE
            .replace("\"meme\"", "\"credits\"")
            .replace("\"legendary\"", "\"mythical\"");
        let bishop = parse("sjk_alpha_001", &bishop).unwrap();
        assert_eq!(
            (bishop.kind.name(), bishop.rarity.name),
            ("Credits", "Mythical")
        );
        assert_eq!(Kind::Meme.name(), "Meme");
        for (from, to) in [
            ("\"legendary\"", "\"mythic\""),
            ("\"meme\"", "\"memes\""),
            ("\"number\": 10", "\"number\": 1000"),
            ("Toxiee and Sol on JoF Memorial", ""),
        ] {
            assert!(parse("x", &FILE.replace(from, to)).is_err(), "{to}");
        }
        // Card 000 is a card like any other.
        let zero = FILE.replace("\"number\": 10", "\"number\": 0");
        assert_eq!(parse("x", &zero).unwrap().number, 0);
        // A card that brings a blade says which; one this client does not know brings
        // nothing it can name.
        assert_eq!(face.grants, None);
        let sun = FILE.replace("\"artist\"", "\"grants\": \"saber_sun\", \"artist\"");
        let sun = parse("sjk_alpha_000", &sun).unwrap();
        assert_eq!(sun.grants.as_deref(), Some("saber_sun"));
        assert_eq!(
            sun.brings().as_deref(),
            Some("Brings the Sun blade while you hold it.")
        );
        let later = FILE.replace("\"artist\"", "\"grants\": \"saber_later\", \"artist\"");
        assert_eq!(parse("x", &later).unwrap().brings(), None);
        assert_eq!(face.brings(), None);
        // A blade card names its shader; its Solar copies drop it.
        let blade = FILE
            .replace("\"meme\"", "\"blade\"")
            .replace("\"artist\"", "\"blade\": \"saber_sun\", \"artist\"");
        let blade = parse("sjk_blades_001", &blade).unwrap();
        assert_eq!((blade.kind, blade.kind.name()), (Kind::Blade, "Blade"));
        assert_eq!(blade.blade.as_deref(), Some("saber_sun"));
        assert_eq!(
            blade.solar_drop().as_deref(),
            Some("Its Solar copies also drop the Sun blade.")
        );
        let blank = FILE.replace("\"artist\"", "\"blade\": \" \", \"artist\"");
        assert_eq!(parse("x", &blank).unwrap().blade, None);
        let later = FILE.replace("\"artist\"", "\"blade\": \"saber_later\", \"artist\"");
        assert_eq!(parse("x", &later).unwrap().solar_drop(), None);
        assert_eq!(
            blade.crafts_for_less().as_deref(),
            Some("Any copy crafts the Sun blade for less.")
        );
        assert_eq!(face.crafts_for_less(), None);
        assert_eq!(face.solar_drop(), None);
        // Its perks, a mark each, in the table's order; a plain card has none.
        let marks: Vec<char> = blade.perks().iter().map(|perk| perk.mark).collect();
        assert_eq!(
            marks,
            [crate::symbols::FINISH[2], crate::symbols::SMITH],
            "the Solar drop, then the craft"
        );
        assert!(face.perks().is_empty());
        assert_eq!(sun.perks()[0].mark, crate::symbols::SABER);
        // A field the client does not know yet is read past.
        let later = FILE.replace("\"artist\"", "\"frame\": \"gold\", \"artist\"");
        assert!(parse("x", &later).is_ok());
    }

    #[test]
    fn a_rank_takes_the_rarity_s_place_and_a_bad_one_is_left_out() {
        let plain = parse("sjk_alpha_010", FILE).unwrap();
        assert_eq!(plain.rank, None);
        let tone = plain.tone();
        assert_eq!((tone.name, tone.mark), ("Legendary", Some('\u{E003}')));
        assert_eq!(tone.to_string(), "\u{E003} Legendary");
        let ranked = |rank: &str| {
            FILE.replace("\"legendary\"", "\"mythical\"")
                .replace("\"artist\"", &format!("{rank}, \"artist\""))
        };
        let leader = parse(
            "jof_001",
            &ranked(r##""rank": " Leader ", "rank_colour": "#22D3ee""##),
        )
        .unwrap();
        assert_eq!(
            leader.rarity.id, "mythical",
            "the rarity stays, for the odds"
        );
        let rank = leader.rank.as_ref().unwrap();
        assert_eq!(rank.name, "Leader");
        let [r, g, b] = [rank.colour.r, rank.colour.g, rank.colour.b]
            .map(|channel| (channel * 255.0).round() as u8);
        assert_eq!([r, g, b], [0x22, 0xD3, 0xEE]);
        let tone = leader.tone();
        assert_eq!(
            (tone.name, tone.mark, tone.colour),
            ("Leader", None, rank.colour)
        );
        assert_eq!(tone.to_string(), "Leader", "no rarity mark, no rarity name");
        // A rank without its colour, a bad colour or a name too long: the card still
        // reads, with its rarity's look.
        for bad in [
            r#""rank": "Leader""#,
            r##""rank_colour": "#22D3EE""##,
            r#""rank": "Leader", "rank_colour": "22D3EE""#,
            r##""rank": "Leader", "rank_colour": "#22D3EG""##,
            r##""rank": "Leader", "rank_colour": "#22D3E""##,
            r##""rank": " ", "rank_colour": "#22D3EE""##,
            r##""rank": "A rank name far too long to read", "rank_colour": "#22D3EE""##,
        ] {
            let face = parse("x", &ranked(bad)).unwrap();
            assert_eq!(face.rank, None, "{bad}");
            assert_eq!(face.tone().name, "Mythical", "{bad}");
        }
    }

    #[test]
    fn a_picture_is_squeezed_into_a_square_chain() {
        let mut png = Vec::new();
        let picture = image::RgbaImage::from_pixel(1_020, 850, image::Rgba([10, 20, 30, 255]));
        image::DynamicImage::ImageRgba8(picture)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let chain = decode(&png, TEXTURE_EDGE).unwrap();
        assert_eq!(chain.size, TEXTURE_EDGE);
        assert_eq!(chain.levels.len(), 11, "1024 down to 1");
        assert_eq!(decode(&png, TEXTURE_EDGE / 2).unwrap().levels.len(), 10);
        assert!(decode(b"not a picture", TEXTURE_EDGE).is_err());
    }

    #[test]
    fn only_the_faces_asked_for_are_decoded() {
        let mut png = Vec::new();
        let picture = image::RgbaImage::from_pixel(1_020, 850, image::Rgba([10, 20, 30, 255]));
        image::DynamicImage::ImageRgba8(picture)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let png: Arc<[u8]> = Arc::from(png);
        let list = (0..3)
            .map(|index| parse(&format!("sjk_alpha_{index:03}"), FILE).unwrap())
            .collect();
        let faces = Arc::new(Faces::new(
            list,
            vec![Some(Arc::clone(&png)), Some(png), None],
            1,
        ));
        let taken = |index: usize| {
            for _ in 0..400 {
                if let Some(chain) = faces.take_art(index) {
                    return Some(chain);
                }
                request(&faces, &[index]);
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            None
        };
        // Face 1 is asked for and decoded; face 0, never asked for, is not.
        request(&faces, &[1]);
        assert!(taken(1).is_some());
        assert!(faces.take_art(0).is_none());
        assert!(faces.take_art(1).is_none(), "the worker keeps no copy");
        // A face without a picture is tried once, then never asked for again.
        request(&faces, &[2]);
        for _ in 0..400 {
            if faces.lock_art().failed.contains(&2) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(faces.lock_art().failed.contains(&2));
        request(&faces, &[2]);
        assert!(faces.lock_art().asked.is_empty());
        // A decoded picture no screen wants any more is dropped.
        request(&faces, &[0]);
        for _ in 0..400 {
            if !faces.lock_art().ready.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        request(&faces, &[]);
        assert!(faces.lock_art().ready.is_empty());
    }

    #[test]
    fn a_picture_the_decoder_panics_on_fails_alone() {
        let mut png = Vec::new();
        let picture = image::RgbaImage::from_pixel(60, 50, image::Rgba([10, 20, 30, 255]));
        image::DynamicImage::ImageRgba8(picture)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let list = (0..2)
            .map(|index| parse(&format!("sjk_alpha_{index:03}"), FILE).unwrap())
            .collect();
        let faces = Arc::new(Faces::new(
            list,
            vec![Some(Arc::from(PANICS_FOR_TEST)), Some(Arc::from(png))],
            1,
        ));
        // Face 0 panics in the decoder, face 1 behind it still decodes.
        request(&faces, &[0, 1]);
        let mut decoded = None;
        for _ in 0..400 {
            decoded = faces.take_art(1);
            if decoded.is_some() {
                break;
            }
            request(&faces, &[0, 1]);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(decoded.is_some(), "the worker went on after the panic");
        assert!(faces.lock_art().failed.contains(&0));
    }

    #[test]
    fn texture_ids_round_trip() {
        assert_eq!(index_of(texture(3)), Some(3));
        assert_eq!(index_of(TextureId(TEXTURE_BASE - 1)), None);
        assert_eq!(index_of(TextureId(TEXTURE_BASE + MAX_FACES as u32)), None);
    }
}
