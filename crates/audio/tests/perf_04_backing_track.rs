//! PERF-04 (P1): backing tracks with time-stretch and page turns on the track
//! timeline. Acceptance: turn markers fire within 50 ms.

use cantor_audio::{
    decode_audio, decode_file, export_wav, resample, stretch, AudioClip, BackingTrack, Engine,
    Synth, TurnMarker, BACKING_TRACK_LABEL,
};
use cantor_timeline::channels::ChannelPlan;
use cantor_timeline::Timemap;
use std::f32::consts::TAU;
use std::path::PathBuf;

const SR: u32 = 48_000;

/// Silent synth: only the backing track is heard.
struct Silent;
impl Synth for Silent {
    fn message(&mut self, _: u8, _: u8, _: u8, _: u8) {}
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) {
        l.fill(0.0);
        r.fill(0.0);
    }
}

fn sine(freq: f32, sr: u32, secs: f64, amp: f32) -> Vec<f32> {
    let n = (secs * sr as f64).round() as usize;
    (0..n)
        .map(|i| amp * (TAU * freq * i as f32 / sr as f32).sin())
        .collect()
}

/// Frequency from the mean period between rising zero crossings (linear
/// interpolation), skipping `skip` samples at each end.
fn freq_of(x: &[f32], sr: u32, skip: usize) -> f64 {
    let x = &x[skip..x.len() - skip];
    let mut cross = Vec::new();
    for i in 1..x.len() {
        if x[i - 1] < 0.0 && x[i] >= 0.0 {
            let t = (i - 1) as f64 + (-x[i - 1] / (x[i] - x[i - 1])) as f64;
            cross.push(t);
        }
    }
    assert!(cross.len() > 10, "too few crossings");
    let period = (cross[cross.len() - 1] - cross[0]) / (cross.len() - 1) as f64;
    sr as f64 / period
}

fn wav_bytes(sr: u32, ch: u16, samples: &[f32]) -> Vec<u8> {
    let spec = hound::WavSpec {
        channels: ch,
        sample_rate: sr,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cur = std::io::Cursor::new(Vec::new());
    let mut w = hound::WavWriter::new(&mut cur, spec).unwrap();
    for &s in samples {
        w.write_sample((s * 32767.0) as i16).unwrap();
    }
    w.finalize().unwrap();
    cur.into_inner()
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/audio")
        .join(name)
}

/// PDF-only score: empty timemap, the recording defines the length.
fn track_only(clip: AudioClip) -> Engine<Silent> {
    let mut e = Engine::new(Silent, &Timemap::default(), &ChannelPlan::default(), SR);
    e.set_backing_track(BackingTrack::new(clip));
    e
}

fn render_secs(e: &mut Engine<Silent>, secs: f64, block: usize) -> (Vec<f32>, Vec<f32>) {
    let total = (secs * SR as f64).round() as usize;
    let (mut ol, mut or) = (Vec::new(), Vec::new());
    let (mut l, mut r) = (vec![0.0; block], vec![0.0; block]);
    while ol.len() < total {
        let n = block.min(total - ol.len());
        e.render(&mut l[..n], &mut r[..n]);
        ol.extend_from_slice(&l[..n]);
        or.extend_from_slice(&r[..n]);
    }
    (ol, or)
}

// ---------------------------------------------------------------- decode

#[test]
fn perf_04_decode_wav_stereo_keeps_channels_and_length() {
    let l = sine(440.0, SR, 0.5, 0.5);
    let r = sine(660.0, SR, 0.5, 0.5);
    let inter: Vec<f32> = l.iter().zip(&r).flat_map(|(a, b)| [*a, *b]).collect();
    let clip = decode_audio(&wav_bytes(SR, 2, &inter), Some("wav"), SR).unwrap();
    assert_eq!(clip.sample_rate, SR);
    assert_eq!(clip.frames(), l.len());
    assert!((freq_of(&clip.left, SR, 100) - 440.0).abs() < 1.0);
    assert!((freq_of(&clip.right, SR, 100) - 660.0).abs() < 1.0);
}

#[test]
fn perf_04_decode_wav_mono_and_resamples_to_engine_rate() {
    let x = sine(440.0, 44_100, 1.0, 0.5);
    let clip = decode_audio(&wav_bytes(44_100, 1, &x), None, SR).unwrap();
    assert_eq!(clip.sample_rate, SR);
    assert!(
        (clip.frames() as i64 - SR as i64).abs() <= 1,
        "{}",
        clip.frames()
    );
    assert_eq!(clip.left, clip.right, "mono on both sides");
    let f = freq_of(&clip.left, SR, 2000);
    assert!((f - 440.0).abs() < 0.5, "{f}");
    // Level kept (no gain change in the passband).
    let peak = clip.left[2000..clip.frames() - 2000]
        .iter()
        .fold(0.0f32, |m, v| m.max(v.abs()));
    assert!((peak - 0.5).abs() < 0.01, "{peak}");
}

#[test]
fn perf_04_resample_is_band_limited() {
    // 20 kHz at 48 kHz cannot pass to 22.05 kHz (Nyquist 11 kHz): removed.
    let x = sine(20_000.0, 48_000, 0.5, 0.5);
    let y = resample(&x, 48_000, 22_050);
    let rms =
        (y[500..y.len() - 500].iter().map(|v| v * v).sum::<f32>() / (y.len() - 1000) as f32).sqrt();
    assert!(rms < 0.01, "aliasing left {rms}");
}

#[test]
fn perf_04_decode_m4a_aac_fixture() {
    let clip = decode_file(&fixture("sine440_0.5s.m4a"), SR).unwrap();
    assert_eq!(clip.sample_rate, SR);
    let secs = clip.duration_sec();
    assert!((0.45..0.6).contains(&secs), "{secs}");
    let f = freq_of(&clip.left, SR, 4000);
    assert!((f - 440.0).abs() < 4.4, "{f}");
}

#[test]
fn perf_04_decode_flac_fixture() {
    let clip = decode_file(&fixture("sine440_0.5s.flac"), 44_100).unwrap();
    assert_eq!(clip.frames(), 22_050);
    let f = freq_of(&clip.left, 44_100, 100);
    assert!((f - 440.0).abs() < 0.5, "{f}");
}

#[test]
fn perf_04_decode_rejects_garbage() {
    assert!(decode_audio(b"not audio at all", None, SR).is_err());
}

// --------------------------------------------------------------- stretch

#[test]
fn perf_04_stretch_keeps_pitch_and_scales_length() {
    let clip = AudioClip::mono(SR, sine(440.0, SR, 2.0, 0.5));
    for scale in [0.25, 0.5, 0.75, 1.0, 1.5, 2.0] {
        let out = stretch(&clip, scale);
        let want = clip.frames() as f64 / scale;
        assert!(
            (out.frames() as f64 - want).abs() <= 1024.0,
            "{scale}: {} vs {want}",
            out.frames()
        );
        let f = freq_of(&out.left, SR, 4000);
        assert!((f - 440.0).abs() < 4.4, "scale {scale}: {f} Hz");
        // No dropouts: the level stays steady after the first grain.
        let mid = &out.left[4000..out.frames() - 4000];
        let peak = mid.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!((0.4..0.6).contains(&peak), "scale {scale}: peak {peak}");
    }
}

#[test]
fn perf_04_stretch_at_one_is_exact() {
    let clip = AudioClip::mono(SR, sine(440.0, SR, 0.3, 0.5));
    assert_eq!(stretch(&clip, 1.0), clip);
}

// ------------------------------------------------------------------ sync

/// A tone burst at each whole second: the onset shows where the track is.
fn bursts(secs: f64) -> AudioClip {
    let n = (secs * SR as f64) as usize;
    let x = (0..n)
        .map(|i| {
            let t = i as f64 / SR as f64;
            if t.fract() < 0.1 && t >= 0.999 {
                0.5 * (TAU * 440.0 * i as f32 / SR as f32).sin()
            } else {
                0.0
            }
        })
        .collect();
    AudioClip::mono(SR, x)
}

/// Output seconds of each burst onset.
fn onsets(x: &[f32]) -> Vec<f64> {
    let mut out = Vec::new();
    let mut quiet = SR as usize;
    for (i, v) in x.iter().enumerate() {
        if v.abs() > 0.05 {
            if quiet > SR as usize / 4 {
                out.push(i as f64 / SR as f64);
            }
            quiet = 0;
        } else {
            quiet += 1;
        }
    }
    out
}

#[test]
fn perf_04_track_alone_defines_duration_and_plays_exact() {
    let ramp: Vec<f32> = (0..SR as usize).map(|i| i as f32 / SR as f32).collect();
    let mut e = track_only(AudioClip::mono(SR, ramp.clone()));
    assert!((e.duration() - 1.0).abs() < 1e-9);
    e.play();
    let (l, _) = render_secs(&mut e, 0.5, 333);
    assert_eq!(&l[..], &ramp[..l.len()], "sample-exact at tempo 1");
    render_secs(&mut e, 0.6, 333);
    assert!(!e.is_playing(), "stops at the end of the track");
}

#[test]
fn perf_04_offset_and_seek_follow_the_score_clock() {
    let ramp: Vec<f32> = (0..2 * SR as usize).map(|i| i as f32 / SR as f32).collect();
    let mut e = track_only(AudioClip::mono(SR, ramp));
    e.set_backing_offset(0.25);
    assert!((e.duration() - 1.75).abs() < 1e-9);
    e.seek(0.5);
    e.play();
    let (l, _) = render_secs(&mut e, 0.01, 64);
    assert!(
        (l[0] - 0.75).abs() < 1e-4,
        "track = score + offset: {}",
        l[0]
    );
    assert!((e.track_position() - 0.76).abs() < 1e-6);
    // Changing the offset while playing jumps the track, not the score.
    e.set_backing_offset(0.0);
    let (l, _) = render_secs(&mut e, 0.01, 64);
    assert!((l[0] - 0.51).abs() < 1e-4, "{}", l[0]);
}

#[test]
fn perf_04_sync_follows_tempo_and_loop() {
    for scale in [0.5, 1.0, 1.5] {
        let mut e = track_only(bursts(6.0));
        e.set_tempo_scale(scale);
        e.play();
        let (l, _) = render_secs(&mut e, 4.0 / scale + 0.2, 512);
        let got = onsets(&l);
        for (k, t) in got.iter().take(4).enumerate() {
            let want = (k + 1) as f64 / scale;
            assert!(
                (t - want).abs() < 0.025,
                "tempo {scale}: burst {k} at {t}, want {want}"
            );
        }
        assert_eq!(got.len(), 4, "tempo {scale}: {got:?}");
    }
    // Loop 0.5..2.5 s: after the wrap the 1 s burst comes again.
    let mut e = track_only(bursts(6.0));
    e.set_loop_seconds(Some((0.5, 2.5)));
    e.seek(0.5);
    e.play();
    let (l, _) = render_secs(&mut e, 4.2, 480);
    let got = onsets(&l);
    let want = [0.5, 1.5, 2.5, 3.5];
    assert_eq!(got.len(), 4, "{got:?}");
    for (t, w) in got.iter().zip(want) {
        assert!((t - w).abs() < 0.005, "{got:?}");
    }
}

#[test]
fn perf_04_backing_is_a_mixer_channel() {
    let mut e = track_only(AudioClip::mono(SR, vec![0.5; SR as usize]));
    let ch = e.backing_channel().unwrap();
    assert_eq!(ch, e.mixer().len() - 1);
    assert_eq!(e.channel_label(ch), Some(BACKING_TRACK_LABEL));
    assert_eq!(BACKING_TRACK_LABEL, "Backing track");
    e.play();
    let (l, _) = render_secs(&mut e, 0.01, 128);
    assert!((l[10] - 0.5).abs() < 1e-6);
    e.mixer_mut().set_volume_db(ch, -6.0206);
    let (l, _) = render_secs(&mut e, 0.01, 128);
    assert!((l[10] - 0.25).abs() < 1e-3, "{}", l[10]);
    e.mixer_mut().set_mute(ch, true);
    let (l, _) = render_secs(&mut e, 0.01, 128);
    assert_eq!(l[10], 0.0);
    e.mixer_mut().set_mute(ch, false);
    e.clear_backing_track();
    assert_eq!(e.mixer().len(), 0);
}

#[test]
fn perf_04_count_in_delays_the_track() {
    let mut e = track_only(AudioClip::mono(SR, vec![0.5; SR as usize]));
    e.set_count_in(true);
    e.play(); // no beats: default count-in of one 0.5 s beat
    let (l, _) = render_secs(&mut e, 0.6, 256);
    assert_eq!(l[SR as usize / 4], 0.0, "silent during the count-in");
    assert!((l[SR as usize / 2 + 10] - 0.5).abs() < 1e-6);
}

// --------------------------------------------------------------- markers

/// Deterministic pseudo-random block sizes.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self, lo: usize, hi: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        lo + (self.0 >> 33) as usize % (hi - lo)
    }
}

/// Render in random blocks; record (marker, fired score sec, output sec of
/// the block end it was seen in).
fn run_markers(e: &mut Engine<Silent>, secs: f64, rng: &mut Lcg) -> Vec<(TurnMarker, f64, f64)> {
    let total = (secs * SR as f64) as usize;
    let mut done = 0;
    let mut out = Vec::new();
    let mut buf = vec![0.0; 4096];
    let mut buf2 = vec![0.0; 4096];
    while done < total {
        let n = rng.next(16, 2048).min(total - done);
        e.render(&mut buf[..n], &mut buf2[..n]);
        done += n;
        for (m, at) in e.take_fired_markers() {
            out.push((m, at, done as f64 / SR as f64));
        }
    }
    out
}

fn markers() -> Vec<TurnMarker> {
    [1.0, 2.37, 3.5, 5.01]
        .iter()
        .enumerate()
        .map(|(i, &at_sec)| TurnMarker {
            at_sec,
            page: i as u32 + 2,
        })
        .collect()
}

#[test]
fn perf_04_markers_fire_within_50ms_at_any_tempo_and_block_size() {
    let mut rng = Lcg(7);
    for scale in [0.5, 1.0, 1.5] {
        for _ in 0..5 {
            let mut e = track_only(AudioClip::mono(SR, vec![0.0; 6 * SR as usize]));
            e.set_turn_markers(markers());
            e.set_tempo_scale(scale);
            e.play();
            let fired = run_markers(&mut e, 6.0 / scale, &mut rng);
            assert_eq!(fired.len(), 4, "tempo {scale}");
            for ((m, at, seen), want) in fired.iter().zip(markers()) {
                assert_eq!(*m, want);
                assert!(
                    (at - want.at_sec).abs() < 1.0 / SR as f64 * 2.0,
                    "frame-accurate"
                );
                // Seen by the UI no later than one block after it is due.
                let due_out = want.at_sec / scale;
                assert!(
                    *seen >= due_out - 1e-9 && *seen - due_out < 0.050,
                    "{seen} vs {due_out}"
                );
            }
        }
    }
}

#[test]
fn perf_04_markers_after_seek_and_loop() {
    let mut rng = Lcg(99);
    let mut e = track_only(AudioClip::mono(SR, vec![0.0; 6 * SR as usize]));
    e.set_turn_markers(markers());
    e.seek(2.0);
    e.play();
    let fired = run_markers(&mut e, 2.0, &mut rng);
    let pages: Vec<u32> = fired.iter().map(|f| f.0.page).collect();
    assert_eq!(pages, [3, 4], "markers before the seek point do not fire");
    for (m, at, _) in &fired {
        assert!((at - m.at_sec).abs() < 0.001);
    }
    // Seeking back re-arms the earlier markers.
    e.seek(0.9);
    let fired = run_markers(&mut e, 0.2, &mut rng);
    assert_eq!(fired.len(), 1);
    assert!((fired[0].1 - 1.0).abs() < 0.001);

    // Loop 2.0..4.0 s: 2.37 and 3.5 fire each pass, 5.01 never.
    e.set_loop_seconds(Some((2.0, 4.0)));
    e.seek(2.0);
    let fired = run_markers(&mut e, 6.05, &mut rng);
    let pages: Vec<u32> = fired.iter().map(|f| f.0.page).collect();
    assert_eq!(pages, [3, 4, 3, 4, 3, 4]);
    for (m, at, _) in &fired {
        assert!((at - m.at_sec).abs() < 0.001, "{at}");
    }
}

#[test]
fn perf_04_markers_with_offset_and_page_at() {
    let mut rng = Lcg(3);
    let mut e = track_only(AudioClip::mono(SR, vec![0.0; 6 * SR as usize]));
    e.set_turn_markers(markers());
    e.set_backing_offset(0.5); // track second 1.0 = score second 0.5
    e.play();
    let fired = run_markers(&mut e, 1.0, &mut rng);
    assert_eq!(fired.len(), 1);
    assert!((fired[0].1 - 0.5).abs() < 0.001, "{}", fired[0].1);
    let t = e.backing_track().unwrap();
    assert_eq!(t.page_at(0.5), None);
    assert_eq!(t.page_at(2.4), Some(3));
}

// ---------------------------------------------------------------- export

#[test]
fn perf_04_export_includes_backing_track() {
    let mut e = track_only(AudioClip::mono(SR, sine(440.0, SR, 1.0, 0.5)));
    e.set_tempo_scale(0.5);
    let wav = export_wav(&mut e, 0.0);
    let data = &wav[44..];
    let s: Vec<f32> = data
        .chunks(4)
        .map(|x| i16::from_le_bytes([x[0], x[1]]) as f32 / 32767.0)
        .collect();
    let secs = s.len() as f64 / SR as f64;
    assert!((secs - 2.0).abs() < 0.010, "{secs}");
    let f = freq_of(&s, SR, 4000);
    assert!((f - 440.0).abs() < 4.4, "{f}");
    let peak = s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    assert!(peak > 0.4, "{peak}");
    assert!(e.take_fired_markers().is_empty());
}
