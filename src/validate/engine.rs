//! The validation algorithm.
//!
//! Implements `spec/validation.md`: active patterns, four scopes of variable,
//! and — the piece that defines the language — first-matching-rule-wins
//! within each pattern.

use std::collections::HashMap;

use super::options::{PhaseSelection, ValidateOptions};
use super::report::{
    ActivePattern, AssertionResult, DiagnosticResult, FiredRule, PropertyResult, Report, ResultKind,
};
use crate::error::{Error, Result};
use crate::schema::model::{Content, Let, LetValue, Pattern, Rule, SchemaModel};
use crate::schema::Schema;
use crate::xml::{Document, NodeId};
use crate::xpath::{
    evaluate, Axis, BinaryOp, Documents, EvalContext, Expr, NodeTest, PathExpr, PathStart, Step,
    Value, Variables,
};

/// How many times validation may re-run to load documents `document()` asked
/// for.
///
/// One pass discovers the URIs and the next has them, so two suffice unless a
/// loaded document itself names another. The cap turns a pathological chain —
/// or a schema that derives a new URI from each document it loads — into an
/// error rather than an unbounded loop.
const MAX_DOCUMENT_PASSES: usize = 8;

/// Everything that stays fixed for the length of one validation pass.
///
/// Bundled because these four travel together through every step of the
/// algorithm, and threading them individually turned each helper's signature
/// into a wall of parameters that obscured the one argument that varies.
#[derive(Clone, Copy)]
struct Run<'a> {
    schema: &'a Schema,
    /// The instant the clock functions report, read once for the whole run.
    current_time: f64,
    /// The tree being validated. This is the target document for a pattern
    /// with `@documents`, not necessarily the instance.
    document: &'a Document,
    documents: &'a Documents,
    options: &'a ValidateOptions,
}

impl<'a> Run<'a> {
    /// The same run, pointed at a different tree.
    fn on(self, document: &'a Document) -> Self {
        Self { document, ..self }
    }

    /// An evaluation context for `node`, wired to this run's registry.
    fn context(self, node: NodeId, variables: &'a Variables) -> EvalContext<'a> {
        EvalContext::new(self.document, node, variables, self.schema.namespaces())
            .with_documents(self.documents)
            .with_version(self.schema.version())
            .with_current_time(self.current_time)
    }
}

/// Validates `document` against `schema`.
///
/// A schema that does not call XPath `document()` — nearly all of them — is
/// validated against the caller's tree directly, and pays nothing for the
/// machinery below.
pub(crate) fn validate(
    schema: &Schema,
    document: &Document,
    options: &ValidateOptions,
) -> Result<Report> {
    if !schema.uses_document_function() {
        return validate_once(schema, document, &Documents::new(), options);
    }
    validate_loading_documents(schema, document, options)
}

/// Validation for a schema that calls `document()`.
///
/// Evaluation holds the tree immutably, so a `document()` call cannot load
/// anything on the spot; it records a miss instead. This runs validation,
/// merges whatever was asked for into a working copy of the tree, and runs
/// again — discarding the earlier report, which was computed against an
/// incomplete document set.
///
/// The repeated work is bounded and only paid by schemas that use the
/// feature. See `spec/xpath.md`.
fn validate_loading_documents(
    schema: &Schema,
    document: &Document,
    options: &ValidateOptions,
) -> Result<Report> {
    // A working copy, because merging documents in mutates the arena and the
    // caller's document must not change under them.
    let mut working = document.clone();
    let mut documents = Documents::new();

    for _ in 0..MAX_DOCUMENT_PASSES {
        documents.clear_missing();
        let report = validate_once(schema, &working, &documents, options)?;

        let wanted = documents.missing();
        if wanted.is_empty() {
            return Ok(report);
        }

        for uri in wanted {
            let text = schema.resolver().resolve(&uri, document.base_uri())?;
            let mut loaded = Document::from_str(&text)?;
            loaded.set_base_uri(uri.clone());
            let root = working.append_document(&loaded);
            documents.insert(uri, root);
        }
    }

    Err(Error::xpath_eval(
        "document()",
        format!(
            "still discovering new documents after {MAX_DOCUMENT_PASSES} passes; \
             a schema whose document() URIs depend on documents it has just \
             loaded cannot be resolved"
        ),
    ))
}

/// One validation pass, against a fixed set of loaded documents.
fn validate_once(
    schema: &Schema,
    document: &Document,
    documents: &Documents,
    options: &ValidateOptions,
) -> Result<Report> {
    let run = Run {
        schema,
        current_time: options.resolve_current_time(),
        document,
        documents,
        options,
    };
    let model = schema.model();
    let phase = resolve_phase(model, &options.phase)?;
    let active = active_patterns(model, phase.as_deref());

    let mut report = Report {
        title: model.title.clone(),
        phase: phase.clone(),
        schema_version: model.schema_version.clone(),
        namespaces: model.namespaces.clone(),
        patterns: Vec::new(),
    };

    let mut variables = Variables::new();

    // Schema-scoped and phase-scoped variables are evaluated against the root
    // of the instance document, and stay bound for the whole run.
    bind_all(run, document.root(), &model.lets, &mut variables, "schema/let")?;
    if let Some(phase_id) = &phase {
        if let Some(phase) = model.phase(phase_id) {
            bind_all(run, document.root(), &phase.lets, &mut variables, "phase/let")?;
        }
    }

    report.patterns = if options.is_parallel() && active.len() > 1 {
        run_patterns_in_parallel(run, &active, &variables)?
    } else {
        run_patterns_sequentially(run, &active, &mut variables)?
    };
    Ok(report)
}

/// Runs the active patterns one after another, honouring `max_failures`.
fn run_patterns_sequentially(
    run: Run<'_>,
    active: &[&Pattern],
    variables: &mut Variables,
) -> Result<Vec<ActivePattern>> {
    let mut out = Vec::new();
    let mut failures = 0;
    for pattern in active {
        for (label, target) in pattern_targets(run, pattern)? {
            let outcome = run_pattern(
                run.on(target.as_ref().unwrap_or(run.document)),
                pattern,
                label,
                variables,
                &mut failures,
            )?;
            out.push(outcome);
            if run.options.max_failures.is_some_and(|limit| failures >= limit) {
                return Ok(out);
            }
        }
    }
    Ok(out)
}

/// Runs the active patterns on worker threads, then restores schema order.
///
/// Patterns are independent — no pattern can observe another's results — so
/// this changes nothing about the report. Order is *restored* rather than
/// preserved: each pattern's output goes into its own slot, and the slots are
/// concatenated in schema order however the threads happened to finish.
///
/// Each worker starts from a clone of the schema- and phase-level variable
/// scope. Those bindings are already evaluated and read-only by this point,
/// and pattern- and rule-level bindings never escape the pattern that made
/// them, so there is nothing to synchronise.
///
/// `std::thread::scope` borrows the schema and document rather than requiring
/// `'static`, which is why this needs no thread-pool dependency.
fn run_patterns_in_parallel(
    run: Run<'_>,
    active: &[&Pattern],
    variables: &Variables,
) -> Result<Vec<ActivePattern>> {
    // One thread per pattern would be wasteful for a schema with fifty of
    // them, so the patterns are dealt out into at most this many chunks.
    let threads = std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .min(active.len())
        .max(1);
    let chunk_size = active.len().div_ceil(threads);

    let chunks: Vec<(usize, &[&Pattern])> = active
        .chunks(chunk_size)
        .enumerate()
        .map(|(index, chunk)| (index * chunk_size, chunk))
        .collect();

    let results = std::thread::scope(|scope| {
        let handles: Vec<_> = chunks
            .into_iter()
            .map(|(offset, chunk)| {
                scope.spawn(move || {
                    let mut out = Vec::new();
                    for (index, pattern) in chunk.iter().enumerate() {
                        // A fresh scope per pattern, from the shared base.
                        let mut variables = variables.clone();
                        let mut failures = 0;
                        for (label, target) in pattern_targets(run, pattern)? {
                            out.push((
                                offset + index,
                                run_pattern(
                                    run.on(target.as_ref().unwrap_or(run.document)),
                                    pattern,
                                    label,
                                    &mut variables,
                                    &mut failures,
                                )?,
                            ));
                        }
                    }
                    Ok::<Vec<(usize, ActivePattern)>, Error>(out)
                })
            })
            .collect();

        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|_| Err(Error::xpath_eval("validate", "a worker thread panicked")))
            })
            .collect::<Vec<_>>()
    });

    // Reassemble in schema order. A pattern with `@documents` contributes
    // several entries, which a stable sort keeps in their own order.
    let mut ordered: Vec<(usize, ActivePattern)> = Vec::new();
    for result in results {
        ordered.extend(result?);
    }
    ordered.sort_by_key(|(index, _)| *index);
    Ok(ordered.into_iter().map(|(_, pattern)| pattern).collect())
}

/// Which phase to run: the caller's choice, the schema's default, or all.
fn resolve_phase(model: &SchemaModel, selection: &PhaseSelection) -> Result<Option<String>> {
    let wanted = match selection {
        PhaseSelection::All => return Ok(None),
        PhaseSelection::Default => match &model.default_phase {
            None => return Ok(None),
            Some(id) => id.clone(),
        },
        PhaseSelection::Named(name) => match name.as_str() {
            "#ALL" => return Ok(None),
            "#DEFAULT" => match &model.default_phase {
                None => return Ok(None),
                Some(id) => id.clone(),
            },
            other => other.to_string(),
        },
    };

    if model.phase(&wanted).is_none() {
        let available: Vec<&str> = model.phases.iter().map(|p| p.id.as_str()).collect();
        return Err(Error::UnknownPhase {
            phase: wanted,
            available: if available.is_empty() {
                "none; this schema declares no phases".to_string()
            } else {
                available.join(", ")
            },
        });
    }
    Ok(Some(wanted))
}

/// The patterns a phase activates, in schema order.
fn active_patterns<'a>(model: &'a SchemaModel, phase: Option<&str>) -> Vec<&'a Pattern> {
    let Some(phase_id) = phase else {
        return model.patterns.iter().collect();
    };
    let phase = model
        .phase(phase_id)
        .expect("resolve_phase has already checked that the phase exists");
    // Schema order, not phase order, so that report order is stable however
    // the phase happens to list its patterns.
    model
        .patterns
        .iter()
        .filter(|pattern| {
            pattern
                .id
                .as_ref()
                .is_some_and(|id| phase.actives.contains(id))
        })
        .collect()
}

/// The documents a pattern runs against.
///
/// Almost always just the instance document. A `@documents` pattern instead
/// runs against each external document its expression names, which is
/// Schematron's own mechanism for cross-document validation.
fn pattern_targets(
    run: Run<'_>,
    pattern: &Pattern,
) -> Result<Vec<(Option<String>, Option<Document>)>> {
    let Run { schema, document, .. } = run;
    let Some(source) = &pattern.documents else {
        return Ok(vec![(None, None)]);
    };

    let expr = schema.expression(source)?;
    let variables = Variables::new();
    let context = run.context(document.root(), &variables);
    let value = evaluate(expr, &context)
        .map_err(|e| Error::xpath_eval(format!("pattern/@documents: {source}"), e.message))?;

    let uris: Vec<String> = match value {
        Value::NodeSet(nodes) => nodes
            .iter()
            .map(|&node| document.string_value(node))
            .collect(),
        other => vec![other.to_xpath_string(document)],
    };

    let mut targets = Vec::with_capacity(uris.len());
    for uri in uris {
        let text = schema.resolver().resolve(&uri, document.base_uri())?;
        let mut loaded = Document::from_str(&text)?;
        loaded.set_base_uri(uri.clone());
        targets.push((Some(uri), Some(loaded)));
    }
    Ok(targets)
}

/// Runs one pattern over one document.
fn run_pattern(
    run: Run<'_>,
    pattern: &Pattern,
    documents_label: Option<String>,
    variables: &mut Variables,
    failures: &mut usize,
) -> Result<ActivePattern> {
    let Run { document, options, .. } = run;
    let mark = variables.mark();
    bind_all(run, document.root(), &pattern.lets, variables, "pattern/let")?;

    let mut active = ActivePattern {
        id: pattern.id.clone(),
        name: pattern.title.clone(),
        documents: documents_label,
        rules: Vec::new(),
    };

    // First-matching-rule-wins: each rule claims the nodes no earlier rule in
    // this pattern has already claimed. Evaluating each rule's context once
    // per document, rather than testing every rule against every node, is
    // what keeps this linear rather than quadratic.
    let mut claims: HashMap<NodeId, usize> = HashMap::new();
    for (index, rule) in pattern.rules.iter().enumerate() {
        for node in matched_nodes(run, rule, variables)? {
            claims.entry(node).or_insert(index);
        }
    }

    if claims.is_empty() {
        variables.truncate(mark);
        return Ok(active);
    }

    for node in document.all_nodes_in_document_order() {
        let Some(&index) = claims.get(&node) else {
            continue;
        };
        let rule = &pattern.rules[index];
        let fired = fire_rule(run, rule, node, variables, failures)?;
        if options.record_fired_rules || !fired.assertions.is_empty() {
            active.rules.push(fired);
        }
        if options.max_failures.is_some_and(|limit| *failures >= limit) {
            break;
        }
    }

    variables.truncate(mark);
    Ok(active)
}

/// The nodes a rule's context matches, evaluated once for the whole document.
///
/// A rule context is an XSLT match pattern, which selects downwards from some
/// ancestor. The standard reduction is to evaluate the pattern from the root
/// across the descendant-or-self axis; see `spec/validation.md`.
fn matched_nodes(run: Run<'_>, rule: &Rule, variables: &Variables) -> Result<Vec<NodeId>> {
    let Run { schema, document, .. } = run;
    let Some(source) = &rule.context else {
        return Ok(Vec::new());
    };
    let expr = schema.expression(source)?;
    let rooted = root_match_expression(expr);
    let context = run.context(document.root(), variables);
    let value = evaluate(&rooted, &context).map_err(|e| {
        Error::xpath_eval(format!("rule/@context: {source}"), e.message)
    })?;
    match value {
        Value::NodeSet(nodes) => Ok(nodes),
        other => Err(Error::xpath_eval(
            format!("rule/@context: {source}"),
            format!(
                "a rule context must select nodes, but this one evaluated to a {}",
                other.type_name()
            ),
        )),
    }
}

/// Rewrites a match pattern into an absolute expression selecting every node
/// it matches anywhere in the document.
///
/// `a/b` becomes `/descendant-or-self::node()/a/b`; an absolute pattern is
/// already rooted and is left alone; a union rewrites each branch.
fn root_match_expression(expr: &Expr) -> Expr {
    match expr {
        Expr::Binary(BinaryOp::Union, left, right) => Expr::Binary(
            BinaryOp::Union,
            Box::new(root_match_expression(left)),
            Box::new(root_match_expression(right)),
        ),
        Expr::Path(path) => match path.start {
            PathStart::Context => {
                let mut steps = Vec::with_capacity(path.steps.len() + 1);
                steps.push(Step {
                    axis: Axis::DescendantOrSelf,
                    node_test: NodeTest::AnyNode,
                    predicates: Vec::new(),
                });
                steps.extend(path.steps.iter().cloned());
                Expr::Path(Box::new(PathExpr {
                    start: PathStart::Root,
                    steps,
                }))
            }
            _ => expr.clone(),
        },
        other => other.clone(),
    }
}

/// Evaluates one rule's assertions against the node it fired on.
fn fire_rule(
    run: Run<'_>,
    rule: &Rule,
    node: NodeId,
    variables: &mut Variables,
    failures: &mut usize,
) -> Result<FiredRule> {
    let Run { schema, document, options, .. } = run;
    let mark = variables.mark();
    bind_all(run, node, &rule.lets, variables, "rule/let")?;

    let mut fired = FiredRule {
        id: rule.id.clone(),
        context: rule.context.clone().unwrap_or_default(),
        role: rule.role.clone(),
        flag: rule.flag.clone(),
        location: document.location(node),
        assertions: Vec::new(),
    };

    for assertion in rule.assertions() {
        let expr = schema.expression(&assertion.test)?;
        let context = run.context(node, variables);
        let outcome = evaluate(expr, &context)
            .map_err(|e| {
                Error::xpath_eval(
                    format!("{}/@test: {}", assertion.kind.as_str(), assertion.test),
                    e.message,
                )
            })?
            .to_boolean();

        if !assertion.kind.is_reported(outcome) {
            continue;
        }

        let subject = resolve_subject(
            run,
            node,
            assertion.subject.as_deref().or(rule.subject.as_deref()),
            variables,
        )?;

        let mut diagnostics = Vec::new();
        for id in &assertion.diagnostics {
            if let Some(diagnostic) = schema.model().diagnostic(id) {
                diagnostics.push(DiagnosticResult {
                    id: id.clone(),
                    text: instantiate(run, node, &diagnostic.content, variables)?,
                });
            }
        }

        let mut properties = Vec::new();
        for id in &assertion.properties {
            if let Some(property) = schema.model().property(id) {
                properties.push(PropertyResult {
                    id: id.clone(),
                    role: property.role.clone(),
                    scheme: property.scheme.clone(),
                    text: instantiate(run, node, &property.content, variables)?,
                });
            }
        }

        let kind = match assertion.kind {
            crate::schema::AssertionKind::Assert => ResultKind::FailedAssert,
            crate::schema::AssertionKind::Report => ResultKind::SuccessfulReport,
        };
        if kind == ResultKind::FailedAssert {
            *failures += 1;
        }

        fired.assertions.push(AssertionResult {
            kind,
            test: assertion.test.clone(),
            location: document.location(subject),
            text: instantiate(run, node, &assertion.content, variables)?,
            id: assertion.id.clone(),
            // The assertion's own label wins; the rule's is the fallback.
            role: assertion.role.clone().or_else(|| rule.role.clone()),
            flag: assertion.flag.clone().or_else(|| rule.flag.clone()),
            see: assertion.see.clone(),
            icon: assertion.icon.clone(),
            fpi: assertion.fpi.clone(),
            diagnostics,
            properties,
        });

        if options.max_failures.is_some_and(|limit| *failures >= limit) {
            break;
        }
    }

    variables.truncate(mark);
    Ok(fired)
}

/// The node an assertion is *about*, which `@subject` can move away from the
/// context node.
fn resolve_subject(
    run: Run<'_>,
    node: NodeId,
    subject: Option<&str>,
    variables: &Variables,
) -> Result<NodeId> {
    let Run { schema, .. } = run;
    let Some(source) = subject else {
        return Ok(node);
    };
    let expr = schema.expression(source)?;
    let context = run.context(node, variables);
    let value = evaluate(expr, &context)
        .map_err(|e| Error::xpath_eval(format!("@subject: {source}"), e.message))?;
    // A subject that selects nothing falls back to the context node rather
    // than failing: the finding is still real, only its location is coarser.
    Ok(value
        .as_node_set()
        .and_then(|nodes| nodes.first().copied())
        .unwrap_or(node))
}

/// Binds a scope's `let` variables, in order, each visible to the next.
fn bind_all(
    run: Run<'_>,
    node: NodeId,
    lets: &[Let],
    variables: &mut Variables,
    location: &str,
) -> Result<()> {
    let Run { schema, .. } = run;
    for binding in lets {
        let value = match &binding.value {
            LetValue::Expression(source) => {
                let expr = schema.expression(source)?;
                let context = run.context(node, variables);
                evaluate(expr, &context).map_err(|e| {
                    Error::xpath_eval(
                        format!("{location}[@name='{}']: {source}", binding.name),
                        e.message,
                    )
                })?
            }
            LetValue::Content(content) => {
                Value::String(instantiate(run, node, content, variables)?)
            }
        };
        variables.bind(binding.name.clone(), value);
    }
    Ok(())
}

/// Builds an assertion's message by walking its rich content.
// `content` and `context` are both the right names for what they hold, and
// renaming either to satisfy the similarity heuristic would read worse.
#[allow(clippy::similar_names)]
fn instantiate(
    run: Run<'_>,
    node: NodeId,
    content: &[Content],
    variables: &Variables,
) -> Result<String> {
    let Run { schema, document, .. } = run;
    let mut out = String::new();
    for fragment in content {
        match fragment {
            Content::Text(text) => out.push_str(text),

            Content::ValueOf { select } => {
                let expr = schema.expression(select)?;
                let context = run.context(node, variables);
                let value = evaluate(expr, &context).map_err(|e| {
                    Error::xpath_eval(format!("value-of/@select: {select}"), e.message)
                })?;
                out.push_str(&value.to_xpath_string(document));
            }

            Content::Name { path } => {
                let target = match path {
                    None => Some(node),
                    Some(path_source) => {
                        let expr = schema.expression(path_source)?;
                        let context =
                            run.context(node, variables);
                        let value = evaluate(expr, &context).map_err(|e| {
                            Error::xpath_eval(format!("name/@path: {path_source}"), e.message)
                        })?;
                        value.as_node_set().and_then(|nodes| nodes.first().copied())
                    }
                };
                if let Some(name) = target.and_then(|t| document.name(t)) {
                    out.push_str(&name.display_name());
                }
            }

            // Presentation elements contribute their content; the markup is
            // kept in the model for renderers that want it, but the plain
            // text of a message is what SVRL carries.
            Content::Emph(inner)
            | Content::Span { content: inner, .. }
            | Content::Dir { content: inner, .. } => {
                out.push_str(&instantiate(run, node, inner, variables)?);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xpath::parse;

    #[test]
    fn relative_patterns_become_rooted_descendant_searches() {
        let expr = parse("a/b").unwrap();
        let rooted = root_match_expression(&expr);
        match rooted {
            Expr::Path(path) => {
                assert_eq!(path.start, PathStart::Root);
                assert_eq!(path.steps[0].axis, Axis::DescendantOrSelf);
                assert_eq!(path.steps.len(), 3);
            }
            other => panic!("unexpected shape: {other:?}"),
        }
    }

    #[test]
    fn absolute_patterns_are_left_alone() {
        let expr = parse("/a/b").unwrap();
        assert_eq!(root_match_expression(&expr), expr);
    }

    #[test]
    fn unions_rewrite_each_branch() {
        let rooted = root_match_expression(&parse("a | b").unwrap());
        match rooted {
            Expr::Binary(BinaryOp::Union, left, right) => {
                for branch in [*left, *right] {
                    match branch {
                        Expr::Path(path) => assert_eq!(path.start, PathStart::Root),
                        other => panic!("unexpected branch: {other:?}"),
                    }
                }
            }
            other => panic!("unexpected shape: {other:?}"),
        }
    }
}
