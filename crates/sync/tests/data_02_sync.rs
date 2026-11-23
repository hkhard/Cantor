//! DATA-02: sync through a user-chosen folder. Two (or three) devices with
//! their own library dirs share one folder; after concurrent edits and syncs
//! in any order they converge to identical libraries (CRDT tests).

mod common;

use cantor_annotations::{Anchor, Command};
use cantor_library::store::{Metadata, SetItem};
use common::*;

const MXL: &[u8] = b"PK\x03\x04 fake mxl of Olaf Trygvason";
const PDF: &[u8] = b"%PDF-1.7 fake pdf";
const DOC: &str = "1_Olaf";
const ID: &str = "1:Olaf";

fn olaf(d: &Device) {
    d.add_file("Olaf.mxl", MXL);
    d.add_file("Olaf.pdf", PDF);
    d.add_file(".forscore/Olaf.4sc", b"original");
    d.edit_store(|s| {
        s.set_metadata(
            ID,
            Metadata {
                title: "Olaf Trygvason".into(),
                composer: "Reissiger".into(),
                ..Default::default()
            },
        )
    });
}

/// A, B and the shared folder, with Olaf added on A and synced to B.
fn two_with_olaf() -> (Device, Device) {
    let shared = temp_dir("shared");
    let a = Device::new(&shared, "A", 1_000);
    let b = Device::new(&shared, "B", 1_000);
    olaf(&a);
    a.sync();
    b.at(1_100).sync();
    (a, b)
}

#[test]
fn data_02_score_added_on_a_appears_on_b() {
    let shared = temp_dir("shared");
    let a = Device::new(&shared, "A", 1_000);
    let b = Device::new(&shared, "B", 1_000);
    olaf(&a);
    let ra = a.sync();
    assert!(ra.sent > 0);
    assert!(shared.join("cantor-sync/devices/A.jsonl").is_file());
    assert!(!shared.join("cantor-sync/devices/B.jsonl").exists());
    let blobs = std::fs::read_dir(shared.join("cantor-sync/blobs"))
        .unwrap()
        .count();
    assert_eq!(blobs, 3, "mxl, pdf and the kept forScore original");

    let rb = b.sync();
    assert!(rb.received > 0);
    assert_eq!(rb.files_written, 3);
    assert!(rb.store_written);
    assert_eq!(std::fs::read(b.lib.join("Olaf.mxl")).unwrap(), MXL);
    assert_eq!(std::fs::read(b.lib.join("Olaf.pdf")).unwrap(), PDF);
    assert!(b.has_file(".forscore/Olaf.4sc"));
    assert_eq!(b.metadata(ID).unwrap().composer, "Reissiger");
    assert_eq!(a.hash(), b.hash());
}

#[test]
fn data_02_sync_is_idempotent() {
    let (a, b) = two_with_olaf();
    a.edit_doc(DOC, vec![ink(0, 0.2, 0.2, "#f00")]);
    a.sync();
    b.sync();
    let log = std::fs::read(a.sync.log_path()).unwrap();
    let (ha, hb) = (a.hash(), b.hash());
    for d in [&a, &b, &a, &b] {
        let r = d.sync();
        assert_eq!(r.sent, 0, "{r:?}");
        assert_eq!(r.files_written + r.files_removed + r.docs_written, 0);
        assert!(!r.store_written);
    }
    assert_eq!(std::fs::read(a.sync.log_path()).unwrap(), log);
    assert_eq!((a.hash(), b.hash()), (ha.clone(), hb));
    assert_eq!(a.hash(), ha);
}

/// Rule: metadata is one last-writer-wins register per score id (by hybrid
/// logical clock). A concurrent title edit on A and tag edit on B do not
/// merge field by field: the later edit wins as a whole on both devices.
#[test]
fn data_02_concurrent_metadata_edits_last_writer_wins_whole_record() {
    let (a, b) = two_with_olaf();
    a.at(2_000).edit_store(|s| {
        let mut m = s.metadata(ID).unwrap().clone();
        m.title = "Olav Trygvason".into();
        s.set_metadata(ID, m);
    });
    b.at(3_000).edit_store(|s| {
        let mut m = s.metadata(ID).unwrap().clone();
        m.tags.push("jul".into());
        s.set_metadata(ID, m);
    });
    a.sync();
    b.sync();
    a.sync();
    for d in [&a, &b] {
        let m = d.metadata(ID).unwrap();
        assert_eq!(m.tags, vec!["jul".to_string()], "B's later edit wins");
        assert_eq!(m.title, "Olaf Trygvason", "A's earlier title edit is lost");
    }
    assert_eq!(a.hash(), b.hash());

    // Edits after seeing each other are ordered causally, whatever the clock:
    // A's wall clock is behind but A edits after syncing, so A wins.
    a.at(2_500).edit_store(|s| {
        let mut m = s.metadata(ID).unwrap().clone();
        m.title = "Olav".into();
        s.set_metadata(ID, m);
    });
    a.sync();
    b.sync();
    assert_eq!(b.metadata(ID).unwrap().title, "Olav");
    assert_eq!(b.metadata(ID).unwrap().tags, vec!["jul".to_string()]);
    assert_eq!(a.hash(), b.hash());
}

fn concurrent_annotations(order: &[usize]) -> (Device, Device) {
    let (a, b) = two_with_olaf();
    a.at(2_000).edit_doc(DOC, vec![ink(0, 0.1, 0.1, "#a00")]);
    b.at(2_100).edit_doc(DOC, vec![ink(0, 0.5, 0.5, "#0b0")]);
    // Offline both got local id 1.
    assert_eq!(a.doc(DOC).unwrap().items[0].id, 1);
    assert_eq!(b.doc(DOC).unwrap().items[0].id, 1);
    for &i in order {
        [&a, &b][i].sync();
    }
    (a, b)
}

fn colors(d: &Device) -> Vec<String> {
    d.doc(DOC)
        .unwrap()
        .items
        .iter()
        .map(|a| match &a.item {
            cantor_annotations::Item::Ink(i) => i.color.clone(),
            _ => String::new(),
        })
        .collect()
}

#[test]
fn data_02_concurrent_annotations_converge_in_any_sync_order() {
    let mut hashes = Vec::new();
    for order in [[0, 1, 0, 1], [1, 0, 1, 0], [0, 1, 1, 0], [1, 0, 0, 1]] {
        let (a, b) = concurrent_annotations(&order);
        for d in [&a, &b] {
            // Ordered by creation (A drew first), both kept.
            assert_eq!(colors(d), vec!["#a00", "#0b0"], "order {order:?}");
            let doc = d.doc(DOC).unwrap();
            assert_eq!(doc.items[0].id, 1, "the earliest keeps its id");
            assert_ne!(doc.items[1].id, 1, "ids are unique after merge");
            assert!(doc.items.iter().all(|a| a.id < (1 << 53)), "JS-safe ids");
            assert!(doc.next_id > doc.items.iter().map(|a| a.id).max().unwrap());
        }
        assert_eq!(a.hash(), b.hash(), "order {order:?}");
        hashes.push(a.hash());
    }
    assert!(
        hashes.windows(2).all(|w| w[0] == w[1]),
        "same result for every order"
    );
}

#[test]
fn data_02_annotation_edits_after_merge_use_merged_ids() {
    let (a, b) = concurrent_annotations(&[0, 1, 0]);
    // B moves A's stroke and erases its own one (now renumbered).
    let doc = b.doc(DOC).unwrap();
    let (first, second) = (doc.items[0].id, doc.items[1].id);
    let to = Anchor::Page {
        page: 1,
        x: 0.9,
        y: 0.9,
    };
    b.at(5_000).edit_doc(
        DOC,
        vec![
            Command::Move {
                id: first,
                anchor: to,
            },
            Command::Remove { id: second },
        ],
    );
    b.sync();
    a.sync();
    let doc = a.doc(DOC).unwrap();
    assert_eq!(doc.items.len(), 1);
    assert_eq!(doc.items[0].anchor, to);
    assert_eq!(a.hash(), b.hash());
}

/// Rule: removing an annotation wins over a concurrent edit (move) of it
/// (OR-set: a remove deletes the add it observed; edits never resurrect).
#[test]
fn data_02_annotation_remove_wins_over_concurrent_move() {
    let (a, b) = two_with_olaf();
    a.at(2_000).edit_doc(DOC, vec![ink(0, 0.1, 0.1, "#a00")]);
    a.sync();
    b.at(2_100).sync();
    b.at(3_000).edit_doc(DOC, vec![Command::Remove { id: 1 }]);
    a.at(4_000).edit_doc(
        DOC,
        vec![Command::Move {
            id: 1,
            anchor: Anchor::Page {
                page: 0,
                x: 0.3,
                y: 0.3,
            },
        }],
    );
    a.sync();
    b.sync();
    a.sync();
    assert!(a.doc(DOC).unwrap().items.is_empty());
    assert!(b.doc(DOC).unwrap().items.is_empty());
    assert_eq!(a.hash(), b.hash());
}

#[test]
fn data_02_layers_sync_by_index() {
    let (a, b) = two_with_olaf();
    a.at(2_000).edit_doc(
        DOC,
        vec![
            Command::AddLayer {
                name: "Fingerings".into(),
            },
            Command::SetLayerVisible {
                layer: 0,
                visible: false,
            },
        ],
    );
    a.sync();
    b.sync();
    let doc = b.doc(DOC).unwrap();
    assert_eq!(doc.layers.len(), 2);
    assert_eq!(doc.layers[1].name, "Fingerings");
    assert!(!doc.layers[0].visible);
    assert_eq!(a.hash(), b.hash());
}

fn delete_olaf(d: &Device) {
    for f in ["Olaf.mxl", "Olaf.pdf", ".forscore/Olaf.4sc"] {
        std::fs::remove_file(d.lib.join(f)).unwrap();
    }
    let _ = std::fs::remove_file(d.doc_path(DOC));
    d.edit_store(|s| s.remove_score(ID, "Olaf"));
}

fn tag_olaf(d: &Device) {
    d.edit_store(|s| {
        let mut m = s.metadata(ID).unwrap().clone();
        m.tags.push("17. mai".into());
        s.set_metadata(ID, m);
    });
}

/// Rule: delete vs concurrent edit is decided per key by the hybrid logical
/// clock (last writer wins). If the delete is later, the score is gone on
/// both devices. If the metadata edit is later, only the edited record
/// survives (an orphan metadata entry for `1:Olaf`): the score files stay
/// deleted because nobody edited them, so the score does not come back in
/// the library list; re-importing the same file picks the metadata up again.
#[test]
fn data_02_delete_vs_concurrent_edit_is_last_writer_wins_per_key() {
    // Delete is later.
    let (a, b) = two_with_olaf();
    a.edit_doc(DOC, vec![ink(0, 0.1, 0.1, "#a00")]);
    a.sync();
    b.sync();
    b.at(4_000);
    tag_olaf(&b);
    a.at(5_000);
    delete_olaf(&a);
    b.sync();
    a.sync();
    b.sync();
    for d in [&a, &b] {
        assert!(!d.has_file("Olaf.mxl") && !d.has_file("Olaf.pdf"));
        assert!(!d.has_file(".forscore/Olaf.4sc"));
        assert!(d.doc(DOC).is_none(), "annotation document removed");
        assert!(d.metadata(ID).is_none());
    }
    assert_eq!(a.hash(), b.hash());

    // Edit is later.
    let (a, b) = two_with_olaf();
    a.at(5_000);
    delete_olaf(&a);
    b.at(6_000);
    tag_olaf(&b);
    a.sync();
    b.sync();
    a.sync();
    for d in [&a, &b] {
        assert!(!d.has_file("Olaf.mxl") && !d.has_file("Olaf.pdf"));
        assert_eq!(d.metadata(ID).unwrap().tags, vec!["17. mai".to_string()]);
    }
    assert_eq!(a.hash(), b.hash());
}

/// Concurrent annotation on a score deleted elsewhere: the annotation
/// document comes back with only the new, unseen annotations (add wins).
#[test]
fn data_02_new_annotation_survives_concurrent_document_delete() {
    let (a, b) = two_with_olaf();
    a.edit_doc(DOC, vec![ink(0, 0.1, 0.1, "#a00")]);
    a.sync();
    b.sync();
    a.at(5_000);
    delete_olaf(&a);
    b.at(4_000).edit_doc(DOC, vec![ink(0, 0.6, 0.6, "#0b0")]);
    a.sync();
    b.sync();
    a.sync();
    for d in [&a, &b] {
        assert_eq!(colors(d), vec!["#0b0"]);
    }
    assert_eq!(a.hash(), b.hash());
}

/// Rule: a setlist's name and its item list are separate LWW registers, so a
/// rename on one device and a reorder on another both survive.
#[test]
fn data_02_setlist_rename_and_reorder_both_survive() {
    let (a, b) = two_with_olaf();
    let mut sl_id = String::new();
    a.at(1_500).edit_store(|s| {
        sl_id = s.create_setlist("Jul");
        let sl = s.setlist_mut(&sl_id).unwrap();
        sl.items.push(SetItem::Score {
            score: ID.into(),
            start_page: None,
            note: String::new(),
        });
        sl.items.push(SetItem::Placeholder {
            title: "Pause".into(),
        });
    });
    a.sync();
    b.at(1_600).sync();
    a.at(2_000)
        .edit_store(|s| s.setlist_mut(&sl_id).unwrap().name = "Julekonsert".into());
    b.at(2_100)
        .edit_store(|s| s.setlist_mut(&sl_id).unwrap().items.reverse());
    b.sync();
    a.sync();
    b.sync();
    for d in [&a, &b] {
        let st = d.store();
        let sl = st.setlist(&sl_id).unwrap();
        assert_eq!(sl.name, "Julekonsert");
        assert!(matches!(sl.items[0], SetItem::Placeholder { .. }));
    }
    assert_eq!(a.hash(), b.hash());

    // Delete on A, later rename on B: B's rename is later but the setlist
    // stays deleted only if the delete is later. Here the delete is later.
    b.at(3_000)
        .edit_store(|s| s.setlist_mut(&sl_id).unwrap().name = "X".into());
    a.at(3_500).edit_store(|s| {
        s.delete_setlist(&sl_id);
    });
    a.sync();
    b.sync();
    a.sync();
    assert!(a.store().setlist(&sl_id).is_none());
    assert!(b.store().setlist(&sl_id).is_none());
    assert_eq!(a.hash(), b.hash());
}

#[test]
fn data_02_file_edit_and_settings_propagate() {
    let (a, b) = two_with_olaf();
    b.at(2_000).add_file("Olaf.measures.json", b"[1,2,3]");
    b.edit_store(|s| s.settings_mut(ID).transpose = -2);
    a.at(2_500).add_file("Olaf.pdf", b"%PDF-1.7 v2");
    b.sync();
    a.sync();
    b.sync();
    for d in [&a, &b] {
        assert_eq!(
            std::fs::read(d.lib.join("Olaf.pdf")).unwrap(),
            b"%PDF-1.7 v2"
        );
        assert!(d.has_file("Olaf.measures.json"));
        assert_eq!(d.store().settings(ID).unwrap().transpose, -2);
    }
    assert_eq!(a.hash(), b.hash());
}

#[test]
fn data_02_excluded_and_private_files_are_not_synced() {
    let (a, b) = two_with_olaf();
    a.add_file("Export/Olaf annotated.pdf", b"x");
    a.add_file("Inbox/new.mxl", b"x");
    a.add_file("Olaf.pdf.tmp", b"x");
    a.add_file(".before-restore-1/Olaf.mxl", b"x");
    a.sync();
    b.sync();
    assert!(!b.has_file("Export/Olaf annotated.pdf"));
    assert!(!b.has_file("Inbox/new.mxl"));
    assert!(!b.has_file("Olaf.pdf.tmp"));
    assert!(!b.has_file(".before-restore-1/Olaf.mxl"));
    assert!(a.lib.join(".sync/state.json").is_file());
}

#[test]
fn data_02_rejects_unsafe_device_ids() {
    let shared = temp_dir("shared");
    for bad in ["", "../x", "a/b", "a b"] {
        assert!(
            cantor_sync::SyncFolder::open(&shared, bad).is_err(),
            "{bad:?}"
        );
    }
    assert!(cantor_sync::SyncFolder::open(&shared, "iPad-mini_2.local").is_ok());
}

/// Both devices sync at the same time, repeatedly, against one folder.
#[test]
fn data_02_concurrent_syncs_on_two_devices_converge() {
    let (a, b) = two_with_olaf();
    std::thread::scope(|s| {
        s.spawn(|| {
            for i in 0..15u32 {
                a.at(10_000 + u64::from(i) * 10)
                    .edit_doc(DOC, vec![ink(0, 0.01 * i as f32, 0.1, "#a00")]);
                a.add_file(&format!("A{i}.pdf"), format!("a{i}").as_bytes());
                a.sync();
            }
        });
        s.spawn(|| {
            for i in 0..15u32 {
                b.at(10_005 + u64::from(i) * 10)
                    .edit_doc(DOC, vec![ink(0, 0.01 * i as f32, 0.7, "#0b0")]);
                b.add_file(&format!("B{i}.pdf"), format!("b{i}").as_bytes());
                b.sync();
            }
        });
    });
    a.sync();
    b.sync();
    a.sync();
    assert_eq!(a.doc(DOC).unwrap().items.len(), 30);
    assert!(a.has_file("B14.pdf") && b.has_file("A14.pdf"));
    assert_eq!(a.hash(), b.hash());
}

/// Small deterministic PRNG (xorshift64*).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.below(v.len() as u64) as usize]
    }
}

const STEMS: &[&str] = &["Olaf", "Vaar", "Jul", "Mai"];

fn random_edit(d: &Device, rng: &mut Rng) {
    let stem = *rng.pick(STEMS);
    let id = format!("1:{stem}");
    let doc = format!("1_{stem}");
    match rng.below(14) {
        0 => d.add_file(
            &format!("{stem}.mxl"),
            format!("mxl {}", rng.below(5)).as_bytes(),
        ),
        1 => d.add_file(
            &format!("{stem}.pdf"),
            format!("pdf {}", rng.below(5)).as_bytes(),
        ),
        2 => {
            let _ = std::fs::remove_file(d.lib.join(format!("{stem}.pdf")));
        }
        3 => d.edit_store(|s| {
            let mut m = s.metadata(&id).cloned().unwrap_or_default();
            m.title = format!("{stem} {}", rng.below(100));
            s.set_metadata(&id, m);
        }),
        4 => d.edit_store(|s| {
            let mut m = s.metadata(&id).cloned().unwrap_or_default();
            m.tags.push(format!("t{}", rng.below(9)));
            m.rating = Some(rng.below(6) as u8);
            s.set_metadata(&id, m);
        }),
        5 => d.edit_store(|s| s.remove_score(&id, stem)),
        6 => d.edit_store(|s| s.settings_mut(&id).tempo_percent = 50.0 + rng.below(100) as f64),
        7 => d.edit_store(|s| {
            let n = s.setlists().len();
            if n == 0 || rng.below(2) == 0 {
                let sid = s.create_setlist(&format!("Set {}", rng.below(9)));
                s.setlist_mut(&sid).unwrap().items.push(SetItem::Score {
                    score: id.clone(),
                    start_page: Some(1),
                    note: String::new(),
                });
            } else {
                let sid = s.setlists()[rng.below(n as u64) as usize].id.clone();
                match rng.below(3) {
                    0 => {
                        s.delete_setlist(&sid);
                    }
                    1 => s.setlist_mut(&sid).unwrap().name = format!("R{}", rng.below(9)),
                    _ => s
                        .setlist_mut(&sid)
                        .unwrap()
                        .items
                        .push(SetItem::Placeholder { title: "p".into() }),
                }
            }
        }),
        8..=10 => {
            let x = rng.below(100) as f32 / 100.0;
            d.edit_doc(&doc, vec![ink(rng.below(3) as u32, x, 0.5, "#123")]);
        }
        11 => {
            if let Some(dd) = d.doc(&doc) {
                if !dd.items.is_empty() {
                    let a = &dd.items[rng.below(dd.items.len() as u64) as usize];
                    let anchor = Anchor::Page {
                        page: 2,
                        x: rng.below(10) as f32 / 10.0,
                        y: 0.1,
                    };
                    d.edit_doc(&doc, vec![Command::Move { id: a.id, anchor }]);
                }
            }
        }
        12 => {
            if let Some(dd) = d.doc(&doc) {
                if !dd.items.is_empty() {
                    let a = &dd.items[rng.below(dd.items.len() as u64) as usize];
                    d.edit_doc(&doc, vec![Command::Remove { id: a.id }]);
                }
            }
        }
        _ => {
            if rng.below(3) == 0 {
                let _ = std::fs::remove_file(d.doc_path(&doc));
            } else {
                d.edit_doc(
                    &doc,
                    vec![Command::AddLayer {
                        name: format!("L{}", rng.below(4)),
                    }],
                );
            }
        }
    }
}

#[test]
fn data_02_random_concurrent_edits_converge() {
    for seed in 1..=24u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
        let shared = temp_dir("shared");
        let n = 2 + (seed % 2) as usize;
        let devs: Vec<Device> = (0..n)
            .map(|i| {
                // Skewed wall clocks.
                Device::new(&shared, &format!("D{i}"), 1_000 + rng.below(500))
            })
            .collect();
        for _ in 0..60 {
            let d = &devs[rng.below(n as u64) as usize];
            let t = d.clock.load(std::sync::atomic::Ordering::SeqCst) + rng.below(40);
            d.at(t);
            if rng.below(4) == 0 {
                d.sync();
            } else {
                random_edit(d, &mut rng);
            }
        }
        // Final rounds in a random rotation.
        let start = rng.below(n as u64) as usize;
        for round in 0..2 {
            for k in 0..n {
                let r = devs[(start + k) % n].sync();
                if round == 1 {
                    assert_eq!(r.sent, 0, "seed {seed}: no new ops after convergence");
                }
            }
        }
        let h0 = devs[0].hash();
        for (i, d) in devs.iter().enumerate() {
            assert_eq!(d.hash(), h0, "seed {seed}: device {i} diverged");
        }
        for d in &devs {
            let r = d.sync();
            assert_eq!(
                r.sent + r.files_written + r.files_removed + r.docs_written,
                0
            );
        }
    }
}

/// SET-03: instructions travel through folder sync. Score instructions are
/// one register per score; a setlist's instructions are a register apart
/// from its name and items, so a rename on one device and an instructions
/// edit on another both survive. Item notes (SET-05) ride with the items.
#[test]
fn data_02_set_03_instructions_and_item_notes_sync() {
    use cantor_library::store::{PartHint, ScoreInstructions};
    let (a, b) = two_with_olaf();
    let mut sl_id = String::new();
    a.at(1_500).edit_store(|s| {
        s.set_instructions(
            ID,
            ScoreInstructions {
                text: "Andas före **t. 12**.".into(),
                parts: vec![PartHint {
                    who: "Bo".into(),
                    part: "Tenor 1".into(),
                }],
            },
        );
        sl_id = s.create_setlist("Vår");
        s.setlist_mut(&sl_id).unwrap().items.push(SetItem::Score {
            score: ID.into(),
            start_page: None,
            note: "Börja i t. 9".into(),
        });
    });
    a.sync();
    b.at(1_600).sync();
    {
        let st = b.store();
        assert_eq!(st.instructions(ID).unwrap().text, "Andas före **t. 12**.");
        assert_eq!(st.instructions(ID).unwrap().parts[0].part, "Tenor 1");
        assert!(matches!(
            &st.setlist(&sl_id).unwrap().items[0],
            SetItem::Score { note, .. } if note == "Börja i t. 9"
        ));
    }
    a.at(2_000)
        .edit_store(|s| s.setlist_mut(&sl_id).unwrap().name = "Vårkonsert".into());
    b.at(2_100).edit_store(|s| {
        s.setlist_mut(&sl_id).unwrap().instructions = "Svart klädsel.".into();
    });
    b.sync();
    a.sync();
    b.sync();
    for d in [&a, &b] {
        let st = d.store();
        let sl = st.setlist(&sl_id).unwrap();
        assert_eq!(sl.name, "Vårkonsert");
        assert_eq!(sl.instructions, "Svart klädsel.");
    }
    assert_eq!(a.hash(), b.hash());

    // Clearing the score's instructions syncs as a removal.
    a.at(3_000)
        .edit_store(|s| s.set_instructions(ID, ScoreInstructions::default()));
    a.sync();
    b.sync();
    assert!(b.store().instructions(ID).is_none());
    assert_eq!(a.hash(), b.hash());
}
