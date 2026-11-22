//! PLAY-05 (per-voice channels inside one staff) and PLAY-09 (instrument per
//! part): every choir voice gets its own mixer channel and a distinct timbre.

use cantor_score::parse_musicxml;
use cantor_timeline::build_timemap;
use cantor_timeline::channels::{plan_channels, Role};

/// One part: (name, explicit 1-based midi-program, voices as (voice number, pitches as MIDI)).
type PartSpec<'a> = (&'a str, Option<u8>, Vec<(u32, Vec<u8>)>);

fn step(p: u8) -> (&'static str, i32, i32) {
    const N: [(&str, i32); 12] = [
        ("C", 0),
        ("C", 1),
        ("D", 0),
        ("D", 1),
        ("E", 0),
        ("F", 0),
        ("F", 1),
        ("G", 0),
        ("G", 1),
        ("A", 0),
        ("A", 1),
        ("B", 0),
    ];
    let (s, alt) = N[(p % 12) as usize];
    (s, alt, p as i32 / 12 - 1)
}

fn xml(parts: &[PartSpec]) -> String {
    let mut pl = String::new();
    let mut body = String::new();
    for (i, (name, prog, voices)) in parts.iter().enumerate() {
        pl += &format!(r#"<score-part id="P{i}"><part-name>{name}</part-name>"#);
        if let Some(p) = prog {
            pl += &format!(
                r#"<midi-instrument id="P{i}-I1"><midi-program>{p}</midi-program></midi-instrument>"#
            );
        }
        pl += "</score-part>";
        body += &format!(
            r#"<part id="P{i}"><measure number="1"><attributes><divisions>1</divisions><time><beats>16</beats><beat-type>4</beat-type></time></attributes>"#
        );
        for (k, (v, pitches)) in voices.iter().enumerate() {
            if k > 0 {
                let prev = voices[k - 1].1.len();
                body += &format!("<backup><duration>{prev}</duration></backup>");
            }
            for &p in pitches {
                let (s, alt, oct) = step(p);
                body += &format!(
                    "<note><pitch><step>{s}</step><alter>{alt}</alter><octave>{oct}</octave></pitch><duration>1</duration><voice>{v}</voice></note>"
                );
            }
        }
        body += "</measure></part>";
    }
    format!(
        r#"<?xml version="1.0"?><score-partwise version="4.0"><part-list>{pl}</part-list>{body}</score-partwise>"#
    )
}

fn plan(parts: &[PartSpec]) -> cantor_timeline::channels::ChannelPlan {
    let s = parse_musicxml(xml(parts).as_bytes()).unwrap();
    let tm = build_timemap(&s);
    plan_channels(&s, &tm)
}

fn line(p: u8, n: usize) -> Vec<u8> {
    vec![p; n]
}

fn roles(pl: &cantor_timeline::channels::ChannelPlan) -> Vec<(Role, u8)> {
    pl.channels.iter().map(|c| (c.role, c.number)).collect()
}

fn assert_distinct_programs(pl: &cantor_timeline::channels::ChannelPlan) {
    let mut progs: Vec<u8> = pl.channels.iter().map(|c| c.program).collect();
    progs.sort_unstable();
    let n = progs.len();
    progs.dedup();
    assert_eq!(progs.len(), n, "every channel has its own timbre: {pl:?}");
}

#[test]
fn play_05_satb_on_two_staves_split_into_four_channels() {
    // OMR output: two parts named "Voice", program 53 (voice oohs), two voices each.
    let pl = plan(&[
        ("Voice", Some(54), vec![(1, line(72, 8)), (2, line(67, 8))]),
        ("Voice", Some(54), vec![(1, line(57, 8)), (2, line(48, 8))]),
    ]);
    assert_eq!(
        roles(&pl),
        [
            (Role::Soprano, 0),
            (Role::Alto, 0),
            (Role::Tenor, 0),
            (Role::Bass, 0)
        ]
    );
    let labels: Vec<_> = pl.channels.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["Soprano", "Alto", "Tenor", "Bass"]);
    assert_distinct_programs(&pl);
    // Notes route to their voice's channel.
    let s = parse_musicxml(
        xml(&[
            ("Voice", Some(54), vec![(1, line(72, 8)), (2, line(67, 8))]),
            ("Voice", Some(54), vec![(1, line(57, 8)), (2, line(48, 8))]),
        ])
        .as_bytes(),
    )
    .unwrap();
    let tm = build_timemap(&s);
    for n in &tm.notes {
        let ch = &pl.channels[pl.channel_of(n)];
        let want = match n.pitch {
            72 => Role::Soprano,
            67 => Role::Alto,
            57 => Role::Tenor,
            _ => Role::Bass,
        };
        assert_eq!(ch.role, want);
    }
}

#[test]
fn play_09_ttbb_from_part_names_including_ocr_noise() {
    let pl = plan(&[
        ("Tenor 1", Some(54), vec![(1, line(62, 8))]),
        ("TenorZ", Some(54), vec![(1, line(58, 8))]),
        ("Bas 1", Some(54), vec![(1, line(53, 8))]),
        ("Bas 2", Some(54), vec![(1, line(46, 8))]),
    ]);
    assert_eq!(
        roles(&pl),
        [
            (Role::Tenor, 1),
            (Role::Tenor, 2),
            (Role::Bass, 1),
            (Role::Bass, 2)
        ]
    );
    let labels: Vec<_> = pl.channels.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["Tenor 1", "Tenor 2", "Bass 1", "Bass 2"]);
    assert_distinct_programs(&pl);
}

#[test]
fn play_09_male_choir_recognised_from_range_when_parts_are_unnamed() {
    // "Do you hear" TTBB: medians 64, 60, 56, 55, all named "Voice".
    let pl = plan(&[
        ("Voice", Some(54), vec![(1, line(64, 8))]),
        ("Voice", Some(54), vec![(1, line(60, 8))]),
        ("Voice", Some(54), vec![(1, line(56, 8))]),
        ("Voice", Some(54), vec![(1, line(55, 8))]),
        ("Piano", Some(1), vec![(1, line(72, 16)), (5, line(48, 16))]),
    ]);
    assert_eq!(
        roles(&pl),
        [
            (Role::Tenor, 1),
            (Role::Tenor, 2),
            (Role::Bass, 1),
            (Role::Bass, 2),
            (Role::Piano, 0)
        ]
    );
    assert_eq!(pl.channels[4].program, 0, "piano stays a piano");
    assert_distinct_programs(&pl);
}

#[test]
fn play_09_female_choir_sa() {
    let pl = plan(&[
        ("Voice", None, vec![(1, line(72, 8)), (2, line(69, 8))]),
        ("Voice", None, vec![(1, line(65, 8)), (2, line(62, 8))]),
    ]);
    assert_eq!(
        roles(&pl),
        [
            (Role::Soprano, 1),
            (Role::Soprano, 2),
            (Role::Alto, 1),
            (Role::Alto, 2)
        ]
    );
    assert_distinct_programs(&pl);
}

#[test]
fn play_05_minor_ocr_voice_merges_into_nearest_channel() {
    // Voice 3 has 2 notes out of 18: an OMR artefact, not a separate singer.
    let pl = plan(&[(
        "Voice",
        Some(54),
        vec![(1, line(72, 8)), (2, line(65, 8)), (3, line(64, 2))],
    )]);
    assert_eq!(pl.channels.len(), 2);
    let s = parse_musicxml(
        xml(&[(
            "Voice",
            Some(54),
            vec![(1, line(72, 8)), (2, line(65, 8)), (3, line(64, 2))],
        )])
        .as_bytes(),
    )
    .unwrap();
    let tm = build_timemap(&s);
    let v3 = tm.notes.iter().find(|n| n.voice == 3).unwrap();
    assert_eq!(pl.channel_of(v3), 1, "joins the lower (alto) line");
}

#[test]
fn play_09_explicit_instrument_respected_and_named_instruments_kept() {
    let pl = plan(&[
        ("Violin", Some(41), vec![(1, line(76, 8))]),
        ("Flute", None, vec![(1, line(79, 8))]),
    ]);
    assert_eq!(pl.channels[0].program, 40);
    assert_eq!(pl.channels[0].role, Role::Instrument);
    assert_eq!(pl.channels[1].program, 73);
}

#[test]
fn play_03_voices_are_panned_apart() {
    let pl = plan(&[
        ("Voice", Some(54), vec![(1, line(72, 8)), (2, line(67, 8))]),
        ("Voice", Some(54), vec![(1, line(57, 8)), (2, line(48, 8))]),
    ]);
    let pans: Vec<f32> = pl.channels.iter().map(|c| c.pan).collect();
    for w in pans.windows(2) {
        assert!(w[0] != w[1], "{pans:?}");
    }
    assert!(
        pans.iter().all(|p| p.abs() <= 0.5),
        "keep it a choir, not hard panned"
    );
}

#[test]
fn play_09_choir_type_from_title_overrides_range() {
    use cantor_timeline::channels::{choir_from_title, plan_channels_for, Choir};
    assert_eq!(
        choir_from_title("En glad calypso om våren TTBB"),
        Some(Choir::Male)
    );
    assert_eq!(choir_from_title("Stad i ljus Manskör 2"), Some(Choir::Male));
    assert_eq!(choir_from_title("Ave verum SSA"), Some(Choir::Female));
    assert_eq!(choir_from_title("Damkör: Vårsång"), Some(Choir::Female));
    assert_eq!(choir_from_title("Gloria SATB"), Some(Choir::Mixed));
    assert_eq!(choir_from_title("Christmas Day - F dur"), None);
    // OMR lost the treble-8 clef on the top staff: it reads an octave high.
    let parts: Vec<PartSpec> = vec![
        ("Voice", Some(54), vec![(1, line(76, 8))]),
        ("Voice", Some(54), vec![(1, line(62, 8))]),
        ("Voice", Some(54), vec![(1, line(50, 8))]),
    ];
    let s = parse_musicxml(xml(&parts).as_bytes()).unwrap();
    let tm = build_timemap(&s);
    let pl = plan_channels_for(&s, &tm, Some(Choir::Male));
    assert!(pl
        .channels
        .iter()
        .all(|c| matches!(c.role, Role::Tenor | Role::Bass)));
}

#[test]
fn play_09_many_lines_of_one_role_still_sound_different() {
    let pl = plan(&[
        ("Tenor", Some(54), vec![(1, line(62, 8)), (2, line(58, 8))]),
        ("Tenor", Some(54), vec![(1, line(60, 8)), (2, line(57, 8))]),
        ("Bass", Some(54), vec![(1, line(53, 8)), (2, line(50, 8))]),
        ("Bass", Some(54), vec![(1, line(48, 8)), (2, line(45, 8))]),
    ]);
    assert_eq!(pl.channels.len(), 8);
    assert_distinct_programs(&pl);
}
