//! Room acoustics (`spatial_fx.rs`): off is the plain mix, occlusion glides and
//! muffles, the reverb stays bounded and dies away.

use super::*;

const RATE: u32 = 44_100;
const BLOCK: usize = 256;
const NEAR: Attenuation = Attenuation::Linear {
    full_volume_distance: 256.0,
    falloff_per_unit: 0.0008,
};

fn mixer(voices: usize, loops: usize) -> Mixer {
    Mixer::new(MixerConfig {
        voices,
        loops,
        sample_rate: RATE,
        loop_attenuation: NEAR,
        doppler: DopplerConfig::disabled(),
        protected_channels: &[],
    })
}

fn placed(source: u32, origin: [f32; 3]) -> PlayRequest {
    PlayRequest {
        origin: Some(origin),
        source: SourceId(source),
        channel: ChannelId(1),
        volume: 1.0,
        attenuation: NEAR,
    }
}

/// A tone at a quarter of the mixer's rate: well above the muffling cutoff.
fn high_tone(length: usize) -> Vec<f32> {
    (0..length).map(|i| [0.0, 0.5, 0.0, -0.5][i % 4]).collect()
}

/// Deterministic noise in [-1, 1].
fn noise(length: usize) -> Vec<f32> {
    let mut state = 0x1234_5678_u32;
    (0..length)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state as f32 / u32::MAX as f32) * 2.0 - 1.0
        })
        .collect()
}

/// Voices, loops and music, rendered for `blocks` blocks.
fn scene(mixer: &mut Mixer, blocks: usize) -> Vec<f32> {
    let tone = mixer.sound_bank_mut().register_pcm(&noise(30_000), 22_050);
    let music = mixer.sound_bank_mut().register_pcm(&high_tone(9_000), RATE);
    mixer.start_background_track(music, music, 0.5);
    mixer.play(tone, placed(1, [400.0, 100.0, 0.0]));
    mixer.play(
        tone,
        PlayRequest {
            origin: None,
            attenuation: Attenuation::None,
            ..placed(2, [0.0; 3])
        },
    );
    let mut rendered = Vec::new();
    let mut out = vec![0.0; BLOCK];
    for block in 0..blocks {
        mixer.clear_loops();
        mixer.set_loop(SourceId(3), tone, [200.0, -300.0, 0.0], [0.0; 3]);
        mixer.set_loop(SourceId(4), tone, [-500.0, 0.0, 40.0], [0.0; 3]);
        mixer.commit_loops();
        if block == 7 {
            mixer.play(tone, placed(5, [900.0, 0.0, 0.0]));
        }
        mixer.render(&mut out);
        rendered.extend_from_slice(&out);
    }
    rendered
}

fn bits(samples: &[f32]) -> Vec<u32> {
    samples.iter().map(|sample| sample.to_bits()).collect()
}

#[test]
fn switched_off_the_mix_is_bit_identical_whatever_was_measured() {
    let mut plain = mixer(16, 8);
    let expected = scene(&mut plain, 40);
    let mut measured = mixer(16, 8);
    measured.set_spatial_effects(true);
    measured.set_spatial_effects(false);
    for source in 1..=5 {
        measured.set_source_occlusion(SourceId(source), 1.0);
    }
    measured.set_reverb(ReverbParams {
        wet: 1.0,
        size: 1.0,
    });
    assert_eq!(bits(&scene(&mut measured, 40)), bits(&expected));
}

#[test]
fn on_in_the_open_with_a_dry_room_sounds_as_off() {
    let mut plain = mixer(16, 8);
    let expected = scene(&mut plain, 40);
    let mut open = mixer(16, 8);
    open.set_spatial_effects(true);
    let heard = scene(&mut open, 40);
    let worst = heard
        .iter()
        .zip(&expected)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f32, f32::max);
    assert!(worst < 1.0e-6, "worst difference {worst}");
}

/// The loudest sample of each block of a placed high tone, occluded from block 4.
fn occluded_envelope(amount: f32) -> Vec<f32> {
    let mut mixer = mixer(4, 0);
    mixer.set_spatial_effects(true);
    let tone = mixer
        .sound_bank_mut()
        .register_pcm(&high_tone(RATE as usize), RATE);
    mixer.play(tone, placed(1, [100.0, 0.0, 0.0]));
    let mut out = vec![0.0; BLOCK];
    (0..80)
        .map(|block| {
            if block == 4 {
                mixer.set_source_occlusion(SourceId(1), amount);
            }
            mixer.render(&mut out);
            out.iter()
                .fold(0.0_f32, |peak, sample| peak.max(sample.abs()))
        })
        .collect()
}

#[test]
fn an_occluded_source_glides_down_and_is_muffled() {
    let envelope = occluded_envelope(1.0);
    let open = envelope[3];
    assert!(open > 0.2, "open level {open}");
    // No step: the first occluded block still holds most of the sound...
    assert!(envelope[4] > open * 0.7, "{envelope:?}");
    // ...and every later block a little less, never jumping.
    for pair in envelope[4..40].windows(2) {
        assert!(pair[1] <= pair[0] + 1.0e-4, "{envelope:?}");
        assert!(pair[0] - pair[1] < open * 0.15, "{envelope:?}");
    }
    // Settled behind the wall, a high tone is nearly gone (lowpassed and quieter).
    assert!(envelope[79] < open * 0.1, "settled {}", envelope[79]);
    // Half occluded is between the two.
    let half = occluded_envelope(0.5)[79];
    assert!(half > envelope[79] && half < open * 0.7, "half {half}");
}

#[test]
fn a_voice_started_by_an_occluded_source_starts_muffled() {
    let mut mixer = mixer(4, 0);
    mixer.set_spatial_effects(true);
    let tone = mixer
        .sound_bank_mut()
        .register_pcm(&high_tone(RATE as usize), RATE);
    mixer.set_source_occlusion(SourceId(1), 1.0);
    mixer.play(tone, placed(1, [100.0, 0.0, 0.0]));
    let mut out = vec![0.0; BLOCK];
    mixer.render(&mut out);
    let peak = out[64..]
        .iter()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    assert!(peak < 0.05, "peak {peak}");
}

#[test]
fn listener_relative_sounds_are_never_occluded() {
    let mut mixer = mixer(4, 0);
    mixer.set_spatial_effects(true);
    let tone = mixer
        .sound_bank_mut()
        .register_pcm(&high_tone(RATE as usize), RATE);
    mixer.set_source_occlusion(SourceId(1), 1.0);
    mixer.play(
        tone,
        PlayRequest {
            origin: None,
            attenuation: Attenuation::None,
            ..placed(1, [0.0; 3])
        },
    );
    let mut out = vec![0.0; BLOCK];
    mixer.render(&mut out);
    assert!((out[2] - 0.5).abs() < 1.0e-6, "{}", out[2]);
}

#[test]
fn an_occluded_loop_is_muffled_too() {
    let mut mixer = mixer(1, 4);
    mixer.set_spatial_effects(true);
    let tone = mixer.sound_bank_mut().register_pcm(&high_tone(4_000), RATE);
    let mut out = vec![0.0; BLOCK];
    let mut peak = |mixer: &mut Mixer| {
        mixer.clear_loops();
        mixer.set_loop(SourceId(7), tone, [100.0, 0.0, 0.0], [0.0; 3]);
        mixer.commit_loops();
        mixer.render(&mut out);
        out.iter()
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()))
    };
    let open = peak(&mut mixer);
    mixer.set_source_occlusion(SourceId(7), 1.0);
    let settled = (0..80).map(|_| peak(&mut mixer)).last().unwrap();
    assert!(settled < open * 0.1, "open {open}, settled {settled}");
}

#[test]
fn the_reverb_stays_bounded_and_dies_away() {
    let mut effects = spatial_fx::SpatialEffects::new(RATE);
    effects.enabled = true;
    effects.set_reverb(ReverbParams {
        wet: MAX_REVERB_WET,
        size: 1.0,
    });
    let input = noise(RATE as usize * 4);
    let mut out = vec![0.0; BLOCK];
    let mut loudest = 0.0_f32;
    // Four seconds of full-scale noise in the largest room.
    for chunk in input.chunks(BLOCK / 2) {
        for ((frame, send), sample) in out
            .chunks_exact_mut(2)
            .zip(effects.send.iter_mut())
            .zip(chunk)
        {
            frame[0] = *sample;
            frame[1] = -*sample;
            *send = *sample;
        }
        effects.apply_reverb(&mut out);
        for sample in &out {
            assert!(sample.is_finite());
            loudest = loudest.max(sample.abs());
        }
    }
    assert!(loudest < 4.0, "loudest {loudest}");
    // Then silence: the tail fades out within three seconds.
    let mut tail = 0.0_f32;
    effects.send.fill(0.0);
    for block in 0..(RATE as usize * 3 / (BLOCK / 2)) {
        out.fill(0.0);
        effects.apply_reverb(&mut out);
        if block > RATE as usize * 5 / 2 / (BLOCK / 2) {
            tail = tail.max(out.iter().fold(0.0_f32, |peak, s| peak.max(s.abs())));
        }
    }
    assert!(tail < 1.0e-3, "tail {tail}");
}

#[test]
fn a_larger_room_rings_longer() {
    let ring = |size: f32| {
        let mut effects = spatial_fx::SpatialEffects::new(RATE);
        effects.enabled = true;
        effects.set_reverb(ReverbParams { wet: 1.0, size });
        let mut out = vec![0.0; BLOCK];
        // Let the room settle, then one click.
        for _ in 0..400 {
            out.fill(0.0);
            effects.apply_reverb(&mut out);
        }
        out.fill(0.0);
        effects.send[0] = 1.0;
        effects.apply_reverb(&mut out);
        effects.send[0] = 0.0;
        // Energy left half a second later.
        let mut energy = 0.0;
        for block in 0..(RATE as usize / (BLOCK / 2)) {
            out.fill(0.0);
            effects.apply_reverb(&mut out);
            if block > RATE as usize / 2 / (BLOCK / 2) {
                energy += out.iter().map(|s| s * s).sum::<f32>();
            }
        }
        energy
    };
    let (small, hall) = (ring(0.0), ring(1.0));
    assert!(hall > small * 4.0, "small {small}, hall {hall}");
}

/// The counting allocator exists in debug test builds only.
#[cfg(debug_assertions)]
#[test]
fn room_acoustics_allocate_nothing_while_mixing() {
    let mut mixer = mixer(16, 8);
    mixer.set_spatial_effects(true);
    mixer.set_reverb(ReverbParams {
        wet: 0.5,
        size: 0.5,
    });
    let tone = mixer.sound_bank_mut().register_pcm(&noise(20_000), RATE);
    let mut out = vec![0.0; BLOCK];
    let before = assert_no_alloc::violation_count();
    assert_no_alloc::assert_no_alloc(|| {
        for block in 0..40_u32 {
            mixer.set_source_occlusion(SourceId(block % 70), (block % 3) as f32 / 2.0);
            mixer.play(tone, placed(block % 5, [300.0, 0.0, 0.0]));
            mixer.clear_loops();
            mixer.set_loop(SourceId(9), tone, [0.0, 300.0, 0.0], [0.0; 3]);
            mixer.commit_loops();
            mixer.render(&mut out);
        }
    });
    assert_eq!(assert_no_alloc::violation_count(), before);
    assert!(out.iter().any(|sample| *sample != 0.0));
}

/// The mixer's cost with every voice and loop busy, room acoustics off and on:
/// `cargo test --release -p sjk-audio spatial_mix_cost -- --ignored --nocapture`.
#[test]
#[ignore = "timing; run in release"]
fn spatial_mix_cost() {
    let run = |enabled: bool| {
        let mut mixer = mixer(128, 32);
        mixer.set_spatial_effects(enabled);
        mixer.set_reverb(ReverbParams {
            wet: 0.6,
            size: 0.8,
        });
        let tone = mixer
            .sound_bank_mut()
            .register_pcm(&noise(RATE as usize * 12), RATE);
        let tones: Vec<_> = (0..32)
            .map(|_| {
                mixer
                    .sound_bank_mut()
                    .register_pcm(&noise(RATE as usize), RATE)
            })
            .collect();
        for voice in 0..128_u32 {
            mixer.set_source_occlusion(SourceId(voice), (voice % 4) as f32 / 3.0);
            mixer.play(tone, placed(voice, [300.0 + voice as f32, 50.0, 0.0]));
        }
        let mut out = vec![0.0; BLOCK];
        let blocks = 4_000;
        let started = std::time::Instant::now();
        for _ in 0..blocks {
            mixer.clear_loops();
            for (index, handle) in tones.iter().enumerate() {
                mixer.set_loop(
                    SourceId(1_000 + index as u32),
                    *handle,
                    [200.0, index as f32 * 10.0, 0.0],
                    [0.0; 3],
                );
            }
            mixer.commit_loops();
            mixer.render(&mut out);
            std::hint::black_box(&out);
        }
        started.elapsed().as_secs_f64() * 1.0e6 / f64::from(blocks)
    };
    let (off, on) = (run(false), run(true));
    let budget = (BLOCK / 2) as f64 * 1.0e6 / f64::from(RATE);
    println!(
        "128 voices + 32 loops, {BLOCK}-sample block: off {off:.1} us, on {on:.1} us \
         (budget {budget:.0} us)"
    );
}

/// The tail a short click leaves once it has ended, from `request`'s voice.
fn tail_after_click(request: PlayRequest) -> f32 {
    let mut mixer = mixer(4, 0);
    mixer.set_spatial_effects(true);
    mixer.set_reverb(ReverbParams {
        wet: 1.0,
        size: 1.0,
    });
    let click = mixer.sound_bank_mut().register_pcm(&[0.8; 32], RATE);
    let mut out = vec![0.0; BLOCK];
    // Let the room's wet level glide up first.
    for _ in 0..200 {
        mixer.render(&mut out);
    }
    mixer.play(click, request);
    mixer.render(&mut out);
    (0..40)
        .map(|_| {
            mixer.render(&mut out);
            out.iter()
                .fold(0.0_f32, |peak, sample| peak.max(sample.abs()))
        })
        .fold(0.0, f32::max)
}

#[test]
fn only_sounds_placed_in_the_world_ring_with_the_room() {
    let placed_tail = tail_after_click(placed(1, [100.0, 0.0, 0.0]));
    assert!(placed_tail > 1.0e-4, "placed tail {placed_tail}");
    // A menu cue, the announcer, the player's own listener-relative sounds: dry.
    let local = PlayRequest {
        origin: None,
        attenuation: Attenuation::None,
        ..placed(2, [0.0; 3])
    };
    assert_eq!(tail_after_click(local), 0.0);
    // Placed but global (no falloff): dry too.
    let global = PlayRequest {
        attenuation: Attenuation::None,
        ..placed(3, [100.0, 0.0, 0.0])
    };
    assert_eq!(tail_after_click(global), 0.0);
}

#[test]
fn a_long_render_with_room_acoustics_is_mixed_in_parts() {
    let mut mixer = mixer(4, 0);
    mixer.set_spatial_effects(true);
    let tone = mixer.sound_bank_mut().register_pcm(&noise(20_000), RATE);
    mixer.play(tone, placed(1, [100.0, 0.0, 0.0]));
    let mut out = vec![0.0; spatial_fx::SEND_FRAMES * 2 * 2 + 6];
    mixer.render(&mut out);
    assert!(out[spatial_fx::SEND_FRAMES * 2 + 10] != 0.0);
}
