//! NFR-13 robustness: damaged or unsupported files give an error, never a
//! panic, and never leave half-imported files in the library.
//!
//! Deterministic byte mutations (no fuzz dependency): empty, random bytes,
//! truncations, bit flips, wrong extension and zips whose header claims a
//! huge unpacked size, for every importer: MusicXML, MXL, PDF, forScore
//! `.4sc`/`.4ss`, `.mscz` sidecar, `.cantorset` bundles and backups.

use cantor_library::store::{SetItem, Setlist, Store};
use cantor_library::{backup, bundle, forscore, import};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cantor-nfr13-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Every file under `dir` (relative paths), to compare before and after.
fn snapshot(dir: &Path) -> BTreeSet<String> {
    fn walk(base: &Path, d: &Path, out: &mut BTreeSet<String>) {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.insert(p.strip_prefix(base).unwrap().display().to_string());
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(dir, dir, &mut out);
    out
}

/// xorshift: the same "random" bytes on every run.
fn noise(seed: u64, n: usize) -> Vec<u8> {
    let mut x = seed | 1;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect()
}

/// Damaged variants of a valid file: truncations and bit flips.
fn mutations(good: &[u8]) -> Vec<Vec<u8>> {
    let mut out = vec![
        Vec::new(),
        noise(7, 4096),
        good[..1.min(good.len())].to_vec(),
    ];
    for cut in [2, 4, 10, 30, 64, 100, 200] {
        let n = good.len() * cut / 256;
        out.push(good[..n].to_vec());
    }
    for k in 0..24u64 {
        let mut b = good.to_vec();
        let r = noise(k + 11, 8);
        let at = (u64::from_le_bytes(r[..8].try_into().unwrap()) as usize) % b.len().max(1);
        if let Some(x) = b.get_mut(at) {
            *x ^= 0x5a;
        }
        out.push(b);
    }
    out
}

const XML: &str = r#"<?xml version="1.0"?><score-partwise version="4.0"><work><work-title>Que</work-title></work>
<part-list><score-part id="P1"><part-name>T</part-name></score-part></part-list>
<part id="P1"><measure number="1"><attributes><divisions>1</divisions></attributes>
<note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration></note></measure></part></score-partwise>"#;

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (n, d) in entries {
        zw.start_file(*n, o).unwrap();
        zw.write_all(d).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn mxl() -> Vec<u8> {
    let container = br#"<?xml version="1.0"?><container><rootfiles><rootfile full-path="score.musicxml"/></rootfiles></container>"#;
    zip_of(&[
        ("META-INF/container.xml", container),
        ("score.musicxml", XML.as_bytes()),
    ])
}

/// A zip whose headers claim every entry unpacks to ~4 GB.
fn huge_declared(zip: &[u8]) -> Vec<u8> {
    let mut b = zip.to_vec();
    let big = 0xFFFF_FFF0u32.to_le_bytes();
    let mut i = 0;
    while i + 4 <= b.len() {
        match &b[i..i + 4] {
            b"PK\x03\x04" if i + 26 <= b.len() => b[i + 22..i + 26].copy_from_slice(&big),
            b"PK\x01\x02" if i + 28 <= b.len() => b[i + 24..i + 28].copy_from_slice(&big),
            _ => {}
        }
        i += 1;
    }
    b
}

/// Import every variant; each must be an error or a clean success, never a
/// panic, and an error must leave the library exactly as it was.
fn hammer(name: &str, variants: Vec<Vec<u8>>) {
    let d = tmp(&format!("hammer-{}", name.replace('.', "_")));
    std::fs::write(d.join("Keep.pdf"), b"%PDF-1.4 existing").unwrap();
    for v in variants {
        let before = snapshot(&d);
        if import::import_file(&d, name, &v).is_err() {
            assert_eq!(
                snapshot(&d),
                before,
                "{name}: failed import left files behind"
            );
        }
    }
}

#[test]
fn nfr_13_empty_random_and_wrong_extension_are_errors() {
    let d = tmp("plain");
    for name in [
        "a.musicxml",
        "a.xml",
        "a.mxl",
        "a.pdf",
        "a.4sc",
        "a.4ss",
        "a.mscz",
        "a.doc",
        "noext",
    ] {
        for bytes in [Vec::new(), noise(3, 2048)] {
            let r = import::import_file(&d, name, &bytes);
            assert!(r.is_err(), "{name} accepted junk");
            assert!(!r.unwrap_err().is_empty());
        }
    }
    // Right bytes, wrong extension: a PDF called .musicxml, MusicXML called .pdf.
    assert!(import::import_file(&d, "x.musicxml", b"%PDF-1.4 hi").is_err());
    assert!(import::import_file(&d, "x.pdf", XML.as_bytes()).is_err());
    assert!(import::import_file(&d, "x.4sc", XML.as_bytes()).is_err());
    assert!(
        import::import_file(&d, "x.mxl", XML.as_bytes()).is_ok(),
        "plain XML in .mxl still plays"
    );
    assert!(
        snapshot(&d).iter().all(|f| f.starts_with("x.")),
        "{:?}",
        snapshot(&d)
    );
}

#[test]
fn nfr_13_musicxml_and_mxl_mutations_never_panic_or_leave_junk() {
    hammer("Que.musicxml", mutations(XML.as_bytes()));
    let m = mxl();
    hammer("Que.mxl", mutations(&m));
    let d = tmp("mxl-trunc");
    assert!(import::import_file(&d, "t.mxl", &m[..m.len() / 2]).is_err());
    assert!(snapshot(&d).is_empty());
    // A header that lies about a 4 GB size: reading follows the real stream,
    // so it either fails or yields the small real score (no 4 GB allocation).
    let _ = import::import_file(&d, "h.mxl", &huge_declared(&m));
    if let Ok(x) = cantor_score::musicxml_bytes(&huge_declared(&m)) {
        assert_eq!(x, XML.as_bytes());
    }
    // The score parser itself (open_score) rejects a truncated MXL too.
    assert!(cantor_score::musicxml_bytes(&m[..m.len() / 2]).is_err());
}

#[test]
fn nfr_13_pdf_mutations_never_panic_or_leave_junk() {
    hammer(
        "Que.pdf",
        mutations(b"%PDF-1.4\n1 0 obj<<>>endobj\ntrailer<<>>\n%%EOF"),
    );
}

#[test]
fn nfr_13_forscore_mutations_never_panic_or_leave_junk() {
    let sc = forscore::write_4sc("Que", b"%PDF-1.4 que");
    hammer("Que.4sc", mutations(&sc));
    let ss = forscore::write_4ss("Set", &[("Que".into(), b"%PDF-1.4 que".to_vec())]);
    hammer("Set.4ss", mutations(&ss));
    for v in mutations(&sc).into_iter().chain(mutations(&ss)) {
        let _ = forscore::parse_4sc(&v);
        let _ = forscore::parse_4ss(&v);
    }
}

#[test]
fn nfr_13_mscz_sidecar_mutations_never_panic_or_leave_junk() {
    let ms = zip_of(&[("Que.mscx", b"<museScore/>")]);
    hammer("Que.mscz", mutations(&ms));
    let d = tmp("mscz");
    assert!(
        import::import_file(&d, "Alone.mscz", &ms).is_err(),
        "needs its score"
    );
    assert!(snapshot(&d).is_empty());
}

#[test]
fn nfr_13_gzip_and_zip_bombs_are_refused_by_size() {
    // A real bomb scaled down: 4 MB of zeros packs to a few kB; the cap stops it.
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    gz.write_all(&vec![0u8; 4 << 20]).unwrap();
    let packed = gz.finish().unwrap();
    assert!(packed.len() < 64 << 10);
    let r = import::read_limited_to(flate2::read::GzDecoder::new(&packed[..]), 1 << 20, "x");
    assert!(r.unwrap_err().contains("too large"));
    assert_eq!(
        import::read_limited_to(&b"abc"[..], 3, "x").unwrap(),
        b"abc".to_vec()
    );
}

fn sample_bundle() -> Vec<u8> {
    let sl = Setlist {
        id: "s".into(),
        name: "Konsert".into(),
        items: vec![SetItem::Score {
            score: "1:Que".into(),
            start_page: None,
            note: String::new(),
        }],
        instructions: String::new(),
    };
    let sc = vec![bundle::BundleScore {
        id: "1:Que".into(),
        title: "Que".into(),
        files: vec![("Que.musicxml".into(), XML.as_bytes().to_vec())],
        ..Default::default()
    }];
    bundle::export_bundle(&sl, &sc).unwrap()
}

#[test]
fn nfr_13_cantorset_mutations_never_panic_or_leave_junk() {
    let good = sample_bundle();
    let mut vs = mutations(&good);
    vs.push(huge_declared(&good));
    for v in vs {
        let (d, ann) = (tmp("bundle"), tmp("bundle-ann"));
        let mut store = Store::open(&d);
        let sets = store.setlists_ordered().len();
        if bundle::import_bundle(&d, &mut store, &ann, &v).is_err() {
            assert!(snapshot(&d).is_empty(), "bundle left {:?}", snapshot(&d));
            assert!(snapshot(&ann).is_empty());
            assert_eq!(store.setlists_ordered().len(), sets);
        }
    }
}

#[test]
fn nfr_13_backup_mutations_never_panic_or_touch_library() {
    let src = tmp("bk-src");
    std::fs::write(src.join("Que.musicxml"), XML).unwrap();
    let good = backup::backup(&src).unwrap();
    let mut vs = mutations(&good);
    vs.push(huge_declared(&good));
    let d = tmp("bk-dst");
    std::fs::write(d.join("Mine.pdf"), b"%PDF-1.4 mine").unwrap();
    for v in vs {
        let before = snapshot(&d);
        if backup::restore(&d, &v).is_err() {
            assert_eq!(snapshot(&d), before, "failed restore changed the library");
        } else {
            // A mutation that still verifies restored a full library: reset.
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("Mine.pdf"), b"%PDF-1.4 mine").unwrap();
        }
    }
}

#[test]
fn nfr_13_crash_mid_write_keeps_library_json_intact() {
    let d = tmp("atomic");
    let mut st = Store::open(&d);
    st.put_setlist(Setlist {
        id: "a".into(),
        name: "Vår".into(),
        items: Vec::new(),
        instructions: String::new(),
    });
    st.save().unwrap();
    // A crash while writing leaves a half temp file next to the store.
    std::fs::write(d.join("library.json.tmp"), b"{\"version\":1,\"setl").unwrap();
    std::fs::write(d.join(".library.json.tmp"), b"{\"vers").unwrap();
    let again = Store::open(&d);
    assert_eq!(again.setlists_ordered().len(), 1);
    assert_eq!(again.setlists_ordered()[0].name, "Vår");
    // And the next save still works over the leftover temp file.
    again.save().unwrap();
    assert_eq!(Store::open(&d).setlists_ordered().len(), 1);

    // write_atomic: the target is the old or the new file, never half.
    let f = d.join("Ann.json");
    import::write_atomic(&f, b"old").unwrap();
    std::fs::write(d.join(".Ann.json.tmp"), b"ha").unwrap();
    assert_eq!(std::fs::read(&f).unwrap(), b"old");
    import::write_atomic(&f, b"new").unwrap();
    assert_eq!(std::fs::read(&f).unwrap(), b"new");
    assert!(!d.join(".Ann.json.tmp").exists());
}
