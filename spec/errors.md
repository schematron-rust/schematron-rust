# Errors

One error type, `schematron::Error`, built with `thiserror`. Every variant
names *what* failed and *where*, because a schema error that says only
"invalid XPath" costs the user more time than the crate saved them.

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error for {path}: {source}")]
    Io { path: String, source: std::io::Error },

    #[error("XML parse error at line {line}, column {column}: {message}")]
    XmlParse { line: usize, column: usize, message: String },

    #[error("schema error in <{element}>{}: {message}", OptionalAt(.location))]
    Schema { element: String, location: Option<String>, message: String },

    #[error("XPath syntax error in {context}: {message}\n  expression: {expression}\n  {caret}")]
    XPathSyntax { context: String, expression: String, position: usize, message: String, caret: String },

    #[error("XPath evaluation error in {context}: {message}")]
    XPathEval { context: String, message: String },

    #[error("unknown phase {phase:?}; the schema defines: {available}")]
    UnknownPhase { phase: String, available: String },

    #[error("unsupported query binding {binding:?}; this crate implements XPath 1.0 (xslt, xpath)")]
    UnsupportedQueryBinding { binding: String },

    #[error("include cycle: {chain}")]
    IncludeCycle { chain: String },

    #[error("include depth limit of {limit} exceeded at {href}")]
    IncludeDepth { limit: usize, href: String },

    #[error("cannot resolve {href}: {message}")]
    Resolve { href: String, message: String },
}
```

## Error message quality

An XPath syntax error prints a caret under the offending character:

```
XPath syntax error in rule[@context='invoice']/assert[1]/@test: expected ')'
  expression: count(line[@qty > 0) > 0
                                 ^
```

The `context` string is a human-readable path to the schema construct, built
during compilation: `pattern[@id='lines']/rule[@context='line']/assert[2]/@test`.

`Error` is `#[non_exhaustive]`, so variants can be added without a breaking
change; match with a `_` arm.

## What is an error versus a finding

A **finding** is a failed assertion: the document broke a rule. It goes in the
`Report`, not in `Result::Err`.

An **error** is the crate being unable to do its job: the schema is malformed,
the document is not well-formed, an expression does not compile, a variable is
unbound. It goes in `Result::Err`.

A false assertion is never an error, and an error is never silently converted
into a false assertion. Doing the latter would let a broken schema report a
clean bill of health.
