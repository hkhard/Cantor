use cantor_annotations::*;
#[test]
fn ann_05_svg_contains_elements() {
    let pen = Item::Ink(Ink {
        points: vec![
            InkPoint {
                x: 0.0,
                y: 0.0,
                pressure: 1.0,
            },
            InkPoint {
                x: 0.1,
                y: 0.1,
                pressure: 0.5,
            },
        ],
        color: "#123456".into(),
        width: 0.004,
        tool: InkTool::Pen,
    });
    let hl = Item::Ink(Ink {
        points: vec![
            InkPoint {
                x: 0.0,
                y: 0.0,
                pressure: 1.0,
            },
            InkPoint {
                x: 0.2,
                y: 0.0,
                pressure: 1.0,
            },
        ],
        color: "#ffff00".into(),
        width: 0.02,
        tool: InkTool::Highlighter,
    });
    let txt = Item::Text(TextNote {
        text: "a<b".into(),
        size: 0.02,
        color: "#000".into(),
    });
    let br = Item::Stamp(Stamp {
        kind: StampKind::Breath,
        scale: 0.03,
    });
    let p = Item::Stamp(Stamp {
        kind: StampKind::P,
        scale: 0.03,
    });
    let sh = Item::Shape(Shape {
        kind: ShapeKind::Ellipse,
        w: 0.1,
        h: 0.05,
        color: "#f00".into(),
    });
    let ar = Item::Shape(Shape {
        kind: ShapeKind::Arrow,
        w: 0.1,
        h: 0.0,
        color: "#f00".into(),
    });
    let r = Item::Shape(Shape {
        kind: ShapeKind::Rect,
        w: 0.1,
        h: 0.1,
        color: "#f00".into(),
    });
    let ras = Item::Raster(RasterLayer {
        page: 0,
        png_bytes: vec![1, 2, 3],
    });
    let items: Vec<Placed> = [&pen, &hl, &txt, &br, &p, &sh, &ar, &r, &ras]
        .iter()
        .map(|i| Placed {
            x: 0.1,
            y: 0.2,
            item: i,
        })
        .collect();
    let s = to_svg(0, &items, 600.0, 800.0);
    assert!(s.starts_with("<svg") && s.trim_end().ends_with("</svg>"));
    assert!(s.contains("viewBox=\"0 0 600 800\""));
    assert!(s.contains("<path") && s.contains("#123456"));
    assert!(s.contains("stroke-opacity=\"0.4\""));
    assert!(s.contains("a&lt;b"));
    assert!(s.contains(">∨<") && s.contains(">p<"));
    assert!(
        s.contains("<ellipse")
            && s.contains("<rect")
            && s.contains("<line")
            && s.contains("<polygon")
    );
    assert!(s.contains("data:image/png;base64,AQID"));
}
