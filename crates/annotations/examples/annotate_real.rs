//! Draw sample annotations onto a real PDF (ANN-05 manual check):
//!   cargo run -p cantor-annotations --example annotate_real -- in.pdf out.pdf
use cantor_annotations::*;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let pdf = std::fs::read(&a[1]).expect("read pdf");
    let mut ed = Editor::new(Document::new());
    let pts: Vec<InkPoint> = (0..20)
        .map(|i| InkPoint {
            x: i as f32 * 0.01,
            y: (i as f32 * 0.3).sin() * 0.01,
            pressure: 1.0,
        })
        .collect();
    let add = |ed: &mut Editor, x, y, item| {
        ed.apply(Command::Add {
            layer: 0,
            anchor: Anchor::Page { page: 1, x, y },
            item,
        })
        .unwrap();
    };
    add(
        &mut ed,
        0.2,
        0.3,
        Item::Ink(Ink {
            points: pts.clone(),
            color: "#1d4ed8".into(),
            width: 0.003,
            tool: InkTool::Pen,
        }),
    );
    add(
        &mut ed,
        0.2,
        0.5,
        Item::Ink(Ink {
            points: pts,
            color: "#f5d90a".into(),
            width: 0.012,
            tool: InkTool::Highlighter,
        }),
    );
    add(
        &mut ed,
        0.6,
        0.3,
        Item::Stamp(Stamp {
            kind: StampKind::Mf,
            scale: 0.03,
        }),
    );
    add(
        &mut ed,
        0.6,
        0.4,
        Item::Stamp(Stamp {
            kind: StampKind::Breath,
            scale: 0.03,
        }),
    );
    add(
        &mut ed,
        0.1,
        0.1,
        Item::Text(TextNote {
            text: "Andas här".into(),
            size: 0.02,
            color: "#b91c1c".into(),
        }),
    );
    let out = annotate_pdf(&pdf, ed.document()).expect("annotate");
    std::fs::write(&a[2], out).unwrap();
}
