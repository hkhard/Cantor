//! ANN-05: annotated PDF export across page rotations and every item kind
//! (pen, single-point dot, text with non-ASCII, stamps with fallback glyphs,
//! all shapes, grayscale and RGB raster layers, short hex colours).
use cantor_annotations::*;

fn pdf(rotate: i32, inherited_box: bool) -> Vec<u8> {
    // Page tree where MediaBox (and Rotate) can sit on the parent, as many
    // scanners write it.
    let (pages_extra, page_extra) = if inherited_box {
        (
            format!("/MediaBox [0 0 595 842] /Rotate {rotate}"),
            String::new(),
        )
    } else {
        (
            String::new(),
            format!("/MediaBox [0 0 595 842] /Rotate {rotate}"),
        )
    };
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        format!("<< /Type /Pages /Kids [3 0 R] /Count 1 {pages_extra} >>"),
        format!("<< /Type /Page /Parent 2 0 R {page_extra} /Contents 4 0 R /Resources << >> >>"),
        "<< /Length 0 >>\nstream\n\nendstream".to_string(),
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

fn png(color: png::ColorType, px: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, 2, 1);
        enc.set_color(color);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(px).unwrap();
    }
    out
}

fn everything() -> Document {
    let mut ed = Editor::new(Document::new());
    let mut add = |x: f32, y: f32, item: Item| {
        ed.apply(Command::Add {
            layer: 0,
            anchor: Anchor::Page { page: 1, x, y },
            item,
        })
        .unwrap();
    };
    let pt = |x, y| InkPoint {
        x,
        y,
        pressure: 1.0,
    };
    add(
        0.1,
        0.1,
        Item::Ink(Ink {
            points: vec![pt(0.0, 0.0), pt(0.1, 0.05)],
            color: "#123".into(),
            width: 0.003,
            tool: InkTool::Pen,
        }),
    );
    add(
        0.2,
        0.2,
        Item::Ink(Ink {
            points: vec![pt(0.0, 0.0)],
            color: "#ff0000".into(),
            width: 0.003,
            tool: InkTool::Pen,
        }),
    );
    add(
        0.3,
        0.3,
        Item::Text(TextNote {
            text: "Andas (här) – lugnt · \\ ✓".into(),
            size: 0.02,
            color: "#000000".into(),
        }),
    );
    for (i, k) in [
        StampKind::Fermata,
        StampKind::Breath,
        StampKind::Tenuto,
        StampKind::Staccato,
        StampKind::Fingering(3),
        StampKind::Custom("rit.".into()),
    ]
    .into_iter()
    .enumerate()
    {
        add(
            0.1 + i as f32 * 0.1,
            0.5,
            Item::Stamp(Stamp {
                kind: k,
                scale: 0.03,
            }),
        );
    }
    for (i, k) in [
        ShapeKind::Line,
        ShapeKind::Rect,
        ShapeKind::Ellipse,
        ShapeKind::Arrow,
    ]
    .into_iter()
    .enumerate()
    {
        add(
            0.1 + i as f32 * 0.2,
            0.7,
            Item::Shape(Shape {
                kind: k,
                w: 0.1,
                h: 0.05,
                color: "#00aa00".into(),
            }),
        );
    }
    add(
        0.0,
        0.0,
        Item::Raster(RasterLayer {
            page: 1,
            png_bytes: png(png::ColorType::Rgb, &[255, 0, 0, 0, 255, 0]),
        }),
    );
    add(
        0.0,
        0.0,
        Item::Raster(RasterLayer {
            page: 1,
            png_bytes: png(png::ColorType::Grayscale, &[0, 255]),
        }),
    );
    add(
        0.0,
        0.0,
        Item::Raster(RasterLayer {
            page: 1,
            png_bytes: png(png::ColorType::GrayscaleAlpha, &[0, 128, 255, 255]),
        }),
    );
    add(
        0.0,
        0.0,
        Item::Raster(RasterLayer {
            page: 1,
            png_bytes: b"not a png".to_vec(),
        }),
    );
    ed.into_document()
}

fn page_content(out: &[u8]) -> String {
    let d = lopdf::Document::load_mem(out).expect("output loads");
    let (_, id) = d.get_pages().into_iter().next().unwrap();
    String::from_utf8_lossy(&d.get_page_content(id)).to_string()
}

#[test]
fn ann_05_every_item_kind_on_every_rotation() {
    for rotate in [0, 90, 180, 270] {
        for inherited in [false, true] {
            let out = annotate_pdf(&pdf(rotate, inherited), &everything()).unwrap();
            let c = page_content(&out);
            // A rotation matrix is applied for rotated pages.
            match rotate {
                90 => assert!(c.contains("0 1 -1 0"), "{rotate}: {c}"),
                180 => assert!(c.contains("-1 0 0 -1"), "{rotate}"),
                270 => assert!(c.contains("0 -1 1 0"), "{rotate}"),
                _ => assert!(c.contains("1 0 0 1 0 0 cm"), "{rotate}"),
            }
            // Ink (incl. the single dot), text, stamps, shapes (curves for the ellipse), images.
            assert!(c.matches(" l S").count() >= 2, "ink strokes");
            assert!(c.contains("Tj"), "text");
            assert!(c.contains(" c") || c.contains(" c\n"), "ellipse curves");
            assert!(c.contains(" re"), "rectangle");
            assert!(
                c.matches(" Do").count() >= 3,
                "three decodable raster layers drawn"
            );
            // Short hex colour #123 -> 0.067 0.133 0.2 RG.
            assert!(c.contains("0.067 0.133 0.2 RG"), "short hex colour: {c}");
            // Non-WinAnsi glyphs fall back instead of breaking the string.
            assert!(
                c.contains("(v)") || c.contains("v) Tj"),
                "breath mark fallback"
            );
            assert!(c.contains("\\(h"), "parentheses escaped");
        }
    }
}

#[test]
fn ann_05_bad_input_is_an_error() {
    assert!(annotate_pdf(b"not a pdf", &everything()).is_err());
}

#[test]
fn lib_05_pdf_info_title_text_pages() {
    let read = |p: &str| {
        std::fs::read(format!(
            "{}/../../fixtures/scores/{p}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    };
    let a = cantor_annotations::pdf_info(&read("the-anchor-song/the-anchor-song.pdf")).unwrap();
    assert_eq!((a.title.as_str(), a.pages), ("The Anchor Song", 2));
    let l = cantor_annotations::pdf_info(&read(
        "mozart-lacrymosa/mozart-lacrymosa-from-requiem-satb-with-piano.pdf",
    ))
    .unwrap();
    assert_eq!(l.pages, 5);
    assert!(
        l.first_page_text.contains("Lacrymosa"),
        "{}",
        l.first_page_text
    );
    assert!(cantor_annotations::pdf_info(b"%PDF-1.4 garbage").is_none());
}
