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
    // One-shot search includes building the index; the PRD limit is for the
    // query (checked above). Unoptimised test builds are ~10x slower.
    let limit = if cfg!(debug_assertions) { 3000 } else { 300 };
    assert!(t.elapsed().as_millis() < limit, "incl. index build");
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
            note: String::new(),
        });
        sl.items.push(SetItem::Placeholder {
            title: "Paus".into(),
        });
        sl.items.push(SetItem::Score {
            score: "2:B".into(),
            start_page: None,
            note: String::new(),
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
                start_page: Some(2),
                note: String::new(),
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
        concert_pitch: true,
        programs: vec![None, Some(52)],
        voice_set: Some("organ".into()),
        instrument_key: "Eb".into(),
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
                start_page: Some(3),
                note: String::new(),
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

#[test]
fn set_01_manual_order_of_library_and_setlist_tabs_persists() {
    let d = std::env::temp_dir().join(format!("cantor-order-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let mut s = Store::open(&d);
    assert!(s.score_order().is_empty());
    s.set_score_order(vec!["1:B".into(), "0:A".into(), "1:C".into()]);
    let a = s.create_setlist("Vår");
    let b = s.create_setlist("Jul");
    s.set_setlist_order(vec![b.clone(), a.clone()]);
    s.save().unwrap();
    let s = Store::open(&d);
    assert_eq!(s.score_order(), &["1:B", "0:A", "1:C"]);
    // Setlists come back in the user's order.
    let names: Vec<&str> = s
        .setlists_ordered()
        .iter()
        .map(|l| l.name.as_str())
        .collect();
    assert_eq!(names, vec!["Jul", "Vår"]);
    // Sorting entries: ordered ids first in that order, the rest by title after.
    let entries = vec![
        ("0:A".to_string(), "Alfa".to_string()),
        ("1:C".into(), "Cesar".into()),
        ("1:B".into(), "Beta".into()),
        ("1:N".into(), "Ny".into()),
    ];
    let sorted: Vec<String> = s
        .in_score_order(entries)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(sorted, vec!["1:B", "0:A", "1:C", "1:N"]);
}

#[test]
fn lib_03_metadata_seeded_from_the_file_header_never_over_user_edits() {
    let d = std::env::temp_dir().join(format!("cantor-seed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let mut s = Store::open(&d);
    let h = cantor_score::Header {
        work_title: "The Anchor Song".into(),
        composer: "B. Gudmundsdottir".into(),
        arranger: "HKH".into(),
        lyricist: "Björk".into(),
        ..Default::default()
    };
    // New score: everything from the file.
    assert!(s.seed_metadata("1:anchor", &h));
    let m = s.metadata("1:anchor").unwrap().clone();
    assert_eq!(
        (m.title.as_str(), m.composer.as_str(), m.arranger.as_str()),
        ("The Anchor Song", "B. Gudmundsdottir", "HKH")
    );
    assert_eq!(m.tags, vec!["Text: Björk".to_string()]);
    // The user edits the composer and clears the arranger on purpose:
    // seeding again changes nothing (a seeded score is never re-seeded).
    let mut edited = m.clone();
    edited.composer = "Björk Guðmundsdóttir".into();
    edited.arranger = String::new();
    s.set_metadata("1:anchor", edited.clone());
    assert!(!s.seed_metadata("1:anchor", &h));
    assert_eq!(s.metadata("1:anchor").unwrap(), &edited);
    // Existing metadata from before this feature (title only, typed by the
    // user): only the empty fields are filled, the title stays.
    s.set_metadata(
        "1:old",
        Metadata {
            title: "Mitt namn".into(),
            ..Default::default()
        },
    );
    assert!(s.seed_metadata("1:old", &h));
    let m = s.metadata("1:old").unwrap();
    assert_eq!(
        (m.title.as_str(), m.composer.as_str()),
        ("Mitt namn", "B. Gudmundsdottir")
    );
    // An empty header seeds nothing.
    assert!(!s.seed_metadata("1:none", &cantor_score::Header::default()));
    assert!(s.metadata("1:none").is_none());
    // Persisted.
    s.save().unwrap();
    assert_eq!(Store::open(&d).metadata("1:anchor").unwrap(), &edited);
}

#[test]
fn lib_04_home_view_defaults_to_all_scores_and_persists() {
    use cantor_library::store::HomeView;
    let d = std::env::temp_dir().join(format!("cantor-home-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let mut s = Store::open(&d);
    // Default: grouped by setlists, "All scores" selected.
    assert_eq!(
        s.home_view(),
        &HomeView {
            group: "setlists".into(),
            tab: String::new()
        }
    );
    s.set_home_view(HomeView {
        group: "genre".into(),
        tab: "Julsånger".into(),
    });
    s.save().unwrap();
    let s = Store::open(&d);
    assert_eq!(
        s.home_view(),
        &HomeView {
            group: "genre".into(),
            tab: "Julsånger".into()
        }
    );
    // Unknown grouping from a newer version falls back to the default.
    let mut s = s;
    s.set_home_view(HomeView {
        group: "mood".into(),
        tab: "x".into(),
    });
    assert_eq!(s.home_view().group, "setlists");
    assert_eq!(s.home_view().tab, "");
}

#[test]
fn play_09_instruments_per_score_and_voice_sets() {
    use cantor_library::store::{builtin_voice_sets, VoiceSet};
    let d = std::env::temp_dir().join(format!("cantor-vs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let mut s = Store::open(&d);
    // Per score: instrument chosen for mixer channel 1.
    s.settings_mut("1:x").programs = vec![None, Some(19)];
    // Built-in sets exist for every choir type, cover S/A/T/B, and the GM
    // programs are valid.
    let b = builtin_voice_sets();
    for choir in ["mixed", "male", "female"] {
        assert!(
            b.iter()
                .filter(|v| v.choir == choir || v.choir.is_empty())
                .count()
                >= 4,
            "{choir}"
        );
    }
    for v in &b {
        assert!(!v.name_sv.is_empty() && !v.name_en.is_empty());
        for r in ["soprano", "alto", "tenor", "bass"] {
            let p = v
                .program_for(r, 1)
                .unwrap_or_else(|| panic!("{} has no {r}", v.id));
            assert!(p < 128);
        }
    }
    // Divisi: a second tenor gets a different instrument when the set has one.
    let classic = b.iter().find(|v| v.id == "classic").unwrap();
    assert_ne!(
        classic.program_for("tenor", 1),
        classic.program_for("tenor", 2)
    );
    // An "all one sound" set (choir aahs) uses it for every voice.
    let aahs = b.iter().find(|v| v.id == "choir-aahs").unwrap();
    assert_eq!(aahs.program_for("soprano", 1), Some(52));
    assert_eq!(aahs.program_for("bass", 3), Some(52));
    // Own sets: saved, listed with the built-ins, chosen as default per
    // choir type, removable; ids never clash with built-ins.
    let mine = VoiceSet {
        id: String::new(),
        name_sv: "Vår kör".into(),
        name_en: "Our choir".into(),
        choir: "male".into(),
        roles: [("tenor".into(), vec![71, 72]), ("bass".into(), vec![70])]
            .into_iter()
            .collect(),
    };
    let id = s.save_voice_set(mine.clone());
    assert!(!id.is_empty() && !b.iter().any(|v| v.id == id));
    s.set_default_voice_set("male", &id);
    s.save().unwrap();
    let mut s = Store::open(&d);
    assert_eq!(s.settings("1:x").unwrap().programs, vec![None, Some(19)]);
    let all = s.voice_sets();
    let got = all.iter().find(|v| v.id == id).unwrap();
    assert_eq!(got.name_en, "Our choir");
    assert_eq!(got.program_for("tenor", 2), Some(72));
    // Roles the set leaves out fall back to the classic choice.
    assert_eq!(got.program_for("soprano", 1), None);
    assert_eq!(
        s.default_voice_set("male").map(|v| v.id.clone()),
        Some(id.clone())
    );
    assert_eq!(
        s.default_voice_set("mixed").map(|v| v.id.clone()),
        Some("classic".into())
    );
    s.remove_voice_set(&id);
    assert!(!s.voice_sets().iter().any(|v| v.id == id));
    assert_eq!(
        s.default_voice_set("male").map(|v| v.id.clone()),
        Some("classic".into())
    );
    // Built-ins cannot be removed.
    s.remove_voice_set("classic");
    assert!(s.voice_sets().iter().any(|v| v.id == "classic"));
}

#[test]
fn lib_01_first_run_hint_is_remembered_in_the_store() {
    let d = std::env::temp_dir().join(format!("cantor-hint-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let mut s = Store::open(&d);
    assert!(!s.hint_seen("reader"));
    s.mark_hint_seen("reader");
    s.save().unwrap();
    let mut s = Store::open(&d);
    assert!(s.hint_seen("reader"));
    assert!(!s.hint_seen("other"));
    // "Show Getting started" brings it back, and that is saved too.
    s.forget_hint("reader");
    s.save().unwrap();
    assert!(!Store::open(&d).hint_seen("reader"));
}
