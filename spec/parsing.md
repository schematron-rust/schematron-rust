# Schema parsing and compilation

Turning a `.sch` file into something runnable happens in five passes. Each
pass is a separate function so that each can be tested and each can report
errors that name a source location.

```
bytes ──1── XML tree ──2── includes resolved ──3── model ──4── abstracts
      expanded ──5── compiled Schema (XPath pre-parsed)
```

## Pass 1 — parse as XML

The schema is an XML document. It is parsed with the crate's own parser
([xml.md](xml.md)). A schema that is not well-formed XML fails here.

## Pass 2 — resolve `include` and `extends href`

`<sch:include href="U"/>` is replaced, in place, by the *document element* of
the document at `U`. `U` is resolved against the base URI of the including
document, so relative includes nest correctly.

Rules:

- Includes are resolved depth first, before the model is built, so an included
  fragment may itself contain includes.
- A cycle is an error naming the full chain, not a stack overflow.
- Inclusion depth is capped (default 64) as a denial-of-service guard.
- `href` is resolved by the crate's `Resolver`, which by default reads the
  local filesystem relative to the schema and refuses absolute `http(s)` URIs.
  Network access is opt-in, never implicit.

`<sch:extends href="U"/>` is **not** implemented; use `include`, which can
splice in a `rule` element just as well. `<sch:extends rule="ID"/>` is the
supported form and is resolved in pass 4.

## Pass 3 — build the model

The tree is walked and mapped onto the types in [data-model.md](data-model.md).
This pass validates the schema against Schematron's own content model:

- Unknown elements in the Schematron namespace are errors.
- Elements from foreign namespaces are ignored, per the standard, which allows
  schemas to carry annotations from other vocabularies.
- Required attributes are checked here: `ns/@prefix`, `ns/@uri`,
  `assert/@test`, `report/@test`, `let/@name`, `param/@name`, `param/@value`,
  `active/@pattern`, `phase/@id`, `diagnostic/@id`.
- Mutually exclusive attributes are checked here: a rule may not be both
  abstract and have a context; a pattern may not be both abstract and `is-a`.

## Pass 4 — expand abstractions

Two independent expansions, in this order.

### Abstract rules

For each `<extends rule="R"/>` in a concrete rule, the assertions and `let`
bindings of abstract rule `R` are spliced into the extending rule **at the
position of the `extends` element**, so ordering — which determines report
order — is preserved. Extension is transitive; a cycle is an error.

### Abstract patterns

For each pattern with `is-a="P"`:

1. Look up abstract pattern `P`. A missing target is an error.
2. Collect the instance's `param` children into a name→value map.
3. Deep-copy `P`'s rules into the instance.
4. In every `@context`, `@test`, `@select`, `@value`, `@subject`, and in the
   `select` of every `value-of`, textually replace each `$name` with the
   corresponding value.

The substitution rule is deliberately narrow, matching the reference
implementation: a `$` followed by a valid XPath QName, where that QName is a
declared parameter. `$name` inside a string literal is still substituted —
this is textual substitution, as the standard defines it. A `$` followed by
an undeclared name is left alone, so it can still resolve as a `let` variable.
Substitution is single-pass: a value containing `$x` does not re-expand.

After this pass, no abstract pattern and no abstract rule remains in the
runnable set.

## Pass 5 — compile expressions

Every XPath expression in the schema is lexed and parsed **once**, at compile
time, and cached on the schema, keyed by its source text — so two rules
sharing a test share one compiled expression. The expressions compiled are:

- `rule/@context`, additionally checked for being a legal XSLT pattern
- `assert/@test`, `report/@test`
- `let/@value`
- `value-of/@select`, `name/@path`
- `pattern/@documents`, `rule/@subject`, `assert/@subject`

This pass also checks everything that can be checked without a document:

- Function names and arities, including naming XPath 2.0 functions as such.
- Namespace prefixes, against the schema's `ns` declarations. The error lists
  the prefixes the schema *does* declare.
- `assert/@diagnostics` and `assert/@properties` references.
- `active/@pattern` and `schema/@defaultPhase` references.
- That each `rule/@context` is a legal XSLT match pattern.

Variable references are *not* checked here: `let` scope is dynamic, so an
unbound `$x` is caught during validation, where the scope is actually known.

Consequences: a syntax error, an unknown function, or a prefix with no `ns`
declaration is reported when the schema is loaded, naming the element and the
expression — not silently at validation time. And validating N documents with
one schema parses each expression once, not N times. The
`compile_once_validate_many` benchmark exists to keep that honest.

The result is a `Schema`, which is `Send + Sync` and immutable, so one
compiled schema can validate documents on many threads concurrently.
