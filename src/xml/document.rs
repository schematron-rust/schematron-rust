//! The XML document tree: an arena of nodes with XPath 1.0 accessors.

use super::node::{NodeData, NodeId, NodeKind, QName};

/// A parsed XML document, stored as an arena of nodes.
///
/// Construct one with [`Document::from_str`], [`Document::from_bytes`], or
/// [`Document::from_path`]. Every accessor takes a [`NodeId`] obtained from
/// this same document.
///
/// # Examples
///
/// ```
/// use schematron::xml::Document;
///
/// let doc = Document::from_str("<a x='1'>hello</a>").unwrap();
/// let a = doc.document_element().unwrap();
/// assert_eq!(doc.name(a).unwrap().local, "a");
/// assert_eq!(doc.string_value(a), "hello");
/// ```
#[derive(Debug, Clone)]
pub struct Document {
    pub(crate) nodes: Vec<NodeData>,
    pub(crate) root: NodeId,
    pub(crate) base_uri: Option<String>,
    /// Roots of further documents merged into this arena.
    ///
    /// XPath `document()` returns nodes of another document, but a node-set
    /// is a list of indices into one arena — so the other document is copied
    /// in beside the first rather than living in an arena of its own. Each
    /// merged document keeps its own `Root` node with no parent, so the two
    /// trees never become each other's ancestors.
    ///
    /// Empty for an ordinary parsed document, which is the case that must
    /// stay free of any cost. See `spec/xpath.md`.
    pub(crate) extra_roots: Vec<NodeId>,
}

impl Document {
    /// Creates an empty document containing only a root node.
    #[must_use]
    pub(crate) fn empty() -> Self {
        let root = NodeData::new(NodeKind::Root, None, 0);
        Self {
            nodes: vec![root],
            root: NodeId(0),
            base_uri: None,
            extra_roots: Vec::new(),
        }
    }

    /// The root node, which is the parent of the document element.
    ///
    /// This is XPath's `/`, and it is not the document element itself. When
    /// other documents have been merged in, this is the root of the
    /// *primary* one — use [`Document::root_of`] to get the root that a
    /// particular node belongs to.
    #[must_use]
    pub const fn root(&self) -> NodeId {
        self.root
    }

    /// The root of the document that `node` belongs to.
    ///
    /// Identical to [`Document::root`] unless other documents have been
    /// merged in by XPath `document()`. It is what an absolute path such as
    /// `/invoice` must start from: inside a merged document, `/` means *that*
    /// document's root, not the primary one's.
    ///
    /// # Examples
    ///
    /// ```
    /// use schematron::xml::Document;
    ///
    /// let doc = Document::from_str("<a><b/></a>").unwrap();
    /// let b = doc.children(doc.document_element().unwrap())[0];
    /// assert_eq!(doc.root_of(b), doc.root());
    /// ```
    #[must_use]
    pub fn root_of(&self, node: NodeId) -> NodeId {
        // The overwhelmingly common case is a document with nothing merged
        // in, where every node belongs to the one root.
        if self.extra_roots.is_empty() {
            return self.root;
        }
        let mut current = node;
        while let Some(parent) = self.parent(current) {
            current = parent;
        }
        current
    }

    /// The roots of every document in this arena, primary first.
    #[must_use]
    pub fn roots(&self) -> Vec<NodeId> {
        let mut roots = vec![self.root];
        roots.extend_from_slice(&self.extra_roots);
        roots
    }

    /// Copies `other` into this arena as a further document, and returns its
    /// new root.
    ///
    /// Used by XPath `document()`. Document order continues across the merge,
    /// so a node-set spanning two documents still sorts deterministically —
    /// XPath 1.0 leaves the relative order of nodes in different documents
    /// implementation-defined, requiring only that it be consistent.
    pub(crate) fn append_document(&mut self, other: &Document) -> NodeId {
        let mut order = self.nodes.iter().map(|node| node.order).max().unwrap_or(0);
        let root = self.copy_from(other, other.root(), None, &mut order);
        self.extra_roots.push(root);
        // The copied tree needs its own subtree ranges and sibling positions.
        self.finalize_subtree(root);
        root
    }

    /// Deep-copies one node and its subtree out of `source` into this arena.
    fn copy_from(
        &mut self,
        source: &Document,
        node: NodeId,
        parent: Option<NodeId>,
        order: &mut usize,
    ) -> NodeId {
        *order += 1;
        let id = NodeId(self.nodes.len());
        let mut data = NodeData::new(source.kind(node), parent, *order);
        data.name = source.name(node).cloned();
        data.value = source.value(node).to_string();
        self.nodes.push(data);
        if let Some(parent) = parent {
            self.nodes[parent.0].children.push(id);
        }

        // Namespace and attribute nodes are numbered before children, which
        // is the document order XPath specifies.
        for &namespace in source.namespaces(node) {
            *order += 1;
            let copy = NodeId(self.nodes.len());
            let mut data = NodeData::new(NodeKind::Namespace, Some(id), *order);
            data.name = source.name(namespace).cloned();
            data.value = source.value(namespace).to_string();
            self.nodes.push(data);
            self.nodes[id.0].namespaces.push(copy);
        }
        for &attribute in source.attributes(node) {
            *order += 1;
            let copy = NodeId(self.nodes.len());
            let mut data = NodeData::new(NodeKind::Attribute, Some(id), *order);
            data.name = source.name(attribute).cloned();
            data.value = source.value(attribute).to_string();
            self.nodes.push(data);
            self.nodes[id.0].attributes.push(copy);
        }
        for &child in source.children(node) {
            self.copy_from(source, child, Some(id), order);
        }
        id
    }

    /// The document element: the single element child of the root.
    ///
    /// Returns `None` only for a document with no element, which the parser
    /// rejects, so in practice this is always `Some` for a parsed document.
    #[must_use]
    pub fn document_element(&self) -> Option<NodeId> {
        self.children(self.root)
            .iter()
            .copied()
            .find(|&n| self.kind(n) == NodeKind::Element)
    }

    /// The base URI used to resolve relative references from this document.
    #[must_use]
    pub fn base_uri(&self) -> Option<&str> {
        self.base_uri.as_deref()
    }

    /// Sets the base URI.
    pub fn set_base_uri(&mut self, uri: impl Into<String>) {
        self.base_uri = Some(uri.into());
    }

    /// The number of nodes in the document, all kinds included.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the document holds nothing but its root node.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.len() <= 1
    }

    pub(crate) fn data(&self, id: NodeId) -> &NodeData {
        &self.nodes[id.0]
    }

    /// The kind of a node.
    #[must_use]
    pub fn kind(&self, id: NodeId) -> NodeKind {
        self.data(id).kind
    }

    /// The expanded name of a node, for the kinds that have one.
    ///
    /// Elements, attributes, and processing instructions have names; the
    /// other kinds return `None`. A namespace node's "name" is its prefix,
    /// which is what XPath's `name()` returns for it, so it is reported here
    /// as a local name.
    #[must_use]
    pub fn name(&self, id: NodeId) -> Option<&QName> {
        self.data(id).name.as_ref()
    }

    /// The parent of a node, or `None` for the root.
    ///
    /// Attribute and namespace nodes report their element as their parent,
    /// which is what the `parent` axis requires, even though they are not in
    /// that element's child list.
    #[must_use]
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.data(id).parent
    }

    /// The children of a node, excluding attribute and namespace nodes.
    #[must_use]
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        &self.data(id).children
    }

    /// The attribute nodes of an element, excluding namespace declarations.
    #[must_use]
    pub fn attributes(&self, id: NodeId) -> &[NodeId] {
        &self.data(id).attributes
    }

    /// The namespace nodes in scope on an element.
    ///
    /// Per XPath, this includes bindings inherited from ancestors and the
    /// implicit binding of the `xml` prefix, and excludes any binding that a
    /// nearer ancestor has overridden or undeclared.
    #[must_use]
    pub fn namespaces(&self, id: NodeId) -> &[NodeId] {
        &self.data(id).namespaces
    }

    /// The position of a node in document order.
    ///
    /// Node-set ordering, `position()`, and the "first node in document
    /// order" rule all reduce to comparing this value.
    #[must_use]
    pub fn order(&self, id: NodeId) -> usize {
        self.data(id).order
    }

    /// The raw stored value of a node.
    ///
    /// For text, comments, and processing instructions this is the content;
    /// for attributes the normalised value; for namespace nodes the URI. For
    /// elements and the root it is empty — use [`Document::string_value`],
    /// which computes the XPath string value.
    #[must_use]
    pub fn value(&self, id: NodeId) -> &str {
        &self.data(id).value
    }

    /// The XPath 1.0 string value of a node.
    ///
    /// For the root and for elements this is the concatenation of every
    /// descendant text node in document order; for every other kind it is the
    /// node's own content.
    ///
    /// # Examples
    ///
    /// ```
    /// use schematron::xml::Document;
    ///
    /// let doc = Document::from_str("<a>one<b>two</b>three</a>").unwrap();
    /// let a = doc.document_element().unwrap();
    /// assert_eq!(doc.string_value(a), "onetwothree");
    /// ```
    #[must_use]
    pub fn string_value(&self, id: NodeId) -> String {
        match self.kind(id) {
            NodeKind::Root | NodeKind::Element => {
                let mut out = String::new();
                self.collect_text(id, &mut out);
                out
            }
            _ => self.data(id).value.clone(),
        }
    }

    fn collect_text(&self, id: NodeId, out: &mut String) {
        for &child in self.children(id) {
            match self.kind(child) {
                NodeKind::Text => out.push_str(&self.data(child).value),
                NodeKind::Element => self.collect_text(child, out),
                _ => {}
            }
        }
    }

    /// The ancestors of a node, nearest first, ending at the root.
    #[must_use]
    pub fn ancestors(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut current = self.parent(id);
        while let Some(node) = current {
            out.push(node);
            current = self.parent(node);
        }
        out
    }

    /// Every node in the subtree rooted at `id`, in document order.
    ///
    /// Includes `id` itself. Excludes attribute and namespace nodes, matching
    /// the `descendant-or-self` axis.
    #[must_use]
    pub fn descendants_or_self(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        self.push_descendants_or_self(id, &mut out);
        out
    }

    fn push_descendants_or_self(&self, id: NodeId, out: &mut Vec<NodeId>) {
        out.push(id);
        for &child in self.children(id) {
            self.push_descendants_or_self(child, out);
        }
    }

    /// Every node of the document in document order, attribute and namespace
    /// nodes included.
    ///
    /// This is the visiting order Schematron rules are offered nodes in: an
    /// element, then its namespace nodes, then its attribute nodes, then its
    /// children. See `spec/validation.md`.
    #[must_use]
    pub fn all_nodes_in_document_order(&self) -> Vec<NodeId> {
        let mut out = Vec::with_capacity(self.nodes.len());
        self.push_all(self.root, &mut out);
        out
    }

    fn push_all(&self, id: NodeId, out: &mut Vec<NodeId>) {
        out.push(id);
        out.extend_from_slice(self.namespaces(id));
        out.extend_from_slice(self.attributes(id));
        for &child in self.children(id) {
            self.push_all(child, out);
        }
    }

    /// The highest document-order value in this node's subtree.
    ///
    /// With [`Document::order`], this gives an O(1) descendant test: `x` is a
    /// descendant of `y` exactly when `order(y) < order(x) <= subtree_end(y)`.
    #[must_use]
    pub fn subtree_end(&self, id: NodeId) -> usize {
        self.data(id).subtree_end
    }

    /// Whether `descendant` lies inside the subtree rooted at `ancestor`.
    ///
    /// Excludes the node itself.
    #[must_use]
    pub fn is_descendant_of(&self, descendant: NodeId, ancestor: NodeId) -> bool {
        let order = self.order(descendant);
        order > self.order(ancestor) && order <= self.subtree_end(ancestor)
    }

    /// The index of `id` among its siblings of the same expanded name,
    /// one-based, as XPath positional predicates count them.
    fn position_among_siblings(&self, id: NodeId) -> usize {
        self.data(id).sibling_position
    }

    /// Computes `subtree_end` and `sibling_position` for every node.
    ///
    /// Run once after a tree is built, in a single pass. Both values are
    /// derived from structure that never changes afterwards, so computing
    /// them here removes a rescan from every later query.
    pub(crate) fn finalize(&mut self) {
        for root in self.roots() {
            self.finalize_subtree(root);
        }
    }

    fn finalize_subtree(&mut self, id: NodeId) -> usize {
        // Attributes and namespace nodes are numbered before children, so the
        // subtree's highest order starts from whichever exists.
        let mut end = self.data(id).order;
        for &node in self
            .data(id)
            .namespaces
            .iter()
            .chain(self.data(id).attributes.iter())
        {
            end = end.max(self.data(node).order);
        }

        // One counter per (kind, expanded name) among this node's children.
        let mut counts: std::collections::HashMap<(NodeKind, Option<String>, String), usize> =
            std::collections::HashMap::new();
        let children = self.data(id).children.clone();
        for child in children {
            let key = {
                let data = self.data(child);
                let (uri, local) = data
                    .name
                    .as_ref()
                    .map_or((None, String::new()), |n| (n.uri.clone(), n.local.clone()));
                (data.kind, uri, local)
            };
            let counter = counts.entry(key).or_insert(0);
            *counter += 1;
            self.nodes[child.0].sibling_position = *counter;
            end = end.max(self.finalize_subtree(child));
        }

        self.nodes[id.0].subtree_end = end;
        end
    }

    /// An absolute XPath expression that unambiguously identifies `id`.
    ///
    /// Names are written in the `*:local` wildcard form so that the result is
    /// usable without knowing the consumer's prefix bindings, which is what
    /// SVRL `@location` requires. See `spec/validation.md`.
    ///
    /// # Examples
    ///
    /// ```
    /// use schematron::xml::Document;
    ///
    /// let doc = Document::from_str("<a><b/><b id='x'/></a>").unwrap();
    /// let second = doc.children(doc.document_element().unwrap())[1];
    /// assert_eq!(doc.location(second), "/a[1]/b[2]");
    /// ```
    #[must_use]
    pub fn location(&self, id: NodeId) -> String {
        match self.kind(id) {
            NodeKind::Root => "/".to_string(),
            NodeKind::Attribute => {
                let parent = self.parent(id).map_or_else(|| "/".to_string(), |p| self.location(p));
                let step = self.name(id).map_or_else(
                    || "@*".to_string(),
                    |name| match name.uri.as_deref() {
                        Some(uri) => {
                            format!("@*[{}]", name_predicate(&name.local, uri))
                        }
                        None => format!("@{}", name.local),
                    },
                );
                format!("{parent}/{step}")
            }
            NodeKind::Namespace => {
                let parent = self.parent(id).map_or_else(|| "/".to_string(), |p| self.location(p));
                let name = self.name(id).map_or_else(String::new, |n| n.local.clone());
                format!("{parent}/namespace::{name}")
            }
            kind => {
                let parent = self.parent(id).map_or_else(String::new, |p| {
                    if self.kind(p) == NodeKind::Root {
                        String::new()
                    } else {
                        self.location(p)
                    }
                });
                let position = self.position_among_siblings(id);
                let step = match kind {
                    NodeKind::Element => self.name(id).map_or_else(
                        || "*".to_string(),
                        |name| match name.uri.as_deref() {
                            Some(uri) => format!("*[{}]", name_predicate(&name.local, uri)),
                            None => name.local.clone(),
                        },
                    ),
                    NodeKind::Text => "text()".to_string(),
                    NodeKind::Comment => "comment()".to_string(),
                    NodeKind::ProcessingInstruction => "processing-instruction()".to_string(),
                    NodeKind::Root | NodeKind::Attribute | NodeKind::Namespace => {
                        unreachable!("handled by the outer match arms")
                    }
                };
                format!("{parent}/{step}[{position}]")
            }
        }
    }
}

/// The predicate that identifies a namespaced name in XPath 1.0.
///
/// XPath 1.0 has no `*:local` wildcard — that is XPath 2.0 — and no default
/// namespace, so a name in a namespace cannot be written as a plain name test
/// unless the consumer happens to have bound the same prefix. `local-name()`
/// and `namespace-uri()` need nothing bound, which is why the ISO reference
/// implementation writes locations this way too.
fn name_predicate(local: &str, uri: &str) -> String {
    format!(
        "local-name()={} and namespace-uri()={}",
        xpath_literal(local),
        xpath_literal(uri)
    )
}

/// An XPath 1.0 string literal.
///
/// The language has no escape inside a literal, so the only lever is the
/// choice of delimiter. A value holding both kinds of quote cannot be written
/// at all; a namespace URI or an XML name never does.
fn xpath_literal(value: &str) -> String {
    if value.contains('\'') {
        format!("\"{value}\"")
    } else {
        format!("'{value}'")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(source: &str) -> Document {
        Document::from_str(source).unwrap()
    }

    #[test]
    fn root_is_not_document_element() {
        let d = doc("<a/>");
        assert_ne!(d.root(), d.document_element().unwrap());
        assert_eq!(d.kind(d.root()), NodeKind::Root);
    }

    #[test]
    fn string_value_concatenates_descendant_text() {
        let d = doc("<a>one<b>two</b>three</a>");
        assert_eq!(d.string_value(d.document_element().unwrap()), "onetwothree");
    }

    #[test]
    fn string_value_of_attribute_is_its_value() {
        let d = doc("<a x='1'/>");
        let a = d.document_element().unwrap();
        let attr = d.attributes(a)[0];
        assert_eq!(d.string_value(attr), "1");
    }

    #[test]
    fn attributes_are_not_children() {
        let d = doc("<a x='1'><b/></a>");
        let a = d.document_element().unwrap();
        assert_eq!(d.children(a).len(), 1);
        assert_eq!(d.attributes(a).len(), 1);
    }

    #[test]
    fn document_order_puts_attributes_before_children() {
        let d = doc("<a x='1'><b/></a>");
        let a = d.document_element().unwrap();
        let attr = d.attributes(a)[0];
        let b = d.children(a)[0];
        assert!(d.order(attr) < d.order(b));
    }

    #[test]
    fn location_uses_positional_predicates() {
        let d = doc("<a><b/><b/></a>");
        let a = d.document_element().unwrap();
        assert_eq!(d.location(d.children(a)[1]), "/a[1]/b[2]");
    }

    #[test]
    fn appending_a_document_keeps_the_two_trees_separate() {
        let mut primary = doc("<a><b/></a>");
        let before = primary.all_nodes_in_document_order().len();
        let root = primary.append_document(&doc("<x><y/></x>"));

        // A second root, with no parent, so neither tree is inside the other.
        assert_eq!(primary.kind(root), NodeKind::Root);
        assert_eq!(primary.parent(root), None);
        assert_eq!(primary.roots().len(), 2);

        // A walk of the primary document must not stray into the merged one,
        // or rules would fire on nodes of a document they never matched.
        let a = primary.document_element().unwrap();
        assert_eq!(primary.name(a).unwrap().local, "a");
        assert_eq!(primary.all_nodes_in_document_order().len(), before);
    }

    #[test]
    fn root_of_finds_the_document_a_node_belongs_to() {
        let mut primary = doc("<a><b/></a>");
        let a = primary.document_element().unwrap();
        let b = primary.children(a)[0];
        let root = primary.append_document(&doc("<x><y/></x>"));

        let x = primary.children(root)[0];
        let y = primary.children(x)[0];

        assert_eq!(primary.root_of(b), primary.root());
        assert_eq!(primary.root_of(y), root);
        assert_ne!(primary.root_of(y), primary.root_of(b));
    }

    #[test]
    fn appended_documents_keep_document_order_increasing() {
        let mut primary = doc("<a/>");
        let highest = primary
            .all_nodes_in_document_order()
            .iter()
            .map(|&n| primary.order(n))
            .max()
            .unwrap();
        let root = primary.append_document(&doc("<x/>"));
        // A cross-document node-set must still sort deterministically.
        assert!(primary.order(root) > highest);
    }

    #[test]
    fn appended_documents_are_finalized() {
        let mut primary = doc("<a/>");
        let root = primary.append_document(&doc("<x><y/><y/></x>"));
        let x = primary.children(root)[0];
        let second = primary.children(x)[1];

        // Sibling positions and subtree ranges must be computed for the copy,
        // or locations and the axes silently misbehave.
        assert_eq!(primary.location(second), "/x[1]/y[2]");
        assert!(primary.is_descendant_of(second, root));
        assert!(!primary.is_descendant_of(second, primary.root()));
    }

    #[test]
    fn appending_copies_attributes_and_namespaces() {
        let mut primary = doc("<a/>");
        let root = primary.append_document(&doc(r#"<x xmlns:p="urn:p" p:q="1"/>"#));
        let x = primary.children(root)[0];

        assert_eq!(primary.attributes(x).len(), 1);
        let attribute = primary.attributes(x)[0];
        assert_eq!(primary.value(attribute), "1");
        assert_eq!(primary.name(attribute).unwrap().uri.as_deref(), Some("urn:p"));
        assert!(!primary.namespaces(x).is_empty());
    }

    #[test]
    fn location_of_attribute() {
        let d = doc("<a><b q='2'/></a>");
        let a = d.document_element().unwrap();
        let b = d.children(a)[0];
        let attr = d.attributes(b)[0];
        assert_eq!(d.location(attr), "/a[1]/b[1]/@q");
    }
}
