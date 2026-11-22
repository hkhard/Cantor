//! ANN-04: measure-anchored annotations keep their place when the score is
//! laid out again (notation reflow) and can be drawn on any view that knows
//! where the measures are (PDF measure map), including the annotated PDF.
use cantor_annotations::*;

fn rect(page: u32, x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect { page, x, y, w, h }
}

fn ink_on_measure(measure: usize) -> Annotation {
    Annotation {
        id: 1,
        layer: 0,
        anchor: Anchor::Measure {
            measure,
            staff: None,
            dx: 0.25,
            dy: 0.5,
        },
        item: Item::Ink(Ink {
            // Points and width in units of the measure box (x by w, y and width by h).
            points: vec![
                InkPoint {
                    x: 0.0,
                    y: 0.0,
                    pressure: 1.0,
                },
                InkPoint {
                    x: 0.5,
                    y: 0.1,
                    pressure: 1.0,
                },
            ],
            color: "#ff0000".into(),
            width: 0.05,
            tool: InkTool::Pen,
        }),
    }
}

#[test]
fn ann_04_measure_item_to_page_units() {
    let a = ink_on_measure(3);
    let layout = |m: usize| (m == 3).then(|| rect(2, 0.2, 0.4, 0.4, 0.2));
    let (page, x, y, item) = placed_on_page(&a, &layout).unwrap();
    assert_eq!(page, 2);
    assert!((x - 0.3).abs() < 1e-6 && (y - 0.5).abs() < 1e-6, "{x} {y}");
    let Item::Ink(ink) = item else { panic!() };
    // 0.5 of the box width (0.4) = 0.2; 0.1 of the box height (0.2) = 0.02.
    assert!((ink.points[1].x - 0.2).abs() < 1e-6 && (ink.points[1].y - 0.02).abs() < 1e-6);
    assert!((ink.width - 0.01).abs() < 1e-6);
    // Same item on a wider layout stretches with its measure.
    let wide = |_: usize| Some(rect(1, 0.0, 0.0, 0.8, 0.2));
    let (_, _, _, Item::Ink(ink)) = placed_on_page(&a, &wide).unwrap() else {
        panic!()
    };
    assert!((ink.points[1].x - 0.4).abs() < 1e-6);
    // Unknown measure: not placed.
    assert!(placed_on_page(&ink_on_measure(9), &layout).is_none());
}

#[test]
fn ann_04_page_items_pass_through() {
    let mut a = ink_on_measure(0);
    a.anchor = Anchor::Page {
        page: 1,
        x: 0.1,
        y: 0.2,
    };
    let (page, x, y, item) = placed_on_page(&a, &|_| None).unwrap();
    assert_eq!((page, x, y), (1, 0.1, 0.2));
    assert_eq!(item, a.item);
}

#[test]
fn ann_04_remove_many_is_one_undo_step() {
    let mut ed = Editor::new(Document::new());
    for i in 0..3 {
        let mut a = ink_on_measure(i);
        a.id = 0;
        ed.apply(Command::Add {
            layer: 0,
            anchor: a.anchor,
            item: a.item,
        })
        .unwrap();
    }
    let ids: Vec<u64> = ed.document().items.iter().take(2).map(|a| a.id).collect();
    ed.apply(Command::RemoveMany { ids }).unwrap();
    assert_eq!(ed.document().items.len(), 1);
    ed.undo();
    assert_eq!(ed.document().items.len(), 3);
    // Unknown ids are ignored, nothing removed -> no-op.
    ed.apply(Command::RemoveMany { ids: vec![999] }).unwrap();
    assert_eq!(ed.document().items.len(), 3);
}

#[test]
fn ann_04_measure_ink_in_annotated_pdf() {
    // A one-page PDF built with lopdf-free bytes from the crate's own test helper
    // is not public, so make a minimal valid PDF by hand.
    let pdf = minimal_pdf();
    let mut doc = Document::new();
    doc.items.push(ink_on_measure(0));
    let layout = |m: usize| (m == 0).then(|| rect(1, 0.1, 0.1, 0.5, 0.2));
    let out = annotate_pdf_with_layout(&pdf, &doc, &layout).unwrap();
    assert!(out.len() > pdf.len(), "ink was drawn");
    // Without a layout the measure item is skipped.
    let same = annotate_pdf(&pdf, &doc).unwrap();
    assert!(same.len() < out.len());
}

fn minimal_pdf() -> Vec<u8> {
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << >> >>",
        "<< /Length 0 >>\nstream\n\nendstream",
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offs = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offs.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for o in offs {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    out
}
