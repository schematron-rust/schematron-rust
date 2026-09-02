# XPath 2.0 support

The `xslt2` and `xpath2` query bindings are XPath 2.0, which is not a
superset of XPath 1.0 with extra functions — it is a different language with
a different type system. This document states exactly how much of it the
crate implements, and, more importantly, **where it still behaves like XPath
1.0 even though a schema declares 2.0**.

Read the divergences section before relying on this. It is short and it
matters.

## Status: phases 1 through 6

Schematron schemas in the wild declare `xslt2` far more often than they use
the parts of XPath 2.0 that are genuinely incompatible with 1.0. The
implementation targets that gap, in order of how much the constructs are
actually used.

**Phase 1** added the function library and conditionals on top of the XPath
1.0 engine.

**Phase 2a** adds the **sequence type**, and with it the constructs that
needed it: sequence construction, ranges, `for`, `some`, `every`, and the
functions that produce or consume sequences.

**Phase 2b** adds the **date and time types**, which is what lets a schema
say the thing Schematron is most often quoted saying: that a date must be in
the past.

**Phase 2c** adds the **value comparisons** — `eq`, `ne`, `lt`, `le`, `gt`,
`ge` — which are how XPath 2.0 says "compare exactly these two values", as
opposed to `=` and its family, which ask whether *some* pair matches.

**Phase 2d** adds **durations and date arithmetic**, so a schema can measure
the distance between two dates rather than only order them.

**Phase 2e** completes the subset: node comparisons, duration scaling, and a
configurable implicit timezone.

**Phase 3** adds the **type operators**: `instance of`, `cast as`,
`castable as` and `treat as`, with the sequence types they take.

**Phase 4** adds the **numeric type hierarchy**: `instance of` and its
companions now recognize `xs:integer`, `xs:decimal`, and `xs:float` as well
as `xs:double`, tracked for numeric literals and explicit casts. See
"Numbers, and what `instance of` reports" below for exactly what is, and
deliberately is not, tracked.

**Phase 5** adds the remaining **sequence-manipulating functions** —
`reverse()`, `subsequence()`, `insert-before()`, `remove()`, and
`unordered()`. None of these were blocked on the sequence type itself, which
phase 2a already shipped; they were simply unwritten.

**Phase 6** adds the **cardinality assertions and atomization** —
`zero-or-one()`, `one-or-more()`, `exactly-one()`, and `data()`. The first
three were an oversight rather than a deliberate gap: unlike every other
unimplemented XPath 2.0 function, they were not named anywhere, so calling
one reported "unknown function" rather than saying what was actually wrong.
`data()` atomizes a node to its typed value; since this crate does no
schema-aware processing, that value is always untyped atomic, which — like
every other untyped value in this engine — is represented as a plain string.

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
| `E eq E`, `ne`, `lt`, `le`, `gt`, `ge` | Value comparisons; see below |
| `E instance of T` | Whether a value matches a sequence type |
| `E castable as T` | Whether a value **could** be cast, without casting it |
| `E cast as T` | Casts, or raises an error |
| `E treat as T` | Passes the value through, or raises an error |
| `E is E` | Whether two expressions select the *same node* |
| `E << E`, `E >> E` | Whether one node precedes or follows another in document order |

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
| `reverse(sequence)` | The items in reverse order |
| `subsequence(sequence, start)` | Items from one-based `start` to the end |
| `subsequence(sequence, start, length)` | Both positions rounded via `fn:round`'s rule, halves upward |
| `insert-before(sequence, position, inserts)` | Splices `inserts` before one-based `position`; out-of-range clamps to an end |
| `remove(sequence, position)` | Drops the item at `position`; out of range leaves the sequence unchanged |
| `unordered(sequence)` | Returns the sequence unchanged — order is implementation-defined, and this is a conformant, deterministic choice |
| `zero-or-one(sequence)` | Passes through a sequence of 0 or 1 items; raises otherwise |
| `one-or-more(sequence)` | Passes through a sequence of 1 or more items; raises on the empty sequence |
| `exactly-one(sequence)` | Passes through a sequence of exactly 1 item; raises otherwise |
| `data(sequence)` | Atomizes: a node becomes its string value, everything else passes through |
| `count`, `exists`, `empty`, `min`, `max`, `avg`, `sum` | Accept a sequence as well as a node-set |
| `current-date()`, `current-dateTime()`, `current-time()` | Stable for a whole validation run; see below |
| `year-from-date`, `month-from-date`, `day-from-date` | Components of a date |
| `year-from-dateTime`, `month-from-dateTime`, `day-from-dateTime` | Components of a dateTime |
| `hours-from-dateTime`, `minutes-from-dateTime`, `seconds-from-dateTime` | Components of a dateTime |
| `hours-from-time`, `minutes-from-time`, `seconds-from-time` | Components of a time |
| `xs:date()`, `xs:dateTime()`, `xs:time()` | Constructors, when a prefix is bound to the XML Schema namespace |
| `xs:dayTimeDuration()`, `xs:yearMonthDuration()` | Duration constructors |
| `days-from-duration`, `hours-from-duration`, `minutes-from-duration`, `seconds-from-duration` | Components of a dayTimeDuration |
| `years-from-duration`, `months-from-duration` | Components of a yearMonthDuration |
| `timezone-from-date`, `timezone-from-dateTime`, `timezone-from-time` | The value's own timezone, as a dayTimeDuration, or the empty sequence |
| `implicit-timezone()` | The run's implicit timezone, as a dayTimeDuration |

## Durations and date arithmetic

### Two duration types, not one

XPath 2.0 has `xs:duration`, and also splits it into two subtypes:

| Type | Lexical form | Holds |
|---|---|---|
| `xs:yearMonthDuration` | `P1Y6M` | A number of months |
| `xs:dayTimeDuration` | `P90DT12H30M` | A number of seconds |

The split exists because a general `xs:duration` is **not totally ordered**:
is one month longer than thirty days? It depends which month. XPath 2.0
therefore leaves `xs:duration` only partially ordered, and this crate
implements the two subtypes rather than the general type — comparing values
that cannot be compared is worse than not offering the type.

Both may be negative, written `-P1D`.

### What the arithmetic does

| Expression | Result |
|---|---|
| date − date, dateTime − dateTime, time − time | `xs:dayTimeDuration` |
| date + dayTimeDuration, date − dayTimeDuration | the same date type |
| date + yearMonthDuration, date − yearMonthDuration | the same date type |
| duration + duration, duration − duration | the same duration type |
| duration × number, number × duration, duration ÷ number | the same duration type |
| duration ÷ duration | a number: how many of the second fit in the first |

So the constraint a schema actually wants to write is now writable:

```xml
<assert test="xs:date(@end) - xs:date(@start) le xs:dayTimeDuration('P90D')">
  A contract may not run for more than ninety days.
</assert>
```

Adding months clamps the day rather than overflowing, which is what XML
Schema requires: 31 January plus one month is 28 February, or 29 February in
a leap year.

Mixing the two duration types in one operation is a **type error**, for the
same reason the types are separate.

## Type operators

### `castable as` is the one to reach for

A Schematron schema usually wants to *check* a value, not convert it. Before
`castable as`, asking whether an attribute held a date meant calling
`xs:date()` — which raises an error and stops the run when it does not:

```xml
<!-- aborts validation if @signed is not a date -->
<assert test="xs:date(@signed) lt current-date()">…</assert>

<!-- reports the bad value as a finding, which is what a schema is for -->
<assert test="@signed castable as xs:date">
  <value-of select="@signed"/> is not a date.
</assert>
<assert test="not(@signed castable as xs:date) or xs:date(@signed) lt current-date()">
  A contract cannot be signed in the future.
</assert>
```

That is the difference between a schema that reports a bad date and a
validation run that dies on one.

### The types

| Written | Matches |
|---|---|
| `item()` | Anything |
| `node()`, `element()`, `attribute()`, `text()`, `comment()`, `processing-instruction()`, `document-node()` | A node of that kind |
| `element(name)`, `attribute(name)` | A node of that kind with that name |
| `xs:string`, `xs:boolean` | An atomic value |
| `xs:date`, `xs:dateTime`, `xs:time` | A date or time |
| `xs:dayTimeDuration`, `xs:yearMonthDuration` | A duration |
| `xs:integer`, `xs:decimal`, `xs:float`, `xs:double` | See the note on numbers below |
| `xs:anyAtomicType` | Any atomic value; no node |
| `empty-sequence()` | Only the empty sequence |

An occurrence indicator may follow: `?` for zero or one, `*` for zero or
more, `+` for one or more. With none, the type matches exactly one item.

These are types, written after `instance of` and its companions. The same
kind tests also work as **node tests inside a path**:

| Written | Selects |
|---|---|
| `element()` | every element, on whatever axis the step uses |
| `element(b)`, `element(*)` | an element of that name; `*` means any |
| `attribute()`, `attribute(id)` | an attribute, named or not |
| `document-node()` | the root node |

`element()` is not the same as `*`. A wildcard selects the axis's *principal*
node type — elements on `child`, attributes on `attribute` — while a kind test
names the kind outright, so `child::attribute()` correctly selects nothing.

One special rule comes with them, from XPath 2.0 section 3.2.1.1: a step whose
node test is an attribute kind test defaults to the **attribute** axis rather
than `child`. So `b/attribute()` means `b/attribute::attribute()`. Without
that rule the test could never match, because the child axis yields no
attributes. An axis written out is always respected.

Under an XPath 1.0 binding these are refused by name — "the `element()` kind
test is XPath 2.0 syntax" — rather than reported as an unknown function, which
would send the reader hunting for a typo.

`cast as` and `castable as` take a single type — an atomic type with an
optional `?` — because casting a sequence has no meaning.

### Casting is lexical

A cast checks the **lexical form** of the value, which is what XML Schema
specifies for untyped input and what a Schematron schema is nearly always
looking at:

| Expression | Result |
|---|---|
| `'2026-08-21' castable as xs:date` | true |
| `'2026-02-30' castable as xs:date` | false — not a real date |
| `'12' castable as xs:integer` | true |
| `'12.5' castable as xs:integer` | false |
| `'12.5' castable as xs:decimal` | true |
| `'x' castable as xs:double` | false |

An empty operand is `false` for `castable as`, and the empty sequence for
`cast as`.

### Numbers, and what `instance of` reports

The crate holds every number as an IEEE 754 double, as XPath 1.0 requires and
as the whole engine's arithmetic is built on — that never changes. What
*does* change, since phase 4, is that a number also carries a static
`xs:integer`/`xs:decimal`/`xs:float`/`xs:double` tag, tracked wherever XPath
2.0 defines it unambiguously and lexically: a numeric literal as written, and
the result of an explicit `cast as`/`castable as`.

A literal's type is lexical, not by value: `1` is `xs:integer` because it has
no decimal point; `1.0` is `xs:decimal` even though it equals the integer
`1`. `xs:integer` derives from `xs:decimal` by restriction in XML Schema, so
an integer also matches `instance of xs:decimal`; `xs:float` and `xs:double`
are separate primitive types, related to neither `xs:decimal` nor each
other, so each matches only itself:

| Expression | Result |
|---|---|
| `1 instance of xs:integer` | true |
| `1 instance of xs:decimal` | true — integer derives from decimal |
| `1 instance of xs:double` | false |
| `1.0 instance of xs:integer` | false — a decimal literal, lexically |
| `1.5 instance of xs:decimal` | true |
| `'1.5' cast as xs:float instance of xs:float` | true |
| `'1' castable as xs:integer` | true |

That `1 instance of xs:double` row corrects a documentation mistake this
section carried before phase 4: it used to claim real XPath 2.0 also answers
`true` there, on the reasoning that every number here was `xs:double` and
that happened to be a harmless-looking default. It isn't — `1` is an
`IntegerLiteral`, typed `xs:integer`, which does not derive from `xs:double`
— so real XPath 2.0 says `false`, and the crate now agrees.

**What stays deliberately untracked.** Arithmetic (`+ - * div mod`), every
function in the library, and `for`'s `to` ranges always produce `xs:double`,
never the operand's own type — this crate does not implement XPath 2.0's
full numeric type promotion (where, say, `1 + 1` would stay `xs:integer`).
Threading a type lattice through every arithmetic operation and every
numeric function is the risk [roadmap/](../roadmap/index.md) weighs phase 4
against; tracking only literals and casts closes the concrete gap above
without taking it on:

| Expression | Result |
|---|---|
| `(1 + 1) instance of xs:integer` | false — arithmetic always widens to double |
| `(1 + 1) instance of xs:double` | true |
| `count((1, 2)) instance of xs:double` | true |

Casting and `castable as` themselves are unaffected by any of this, because
they still read the lexical form rather than a tracked type — lexical is the
right reading for the untyped values a schema actually inspects.

## Node comparisons

Three operators that ask about nodes rather than values:

| Operator | True when |
|---|---|
| `A is B` | Both select the **same node** — identity, not equal content |
| `A << B` | `A` precedes `B` in document order |
| `A >> B` | `A` follows `B` in document order |

`is` is the one worth knowing. Two elements with identical content are equal
by `=` and are *not* the same node:

```xml
<!-- true: some b has the same string value as some c -->
<assert test="b = c">…</assert>

<!-- true only if they are literally the same element -->
<assert test="b is c">…</assert>
```

All three take exactly one node on each side. An empty operand yields the
empty sequence, so the comparison is false. More than one node, or anything
that is not a node, is a type error — the same strictness as the value
comparisons, for the same reason.

## Value comparisons

`=` and `eq` are not spellings of the same thing, and the difference is the
most useful part of XPath 2.0 for a schema author.

| | `=` (general) | `eq` (value) |
|---|---|---|
| Operands | Any number of items | Exactly one each |
| Question asked | Does *some* pair match? | Do *these two* match? |
| Several items | Existential, so `(1, 2) = 1` is true | **Type error** |
| Empty operand | False | The empty sequence, so false |
| Mismatched types | Coerced | **Type error** |

The reason to reach for `eq` is that it *fails* where `=` quietly succeeds:

```xml
<!-- true if ANY line has qty 1, which may not be what you meant -->
<assert test="line/@qty = 1">…</assert>

<!-- an error unless there is exactly one line, which is what you meant -->
<assert test="line/@qty eq 1">…</assert>
```

### Untyped operands are cast to string, not coerced

This is the rule that surprises people, and it is deliberate. A general
comparison casts an untyped operand to the *other* operand's type. A value
comparison casts it to `xs:string` and then requires the types to match:

| Expression, where `@n` is `"1"` | Result |
|---|---|
| `@n = 1` | true — `@n` is cast to a number |
| `@n eq "1"` | true — both are strings |
| `@n eq 1` | **type error** — string against number |
| `@d eq xs:date('2020-01-01')` | **type error** — string against date |

Everything in an XML document is untyped, so comparing an attribute to a
number with `eq` is an error. That is not a limitation to work around; it is
`eq` telling you the comparison you wrote is not the one you meant. Write
`number(@n) eq 1`, or use `=`.

### Chained comparisons

XPath 2.0 makes comparisons non-associative, so `a eq b eq c` is a syntax
error. This crate parses it left-associatively, as `(a eq b) eq c`, which then
fails at evaluation as a boolean compared against something else. The outcome
is an error either way; only the message differs.

## Dates and times

### The types

`xs:date`, `xs:dateTime` and `xs:time`, in their XML Schema lexical forms:

```
2026-08-21              a date
2026-08-21Z             a date in UTC
2026-08-21+01:00        a date at an offset
2026-08-21T10:30:00     a dateTime
2026-08-21T10:30:00.5Z  a dateTime with fractional seconds
10:30:00                a time
```

A value with no timezone is compared as if it were in the **implicit
timezone**, which defaults to UTC and can be set:

```rust
let options = ValidateOptions::new().with_implicit_timezone(-5 * 60);
```

XPath 2.0 takes the implicit timezone from the evaluation context, which for
most processors means the machine's local offset. Defaulting to UTC instead
makes a validation run reproducible on any machine — the same reason the clock
is captured once and can be supplied — while leaving the choice available to a
caller who needs local semantics.

### Comparing an untyped value to a date

The point of the feature. An attribute in an XML document is untyped, so
comparing it to a date casts it to a date first, exactly as XPath 2.0
specifies for untyped atomic operands:

```xml
<assert test="xs:date(@ContractDate) &lt; current-date()">
  A contract date must be in the past.
</assert>
```

and equally, without the constructor, because the untyped operand takes its
type from the other side:

```xml
<assert test="@ContractDate &lt; current-date()">
  A contract date must be in the past.
</assert>
```

An untyped value that does not parse as the other operand's type is an
**error naming the value**, not a silently false test. A date typo should fail
loudly; that is the whole reason for checking it.

### The clock is captured once, and can be supplied

`current-date()`, `current-dateTime()` and `current-time()` must return the
same instant throughout one validation, which XPath 2.0 requires and which
also stops a rule contradicting itself halfway down a document.

The instant is therefore read **once per validation run**, not per call. And
because a validator whose result depends on the wall clock cannot be tested
or reproduced, it can be supplied:

```rust
let options = ValidateOptions::new().with_current_time(fixed_instant);
```

With no instant supplied, the system clock is read once at the start of the
run. With one supplied, the run is deterministic — which is how this crate's
own tests for date rules are written, and how a caller should write theirs.

## What is not implemented

Every one of these is a **hard error naming the construct**, at schema-compile
time. None of them silently does something else.

| Construct | Why not |
|---|---|
| Schema-aware types — `element(name, type)`, `schema-element()` | Needs a schema processor, which is out of scope |
| The general `xs:duration` | Only partially ordered; see above |
| `adjust-date-to-timezone()` and its companions | Needs a timezone-bearing cast |
| `for-each()` | Not actually XPath 2.0: it takes a function item, an XPath 3.0 feature this crate has no representation for. Unlike its five neighbours in the table above, this one isn't a matter of writing the function. |
| `xslt3`, `xpath3`, `xpath31` bindings | Still refused; use `allow_unknown_query_binding` |

## Divergences: where 2.0 still behaves like 1.0

**This is the part to read.** Under a 2.0 binding, expressions still evaluate
on the XPath 1.0 engine, so constructs shared by both languages keep 1.0
semantics. For untyped XML — which is every document Schematron validates,
since the crate does no schema-aware processing — the two agree in almost
every case. They do not agree here:

| Expression | XPath 1.0, and this crate | XPath 2.0 |
|---|---|---|
| A date with no timezone | Compared in the implicit timezone, which **defaults to UTC** rather than to the machine's local offset | Compared in the processor's implicit timezone |
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
