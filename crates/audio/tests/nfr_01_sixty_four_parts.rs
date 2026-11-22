//! NFR-01: no dropouts at 64 parts. A synthetic 64-part score (eighth notes in
//! every part, 120 bpm) is rendered for 10 s through the real SoundFont synth
//! in 128-frame blocks at 48 kHz. In release builds the run and the p99 block
//! must render in under half the block duration (2x margin), with at most 1%
//! of blocks over that (OS preemption); debug builds only print the
//! realtime factor (unoptimised code is several times slower).
//!
//! Measured on the desktop, not on an iPad A14: the device check stays a
//! manual item. Skipped when the bundled SoundFont is absent.

use cantor_audio::{Engine, SoundFontSynth};
use cantor_score::parse_musicxml;
use cantor_timeline::build_timemap;
use cantor_timeline::channels::plan_channels;
use std::time::{Duration, Instant};

const SR: u32 = 48_000;
const BLOCK: usize = 128;
const PARTS: usize = 64;

fn soundfont() -> Option<std::path::PathBuf> {
    let p: std::path::PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "assets",
        "sf2",
        "GeneralUser-GS.sf2",
    ]
    .iter()
    .collect();
    p.exists().then_some(p)
}

/// 64 parts, 6 measures of 4/4 at 120 bpm (12 s), eighth notes moving
/// through a scale, each part in its own register.
fn score_xml() -> String {
    let steps = ["C", "D", "E", "F", "G", "A", "B"];
    let mut list = String::new();
    let mut parts = String::new();
    for p in 0..PARTS {
        list.push_str(&format!(
            r#"<score-part id="P{p}"><part-name>Voice {p}</part-name></score-part>"#
        ));
        let octave = 2 + p % 4;
        parts.push_str(&format!(r#"<part id="P{p}">"#));
        for m in 1..=6 {
            parts.push_str(&format!(r#"<measure number="{m}">"#));
            if m == 1 {
                parts.push_str(r#"<attributes><divisions>2</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><direction><sound tempo="120"/></direction>"#);
            }
            for k in 0..8 {
                let s = steps[(p + m * 8 + k) % 7];
                parts.push_str(&format!(
                    r#"<note><pitch><step>{s}</step><octave>{octave}</octave></pitch><duration>1</duration></note>"#
                ));
            }
            parts.push_str("</measure>");
        }
        parts.push_str("</part>");
    }
    format!(
        r#"<?xml version="1.0"?><score-partwise version="4.0"><part-list>{list}</part-list>{parts}</score-partwise>"#
    )
}

#[test]
fn nfr_01_sixty_four_parts_render_faster_than_realtime_in_128_frame_blocks() {
    let Some(sf) = soundfont() else {
        eprintln!("skipped: SoundFont not present");
        return;
    };
    let s = parse_musicxml(score_xml().as_bytes()).unwrap();
    assert_eq!(s.parts.len(), PARTS);
    let tm = build_timemap(&s);
    assert_eq!(tm.notes.len(), PARTS * 6 * 8);
    let plan = plan_channels(&s, &tm);
    let synth = SoundFontSynth::open(&sf, SR).unwrap();
    let mut e = Engine::new(synth, &tm, &plan, SR);
    e.play();

    let blocks = 10 * SR as usize / BLOCK;
    let budget = Duration::from_secs_f64(BLOCK as f64 / SR as f64); // 2.67 ms
    let (mut l, mut r) = (vec![0.0f32; BLOCK], vec![0.0f32; BLOCK]);
    let mut times = Vec::with_capacity(blocks);
    let mut peak = 0.0f32;
    let total = Instant::now();
    for _ in 0..blocks {
        let t = Instant::now();
        e.render(&mut l, &mut r);
        times.push(t.elapsed());
        peak = l.iter().chain(&r).fold(peak, |p, x| p.max(x.abs()));
    }
    let total = total.elapsed();
    assert!(peak > 0.01, "audible: peak {peak}");
    times.sort();
    let p99 = times[times.len() * 99 / 100];
    let max = *times.last().unwrap();
    let mean = total / blocks as u32;
    let rtf = 10.0 / total.as_secs_f64();
    let over = times.iter().filter(|t| **t > budget / 2).count();
    println!(
        "NFR-01 {PARTS} parts, {blocks} blocks of {BLOCK} frames (budget {:.3} ms): \
         mean {:.3} ms, p99 {:.3} ms, max {:.3} ms, {over} blocks over half budget, \
         realtime factor {rtf:.1}x ({})",
        budget.as_secs_f64() * 1e3,
        mean.as_secs_f64() * 1e3,
        p99.as_secs_f64() * 1e3,
        max.as_secs_f64() * 1e3,
        if cfg!(debug_assertions) {
            "debug, not asserted"
        } else {
            "release"
        }
    );
    if !cfg!(debug_assertions) {
        // 2x margin on the whole run and on the 99th-percentile block. Wall
        // time of single blocks also includes OS preemption on a loaded test
        // machine (seen: scattered 1.5-12 ms blocks at load average 27, not
        // tied to note onsets), so up to 1% of blocks may exceed half the
        // budget; a real CPU-bound overload would push p99 over first.
        assert!(rtf >= 2.0, "realtime factor {rtf:.2}");
        assert!(p99 < budget / 2, "p99 block {p99:?} vs {:?}", budget / 2);
        assert!(
            over * 100 <= blocks,
            "{over} of {blocks} blocks over half budget"
        );
    } else {
        // Loose bound: catches a gross regression without timing flakes.
        assert!(rtf >= 0.25, "debug realtime factor {rtf:.2}");
    }
}
