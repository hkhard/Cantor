//! LIB-07 / LIB-08: forScore .4SC score and .4SS setlist import.
//!
//! Synthetic fixtures are built in-test (committable). Tests against the real
//! samples in `scores/` (private, git-ignored) run only when the files exist.

use cantor_library::forscore::{
    parse_4sc, parse_4ss, ForscoreError, PageView, Point, Rect, SetlistEntry,
};
use flate2::{write::GzEncoder, Compression};
use plist::{Dictionary, Uid, Value};
use std::io::Write;
use std::path::PathBuf;

const PDF: &[u8] = b"%PDF-1.4\n%fake\n";

fn gz_bplist(v: &Value) -> Vec<u8> {
    let mut raw = Vec::new();
    v.to_writer_binary(&mut raw).unwrap();
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(&raw).unwrap();
    gz.finish().unwrap()
}

fn score_dict(file: &str, title: &str) -> Dictionary {
    let mut d = Dictionary::new();
    d.insert("pdfData".into(), Value::Data(PDF.to_vec()));
    d.insert(format!("{file}|title"), title.into());
    d.insert(format!("{file}|1|rect"), "{{19, 54}, {743, 971.5}}".into());
    d.insert(format!("{file}|1|zoom"), Value::Real(1.25));
    d.insert(format!("{file}|1|offset"), "{6.5, 37.75}".into());
    d.insert(format!("{file}|1|trOffset"), "{19.5, 55.25}".into());
    d.insert(format!("{file}|2|rect"), "{{0, 0}, {768, 1004}}".into());
    d.insert(format!("{file}|composer"), "Gustav Holst".into()); // unknown today, must survive
    d.insert(format!("{file}|3|futureField"), Value::Integer(7.into()));
    d
}

#[test]
fn lib_07_parses_synthetic_4sc_metadata_pdf_and_pages() {
    let bytes = gz_bplist(&Value::Dictionary(score_dict("Holst.pdf", "In the bleak")));
    let s = parse_4sc(&bytes).expect("parses");

    assert_eq!(s.file_name, "Holst.pdf");
    assert_eq!(s.title.as_deref(), Some("In the bleak"));
    assert_eq!(s.pdf, PDF);
    assert_eq!(
        s.pages.get(&1),
        Some(&PageView {
            rect: Some(Rect {
                x: 19.0,
                y: 54.0,
                w: 743.0,
                h: 971.5
            }),
            zoom: Some(1.25),
            offset: Some(Point { x: 6.5, y: 37.75 }),
            tr_offset: Some(Point { x: 19.5, y: 55.25 }),
        })
    );
    assert_eq!(
        s.pages[&2].rect,
        Some(Rect {
            x: 0.0,
            y: 0.0,
            w: 768.0,
            h: 1004.0
        })
    );
}

#[test]
fn lib_07_unknown_fields_preserved() {
    let bytes = gz_bplist(&Value::Dictionary(score_dict("Holst.pdf", "T")));
    let s = parse_4sc(&bytes).unwrap();
    assert_eq!(
        s.unknown.get("Holst.pdf|composer"),
        Some(&Value::String("Gustav Holst".into()))
    );
    assert_eq!(
        s.unknown.get("Holst.pdf|3|futureField"),
        Some(&Value::Integer(7.into()))
    );
    // Known fields are not duplicated into `unknown`.
    assert!(!s.unknown.contains_key("pdfData"));
    assert!(!s.unknown.contains_key("Holst.pdf|1|rect"));
}

#[test]
fn lib_07_invalid_input_is_error_not_panic() {
    assert!(matches!(parse_4sc(b""), Err(ForscoreError::NotGzip)));
    assert!(matches!(
        parse_4sc(b"plain text"),
        Err(ForscoreError::NotGzip)
    ));
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(b"not a plist").unwrap();
    assert!(matches!(
        parse_4sc(&gz.finish().unwrap()),
        Err(ForscoreError::Plist(_))
    ));
    // Valid plist without pdfData
    let bytes = gz_bplist(&Value::Dictionary(Dictionary::new()));
    assert!(matches!(parse_4sc(&bytes), Err(ForscoreError::MissingPdf)));
}

/// Builds an NSKeyedArchiver archive with the same shape as forScore's .4ss.
fn keyed_archive(items: &[(&str, &str, &str)], files: &[Dictionary]) -> Value {
    let mut objects: Vec<Value> = vec!["$null".into()];
    let mut push = |v: Value| {
        objects.push(v);
        Uid::new(objects.len() as u64 - 1)
    };
    fn class(name: &str, parents: &[&str]) -> Value {
        let mut d = Dictionary::new();
        d.insert("$classname".into(), name.into());
        let mut cl = vec![Value::from(name)];
        cl.extend(parents.iter().map(|p| Value::from(*p)));
        d.insert("$classes".into(), Value::Array(cl));
        Value::Dictionary(d)
    }
    let c_mdict = push(class("NSMutableDictionary", &["NSDictionary", "NSObject"]));
    let c_dict = push(class("NSDictionary", &["NSObject"]));
    let c_marr = push(class("NSMutableArray", &["NSArray", "NSObject"]));
    let c_arr = push(class("NSArray", &["NSObject"]));

    let dict_obj = |push: &mut dyn FnMut(Value) -> Uid, entries: Vec<(String, Value)>, cls: Uid| {
        let mut ks = Vec::new();
        let mut vs = Vec::new();
        for (k, v) in entries {
            ks.push(Value::Uid(push(Value::String(k))));
            vs.push(Value::Uid(push(v)));
        }
        let mut d = Dictionary::new();
        d.insert("NS.keys".into(), Value::Array(ks));
        d.insert("NS.objects".into(), Value::Array(vs));
        d.insert("$class".into(), Value::Uid(cls));
        push(Value::Dictionary(d))
    };

    let mut item_uids = Vec::new();
    for (title, path, id) in items {
        let u = dict_obj(
            &mut push,
            vec![
                ("Title".into(), (*title).into()),
                ("FilePath".into(), (*path).into()),
                ("Identifier".into(), (*id).into()),
            ],
            c_mdict,
        );
        item_uids.push(Value::Uid(u));
    }
    let mut file_uids = Vec::new();
    for f in files {
        let entries = f.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        file_uids.push(Value::Uid(dict_obj(&mut push, entries, c_dict)));
    }
    let arr = |push: &mut dyn FnMut(Value) -> Uid, items: Vec<Value>, cls: Uid| {
        let mut d = Dictionary::new();
        d.insert("NS.objects".into(), Value::Array(items));
        d.insert("$class".into(), Value::Uid(cls));
        push(Value::Dictionary(d))
    };
    let setlist = arr(&mut push, item_uids, c_marr);
    let files_u = arr(&mut push, file_uids, c_arr);

    let mut top = Dictionary::new();
    top.insert("setlist".into(), Value::Uid(setlist));
    top.insert("files".into(), Value::Uid(files_u));
    let mut root = Dictionary::new();
    root.insert("$version".into(), Value::Integer(100000.into()));
    root.insert("$archiver".into(), "NSKeyedArchiver".into());
    root.insert("$top".into(), Value::Dictionary(top));
    root.insert("$objects".into(), Value::Array(objects));
    Value::Dictionary(root)
}

#[test]
fn lib_08_setlist_order_and_placeholders() {
    let archive = keyed_archive(
        &[
            ("B piece", "b.pdf", "ID-B"),
            ("Missing", "gone.pdf", "ID-X"),
            ("A piece", "a.pdf", "ID-A"),
        ],
        &[
            score_dict("a.pdf", "A piece"),
            score_dict("b.pdf", "B piece"),
        ],
    );
    let set = parse_4ss(&gz_bplist(&archive)).expect("parses");

    let titles: Vec<_> = set.items.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(
        titles,
        ["B piece", "Missing", "A piece"],
        "setlist order kept"
    );
    assert_eq!(set.items[0].identifier, "ID-B");
    match &set.items[0].entry {
        SetlistEntry::Score(s) => assert_eq!(s.file_name, "b.pdf"),
        other => panic!("expected score, got {other:?}"),
    }
    assert!(
        matches!(set.items[1].entry, SetlistEntry::Placeholder),
        "no file => placeholder"
    );
    match &set.items[2].entry {
        SetlistEntry::Score(s) => assert_eq!(s.title.as_deref(), Some("A piece")),
        other => panic!("expected score, got {other:?}"),
    }
}

#[test]
fn lib_08_rejects_non_keyed_archive() {
    let bytes = gz_bplist(&Value::Dictionary(Dictionary::new()));
    assert!(matches!(
        parse_4ss(&bytes),
        Err(ForscoreError::NotKeyedArchive)
    ));
}

// ---- Real samples (private, optional) ----

/// forScore stores names in NFD (decomposed, as Apple file systems do). Cantor
/// normalises titles to NFC so Swedish search (LIB-04) matches typed text.
#[test]
fn lib_08_titles_normalised_to_nfc_but_file_names_kept_exact() {
    let nfd_title = "Mansko\u{308}r";
    let nfd_file = "Mansko\u{308}r.pdf";
    let archive = keyed_archive(
        &[(nfd_title, nfd_file, "ID")],
        &[score_dict(nfd_file, nfd_title)],
    );
    let set = parse_4ss(&gz_bplist(&archive)).unwrap();
    let it = &set.items[0];
    assert_eq!(it.title, "Manskör");
    assert_eq!(
        it.file_path, nfd_file,
        "file path kept byte-exact for round trip"
    );
    match &it.entry {
        SetlistEntry::Score(s) => {
            assert_eq!(s.title.as_deref(), Some("Manskör"));
            assert_eq!(s.file_name, nfd_file);
        }
        SetlistEntry::Placeholder => panic!("NFD file name must still join"),
    }
}

/// Private samples live in `<repo>/scores/` or in `$CANTOR_SAMPLES` (used by the
/// self-hosted CI runner, whose checkout does not contain git-ignored files).
fn sample(name: &str) -> Option<Vec<u8>> {
    let dir: PathBuf = match std::env::var_os("CANTOR_SAMPLES") {
        Some(d) => d.into(),
        None => [env!("CARGO_MANIFEST_DIR"), "..", "..", "scores"]
            .iter()
            .collect(),
    };
    std::fs::read(dir.join(name)).ok()
}

#[test]
fn lib_07_every_example_imports_christmas_day() {
    let Some(bytes) = sample("Christmas Day - F dur.4sc") else {
        eprintln!("skipped: private sample not present");
        return;
    };
    let s = parse_4sc(&bytes).unwrap();
    assert_eq!(s.file_name, "Christmas Day - F dur.pdf");
    assert_eq!(s.title.as_deref(), Some("Christmas Day - F dur"));
    assert!(s.pdf.starts_with(b"%PDF-"));
    assert_eq!(
        s.pages.len(),
        8,
        "forScore stores view state for 8 page slots"
    );
    let r = s.pages[&1].rect.expect("page 1 rect");
    // LIB-07 tolerance: positions within 2 pt.
    for (got, want) in [(r.x, 19.0), (r.y, 54.0), (r.w, 743.0), (r.h, 971.3)] {
        assert!((got - want).abs() < 2.0, "{got} vs {want}");
    }
    assert!(
        s.unknown.is_empty(),
        "no fields beyond the documented ones: {:?}",
        s.unknown.keys()
    );
}

#[test]
fn lib_08_every_example_imports_cmb_40() {
    let Some(bytes) = sample("CMB 40.4ss") else {
        eprintln!("skipped: private sample not present");
        return;
    };
    let set = parse_4ss(&bytes).unwrap();
    assert_eq!(set.items.len(), 10);
    assert_eq!(set.items[0].title, "kunglilje0001");
    assert_eq!(set.items[9].title, "Stad i ljus Manskör 2");
    for it in &set.items {
        match &it.entry {
            SetlistEntry::Score(s) => assert!(s.pdf.starts_with(b"%PDF-"), "{}", it.title),
            SetlistEntry::Placeholder => panic!("{} should have a file", it.title),
        }
    }
}
