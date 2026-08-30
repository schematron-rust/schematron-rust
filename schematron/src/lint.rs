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
//! `spec/linting/`.
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
use crate::xpath::{Axis, Expr, NodeTest, PathStart};

/// What kind of problem a [`Lint`] reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[non_exhaustive]
pub enum LintKind {
    /// A `let` that redeclares a name from an enclosing scope. **Portability
    /// only** — see [`Schema::portability`].
    VariableShadowsAnOuterScope,
    /// The `following` axis taken from an attribute node. **Portability
    /// only.**
    FollowingFromAnAttribute,
    /// Rules on `@x` and `@p:x` in one pattern. **Portability only.**
    CollidingAttributeContexts,
    /// Whitespace between two inline elements in a message. **Portability
    /// only.**
    SpaceBetweenInlineElements,
    /// A rule context selecting text, comment or processing-instruction
    /// nodes. **Portability only.**
    ContextSelectsANonElementKind,
    /// `@flag` or `@role` on a rule, inherited by its assertions.
    /// **Portability only.**
    FlagOrRoleOnTheRule,
    /// `@subject`, which moves the reported location. **Portability only.**
    SubjectMovesTheLocation,
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
    /// A `key` that no expression looks up.
    UnreferencedKey,
    /// A `let` whose name no expression mentions.
    UnreferencedVariable,
    /// A rule that matches nodes and reports nothing.
    RuleWithNoAssertions,
    /// A pattern with no rules, which cannot do anything.
    PatternWithNoRules,
    /// Two assertions in one rule testing the same thing.
    DuplicateAssertionTest,
    /// A phase that activates no pattern.
    PhaseWithNoPatterns,
}

impl LintKind {
    /// A short stable identifier, for filtering and for machine consumers.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            LintKind::VariableShadowsAnOuterScope => "variable-shadows-outer-scope",
            LintKind::FollowingFromAnAttribute => "following-from-an-attribute",
            LintKind::CollidingAttributeContexts => "colliding-attribute-contexts",
            LintKind::SpaceBetweenInlineElements => "space-between-inline-elements",
            LintKind::ContextSelectsANonElementKind => "context-selects-non-element-kind",
            LintKind::FlagOrRoleOnTheRule => "flag-or-role-on-the-rule",
            LintKind::SubjectMovesTheLocation => "subject-moves-the-location",
            LintKind::UnreachableRule => "unreachable-rule",
            LintKind::DuplicateRuleContext => "duplicate-rule-context",
            LintKind::UnprefixedNameInNamespacedSchema => "unprefixed-name",
            LintKind::UnreferencedDiagnostic => "unreferenced-diagnostic",
            LintKind::UnreferencedProperty => "unreferenced-property",
            LintKind::EmptyMessage => "empty-message",
            LintKind::ConstantTest => "constant-test",
            LintKind::PatternInNoPhase => "pattern-in-no-phase",
            LintKind::UnreferencedKey => "unreferenced-key",
            LintKind::UnreferencedVariable => "unreferenced-variable",
            LintKind::RuleWithNoAssertions => "rule-with-no-assertions",
            LintKind::PatternWithNoRules => "pattern-with-no-rules",
            LintKind::DuplicateAssertionTest => "duplicate-assertion-test",
            LintKind::PhaseWithNoPatterns => "phase-with-no-patterns",
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
    /// Constructs that behave differently under other Schematron processors.
    ///
    /// These are **not mistakes**. A schema that uses `@subject` is correct,
    /// and this crate implements it as the standard describes. But the ISO
    /// reference implementation — the XSLT skeleton most other tools are
    /// built on — behaves differently for each of the constructs reported
    /// here, and a schema author has no way to discover that. Every one is
    /// backed by a divergence recorded in `spec/conformance/`, established
    /// by running both implementations.
    ///
    /// They are deliberately kept out of [`Schema::lint`]. A linter that
    /// reports correct code as a problem gets switched off, and then it
    /// catches nothing; portability is a separate question, asked separately.
    ///
    /// # Examples
    ///
    /// ```
    /// use schematron::Schema;
    ///
    /// let schema = Schema::from_str(
    ///     r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
    ///          <pattern>
    ///            <rule context="a" flag="warning">
    ///              <assert test="b">needs a b</assert>
    ///            </rule>
    ///          </pattern>
    ///        </schema>"#,
    /// )
    /// .unwrap();
    ///
    /// // Correct, and portable nowhere: the reference drops a rule's flag.
    /// assert!(schema.lint().is_empty());
    /// assert_eq!(schema.portability().len(), 1);
    /// ```
    #[must_use]
    pub fn portability(&self) -> Vec<Lint> {
        let mut lints = Vec::new();
        let model = self.model();

        // A `let` that redeclares an enclosing name. The reference compiles
        // every binding into one XSLT scope, so this is not a divergence in
        // behaviour but a schema it cannot compile at all.
        let mut outer: Vec<(&str, &str)> = model
            .lets
            .iter()
            .map(|binding| (binding.name.as_str(), "schema"))
            .collect();
        for phase in &model.phases {
            let at = format!("phase[@id='{}']", phase.id);
            for binding in &phase.lets {
                shadowing(&outer, binding, &at, &mut lints);
            }
        }
        for (index, pattern) in model.patterns.iter().enumerate() {
            let location = pattern_location(pattern, index);
            let mut here = outer.clone();
            for binding in &pattern.lets {
                shadowing(&here, binding, &location, &mut lints);
                here.push((binding.name.as_str(), "pattern"));
            }
            // Rules on `@x` and `@p:x` in one pattern. The reference's `@x`
            // rule claims both, so the first one written takes every node and
            // the second never fires.
            let attributes: Vec<(&str, Option<&str>, &str)> = pattern
                .rules
                .iter()
                .filter_map(|rule| rule.context.as_deref())
                .filter_map(|context| {
                    let Ok(Expr::Path(path)) = self.expression(context) else {
                        return None;
                    };
                    let step = path.steps.last()?;
                    if step.axis != Axis::Attribute {
                        return None;
                    }
                    match &step.node_test {
                        NodeTest::Name(name) => {
                            Some((name.local.as_str(), name.prefix.as_deref(), context))
                        }
                        _ => None,
                    }
                })
                .collect();
            for (index, (local, prefix, context)) in attributes.iter().enumerate() {
                let clash = attributes[..index]
                    .iter()
                    .find(|(other, other_prefix, _)| other == local && other_prefix != prefix);
                if let Some((_, _, earlier)) = clash {
                    lints.push(Lint::new(
                        LintKind::CollidingAttributeContexts,
                        format!("{location}/rule[@context='{context}']"),
                        format!(
                            "this pattern has rules on both {earlier} and {context}"
                        ),
                        "an unprefixed attribute name test matches only the \
                         no-namespace attribute here, correctly; the reference's \
                         template matcher ignores the namespace, so its earlier rule \
                         claims both and this one never fires. See \
                         spec/conformance/.",
                    ));
                }
            }

            for rule in &pattern.rules {
                let context = rule.context.as_deref().unwrap_or_default();
                let at = format!("{location}/rule[@context='{context}']");
                for binding in &rule.lets {
                    shadowing(&here, binding, &at, &mut lints);
                }
                self.portability_of_rule(rule, &location, &mut lints);
            }
        }
        outer.clear();
        lints
    }

    /// The rule-level portability checks.
    fn portability_of_rule(&self, rule: &crate::schema::Rule, location: &str, lints: &mut Vec<Lint>) {
        let context = rule.context.as_deref().unwrap_or_default();
        let at = format!("{location}/rule[@context='{context}']");

        // A context selecting text, comment or processing-instruction nodes.
        // The reference walks the tree with `@*|*`, so it never offers such a
        // node to a rule and the rule silently never fires.
        if let Ok(Expr::Path(path)) = self.expression(context) {
            if let Some(step) = path.steps.last() {
                let kind = match step.node_test {
                    NodeTest::Text => Some("text()"),
                    NodeTest::Comment => Some("comment()"),
                    NodeTest::ProcessingInstruction(_) => Some("processing-instruction()"),
                    _ => None,
                };
                if let Some(kind) = kind {
                    lints.push(Lint::new(
                        LintKind::ContextSelectsANonElementKind,
                        at.clone(),
                        format!("the context selects {kind} nodes"),
                        "the ISO reference implementation visits only elements \
                         and attributes, so this rule never fires there. It \
                         works here. See spec/conformance/.",
                    ));
                }
            }
        }

        // `@flag` or `@role` on the rule, inherited by an assertion that sets
        // neither. The reference emits neither on the finding.
        let inherits = rule
            .assertions()
            .any(|assertion| assertion.flag.is_none() && assertion.role.is_none());
        if inherits && (rule.flag.is_some() || rule.role.is_some()) {
            lints.push(Lint::new(
                LintKind::FlagOrRoleOnTheRule,
                at.clone(),
                "the rule sets @flag or @role, and an assertion below it sets neither"
                    .to_string(),
                "findings inherit the rule's value here; the ISO reference \
                 implementation leaves them off, so anything filtering on the \
                 flag sees different results. Set it on the assertion to be \
                 portable.",
            ));
        }

        // `following::` taken from an attribute node. The reference gives the
        // attribute's *element's* following nodes, excluding its children.
        let mut sources: Vec<(&str, String)> = vec![(context, at.clone())];
        for binding in &rule.lets {
            if let crate::schema::LetValue::Expression(value) = &binding.value {
                sources.push((value.as_str(), format!("{at}/let[@name='{}']", binding.name)));
            }
        }
        for assertion in rule.assertions() {
            sources.push((assertion.test.as_str(), at.clone()));
        }
        for (source, where_) in sources {
            if source.is_empty() {
                continue;
            }
            if let Ok(expr) = self.expression(source) {
                if following_from_an_attribute(expr) {
                    lints.push(Lint::new(
                        LintKind::FollowingFromAnAttribute,
                        where_,
                        format!("{source} takes the following axis from an attribute"),
                        "an attribute's following nodes include its own element's \
                         children here, which is what XPath 1.0 says and what Java's \
                         engine gives; the ISO reference implementation excludes them. \
                         See spec/conformance/.",
                    ));
                    break;
                }
            }
        }

        // Whitespace between two inline elements in a message, which the
        // reference cannot preserve.
        for assertion in rule.assertions() {
            if space_between_inline_elements(&assertion.content) {
                lints.push(Lint::new(
                    LintKind::SpaceBetweenInlineElements,
                    at.clone(),
                    "the message has whitespace between two inline elements".to_string(),
                    "the validator the reference generates is itself an XSLT \
                     stylesheet, and XSLT strips whitespace-only text from a \
                     stylesheet, so the space is lost there. Put a word between them, \
                     or accept the difference.",
                ));
                break;
            }
        }

        // `@subject`, which moves the reported location.
        let subject = rule.subject.is_some()
            || rule.assertions().any(|assertion| assertion.subject.is_some());
        if subject {
            lints.push(Lint::new(
                LintKind::SubjectMovesTheLocation,
                at,
                "@subject moves the reported location to the node it selects".to_string(),
                "the ISO reference implementation reports the context node \
                 instead, although its own source says the subject should be \
                 used. Consumers reading @location will disagree.",
            ));
        }
    }

    /// Inspects this schema for constructs that are legal but probably wrong.
    ///
    /// Returns lints in schema order, so the output reads down the file. An
    /// empty result does not mean the schema is correct — only that none of
    /// the patterns in `spec/linting/` matched.
    ///
    /// For constructs that are correct here but behave differently under
    /// other processors, see [`Schema::portability`], which is deliberately
    /// separate.
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

        lint_phases(self, &mut lints);
        lint_variables(self, &mut lints);
        lint_unreferenced(self, &mut lints);
        lints
    }

    fn lint_pattern(&self, pattern: &Pattern, location: &str, lints: &mut Vec<Lint>) {
        if pattern.rules.is_empty() {
            lints.push(Lint::new(
                LintKind::PatternWithNoRules,
                location,
                "this pattern has no rules, so it cannot do anything",
                "add a <rule>, or delete the pattern.",
            ));
        }

        // A rule is unreachable when an earlier rule in the same pattern
        // matches everything it would. Comparison is pairwise, because the
        // relation is between two contexts rather than a property of one.
        for (index, rule) in pattern.rules.iter().enumerate() {
            let Some(context) = rule.context.as_deref() else {
                continue;
            };
            let rule_location = format!("{location}/{}", rule_step(index));

            let duplicate = pattern.rules[..index]
                .iter()
                .filter_map(|earlier| earlier.context.as_deref())
                .find(|&earlier| earlier == context);

            // The exact-duplicate case is checked first: it is a special case
            // of subsumption, and its message is the more specific one.
            let shadow = if duplicate.is_some() {
                None
            } else {
                pattern.rules[..index].iter().find_map(|earlier| {
                    let earlier_context = earlier.context.as_deref()?;
                    self.subsumes(earlier_context, context)
                        .then_some(earlier_context)
                })
            };

            if let Some(earlier) = shadow {
                lints.push(Lint::new(
                    LintKind::UnreachableRule,
                    &rule_location,
                    format!(
                        "the rule with context {context:?} can never fire: an earlier \
                         rule in the same pattern, context={earlier:?}, already matches \
                         every node this one would"
                    ),
                    "within one pattern a node is processed by the first matching rule \
                     only. Move this rule into a pattern of its own, or put it before \
                     the broader rule.",
                ));
            } else if let Some(earlier) = duplicate {
                lints.push(Lint::new(
                    LintKind::DuplicateRuleContext,
                    &rule_location,
                    format!(
                        "the rule with context {context:?} can never fire: an earlier \
                         rule in the same pattern has the same context {earlier:?}"
                    ),
                    "merge the two rules, or move this one into a pattern of its own.",
                ));
            }

            self.lint_rule(rule, &rule_location, lints);
        }
    }

    fn lint_rule(&self, rule: &Rule, location: &str, lints: &mut Vec<Lint>) {
        if let Some(context) = rule.context.as_deref() {
            self.lint_unprefixed(context, location, "@context", lints);
        }

        if rule.assertions().next().is_none() {
            lints.push(Lint::new(
                LintKind::RuleWithNoAssertions,
                location,
                "this rule matches nodes and reports nothing",
                "it still claims those nodes, so a later rule in the same pattern \
                 will not see them. Add an assert or report, or delete the rule.",
            ));
        }

        // Two assertions testing the same thing: whichever fires, the reader
        // cannot tell which was meant.
        let mut seen: Vec<&str> = Vec::new();
        for assertion in rule.assertions() {
            if seen.contains(&assertion.test.as_str()) {
                lints.push(Lint::new(
                    LintKind::DuplicateAssertionTest,
                    location,
                    format!(
                        "two assertions in this rule test {:?}",
                        assertion.test
                    ),
                    "merge them, or check whether one was meant to test something \
                     else.",
                ));
            }
            seen.push(&assertion.test);
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

/// Whether an expression calls `key()` with this name as a literal.
///
/// A computed name cannot be traced, so a schema that builds one is
/// conservatively treated as looking up every key.
fn looks_up_key(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Function { name: called, args } if called == "key" => {
            match args.first() {
                Some(Expr::Literal(literal)) => literal == name,
                // A computed name might be any key, so assume it is this one.
                Some(_) => true,
                None => false,
            }
        }
        Expr::Function { args, .. } => args.iter().any(|arg| looks_up_key(arg, name)),
        Expr::Literal(_) | Expr::Number(_, _) | Expr::Variable(_) => false,
        Expr::Negate(inner) => looks_up_key(inner, name),
        Expr::Binary(_, left, right) => {
            looks_up_key(left, name) || looks_up_key(right, name)
        }
        Expr::TypeOp { value, .. } => looks_up_key(value, name),
        Expr::Sequence(members) => members.iter().any(|m| looks_up_key(m, name)),
        Expr::Range(from, to) => looks_up_key(from, name) || looks_up_key(to, name),
        Expr::For { input, body, .. } => {
            looks_up_key(input, name) || looks_up_key(body, name)
        }
        Expr::Quantified { input, test, .. } => {
            looks_up_key(input, name) || looks_up_key(test, name)
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            looks_up_key(condition, name)
                || looks_up_key(then_branch, name)
                || looks_up_key(else_branch, name)
        }
        Expr::Path(path) => {
            let start = match &path.start {
                PathStart::Expr(expr, predicates) => {
                    looks_up_key(expr, name)
                        || predicates.iter().any(|p| looks_up_key(p, name))
                }
                PathStart::Root | PathStart::Context => false,
            };
            start
                || path
                    .steps
                    .iter()
                    .any(|step| step.predicates.iter().any(|p| looks_up_key(p, name)))
        }
    }
}

impl Schema {
    /// Whether every node matching `narrow` also matches `broad`.
    ///
    /// Deciding this in general is not practical, so the test is deliberately
    /// one-directional and conservative: `broad` must carry no predicates,
    /// and its steps must generalise a **suffix** of `narrow`'s. That covers
    /// the shapes the mistake actually takes —
    ///
    /// | `broad` | `narrow` | why |
    /// |---|---|---|
    /// | `*` | `a` | a wildcard generalises a name |
    /// | `node()` | `text()` | a node test generalises a kind |
    /// | `a` | `a[@x]` | a predicate only narrows |
    /// | `b` | `a/b` | a longer path only narrows |
    ///
    /// — and reports nothing it is not certain of, because a linter with
    /// false positives gets switched off, after which it catches nothing.
    fn subsumes(&self, broad: &str, narrow: &str) -> bool {
        let (Ok(Expr::Path(broad)), Ok(Expr::Path(narrow))) =
            (self.expression(broad), self.expression(narrow))
        else {
            return false;
        };

        // Only relative paths are compared. An absolute context is anchored
        // and the reasoning below does not hold for it.
        if broad.start != PathStart::Context || narrow.start != PathStart::Context {
            return false;
        }
        // Any predicate on the broader rule could exclude a node, so it is no
        // longer certainly broader.
        if broad.steps.iter().any(|step| !step.predicates.is_empty()) {
            return false;
        }
        if broad.steps.is_empty() || broad.steps.len() > narrow.steps.len() {
            return false;
        }

        // Align at the end: `b` generalises the `b` of `a/b`.
        let offset = narrow.steps.len() - broad.steps.len();
        broad
            .steps
            .iter()
            .zip(&narrow.steps[offset..])
            .all(|(broad, narrow)| {
                broad.axis == narrow.axis
                    && node_test_subsumes(&broad.node_test, &narrow.node_test)
            })
    }
}

/// Whether every node passing `narrow` also passes `broad`, on one axis.
fn node_test_subsumes(broad: &NodeTest, narrow: &NodeTest) -> bool {
    match (broad, narrow) {
        // `node()` admits everything the axis yields. `*` admits every node
        // of the axis's principal type, which is what a name test and a
        // namespace wildcard both select — so on one axis the two cases
        // coincide.
        (NodeTest::AnyNode, _)
        | (
            NodeTest::Wildcard,
            NodeTest::Wildcard | NodeTest::Name(_) | NodeTest::NamespaceWildcard(_),
        ) => true,
        // Anything else only subsumes itself.
        (a, b) => a == b,
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
        Expr::Number(_, _) | Expr::Literal(_) => Some("has a fixed outcome"),
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
        Expr::Literal(_) | Expr::Number(_, _) | Expr::Variable(_) => None,
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

/// Reports a phase that activates nothing.
fn lint_phases(schema: &Schema, lints: &mut Vec<Lint>) {
    for phase in &schema.model().phases {
        if phase.actives.is_empty() {
            lints.push(Lint::new(
                LintKind::PhaseWithNoPatterns,
                format!("phase[@id='{}']", phase.id),
                "this phase activates no pattern, so selecting it validates nothing",
                "add an <active pattern=\"…\"/>, or delete the phase.",
            ));
        }
    }
}

/// Reports a `let` whose name no expression anywhere mentions.
///
/// Deliberately conservative: it asks whether the name appears at all, not
/// whether it is in scope where it is used. See `spec/linting/`.
/// Whether an expression takes the `following` axis from an attribute node.
fn following_from_an_attribute(expr: &Expr) -> bool {
    fn in_path(path: &crate::xpath::PathExpr) -> bool {
        path.steps.windows(2).any(|pair| {
            pair[0].axis == Axis::Attribute && pair[1].axis == Axis::Following
        })
    }
    match expr {
        Expr::Path(path) => in_path(path),
        Expr::Binary(_, left, right) => {
            following_from_an_attribute(left) || following_from_an_attribute(right)
        }
        Expr::Negate(inner) => following_from_an_attribute(inner),
        Expr::Function { args, .. } => args.iter().any(following_from_an_attribute),
        Expr::Sequence(items) => items.iter().any(following_from_an_attribute),
        _ => false,
    }
}

/// Whether a message holds whitespace-only text between two elements.
///
/// Only *between*: leading and trailing whitespace belongs to a text node
/// that has other content, and every implementation keeps that.
fn space_between_inline_elements(content: &[Content]) -> bool {
    let inline = |item: &Content| !matches!(item, Content::Text(_));
    content.windows(3).any(|window| {
        inline(&window[0])
            && matches!(&window[1], Content::Text(text) if !text.is_empty() && text.trim().is_empty())
            && inline(&window[2])
    })
}

/// Reports a binding that redeclares a name from an enclosing scope.
///
/// Nested scopes only. Two sibling rules each binding `$qty` is ordinary and
/// portable: the reference compiles each rule into its own template, so their
/// variables never meet.
fn shadowing(
    outer: &[(&str, &str)],
    binding: &crate::schema::Let,
    location: &str,
    lints: &mut Vec<Lint>,
) {
    let Some((_, where_)) = outer.iter().find(|(name, _)| *name == binding.name) else {
        return;
    };
    lints.push(Lint::new(
        LintKind::VariableShadowsAnOuterScope,
        format!("{location}/let[@name='{}']", binding.name),
        format!(
            "${} is already bound at {where_} level",
            binding.name
        ),
        "the four nested scopes are what the standard describes, and this \
         crate implements them — but the ISO reference implementation compiles \
         every binding into one XSLT scope and refuses the schema outright. \
         Rename one of them to be portable.",
    ));
}

fn lint_variables(schema: &Schema, lints: &mut Vec<Lint>) {
    let model = schema.model();

    let referenced = |name: &str| {
        schema
            .expressions
            .values()
            .any(|expr| references_variable(expr, name))
    };

    let mut report = |name: &str, location: String| {
        if referenced(name) {
            return;
        }
        lints.push(Lint::new(
            LintKind::UnreferencedVariable,
            location,
            format!("no expression mentions ${name}"),
            "its value is computed anyway — once per node, for a rule-level \
             binding. Reference it, or delete it.",
        ));
    };

    for binding in &model.lets {
        report(&binding.name, format!("let[@name='{}']", binding.name));
    }
    for phase in &model.phases {
        for binding in &phase.lets {
            report(
                &binding.name,
                format!("phase[@id='{}']/let[@name='{}']", phase.id, binding.name),
            );
        }
    }
    for (index, pattern) in model.patterns.iter().enumerate() {
        let where_ = pattern_location(pattern, index);
        for binding in &pattern.lets {
            report(&binding.name, format!("{where_}/let[@name='{}']", binding.name));
        }
        for (rule_index, rule) in pattern.rules.iter().enumerate() {
            for binding in &rule.lets {
                report(
                    &binding.name,
                    format!(
                        "{where_}/{}/let[@name='{}']",
                        rule_step(rule_index),
                        binding.name
                    ),
                );
            }
        }
    }
}

/// Whether an expression mentions `$name` anywhere inside it.
fn references_variable(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Variable(variable) => variable.to_string() == name,
        Expr::Literal(_) | Expr::Number(_, _) => false,
        Expr::Negate(inner) => references_variable(inner, name),
        Expr::Binary(_, left, right) => {
            references_variable(left, name) || references_variable(right, name)
        }
        Expr::Function { args, .. } => args.iter().any(|a| references_variable(a, name)),
        Expr::TypeOp { value, .. } => references_variable(value, name),
        Expr::Sequence(members) => members.iter().any(|m| references_variable(m, name)),
        Expr::Range(from, to) => {
            references_variable(from, name) || references_variable(to, name)
        }
        Expr::For { input, body, .. } => {
            references_variable(input, name) || references_variable(body, name)
        }
        Expr::Quantified { input, test, .. } => {
            references_variable(input, name) || references_variable(test, name)
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            references_variable(condition, name)
                || references_variable(then_branch, name)
                || references_variable(else_branch, name)
        }
        Expr::Path(path) => {
            let start = match &path.start {
                PathStart::Expr(expr, predicates) => {
                    references_variable(expr, name)
                        || predicates.iter().any(|p| references_variable(p, name))
                }
                PathStart::Root | PathStart::Context => false,
            };
            start
                || path
                    .steps
                    .iter()
                    .any(|step| step.predicates.iter().any(|p| references_variable(p, name)))
        }
    }
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

    // A key's index is built eagerly, so one nothing looks up is work done
    // for nothing on every validation. See spec/keys/.
    for key in &model.keys {
        let looked_up = schema
            .expressions
            .values()
            .any(|expr| looks_up_key(expr, &key.name));
        if !looked_up {
            lints.push(Lint::new(
                LintKind::UnreferencedKey,
                format!("key[@name='{}']", key.name),
                "no expression looks this key up",
                "its index is built on every validation whether or not anything \
                 uses it. Reference it with key('…', …), or delete it.",
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
    fn a_node_test_claims_every_kind_of_node() {
        let found = kinds(
            r#"<pattern>
                 <rule context="node()"><assert test="@id">m</assert></rule>
                 <rule context="a"><assert test="b">m</assert></rule>
               </pattern>"#,
        );
        assert!(found.contains(&LintKind::UnreachableRule), "{found:?}");
    }

    #[test]
    fn an_attribute_wildcard_does_not_shadow_an_element_rule() {
        // `@*` claims attributes; an element rule after it still fires, as
        // running the validator confirms. Reporting it was a false positive,
        // which is how a linter loses its reader.
        let found = kinds(
            r#"<pattern>
                 <rule context="@*"><assert test="true()">m</assert></rule>
                 <rule context="a"><assert test="b">m</assert></rule>
               </pattern>"#,
        );
        assert!(!found.contains(&LintKind::UnreachableRule), "{found:?}");
    }

    #[test]
    fn an_attribute_wildcard_does_shadow_a_named_attribute_rule() {
        let found = kinds(
            r#"<pattern>
                 <rule context="@*"><assert test="true()">m</assert></rule>
                 <rule context="@x"><assert test="true()">m</assert></rule>
               </pattern>"#,
        );
        assert!(found.contains(&LintKind::UnreachableRule), "{found:?}");
    }

    #[test]
    fn a_predicate_only_narrows_so_the_bare_context_shadows_it() {
        let found = kinds(
            r#"<pattern>
                 <rule context="a"><assert test="b">m</assert></rule>
                 <rule context="a[@x]"><assert test="c">m</assert></rule>
               </pattern>"#,
        );
        assert!(found.contains(&LintKind::UnreachableRule), "{found:?}");
    }

    #[test]
    fn a_longer_path_only_narrows_so_the_shorter_one_shadows_it() {
        let found = kinds(
            r#"<pattern>
                 <rule context="b"><assert test="@id">m</assert></rule>
                 <rule context="a/b"><assert test="@id">m</assert></rule>
               </pattern>"#,
        );
        assert!(found.contains(&LintKind::UnreachableRule), "{found:?}");
    }

    #[test]
    fn the_narrower_rule_placed_first_is_the_idiomatic_ordering() {
        // The else-branch idiom must never be reported.
        for body in [
            r#"<pattern>
                 <rule context="a[@x]"><assert test="c">m</assert></rule>
                 <rule context="a"><assert test="b">m</assert></rule>
               </pattern>"#,
            r#"<pattern>
                 <rule context="a/b"><assert test="@id">m</assert></rule>
                 <rule context="b"><assert test="@id">m</assert></rule>
               </pattern>"#,
        ] {
            let found = kinds(body);
            assert!(!found.contains(&LintKind::UnreachableRule), "{body} -> {found:?}");
        }
    }

    #[test]
    fn unrelated_contexts_do_not_shadow_each_other() {
        let found = kinds(
            r#"<pattern>
                 <rule context="a"><assert test="@id">m</assert></rule>
                 <rule context="b"><assert test="@id">m</assert></rule>
                 <rule context="c/d"><assert test="@id">m</assert></rule>
               </pattern>"#,
        );
        assert!(!found.contains(&LintKind::UnreachableRule), "{found:?}");
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
    fn a_rule_with_no_assertions_is_reported() {
        // It still claims its nodes, so a later rule in the pattern will not
        // see them — which makes an empty rule worse than useless.
        let found = kinds(r#"<pattern><rule context="a"/></pattern>"#);
        assert_eq!(found, vec![LintKind::RuleWithNoAssertions]);
    }

    #[test]
    fn a_pattern_with_no_rules_is_reported() {
        let found = kinds(r#"<pattern id="empty"/>"#);
        assert_eq!(found, vec![LintKind::PatternWithNoRules]);
    }

    #[test]
    fn two_assertions_testing_the_same_thing_are_reported() {
        let found = kinds(
            r#"<pattern><rule context="a">
                 <assert test="b">one</assert>
                 <assert test="b">two</assert>
               </rule></pattern>"#,
        );
        assert_eq!(found, vec![LintKind::DuplicateAssertionTest]);
    }

    #[test]
    fn different_tests_in_one_rule_are_not_reported() {
        let found = kinds(
            r#"<pattern><rule context="a">
                 <assert test="b">one</assert>
                 <assert test="c">two</assert>
               </rule></pattern>"#,
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn the_same_test_in_different_rules_is_not_reported() {
        // Two rules may legitimately check the same thing about different
        // contexts.
        let found = kinds(
            r#"<pattern>
                 <rule context="a"><assert test="@id">m</assert></rule>
                 <rule context="b"><assert test="@id">m</assert></rule>
               </pattern>"#,
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_phase_that_activates_nothing_is_reported() {
        let found = kinds(
            r#"<phase id="empty"/>
               <pattern id="p"><rule context="a"><assert test="b">m</assert></rule></pattern>"#,
        );
        // The pattern is in no phase either, which is its own lint.
        assert!(found.contains(&LintKind::PhaseWithNoPatterns), "{found:?}");
    }

    #[test]
    fn a_variable_nothing_mentions_is_reported() {
        let found = kinds(
            r#"<let name="unused" value="1"/>
               <pattern><rule context="a"><assert test="b">m</assert></rule></pattern>"#,
        );
        assert_eq!(found, vec![LintKind::UnreferencedVariable]);
    }

    #[test]
    fn a_variable_that_is_used_is_not_reported() {
        for body in [
            // Schema scope, used in a test.
            r#"<let name="v" value="1"/>
               <pattern><rule context="a"><assert test="$v">m</assert></rule></pattern>"#,
            // Rule scope, used in a message.
            r#"<pattern><rule context="a">
                 <let name="v" value="1"/>
                 <assert test="b"><value-of select="$v"/></assert>
               </rule></pattern>"#,
            // Used only inside a predicate.
            r#"<let name="v" value="1"/>
               <pattern><rule context="a"><assert test="b[c = $v]">m</assert></rule></pattern>"#,
            // Used only by another `let`.
            r#"<let name="v" value="1"/>
               <let name="w" value="$v + 1"/>
               <pattern><rule context="a"><assert test="$w">m</assert></rule></pattern>"#,
        ] {
            let found = kinds(body);
            assert!(
                !found.contains(&LintKind::UnreferencedVariable),
                "{body} -> {found:?}"
            );
        }
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
