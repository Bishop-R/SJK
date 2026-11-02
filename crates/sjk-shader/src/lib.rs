//! Parsing and resolution for Jedi Academy's Quake 3 shader scripts.

use sjk_vfs::{VfsError, VirtualFileSystem, VirtualPath};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;

mod catalog;
pub use catalog::DefinitionSource;
mod fog;
pub use fog::{FogPass, infer_sort};
mod parse;
mod recovery;
pub use parse::parse_shader_script;
use parse::*;
mod deforms;
mod material;
mod stage_colour;
mod stage_parse;
pub use deforms::Deform;
pub use material::{
    DEFAULT_NORMAL_SCALE, INITIAL_SPECULAR_SCALE, SPEC_GLOSS_SCALE, SpecularLayout, StageMaterial,
    packed_specular_scale,
};
pub use stage_colour::{AlphaGen, RgbGen, StageColour};
mod surface_sprites;
use stage_parse::parse_stage;
pub use surface_sprites::{SpriteFacing, SpriteKind, SurfaceSprites};

#[derive(Clone, Debug, PartialEq)]
pub struct ShaderDefinition {
    pub name: String,
    /// Authored surface-emission hint; prevents ambient-only effects darkening light sources.
    pub emits_light: bool,
    /// `q3map_surfacelight` value in q3map light units; zero when the surface emits nothing.
    pub surface_light: f32,
    /// Optional compiler emission image (`q3map_lightimage`), separate from diffuse paint.
    pub light_image: Option<String>,
    /// Ordered runtime geometry deformations, independent of texture stages.
    pub deforms: Vec<Deform>,
    /// Portal fade/culling distance, also reused as an authored flare radius.
    pub portal_range: Option<f32>,
    pub editor_image: Option<String>,
    pub stage_images: Vec<String>,
    pub diffuse_images: Vec<String>,
    pub emissive_images: Vec<String>,
    pub stages: Vec<ShaderStage>,
    /// Explicit Q3 draw sort, or `None` when the renderer must infer it.
    pub sort: Option<f32>,
    /// Q3 face culling mode. The default is front-sided rendering.
    pub cull: ShaderCull,
    /// Optional Q3 sky-box and cloud-layer parameters.
    pub sky: Option<SkyParms>,
    /// Optional map-compiler sun declaration retained for renderer consumers.
    pub sun: Option<SunParms>,
    /// Optional fog-volume parameters; a stage-less shader carrying them is a
    /// fog-only shader drawn purely by the fog pass.
    pub fog: Option<FogParms>,
    /// `surfaceparm fog`: the material carries fog contents.
    pub fog_contents: bool,
    /// `noglfog` suppresses the volume pass.
    pub no_gl_fog: bool,
    /// `polygonOffset`: the surface is laid onto another (painted markings, decals) and
    /// must win the depth test against it instead of fighting with it.
    pub polygon_offset: bool,
    /// `nopicmip` or `nomipmaps` seen so far: the stages parsed after it keep their
    /// images' full size whatever the texture detail ([`ShaderStage::no_picmip`]).
    pub no_picmip: bool,
}

/// Generic Quake 3 `fogParms` values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FogParms {
    /// Fog colour the volume converges to.
    pub color: [f32; 3],
    /// Distance in map units at which the fog becomes fully opaque.
    pub depth_for_opaque: f32,
}

/// Generic Quake 3 `skyParms` values.
#[derive(Clone, Debug, PartialEq)]
pub struct SkyParms {
    /// Prefix used to resolve the six outer-box images, or no box for `-`.
    pub outer_box: Option<String>,
    /// Height of the projected cloud layer. Zero in scripts becomes 512.
    pub cloud_height: f32,
    /// Inner-box prefix, retained even though rd-vanilla does not render it.
    pub inner_box: Option<String>,
}

/// Generic Quake 3 `sun`/`q3map_sun` declaration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SunParms {
    /// RGB light colour from the shader declaration.
    pub color: [f32; 3],
    /// Sun-light intensity used by the map compiler.
    pub intensity: f32,
    /// Unit direction derived from compass and elevation angles.
    pub direction: [f32; 3],
}

/// Generic Quake 3 shader culling semantics.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ShaderCull {
    #[default]
    Front,
    Back,
    TwoSided,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShaderStage {
    /// Raven geometry generated over this stage's source triangles.
    pub surface_sprites: Option<SurfaceSprites>,
    /// Per-stage distance for `alphaGen portal` (stock default 256 units).
    pub portal_range: Option<f32>,
    pub images: Vec<String>,
    /// `videoMap <cinematic>`: a RoQ video drawn as the stage's image, looping
    /// (rd-vanilla `tr_shader.cpp` ParseStage, `CIN_PlayCinematic(..., CIN_loop |
    /// CIN_silent | CIN_shader)`). The path is as written; `video/` and `.roq` are
    /// the defaults for a bare name.
    pub video: Option<String>,
    pub animation_frequency: Option<f32>,
    pub one_shot: bool,
    pub clamp: bool,
    pub blend: StageBlend,
    pub glow: bool,
    pub alpha_function: Option<String>,
    pub rgb_generator: Option<String>,
    pub alpha_generator: Option<String>,
    /// rd-vanilla's resolved generators for this stage as parsed; see [`StageColour`].
    /// Code that rewrites the generator strings after parsing does not update it.
    pub resolved_colour: StageColour,
    pub rgb_wave: Option<WaveForm>,
    pub alpha_wave: Option<WaveForm>,
    pub texture_modifications: Vec<TextureModification>,
    pub rgb_constant: Option<[f32; 3]>,
    pub alpha_constant: Option<f32>,
    pub texture_generator: TextureGenerator,
    /// The s and t axes of [`TextureGenerator::Vector`]; zero otherwise.
    pub texture_vectors: [[f32; 3]; 2],
    pub depth_write: bool,
    pub depth_function: DepthFunction,
    /// Optional rend2 material-map keywords; ignored unless material maps are enabled.
    pub material: StageMaterial,
    /// The shader said `nopicmip` or `nomipmaps` before this stage: its images, and
    /// rend2's material maps for it, keep their full size whatever `r_picmip`
    /// (rd-vanilla `ParseStage` passes `!shader.noPicMip` to `R_FindImageFile` as each
    /// stage is parsed; `nomipmaps` sets it too). Sky boxes take the texture detail
    /// regardless, as in `ParseSkyParms`.
    pub no_picmip: bool,
}

/// Texture-coordinate source selected by a Q3 stage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextureGenerator {
    #[default]
    Base,
    Lightmap,
    Environment,
    /// `tcGen vector ( sx sy sz ) ( tx ty tz )`: s and t are the vertex position dotted
    /// with [`ShaderStage::texture_vectors`] (rd-vanilla `TCGEN_VECTOR`), a world-space
    /// projection such as caustics laid over a whole room.
    Vector,
}

/// Depth comparison selected by a Q3 stage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DepthFunction {
    #[default]
    LessEqual,
    Equal,
    Disabled,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WaveForm {
    pub function: String,
    pub base: f32,
    pub amplitude: f32,
    pub phase: f32,
    pub frequency: f32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StageBlend {
    Replace,
    Add,
    Filter,
    Alpha,
    Custom { source: String, destination: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextureModification {
    pub kind: String,
    pub arguments: Vec<f32>,
    /// `stretch` carries a complete waveform; other tcMods leave this empty.
    pub wave: Option<WaveForm>,
}

impl ShaderDefinition {
    /// Whether the renderer must draw this shader's surfaces at all. A defined
    /// shader with no stages and no sky has no colour pass in rd-vanilla
    /// (`tr_shader.cpp:3184-3187` sorts it `SS_FOG`): fog-only volumes and
    /// stage-less placeholders vanish instead of showing a checkerboard.
    pub fn has_color_pass(&self) -> bool {
        !self.stages.is_empty() || self.sky.is_some()
    }

    pub fn primary_image(&self) -> Option<&str> {
        self.diffuse_images
            .iter()
            .find(|image| is_concrete_image(image))
            .map(String::as_str)
            .or_else(|| {
                self.stage_images
                    .iter()
                    .find(|image| is_concrete_image(image))
                    .map(String::as_str)
            })
            .or(self.editor_image.as_deref())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShaderCatalog {
    definitions: HashMap<String, ShaderDefinition>,
}

impl ShaderCatalog {
    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    /// Every defined shader's name as [`ShaderCatalog::get`] keys it (lower case, `/`
    /// separators), in no particular order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.definitions.keys().map(String::as_str)
    }

    pub fn get(&self, name: &str) -> Option<&ShaderDefinition> {
        let normalized = normalize_name(name);
        self.definitions.get(&normalized).or_else(|| {
            let stem = strip_extension(&normalized);
            (stem.len() < normalized.len())
                .then(|| self.definitions.get(stem))
                .flatten()
        })
    }

    pub fn resolve_image(
        &self,
        vfs: &VirtualFileSystem,
        material_name: &str,
    ) -> Result<Option<VirtualPath>, ShaderError> {
        let mut candidates = Vec::new();
        if let Some(definition) = self.get(material_name) {
            candidates.extend(
                definition
                    .diffuse_images
                    .iter()
                    .filter(|image| is_concrete_image(image))
                    .map(String::as_str),
            );
            candidates.extend(
                definition
                    .stage_images
                    .iter()
                    .filter(|image| is_concrete_image(image))
                    .map(String::as_str),
            );
            if let Some(editor_image) = definition.editor_image.as_deref() {
                candidates.push(editor_image);
            }
        }
        candidates.push(material_name);

        for candidate in candidates {
            let candidate = normalize_name(candidate);
            let has_known_extension = [".tga", ".jpg", ".jpeg", ".png"]
                .iter()
                .any(|extension| candidate.ends_with(extension));
            if has_known_extension {
                if vfs.contains(&candidate)? {
                    return Ok(Some(VirtualPath::new(&candidate).map_err(VfsError::from)?));
                }
                // Retail scripts often name a .tga while the packaged image
                // is a .jpg. The original loader tries sibling extensions, in
                // its loader order (`R_LoadImage`, rd-common `tr_image_load.cpp`).
                let stem = candidate
                    .rsplit_once('.')
                    .map_or(candidate.as_str(), |(stem, _)| stem);
                for extension in IMAGE_EXTENSIONS {
                    let path = format!("{stem}{extension}");
                    if vfs.contains(&path)? {
                        return Ok(Some(VirtualPath::new(&path).map_err(VfsError::from)?));
                    }
                }
                continue;
            }
            for extension in IMAGE_EXTENSIONS {
                let path = format!("{candidate}{extension}");
                if vfs.contains(&path)? {
                    return Ok(Some(VirtualPath::new(&path).map_err(VfsError::from)?));
                }
            }
            // Any other extension is dropped, as `COM_StripExtension` does in
            // `R_FindShader` and `R_LoadImage`: a model naming `tex.jpgv` (a typo in
            // JoF's HD E11) loads `tex.png` there.
            let stem = strip_extension(&candidate);
            if stem.len() < candidate.len() {
                for extension in IMAGE_EXTENSIONS {
                    let path = format!("{stem}{extension}");
                    if vfs.contains(&path)? {
                        return Ok(Some(VirtualPath::new(&path).map_err(VfsError::from)?));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Resolve one concrete stage image without following a shader's diffuse
    /// fallback list. Q3 `map`/`animMap` stages use this exact-name lookup.
    pub fn resolve_stage_image(
        &self,
        vfs: &VirtualFileSystem,
        image_name: &str,
    ) -> Result<Option<VirtualPath>, ShaderError> {
        resolve_concrete_image(vfs, image_name)
    }

    pub fn resolve_emissive_image(
        &self,
        vfs: &VirtualFileSystem,
        material_name: &str,
    ) -> Result<Option<VirtualPath>, ShaderError> {
        let Some(definition) = self.get(material_name) else {
            return Ok(None);
        };
        for image in &definition.emissive_images {
            let candidate = normalize_name(image);
            let stem = candidate
                .rsplit_once('.')
                .map_or(candidate.as_str(), |(stem, _)| stem);
            for path in [
                candidate.clone(),
                format!("{stem}.jpg"),
                format!("{stem}.png"),
                format!("{stem}.tga"),
            ] {
                if vfs.contains(&path)? {
                    return Ok(Some(VirtualPath::new(&path).map_err(VfsError::from)?));
                }
            }
        }
        Ok(None)
    }
}

fn resolve_concrete_image(
    vfs: &VirtualFileSystem,
    image_name: &str,
) -> Result<Option<VirtualPath>, ShaderError> {
    let candidate = normalize_name(image_name);
    // `R_LoadImage` drops whatever extension the name has before trying its loaders.
    let stem = strip_extension(&candidate).to_owned();
    for path in [
        candidate,
        format!("{stem}.jpg"),
        format!("{stem}.png"),
        format!("{stem}.tga"),
    ] {
        if vfs.contains(&path)? {
            return Ok(Some(VirtualPath::new(&path).map_err(VfsError::from)?));
        }
    }
    Ok(None)
}

/// The order OpenJK and EternalJK try image types (`R_ImageLoader_Init` in
/// rd-common `tr_image_load.cpp`): JPEG, then PNG, then TGA. `R_LoadImage` still
/// tries the extension named first, so this order only applies when the named
/// file is missing or the name has no extension. A `.png` beside a base `.tga`
/// therefore wins for an extensionless name, not for a name that says `.tga`.
const IMAGE_EXTENSIONS: [&str; 3] = [".jpg", ".png", ".tga"];

/// The name without its extension, whatever it is: `COM_StripExtension` (q_shared.c)
/// cuts at the last dot that follows the last slash.
fn strip_extension(name: &str) -> &str {
    match name.rfind('.') {
        Some(dot) if name.rfind('/').is_none_or(|slash| slash < dot) => &name[..dot],
        _ => name,
    }
}

fn is_concrete_image(image: &str) -> bool {
    !image.starts_with('$') && image != "-"
}

/// A material or image name as the catalogue and the file system take it: forward
/// slashes, lower case, and no leading slash. Skins and shaders written by hand name
/// `/models/...` at times (JoF's Pillowtalk skin); the original game's file system
/// skips the slash (`FS_FOpenFileRead`), and a kept one made the whole map load fail
/// (Mae's report, siege_desert, 17/10/2026).
fn normalize_name(name: &str) -> String {
    name.replace('\\', "/")
        .trim_start_matches('/')
        .to_ascii_lowercase()
}

#[derive(Debug)]
pub enum ShaderError {
    /// A rejected material; the nested error retains its source and location.
    SkippedShader {
        name: String,
        error: Box<ShaderError>,
    },
    Vfs(VfsError),
    DisappearedAsset(VirtualPath),
    Syntax {
        source: VirtualPath,
        offset: usize,
        message: &'static str,
    },
}

impl fmt::Display for ShaderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SkippedShader { name, error } => {
                write!(formatter, "skipping shader {name}: {error}")
            }
            Self::Vfs(error) => error.fmt(formatter),
            Self::DisappearedAsset(path) => {
                write!(formatter, "listed shader asset {path} disappeared")
            }
            Self::Syntax {
                source,
                offset,
                message,
            } => write!(
                formatter,
                "shader syntax error in {source} at token/byte {offset}: {message}"
            ),
        }
    }
}

impl Error for ShaderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Vfs(error) => Some(error),
            _ => None,
        }
    }
}

impl From<VfsError> for ShaderError {
    fn from(value: VfsError) -> Self {
        Self::Vfs(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vfs(files: &[&str]) -> VirtualFileSystem {
        let mut vfs = VirtualFileSystem::new();
        vfs.mount_memory("test", files.iter().map(|path| (*path, vec![0_u8])))
            .expect("memory mount");
        vfs
    }

    fn resolved(vfs: &VirtualFileSystem, name: &str) -> Option<String> {
        ShaderCatalog::default()
            .resolve_image(vfs, name)
            .expect("lookup")
            .map(|path| path.as_str().to_owned())
    }

    #[test]
    fn extensionless_name_tries_jpg_then_png_then_tga() {
        let all = vfs(&["textures/a/b.tga", "textures/a/b.png", "textures/a/b.jpg"]);
        assert_eq!(
            resolved(&all, "textures/a/b").as_deref(),
            Some("textures/a/b.jpg")
        );
        let png_tga = vfs(&["textures/a/b.tga", "textures/a/b.png"]);
        assert_eq!(
            resolved(&png_tga, "textures/a/b").as_deref(),
            Some("textures/a/b.png")
        );
        let tga = vfs(&["textures/a/b.tga"]);
        assert_eq!(
            resolved(&tga, "textures/a/b").as_deref(),
            Some("textures/a/b.tga")
        );
    }

    #[test]
    fn a_leading_slash_is_skipped_instead_of_failing_the_lookup() {
        // JoF's Pillowtalk `model_default.skin`: `/models/players/Pillowtalk/legs_green.tga`.
        let files = vfs(&["models/players/pillowtalk/legs_green.tga"]);
        assert_eq!(
            resolved(&files, "/models/players/Pillowtalk/legs_green.tga").as_deref(),
            Some("models/players/pillowtalk/legs_green.tga")
        );
        assert_eq!(
            ShaderCatalog::default()
                .resolve_stage_image(&files, "\\models/players/pillowtalk/legs_green")
                .expect("lookup")
                .map(|path| path.as_str().to_owned())
                .as_deref(),
            Some("models/players/pillowtalk/legs_green.tga")
        );
    }

    #[test]
    fn named_extension_wins_when_the_file_exists() {
        // `R_LoadImage` tries the named extension first, so a base `.tga` beside
        // an HD `.png` is still used when the script names the `.tga`.
        let both = vfs(&["textures/a/b.tga", "textures/a/b.png"]);
        assert_eq!(
            resolved(&both, "textures/a/b.tga").as_deref(),
            Some("textures/a/b.tga")
        );
        assert_eq!(
            resolved(&both, "textures/a/b.png").as_deref(),
            Some("textures/a/b.png")
        );
    }

    #[test]
    fn an_unknown_extension_is_stripped_as_com_strip_extension_does() {
        // JoF's HD E11 (`blaster_w.glm`, surface `bl_low`) names `tex.jpgv`.
        let files = vfs(&["models/weapons2/blaster_r/tex.png"]);
        assert_eq!(
            resolved(&files, "models/weapons2/blaster_r/tex.jpgv").as_deref(),
            Some("models/weapons2/blaster_r/tex.png")
        );
        let stage = ShaderCatalog::default()
            .resolve_stage_image(&files, "models/weapons2/blaster_r/tex.jpgv")
            .expect("lookup")
            .map(|path| path.as_str().to_owned());
        assert_eq!(stage.as_deref(), Some("models/weapons2/blaster_r/tex.png"));
        // A dot in a folder name is not an extension.
        let folder = vfs(&["models/a.b/c.jpg"]);
        assert_eq!(
            resolved(&folder, "models/a.b/c").as_deref(),
            Some("models/a.b/c.jpg")
        );
        assert_eq!(strip_extension("models/a.b/c"), "models/a.b/c");
        assert_eq!(strip_extension("models/a/c.jpgv"), "models/a/c");
    }

    #[test]
    fn a_shader_is_found_through_any_extension() {
        let mut files = VirtualFileSystem::new();
        files
            .mount_memory(
                "test",
                [(
                    "shaders/test.shader",
                    b"models/a/tex\n{\n {\n map models/a/real.tga\n }\n}\n".to_vec(),
                )],
            )
            .expect("memory mount");
        let catalog = ShaderCatalog::load(&files).expect("catalog");
        assert!(catalog.get("models/a/tex.jpgv").is_some());
        assert!(catalog.get("models/a.b").is_none());
    }

    #[test]
    fn missing_named_extension_falls_back_in_loader_order() {
        let files = vfs(&["textures/a/b.tga", "textures/a/b.png"]);
        assert_eq!(
            resolved(&files, "textures/a/b.jpg").as_deref(),
            Some("textures/a/b.png")
        );
        let stage = ShaderCatalog::default()
            .resolve_stage_image(&files, "textures/a/b.jpeg")
            .expect("lookup")
            .map(|path| path.as_str().to_owned());
        assert_eq!(stage.as_deref(), Some("textures/a/b.png"));
    }
}
