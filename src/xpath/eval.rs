//! The XPath 1.0 evaluator.
//!
//! Takes a parsed [`Expr`] and an [`EvalContext`] and produces a [`Value`].
//! The interesting parts are the axes, and the comparison rules, where XPath
//! 1.0's existential node-set semantics differ from what most readers expect
//! and are implemented literally rather than "fixed".

use super::ast::{Axis, BinaryOp, Expr, NodeTest, PathExpr, PathStart, Quantifier, Step};
use super::context::EvalContext;
use super::functions;
use super::temporal::{Temporal, TemporalKind};
use super::value::{flatten_into_sequence, Item, Value};
use crate::xml::{Document, NodeId, NodeKind};

/// A failure during evaluation.
///
/// Evaluation errors are genuine faults — an unbound variable, a type error,
/// an unknown function — not "the test was false". Schematron treats them as
/// errors so that a broken schema cannot report a clean bill of health.
#[derive(Debug, Clone)]
pub struct EvalError {
    /// What went wrong.
    pub message: String,
}

impl EvalError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for EvalError {}

/// Evaluates an expression.
///
/// # Errors
///
/// Returns an [`EvalError`] for an unbound variable, a type error such as a
/// union of non-node-sets, or an unknown function.
///
/// # Examples
///
/// ```
/// use schematron::xml::Document;
/// use schematron::xpath::{evaluate, parse, EvalContext, Namespaces, Value, Variables};
///
/// let doc = Document::from_str("<a><b>1</b><b>2</b></a>").unwrap();
/// let expr = parse("count(b)").unwrap();
/// let vars = Variables::new();
/// let ns = Namespaces::new();
/// let context = EvalContext::new(&doc, doc.document_element().unwrap(), &vars, &ns);
/// assert_eq!(evaluate(&expr, &context).unwrap(), Value::Number(2.0));
/// ```
pub fn evaluate(expr: &Expr, context: &EvalContext<'_>) -> Result<Value, EvalError> {
    match expr {
        Expr::Literal(text) => Ok(Value::String(text.clone())),
        Expr::Number(value) => Ok(Value::Number(*value)),

        Expr::Variable(name) => {
            let key = name.to_string();
            context.variables.lookup(&key).cloned().ok_or_else(|| {
                let known: Vec<&str> = context.variables.names().collect();
                EvalError::new(format!(
                    "variable ${key} is not bound{}",
                    if known.is_empty() {
                        String::new()
                    } else {
                        format!("; bound here: {}", known.join(", "))
                    }
                ))
            })
        }

        Expr::Negate(inner) => {
            let value = evaluate(inner, context)?;
            Ok(Value::Number(-value.to_number(context.document)))
        }

        Expr::Binary(op, left, right) => evaluate_binary(*op, left, right, context),

        Expr::Function { name, args } => {
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                values.push(evaluate(arg, context)?);
            }
            functions::call(name, &values, context)
        }

        Expr::Path(path) => Ok(Value::NodeSet(evaluate_path(path, context)?)),

        Expr::Sequence(members) => {
            let mut values = Vec::with_capacity(members.len());
            for member in members {
                values.push(evaluate(member, context)?);
            }
            Ok(Value::Sequence(flatten_into_sequence(values)))
        }

        Expr::Range(from, to) => {
            let from = evaluate(from, context)?.to_number(context.document);
            let to = evaluate(to, context)?.to_number(context.document);
            Ok(Value::Sequence(range_items(from, to)?))
        }

        Expr::For {
            variable,
            input,
            body,
        } => evaluate_for(variable, input, body, context),

        Expr::Quantified {
            quantifier,
            variable,
            input,
            test,
        } => evaluate_quantified(*quantifier, variable, input, test, context),

        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            // Only the taken branch is evaluated, so a conditional can guard
            // an expression that would fail on the other side.
            let holds = evaluate(condition, context)?
                .effective_boolean_value()
                .map_err(EvalError::new)?;
            if holds {
                evaluate(then_branch, context)
            } else {
                evaluate(else_branch, context)
            }
        }
    }
}

fn evaluate_binary(
    op: BinaryOp,
    left: &Expr,
    right: &Expr,
    context: &EvalContext<'_>,
) -> Result<Value, EvalError> {
    // `and` and `or` short-circuit, which matters when the right operand
    // would raise an error on nodes the left operand has already ruled out.
    match op {
        BinaryOp::And => {
            if !evaluate(left, context)?.to_boolean() {
                return Ok(Value::Boolean(false));
            }
            return Ok(Value::Boolean(evaluate(right, context)?.to_boolean()));
        }
        BinaryOp::Or => {
            if evaluate(left, context)?.to_boolean() {
                return Ok(Value::Boolean(true));
            }
            return Ok(Value::Boolean(evaluate(right, context)?.to_boolean()));
        }
        _ => {}
    }

    let left = evaluate(left, context)?;
    let right = evaluate(right, context)?;
    let document = context.document;

    match op {
        BinaryOp::And | BinaryOp::Or => unreachable!("handled above"),

        BinaryOp::Union => match (left, right) {
            (Value::NodeSet(a), Value::NodeSet(b)) => {
                let mut merged = a;
                merged.extend(b);
                Ok(Value::NodeSet(sort_and_deduplicate(merged, document)))
            }
            (a, b) => Err(EvalError::new(format!(
                "the union operator | requires two node-sets, but was given a {} and a {}",
                a.type_name(),
                b.type_name()
            ))),
        },

        BinaryOp::Equal | BinaryOp::NotEqual => {
            let want_equal = op == BinaryOp::Equal;
            // XPath 2.0: a comparison involving a date compares instants, not
            // strings, so an offset is honoured and an untyped operand is
            // cast. Unreachable under XPath 1.0, which has no temporal type.
            if has_temporal(&left) || has_temporal(&right) {
                let kind = temporal_kind_of(&left, &right);
                let a = temporal_operands(&left, kind, document)?;
                let b = temporal_operands(&right, kind, document)?;
                let equal = a.iter().any(|x| b.iter().any(|y| x == y));
                return Ok(Value::Boolean(equal == want_equal));
            }
            Ok(Value::Boolean(compare_equality(
                &left, &right, document, want_equal,
            )))
        }

        BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
            if has_temporal(&left) || has_temporal(&right) {
                let kind = temporal_kind_of(&left, &right);
                let a = temporal_operands(&left, kind, document)?;
                let b = temporal_operands(&right, kind, document)?;
                let holds = a.iter().any(|x| {
                    b.iter().any(|y| match op {
                        BinaryOp::Less => x < y,
                        BinaryOp::LessEqual => x <= y,
                        BinaryOp::Greater => x > y,
                        BinaryOp::GreaterEqual => x >= y,
                        _ => unreachable!("outer match limits the operators"),
                    })
                });
                return Ok(Value::Boolean(holds));
            }
            Ok(Value::Boolean(compare_relational(op, &left, &right, document)))
        }

        BinaryOp::Add
        | BinaryOp::Subtract
        | BinaryOp::Multiply
        | BinaryOp::Divide
        | BinaryOp::Modulo => {
            let a = left.to_number(document);
            let b = right.to_number(document);
            Ok(Value::Number(match op {
                BinaryOp::Add => a + b,
                BinaryOp::Subtract => a - b,
                BinaryOp::Multiply => a * b,
                // IEEE division: no error on zero, the result is an infinity
                // or NaN, exactly as XPath 1.0 requires.
                BinaryOp::Divide => a / b,
                BinaryOp::Modulo => a % b,
                _ => unreachable!("outer match limits the operators"),
            }))
        }
    }
}

/// `=` and `!=`, with XPath 1.0's existential node-set semantics.
///
/// When either side is a node-set, the comparison is true if *any* node
/// satisfies it. That is why `a != b` is not `not(a = b)` for node-sets: both
/// can be true at once when `a` has two different values.
fn compare_equality(left: &Value, right: &Value, document: &Document, want_equal: bool) -> bool {
    let test = |a: &str, b: &str| (a == b) == want_equal;

    match (left, right) {
        // XPath 2.0: a general comparison involving a sequence is
        // existential over its atomized items, exactly as it is for a
        // node-set. Handled first so that every XPath 1.0 arm below is
        // reached by precisely the values it was written for.
        (Value::Sequence(_), _) | (_, Value::Sequence(_)) => {
            let a = comparable_strings(left, document);
            let b = comparable_strings(right, document);
            a.iter()
                .any(|left| b.iter().any(|right| test(left, right)))
        }

        (Value::NodeSet(a), Value::NodeSet(b)) => {
            let b_strings: Vec<String> = b.iter().map(|&n| document.string_value(n)).collect();
            a.iter().any(|&node| {
                let a_string = document.string_value(node);
                b_strings.iter().any(|b_string| test(&a_string, b_string))
            })
        }

        (Value::NodeSet(nodes), other) | (other, Value::NodeSet(nodes)) => match other {
            // Comparing a node-set to a boolean converts the node-set to a
            // boolean first — that is, to whether it is non-empty — and does
            // not look at the nodes' values at all.
            Value::Boolean(b) => (nodes.is_empty() != *b) == want_equal,
            Value::Number(n) => nodes.iter().any(|&node| {
                let value = super::value::parse_number(&document.string_value(node));
                // NaN is never equal to anything, so `!=` against a
                // non-numeric string is true, and `=` is false.
                (value == *n) == want_equal
            }),
            Value::String(s) => nodes
                .iter()
                .any(|&node| test(&document.string_value(node), s)),
            Value::NodeSet(_) => unreachable!("matched by the arm above"),
            Value::Sequence(_) => unreachable!("matched by the sequence arm above"),
        },

        // Neither side is a node-set: boolean wins, then number, then string.
        (a, b) => {
            if matches!(a, Value::Boolean(_)) || matches!(b, Value::Boolean(_)) {
                (a.to_boolean() == b.to_boolean()) == want_equal
            } else if matches!(a, Value::Number(_)) || matches!(b, Value::Number(_)) {
                (a.to_number(document) == b.to_number(document)) == want_equal
            } else {
                test(&a.to_xpath_string(document), &b.to_xpath_string(document))
            }
        }
    }
}

/// `<`, `<=`, `>`, `>=`, which always compare as numbers.
fn compare_relational(op: BinaryOp, left: &Value, right: &Value, document: &Document) -> bool {
    let compare = |a: f64, b: f64| match op {
        BinaryOp::Less => a < b,
        BinaryOp::LessEqual => a <= b,
        BinaryOp::Greater => a > b,
        BinaryOp::GreaterEqual => a >= b,
        _ => unreachable!("caller limits the operators"),
    };

    let numbers = |value: &Value| -> Vec<f64> {
        match value {
            Value::NodeSet(nodes) => nodes
                .iter()
                .map(|&n| super::value::parse_number(&document.string_value(n)))
                .collect(),
            // XPath 2.0: existential over the sequence's items, as for a
            // node-set.
            Value::Sequence(items) => items.iter().map(|item| item.to_number(document)).collect(),
            other => vec![other.to_number(document)],
        }
    };

    let left_numbers = numbers(left);
    let right_numbers = numbers(right);
    // Existential again: true if any pair satisfies the comparison. For two
    // scalars this degenerates to the obvious single comparison.
    left_numbers
        .iter()
        .any(|&a| right_numbers.iter().any(|&b| compare(a, b)))
}

/// `for $v in E return E`: evaluates the body once per item and concatenates.
fn evaluate_for(
    variable: &super::ast::NameTest,
    input: &Expr,
    body: &Expr,
    context: &EvalContext<'_>,
) -> Result<Value, EvalError> {
    let items = evaluate(input, context)?.into_items();
    let mut out = Vec::with_capacity(items.len());

    // One clone of the enclosing scope for the whole loop; the binding is
    // replaced per iteration rather than the scope being rebuilt each time.
    let mut scope = context.variables.clone();
    let mark = scope.mark();
    let name = variable.to_string();

    for item in items {
        scope.truncate(mark);
        scope.bind(name.clone(), item_to_value(item));
        let inner = EvalContext {
            variables: &scope,
            ..*context
        };
        out.push(evaluate(body, &inner)?);
    }
    Ok(Value::Sequence(flatten_into_sequence(out)))
}

/// `some`/`every $v in E satisfies E`.
fn evaluate_quantified(
    quantifier: Quantifier,
    variable: &super::ast::NameTest,
    input: &Expr,
    test: &Expr,
    context: &EvalContext<'_>,
) -> Result<Value, EvalError> {
    let items = evaluate(input, context)?.into_items();
    let mut scope = context.variables.clone();
    let mark = scope.mark();
    let name = variable.to_string();

    for item in items {
        scope.truncate(mark);
        scope.bind(name.clone(), item_to_value(item));
        let inner = EvalContext {
            variables: &scope,
            ..*context
        };
        let holds = evaluate(test, &inner)?
            .effective_boolean_value()
            .map_err(EvalError::new)?;
        // Both quantifiers short-circuit on the first decisive item.
        match quantifier {
            Quantifier::Some if holds => return Ok(Value::Boolean(true)),
            Quantifier::Every if !holds => return Ok(Value::Boolean(false)),
            _ => {}
        }
    }
    // `every` over an empty sequence is true, `some` is false.
    Ok(Value::Boolean(matches!(quantifier, Quantifier::Every)))
}

/// Wraps a sequence item back into a value, for binding to a variable.
fn item_to_value(item: Item) -> Value {
    match item {
        // A single node binds as a one-node node-set, so paths can continue
        // from it: `for $x in a return $x/b`.
        Item::Node(node) => Value::NodeSet(vec![node]),
        Item::String(text) => Value::String(text),
        Item::Number(number) => Value::Number(number),
        Item::Boolean(boolean) => Value::Boolean(boolean),
        // A temporal has no scalar `Value` of its own; XPath 2.0 treats every
        // value as a sequence, and this is a one-item one.
        Item::Temporal(_) => Value::Sequence(vec![item]),
    }
}

/// Whether a value carries any date, dateTime, or time item.
///
/// Only XPath 2.0 can produce one, so this is always false under XPath 1.0
/// and the temporal comparison path below is unreachable there.
fn has_temporal(value: &Value) -> bool {
    value
        .as_sequence()
        .is_some_and(|items| items.iter().any(|item| item.as_temporal().is_some()))
}

/// The temporal operands a value contributes to a comparison.
///
/// A temporal item is used as it is. Anything else is *untyped* — an
/// attribute or element in an XML document has no type — and is cast to
/// `kind`, which XPath 2.0 requires when an untyped operand meets a typed
/// one. A value that will not cast is an error naming it, because a date
/// typo should fail loudly rather than make a test quietly false.
fn temporal_operands(
    value: &Value,
    kind: TemporalKind,
    document: &Document,
) -> Result<Vec<Temporal>, EvalError> {
    let mut out = Vec::new();
    for item in value.clone().into_items() {
        if let Some(temporal) = item.as_temporal() {
            out.push(*temporal);
            continue;
        }
        let text = item.to_xpath_string(document);
        let parsed = Temporal::parse(&text, kind).map_err(EvalError::new)?;
        out.push(parsed);
    }
    Ok(out)
}

/// The type a temporal comparison should cast untyped operands to.
fn temporal_kind_of(left: &Value, right: &Value) -> TemporalKind {
    for value in [left, right] {
        if let Some(items) = value.as_sequence() {
            if let Some(temporal) = items.iter().find_map(Item::as_temporal) {
                return temporal.kind();
            }
        }
    }
    TemporalKind::DateTime
}

/// The largest `to` range that will be materialised.
///
/// XPath 2.0 sets no limit, but a range becomes a real `Vec`, so an absurd
/// one would be stopped by the allocator rather than by an error. The fuzz
/// targets exist to keep "hostile input is an error" true, and this is part
/// of it.
const MAX_RANGE: f64 = 1_000_000.0;

/// The items of `from to to`, ascending.
///
/// A descending range is the empty sequence, which XPath 2.0 specifies. A
/// bound that is not a number is a type error rather than an empty result,
/// because silently yielding nothing would make a broken range look like an
/// empty one.
fn range_items(from: f64, to: f64) -> Result<Vec<Item>, EvalError> {
    if from.is_nan() || to.is_nan() {
        return Err(EvalError::new(
            "the bounds of a `to` range must be numbers",
        ));
    }
    let from = from.trunc();
    let to = to.trunc();
    if from > to {
        return Ok(Vec::new());
    }

    // A range is materialised, so an absurd one would exhaust memory.
    if to - from >= MAX_RANGE {
        return Err(EvalError::new(format!(
            "a `to` range of {} items exceeds the limit of {MAX_RANGE:.0}",
            to - from + 1.0
        )));
    }

    let mut items = Vec::new();
    let mut current = from;
    while current <= to {
        items.push(Item::Number(current));
        current += 1.0;
    }
    Ok(items)
}

/// The string values a value contributes to a general comparison.
///
/// A sequence or node-set contributes one per item; a scalar contributes one.
fn comparable_strings(value: &Value, document: &Document) -> Vec<String> {
    match value {
        Value::Sequence(items) => items
            .iter()
            .map(|item| item.to_xpath_string(document))
            .collect(),
        Value::NodeSet(nodes) => nodes
            .iter()
            .map(|&node| document.string_value(node))
            .collect(),
        other => vec![other.to_xpath_string(document)],
    }
}

/// Sorts a node-set into document order and removes duplicates.
fn sort_and_deduplicate(mut nodes: Vec<NodeId>, document: &Document) -> Vec<NodeId> {
    nodes.sort_unstable_by_key(|&n| document.order(n));
    nodes.dedup();
    nodes
}

fn evaluate_path(path: &PathExpr, context: &EvalContext<'_>) -> Result<Vec<NodeId>, EvalError> {
    let mut current = match &path.start {
        // The root of the document the context node belongs to, which is the
        // primary one unless `document()` has merged another in. `/invoice`
        // evaluated inside a loaded document must mean *that* document's root.
        PathStart::Root => vec![context.document.root_of(context.node)],
        PathStart::Context => vec![context.node],
        PathStart::Expr(expr, predicates) => {
            let value = evaluate(expr, context)?;
            let Value::NodeSet(nodes) = value else {
                return Err(EvalError::new(format!(
                    "a path can only continue from a node-set, but the left side is a {}",
                    value.type_name()
                )));
            };
            let mut nodes = sort_and_deduplicate(nodes, context.document);
            for predicate in predicates {
                nodes = filter_by_predicate(nodes, predicate, context)?;
            }
            nodes
        }
    };

    for step in &path.steps {
        current = evaluate_step(step, &current, context)?;
    }
    Ok(current)
}

fn evaluate_step(
    step: &Step,
    input: &[NodeId],
    context: &EvalContext<'_>,
) -> Result<Vec<NodeId>, EvalError> {
    let document = context.document;
    let mut output: Vec<NodeId> = Vec::new();

    for &node in input {
        // Axis order matters: a reverse axis numbers positions outwards from
        // the context node, so predicates must see it in that order.
        let mut candidates: Vec<NodeId> = collect_axis(document, node, step.axis)
            .into_iter()
            .filter(|&candidate| {
                matches_node_test(document, candidate, step.axis, &step.node_test, context)
            })
            .collect();

        for predicate in &step.predicates {
            candidates = filter_by_predicate(candidates, predicate, context)?;
        }
        output.extend(candidates);
    }

    Ok(sort_and_deduplicate(output, document))
}

/// Applies one predicate, giving each node the position it holds in the list.
fn filter_by_predicate(
    nodes: Vec<NodeId>,
    predicate: &Expr,
    context: &EvalContext<'_>,
) -> Result<Vec<NodeId>, EvalError> {
    let size = nodes.len();
    let mut kept = Vec::with_capacity(size);
    for (index, node) in nodes.into_iter().enumerate() {
        let position = index + 1;
        let inner = context.focus(node, position, size);
        let value = evaluate(predicate, &inner)?;
        // A numeric predicate is shorthand for `position() = n`.
        let keep = match value {
            Value::Number(n) => {
                #[allow(clippy::cast_precision_loss)]
                let position = position as f64;
                (n - position).abs() < f64::EPSILON
            }
            other => other.to_boolean(),
        };
        if keep {
            kept.push(node);
        }
    }
    Ok(kept)
}

/// The nodes on an axis, in axis order.
///
/// Reverse axes come back nearest-first; forward axes come back in document
/// order.
fn collect_axis(document: &Document, node: NodeId, axis: Axis) -> Vec<NodeId> {
    match axis {
        Axis::SelfAxis => vec![node],
        Axis::Child => document.children(node).to_vec(),
        Axis::Attribute => document.attributes(node).to_vec(),
        Axis::Namespace => document.namespaces(node).to_vec(),
        Axis::Parent => document.parent(node).into_iter().collect(),
        Axis::Ancestor => document.ancestors(node),
        Axis::AncestorOrSelf => {
            let mut nodes = vec![node];
            nodes.extend(document.ancestors(node));
            nodes
        }
        Axis::Descendant => {
            let mut nodes = document.descendants_or_self(node);
            nodes.remove(0);
            nodes
        }
        Axis::DescendantOrSelf => document.descendants_or_self(node),
        Axis::FollowingSibling => siblings(document, node, true),
        Axis::PrecedingSibling => siblings(document, node, false),
        Axis::Following => {
            // Everything after this node in document order that is not one of
            // its own descendants. Attribute and namespace nodes are excluded
            // from both the following and preceding axes.
            //
            // "Not a descendant" is `order > subtree_end`, an integer compare,
            // rather than a membership test against a collected set.
            let after = document.subtree_end(node);
            document
                .descendants_or_self(document.root_of(node))
                .into_iter()
                .filter(|&n| document.order(n) > after)
                .collect()
        }
        Axis::Preceding => {
            // Before this node, and not one of its ancestors. An ancestor is
            // exactly a node whose subtree still contains this one, so
            // excluding ancestors is `subtree_end < order`.
            let order = document.order(node);
            let mut nodes: Vec<NodeId> = document
                .descendants_or_self(document.root_of(node))
                .into_iter()
                .filter(|&n| document.order(n) < order && document.subtree_end(n) < order)
                .collect();
            // Reverse axis: nearest first.
            nodes.reverse();
            nodes
        }
    }
}

/// Siblings on one side of a node, in axis order.
fn siblings(document: &Document, node: NodeId, following: bool) -> Vec<NodeId> {
    // Attribute and namespace nodes have no siblings, by definition of the
    // sibling axes, even though they have a parent.
    if matches!(
        document.kind(node),
        NodeKind::Attribute | NodeKind::Namespace
    ) {
        return Vec::new();
    }
    let Some(parent) = document.parent(node) else {
        return Vec::new();
    };
    let children = document.children(parent);
    let Some(index) = children.iter().position(|&n| n == node) else {
        return Vec::new();
    };
    if following {
        children[index + 1..].to_vec()
    } else {
        // Reverse axis: nearest first.
        let mut before = children[..index].to_vec();
        before.reverse();
        before
    }
}

/// The node kind an axis principally yields.
///
/// The attribute and namespace axes yield their own kinds; every other axis
/// yields elements. This is what makes `child::foo` mean "element named foo"
/// while `attribute::foo` means "attribute named foo".
const fn principal_kind(axis: Axis) -> NodeKind {
    match axis {
        Axis::Attribute => NodeKind::Attribute,
        Axis::Namespace => NodeKind::Namespace,
        _ => NodeKind::Element,
    }
}

fn matches_node_test(
    document: &Document,
    node: NodeId,
    axis: Axis,
    test: &NodeTest,
    context: &EvalContext<'_>,
) -> bool {
    let kind = document.kind(node);
    match test {
        NodeTest::AnyNode => true,
        NodeTest::Text => kind == NodeKind::Text,
        NodeTest::Comment => kind == NodeKind::Comment,
        NodeTest::ProcessingInstruction(target) => {
            kind == NodeKind::ProcessingInstruction
                && match target {
                    None => true,
                    Some(wanted) => {
                        document.name(node).is_some_and(|n| &n.local == wanted)
                    }
                }
        }
        NodeTest::Wildcard => kind == principal_kind(axis),
        NodeTest::NamespaceWildcard(prefix) => {
            kind == principal_kind(axis)
                && match context.namespaces.resolve(prefix) {
                    // An undeclared prefix cannot match anything. The schema
                    // compiler rejects these, so reaching here means the
                    // expression was evaluated outside a compiled schema.
                    None => false,
                    Some(uri) => document
                        .name(node)
                        .is_some_and(|n| n.uri.as_deref() == Some(uri)),
                }
        }
        NodeTest::Name(name) => {
            if kind != principal_kind(axis) {
                return false;
            }
            // On the namespace axis the "name" of a node is its prefix.
            if axis == Axis::Namespace {
                return document.name(node).is_some_and(|n| n.local == name.local);
            }
            let uri = match &name.prefix {
                Some(prefix) => match context.namespaces.resolve(prefix) {
                    Some(uri) => Some(uri),
                    None => return false,
                },
                // XPath 1.0: an unprefixed name is in no namespace.
                None => None,
            };
            document
                .name(node)
                .is_some_and(|n| n.matches_parts(uri, &name.local))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xpath::{parse, Namespaces, Variables};

    fn eval(source: &str, expression: &str) -> Value {
        let document = Document::from_str(source).unwrap();
        let expr = parse(expression).unwrap();
        let variables = Variables::new();
        let namespaces = Namespaces::new();
        let context = EvalContext::new(
            &document,
            document.document_element().unwrap(),
            &variables,
            &namespaces,
        );
        evaluate(&expr, &context).unwrap()
    }

    fn number(source: &str, expression: &str) -> f64 {
        let document = Document::from_str(source).unwrap();
        eval(source, expression).to_number(&document)
    }

    fn boolean(source: &str, expression: &str) -> bool {
        eval(source, expression).to_boolean()
    }

    fn text(source: &str, expression: &str) -> String {
        let document = Document::from_str(source).unwrap();
        eval(source, expression).to_xpath_string(&document)
    }

    const DOC: &str = "<a><b id='1'>one</b><b id='2'>two</b><c><d/></c></a>";

    #[test]
    fn child_axis_selects_named_children() {
        assert_eq!(number(DOC, "count(b)"), 2.0);
        assert_eq!(number(DOC, "count(child::b)"), 2.0);
    }

    #[test]
    fn descendant_axis_reaches_grandchildren() {
        assert_eq!(number(DOC, "count(descendant::*)"), 4.0);
        assert_eq!(number(DOC, "count(.//d)"), 1.0);
    }

    #[test]
    fn attribute_axis_needs_the_attribute_principal_type() {
        assert_eq!(number(DOC, "count(b/@id)"), 2.0);
        // `b/id` is an element test, so it matches nothing.
        assert_eq!(number(DOC, "count(b/id)"), 0.0);
    }

    #[test]
    fn parent_and_ancestor_axes() {
        assert_eq!(number(DOC, "count(c/d/parent::c)"), 1.0);
        assert_eq!(number(DOC, "count(c/d/ancestor::*)"), 2.0);
        assert_eq!(number(DOC, "count(c/d/ancestor-or-self::*)"), 3.0);
    }

    #[test]
    fn sibling_axes_are_side_specific() {
        assert_eq!(number(DOC, "count(c/preceding-sibling::*)"), 2.0);
        assert_eq!(number(DOC, "count(b[1]/following-sibling::*)"), 2.0);
    }

    #[test]
    fn following_and_preceding_exclude_ancestors_and_descendants() {
        assert_eq!(number(DOC, "count(b[1]/following::*)"), 3.0);
        assert_eq!(number(DOC, "count(c/d/preceding::*)"), 2.0);
    }

    #[test]
    fn predicates_number_positions_from_one() {
        assert_eq!(text(DOC, "b[1]"), "one");
        assert_eq!(text(DOC, "b[2]"), "two");
        assert_eq!(text(DOC, "b[last()]"), "two");
        assert_eq!(number(DOC, "count(b[position() = 1])"), 1.0);
    }

    #[test]
    fn reverse_axis_predicates_count_outwards() {
        // The nearest preceding sibling of `c` is the second `b`, not the first.
        assert_eq!(text(DOC, "c/preceding-sibling::b[1]"), "two");
    }

    #[test]
    fn node_set_equality_is_existential() {
        // One of the two `b` elements has id 1, so this is true, and yet the
        // negation is also true because the other one does not.
        assert!(boolean(DOC, "b/@id = '1'"));
        assert!(boolean(DOC, "b/@id != '1'"));
    }

    #[test]
    fn empty_node_set_compares_false_both_ways() {
        assert!(!boolean(DOC, "nothing = 'x'"));
        assert!(!boolean(DOC, "nothing != 'x'"));
    }

    #[test]
    fn relational_comparison_is_numeric() {
        assert!(boolean(DOC, "b/@id > 1"));
        assert!(!boolean(DOC, "b/@id > 2"));
        // A non-numeric string becomes NaN, and NaN fails every comparison.
        assert!(!boolean(DOC, "'x' > 0"));
    }

    #[test]
    fn arithmetic_follows_ieee_754() {
        assert_eq!(number(DOC, "1 div 0"), f64::INFINITY);
        assert!(number(DOC, "0 div 0").is_nan());
        assert_eq!(number(DOC, "5 mod 3"), 2.0);
        assert_eq!(number(DOC, "-5 mod 3"), -2.0);
    }

    #[test]
    fn union_merges_into_document_order() {
        assert_eq!(number(DOC, "count(b | c)"), 3.0);
        // A node in both operands appears once.
        assert_eq!(number(DOC, "count(b | b)"), 2.0);
    }

    #[test]
    fn union_of_non_node_sets_is_an_error() {
        let document = Document::from_str(DOC).unwrap();
        let expr = parse("1 | 2").unwrap();
        let variables = Variables::new();
        let namespaces = Namespaces::new();
        let context = EvalContext::new(&document, document.root(), &variables, &namespaces);
        assert!(evaluate(&expr, &context).is_err());
    }

    #[test]
    fn string_functions() {
        assert_eq!(text(DOC, "concat('a', 'b')"), "ab");
        assert_eq!(text(DOC, "substring('12345', 2, 3)"), "234");
        assert_eq!(text(DOC, "substring('12345', 1.5, 2.6)"), "234");
        assert_eq!(text(DOC, "substring-before('a/b', '/')"), "a");
        assert_eq!(text(DOC, "substring-after('a/b', '/')"), "b");
        assert_eq!(text(DOC, "normalize-space('  a  b ')"), "a b");
        assert_eq!(text(DOC, "translate('bar', 'abc', 'ABC')"), "BAr");
        assert_eq!(number(DOC, "string-length('abc')"), 3.0);
        assert!(boolean(DOC, "starts-with('abc', 'a')"));
        assert!(boolean(DOC, "contains('abc', 'b')"));
    }

    #[test]
    fn numeric_functions() {
        assert_eq!(number(DOC, "sum(b/@id)"), 3.0);
        assert_eq!(number(DOC, "floor(1.9)"), 1.0);
        assert_eq!(number(DOC, "ceiling(1.1)"), 2.0);
        assert_eq!(number(DOC, "round(1.5)"), 2.0);
        assert_eq!(number(DOC, "round(-1.5)"), -1.0);
    }

    #[test]
    fn name_functions_use_the_context_node() {
        assert_eq!(text(DOC, "name()"), "a");
        assert_eq!(text(DOC, "local-name(b[1])"), "b");
        assert_eq!(text(DOC, "name(b[2]/@id)"), "id");
    }

    #[test]
    fn unbound_variable_is_an_error_not_an_empty_node_set() {
        let document = Document::from_str(DOC).unwrap();
        let expr = parse("$missing").unwrap();
        let variables = Variables::new();
        let namespaces = Namespaces::new();
        let context = EvalContext::new(&document, document.root(), &variables, &namespaces);
        let error = evaluate(&expr, &context).unwrap_err();
        assert!(error.message.contains("$missing"), "{error}");
    }

    #[test]
    fn variables_resolve_when_bound() {
        let document = Document::from_str(DOC).unwrap();
        let expr = parse("$n + 1").unwrap();
        let mut variables = Variables::new();
        variables.bind("n", Value::Number(41.0));
        let namespaces = Namespaces::new();
        let context = EvalContext::new(&document, document.root(), &variables, &namespaces);
        assert_eq!(evaluate(&expr, &context).unwrap(), Value::Number(42.0));
    }

    #[test]
    fn prefixed_names_resolve_through_the_namespace_bindings() {
        let document = Document::from_str(r#"<a xmlns="urn:n"><b/></a>"#).unwrap();
        let mut namespaces = Namespaces::new();
        namespaces.insert("p", "urn:n");
        let variables = Variables::new();
        let context = EvalContext::new(
            &document,
            document.document_element().unwrap(),
            &variables,
            &namespaces,
        );

        let prefixed = parse("count(p:b)").unwrap();
        assert_eq!(evaluate(&prefixed, &context).unwrap(), Value::Number(1.0));

        // An unprefixed name is in no namespace, so it matches nothing here.
        let bare = parse("count(b)").unwrap();
        assert_eq!(evaluate(&bare, &context).unwrap(), Value::Number(0.0));
    }

    #[test]
    fn text_and_comment_node_tests() {
        let source = "<a>x<!--c--><?pi d?></a>";
        assert_eq!(number(source, "count(text())"), 1.0);
        assert_eq!(number(source, "count(comment())"), 1.0);
        assert_eq!(number(source, "count(processing-instruction())"), 1.0);
        assert_eq!(number(source, "count(processing-instruction('pi'))"), 1.0);
        assert_eq!(number(source, "count(processing-instruction('other'))"), 0.0);
        assert_eq!(number(source, "count(node())"), 3.0);
    }

    #[test]
    fn absolute_paths_start_at_the_root() {
        assert_eq!(number(DOC, "count(/a/b)"), 2.0);
        assert_eq!(number(DOC, "count(//b)"), 2.0);
        assert_eq!(number(DOC, "count(/)"), 1.0);
    }

    #[test]
    fn boolean_operators_short_circuit() {
        // The right operand would raise an unbound-variable error if it ran.
        let document = Document::from_str(DOC).unwrap();
        let variables = Variables::new();
        let namespaces = Namespaces::new();
        let context = EvalContext::new(&document, document.root(), &variables, &namespaces);
        let expr = parse("false() and $missing").unwrap();
        assert_eq!(evaluate(&expr, &context).unwrap(), Value::Boolean(false));
        let expr = parse("true() or $missing").unwrap();
        assert_eq!(evaluate(&expr, &context).unwrap(), Value::Boolean(true));
    }

    #[test]
    fn lang_matches_inherited_xml_lang() {
        let source = r#"<a xml:lang="en"><b/></a>"#;
        assert!(boolean(source, "b[lang('en')]"));
        assert!(!boolean(source, "b[lang('fr')]"));
    }

    #[test]
    fn wrong_arity_is_an_error_not_a_panic() {
        // Found by the fuzz_xpath target: `not()` parses fine, and the
        // evaluator used to index args[0] on the strength of the schema
        // compiler having checked arity — which it has not, on this path.
        let document = Document::from_str(DOC).unwrap();
        let variables = Variables::new();
        let namespaces = Namespaces::new();
        let context = EvalContext::new(&document, document.root(), &variables, &namespaces);

        for source in ["not()", "translate('a')", "substring('a')", "concat('a')", "boolean()"] {
            let expr = parse(source).unwrap();
            let error = evaluate(&expr, &context)
                .expect_err(&format!("{source} should be an arity error"));
            assert!(error.message.contains("argument"), "{source}: {error}");
        }
    }

    #[test]
    fn id_function_finds_elements_by_id_attribute() {
        assert_eq!(number(DOC, "count(id('1'))"), 1.0);
        assert_eq!(number(DOC, "count(id('1 2'))"), 2.0);
    }
}
