//! Opt-in room acoustics for the mixer: occlusion (a muffled, quieter sound behind
//! a wall) and a small reverb sized by the room round the listener.
//!
//! The game side finds what is in the way and how large the room is (traces on its
//! own thread) and hands the results over as plain numbers
//! ([`crate::Mixer::set_source_occlusion`], [`crate::Mixer::set_reverb`]); this
//! module only filters. Everything is sized when the mixer is built: nothing here
//! allocates, frees or locks while mixing. With the effects off
//! ([`crate::Mixer::set_spatial_effects`]) the mixer takes its plain path, sample for
//! sample the output it had before this module existed.

use crate::SourceId;

/// How many sources' occlusion the mixer remembers, for voices that start after
/// their source was measured.
pub(crate) const OCCLUSION_SOURCES: usize = 64;

/// Time constant of the occlusion glide: a door shutting muffles in about this
/// long, without a click.
const OCCLUSION_GLIDE_SECONDS: f32 = 0.05;

/// Cutoff of the one-pole lowpass a fully occluded sound is heard through.
const MUFFLE_CUTOFF_HZ: f32 = 700.0;

/// Loudness of the muffled sound against the open one: a wall takes the highs
/// and about 7 dB of what is left.
pub(crate) const MUFFLED_GAIN: f32 = 0.45;

/// Time constant of the reverb's glide toward a new room.
const ROOM_GLIDE_SECONDS: f32 = 0.4;

/// Input gain of the comb bank (Freeverb's fixed gain, doubled for half the combs).
const REVERB_INPUT_GAIN: f32 = 0.03;

/// The most the reverb's tail may sound against the dry mix.
pub const MAX_REVERB_WET: f32 = 1.0;

/// Comb feedback range: the smallest room rings this long, the largest hall no
/// longer (both well inside 1, so the tail always dies away).
const MIN_FEEDBACK: f32 = 0.70;
const MAX_FEEDBACK: f32 = 0.90;

/// Comb and allpass lengths at 44.1 kHz: four of Freeverb's combs and two of its
/// allpasses, the right ear's 23 samples longer for width.
const COMB_LENGTHS: [usize; 4] = [1116, 1277, 1422, 1557];
const ALLPASS_LENGTHS: [usize; 2] = [556, 341];
const STEREO_SPREAD: usize = 23;

/// Room parameters the reverb glides toward.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReverbParams {
    /// How loud the tail is, 0 (none) to [`MAX_REVERB_WET`].
    pub wet: f32,
    /// Room size, 0 (a cupboard) to 1 (a hall): how long the tail rings and how
    /// little its highs are damped.
    pub size: f32,
}

impl ReverbParams {
    fn clamped(self) -> Self {
        Self {
            wet: finite_or_zero(self.wet).clamp(0.0, MAX_REVERB_WET),
            size: finite_or_zero(self.size).clamp(0.0, 1.0),
        }
    }
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

/// The mixer's room-acoustics state, built once with the mixer.
pub(crate) struct SpatialEffects {
    pub(crate) enabled: bool,
    /// The last occlusion measured for each source (0 open, 1 behind a wall).
    sources: [(SourceId, f32); OCCLUSION_SOURCES],
    sources_used: usize,
    next_replaced: usize,
    /// Lowpass coefficient of the muffled sound at the mixer's rate.
    pub(crate) muffle_coefficient: f32,
    sample_rate: f32,
    reverb: Reverb,
    target: ReverbParams,
    current: ReverbParams,
}

impl SpatialEffects {
    pub(crate) fn new(sample_rate: u32) -> Self {
        let rate = sample_rate.max(1) as f32;
        Self {
            enabled: false,
            sources: [(SourceId(0), 0.0); OCCLUSION_SOURCES],
            sources_used: 0,
            next_replaced: 0,
            muffle_coefficient: one_pole_coefficient(MUFFLE_CUTOFF_HZ, rate),
            sample_rate: rate,
            reverb: Reverb::new(sample_rate),
            target: ReverbParams::default(),
            current: ReverbParams::default(),
        }
    }

    /// The occlusion last measured for `source`, 0 when it never was.
    pub(crate) fn occlusion_of(&self, source: SourceId) -> f32 {
        self.sources[..self.sources_used]
            .iter()
            .find(|(known, _)| *known == source)
            .map_or(0.0, |(_, amount)| *amount)
    }

    /// Remember `source`'s occlusion; a full table gives up its oldest entry.
    pub(crate) fn remember(&mut self, source: SourceId, amount: f32) {
        if let Some(entry) = self.sources[..self.sources_used]
            .iter_mut()
            .find(|(known, _)| *known == source)
        {
            entry.1 = amount;
            return;
        }
        if self.sources_used < OCCLUSION_SOURCES {
            self.sources[self.sources_used] = (source, amount);
            self.sources_used += 1;
            return;
        }
        self.sources[self.next_replaced] = (source, amount);
        self.next_replaced = (self.next_replaced + 1) % OCCLUSION_SOURCES;
    }

    /// Forget every source's occlusion and silence the reverb's tail.
    pub(crate) fn clear(&mut self) {
        self.sources_used = 0;
        self.next_replaced = 0;
        self.reverb.silence();
    }

    pub(crate) fn set_reverb(&mut self, params: ReverbParams) {
        self.target = params.clamped();
    }

    /// The fraction of the way an occlusion glides toward its target over `frames`.
    pub(crate) fn occlusion_step(&self, frames: usize) -> f32 {
        glide_fraction(frames, self.sample_rate, OCCLUSION_GLIDE_SECONDS)
    }

    /// Add the reverb of the effects mix in `out` (interleaved stereo) to it.
    pub(crate) fn apply_reverb(&mut self, out: &mut [f32]) {
        let frames = out.len() / 2;
        if frames == 0 {
            return;
        }
        let fraction = glide_fraction(frames, self.sample_rate, ROOM_GLIDE_SECONDS);
        let start = self.current;
        let end = ReverbParams {
            wet: start.wet + (self.target.wet - start.wet) * fraction,
            size: start.size + (self.target.size - start.size) * fraction,
        };
        self.current = end;
        // A dry room with a silent tail costs nothing.
        if start.wet <= 0.0 && end.wet <= 0.0 && self.reverb.quiet {
            return;
        }
        // Feedback and damping follow the room per block; the wet level ramps
        // within the block so a change never steps.
        let feedback = MIN_FEEDBACK + (MAX_FEEDBACK - MIN_FEEDBACK) * end.size;
        let damping = 0.45 - 0.25 * end.size;
        let wet_step = (end.wet - start.wet) / frames as f32;
        let mut wet = start.wet;
        let mut loudest = 0.0_f32;
        for frame in out.chunks_exact_mut(2) {
            let input = (frame[0] + frame[1]) * 0.5 * REVERB_INPUT_GAIN;
            let [left, right] = self.reverb.process(input, feedback, damping);
            loudest = loudest.max(left.abs()).max(right.abs());
            wet += wet_step;
            frame[0] += left * wet;
            frame[1] += right * wet;
        }
        self.reverb.quiet = loudest < 1.0e-6;
    }
}

/// The fraction of the distance a first-order glide with time constant `seconds`
/// covers in `frames` at `rate`.
fn glide_fraction(frames: usize, rate: f32, seconds: f32) -> f32 {
    1.0 - (-(frames as f32) / (rate * seconds)).exp()
}

/// Coefficient `a` of the one-pole lowpass `y += a * (x - y)` with cutoff `hz`.
pub(crate) fn one_pole_coefficient(hz: f32, rate: f32) -> f32 {
    1.0 - (-std::f32::consts::TAU * hz / rate).exp()
}

/// One sample of a voice through its occlusion: `occlusion` 0 is the sample as it
/// is, 1 the lowpassed sample at [`MUFFLED_GAIN`], between the two a blend.
#[inline]
pub(crate) fn occlude(sample: f32, lowpass: &mut f32, coefficient: f32, occlusion: f32) -> f32 {
    *lowpass += coefficient * (sample - *lowpass);
    sample + (*lowpass * MUFFLED_GAIN - sample) * occlusion
}

/// Below this a filter's memory is flushed to zero: denormal floats are slow.
const DENORMAL: f32 = 1.0e-20;

struct Comb {
    buffer: Box<[f32]>,
    index: usize,
    store: f32,
}

impl Comb {
    fn new(length: usize) -> Self {
        Self {
            buffer: vec![0.0; length.max(1)].into_boxed_slice(),
            index: 0,
            store: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, input: f32, feedback: f32, damping: f32) -> f32 {
        let output = self.buffer[self.index];
        self.store = output * (1.0 - damping) + self.store * damping;
        if self.store.abs() < DENORMAL {
            self.store = 0.0;
        }
        self.buffer[self.index] = input + self.store * feedback;
        self.index += 1;
        if self.index == self.buffer.len() {
            self.index = 0;
        }
        output
    }
}

struct Allpass {
    buffer: Box<[f32]>,
    index: usize,
}

impl Allpass {
    fn new(length: usize) -> Self {
        Self {
            buffer: vec![0.0; length.max(1)].into_boxed_slice(),
            index: 0,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buffer[self.index];
        let mut stored = input + delayed * 0.5;
        if stored.abs() < DENORMAL {
            stored = 0.0;
        }
        self.buffer[self.index] = stored;
        self.index += 1;
        if self.index == self.buffer.len() {
            self.index = 0;
        }
        delayed - input
    }
}

/// A small Schroeder/Freeverb reverb: mono in, stereo out, four parallel combs
/// and two allpasses an ear.
pub(crate) struct Reverb {
    combs: [[Comb; 4]; 2],
    allpasses: [[Allpass; 2]; 2],
    /// Whether the last block's tail was silent (skipped while dry).
    quiet: bool,
}

impl Reverb {
    pub(crate) fn new(sample_rate: u32) -> Self {
        let scale = sample_rate.max(1) as f32 / 44_100.0;
        let length = |samples: usize, ear: usize| {
            (((samples + ear * STEREO_SPREAD) as f32) * scale).round() as usize
        };
        Self {
            combs: std::array::from_fn(|ear| {
                std::array::from_fn(|index| Comb::new(length(COMB_LENGTHS[index], ear)))
            }),
            allpasses: std::array::from_fn(|ear| {
                std::array::from_fn(|index| Allpass::new(length(ALLPASS_LENGTHS[index], ear)))
            }),
            quiet: true,
        }
    }

    fn silence(&mut self) {
        for comb in self.combs.iter_mut().flatten() {
            comb.buffer.fill(0.0);
            comb.store = 0.0;
        }
        for allpass in self.allpasses.iter_mut().flatten() {
            allpass.buffer.fill(0.0);
        }
        self.quiet = true;
    }

    /// One input sample through both ears.
    #[inline]
    pub(crate) fn process(&mut self, input: f32, feedback: f32, damping: f32) -> [f32; 2] {
        let feedback = feedback.clamp(0.0, MAX_FEEDBACK);
        std::array::from_fn(|ear| {
            let mut sum = 0.0;
            for comb in &mut self.combs[ear] {
                sum += comb.process(input, feedback, damping);
            }
            for allpass in &mut self.allpasses[ear] {
                sum = allpass.process(sum);
            }
            sum
        })
    }
}
