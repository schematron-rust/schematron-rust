# XPath 3.0 support

The `xslt3` and `xpath3` query bindings are XPath 3.0, a further superset of
2.0 the way 2.0 is a superset of 1.0: every 1.0 and 2.0 construct this crate
implements is available under a 3.0 binding too, at exactly its 1.0/2.0
semantics — see [spec/xpath2/](../xpath2/index.md) for what that means and
where it still diverges from a genuine 2.0 processor.

This document states exactly how much of XPath 3.0 itself the crate
implements. `xpath31` and `xslt31` remain refused: XPath 3.1 adds maps and
arrays, which this crate does not have.

## Status: phase 4

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

Phase 4 adds the simple map operator `!` itself, and with it the
foundation phases 2 and 3 had both recorded as blocking it: `EvalContext`
can now carry a context item that isn't a node. See "The simple map
operator" below for what that took and where it still stops short.

Writing up phase 4's own documentation turned up one more gap the earlier
phases had missed entirely, not merely mis-scoped the way `sort` was:
**`let $v := E return E`**, XPath 3.0's `let` expression — new to the
*grammar* in 3.0 (`ExprSingle` gains `LetExpr`; XPath 2.0's `ExprSingle`
has no such alternative), the same way `!` and `=>` are, and unrelated to
Schematron's own `<let>` element. It needed nothing beyond what `for` and
`some`/`every` already use — one more branch in the same "is this name
followed by `$`" dispatch — so it is fixed in the same commit as phase 4
rather than deferred to a phase of its own. See "The `let` expression"
below.

The same check turned up **EQNames**, `Q{uri}local`, too — implemented for
node name tests, the position that was genuinely tractable; see "EQNames"
below for that scope and for a real stack-overflow regression fuzzing it
found in the process, in `NameTest` rather than in EQNames themselves.

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
| `E1 ! E2` | The simple map operator: `E2` evaluated once per item of `E1`, that item as the context item. See below |
| `let $v := E return E` | Binds `E`'s value — the whole value, not iterated — to `$v` for the second `E`. See below |
| `Q{uri}local` | An EQName: names an element or attribute by namespace URI directly, no prefix to declare. Node name tests only. See below |

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

## The simple map operator

`E1 ! E2` evaluates `E2` once for each item of `E1`, with that item as the
**context item**, and concatenates the results. Chains left-associatively
(`E1 ! E2 ! E3` is `(E1 ! E2) ! E3`) and binds tighter than every other
operator except a path step itself — real XPath 3.0's grammar nests
`UnionExpr` around `SimpleMapExpr`, so `a ! b | c` means `(a ! b) | c`, and
`RangeExpr` around that in turn, so `1 to 2 ! f()` means `1 to (2 ! f())`.
Per F&O, the context *position* and *size* are reset to `1` for every
evaluation of `E2`, unlike an axis step's predicates, which see the step's
own position among its siblings.

`.` is how `E2` refers to the current item — the same way a path step's
predicate refers to the node it is testing. When the item is a **node**,
this is exactly `for`'s per-iteration binding, except shifting the context
item instead of binding a variable: the context node moves to it, and
every axis, kind test, and function that already reads `EvalContext::node`
keeps working unchanged.

When the item is **not** a node — an atomic value, or a function item —
there is no node for `EvalContext::node` to become. That is where phases 2
and 3 both stopped: this crate's evaluator had never needed to represent
"the context item is not a node" before, because `for-each()`'s action
binds its parameter by *name*, never through `.`, and `!` has no name to
bind. `EvalContext` now carries an optional [`context_item`] beside `node`
for exactly this case, read in exactly one place — the `Expr::Path` arm of
`evaluate` — to resolve a bare `.`. Anything wanting a real axis from a
non-node item (`child::x`, `@a`, even `..`) is a dynamic error naming what
the context item actually is, because there is no node to walk it from —
the same answer real XPath 3.0 gives a step over a non-node context item,
not an approximation invented for this crate.

[`context_item`]: https://docs.rs/schematron/latest/schematron/xpath/struct.EvalContext.html#structfield.context_item

## The `let` expression

`let $v := E1 return E2` evaluates `E1`, binds the result to `$v` as one
whole value, and evaluates `E2` with that binding in scope. Distinct from
`for`, which this crate already had: `for $v in E1 return E2` *iterates*
`E1`, rebinding `$v` to each item in turn and evaluating `E2` once per
item, so its results concatenate; `let` evaluates `E2` exactly once, with
`$v` bound to the whole of `E1`'s value regardless of how many items that
is. `let $v := (1, 2, 3) return count($v)` is `3`; `for $v in (1, 2, 3)
return count($v)` is `(1, 1, 1)`.

Also distinct from Schematron's own `<let name="..." value="..."/>`
element, which declares a schema- or rule-scoped variable outside any
single expression — this is the *XPath* `let`, written inside a test or
value like any other expression, and scoped only to the `return` that
follows it. Nesting shadows exactly the way a closure's parameter would:
`let $x := 1 return let $x := 2 return $x` is `2`.

Parsed the same way `for`, `some`, and `every` already are: `let` is an
ordinary name until a `$` follows it directly, which is what tells it
apart from an element or attribute actually named `let`. Not part of any
earlier phase's own accounting — found by checking this crate's XPath 3.0
subset against an authoritative feature list while writing up phase 4,
not against what earlier phases here happened to already record — so it
carries no phase number of its own; see the status section above.

## EQNames

`Q{uri}local` names an element or attribute by namespace URI directly —
`Q{http://example.com/ns}foo` selects the same nodes `p:foo` would if the
schema declared `<ns prefix="p" uri="http://example.com/ns"/>`, without
needing that declaration at all. `Q{}local` — an empty braced URI literal
— means *no* namespace, the same as writing `local` unprefixed, never "the
prefix that happens to be empty."

`Q{` is recognized as the start of one, whatever the query binding, the
same way the other 3.0-only tokens are — a 1.0/2.0 schema that writes one
gets a compile-time refusal naming it, "an EQName," rather than a
confusing parse error about a stray `{`. The one lexical wrinkle: `Q`
directly followed by `{`, no space, is what triggers it, so an element or
attribute genuinely named `Q` is unaffected — `Q`, `Q/a`, `@Q` all still
mean exactly what they did before, and only the exact `Q{` sequence, which
was a syntax error either way before this existed, changes meaning.

**Scoped to node name tests** — `Q{uri}local` in a path step
(`Q{uri}local`, `@Q{uri}local`, `child::Q{uri}local`, and so on) — not
extended to variable names, type names in `instance of`/`cast as`, or
function references, which real XPath 3.0 also allows an EQName to name.
Node name matching in this crate already resolves a prefix to its URI and
compares by URI (`context.namespaces.resolve(prefix)`, then compared
against the document node's own resolved namespace), so accepting a URI
directly instead of a prefix to resolve was a genuinely small, contained
change. Variable binding is not: `Variables` keys a binding by the
*lexical* spelling of its name (`prefix:local` as written), not by
resolved URI, so `$p:x` and `$q:x` are already two different bindings in
this crate even when `p` and `q` resolve to the same URI — a pre-existing
simplification nobody has hit in practice. Making `$Q{uri}local`
interchangeable with every prefixed spelling of the same expanded name
would mean reworking that keying from lexical to expanded-name everywhere
variables are bound and looked up, which is a foundation-level change for
a syntax real schemas essentially never write; not worth that risk today,
by the same reasoning [spec/roadmap/](../roadmap/index.md) already applies
to streaming validation and `no_std`. Type names and function references
were left out for the same reason: this crate's names are schema-`<ns>`
scoped throughout, and neither position was worth reopening that on its
own.

Fuzzing this immediately after writing it — the same discipline applied to
every construct in this document — found a real, if narrow, stack-overflow
regression, not in EQName parsing itself but in `NameTest`, the type it
extended: adding one field to a struct held inline (not boxed) in several
`Expr` variants was enough to push `MAX_RECURSION_DEPTH`'s already-thin
margin negative, so the deepest *legal* nesting the parser accepted
started to overflow the stack instead of returning cleanly. See
`MAX_RECURSION_DEPTH`'s own doc comment in `src/xpath/parser.rs` for the
full account and how to re-measure; the short version is that the constant
now keeps real headroom below the last-measured danger zone instead of
sitting just under it.

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
| `sort` | Not actually XPath 3.0: real F&O 3.0 has no `fn:sort` at all — it was added in 3.1, alongside maps and arrays. Not a gap in this phase; see the next row |
| Maps and arrays | XPath 3.1, not 3.0; needs the `xpath31`/`xslt31` bindings, which remain refused |
| `xpath31`, `xslt31` bindings | Still refused; use `allow_unknown_query_binding` |
| `Q{uri}local` as a variable name, a type name, or a function reference | EQNames are implemented for node name tests only — see "EQNames" above for why the other positions were left out |
| Union types in casts and signatures (`(xs:integer \| xs:string)`) | No union item type exists in this crate's type model; `instance of`/`cast as`/`treat as` take one atomic type, and inline functions parse no type annotations at all — see below |

One real gap remains fully open — union types — found the same way `let`
and EQNames were: checking this crate's XPath 3.0 subset against an
authoritative list of what 3.0 actually added, not against what earlier
phases here happened to already record.

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
      <assert test="let $names := line/@sku return count($names) = count(distinct-values($names))">
        Every line's SKU must be unique within the invoice.
      </assert>
      <assert test="every $amount in (line/@amount ! number(.)) satisfies $amount &gt; 0">
        Every line amount must be positive.
      </assert>
    </rule>
  </pattern>
</schema>
```
