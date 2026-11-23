//! A small ordered XML tree, serialised the way Python's
//! `xml.etree.ElementTree` writes it (`<a />` for empty elements, attribute
//! order kept), so the output can be compared with homr's byte for byte.

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    El(El),
    Comment(String),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct El {
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    pub text: Option<String>,
    pub children: Vec<Node>,
}

impl El {
    pub fn new(tag: &str) -> El {
        El {
            tag: tag.to_string(),
            ..El::default()
        }
    }
    pub fn with_attr(mut self, k: &str, v: &str) -> El {
        self.set(k, v);
        self
    }
    pub fn with_text(mut self, t: impl Into<String>) -> El {
        self.text = Some(t.into());
        self
    }
    pub fn get(&self, k: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(a, _)| a == k)
            .map(|(_, v)| v.as_str())
    }
    pub fn set(&mut self, k: &str, v: &str) {
        match self.attrs.iter_mut().find(|(a, _)| a == k) {
            Some(e) => e.1 = v.to_string(),
            None => self.attrs.push((k.to_string(), v.to_string())),
        }
    }
    pub fn remove_attr(&mut self, k: &str) {
        self.attrs.retain(|(a, _)| a != k);
    }
    /// `ET.SubElement`: appends and returns the new child.
    pub fn sub(&mut self, child: El) -> &mut El {
        self.children.push(Node::El(child));
        match self.children.last_mut() {
            Some(Node::El(e)) => e,
            _ => unreachable!(),
        }
    }
    /// Appends a child element with text.
    pub fn sub_text(&mut self, tag: &str, text: impl Into<String>) {
        self.sub(El::new(tag).with_text(text));
    }
    pub fn push(&mut self, child: El) {
        self.children.push(Node::El(child));
    }
    pub fn elements(&self) -> impl Iterator<Item = &El> {
        self.children.iter().filter_map(|n| match n {
            Node::El(e) => Some(e),
            Node::Comment(_) => None,
        })
    }
    pub fn elements_mut(&mut self) -> impl Iterator<Item = &mut El> {
        self.children.iter_mut().filter_map(|n| match n {
            Node::El(e) => Some(e),
            Node::Comment(_) => None,
        })
    }
    /// `find(tag)`: the first child element with this tag.
    pub fn find(&self, tag: &str) -> Option<&El> {
        self.elements().find(|e| e.tag == tag)
    }
    pub fn find_mut(&mut self, tag: &str) -> Option<&mut El> {
        self.elements_mut().find(|e| e.tag == tag)
    }
    /// `findtext(tag, default)`.
    pub fn findtext<'a>(&'a self, tag: &str, default: &'a str) -> &'a str {
        match self.find(tag) {
            Some(e) => e.text.as_deref().unwrap_or(""),
            None => default,
        }
    }
    /// The index in `children` of the first element with this tag.
    pub fn index_of(&self, tag: &str) -> Option<usize> {
        self.children
            .iter()
            .position(|n| matches!(n, Node::El(e) if e.tag == tag))
    }
    pub fn child_mut(&mut self, index: usize) -> &mut El {
        match &mut self.children[index] {
            Node::El(e) => e,
            Node::Comment(_) => panic!("not an element"),
        }
    }
    pub fn child(&self, index: usize) -> &El {
        match &self.children[index] {
            Node::El(e) => e,
            Node::Comment(_) => panic!("not an element"),
        }
    }

    pub fn write(&self, out: &mut String) {
        out.push('<');
        out.push_str(&self.tag);
        for (k, v) in &self.attrs {
            out.push(' ');
            out.push_str(k);
            out.push_str("=\"");
            escape(v, true, out);
            out.push('"');
        }
        let text = self.text.as_deref().filter(|t| !t.is_empty());
        if text.is_none() && self.children.is_empty() {
            out.push_str(" />");
            return;
        }
        out.push('>');
        if let Some(t) = text {
            escape(t, false, out);
        }
        for c in &self.children {
            match c {
                Node::El(e) => e.write(out),
                Node::Comment(t) => {
                    out.push_str("<!--");
                    out.push_str(t);
                    out.push_str("-->");
                }
            }
        }
        out.push_str("</");
        out.push_str(&self.tag);
        out.push('>');
    }
}

fn escape(s: &str, attr: bool, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            '\n' if attr => out.push_str("&#10;"),
            _ => out.push(c),
        }
    }
}

/// The document as `ElementTree.write(..., encoding="unicode", xml_declaration=True)`.
pub fn to_document(root: &El) -> String {
    let mut out = String::from("<?xml version='1.0' encoding='utf-8'?>\n");
    root.write(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lib_06_serialises_like_elementtree() {
        let mut root = El::new("a").with_attr("x", "1&\"");
        root.sub(El::new("b"));
        root.sub_text("c", "<t>");
        root.sub(El::new("d").with_text(""));
        root.children.push(Node::Comment(" hi ".into()));
        assert_eq!(
            to_document(&root),
            "<?xml version='1.0' encoding='utf-8'?>\n<a x=\"1&amp;&quot;\"><b /><c>&lt;t&gt;</c><d /><!-- hi --></a>"
        );
        assert_eq!(root.findtext("c", "z"), "<t>");
        assert_eq!(root.findtext("q", "z"), "z");
        assert_eq!(root.index_of("d"), Some(2));
    }
}
