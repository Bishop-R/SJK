//! The hit sounds of `cg_hitsounds` 1 to 4, and the teammate's, when the game data has
//! none of its own (Lex's request, 03/11/2026).
//!
//! `cg_hitsounds` is TaystJK's and JoF EJK's (jaPRO's) option: a sound when the
//! player's hit counter (`PERS_HITS`) rises, the server's count of hits on foes, and
//! another when it falls, a hit on a teammate (`CG_CheckLocalSounds`). Their files,
//! `sound/effects/hitsound.wav` to `hitsound4.wav` and `hitsoundteam.wav`, come with
//! jaPRO's and EJK's packs, not with Jedi Academy, so without such a pack the option was
//! silent. A pack's file still wins ([`super::transitions::CUES`]); without one, the
//! sound is made here: short tones, so no file is shipped or borrowed.

/// Samples a second of the made sounds.
const RATE: u32 = 22_050;

/// One made sound: tones one after the other.
struct Recipe {
    /// Each tone: its start and end pitch (Hz) and length (seconds).
    tones: &'static [(f32, f32, f32)],
    /// Peak level, 0 to 1.
    level: f32,
}

/// `cg_hitsounds` 1 to 4, then the teammate's (the order of `CUES`' first five).
const RECIPES: [Recipe; 5] = [
    // 1: a short high tick.
    Recipe {
        tones: &[(1_900.0, 1_900.0, 0.045)],
        level: 0.55,
    },
    // 2: a softer, lower blip.
    Recipe {
        tones: &[(1_100.0, 1_000.0, 0.070)],
        level: 0.6,
    },
    // 3: two quick rising notes.
    Recipe {
        tones: &[(1_400.0, 1_400.0, 0.035), (2_100.0, 2_100.0, 0.045)],
        level: 0.5,
    },
    // 4: a dull knock.
    Recipe {
        tones: &[(520.0, 380.0, 0.060)],
        level: 0.75,
    },
    // A teammate hit: a falling buzz.
    Recipe {
        tones: &[(480.0, 260.0, 0.140)],
        level: 0.5,
    },
];

/// The made sound standing in for `CUES[index]`, as a WAV file; `None` past the five.
pub(super) fn made(index: usize) -> Option<Box<[u8]>> {
    RECIPES.get(index).map(|recipe| wav(&samples(recipe)))
}

/// The recipe's samples: each tone a sine gliding between its pitches, with a 2 ms
/// rise and an exponential fall so it clicks neither in nor out.
fn samples(recipe: &Recipe) -> Vec<i16> {
    let mut out = Vec::new();
    for &(from, to, length) in recipe.tones {
        let count = (length * RATE as f32) as usize;
        let mut phase = 0.0_f32;
        for n in 0..count {
            let t = n as f32 / count as f32;
            let pitch = from + (to - from) * t;
            phase += std::f32::consts::TAU * pitch / RATE as f32;
            let rise = (n as f32 / (0.002 * RATE as f32)).min(1.0);
            let fall = (-4.0 * t).exp() * (1.0 - t);
            let value = phase.sin() * recipe.level * rise * fall;
            out.push((value * f32::from(i16::MAX)) as i16);
        }
    }
    out
}

/// A mono 16-bit PCM WAV file of `samples` at [`RATE`].
fn wav(samples: &[i16]) -> Box<[u8]> {
    let data = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1_u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2_u16.to_le_bytes());
    out.extend_from_slice(&16_u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out.into_boxed_slice()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_short_sounds_that_decode() {
        for index in 0..5 {
            let bytes = made(index).unwrap();
            let decoded = sjk_audio::decode_encoded(&bytes, "wav", RATE).expect("a valid WAV");
            let seconds = decoded.samples().len() as f32 / RATE as f32;
            assert!((0.03..0.2).contains(&seconds), "{index}: {seconds} s");
        }
        assert!(made(5).is_none(), "the saber hits are the game's own");
    }

    #[test]
    fn they_start_and_end_silent_and_never_clip() {
        for recipe in &RECIPES {
            let samples = samples(recipe);
            assert!(samples[0].abs() < 2_000);
            assert!(samples.last().unwrap().abs() < 200);
            assert!(samples.iter().all(|s| *s > i16::MIN && *s < i16::MAX));
        }
    }
}
