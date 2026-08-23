//! Evaluation context: variables, namespace bindings, and the context node.

use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;

use super::value::Value;
use super::version::XPathVersion;
use crate::xml::{Document, NodeId};

/// The documents XPath `document()` can reach, and a record of what it asked
/// for and did not find.
///
/// The validator holds the document tree immutably while evaluating, so a
/// `document()` call cannot load anything on the spot. Instead a miss is
/// *recorded*: the validator loads whatever was requested, merges it into the
/// arena, and runs again. Two passes suffice unless one loaded document names
/// another. See `spec/xpath.md`.
///
/// # Examples
///
/// ```
/// use schematron::xml::Document;
/// use schematron::xpath::Documents;
///
/// let doc = Document::from_str("<a/>").unwrap();
/// let mut documents = Documents::new();
/// documents.insert("parts.xml", None, "/data/parts.xml", doc.root());
///
/// assert_eq!(documents.lookup("parts.xml", None), Some(doc.root()));
/// assert_eq!(documents.lookup("missing.xml", None), None);
/// assert_eq!(
///     documents.missing(),
///     vec![("missing.xml".to_string(), None)]
/// );
///
/// // The same name asked for relative to a different base is a different
/// // request: it may well be a different file.
/// assert_eq!(documents.lookup("parts.xml", Some("/other/cat.xml")), None);
///
/// // And a loaded root remembers what it was called, so a node in it can
/// // serve as the base for a further `document()` call.
/// assert_eq!(documents.origin_of(doc.root()), Some("/data/parts.xml"));
/// ```
#[derive(Debug, Default)]
pub struct Documents {
    /// Keyed by the pair actually asked for: the URI as written, and the base
    /// it is to be resolved against. `document('a.xml')` from the instance and
    /// `document('a.xml', $node)` where `$node` came from another document are
    /// different requests that may name different files, so they cannot share
    /// a key.
    loaded: HashMap<Request, NodeId>,
    /// The URI each loaded root came from, so that a node in it can serve as
    /// the base for a further `document()` call.
    origins: HashMap<NodeId, String>,
    /// URIs asked for that are not in `loaded`, in a deterministic order so
    /// that a validation run is reproducible.
    ///
    /// A `Mutex` rather than a `RefCell` because a registry is shared across
    /// worker threads when patterns are evaluated in parallel, and a `RefCell`
    /// is not `Sync`. The lock is taken only on a miss, which happens once per
    /// URI per pass and never on the hot path.
    missing: Mutex<BTreeSet<Request>>,
}

/// A request for a document: the URI as written, and what to resolve it
/// against.
pub type Request = (String, Option<String>);

impl Documents {
    /// An empty registry, which makes every `document()` call a miss.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that the document asked for as `uri` relative to `base` is
    /// rooted at `root`, and that `root`'s own URI is `origin`.
    pub fn insert(
        &mut self,
        uri: impl Into<String>,
        base: Option<String>,
        origin: impl Into<String>,
        root: NodeId,
    ) {
        self.loaded.insert((uri.into(), base), root);
        self.origins.insert(root, origin.into());
    }

    /// Looks up a URI resolved against `base`, recording a miss when absent.
    #[must_use]
    pub fn lookup(&self, uri: &str, base: Option<&str>) -> Option<NodeId> {
        let request = (uri.to_string(), base.map(ToString::to_string));
        if let Some(root) = self.loaded.get(&request) {
            return Some(*root);
        }
        self.record_missing(request);
        None
    }

    /// The URI a loaded root came from, for use as a base URI.
    #[must_use]
    pub fn origin_of(&self, root: NodeId) -> Option<&str> {
        self.origins.get(&root).map(String::as_str)
    }

    /// Records a request that was not found.
    ///
    /// A poisoned lock cannot lose correctness here: the worst outcome is a
    /// request that is not recorded, which the next pass rediscovers.
    fn record_missing(&self, request: Request) {
        if let Ok(mut missing) = self.missing.lock() {
            missing.insert(request);
        }
    }

    /// The requests made but not satisfied, sorted.
    #[must_use]
    pub fn missing(&self) -> Vec<Request> {
        self.missing
            .lock()
            .map(|missing| missing.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Forgets the recorded misses, before another pass.
    pub fn clear_missing(&self) {
        if let Ok(mut missing) = self.missing.lock() {
            missing.clear();
        }
    }

    /// How many documents are loaded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.loaded.len()
    }

    /// Whether no document is loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.loaded.is_empty()
    }
}

/// Namespace prefix bindings for XPath expressions.
///
/// In Schematron these come from the schema's `ns` elements, and from nowhere
/// else: an XPath expression does *not* inherit the prefixes declared on the
/// schema document's own elements. An unprefixed name matches no namespace,
/// per XPath 1.0, so there is deliberately no default-namespace fallback.
///
/// # Examples
///
/// ```
/// use schematron::xpath::Namespaces;
///
/// let mut ns = Namespaces::new();
/// ns.insert("inv", "http://example.com/invoice");
/// assert_eq!(ns.resolve("inv"), Some("http://example.com/invoice"));
/// assert_eq!(ns.resolve("nope"), None);
/// ```
#[derive(Debug, Clone, Default)]
pub struct Namespaces {
    bindings: HashMap<String, String>,
}

impl Namespaces {
    /// An empty set of bindings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds a prefix to a namespace URI, replacing any previous binding.
    pub fn insert(&mut self, prefix: impl Into<String>, uri: impl Into<String>) {
        self.bindings.insert(prefix.into(), uri.into());
    }

    /// Looks up a prefix.
    #[must_use]
    pub fn resolve(&self, prefix: &str) -> Option<&str> {
        // The `xml` prefix is bound everywhere, without being declared.
        if prefix == "xml" {
            return Some(crate::xml::XML_NAMESPACE);
        }
        self.bindings.get(prefix).map(String::as_str)
    }

    /// Every binding, for reporting in SVRL output.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.bindings.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Whether any prefix is bound.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

/// The named indexes that XPath `key()` looks up.
///
/// Built once per document per validation run, before any pattern runs. See
/// `spec/keys.md`.
///
/// # Examples
///
/// ```
/// use schematron::xml::Document;
/// use schematron::xpath::Keys;
///
/// let doc = Document::from_str("<a><b id='x'/></a>").unwrap();
/// let b = doc.children(doc.document_element().unwrap())[0];
///
/// let mut keys = Keys::new();
/// keys.insert("parts", "x", b);
///
/// assert_eq!(keys.lookup("parts", "x"), vec![b]);
/// assert!(keys.lookup("parts", "missing").is_empty());
/// assert!(keys.is_declared("parts"));
/// ```
#[derive(Debug, Clone, Default)]
pub struct Keys {
    /// key name -> key value -> the nodes indexed under it.
    indexes: HashMap<String, HashMap<String, Vec<NodeId>>>,
}

impl Keys {
    /// An empty set of indexes.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares a key with no entries yet.
    ///
    /// Needed so that a key matching nothing is still *declared*, which is
    /// what distinguishes "no such node" from "no such key".
    pub fn declare(&mut self, name: impl Into<String>) {
        self.indexes.entry(name.into()).or_default();
    }

    /// Indexes `node` under `value` for the key called `name`.
    pub fn insert(&mut self, name: impl Into<String>, value: impl Into<String>, node: NodeId) {
        self.indexes
            .entry(name.into())
            .or_default()
            .entry(value.into())
            .or_default()
            .push(node);
    }

    /// The nodes indexed under `value`, or empty when there are none.
    #[must_use]
    pub fn lookup(&self, name: &str, value: &str) -> Vec<NodeId> {
        self.indexes
            .get(name)
            .and_then(|index| index.get(value))
            .cloned()
            .unwrap_or_default()
    }

    /// Whether a key of this name was declared.
    #[must_use]
    pub fn is_declared(&self, name: &str) -> bool {
        self.indexes.contains_key(name)
    }

    /// How many keys are declared.
    #[must_use]
    pub fn len(&self) -> usize {
        self.indexes.len()
    }

    /// Whether no key is declared.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.indexes.is_empty()
    }
}

/// A stack of variable bindings, innermost last.
///
/// Schematron nests four scopes — schema, phase, pattern, rule — and an inner
/// `let` shadows an outer one of the same name. A stack with reverse lookup
/// gives that for free, and [`Variables::mark`] and [`Variables::truncate`]
/// let a scope be unwound without rebuilding the outer ones.
///
/// # Examples
///
/// ```
/// use schematron::xpath::{Value, Variables};
///
/// let mut vars = Variables::new();
/// vars.bind("x", Value::Number(1.0));
/// let mark = vars.mark();
/// vars.bind("x", Value::Number(2.0));
/// assert_eq!(vars.lookup("x"), Some(&Value::Number(2.0)));
/// vars.truncate(mark);
/// assert_eq!(vars.lookup("x"), Some(&Value::Number(1.0)));
/// ```
#[derive(Debug, Clone, Default)]
pub struct Variables {
    entries: Vec<(String, Value)>,
}

impl Variables {
    /// An empty scope.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds a name, shadowing any existing binding of that name.
    pub fn bind(&mut self, name: impl Into<String>, value: Value) {
        self.entries.push((name.into(), value));
    }

    /// Looks up a name, innermost binding first.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<&Value> {
        self.entries
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v)
    }

    /// Records the current depth, so a scope can be unwound later.
    #[must_use]
    pub fn mark(&self) -> usize {
        self.entries.len()
    }

    /// Discards every binding made since `mark`.
    pub fn truncate(&mut self, mark: usize) {
        self.entries.truncate(mark);
    }

    /// The names bound, outermost first.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(n, _)| n.as_str())
    }
}

/// Everything an expression needs in order to be evaluated.
///
/// Built fresh for each evaluation; the parts that are expensive to build —
/// variables, namespaces — are borrowed rather than cloned.
// Fields will be added: a configurable implicit timezone is on the roadmap.
// Marking it non-exhaustive now means that will not be a breaking change.
// `new`, `focus`, and the `with_*` builders cover every field.
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub struct EvalContext<'a> {
    /// The document the context node lives in.
    pub document: &'a Document,
    /// The context node.
    pub node: NodeId,
    /// The context position, one-based.
    pub position: usize,
    /// The context size.
    pub size: usize,
    /// Variables in scope.
    pub variables: &'a Variables,
    /// Namespace prefix bindings.
    pub namespaces: &'a Namespaces,
    /// The node the enclosing rule fired on, which `current()` returns.
    ///
    /// Unlike [`EvalContext::node`], this does not change inside predicates,
    /// which is the whole point of the function.
    pub current: NodeId,
    /// The XPath version expressions are evaluated as.
    ///
    /// Defaults to [`XPathVersion::V1`]; the validator sets it from the
    /// schema's query binding.
    pub version: XPathVersion,
    /// The instant `current-date()` and its companions report, in seconds
    /// since the Unix epoch.
    ///
    /// Read once per validation run rather than per call, which XPath 2.0
    /// requires and which also stops one rule contradicting another halfway
    /// down a document. `None` makes the three functions an error, so a
    /// caller evaluating XPath directly cannot silently get an arbitrary
    /// instant.
    pub current_time: Option<f64>,
    /// The timezone a date or time with no offset is read as being in,
    /// in minutes from UTC.
    ///
    /// Defaults to zero, which keeps a validation run reproducible on any
    /// machine. See `spec/xpath2.md`.
    pub implicit_timezone: i32,
    /// The named indexes `key()` looks up, when the caller supplies any.
    ///
    /// `None` makes `key()` an error rather than an empty node-set, because
    /// an empty result would turn a missing index into a passing assertion.
    pub keys: Option<&'a Keys>,
    /// The documents `document()` can reach, when the caller supplies any.
    ///
    /// `None` — the default from [`EvalContext::new`] — makes `document()` an
    /// error rather than an empty node-set, because silently returning
    /// nothing would turn a misconfigured lookup into a passing assertion.
    pub documents: Option<&'a Documents>,
}

impl<'a> EvalContext<'a> {
    /// Builds a context for a single node, with position and size 1.
    #[must_use]
    pub fn new(
        document: &'a Document,
        node: NodeId,
        variables: &'a Variables,
        namespaces: &'a Namespaces,
    ) -> Self {
        Self {
            document,
            node,
            position: 1,
            size: 1,
            variables,
            namespaces,
            current: node,
            version: XPathVersion::V1,
            current_time: None,
            implicit_timezone: 0,
            keys: None,
            documents: None,
        }
    }

    /// The same context, with the indexes `key()` looks up.
    #[must_use]
    pub fn with_keys(mut self, keys: &'a Keys) -> Self {
        self.keys = Some(keys);
        self
    }

    /// The same context, with the timezone that a date or time carrying no
    /// offset is read as being in.
    #[must_use]
    pub fn with_implicit_timezone(mut self, minutes: i32) -> Self {
        self.implicit_timezone = minutes;
        self
    }

    /// The same context, with the instant the clock functions report.
    #[must_use]
    pub fn with_current_time(mut self, seconds: f64) -> Self {
        self.current_time = Some(seconds);
        self
    }

    /// The same context, evaluating as a particular XPath version.
    #[must_use]
    pub fn with_version(mut self, version: XPathVersion) -> Self {
        self.version = version;
        self
    }

    /// The same context, with a registry for XPath `document()`.
    #[must_use]
    pub fn with_documents(mut self, documents: &'a Documents) -> Self {
        self.documents = Some(documents);
        self
    }

    /// The same context, focused on a different node with a new position.
    #[must_use]
    pub fn focus(&self, node: NodeId, position: usize, size: usize) -> Self {
        Self {
            node,
            position,
            size,
            ..*self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inner_binding_shadows_outer() {
        let mut vars = Variables::new();
        vars.bind("x", Value::Number(1.0));
        vars.bind("x", Value::Number(2.0));
        assert_eq!(vars.lookup("x"), Some(&Value::Number(2.0)));
    }

    #[test]
    fn truncate_restores_the_outer_scope() {
        let mut vars = Variables::new();
        vars.bind("x", Value::Number(1.0));
        let mark = vars.mark();
        vars.bind("x", Value::Number(2.0));
        vars.truncate(mark);
        assert_eq!(vars.lookup("x"), Some(&Value::Number(1.0)));
    }

    #[test]
    fn unbound_lookup_returns_none() {
        assert!(Variables::new().lookup("x").is_none());
    }

    #[test]
    fn xml_prefix_is_bound_without_declaration() {
        assert_eq!(
            Namespaces::new().resolve("xml"),
            Some(crate::xml::XML_NAMESPACE)
        );
    }
}
