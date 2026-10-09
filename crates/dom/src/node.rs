//! Node arena. NodeId 0 is always the Document node. A released slot is `None`; ids are never reused, so a stale
//! NodeId is detected instead of silently naming another node.

use crate::error::DomError;
use crate::events::Listener;
use crate::style::Decl;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub(crate) u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Document,
    Element,
    Text,
    Comment,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Namespace {
    Html,
    Svg,
    MathMl,
}

impl Namespace {
    pub fn uri(self) -> &'static str {
        match self {
            Namespace::Html => "http://www.w3.org/1999/xhtml",
            Namespace::Svg => "http://www.w3.org/2000/svg",
            Namespace::MathMl => "http://www.w3.org/1998/Math/MathML",
        }
    }
}

pub(crate) const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
pub(crate) const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";
pub(crate) const XMLNS_NS: &str = "http://www.w3.org/2000/xmlns/";

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Attr {
    pub ns: Option<String>,
    pub prefix: Option<String>,
    pub local: String,
    pub value: String,
}

impl Attr {
    pub(crate) fn qualified(&self) -> String {
        match &self.prefix {
            Some(p) => format!("{p}:{}", self.local),
            None => self.local.clone(),
        }
    }
}

/// Form-control state. `value` is the dirty value of an input (value mode) or the raw value of a textarea.
#[derive(Clone, Debug, Default)]
pub(crate) struct Form {
    pub value_dirty: bool,
    pub value: String,
    pub checked: bool,
    pub checked_dirty: bool,
    pub selected: bool,
    pub selected_dirty: bool,
}

#[derive(Debug)]
pub(crate) struct Node {
    pub kind: NodeKind,
    pub ns: Namespace,
    /// Local name of an element (lowercase for HTML elements).
    pub local: String,
    /// Character data of a Text or Comment node.
    pub data: String,
    /// The `is` value given to createElement; it has no behaviour here and no attribute.
    pub is_value: Option<String>,
    pub attrs: Vec<Attr>,
    pub parent: Option<NodeId>,
    pub first: Option<NodeId>,
    pub last: Option<NodeId>,
    pub prev: Option<NodeId>,
    pub next: Option<NodeId>,
    pub form: Form,
    pub style: Vec<Decl>,
    /// Declarations changed through CSSOM while the `style` attribute was absent. The attribute is appended when it
    /// is next read, as Chrome does (see style.rs).
    pub style_pending: bool,
    pub listeners: Vec<Listener>,
}

impl Node {
    pub(crate) fn new(kind: NodeKind, ns: Namespace, local: String, data: String) -> Node {
        Node {
            kind,
            ns,
            local,
            data,
            is_value: None,
            attrs: Vec::new(),
            parent: None,
            first: None,
            last: None,
            prev: None,
            next: None,
            form: Form::default(),
            style: Vec::new(),
            style_pending: false,
            listeners: Vec::new(),
        }
    }
}

/// The document: an arena of nodes plus the focused element and the listener counter.
#[derive(Debug)]
pub struct Document {
    pub(crate) nodes: Vec<Option<Node>>,
    pub(crate) focused: Option<NodeId>,
    pub(crate) next_uid: u64,
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    /// An empty document: no children, nothing focused.
    pub fn new() -> Document {
        let root = Node::new(NodeKind::Document, Namespace::Html, String::new(), String::new());
        Document { nodes: vec![Some(root)], focused: None, next_uid: 1 }
    }

    pub fn document(&self) -> NodeId {
        NodeId(0)
    }

    pub(crate) fn check(&self, id: NodeId) -> Result<(), DomError> {
        match self.nodes.get(id.0 as usize) {
            Some(Some(_)) => Ok(()),
            _ => Err(DomError::StaleNode),
        }
    }

    /// Node data of a live id. Callers validate ids first with `check`, and live nodes only reference live nodes.
    pub(crate) fn n(&self, id: NodeId) -> &Node {
        self.nodes[id.0 as usize].as_ref().expect("live node")
    }

    pub(crate) fn nm(&mut self, id: NodeId) -> &mut Node {
        self.nodes[id.0 as usize].as_mut().expect("live node")
    }

    pub(crate) fn alloc(&mut self, node: Node) -> NodeId {
        self.nodes.push(Some(node));
        NodeId((self.nodes.len() - 1) as u32)
    }

    /// createElement / createElementNS. HTML local names are lowercased; the name must be a valid XML Name.
    pub fn create_element(&mut self, ns: Namespace, tag: &str, is: Option<&str>) -> Result<NodeId, DomError> {
        if !valid_element_name(tag) {
            return Err(DomError::InvalidCharacterError);
        }
        let local = if ns == Namespace::Html { tag.to_ascii_lowercase() } else { tag.to_string() };
        let mut node = Node::new(NodeKind::Element, ns, local, String::new());
        node.is_value = is.map(str::to_string);
        Ok(self.alloc(node))
    }

    pub fn create_text(&mut self, data: &str) -> NodeId {
        self.alloc(Node::new(NodeKind::Text, Namespace::Html, String::new(), data.to_string()))
    }

    pub fn create_comment(&mut self, data: &str) -> NodeId {
        self.alloc(Node::new(NodeKind::Comment, Namespace::Html, String::new(), data.to_string()))
    }

    pub fn kind(&self, id: NodeId) -> Option<NodeKind> {
        self.nodes.get(id.0 as usize)?.as_ref().map(|n| n.kind)
    }

    pub fn namespace(&self, id: NodeId) -> Option<Namespace> {
        self.live(id).filter(|n| n.kind == NodeKind::Element).map(|n| n.ns)
    }

    pub fn local_name(&self, id: NodeId) -> Option<&str> {
        self.live(id).filter(|n| n.kind == NodeKind::Element).map(|n| n.local.as_str())
    }

    /// tagName: uppercase for HTML elements, the qualified name otherwise.
    pub fn tag_name(&self, id: NodeId) -> Option<String> {
        let n = self.live(id).filter(|n| n.kind == NodeKind::Element)?;
        Some(if n.ns == Namespace::Html { n.local.to_ascii_uppercase() } else { n.local.clone() })
    }

    pub fn node_name(&self, id: NodeId) -> Option<String> {
        let n = self.live(id)?;
        Some(match n.kind {
            NodeKind::Document => "#document".to_string(),
            NodeKind::Text => "#text".to_string(),
            NodeKind::Comment => "#comment".to_string(),
            NodeKind::Element => self.tag_name(id)?,
        })
    }

    pub(crate) fn live(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.0 as usize)?.as_ref()
    }
}

/// XML Name production (approximated: non-ASCII characters are accepted), used by createElement.
pub(crate) fn valid_element_name(name: &str) -> bool {
    let mut chars = name.chars();
    let first_ok = match chars.next() {
        Some(c) => c.is_ascii_alphabetic() || c == '_' || c == ':' || !c.is_ascii(),
        None => false,
    };
    first_ok && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':') || !c.is_ascii())
}

/// Valid attribute local name for setAttribute: any non-empty string without ASCII whitespace, NUL, '/', '=' or '>'.
pub(crate) fn valid_attribute_name(name: &str) -> bool {
    !name.is_empty()
        && !name.chars().any(|c| c == '\0' || c == '/' || c == '=' || c == '>' || (c.is_ascii() && crate::value::is_ascii_ws(c)))
}
