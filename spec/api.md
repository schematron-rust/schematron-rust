# Library API

The whole crate is reachable through a handful of types. The short path:

```rust
use schematron::{Schema, Document};

let schema = Schema::from_path("rules.sch")?;
let doc = Document::from_path("data.xml")?;
let report = schema.validate(&doc)?;

if report.is_valid() {
    println!("valid");
} else {
    for failure in report.failures() {
        println!("{}: {}", failure.location, failure.text);
    }
}
```

## `Schema`

A compiled, immutable, thread-safe schema.

```rust
impl Schema {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Schema>;
    pub fn from_str(source: &str) -> Result<Schema>;
    pub fn from_str_with(source: &str, options: &SchemaOptions) -> Result<Schema>;

    pub fn validate(&self, document: &Document) -> Result<Report>;
    pub fn validate_with(&self, document: &Document, options: &ValidateOptions) -> Result<Report>;

    pub fn id(&self) -> Option<&str>;
    pub fn title(&self) -> Option<&str>;
    pub fn query_binding(&self) -> QueryBinding;
    pub fn phases(&self) -> impl Iterator<Item = &str>;
    pub fn default_phase(&self) -> Option<&str>;

    /// Whether any expression calls XPath `document()`.
    pub fn uses_document_function(&self) -> bool;
}
```

`Schema: Send + Sync`. Compile once, share across threads, validate in
parallel.

## `Document`

```rust
impl Document {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Document>;
    pub fn from_str(source: &str) -> Result<Document>;
    pub fn from_bytes(bytes: &[u8]) -> Result<Document>;
    pub fn root(&self) -> NodeId;
    pub fn document_element(&self) -> Option<NodeId>;
}
```

## `SchemaOptions`

| Option | Default | Effect |
|---|---|---|
| `base_uri` | derived from the path | Base for resolving `include` hrefs |
| `resolver` | `FileResolver` | How `include` and `pattern/@documents` fetch a URI |
| `max_include_depth` | 64 | Cycle and blow-up guard |
| `allow_unknown_query_binding` | `false` | Compile an `xslt2`/`xslt3` schema anyway, best effort |

Each has a builder: `SchemaOptions::new().with_resolver(…).with_base_uri(…)`.

### Resolvers

```rust
pub trait Resolver: Debug + Send + Sync {
    fn resolve(&self, href: &str, base: Option<&str>) -> Result<String>;
    fn rebase(&self, href: &str, base: Option<&str>) -> Option<String> { /* … */ }
}
```

Two are provided. `FileResolver` reads local files, resolving relative hrefs
against the including document, and refuses `http:` and `https:` URIs — the
library never reaches the network on your behalf. `MemoryResolver` serves from
a map, for embedding a schema and its includes in a binary, and for tests.
Implementing your own is one method.

The resolver serves three things: `include`, `pattern/@documents`, and XPath
`document()`. Supplying one is therefore the single place to control every
external fetch a schema can make.

## `ValidateOptions`

| Option | Default | Effect |
|---|---|---|
| `phase` | `PhaseSelection::Default` | `Default`, `All`, or `Named(String)` |
| `max_failures` | none | Stop after N failed assertions |
| `record_fired_rules` | `true` | Set false to skip bookkeeping for rules that found nothing |
| `parallel_patterns` | `false` | Evaluate patterns on separate threads; the report is unchanged. See [validation.md](validation.md) |
| `current_time` | system clock, read once | The instant `current-date()` reports. Set it to make a run with date rules reproducible |
| `implicit_timezone` | UTC | The timezone a date with no offset is read as being in. See [xpath2.md](xpath2.md) |

`PhaseSelection::from("#ALL")`, `from("#DEFAULT")`, and `from("my-phase")` all
do what you would expect, so a string from a config file can be passed
straight through.

## `Report`

The result is data, not text.

```rust
pub struct Report {
    pub title: Option<String>,
    pub phase: Option<String>,
    pub schema_version: Option<String>,
    pub namespaces: Vec<Ns>,
    pub patterns: Vec<ActivePattern>,
}

pub struct ActivePattern { pub id, pub name, pub documents, pub rules: Vec<FiredRule> }
pub struct FiredRule { pub id, pub context, pub role, pub flag, pub location, pub assertions: Vec<AssertionResult> }

pub struct AssertionResult {
    pub kind: AssertionKind,        // FailedAssert | SuccessfulReport
    pub test: String,
    pub location: String,
    pub text: String,
    pub id: Option<String>,
    pub role: Option<String>,
    pub flag: Option<String>,
    pub see: Option<String>,
    pub icon: Option<String>,
    pub fpi: Option<String>,
    pub diagnostics: Vec<DiagnosticResult>,
    pub properties: Vec<PropertyResult>,
}
```

Accessors:

```rust
impl Report {
    pub fn is_valid(&self) -> bool;                 // no failed asserts
    pub fn failures(&self) -> impl Iterator<Item = &AssertionResult>;
    pub fn reports(&self) -> impl Iterator<Item = &AssertionResult>;
    pub fn assertions(&self) -> impl Iterator<Item = &AssertionResult>;
    pub fn with_flag<'a>(&'a self, flag: &'a str) -> impl Iterator<Item = &'a AssertionResult>;
    pub fn count_failures(&self) -> usize;

    pub fn fired_rules(&self) -> impl Iterator<Item = &FiredRule>;
    pub fn count_fired_rules(&self) -> usize;

    pub fn to_svrl(&self) -> String;
    pub fn to_svrl_with(&self, options: &SvrlOptions) -> String;
    pub fn to_text(&self) -> String;
    pub fn to_text_with(&self, options: &TextOptions) -> String;
    pub fn to_json(&self) -> Result<String, serde_json::Error>;   // feature = "serde"
}
```

`count_fired_rules()` is the debugging tool: a count of zero on a non-empty
document means no rule context matched anything, which is almost always a
missing `ns` prefix binding rather than a document that happens to be clean.

`is_valid()` counts only `FailedAssert`. A `successful-report` is an
observation, not a failure — that is what the standard means by "report", and
conflating the two is the most common way to misuse Schematron.

## Features

| Feature | Default | Effect |
|---|---|---|
| `serde` | on | `Serialize`/`Deserialize` on the schema model and the report, and `Report::to_json` |
| `cli` | on | Builds the `schematron` binary; pulls in `clap` |

Turn both off for the smallest possible dependency footprint: the crate then
depends only on `quick-xml` and `thiserror`.

## Lower layers

The XML and XPath layers are public because they are useful on their own:

```rust
use schematron::xml::Document;
use schematron::xpath::{evaluate, parse, EvalContext, Namespaces, Variables};

let document = Document::from_str("<a><b>1</b><b>2</b></a>")?;
let expr = parse("count(b)")?;
let variables = Variables::new();
let namespaces = Namespaces::new();
let context = EvalContext::new(&document, document.document_element().unwrap(), &variables, &namespaces);
assert_eq!(evaluate(&expr, &context)?.to_number(&document), 2.0);
# Ok::<(), Box<dyn std::error::Error>>(())
```

See `examples/xpath_engine.rs`.

## Errors

Everything returns `schematron::Result<T>` = `Result<T, schematron::Error>`.
See [errors.md](errors.md).
