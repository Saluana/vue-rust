//! Error values of the DOM operations. `name()` is the DOM exception name recorded in the Chrome corpora.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DomError {
    HierarchyRequestError,
    NotFoundError,
    InvalidCharacterError,
    NamespaceError,
    SyntaxError,
    NotSupportedError,
    /// `release` of a node that is still attached to a parent.
    NotDetached,
    /// The NodeId was released, or it does not belong to this document.
    StaleNode,
    /// The operation is not defined for this node kind.
    InvalidNodeType,
    /// A getter-only property was assigned (a TypeError in strict-mode JavaScript).
    ReadOnlyProperty,
    /// A property that vn-dom does not model (object-valued or outside the supported set).
    UnsupportedProperty,
    /// A selector outside the supported grammar (see query.rs).
    UnsupportedSelector,
    /// A style property name outside the supported table (see style.rs).
    UnsupportedStyleProperty,
}

impl DomError {
    pub fn name(self) -> &'static str {
        match self {
            DomError::HierarchyRequestError => "HierarchyRequestError",
            DomError::NotFoundError => "NotFoundError",
            DomError::InvalidCharacterError => "InvalidCharacterError",
            DomError::NamespaceError => "NamespaceError",
            DomError::SyntaxError => "SyntaxError",
            DomError::NotSupportedError => "NotSupportedError",
            DomError::NotDetached => "NotDetached",
            DomError::StaleNode => "StaleNode",
            DomError::InvalidNodeType => "InvalidNodeType",
            DomError::ReadOnlyProperty => "TypeError",
            DomError::UnsupportedProperty => "UnsupportedProperty",
            DomError::UnsupportedSelector => "UnsupportedSelector",
            DomError::UnsupportedStyleProperty => "UnsupportedStyleProperty",
        }
    }
}
