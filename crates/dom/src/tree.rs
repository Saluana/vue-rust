//! Tree structure: insertion (with move and the pre-insertion checks), removal, navigation, text content, release.
//! Mutations that can remove a focused element take the event invoker: Chrome fires blur and focusout first.

use crate::error::DomError;
use crate::events::Invoke;
use crate::node::{NodeId, NodeKind};
use crate::Document;

impl Document {
    pub fn parent_node(&self, id: NodeId) -> Option<NodeId> {
        self.live(id)?.parent
    }

    pub fn first_child(&self, id: NodeId) -> Option<NodeId> {
        self.live(id)?.first
    }

    pub fn last_child(&self, id: NodeId) -> Option<NodeId> {
        self.live(id)?.last
    }

    pub fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.live(id)?.next
    }

    pub fn previous_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.live(id)?.prev
    }

    /// childNodes in tree order (a snapshot, not a live list).
    pub fn child_nodes(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut c = self.live(id).and_then(|n| n.first);
        while let Some(x) = c {
            out.push(x);
            c = self.n(x).next;
        }
        out
    }

    /// Inclusive: a node contains itself.
    pub fn contains(&self, ancestor: NodeId, node: NodeId) -> bool {
        let mut cur = Some(node);
        while let Some(x) = cur {
            if x == ancestor {
                return true;
            }
            cur = self.live(x).and_then(|n| n.parent);
        }
        false
    }

    /// The topmost ancestor (the Document for a connected node).
    pub fn root_of(&self, id: NodeId) -> NodeId {
        let mut cur = id;
        while let Some(p) = self.live(cur).and_then(|n| n.parent) {
            cur = p;
        }
        cur
    }

    pub fn is_connected(&self, id: NodeId) -> bool {
        self.live(id).is_some() && self.live(self.root_of(id)).map(|n| n.kind) == Some(NodeKind::Document)
    }

    /// insertBefore / appendChild (anchor None). Moves an attached child. Errors leave the tree unchanged.
    pub fn insert_before(
        &mut self,
        parent: NodeId,
        child: NodeId,
        anchor: Option<NodeId>,
        inv: &mut Invoke<'_>,
    ) -> Result<(), DomError> {
        self.check(parent)?;
        self.check(child)?;
        if let Some(a) = anchor {
            self.check(a)?;
        }
        self.validate_insert(parent, child, anchor)?;
        if let Some(f) = self.focused {
            if self.contains(child, f) {
                self.prepare_removal(child, inv);
                // A listener may have released nodes or moved them: validate again before using them.
                self.check(parent)?;
                self.check(child)?;
                if let Some(a) = anchor {
                    self.check(a)?;
                }
                self.validate_insert(parent, child, anchor)?;
            }
        }
        // The reference child is computed before the move: inserting a child before itself keeps its place.
        let reference = if anchor == Some(child) { self.n(child).next } else { anchor };
        let old_parent = self.n(child).parent;
        self.unlink(child);
        self.link_before(parent, child, reference);
        // An inserted option that is selected wins over the current selection; the reset only fills an empty one.
        self.after_insert(child);
        self.after_tree_change(&[old_parent, Some(parent)]);
        Ok(())
    }

    pub fn append_child(&mut self, parent: NodeId, child: NodeId, inv: &mut Invoke<'_>) -> Result<(), DomError> {
        self.insert_before(parent, child, None, inv)
    }

    /// removeChild: `child` must be a child of `parent`.
    pub fn remove_child(&mut self, parent: NodeId, child: NodeId, inv: &mut Invoke<'_>) -> Result<(), DomError> {
        self.check(parent)?;
        self.check(child)?;
        if self.n(child).parent != Some(parent) {
            return Err(DomError::NotFoundError);
        }
        self.prepare_removal(child, inv);
        self.check(parent)?;
        self.check(child)?;
        if self.n(child).parent != Some(parent) {
            return Err(DomError::NotFoundError);
        }
        self.unlink(child);
        self.after_tree_change(&[Some(parent)]);
        Ok(())
    }

    /// ChildNode.remove(): removes from the parent if there is one.
    pub fn detach(&mut self, child: NodeId, inv: &mut Invoke<'_>) -> Result<(), DomError> {
        self.check(child)?;
        match self.n(child).parent {
            Some(p) => self.remove_child(p, child, inv),
            None => Ok(()),
        }
    }

    /// Frees a detached subtree. The ids become stale; they are never reused.
    pub fn release(&mut self, id: NodeId) -> Result<(), DomError> {
        self.check(id)?;
        if self.n(id).kind == NodeKind::Document {
            return Err(DomError::InvalidNodeType);
        }
        if self.n(id).parent.is_some() {
            return Err(DomError::NotDetached);
        }
        let mut stack = vec![id];
        while let Some(x) = stack.pop() {
            stack.extend(self.child_nodes(x));
            self.nodes[x.0 as usize] = None;
        }
        Ok(())
    }

    /// nodeValue: the data of a Text or Comment node; None for every other kind (null in JavaScript).
    pub fn node_value(&self, id: NodeId) -> Option<String> {
        let n = self.live(id)?;
        matches!(n.kind, NodeKind::Text | NodeKind::Comment).then(|| n.data.clone())
    }

    /// nodeValue setter: sets the data of a Text or Comment node; no effect on other kinds.
    pub fn set_node_value(&mut self, id: NodeId, value: &str) -> Result<(), DomError> {
        self.check(id)?;
        let n = self.nm(id);
        if matches!(n.kind, NodeKind::Text | NodeKind::Comment) {
            n.data = value.to_string();
        }
        Ok(())
    }

    /// textContent: concatenated Text descendants for an element; data for Text and Comment; None for a document.
    pub fn text_content(&self, id: NodeId) -> Option<String> {
        let n = self.live(id)?;
        match n.kind {
            NodeKind::Document => None,
            NodeKind::Text | NodeKind::Comment => Some(n.data.clone()),
            NodeKind::Element => {
                let mut out = String::new();
                self.collect_text(id, &mut out);
                Some(out)
            }
        }
    }

    fn collect_text(&self, id: NodeId, out: &mut String) {
        for c in self.child_nodes(id) {
            match self.n(c).kind {
                NodeKind::Text => out.push_str(&self.n(c).data),
                NodeKind::Element => self.collect_text(c, out),
                _ => {}
            }
        }
    }

    /// textContent setter. Elements replace all children with one Text node (none when `text` is empty); Text and
    /// Comment nodes take the new data; the document is unaffected.
    pub fn set_text_content(&mut self, id: NodeId, text: &str, inv: &mut Invoke<'_>) -> Result<(), DomError> {
        self.check(id)?;
        match self.n(id).kind {
            NodeKind::Document => Ok(()),
            NodeKind::Text | NodeKind::Comment => {
                self.nm(id).data = text.to_string();
                Ok(())
            }
            NodeKind::Element => {
                for c in self.child_nodes(id) {
                    self.prepare_removal(c, inv);
                }
                // Listeners may have changed the children during the focus events: take the list again.
                self.check(id)?;
                for c in self.child_nodes(id) {
                    self.unlink(c);
                }
                if !text.is_empty() {
                    let t = self.create_text(text);
                    self.link_before(id, t, None);
                }
                self.after_tree_change(&[Some(id)]);
                Ok(())
            }
        }
    }

    fn validate_insert(&self, parent: NodeId, child: NodeId, anchor: Option<NodeId>) -> Result<(), DomError> {
        let pk = self.n(parent).kind;
        if !matches!(pk, NodeKind::Document | NodeKind::Element) {
            return Err(DomError::HierarchyRequestError);
        }
        if self.n(child).kind == NodeKind::Document || self.contains(child, parent) {
            return Err(DomError::HierarchyRequestError);
        }
        if let Some(a) = anchor {
            if self.n(a).parent != Some(parent) {
                return Err(DomError::NotFoundError);
            }
        }
        if pk == NodeKind::Document {
            match self.n(child).kind {
                NodeKind::Text => return Err(DomError::HierarchyRequestError),
                NodeKind::Element => {
                    let other = self
                        .child_nodes(parent)
                        .into_iter()
                        .any(|c| c != child && self.n(c).kind == NodeKind::Element);
                    if other {
                        return Err(DomError::HierarchyRequestError);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Detaches `child` from its parent, keeping its own subtree.
    pub(crate) fn unlink(&mut self, child: NodeId) {
        let (parent, prev, next) = {
            let n = self.n(child);
            (n.parent, n.prev, n.next)
        };
        if let Some(p) = parent {
            match prev {
                Some(pr) => self.nm(pr).next = next,
                None => self.nm(p).first = next,
            }
            match next {
                Some(nx) => self.nm(nx).prev = prev,
                None => self.nm(p).last = prev,
            }
        }
        let n = self.nm(child);
        n.parent = None;
        n.prev = None;
        n.next = None;
    }

    /// Links an unattached `child` into `parent` before `reference` (or as the last child).
    pub(crate) fn link_before(&mut self, parent: NodeId, child: NodeId, reference: Option<NodeId>) {
        let prev = match reference {
            Some(r) => self.n(r).prev,
            None => self.n(parent).last,
        };
        {
            let c = self.nm(child);
            c.parent = Some(parent);
            c.prev = prev;
            c.next = reference;
        }
        match prev {
            Some(p) => self.nm(p).next = Some(child),
            None => self.nm(parent).first = Some(child),
        }
        match reference {
            Some(r) => self.nm(r).prev = Some(child),
            None => self.nm(parent).last = Some(child),
        }
    }
}
