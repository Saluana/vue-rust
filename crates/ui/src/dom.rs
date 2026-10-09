//! The document as the cascade sees it. vn-dom reads attributes and form state through `&mut self` (a pending style
//! attribute is materialised on read), so the element tree is copied once into a `Snapshot` and matched from there.
//! The element states come from the document (focus, checked, disabled, empty) and from a `StateProvider` (hover,
//! active, focus-visible), which the tree owner maintains.

use vn_css::matching::{Element, ElementState};
use vn_dom::{Document, HostValue, Namespace, NodeId, NodeKind};

/// Pointer and focus states that the tree owner maintains. The defaults say that nothing is hovered or pressed and
/// that a focused element is focus-visible.
pub trait StateProvider {
    fn hover(&self, _id: NodeId) -> bool {
        false
    }
    fn active(&self, _id: NodeId) -> bool {
        false
    }
    fn focus_visible(&self, _id: NodeId) -> bool {
        true
    }
}

/// No hover, no press; focused elements are focus-visible.
pub struct NoStates;

impl StateProvider for NoStates {}

#[derive(Debug, Clone)]
pub(crate) struct SnapEl {
    pub node: NodeId,
    pub local: String,
    pub html: bool,
    pub attrs: Vec<(String, String)>,
    pub parent: Option<usize>,
    pub prev: Option<usize>,
    pub next: Option<usize>,
    pub first: Option<usize>,
    pub last: Option<usize>,
    pub root: bool,
    pub state: ElementState,
}

/// Elements in document order (pre-order); indices refer to this list.
#[derive(Debug, Clone, Default)]
pub(crate) struct Snapshot {
    pub els: Vec<SnapEl>,
}

const FORM_CONTROLS: &[&str] = &["button", "input", "select", "textarea", "optgroup", "option", "fieldset"];

/// Copies the element tree under the document node (the first element child is the root element).
pub(crate) fn snapshot(doc: &mut Document, states: &dyn StateProvider) -> Snapshot {
    let mut snap = Snapshot::default();
    let top: Vec<NodeId> = doc.child_nodes(doc.document());
    if let Some(root) = top.into_iter().find(|n| doc.kind(*n) == Some(NodeKind::Element)) {
        visit(doc, root, None, true, &mut snap, states);
    }
    // Focus-within: the focused element and each of its element ancestors.
    if let Some(focus) = doc.active_element() {
        let mut cur = snap.els.iter().position(|e| e.node == focus);
        while let Some(i) = cur {
            snap.els[i].state.focus_within = true;
            cur = snap.els[i].parent;
        }
    }
    snap
}

fn visit(doc: &mut Document, node: NodeId, parent: Option<usize>, root: bool, out: &mut Snapshot, states: &dyn StateProvider) -> usize {
    let idx = out.els.len();
    let local = doc.local_name(node).unwrap_or("").to_string();
    let html = doc.namespace(node) == Some(Namespace::Html);
    let mut attrs = Vec::new();
    for name in doc.attribute_names(node).unwrap_or_default() {
        if let Ok(Some(value)) = doc.get_attribute(node, &name) {
            attrs.push((name, value));
        }
    }
    let focused = doc.active_element() == Some(node);
    let has_attr = |k: &str| attrs.iter().any(|(n, _)| n == k);
    let disabled = html && FORM_CONTROLS.contains(&local.as_str()) && has_attr("disabled");
    let form_control = html && FORM_CONTROLS.contains(&local.as_str());
    let checked = match local.as_str() {
        "input" if html => matches!(doc.get_property(node, "checked"), Ok(HostValue::Bool(true))),
        "option" if html => matches!(doc.get_property(node, "selected"), Ok(HostValue::Bool(true))),
        _ => false,
    };
    let children = doc.child_nodes(node);
    let empty = children.iter().all(|c| match doc.kind(*c) {
        Some(NodeKind::Comment) => true,
        Some(NodeKind::Text) => doc.node_value(*c).is_none_or(|s| s.is_empty()),
        _ => false,
    });
    let state = ElementState {
        hover: states.hover(node),
        active: states.active(node),
        focus: focused,
        focus_visible: focused && states.focus_visible(node),
        focus_within: focused,
        disabled: disabled,
        enabled: form_control && !disabled,
        checked,
        empty,
    };
    out.els.push(SnapEl { node, local, html, attrs, parent, prev: None, next: None, first: None, last: None, root, state });
    let mut kids = Vec::new();
    for c in children {
        if doc.kind(c) == Some(NodeKind::Element) {
            kids.push(visit(doc, c, Some(idx), false, out, states));
        }
    }
    for (k, &ci) in kids.iter().enumerate() {
        out.els[ci].prev = if k > 0 { Some(kids[k - 1]) } else { None };
        out.els[ci].next = kids.get(k + 1).copied();
    }
    out.els[idx].first = kids.first().copied();
    out.els[idx].last = kids.last().copied();
    idx
}

/// An element of a snapshot, as the matcher sees it.
#[derive(Clone, Copy)]
pub(crate) struct El<'a> {
    pub snap: &'a Snapshot,
    pub i: usize,
}

impl El<'_> {
    pub fn data(&self) -> &SnapEl {
        &self.snap.els[self.i]
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.data().attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
}

impl<'a> Element for El<'a> {
    fn local_name(&self) -> &str {
        &self.data().local
    }

    fn namespace_is_html(&self) -> bool {
        self.data().html
    }

    fn id(&self) -> Option<&str> {
        self.attr("id")
    }

    fn has_class(&self, class: &str) -> bool {
        self.attr("class").is_some_and(|c| c.split(|ch: char| ch.is_ascii_whitespace()).any(|w| w == class))
    }

    fn attribute(&self, name: &str) -> Option<&str> {
        self.attr(name)
    }

    fn parent_element(&self) -> Option<Self> {
        self.data().parent.map(|p| El { snap: self.snap, i: p })
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        self.data().prev.map(|p| El { snap: self.snap, i: p })
    }

    fn next_sibling_element(&self) -> Option<Self> {
        self.data().next.map(|p| El { snap: self.snap, i: p })
    }

    fn first_child_element(&self) -> Option<Self> {
        self.data().first.map(|p| El { snap: self.snap, i: p })
    }

    fn last_child_element(&self) -> Option<Self> {
        self.data().last.map(|p| El { snap: self.snap, i: p })
    }

    fn is_root(&self) -> bool {
        self.data().root
    }

    fn state(&self) -> ElementState {
        self.data().state
    }
}
