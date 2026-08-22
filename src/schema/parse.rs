//! Pass 3 of schema compilation: XML tree to [`SchemaModel`].
//!
//! This is also where the schema's own content model is checked — required
//! attributes, mutually exclusive attributes, unknown Schematron elements —
//! so that later passes can assume a well-formed model. Elements from foreign
//! namespaces are ignored, which the standard requires so that schemas can
//! carry annotations from other vocabularies.

use super::model::{
    Assertion, AssertionKind, Content, Diagnostic, Key, Let, LetValue, Ns, Paragraph, Param,
    Pattern, Phase, Property, QueryBinding, Rule, RuleChild, SchemaModel,
    SCHEMATRON_1_5_NAMESPACE, SCHEMATRON_NAMESPACE,
};
use crate::error::{Error, Result};
use crate::xml::{Document, NodeId, NodeKind};

/// Parses a Schematron schema document into a model.
///
/// The document must already have had its `include` elements resolved.
pub(crate) fn parse_schema(document: &Document) -> Result<SchemaModel> {
    let root = document
        .document_element()
        .ok_or_else(|| Error::schema("schema", None, "the document has no root element"))?;

    let parser = Parser { document };
    if !parser.is_schematron(root) {
        let name = document
            .name(root)
            .map_or_else(String::new, crate::xml::QName::display_name);
        return Err(Error::schema(
            "schema",
            None,
            format!(
                "the root element is <{name}>, not a Schematron <schema> in namespace \
                 {SCHEMATRON_NAMESPACE}"
            ),
        ));
    }
    if parser.local_name(root) != "schema" {
        return Err(Error::schema(
            parser.local_name(root),
            None,
            "the root element of a schema must be <schema>",
        ));
    }
    parser.schema(root)
}

struct Parser<'a> {
    document: &'a Document,
}

impl Parser<'_> {
    /// Whether a node is a Schematron element, in either namespace.
    fn is_schematron(&self, node: NodeId) -> bool {
        self.document.kind(node) == NodeKind::Element
            && self.document.name(node).is_some_and(|n| {
                matches!(
                    n.uri.as_deref(),
                    Some(SCHEMATRON_NAMESPACE | SCHEMATRON_1_5_NAMESPACE)
                )
            })
    }

    fn local_name(&self, node: NodeId) -> String {
        self.document
            .name(node)
            .map_or_else(String::new, |n| n.local.clone())
    }

    /// The Schematron element children of a node, in document order.
    fn elements(&self, node: NodeId) -> Vec<NodeId> {
        self.document
            .children(node)
            .iter()
            .copied()
            .filter(|&child| self.is_schematron(child))
            .collect()
    }

    /// An attribute in no namespace, which is how Schematron spells all of
    /// its own attributes.
    fn attribute(&self, node: NodeId, name: &str) -> Option<String> {
        self.document
            .attributes(node)
            .iter()
            .copied()
            .find(|&a| {
                self.document
                    .name(a)
                    .is_some_and(|n| n.uri.is_none() && n.local == name)
            })
            .map(|a| self.document.value(a).to_string())
    }

    /// An attribute in the XML namespace, such as `xml:lang`.
    fn xml_attribute(&self, node: NodeId, name: &str) -> Option<String> {
        self.document
            .attributes(node)
            .iter()
            .copied()
            .find(|&a| {
                self.document.name(a).is_some_and(|n| {
                    n.uri.as_deref() == Some(crate::xml::XML_NAMESPACE) && n.local == name
                })
            })
            .map(|a| self.document.value(a).to_string())
    }

    fn required_attribute(
        &self,
        node: NodeId,
        name: &str,
        location: &str,
    ) -> Result<String> {
        self.attribute(node, name).ok_or_else(|| {
            Error::schema(
                self.local_name(node),
                Some(location.to_string()),
                format!("the @{name} attribute is required"),
            )
        })
    }

    /// A boolean attribute; Schematron spells true as the literal `"true"`.
    fn boolean_attribute(&self, node: NodeId, name: &str) -> bool {
        self.attribute(node, name).as_deref() == Some("true")
    }

    /// Splits a whitespace-separated list of identifiers.
    fn id_list(&self, node: NodeId, name: &str) -> Vec<String> {
        self.attribute(node, name)
            .map(|value| value.split_whitespace().map(ToString::to_string).collect())
            .unwrap_or_default()
    }

    fn schema(&self, node: NodeId) -> Result<SchemaModel> {
        let mut model = SchemaModel {
            id: self.attribute(node, "id"),
            schema_version: self.attribute(node, "schemaVersion"),
            default_phase: self.attribute(node, "defaultPhase"),
            query_binding: self
                .attribute(node, "queryBinding")
                .map_or(QueryBinding::Default, |v| QueryBinding::parse(&v)),
            lang: self.xml_attribute(node, "lang"),
            ..SchemaModel::default()
        };

        for child in self.elements(node) {
            let name = self.local_name(child);
            match name.as_str() {
                "title" => model.title = Some(self.plain_text(child)),
                "ns" => model.namespaces.push(self.ns(child)?),
                "let" => model.lets.push(self.binding(child, "schema")?),
                "phase" => model.phases.push(self.phase(child)?),
                "key" => model.keys.push(Key {
                    name: self.required_attribute(child, "name", "schema/key")?,
                    match_pattern: self.required_attribute(child, "match", "schema/key")?,
                    use_expression: self.required_attribute(child, "use", "schema/key")?,
                }),
                "pattern" => {
                    let index = model.patterns.len();
                    model.patterns.push(self.pattern(child, index)?);
                }
                "diagnostics" => {
                    for diagnostic in self.elements(child) {
                        if self.local_name(diagnostic) == "diagnostic" {
                            model.diagnostics.push(self.diagnostic(diagnostic)?);
                        }
                    }
                }
                "properties" => {
                    for property in self.elements(child) {
                        if self.local_name(property) == "property" {
                            model.properties.push(self.property(property)?);
                        }
                    }
                }
                "p" => model.paragraphs.push(self.paragraph(child)),
                // `include` is resolved before this pass; reaching one here
                // means resolution was skipped, which is a caller error.
                "include" => {
                    return Err(Error::schema(
                        "include",
                        Some("schema".into()),
                        "an include survived resolution; this is a bug in the caller",
                    ))
                }
                other => {
                    return Err(Error::schema(
                        other,
                        Some("schema".into()),
                        format!("<{other}> is not a Schematron element"),
                    ))
                }
            }
        }

        if model.patterns.is_empty() {
            return Err(Error::schema(
                "schema",
                None,
                "a schema must contain at least one <pattern>",
            ));
        }
        Ok(model)
    }

    fn ns(&self, node: NodeId) -> Result<Ns> {
        Ok(Ns {
            prefix: self.required_attribute(node, "prefix", "schema/ns")?,
            uri: self.required_attribute(node, "uri", "schema/ns")?,
        })
    }

    fn binding(&self, node: NodeId, location: &str) -> Result<Let> {
        let name = self.required_attribute(node, "name", &format!("{location}/let"))?;
        let value = match self.attribute(node, "value") {
            Some(expression) => LetValue::Expression(expression),
            None => LetValue::Content(self.content(node)),
        };
        Ok(Let { name, value })
    }

    fn phase(&self, node: NodeId) -> Result<Phase> {
        let id = self.required_attribute(node, "id", "schema/phase")?;
        let location = format!("phase[@id='{id}']");
        let mut phase = Phase {
            id,
            ..Phase::default()
        };
        for child in self.elements(node) {
            match self.local_name(child).as_str() {
                "active" => phase
                    .actives
                    .push(self.required_attribute(child, "pattern", &location)?),
                "let" => phase.lets.push(self.binding(child, &location)?),
                "p" => phase.paragraphs.push(self.paragraph(child)),
                other => {
                    return Err(Error::schema(
                        other,
                        Some(location),
                        format!("<{other}> is not allowed inside <phase>"),
                    ))
                }
            }
        }
        Ok(phase)
    }

    fn pattern(&self, node: NodeId, index: usize) -> Result<Pattern> {
        let id = self.attribute(node, "id");
        let location = id.as_ref().map_or_else(
            || format!("pattern[{}]", index + 1),
            |id| format!("pattern[@id='{id}']"),
        );

        let mut pattern = Pattern {
            id: id.clone(),
            is_abstract: self.boolean_attribute(node, "abstract"),
            is_a: self.attribute(node, "is-a"),
            documents: self.attribute(node, "documents"),
            ..Pattern::default()
        };

        if pattern.is_abstract && pattern.is_a.is_some() {
            return Err(Error::schema(
                "pattern",
                Some(location),
                "a pattern cannot be both abstract and an instance of one; \
                 drop either @abstract or @is-a",
            ));
        }
        if pattern.is_abstract && pattern.id.is_none() {
            return Err(Error::schema(
                "pattern",
                Some(location),
                "an abstract pattern needs an @id so that @is-a can reference it",
            ));
        }

        for child in self.elements(node) {
            match self.local_name(child).as_str() {
                "title" => pattern.title = Some(self.plain_text(child)),
                "let" => pattern.lets.push(self.binding(child, &location)?),
                "rule" => {
                    let rule_index = pattern.rules.len();
                    pattern.rules.push(self.rule(child, &location, rule_index)?);
                }
                "param" => pattern.params.push(Param {
                    name: self.required_attribute(child, "name", &location)?,
                    value: self.required_attribute(child, "value", &location)?,
                }),
                "p" => pattern.paragraphs.push(self.paragraph(child)),
                other => {
                    return Err(Error::schema(
                        other,
                        Some(location),
                        format!("<{other}> is not allowed inside <pattern>"),
                    ))
                }
            }
        }

        if pattern.is_a.is_some() && !pattern.rules.is_empty() {
            return Err(Error::schema(
                "pattern",
                Some(location),
                "an @is-a pattern takes its rules from the abstract pattern, \
                 so it may contain only <param> children",
            ));
        }
        Ok(pattern)
    }

    fn rule(&self, node: NodeId, pattern_location: &str, index: usize) -> Result<Rule> {
        let context = self.attribute(node, "context");
        let id = self.attribute(node, "id");
        let location = match (&context, &id) {
            (Some(context), _) => format!("{pattern_location}/rule[@context='{context}']"),
            (None, Some(id)) => format!("{pattern_location}/rule[@id='{id}']"),
            (None, None) => format!("{pattern_location}/rule[{}]", index + 1),
        };

        let is_abstract = self.boolean_attribute(node, "abstract");
        if is_abstract && context.is_some() {
            return Err(Error::schema(
                "rule",
                Some(location),
                "an abstract rule must not have a @context; it is spliced into \
                 the rule that extends it",
            ));
        }
        if is_abstract && id.is_none() {
            return Err(Error::schema(
                "rule",
                Some(location),
                "an abstract rule needs an @id so that <extends> can reference it",
            ));
        }
        if !is_abstract && context.is_none() {
            return Err(Error::schema(
                "rule",
                Some(location),
                "a rule needs a @context, unless it is abstract",
            ));
        }

        let mut rule = Rule {
            context,
            id,
            is_abstract,
            flag: self.attribute(node, "flag"),
            role: self.attribute(node, "role"),
            subject: self.attribute(node, "subject"),
            ..Rule::default()
        };

        for child in self.elements(node) {
            match self.local_name(child).as_str() {
                "let" => rule.lets.push(self.binding(child, &location)?),
                "assert" => rule.body.push(RuleChild::Assertion(self.assertion(
                    child,
                    AssertionKind::Assert,
                    &location,
                )?)),
                "report" => rule.body.push(RuleChild::Assertion(self.assertion(
                    child,
                    AssertionKind::Report,
                    &location,
                )?)),
                "extends" => {
                    let target = self.attribute(child, "rule").ok_or_else(|| {
                        Error::schema(
                            "extends",
                            Some(location.clone()),
                            "an <extends> needs @rule or @href; an @href is \
                             spliced in earlier, when includes are resolved, \
                             so reaching here means neither was present",
                        )
                    })?;
                    rule.body.push(RuleChild::Extends(target));
                }
                other => {
                    return Err(Error::schema(
                        other,
                        Some(location),
                        format!("<{other}> is not allowed inside <rule>"),
                    ))
                }
            }
        }
        Ok(rule)
    }

    fn assertion(
        &self,
        node: NodeId,
        kind: AssertionKind,
        rule_location: &str,
    ) -> Result<Assertion> {
        let location = format!("{rule_location}/{}", kind.as_str());
        Ok(Assertion {
            kind,
            test: self.required_attribute(node, "test", &location)?,
            id: self.attribute(node, "id"),
            flag: self.attribute(node, "flag"),
            role: self.attribute(node, "role"),
            subject: self.attribute(node, "subject"),
            diagnostics: self.id_list(node, "diagnostics"),
            properties: self.id_list(node, "properties"),
            see: self.attribute(node, "see"),
            icon: self.attribute(node, "icon"),
            fpi: self.attribute(node, "fpi"),
            content: self.content(node),
        })
    }

    fn diagnostic(&self, node: NodeId) -> Result<Diagnostic> {
        Ok(Diagnostic {
            id: self.required_attribute(node, "id", "schema/diagnostics/diagnostic")?,
            lang: self.xml_attribute(node, "lang"),
            content: self.content(node),
        })
    }

    fn property(&self, node: NodeId) -> Result<Property> {
        Ok(Property {
            id: self.required_attribute(node, "id", "schema/properties/property")?,
            role: self.attribute(node, "role"),
            scheme: self.attribute(node, "scheme"),
            content: self.content(node),
        })
    }

    fn paragraph(&self, node: NodeId) -> Paragraph {
        Paragraph {
            id: self.attribute(node, "id"),
            class: self.attribute(node, "class"),
            icon: self.attribute(node, "icon"),
            content: self.content(node),
        }
    }

    /// Reads mixed content into a sequence of [`Content`] fragments.
    ///
    /// Unknown child elements contribute their text but not their markup,
    /// which keeps a schema annotated with, say, XHTML from losing its
    /// message text.
    fn content(&self, node: NodeId) -> Vec<Content> {
        let mut out = Vec::new();
        for &child in self.document.children(node) {
            match self.document.kind(child) {
                NodeKind::Text => {
                    let text = self.document.value(child);
                    if !text.is_empty() {
                        out.push(Content::Text(text.to_string()));
                    }
                }
                NodeKind::Element if self.is_schematron(child) => {
                    match self.local_name(child).as_str() {
                        "value-of" => out.push(Content::ValueOf {
                            select: self.attribute(child, "select").unwrap_or_default(),
                        }),
                        "name" => out.push(Content::Name {
                            path: self.attribute(child, "path"),
                        }),
                        "emph" => out.push(Content::Emph(self.content(child))),
                        "span" => out.push(Content::Span {
                            class: self.attribute(child, "class"),
                            content: self.content(child),
                        }),
                        "dir" => out.push(Content::Dir {
                            value: self.attribute(child, "value"),
                            content: self.content(child),
                        }),
                        // Any other Schematron element in mixed content is
                        // not part of the message; skip its markup.
                        _ => out.extend(self.content(child)),
                    }
                }
                NodeKind::Element => out.extend(self.content(child)),
                _ => {}
            }
        }
        out
    }

    /// The plain text of an element, for titles.
    fn plain_text(&self, node: NodeId) -> String {
        self.document.string_value(node).trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(source: &str) -> SchemaModel {
        let document = Document::from_str(source).unwrap();
        parse_schema(&document).unwrap()
    }

    fn error(source: &str) -> String {
        let document = Document::from_str(source).unwrap();
        parse_schema(&document).unwrap_err().to_string()
    }

    const MINIMAL: &str = r#"
        <schema xmlns="http://purl.oclc.org/dsdl/schematron">
          <pattern><rule context="a"><assert test="b">m</assert></rule></pattern>
        </schema>
    "#;

    #[test]
    fn parses_a_minimal_schema() {
        let m = model(MINIMAL);
        assert_eq!(m.patterns.len(), 1);
        assert_eq!(m.patterns[0].rules.len(), 1);
        assert_eq!(m.patterns[0].rules[0].context.as_deref(), Some("a"));
        let assertion = m.patterns[0].rules[0].assertions().next().unwrap();
        assert_eq!(assertion.test, "b");
        assert_eq!(assertion.kind, AssertionKind::Assert);
    }

    #[test]
    fn parses_schema_attributes() {
        let m = model(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron"
                       id="s" schemaVersion="2" defaultPhase="p" queryBinding="xslt" xml:lang="en">
                 <title>T</title>
                 <ns prefix="x" uri="urn:x"/>
                 <phase id="p"><active pattern="q"/></phase>
                 <pattern id="q"><rule context="a"><assert test="b">m</assert></rule></pattern>
               </schema>"#,
        );
        assert_eq!(m.id.as_deref(), Some("s"));
        assert_eq!(m.schema_version.as_deref(), Some("2"));
        assert_eq!(m.default_phase.as_deref(), Some("p"));
        assert_eq!(m.query_binding, QueryBinding::Xslt);
        assert_eq!(m.lang.as_deref(), Some("en"));
        assert_eq!(m.title.as_deref(), Some("T"));
        assert_eq!(m.namespaces[0].prefix, "x");
        assert_eq!(m.phases[0].actives, vec!["q".to_string()]);
    }

    #[test]
    fn accepts_the_legacy_namespace() {
        let m = model(
            r#"<schema xmlns="http://www.ascc.net/xml/schematron">
                 <pattern><rule context="a"><assert test="b">m</assert></rule></pattern>
               </schema>"#,
        );
        assert_eq!(m.patterns.len(), 1);
    }

    #[test]
    fn rejects_a_non_schematron_root() {
        let message = error("<schema><pattern/></schema>");
        assert!(message.contains("not a Schematron"), "{message}");
    }

    #[test]
    fn rejects_a_schema_with_no_patterns() {
        let message = error(r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron"/>"#);
        assert!(message.contains("at least one"), "{message}");
    }

    #[test]
    fn requires_a_test_on_an_assertion() {
        let message = error(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern><rule context="a"><assert>m</assert></rule></pattern>
               </schema>"#,
        );
        assert!(message.contains("@test"), "{message}");
    }

    #[test]
    fn requires_a_context_on_a_concrete_rule() {
        let message = error(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern><rule><assert test="b">m</assert></rule></pattern>
               </schema>"#,
        );
        assert!(message.contains("@context"), "{message}");
    }

    #[test]
    fn rejects_an_abstract_rule_with_a_context() {
        let message = error(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern><rule abstract="true" id="r" context="a"/></pattern>
               </schema>"#,
        );
        assert!(message.contains("must not have a @context"), "{message}");
    }

    #[test]
    fn rejects_a_pattern_that_is_both_abstract_and_an_instance() {
        let message = error(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern id="p" abstract="true" is-a="q"/>
               </schema>"#,
        );
        assert!(message.contains("cannot be both"), "{message}");
    }

    #[test]
    fn rejects_unknown_schematron_elements() {
        let message = error(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <nonsense/>
                 <pattern><rule context="a"><assert test="b">m</assert></rule></pattern>
               </schema>"#,
        );
        assert!(message.contains("not a Schematron element"), "{message}");
    }

    #[test]
    fn parses_rich_content() {
        let m = model(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern><rule context="a">
                   <assert test="b">Value <value-of select="@x"/> of <name/> is wrong.</assert>
                 </rule></pattern>
               </schema>"#,
        );
        let content = &m.patterns[0].rules[0].assertions().next().unwrap().content;
        assert_eq!(content.len(), 5);
        assert!(matches!(content[1], Content::ValueOf { .. }));
        assert!(matches!(content[3], Content::Name { path: None }));
    }

    #[test]
    fn parses_diagnostics_and_references_to_them() {
        let m = model(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern><rule context="a">
                   <assert test="b" diagnostics="d1 d2">m</assert>
                 </rule></pattern>
                 <diagnostics>
                   <diagnostic id="d1">one</diagnostic>
                   <diagnostic id="d2">two</diagnostic>
                 </diagnostics>
               </schema>"#,
        );
        let assertion = m.patterns[0].rules[0].assertions().next().unwrap();
        assert_eq!(assertion.diagnostics, vec!["d1", "d2"]);
        assert_eq!(m.diagnostics.len(), 2);
        assert!(m.diagnostic("d1").is_some());
    }

    #[test]
    fn parses_both_let_forms() {
        let m = model(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <let name="a" value="1"/>
                 <let name="b">text</let>
                 <pattern><rule context="a"><assert test="b">m</assert></rule></pattern>
               </schema>"#,
        );
        assert!(matches!(m.lets[0].value, LetValue::Expression(_)));
        assert!(matches!(m.lets[1].value, LetValue::Content(_)));
    }

    #[test]
    fn ignores_foreign_namespace_elements() {
        let m = model(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron" xmlns:h="urn:h">
                 <h:note>ignored</h:note>
                 <pattern><rule context="a"><assert test="b">m</assert></rule></pattern>
               </schema>"#,
        );
        assert_eq!(m.patterns.len(), 1);
    }

    #[test]
    fn keeps_rule_body_order_for_extends() {
        let m = model(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern>
                   <rule abstract="true" id="base"><assert test="x">base</assert></rule>
                   <rule context="a">
                     <assert test="one">1</assert>
                     <extends rule="base"/>
                     <assert test="two">2</assert>
                   </rule>
                 </pattern>
               </schema>"#,
        );
        let body = &m.patterns[0].rules[1].body;
        assert!(matches!(body[0], RuleChild::Assertion(_)));
        assert!(matches!(body[1], RuleChild::Extends(_)));
        assert!(matches!(body[2], RuleChild::Assertion(_)));
    }
}
