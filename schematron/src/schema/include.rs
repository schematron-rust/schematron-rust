//! Pass 2 of schema compilation: resolving `<sch:include>` and
//! `<sch:extends href>`.
//!
//! Both splice another document in place. Resolution happens on the XML tree,
//! before the model is built, so a spliced fragment can itself contain
//! includes and can be any Schematron element — a pattern, a rule, a
//! diagnostic.
//!
//! The two differ in what they splice, which is the distinction ISO/IEC
//! 19757-3 draws and the reference implementation states outright:
//!
//! - `<sch:include href="U"/>` is replaced by **the element** `U` names.
//! - `<sch:extends href="U"/>` is replaced by **the children** of that
//!   element, because it appears inside a `rule` that already exists and is
//!   contributing assertions to it.
//!
//! An `href` may carry a fragment identifier — `lib.sch#dates` selects the
//! element with that `@id` or `@xml:id` rather than the document element, and
//! a bare `#dates` selects one from the document already being read. With no
//! DTD there are no attributes typed `ID`, so `id` and `xml:id` are what
//! counts, the same convention [`crate::xpath`]'s `id()` uses.

use super::model::{SCHEMATRON_1_5_NAMESPACE, SCHEMATRON_NAMESPACE};
use super::resolver::Resolver;
use crate::error::{Error, Result};
use crate::xml::{Document, NodeId, NodeKind};

/// Rebuilds `document` with every `include` replaced by its target.
///
/// # Errors
///
/// Returns [`Error::IncludeCycle`] when includes form a loop,
/// [`Error::IncludeDepth`] when they nest deeper than `max_depth`, and
/// [`Error::Resolve`] when a target cannot be fetched.
pub(crate) fn resolve_includes(
    document: &Document,
    resolver: &dyn Resolver,
    max_depth: usize,
) -> Result<Document> {
    let mut output = Document::empty();
    let mut chain: Vec<String> = document
        .base_uri()
        .map(ToString::to_string)
        .into_iter()
        .collect();

    let root = output.root;
    let mut context = Context {
        resolver,
        max_depth,
        chain: &mut chain,
        order: 0,
    };
    for &child in document.children(document.root()) {
        context.copy(document, child, &mut output, root)?;
    }
    output.base_uri = document.base_uri().map(ToString::to_string);
    output.finalize();
    Ok(output)
}

struct Context<'a> {
    resolver: &'a dyn Resolver,
    max_depth: usize,
    /// The stack of documents currently being included, for cycle reporting.
    chain: &'a mut Vec<String>,
    /// The document-order counter for the rebuilt tree.
    order: usize,
}

impl Context<'_> {
    fn next_order(&mut self) -> usize {
        self.order += 1;
        self.order
    }

    /// Whether a node splices another document in, and if so how.
    ///
    /// `<sch:extends rule="ID"/>` is deliberately not matched here: it names
    /// an abstract rule in this same schema and is expanded later, in
    /// [`super::expand`], once the model exists.
    fn directive(document: &Document, node: NodeId) -> Option<(String, Splice)> {
        if document.kind(node) != NodeKind::Element {
            return None;
        }
        let name = document.name(node)?;
        let in_schematron = matches!(
            name.uri.as_deref(),
            Some(SCHEMATRON_NAMESPACE | SCHEMATRON_1_5_NAMESPACE)
        );
        if !in_schematron {
            return None;
        }
        let splice = match name.local.as_str() {
            "include" => Splice::Element,
            "extends" => Splice::Children,
            _ => return None,
        };
        let href = attribute(document, node, "href")?;
        Some((href, splice))
    }

    /// Copies one node, expanding it if it is an include.
    fn copy(
        &mut self,
        source: &Document,
        node: NodeId,
        output: &mut Document,
        parent: NodeId,
    ) -> Result<()> {
        if let Some((href, splice)) = Self::directive(source, node) {
            return self.expand(source, &href, splice, output, parent);
        }
        self.copy_verbatim(source, node, output, parent)
    }

    /// Copies a node and its subtree unchanged, apart from renumbering.
    fn copy_verbatim(
        &mut self,
        source: &Document,
        node: NodeId,
        output: &mut Document,
        parent: NodeId,
    ) -> Result<()> {
        let order = self.next_order();
        let id = NodeId(output.nodes.len());
        let mut data = crate::xml::NodeData::new(source.kind(node), Some(parent), order);
        data.name = source.name(node).cloned();
        data.value = source.value(node).to_string();
        output.nodes.push(data);
        output.nodes[parent.0].children.push(id);

        for &namespace in source.namespaces(node) {
            let order = self.next_order();
            let child = NodeId(output.nodes.len());
            let mut data = crate::xml::NodeData::new(NodeKind::Namespace, Some(id), order);
            data.name = source.name(namespace).cloned();
            data.value = source.value(namespace).to_string();
            output.nodes.push(data);
            output.nodes[id.0].namespaces.push(child);
        }
        for &attribute in source.attributes(node) {
            let order = self.next_order();
            let child = NodeId(output.nodes.len());
            let mut data = crate::xml::NodeData::new(NodeKind::Attribute, Some(id), order);
            data.name = source.name(attribute).cloned();
            data.value = source.value(attribute).to_string();
            output.nodes.push(data);
            output.nodes[id.0].attributes.push(child);
        }
        for &child in source.children(node) {
            self.copy(source, child, output, id)?;
        }
        Ok(())
    }

    /// Splices the target of `href` in place of the directive element.
    fn expand(
        &mut self,
        source: &Document,
        href: &str,
        splice: Splice,
        output: &mut Document,
        parent: NodeId,
    ) -> Result<()> {
        if self.chain.len() >= self.max_depth {
            return Err(Error::IncludeDepth {
                limit: self.max_depth,
                href: href.to_string(),
            });
        }

        let (uri, fragment) = split_fragment(href);
        if uri.is_empty() && fragment.is_none() {
            return Err(Error::Resolve {
                href: href.to_string(),
                message: "an href must name a document, a fragment, or both".to_string(),
            });
        }

        // A bare `#id` stays inside the document being read, so there is
        // nothing to fetch and the base URI does not move.
        if uri.is_empty() {
            let key = same_document_key(source, href);
            return self.guard(key, |context| {
                let target = fragment
                    .and_then(|fragment| find_fragment(source, fragment))
                    .ok_or_else(|| missing_fragment(href))?;
                context.splice(source, target, splice, output, parent)
            });
        }

        let base = self.chain.last().map(String::as_str);
        let rebased = self.resolver.rebase(uri, base);
        // The fragment is part of the identity: two fragments of one document
        // are two different targets, and only a repeat of the *same* target
        // is a cycle.
        let key = match (&rebased, fragment) {
            (Some(rebased), Some(fragment)) => format!("{rebased}#{fragment}"),
            (Some(rebased), None) => rebased.clone(),
            (None, _) => href.to_string(),
        };

        let text = self.resolver.resolve(uri, base)?;
        let mut included = Document::from_str(&text)?;
        if let Some(rebased) = rebased {
            included.set_base_uri(rebased);
        }

        let target = match fragment {
            Some(fragment) => {
                find_fragment(&included, fragment).ok_or_else(|| missing_fragment(href))?
            }
            None => included.document_element().ok_or_else(|| Error::Resolve {
                href: href.to_string(),
                message: "the included document has no root element".to_string(),
            })?,
        };

        self.guard(key, |context| {
            context.splice(&included, target, splice, output, parent)
        })
    }

    /// Runs `body` with `key` on the chain, refusing to re-enter a target
    /// already being spliced.
    fn guard<F>(&mut self, key: String, body: F) -> Result<()>
    where
        F: FnOnce(&mut Self) -> Result<()>,
    {
        if self.chain.iter().any(|seen| seen == &key) {
            let mut chain = self.chain.clone();
            chain.push(key);
            return Err(Error::IncludeCycle {
                chain: chain.join(" -> "),
            });
        }
        self.chain.push(key);
        let result = body(self);
        self.chain.pop();
        result
    }

    /// Copies the target itself, or just its children.
    fn splice(
        &mut self,
        source: &Document,
        target: NodeId,
        splice: Splice,
        output: &mut Document,
        parent: NodeId,
    ) -> Result<()> {
        match splice {
            Splice::Element => self.copy(source, target, output, parent),
            Splice::Children => {
                for &child in source.children(target) {
                    self.copy(source, child, output, parent)?;
                }
                Ok(())
            }
        }
    }
}

/// What a directive puts in its own place.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Splice {
    /// `include`: the target element itself.
    Element,
    /// `extends href`: the target's children, without the element.
    Children,
}

/// An unprefixed attribute's value.
fn attribute(document: &Document, node: NodeId, wanted: &str) -> Option<String> {
    document
        .attributes(node)
        .iter()
        .copied()
        .find(|&a| {
            document
                .name(a)
                .is_some_and(|n| n.uri.is_none() && n.local == wanted)
        })
        .map(|a| document.value(a).to_string())
}

/// Splits `href` into its document part and its fragment identifier.
fn split_fragment(href: &str) -> (&str, Option<&str>) {
    match href.split_once('#') {
        Some((uri, "")) => (uri, None),
        Some((uri, fragment)) => (uri, Some(fragment)),
        None => (href, None),
    }
}

/// The Schematron element carrying `@id` or `@xml:id` equal to `fragment`.
///
/// Restricted to the Schematron namespace, as the reference implementation
/// is: it lets a fragment address a schema embedded in a larger host
/// document, which is the case the restriction exists to serve.
fn find_fragment(document: &Document, fragment: &str) -> Option<NodeId> {
    (0..document.nodes.len()).map(NodeId).find(|&node| {
        if document.kind(node) != NodeKind::Element {
            return false;
        }
        let in_schematron = document.name(node).is_some_and(|name| {
            matches!(
                name.uri.as_deref(),
                Some(SCHEMATRON_NAMESPACE | SCHEMATRON_1_5_NAMESPACE)
            )
        });
        in_schematron
            && document.attributes(node).iter().any(|&a| {
                document.name(a).is_some_and(|name| {
                    name.local == "id"
                        && matches!(
                            name.uri.as_deref(),
                            None | Some(crate::xml::XML_NAMESPACE)
                        )
                }) && document.value(a) == fragment
            })
    })
}

/// The cycle key for a same-document reference.
///
/// It must name the *document holding the reference*, not the enclosing
/// chain entry: keying off the chain makes `#loop` inside `#loop` look like a
/// new target every time, so a self-reference walks to the depth limit and
/// reports the wrong diagnosis.
fn same_document_key(source: &Document, href: &str) -> String {
    match source.base_uri() {
        Some(base) => format!("{base}{href}"),
        None => href.to_string(),
    }
}

fn missing_fragment(href: &str) -> Error {
    Error::Resolve {
        href: href.to_string(),
        message: "no Schematron element carries that @id or @xml:id".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::resolver::MemoryResolver;
    use super::*;

    fn resolve(source: &str, resolver: &MemoryResolver) -> Result<Document> {
        let document = Document::from_str(source).unwrap();
        resolve_includes(&document, resolver, 64)
    }

    #[test]
    fn splices_the_included_document_element() {
        let resolver = MemoryResolver::new().with(
            "p.sch",
            r#"<pattern xmlns="http://purl.oclc.org/dsdl/schematron" id="included"/>"#,
        );
        let document = resolve(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <include href="p.sch"/>
               </schema>"#,
            &resolver,
        )
        .unwrap();
        let schema = document.document_element().unwrap();
        let child = document.children(schema)[1];
        assert_eq!(document.name(child).unwrap().local, "pattern");
        assert_eq!(document.value(document.attributes(child)[0]), "included");
    }

    #[test]
    fn resolves_nested_includes() {
        let resolver = MemoryResolver::new()
            .with(
                "a.sch",
                r#"<pattern xmlns="http://purl.oclc.org/dsdl/schematron">
                     <include href="b.sch"/>
                   </pattern>"#,
            )
            .with(
                "b.sch",
                r#"<rule xmlns="http://purl.oclc.org/dsdl/schematron" context="x"/>"#,
            );
        let document = resolve(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <include href="a.sch"/>
               </schema>"#,
            &resolver,
        )
        .unwrap();
        let schema = document.document_element().unwrap();
        let pattern = document.children(schema)[1];
        let rule = document
            .children(pattern)
            .iter()
            .copied()
            .find(|&n| document.kind(n) == NodeKind::Element)
            .unwrap();
        assert_eq!(document.name(rule).unwrap().local, "rule");
    }

    #[test]
    fn detects_cycles_instead_of_recursing_forever() {
        let resolver = MemoryResolver::new().with(
            "a.sch",
            r#"<pattern xmlns="http://purl.oclc.org/dsdl/schematron">
                 <include href="a.sch"/>
               </pattern>"#,
        );
        let error = resolve(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <include href="a.sch"/>
               </schema>"#,
            &resolver,
        )
        .unwrap_err();
        assert!(matches!(error, Error::IncludeCycle { .. }), "{error}");
    }

    #[test]
    fn reports_a_missing_target() {
        let error = resolve(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <include href="missing.sch"/>
               </schema>"#,
            &MemoryResolver::new(),
        )
        .unwrap_err();
        assert!(matches!(error, Error::Resolve { .. }), "{error}");
    }

    /// The library used by the fragment tests below.
    fn library() -> MemoryResolver {
        MemoryResolver::new().with(
            "lib.sch",
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern id="wanted"><rule context="a"/></pattern>
                 <pattern id="unwanted"><rule context="b"/></pattern>
                 <rule id="dated"><assert test="x">one</assert><assert test="y">two</assert></rule>
               </schema>"#,
        )
    }

    /// The elements spliced under the document element, by local name plus
    /// whichever of `@id` or `@test` identifies them.
    fn spliced(document: &Document) -> Vec<String> {
        let schema = document.document_element().unwrap();
        document
            .children(schema)
            .iter()
            .copied()
            .filter(|&n| document.kind(n) == NodeKind::Element)
            .map(|n| {
                let name = document.name(n).unwrap().local.clone();
                let id = document
                    .attributes(n)
                    .iter()
                    .copied()
                    .find(|&a| {
                        let local = &document.name(a).unwrap().local;
                        local == "id" || local == "test"
                    })
                    .map(|a| document.value(a).to_string());
                match id {
                    Some(id) => format!("{name}#{id}"),
                    None => name,
                }
            })
            .collect()
    }

    #[test]
    fn an_include_fragment_splices_only_the_named_element() {
        let document = resolve(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <include href="lib.sch#wanted"/>
               </schema>"#,
            &library(),
        )
        .unwrap();
        assert_eq!(spliced(&document), ["pattern#wanted"]);
    }

    #[test]
    fn an_extends_href_splices_the_children_not_the_element() {
        let document = resolve(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <extends href="lib.sch#dated"/>
               </schema>"#,
            &library(),
        )
        .unwrap();
        // The `rule` itself is gone; its two assertions took its place.
        assert_eq!(spliced(&document), ["assert#x", "assert#y"]);
    }

    #[test]
    fn a_bare_fragment_stays_in_the_document_being_read() {
        let document = resolve(
            r##"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern id="here"><rule context="a"/></pattern>
                 <include href="#here"/>
               </schema>"##,
            &MemoryResolver::new(),
        )
        .unwrap();
        assert_eq!(spliced(&document), ["pattern#here", "pattern#here"]);
    }

    #[test]
    fn an_xml_id_identifies_a_fragment_too() {
        let resolver = MemoryResolver::new().with(
            "lib.sch",
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron"
                       xmlns:xml="http://www.w3.org/XML/1998/namespace">
                 <pattern xml:id="byxmlid"><rule context="a"/></pattern>
               </schema>"#,
        );
        let document = resolve(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <include href="lib.sch#byxmlid"/>
               </schema>"#,
            &resolver,
        )
        .unwrap();
        assert_eq!(spliced(&document), ["pattern#byxmlid"]);
    }

    #[test]
    fn a_fragment_that_names_nothing_is_an_error() {
        let error = resolve(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <include href="lib.sch#absent"/>
               </schema>"#,
            &library(),
        )
        .unwrap_err();
        assert!(
            matches!(&error, Error::Resolve { message, .. } if message.contains("@id")),
            "{error}"
        );
    }

    #[test]
    fn two_fragments_of_one_document_are_not_a_cycle() {
        let document = resolve(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <include href="lib.sch#wanted"/>
                 <include href="lib.sch#unwanted"/>
               </schema>"#,
            &library(),
        )
        .unwrap();
        assert_eq!(spliced(&document), ["pattern#wanted", "pattern#unwanted"]);
    }

    #[test]
    fn a_fragment_that_includes_itself_is_a_cycle() {
        let resolver = MemoryResolver::new().with(
            "lib.sch",
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern id="loop"><include href="lib.sch#loop"/></pattern>
               </schema>"#,
        );
        let error = resolve(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <include href="lib.sch#loop"/>
               </schema>"#,
            &resolver,
        )
        .unwrap_err();
        assert!(matches!(error, Error::IncludeCycle { .. }), "{error}");
    }

    #[test]
    fn a_same_document_fragment_that_includes_itself_is_a_cycle() {
        let error = resolve(
            r##"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern id="loop"><include href="#loop"/></pattern>
               </schema>"##,
            &MemoryResolver::new(),
        )
        .unwrap_err();
        assert!(matches!(error, Error::IncludeCycle { .. }), "{error}");
    }

    #[test]
    fn an_empty_href_is_an_error_rather_than_a_panic() {
        let error = resolve(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <include href=""/>
               </schema>"#,
            &MemoryResolver::new(),
        )
        .unwrap_err();
        assert!(matches!(error, Error::Resolve { .. }), "{error}");
    }

    #[test]
    fn a_trailing_hash_means_no_fragment() {
        assert_eq!(split_fragment("lib.sch#"), ("lib.sch", None));
        assert_eq!(split_fragment("lib.sch"), ("lib.sch", None));
        assert_eq!(split_fragment("lib.sch#a"), ("lib.sch", Some("a")));
        assert_eq!(split_fragment("#a"), ("", Some("a")));
    }

    #[test]
    fn extends_rule_is_left_for_the_expansion_pass() {
        // `extends rule=` names an abstract rule in this schema, not a
        // document, so pass 2 must not touch it.
        let document = resolve(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <extends rule="base"/>
               </schema>"#,
            &MemoryResolver::new(),
        )
        .unwrap();
        assert_eq!(spliced(&document), ["extends"]);
    }

    #[test]
    fn documents_without_includes_survive_unchanged() {
        let document = resolve(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern id="p"><rule context="a"/></pattern>
               </schema>"#,
            &MemoryResolver::new(),
        )
        .unwrap();
        let schema = document.document_element().unwrap();
        assert_eq!(document.name(schema).unwrap().local, "schema");
        assert_eq!(document.children(schema).len(), 3);
    }
}
