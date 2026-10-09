//! Attributes. HTML elements lowercase the names given to setAttribute, getAttribute and friends; other namespaces
//! keep their case. Attribute lists keep insertion order (a replaced attribute keeps its position). Changes to the
//! attributes that mirror IDL state (style, checked, selected, disabled, multiple, name, type) run the hooks in
//! form.rs and style.rs.

use crate::error::DomError;
use crate::form::Mode;
use crate::node::{valid_attribute_name, Attr, Namespace, NodeId, NodeKind, XLINK_NS, XMLNS_NS, XML_NS};
use crate::Document;

/// What happened to an attribute, as seen by the hooks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Change {
    Added,
    Replaced,
    Removed,
}

impl Document {
    pub(crate) fn is_html_element(&self, id: NodeId) -> bool {
        let n = self.n(id);
        n.kind == NodeKind::Element && n.ns == Namespace::Html
    }

    /// HTML elements match attribute names ASCII case-insensitively; the name is lowercased for them.
    fn lookup_name(&self, id: NodeId, name: &str) -> String {
        if self.is_html_element(id) {
            name.to_ascii_lowercase()
        } else {
            name.to_string()
        }
    }

    pub(crate) fn attr_index(&self, id: NodeId, qualified: &str) -> Option<usize> {
        self.n(id).attrs.iter().position(|a| a.qualified() == qualified)
    }

    /// Raw attribute value by qualified name, without the lazy style materialisation (internal reads only).
    pub(crate) fn attr_raw(&self, id: NodeId, qualified: &str) -> Option<String> {
        self.attr_index(id, qualified).map(|i| self.n(id).attrs[i].value.clone())
    }

    /// getAttribute. Reads may materialise a pending `style` attribute (see style.rs).
    pub fn get_attribute(&mut self, id: NodeId, name: &str) -> Result<Option<String>, DomError> {
        self.check(id)?;
        self.sync_style_attribute(id);
        let q = self.lookup_name(id, name);
        Ok(self.attr_raw(id, &q))
    }

    pub fn has_attribute(&mut self, id: NodeId, name: &str) -> Result<bool, DomError> {
        Ok(self.get_attribute(id, name)?.is_some())
    }

    /// Qualified names in insertion order.
    pub fn attribute_names(&mut self, id: NodeId) -> Result<Vec<String>, DomError> {
        self.check(id)?;
        self.sync_style_attribute(id);
        Ok(self.n(id).attrs.iter().map(Attr::qualified).collect())
    }

    /// setAttribute. Errors: InvalidNodeType for non-elements, InvalidCharacterError for an invalid name.
    pub fn set_attribute(&mut self, id: NodeId, name: &str, value: &str) -> Result<(), DomError> {
        self.check(id)?;
        if self.n(id).kind != NodeKind::Element {
            return Err(DomError::InvalidNodeType);
        }
        if !valid_attribute_name(name) {
            return Err(DomError::InvalidCharacterError);
        }
        let qname = self.lookup_name(id, name);
        let pre = self.type_snapshot(id, &qname);
        let change = self.write_attr(id, &qname, value);
        self.attr_hooks(id, &qname, change, pre);
        Ok(())
    }

    /// setAttributeNS. Errors: InvalidCharacterError for an invalid name, NamespaceError for a prefix without a
    /// namespace or a reserved prefix used with the wrong namespace.
    pub fn set_attribute_ns(&mut self, id: NodeId, ns: Option<&str>, qualified: &str, value: &str) -> Result<(), DomError> {
        self.check(id)?;
        if self.n(id).kind != NodeKind::Element {
            return Err(DomError::InvalidNodeType);
        }
        if !valid_attribute_name(qualified) {
            return Err(DomError::InvalidCharacterError);
        }
        let (prefix, local) = match qualified.split_once(':') {
            Some((p, l)) if !p.is_empty() && !l.is_empty() => (Some(p.to_string()), l.to_string()),
            _ => (None, qualified.to_string()),
        };
        let reserved_xml = prefix.as_deref() == Some("xml");
        let reserved_xmlns = prefix.as_deref() == Some("xmlns") || (prefix.is_none() && local == "xmlns");
        if (prefix.is_some() && ns.is_none())
            || (reserved_xml && ns != Some(XML_NS))
            || (reserved_xmlns && ns != Some(XMLNS_NS))
            || (ns == Some(XMLNS_NS) && !reserved_xmlns)
        {
            return Err(DomError::NamespaceError);
        }
        let ns_owned = ns.map(str::to_string);
        // Same namespace and local name: the value changes and the qualified name stays as it was.
        let existing = self.n(id).attrs.iter().position(|a| a.ns == ns_owned && a.local == local);
        let change = match existing {
            Some(i) => {
                self.nm(id).attrs[i].value = value.to_string();
                Change::Replaced
            }
            None => {
                self.nm(id).attrs.push(Attr { ns: ns_owned, prefix, local: local.clone(), value: value.to_string() });
                Change::Added
            }
        };
        if ns.is_none() {
            self.attr_hooks(id, &local, change, None);
        }
        Ok(())
    }

    /// removeAttribute. Removing an absent attribute changes nothing, except that `style` also clears declarations.
    pub fn remove_attribute(&mut self, id: NodeId, name: &str) -> Result<(), DomError> {
        self.check(id)?;
        if self.n(id).kind != NodeKind::Element {
            return Err(DomError::InvalidNodeType);
        }
        let qname = self.lookup_name(id, name);
        let pre = self.type_snapshot(id, &qname);
        match self.attr_index(id, &qname) {
            Some(i) => {
                self.nm(id).attrs.remove(i);
                self.attr_hooks(id, &qname, Change::Removed, pre);
            }
            None if qname == "style" => self.style_attribute_removed(id),
            None => {}
        }
        Ok(())
    }

    /// Replaces the value in place or appends. Returns whether the attribute was added or replaced.
    pub(crate) fn write_attr(&mut self, id: NodeId, qname: &str, value: &str) -> Change {
        match self.attr_index(id, qname) {
            Some(i) => {
                self.nm(id).attrs[i].value = value.to_string();
                Change::Replaced
            }
            None => {
                let attr = Attr { ns: None, prefix: None, local: qname.to_string(), value: value.to_string() };
                self.nm(id).attrs.push(attr);
                Change::Added
            }
        }
    }

    /// Before a `type` change on an input: the mode and the current value, so the hook can carry the value over.
    fn type_snapshot(&self, id: NodeId, qname: &str) -> Option<(Mode, String)> {
        (qname == "type" && self.n(id).kind == NodeKind::Element && self.is_html_input(id))
            .then(|| (self.input_mode(id), self.input_value(id)))
    }

    /// The IDL-mirroring hooks for one attribute change.
    fn attr_hooks(&mut self, id: NodeId, qname: &str, change: Change, pre: Option<(Mode, String)>) {
        match qname {
            "style" if change == Change::Removed => self.style_attribute_removed(id),
            "style" => self.style_attribute_changed(id),
            "checked" => self.checked_attribute_changed(id, change),
            "selected" => self.selected_attribute_changed(id, change),
            "disabled" => self.option_disabled_changed(id),
            "multiple" => self.multiple_attribute_changed(id),
            "name" => self.radio_name_changed(id),
            "type" => {
                if let Some((old_mode, old_value)) = pre {
                    self.input_type_changed(id, old_mode, old_value);
                }
            }
            _ => {}
        }
    }

    /// The qualified attribute name as the HTML serializer prints it: namespaced attributes get their conventional
    /// prefix (xlink:, xml:, xmlns).
    pub(crate) fn serial_attr_name(attr: &Attr) -> String {
        match attr.ns.as_deref() {
            Some(XLINK_NS) => format!("xlink:{}", attr.local),
            Some(XML_NS) => format!("xml:{}", attr.local),
            Some(XMLNS_NS) if attr.local == "xmlns" => "xmlns".to_string(),
            Some(XMLNS_NS) => format!("xmlns:{}", attr.local),
            _ => attr.qualified(),
        }
    }
}
