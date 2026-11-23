//! Measure map (LIB-05): where each written measure sits on the original
//! page, read from the Audiveris `.omr` book.
//!
//! The book is a zip: `book.xml` lists sheets in order with their pages
//! (`movement-start="true"` where a new movement begins), each
//! `sheet#N/sheet#N.xml` has `<picture width height>` and, per `<page>` and
//! `<system>`, the measures as `<stack left right>` (pixels) plus barlines
//! with `<bounds y h>` that give the system's vertical extent. Measures are
//! numbered in reading order, which is also the order of `<measure>` in the
//! MusicXML Audiveris exports per movement, so box `i` of movement `k`
//! belongs to written measure `i` of `<name>.mvt{k+1}.mxl`.
//!
//! Courtesy stacks (`special="CAUTIONARY"`, end-of-line key/time changes)
//! are not measures. Sheet XML files are megabytes of pixel runs; they are
//! streamed.

use quick_xml::events::Event;
use quick_xml::Reader;
use std::io::Read;
use std::path::Path;

/// A measure's box on its page, as fractions of the page width/height.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct MeasureBox {
    /// 1-based page (sheet) of the PDF.
    pub page: u32,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum MapError {
    #[error("not an Audiveris book: {0}")]
    Book(String),
}

fn err(e: impl std::fmt::Display) -> MapError {
    MapError::Book(e.to_string())
}

fn attr(e: &quick_xml::events::BytesStart<'_>, name: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == name)
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

fn num(e: &quick_xml::events::BytesStart<'_>, name: &[u8]) -> Option<f64> {
    attr(e, name).and_then(|v| v.parse().ok())
}

/// Measure boxes of one sheet, grouped by the sheet's page id, in reading order.
fn sheet_boxes(xml: &[u8], sheet: u32) -> Result<Vec<(u32, Vec<MeasureBox>)>, MapError> {
    let mut r = Reader::from_reader(xml);
    let mut buf = Vec::new();
    let (mut width, mut height) = (0.0, 0.0);
    let mut stacks: Vec<(f64, f64)> = Vec::new();
    let (mut y0, mut y1) = (f64::MAX, f64::MIN);
    let mut in_system = false;
    let mut in_barline = false;
    let mut out: Vec<(u32, Vec<MeasureBox>)> = Vec::new();
    loop {
        match r.read_event_into(&mut buf).map_err(err)? {
            Event::Start(e) | Event::Empty(e) => match e.name().as_ref() {
                b"picture" => {
                    width = num(&e, b"width").unwrap_or(0.0);
                    height = num(&e, b"height").unwrap_or(0.0);
                }
                b"page" => {
                    let id = attr(&e, b"id").and_then(|v| v.parse().ok()).unwrap_or(1);
                    out.push((id, Vec::new()));
                }
                b"system" => in_system = true,
                b"stack" if in_system && attr(&e, b"special").as_deref() != Some("CAUTIONARY") => {
                    if let (Some(l), Some(rr)) = (num(&e, b"left"), num(&e, b"right")) {
                        stacks.push((l, rr));
                    }
                }
                b"barline" if in_system => in_barline = true,
                b"bounds" if in_barline => {
                    if let (Some(y), Some(h)) = (num(&e, b"y"), num(&e, b"h")) {
                        y0 = y0.min(y);
                        y1 = y1.max(y + h);
                    }
                    in_barline = false;
                }
                _ => {}
            },
            Event::End(e) => match e.name().as_ref() {
                b"system" => {
                    if out.is_empty() {
                        out.push((1, Vec::new()));
                    }
                    let page = &mut out.last_mut().expect("page").1;
                    if width > 0.0 && height > 0.0 && y1 > y0 {
                        for &(l, rr) in &stacks {
                            page.push(MeasureBox {
                                page: sheet,
                                x: l / width,
                                y: y0 / height,
                                w: (rr - l) / width,
                                h: (y1 - y0) / height,
                            });
                        }
                    }
                    stacks.clear();
                    y0 = f64::MAX;
                    y1 = f64::MIN;
                    in_system = false;
                }
                b"barline" => in_barline = false,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

/// A sheet number with its pages `(page id, starts a movement)`.
type SheetPages = (u32, Vec<(u32, bool)>);

/// Sheets in book order, each with its pages.
fn book_sheets(xml: &[u8]) -> Result<Vec<SheetPages>, MapError> {
    let mut r = Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut out: Vec<SheetPages> = Vec::new();
    let mut depth_in_sheet = false;
    loop {
        match r.read_event_into(&mut buf).map_err(err)? {
            Event::Start(e) if e.name().as_ref() == b"sheet" => {
                if let Some(n) = attr(&e, b"number").and_then(|v| v.parse().ok()) {
                    out.push((n, Vec::new()));
                    depth_in_sheet = true;
                }
            }
            Event::Empty(e) if e.name().as_ref() == b"sheet" => {
                if let Some(n) = attr(&e, b"number").and_then(|v| v.parse().ok()) {
                    out.push((n, Vec::new()));
                }
            }
            Event::Start(e) | Event::Empty(e) if depth_in_sheet && e.name().as_ref() == b"page" => {
                let id = attr(&e, b"id").and_then(|v| v.parse().ok()).unwrap_or(1);
                let start = attr(&e, b"movement-start").as_deref() == Some("true");
                if let Some(s) = out.last_mut() {
                    s.1.push((id, start));
                }
            }
            Event::End(e) if e.name().as_ref() == b"sheet" => depth_in_sheet = false,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

/// Measure boxes for each movement of a `.omr` book.
pub fn measure_map_movements(omr: &[u8]) -> Result<Vec<Vec<MeasureBox>>, MapError> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(omr)).map_err(err)?;
    let mut read = |name: &str| -> Result<Vec<u8>, MapError> {
        let mut f = zip.by_name(name).map_err(err)?;
        let mut b = Vec::new();
        f.read_to_end(&mut b).map_err(err)?;
        Ok(b)
    };
    let sheets = book_sheets(&read("book.xml")?)?;
    let mut movements: Vec<Vec<MeasureBox>> = Vec::new();
    for (n, pages) in sheets {
        // A sheet Audiveris skipped (no music) has no XML: no measures.
        let Ok(xml) = read(&format!("sheet#{n}/sheet#{n}.xml")) else {
            continue;
        };
        for (page_id, boxes) in sheet_boxes(&xml, n)? {
            let starts = pages
                .iter()
                .find(|(id, _)| *id == page_id)
                .is_some_and(|(_, s)| *s);
            if starts || movements.is_empty() {
                movements.push(Vec::new());
            }
            movements.last_mut().expect("movement").extend(boxes);
        }
    }
    Ok(movements)
}

/// Measure boxes for the first movement (the one Cantor converts), in
/// written-measure order.
pub fn measure_map(omr: &[u8]) -> Result<Vec<MeasureBox>, MapError> {
    Ok(measure_map_movements(omr)?
        .into_iter()
        .next()
        .unwrap_or_default())
}

/// Where a preprocessed OMR image came from: 1-based PDF `page` and the crop
/// rectangle on it as fractions of the page. Written per image, in image
/// order, by `tools/omr/split_spreads.py` and `clean_scan.py` as
/// `<image>.pages.json`.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct PageCrop {
    pub page: u32,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Parse a crop sidecar (`[{page,x,y,w,h}, ...]`).
pub fn parse_crops(json: &[u8]) -> Result<Vec<PageCrop>, MapError> {
    serde_json::from_slice(json).map_err(err)
}

/// Map boxes on preprocessed images (`box.page` = 1-based image) back to the
/// PDF pages they were cut from.
pub fn uncrop(boxes: &mut [MeasureBox], crops: &[PageCrop]) -> Result<(), MapError> {
    for b in boxes {
        let c = b
            .page
            .checked_sub(1)
            .and_then(|i| crops.get(i as usize))
            .ok_or_else(|| MapError::Book(format!("no crop for image {}", b.page)))?;
        *b = MeasureBox {
            page: c.page,
            x: c.x + b.x * c.w,
            y: c.y + b.y * c.h,
            w: b.w * c.w,
            h: b.h * c.h,
        };
    }
    Ok(())
}

/// Index of the movement an Audiveris export file holds, given how many
/// movements the book has: `<n>.mvtK.mxl` is movement K, `<n>.mxl` the only
/// one, and `<n>.mvtnull.mxl` (a `-sheets N` run, where every movement is
/// written to the same file) the last one.
pub fn export_movement(file: &str, movements: usize) -> Option<usize> {
    if movements == 0 {
        return None;
    }
    let stem = file
        .strip_suffix(".mxl")
        .or_else(|| file.strip_suffix(".musicxml"))
        .or_else(|| file.strip_suffix(".xml"))
        .unwrap_or(file);
    let Some((_, tag)) = stem.rsplit_once(".mvt") else {
        return Some(0);
    };
    if tag == "null" {
        return Some(movements - 1);
    }
    let k: usize = tag.parse().ok()?;
    (1..=movements).contains(&k).then(|| k - 1)
}

/// Map boxes on preprocessed images back to the PDF when `dir` has a crop
/// sidecar. `None` if the sidecar does not cover the boxes.
fn to_pdf(dir: &Path, name: &str, mut boxes: Vec<MeasureBox>) -> Option<Vec<MeasureBox>> {
    match std::fs::read(dir.join(format!("{name}.pages.json"))) {
        Ok(json) => {
            let crops = parse_crops(&json).ok()?;
            uncrop(&mut boxes, &crops).ok()?;
            Some(boxes)
        }
        Err(_) => Some(boxes),
    }
}

/// Candidate maps in the converter's ladder order: whole book, cleaned book,
/// spread-split book, raw page books, cleaned page books; every movement of
/// a whole book is a candidate (movement 1 first). The caller picks the one
/// whose measure count matches the MusicXML that was kept.
pub fn candidates(work: &Path, name: &str) -> Vec<(Vec<MeasureBox>, String)> {
    let movements = |p: &Path| {
        std::fs::read(p)
            .ok()
            .and_then(|b| measure_map_movements(&b).ok())
    };
    let mut out = Vec::new();
    for (sub, label) in [("", "book"), ("cleaned", "cleaned"), ("split", "split")] {
        let dir = work.join(sub);
        let Some(mv) = movements(&dir.join(format!("{name}.omr"))) else {
            continue;
        };
        for (k, b) in mv.into_iter().enumerate() {
            if b.is_empty() {
                continue;
            }
            if let Some(b) = to_pdf(&dir, name, b) {
                let label = if k == 0 {
                    label.to_string()
                } else {
                    format!("{label} movement {}", k + 1)
                };
                out.push((b, label));
            }
        }
    }
    // Page books: the converter joined, in page order, the one export each
    // page run left behind; take that export's movement. Pages without an
    // export were skipped by the converter and have no measures.
    let pages = |root: &Path| -> Option<Vec<MeasureBox>> {
        let dir = root.join("pages");
        let mut out = Vec::new();
        let mut n = 1;
        while dir.join(format!("page{n}")).exists() {
            let page = dir.join(format!("page{n}"));
            let export = std::fs::read_dir(&page)
                .ok()?
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .find(|f| f.ends_with(".mxl"));
            if let (Some(export), Some(mv)) = (export, movements(&page.join(format!("{name}.omr"))))
            {
                if let Some(k) = export_movement(&export, mv.len()) {
                    let mut b = mv[k].clone();
                    for m in &mut b {
                        m.page = n; // page books hold one sheet each
                    }
                    out.extend(b);
                }
            }
            n += 1;
        }
        (!out.is_empty()).then_some(out)
    };
    if let Some(b) = pages(work) {
        out.push((b, "pages".into()));
    }
    let cleaned = work.join("cleaned");
    if let Some(b) = pages(&cleaned).and_then(|b| to_pdf(&cleaned, name, b)) {
        out.push((b, "cleaned pages".into()));
    }
    out
}

/// The measure map for a converter work dir (`.<name>.omr`): the candidate
/// whose box count equals `measures`, else the first. Returns the boxes and
/// where they came from (`book`, `cleaned`, `pages`, ...).
pub fn map_for_work(work: &Path, name: &str, measures: usize) -> Option<(Vec<MeasureBox>, String)> {
    let cands = candidates(work, name);
    cands
        .iter()
        .find(|(b, _)| b.len() == measures)
        .or(cands.first())
        .cloned()
}
