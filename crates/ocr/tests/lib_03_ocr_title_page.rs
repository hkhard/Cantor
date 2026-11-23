//! LIB-03 (metadata) from the printed page: on-device text recognition
//! (Apple Vision, Swedish) and a title-page reader that fills title,
//! composer, arranger and lyricist for PDF-only scores.

use cantor_ocr::{guess_metadata, Line};

fn line(text: &str, x: f32, y: f32, w: f32, h: f32) -> Line {
    Line {
        text: text.into(),
        x,
        y,
        w,
        h,
        confidence: 1.0,
    }
}

#[test]
fn lib_03_title_is_the_biggest_text_near_the_top() {
    let lines = vec![
        line("Vårvindar friska", 0.29, 0.06, 0.38, 0.035),
        line("för blandad kör", 0.40, 0.12, 0.15, 0.018),
        line("Text: Julia Nyberg", 0.07, 0.16, 0.15, 0.014),
        line("Svensk folkmelodi", 0.78, 0.15, 0.15, 0.014),
        line("Arr. Hugo Alfvén", 0.79, 0.17, 0.14, 0.014),
        line(
            "Vår-vin-dar fris-ka, le-ka och hvis-ka",
            0.11,
            0.27,
            0.24,
            0.012,
        ),
        line(
            "© 2026 Cantor test fixture. Fri att använda.",
            0.07,
            0.97,
            0.22,
            0.01,
        ),
    ];
    let m = guess_metadata(&lines);
    assert_eq!(m.title, "Vårvindar friska");
    assert_eq!(m.subtitle, "för blandad kör");
    assert_eq!(m.arranger, "Hugo Alfvén");
    assert_eq!(m.lyricist, "Julia Nyberg");
    // A right-aligned credit that is not "Arr." is the composer line.
    assert_eq!(m.composer, "Svensk folkmelodi");
    assert_eq!(m.choir, Some("SATB"));
}

#[test]
fn lib_03_credit_patterns_swedish_and_english() {
    let cases = [
        ("Musik: Lars-Erik Larsson", "composer", "Lars-Erik Larsson"),
        ("Music by John Rutter", "composer", "John Rutter"),
        ("arr. Gunnar Eriksson", "arranger", "Gunnar Eriksson"),
        ("Arrangemang: Anders Öhrwall", "arranger", "Anders Öhrwall"),
        ("Bearb. Robert Sund", "arranger", "Robert Sund"),
        (
            "Words by Christina Rossetti",
            "lyricist",
            "Christina Rossetti",
        ),
        ("Ord: Britt G. Hallqvist", "lyricist", "Britt G. Hallqvist"),
        ("Text och musik: Evert Taube", "composer", "Evert Taube"),
    ];
    for (text, field, want) in cases {
        let m = guess_metadata(&[
            line("Titel", 0.4, 0.05, 0.2, 0.03),
            line(text, 0.7, 0.12, 0.2, 0.014),
        ]);
        let got = match field {
            "composer" => &m.composer,
            "arranger" => &m.arranger,
            _ => &m.lyricist,
        };
        assert_eq!(got, want, "{text}");
    }
    // "Text och musik" credits both.
    let m = guess_metadata(&[
        line("Titel", 0.4, 0.05, 0.2, 0.03),
        line("Text och musik: Evert Taube", 0.7, 0.12, 0.2, 0.014),
    ]);
    assert_eq!(m.lyricist, "Evert Taube");
}

#[test]
fn lib_03_composer_with_years_and_choir_types() {
    let m = guess_metadata(&[
        line("Ave Maria", 0.4, 0.05, 0.2, 0.03),
        line("TTBB", 0.45, 0.10, 0.05, 0.015),
        line("Jacob Arcadelt (1507–1568)", 0.70, 0.12, 0.25, 0.014),
    ]);
    assert_eq!(m.composer, "Jacob Arcadelt");
    assert_eq!(m.choir, Some("TTBB"));
    for (t, c) in [
        ("för manskör", "TTBB"),
        ("Damkör SSA", "SSAA"),
        ("for mixed choir", "SATB"),
        ("SSAA a cappella", "SSAA"),
    ] {
        let m = guess_metadata(&[
            line("X", 0.4, 0.05, 0.2, 0.03),
            line(t, 0.4, 0.1, 0.2, 0.015),
        ]);
        assert_eq!(m.choir, Some(c), "{t}");
    }
}

#[test]
fn lib_03_noise_is_not_a_title() {
    // Tempo marks, page numbers, copyright lines and single letters near the
    // top are skipped; the title is the biggest remaining line.
    let m = guess_metadata(&[
        line("2", 0.9, 0.02, 0.01, 0.04),
        line("Andante ♩ = 72", 0.07, 0.04, 0.12, 0.04),
        line("Copyright 1998 Gehrmans", 0.3, 0.01, 0.3, 0.04),
        line("S", 0.05, 0.3, 0.01, 0.04),
        line("Stad i ljus", 0.35, 0.07, 0.25, 0.03),
    ]);
    assert_eq!(m.title, "Stad i ljus");
    assert_eq!(guess_metadata(&[]).title, "");
}

/// The real recogniser on a public fixture page (rendered by
/// `fixtures/ocr/README.md`). Apple platforms only.
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[test]
fn lib_03_vision_reads_swedish_title_page() {
    let png = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/ocr/title_page_sv.png"
    ))
    .unwrap();
    let t = std::time::Instant::now();
    let lines = cantor_ocr::recognize(&png, &["sv-SE", "en-US"]).expect("vision");
    let ms = t.elapsed().as_millis();
    let all: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
    // Swedish letters come through (å, ö, é), not "Varvindar"/"for".
    assert!(all.contains(&"Vårvindar friska"), "{all:?}");
    assert!(all.iter().any(|l| l.contains("för blandad kör")), "{all:?}");
    assert!(all.iter().any(|l| l.contains("Alfvén")), "{all:?}");
    // Boxes are top-left based, 0..1.
    let title = lines.iter().find(|l| l.text == "Vårvindar friska").unwrap();
    assert!(title.y < 0.12 && title.h > 0.02, "{title:?}");
    let m = guess_metadata(&lines);
    assert_eq!(m.title, "Vårvindar friska");
    assert_eq!(m.arranger, "Hugo Alfvén");
    assert_eq!(m.lyricist, "Julia Nyberg");
    eprintln!("vision: {} lines in {ms} ms", lines.len());
}

#[test]
fn lib_03_bad_image_is_an_error_not_a_crash() {
    let r = cantor_ocr::recognize(b"not an image", &["sv-SE"]);
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    assert!(r.is_err());
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    assert!(matches!(r, Err(cantor_ocr::OcrError::Unsupported)));
}

#[test]
fn lib_03_layouts_seen_in_real_choir_scores() {
    // Role word alone, name on the next line; names continued with "and".
    let m = guess_metadata(&[
        line("Do You Hear What I Hear", 0.227, 0.161, 0.5, 0.021),
        line("For TTBB* and Piano", 0.235, 0.182, 0.5, 0.010),
        line("Arranged by", 0.164, 0.207, 0.1, 0.010),
        line("Words and Music by", 0.422, 0.207, 0.15, 0.010),
        line("ROBERT STERLING", 0.164, 0.215, 0.12, 0.010),
        line("NOEL REGNEY", 0.430, 0.215, 0.1, 0.012),
        line("and GLORIA SHAYNE", 0.410, 0.223, 0.14, 0.014),
        line("Flowing, mysterious (- = ca. 92)", 0.206, 0.230, 0.2, 0.014),
    ]);
    assert_eq!(m.title, "Do You Hear What I Hear");
    assert_eq!(m.arranger, "Robert Sterling");
    assert_eq!(m.composer, "Noel Regney and Gloria Shayne");
    assert_eq!(m.choir, Some("TTBB"));
    // Comma continuation and ALL CAPS.
    let m = guess_metadata(&[
        line("CHRISTMAS DAY", 0.352, 0.060, 0.3, 0.023),
        line(
            "Words and Music by MICHAEL W. SMITH,",
            0.596,
            0.092,
            0.35,
            0.018,
        ),
        line("CINDY MORGAN and WES KING", 0.626, 0.109, 0.3, 0.013),
        line("Moderately slow", 0.197, 0.126, 0.1, 0.016),
    ]);
    assert_eq!(m.title, "Christmas Day");
    assert_eq!(m.composer, "Michael W. Smith, Cindy Morgan and Wes King");
    // Bare names: left lyricist, right composer; music below is ignored.
    let m = guess_metadata(&[
        line("Olaf Trygvason", 0.393, 0.086, 0.2, 0.025),
        line("Bjørnstjerne Bjørnson", 0.113, 0.112, 0.18, 0.013),
        line("F. A. Reissiger", 0.772, 0.112, 0.12, 0.013),
        line("Tenor I", 0.111, 0.160, 0.06, 0.019),
        line("l slepai.", 0.519, 0.156, 0.06, 0.042),
    ]);
    assert_eq!(
        (m.title.as_str(), m.composer.as_str(), m.lyricist.as_str()),
        ("Olaf Trygvason", "F. A. Reissiger", "Bjørnstjerne Bjørnson")
    );
    // Two right-hand names: composer then lyricist.
    let m = guess_metadata(&[
        line("Island", 0.428, 0.047, 0.1, 0.028),
        line("Henrik Möller", 0.809, 0.102, 0.12, 0.012),
        line("Albert Ulrik Bååth", 0.778, 0.118, 0.15, 0.013),
        line("Moderato", 0.126, 0.154, 0.08, 0.013),
        line("stolt som i sa - gan", 0.524, 0.220, 0.15, 0.016),
    ]);
    assert_eq!(
        (m.composer.as_str(), m.lyricist.as_str(), m.choir),
        ("Henrik Möller", "Albert Ulrik Bååth", None)
    );
    // A centred name under the title is the composer; "arr. af" (Swedish "by").
    let m = guess_metadata(&[
        line("En glad calypso om våren", 0.298, 0.067, 0.4, 0.028),
        line("Olle Adolphsson", 0.447, 0.095, 0.12, 0.014),
        line("Arr.: Hans Wickberg", 0.817, 0.106, 0.15, 0.015),
        line("1. Jag", 0.223, 0.199, 0.05, 0.015),
    ]);
    assert_eq!(
        (
            m.composer.as_str(),
            m.arranger.as_str(),
            m.subtitle.as_str()
        ),
        ("Olle Adolphsson", "Hans Wickberg", "")
    );
    let m = guess_metadata(&[
        line("Fredmans :", 0.247, 0.042, 0.2, 0.023),
        line("Epistel Nº 72.", 0.492, 0.042, 0.25, 0.031),
        line("arr. af J. P. Cronhamn.", 0.591, 0.087, 0.3, 0.021),
        line("Andante.", 0.240, 0.101, 0.1, 0.025),
    ]);
    assert_eq!(
        (m.title.as_str(), m.arranger.as_str()),
        ("Fredmans Epistel Nº 72.", "J. P. Cronhamn")
    );
    // A composer printed level with the title, with years.
    let m = guess_metadata(&[
        line("I bröllopsgården", 0.092, 0.066, 0.3, 0.025),
        line("August Söderman (1832-1876)", 0.708, 0.065, 0.25, 0.017),
        line("Text: R.Gustafsson", 0.782, 0.076, 0.15, 0.015),
        line("ur \"Ett bondbröllop\"", 0.096, 0.087, 0.2, 0.023),
        line("d=126", 0.211, 0.190, 0.05, 0.016),
    ]);
    assert_eq!(
        (
            m.composer.as_str(),
            m.lyricist.as_str(),
            m.subtitle.as_str()
        ),
        ("August Söderman", "R.Gustafsson", "ur \"Ett bondbröllop\"")
    );
}
