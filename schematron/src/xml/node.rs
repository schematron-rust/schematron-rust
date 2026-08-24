//! XML node identity, kinds, and qualified names.
//!
//! The XPath 1.0 data model has exactly seven kinds of node, and every one of
//! them is reachable from some axis, so the crate models all seven rather
//! than the four that a typical "XML tree" type provides.

use std::fmt;

/// A handle to a node inside a [`Document`](crate::xml::Document).
///
/// This is an index into the document's arena, so it is `Copy`, cheap to pass
/// around, and meaningless without the document it came from. Using a
/// `NodeId` with a different document is a logic error; it will either panic
/// on an out-of-range index or silently name the wrong node.
///
/// # Examples
///
/// ```
/// use schematron::xml::Document;
///
/// let doc = Document::from_str("<a><b/></a>").unwrap();
/// let root = doc.root();
/// let element = doc.document_element().unwrap();
/// assert_ne!(root, element);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub(crate) usize);

impl NodeId {
    /// The underlying arena index.
    ///
    /// Exposed for diagnostics and for building side tables keyed by node.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "node#{}", self.0)
    }
}

/// The seven node kinds of the XPath 1.0 data model.
///
/// Attribute and namespace nodes are genuine nodes with an element as their
/// parent, but they are deliberately *not* children of that element: they are
/// reachable only through the `attribute` and `namespace` axes, so
/// `child::node()` never returns them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    /// The document root; the parent of the document element.
    Root,
    /// An element.
    Element,
    /// An attribute of an element.
    Attribute,
    /// A namespace binding in scope on an element.
    Namespace,
    /// Character data.
    Text,
    /// A comment.
    Comment,
    /// A processing instruction.
    ProcessingInstruction,
}

impl NodeKind {
    /// A short human-readable name, used in error messages.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            NodeKind::Root => "root",
            NodeKind::Element => "element",
            NodeKind::Attribute => "attribute",
            NodeKind::Namespace => "namespace",
            NodeKind::Text => "text",
            NodeKind::Comment => "comment",
            NodeKind::ProcessingInstruction => "processing-instruction",
        }
    }
}

/// An expanded name: an optional namespace URI plus a local part, with the
/// prefix retained for display.
///
/// XPath compares names by namespace URI and local part only; the prefix is
/// carried so that error messages and generated locations can show the name
/// the way the document author wrote it.
///
/// # Examples
///
/// ```
/// use schematron::xml::QName;
///
/// let a = QName::new(Some("inv"), "total", Some("http://example.com/i"));
/// let b = QName::new(Some("x"), "total", Some("http://example.com/i"));
/// // Different prefixes, same expanded name.
/// assert!(a.matches(&b));
/// assert_eq!(a.display_name(), "inv:total");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QName {
    /// The prefix as written in the document, if any.
    pub prefix: Option<String>,
    /// The local part.
    pub local: String,
    /// The namespace URI the prefix resolved to, if any.
    pub uri: Option<String>,
}

impl QName {
    /// Builds a qualified name.
    pub fn new(
        prefix: Option<impl Into<String>>,
        local: impl Into<String>,
        uri: Option<impl Into<String>>,
    ) -> Self {
        Self {
            prefix: prefix.map(Into::into),
            local: local.into(),
            uri: uri.map(Into::into),
        }
    }

    /// Builds a qualified name in no namespace.
    pub fn local(local: impl Into<String>) -> Self {
        Self {
            prefix: None,
            local: local.into(),
            uri: None,
        }
    }

    /// Compares expanded names: namespace URI and local part, ignoring prefix.
    #[must_use]
    pub fn matches(&self, other: &QName) -> bool {
        self.local == other.local && self.uri == other.uri
    }

    /// Compares against an expanded name given as its parts.
    #[must_use]
    pub fn matches_parts(&self, uri: Option<&str>, local: &str) -> bool {
        self.local == local && self.uri.as_deref() == uri
    }

    /// The name as written, `prefix:local` or `local`.
    ///
    /// This is what the XPath `name()` function returns.
    #[must_use]
    pub fn display_name(&self) -> String {
        match &self.prefix {
            Some(p) if !p.is_empty() => format!("{p}:{}", self.local),
            _ => self.local.clone(),
        }
    }
}

impl fmt::Display for QName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display_name())
    }
}

/// The stored form of one node.
///
/// Kept crate-private: callers reach node data through [`Document`] accessors,
/// which keeps the arena representation free to change.
///
/// [`Document`]: crate::xml::Document
#[derive(Debug, Clone)]
pub(crate) struct NodeData {
    pub(crate) kind: NodeKind,
    pub(crate) parent: Option<NodeId>,
    pub(crate) children: Vec<NodeId>,
    pub(crate) attributes: Vec<NodeId>,
    pub(crate) namespaces: Vec<NodeId>,
    pub(crate) name: Option<QName>,
    pub(crate) value: String,
    /// Position in document order; see `spec/xml/`.
    pub(crate) order: usize,
    /// The highest document-order value in this node's subtree, itself
    /// included.
    ///
    /// Turns "is `x` a descendant of `y`" into an integer range check, which
    /// is what keeps the `following` and `preceding` axes linear instead of
    /// quadratic.
    pub(crate) subtree_end: usize,
    /// One-based index among the siblings sharing this node's kind and
    /// expanded name.
    ///
    /// Precomputed because generating a location for every finding would
    /// otherwise rescan the parent's child list once per finding, which is
    /// quadratic on a document with many siblings — exactly the shape
    /// Schematron is usually pointed at.
    pub(crate) sibling_position: usize,
}

impl NodeData {
    pub(crate) fn new(kind: NodeKind, parent: Option<NodeId>, order: usize) -> Self {
        Self {
            kind,
            parent,
            children: Vec::new(),
            attributes: Vec::new(),
            namespaces: Vec::new(),
            name: None,
            value: String::new(),
            order,
            subtree_end: order,
            sibling_position: 1,
        }
    }
}

/// The XML namespace URI, bound implicitly to the `xml` prefix everywhere.
pub const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";

/// The namespace URI for namespace declarations themselves.
pub const XMLNS_NAMESPACE: &str = "http://www.w3.org/2000/xmlns/";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qname_matches_ignores_prefix() {
        let a = QName::new(Some("p"), "x", Some("urn:n"));
        let b = QName::new(Some("q"), "x", Some("urn:n"));
        assert!(a.matches(&b));
    }

    #[test]
    fn qname_matches_distinguishes_namespace() {
        let a = QName::new(Some("p"), "x", Some("urn:n"));
        let b = QName::local("x");
        assert!(!a.matches(&b));
    }

    #[test]
    fn qname_display_name_uses_prefix() {
        assert_eq!(QName::new(Some("p"), "x", Some("urn:n")).display_name(), "p:x");
        assert_eq!(QName::local("x").display_name(), "x");
    }

    #[test]
    fn node_kind_as_str() {
        assert_eq!(NodeKind::Element.as_str(), "element");
        assert_eq!(NodeKind::ProcessingInstruction.as_str(), "processing-instruction");
    }
}
