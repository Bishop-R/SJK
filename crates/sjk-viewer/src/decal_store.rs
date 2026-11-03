//! Persistent world decals with the reference's ring-buffer lifetime.
//!
//! Mirrors rd-vanilla `tr_decals.cpp`: two rings (`NORMAL`, `FADE`) of
//! `MAX_DECAL_POLYS` slots, only the first `r_markcount` of which are used
//! (`tr_init.cpp:1617`, default 100). `RE_AllocDecal` (`tr_decals.cpp:87-125`)
//! recycles the head slot and, when that slot's group is older than the
//! current frame, every neighbouring slot from the same frame with it;
//! `RE_FreeDecal` (`tr_decals.cpp:67-81`) moves a normal decal to the fade
//! ring for `DECAL_FADE_TIME` milliseconds. `R_AddDecals`
//! (`tr_decals.cpp:258-323`) emits each live poly per frame, with fading
//! polys' vertex alpha at `255 * (1 - t / DECAL_FADE_TIME)`.
//!
//! Requests arrive from effect spawning without world access and are
//! projected when the frame is prepared, so the queue is a fixed-capacity
//! buffer that drops (and counts) overflow instead of growing.
//!
//! Temporary marks (`CG_ImpactMark(..., temporary = qtrue)`, used by
//! `CG_PlayerShadow`) bypass the rings: cgame clips them with the same
//! `R_MarkFragments` and hands the polys to `R_AddPolysToScene` for the
//! current frame only (`cg_marks.c:131-230`).
//!
//! Cgame's own lasting marks (`CG_ImpactMark(..., temporary = qfalse)` with
//! `cg_marks 1`, such as footprints) live in a third list: at most
//! `MAX_MARK_POLYS` polys, the oldest mark's polys given up for a new one
//! (`CG_AllocMark`), each kept `MARK_TOTAL_TIME` and faded over its last
//! `MARK_FADE_TIME` (`CG_AddMarks`, `cg_marks.c:238-305`).
//!
//! Players' tags (`tags.rs`) have a fourth list of their own, a slot per player: a tag
//! is projected once when it changes and kept until it goes, so impact marks never
//! push one out and `cg_marks 0` keeps them. Their own pass draws them
//! (`tag_render.rs`, [`DecalStore::tag_fragments`]), not this store's [`DecalStore::emit`].

use super::*;
use crate::decal_marks::{DecalRequest, DecalScratch, DecalSurfaces, DecalVertex};

/// `MAX_DECAL_POLYS` (`tr_decals.cpp:27`).
const MAX_DECAL_POLYS: usize = 500;
/// `r_markcount` default (`tr_init.cpp:1617`).
const MARK_COUNT: usize = 100;
/// `DECAL_FADE_TIME` (`tr_decals.cpp:47`).
const FADE_TIME_MILLIS: u32 = 1_000;
/// Marks that can wait for one frame's projection pass.
const MAX_PENDING_REQUESTS: usize = 64;
/// Per-frame polys for temporary marks; `MAX_MARK_FRAGMENTS` twice over.
const MAX_TEMPORARY_POLYS: usize = 256;
/// `MAX_MARK_POLYS` (`cg_local.h:71`): cgame's lasting mark polys.
const MAX_MARK_POLYS: usize = 256;
/// `MARK_TOTAL_TIME` and `MARK_FADE_TIME` (`cg_marks.c:238-239`).
const MARK_TOTAL_TIME: u32 = 10_000;
const MARK_FADE_TIME: u32 = 1_000;
/// Fragments one tag keeps at most (a tag over a corner or a patch is cut in several).
const MAX_TAG_POLYS: usize = 512;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ring {
    Normal = 0,
    Fade = 1,
}

/// One stored fragment (`decalPoly_t`, `tr_decals.cpp:29-37`).
#[derive(Clone)]
struct DecalPoly {
    /// Frame time the poly was allocated on; zero marks a free slot.
    time: u32,
    /// Zero for normal decals; the end of the fade otherwise.
    fade_time: u32,
    fade_duration: u32,
    shader: Arc<str>,
    /// Blend pipeline slot of the shader, resolved once at projection time.
    slot: u8,
    color: [f32; 4],
    vertices: [DecalVertex; crate::decal_marks::MAX_VERTICES_ON_DECAL],
    count: u8,
}

impl DecalPoly {
    fn fill(&mut self, request: &DecalRequest, slot: u8, vertices: &[DecalVertex]) {
        self.shader = Arc::clone(&request.shader);
        self.slot = slot;
        self.color = request.color;
        self.count = vertices.len() as u8;
        self.vertices[..vertices.len()].copy_from_slice(vertices);
    }

    fn free() -> Self {
        Self {
            time: 0,
            fade_time: 0,
            fade_duration: FADE_TIME_MILLIS,
            shader: Arc::from(""),
            slot: 0,
            color: [0.0; 4],
            vertices: [DecalVertex::default(); crate::decal_marks::MAX_VERTICES_ON_DECAL],
            count: 0,
        }
    }
}

/// Ring buffer state for one decal type (`re_decalPolyHead`/`Total`).
struct DecalRing {
    polys: Vec<DecalPoly>,
    head: usize,
    total: usize,
}

impl DecalRing {
    fn new() -> Self {
        Self {
            polys: vec![DecalPoly::free(); MAX_DECAL_POLYS],
            head: 0,
            total: 0,
        }
    }
}

/// A player's tag: what it is, and its fragments once projected (`None` until then),
/// each a convex polygon on the world.
#[derive(Clone)]
struct TagMark {
    tag: sjk_identity::PlacedTag,
    polys: Option<Vec<Vec<DecalVertex>>>,
}

/// A poly emitted this frame by [`DecalStore::emit`].
pub(crate) struct DecalDraw<'a> {
    pub(crate) shader: &'a Arc<str>,
    pub(crate) slot: u8,
    pub(crate) color: [f32; 4],
    pub(crate) vertices: &'a [DecalVertex],
}

pub(crate) struct DecalStore {
    rings: [DecalRing; 2],
    pending: Vec<DecalRequest>,
    pending_temporary: Vec<DecalRequest>,
    /// Polys of this frame's temporary marks; replaced by every projection.
    temporary: Vec<DecalPoly>,
    pending_marks: Vec<DecalRequest>,
    /// Cgame's lasting marks, oldest first.
    marks: std::collections::VecDeque<DecalPoly>,
    /// Each player slot's tag: the tag, and its fragments once projected.
    tags: Vec<Option<TagMark>>,
    /// Counts changes to [`Self::tags`] (a tag set, taken off or projected), so their
    /// pass rebuilds its geometry only then.
    tags_version: u64,
    dropped_requests: usize,
    scratch: DecalScratch,
    /// Tags' buffers ([`DecalScratch::for_tags`]).
    tag_scratch: DecalScratch,
    epoch: Instant,
    /// `cg_marks`; temporary marks bypass it.
    marks_enabled: bool,
}

impl Default for DecalStore {
    fn default() -> Self {
        Self {
            rings: [DecalRing::new(), DecalRing::new()],
            pending: Vec::with_capacity(MAX_PENDING_REQUESTS),
            pending_temporary: Vec::with_capacity(MAX_PENDING_REQUESTS),
            temporary: Vec::with_capacity(MAX_TEMPORARY_POLYS),
            pending_marks: Vec::with_capacity(MAX_PENDING_REQUESTS),
            marks: std::collections::VecDeque::with_capacity(MAX_MARK_POLYS),
            tags: vec![None; crate::looks::SLOTS],
            tags_version: 0,
            dropped_requests: 0,
            scratch: DecalScratch::default(),
            tag_scratch: DecalScratch::for_tags(),
            epoch: Instant::now(),
            marks_enabled: true,
        }
    }
}

impl DecalStore {
    /// `cg_marks 0`: `CG_ImpactMark` returns before queueing
    /// (`codemp/cgame/cg_marks.c:146`); temporary marks (shadows) are unaffected.
    pub(crate) fn set_marks_enabled(&mut self, enabled: bool) {
        self.marks_enabled = enabled;
    }

    /// Queue a mark for the next projection pass.
    pub(crate) fn request(&mut self, request: DecalRequest) {
        if !self.marks_enabled {
            return;
        }
        if self.pending.len() < self.pending.capacity() {
            self.pending.push(request);
        } else {
            self.dropped_requests += 1;
        }
    }

    /// Queue a mark that lives for the next frame only.
    pub(crate) fn request_temporary(&mut self, request: DecalRequest) {
        if self.pending_temporary.len() < self.pending_temporary.capacity() {
            self.pending_temporary.push(request);
        } else {
            self.dropped_requests += 1;
        }
    }

    /// Queue a lasting cgame mark (`CG_ImpactMark`, `temporary` false): drawn for ten
    /// seconds, the last one fading. `cg_marks 0` drops it, as the reference returns.
    pub(crate) fn request_mark(&mut self, request: DecalRequest) {
        if !self.marks_enabled {
            return;
        }
        if self.pending_marks.len() < self.pending_marks.capacity() {
            self.pending_marks.push(request);
        } else {
            self.dropped_requests += 1;
        }
    }

    /// Show `tag` as the tag of player slot `slot`, or none: projected at the next pass
    /// when it changed, kept as it is otherwise.
    pub(crate) fn set_tag(&mut self, slot: usize, tag: Option<&sjk_identity::PlacedTag>) {
        let Some(entry) = self.tags.get_mut(slot) else {
            return;
        };
        if entry.as_ref().map(|mark| &mark.tag) == tag {
            return;
        }
        *entry = tag.map(|tag| TagMark {
            tag: tag.clone(),
            polys: None,
        });
        self.tags_version += 1;
    }

    /// Counts the tags' changes: their fragments are the same while it is.
    pub(crate) fn tags_version(&self) -> u64 {
        self.tags_version
    }

    /// The tags projected so far, each with its fragments (convex polygons on the world).
    pub(crate) fn tag_fragments(
        &self,
    ) -> impl Iterator<Item = (&sjk_identity::PlacedTag, &[Vec<DecalVertex>])> {
        self.tags
            .iter()
            .flatten()
            .filter_map(|mark| Some((&mark.tag, mark.polys.as_deref()?)))
    }

    /// Fragments of the tags on show (world shots).
    #[cfg(test)]
    pub(crate) fn tag_polys(&self) -> usize {
        self.tag_fragments().map(|(_, polys)| polys.len()).sum()
    }

    /// Lasting cgame mark polys stored and requests waiting (world shots).
    #[cfg(test)]
    pub(crate) fn mark_counts(&self) -> (usize, usize) {
        (self.marks.len(), self.pending_marks.len())
    }

    /// Whether any poly would be emitted this frame.
    pub(crate) fn has_polys(&self) -> bool {
        self.live_count() != (0, 0) || !self.temporary.is_empty() || !self.marks.is_empty()
    }

    /// Live polys in the normal and fade rings.
    pub(crate) fn live_count(&self) -> (usize, usize) {
        (self.rings[0].total, self.rings[1].total)
    }

    /// Project every queued request into the world (`RE_AddDecalToScene`).
    /// `slot_of` resolves a shader's blend pipeline slot. Returns the number
    /// of fragments stored.
    pub(crate) fn project_pending(
        &mut self,
        world: &DecalSurfaces,
        bsp: &Bsp,
        now: Instant,
        slot_of: impl Fn(&str) -> usize,
    ) -> usize {
        let time = self.frame_time(now);
        let mut stored = 0;
        for index in 0..self.pending.len() {
            let request = self.pending[index].clone();
            let slot = slot_of(&request.shader) as u8;
            let Self { rings, scratch, .. } = self;
            crate::decal_marks::project(world, bsp, &request, scratch, |vertices| {
                alloc(rings, Ring::Normal, time).fill(&request, slot, vertices);
                stored += 1;
            });
        }
        self.pending.clear();
        self.temporary.clear();
        for index in 0..self.pending_temporary.len() {
            let request = self.pending_temporary[index].clone();
            let slot = slot_of(&request.shader) as u8;
            let Self {
                temporary,
                scratch,
                dropped_requests,
                ..
            } = self;
            crate::decal_marks::project(world, bsp, &request, scratch, |vertices| {
                if temporary.len() == temporary.capacity() {
                    *dropped_requests += 1;
                    return;
                }
                let mut poly = DecalPoly::free();
                poly.time = time;
                poly.fill(&request, slot, vertices);
                temporary.push(poly);
                stored += 1;
            });
        }
        self.pending_temporary.clear();
        for index in 0..self.pending_marks.len() {
            let request = self.pending_marks[index].clone();
            let slot = slot_of(&request.shader) as u8;
            let Self { marks, scratch, .. } = self;
            crate::decal_marks::project(world, bsp, &request, scratch, |vertices| {
                let mut poly = DecalPoly::free();
                poly.time = time;
                poly.fill(&request, slot, vertices);
                push_mark(marks, poly);
                stored += 1;
            });
        }
        self.pending_marks.clear();
        for index in 0..self.tags.len() {
            let Self {
                tags,
                tag_scratch: scratch,
                tags_version,
                ..
            } = self;
            let Some(mark) = tags[index].as_mut().filter(|mark| mark.polys.is_none()) else {
                continue;
            };
            let mut polys = Vec::new();
            if let Some(request) = crate::tags::decal(&mark.tag) {
                crate::decal_marks::project(world, bsp, &request, scratch, |vertices| {
                    if polys.len() < MAX_TAG_POLYS {
                        polys.push(vertices.to_vec());
                    }
                });
            }
            stored += polys.len();
            mark.polys = Some(polys);
            *tags_version += 1;
        }
        stored
    }

    /// `R_AddDecals`: hand every live poly to `draw`, retiring finished fades.
    pub(crate) fn emit(&mut self, now: Instant, mut draw: impl FnMut(DecalDraw<'_>)) {
        let time = self.frame_time(now);
        for ring in [Ring::Normal, Ring::Fade] {
            let head = self.rings[ring as usize].head;
            let mut index = head;
            loop {
                self.emit_slot(ring, index, time, &mut draw);
                index = (index + 1) % MARK_COUNT;
                if index == head {
                    break;
                }
            }
        }
        for poly in &self.temporary {
            draw(DecalDraw {
                shader: &poly.shader,
                slot: poly.slot,
                color: poly.color,
                vertices: &poly.vertices[..usize::from(poly.count)],
            });
        }
        while self
            .marks
            .front()
            .is_some_and(|poly| mark_fade(time.wrapping_sub(poly.time)).is_none())
        {
            self.marks.pop_front();
        }
        for poly in &self.marks {
            let fade = mark_fade(time.wrapping_sub(poly.time)).unwrap_or(0.0);
            draw(DecalDraw {
                shader: &poly.shader,
                slot: poly.slot,
                color: [
                    poly.color[0] * fade,
                    poly.color[1] * fade,
                    poly.color[2] * fade,
                    poly.color[3],
                ],
                vertices: &poly.vertices[..usize::from(poly.count)],
            });
        }
    }

    fn emit_slot(
        &mut self,
        ring: Ring,
        index: usize,
        time: u32,
        draw: &mut impl FnMut(DecalDraw<'_>),
    ) {
        let poly = &self.rings[ring as usize].polys[index];
        if poly.time == 0 {
            return;
        }
        let alpha = if poly.fade_time == 0 {
            poly.color[3]
        } else {
            let elapsed = time.wrapping_sub(poly.time);
            if elapsed >= poly.fade_duration {
                free(&mut self.rings, ring, index, time);
                return;
            }
            1.0 - elapsed as f32 / poly.fade_duration as f32
        };
        draw(DecalDraw {
            shader: &poly.shader,
            slot: poly.slot,
            color: [poly.color[0], poly.color[1], poly.color[2], alpha],
            vertices: &poly.vertices[..usize::from(poly.count)],
        });
    }

    /// Milliseconds since the store was created, never zero (`tr.refdef.time`).
    fn frame_time(&self, now: Instant) -> u32 {
        (now.saturating_duration_since(self.epoch).as_millis() as u32).max(1)
    }
}

/// `CG_AllocMark`: with every poly in use, the oldest mark's polys (all those of its
/// time) are given up first.
fn push_mark(marks: &mut std::collections::VecDeque<DecalPoly>, poly: DecalPoly) {
    if marks.len() >= MAX_MARK_POLYS
        && let Some(oldest) = marks.front().map(|poly| poly.time)
    {
        while marks.front().is_some_and(|poly| poly.time == oldest) {
            marks.pop_front();
        }
    }
    marks.push_back(poly);
}

/// A lasting mark's colour scale `age` milliseconds on (`CG_AddMarks` without
/// `alphaFade`: the colour, not the alpha, goes to black over the last second), or
/// `None` once it is gone.
fn mark_fade(age: u32) -> Option<f32> {
    if age > MARK_TOTAL_TIME {
        return None;
    }
    let left = MARK_TOTAL_TIME - age;
    Some(if left < MARK_FADE_TIME {
        left as f32 / MARK_FADE_TIME as f32
    } else {
        1.0
    })
}

/// `RE_AllocDecal`: always succeeds, evicting the head slot's whole
/// same-frame group when it is older than this frame.
fn alloc(rings: &mut [DecalRing; 2], ring: Ring, time: u32) -> &mut DecalPoly {
    let head = rings[ring as usize].head;
    let existing = rings[ring as usize].polys[head].time;
    if existing != 0 {
        if existing != time {
            let mut index = head;
            loop {
                index = (index + 1) % MARK_COUNT;
                if index == head || rings[ring as usize].polys[index].time != existing {
                    break;
                }
                free(rings, ring, index, time);
            }
        }
        free(rings, ring, head, time);
    }
    let state = &mut rings[ring as usize];
    // Retain the slot's shared shader allocation until fill replaces it.
    state.polys[head].fade_time = 0;
    state.polys[head].fade_duration = FADE_TIME_MILLIS;
    state.polys[head].count = 0;
    state.polys[head].time = time;
    state.total += 1;
    state.head = (head + 1) % MARK_COUNT;
    &mut state.polys[head]
}

/// `RE_FreeDecal`: a normal decal moves to the fade ring before it goes.
fn free(rings: &mut [DecalRing; 2], ring: Ring, index: usize, time: u32) {
    if rings[ring as usize].polys[index].time == 0 {
        return;
    }
    if ring == Ring::Normal {
        let mut fading = rings[0].polys[index].clone();
        fading.time = time;
        fading.fade_time = time + FADE_TIME_MILLIS;
        fading.fade_duration = FADE_TIME_MILLIS;
        *alloc(rings, Ring::Fade, time) = fading;
    }
    let state = &mut rings[ring as usize];
    state.polys[index].time = 0;
    state.total -= 1;
}

#[path = "saber_cut_marks.rs"]
pub(crate) mod saber_cuts;

#[cfg(test)]
mod mark_tests {
    use super::*;

    #[test]
    fn a_mark_lasts_ten_seconds_and_darkens_over_the_last() {
        assert_eq!(mark_fade(0), Some(1.0));
        assert_eq!(mark_fade(9_000), Some(1.0));
        assert_eq!(mark_fade(9_500), Some(0.5));
        assert_eq!(mark_fade(10_000), Some(0.0));
        assert_eq!(mark_fade(10_001), None);
    }

    #[test]
    fn a_full_list_gives_up_the_oldest_marks_polys_together() {
        let mut marks = std::collections::VecDeque::new();
        let poly = |time| DecalPoly {
            time,
            ..DecalPoly::free()
        };
        for index in 0..MAX_MARK_POLYS {
            // Three polys per mark: times 1, 1, 1, 2, 2, 2, ...
            marks.push_back(poly(1 + index as u32 / 3));
        }
        push_mark(&mut marks, poly(500));
        assert_eq!(marks.len(), MAX_MARK_POLYS - 3 + 1);
        assert_eq!(marks.front().map(|poly| poly.time), Some(2));
        assert_eq!(marks.back().map(|poly| poly.time), Some(500));
    }
}
