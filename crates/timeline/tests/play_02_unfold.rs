//! PLAY-02: repeats, voltas, D.C./D.S./Coda/Fine unfold into the expected
//! measure order. Each fixture is a one-part score of empty 1/4 measures whose
//! navigation is the only thing under test.

use cantor_score::parse_musicxml;
use cantor_timeline::unfold;

/// Build a score from measure snippets; `{}` per measure is the extra XML
/// (barlines, directions) placed inside it.
fn score(measures: &[&str]) -> Vec<usize> {
    let mut xml = String::from(
        r#"<?xml version="1.0"?><score-partwise version="4.0">
        <part-list><score-part id="P1"><part-name>V</part-name></score-part></part-list><part id="P1">"#,
    );
    for (i, extra) in measures.iter().enumerate() {
        let attrs = if i == 0 {
            "<attributes><divisions>1</divisions><time><beats>1</beats><beat-type>4</beat-type></time></attributes>"
        } else {
            ""
        };
        xml.push_str(&format!(
            r#"<measure number="{n}">{attrs}{extra}<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration></note></measure>"#,
            n = i + 1
        ));
    }
    xml.push_str("</part></score-partwise>");
    // Return 1-based measure numbers for readable assertions.
    unfold(&parse_musicxml(xml.as_bytes()).unwrap())
        .into_iter()
        .map(|i| i + 1)
        .collect()
}

const FWD: &str = r#"<barline location="left"><repeat direction="forward"/></barline>"#;
const BWD: &str = r#"<barline location="right"><repeat direction="backward"/></barline>"#;
fn bwd_times(n: u32) -> String {
    format!(r#"<barline location="right"><repeat direction="backward" times="{n}"/></barline>"#)
}
fn ending_start(nums: &str) -> String {
    format!(r#"<barline location="left"><ending number="{nums}" type="start"/></barline>"#)
}
fn ending_stop(nums: &str) -> String {
    format!(
        r#"<barline location="right"><ending number="{nums}" type="stop"/><repeat direction="backward"/></barline>"#
    )
}
fn ending_discontinue(nums: &str) -> String {
    format!(r#"<barline location="right"><ending number="{nums}" type="discontinue"/></barline>"#)
}
fn sound(attr: &str) -> String {
    format!(
        r#"<direction><direction-type><words>x</words></direction-type><sound {attr}/></direction>"#
    )
}

#[test]
fn play_02_no_repeats_is_written_order() {
    assert_eq!(score(&["", "", ""]), [1, 2, 3]);
}

#[test]
fn play_02_simple_repeat_from_start() {
    assert_eq!(score(&["", BWD, ""]), [1, 2, 1, 2, 3]);
}

#[test]
fn play_02_simple_repeat_with_forward_barline() {
    assert_eq!(score(&["", FWD, BWD, ""]), [1, 2, 3, 2, 3, 4]);
}

#[test]
fn play_02_repeat_times_attribute() {
    assert_eq!(score(&[FWD, &bwd_times(3)]), [1, 2, 1, 2, 1, 2]);
}

#[test]
fn play_02_two_sections_each_repeated() {
    assert_eq!(score(&[FWD, BWD, FWD, BWD]), [1, 2, 1, 2, 3, 4, 3, 4]);
}

#[test]
fn play_02_voltas_1_2() {
    // |: 1 2 |1. 3 :|2. 4 | 5
    let e1 = format!("{}{}", ending_start("1"), ending_stop("1"));
    let e2 = format!("{}{}", ending_start("2"), ending_discontinue("2"));
    assert_eq!(score(&[FWD, "", &e1, &e2, ""]), [1, 2, 3, 1, 2, 4, 5]);
}

#[test]
fn play_02_volta_spanning_two_measures_and_shared_numbers() {
    // |: 1 |1.,2. 2 3 :|3. 4
    let e12a = ending_start("1, 2");
    let e3 = format!("{}{}", ending_start("3"), ending_discontinue("3"));
    let bwd3 = r#"<barline location="right"><ending number="1, 2" type="stop"/><repeat direction="backward" times="3"/></barline>"#;
    assert_eq!(score(&[FWD, &e12a, bwd3, &e3]), [1, 2, 3, 1, 2, 3, 1, 4]);
}

#[test]
fn play_02_da_capo_al_fine() {
    // 1 2(Fine) 3 4(D.C.)  ->  1 2 3 4 1 2
    assert_eq!(
        score(&["", &sound(r#"fine="yes""#), "", &sound(r#"dacapo="yes""#)]),
        [1, 2, 3, 4, 1, 2]
    );
}

#[test]
fn play_02_dal_segno_al_coda_unfolds_in_order() {
    // 1 2(segno) 3(to coda) 4(D.S.) 5(coda) 6 -> 1 2 3 4 2 3 5 6
    assert_eq!(
        score(&[
            "",
            &sound(r#"segno="s1""#),
            &sound(r#"tocoda="c1""#),
            &sound(r#"dalsegno="s1""#),
            &sound(r#"coda="c1""#),
            "",
        ]),
        [1, 2, 3, 4, 2, 3, 5, 6]
    );
}

#[test]
fn play_02_repeats_not_taken_again_after_da_capo() {
    // 1 2:|(Fine) 3(D.C. al Fine) -> 1 2 1 2 3, then D.C. plays 1 2 without the repeat
    assert_eq!(
        score(&[
            "",
            &format!("{BWD}{}", sound(r#"fine="yes""#)),
            &sound(r#"dacapo="yes""#)
        ]),
        [1, 2, 1, 2, 3, 1, 2]
    );
}

#[test]
fn play_02_da_capo_skips_repeat_inside_section() {
    // |: 1 2 :| 3(D.C.) -> 1 2 1 2 3 1 2 3 (no repeat the second time, no Fine)
    assert_eq!(
        score(&[FWD, BWD, &sound(r#"dacapo="yes""#)]),
        [1, 2, 1, 2, 3, 1, 2, 3]
    );
}

#[test]
fn play_02_volta_count_inferred_from_ending_numbers() {
    // |: 1 |1.,2. 2 :|3. 3 with no times attribute: endings imply 3 passes.
    let e12 = format!(
        r#"{}<barline location="right"><ending number="1, 2" type="stop"/><repeat direction="backward"/></barline>"#,
        ending_start("1, 2")
    );
    let e3 = format!("{}{}", ending_start("3"), ending_discontinue("3"));
    assert_eq!(score(&[FWD, &e12, &e3]), [1, 2, 1, 2, 1, 3]);
}

#[test]
fn play_02_dangling_jumps_do_not_loop_forever() {
    // D.S. without a segno and a coda marker without a target are ignored.
    assert_eq!(score(&["", &sound(r#"dalsegno="nope""#), ""]), [1, 2, 3]);
    assert_eq!(score(&[&sound(r#"tocoda="x""#), ""]), [1, 2]);
}

#[test]
fn play_02_unfolding_is_bounded() {
    // Pathological: backward repeat times=1000 must be capped.
    let n = score(&[FWD, &bwd_times(1000)]).len();
    assert!(n <= 2 * 64, "capped, got {n}");
}

#[test]
fn play_02_unfolded_duration_equals_sum_of_visited_measures() {
    let s = parse_musicxml(
        br#"<?xml version="1.0"?><score-partwise><part-list><score-part id="P1"><part-name>V</part-name></score-part></part-list>
        <part id="P1">
          <measure number="1"><attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes>
            <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration></note></measure>
          <measure number="2"><note><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration></note>
            <barline location="right"><repeat direction="backward"/></barline></measure>
          <measure number="3"><note><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration></note></measure>
        </part></score-partwise>"#,
    )
    .unwrap();
    let tm = cantor_timeline::build_timemap(&s);
    assert_eq!(tm.duration_ticks, 5 * 2 * cantor_score::PPQ);
    let pitches: Vec<u8> = tm.notes.iter().map(|n| n.pitch).collect();
    assert_eq!(pitches, [60, 62, 60, 62, 64]);
    // Each note records which written measure it came from.
    let measures: Vec<usize> = tm.notes.iter().map(|n| n.measure).collect();
    assert_eq!(measures, [0, 1, 0, 1, 2]);
    assert_eq!(tm.measure_order, [0, 1, 0, 1, 2]);
}

/// "Only You" (Vince Clarke, arr. HKH), bars 8-22, as exported by MuseScore:
/// two backward repeats share one forward repeat at bar 8. Voltas 1,3,5 /
/// 2,4,6 end the first; 1,2,3 (printed "3,5") / 4 (printed "7") the second.
/// Bars 1-7 are a plain |: 3 4 5 |1. 6 :|2. 7 intro.
fn only_you(tail: &[&str]) -> Vec<usize> {
    let mut bars: Vec<String> = vec![String::new(), String::new()];
    let e1 = format!("{}{}", ending_start("1"), ending_stop("1"));
    let e2 = format!("{}{}", ending_start("2"), ending_discontinue("2"));
    bars.extend([FWD.to_string(), String::new(), String::new(), e1, e2]);
    // bar 8: |: segno
    bars.push(format!("{FWD}{}", sound(r#"segno="segno""#)));
    bars.extend([String::new(), String::new()]);
    // bar 11: 1,3,5 To Coda :| x6
    bars.push(format!(
        r#"{}{}<barline location="right"><ending number="1, 3, 5" type="stop"/><repeat direction="backward" times="6"/></barline>"#,
        ending_start("1, 3, 5"),
        sound(r#"tocoda="codab""#)
    ));
    // bar 12: 2,4,6 (no repeat sign of its own), bars 13-19 plain
    bars.push(format!(
        "{}{}",
        ending_start("2, 4, 6"),
        ending_discontinue("2, 4, 6")
    ));
    bars.extend((13..=19).map(|_| String::new()));
    // bar 20: 1,2,3 :| x3
    bars.push(format!(
        r#"{}<barline location="right"><ending number="1, 2, 3" type="stop"/><repeat direction="backward" times="3"/></barline>"#,
        ending_start("1, 2, 3")
    ));
    // bars 21-22: volta 4
    bars.push(ending_start("4"));
    bars.push(ending_discontinue("4"));
    bars.extend(tail.iter().map(|s| s.to_string()));
    let refs: Vec<&str> = bars.iter().map(String::as_str).collect();
    score(&refs)
}

#[test]
fn play_02_second_repeat_without_forward_sign_returns_to_the_last_forward_sign() {
    // A backward repeat goes back to the last forward repeat sign, even when
    // another backward repeat came after it (MuseScore and common practice).
    // Was: bar 20 jumped back to bar 13 (after bar 12) and looped there.
    let order = only_you(&[""]);
    let s = |v: std::ops::RangeInclusive<usize>| v.collect::<Vec<_>>();
    let mut want = vec![];
    want.extend(s(1..=6));
    want.extend(s(3..=5));
    want.extend(s(7..=11)); // pass 1: verse, volta 1,3,5
    want.extend(s(8..=10));
    want.extend(s(12..=20)); // pass 2: volta 2,4,6, on to the 1,2,3 volta
    want.extend(s(8..=11)); // pass 3 (back to bar 8)
    want.extend(s(8..=10));
    want.extend(s(12..=19)); // pass 4: volta 2,4,6, skip volta 1,2,3
    want.extend(s(21..=23)); // volta 4, then on
    assert_eq!(order, want);
}

#[test]
fn play_02_to_coda_before_a_volta_group_jumps_after_the_dal_segno() {
    // Only You, as MuseScore plays it (scores/only-you-vince-clarke.mid):
    // D.S. al Coda at bar 24 (here), segno at 8, "To Coda" printed before the
    // 1,3,5 volta (its <sound tocoda> sits in the volta's first bar, 11), and
    // the coda sign written at bar 27 although the coda starts right after
    // the D.S. (bar 25, "no more!"). After the D.S.: 8 9 10, then the coda.
    let ds = sound(r#"dalsegno="segno""#);
    let coda = sound(r#"coda="codab""#);
    let tail = ["", &ds, "", "", &coda, ""]; // bars 23..28
    let order = only_you(&tail);
    let after_volta4: Vec<usize> = order.iter().copied().skip_while(|&b| b != 21).collect();
    assert_eq!(after_volta4, [21, 22, 23, 24, 8, 9, 10, 25, 26, 27, 28]);
}

#[test]
fn play_02_dal_segno_al_coda_with_reachable_to_coda_is_still_taken() {
    // The usual case is unchanged: To Coda outside any volta.
    assert_eq!(
        score(&[
            "",
            &sound(r#"segno="s1""#),
            &sound(r#"tocoda="c1""#),
            &sound(r#"dalsegno="s1""#),
            &sound(r#"coda="c1""#),
        ]),
        [1, 2, 3, 4, 2, 3, 5]
    );
}
