//! Following the system's default output device (#210). The output used to be opened
//! once, on the device that was the default at start: plugging headphones in kept the
//! game on the speakers until a restart, while other programs moved over. Windows does
//! not take the old device away, so the stream raises no error; the output now looks at
//! the default device every [`CHECK`] and, when it changed (or the stream stopped
//! rendering), moves the mixer to it as it is: the playing sounds, loops and music go
//! on where they were, only the audio buffered for the old device is lost.
//!
//! The mixer lives inside the device's callback once it plays; [`Handback`] gives it
//! back when the stream is dropped, so a new stream can play it.

use super::MixerSource;
use rodio::Source;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How often the default device is looked at: one cheap system call.
pub(super) const CHECK: Duration = Duration::from_millis(1_500);
/// How long a dropped stream may take to give its mixer back.
const RETURN_WAIT: Duration = Duration::from_millis(500);

/// Where the mixer goes back to when its stream ends.
pub(super) type Home = Arc<Mutex<Option<MixerSource>>>;

/// The mixer as a device's callback pulls it; dropped with the stream, it puts the
/// mixer back in its [`Home`]. The lock is only taken then, never while mixing.
pub(super) struct Handback {
    source: Option<MixerSource>,
    home: Home,
}

impl Handback {
    pub(super) fn new(source: MixerSource, home: &Home) -> Self {
        Self {
            source: Some(source),
            home: Arc::clone(home),
        }
    }
}

impl Iterator for Handback {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        self.source.as_mut()?.next()
    }
}

impl Source for Handback {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.source.as_ref().map_or(2, Source::channels)
    }

    fn sample_rate(&self) -> u32 {
        self.source
            .as_ref()
            .map_or(super::SAMPLE_RATE, Source::sample_rate)
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Drop for Handback {
    fn drop(&mut self) {
        if let Some(source) = self.source.take()
            && let Ok(mut home) = self.home.lock()
        {
            *home = Some(source);
        }
    }
}

/// The mixer back from a stream just dropped, waiting a little for the device's thread.
pub(super) fn take_back(home: &Home) -> Option<MixerSource> {
    let deadline = Instant::now() + RETURN_WAIT;
    loop {
        if let Some(source) = home.lock().ok().and_then(|mut home| home.take()) {
            return Some(source);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The system's default output device now.
pub(super) fn default_device() -> Option<rodio::cpal::Device> {
    rodio::cpal::default_host().default_output_device()
}

/// `device`'s name, which tells devices apart.
pub(super) fn name(device: &rodio::cpal::Device) -> Option<String> {
    device.name().ok()
}

/// Name and format of `device`, for the log.
pub(super) fn describe(device: Option<&rodio::cpal::Device>) -> String {
    let Some(device) = device else {
        return "no default output device".into();
    };
    let name = device.name().unwrap_or_else(|_| "unnamed device".into());
    match device.default_output_config() {
        Ok(config) => format!(
            "device={name} {}Hz {}ch {:?}",
            config.sample_rate().0,
            config.channels(),
            config.sample_format()
        ),
        Err(error) => format!("device={name} (format unknown: {error})"),
    }
}

/// When the output looks at the default device again, and what it saw last.
pub(super) struct Follow {
    /// The name of the device the stream plays on (`None`: opened by rodio's own
    /// fallback, or unnamed).
    pub(super) name: Option<String>,
    pub(super) home: Home,
    next_check: Instant,
    /// Mix blocks rendered at the last look: unchanged since, the stream stopped.
    blocks: u64,
    /// The count was unchanged at the last look too.
    still: bool,
}

impl Follow {
    pub(super) fn new(device: Option<&rodio::cpal::Device>, home: Home) -> Self {
        Self {
            name: device.and_then(name),
            home,
            next_check: Instant::now() + CHECK,
            blocks: 0,
            still: false,
        }
    }

    /// Whether it is time to look ([`CHECK`]): the next look is then scheduled.
    pub(super) fn due(&mut self, now: Instant) -> bool {
        if now < self.next_check {
            return false;
        }
        self.next_check = now + CHECK;
        true
    }

    /// Whether the stream just stopped rendering: nothing since the last look, counting
    /// `blocks`, after it had rendered. A stream that stays still (a device that opens
    /// but never plays) is reopened once, not at every look.
    pub(super) fn stalled(&mut self, blocks: u64) -> bool {
        let still = blocks == self.blocks;
        self.blocks = blocks;
        let stalled = still && !self.still;
        self.still = still;
        stalled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dropped_handback_returns_its_mixer_home() {
        let (_output, source) = super::super::AudioOutput::test_source();
        let home: Home = Arc::default();
        let mut handback = Handback::new(source, &home);
        assert!(handback.next().is_some());
        assert!(home.lock().unwrap().is_none());
        drop(handback);
        let mut back = take_back(&home).expect("the mixer came back");
        // It mixes on, as it did.
        assert!(back.next().is_some());
        assert!(take_back(&Arc::default()).is_none());
    }

    #[test]
    fn the_device_is_looked_at_every_check_and_a_still_count_is_a_stall() {
        let mut follow = Follow::new(None, Arc::default());
        let now = Instant::now();
        assert!(!follow.due(now));
        assert!(follow.due(now + CHECK));
        assert!(!follow.due(now + CHECK));
        assert!(!follow.stalled(10));
        assert!(follow.stalled(10));
        // Still stopped: not reopened at every look.
        assert!(!follow.stalled(10));
        assert!(!follow.stalled(11));
        assert!(follow.stalled(11));
    }
}
