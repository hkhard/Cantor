//! Minimal element tree built from quick-xml events.
//!
//! MusicXML files are small enough (a few MB at most) that a tree is simpler
//! and safer than a hand-written streaming state machine.

use quick_xml::events::Event as XmlEvent;
use quick_xml::Reader;

#[derive(Debug, Default)]
pub(crate) struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Element>,
    pub text: String,
}

impl Element {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.name == name)
    }

    pub fn children<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    pub fn child_text(&self, name: &str) -> Option<&str> {
        self.child(name).map(|c| c.text.trim())
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum DomError {
    #[error("malformed XML at byte {pos}: {msg}")]
    Xml { pos: u64, msg: String },
    #[error("document is empty")]
    Empty,
    #[error("unexpected end of document inside <{0}>")]
    Unclosed(String),
}

fn element_from(e: &quick_xml::events::BytesStart<'_>, pos: u64) -> Result<Element, DomError> {
    let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
    let mut attrs = Vec::new();
    for a in e.attributes() {
        let a = a.map_err(|err| DomError::Xml {
            pos,
            msg: err.to_string(),
        })?;
        let key = String::from_utf8_lossy(a.key.as_ref()).into_owned();
        let value = a
            .unescape_value()
            .map_err(|err| DomError::Xml {
                pos,
                msg: err.to_string(),
            })?
            .into_owned();
        attrs.push((key, value));
    }
    Ok(Element {
        name,
        attrs,
        ..Default::default()
    })
}

pub(crate) fn parse(bytes: &[u8]) -> Result<Element, DomError> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;

    loop {
        let pos = reader.buffer_position();
        let ev = reader
            .read_event_into(&mut buf)
            .map_err(|err| DomError::Xml {
                pos,
                msg: err.to_string(),
            })?;
        match ev {
            XmlEvent::Start(e) => stack.push(element_from(&e, pos)?),
            XmlEvent::Empty(e) => {
                let el = element_from(&e, pos)?;
                attach(&mut stack, &mut root, el, pos)?;
            }
            XmlEvent::End(_) => {
                let el = stack.pop().ok_or(DomError::Xml {
                    pos,
                    msg: "closing tag without opening tag".into(),
                })?;
                attach(&mut stack, &mut root, el, pos)?;
            }
            XmlEvent::Text(t) => {
                let s = t.unescape().map_err(|err| DomError::Xml {
                    pos,
                    msg: err.to_string(),
                })?;
                match stack.last_mut() {
                    Some(top) => top.text.push_str(&s),
                    None if s.trim().is_empty() => {}
                    None => {
                        return Err(DomError::Xml {
                            pos,
                            msg: "text outside root element".into(),
                        })
                    }
                }
            }
            XmlEvent::CData(t) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&String::from_utf8_lossy(&t));
                }
            }
            XmlEvent::Eof => break,
            // Declarations, DOCTYPE, comments and processing instructions are ignored.
            _ => {}
        }
        buf.clear();
    }

    if let Some(open) = stack.pop() {
        return Err(DomError::Unclosed(open.name));
    }
    root.ok_or(DomError::Empty)
}

fn attach(
    stack: &mut [Element],
    root: &mut Option<Element>,
    el: Element,
    pos: u64,
) -> Result<(), DomError> {
    match stack.last_mut() {
        Some(parent) => parent.children.push(el),
        None if root.is_none() => *root = Some(el),
        None => {
            return Err(DomError::Xml {
                pos,
                msg: "more than one root element".into(),
            });
        }
    }
    Ok(())
}
