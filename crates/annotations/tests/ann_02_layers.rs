use cantor_annotations::*;
#[test]
fn ann_02_layer_visibility_persists() {
    let mut e = Editor::new(Document::new());
    let l = e
        .apply(Command::AddLayer {
            name: "Teacher".into(),
        })
        .unwrap()
        .unwrap() as usize;
    e.apply(Command::Add {
        layer: l,
        anchor: Anchor::Page {
            page: 0,
            x: 0.1,
            y: 0.1,
        },
        item: Item::Text(TextNote {
            text: "slow".into(),
            size: 0.02,
            color: "#f00".into(),
        }),
    })
    .unwrap();
    e.apply(Command::SetLayerVisible {
        layer: l,
        visible: false,
    })
    .unwrap();
    let json = e.document().to_json().unwrap();
    let d = Document::from_json(&json).unwrap();
    assert_eq!(d, *e.document());
    assert!(!d.layers[l].visible);
    assert_eq!(d.layers[l].name, "Teacher");
    assert_eq!(d.visible_items().count(), 0);
    e.undo();
    assert!(e.document().layers[l].visible);
}
