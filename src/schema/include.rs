//! Pass 2 of schema compilation: resolving `<sch:include>`.
//!
//! An include is replaced, in place, by the document element of the document
//! it names. Resolution happens on the XML tree, before the model is built,
//! so an included fragment can itself contain includes and can be any
//! Schematron element — a pattern, a rule, a diagnostic.

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

    /// Whether a node is `<sch:include>`, returning its `@href`.
    fn include_href(document: &Document, node: NodeId) -> Option<String> {
        if document.kind(node) != NodeKind::Element {
            return None;
        }
        let name = document.name(node)?;
        let in_schematron = matches!(
            name.uri.as_deref(),
            Some(SCHEMATRON_NAMESPACE | SCHEMATRON_1_5_NAMESPACE)
        );
        if !in_schematron || name.local != "include" {
            return None;
        }
        document
            .attributes(node)
            .iter()
            .copied()
            .find(|&a| {
                document
                    .name(a)
                    .is_some_and(|n| n.uri.is_none() && n.local == "href")
            })
            .map(|a| document.value(a).to_string())
    }

    /// Copies one node, expanding it if it is an include.
    fn copy(
        &mut self,
        source: &Document,
        node: NodeId,
        output: &mut Document,
        parent: NodeId,
    ) -> Result<()> {
        if let Some(href) = Self::include_href(source, node) {
            return self.expand(&href, output, parent);
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

    /// Fetches an included document and copies its document element in place
    /// of the `include` element.
    fn expand(
        &mut self,
        href: &str,
        output: &mut Document,
        parent: NodeId,
    ) -> Result<()> {
        if self.chain.len() >= self.max_depth {
            return Err(Error::IncludeDepth {
                limit: self.max_depth,
                href: href.to_string(),
            });
        }

        let base = self.chain.last().map(String::as_str);
        let rebased = self.resolver.rebase(href, base);
        let key = rebased.clone().unwrap_or_else(|| href.to_string());
        if self.chain.iter().any(|seen| seen == &key) {
            let mut chain = self.chain.clone();
            chain.push(key);
            return Err(Error::IncludeCycle {
                chain: chain.join(" -> "),
            });
        }

        let text = self.resolver.resolve(href, base)?;
        let mut included = Document::from_str(&text)?;
        if let Some(rebased) = rebased {
            included.set_base_uri(rebased);
        }
        let element = included.document_element().ok_or_else(|| Error::Resolve {
            href: href.to_string(),
            message: "the included document has no root element".to_string(),
        })?;

        self.chain.push(key);
        let result = self.copy(&included, element, output, parent);
        self.chain.pop();
        result
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
