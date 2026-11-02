//! Slider peek (`docs/sjk-ui.md`, Settings): over a match, holding a Settings
//! slider for a moment fades the rest of the screen away (the rail, the other
//! rows, the detail column, the keys and the darkening behind), leaving only
//! that slider's row where it was over the live game, so the change it makes
//! can be seen while dragging. Releasing brings the screen back.
//!
//! The state here is time alone: when the hold began, and when it ended and
//! from how far faded. The SJK UI's view reads [`SliderPeek::show`] each frame.
//! Nothing allocates.

use std::time::{Duration, Instant};

/// The setting: on (the default) lets a held slider peek at the game.
pub(crate) const CVAR: &str = "ui_sliderPeek";

/// How long the pointer holds a slider before the screen starts to fade: a
/// quick click stays well under it, so it never flashes the menu away.
pub(crate) const HOLD: Duration = Duration::from_millis(300);

/// How long the screen takes to fade out, or back in after the release.
pub(crate) const FADE: Duration = Duration::from_millis(150);

/// Where a peek stands.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Phase {
    /// No slider is held and the screen shows whole.
    #[default]
    Idle,
    /// Slider row `row` has been held since `since`.
    Held { row: usize, since: Instant },
    /// The pointer let go of row `row` at `at`, the screen `from` faded then.
    Leaving { row: usize, at: Instant, from: f32 },
}

/// A held slider's peek at the game behind Settings.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SliderPeek {
    phase: Phase,
}

impl SliderPeek {
    /// The pointer pressed slider row `row` at `now`. A press on the row still
    /// fading back carries on from how far faded it is.
    pub(crate) fn press(&mut self, row: usize, now: Instant) {
        let level = match self.phase {
            Phase::Leaving { row: leaving, .. } if leaving == row => self.level(now),
            _ => 0.0,
        };
        let since = if level > 0.0 {
            now.checked_sub(HOLD + FADE.mul_f32(level)).unwrap_or(now)
        } else {
            now
        };
        self.phase = Phase::Held { row, since };
    }

    /// The pointer let go at `now`: a faded screen fades back in.
    pub(crate) fn release(&mut self, now: Instant) {
        let Phase::Held { row, .. } = self.phase else {
            return;
        };
        let from = self.level(now);
        self.phase = if from > 0.0 {
            Phase::Leaving { row, at: now, from }
        } else {
            Phase::Idle
        };
    }

    /// Show the whole screen at once (it opened, a key was pressed, or no
    /// game is behind it).
    pub(crate) fn cancel(&mut self) {
        self.phase = Phase::Idle;
    }

    /// Whether a slider is held, peeking yet or not.
    #[cfg(test)]
    pub(crate) fn held(&self) -> bool {
        matches!(self.phase, Phase::Held { .. })
    }

    /// The row peeking at `now` and how far the rest of the screen has faded
    /// (eased, 0 shows it whole, 1 hides it); `None` when nothing peeks.
    pub(crate) fn show(&mut self, now: Instant) -> Option<(usize, f32)> {
        let level = self.level(now);
        match self.phase {
            Phase::Leaving { .. } if level <= 0.0 => {
                self.phase = Phase::Idle;
                None
            }
            Phase::Held { row, .. } | Phase::Leaving { row, .. } if level > 0.0 => {
                Some((row, smoothstep(level)))
            }
            _ => None,
        }
    }

    /// How far faded the screen is at `now`, linear from 0 to 1.
    fn level(&self, now: Instant) -> f32 {
        let fade = FADE.as_secs_f32();
        match self.phase {
            Phase::Idle => 0.0,
            Phase::Held { since, .. } => {
                let held = now.saturating_duration_since(since);
                (held.saturating_sub(HOLD).as_secs_f32() / fade).min(1.0)
            }
            Phase::Leaving { at, from, .. } => {
                let gone = now.saturating_duration_since(at).as_secs_f32();
                (from - gone / fade).max(0.0)
            }
        }
    }
}

/// Ease in and out over 0 to 1.
fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn a_quick_click_never_peeks() {
        let start = Instant::now();
        let mut peek = SliderPeek::default();
        peek.press(3, start);
        assert_eq!(peek.show(start + ms(100)), None);
        assert_eq!(peek.show(start + ms(299)), None);
        peek.release(start + ms(200));
        assert!(!peek.held());
        assert_eq!(peek.show(start + ms(400)), None, "let go before the hold");
    }

    #[test]
    fn a_hold_fades_the_screen_out_then_back_in() {
        let start = Instant::now();
        let mut peek = SliderPeek::default();
        peek.press(5, start);
        assert_eq!(peek.show(start + HOLD), None, "the fade starts at the hold");
        let (row, half) = peek.show(start + HOLD + ms(75)).expect("fading");
        assert_eq!(row, 5);
        assert!((half - 0.5).abs() < 0.01, "eased halfway: {half}");
        assert_eq!(peek.show(start + HOLD + FADE), Some((5, 1.0)));
        assert_eq!(peek.show(start + ms(5_000)), Some((5, 1.0)), "held, stays");
        let release = start + ms(5_000);
        peek.release(release);
        assert!(!peek.held());
        let (_, back) = peek.show(release + ms(75)).expect("fading back");
        assert!((back - 0.5).abs() < 0.01, "halfway back: {back}");
        assert_eq!(peek.show(release + FADE), None, "whole again");
        assert_eq!(peek.phase, Phase::Idle);
    }

    #[test]
    fn letting_go_mid_fade_returns_from_where_it_was() {
        let start = Instant::now();
        let mut peek = SliderPeek::default();
        peek.press(1, start);
        let release = start + HOLD + ms(30);
        peek.release(release);
        let (_, at) = peek.show(release).expect("a fifth faded");
        assert!((at - smoothstep(0.2)).abs() < 0.01, "{at}");
        assert_eq!(
            peek.show(release + ms(30)),
            None,
            "back in the time it took"
        );
    }

    #[test]
    fn pressing_again_while_it_fades_back_carries_on() {
        let start = Instant::now();
        let mut peek = SliderPeek::default();
        peek.press(2, start);
        let release = start + HOLD + FADE;
        peek.release(release);
        let again = release + ms(75);
        peek.press(2, again);
        let (_, level) = peek.show(again).expect("still half faded");
        assert!((level - 0.5).abs() < 0.01, "no jump: {level}");
        assert_eq!(peek.show(again + ms(75)), Some((2, 1.0)));
        // Another row starts its own hold.
        peek.release(again + ms(75));
        peek.press(4, again + ms(80));
        assert_eq!(peek.show(again + ms(80)), None);
    }

    #[test]
    fn cancel_shows_the_screen_at_once() {
        let start = Instant::now();
        let mut peek = SliderPeek::default();
        peek.press(0, start);
        assert!(peek.show(start + ms(1_000)).is_some());
        peek.cancel();
        assert_eq!(peek.show(start + ms(1_001)), None);
        peek.release(start + ms(1_002));
        assert_eq!(peek.show(start + ms(1_003)), None);
    }

    // `violation_count` exists only in debug builds (`warn_debug`).
    #[cfg(debug_assertions)]
    #[test]
    fn peeking_allocates_nothing() {
        let start = Instant::now();
        let mut peek = SliderPeek::default();
        peek.press(0, start);
        let _ = peek.show(start);
        let before = assert_no_alloc::violation_count();
        assert_no_alloc::assert_no_alloc(|| {
            for step in 0..64 {
                let _ = peek.show(start + ms(step * 10));
            }
            peek.release(start + ms(700));
            let _ = peek.show(start + ms(750));
        });
        assert_eq!(assert_no_alloc::violation_count(), before);
    }
}
