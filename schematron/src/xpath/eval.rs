//! The XPath 1.0 evaluator.
//!
//! Takes a parsed [`Expr`] and an [`EvalContext`] and produces a [`Value`].
//! The interesting parts are the axes, and the comparison rules, where XPath
//! 1.0's existential node-set semantics differ from what most readers expect
//! and are implemented literally rather than "fixed".

use super::ast::{
    Axis, BinaryOp, Expr, ItemType, NodeTest, Occurrence, PathExpr, PathStart, Quantifier,
    SequenceType, Step, TypeOp,
};
use super::context::EvalContext;
use std::cmp::Ordering;

use super::decimal::{self, Exact};
use super::functions;
use super::numeric;
use super::temporal::{add_months, add_seconds, Duration, DurationKind, Temporal, TemporalKind};
use super::value::{flatten_into_sequence, FunctionItem, Item, NumericType, Value};
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
/// assert_eq!(evaluate(&expr, &context).unwrap(), Value::Number(2.0, Default::default()));
/// ```
pub fn evaluate(expr: &Expr, context: &EvalContext<'_>) -> Result<Value, EvalError> {
    match expr {
        Expr::Literal(text) => Ok(Value::String(text.clone())),
        Expr::Number(value, numeric_type) => Ok(literal_number(*value, *numeric_type, context)),

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

        Expr::Negate(inner) => Ok(negate(&evaluate(inner, context)?, context.document)),

        Expr::Binary(op, left, right) => evaluate_binary(*op, left, right, context),

        Expr::Function { name, args } => {
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                values.push(evaluate(arg, context)?);
            }
            functions::call(name, &values, context)
        }

        Expr::Path(path) => evaluate_path_or_context_item(path, context),

        Expr::TypeOp {
            op,
            value,
            sequence_type,
        } => {
            let value = evaluate(value, context)?;
            evaluate_type_op(*op, &value, sequence_type, context)
        }

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

        // No iteration, no budget: `value` is bound to `variable` once, as
        // a whole `Value`, not per-item the way `for`'s binding is — see
        // `Expr::Let`'s doc comment.
        Expr::Let {
            variable,
            value,
            body,
        } => {
            let bound = evaluate(value, context)?;
            let mut scope = context.variables.clone();
            scope.bind(variable.to_string(), bound);
            let inner = EvalContext {
                variables: &scope,
                ..context.clone()
            };
            evaluate(body, &inner)
        }

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

        Expr::InlineFunction { params, body } => Ok(inline_function_value(params, body, context)),
        Expr::NamedFunctionRef { name, arity } => Ok(named_function_value(name, *arity)),
        Expr::DynamicCall { function, args } => evaluate_dynamic_call(function, args, context),
        // Pure sugar — see the doc comment on `Expr::Arrow`. Whatever call
        // the parser built (`Expr::Function` or `Expr::DynamicCall`)
        // evaluates exactly as it would if written directly.
        Expr::Arrow(called) => evaluate(called, context),
    }
}

/// Builds the closure an [`Expr::InlineFunction`] evaluates to.
///
/// `body`'s `Arc` is cloned, not the tree it points to — see the field's
/// doc comment in `ast.rs`. `captured` is the lexical environment now, by
/// value, so the closure is good for as long as it is held, independent of
/// how long this evaluation lasts.
fn inline_function_value(
    params: &[String],
    body: &std::sync::Arc<Expr>,
    context: &EvalContext<'_>,
) -> Value {
    Value::Sequence(vec![Item::Function(FunctionItem::Inline {
        params: params.to_vec(),
        body: body.clone(),
        captured: context.variables.clone(),
    })])
}

/// Builds the reference an [`Expr::NamedFunctionRef`] evaluates to.
fn named_function_value(name: &str, arity: usize) -> Value {
    Value::Sequence(vec![Item::Function(FunctionItem::Named {
        name: name.to_string(),
        arity,
    })])
}

/// Evaluates an [`Expr::DynamicCall`]: the target to a function item, each
/// argument, and then the call itself.
fn evaluate_dynamic_call(
    function: &Expr,
    args: &[Expr],
    context: &EvalContext<'_>,
) -> Result<Value, EvalError> {
    let function_value = evaluate(function, context)?;
    let item = single_function_item(&function_value)?;
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        values.push(evaluate(arg, context)?);
    }
    call_function_item(item, values, context)
}

/// Extracts the one function item a dynamic call's target must evaluate to.
///
/// Anything else — zero items, more than one, or an item that is not a
/// function — is an error naming what was found instead, the same
/// discipline [`compare_items`] applies to a mismatched comparison operand.
fn single_function_item(value: &Value) -> Result<&FunctionItem, EvalError> {
    match value.as_sequence() {
        Some([Item::Function(function)]) => Ok(function),
        Some([other]) => Err(EvalError::new(format!(
            "a dynamic call's target must be a function item, not a {}",
            other.type_name()
        ))),
        Some(items) => Err(EvalError::new(format!(
            "a dynamic call's target must be exactly one function item, but there \
             are {}",
            items.len()
        ))),
        None => Err(EvalError::new(format!(
            "a dynamic call's target must be a function item, not a {}",
            value.type_name()
        ))),
    }
}

/// Invokes a function item with already-evaluated arguments.
///
/// A [`FunctionItem::Named`] dispatches through the ordinary built-in
/// function table — the reference does not close over anything, so there is
/// nothing to restore afterward. A [`FunctionItem::Inline`] evaluates its
/// body against the environment it captured when written, extended with the
/// arguments bound to its parameters — lexical scoping, not the dynamic
/// scoping every other construct in this engine uses, because a closure
/// that saw its *caller's* variables would not be a closure.
///
/// Every field of `context` other than `variables` — the document, the
/// current node, the clock, the version — comes from the *caller's*
/// context, not from wherever the function item was written. XPath has no
/// notion of a function item outliving the run that created it, so this is
/// never observable: only `variables` is genuinely lexical here.
pub(crate) fn call_function_item(
    function: &FunctionItem,
    args: Vec<Value>,
    context: &EvalContext<'_>,
) -> Result<Value, EvalError> {
    if args.len() != function.arity() {
        return Err(EvalError::new(format!(
            "a function item declared with {} parameter(s) was called with {}",
            function.arity(),
            args.len()
        )));
    }
    match function {
        FunctionItem::Named { name, .. } => functions::call(name, &args, context),
        FunctionItem::Inline {
            params,
            body,
            captured,
        } => {
            let mut scope = captured.clone();
            for (param, value) in params.iter().zip(args) {
                scope.bind(param.clone(), value);
            }
            let inner = EvalContext {
                variables: &scope,
                ..context.clone()
            };
            evaluate(body, &inner)
        }
    }
}

/// `for-each($sequence, $action)`: applies `action` to each item and
/// concatenates the results.
///
/// A multiplying construct exactly like `for`, so it shares the same
/// budget: `action` runs once per item, and if `action` itself contains a
/// nested `for-each`, a range, or a `for`, the product is what the shared
/// budget bounds — see [`SequenceScope`]. Kept in `eval.rs` rather than
/// `functions.rs`, where the rest of the XPath 2.0/3.0 function library
/// lives, because it needs that machinery directly.
pub(crate) fn for_each(
    items: Vec<Item>,
    action: &FunctionItem,
    context: &EvalContext<'_>,
) -> Result<Value, EvalError> {
    let _scope = SequenceScope::enter()?;
    spend_len(items.len())?;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let result = call_function_item(action, vec![item_to_value(item)], context)?;
        out.push(result);
    }
    Ok(Value::Sequence(flatten_into_sequence(out)))
}

/// `filter($sequence, $predicate)`: keeps the items for which `predicate`'s
/// effective boolean value holds, in order.
///
/// A multiplying construct exactly like `for-each`, so it shares the same
/// budget — see [`SequenceScope`] and `for_each`'s doc comment, which this
/// mirrors.
pub(crate) fn filter(
    items: Vec<Item>,
    predicate: &FunctionItem,
    context: &EvalContext<'_>,
) -> Result<Value, EvalError> {
    let _scope = SequenceScope::enter()?;
    spend_len(items.len())?;
    let mut kept = Vec::new();
    for item in items {
        let holds = call_function_item(predicate, vec![item_to_value(item.clone())], context)?
            .effective_boolean_value()
            .map_err(EvalError::new)?;
        if holds {
            kept.push(item);
        }
    }
    Ok(Value::Sequence(kept))
}

/// `fold-left($sequence, $zero, $f)`: combines the sequence from the left,
/// `f(f(f(zero, i1), i2), i3)…` — `$zero` need not be a single item, so it
/// is threaded through as a whole [`Value`] rather than an [`Item`].
///
/// Shares [`SequenceScope`]'s budget the same way `for_each` does.
pub(crate) fn fold_left(
    items: Vec<Item>,
    zero: Value,
    f: &FunctionItem,
    context: &EvalContext<'_>,
) -> Result<Value, EvalError> {
    let _scope = SequenceScope::enter()?;
    spend_len(items.len())?;
    let mut accumulator = zero;
    for item in items {
        accumulator = call_function_item(f, vec![accumulator, item_to_value(item)], context)?;
    }
    Ok(accumulator)
}

/// `fold-right($sequence, $zero, $f)`: combines the sequence from the
/// right, `f(i1, f(i2, f(i3, zero)))` — the mirror image of `fold_left`,
/// including which argument is the running accumulator.
///
/// Shares [`SequenceScope`]'s budget the same way `for_each` does.
pub(crate) fn fold_right(
    items: Vec<Item>,
    zero: Value,
    f: &FunctionItem,
    context: &EvalContext<'_>,
) -> Result<Value, EvalError> {
    let _scope = SequenceScope::enter()?;
    spend_len(items.len())?;
    let mut accumulator = zero;
    for item in items.into_iter().rev() {
        accumulator = call_function_item(f, vec![item_to_value(item), accumulator], context)?;
    }
    Ok(accumulator)
}

/// `for-each-pair($sequence1, $sequence2, $f)`: applies `f` to
/// corresponding items of both sequences and concatenates the results,
/// stopping at the shorter sequence — F&O does not treat the length
/// mismatch as an error.
///
/// Shares [`SequenceScope`]'s budget the same way `for_each` does, charged
/// by the number of pairs actually formed, not by either input's own
/// length.
pub(crate) fn for_each_pair(
    a: Vec<Item>,
    b: Vec<Item>,
    f: &FunctionItem,
    context: &EvalContext<'_>,
) -> Result<Value, EvalError> {
    let _scope = SequenceScope::enter()?;
    let pairs = a.len().min(b.len());
    spend_len(pairs)?;
    let mut out = Vec::with_capacity(pairs);
    for (x, y) in a.into_iter().zip(b) {
        let result = call_function_item(f, vec![item_to_value(x), item_to_value(y)], context)?;
        out.push(result);
    }
    Ok(Value::Sequence(flatten_into_sequence(out)))
}

/// `E1 ! E2` — XPath 3.0's simple map operator: evaluates `E2` once for
/// each item of `E1`, with that item as the context item, and concatenates
/// the results.
///
/// A multiplying construct exactly like `for-each`, so it shares the same
/// budget — see [`SequenceScope`]. Per XPath 3.0 section 3.14, each item's
/// position in `E1`'s result is the context *position* and that result's
/// length is the context *size*, so `(10, 20, 30) ! position()` is
/// `(1, 2, 3)`.
///
/// The interesting part is what "the context item" means for `.` when the
/// item isn't a node. A node moves `context.node` exactly the way `for`'s
/// per-iteration binding would if `for` rebound the context item instead
/// of a variable — ordinary, and every axis and predicate already knows
/// how to read it. An atomic value or a function item has no node to
/// become, so it goes into [`EvalContext::context_item`] instead, which
/// only the `Expr::Path` arm of `evaluate` reads, and only to resolve a
/// bare `.` — a step needing a real axis (`child::x`, `@a`, even `..`) is
/// a dynamic error there, precisely because there is no node to walk from,
/// the same way it would be in real XPath 3.0.
fn evaluate_simple_map(
    left: &Expr,
    right: &Expr,
    context: &EvalContext<'_>,
) -> Result<Value, EvalError> {
    let _scope = SequenceScope::enter()?;
    let items = evaluate(left, context)?.into_items();
    spend_len(items.len())?;
    let size = items.len();
    let mut out = Vec::with_capacity(size);
    // XPath 3.0 section 3.14: each item provides an inner focus, with its
    // position in the sequence as the context position and the sequence's
    // length as the context size.
    for (position, item) in (1..).zip(items) {
        let inner = match item {
            Item::Node(node) => context.focus(node, position, size),
            other => EvalContext {
                context_item: Some(other),
                position,
                size,
                ..context.clone()
            },
        };
        out.push(evaluate(right, &inner)?);
    }
    Ok(Value::Sequence(flatten_into_sequence(out)))
}

// Keeping the exhaustive operator classification and evaluation together
// makes a newly added `BinaryOp` fail compilation instead of silently taking
// the wrong eager or short-circuit path.
#[allow(clippy::too_many_lines)]
fn evaluate_binary(
    op: BinaryOp,
    left: &Expr,
    right: &Expr,
    context: &EvalContext<'_>,
) -> Result<Value, EvalError> {
    enum Eager {
        Union,
        NodeComparison(BinaryOp),
        Concat,
        ValueComparison(BinaryOp),
        Equality(BinaryOp),
        Relational(BinaryOp),
        Arithmetic(BinaryOp),
    }

    // `and` and `or` short-circuit, which matters when the right operand
    // would raise an error on nodes the left operand has already ruled out.
    let eager = match op {
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
        // `!` evaluates `right` once per item of `left`, each with its own
        // shifted context, rather than evaluating both sides once against
        // the shared one — genuinely different from every operator below,
        // not just short-circuited the way `and`/`or` are.
        BinaryOp::SimpleMap => return evaluate_simple_map(left, right, context),
        BinaryOp::Union => Eager::Union,
        BinaryOp::NodeIs | BinaryOp::NodeBefore | BinaryOp::NodeAfter => Eager::NodeComparison(op),
        BinaryOp::Concat => Eager::Concat,
        BinaryOp::ValueEqual
        | BinaryOp::ValueNotEqual
        | BinaryOp::ValueLess
        | BinaryOp::ValueLessEqual
        | BinaryOp::ValueGreater
        | BinaryOp::ValueGreaterEqual => Eager::ValueComparison(op),
        BinaryOp::Equal | BinaryOp::NotEqual => Eager::Equality(op),
        BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
            Eager::Relational(op)
        }
        BinaryOp::Add
        | BinaryOp::Subtract
        | BinaryOp::Multiply
        | BinaryOp::Divide
        | BinaryOp::Modulo => Eager::Arithmetic(op),
    };

    let left = evaluate(left, context)?;
    let right = evaluate(right, context)?;
    let document = context.document;

    match eager {
        Eager::Union => match (left, right) {
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

        Eager::NodeComparison(op) => compare_by_node(op, &left, &right, document),

        Eager::Concat => evaluate_concat(BinaryOp::Concat, &left, &right, document),

        Eager::ValueComparison(op) => {
            compare_by_value(op, &left, &right, document, context.implicit_timezone)
        }

        Eager::Equality(op) => {
            reject_function_item(op.as_str(), &left, &right)?;
            let want_equal = op == BinaryOp::Equal;
            // XPath 2.0: a comparison involving a date compares instants, not
            // strings, so an offset is honoured and an untyped operand is
            // cast. Unreachable under XPath 1.0, which has no temporal type.
            if has_temporal(&left) || has_temporal(&right) {
                return compare_temporals(op, &left, &right, context);
            }
            if let Some(ordering) = decimal::compare(&left, &right) {
                return Ok(Value::Boolean((ordering == Ordering::Equal) == want_equal));
            }
            Ok(Value::Boolean(compare_equality(
                &left, &right, document, want_equal,
            )))
        }

        Eager::Relational(op) => {
            reject_function_item(op.as_str(), &left, &right)?;
            if has_temporal(&left) || has_temporal(&right) {
                return compare_temporals(op, &left, &right, context);
            }
            if let Some(ordering) = decimal::compare(&left, &right) {
                return Ok(Value::Boolean(ordering_satisfies(op, ordering)));
            }
            Ok(Value::Boolean(compare_relational(
                op, &left, &right, document,
            )))
        }

        Eager::Arithmetic(op)
            if matches!(op, BinaryOp::Add | BinaryOp::Subtract)
                && (lone_temporal(&left).is_some()
                    || lone_duration(&left).is_some()
                    || lone_duration(&right).is_some()) =>
        {
            temporal_arithmetic(op, &left, &right)
        }

        Eager::Arithmetic(op)
            if matches!(op, BinaryOp::Multiply | BinaryOp::Divide)
                && (lone_duration(&left).is_some() || lone_duration(&right).is_some()) =>
        {
            scale_duration(op, &left, &right, document)
        }

        Eager::Arithmetic(op) => {
            // Two exact operands stay exact, with XPath 2.0's promotion.
            if let (Some(a), Some(b)) = (Exact::of_value(&left), Exact::of_value(&right)) {
                return decimal::arithmetic(op, a, b).map(Exact::into_value);
            }
            let a = left.to_number(document);
            let b = right.to_number(document);
            let result = match op {
                BinaryOp::Add => a + b,
                BinaryOp::Subtract => a - b,
                BinaryOp::Multiply => a * b,
                // IEEE division: no error on zero, the result is an infinity
                // or NaN, exactly as XPath 1.0 requires.
                BinaryOp::Divide => a / b,
                _ => a % b,
            };
            Ok(Value::Number(
                result,
                // Any non-exact operand promotes the result to `xs:double`.
                // See "Exact decimal arithmetic" in `spec/xpath2/`.
                NumericType::Double,
            ))
        }
    }
}

/// `||`, split out to keep `evaluate_binary` under the line-count lint.
///
/// Atomizes both operands to their string value — the same conversion
/// `concat()` applies to each of its arguments — and rejects a function
/// item explicitly rather than silently stringifying it to nothing, same
/// as every other operator that atomizes.
fn evaluate_concat(
    op: BinaryOp,
    left: &Value,
    right: &Value,
    document: &Document,
) -> Result<Value, EvalError> {
    reject_function_item(op.as_str(), left, right)?;
    Ok(Value::String(format!(
        "{}{}",
        left.to_xpath_string(document),
        right.to_xpath_string(document)
    )))
}

/// Rejects a function item as either operand of a general comparison.
///
/// `=`, `!=`, `<`, `<=`, `>`, `>=` all atomize their operands, and `eq` and
/// its family already get this for free from [`compare_items`]'s exhaustive
/// match — a function item simply matches no arm, and the catch-all names
/// both types. General comparison's own path (`compare_equality`,
/// `compare_relational`) returns a plain `bool` with no error path of its
/// own, so it needs the check made explicit here instead.
fn reject_function_item(op: &str, left: &Value, right: &Value) -> Result<(), EvalError> {
    if left.contains_function_item() || right.contains_function_item() {
        return Err(EvalError::new(format!(
            "`{op}` cannot compare a function item; it has no value to compare"
        )));
    }
    Ok(())
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
            a.iter().any(|left| b.iter().any(|right| test(left, right)))
        }

        (Value::NodeSet(a), Value::NodeSet(b)) => {
            let b_strings: Vec<String> = b.iter().map(|&n| document.string_value(n)).collect();
            a.iter().any(|&node| {
                let a_string = document.string_value(node);
                b_strings.iter().any(|b_string| test(&a_string, b_string))
            })
        }

        (Value::NodeSet(nodes), Value::Boolean(b)) | (Value::Boolean(b), Value::NodeSet(nodes)) => {
            // Comparing a node-set to a boolean converts the node-set to a
            // boolean first — that is, to whether it is non-empty — and does
            // not look at the nodes' values at all.
            (nodes.is_empty() != *b) == want_equal
        }
        (Value::NodeSet(nodes), Value::Number(n, _))
        | (Value::Number(n, _), Value::NodeSet(nodes)) => nodes.iter().any(|&node| {
            let value = super::value::parse_number(&document.string_value(node));
            // NaN is never equal to anything, so `!=` against a
            // non-numeric string is true, and `=` is false.
            (value == *n) == want_equal
        }),
        (Value::NodeSet(nodes), Value::String(s)) | (Value::String(s), Value::NodeSet(nodes)) => {
            nodes
                .iter()
                .any(|&node| test(&document.string_value(node), s))
        }

        // Neither side is a node-set: boolean wins, then number, then string.
        (a, b) => {
            if matches!(a, Value::Boolean(_)) || matches!(b, Value::Boolean(_)) {
                (a.to_boolean() == b.to_boolean()) == want_equal
            } else if matches!(a, Value::Number(_, _)) || matches!(b, Value::Number(_, _)) {
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

    // XPath 1.0 section 3.4: when one side is a node-set and the other a
    // boolean, the node-set is converted with `boolean()` — that is, to
    // whether it is non-empty — and the two booleans are then compared as
    // numbers. It is *not* existential over the nodes, and the difference
    // shows when the node-set is empty: `z >= false()` is `0 >= 0`, true,
    // whereas an existential walk over no nodes yields false.
    let as_boolean = |value: &Value, other: &Value| -> Option<Value> {
        match (value, other) {
            (Value::NodeSet(nodes), Value::Boolean(_)) => Some(Value::Boolean(!nodes.is_empty())),
            _ => None,
        }
    };
    let converted_left = as_boolean(left, right);
    let converted_right = as_boolean(right, left);
    let left = converted_left.as_ref().unwrap_or(left);
    let right = converted_right.as_ref().unwrap_or(right);

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
    let _scope = SequenceScope::enter()?;
    let items = evaluate(input, context)?.into_items();
    spend_len(items.len())?;
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
            ..context.clone()
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
    // `some`/`every` do not accumulate a sequence, but they iterate one, and
    // nesting them multiplies the iterations exactly as `for` does.
    let _scope = SequenceScope::enter()?;
    let items = evaluate(input, context)?.into_items();
    spend_len(items.len())?;
    let mut scope = context.variables.clone();
    let mark = scope.mark();
    let name = variable.to_string();

    for item in items {
        scope.truncate(mark);
        scope.bind(name.clone(), item_to_value(item));
        let inner = EvalContext {
            variables: &scope,
            ..context.clone()
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
        // Preserves the tag: a `for`-bound integer literal is still one.
        Item::Number(number, ty) => Value::Number(number, ty),
        Item::Boolean(boolean) => Value::Boolean(boolean),
        // Neither a temporal, a duration, nor a function item has a scalar
        // `Value` of its own; XPath 2.0 treats every value as a sequence,
        // and these are one-item ones.
        Item::Temporal(_) | Item::Duration(_) | Item::Function(_) => Value::Sequence(vec![item]),
    }
}

/// A general comparison where either side holds a date, dateTime, or time.
///
/// Compares instants rather than strings, so a timezone offset is honoured,
/// and casts an untyped operand to the other side's type — which is what lets
/// `@ContractDate < current-date()` be written without a constructor.
fn compare_temporals(
    op: BinaryOp,
    left: &Value,
    right: &Value,
    context: &EvalContext<'_>,
) -> Result<Value, EvalError> {
    use std::cmp::Ordering;

    let kind = temporal_kind_of(left, right);
    let a = temporal_operands(left, kind, context.document)?;
    let b = temporal_operands(right, kind, context.document)?;
    let zone = context.implicit_timezone;

    let holds = a.iter().any(|x| {
        b.iter().any(|y| {
            let ordering = x.compare_in(y, zone);
            match op {
                BinaryOp::Equal => ordering == Some(Ordering::Equal),
                BinaryOp::NotEqual => ordering != Some(Ordering::Equal),
                BinaryOp::Less => ordering == Some(Ordering::Less),
                BinaryOp::LessEqual => {
                    matches!(ordering, Some(Ordering::Less | Ordering::Equal))
                }
                BinaryOp::Greater => ordering == Some(Ordering::Greater),
                BinaryOp::GreaterEqual => {
                    matches!(ordering, Some(Ordering::Greater | Ordering::Equal))
                }
                _ => unreachable!("caller limits the operators"),
            }
        })
    });
    Ok(Value::Boolean(holds))
}

/// Multiplying or dividing a duration.
///
/// A duration scaled by a number stays a duration; a duration divided by
/// another of the same subtype is a number — how many of the second fit in
/// the first. Anything else is a type error.
///
/// Months are scaled as whole months, rounding half away from zero, because
/// XPath 2.0 defines `xs:yearMonthDuration` as an integer number of months
/// and there is no such thing as half a month.
fn scale_duration(
    op: BinaryOp,
    left: &Value,
    right: &Value,
    document: &Document,
) -> Result<Value, EvalError> {
    let item = |item: Item| Ok(Value::Sequence(vec![item]));
    let divide = op == BinaryOp::Divide;

    // duration div duration, of the same subtype, gives a plain number.
    if let (Some(a), Some(b)) = (lone_duration(left), lone_duration(right)) {
        if !divide {
            return Err(EvalError::new(
                "two durations cannot be multiplied; divide them to ask how many \
                 of one fit in the other",
            ));
        }
        if a.kind() != b.kind() {
            return Err(EvalError::new(format!(
                "cannot divide a {} by a {}",
                a.kind().as_str(),
                b.kind().as_str()
            )));
        }
        return Ok(Value::Number(
            match a.kind() {
                DurationKind::YearMonth => as_f64_i64(a.to_months())? / as_f64_i64(b.to_months())?,
                DurationKind::DayTime => a.to_seconds() / b.to_seconds(),
            },
            NumericType::Double,
        ));
    }

    // Otherwise one side is a duration and the other must be a number.
    let (duration, factor) = match (lone_duration(left), lone_duration(right)) {
        (Some(duration), None) => (duration, right.to_number(document)),
        // `2 * P1D` is legal; `2 div P1D` is not.
        (None, Some(duration)) if !divide => (duration, left.to_number(document)),
        _ => {
            return Err(EvalError::new(format!(
                "`{}` has no meaning for these operands; a duration may be \
                 multiplied or divided by a number",
                op.as_str()
            )))
        }
    };

    if factor.is_nan() {
        return Err(EvalError::new("a duration can only be scaled by a number"));
    }
    let factor = if divide { 1.0 / factor } else { factor };

    item(Item::Duration(match duration.kind() {
        DurationKind::YearMonth => {
            let months = as_f64_i64(duration.to_months())? * factor;
            // `NaN` was refused above; an infinite or out of range result has no
            // month count, and no fallback is declared for it (RCA-DECL-5).
            let rounded = numeric::f64_to_i64(months.abs().round().copysign(months))
                .ok_or_else(month_overflow)?;
            Duration::from_months(rounded)
        }
        DurationKind::DayTime => Duration::from_seconds(duration.to_seconds() * factor),
    }))
}

/// Converts a month count to `f64`, refusing one above 2^53 where it would round.
fn as_f64_i64(value: i64) -> Result<f64, EvalError> {
    numeric::i64_to_f64(value).ok_or_else(month_overflow)
}

/// The refusal for date or time arithmetic whose result cannot be represented.
fn temporal_range() -> EvalError {
    EvalError::new("the result is outside the range of dates and times this engine represents")
}

/// The refusal for a month count that does not fit the arithmetic done on it.
fn month_overflow() -> EvalError {
    EvalError::new("a year-month duration is too large for this arithmetic")
}

/// `instance of`, `castable as`, `cast as`, `treat as`.
fn evaluate_type_op(
    op: TypeOp,
    value: &Value,
    sequence_type: &SequenceType,
    context: &EvalContext<'_>,
) -> Result<Value, EvalError> {
    let items = value.clone().into_items();

    match op {
        TypeOp::InstanceOf => Ok(Value::Boolean(matches_sequence_type(
            &items,
            sequence_type,
            context,
        ))),

        TypeOp::TreatAs => {
            if matches_sequence_type(&items, sequence_type, context) {
                return Ok(value.clone());
            }
            Err(EvalError::new(format!(
                "`treat as {}` was given a value that is not one",
                sequence_type.as_written()
            )))
        }

        TypeOp::CastableAs | TypeOp::CastAs => {
            let casting = op == TypeOp::CastAs;
            // An empty operand is false for `castable as`, and the empty
            // sequence for `cast as` — but only when the type admits it.
            if items.is_empty() {
                if sequence_type.occurrence == Occurrence::ZeroOrOne {
                    return Ok(if casting {
                        Value::Sequence(Vec::new())
                    } else {
                        Value::Boolean(true)
                    });
                }
                return if casting {
                    Err(EvalError::new(format!(
                        "`cast as {}` was given nothing to cast; write `{}?` to \
                         allow that",
                        sequence_type.as_written(),
                        sequence_type.as_written()
                    )))
                } else {
                    Ok(Value::Boolean(false))
                };
            }
            let [item] = items.as_slice() else {
                return if casting {
                    Err(EvalError::new(format!(
                        "`cast as {}` takes one value, and was given {}",
                        sequence_type.as_written(),
                        items.len()
                    )))
                } else {
                    Ok(Value::Boolean(false))
                };
            };

            let ItemType::Atomic(name) = &sequence_type.item_type else {
                return Err(EvalError::new(format!(
                    "`{}` takes an atomic type",
                    op.as_str()
                )));
            };

            match cast_item(item, name, context.document) {
                Ok(cast) => Ok(if casting {
                    Value::Sequence(vec![cast])
                } else {
                    Value::Boolean(true)
                }),
                Err(message) => {
                    if casting {
                        Err(EvalError::new(message))
                    } else {
                        Ok(Value::Boolean(false))
                    }
                }
            }
        }
    }
}

/// Whether a sequence matches a sequence type.
fn matches_sequence_type(
    items: &[Item],
    sequence_type: &SequenceType,
    context: &EvalContext<'_>,
) -> bool {
    if sequence_type.item_type == ItemType::EmptySequence {
        return items.is_empty();
    }
    if !sequence_type.occurrence.admits(items.len()) {
        return false;
    }
    items
        .iter()
        .all(|item| matches_item_type(item, &sequence_type.item_type, context))
}

/// Whether one item matches an item type.
fn matches_item_type(item: &Item, item_type: &ItemType, context: &EvalContext<'_>) -> bool {
    match item_type {
        ItemType::AnyItem => true,
        ItemType::EmptySequence => false,

        ItemType::Node { kind, name } => {
            let Item::Node(node) = item else {
                return false;
            };
            let document = context.document;
            if let Some(wanted) = kind {
                if document.kind(*node) != *wanted {
                    return false;
                }
            }
            match name {
                None => true,
                Some(wanted) => {
                    let uri = match &wanted.prefix {
                        Some(prefix) => match context.namespaces.resolve(prefix) {
                            Some(uri) => Some(uri),
                            None => return false,
                        },
                        None => None,
                    };
                    document
                        .name(*node)
                        .is_some_and(|actual| actual.matches_parts(uri, &wanted.local))
                }
            }
        }

        ItemType::Atomic(name) => {
            let local = name.rsplit(':').next().unwrap_or(name);
            match item {
                // A node is not an atomic value; it would have to be
                // atomized first, which `instance of` does not do. A
                // function item is not atomic either, for the same reason a
                // node isn't: it has no typed value. Real XPath 3.0 tests
                // one with its own item type, `function(*)`, which this
                // phase does not implement, so no `instance of` target
                // matches a function item at all.
                Item::Node(_) | Item::Function(_) => false,
                Item::String(_) => matches!(local, "string" | "anyAtomicType" | "untypedAtomic"),
                Item::Boolean(_) => matches!(local, "boolean" | "anyAtomicType"),
                Item::Number(_, ty) => numeric_type_matches(*ty, local),
                Item::Temporal(temporal) => {
                    local == "anyAtomicType"
                        || local
                            == temporal
                                .kind()
                                .as_str()
                                .rsplit(':')
                                .next()
                                .unwrap_or_default()
                }
                Item::Duration(duration) => {
                    local == "anyAtomicType"
                        || local == "duration"
                        || local
                            == duration
                                .kind()
                                .as_str()
                                .rsplit(':')
                                .next()
                                .unwrap_or_default()
                }
            }
        }
    }
}

/// Whether a numeric item's tracked type matches an atomic type name.
///
/// `xs:integer` derives from `xs:decimal` by restriction in XML Schema, so
/// an `Integer`-tagged value also matches `xs:decimal`; the reverse is not
/// true. `xs:float` and `xs:double` are separate primitive types, related to
/// neither `xs:decimal` nor each other, so each matches only itself. See
/// [`NumericType`] and `spec/xpath2/`.
fn numeric_type_matches(ty: NumericType, local: &str) -> bool {
    matches!(
        (ty, local),
        (_, "anyAtomicType")
            | (NumericType::Double, "double")
            | (NumericType::Float, "float")
            | (NumericType::Decimal(_) | NumericType::Integer(_), "decimal")
            | (
                NumericType::Integer(_),
                "integer" | "long" | "int" | "short" | "byte"
            )
    )
}

/// Casts one item to an atomic type, by its lexical form.
///
/// Reading the lexical form is what XML Schema specifies for untyped input,
/// and untyped is what a Schematron schema nearly always has in front of it.
pub(super) fn cast_item(item: &Item, type_name: &str, document: &Document) -> Result<Item, String> {
    let local = type_name.rsplit(':').next().unwrap_or(type_name);
    let text = item.to_xpath_string(document);
    let reject = || format!("{text:?} cannot be cast to {type_name}");

    Ok(match local {
        "string" | "untypedAtomic" | "anyAtomicType" => Item::String(text),
        "boolean" => match text.trim() {
            "true" | "1" => Item::Boolean(true),
            "false" | "0" => Item::Boolean(false),
            _ => return Err(reject()),
        },
        "double" | "float" => {
            let number = super::value::parse_number(&text);
            if number.is_nan() && text.trim() != "NaN" {
                return Err(reject());
            }
            let numeric_type = if local == "float" {
                NumericType::Float
            } else {
                NumericType::Double
            };
            Item::Number(number, numeric_type)
        }
        // Exact: the lexical form is read as a decimal, never through a
        // double, so `xs:decimal('0.1')` is exactly 0.1. A value too large
        // for the representation is refused, as XPath 2.0's FOCA0001 does.
        "decimal" => {
            let exact = super::decimal::parse_lexical(&text).ok_or_else(reject)?;
            super::decimal::Exact {
                value: exact,
                integer: false,
            }
            .into_item()
        }
        "integer" | "long" | "int" | "short" | "byte" => {
            let number = super::value::parse_number(&text);
            if number.is_nan() || number.fract() != 0.0 {
                return Err(reject());
            }
            let exact = super::decimal::parse_lexical(&text).ok_or_else(reject)?;
            super::decimal::Exact {
                value: exact.trunc().normalize(),
                integer: true,
            }
            .into_item()
        }
        // The parse errors carry the specific reason; the framing is added
        // here so that every failed cast reads the same way whatever the
        // target type.
        "date" => Item::Temporal(
            Temporal::parse(&text, TemporalKind::Date).map_err(|why| detailed(&reject(), &why))?,
        ),
        "dateTime" => Item::Temporal(
            Temporal::parse(&text, TemporalKind::DateTime)
                .map_err(|why| detailed(&reject(), &why))?,
        ),
        "time" => Item::Temporal(
            Temporal::parse(&text, TemporalKind::Time).map_err(|why| detailed(&reject(), &why))?,
        ),
        "dayTimeDuration" => Item::Duration(
            Duration::parse(&text, DurationKind::DayTime)
                .map_err(|why| detailed(&reject(), &why))?,
        ),
        "yearMonthDuration" => Item::Duration(
            Duration::parse(&text, DurationKind::YearMonth)
                .map_err(|why| detailed(&reject(), &why))?,
        ),
        other => return Err(format!("{other} is not a type this crate can cast to")),
    })
}

/// Joins a cast's framing to the specific reason it failed.
fn detailed(framing: &str, why: &str) -> String {
    format!("{framing}: {why}")
}

/// XPath 2.0's node comparisons: `is`, `<<`, `>>`.
///
/// `is` asks about identity, not content: two elements with the same string
/// value are equal by `=` and are not the same node. `<<` and `>>` ask about
/// document order.
///
/// Each takes exactly one node on either side. An empty operand yields the
/// empty sequence, so the comparison is false; anything else is a type error,
/// the same strictness as the value comparisons and for the same reason.
fn compare_by_node(
    op: BinaryOp,
    left: &Value,
    right: &Value,
    document: &Document,
) -> Result<Value, EvalError> {
    let node_of = |value: &Value, side: &str| -> Result<Option<NodeId>, EvalError> {
        let items = value.clone().into_items();
        match items.as_slice() {
            [] => Ok(None),
            [Item::Node(node)] => Ok(Some(*node)),
            [other] => Err(EvalError::new(format!(
                "the {side} operand of `{}` is a {}, and a node comparison takes a \
                 node",
                op.as_str(),
                other.type_name()
            ))),
            many => Err(EvalError::new(format!(
                "the {side} operand of `{}` has {} nodes, and a node comparison \
                 takes exactly one",
                op.as_str(),
                many.len()
            ))),
        }
    };

    let Some(a) = node_of(left, "left")? else {
        // Nothing to compare, so nothing is claimed.
        return Ok(Value::Sequence(Vec::new()));
    };
    let Some(b) = node_of(right, "right")? else {
        // Nothing to compare, so nothing is claimed.
        return Ok(Value::Sequence(Vec::new()));
    };

    Ok(Value::Boolean(match op {
        BinaryOp::NodeIs => a == b,
        BinaryOp::NodeBefore => document.order(a) < document.order(b),
        BinaryOp::NodeAfter => document.order(a) > document.order(b),
        _ => unreachable!("caller limits the operators"),
    }))
}

/// The single temporal a value holds, if that is all it holds.
fn lone_temporal(value: &Value) -> Option<Temporal> {
    match value.as_sequence()? {
        [Item::Temporal(temporal)] => Some(*temporal),
        _ => None,
    }
}

/// The single duration a value holds, if that is all it holds.
fn lone_duration(value: &Value) -> Option<Duration> {
    match value.as_sequence()? {
        [Item::Duration(duration)] => Some(*duration),
        _ => None,
    }
}

/// Arithmetic involving a date or a duration.
///
/// Subtracting two dates measures the distance between them; adding a
/// duration to a date moves it. Anything else — a date plus a date, or two
/// durations of different subtypes — is a type error rather than a number,
/// because XPath 2.0 gives it no meaning and inventing one would hide a
/// mistake. See `spec/xpath2/`.
fn temporal_arithmetic(op: BinaryOp, left: &Value, right: &Value) -> Result<Value, EvalError> {
    let subtract = op == BinaryOp::Subtract;
    let item = |item: Item| Ok(Value::Sequence(vec![item]));

    match (
        lone_temporal(left),
        lone_temporal(right),
        lone_duration(left),
        lone_duration(right),
    ) {
        // date - date, yielding the distance between them.
        (Some(a), Some(b), _, _) => {
            if !subtract {
                return Err(EvalError::new(format!(
                    "two {} values cannot be added; subtract them to measure the \
                     distance between them",
                    a.kind().as_str()
                )));
            }
            if a.kind() != b.kind() {
                return Err(EvalError::new(format!(
                    "cannot subtract a {} from a {}",
                    b.kind().as_str(),
                    a.kind().as_str()
                )));
            }
            // An instant too far from the epoch to be an exact `f64` has no
            // difference to report, and no fallback is declared for it.
            let (Some(a), Some(b)) = (a.to_seconds(), b.to_seconds()) else {
                return Err(temporal_range());
            };
            item(Item::Duration(Duration::from_seconds(a - b)))
        }

        // date +/- duration, moving the date.
        (Some(temporal), None, _, Some(duration)) => {
            let signed = if subtract { -1 } else { 1 };
            item(Item::Temporal(match duration.kind() {
                DurationKind::YearMonth => {
                    let months = duration
                        .to_months()
                        .checked_mul(i64::from(signed))
                        .ok_or_else(month_overflow)?;
                    add_months(&temporal, months).ok_or_else(temporal_range)?
                }
                DurationKind::DayTime => {
                    add_seconds(&temporal, duration.to_seconds() * f64::from(signed))
                        .ok_or_else(temporal_range)?
                }
            }))
        }

        // duration +/- duration, of the same subtype.
        (None, None, Some(a), Some(b)) => {
            if a.kind() != b.kind() {
                return Err(EvalError::new(format!(
                    "cannot combine a {} with a {}: the two are separate types \
                     because whether a month exceeds thirty days has no answer",
                    a.kind().as_str(),
                    b.kind().as_str()
                )));
            }
            let sign = if subtract { -1.0 } else { 1.0 };
            item(Item::Duration(match a.kind() {
                DurationKind::YearMonth => {
                    let signed = b
                        .to_months()
                        .checked_mul(if subtract { -1 } else { 1 })
                        .ok_or_else(month_overflow)?;
                    Duration::from_months(
                        a.to_months()
                            .checked_add(signed)
                            .ok_or_else(month_overflow)?,
                    )
                }
                DurationKind::DayTime => {
                    Duration::from_seconds(a.to_seconds() + b.to_seconds() * sign)
                }
            }))
        }

        // duration + date is legal in XPath 2.0; the operands commute.
        (None, Some(temporal), Some(duration), None) if !subtract => {
            item(Item::Temporal(match duration.kind() {
                DurationKind::YearMonth => {
                    add_months(&temporal, duration.to_months()).ok_or_else(temporal_range)?
                }
                DurationKind::DayTime => {
                    add_seconds(&temporal, duration.to_seconds()).ok_or_else(temporal_range)?
                }
            }))
        }

        _ => Err(EvalError::new(format!(
            "`{}` has no meaning for these operands; see spec/xpath2/ for the \
             date and duration arithmetic that is defined",
            op.as_str()
        ))),
    }
}

/// XPath 2.0's value comparisons: `eq`, `ne`, `lt`, `le`, `gt`, `ge`.
///
/// Where a general comparison asks whether *some* pair of items matches, a
/// value comparison compares exactly two values — and says so when it cannot.
/// Three rules follow, and each is a deliberate strictness rather than a
/// limitation:
///
/// - An empty operand yields the empty sequence, which is false in a boolean
///   position. Nothing to compare, so nothing is claimed.
/// - An operand of two or more items is a **type error**. `=` would quietly
///   pick whichever pair happened to match.
/// - Operands of different types are a **type error**. An untyped value — and
///   everything in an XML document is untyped — counts as a string, so
///   `@n eq 1` is an error even when `@n` is `"1"`. That is `eq` reporting
///   that the comparison written is not the one meant.
///
/// See `spec/xpath2/`.
fn compare_by_value(
    op: BinaryOp,
    left: &Value,
    right: &Value,
    document: &Document,
    implicit_timezone: i32,
) -> Result<Value, EvalError> {
    let left_items = left.clone().into_items();
    let right_items = right.clone().into_items();

    // An empty operand makes the whole comparison the empty sequence.
    if left_items.is_empty() || right_items.is_empty() {
        return Ok(Value::Sequence(Vec::new()));
    }

    let single = |items: &[Item], side: &str| -> Result<Item, EvalError> {
        match items {
            [only] => Ok(only.clone()),
            _ => Err(EvalError::new(format!(
                "the {side} operand of `{}` has {} items, and a value comparison \
                 takes exactly one; use `{}` for an existential comparison",
                op.as_str(),
                items.len(),
                general_counterpart(op)
            ))),
        }
    };

    let a = single(&left_items, "left")?;
    let b = single(&right_items, "right")?;
    let ordering = compare_items(&a, &b, op, document, implicit_timezone)?;

    Ok(Value::Boolean(match op {
        BinaryOp::ValueEqual => ordering == Some(std::cmp::Ordering::Equal),
        BinaryOp::ValueNotEqual => ordering != Some(std::cmp::Ordering::Equal),
        BinaryOp::ValueLess => ordering == Some(std::cmp::Ordering::Less),
        BinaryOp::ValueLessEqual => {
            matches!(
                ordering,
                Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
            )
        }
        BinaryOp::ValueGreater => ordering == Some(std::cmp::Ordering::Greater),
        BinaryOp::ValueGreaterEqual => {
            matches!(
                ordering,
                Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
            )
        }
        _ => unreachable!("caller limits the operators"),
    }))
}

/// Orders two atomic items of the same type.
///
/// `None` means the two are incomparable but not an error, which happens only
/// for NaN.
fn compare_items(
    a: &Item,
    b: &Item,
    op: BinaryOp,
    document: &Document,
    implicit_timezone: i32,
) -> Result<Option<std::cmp::Ordering>, EvalError> {
    // A node atomizes to its string value, which is untyped and therefore
    // compares as a string — the rule that makes `@n eq 1` an error.
    let atomize = |item: &Item| -> Item {
        match item {
            Item::Node(node) => Item::String(document.string_value(*node)),
            other => other.clone(),
        }
    };
    let (a, b) = (atomize(a), atomize(b));

    match (&a, &b) {
        (Item::String(x), Item::String(y)) => Ok(Some(x.cmp(y))),
        (Item::Number(_, tx), Item::Number(_, ty))
            if Exact::of(*tx).is_some() && Exact::of(*ty).is_some() =>
        {
            Ok(Exact::of(*tx)
                .zip(Exact::of(*ty))
                .map(|(x, y)| x.value.cmp(&y.value)))
        }
        (Item::Number(x, _), Item::Number(y, _)) => Ok(x.partial_cmp(y)),
        (Item::Boolean(x), Item::Boolean(y)) => Ok(Some(x.cmp(y))),
        (Item::Temporal(x), Item::Temporal(y)) if x.kind() == y.kind() => {
            Ok(x.compare_in(y, implicit_timezone))
        }
        (Item::Duration(x), Item::Duration(y)) if x.kind() == y.kind() => Ok(x.partial_cmp(y)),
        _ => Err(EvalError::new(format!(
            "`{}` compares a {} with a {}, and a value comparison requires the \
             same type on both sides. Everything in an XML document is untyped \
             and so counts as a string; cast it, or use `{}`, which coerces.",
            op.as_str(),
            a.type_name(),
            b.type_name(),
            general_counterpart(op)
        ))),
    }
}

/// A numeric literal's value. XPath 1.0 has one numeric type, so a 1.0
/// literal is a double whatever its lexical form; only 2.0 and later keep it
/// exact. See "Exact decimal arithmetic" in `spec/xpath2/`.
fn literal_number(value: f64, numeric_type: NumericType, context: &EvalContext<'_>) -> Value {
    if context.version.is_v2() {
        Value::Number(value, numeric_type)
    } else {
        Value::Number(value, NumericType::Double)
    }
}

/// Unary minus: an exact number keeps its type, anything else is a double.
fn negate(value: &Value, document: &Document) -> Value {
    if let Some(exact) = Exact::of_value(value) {
        decimal::negate(exact).into_value()
    } else {
        Value::Number(-value.to_number(document), NumericType::Double)
    }
}

/// Whether `ordering` satisfies a relational operator (`<`, `<=`, `>`, `>=`).
const fn ordering_satisfies(op: BinaryOp, ordering: Ordering) -> bool {
    match op {
        BinaryOp::Less => matches!(ordering, Ordering::Less),
        BinaryOp::LessEqual => !matches!(ordering, Ordering::Greater),
        BinaryOp::Greater => matches!(ordering, Ordering::Greater),
        _ => !matches!(ordering, Ordering::Less),
    }
}

/// The general comparison an error message should point the reader at.
const fn general_counterpart(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::ValueNotEqual => "!=",
        BinaryOp::ValueLess => "<",
        BinaryOp::ValueLessEqual => "<=",
        BinaryOp::ValueGreater => ">",
        BinaryOp::ValueGreaterEqual => ">=",
        // `eq`, and anything else this is ever called with.
        _ => "=",
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
/// A shared ceiling on the work a single expression may do in the constructs
/// that **multiply**.
///
/// `MAX_RANGE` bounds one range, and that is not enough: each range in
/// `for $i in 1 to 999 return for $j in 1 to 999 return for $k in 1 to 999
/// return $k` is comfortably under it, and the three together ask for close
/// to a billion items. A limit checked at each level separately can never
/// see that, because no level exceeds it. The budget is shared by every
/// nested construct in one expression, so the product is what is bounded.
///
/// Held in thread-local state rather than in [`EvalContext`], which is `Copy`
/// — a counter stored by value would be duplicated at exactly the nesting
/// points that need to share it. Evaluation is single-threaded within a
/// pattern, so a thread-local is the right scope, and it is touched only by
/// these rare constructs rather than on the hot path.
const MAX_SEQUENCE_WORK: u64 = 10_000_000;

thread_local! {
    static SEQUENCE_WORK: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static SEQUENCE_DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Enters a multiplying construct, starting a fresh budget at the outermost
/// one and restoring the depth on the way out however the body exits.
struct SequenceScope;

impl SequenceScope {
    fn enter() -> Result<Self, EvalError> {
        SEQUENCE_DEPTH.with(|depth| {
            // No declared fallback (RCA-DECL-5): a depth counter that cannot
            // count further is a refusal, not a depth that stays put.
            let deeper = depth.get().checked_add(1).ok_or_else(|| {
                EvalError::new("multiplying constructs are nested too deeply to be counted")
            })?;
            if depth.get() == 0 {
                SEQUENCE_WORK.with(|work| work.set(MAX_SEQUENCE_WORK));
            }
            depth.set(deeper);
            Ok(Self)
        })
    }
}

impl Drop for SequenceScope {
    fn drop(&mut self) {
        // Fallback: `OUTERMOST_DEPTH` (0) — a scope that closes with no scope
        // open leaves the outermost depth; the depth counter never goes below 0.
        SEQUENCE_DEPTH.with(|depth| {
            depth.set(depth.get().checked_sub(1).unwrap_or(OUTERMOST_DEPTH));
        });
    }
}

/// The depth of no open multiplying construct.
const OUTERMOST_DEPTH: u32 = 0;

/// The refusal for spending more than the shared budget holds.
fn over_budget() -> EvalError {
    EvalError::new(format!(
        "this expression asks for more than {MAX_SEQUENCE_WORK} items across \
         its nested ranges and `for` expressions; each one may be within the \
         limit while the product is not"
    ))
}

/// Charges `items` against the shared budget.
fn spend(items: u64) -> Result<(), EvalError> {
    SEQUENCE_WORK.with(|work| {
        let left = work.get().checked_sub(items).ok_or_else(over_budget)?;
        work.set(left);
        Ok(())
    })
}

/// Charges a collection's length against the shared budget.
fn spend_len(len: usize) -> Result<(), EvalError> {
    // `usize` is never wider than `u64` on a supported target; where it were, the
    // count would be more than the budget holds, so it is refused the same way.
    u64::try_from(len)
        .ok()
        .ok_or_else(over_budget)
        .and_then(spend)
}

const MAX_RANGE: f64 = 1_000_000.0;

/// The items of `from to to`, ascending.
///
/// A descending range is the empty sequence, which XPath 2.0 specifies. A
/// bound that is not a number is a type error rather than an empty result,
/// because silently yielding nothing would make a broken range look like an
/// empty one.
fn range_items(from: f64, to: f64) -> Result<Vec<Item>, EvalError> {
    if from.is_nan() || to.is_nan() {
        return Err(EvalError::new("the bounds of a `to` range must be numbers"));
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

    // A range opens a scope of its own, so a bare `1 to 999999` starts a
    // fresh budget rather than drawing on one that was never opened, and a
    // range inside a `for` shares that loop's budget.
    let _scope = SequenceScope::enter()?;
    // `to - from` is below `MAX_RANGE` here, so the count is a small whole number.
    let count = numeric::f64_to_u64(to - from + 1.0).ok_or_else(over_budget)?;
    spend(count)?;

    let mut items = Vec::new();
    let mut current = from;
    while current <= to {
        // XPath 2.0 types a range's items as `xs:integer`. `current` is
        // integral here; fallback: `xs:double`, for a bound beyond the
        // `i64` range, where no exact integer could be formed anyway.
        let numeric_type = numeric::f64_to_i64(current).map_or(NumericType::Double, |integer| {
            NumericType::Integer(rust_decimal::Decimal::from(integer))
        });
        items.push(Item::Number(current, numeric_type));
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

/// `Expr::Path`'s evaluation, split from the ordinary node-based
/// [`evaluate_path`] so that a non-node [`EvalContext::context_item`] — set
/// only while `!` is mapping over an item that isn't a node — is checked
/// exactly once, in exactly one place.
///
/// Only [`PathStart::Context`] depends on the context item at all: an
/// absolute path (`PathStart::Root`) starts from the document regardless
/// of it, and a path continuing from another expression
/// (`PathStart::Expr`) starts from that expression's own result — so both
/// fall through to [`evaluate_path`] unchanged even while an atomic
/// context item is active. A relative path does depend on it, and admits
/// exactly one shape when the context item isn't a node: a bare `.`
/// (`self::node()`, unpredicated, with no further steps) — anything
/// wanting an axis to walk is an error, because there is no node to walk
/// it from.
fn evaluate_path_or_context_item(
    path: &PathExpr,
    context: &EvalContext<'_>,
) -> Result<Value, EvalError> {
    if let Some(item) = &context.context_item {
        if matches!(path.start, PathStart::Context) {
            return if is_bare_context_item_step(&path.steps) {
                Ok(item_to_value(item.clone()))
            } else {
                Err(EvalError::new(format!(
                    "this step needs the context item to be a node, but it is a {} \
                     — the simple map operator `!` is mapping over a sequence that \
                     is not all nodes",
                    item.type_name()
                )))
            };
        }
    }
    Ok(Value::NodeSet(evaluate_path(path, context)?))
}

/// Whether `steps` is exactly what a bare `.` parses to: `self::node()`,
/// once, with no predicates. The only shape a relative path may take when
/// the context item is not a node — see [`evaluate_path_or_context_item`].
fn is_bare_context_item_step(steps: &[Step]) -> bool {
    matches!(
        steps,
        [Step {
            axis: Axis::SelfAxis,
            node_test: NodeTest::AnyNode,
            predicates,
        }] if predicates.is_empty()
    )
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

/// Whether the number `n` of a numeric predicate names the one-based `position`.
///
/// A position above 2^53 has no exact `f64` to compare with, so it is refused
/// rather than compared as a rounded number.
fn position_matches(n: f64, position: usize) -> Result<bool, EvalError> {
    numeric::usize_to_f64(position)
        .map(|position| (n - position).abs() < f64::EPSILON)
        .ok_or_else(|| EvalError::new("a context position is too large to compare with a number"))
}

/// Applies one predicate, giving each node the position it holds in the list.
fn filter_by_predicate(
    nodes: Vec<NodeId>,
    predicate: &Expr,
    context: &EvalContext<'_>,
) -> Result<Vec<NodeId>, EvalError> {
    let size = nodes.len();
    let mut kept = Vec::with_capacity(size);
    // Positions count from 1 and never pass `size`, so none can overflow.
    for (position, node) in (1..=size).zip(nodes) {
        let inner = context.focus(node, position, size);
        // A numeric predicate is shorthand for `position() = n`.
        let keep = evaluate(predicate, &inner).and_then(|value| match value {
            Value::Number(n, _) => position_matches(n, position),
            other => Ok(other.to_boolean()),
        })?;
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
        Axis::Descendant => document.descendants(node),
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
    if following {
        // Skip up to the node itself; the siblings follow it.
        children
            .iter()
            .copied()
            .skip_while(|&n| n != node)
            .skip(1)
            .collect()
    } else {
        // Reverse axis: nearest first.
        let mut before: Vec<NodeId> = children
            .iter()
            .copied()
            .take_while(|&n| n != node)
            .collect();
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

pub(crate) fn matches_node_test(
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
                    Some(wanted) => document.name(node).is_some_and(|n| &n.local == wanted),
                }
        }
        // A kind test names the kind outright, so unlike `*` it does not
        // depend on the axis: `child::attribute()` selects nothing, because
        // the child axis yields no attributes.
        NodeTest::Kind {
            kind: wanted,
            name: None,
        } => kind == *wanted,
        NodeTest::Kind {
            kind: wanted,
            name: Some(name),
        } => {
            if kind != *wanted {
                return false;
            }
            let uri = match &name.prefix {
                Some(prefix) => match context.namespaces.resolve(prefix) {
                    Some(uri) => Some(uri),
                    None => return false,
                },
                // As everywhere else here, an unprefixed name is in no
                // namespace; there is no default namespace to fall back on.
                None => None,
            };
            document
                .name(node)
                .is_some_and(|actual| actual.matches_parts(uri, &name.local))
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
            // An EQName (`Q{uri}local`) already carries its namespace URI —
            // nothing to resolve — and treats an empty braced URI literal
            // the same as no namespace, matching `Q{}local`'s own meaning.
            let uri = match &name.uri {
                Some(uri) if uri.is_empty() => None,
                Some(uri) => Some(uri.as_str()),
                None => match &name.prefix {
                    Some(prefix) => match context.namespaces.resolve(prefix) {
                        Some(uri) => Some(uri),
                        None => return false,
                    },
                    // XPath 1.0: an unprefixed name is in no namespace.
                    None => None,
                },
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
    use crate::xpath::{parse, Namespaces, Variables, XPathVersion};

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

    /// Evaluates under `version`, returning the error message if any.
    fn eval_error(version: XPathVersion, expression: &str) -> Option<String> {
        let document = Document::from_str(DOC).unwrap();
        let expr = parse(expression).unwrap();
        let variables = Variables::new();
        let namespaces = Namespaces::new();
        let context = EvalContext::new(
            &document,
            document.document_element().unwrap(),
            &variables,
            &namespaces,
        )
        .with_version(version);
        evaluate(&expr, &context).err().map(|e| e.message)
    }

    #[test]
    fn functions_from_a_later_version_are_unknown_at_runtime() {
        // The compiler refuses these first; a direct evaluate() call must too.
        let v1 = eval_error(XPathVersion::V1, "upper-case('a')").unwrap();
        assert!(v1.contains("is an XPath 2.0 function"), "{v1}");
        let v2 = eval_error(XPathVersion::V2, "for-each((), ())").unwrap();
        assert!(v2.contains("XPath 3.0"), "{v2}");
        assert_eq!(eval_error(XPathVersion::V2, "upper-case('a')"), None);
    }

    #[test]
    fn following_and_preceding_stop_at_a_subtree_boundary() {
        // `d` is the last node of `c`'s subtree and of `a`'s.
        assert_eq!(number(DOC, "count(c/following::node())"), 0.0);
        assert_eq!(number(DOC, "count(c/d/preceding::*)"), 2.0);
        assert_eq!(number(DOC, "count(b[1]/following::*)"), 3.0);
    }

    #[test]
    fn axes_stop_exactly_at_a_subtree_that_ends_in_text() {
        // Here `c`'s subtree ends with its text node rather than with a
        // namespace node, so an off-by-one at its end would show.
        let source = "<a><c>t</c></a>";
        assert_eq!(number(source, "count(c/following::node())"), 0.0);
        assert_eq!(number(source, "count(c/text()/preceding::node())"), 0.0);
    }

    #[test]
    fn the_sequence_budget_starts_afresh_for_each_evaluation() {
        // Each evaluation spends 900 000 of the 10 000 000-item budget; twelve
        // of them only fit because the budget resets once a scope closes.
        for _ in 0..12 {
            assert_eq!(eval_error(XPathVersion::V2, "count(1 to 900000)"), None);
        }
    }

    #[test]
    fn lang_reads_only_xml_lang() {
        assert!(!boolean("<a lang='en'/>", "lang('en')"));
        assert!(!boolean("<a xml:space='preserve'/>", "lang('preserve')"));
        assert!(boolean("<a xml:lang='en-GB'/>", "lang('en')"));
    }

    #[test]
    fn whitespace_may_surround_an_axis_separator() {
        assert_eq!(number(DOC, "count(child :: b)"), 2.0);
    }

    #[test]
    fn sequential_groups_do_not_accumulate_nesting_depth() {
        // Each group opens and closes one level. If closing did not count,
        // twenty groups plus the chain itself would pass the limit of 32.
        let expression = vec!["(1)"; 20].join(" + ");
        assert_eq!(number(DOC, &expression), 20.0);
    }

    #[test]
    fn numbers_far_from_one_print_without_an_exponent() {
        assert_eq!(text(DOC, "1 div 10000000"), "0.0000001");
        assert_eq!(
            text(DOC, "100000000000 * 100000000000"),
            "10000000000000000000000"
        );
        assert_eq!(text(DOC, "-1 div 10000000"), "-0.0000001");
        assert_eq!(text(DOC, "1 div 4"), "0.25");
    }

    #[test]
    fn a_position_predicate_needs_an_exact_match() {
        // 1 + 2^-52 is a different number from 1, so it selects nothing.
        assert_eq!(number(DOC, "count(b[1.0000000000000002])"), 0.0);
        assert_eq!(number(DOC, "count(b[1.0])"), 1.0);
    }

    #[test]
    fn id_matches_only_id_attributes() {
        let source = "<a><b k='x'/><b id='y'/><b xml:id='z'/></a>";
        assert_eq!(number(source, "count(id('x'))"), 0.0);
        assert_eq!(number(source, "count(id('y z'))"), 2.0);
    }

    #[test]
    fn equality_converts_by_the_xpath_one_precedence() {
        // A node-set against a number compares each node's number.
        assert!(boolean("<a><n>2</n><n>x</n></a>", "n = 2"));
        assert!(!boolean("<a><n>2</n></a>", "n = 3"));
        assert!(boolean("<a><n>2</n><n>3</n></a>", "n != 2"));
        assert!(!boolean("<a><n>2</n></a>", "n != 2"));
        // Neither side a node-set: a boolean on either side wins, then a number.
        assert!(boolean(DOC, "true() = 'x'"));
        assert!(boolean(DOC, "'x' = true()"));
        assert!(boolean(DOC, "1 = '1.0'"));
        assert!(boolean(DOC, "'1.0' = 1"));
    }

    fn with_exhausted_sequence_budget<T>(
        run: impl FnOnce() -> Result<T, EvalError>,
    ) -> Result<T, EvalError> {
        SEQUENCE_DEPTH.with(|depth| depth.set(1));
        SEQUENCE_WORK.with(|work| work.set(0));
        let result = run();
        SEQUENCE_DEPTH.with(|depth| depth.set(0));
        SEQUENCE_WORK.with(|work| work.set(0));
        result
    }

    #[test]
    fn every_multiplying_construct_refuses_an_exhausted_shared_budget() {
        let document = Document::from_str("<a/>").unwrap();
        let variables = Variables::new();
        let namespaces = Namespaces::new();
        let context = EvalContext::new(
            &document,
            document.document_element().unwrap(),
            &variables,
            &namespaces,
        )
        .with_version(XPathVersion::V3);
        let item = Item::Number(1.0, NumericType::Double);
        let unary = FunctionItem::Named {
            name: "string".into(),
            arity: 1,
        };
        let binary = FunctionItem::Named {
            name: "concat".into(),
            arity: 2,
        };

        assert!(with_exhausted_sequence_budget(|| {
            for_each(vec![item.clone()], &unary, &context)
        })
        .is_err());
        assert!(with_exhausted_sequence_budget(|| {
            filter(vec![item.clone()], &unary, &context)
        })
        .is_err());
        assert!(with_exhausted_sequence_budget(|| {
            fold_left(
                vec![item.clone()],
                Value::String(String::new()),
                &binary,
                &context,
            )
        })
        .is_err());
        assert!(with_exhausted_sequence_budget(|| {
            fold_right(
                vec![item.clone()],
                Value::String(String::new()),
                &binary,
                &context,
            )
        })
        .is_err());
        assert!(with_exhausted_sequence_budget(|| {
            for_each_pair(vec![item.clone()], vec![item.clone()], &binary, &context)
        })
        .is_err());

        for expression in [
            "(1) ! .",
            "for $x in 1 return $x",
            "some $x in 1 satisfies true()",
        ] {
            let parsed = parse(expression).unwrap();
            assert!(with_exhausted_sequence_budget(|| evaluate(&parsed, &context)).is_err());
        }
    }

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
    fn a_node_set_against_a_boolean_converts_rather_than_iterating() {
        // XPath 1.0 section 3.4. `boolean()` the node-set, then compare the
        // two booleans as numbers: true is 1, false is 0.
        //
        // The empty node-set is the case that separates this from the
        // existential rule the other operand types use. An existential walk
        // over no nodes is false whatever the comparison; the conversion
        // rule gives `0 >= 0`, which is true.
        assert!(boolean(DOC, "nothing >= false()"));
        assert!(boolean(DOC, "nothing < true()"));
        assert!(!boolean(DOC, "nothing > false()"));
        assert!(!boolean(DOC, "nothing >= true()"));

        // A non-empty node-set is `true`, whatever the nodes contain, so the
        // string values never enter into it.
        assert!(boolean(DOC, "b >= false()"));
        assert!(boolean(DOC, "b > false()"));
        assert!(!boolean(DOC, "b < true()"));
        assert!(boolean(DOC, "b >= true()"));
    }

    #[test]
    fn sum_of_an_empty_node_set_is_positive_zero() {
        assert_eq!(number(DOC, "sum(nothing)"), 0.0);
        assert!(number(DOC, "sum(nothing)").is_sign_positive());

        // `0.0 == -0.0` is true in IEEE 754, so the equality above passes
        // either way and proves nothing on its own. Division is what makes
        // the sign observable, and it is how the bug reached daylight.
        assert_eq!(number(DOC, "1 div sum(nothing)"), f64::INFINITY);
        assert_eq!(number(DOC, "-1 div sum(nothing)"), f64::NEG_INFINITY);
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
        variables.bind("n", Value::Number(41.0, NumericType::Double));
        let namespaces = Namespaces::new();
        let context = EvalContext::new(&document, document.root(), &variables, &namespaces);
        assert_eq!(
            evaluate(&expr, &context).unwrap(),
            Value::Number(42.0, NumericType::Double)
        );

        let missing = parse("$missing").unwrap();
        let error = evaluate(&missing, &context).unwrap_err();
        assert!(error.message.contains("bound here: n"));
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
        assert_eq!(
            evaluate(&prefixed, &context).unwrap(),
            Value::Number(1.0, NumericType::Double)
        );

        // An unprefixed name is in no namespace, so it matches nothing here.
        let bare = parse("count(b)").unwrap();
        assert_eq!(
            evaluate(&bare, &context).unwrap(),
            Value::Number(0.0, NumericType::Double)
        );
    }

    #[test]
    fn text_and_comment_node_tests() {
        let source = "<a>x<!--c--><?pi d?></a>";
        assert_eq!(number(source, "count(text())"), 1.0);
        assert_eq!(number(source, "count(comment())"), 1.0);
        assert_eq!(number(source, "count(processing-instruction())"), 1.0);
        assert_eq!(number(source, "count(processing-instruction('pi'))"), 1.0);
        assert_eq!(
            number(source, "count(processing-instruction('other'))"),
            0.0
        );
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

        for source in [
            "not()",
            "translate('a')",
            "substring('a')",
            "concat('a')",
            "boolean()",
        ] {
            let expr = parse(source).unwrap();
            let error =
                evaluate(&expr, &context).expect_err(&format!("{source} should be an arity error"));
            assert!(error.message.contains("argument"), "{source}: {error}");
        }
    }

    #[test]
    fn id_function_finds_elements_by_id_attribute() {
        assert_eq!(number(DOC, "count(id('1'))"), 1.0);
        assert_eq!(number(DOC, "count(id('1 2'))"), 2.0);
    }

    #[test]
    fn internal_value_axis_and_temporal_edges_are_total() {
        assert_eq!(item_to_value(Item::Boolean(true)), Value::Boolean(true));
        assert!(range_items(f64::NAN, 1.0).is_err());

        let mut document =
            Document::from_str(r#"<a xmlns:p="urn:p" id="x"><p:b/><c/></a>"#).unwrap();
        let root = document.root();
        let element = document.document_element().unwrap();
        let attribute = document.attributes(element)[0];
        let p_element = document.children(element)[0];
        let namespace = document.namespaces(element)[0];
        let variables = Variables::new();
        let mut namespaces = Namespaces::new();
        let context = EvalContext::new(&document, element, &variables, &namespaces);

        assert!(siblings(&document, attribute, true).is_empty());
        assert!(siblings(&document, root, true).is_empty());
        assert_eq!(
            comparable_strings(&Value::NodeSet(vec![element]), &document),
            [""]
        );
        assert!(!matches_node_test(
            &document,
            p_element,
            Axis::Child,
            &NodeTest::Name(super::super::ast::NameTest::parse("missing:b")),
            &context,
        ));
        assert!(!matches_node_test(
            &document,
            p_element,
            Axis::Child,
            &NodeTest::NamespaceWildcard("missing".into()),
            &context,
        ));
        assert!(matches_node_test(
            &document,
            namespace,
            Axis::Namespace,
            &NodeTest::Name(super::super::ast::NameTest::parse(
                &document.name(namespace).unwrap().local,
            )),
            &context,
        ));

        namespaces.insert("p", "urn:p");
        let context = EvalContext::new(&document, element, &variables, &namespaces);
        assert!(matches_node_test(
            &document,
            p_element,
            Axis::Child,
            &NodeTest::NamespaceWildcard("p".into()),
            &context,
        ));

        // A deliberately inconsistent arena exercises the defensive sibling
        // lookup used for synthetic nodes assembled inside the crate.
        document.nodes[element.0].children.clear();
        assert!(siblings(&document, p_element, true).is_empty());

        let day = Duration::parse("P1D", DurationKind::DayTime).unwrap();
        let month = Duration::parse("P1M", DurationKind::YearMonth).unwrap();
        let day_value = Value::Sequence(vec![Item::Duration(day)]);
        let month_value = Value::Sequence(vec![Item::Duration(month)]);
        let number = Value::Number(2.0, NumericType::Double);
        assert!(scale_duration(BinaryOp::Multiply, &day_value, &month_value, &document).is_err());
        assert!(scale_duration(BinaryOp::Divide, &day_value, &month_value, &document).is_err());
        assert!(scale_duration(BinaryOp::Divide, &number, &day_value, &document).is_err());
        assert!(scale_duration(
            BinaryOp::Multiply,
            &day_value,
            &Value::String("not a number".into()),
            &document,
        )
        .is_err());

        assert_eq!(
            temporal_kind_of(&Value::String("x".into()), &Value::String("y".into())),
            TemporalKind::DateTime
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn internal_type_comparison_and_path_errors_cover_every_result_shape() {
        let document = Document::from_str("<a><b/></a>").unwrap();
        let variables = Variables::new();
        let namespaces = Namespaces::new();
        let context = EvalContext::new(
            &document,
            document.document_element().unwrap(),
            &variables,
            &namespaces,
        );
        let optional_string = SequenceType {
            item_type: ItemType::Atomic("xs:string".into()),
            occurrence: Occurrence::ZeroOrOne,
        };
        let required_string = SequenceType {
            item_type: ItemType::Atomic("xs:string".into()),
            occurrence: Occurrence::One,
        };
        let empty = Value::Sequence(Vec::new());
        assert_eq!(
            evaluate_type_op(TypeOp::CastAs, &empty, &optional_string, &context).unwrap(),
            empty
        );
        assert_eq!(
            evaluate_type_op(TypeOp::CastableAs, &empty, &optional_string, &context).unwrap(),
            Value::Boolean(true)
        );
        assert!(evaluate_type_op(TypeOp::CastAs, &empty, &required_string, &context).is_err());
        assert_eq!(
            evaluate_type_op(TypeOp::CastableAs, &empty, &required_string, &context).unwrap(),
            Value::Boolean(false)
        );

        let many = Value::Sequence(vec![Item::String("a".into()), Item::String("b".into())]);
        assert!(evaluate_type_op(TypeOp::CastAs, &many, &required_string, &context).is_err());
        assert_eq!(
            evaluate_type_op(TypeOp::CastableAs, &many, &required_string, &context).unwrap(),
            Value::Boolean(false)
        );
        let node_type = SequenceType {
            item_type: ItemType::Node {
                kind: Some(NodeKind::Element),
                name: None,
            },
            occurrence: Occurrence::One,
        };
        assert!(evaluate_type_op(
            TypeOp::CastAs,
            &Value::String("a".into()),
            &node_type,
            &context,
        )
        .is_err());

        let named_node = ItemType::Node {
            kind: Some(NodeKind::Element),
            name: Some(super::super::ast::NameTest::parse("missing:b")),
        };
        assert!(!matches_item_type(
            &Item::String("b".into()),
            &named_node,
            &context
        ));
        assert!(!matches_item_type(
            &Item::Node(document.root()),
            &named_node,
            &context
        ));
        assert!(!matches_item_type(
            &Item::Node(document.children(document.document_element().unwrap())[0]),
            &named_node,
            &context
        ));

        for (text, ty) in [
            ("not-a-date", "date"),
            ("not-a-date-time", "dateTime"),
            ("not-a-time", "time"),
            ("not-a-duration", "dayTimeDuration"),
            ("not-a-duration", "yearMonthDuration"),
            ("x", "unknown"),
        ] {
            assert!(cast_item(&Item::String(text.into()), ty, &document).is_err());
        }

        let date = Value::Sequence(vec![Item::Temporal(
            Temporal::parse("2024-01-01", TemporalKind::Date).unwrap(),
        )]);
        let later = Value::Sequence(vec![Item::Temporal(
            Temporal::parse("2024-01-02", TemporalKind::Date).unwrap(),
        )]);
        for op in [
            BinaryOp::Equal,
            BinaryOp::NotEqual,
            BinaryOp::Less,
            BinaryOp::LessEqual,
            BinaryOp::Greater,
            BinaryOp::GreaterEqual,
        ] {
            assert!(compare_temporals(op, &date, &later, &context).is_ok());
        }
        assert!(std::panic::catch_unwind(|| {
            let _ = compare_temporals(BinaryOp::Add, &date, &later, &context);
        })
        .is_err());

        let time = Value::Sequence(vec![Item::Temporal(
            Temporal::parse("12:00:00", TemporalKind::Time).unwrap(),
        )]);
        assert!(temporal_arithmetic(BinaryOp::Add, &date, &later).is_err());
        assert!(temporal_arithmetic(BinaryOp::Subtract, &date, &time).is_err());
        let day = Value::Sequence(vec![Item::Duration(Duration::from_seconds(86_400.0))]);
        let month = Value::Sequence(vec![Item::Duration(Duration::from_months(1))]);
        assert!(temporal_arithmetic(BinaryOp::Add, &day, &month).is_err());
        assert!(temporal_arithmetic(BinaryOp::Subtract, &day, &date).is_err());
        assert!(temporal_arithmetic(BinaryOp::Add, &day, &date).is_ok());

        assert!(std::panic::catch_unwind(|| {
            let _ = compare_by_node(
                BinaryOp::Add,
                &Value::NodeSet(vec![document.root()]),
                &Value::NodeSet(vec![document.root()]),
                &document,
            );
        })
        .is_err());
        assert!(std::panic::catch_unwind(|| {
            let _ = compare_by_value(
                BinaryOp::Add,
                &Value::String("a".into()),
                &Value::String("b".into()),
                &document,
                0,
            );
        })
        .is_err());
        assert!(std::panic::catch_unwind(|| {
            compare_relational(
                BinaryOp::Add,
                &Value::Number(1.0, NumericType::Double),
                &Value::Number(2.0, NumericType::Double),
                &document,
            )
        })
        .is_err());

        let non_node_path = PathExpr {
            start: PathStart::Expr(Box::new(Expr::Literal("x".into())), Vec::new()),
            steps: Vec::new(),
        };
        assert!(evaluate_path(&non_node_path, &context).is_err());
        let mut atomic_context = context;
        atomic_context.context_item = Some(Item::String("x".into()));
        let walking = PathExpr {
            start: PathStart::Context,
            steps: vec![Step {
                axis: Axis::Child,
                node_test: NodeTest::Wildcard,
                predicates: Vec::new(),
            }],
        };
        assert!(evaluate_path_or_context_item(&walking, &atomic_context).is_err());
    }

    #[test]
    fn nested_expression_errors_propagate_from_every_composite_shape() {
        let document = Document::from_str("<root/>").unwrap();
        let variables = Variables::new();
        let namespaces = Namespaces::new();
        let context = EvalContext::new(
            &document,
            document.document_element().unwrap(),
            &variables,
            &namespaces,
        )
        .with_version(XPathVersion::V3);

        for source in [
            "-$missing",
            "($missing, 1)",
            "$missing to 1",
            "1 to $missing",
            "let $x := $missing return $x",
            "if ($missing) then 1 else 2",
            "($missing)(1)",
            "concat#2($missing, 'x')",
            "$missing and true()",
            "$missing or false()",
            "1 ! $missing",
            "for $x in $missing return $x",
            "some $x in $missing satisfies true()",
            "(.)[$missing]/child",
        ] {
            let expression = parse(source).unwrap();
            assert!(evaluate(&expression, &context).is_err(), "{source}");
        }

        for source in [
            "1 instance of empty-sequence()",
            "xs:yearMonthDuration('P1M') + xs:date('2024-01-01')",
            "child::missing:root",
        ] {
            let expression = parse(source).unwrap();
            assert!(evaluate(&expression, &context).is_ok(), "{source}");
        }

        let date = Value::Sequence(vec![Item::Temporal(
            Temporal::parse("2024-01-01", TemporalKind::Date).unwrap(),
        )]);
        assert_eq!(
            temporal_kind_of(&Value::String("2024-01-01".into()), &date),
            TemporalKind::Date
        );
        assert_eq!(
            temporal_kind_of(&Value::String("a".into()), &Value::String("b".into())),
            TemporalKind::DateTime
        );
        assert_eq!(
            temporal_kind_of(
                &Value::Sequence(vec![Item::String("a".into())]),
                &Value::String("b".into()),
            ),
            TemporalKind::DateTime
        );
        assert!(compare_equality(
            &Value::NodeSet(Vec::new()),
            &Value::Boolean(false),
            &document,
            true,
        ));
        assert!(!matches_item_type(
            &Item::String("x".into()),
            &ItemType::EmptySequence,
            &context,
        ));

        let namespaced_document =
            Document::from_str("<p:root xmlns:p='urn:p'><p:a/></p:root>").unwrap();
        let mut bound_namespaces = Namespaces::new();
        bound_namespaces.insert("p", "urn:p");
        let namespaced_context = EvalContext::new(
            &namespaced_document,
            namespaced_document.document_element().unwrap(),
            &variables,
            &bound_namespaces,
        )
        .with_version(XPathVersion::V3);
        for source in [
            ". instance of element(p:root)",
            "child::element(p:a)",
            "child::element(missing:a)",
        ] {
            assert!(evaluate(&parse(source).unwrap(), &namespaced_context).is_ok());
        }

        let mut atomic_context = context.clone();
        atomic_context.context_item = Some(Item::String("x".into()));
        assert!(evaluate(&parse("/root").unwrap(), &atomic_context).is_ok());

        // These reach the effective-boolean-value error paths after their
        // condition/predicate has evaluated successfully to a function item.
        for source in [
            "if ((1, 2)) then 1 else 2",
            "some $x in (1) satisfies (1, 2)",
            "filter((1), function($x) { (1, 2) })",
        ] {
            let expression = parse(source).unwrap();
            assert!(evaluate(&expression, &context).is_err(), "{source}");
        }
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn a_numeric_predicate_beyond_2_pow_53_has_no_exact_position() {
        assert!(position_matches(3.0, 3).unwrap());
        assert!(!position_matches(3.0, 4).unwrap());
        assert!(position_matches(3.0, (1_usize << 53) + 1).is_err());
    }

    #[test]
    fn a_depth_counter_that_cannot_count_further_is_refused_by_every_multiplying_construct() {
        let document = Document::from_str("<a/>").unwrap();
        let variables = Variables::new();
        let namespaces = Namespaces::new();
        let context = EvalContext::new(
            &document,
            document.document_element().unwrap(),
            &variables,
            &namespaces,
        )
        .with_version(XPathVersion::V3);
        for source in [
            "for-each((1, 2), function($x) { $x })",
            "filter((1, 2), function($x) { true() })",
            "fold-left((1, 2), 0, function($a, $b) { $a + $b })",
            "fold-right((1, 2), 0, function($a, $b) { $a + $b })",
            "for-each-pair((1, 2), (3, 4), function($a, $b) { $a + $b })",
            "(1, 2) ! .",
            "for $i in (1, 2) return $i",
            "some $i in (1, 2) satisfies true()",
            "1 to 2",
        ] {
            let expression = parse(source).unwrap();
            SEQUENCE_DEPTH.with(|depth| depth.set(u32::MAX));
            let result = evaluate(&expression, &context);
            SEQUENCE_DEPTH.with(|depth| depth.set(0));
            let error = result.expect_err(source);
            assert!(
                error.to_string().contains("nested too deeply"),
                "{source}: {error}"
            );
        }
    }

    #[test]
    fn a_range_with_equal_infinite_bounds_has_no_countable_length() {
        assert!(range_items(f64::INFINITY, f64::INFINITY).is_err());
    }
}
