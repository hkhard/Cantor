//! LIB-05: measure map from the Audiveris `.omr` book, so the play cursor
//! and page turns work on the original PDF page.
//!
//! The fixture is a hand-written miniature of Audiveris 5.11's format: a zip
//! with `book.xml` and `sheet#N/sheet#N.xml`, measures as `<stack left right>`
//! inside `<system>`, staff heights from `<barline><bounds>`.

use cantor_omr::measure_map::{measure_map, MeasureBox};
use std::io::Write;

/// A system: measures as (left, right) pixels, and its (top, bottom) pixels.
type System<'a> = (&'a [(u32, u32)], (u32, u32));

fn sheet(width: u32, height: u32, systems: &[System]) -> String {
    let mut x = format!(
        r#"<?xml version="1.0" ?><sheet><picture width="{width}" height="{height}"/><page id="1" measure-count="{}">"#,
        systems.iter().map(|s| s.0.len()).sum::<usize>()
    );
    let mut id = 0;
    for (si, (stacks, (y0, y1))) in systems.iter().enumerate() {
        x += &format!(r#"<system id="{}">"#, si + 1);
        for &(l, r) in stacks.iter() {
            id += 1;
            x += &format!(r#"<stack id="{id}" left="{l}" right="{r}" expected="1"/>"#);
        }
        // Two staves: top barline and bottom barline give the system height.
        x += &format!(
            r#"<barline staff="1" id="1"><bounds x="0" y="{y0}" w="3" h="40"/></barline>
               <barline staff="2" id="2"><bounds x="0" y="{}" w="3" h="40"/></barline>"#,
            y1 - 40
        );
        x += "</system>";
    }
    x + "</page></sheet>"
}

fn book(sheets: &[String]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let opt = zip::write::SimpleFileOptions::default();
        let mut b = String::from(r#"<?xml version="1.0" ?><book>"#);
        for (i, _) in sheets.iter().enumerate() {
            b += &format!(
                r#"<sheet number="{}"><input><number>{}</number></input><page id="1"/></sheet>"#,
                i + 1,
                i + 1
            );
        }
        b += "</book>";
        z.start_file("book.xml", opt).unwrap();
        z.write_all(b.as_bytes()).unwrap();
        for (i, s) in sheets.iter().enumerate() {
            z.start_file(format!("sheet#{0}/sheet#{0}.xml", i + 1), opt)
                .unwrap();
            z.write_all(s.as_bytes()).unwrap();
        }
        z.finish().unwrap();
    }
    buf.into_inner()
}

#[test]
fn lib_05_measure_boxes_in_reading_order_across_pages() {
    let p1 = sheet(
        2000,
        3000,
        &[
            (&[(400, 900), (900, 1800)], (500, 1000)),
            (&[(200, 1800)], (1200, 1700)),
        ],
    );
    let p2 = sheet(2000, 3000, &[(&[(200, 1000), (1000, 1800)], (400, 900))]);
    let map = measure_map(&book(&[p1, p2])).unwrap();
    assert_eq!(map.len(), 5, "one box per written measure");
    assert_eq!(map[0].page, 1);
    assert_eq!(map[3].page, 2);
    // Fractions of the page so the UI can draw on any zoom.
    let m = &map[1];
    assert!(
        (m.x - 0.45).abs() < 1e-9 && (m.w - 0.45).abs() < 1e-9,
        "{m:?}"
    );
    assert!((m.y - 500.0 / 3000.0).abs() < 1e-9);
    assert!((m.h - 500.0 / 3000.0).abs() < 1e-9);
    assert_eq!(
        map[2],
        MeasureBox {
            page: 1,
            x: 0.1,
            y: 0.4,
            w: 0.8,
            h: 500.0 / 3000.0
        }
    );
}

#[test]
fn lib_05_sheet_order_follows_book_not_zip_order() {
    // sheet#10 sorts before sheet#2 as text; the map must not.
    let sheets: Vec<String> = (0..10)
        .map(|_| sheet(1000, 1000, &[(&[(100, 900)], (100, 300))]))
        .collect();
    let map = measure_map(&book(&sheets)).unwrap();
    let pages: Vec<u32> = map.iter().map(|m| m.page).collect();
    assert_eq!(pages, (1..=10).collect::<Vec<_>>());
}

#[test]
fn lib_05_invalid_book_is_an_error_not_a_panic() {
    assert!(measure_map(b"not a zip").is_err());
}

/// A courtesy key/time change at the end of a line is a zero-length stack
/// (`special="CAUTIONARY"`); it is not a measure in the MusicXML.
#[test]
fn lib_05_cautionary_stack_is_not_a_measure() {
    let s = r#"<?xml version="1.0" ?><sheet><picture width="1000" height="1000"/><page id="1">
      <system id="1">
        <stack id="1" left="100" right="500"/>
        <stack id="2" left="500" right="900"/>
        <stack id="2" left="900" right="990" special="CAUTIONARY" duration="0"/>
        <barline staff="1"><bounds x="0" y="100" w="3" h="200"/></barline>
      </system></page></sheet>"#
        .to_string();
    let map = measure_map(&book(&[s])).unwrap();
    assert_eq!(map.len(), 2);
}

/// Audiveris splits a book into movements and exports `<name>.mvtN.mxl`;
/// Cantor uses movement 1, so the map must stop where movement 2 starts,
/// even in the middle of a sheet.
#[test]
fn lib_05_map_per_movement() {
    use cantor_omr::measure_map::measure_map_movements;
    let s1 = r#"<?xml version="1.0" ?><sheet><picture width="1000" height="1000"/>
      <page id="1"><system id="1"><stack id="1" left="100" right="900"/>
        <barline staff="1"><bounds x="0" y="100" w="3" h="100"/></barline></system></page>
      <page id="2"><system id="2"><stack id="1" left="100" right="500"/><stack id="2" left="500" right="900"/>
        <barline staff="1"><bounds x="0" y="600" w="3" h="100"/></barline></system></page>
      </sheet>"#;
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let opt = zip::write::SimpleFileOptions::default();
        z.start_file("book.xml", opt).unwrap();
        z.write_all(
            br#"<book><sheet number="1"><page id="1" movement-start="true"/><page id="2" movement-start="true"/></sheet></book>"#,
        )
        .unwrap();
        z.start_file("sheet#1/sheet#1.xml", opt).unwrap();
        z.write_all(s1.as_bytes()).unwrap();
        z.finish().unwrap();
    }
    let mv = measure_map_movements(&buf.into_inner()).unwrap();
    assert_eq!(mv.len(), 2);
    assert_eq!(mv[0].len(), 1);
    assert_eq!(mv[1].len(), 2);
    assert!(mv[1][0].y > 0.5);
}

/// Real book from the private samples, when present: Olaf Trygvason has
/// 62 measures in MusicXML over 4 pages.
#[test]
fn lib_05_real_book_matches_musicxml_measure_count() {
    let root: std::path::PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "scores", "out"]
        .iter()
        .collect();
    let omr = root.join(".Olaf_Trygvason.omr/Olaf_Trygvason.omr");
    let Ok(bytes) = std::fs::read(&omr) else {
        eprintln!("skipped: private sample not present");
        return;
    };
    let map = measure_map(&bytes).unwrap();
    let xml = std::fs::read(root.join("Olaf_Trygvason.mxl")).unwrap();
    let score = cantor_score::parse_score_file(&xml).unwrap();
    assert_eq!(map.len(), score.measure_count());
    assert_eq!(map.last().unwrap().page, 4);
    assert!(map
        .iter()
        .all(|b| b.w > 0.0 && b.h > 0.0 && b.x + b.w <= 1.0));
}

/// OMR that ran on preprocessed images (spreads split at the gutter, scan
/// borders cropped) gives boxes as fractions of the processed image. The
/// preprocessor records, per image, the source PDF page and the crop
/// rectangle (fractions of that page); boxes map back through it so the
/// app draws them on the PDF it shows.
#[test]
fn lib_05_cropped_images_map_back_to_pdf_pages() {
    use cantor_omr::measure_map::{uncrop, PageCrop};
    // Image 1 = left half of PDF page 1, image 2 = right half (from x 0.5),
    // image 3 = PDF page 2 with a 10% border cropped all round.
    let crops = vec![
        PageCrop {
            page: 1,
            x: 0.0,
            y: 0.0,
            w: 0.5,
            h: 1.0,
        },
        PageCrop {
            page: 1,
            x: 0.5,
            y: 0.0,
            w: 0.5,
            h: 1.0,
        },
        PageCrop {
            page: 2,
            x: 0.1,
            y: 0.1,
            w: 0.8,
            h: 0.8,
        },
    ];
    let b = |page, x, y, w, h| MeasureBox { page, x, y, w, h };
    let mut boxes = vec![
        b(1, 0.2, 0.1, 0.4, 0.2),
        b(2, 0.0, 0.5, 0.5, 0.2),
        b(3, 0.5, 0.5, 0.25, 0.25),
    ];
    uncrop(&mut boxes, &crops).unwrap();
    let close = |a: &MeasureBox, e: &MeasureBox| {
        a.page == e.page
            && [(a.x, e.x), (a.y, e.y), (a.w, e.w), (a.h, e.h)]
                .iter()
                .all(|(p, q)| (p - q).abs() < 1e-9)
    };
    assert!(
        close(&boxes[0], &b(1, 0.1, 0.1, 0.2, 0.2)),
        "{:?}",
        boxes[0]
    );
    assert!(
        close(&boxes[1], &b(1, 0.5, 0.5, 0.25, 0.2)),
        "{:?}",
        boxes[1]
    );
    assert!(
        close(&boxes[2], &b(2, 0.5, 0.5, 0.2, 0.2)),
        "{:?}",
        boxes[2]
    );
    // A box on an image the sidecar does not know is an error, not a guess.
    let mut stray = vec![b(4, 0.0, 0.0, 0.1, 0.1)];
    assert!(uncrop(&mut stray, &crops).is_err());
}

/// The sidecar is the JSON the Python preprocessors write.
#[test]
fn lib_05_crop_sidecar_parses() {
    use cantor_omr::measure_map::parse_crops;
    let c = parse_crops(br#"[{"page":3,"x":0.01,"y":0.02,"w":0.48,"h":0.95}]"#).unwrap();
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].page, 3);
    assert!((c[0].w - 0.48).abs() < 1e-12);
    assert!(parse_crops(b"nope").is_err());
}

/// Which movement of a book an Audiveris export holds: `<n>.mvtK.mxl` is
/// movement K, a single-movement book exports `<n>.mxl`, and a single-sheet
/// run (`-sheets N`) on a multi-movement sheet writes every movement to
/// `<n>.mvtnull.mxl`, so the file left behind is the last movement.
#[test]
fn lib_05_export_name_selects_movement() {
    use cantor_omr::measure_map::export_movement;
    assert_eq!(export_movement("Song.mxl", 1), Some(0));
    assert_eq!(export_movement("Song.mvt2.mxl", 3), Some(1));
    assert_eq!(export_movement("Song.mvtnull.mxl", 6), Some(5));
    assert_eq!(
        export_movement("Song.mvt4.mxl", 3),
        None,
        "no such movement"
    );
    assert_eq!(export_movement("Song.mxl", 0), None, "empty book");
}

/// Real page books of "I bröllopsgården" (private sample, when present): the
/// joined MusicXML is pages 2-6 in order, one movement each as exported.
#[test]
fn lib_05_real_page_books_follow_exported_movements() {
    use cantor_omr::measure_map::{export_movement, measure_map_movements};
    let root: std::path::PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "scores", "out"]
        .iter()
        .collect();
    let pages = root.join(".I bröllopsgården.omr/cleaned/pages");
    if !pages.exists() {
        eprintln!("skipped: private sample not present");
        return;
    }
    let mut total = 0;
    for n in 1..=6 {
        let dir = pages.join(format!("page{n}"));
        let Some(export) = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .find(|f| f.ends_with(".mxl"))
        else {
            continue; // page without music, skipped by the converter
        };
        let mv = measure_map_movements(&std::fs::read(dir.join("I bröllopsgården.omr")).unwrap())
            .unwrap();
        let k = export_movement(&export, mv.len()).unwrap();
        let xml = std::fs::read(dir.join(&export)).unwrap();
        let score = cantor_score::parse_score_file(&xml).unwrap();
        assert_eq!(mv[k].len(), score.measure_count(), "page {n} ({export})");
        total += mv[k].len();
    }
    let joined = std::fs::read(root.join("I bröllopsgården.musicxml")).unwrap();
    assert_eq!(
        total,
        cantor_score::parse_score_file(&joined)
            .unwrap()
            .measure_count()
    );
}

/// Real spread-split book of "Do you hear what I hear" (private sample, when
/// present): movement 2 is the exported MusicXML; through the crop sidecar
/// its boxes land on the PDF the app shows (landscape screenshot spreads,
/// odd pages; even pages are repeats dropped by split_spreads.py).
#[test]
fn lib_05_real_split_spreads_map_to_pdf_pages() {
    use cantor_omr::measure_map::{measure_map_movements, parse_crops, uncrop};
    let root: std::path::PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "scores", "out"]
        .iter()
        .collect();
    let split = root.join(".Do you her what I hear TTBB.omr/split");
    let (Ok(book), Ok(json)) = (
        std::fs::read(split.join("Do you her what I hear TTBB.omr")),
        std::fs::read(split.join("Do you her what I hear TTBB.pages.json")),
    ) else {
        eprintln!("skipped: private sample not present");
        return;
    };
    let mut mv = measure_map_movements(&book).unwrap();
    let xml = std::fs::read(root.join("Do you her what I hear TTBB.mxl")).unwrap();
    let measures = cantor_score::parse_score_file(&xml)
        .unwrap()
        .measure_count();
    assert_eq!(mv[1].len(), measures);
    uncrop(&mut mv[1], &parse_crops(&json).unwrap()).unwrap();
    assert!(mv[1].iter().all(|b| b.page % 2 == 1 && b.page <= 17));
    assert!(mv[1]
        .iter()
        .all(|b| b.x > 0.1 && b.x + b.w < 0.9 && b.y > 0.1 && b.y + b.h < 0.9));
    // Page 1 holds the left and right half of the first spread.
    let p1: Vec<_> = mv[1].iter().filter(|b| b.page == 1).collect();
    assert!(p1.iter().any(|b| b.x < 0.45) && p1.iter().any(|b| b.x > 0.5));
}
