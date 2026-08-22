//! Schema linting: finding constructs that are legal but almost certainly
//! wrong.
//!
//! Schematron makes two mistakes easy to write and neither produces an error.
//! A rule shadowed by an earlier one in the same pattern never fires; an
//! unprefixed name in a namespaced vocabulary matches nothing. Both leave a
//! schema that compiles, runs, and reports nothing — which reads exactly like
//! a clean document.
//!
//! A lint is not a validation finding and not a compile error. It is a remark
//! about the schema, made without looking at any document. See
//! `spec/linting.md`.
//!
//! # Examples
//!
//! ```
//! use schematron::Schema;
//!
//! let schema = Schema::from_str(r#"
//!     <schema xmlns="http://purl.oclc.org/dsdl/schematron">
//!       <pattern>
//!         <rule context="*"><assert test="@id">needs an id</assert></rule>
//!         <rule context="invoice"><assert test="total">needs a total</assert></rule>
//!       </pattern>
//!     </schema>
//! "#)?;
//!
//! let lints = schema.lint();
//! assert_eq!(lints.len(), 1);
//! assert!(lints[0].message.contains("can never fire"));
//! # Ok::<(), schematron::Error>(())
//! ```

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::schema::{Content, Pattern, Rule, Schema};
use crate::xpath::{Expr, NodeTest, PathStart};

/// What kind of problem a [`Lint`] reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[non_exhaustive]
pub enum LintKind {
    /// A rule no node can reach, because an earlier rule in the same pattern
    /// claims everything it would match.
    UnreachableRule,
    /// Two rules in one pattern share a `@context`; the second never fires.
    DuplicateRuleContext,
    /// An unprefixed element name, in a schema that declares prefixes.
    UnprefixedNameInNamespacedSchema,
    /// A `diagnostic` that no assertion references.
    UnreferencedDiagnostic,
    /// A `property` that no assertion references.
    UnreferencedProperty,
    /// An assertion whose human-readable message is empty.
    EmptyMessage,
    /// A test that does not depend on the document.
    ConstantTest,
    /// A pattern that no phase activates, in a schema that declares phases.
    PatternInNoPhase,
}

impl LintKind {
    /// A short stable identifier, for filtering and for machine consumers.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            LintKind::UnreachableRule => "unreachable-rule",
            LintKind::DuplicateRuleContext => "duplicate-rule-context",
            LintKind::UnprefixedNameInNamespacedSchema => "unprefixed-name",
            LintKind::UnreferencedDiagnostic => "unreferenced-diagnostic",
            LintKind::UnreferencedProperty => "unreferenced-property",
            LintKind::EmptyMessage => "empty-message",
            LintKind::ConstantTest => "constant-test",
            LintKind::PatternInNoPhase => "pattern-in-no-phase",
        }
    }
}

/// One remark about a schema.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Lint {
    /// What kind of problem this is.
    pub kind: LintKind,
    /// Where in the schema, as a readable path such as
    /// `pattern[@id='lines']/rule[@context='line']`.
    pub location: String,
    /// What is wrong.
    pub message: String,
    /// What to do about it.
    pub help: Option<String>,
}

impl Lint {
    fn new(
        kind: LintKind,
        location: impl Into<String>,
        message: impl Into<String>,
        help: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            location: location.into(),
            message: message.into(),
            help: Some(help.into()),
        }
    }
}

impl std::fmt::Display for Lint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.location, self.message)
    }
}

impl Schema {
    /// Inspects this schema for constructs that are legal but probably wrong.
    ///
    /// Returns lints in schema order, so the output reads down the file. An
    /// empty result does not mean the schema is correct — only that none of
    /// the patterns in `spec/linting.md` matched.
    ///
    /// # Examples
    ///
    /// ```
    /// use schematron::Schema;
    ///
    /// let schema = Schema::from_str(r#"
    ///     <schema xmlns="http://purl.oclc.org/dsdl/schematron">
    ///       <pattern><rule context="a"><assert test="b">m</assert></rule></pattern>
    ///     </schema>
    /// "#)?;
    /// assert!(schema.lint().is_empty());
    /// # Ok::<(), schematron::Error>(())
    /// ```
    #[must_use]
    pub fn lint(&self) -> Vec<Lint> {
        let mut lints = Vec::new();
        let model = self.model();

        for (index, pattern) in model.patterns.iter().enumerate() {
            let location = pattern_location(pattern, index);
            self.lint_pattern(pattern, &location, &mut lints);
            lint_pattern_phases(self, pattern, &location, &mut lints);
        }

        lint_unreferenced(self, &mut lints);
        lints
    }

    fn lint_pattern(&self, pattern: &Pattern, location: &str, lints: &mut Vec<Lint>) {
        // A context that claims every node of its kind makes every later rule
        // in the pattern unreachable, whatever those rules say.
        let mut claimed_by: Option<(&str, &str)> = None;
        let mut seen: Vec<&str> = Vec::new();

        for (index, rule) in pattern.rules.iter().enumerate() {
            let Some(context) = rule.context.as_deref() else {
                continue;
            };
            let rule_location = format!("{location}/{}", rule_step(index));

            if let Some((earlier, kind)) = claimed_by {
                lints.push(Lint::new(
                    LintKind::UnreachableRule,
                    &rule_location,
                    format!(
                        "the rule with context {context:?} can never fire: an earlier \
                         rule in the same pattern, context={earlier:?}, already claims \
                         every {kind}"
                    ),
                    "within one pattern a node is processed by the first matching rule \
                     only. Move this rule into a pattern of its own, or put it before \
                     the broader rule.",
                ));
            } else if let Some(earlier) = seen.iter().find(|&&c| c == context) {
                lints.push(Lint::new(
                    LintKind::DuplicateRuleContext,
                    &rule_location,
                    format!(
                        "the rule with context {context:?} can never fire: an earlier \
                         rule in the same pattern has the same context {earlier:?}"
                    ),
                    "merge the two rules, or move this one into a pattern of its own.",
                ));
            } else if let Some(kind) = universal_context(self, context) {
                claimed_by = Some((context, kind));
            }
            seen.push(context);

            self.lint_rule(rule, &rule_location, lints);
        }
    }

    fn lint_rule(&self, rule: &Rule, location: &str, lints: &mut Vec<Lint>) {
        if let Some(context) = rule.context.as_deref() {
            self.lint_unprefixed(context, location, "@context", lints);
        }

        for (index, assertion) in rule.assertions().enumerate() {
            let where_ = format!(
                "{location}/{}[{}]",
                assertion.kind.as_str(),
                index + 1
            );

            if is_message_empty(&assertion.content) {
                lints.push(Lint::new(
                    LintKind::EmptyMessage,
                    &where_,
                    "this assertion has no message",
                    "a report that says nothing cannot be acted on; describe what is \
                     wrong and, where it helps, what the value was.",
                ));
            }

            if let Some(constant) = constant_test(self, &assertion.test) {
                lints.push(Lint::new(
                    LintKind::ConstantTest,
                    &where_,
                    format!(
                        "the test {:?} is constant, so this assertion {constant} \
                         regardless of the document",
                        assertion.test
                    ),
                    "a constant test is usually a placeholder left behind, or a \
                     mistake in the expression.",
                ));
            }

            self.lint_unprefixed(&assertion.test, &where_, "@test", lints);
        }
    }

    /// Reports an unprefixed element name in a schema that declares prefixes.
    fn lint_unprefixed(
        &self,
        source: &str,
        location: &str,
        attribute: &str,
        lints: &mut Vec<Lint>,
    ) {
        if self.namespaces().is_empty() {
            return;
        }
        let Ok(expr) = self.expression(source) else {
            return;
        };
        let Some(name) = first_unprefixed_element_name(expr) else {
            return;
        };
        let prefixes: Vec<&str> = self
            .model()
            .namespaces
            .iter()
            .map(|ns| ns.prefix.as_str())
            .collect();
        lints.push(Lint::new(
            LintKind::UnprefixedNameInNamespacedSchema,
            location,
            format!(
                "{attribute}={source:?} uses the unprefixed name {name:?}, which \
                 matches only elements in no namespace"
            ),
            format!(
                "XPath 1.0 has no default namespace. If {name:?} is in a namespace, \
                 write it with one of the prefixes this schema declares: {}.",
                prefixes.join(", ")
            ),
        ));
    }
}

/// A context that claims every node of some kind, making later rules in the
/// same pattern unreachable.
///
/// Deliberately limited to the three certain cases. General subsumption of
/// XPath patterns is not practical to decide, and a linter with false
/// positives gets switched off — after which it catches nothing at all.
fn universal_context(schema: &Schema, context: &str) -> Option<&'static str> {
    let expr = schema.expression(context).ok()?;
    let Expr::Path(path) = expr else {
        return None;
    };
    if path.start != PathStart::Context || path.steps.len() != 1 {
        return None;
    }
    let step = &path.steps[0];
    if !step.predicates.is_empty() {
        return None;
    }
    match (&step.node_test, step.axis) {
        (NodeTest::Wildcard, crate::xpath::Axis::Attribute) => Some("attribute"),
        (NodeTest::Wildcard, crate::xpath::Axis::Child) => Some("element"),
        (NodeTest::AnyNode, crate::xpath::Axis::Child) => Some("node"),
        _ => None,
    }
}

/// Whether a test is a constant, and what that makes the assertion do.
fn constant_test(schema: &Schema, source: &str) -> Option<&'static str> {
    let expr = schema.expression(source).ok()?;
    match expr {
        Expr::Function { name, args } if args.is_empty() && name == "true" => {
            Some("always holds")
        }
        Expr::Function { name, args } if args.is_empty() && name == "false" => {
            Some("never holds")
        }
        Expr::Number(_) | Expr::Literal(_) => Some("has a fixed outcome"),
        _ => None,
    }
}

/// The first unprefixed element name test in an expression, if any.
///
/// Wildcards and node-type tests are not reported: `*` and `node()` are not
/// namespace mistakes, and neither is an attribute name, since an unprefixed
/// attribute really is in no namespace.
fn first_unprefixed_element_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Literal(_) | Expr::Number(_) | Expr::Variable(_) => None,
        Expr::Negate(inner) => first_unprefixed_element_name(inner),
        Expr::Binary(_, left, right) => {
            first_unprefixed_element_name(left).or_else(|| first_unprefixed_element_name(right))
        }
        Expr::Function { args, .. } => args.iter().find_map(first_unprefixed_element_name),
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => [condition, then_branch, else_branch]
            .into_iter()
            .find_map(|branch| first_unprefixed_element_name(branch)),
        Expr::TypeOp { value, .. } => first_unprefixed_element_name(value),
        Expr::Sequence(members) => members.iter().find_map(first_unprefixed_element_name),
        Expr::Range(from, to) => [from, to]
            .into_iter()
            .find_map(|part| first_unprefixed_element_name(part)),
        Expr::For { input, body, .. } => [input, body]
            .into_iter()
            .find_map(|part| first_unprefixed_element_name(part)),
        Expr::Quantified { input, test, .. } => [input, test]
            .into_iter()
            .find_map(|part| first_unprefixed_element_name(part)),
        Expr::Path(path) => {
            if let PathStart::Expr(start, predicates) = &path.start {
                if let Some(found) = first_unprefixed_element_name(start) {
                    return Some(found);
                }
                if let Some(found) = predicates.iter().find_map(first_unprefixed_element_name) {
                    return Some(found);
                }
            }
            for step in &path.steps {
                if step.axis != crate::xpath::Axis::Attribute {
                    if let NodeTest::Name(name) = &step.node_test {
                        if name.prefix.is_none() {
                            return Some(name.local.clone());
                        }
                    }
                }
                if let Some(found) = step.predicates.iter().find_map(first_unprefixed_element_name)
                {
                    return Some(found);
                }
            }
            None
        }
    }
}

/// Whether an assertion's rich content renders to nothing.
fn is_message_empty(content: &[Content]) -> bool {
    content.iter().all(|fragment| match fragment {
        Content::Text(text) => text.trim().is_empty(),
        // A `value-of` or a `name` produces text at validation time, so a
        // message made only of those is not empty.
        Content::ValueOf { .. } | Content::Name { .. } => false,
        Content::Emph(inner)
        | Content::Span { content: inner, .. }
        | Content::Dir { content: inner, .. } => is_message_empty(inner),
    })
}

fn lint_pattern_phases(schema: &Schema, pattern: &Pattern, location: &str, lints: &mut Vec<Lint>) {
    let model = schema.model();
    if model.phases.is_empty() {
        return;
    }
    let Some(id) = pattern.id.as_deref() else {
        // A pattern with no id cannot be activated by any phase at all.
        lints.push(Lint::new(
            LintKind::PatternInNoPhase,
            location,
            "this pattern has no @id, so no phase can activate it",
            "give it an @id and list it in a phase, or run with phase #ALL.",
        ));
        return;
    };
    if model
        .phases
        .iter()
        .any(|phase| phase.actives.iter().any(|active| active == id))
    {
        return;
    }
    lints.push(Lint::new(
        LintKind::PatternInNoPhase,
        location,
        format!("no phase activates the pattern {id:?}"),
        "it runs only under phase #ALL. Add an <active pattern=\"…\"/> to a phase, \
         or remove the pattern.",
    ));
}

fn lint_unreferenced(schema: &Schema, lints: &mut Vec<Lint>) {
    let model = schema.model();
    let assertions = || {
        model
            .patterns
            .iter()
            .flat_map(|pattern| pattern.rules.iter())
            .flat_map(Rule::assertions)
    };

    for diagnostic in &model.diagnostics {
        if !assertions().any(|a| a.diagnostics.contains(&diagnostic.id)) {
            lints.push(Lint::new(
                LintKind::UnreferencedDiagnostic,
                format!("diagnostic[@id='{}']", diagnostic.id),
                "no assertion references this diagnostic",
                "reference it with diagnostics=\"…\" on an assert or report, or \
                 delete it.",
            ));
        }
    }

    for property in &model.properties {
        if !assertions().any(|a| a.properties.contains(&property.id)) {
            lints.push(Lint::new(
                LintKind::UnreferencedProperty,
                format!("property[@id='{}']", property.id),
                "no assertion references this property",
                "reference it with properties=\"…\" on an assert or report, or \
                 delete it.",
            ));
        }
    }
}

fn pattern_location(pattern: &Pattern, index: usize) -> String {
    pattern.id.as_ref().map_or_else(
        || format!("pattern[{}]", index + 1),
        |id| format!("pattern[@id='{id}']"),
    )
}

/// A rule's position within its pattern.
///
/// Positional rather than by `@context`, because two rules in one pattern can
/// share a context — which is itself one of the things being reported, and
/// would give two lints identical, indistinguishable locations.
fn rule_step(index: usize) -> String {
    format!("rule[{}]", index + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lints_of(body: &str) -> Vec<Lint> {
        let source =
            format!(r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">{body}</schema>"#);
        Schema::from_str(&source)
            .expect("schema should compile")
            .lint()
    }

    fn kinds(body: &str) -> Vec<LintKind> {
        lints_of(body).into_iter().map(|lint| lint.kind).collect()
    }

    #[test]
    fn a_clean_schema_has_no_lints() {
        assert!(kinds(
            r#"<pattern><rule context="a"><assert test="b">needs b</assert></rule></pattern>"#
        )
        .is_empty());
    }

    #[test]
    fn a_rule_after_a_wildcard_rule_is_unreachable() {
        let found = kinds(
            r#"<pattern>
                 <rule context="*"><assert test="@id">m</assert></rule>
                 <rule context="invoice"><assert test="total">m</assert></rule>
               </pattern>"#,
        );
        assert_eq!(found, vec![LintKind::UnreachableRule]);
    }

    #[test]
    fn node_test_and_attribute_wildcards_also_claim_everything() {
        for universal in ["node()", "@*"] {
            let found = kinds(&format!(
                r#"<pattern>
                     <rule context="{universal}"><assert test="true()">m</assert></rule>
                     <rule context="a"><assert test="b">m</assert></rule>
                   </pattern>"#
            ));
            assert!(
                found.contains(&LintKind::UnreachableRule),
                "{universal} did not shadow: {found:?}"
            );
        }
    }

    #[test]
    fn a_wildcard_rule_last_is_fine() {
        // The idiomatic catch-all ordering must not be reported.
        let found = kinds(
            r#"<pattern>
                 <rule context="invoice"><assert test="total">m</assert></rule>
                 <rule context="*"><assert test="@id">m</assert></rule>
               </pattern>"#,
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_wildcard_with_a_predicate_is_not_universal() {
        // `*[@x]` does not claim every element, so nothing is shadowed.
        let found = kinds(
            r#"<pattern>
                 <rule context="*[@x]"><assert test="true()">m</assert></rule>
                 <rule context="a"><assert test="b">m</assert></rule>
               </pattern>"#,
        );
        assert!(!found.contains(&LintKind::UnreachableRule), "{found:?}");
    }

    #[test]
    fn wildcards_in_separate_patterns_do_not_shadow() {
        let found = kinds(
            r#"<pattern><rule context="*"><assert test="@id">m</assert></rule></pattern>
               <pattern><rule context="a"><assert test="b">m</assert></rule></pattern>"#,
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn duplicate_contexts_are_reported() {
        let found = kinds(
            r#"<pattern>
                 <rule context="a"><assert test="b">m</assert></rule>
                 <rule context="a"><assert test="c">m</assert></rule>
               </pattern>"#,
        );
        assert_eq!(found, vec![LintKind::DuplicateRuleContext]);
    }

    #[test]
    fn unprefixed_names_are_reported_only_when_prefixes_are_declared() {
        // No prefixes declared: nothing to suspect.
        assert!(kinds(
            r#"<pattern><rule context="invoice"><assert test="total">m</assert></rule></pattern>"#
        )
        .is_empty());

        // Prefixes declared, and an unprefixed context.
        let found = kinds(
            r#"<ns prefix="inv" uri="urn:inv"/>
               <pattern><rule context="invoice"><assert test="inv:total">m</assert></rule></pattern>"#,
        );
        assert_eq!(found, vec![LintKind::UnprefixedNameInNamespacedSchema]);
    }

    #[test]
    fn prefixed_names_are_not_reported() {
        let found = kinds(
            r#"<ns prefix="inv" uri="urn:inv"/>
               <pattern>
                 <rule context="inv:invoice"><assert test="inv:total">m</assert></rule>
               </pattern>"#,
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn attribute_names_and_wildcards_are_not_namespace_mistakes() {
        // An unprefixed attribute really is in no namespace, and `*` is not a
        // name at all.
        let found = kinds(
            r#"<ns prefix="inv" uri="urn:inv"/>
               <pattern>
                 <rule context="inv:invoice"><assert test="@id">m</assert></rule>
                 <rule context="*"><assert test="count(*) > 0">m</assert></rule>
               </pattern>"#,
        );
        assert!(
            !found.contains(&LintKind::UnprefixedNameInNamespacedSchema),
            "{found:?}"
        );
    }

    #[test]
    fn empty_messages_are_reported() {
        let found = kinds(r#"<pattern><rule context="a"><assert test="b">   </assert></rule></pattern>"#);
        assert_eq!(found, vec![LintKind::EmptyMessage]);
    }

    #[test]
    fn a_message_made_only_of_value_of_is_not_empty() {
        let found = kinds(
            r#"<pattern><rule context="a">
                 <assert test="b"><value-of select="@x"/></assert>
               </rule></pattern>"#,
        );
        assert!(!found.contains(&LintKind::EmptyMessage), "{found:?}");
    }

    #[test]
    fn constant_tests_are_reported() {
        let found = kinds(r#"<pattern><rule context="a"><assert test="true()">m</assert></rule></pattern>"#);
        assert_eq!(found, vec![LintKind::ConstantTest]);

        let found = kinds(r#"<pattern><rule context="a"><assert test="false()">m</assert></rule></pattern>"#);
        assert_eq!(found, vec![LintKind::ConstantTest]);
    }

    #[test]
    fn unreferenced_diagnostics_are_reported() {
        let found = kinds(
            r#"<pattern><rule context="a"><assert test="b">m</assert></rule></pattern>
               <diagnostics><diagnostic id="unused">text</diagnostic></diagnostics>"#,
        );
        assert_eq!(found, vec![LintKind::UnreferencedDiagnostic]);
    }

    #[test]
    fn referenced_diagnostics_are_not_reported() {
        let found = kinds(
            r#"<pattern><rule context="a">
                 <assert test="b" diagnostics="used">m</assert>
               </rule></pattern>
               <diagnostics><diagnostic id="used">text</diagnostic></diagnostics>"#,
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn patterns_no_phase_activates_are_reported() {
        let found = kinds(
            r#"<phase id="quick"><active pattern="one"/></phase>
               <pattern id="one"><rule context="a"><assert test="b">m</assert></rule></pattern>
               <pattern id="two"><rule context="c"><assert test="d">m</assert></rule></pattern>"#,
        );
        assert_eq!(found, vec![LintKind::PatternInNoPhase]);
    }

    #[test]
    fn phases_are_only_considered_when_the_schema_declares_them() {
        let found = kinds(
            r#"<pattern id="one"><rule context="a"><assert test="b">m</assert></rule></pattern>"#,
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn every_lint_carries_a_location_and_help() {
        let lints = lints_of(
            r#"<pattern>
                 <rule context="*"><assert test="true()">   </assert></rule>
                 <rule context="a"><assert test="b">m</assert></rule>
               </pattern>"#,
        );
        assert!(!lints.is_empty());
        for lint in &lints {
            assert!(!lint.location.is_empty(), "{lint:?}");
            assert!(lint.help.is_some(), "{lint:?}");
            assert!(!lint.kind.as_str().is_empty());
        }
    }
}
