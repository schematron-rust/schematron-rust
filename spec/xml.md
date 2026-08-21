# XML data model and parser

Schematron is defined over the XPath data model, so the crate needs a real
XPath tree, not an event stream. `schematron::xml` provides it.

## Tree representation

An arena. `Document` owns `Vec<NodeData>`; a `NodeId` is an index. This gives
cheap copyable node handles, O(1) parent and child access, and no reference
cycles for the borrow checker to fight over.

```rust
pub struct Document { /* nodes, root, base_uri */ }
pub struct NodeId(usize);

pub enum NodeKind {
    Root, Element, Attribute, Namespace, Text, Comment, ProcessingInstruction,
}

pub struct QName { pub prefix: Option<String>, pub local: String, pub uri: Option<String> }
```

Each node carries: kind, parent, children, attributes, namespace nodes, an
expanded name, a value, and a document-order index.

## The seven node kinds

XPath 1.0 requires all seven. Attribute and namespace nodes are real nodes
with the element as parent, but they are *not* children of it: they are
reached only through the `attribute` and `namespace` axes. The crate models
this by storing them in separate `attributes` and `namespaces` vectors, so
`child::node()` never returns them.

## Document order

Assigned once, during construction, as a monotonically increasing counter in
this visit order:

1. root
2. element start
3. the element's namespace nodes
4. the element's attribute nodes
5. the element's children, recursively

Node-set comparison, `position()`, `<<`-style ordering, and the "first node in
document order" rule of `value-of` all reduce to comparing this index.

## String values

Per XPath 1.0:

| Kind | String value |
|---|---|
| Root, Element | Concatenation of all descendant text nodes, in document order |
| Attribute | The normalised attribute value |
| Namespace | The namespace URI |
| Text | The character data |
| Comment | The comment content, without `<!--` and `-->` |
| ProcessingInstruction | The content after the target and its whitespace |

## Parsing

Byte input → tree, using `quick-xml` as the low-level tokenizer (pure Rust)
with the crate supplying namespace resolution, entity expansion, and tree
construction.

Handled:

- XML declaration; UTF-8 and UTF-16 input, transcoded to UTF-8 internally.
- Namespace declarations, including default namespaces and undeclaration
  (`xmlns=""`), with correct scoping.
- The five predefined entities and numeric character references.
- CDATA sections, merged into adjacent text.
- Comments and processing instructions, preserved as nodes.
- `xml:` prefix bound implicitly to `http://www.w3.org/XML/1998/namespace`.

Deliberately not handled, and reported as errors rather than guessed at:

- DTD internal subsets defining entities. A `<!DOCTYPE>` is skipped, but a
  reference to an entity it defines is an error.
- External entity resolution. This is a security boundary: the parser never
  fetches an external entity, so XXE is structurally impossible.

## Text node handling

Adjacent character data, CDATA, and entity references coalesce into one text
node. Whitespace is preserved exactly; the crate never strips whitespace-only
text nodes, because Schematron rules legitimately match `text()`.

## Subtree ranges

Alongside its document-order index, each node stores the highest order value
in its own subtree, and its one-based position among siblings sharing its kind
and expanded name. Both are computed in a single pass when the tree is built.

They exist because the obvious implementations are quadratic, and this crate's
whole job is to visit every node of a document:

- "is `x` a descendant of `y`" becomes an integer range check rather than a
  membership test against a collected set, which is what keeps the `following`
  and `preceding` axes linear.
- Generating an SVRL `@location` becomes O(depth) rather than O(siblings),
  which matters because a location is generated for every finding, and a
  document with ten thousand siblings is exactly the shape Schematron gets
  pointed at.

The benchmark that caught the second one is `bench_validate`; fixing it made
validating a 10 000-element document fourteen times faster.

## Escaping

`schematron::xml::escape_text` and `escape_attribute` render text back out for
SVRL, escaping `<`, `>`, and `&` in content, and additionally `"` and the
whitespace characters that attribute-value normalisation would otherwise
collapse.
