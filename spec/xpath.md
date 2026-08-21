# XPath 1.0 engine

Schematron's `xslt` and `xpath` query bindings are XPath 1.0. The crate
implements XPath 1.0 in full, in `schematron::xpath`, with no external XPath
crate.

Pipeline: `&str` → lexer → tokens → recursive-descent parser → `Expr` AST →
evaluator → `Value`.

## Values

```rust
pub enum Value {
    NodeSet(Vec<NodeId>), // sorted by document order, deduplicated
    Boolean(bool),
    Number(f64),
    String(String),
}
```

Conversions follow XPath 1.0 exactly:

- **to boolean** — node-set: non-empty. number: not zero and not NaN. string: non-empty.
- **to number** — string: parsed as an XPath number, else NaN. boolean: 1 or 0.
  node-set: number of its string value.
- **to string** — node-set: string value of the first node in document order,
  or `""` if empty. boolean: `true` / `false`. number: the XPath number format.

### The number format

This is the detail engines get wrong. XPath 1.0 `string(number)`:

- `NaN` → `NaN`
- positive infinity → `Infinity`, negative → `-Infinity`
- an integer → decimal digits with no `.` and no exponent, with `-0` → `0`
- otherwise → the shortest decimal that round-trips, **never** in exponential
  notation, so `1e21` prints as `1000000000000000000000`.

## Depth limit

Sub-expression nesting — parentheses, predicates, function arguments, unary
minus — is capped at **64**. Recursive descent on a thousand nested
parentheses would otherwise exhaust the stack, and an error beats a crash.
The limit does not count the *length* of a location path, which is parsed
iteratively, so `a/b/c/…` of any length is fine. Sixty-four is far beyond any
expression a person writes; the ceiling is set by how much stack one nesting
level costs in an unoptimised build, where each level descends the whole
precedence chain below.

## Grammar

The full XPath 1.0 grammar is implemented, at standard precedence:

Under an XPath 2.0 binding the top production is
`Expr := ExprSingle ("," ExprSingle)*`, where the comma builds a sequence, and
function arguments and predicates take an `ExprSingle` so that a comma there
still separates arguments. See [xpath2.md](xpath2.md).

```
Expr        := OrExpr
OrExpr      := AndExpr        ('or' AndExpr)*
AndExpr     := EqualityExpr   ('and' EqualityExpr)*
EqualityExpr:= RelationalExpr (('=' | '!=') RelationalExpr)*
RelationalExpr := AdditiveExpr (('<'|'>'|'<='|'>=') AdditiveExpr)*
AdditiveExpr:= MultiplicativeExpr (('+'|'-') MultiplicativeExpr)*
MultiplicativeExpr := UnaryExpr (('*'|'div'|'mod') UnaryExpr)*
UnaryExpr   := '-'* UnionExpr
UnionExpr   := PathExpr ('|' PathExpr)*
PathExpr    := LocationPath | FilterExpr (('/'|'//') RelativeLocationPath)?
FilterExpr  := PrimaryExpr Predicate*
PrimaryExpr := '$' QName | '(' Expr ')' | Literal | Number | FunctionCall
LocationPath:= '/' RelativeLocationPath? | '//' RelativeLocationPath | RelativeLocationPath
Step        := AxisSpecifier NodeTest Predicate* | '.' | '..'
NodeTest    := QName | '*' | NCName ':' '*'
             | 'node()' | 'text()' | 'comment()' | 'processing-instruction(Literal?)'
```

### Lexer disambiguation

XPath's tokenizer is context-sensitive. The three rules from the spec are
implemented by tracking the preceding token:

1. `*` is the multiply operator if the previous token is a name, number,
   string, `)`, `]`, or `.`; otherwise it is the wildcard node test.
2. `and`, `or`, `div`, `mod` are operators in those same positions, and
   element names otherwise.
3. A name followed by `(` is a function name unless it is a node type
   (`node`, `text`, `comment`, `processing-instruction`); a name followed by
   `::` is an axis name.

**The order matters.** Rule 2 is applied before rule 3, as the standard
specifies. With the two the other way round, `a and (b)` lexes as a call to a
function named `and` and fails to parse — and `and`, `or`, `div` and `mod`
followed by an opening parenthesis are all perfectly ordinary things to
write. This was a real defect, shipped in 0.1.0 and fixed after the XPath 2.0
keyword handling turned up the same interaction for `in (`.

## Axes

All thirteen: `ancestor`, `ancestor-or-self`, `attribute`, `child`,
`descendant`, `descendant-or-self`, `following`, `following-sibling`,
`namespace`, `parent`, `preceding`, `preceding-sibling`, `self`. Abbreviations
`@`, `//`, `.`, `..` expand to their full forms.

Reverse axes (`ancestor`, `ancestor-or-self`, `preceding`,
`preceding-sibling`, `parent`) evaluate predicates with reverse proximity
position, then return the node-set in document order.

## Comparison semantics

The node-set rules are the ones that surprise people, and they are implemented
literally:

- node-set **op** node-set — true if *any* pair of nodes has string values for
  which the comparison holds.
- node-set **op** other — true if *any* node's string value, converted to the
  other operand's type, satisfies the comparison.
- `=` and `!=` on two non-node-sets: if either is boolean, compare as boolean;
  else if either is number, compare as number; else compare as string.
- `<`, `<=`, `>`, `>=` always compare as numbers.

Therefore `a != b` is not `not(a = b)` when node-sets are involved. The crate
does not "fix" this.

## Function library

All 27 core functions:

`last`, `position`, `count`, `id`, `local-name`, `namespace-uri`, `name`,
`string`, `concat`, `starts-with`, `contains`, `substring-before`,
`substring-after`, `substring`, `string-length`, `normalize-space`,
`translate`, `boolean`, `not`, `true`, `false`, `lang`, `number`, `sum`,
`floor`, `ceiling`, `round`.

Plus two functions from the XSLT library, which the `xslt` query binding makes
available:

| Function | Notes |
|---|---|
| `current()` | The node the rule fired on, unaffected by predicates. Unlike `.`, it does not change inside a predicate, which is the whole point of it. |
| `document(uri)` | The root nodes of external documents. See below. |

The two-argument `document(uri, base)` form is **not** implemented; URIs
resolve against the instance document's base URI.

## `document()` and cross-document node-sets

`document()` is the one function that breaks an assumption the rest of the
engine rests on: a node-set is a list of indices into **one** arena, and
`document()` returns nodes of a different document.

Two mechanisms make it work.

**The arena is shared.** A loaded document is copied in beside the instance,
keeping its own root node with no parent, so neither tree becomes the other's
ancestor. Document order continues across the merge, so a node-set spanning
two documents still sorts deterministically — XPath 1.0 leaves the relative
order of nodes in different documents implementation-defined, requiring only
that it be consistent.

Because a merged tree is still a separate document, `/` means *that*
document's root when the context node is inside it, and `id()` searches only
the document its context node belongs to. Rules never fire on nodes of a
loaded document: the validator walks the instance only.

**Loading happens between passes.** Evaluation holds the tree immutably, so a
`document()` call cannot load anything on the spot. It records the URI as a
*miss* and contributes no node. The validator then loads everything that was
asked for, merges it in, and runs again, discarding the earlier report — which
was computed against an incomplete document set.

One pass discovers the URIs and the next has them, so two passes suffice
unless a loaded document itself names the next one, as in
`document(document(@first)/hop/@next)`. That resolves on the third. The loop is
capped at eight passes; exceeding it is an error, not a hang.

The cost is paid only by schemas that use the feature. Whether any expression
calls `document()` is determined when the schema is compiled, and a schema
that does not is validated against the caller's tree directly, with no copy
and no second pass.

Two consequences worth stating plainly:

- **`document()` needs the validator.** Evaluating an expression directly
  through this module has no registry, and `document()` is then an *error*
  rather than an empty node-set — silently returning nothing would turn a
  broken lookup into a passing assertion.
- **It obeys the resolver.** The default resolver refuses `http:` and
  `https:`, so `document()` is not a hole in the no-implicit-network rule.

`pattern/@documents` remains the better tool when the goal is to *validate*
external documents rather than to read values out of them; see
[validation.md](validation.md).

Arity is checked twice: once when the schema is compiled, so a typo fails
immediately, and again in the function library itself, because [`evaluate`] is
public and can be handed an expression that never went through a schema. The
second check is what the `fuzz_xpath` target found missing.

Calling an unknown function is an error at schema-compile time. Calling an
XPath 2.0 function — `matches`, `tokenize`, `current-date`, and the rest — is
also a compile-time error, and the message says that it is an XPath 2.0
function rather than merely that it is unknown.

[`evaluate`]: https://docs.rs/schematron/latest/schematron/xpath/fn.evaluate.html

## Context

```rust
pub struct EvalContext<'a> {
    pub document: &'a Document,
    pub node: NodeId,
    pub position: usize,   // 1-based
    pub size: usize,
    pub variables: &'a Variables,
    pub namespaces: &'a Namespaces,
    pub current: NodeId,   // what current() returns
}
```

Namespace bindings come from the schema's `ns` elements only. An unprefixed
name in an XPath expression matches **no namespace**, per XPath 1.0 — there is
no default-namespace fallback. Schemas for namespaced vocabularies must
therefore declare a prefix and use it, which is exactly what the standard
requires.
