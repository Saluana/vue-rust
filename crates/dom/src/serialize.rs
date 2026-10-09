//! Serialization (WP-U0). outerHTML and innerHTML follow the HTML fragment serialization algorithm (escaping, void
//! elements, raw text elements, foreign elements with end tags). `snapshot` is the canonical JSON the differential
//! tests compare; its rule is implemented in the same way in tests/dom/gen/page-runner.mjs (snap):
//!   element  {"$":"el","tag":L[,"ns":"svg"|"math"],"attrs":[[qualifiedName,value],...][,"props":{...}],"children":[...]}
//!   text     {"$":"text","v":D}          comment {"$":"comment","v":D}          document {"$":"doc","children":[...]}
//! props lists value (input, textarea), checked (input) and selected (option) when they differ from the value their
//! attributes give: the value attribute (or "on" / "" when absent), the textarea's normalized text, the checked and
//! selected attributes. The disabled rule never fires here because the disabled IDL property reflects its attribute.

use crate::node::{NodeId, NodeKind, Namespace};
use crate::Document;

const VOID: &[&str] = &[
    "area", "base", "basefont", "bgsound", "br", "col", "embed", "frame", "hr", "img", "input", "keygen", "link", "meta",
    "param", "source", "track", "wbr",
];
const RAW_TEXT: &[&str] = &["style", "script", "xmp", "iframe", "noembed", "noframes"];

fn escape_text(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '\u{a0}' => o.push_str("&nbsp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            c => o.push(c),
        }
    }
    o
}

fn escape_attr(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '\u{a0}' => o.push_str("&nbsp;"),
            '"' => o.push_str("&quot;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            c => o.push(c),
        }
    }
    o
}

/// JSON string literal as JSON.stringify writes it (lowercase \u escapes for control characters).
pub(crate) fn json_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\u{8}' => o.push_str("\\b"),
            '\u{c}' => o.push_str("\\f"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

impl Document {
    /// outerHTML of an element; None for other nodes (undefined in JavaScript).
    pub fn outer_html(&mut self, id: NodeId) -> Option<String> {
        self.check(id).ok()?;
        if self.n(id).kind != NodeKind::Element {
            return None;
        }
        let mut out = String::new();
        self.ser_element(id, &mut out);
        Some(out)
    }

    /// innerHTML of an element; None for other nodes (the Document has no innerHTML property).
    pub fn inner_html(&mut self, id: NodeId) -> Option<String> {
        self.check(id).ok()?;
        if self.n(id).kind != NodeKind::Element {
            return None;
        }
        let mut out = String::new();
        self.ser_children(id, false, &mut out);
        Some(out)
    }

    fn ser_element(&mut self, id: NodeId, out: &mut String) {
        self.sync_style_attribute(id);
        let (tag, html) = {
            let n = self.n(id);
            (n.local.clone(), n.ns == Namespace::Html)
        };
        out.push('<');
        out.push_str(&tag);
        for a in &self.n(id).attrs {
            out.push(' ');
            out.push_str(&Document::serial_attr_name(a));
            out.push_str("=\"");
            out.push_str(&escape_attr(&a.value));
            out.push('"');
        }
        out.push('>');
        if html && VOID.contains(&tag.as_str()) {
            return;
        }
        let raw = html && RAW_TEXT.contains(&tag.as_str());
        self.ser_children(id, raw, out);
        out.push_str("</");
        out.push_str(&tag);
        out.push('>');
    }

    fn ser_children(&mut self, parent: NodeId, raw: bool, out: &mut String) {
        for c in self.child_nodes(parent) {
            match self.n(c).kind {
                NodeKind::Text => {
                    let d = &self.n(c).data;
                    out.push_str(&if raw { d.clone() } else { escape_text(d) });
                }
                NodeKind::Comment => {
                    out.push_str("<!--");
                    out.push_str(&self.n(c).data);
                    out.push_str("-->");
                }
                NodeKind::Element => self.ser_element(c, out),
                NodeKind::Document => {}
            }
        }
    }

    /// The canonical snapshot of a node and its subtree (see the module comment).
    pub fn snapshot(&mut self, id: NodeId) -> Result<String, crate::DomError> {
        self.check(id)?;
        let mut out = String::new();
        self.snap(id, &mut out);
        Ok(out)
    }

    fn snap(&mut self, id: NodeId, out: &mut String) {
        match self.n(id).kind {
            NodeKind::Document => {
                out.push_str("{\"$\":\"doc\",\"children\":[");
                self.snap_children(id, out);
                out.push_str("]}");
            }
            NodeKind::Text => {
                let v = json_str(&self.n(id).data);
                out.push_str(&format!("{{\"$\":\"text\",\"v\":{v}}}"));
            }
            NodeKind::Comment => {
                let v = json_str(&self.n(id).data);
                out.push_str(&format!("{{\"$\":\"comment\",\"v\":{v}}}"));
            }
            NodeKind::Element => {
                self.sync_style_attribute(id);
                let (tag, ns, attrs) = {
                    let n = self.n(id);
                    (n.local.clone(), n.ns, n.attrs.clone())
                };
                out.push_str(&format!("{{\"$\":\"el\",\"tag\":{}", json_str(&tag)));
                match ns {
                    Namespace::Html => {}
                    Namespace::Svg => out.push_str(",\"ns\":\"svg\""),
                    Namespace::MathMl => out.push_str(",\"ns\":\"math\""),
                }
                let pairs: Vec<String> = attrs
                    .iter()
                    .map(|a| format!("[{},{}]", json_str(&a.qualified()), json_str(&a.value)))
                    .collect();
                out.push_str(&format!(",\"attrs\":[{}]", pairs.join(",")));
                let props = self.snapshot_props(id);
                if !props.is_empty() {
                    out.push_str(&format!(",\"props\":{{{}}}", props.join(",")));
                }
                out.push_str(",\"children\":[");
                self.snap_children(id, out);
                out.push_str("]}");
            }
        }
    }

    fn snap_children(&mut self, id: NodeId, out: &mut String) {
        for (i, c) in self.child_nodes(id).into_iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            self.snap(c, out);
        }
    }

    /// The "key":value items of the snapshot props object, in the order value, checked, selected.
    fn snapshot_props(&self, id: NodeId) -> Vec<String> {
        let mut out = Vec::new();
        if self.is_html_input(id) {
            let fallback = match self.input_type_name(id) {
                Some("checkbox") | Some("radio") => "on",
                _ => "",
            };
            let default = self.attr_raw(id, "value").unwrap_or_else(|| fallback.to_string());
            let value = self.input_value(id);
            if value != default {
                out.push(format!("\"value\":{}", json_str(&value)));
            }
            let checked = self.n(id).form.checked;
            if checked != self.attr_raw(id, "checked").is_some() {
                out.push(format!("\"checked\":{checked}"));
            }
        }
        if self.is_html_element_named(id, "textarea") {
            let default = self.text_content(id).unwrap_or_default().replace("\r\n", "\n").replace('\r', "\n");
            let value = self.textarea_value(id);
            if value != default {
                out.push(format!("\"value\":{}", json_str(&value)));
            }
        }
        if self.is_html_option(id) {
            let selected = self.n(id).form.selected;
            if selected != self.attr_raw(id, "selected").is_some() {
                out.push(format!("\"selected\":{selected}"));
            }
        }
        out
    }
}
