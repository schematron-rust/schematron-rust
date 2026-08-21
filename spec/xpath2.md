# XPath 2.0 support

The `xslt2` and `xpath2` query bindings are XPath 2.0, which is not a
superset of XPath 1.0 with extra functions — it is a different language with
a different type system. This document states exactly how much of it the
crate implements, and, more importantly, **where it still behaves like XPath
1.0 even though a schema declares 2.0**.

Read the divergences section before relying on this. It is short and it
matters.

## Status: phases 1 and 2a

Schematron schemas in the wild declare `xslt2` far more often than they use
the parts of XPath 2.0 that are genuinely incompatible with 1.0. The
implementation targets that gap, in order of how much the constructs are
actually used.

**Phase 1** added the function library and conditionals on top of the XPath
1.0 engine.

**Phase 2a** adds the **sequence type**, and with it the constructs that
needed it: sequence construction, ranges, `for`, `some`, `every`, and the
functions that produce or consume sequences.

**Phase 2b** — date and time types, and value comparisons (`eq`, `ne`, `lt`,
`le`, `gt`, `ge`) — remains roadmap item 1 in [roadmap.md](roadmap.md).

### The sequence type, and why XPath 1.0 is unaffected

XPath 2.0 replaces the node-set with the sequence: an ordered, possibly
heterogeneous list of items, where an item is a node or an atomic value.
Sequences do not nest — building one out of others flattens them.

This crate keeps the node-set as well, and adds `Value::Sequence` beside it.
The invariant that makes that safe is:

> **A `Value::Sequence` is unreachable under XPath 1.0.** Nothing in the 1.0
> grammar or function library can construct one.

So an XPath 1.0 expression evaluates through exactly the code it did before,
with exactly the same results. The sequence type is additive, not a
replacement, and the 1.0 engine cannot tell it exists.

A path expression still yields a node-set rather than a sequence of nodes.
The two behave alike for every operation the crate supports, and keeping
paths on the node-set means the whole of XPath 1.0 stays on its original,
exact code path.

### Effective boolean value

A sequence in a boolean position uses XPath 2.0's effective boolean value:

| Sequence | Result |
|---|---|
| empty | false |
| first item is a node | true |
| exactly one boolean | itself |
| exactly one string | non-empty |
| exactly one number | not zero and not NaN |
| anything else | **type error** |

The last row is a genuine XPath 2.0 type error, and the crate raises it rather
than guessing — so `if (1, 2) then …` fails instead of quietly taking a
branch.

## What is implemented

Available only when the schema declares `queryBinding="xslt2"` or
`"xpath2"`. Under an XPath 1.0 binding these are errors, so a 1.0 schema
cannot accidentally acquire 2.0 behaviour.

### Syntax

| Construct | Notes |
|---|---|
| `if (E) then E else E` | Both branches required, as XPath 2.0 requires |
| `(E, E, E)` | Sequence construction; nested sequences flatten |
| `E to E` | An ascending range of integers; descending yields the empty sequence |
| `for $v in E return E` | Iterates a sequence or node-set, yielding a sequence |
| `some $v in E satisfies E` | True when any item satisfies the test |
| `every $v in E satisfies E` | True when every item does; true for an empty input |

### Functions

| Function | Notes |
|---|---|
| `matches(input, pattern)` | Regular expression test |
| `matches(input, pattern, flags)` | Flags `i`, `m`, `s`, `x` |
| `replace(input, pattern, replacement)` | `$1`…`$9` in the replacement |
| `replace(input, pattern, replacement, flags)` | |
| `upper-case(string)`, `lower-case(string)` | |
| `ends-with(string, string)` | The counterpart of 1.0's `starts-with` |
| `abs(number)` | |
| `min(node-set)`, `max(node-set)`, `avg(node-set)` | Numeric, over a node-set |
| `exists(node-set)`, `empty(node-set)` | |
| `string-join(sequence, separator)` | Joins the string values, in order |
| `tokenize(input, pattern)` | Splits on a regular expression, yielding a sequence |
| `tokenize(input, pattern, flags)` | |
| `distinct-values(sequence)` | Removes duplicates, keeping first-seen order |
| `index-of(sequence, value)` | The one-based positions of matching items |
| `count`, `exists`, `empty`, `min`, `max`, `avg`, `sum` | Accept a sequence as well as a node-set |

## What is not implemented

Every one of these is a **hard error naming the construct**, at schema-compile
time. None of them silently does something else.

| Construct | Why it needs phase 2b |
|---|---|
| Value comparisons: `eq`, `ne`, `lt`, `le`, `gt`, `ge` | Different semantics from `=`, `<`, and the rest |
| `instance of`, `cast as`, `castable as`, `treat as` | Needs the type system |
| Date and time types, `current-date()`, `current-dateTime()` | Needs those types and their arithmetic |
| Sequence types in general — `element()`, `item()*` | Needs the type system |
| `xslt3`, `xpath3`, `xpath31` bindings | Still refused; use `allow_unknown_query_binding` |

## Divergences: where 2.0 still behaves like 1.0

**This is the part to read.** Under a 2.0 binding, expressions still evaluate
on the XPath 1.0 engine, so constructs shared by both languages keep 1.0
semantics. For untyped XML — which is every document Schematron validates,
since the crate does no schema-aware processing — the two agree in almost
every case. They do not agree here:

| Expression | XPath 1.0, and this crate | XPath 2.0 |
|---|---|---|
| `1 + 'a'` | `NaN`, so the test is false | Type error |
| `'x' div 2` | `NaN` | Type error |
| `string(a)` where `a` selects several nodes | The first node's string value | Type error |
| `a = b` on node-sets | Existential over string values | Existential over atomized values |
| Untyped comparison | Always via string or number | Depends on the static type |

The pattern is consistent: **where XPath 2.0 raises a type error, this crate
produces `NaN` or picks the first node.** A schema that depends on a type
error to fail an assertion will pass here instead.

That is a real difference and it is why this document exists rather than a
line in a table. If it matters to you, do not declare `xslt2` — or wait for
phase 2.

## Regular expressions

`matches()` and `replace()` use the `regex` crate, whose syntax is close to
but not identical with the XML Schema regular expressions XPath 2.0
specifies.

Supported: character classes, quantifiers, alternation, groups, anchors, the
`\d \w \s` shorthands and their negations, Unicode categories via `\p{…}`.

Not supported, and an error rather than a wrong match: backreferences and
lookaround, neither of which XML Schema regular expressions have either. The
XML Schema shorthands `\i` and `\c`, and character-class subtraction
`[a-z-[aeiou]]`, are **not** translated and will fail to compile as patterns.

A **literal** pattern that does not compile is an error when the schema loads,
naming the pattern — the schema fails before it touches a document. A pattern
computed at runtime, as in `matches(@x, @pattern)`, cannot be checked that
early and is validated when it is evaluated. Either way it is an error, never
a silently false test.

## Using it

```xml
<schema xmlns="http://purl.oclc.org/dsdl/schematron" queryBinding="xslt2">
  <pattern>
    <rule context="invoice">
      <assert test="matches(@id, '^INV-[0-9]{4}$')">
        An invoice id must look like INV-0000.
      </assert>
      <assert test="if (@type = 'credit') then total &lt; 0 else total &gt;= 0">
        A credit note must have a negative total.
      </assert>
    </rule>
  </pattern>
</schema>
```

No feature flag and no option: declaring the binding is what enables it.
