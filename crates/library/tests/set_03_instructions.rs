//! SET-03 instructions and part hints per score and per setlist, SET-05
//! notes per setlist item: stored in `library.json`, kept through renames,
//! removed with the score, old files still load, and in backups (DATA-03).

use cantor_library::store::{PartHint, ScoreInstructions, SetItem, Store};

fn tmp(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "cantor-set03-{tag}-{}-{}",
        std::process::id(),
        cantor_library::store::new_id()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn hints() -> ScoreInstructions {
    ScoreInstructions {
        text: "Piano från t. 5.\n\n**Tenorer**: andas i m. 12.".into(),
        parts: vec![
            PartHint {
                who: "Åsa".into(),
                part: "Alto".into(),
            },
            PartHint {
                who: "Bo".into(),
                part: "Tenor 1".into(),
            },
            PartHint {
                who: "  ".into(),
                part: "Bass".into(),
            },
        ],
    }
}

#[test]
fn set_03_instructions_persist_and_follow_the_score() {
    let dir = tmp("persist");
    let mut s = Store::open(&dir);
    s.set_instructions("1:A", hints());
    let id = s.create_setlist("Vår");
    {
        let sl = s.setlist_mut(&id).unwrap();
        sl.instructions = "Samling 17.30 i kyrkan.".into();
        sl.items.push(SetItem::Score {
            score: "1:A".into(),
            start_page: None,
            note: "Hoppa över vers 2".into(),
        });
    }
    s.set_member("asa");
    s.save().unwrap();

    let s2 = Store::open(&dir);
    let ins = s2.instructions("1:A").unwrap();
    assert!(ins.text.contains("m. 12"));
    assert_eq!(ins.parts.len(), 2, "blank hint rows are dropped");
    assert_eq!(s2.member(), "asa");
    assert_eq!(
        s2.member_part("1:A"),
        Some("Alto"),
        "names match without case or accents"
    );
    let sl = s2.setlist(&id).unwrap();
    assert_eq!(sl.instructions, "Samling 17.30 i kyrkan.");
    assert!(matches!(&sl.items[0], SetItem::Score { note, .. } if note == "Hoppa över vers 2"));

    let mut s3 = s2.clone();
    s3.rename_score_id("1:A", "1:B");
    assert!(s3.instructions("1:A").is_none());
    assert_eq!(s3.instructions("1:B").unwrap().parts.len(), 2);
    s3.remove_score("1:B", "B");
    assert!(s3.instructions("1:B").is_none());

    // Empty instructions remove the entry.
    let mut s4 = s2.clone();
    s4.set_instructions("1:A", ScoreInstructions::default());
    assert!(s4.instructions("1:A").is_none());
    assert!(s4.all_instructions().is_empty());
    assert_eq!(s4.member_part("1:A"), None);
}

#[test]
fn set_03_old_library_json_without_instructions_loads() {
    let dir = tmp("old");
    std::fs::write(
        dir.join("library.json"),
        r#"{"version":1,"metadata":{},"setlists":[{"id":"s","name":"Old","items":[{"kind":"score","score":"1:A","start_page":2}]}],"settings":{}}"#,
    )
    .unwrap();
    let s = Store::open(&dir);
    let sl = s.setlist("s").unwrap();
    assert!(sl.instructions.is_empty());
    assert_eq!(
        sl.items[0],
        SetItem::Score {
            score: "1:A".into(),
            start_page: Some(2),
            note: String::new()
        }
    );
    // Empty notes and instructions are not written, so older builds read it.
    assert!(!s.export_json().contains("\"note\""));
}

#[test]
fn set_03_instructions_are_in_backups() {
    let dir = tmp("backup");
    let mut s = Store::open(&dir);
    s.set_instructions("1:A", hints());
    s.save().unwrap();
    let h = cantor_library::backup::library_hash(&dir).unwrap();
    let bytes = cantor_library::backup::backup(&dir).unwrap();
    let to = tmp("restored");
    let r = cantor_library::backup::restore(&to, &bytes).unwrap();
    assert_eq!(r.library_hash, h);
    assert_eq!(
        Store::open(&to).instructions("1:A").unwrap().parts[1].who,
        "Bo"
    );
}

#[test]
fn set_03_merge_import_keeps_instructions() {
    let dir = tmp("merge");
    let mut a = Store::open(&dir);
    a.set_instructions("1:A", hints());
    let json = a.export_json();
    let mut b = Store::open(tmp("merge-b"));
    b.import_json(&json, true).unwrap();
    assert_eq!(b.instructions("1:A"), a.instructions("1:A"));
}
