//! Optional game fonts: Jedi Academy's own font faces, as bundled vector fonts.
//!
//! `ui_gameFont` swaps the bundled Inter for the game's own fonts on every
//! surface the retail game drew with them ([`RetailFont`]):
//!
//! - `ergoec`, the medium font (`FONT_MEDIUM`): menus (`assetGlobalDef` in
//!   `ui/main.menu` and `ui/ingame.menu`), the crosshair name, centre prints,
//!   the warmup text and match timer, enemy info, and scoreboard names and
//!   headings;
//! - `ocr_a`, the small font (`FONT_SMALL`; OpenJK `codemp` `cg_main.c`
//!   registers it as `qhSmallFont`): the chat box (`CG_ChatBox_DrawStrings`),
//!   weapon, Force and inventory selection names (`UI_SMALLFONT`) and
//!   scoreboard numbers;
//! - the console character set, drawn with the bundled vector font JetBrains
//!   Mono instead of retail's `charsgrid_med` bitmap ([`text::console_font`]):
//!   the console and its notify lines, what the cgame drew with
//!   `CG_DrawBigString`, `CG_DrawSmallString` or `CG_DrawStringExt` (FPS,
//!   snapshot, vote, team overlay, connection interrupted), and obituaries,
//!   which the cgame printed to the console.
//!
//! SJK draws no bitmap fonts, so the medium and small fonts are SJK Menu and
//! SJK Chat, bundled vector replacements for the retail `ergoec` and `ocr_a`
//! bitmaps that keep their layout ([`text::retail_font`]); they need no game
//! data. A font that fails to load leaves its surfaces on Inter.
//! Centre prints and the words in the world (over mystery boxes, the box hunt's
//! mark) are SJK's headline text, drawn in Fira Sans Condensed
//! ([`RetailFont::Headline`], [`text::HEADLINE`]) whatever `ui_gameFont` says:
//! close to `ergoec`, but a real typeface rather than a tracing of a bitmap.
//! `cg_classicHudFont` keeps its own scope, the status HUD's `arialnb`, whose
//! replacement is SJK HUD.
//!
//! The fonts load the first time the option is on and stay resident for the
//! world. A world installed while the option is on loads them on its install
//! worker ([`GameFonts::preload`]), so a map change does not rasterize the fonts
//! on the frame thread. Each font has its own vertex buffer and atlas bind
//! group. The medium and small fonts draw before the Inter text, as when every
//! surface shared one buffer; the console font draws after all other text so
//! the console stays on top ([`GameFonts::draw_console`]).

use crate::GpuState;
use crate::gpu_texture;
use crate::text::system_font::{self, Scope};
use crate::text::{self, MAX_TEXT_VERTICES, TextVertex, UiFont, retail_font};
use sjk_ui::{DrawList, TextId};
use std::sync::mpsc::{self, Receiver};

/// The cvar that turns the game fonts on.
pub(crate) const CVAR: &str = "ui_gameFont";
/// Smallest long side a font atlas is mipmapped down to: below it, tightly
/// packed neighbouring glyphs would bleed into each other.
const MIN_MIP_SIZE: u32 = 512;

/// How much larger than retail's `ergoec` line centre prints set the headline
/// font: its capitals fill 0.936 of a line, SJK Menu's (the `ergoec` layout) a
/// little more, so this keeps centre prints' capitals the size they were.
pub(crate) const HEADLINE_GROWTH: f32 = 1.07;

/// The retail font a text surface was drawn with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RetailFont {
    /// `FONT_MEDIUM`, `ergoec`.
    Medium,
    /// `FONT_SMALL`, `ocr_a`.
    Small,
    /// The console character set.
    Console,
    /// Not a retail font: SJK's headline face for centre prints and the words in
    /// the world, on whether `ui_gameFont` is or not.
    Headline,
    /// Not a retail font: a font installed on the PC the player chose for chat
    /// (`cg_chatFont`, [`text::system_font`]), once it is loaded.
    Chat,
}

impl RetailFont {
    const ALL: [Self; 5] = [
        Self::Medium,
        Self::Small,
        Self::Console,
        Self::Headline,
        Self::Chat,
    ];

    const fn index(self) -> usize {
        match self {
            Self::Medium => 0,
            Self::Small => 1,
            Self::Console => 2,
            Self::Headline => 3,
            Self::Chat => 4,
        }
    }
}

/// One loaded font with its per-frame vertices and GPU resources.
struct Layer {
    font: UiFont,
    vertices: Vec<TextVertex>,
    buffer: wgpu::Buffer,
    /// The font's atlas, kept to bind it again with the wide atlas ([`GameFonts::rebind`]).
    atlas: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    /// The atlas was uploaded as a distance field.
    distance_field: bool,
    count: u32,
}

/// Device resources the font layers are created with.
pub(crate) struct Device<'a> {
    pub(crate) device: &'a wgpu::Device,
    pub(crate) queue: &'a crate::frame_queue::FrameQueue,
    /// The text pipeline's atlas bind group layout.
    pub(crate) layout: &'a wgpu::BindGroupLayout,
    pub(crate) sampler: &'a wgpu::Sampler,
    /// The wide-glyph and emoji atlases every text bind group carries.
    pub(crate) wide: &'a text::wide_texture::WideTexture,
}

impl Layer {
    fn load(face: &retail_font::RetailFace, gpu: &Device<'_>) -> Option<Self> {
        match retail_font::load(face) {
            Ok(atlas) => Some(Self::upload(face.name, atlas.font, atlas.image, gpu)),
            Err(error) => {
                crate::log::progress(format_args!(
                    "warning: game font {} unavailable, keeping Inter: {error}",
                    face.name
                ));
                None
            }
        }
    }

    fn load_console(gpu: &Device<'_>) -> Option<Self> {
        match text::console_font::load() {
            Ok(atlas) => Some(Self::upload(
                text::console_font::NAME,
                atlas.font,
                atlas.image,
                gpu,
            )),
            Err(error) => {
                crate::log::progress(format_args!(
                    "warning: console font unavailable, keeping Inter: {error}"
                ));
                None
            }
        }
    }

    fn upload(name: &str, font: UiFont, image: image::GrayImage, gpu: &Device<'_>) -> Self {
        let started = std::time::Instant::now();
        let (width, height) = image.dimensions();
        // The same atlas in every world: a map change's next world takes the last one's.
        let key = crate::shared_textures::key(&[
            name.as_bytes(),
            &width.to_le_bytes(),
            &height.to_le_bytes(),
            image.as_raw(),
        ]);
        let (view, distance_field) = crate::shared_textures::share("game font", key, || {
            let (field, distance_field) = text::sdf::for_atlas(image);
            if distance_field {
                crate::log::progress(format_args!(
                    "game font {name}: {width}x{height} atlas, {}x{} distance field in {:.1} ms",
                    field.width(),
                    field.height(),
                    started.elapsed().as_secs_f64() * 1_000.0
                ));
            } else {
                crate::log::progress(format_args!(
                    "game font {name}: {width}x{height} atlas, drawn from coverage"
                ));
            }
            let view = gpu_texture::create_r8_texture_mipmapped(
                gpu.device,
                gpu.queue,
                "SJK game font atlas",
                &field,
                mip_levels(field.width(), field.height()),
            );
            (view, distance_field)
        });
        let bind_group = text::wide_texture::bind_group(
            gpu.device,
            "SJK game font bind group",
            gpu.layout,
            &view,
            gpu.sampler,
            gpu.wide,
        );
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SJK game font text vertices"),
            size: (MAX_TEXT_VERTICES * std::mem::size_of::<TextVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            font,
            vertices: Vec::with_capacity(4_096),
            buffer,
            atlas: view,
            bind_group,
            distance_field,
            count: 0,
        }
    }

    fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        pipeline: &wgpu::RenderPipeline,
        sdf_pipeline: &wgpu::RenderPipeline,
    ) {
        if self.count != 0 {
            pass.set_pipeline(if self.distance_field {
                sdf_pipeline
            } else {
                pipeline
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.buffer.slice(..));
            pass.draw(0..self.count, 0..1);
        }
    }
}

/// Mip levels for an atlas or its distance field of `width` x `height`: one per
/// halving down to [`MIN_MIP_SIZE`], so large atlases minify cleanly to UI
/// text sizes.
pub(crate) fn mip_levels(width: u32, height: u32) -> u32 {
    1 + (width.max(height) / MIN_MIP_SIZE).max(1).ilog2()
}

/// Lazily loaded game fonts and this frame's on/off decision.
#[derive(Default)]
pub(crate) struct GameFonts {
    enabled: bool,
    attempted: bool,
    /// The console font was loaded or failed to (the classic console uses it
    /// whether `ui_gameFont` is on or not).
    console_attempted: bool,
    menu: Option<Layer>,
    chat: Option<Layer>,
    console: Option<Layer>,
    /// The headline family, loaded with the SJK UI's ([`RetailFont::Headline`]).
    headline: Option<Layer>,
    /// The SJK UI's families were loaded or failed to: they load the first
    /// time `ui_menuStyle sjk` is on ([`SjkFonts`]).
    sjk_attempted: bool,
    sjk: Option<SjkLayers>,
    /// The fonts from the PC the player chose for chat and the console, by
    /// [`Scope::index`].
    system: [SystemSlot; 2],
}

/// A font from the PC for one [`Scope`]: the family its cvar named when last read,
/// the worker loading it and, once loaded, its layer.
#[derive(Default)]
struct SystemSlot {
    wanted: String,
    /// The family [`Self::layer`] was made from, as the system spells it.
    shown: String,
    /// The last family that could not be loaded, so it is not tried every frame.
    failed: String,
    /// The one load in flight, for this family: a change while it runs waits for it,
    /// and its result is dropped when the family is no longer wanted.
    job: Option<(String, Receiver<Result<system_font::Loaded, String>>)>,
    layer: Option<Layer>,
}

/// The SJK UI's display and body families on the GPU.
struct SjkLayers {
    display: Layer,
    body: Layer,
}

/// DPI scale the SJK UI's families are rasterized at: their glyphs are 144
/// pixels tall, so titles stay sharp at 4K whatever the monitor's scale.
const SJK_RASTER_SCALE: f64 = 1.5;

/// Where an SJK UI screen appends its text this frame: the display family
/// (Rajdhani) and the body family (Exo 2), each with its own vertices, and the
/// code family (JetBrains Mono, the console font's layer, drawn after the other
/// text) when it is loaded.
pub(crate) struct SjkFonts<'a> {
    pub(crate) display: (&'a mut Vec<TextVertex>, &'a UiFont),
    pub(crate) body: (&'a mut Vec<TextVertex>, &'a UiFont),
    pub(crate) code: Option<(&'a mut Vec<TextVertex>, &'a UiFont)>,
}

impl GameFonts {
    /// Fonts for a new world: loaded now when `enabled`, else on first use. The
    /// console font alone is loaded when `console` (the classic console is in
    /// use).
    pub(crate) fn preload(enabled: bool, console: bool, gpu: &Device<'_>) -> Self {
        let mut fonts = Self::default();
        if enabled {
            fonts.load(gpu);
        } else if console {
            fonts.load_console(gpu);
        }
        fonts
    }

    fn load(&mut self, gpu: &Device<'_>) {
        self.attempted = true;
        self.menu = Layer::load(&retail_font::MENU, gpu);
        self.chat = Layer::load(&retail_font::CHAT, gpu);
        self.load_console(gpu);
    }

    /// Load the SJK UI's families, once, with the console font its code family
    /// draws in.
    fn load_sjk(&mut self, gpu: &Device<'_>, logo: Option<&text::LogoGlyph>) {
        self.sjk_attempted = true;
        self.load_console(gpu);
        let layer = |family: &text::Family| match text::load_family(family, SJK_RASTER_SCALE, logo)
        {
            Ok(atlas) => Some(Layer::upload(family.name, atlas.font, atlas.image, gpu)),
            Err(error) => {
                crate::log::progress(format_args!(
                    "warning: SJK UI font {} unavailable, keeping Inter: {error}",
                    family.name
                ));
                None
            }
        };
        self.headline = layer(&text::HEADLINE);
        self.sjk = match (layer(&text::DISPLAY), layer(&text::BODY)) {
            (Some(display), Some(body)) => Some(SjkLayers { display, body }),
            _ => None,
        };
    }

    /// Whether the SJK UI's families are loaded.
    pub(crate) fn has_sjk(&self) -> bool {
        self.sjk.is_some()
    }

    /// The SJK UI's display and body families' metrics, to measure text with
    /// before it is appended; `None` while they are not loaded.
    pub(crate) fn sjk_metrics(&self) -> Option<(&UiFont, &UiFont)> {
        self.sjk
            .as_ref()
            .map(|layers| (&layers.display.font, &layers.body.font))
    }

    /// The SJK UI's text targets, once its families are loaded; `None` before
    /// that, or when they failed to load (the screen then draws in Inter).
    pub(crate) fn sjk(&mut self) -> Option<SjkFonts<'_>> {
        let layers = self.sjk.as_mut()?;
        Some(SjkFonts {
            display: (&mut layers.display.vertices, &layers.display.font),
            body: (&mut layers.body.vertices, &layers.body.font),
            code: self
                .console
                .as_mut()
                .map(|layer| (&mut layer.vertices, &layer.font)),
        })
    }

    /// Load the console font unless it was already loaded or failed to.
    fn load_console(&mut self, gpu: &Device<'_>) {
        if !self.console_attempted {
            self.console_attempted = true;
            self.console = Layer::load_console(gpu);
        }
    }

    /// The console font's metrics when it is loaded, whether `ui_gameFont` is
    /// on or not (the classic console draws with it).
    pub(crate) fn console_font(&self) -> Option<&UiFont> {
        self.console_layer().map(|layer| &layer.font)
    }

    /// The SJK UI's display and body families' atlases, each with whether it
    /// is a distance field, once loaded (the SJK UI's console draws its
    /// labels with them on the console's layer).
    pub(crate) fn sjk_atlases(&self) -> Option<[(&wgpu::BindGroup, bool); 2]> {
        self.sjk.as_ref().map(|layers| {
            [&layers.display, &layers.body].map(|layer| (&layer.bind_group, layer.distance_field))
        })
    }

    /// The console font's atlas and whether it is a distance field.
    pub(crate) fn console_atlas(&self) -> Option<(&wgpu::BindGroup, bool)> {
        self.console_layer()
            .map(|layer| (&layer.bind_group, layer.distance_field))
    }

    /// The layer the console's own grid (the classic console and the SJK UI's) draws
    /// with: the player's font from the PC (`con_font`) once loaded, else JetBrains
    /// Mono. The HUD's and notify lines' console text keeps JetBrains Mono.
    fn console_layer(&self) -> Option<&Layer> {
        self.system[Scope::Console.index()]
            .layer
            .as_ref()
            .or(self.console.as_ref())
    }

    fn slot(&self, font: RetailFont) -> &Option<Layer> {
        match font {
            RetailFont::Medium => &self.menu,
            RetailFont::Small => &self.chat,
            RetailFont::Console => &self.console,
            RetailFont::Headline => &self.headline,
            RetailFont::Chat => &self.system[Scope::Chat.index()].layer,
        }
    }

    fn slot_mut(&mut self, font: RetailFont) -> &mut Option<Layer> {
        match font {
            RetailFont::Medium => &mut self.menu,
            RetailFont::Small => &mut self.chat,
            RetailFont::Console => &mut self.console,
            RetailFont::Headline => &mut self.headline,
            RetailFont::Chat => &mut self.system[Scope::Chat.index()].layer,
        }
    }

    /// Whether `font` is on this frame: the retail ones with `ui_gameFont`, the
    /// headline and the player's chat font always.
    fn on(&self, font: RetailFont) -> bool {
        match font {
            RetailFont::Headline | RetailFont::Chat => true,
            RetailFont::Medium | RetailFont::Small | RetailFont::Console => self.enabled,
        }
    }

    /// Whether `font` is on and loaded this frame.
    pub(crate) fn active(&self, font: RetailFont) -> bool {
        self.on(font) && self.slot(font).is_some()
    }

    /// Layout metrics of `font` when it is on and loaded.
    pub(crate) fn font(&self, font: RetailFont) -> Option<&UiFont> {
        self.slot(font)
            .as_ref()
            .filter(|_| self.on(font))
            .map(|layer| &layer.font)
    }

    /// Text target for a surface retail drew with `font`: that font when it is
    /// on and loaded, otherwise the given Inter `vertices` and `inter`.
    pub(crate) fn target<'a>(
        &'a mut self,
        font: RetailFont,
        vertices: &'a mut Vec<TextVertex>,
        inter: &'a UiFont,
    ) -> (&'a mut Vec<TextVertex>, &'a UiFont) {
        let on = self.on(font);
        target(on, self.slot_mut(font), vertices, inter)
    }

    /// Drop the text every retail font holds this frame, for a screen that covers
    /// the frame (the console's command browser).
    pub(crate) fn clear_text(&mut self) {
        for layer in self.layers_mut() {
            layer.vertices.clear();
        }
    }

    /// Text target for menus ([`RetailFont::Medium`]).
    pub(crate) fn menu<'a>(
        &'a mut self,
        vertices: &'a mut Vec<TextVertex>,
        font: &'a UiFont,
    ) -> (&'a mut Vec<TextVertex>, &'a UiFont) {
        self.target(RetailFont::Medium, vertices, font)
    }

    /// Append one draw list's text, each command in the font `font_of` names
    /// for it when that font is on and loaded, and everything else to
    /// `fallback`. With the option off this is the single pass it replaces.
    /// Under a card (`cover`, [`crate::menu_widgets::MenuCanvas::cover`]): the
    /// runs before it less what the card covers, then the card's own. Each command's
    /// colour codes draw in the palette `palette_of` gives for it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn append_routed<'s>(
        &mut self,
        draw_list: &DrawList,
        resolve: impl Fn(TextId) -> &'s str + Copy,
        font_of: impl Fn(TextId, &str) -> Option<RetailFont> + Copy,
        palette_of: impl Fn(TextId) -> text::CodePalette + Copy,
        fallback: (&mut Vec<TextVertex>, &UiFont),
        viewport: [f32; 2],
        style: text::TextStyle,
        cover: Option<(sjk_ui::Rect, u32)>,
    ) {
        let active = RetailFont::ALL.map(|font| self.active(font));
        let (vertices, fallback_font) = fallback;
        // Without a card one pass of everything; with one, the page's then the card's.
        let passes: &[Option<bool>] = if cover.is_some() {
            &[Some(false), Some(true)]
        } else {
            &[None]
        };
        for &card in passes {
            let first = cover.map_or(0, |(_, first)| first);
            let in_pass = |id: TextId| card.is_none_or(|card| (id.0 >= first) == card);
            let drop_under = |vertices: &mut Vec<TextVertex>, start: usize| {
                if let (Some(false), Some((rect, _))) = (card, cover) {
                    text::drop_quads_under(vertices, start, rect, viewport);
                }
            };
            for font in RetailFont::ALL {
                if !active[font.index()] {
                    continue;
                }
                if let Some(layer) = self.slot_mut(font) {
                    let start = layer.vertices.len();
                    crate::ui_renderer::append_text_commands_paletted(
                        draw_list,
                        resolve,
                        |id, text| in_pass(id) && font_of(id, text) == Some(font),
                        &mut layer.vertices,
                        &layer.font,
                        viewport,
                        style,
                        palette_of,
                    );
                    drop_under(&mut layer.vertices, start);
                }
            }
            let start = vertices.len();
            crate::ui_renderer::append_text_commands_paletted(
                draw_list,
                resolve,
                |id, text| {
                    in_pass(id) && !font_of(id, text).is_some_and(|font| active[font.index()])
                },
                vertices,
                fallback_font,
                viewport,
                style,
                palette_of,
            );
            drop_under(vertices, start);
        }
    }

    fn layers_mut(&mut self) -> impl Iterator<Item = &mut Layer> {
        let sjk = self
            .sjk
            .iter_mut()
            .flat_map(|layers| [&mut layers.display, &mut layers.body]);
        self.menu
            .iter_mut()
            .chain(self.chat.iter_mut())
            .chain(self.console.iter_mut())
            .chain(self.headline.iter_mut())
            .chain(sjk)
            .chain(
                self.system
                    .iter_mut()
                    .filter_map(|slot| slot.layer.as_mut()),
            )
    }

    /// Bind every loaded font's atlas again with `gpu`'s wide and emoji atlases, after
    /// one of those was made ([`text::wide_texture::WideTexture::update`]).
    pub(crate) fn rebind(&mut self, gpu: &Device<'_>) {
        for layer in self.layers_mut() {
            layer.bind_group = text::wide_texture::bind_group(
                gpu.device,
                "SJK game font bind group",
                gpu.layout,
                &layer.atlas,
                gpu.sampler,
                gpu.wide,
            );
        }
    }

    /// Copy this frame's vertices to the GPU.
    pub(crate) fn upload(&mut self, queue: &crate::frame_queue::FrameQueue) {
        for layer in self.layers_mut() {
            layer.count = u32::try_from(layer.vertices.len()).unwrap_or(0);
            if layer.count != 0 {
                queue.write_buffer(&layer.buffer, 0, bytemuck::cast_slice(&layer.vertices));
            }
        }
    }

    /// Draw the uploaded medium, small and headline font text, which sits under the Inter
    /// text: distance-field atlases with `sdf_pipeline`, coverage atlases with
    /// the plain text `pipeline`.
    pub(crate) fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        pipeline: &wgpu::RenderPipeline,
        sdf_pipeline: &wgpu::RenderPipeline,
    ) {
        let chat = &self.system[Scope::Chat.index()].layer;
        for layer in self
            .menu
            .iter()
            .chain(&self.chat)
            .chain(&self.headline)
            .chain(chat)
        {
            layer.draw(pass, pipeline, sdf_pipeline);
        }
        if let Some(layers) = &self.sjk {
            layers.display.draw(pass, pipeline, sdf_pipeline);
            layers.body.draw(pass, pipeline, sdf_pipeline);
        }
    }

    /// Draw the uploaded console font text, after all other text.
    pub(crate) fn draw_console(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        pipeline: &wgpu::RenderPipeline,
        sdf_pipeline: &wgpu::RenderPipeline,
    ) {
        if let Some(layer) = &self.console {
            layer.draw(pass, pipeline, sdf_pipeline);
        }
    }

    /// Follow `cg_chatFont` and `con_font` in `console`. A new family is taken at once
    /// when an earlier world made it, else loaded on one worker at a time; the font on
    /// show stays until the new one is ready (empty drops it at once). A load that
    /// finishes for a family no longer wanted is dropped, unlogged and uncached, and the
    /// wanted one starts. Missing or unreadable keeps SJK's own font. Every frame this
    /// compares the cvars' text; it allocates only when a value changes or a load ends.
    fn follow_system_fonts(
        &mut self,
        console: Option<&crate::console::ViewerConsole>,
        gpu: &Device<'_>,
    ) {
        for scope in Scope::ALL {
            let slot = &mut self.system[scope.index()];
            let value = console
                .and_then(|console| console.text_value(scope.cvar()))
                .unwrap_or("")
                .trim();
            if value != slot.wanted {
                slot.wanted.clear();
                slot.wanted.push_str(value);
            }
            if slot.wanted.is_empty() && slot.layer.is_some() {
                slot.layer = None;
                slot.shown.clear();
            }
            let finished = match &slot.job {
                Some((_, receiver)) => match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(mpsc::TryRecvError::Disconnected) => {
                        Some(Err("the font worker stopped".to_owned()))
                    }
                    Err(mpsc::TryRecvError::Empty) => None,
                },
                None => None,
            };
            if let Some(result) = finished
                && let Some((family, _)) = slot.job.take()
                && family.eq_ignore_ascii_case(&slot.wanted)
            {
                match result {
                    Ok((name, atlas)) => {
                        system_font::remember(scope, &name, &atlas);
                        crate::log::progress(format_args!(
                            "{} font: {name} from the system",
                            scope.cvar()
                        ));
                        slot.layer = Some(Layer::upload(
                            scope.cvar(),
                            atlas.font.clone(),
                            atlas.image.clone(),
                            gpu,
                        ));
                        slot.shown = name;
                    }
                    Err(error) => {
                        crate::log::progress(format_args!(
                            "warning: {} font unavailable, keeping SJK's: {error}",
                            scope.cvar()
                        ));
                        slot.layer = None;
                        slot.shown.clear();
                        slot.failed = family;
                    }
                }
            }
            let current = slot.layer.is_some() && slot.shown.eq_ignore_ascii_case(&slot.wanted);
            if slot.wanted.is_empty()
                || current
                || slot.job.is_some()
                || slot.failed.eq_ignore_ascii_case(&slot.wanted)
            {
                continue;
            }
            if let Some((name, atlas)) = system_font::cached(scope, &slot.wanted) {
                slot.layer = Some(Layer::upload(
                    scope.cvar(),
                    atlas.font.clone(),
                    atlas.image.clone(),
                    gpu,
                ));
                slot.shown = name;
                continue;
            }
            let (sender, receiver) = mpsc::sync_channel(1);
            let family = slot.wanted.clone();
            let request = family.clone();
            let spawned = std::thread::Builder::new()
                .name("system font".to_owned())
                .spawn(move || {
                    let _ = sender.send(system_font::load_installed(scope, &request));
                });
            match spawned {
                Ok(_) => slot.job = Some((family, receiver)),
                Err(_) => slot.failed = family,
            }
        }
    }
}

fn target<'a>(
    enabled: bool,
    layer: &'a mut Option<Layer>,
    vertices: &'a mut Vec<TextVertex>,
    font: &'a UiFont,
) -> (&'a mut Vec<TextVertex>, &'a UiFont) {
    match layer {
        Some(layer) if enabled => (&mut layer.vertices, &layer.font),
        _ => (vertices, font),
    }
}

/// Read the option, load the fonts on first use and clear last frame's text.
/// Call before any text is appended.
pub(crate) fn prepare(gpu: &mut GpuState) {
    let enabled = enabled(gpu.console.as_ref());
    // The console is a character grid on its own layer (the classic one or
    // the SJK UI's), which needs the console font whatever `ui_gameFont` says.
    let console = gpu.console.is_some();
    let fonts = &mut gpu.game_fonts;
    fonts.enabled = enabled;
    for layer in fonts.layers_mut() {
        layer.vertices.clear();
    }
    let load = enabled && !fonts.attempted;
    let load_console = console && !fonts.console_attempted;
    // The SJK UI's families: every menu style draws SJK's own pages (Profile,
    // Update, Import, Camera control, ...) in them.
    let load_sjk = !fonts.sjk_attempted;
    let device = Device {
        device: &gpu.device,
        queue: &gpu.queue,
        layout: &gpu.text_layout,
        sampler: &gpu.text_sampler,
        wide: &gpu.wide_text,
    };
    fonts.follow_system_fonts(gpu.console.as_ref(), &device);
    if load || load_console || load_sjk {
        if load {
            fonts.load(&device);
        } else if load_console {
            fonts.load_console(&device);
        }
        if load_sjk {
            fonts.load_sjk(&device, gpu.logo_glyph.as_ref());
        }
    }
}

/// Whether `console` draws the classic console, whose background is the
/// `console` shader and whose command browser is classic+.
pub(crate) fn classic_console(console: Option<&crate::console::ViewerConsole>) -> bool {
    console.is_some_and(|console| {
        console.console_style() == crate::console::console_options::ConsoleStyle::Classic
    })
}

/// Whether the option is on in `console`, for preloading a world's fonts.
pub(crate) fn enabled(console: Option<&crate::console::ViewerConsole>) -> bool {
    console
        .and_then(|console| console.bool_cvar(CVAR))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_atlases_get_no_mips() {
        assert_eq!(mip_levels(512, 256), 1);
        assert_eq!(mip_levels(256, 256), 1);
    }

    #[test]
    fn large_atlases_mip_down_to_512() {
        assert_eq!(mip_levels(1_024, 512), 2);
        assert_eq!(mip_levels(4_096, 1_024), 4);
    }

    #[test]
    fn falls_back_to_inter_when_off_or_missing() {
        let inter = text::console_font::load().unwrap().font;
        let mut vertices = Vec::new();
        let mut missing = None;
        let (chosen, font) = target(true, &mut missing, &mut vertices, &inter);
        assert!(std::ptr::eq(font, &inter));
        chosen.push(bytemuck::Zeroable::zeroed());
        assert_eq!(vertices.len(), 1);
    }

    #[test]
    fn routing_with_no_font_loaded_sends_everything_to_the_fallback() {
        let inter = text::console_font::load().unwrap().font;
        let mut list = DrawList::new(8);
        for id in 0..3 {
            let _ = list.push(sjk_ui::DrawCommand::Text {
                rect: sjk_ui::Rect::new(0.0, 0.0, 100.0, 16.0),
                text: TextId(id),
                size: 16.0,
                color: sjk_ui::Color::new(1.0, 1.0, 1.0, 1.0),
                align: sjk_ui::TextAlign::Start,
                overflow: sjk_ui::TextOverflow::Clip,
                weight: sjk_ui::FontWeight::Regular,
                letter_spacing: 0.0,
            });
        }
        let mut fonts = GameFonts {
            enabled: true,
            ..GameFonts::default()
        };
        let mut vertices = Vec::new();
        fonts.append_routed(
            &list,
            |_| "ab",
            |id, _| (id.0 == 1).then_some(RetailFont::Medium),
            |_| text::CodePalette::Game,
            (&mut vertices, &inter),
            [640.0, 480.0],
            text::TextStyle::NEUTRAL,
            None,
        );
        // Three runs of two glyphs, each a shadow and a glyph quad.
        assert_eq!(vertices.len(), 3 * 2 * 2 * 6);
        // A card from run 2 on over the first two (the chat's sender card): their
        // glyphs under it are left out, the card's own are drawn.
        vertices.clear();
        fonts.append_routed(
            &list,
            |_| "ab",
            |id, _| (id.0 == 1).then_some(RetailFont::Medium),
            |_| text::CodePalette::Game,
            (&mut vertices, &inter),
            [640.0, 480.0],
            text::TextStyle::NEUTRAL,
            Some((sjk_ui::Rect::new(0.0, 0.0, 200.0, 40.0), 2)),
        );
        assert_eq!(vertices.len(), 2 * 2 * 6);
    }

    #[test]
    fn the_headline_font_draws_capitals_as_tall_as_sjk_menu() {
        // Centre prints keep their size (`CENTER_FONT_1080`, set for `ergoec`'s line)
        // in the headline font: grown by `HEADLINE_GROWTH`, its capitals are as tall
        // as SJK Menu's, within 2 %.
        let headline = text::load_family(&text::HEADLINE, 1.0, None).unwrap().font;
        let menu = retail_font::load(&retail_font::MENU).unwrap().font;
        let capitals =
            |font: &UiFont| font.glyph(text::TextFace::Semibold, b'H').height / font.height;
        let ratio = capitals(&headline) * HEADLINE_GROWTH / capitals(&menu);
        assert!((0.98..=1.02).contains(&ratio), "capitals {ratio}");
    }

    #[test]
    fn line_heights_become_glyph_scales_in_any_font() {
        // 16-pixel console characters drawn on a 24-pixel line at 1080 lines.
        let font = text::console_font::load().unwrap().font;
        let scale = crate::ui_scale::glyph_scale(&font, 24.0, 1.0);
        assert_eq!(scale, 1.5);
    }
}
