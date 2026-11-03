//! Sounds muted by name (`s_muteSounds`, akr's and Neffie's request, 03/11/2026;
//! Settings > Sound "Muted sounds", empty by default): words, and every sound whose file
//! path holds one of them is never played, wherever it comes from (an item, a power-up, a
//! player, the map, a mod's sound). `s_muteSounds "ysalamiri hacking"` silences
//! anything with either word in its path.
//!
//! The test runs where the mixer is fed ([`super::AudioOutput::send`]): one bit per
//! sound handle, worked out again only when the words change or new sounds load, so a
//! played sound costs a bit test.

/// The setting.
pub(crate) const CVAR: &str = "s_muteSounds";

/// The muted sound handles and what they were worked out from.
#[derive(Default)]
pub(crate) struct MutedSounds {
    /// The words, lowercase, as last read.
    words: String,
    /// How many sounds were known when the bits were worked out.
    known: usize,
    /// One bit per handle number.
    bits: Vec<u64>,
}

impl MutedSounds {
    /// Follow the setting's `words` over the known sounds (`path`, handle number):
    /// worked out again only when either changed.
    pub(crate) fn sync<'a>(
        &mut self,
        words: &str,
        sounds: impl ExactSizeIterator<Item = (&'a str, u32)>,
    ) {
        let unchanged = self.known == sounds.len()
            && self.words.len() == words.len()
            && self.words.eq_ignore_ascii_case(words);
        if unchanged {
            return;
        }
        self.words.clear();
        self.words.push_str(&words.to_ascii_lowercase());
        self.known = sounds.len();
        self.bits.clear();
        if self.words.split_ascii_whitespace().next().is_none() {
            return;
        }
        for (path, handle) in sounds {
            if matches(&self.words, path) {
                let (word, bit) = (handle as usize / 64, handle % 64);
                if self.bits.len() <= word {
                    self.bits.resize(word + 1, 0);
                }
                self.bits[word] |= 1 << bit;
            }
        }
    }

    /// Whether the sound with handle number `handle` is muted.
    pub(crate) fn mutes(&self, handle: u32) -> bool {
        self.bits
            .get(handle as usize / 64)
            .is_some_and(|bits| bits & (1 << (handle % 64)) != 0)
    }
}

/// Whether `path` holds one of `words` (lowercase), case aside.
fn matches(words: &str, path: &str) -> bool {
    let path = path.to_ascii_lowercase();
    words
        .split_ascii_whitespace()
        .any(|word| path.contains(word))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOUNDS: [(&str, u32); 4] = [
        ("sound/player/ysalamiri_loop.wav", 3),
        ("sound/player/hacking.wav", 70),
        ("sound/weapons/saber/saberhum1.wav", 5),
        ("sound/player/supp_healed.wav", 130),
    ];

    #[test]
    fn a_word_mutes_every_sound_holding_it() {
        let mut muted = MutedSounds::default();
        muted.sync("YSALAMIRI  hacking", SOUNDS.into_iter());
        assert!(muted.mutes(3));
        assert!(muted.mutes(70));
        assert!(!muted.mutes(5));
        assert!(!muted.mutes(130));
        assert!(!muted.mutes(10_000), "a handle past the bits");
    }

    #[test]
    fn it_follows_the_words_and_new_sounds() {
        let mut muted = MutedSounds::default();
        muted.sync("", SOUNDS.into_iter());
        assert!(!muted.mutes(3), "nothing muted by default");
        muted.sync("supp_", SOUNDS[..3].iter().copied());
        assert!(!muted.mutes(130), "not loaded yet");
        muted.sync("supp_", SOUNDS.into_iter());
        assert!(muted.mutes(130), "loaded since");
        muted.sync("", SOUNDS.into_iter());
        assert!(!muted.mutes(130), "cleared");
    }
}
