# Schema linting

Schematron makes two mistakes very easy to write, and neither one produces an
error. The schema compiles, the validator runs, and nothing is reported —
which reads exactly like a clean document.

1. **A rule that can never fire**, because an earlier rule in the same pattern
   already claimed its nodes. First-matching-rule-wins is a feature, and it is
   also a trapdoor.
2. **An unprefixed name in a namespaced vocabulary.** XPath 1.0 has no default
   namespace, so `context="invoice"` matches elements in *no* namespace and a
   namespaced document matches nothing at all.

Linting is the crate's answer. It inspects a compiled schema and reports
constructs that are legal but almost certainly not what the author meant.

## What linting is not

A lint is **not** a validation finding, and **not** a compile error. It is a
remark about the schema, produced without looking at any document. A schema
with lints is still valid and still runs.

The crate never refuses to run a schema because it has lints. The decision to
treat them as fatal belongs to the caller — see the exit code in
[cli.md](cli.md).

## The lints

Each carries a `LintKind`, a location inside the schema, a message, and a
`help` line saying what to do about it.

| Kind | Reports |
|---|---|
| `UnreachableRule` | A rule that no node can reach, because an earlier rule in the same pattern claims everything it would |
| `DuplicateRuleContext` | Two rules in one pattern with the same `@context`; the second can never fire |
| `UnprefixedNameInNamespacedSchema` | A rule context or test using an unprefixed element name, in a schema that declares namespace prefixes |
| `UnreferencedDiagnostic` | A `diagnostic` no assertion references |
| `UnreferencedProperty` | A `property` no assertion references |
| `EmptyMessage` | An `assert` or `report` whose message is empty or whitespace |
| `ConstantTest` | A test that is a constant, such as `true()` or `false()`, and so does not depend on the document |
| `PatternInNoPhase` | A pattern that no phase activates, in a schema that declares phases |
| `UnreferencedKey` | A `key` that no expression looks up; its index is built regardless |
| `UnreferencedVariable` | A `let` whose name no expression mentions; its value is computed regardless |
| `RuleWithNoAssertions` | A rule that matches nodes and reports nothing |
| `PatternWithNoRules` | A pattern that cannot do anything |
| `DuplicateAssertionTest` | Two assertions in one rule with the same test |
| `PhaseWithNoPatterns` | A phase that activates nothing, so selecting it validates nothing |

### `UnreachableRule` is deliberately conservative

Deciding in general whether one XPath pattern subsumes another is not
practical. The linter therefore reports only the cases that are certain:

- An earlier rule whose context is `*`, which claims every element.
- An earlier rule whose context is `node()`, which claims every node.
- An earlier rule whose context is `@*`, which claims every attribute.

A narrower rule *after* one of those is unreachable, and that is the shape the
mistake almost always takes. Subtler shadowing — `a/b` after `a` — is not
reported, because it depends on the document and the answer would be a guess.

False positives are worse than misses here: a linter that cries wolf gets
switched off, and then it catches nothing at all.

### The "unreferenced" lints are conservative

`UnreferencedVariable` asks whether the name appears in *any* expression in
the schema, not whether it is in scope at the point it is used. A rule-level
`let` that nothing in its own rule uses, while a different rule happens to
reference the same name, is therefore not reported.

That is the deliberate direction to err in. Getting scope exactly right would
mean modelling shadowing across four nested scopes to report something that
costs a little time; getting it wrong would mean reporting a variable that is
used, which is how a linter loses its reader's trust.

### `UnprefixedNameInNamespacedSchema` is a hint, not a verdict

A schema that declares `<ns>` and then uses an unprefixed name *might* be
correct — documents legitimately mix namespaced and non-namespaced elements.
The lint fires anyway, because the mistake is common enough and silent enough
that a false positive costs a moment and a miss costs an afternoon.

A schema that declares no prefixes at all never triggers it.

## Library API

```rust
let schema = Schema::from_path("rules.sch")?;

for lint in schema.lint() {
    println!("{}: {}", lint.location, lint.message);
    if let Some(help) = &lint.help {
        println!("  help: {help}");
    }
}
```

```rust
pub struct Lint {
    pub kind: LintKind,
    pub location: String,
    pub message: String,
    pub help: Option<String>,
}

impl Schema {
    pub fn lint(&self) -> Vec<Lint>;
}
```

Lints come back in schema order, so the output reads down the file.

## Command line

```sh
schematron --schema rules.sch --lint
```

Prints the lints and exits: `0` when the schema is clean, `1` when anything
was reported, so a build can gate on it. No document is needed.

## Relationship to `--explain`

[`--explain`](cli.md) prints what the compiled schema *will do* — patterns,
rules, contexts, tests — and notes which rules can only see nodes no earlier
rule claimed. `--lint` is the automated form of reading that output and
noticing something wrong. Use `--explain` to understand a schema, `--lint` to
check one.
