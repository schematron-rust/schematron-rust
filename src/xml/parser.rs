//! Byte input to document tree.
//!
//! `quick-xml` supplies the low-level tokenizer; everything above it —
//! namespace scoping, entity policy, node identity, document order — is here,
//! because the XPath data model needs decisions that a pull parser does not
//! make for you.

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::path::Path;

use super::document::Document;
use super::node::{NodeData, NodeId, NodeKind, QName, XML_NAMESPACE};
use crate::error::{Error, Result};

/// The maximum element nesting depth accepted.
///
/// Tree construction is iterative, but the recursive accessors — string
/// values, descendant walks — are not, so a hostile document with a million
/// nested elements would exhaust the stack later even though it parsed. The
/// cap turns that into an error at parse time.
pub const MAX_DEPTH: usize = 1024;

/// One entry of the namespace scope stack.
struct NsFrame {
    /// Prefix, or the empty string for the default namespace.
    prefix: String,
    /// URI, or the empty string for an undeclaration such as `xmlns=""`.
    uri: String,
}

struct Builder<'a> {
    doc: Document,
    order: usize,
    ns_stack: Vec<NsFrame>,
    /// Index into `ns_stack` marking where each open element's declarations begin.
    ns_marks: Vec<usize>,
    source: &'a str,
}

impl<'a> Builder<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            doc: Document::empty(),
            order: 0,
            ns_stack: vec![NsFrame {
                prefix: "xml".to_string(),
                uri: XML_NAMESPACE.to_string(),
            }],
            ns_marks: Vec::new(),
            source,
        }
    }

    fn next_order(&mut self) -> usize {
        self.order += 1;
        self.order
    }

    fn push_node(&mut self, kind: NodeKind, parent: NodeId) -> NodeId {
        let order = self.next_order();
        let id = NodeId(self.doc.nodes.len());
        self.doc.nodes.push(NodeData::new(kind, Some(parent), order));
        self.doc.nodes[parent.0].children.push(id);
        id
    }

    /// Resolves a prefix against the current scope.
    ///
    /// Returns `None` for a prefix bound to the empty URI, which is how
    /// `xmlns=""` undeclares the default namespace.
    fn lookup(&self, prefix: &str) -> Option<&str> {
        self.ns_stack
            .iter()
            .rev()
            .find(|frame| frame.prefix == prefix)
            .and_then(|frame| {
                if frame.uri.is_empty() {
                    None
                } else {
                    Some(frame.uri.as_str())
                }
            })
    }

    /// Splits `prefix:local` and resolves the prefix.
    fn qname(&self, raw: &str, is_attribute: bool, position: usize) -> Result<QName> {
        match raw.split_once(':') {
            Some((prefix, local)) => {
                let uri = self.lookup(prefix).map(ToString::to_string);
                if uri.is_none() {
                    return Err(self.error(
                        position,
                        format!("namespace prefix {prefix:?} is not declared"),
                    ));
                }
                Ok(QName::new(Some(prefix), local, uri))
            }
            // An unprefixed attribute is in no namespace, even when a default
            // namespace is in scope. An unprefixed element takes the default.
            None if is_attribute => Ok(QName::local(raw)),
            None => Ok(QName::new(
                None::<String>,
                raw,
                self.lookup("").map(ToString::to_string),
            )),
        }
    }

    fn error(&self, position: usize, message: impl Into<String>) -> Error {
        let (line, column) = line_column(self.source, position);
        Error::XmlParse {
            line,
            column,
            message: message.into(),
        }
    }

    /// Pushes the namespace declarations of a start tag, returning nothing;
    /// the caller has already recorded the stack mark.
    fn declare_namespaces(&mut self, start: &BytesStart<'_>, position: usize) -> Result<()> {
        for attribute in start.attributes().with_checks(false) {
            let attribute = attribute
                .map_err(|e| self.error(position, format!("malformed attribute: {e}")))?;
            let key = std::str::from_utf8(attribute.key.as_ref())
                .map_err(|e| self.error(position, format!("attribute name is not UTF-8: {e}")))?;
            let prefix = if key == "xmlns" {
                ""
            } else if let Some(rest) = key.strip_prefix("xmlns:") {
                rest
            } else {
                continue;
            };
            let uri = decode_value(&attribute.value)
                .map_err(|message| self.error(position, message))?;
            self.ns_stack.push(NsFrame {
                prefix: prefix.to_string(),
                uri,
            });
        }
        Ok(())
    }

    /// Builds the namespace nodes in scope on an element.
    ///
    /// XPath wants one node per *visible* binding: nearest declaration wins,
    /// and an undeclared prefix contributes nothing.
    fn attach_namespace_nodes(&mut self, element: NodeId) {
        let mut seen: Vec<String> = Vec::new();
        let mut visible: Vec<(String, String)> = Vec::new();
        for frame in self.ns_stack.iter().rev() {
            if seen.iter().any(|p| p == &frame.prefix) {
                continue;
            }
            seen.push(frame.prefix.clone());
            if !frame.uri.is_empty() {
                visible.push((frame.prefix.clone(), frame.uri.clone()));
            }
        }
        visible.reverse();
        for (prefix, uri) in visible {
            let order = self.next_order();
            let id = NodeId(self.doc.nodes.len());
            let mut data = NodeData::new(NodeKind::Namespace, Some(element), order);
            data.name = Some(QName::local(prefix));
            data.value = uri;
            self.doc.nodes.push(data);
            self.doc.nodes[element.0].namespaces.push(id);
        }
    }

    fn attach_attributes(&mut self, element: NodeId, start: &BytesStart<'_>, position: usize) -> Result<()> {
        for attribute in start.attributes().with_checks(false) {
            let attribute = attribute
                .map_err(|e| self.error(position, format!("malformed attribute: {e}")))?;
            let key = std::str::from_utf8(attribute.key.as_ref())
                .map_err(|e| self.error(position, format!("attribute name is not UTF-8: {e}")))?
                .to_string();
            if key == "xmlns" || key.starts_with("xmlns:") {
                continue;
            }
            let name = self.qname(&key, true, position)?;
            let value = decode_value(&attribute.value)
                .map_err(|message| self.error(position, message))?;
            let order = self.next_order();
            let id = NodeId(self.doc.nodes.len());
            let mut data = NodeData::new(NodeKind::Attribute, Some(element), order);
            data.name = Some(name);
            data.value = value;
            self.doc.nodes.push(data);
            self.doc.nodes[element.0].attributes.push(id);
        }
        Ok(())
    }

    /// Appends character data, merging with a preceding text node so that
    /// CDATA boundaries and entity references do not fragment the tree.
    fn push_text(&mut self, parent: NodeId, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(&last) = self.doc.nodes[parent.0].children.last() {
            if self.doc.nodes[last.0].kind == NodeKind::Text {
                self.doc.nodes[last.0].value.push_str(text);
                return;
            }
        }
        let id = self.push_node(NodeKind::Text, parent);
        self.doc.nodes[id.0].value = text.to_string();
    }
}

fn decode_value(bytes: &[u8]) -> std::result::Result<String, String> {
    let raw = std::str::from_utf8(bytes).map_err(|e| format!("value is not UTF-8: {e}"))?;
    unescape(raw)
}

/// Expands the five predefined entities and numeric character references.
///
/// A reference to any other entity is an error rather than a silent pass
/// through: DTD-defined entities are not resolved, and pretending an
/// unresolved entity is literal text would corrupt the data model.
fn unescape(input: &str) -> std::result::Result<String, String> {
    if !input.contains('&') {
        return Ok(input.to_string());
    }
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find(';') else {
            return Err("unterminated entity reference: no ';' after '&'".to_string());
        };
        let name = &after[..end];
        match name {
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "amp" => out.push('&'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ => {
                let code = if let Some(hex) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
                    u32::from_str_radix(hex, 16).ok()
                } else if let Some(dec) = name.strip_prefix('#') {
                    dec.parse::<u32>().ok()
                } else {
                    None
                };
                match code.and_then(char::from_u32) {
                    Some(c) => out.push(c),
                    None => {
                        return Err(format!(
                            "unknown entity reference &{name};: only the predefined entities and \
                             numeric character references are expanded, because DTD entity \
                             declarations are not processed"
                        ))
                    }
                }
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

fn line_column(source: &str, position: usize) -> (usize, usize) {
    let position = position.min(source.len());
    let before = &source[..position];
    let line = before.matches('\n').count() + 1;
    let column = before.rfind('\n').map_or(position, |i| position - i - 1) + 1;
    (line, column)
}

impl Document {
    /// Parses a document from a string.
    ///
    /// Named `from_str` rather than implementing [`std::str::FromStr`]
    /// deliberately, to match [`Document::from_bytes`] and
    /// [`Document::from_path`], which the trait cannot express.
    ///
    /// # Errors
    ///
    /// Returns [`Error::XmlParse`] if the input is not well-formed, if a
    /// namespace prefix is used without being declared, or if it references
    /// an entity that is not predefined.
    ///
    /// # Examples
    ///
    /// ```
    /// use schematron::xml::Document;
    ///
    /// let doc = Document::from_str("<a><b/></a>").unwrap();
    /// assert!(doc.document_element().is_some());
    /// assert!(Document::from_str("<a>").is_err());
    /// ```
    #[allow(clippy::should_implement_trait)] // See the note above.
    pub fn from_str(source: &str) -> Result<Document> {
        parse(source)
    }

    /// Parses a document from bytes, transcoding UTF-16 to UTF-8 if needed.
    ///
    /// # Errors
    ///
    /// As [`Document::from_str`], plus an error if the bytes are not valid in
    /// any encoding the crate understands.
    pub fn from_bytes(bytes: &[u8]) -> Result<Document> {
        let source = decode_bytes(bytes)?;
        parse(&source)
    }

    /// Reads and parses a document from a file, recording its path as the
    /// base URI for relative reference resolution.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the file cannot be read, or a parse error as
    /// [`Document::from_str`].
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Document> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.display().to_string(),
            source,
        })?;
        let mut doc = Document::from_bytes(&bytes)?;
        doc.set_base_uri(path.display().to_string());
        Ok(doc)
    }
}

/// Transcodes input bytes into a UTF-8 string.
///
/// Only UTF-8 and UTF-16 are recognised, by byte order mark. Anything else is
/// treated as UTF-8, which is correct for the ASCII-compatible encodings and
/// an explicit error for the rest.
fn decode_bytes(bytes: &[u8]) -> Result<String> {
    let parse_error = |message: String| Error::XmlParse {
        line: 1,
        column: 1,
        message,
    };
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8(bytes[3..].to_vec())
            .map_err(|e| parse_error(format!("invalid UTF-8: {e}")));
    }
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        let big_endian = bytes[0] == 0xFE;
        let body = &bytes[2..];
        if !body.len().is_multiple_of(2) {
            return Err(parse_error("truncated UTF-16 input".to_string()));
        }
        // `as_chunks` gives fixed-size arrays, so the code unit conversions
        // below need no indexing and no bounds checks.
        let units: Vec<u16> = body
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| {
                if big_endian {
                    u16::from_be_bytes(pair)
                } else {
                    u16::from_le_bytes(pair)
                }
            })
            .collect();
        return String::from_utf16(&units)
            .map_err(|e| parse_error(format!("invalid UTF-16: {e}")));
    }
    String::from_utf8(bytes.to_vec()).map_err(|e| parse_error(format!("invalid UTF-8: {e}")))
}

/// Opens an element: declares its namespaces, creates the node, then attaches
/// its namespace nodes and attribute nodes in document order.
fn start_element(
    builder: &mut Builder<'_>,
    parent: NodeId,
    start: &BytesStart<'_>,
    position: usize,
) -> Result<NodeId> {
    builder.ns_marks.push(builder.ns_stack.len());
    builder.declare_namespaces(start, position)?;

    let raw = std::str::from_utf8(start.name().as_ref())
        .map_err(|e| builder.error(position, format!("element name is not UTF-8: {e}")))?
        .to_string();

    let element = builder.push_node(NodeKind::Element, parent);
    let name = builder.qname(&raw, false, position)?;
    builder.doc.nodes[element.0].name = Some(name);

    builder.attach_namespace_nodes(element);
    builder.attach_attributes(element, start, position)?;
    Ok(element)
}

/// Closes an element by discarding the namespace declarations it introduced.
fn end_element(builder: &mut Builder<'_>) {
    if let Some(mark) = builder.ns_marks.pop() {
        builder.ns_stack.truncate(mark);
    }
}

fn parse(source: &str) -> Result<Document> {
    let mut reader = Reader::from_str(source);
    let config = reader.config_mut();
    config.trim_text(false);
    config.expand_empty_elements = false;
    config.check_end_names = true;
    // XML 1.0 section 2.5 forbids `--` inside a comment, and quick-xml does
    // not check for it unless asked. Left off, `<!-- a -- b -->` parses here
    // and is rejected by every other XML tool — and a validator that reports
    // a document valid when its parser is the only one that would read it is
    // worse than one that refuses.
    config.check_comments = true;

    let mut builder = Builder::new(source);
    let mut open: Vec<NodeId> = vec![builder.doc.root];
    let mut saw_element = false;

    loop {
        // The buffer position cannot exceed the source length, which is
        // already a usize, so this conversion cannot lose anything.
        let position = usize::try_from(reader.buffer_position()).unwrap_or(usize::MAX);
        let event = reader
            .read_event()
            .map_err(|e| builder.error(position, e.to_string()))?;

        // The parent is always the innermost open element, or the root.
        let parent = *open.last().expect("the root is never popped");

        match event {
            Event::Eof => break,

            // The XML declaration carries no node. A DOCTYPE is skipped: the
            // crate does no DTD processing, and says so plainly rather than
            // half-honouring it. See spec/conformance.md.
            Event::Decl(_) | Event::DocType(_) => {}

            Event::Start(start) => {
                if open.len() > MAX_DEPTH {
                    return Err(builder.error(
                        position,
                        format!("element nesting deeper than the limit of {MAX_DEPTH}"),
                    ));
                }
                let element = start_element(&mut builder, parent, &start, position)?;
                open.push(element);
                saw_element = true;
            }

            Event::Empty(start) => {
                start_element(&mut builder, parent, &start, position)?;
                end_element(&mut builder);
                saw_element = true;
            }

            Event::End(_) => {
                // quick-xml has already checked that the name matches.
                open.pop();
                end_element(&mut builder);
            }

            Event::Text(text) => {
                let raw = std::str::from_utf8(text.as_ref())
                    .map_err(|e| builder.error(position, format!("text is not UTF-8: {e}")))?;
                let decoded = unescape(raw).map_err(|m| builder.error(position, m))?;
                // Whitespace outside the document element is not a text node.
                if parent == builder.doc.root {
                    if !decoded.trim().is_empty() {
                        return Err(builder.error(
                            position,
                            "character data outside the document element",
                        ));
                    }
                } else {
                    builder.push_text(parent, &decoded);
                }
            }

            Event::CData(cdata) => {
                let raw = std::str::from_utf8(cdata.as_ref())
                    .map_err(|e| builder.error(position, format!("CDATA is not UTF-8: {e}")))?
                    .to_string();
                if parent != builder.doc.root {
                    // CDATA is literal: no entity expansion, and it merges
                    // with adjacent character data into one text node.
                    builder.push_text(parent, &raw);
                }
            }

            Event::Comment(comment) => {
                let raw = std::str::from_utf8(comment.as_ref())
                    .map_err(|e| builder.error(position, format!("comment is not UTF-8: {e}")))?
                    .to_string();
                let id = builder.push_node(NodeKind::Comment, parent);
                builder.doc.nodes[id.0].value = raw;
            }

            Event::PI(pi) => {
                let raw = std::str::from_utf8(pi.as_ref())
                    .map_err(|e| {
                        builder.error(position, format!("processing instruction is not UTF-8: {e}"))
                    })?
                    .to_string();
                let (target, content) = match raw.find(char::is_whitespace) {
                    Some(i) => (raw[..i].to_string(), raw[i..].trim_start().to_string()),
                    None => (raw.clone(), String::new()),
                };
                let id = builder.push_node(NodeKind::ProcessingInstruction, parent);
                builder.doc.nodes[id.0].name = Some(QName::local(target));
                builder.doc.nodes[id.0].value = content;
            }

        }
    }

    if open.len() != 1 {
        return Err(builder.error(source.len(), "unclosed element at end of input"));
    }
    if !saw_element {
        return Err(builder.error(0, "document has no element"));
    }
    let mut document = builder.doc;
    document.finalize();
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml::NodeKind;

    fn doc(source: &str) -> Document {
        Document::from_str(source).unwrap()
    }

    #[test]
    fn parses_nested_elements() {
        let d = doc("<a><b><c/></b></a>");
        let a = d.document_element().unwrap();
        let b = d.children(a)[0];
        assert_eq!(d.name(b).unwrap().local, "b");
        assert_eq!(d.children(b).len(), 1);
    }

    #[test]
    fn resolves_default_namespace_for_elements_only() {
        let d = doc(r#"<a xmlns="urn:n" x="1"/>"#);
        let a = d.document_element().unwrap();
        assert_eq!(d.name(a).unwrap().uri.as_deref(), Some("urn:n"));
        let attr = d.attributes(a)[0];
        // An unprefixed attribute is in no namespace even under a default.
        assert_eq!(d.name(attr).unwrap().uri, None);
    }

    #[test]
    fn resolves_prefixed_names() {
        let d = doc(r#"<p:a xmlns:p="urn:n" p:x="1"/>"#);
        let a = d.document_element().unwrap();
        assert_eq!(d.name(a).unwrap().uri.as_deref(), Some("urn:n"));
        let attr = d.attributes(a)[0];
        assert_eq!(d.name(attr).unwrap().uri.as_deref(), Some("urn:n"));
    }

    #[test]
    fn undeclares_default_namespace() {
        let d = doc(r#"<a xmlns="urn:n"><b xmlns=""/></a>"#);
        let a = d.document_element().unwrap();
        let b = d.children(a)[0];
        assert_eq!(d.name(b).unwrap().uri, None);
    }

    #[test]
    fn undeclared_prefix_is_an_error() {
        assert!(Document::from_str("<p:a/>").is_err());
    }

    #[test]
    fn namespace_nodes_include_inherited_and_xml() {
        let d = doc(r#"<a xmlns:p="urn:n"><b/></a>"#);
        let a = d.document_element().unwrap();
        let b = d.children(a)[0];
        let prefixes: Vec<String> = d
            .namespaces(b)
            .iter()
            .map(|&n| d.name(n).unwrap().local.clone())
            .collect();
        assert!(prefixes.contains(&"p".to_string()));
        assert!(prefixes.contains(&"xml".to_string()));
    }

    #[test]
    fn expands_predefined_entities() {
        let d = doc("<a>&lt;&amp;&gt;&#65;&#x42;</a>");
        assert_eq!(d.string_value(d.document_element().unwrap()), "<&>AB");
    }

    #[test]
    fn rejects_unknown_entity() {
        assert!(Document::from_str("<a>&nbsp;</a>").is_err());
    }

    #[test]
    fn merges_cdata_with_adjacent_text() {
        let d = doc("<a>one<![CDATA[<two>]]>three</a>");
        let a = d.document_element().unwrap();
        assert_eq!(d.children(a).len(), 1);
        assert_eq!(d.string_value(a), "one<two>three");
    }

    #[test]
    fn keeps_comments_and_processing_instructions() {
        let d = doc("<a><!--c--><?target data?></a>");
        let a = d.document_element().unwrap();
        assert_eq!(d.kind(d.children(a)[0]), NodeKind::Comment);
        assert_eq!(d.value(d.children(a)[0]), "c");
        let pi = d.children(a)[1];
        assert_eq!(d.kind(pi), NodeKind::ProcessingInstruction);
        assert_eq!(d.name(pi).unwrap().local, "target");
        assert_eq!(d.value(pi), "data");
    }

    #[test]
    fn preserves_whitespace_in_text() {
        let d = doc("<a>  <b/>  </a>");
        let a = d.document_element().unwrap();
        assert_eq!(d.children(a).len(), 3);
    }

    #[test]
    fn rejects_unclosed_element() {
        assert!(Document::from_str("<a><b></a>").is_err());
    }

    #[test]
    fn rejects_a_double_hyphen_inside_a_comment() {
        // XML 1.0 section 2.5: "For compatibility, the string `--` MUST NOT
        // occur within comments." A comment may also not end `--->`, which is
        // the same rule seen from the other end.
        assert!(Document::from_str("<a><!-- x-- y --></a>").is_err());
        assert!(Document::from_str("<a><!-- x ---></a>").is_err());
        assert!(Document::from_str("<a><!--- x ---></a>").is_err());
        assert!(Document::from_str("<a><!---></a>").is_err());

        // The legal neighbours, including the empty comment.
        assert!(Document::from_str("<a><!-- x --></a>").is_ok());
        assert!(Document::from_str("<a><!----></a>").is_ok());
        assert!(Document::from_str("<a><!-- - --></a>").is_ok());
    }

    #[test]
    fn rejects_text_outside_document_element() {
        assert!(Document::from_str("junk<a/>").is_err());
    }

    #[test]
    fn skips_doctype_without_failing() {
        let d = doc("<!DOCTYPE a><a/>");
        assert_eq!(d.name(d.document_element().unwrap()).unwrap().local, "a");
    }

    #[test]
    fn decodes_utf16_with_byte_order_mark() {
        let text = "<a>x</a>";
        let mut bytes = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let d = Document::from_bytes(&bytes).unwrap();
        assert_eq!(d.string_value(d.document_element().unwrap()), "x");
    }

    #[test]
    fn line_column_counts_from_one() {
        assert_eq!(line_column("abc\ndef", 5), (2, 2));
        assert_eq!(line_column("abc", 0), (1, 1));
    }
}
