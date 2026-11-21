//! forScore import: `.4sc` scores (LIB-07) and `.4ss` setlists (LIB-08).
//!
//! Format notes: `docs/formats/forscore.md`. Both are gzip-wrapped binary plists.
//! A `.4sc` is a flat dictionary; a `.4ss` is an NSKeyedArchiver archive whose
//! `files` entries are `.4sc`-shaped dictionaries.
//!
//! Unknown keys are kept verbatim in [`ForscoreScore::unknown`] so nothing is
//! dropped silently.

use flate2::read::GzDecoder;
use plist::{Dictionary, Value};
use std::collections::BTreeMap;
use std::io::Read;

#[derive(Debug, thiserror::Error)]
pub enum ForscoreError {
    #[error("not a forScore file: expected gzip data")]
    NotGzip,
    #[error("corrupt gzip stream: {0}")]
    Gzip(#[from] std::io::Error),
    #[error("not a property list: {0}")]
    Plist(#[from] plist::Error),
    #[error("score has no embedded PDF (pdfData)")]
    MissingPdf,
    #[error("score has no file-name keys, cannot tell which PDF it describes")]
    MissingFileName,
    #[error("setlist is not an NSKeyedArchiver archive")]
    NotKeyedArchive,
    #[error("malformed setlist archive: {0}")]
    Archive(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Per-page view state stored by forScore (1-based page index in the map key).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PageView {
    pub rect: Option<Rect>,
    pub zoom: Option<f64>,
    pub offset: Option<Point>,
    pub tr_offset: Option<Point>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForscoreScore {
    /// Name of the PDF inside forScore, e.g. `Christmas Day - F dur.pdf`.
    pub file_name: String,
    pub title: Option<String>,
    pub pdf: Vec<u8>,
    /// View state per 1-based page. May contain more slots than the PDF has pages.
    pub pages: BTreeMap<u32, PageView>,
    /// Every key not understood yet, verbatim.
    pub unknown: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SetlistEntry {
    Score(ForscoreScore),
    /// Item without a matching file (forScore placeholder or missing file).
    Placeholder,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetlistItem {
    pub title: String,
    pub file_path: String,
    pub identifier: String,
    pub entry: SetlistEntry,
    pub unknown: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForscoreSetlist {
    pub items: Vec<SetlistItem>,
}

fn decode(bytes: &[u8]) -> Result<Value, ForscoreError> {
    if bytes.len() < 2 || bytes[0] != 0x1f || bytes[1] != 0x8b {
        return Err(ForscoreError::NotGzip);
    }
    let mut raw = Vec::new();
    GzDecoder::new(bytes).read_to_end(&mut raw)?;
    Ok(Value::from_reader(std::io::Cursor::new(raw))?)
}

/// Parse a `.4sc` file.
pub fn parse_4sc(bytes: &[u8]) -> Result<ForscoreScore, ForscoreError> {
    match decode(bytes)? {
        Value::Dictionary(d) => score_from_dict(d),
        _ => Err(ForscoreError::MissingPdf),
    }
}

fn score_from_dict(d: Dictionary) -> Result<ForscoreScore, ForscoreError> {
    let mut pdf = None;
    let mut file_name: Option<String> = None;
    let mut title = None;
    let mut pages: BTreeMap<u32, PageView> = BTreeMap::new();
    let mut unknown = BTreeMap::new();

    for (key, value) in d {
        if key == "pdfData" {
            match value {
                Value::Data(b) => pdf = Some(b),
                other => {
                    unknown.insert(key, other);
                }
            }
            continue;
        }
        let parts: Vec<&str> = key.split('|').collect();
        if parts.len() >= 2 && file_name.is_none() {
            file_name = Some(parts[0].to_string());
        }
        let known = match parts.as_slice() {
            [_, "title"] => value.as_string().map(|s| title = Some(nfc(s))).is_some(),
            [_, page, field] => match page.parse::<u32>() {
                Ok(n) => apply_page_field(pages.entry(n).or_default(), field, &value),
                Err(_) => false,
            },
            _ => false,
        };
        if !known {
            unknown.insert(key, value);
        }
    }

    // Drop page slots that only held unknown fields so `pages` reflects parsed state.
    pages.retain(|_, v| *v != PageView::default());

    let pdf = pdf.ok_or(ForscoreError::MissingPdf)?;
    let file_name = file_name.ok_or(ForscoreError::MissingFileName)?;
    Ok(ForscoreScore {
        file_name,
        title,
        pdf,
        pages,
        unknown,
    })
}

/// Display text is normalised to NFC (forScore writes NFD, like Apple file
/// systems). File names are left byte-exact so joins and round trips work.
fn nfc(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfc().collect()
}

fn apply_page_field(p: &mut PageView, field: &str, v: &Value) -> bool {
    match field {
        "rect" => v
            .as_string()
            .and_then(parse_rect)
            .map(|r| p.rect = Some(r))
            .is_some(),
        "zoom" => v.as_real().map(|z| p.zoom = Some(z)).is_some(),
        "offset" => v
            .as_string()
            .and_then(parse_point)
            .map(|o| p.offset = Some(o))
            .is_some(),
        "trOffset" => v
            .as_string()
            .and_then(parse_point)
            .map(|o| p.tr_offset = Some(o))
            .is_some(),
        _ => false,
    }
}

/// Parse numbers out of Cocoa geometry strings like `{{1, 2}, {3, 4}}`.
fn numbers(s: &str) -> Option<Vec<f64>> {
    s.split(['{', '}', ','])
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| t.parse().ok())
        .collect()
}

fn parse_point(s: &str) -> Option<Point> {
    match numbers(s)?.as_slice() {
        [x, y] => Some(Point { x: *x, y: *y }),
        _ => None,
    }
}

fn parse_rect(s: &str) -> Option<Rect> {
    match numbers(s)?.as_slice() {
        [x, y, w, h] => Some(Rect {
            x: *x,
            y: *y,
            w: *w,
            h: *h,
        }),
        _ => None,
    }
}

/// Minimal NSKeyedArchiver reader: resolves UIDs into plain plist values for the
/// Foundation collection classes forScore uses.
struct Archive {
    objects: Vec<Value>,
}

impl Archive {
    fn get<'a>(&'a self, v: &'a Value) -> Result<&'a Value, ForscoreError> {
        match v {
            Value::Uid(u) => self
                .objects
                .get(u.get() as usize)
                .ok_or_else(|| ForscoreError::Archive(format!("dangling UID {}", u.get()))),
            other => Ok(other),
        }
    }

    fn resolve(&self, v: &Value, depth: usize) -> Result<Value, ForscoreError> {
        if depth > 64 {
            return Err(ForscoreError::Archive("nesting too deep or cyclic".into()));
        }
        let v = self.get(v)?;
        let Value::Dictionary(d) = v else {
            return Ok(v.clone());
        };
        let Some(class) = d.get("$class") else {
            return Ok(v.clone());
        };
        let class = self
            .get(class)?
            .as_dictionary()
            .and_then(|c| c.get("$classname"))
            .and_then(Value::as_string)
            .unwrap_or_default();
        let list = |k: &str| -> Result<Vec<Value>, ForscoreError> {
            d.get(k)
                .and_then(Value::as_array)
                .ok_or_else(|| ForscoreError::Archive(format!("{class} without {k}")))?
                .iter()
                .map(|x| self.resolve(x, depth + 1))
                .collect()
        };
        Ok(match class {
            "NSArray" | "NSMutableArray" | "NSSet" | "NSMutableSet" => {
                Value::Array(list("NS.objects")?)
            }
            "NSDictionary" | "NSMutableDictionary" => {
                let keys = list("NS.keys")?;
                let vals = list("NS.objects")?;
                let mut out = Dictionary::new();
                for (k, v) in keys.into_iter().zip(vals) {
                    let k = k
                        .as_string()
                        .ok_or_else(|| ForscoreError::Archive("non-string dictionary key".into()))?
                        .to_string();
                    out.insert(k, v);
                }
                Value::Dictionary(out)
            }
            "NSString" | "NSMutableString" => {
                d.get("NS.string").cloned().unwrap_or(Value::from(""))
            }
            "NSData" | "NSMutableData" => match d.get("NS.data") {
                Some(x) => self.resolve(x, depth + 1)?,
                None => Value::Data(Vec::new()),
            },
            // Unknown classes are kept as their raw archived dictionary.
            _ => v.clone(),
        })
    }
}

/// Parse a `.4ss` setlist file.
pub fn parse_4ss(bytes: &[u8]) -> Result<ForscoreSetlist, ForscoreError> {
    let root = decode(bytes)?;
    let root = root.as_dictionary().ok_or(ForscoreError::NotKeyedArchive)?;
    if root.get("$archiver").and_then(Value::as_string) != Some("NSKeyedArchiver") {
        return Err(ForscoreError::NotKeyedArchive);
    }
    let objects = root
        .get("$objects")
        .and_then(Value::as_array)
        .ok_or(ForscoreError::NotKeyedArchive)?
        .clone();
    let top = root
        .get("$top")
        .and_then(Value::as_dictionary)
        .ok_or(ForscoreError::NotKeyedArchive)?;
    let ar = Archive { objects };

    let top_array = |k: &str| -> Result<Vec<Value>, ForscoreError> {
        match top.get(k) {
            None => Ok(Vec::new()),
            Some(v) => match ar.resolve(v, 0)? {
                Value::Array(a) => Ok(a),
                _ => Err(ForscoreError::Archive(format!("$top.{k} is not an array"))),
            },
        }
    };

    let mut files: BTreeMap<String, ForscoreScore> = BTreeMap::new();
    for f in top_array("files")? {
        if let Value::Dictionary(d) = f {
            let s = score_from_dict(d)?;
            files.insert(s.file_name.clone(), s);
        }
    }

    let mut items = Vec::new();
    for it in top_array("setlist")? {
        let Value::Dictionary(mut d) = it else {
            return Err(ForscoreError::Archive(
                "setlist item is not a dictionary".into(),
            ));
        };
        let mut take = |k: &str| {
            d.remove(k)
                .and_then(|v| v.into_string())
                .unwrap_or_default()
        };
        let title = nfc(&take("Title"));
        let file_path = take("FilePath");
        let identifier = take("Identifier");
        let entry = match files.get(&file_path) {
            Some(s) => SetlistEntry::Score(s.clone()),
            None => SetlistEntry::Placeholder,
        };
        items.push(SetlistItem {
            title,
            file_path,
            identifier,
            entry,
            unknown: d.into_iter().collect(),
        });
    }
    Ok(ForscoreSetlist { items })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_strings_parse() {
        assert_eq!(parse_point("{6.5, 17.6}"), Some(Point { x: 6.5, y: 17.6 }));
        assert_eq!(
            parse_rect("{{0, 0}, {768, 1004}}"),
            Some(Rect {
                x: 0.0,
                y: 0.0,
                w: 768.0,
                h: 1004.0
            })
        );
        assert_eq!(parse_point("{1}"), None);
        assert_eq!(parse_rect("{{a, 0}, {1, 1}}"), None);
    }
}

/// Fixture builders for tests in this and dependent crates.
#[doc(hidden)]
pub mod testing {
    use flate2::{write::GzEncoder, Compression};
    use plist::{Dictionary, Value};
    use std::io::Write;

    /// Minimal `.4sc`: gzip(binary plist {pdfData, "<file>|title"}).
    pub fn fake_4sc(file: &str, title: &str, pdf: &[u8]) -> Vec<u8> {
        let mut d = Dictionary::new();
        d.insert("pdfData".into(), Value::Data(pdf.to_vec()));
        d.insert(format!("{file}|title"), title.into());
        let mut raw = Vec::new();
        Value::Dictionary(d)
            .to_writer_binary(&mut raw)
            .expect("plist to Vec");
        let mut gz = GzEncoder::new(Vec::new(), Compression::default());
        gz.write_all(&raw).expect("gzip to Vec");
        gz.finish().expect("gzip to Vec")
    }
}
