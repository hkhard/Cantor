//! LIB-03 metadata persistence, LIB-04 search, SET-01 setlists, PLAY-04 custom
//! presets, LIB-08 setlist import into the store.

use cantor_library::forscore::{ForscoreSetlist, SetlistEntry, SetlistItem};
use cantor_library::store::*;
use std::collections::BTreeMap;
use std::time::Instant;

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "cantor_store_{name}_{}_{}",
        std::process::id(),
        new_id()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn meta() -> Metadata {
    Metadata {
        title: "Olaf Trygvason".into(),
        composer: "Edvard Grieg".into(),
        arranger: "".into(),
        key: "D minor".into(),
        tempo: Some(92.0),
        genre: "Romantik".into(),
        tags: vec!["manskör".into(), "vår".into()],
        rating: Some(4),
        difficulty: Some(3),
        part_names: vec!["Tenor 1".into(), "Bas 2".into()],
    }
}

#[test]
fn lib_03_metadata_survives_restart() {
    let dir = tmp("restart");
    let mut s = Store::open(&dir);
    s.set_metadata("1:Olaf Trygvason", meta());
    s.save().unwrap();
    let s2 = Store::open(&dir);
    assert_eq!(s2.metadata("1:Olaf Trygvason"), Some(&meta()));
    assert!(dir.join(STORE_FILE).exists());
}

#[test]
fn lib_03_rating_clamped_and_export_import_round_trip() {
    let dir = tmp("export");
    let mut s = Store::open(&dir);
    let mut m = meta();
    m.rating = Some(9);
    s.set_metadata("a", m);
    assert_eq!(s.metadata("a").unwrap().rating, Some(5));
    let json = s.export_json();
    assert!(json.contains("\"version\": 1"));
    let mut t = Store::open(tmp("export2"));
    t.set_metadata("other", Metadata::default());
    t.import_json(&json, true).unwrap();
    assert_eq!(t.metadata("a"), s.metadata("a"));
    assert!(t.metadata("other").is_some());
    t.import_json(&json, false).unwrap();
    assert!(t.metadata("other").is_none());
    assert!(t.import_json("not json", true).is_err());
}

#[test]
fn lib_03_corrupt_file_gives_empty_store_and_backup() {
    let dir = tmp("corrupt");
    std::fs::write(dir.join(STORE_FILE), "{garbage").unwrap();
    let s = Store::open(&dir);
    assert!(s.metadata("x").is_none());
    assert!(dir.join(format!("{STORE_FILE}.bak")).exists());
}

fn entries() -> Vec<(String, String)> {
    vec![
        ("1".into(), "Vårvindar friska".into()),
        ("2".into(), "Olaf Trygvason".into()),
        ("3".into(), "Sång till Olaf".into()),
    ]
}

#[test]
fn lib_04_search_diacritics_fields_ranking() {
    let mut s = Store::open(tmp("search"));
    s.set_metadata("2", meta());
    let e = entries();
    assert_eq!(s.search("varvindar", &e), vec!["1"]);
    assert_eq!(s.search("VÅRVINDAR", &e), vec!["1"]);
    assert_eq!(s.search("grieg", &e), vec!["2"]);
    assert_eq!(s.search("bas 2", &e), vec!["2"]);
    assert_eq!(s.search("manskor", &e), vec!["2"]);
    assert_eq!(s.search("olaf", &e), vec!["2", "3"]); // prefix before contains
    assert_eq!(s.search("olaf grieg", &e), vec!["2"]);
    assert!(s.search("zzz", &e).is_empty());
    assert_eq!(s.search("", &e).len(), 3);
}

#[test]
fn lib_04_ten_thousand_items_under_100ms() {
    let mut s = Store::open(tmp("perf"));
    let mut e = Vec::new();
    for i in 0..10_000 {
        let id = format!("{i}:Score {i}");
        let mut m = meta();
        m.title = format!("Sång nummer {i}");
        m.composer = format!("Kompositör {}", i % 97);
        s.set_metadata(&id, m);
        e.push((id, format!("Sång nummer {i}")));
    }
    let idx = s.search_index(&e);
    let t = Instant::now();
    let r = idx.search("kompositor 42 sang");
    let el = t.elapsed();
    assert!(!r.is_empty());
    assert!(el.as_millis() < 100, "search took {el:?}");
    let t = Instant::now();
    let _ = s.search("nummer 9999", &e);
    assert!(t.elapsed().as_millis() < 100 * 3, "incl. index build");
}

#[test]
fn set_01_setlists_persist_and_cascade() {
    let dir = tmp("sets");
    let mut s = Store::open(&dir);
    s.set_metadata(
        "1:A",
        Metadata {
            title: "A".into(),
            ..Default::default()
        },
    );
    let id = s.create_setlist("Konsert");
    {
        let sl = s.setlist_mut(&id).unwrap();
        sl.items.push(SetItem::Score {
            score: "1:A".into(),
            start_page: Some(2),
        });
        sl.items.push(SetItem::Placeholder {
            title: "Paus".into(),
        });
        sl.items.push(SetItem::Score {
            score: "2:B".into(),
            start_page: None,
        });
        sl.items.swap(0, 1); // reorder
    }
    s.rename_score_id("1:A", "9:A");
    assert!(s.metadata("9:A").is_some());
    s.remove_score("2:B", "B");
    s.save().unwrap();
    let s2 = Store::open(&dir);
    let sl = s2.setlist(&id).unwrap();
    assert_eq!(sl.name, "Konsert");
    assert_eq!(
        sl.items,
        vec![
            SetItem::Placeholder {
                title: "Paus".into()
            },
            SetItem::Score {
                score: "9:A".into(),
                start_page: Some(2)
            },
            SetItem::Placeholder { title: "B".into() },
        ]
    );
    let id2 = s.create_setlist("Other");
    assert_ne!(id, id2);
    assert_eq!(s.setlists().len(), 2);
    assert!(s.delete_setlist(&id2));
}

#[test]
fn play_04_custom_presets_and_settings_persist() {
    let dir = tmp("settings");
    let mut s = Store::open(&dir);
    let strip = MixerStrip {
        volume_db: -12.0,
        mute: false,
        solo: true,
    };
    let st = ScoreSettings {
        my_part: Some(1),
        strips: vec![strip.clone(), MixerStrip::default()],
        tempo_percent: 80.0,
        view: View::Notation,
        choir: Choir::Male,
        presets: vec![Preset {
            name: "Rehearsal".into(),
            strips: vec![strip],
        }],
        transpose: -2,
        hidden_parts: vec![3],
    };
    s.set_settings("1:A", st.clone());
    s.save().unwrap();
    assert_eq!(Store::open(&dir).settings("1:A"), Some(&st));
    assert!(serde_json_contains(&s.export_json(), "\"notation\""));
    assert_eq!(ScoreSettings::default().tempo_percent, 100.0);
}

fn serde_json_contains(j: &str, n: &str) -> bool {
    j.contains(n)
}

#[test]
fn lib_08_setlist_from_forscore_keeps_order_and_placeholders() {
    let item = |t: &str, f: &str, page: Option<i64>| {
        let mut unknown = BTreeMap::new();
        if let Some(p) = page {
            unknown.insert("StartPage".to_string(), plist::Value::Integer(p.into()));
        }
        SetlistItem {
            title: t.into(),
            file_path: f.into(),
            identifier: "x".into(),
            entry: SetlistEntry::Placeholder,
            unknown,
        }
    };
    let fs = ForscoreSetlist {
        items: vec![
            item("Kung Lilje", "kung.pdf", Some(3)),
            item("Paus", "", None),
            item("Missing", "missing.pdf", None),
        ],
    };
    let sl = setlist_from_forscore(
        "CMB 40",
        &fs,
        &[("Kung Lilje".into(), "1:Kung Lilje".into())],
    );
    assert_eq!(sl.name, "CMB 40");
    assert_eq!(
        sl.items,
        vec![
            SetItem::Score {
                score: "1:Kung Lilje".into(),
                start_page: Some(3)
            },
            SetItem::Placeholder {
                title: "Paus".into()
            },
            SetItem::Placeholder {
                title: "Missing".into()
            },
        ]
    );
}
