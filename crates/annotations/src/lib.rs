//! # cantor-annotations
//!
//! Annotation data model and editing logic for Cantor (PRD ANN-01..ANN-05).
//!
//! * [`Anchor`]: where an annotation lives. `Page` anchors use fractions (0..1) of a
//!   PDF page. `Measure` anchors use a written measure index plus an offset expressed
//!   as fractions of that measure's box, so annotations on MusicXML scores survive
//!   re-layout (ANN-04). See [`resolve`] and [`anchor_at`].
//! * [`Item`]: ink strokes (pen / highlighter with pressure), text, stamps
//!   (fingerings, breath marks, dynamics...), shapes and imported raster layers
//!   (forScore ink PNGs). All sizes are page-relative, so they scale with zoom (ANN-03).
//! * [`Document`]: versioned, serde-serializable set of [`Layer`]s and
//!   [`Annotation`]s. Layer visibility persists through JSON round trips (ANN-02).
//! * [`Editor`]: applies [`Command`]s with undo/redo of at least 100 steps,
//!   including a stroke eraser (ANN-01). [`simplify`] reduces stroke points (RDP).
//! * [`to_svg`]: renders one page's annotations as SVG, the basis for annotated
//!   PDF export (ANN-05).

pub mod pdf;
pub use pdf::annotate_pdf;

use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// Current document format version.
pub const DOCUMENT_VERSION: u32 = 1;
/// Maximum undo depth kept by [`Editor`].
pub const UNDO_LIMIT: usize = 200;

/// Position of an annotation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Anchor {
    /// Fractions (0..1) of a PDF page.
    Page { page: u32, x: f32, y: f32 },
    /// Written measure index; `dx`/`dy` are fractions of the measure's box.
    Measure {
        measure: usize,
        staff: Option<u32>,
        dx: f32,
        dy: f32,
    },
}

impl Anchor {
    /// Page number for page anchors, `None` for measure anchors.
    pub fn page_hint(&self) -> Option<u32> {
        match self {
            Anchor::Page { page, .. } => Some(*page),
            Anchor::Measure { .. } => None,
        }
    }
}

/// A measure box in page fractions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub page: u32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    /// True if the point lies inside (inclusive) on the same page.
    pub fn contains(&self, page: u32, x: f32, y: f32) -> bool {
        page == self.page
            && x >= self.x
            && x <= self.x + self.w
            && y >= self.y
            && y <= self.y + self.h
    }
    fn dist2(&self, x: f32, y: f32) -> f32 {
        let dx = (self.x - x).max(0.0).max(x - (self.x + self.w));
        let dy = (self.y - y).max(0.0).max(y - (self.y + self.h));
        dx * dx + dy * dy
    }
}

/// Resolve an anchor to `(page, x, y)` in page fractions using the current layout.
pub fn resolve(anchor: &Anchor, layout: &dyn Fn(usize) -> Option<Rect>) -> Option<(u32, f32, f32)> {
    match *anchor {
        Anchor::Page { page, x, y } => Some((page, x, y)),
        Anchor::Measure {
            measure, dx, dy, ..
        } => {
            let r = layout(measure)?;
            Some((r.page, r.x + dx * r.w, r.y + dy * r.h))
        }
    }
}

/// Inverse of [`resolve`]: anchor a point to the measure box containing it, else the
/// nearest box on that page, else a page anchor. `boxes[i]` is measure `i`.
pub fn anchor_at(page: u32, x: f32, y: f32, boxes: &[Rect]) -> Anchor {
    let on_page = boxes.iter().enumerate().filter(|(_, r)| r.page == page);
    let best = on_page
        .clone()
        .find(|(_, r)| r.contains(page, x, y))
        .or_else(|| on_page.min_by(|a, b| a.1.dist2(x, y).total_cmp(&b.1.dist2(x, y))));
    match best {
        Some((m, r)) => Anchor::Measure {
            measure: m,
            staff: None,
            dx: if r.w > 0.0 { (x - r.x) / r.w } else { 0.0 },
            dy: if r.h > 0.0 { (y - r.y) / r.h } else { 0.0 },
        },
        None => Anchor::Page { page, x, y },
    }
}

/// One sampled ink point, relative to the anchor, in page fractions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InkPoint {
    pub x: f32,
    pub y: f32,
    pub pressure: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InkTool {
    Pen,
    Highlighter,
}

/// Freehand stroke. `width` is a fraction of page height (at pressure 1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ink {
    pub points: Vec<InkPoint>,
    pub color: String,
    pub width: f32,
    pub tool: InkTool,
}

/// Text note. `size` is a fraction of page height.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextNote {
    pub text: String,
    pub size: f32,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StampKind {
    Fingering(u8),
    Breath,
    Fermata,
    Pp,
    P,
    Mp,
    Mf,
    F,
    Ff,
    Crescendo,
    Diminuendo,
    Accent,
    Tenuto,
    Staccato,
    Custom(String),
}

impl StampKind {
    /// Text glyph used to render the stamp.
    pub fn glyph(&self) -> String {
        match self {
            StampKind::Fingering(n) => n.to_string(),
            StampKind::Breath => "∨".into(),
            StampKind::Fermata => "𝄐".into(),
            StampKind::Pp => "pp".into(),
            StampKind::P => "p".into(),
            StampKind::Mp => "mp".into(),
            StampKind::Mf => "mf".into(),
            StampKind::F => "f".into(),
            StampKind::Ff => "ff".into(),
            StampKind::Crescendo => "<".into(),
            StampKind::Diminuendo => ">".into(),
            StampKind::Accent => ">".into(),
            StampKind::Tenuto => "–".into(),
            StampKind::Staccato => "·".into(),
            StampKind::Custom(s) => s.clone(),
        }
    }
}

/// Stamp; `scale` is its height as a fraction of page height, so it scales with zoom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stamp {
    pub kind: StampKind,
    pub scale: f32,
}

impl Stamp {
    /// Rendered size in pixels for a page drawn `page_height_px` tall.
    pub fn pixel_size(&self, page_height_px: f32) -> f32 {
        self.scale * page_height_px
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    Line,
    Rect,
    Ellipse,
    Arrow,
}

/// Shape spanning `w` x `h` (page fractions) from its anchor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub kind: ShapeKind,
    pub w: f32,
    pub h: f32,
    pub color: String,
}

/// Imported raster ink (e.g. a forScore PNG layer) drawn under vector annotations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RasterLayer {
    pub page: u32,
    pub png_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Item {
    Ink(Ink),
    Text(TextNote),
    Stamp(Stamp),
    Shape(Shape),
    Raster(RasterLayer),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub id: u64,
    /// Index into [`Document::layers`].
    pub layer: usize,
    pub anchor: Anchor,
    pub item: Item,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub version: u32,
    pub layers: Vec<Layer>,
    pub items: Vec<Annotation>,
    /// Next id to assign.
    #[serde(default)]
    pub next_id: u64,
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    /// New document with one visible layer named "Default".
    pub fn new() -> Self {
        Document {
            version: DOCUMENT_VERSION,
            layers: vec![Layer {
                name: "Default".into(),
                visible: true,
            }],
            items: Vec::new(),
            next_id: 1,
        }
    }
    pub fn get(&self, id: u64) -> Option<&Annotation> {
        self.items.iter().find(|a| a.id == id)
    }
    /// Items on visible layers.
    pub fn visible_items(&self) -> impl Iterator<Item = &Annotation> {
        self.items
            .iter()
            .filter(|a| self.layers.get(a.layer).is_some_and(|l| l.visible))
    }
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

/// Edit commands. Serialised with a `"op"` tag so the UI can send them as
/// JSON, e.g. `{"op":"Add","layer":0,"anchor":{...},"item":{...}}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum Command {
    Add {
        layer: usize,
        anchor: Anchor,
        item: Item,
    },
    Remove {
        id: u64,
    },
    Move {
        id: u64,
        anchor: Anchor,
    },
    /// Stroke eraser: removes ink on page-anchored items of `page` with any point within
    /// `radius` of an erase point (page fractions).
    Erase {
        page: u32,
        points: Vec<(f32, f32)>,
        radius: f32,
    },
    AddLayer {
        name: String,
    },
    SetLayerVisible {
        layer: usize,
        visible: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    NoSuchItem(u64),
    NoSuchLayer(usize),
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditError::NoSuchItem(i) => write!(f, "no annotation with id {i}"),
            EditError::NoSuchLayer(l) => write!(f, "no layer {l}"),
        }
    }
}
impl std::error::Error for EditError {}

/// Command-based editor with snapshot undo/redo (up to [`UNDO_LIMIT`] steps).
#[derive(Debug, Clone)]
pub struct Editor {
    doc: Document,
    undo: Vec<Document>,
    redo: Vec<Document>,
}

impl Editor {
    pub fn new(doc: Document) -> Self {
        Editor {
            doc,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
    pub fn document(&self) -> &Document {
        &self.doc
    }
    pub fn into_document(self) -> Document {
        self.doc
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Apply a command. Returns the new item id for `Add`, the new layer index for
    /// `AddLayer`, else `None`. Clears the redo stack on success.
    pub fn apply(&mut self, cmd: Command) -> Result<Option<u64>, EditError> {
        let before = self.doc.clone();
        let d = &mut self.doc;
        let out = match cmd {
            Command::Add {
                layer,
                anchor,
                item,
            } => {
                if layer >= d.layers.len() {
                    return Err(EditError::NoSuchLayer(layer));
                }
                let id = d
                    .next_id
                    .max(d.items.iter().map(|a| a.id + 1).max().unwrap_or(1));
                d.next_id = id + 1;
                d.items.push(Annotation {
                    id,
                    layer,
                    anchor,
                    item,
                });
                Some(id)
            }
            Command::Remove { id } => {
                let i = d
                    .items
                    .iter()
                    .position(|a| a.id == id)
                    .ok_or(EditError::NoSuchItem(id))?;
                d.items.remove(i);
                None
            }
            Command::Move { id, anchor } => {
                let a = d
                    .items
                    .iter_mut()
                    .find(|a| a.id == id)
                    .ok_or(EditError::NoSuchItem(id))?;
                a.anchor = anchor;
                None
            }
            Command::Erase {
                page,
                points,
                radius,
            } => {
                let r2 = radius * radius;
                d.items.retain(|a| {
                    let (
                        Anchor::Page {
                            page: p,
                            x: ax,
                            y: ay,
                        },
                        Item::Ink(ink),
                    ) = (&a.anchor, &a.item)
                    else {
                        return true;
                    };
                    if *p != page {
                        return true;
                    }
                    !ink.points.iter().any(|q| {
                        points.iter().any(|&(ex, ey)| {
                            let (dx, dy) = (ax + q.x - ex, ay + q.y - ey);
                            dx * dx + dy * dy <= r2
                        })
                    })
                });
                if d.items.len() == before.items.len() {
                    return Ok(None);
                }
                None
            }
            Command::AddLayer { name } => {
                d.layers.push(Layer {
                    name,
                    visible: true,
                });
                Some((d.layers.len() - 1) as u64)
            }
            Command::SetLayerVisible { layer, visible } => {
                d.layers
                    .get_mut(layer)
                    .ok_or(EditError::NoSuchLayer(layer))?
                    .visible = visible;
                None
            }
        };
        self.undo.push(before);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        Ok(out)
    }

    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(prev) => {
                self.redo.push(std::mem::replace(&mut self.doc, prev));
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.redo.pop() {
            Some(next) => {
                self.undo.push(std::mem::replace(&mut self.doc, next));
                true
            }
            None => false,
        }
    }
}

/// Ramer-Douglas-Peucker simplification; keeps endpoints and points deviating more
/// than `tolerance` (page fractions) from the simplified polyline.
pub fn simplify(points: &[InkPoint], tolerance: f32) -> Vec<InkPoint> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut stack = vec![(0usize, points.len() - 1)];
    while let Some((s, e)) = stack.pop() {
        let (a, b) = (points[s], points[e]);
        let (vx, vy) = (b.x - a.x, b.y - a.y);
        let len = (vx * vx + vy * vy).sqrt();
        let mut best = (0.0f32, 0usize);
        for (i, p) in points.iter().enumerate().take(e).skip(s + 1) {
            let d = if len == 0.0 {
                ((p.x - a.x).powi(2) + (p.y - a.y).powi(2)).sqrt()
            } else {
                ((p.x - a.x) * vy - (p.y - a.y) * vx).abs() / len
            };
            if d > best.0 {
                best = (d, i);
            }
        }
        if best.0 > tolerance {
            keep[best.1] = true;
            stack.push((s, best.1));
            stack.push((best.1, e));
        }
    }
    points
        .iter()
        .zip(keep)
        .filter(|(_, k)| *k)
        .map(|(p, _)| *p)
        .collect()
}

/// An item with its anchor already resolved to page fractions.
#[derive(Debug, Clone, Copy)]
pub struct Placed<'a> {
    pub x: f32,
    pub y: f32,
    pub item: &'a Item,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Render one page's annotations as an SVG document `width` x `height` pixels.
/// Raster layers for other pages are skipped.
pub fn to_svg(page: u32, items: &[Placed], width: f32, height: f32) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">"
    );
    let (w, h) = (width, height);
    for p in items {
        let (ox, oy) = (p.x * w, p.y * h);
        match p.item {
            Item::Raster(r) if r.page == page => {
                let _ = writeln!(
                    s,
                    "<image x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" href=\"data:image/png;base64,{}\"/>",
                    base64(&r.png_bytes)
                );
            }
            Item::Raster(_) => {}
            Item::Ink(ink) => {
                let opacity = if ink.tool == InkTool::Highlighter {
                    " stroke-opacity=\"0.4\""
                } else {
                    ""
                };
                for seg in ink.points.windows(2) {
                    let sw = ink.width * h * (seg[0].pressure + seg[1].pressure) / 2.0;
                    let _ = writeln!(
                        s,
                        "<path d=\"M{} {} L{} {}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{sw}\" stroke-linecap=\"round\"{opacity}/>",
                        ox + seg[0].x * w,
                        oy + seg[0].y * h,
                        ox + seg[1].x * w,
                        oy + seg[1].y * h,
                        esc(&ink.color)
                    );
                }
            }
            Item::Text(t) => {
                let _ = writeln!(
                    s,
                    "<text x=\"{ox}\" y=\"{oy}\" font-size=\"{}\" fill=\"{}\">{}</text>",
                    t.size * h,
                    esc(&t.color),
                    esc(&t.text)
                );
            }
            Item::Stamp(st) => {
                let _ = writeln!(
                    s,
                    "<text x=\"{ox}\" y=\"{oy}\" font-size=\"{}\" font-style=\"italic\" font-weight=\"bold\" fill=\"#000\">{}</text>",
                    st.pixel_size(h),
                    esc(&st.kind.glyph())
                );
            }
            Item::Shape(sh) => {
                let (sw, shh) = (sh.w * w, sh.h * h);
                let c = esc(&sh.color);
                match sh.kind {
                    ShapeKind::Rect => {
                        let _ = writeln!(s, "<rect x=\"{ox}\" y=\"{oy}\" width=\"{sw}\" height=\"{shh}\" fill=\"none\" stroke=\"{c}\" stroke-width=\"2\"/>");
                    }
                    ShapeKind::Ellipse => {
                        let _ = writeln!(
                            s,
                            "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"none\" stroke=\"{c}\" stroke-width=\"2\"/>",
                            ox + sw / 2.0,
                            oy + shh / 2.0,
                            sw / 2.0,
                            shh / 2.0
                        );
                    }
                    ShapeKind::Line | ShapeKind::Arrow => {
                        let (x2, y2) = (ox + sw, oy + shh);
                        let _ = writeln!(s, "<line x1=\"{ox}\" y1=\"{oy}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"{c}\" stroke-width=\"2\"/>");
                        if sh.kind == ShapeKind::Arrow {
                            let len = (sw * sw + shh * shh).sqrt().max(1e-6);
                            let (ux, uy) = (sw / len, shh / len);
                            let hl = (len * 0.2).clamp(4.0, 16.0);
                            let (bx, by) = (x2 - ux * hl, y2 - uy * hl);
                            let (px, py) = (-uy * hl * 0.5, ux * hl * 0.5);
                            let _ = writeln!(
                                s,
                                "<polygon points=\"{x2},{y2} {},{} {},{}\" fill=\"{c}\"/>",
                                bx + px,
                                by + py,
                                bx - px,
                                by - py
                            );
                        }
                    }
                }
            }
        }
    }
    s.push_str("</svg>\n");
    s
}
