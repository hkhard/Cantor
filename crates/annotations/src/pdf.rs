//! Annotated PDF export (PRD ANN-05).
//!
//! [`annotate_pdf`] burns the visible annotations of a [`Document`] into the page
//! content of a PDF so they show up in any viewer (Preview, Acrobat...).
//!
//! * Page-anchored items ([`Anchor::Page`]) are always drawn. Measure-anchored
//!   items are drawn by [`annotate_pdf_with_layout`] where the measure's box on the
//!   PDF is known (measure map), and skipped by [`annotate_pdf`].
//! * Items on hidden layers are not drawn.
//! * Existing page content is wrapped in `q`/`Q` so the annotation stream starts
//!   with a clean graphics state. Coordinates honour the CropBox (else MediaBox) and
//!   `/Rotate` (0, 90, 180, 270).
//! * Raster items (forScore PNG layers) are decoded with the `png` crate and embedded
//!   as an RGB image XObject with an alpha SMask covering the page, drawn beneath the
//!   vector items. PNGs that fail to decode are skipped.
//! * Text uses the standard Helvetica fonts with WinAnsi encoding; characters outside
//!   it are mapped to close fallbacks (fermata -> `U`, breath mark -> `v`) or `?`.

use crate::{Anchor, Document, InkTool, Item, ShapeKind};
use lopdf::{dictionary, Dictionary, Object, ObjectId, Stream};
use std::fmt::Write as _;

const FONT_TEXT: &str = "CantorHelv";
const FONT_STAMP: &str = "CantorHelvBO";
const GS_HIGHLIGHT: &str = "CantorGSHi";

/// Return a copy of `pdf` with the visible page-anchored annotations of `doc` drawn
/// onto their pages (`Anchor::Page::page` is 1-based). Pages without items are left
/// untouched.
pub fn annotate_pdf(pdf: &[u8], doc: &Document) -> Result<Vec<u8>, String> {
    annotate_pdf_with_layout(pdf, doc, &|_| None)
}

/// Like [`annotate_pdf`], also drawing measure-anchored items where `layout`
/// knows the measure's box on this PDF (the PDF measure map, LIB-05).
pub fn annotate_pdf_with_layout(
    pdf: &[u8],
    doc: &Document,
    layout: &dyn Fn(usize) -> Option<crate::Rect>,
) -> Result<Vec<u8>, String> {
    let mut pdoc = lopdf::Document::load_mem(pdf).map_err(|e| format!("cannot load PDF: {e}"))?;
    let pages = pdoc.get_pages();
    // Everything as page-anchored items in page units.
    let placed: Vec<crate::Annotation> = doc
        .visible_items()
        .filter_map(|a| {
            let (page, x, y, item) = crate::placed_on_page(a, layout)?;
            Some(crate::Annotation {
                id: a.id,
                layer: a.layer,
                anchor: Anchor::Page { page, x, y },
                item,
            })
        })
        .collect();
    for (&num, &page_id) in &pages {
        let items: Vec<_> = placed
            .iter()
            .filter(|a| match (&a.item, a.anchor) {
                (Item::Raster(r), _) => r.page == num,
                (_, Anchor::Page { page, .. }) => page == num,
                _ => false,
            })
            .collect();
        if items.is_empty() {
            continue;
        }
        annotate_page(&mut pdoc, page_id, &items)?;
    }
    let mut out = Vec::new();
    pdoc.save_to(&mut out)
        .map_err(|e| format!("cannot write PDF: {e}"))?;
    Ok(out)
}

fn inherited(pdoc: &lopdf::Document, page_id: ObjectId, key: &[u8]) -> Option<Object> {
    let mut id = page_id;
    for _ in 0..32 {
        let dict = pdoc.get_dictionary(id).ok()?;
        if let Ok(v) = dict.get(key) {
            return Some(match v {
                Object::Reference(r) => pdoc.get_object(*r).ok()?.clone(),
                o => o.clone(),
            });
        }
        id = dict.get(b"Parent").ok()?.as_reference().ok()?;
    }
    None
}

fn page_box(pdoc: &lopdf::Document, page_id: ObjectId) -> [f32; 4] {
    for key in [&b"CropBox"[..], b"MediaBox"] {
        if let Some(Object::Array(a)) = inherited(pdoc, page_id, key) {
            let v: Vec<f32> = a
                .iter()
                .filter_map(|o| match o {
                    Object::Reference(r) => pdoc.get_object(*r).ok()?.as_float().ok(),
                    o => o.as_float().ok(),
                })
                .collect();
            if v.len() == 4 {
                return [
                    v[0].min(v[2]),
                    v[1].min(v[3]),
                    v[0].max(v[2]),
                    v[1].max(v[3]),
                ];
            }
        }
    }
    [0.0, 0.0, 612.0, 792.0]
}

fn rgb(color: &str) -> (f32, f32, f32) {
    let h = color.trim().trim_start_matches('#');
    let p = |i: usize| {
        h.get(i..i + 2)
            .and_then(|s| u8::from_str_radix(s, 16).ok())
            .unwrap_or(0) as f32
            / 255.0
    };
    if h.len() == 3 {
        let q = |i: usize| {
            h.get(i..i + 1)
                .and_then(|s| u8::from_str_radix(s, 16).ok())
                .unwrap_or(0) as f32
                / 15.0
        };
        return (q(0), q(1), q(2));
    }
    (p(0), p(2), p(4))
}

/// Encode text as a WinAnsi PDF literal string (with escapes).
fn pdf_string(text: &str) -> String {
    let mut s = String::from("(");
    for c in text.chars() {
        let b: u8 = match c {
            '(' | ')' | '\\' => {
                s.push('\\');
                c as u8
            }
            ' '..='~' => c as u8,
            '–' => 0x96,
            '—' => 0x97,
            '·' => 0xB7,
            '∨' => b'v',
            '𝄐' => b'U',
            '\u{a0}'..='\u{ff}' => c as u32 as u8,
            _ => b'?',
        };
        if b < 0x80 {
            s.push(b as char);
        } else {
            let _ = write!(s, "\\{b:03o}");
        }
    }
    s.push(')');
    s
}

fn f(v: f32) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    if r == 0.0 {
        "0".into()
    } else {
        format!("{r}")
    }
}

/// Decode a PNG to (width, height, rgb, alpha).
fn decode_png(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>, Vec<u8>)> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let px = (info.width * info.height) as usize;
    let (mut rgb, mut alpha) = (Vec::with_capacity(px * 3), Vec::with_capacity(px));
    let data = &buf[..info.buffer_size()];
    match info.color_type {
        png::ColorType::Rgba => data.as_chunks::<4>().0.iter().for_each(|c| {
            rgb.extend_from_slice(&c[..3]);
            alpha.push(c[3]);
        }),
        png::ColorType::Rgb => data.as_chunks::<3>().0.iter().for_each(|c| {
            rgb.extend_from_slice(c);
            alpha.push(255);
        }),
        png::ColorType::GrayscaleAlpha => data.as_chunks::<2>().0.iter().for_each(|c| {
            rgb.extend_from_slice(&[c[0]; 3]);
            alpha.push(c[1]);
        }),
        png::ColorType::Grayscale => data.iter().for_each(|&g| {
            rgb.extend_from_slice(&[g; 3]);
            alpha.push(255);
        }),
        png::ColorType::Indexed => return None,
    }
    (rgb.len() == px * 3).then_some((info.width, info.height, rgb, alpha))
}

fn image_xobject(pdoc: &mut lopdf::Document, png_bytes: &[u8]) -> Option<ObjectId> {
    let (w, h, rgb, alpha) = decode_png(png_bytes)?;
    let mut mask = Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Image", "Width" => w as i64,
            "Height" => h as i64, "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8,
        },
        alpha,
    );
    let _ = mask.compress();
    let mask_id = pdoc.add_object(mask);
    let mut img = Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Image", "Width" => w as i64,
            "Height" => h as i64, "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8,
            "SMask" => mask_id,
        },
        rgb,
    );
    let _ = img.compress();
    Some(pdoc.add_object(img))
}

fn annotate_page(
    pdoc: &mut lopdf::Document,
    page_id: ObjectId,
    items: &[&crate::Annotation],
) -> Result<(), String> {
    let [llx, lly, urx, ury] = page_box(pdoc, page_id);
    let (bw, bh) = (urx - llx, ury - lly);
    let rotate = inherited(pdoc, page_id, b"Rotate")
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(0)
        .rem_euclid(360);
    // Matrix from displayed (unrotated, y-up) space to PDF user space.
    let (cm, dw, dh) = match rotate {
        90 => ([0.0, 1.0, -1.0, 0.0, urx, lly], bh, bw),
        180 => ([-1.0, 0.0, 0.0, -1.0, urx, ury], bw, bh),
        270 => ([0.0, -1.0, 1.0, 0.0, llx, ury], bh, bw),
        _ => ([1.0, 0.0, 0.0, 1.0, llx, lly], bw, bh),
    };
    let pt = |u: f32, v: f32| (u * dw, dh - v * dh);

    let mut s = String::from("Q q\n");
    let _ = writeln!(
        s,
        "{} {} {} {} {} {} cm",
        f(cm[0]),
        f(cm[1]),
        f(cm[2]),
        f(cm[3]),
        f(cm[4]),
        f(cm[5])
    );
    let mut images: Vec<(String, ObjectId)> = Vec::new();
    let (mut need_font, mut need_stamp, mut need_gs) = (false, false, false);

    // Raster layers first, beneath vector annotations.
    for a in items {
        if let Item::Raster(r) = &a.item {
            if let Some(id) = image_xobject(pdoc, &r.png_bytes) {
                let name = format!("CantorIm{}", images.len());
                let _ = writeln!(s, "q {} 0 0 {} 0 0 cm /{name} Do Q", f(dw), f(dh));
                images.push((name, id));
            }
        }
    }
    for a in items {
        let Anchor::Page { x: ax, y: ay, .. } = a.anchor else {
            continue;
        };
        match &a.item {
            Item::Raster(_) => {}
            Item::Ink(ink) => {
                let (r, g, b) = rgb(&ink.color);
                s.push_str("q 1 J 1 j\n");
                if ink.tool == InkTool::Highlighter {
                    need_gs = true;
                    let _ = writeln!(s, "/{GS_HIGHLIGHT} gs");
                }
                let _ = writeln!(s, "{} {} {} RG", f(r), f(g), f(b));
                if ink.tool == InkTool::Highlighter && ink.points.len() > 1 {
                    // One path: stroking each translucent segment on its own
                    // darkens every joint where segments overlap.
                    let avg = ink.points.iter().map(|p| p.pressure).sum::<f32>()
                        / ink.points.len() as f32;
                    let _ = write!(s, "{} w", f(ink.width * dh * avg));
                    for (i, p) in ink.points.iter().enumerate() {
                        let (x, y) = pt(ax + p.x, ay + p.y);
                        let _ = write!(s, " {} {} {}", f(x), f(y), if i == 0 { "m" } else { "l" });
                    }
                    s.push_str(" S\nQ\n");
                    continue;
                }
                for seg in ink.points.windows(2) {
                    let w = ink.width * dh * (seg[0].pressure + seg[1].pressure) / 2.0;
                    let (x0, y0) = pt(ax + seg[0].x, ay + seg[0].y);
                    let (x1, y1) = pt(ax + seg[1].x, ay + seg[1].y);
                    let _ = writeln!(
                        s,
                        "{} w {} {} m {} {} l S",
                        f(w),
                        f(x0),
                        f(y0),
                        f(x1),
                        f(y1)
                    );
                }
                if ink.points.len() == 1 {
                    let p = ink.points[0];
                    let (x0, y0) = pt(ax + p.x, ay + p.y);
                    let w = ink.width * dh * p.pressure;
                    let _ = writeln!(
                        s,
                        "{} w {} {} m {} {} l S",
                        f(w),
                        f(x0),
                        f(y0),
                        f(x0),
                        f(y0)
                    );
                }
                s.push_str("Q\n");
            }
            Item::Text(t) => {
                need_font = true;
                let (r, g, b) = rgb(&t.color);
                let (x, y) = pt(ax, ay);
                let _ = writeln!(
                    s,
                    "q {} {} {} rg BT /{FONT_TEXT} {} Tf {} {} Td {} Tj ET Q",
                    f(r),
                    f(g),
                    f(b),
                    f(t.size * dh),
                    f(x),
                    f(y),
                    pdf_string(&t.text)
                );
            }
            Item::Stamp(st) => {
                need_stamp = true;
                let (x, y) = pt(ax, ay);
                let _ = writeln!(
                    s,
                    "q 0 0 0 rg BT /{FONT_STAMP} {} Tf {} {} Td {} Tj ET Q",
                    f(st.scale * dh),
                    f(x),
                    f(y),
                    pdf_string(&st.kind.glyph())
                );
            }
            Item::Shape(sh) => {
                let (r, g, b) = rgb(&sh.color);
                let _ = writeln!(
                    s,
                    "q 2 w 1 J 1 j {} {} {} RG {} {} {} rg",
                    f(r),
                    f(g),
                    f(b),
                    f(r),
                    f(g),
                    f(b)
                );
                let (x0, y0) = pt(ax, ay);
                let (x1, y1) = pt(ax + sh.w, ay + sh.h);
                match sh.kind {
                    ShapeKind::Rect => {
                        let _ =
                            writeln!(s, "{} {} {} {} re S", f(x0), f(y0), f(x1 - x0), f(y1 - y0));
                    }
                    ShapeKind::Ellipse => {
                        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
                        let (rx, ry) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
                        let k = 0.552_284_8;
                        let (kx, ky) = (rx * k, ry * k);
                        let _ = writeln!(s, "{} {} m", f(cx + rx), f(cy));
                        for c in [
                            [cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry],
                            [cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy],
                            [cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry],
                            [cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy],
                        ] {
                            let _ = writeln!(
                                s,
                                "{} {} {} {} {} {} c",
                                f(c[0]),
                                f(c[1]),
                                f(c[2]),
                                f(c[3]),
                                f(c[4]),
                                f(c[5])
                            );
                        }
                        s.push_str("S\n");
                    }
                    ShapeKind::Line | ShapeKind::Arrow => {
                        let _ = writeln!(s, "{} {} m {} {} l S", f(x0), f(y0), f(x1), f(y1));
                        if sh.kind == ShapeKind::Arrow {
                            let (vx, vy) = (x1 - x0, y1 - y0);
                            let len = (vx * vx + vy * vy).sqrt().max(1e-6);
                            let (ux, uy) = (vx / len, vy / len);
                            let hl = (len * 0.2).clamp(4.0, 16.0);
                            let (bx, by) = (x1 - ux * hl, y1 - uy * hl);
                            let (px, py) = (-uy * hl * 0.5, ux * hl * 0.5);
                            let _ = writeln!(
                                s,
                                "{} {} m {} {} l {} {} l h f",
                                f(x1),
                                f(y1),
                                f(bx + px),
                                f(by + py),
                                f(bx - px),
                                f(by - py)
                            );
                        }
                    }
                }
                s.push_str("Q\n");
            }
        }
    }
    s.push_str("Q\n");

    // Content: [q, ...existing, Q + annotations]. Our stream begins with "Q q" so the
    // original content is balanced and our drawing starts from a clean CTM.
    let existing: Vec<Object> = match pdoc
        .get_dictionary(page_id)
        .map_err(|e| e.to_string())?
        .get(b"Contents")
    {
        Ok(Object::Array(a)) => a.clone(),
        Ok(Object::Reference(r)) => match pdoc.get_object(*r) {
            Ok(Object::Array(a)) => a.clone(),
            _ => vec![Object::Reference(*r)],
        },
        _ => Vec::new(),
    };
    let pre = pdoc.add_object(Stream::new(Dictionary::new(), b"q\n".to_vec()));
    let mut post = Stream::new(Dictionary::new(), s.into_bytes());
    let _ = post.compress();
    let post = pdoc.add_object(post);
    let mut contents = vec![Object::Reference(pre)];
    contents.extend(existing);
    contents.push(Object::Reference(post));

    // Resources: materialise (possibly inherited / referenced) dict onto the page.
    let mut res = match inherited(pdoc, page_id, b"Resources") {
        Some(Object::Dictionary(d)) => d,
        _ => Dictionary::new(),
    };
    let mut sub = |pdoc: &lopdf::Document, key: &[u8], entries: Vec<(String, Object)>| {
        if entries.is_empty() {
            return;
        }
        let mut d = match res.get(key) {
            Ok(Object::Dictionary(d)) => d.clone(),
            Ok(Object::Reference(r)) => pdoc
                .get_dictionary(*r)
                .cloned()
                .unwrap_or_else(|_| Dictionary::new()),
            _ => Dictionary::new(),
        };
        for (k, v) in entries {
            d.set(k, v);
        }
        res.set(key.to_vec(), Object::Dictionary(d));
    };
    let font = |base: &str| {
        Object::Dictionary(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => base,
            "Encoding" => "WinAnsiEncoding",
        })
    };
    let mut fonts = Vec::new();
    if need_font {
        fonts.push((FONT_TEXT.to_string(), font("Helvetica")));
    }
    if need_stamp {
        fonts.push((FONT_STAMP.to_string(), font("Helvetica-BoldOblique")));
    }
    sub(pdoc, b"Font", fonts);
    if need_gs {
        sub(
            pdoc,
            b"ExtGState",
            vec![(
                GS_HIGHLIGHT.to_string(),
                Object::Dictionary(dictionary! { "Type" => "ExtGState", "CA" => 0.4, "ca" => 0.4 }),
            )],
        );
    }
    sub(
        pdoc,
        b"XObject",
        images
            .into_iter()
            .map(|(n, id)| (n, Object::Reference(id)))
            .collect(),
    );

    let page = pdoc
        .get_object_mut(page_id)
        .and_then(Object::as_dict_mut)
        .map_err(|e| e.to_string())?;
    page.set("Contents", Object::Array(contents));
    page.set("Resources", Object::Dictionary(res));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Annotation, Ink, InkPoint, Layer, StampKind, TextNote};
    use lopdf::content::{Content, Operation};

    fn two_page_pdf() -> Vec<u8> {
        let mut d = lopdf::Document::with_version("1.5");
        let pages_id = d.new_object_id();
        let mut kids = Vec::new();
        for _ in 0..2 {
            let content = Content {
                operations: vec![
                    Operation::new(
                        "cm",
                        vec![2.into(), 0.into(), 0.into(), 2.into(), 0.into(), 0.into()],
                    ),
                    Operation::new("re", vec![10.into(), 10.into(), 20.into(), 20.into()]),
                    Operation::new("f", vec![]),
                ],
            };
            let cid = d.add_object(Stream::new(Dictionary::new(), content.encode().unwrap()));
            kids.push(Object::Reference(d.add_object(dictionary! {
                "Type" => "Page", "Parent" => pages_id, "Contents" => cid,
            })));
        }
        d.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => kids, "Count" => 2,
                "MediaBox" => vec![0.into(), 0.into(), 600.into(), 800.into()],
                "Resources" => dictionary! {},
            }),
        );
        let cat = d.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        d.trailer.set("Root", cat);
        let mut out = Vec::new();
        d.save_to(&mut out).unwrap();
        out
    }

    fn doc() -> Document {
        let mut doc = Document::new();
        doc.layers.push(Layer {
            name: "Hidden".into(),
            visible: false,
        });
        let ink = |color: &str| {
            Item::Ink(Ink {
                points: vec![
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
                ],
                color: color.into(),
                width: 0.005,
                tool: InkTool::Pen,
            })
        };
        let at = |x, y| Anchor::Page { page: 1, x, y };
        doc.items = vec![
            Annotation {
                id: 1,
                layer: 0,
                anchor: at(0.1, 0.1),
                item: ink("#ff0000"),
            },
            Annotation {
                id: 2,
                layer: 1,
                anchor: at(0.2, 0.2),
                item: ink("#00ff00"),
            },
            Annotation {
                id: 3,
                layer: 0,
                anchor: at(0.5, 0.5),
                item: Item::Text(TextNote {
                    text: "rit.".into(),
                    size: 0.02,
                    color: "#000000".into(),
                }),
            },
            Annotation {
                id: 4,
                layer: 0,
                anchor: at(0.3, 0.3),
                item: Item::Stamp(crate::Stamp {
                    kind: StampKind::Fermata,
                    scale: 0.03,
                }),
            },
            Annotation {
                id: 5,
                layer: 0,
                anchor: Anchor::Measure {
                    measure: 0,
                    staff: None,
                    dx: 0.0,
                    dy: 0.0,
                },
                item: ink("#0000ff"),
            },
        ];
        doc
    }

    fn page_ops(d: &lopdf::Document, n: u32) -> Vec<Operation> {
        let id = d.get_pages()[&n];
        Content::decode(&d.get_page_content(id)).unwrap().operations
    }

    fn nums(op: &Operation) -> Vec<f32> {
        op.operands.iter().map(|o| o.as_float().unwrap()).collect()
    }

    #[test]
    fn ann_05_export_draws_visible_page_items() {
        let out = annotate_pdf(&two_page_pdf(), &doc()).unwrap();
        assert!(out.starts_with(b"%PDF"));
        let d = lopdf::Document::load_mem(&out).unwrap();
        let ops = page_ops(&d, 1);
        // Clean CTM: first op is q, existing content balanced before ours.
        assert_eq!(ops[0].operator, "q");
        // Red stroke from (0.1,0.1) to (0.6,0.6) of a 600x800 page, y flipped.
        let i = ops
            .iter()
            .position(|o| o.operator == "RG" && nums(o) == vec![1.0, 0.0, 0.0])
            .expect("red stroke color");
        let m = ops[i..].iter().find(|o| o.operator == "m").unwrap();
        let l = ops[i..].iter().find(|o| o.operator == "l").unwrap();
        assert_eq!(nums(m), vec![60.0, 720.0]);
        assert_eq!(nums(l), vec![360.0, 320.0]);
        assert!(ops[i..].iter().any(|o| o.operator == "S"));
        let w = ops[i..].iter().find(|o| o.operator == "w").unwrap();
        assert!((nums(w)[0] - 4.0).abs() < 1e-3);
        assert!(ops
            .iter()
            .any(|o| o.operator == "J" && nums(o) == vec![1.0]));
        // Hidden-layer green and measure-anchored blue inks are not drawn.
        assert!(!ops
            .iter()
            .any(|o| o.operator == "RG" && nums(o) == vec![0.0, 1.0, 0.0]));
        assert!(!ops
            .iter()
            .any(|o| o.operator == "RG" && nums(o) == vec![0.0, 0.0, 1.0]));
        // Text and fermata fallback glyph.
        let tj: Vec<_> = ops
            .iter()
            .filter(|o| o.operator == "Tj")
            .map(|o| o.operands[0].as_str().unwrap().to_vec())
            .collect();
        assert!(tj.contains(&b"rit.".to_vec()) && tj.contains(&b"U".to_vec()));
        // Fonts present.
        let fonts = d.get_page_fonts(d.get_pages()[&1]).unwrap();
        let bases: Vec<_> = fonts
            .values()
            .map(|f| f.get(b"BaseFont").unwrap().as_name().unwrap().to_vec())
            .collect();
        assert!(bases.contains(&b"Helvetica".to_vec()));
        assert!(bases.contains(&b"Helvetica-BoldOblique".to_vec()));
        // Page 2 untouched.
        let p2 = page_ops(&d, 2);
        assert_eq!(p2.len(), 3);
        assert_eq!(p2[0].operator, "cm");
    }

    #[test]
    fn ann_05_export_highlighter_uses_alpha_gs() {
        let mut doc = Document::new();
        doc.items.push(Annotation {
            id: 1,
            layer: 0,
            anchor: Anchor::Page {
                page: 2,
                x: 0.0,
                y: 0.0,
            },
            item: Item::Ink(Ink {
                points: vec![
                    InkPoint {
                        x: 0.0,
                        y: 0.0,
                        pressure: 0.5,
                    },
                    InkPoint {
                        x: 0.1,
                        y: 0.0,
                        pressure: 0.5,
                    },
                ],
                color: "#ffff00".into(),
                width: 0.01,
                tool: InkTool::Highlighter,
            }),
        });
        let d = lopdf::Document::load_mem(&annotate_pdf(&two_page_pdf(), &doc).unwrap()).unwrap();
        assert_eq!(page_ops(&d, 1).len(), 3);
        let ops = page_ops(&d, 2);
        assert!(ops.iter().any(|o| o.operator == "gs"));
        let (res, _) = d.get_page_resources(d.get_pages()[&2]).unwrap();
        let gs = res.unwrap().get(b"ExtGState").unwrap().as_dict().unwrap();
        let ca = gs.get(GS_HIGHLIGHT.as_bytes()).unwrap().as_dict().unwrap();
        assert!((ca.get(b"CA").unwrap().as_float().unwrap() - 0.4).abs() < 1e-6);
    }

    #[test]
    fn ann_05_export_embeds_raster_png() {
        let mut png_bytes = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut png_bytes, 2, 2);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header()
                .unwrap()
                .write_image_data(&[255u8; 16])
                .unwrap();
        }
        let mut doc = Document::new();
        doc.items.push(Annotation {
            id: 1,
            layer: 0,
            anchor: Anchor::Page {
                page: 1,
                x: 0.0,
                y: 0.0,
            },
            item: Item::Raster(crate::RasterLayer { page: 1, png_bytes }),
        });
        let d = lopdf::Document::load_mem(&annotate_pdf(&two_page_pdf(), &doc).unwrap()).unwrap();
        assert!(page_ops(&d, 1).iter().any(|o| o.operator == "Do"));
    }

    #[test]
    fn ann_05_export_rejects_garbage() {
        assert!(annotate_pdf(b"not a pdf", &Document::new()).is_err());
    }
}
