//! Focus (WP-U0). Chrome behaviour: focus() on an unfocusable element does nothing; a focus change fires blur and
//! focusout on the old element (with no focused element at that moment) and then focus and focusin on the new one;
//! blur() on a focused element fires blur then focusout; removing or moving a focused subtree fires blur and focusout
//! on the focused element while it is still attached, then clears the focus. Not modelled: rendering-dependent focus
//! (display:none, visibility, `area` with a map), and focus fixup after attribute changes such as tabindex removal.

use crate::error::DomError;
use crate::events::Invoke;
use crate::node::{NodeId, NodeKind, Namespace};
use crate::value::parse_html_integer;
use crate::Document;

impl Document {
    pub fn active_element(&self) -> Option<NodeId> {
        self.focused
    }

    /// The tabIndex IDL value: a valid tabindex attribute, else 0 for focusable-by-default HTML elements, else -1.
    pub fn tab_index(&self, id: NodeId) -> i32 {
        let attr = self.attr_raw(id, "tabindex").and_then(|v| parse_html_integer(&v));
        if let Some(v) = attr {
            return v;
        }
        let n = self.n(id);
        let by_default = n.ns == Namespace::Html
            && matches!(n.local.as_str(), "a" | "area" | "button" | "input" | "select" | "textarea" | "iframe");
        if by_default { 0 } else { -1 }
    }

    /// Focusable: connected, an element, not a disabled control, not an input type=hidden, and either a valid
    /// tabindex or one of the focusable HTML elements (a with href, button, input, select, textarea, iframe).
    pub(crate) fn is_focusable(&self, id: NodeId) -> bool {
        let n = self.n(id);
        if n.kind != NodeKind::Element || !self.is_connected(id) || self.is_disabled_control(id) {
            return false;
        }
        if self.is_html_input(id) && self.input_type_name(id) == Some("hidden") {
            return false;
        }
        if self.attr_raw(id, "tabindex").and_then(|v| parse_html_integer(&v)).is_some() {
            return true;
        }
        if n.ns != Namespace::Html {
            return false;
        }
        match n.local.as_str() {
            "a" => self.attr_raw(id, "href").is_some(),
            "button" | "input" | "select" | "textarea" | "iframe" => true,
            _ => false,
        }
    }

    pub fn focus(&mut self, id: NodeId, inv: &mut Invoke<'_>) -> Result<(), DomError> {
        self.check(id)?;
        if !self.is_focusable(id) || self.focused == Some(id) {
            return Ok(());
        }
        if let Some(old) = self.focused.take() {
            self.fire(old, "blur", false, inv);
            self.fire(old, "focusout", true, inv);
        }
        // A blur listener may have released, detached or disabled the new target.
        self.check(id)?;
        if !self.is_focusable(id) {
            return Ok(());
        }
        self.focused = Some(id);
        self.fire(id, "focus", false, inv);
        self.fire(id, "focusin", true, inv);
        Ok(())
    }

    pub fn blur(&mut self, id: NodeId, inv: &mut Invoke<'_>) -> Result<(), DomError> {
        self.check(id)?;
        if self.focused != Some(id) {
            return Ok(());
        }
        self.focused = None;
        self.fire(id, "blur", false, inv);
        self.fire(id, "focusout", true, inv);
        Ok(())
    }

    /// Called before `child` leaves its parent or moves: if the focused element is inside it, focus is lost first.
    pub(crate) fn prepare_removal(&mut self, child: NodeId, inv: &mut Invoke<'_>) {
        if let Some(f) = self.focused {
            if self.contains(child, f) {
                self.focused = None;
                self.fire(f, "blur", false, inv);
                self.fire(f, "focusout", true, inv);
            }
        }
    }
}
