# XPath 3.0 support

The `xslt3` and `xpath3` query bindings are XPath 3.0, a further superset of
2.0 the way 2.0 is a superset of 1.0: every 1.0 and 2.0 construct this crate
implements is available under a 3.0 binding too, at exactly its 1.0/2.0
semantics — see [spec/xpath2/](../xpath2/index.md) for what that means and
where it still diverges from a genuine 2.0 processor.

This document states exactly how much of XPath 3.0 itself the crate
implements. `xpath31` and `xslt31` remain refused: XPath 3.1 adds maps and
arrays, which this crate does not have.

## Status: phase 3

XPath 3.0's headline addition is the **function item** — a value that is
itself a function, which a *dynamic call* can invoke. Phase 1 implemented
exactly what's needed to make that real, plus the one function it exists
for:

- **Inline function expressions**: `function($a, $b) { … }`.
- **Named function references**: `name#arity`, naming one of this crate's
  own built-in functions (from 1.0, 2.0, or 3.0 itself) without calling it.
- **Dynamic function calls**: `$f(1, 2)`, chainable (`$f()()`).
- **`for-each($sequence, $action)`**: applies `action` — a function item —
  to each item and concatenates the results. The one function that actually
  needed all of the above; see [spec/xpath2/](../xpath2/index.md)'s history
  of it being misfiled as an XPath 2.0 sequence function before that.

Phase 2 added the two remaining 3.0 operators that need no new evaluation
machinery — both are pure syntax, checked at the same compile-time gate as
everything else version-specific:

- **The arrow operator `=>`**: `$x => f(1)` is sugar for `f($x, 1)`,
  chainable (`$x => f() => g()`). See "The arrow operator" below.
- **String concatenation `||`**: `E || E`, atomizing each side the way
  `concat()`'s arguments already do.

Phase 3 finishes the higher-order sequence function library `for-each()`
belongs to, plus the functions that introspect a function item itself
rather than applying one — none of them needed anything phases 1 and 2
had not already built:

- **`filter(seq, predicate)`**, **`fold-left(seq, zero, f)`**,
  **`fold-right(seq, zero, f)`**, **`for-each-pair(seq1, seq2, f)`** — see
  "The rest of the higher-order sequence functions" below.
- **`function-lookup(name, arity)`**, **`function-arity(f)`**,
  **`function-name(f)`** — see "Introspecting a function item" below.

Not in this phase: the simple map operator `!`. It is a hard error naming
the construct, same as everything this crate does not implement — see
"What is not implemented" below, and "Why `!` needs more than syntax" for
why it did not land alongside everything else.

## What is implemented

Available only when the schema declares `queryBinding="xslt3"` or
`"xpath3"`. Under an earlier binding these are errors, so a 1.0 or 2.0
schema cannot accidentally acquire 3.0 behavior.

### Syntax

| Construct | Notes |
|---|---|
| `function($a, $b) { E }` | An inline function expression — a closure. No parameter or return type annotations; see below |
| `name#arity` | A named function reference to any built-in this crate implements, at the version it belongs to |
| `E(E, E, …)` | A dynamic call: `E` must evaluate to exactly one function item |
| `E => f(…)` | The arrow operator: `E` piped in as `f`'s first argument. See below |
| `E \|\| E` | String concatenation: the string value of each side, joined |

### Functions

| Function | Notes |
|---|---|
| `for-each(sequence, action)` | Applies `action` to each item, in order, and concatenates the results |
| `filter(sequence, predicate)` | Keeps the items `predicate`'s effective boolean value holds for |
| `fold-left(sequence, zero, f)` | Combines from the left: `f(f(f(zero, i1), i2), i3)…` |
| `fold-right(sequence, zero, f)` | Combines from the right: `f(i1, f(i2, f(i3, zero)))` |
| `for-each-pair(sequence1, sequence2, f)` | Applies `f` to corresponding items of both, stopping at the shorter |
| `function-lookup(name, arity)` | The named built-in at that arity as a function item, or the empty sequence when there isn't one |
| `function-arity(f)` | `f`'s arity |
| `function-name(f)` | `f`'s name, or the empty sequence for an inline function, which has none |

## Function items are not values in the usual sense

A function item is not atomizable: it has no string value and no numeric
value, and comparing one — with `=`, `!=`, `<`, `eq`, or any other
comparison — is an error rather than a silently wrong answer. `string()`,
`number()`, `concat()`, and every comparison operator reject a function
item explicitly, naming it. `instance of` reports `false` for every atomic
type test against one, since this phase does not implement 3.0's own item
type for it, `function(*)`.

## Closures are genuinely lexical

An inline function expression captures the variable bindings visible where
it is *written*, not where it is later called:

```
for $n in (10)
return for-each((1, 2, 3), function($x) { $x * $n })
```

returns `(10, 20, 30)` — `$n` resolves from the `for` that was in scope when
the `function(...)` expression was evaluated, independent of whatever
variables happen to be bound at the call site inside `for-each`. Every other
part of `EvalContext` — the document, the current node, the clock, the
implicit timezone — comes from the *caller's* context instead, because a
function item never outlives the validation run that created it, so there
is no other run for those to meaningfully come from.

## The arrow operator

`E => f(args…)` prepends `E` as `f`'s first argument, ahead of whatever
`args` are written — `'a-b-c' => tokenize('-')` means
`tokenize('a-b-c', '-')`. The target can be:

- a **named function**, `E => f(…)` — any built-in this crate implements,
  at whatever version it belongs to;
- a **variable holding a function item**, `E => $f(…)`;
- a **parenthesized expression**, `E => (expr)(…)`, which must evaluate to
  a function item, exactly like a dynamic call's target.

It chains: `E => f() => g()` means `g(f(E))`, each arrow feeding the next.
The operator is pure sugar in this crate — parsed straight into an ordinary
call (`Expr::Function` for a named target, `Expr::DynamicCall` for the
other two), so it evaluates with no logic of its own and inherits whatever
that call already does: an unknown function name is still "unknown
function", a wrong arity is still a wrong arity, and a target that isn't a
function item is still rejected by name. The wrapper exists only so a 1.0
or 2.0 binding can reject `=>` itself by name, rather than accepting
whichever ordinary call it would otherwise desugar to.

## The rest of the higher-order sequence functions

`for-each()` was the one the function-item machinery existed for; the
other four are its siblings, and none needed anything new:

- **`filter(seq, predicate)`** calls `predicate` once per item, keeping the
  item when the result's effective boolean value holds — the same
  conversion `if`, `some`, and `every` already apply to their own
  conditions, not a stricter `xs:boolean`-only check.
- **`fold-left(seq, zero, f)`** and **`fold-right(seq, zero, f)`** combine
  a sequence into one value by repeated application, from opposite ends.
  `zero` need not be a single item — it is threaded through as a whole
  value, exactly like a `let`-bound one — so `fold-left((), 'seed', …)`
  legitimately returns `'seed'` unevaluated for an empty sequence. The two
  are genuinely different, not just written in reverse: `fold-left((1, 2,
  3), 0, function($acc, $x) { $acc - $x })` is `((0 - 1) - 2) - 3 = -6`,
  while `fold-right((1, 2, 3), 0, function($x, $acc) { $x - $acc })` is
  `1 - (2 - (3 - 0)) = 2` — note the argument order also flips: the
  running value is `$f`'s *first* parameter in `fold-left` and its
  *second* in `fold-right`, matching which side of the combination it sits
  on.
- **`for-each-pair(seq1, seq2, f)`** applies `f` to corresponding items of
  both sequences and concatenates the results. A length mismatch is not an
  error — F&O does not treat it as one — the shorter sequence simply
  decides how many pairs there are.

Every one of these is a genuinely multiplying construct, exactly like
`for-each()`, `for`, and a `to` range: nesting them multiplies the work,
and they all draw on the *same* shared budget those already do, not one
of their own — a `filter` inside a `for-each` inside another `for-each` is
bounded by the same limit a triple-nested `for-each` is. See
[spec/conformance/](../conformance/index.md)'s limits table.

## Introspecting a function item

- **`function-arity(f)`** returns how many parameters `f` takes.
- **`function-name(f)`** returns the name a named function reference
  (`name#arity`) carries, or the empty sequence for an inline function
  expression, which was never given one.
- **`function-lookup(name, arity)`** is the dynamic counterpart to
  `name#arity`: where that syntax names a function literally and is
  checked when the schema compiles — a typo is a compile-time error naming
  the construct — `$name` here is an ordinary computed string, so there is
  nothing to check until the call actually runs. A lookup that finds
  nothing is not an error; F&O specifies the empty sequence instead, since
  a dynamic lookup failing is an ordinary, expected outcome to test for
  (`exists(function-lookup(...))`), not a broken schema.

## Why `!` needs more than syntax

The simple map operator, `E1 ! E2`, evaluates `E2` once per item of `E1`
with that item as the **context item** — and `.` is how `E2` refers to it,
the same way a path step's predicate refers to the node it is testing.
That is where this crate's model runs out: `EvalContext` — the struct every
expression evaluates against — holds a context **node** (`node: NodeId`),
not a context **item**. Every axis, every `.`, every `self::` step reads
that field, and it is always a real node in the document arena.

`for-each()` sidesteps this entirely: its action is a function item with a
named parameter, and the item it is mapping over is *bound to that name*,
never routed through `.`. `!` has no such parameter to bind — `.` is the
only way its right side can see the item at all. So implementing it needs
`EvalContext` itself to carry an arbitrary item, not just a node, and every
place `.` is resolved to fall back to that item when there is no node to
be — a change to the evaluator's foundation, not one function or operator.
Approximating it by leaving `.` pointing at whatever node was already in
scope would be silently wrong whenever the mapped-over item is not a node
— the same failure mode `AGENTS.md`'s "an evaluation error is never
silently a false assertion" rules out for an assertion's overall result,
applied to a single silently wrong `.`. Hence `!` stays a hard error,
naming itself, until that foundation is built.

## No parameter or return type annotations

Real XPath 3.0 allows `function($a as xs:integer) as xs:integer { … }`.
This phase does not parse the `as SequenceType` annotations at all — a
schema that writes one gets a plain parse error, not a silently ignored
annotation. Write the function without them; nothing here does static type
checking of function parameters in any case; a mismatched argument type
just does what passing anything else to that operation already does.

## What is not implemented

Every one of these is a **hard error naming the construct**, at
schema-compile time (or, for `trace()`, at the same point 2.0's
not-implemented functions are — see [spec/xpath2/](../xpath2/index.md)).
None of them silently does something else.

| Construct | Why not |
|---|---|
| `!` (the simple map operator) | Needs a context item that can be any value, not just a node; see "Why `!` needs more than syntax" above |
| `sort` | Not actually XPath 3.0: real F&O 3.0 has no `fn:sort` at all — it was added in 3.1, alongside maps and arrays. Not a gap in this phase; see the next row |
| Maps and arrays | XPath 3.1, not 3.0; needs the `xpath31`/`xslt31` bindings, which remain refused |
| `xpath31`, `xslt31` bindings | Still refused; use `allow_unknown_query_binding` |

## Using it

```xml
<schema xmlns="http://purl.oclc.org/dsdl/schematron" queryBinding="xslt3">
  <pattern>
    <rule context="invoice">
      <assert test="sum(for-each(line/@amount, number#1)) = @total">
        The line amounts must sum to the invoice total.
      </assert>
      <assert test="@id => string-length() = 8">
        The invoice id must be exactly 8 characters.
      </assert>
      <assert test="(@prefix || @number) = @id">
        The invoice id must be the prefix and the number concatenated.
      </assert>
      <assert test="fold-left(line/@amount, 0, function($sum, $x) { $sum + number($x) }) = @total">
        The invoice total must be the sum of every line amount.
      </assert>
    </rule>
  </pattern>
</schema>
```
