use cantor_annotations::*;
fn ink(x: f32) -> Item {
    Item::Ink(Ink {
        points: vec![
            InkPoint {
                x,
                y: 0.0,
                pressure: 1.0,
            },
            InkPoint {
                x: x + 0.01,
                y: 0.0,
                pressure: 0.5,
            },
        ],
        color: "#000".into(),
        width: 0.002,
        tool: InkTool::Pen,
    })
}
fn page() -> Anchor {
    Anchor::Page {
        page: 0,
        x: 0.0,
        y: 0.0,
    }
}
#[test]
fn ann_01_undo_100_steps_and_redo() {
    let mut e = Editor::new(Document::new());
    for i in 0..150 {
        e.apply(Command::Add {
            layer: 0,
            anchor: page(),
            item: ink(i as f32 * 0.001),
        })
        .unwrap();
    }
    assert_eq!(e.document().items.len(), 150);
    for _ in 0..100 {
        assert!(e.undo());
    }
    assert_eq!(e.document().items.len(), 50);
    assert!(e.redo());
    assert_eq!(e.document().items.len(), 51);
    e.apply(Command::Add {
        layer: 0,
        anchor: page(),
        item: ink(0.5),
    })
    .unwrap();
    assert!(!e.can_redo());
    assert_eq!(e.document().items.len(), 52);
}
#[test]
fn ann_01_stroke_eraser_move_remove() {
    let mut e = Editor::new(Document::new());
    let a = e
        .apply(Command::Add {
            layer: 0,
            anchor: page(),
            item: ink(0.1),
        })
        .unwrap()
        .unwrap();
    let b = e
        .apply(Command::Add {
            layer: 0,
            anchor: page(),
            item: ink(0.8),
        })
        .unwrap()
        .unwrap();
    e.apply(Command::Erase {
        page: 0,
        points: vec![(0.105, 0.0)],
        radius: 0.01,
    })
    .unwrap();
    assert_eq!(e.document().items.len(), 1);
    assert_eq!(e.document().items[0].id, b);
    e.undo();
    assert!(e.document().get(a).is_some());
    e.apply(Command::Move {
        id: b,
        anchor: Anchor::Page {
            page: 1,
            x: 0.5,
            y: 0.5,
        },
    })
    .unwrap();
    assert_eq!(e.document().get(b).unwrap().anchor.page_hint(), Some(1));
    e.apply(Command::Remove { id: a }).unwrap();
    assert!(e.document().get(a).is_none());
    assert!(e.apply(Command::Remove { id: 999 }).is_err());
}
#[test]
fn ann_01_simplify_rdp() {
    let pts: Vec<InkPoint> = (0..=100)
        .map(|i| InkPoint {
            x: i as f32 / 100.0,
            y: 0.0001 * ((i % 2) as f32),
            pressure: 1.0,
        })
        .collect();
    let s = simplify(&pts, 0.001);
    assert_eq!(s.len(), 2);
    let corner = vec![
        InkPoint {
            x: 0.0,
            y: 0.0,
            pressure: 1.0,
        },
        InkPoint {
            x: 0.5,
            y: 0.5,
            pressure: 1.0,
        },
        InkPoint {
            x: 1.0,
            y: 0.0,
            pressure: 1.0,
        },
    ];
    assert_eq!(simplify(&corner, 0.01).len(), 3);
}
