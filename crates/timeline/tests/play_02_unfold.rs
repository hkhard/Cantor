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
