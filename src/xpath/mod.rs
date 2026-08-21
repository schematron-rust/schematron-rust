//! A pure Rust XPath 1.0 engine.
//!
//! Schematron's `xslt` and `xpath` query language bindings are XPath 1.0, so
//! validating a schema means evaluating XPath. This module implements the
//! language in full — all thirteen axes, all twenty-seven core functions, and
//! XPath 1.0's conversion and comparison semantics — with no external XPath
//! crate and no XSLT processor.
//!
//! The pipeline is: source text, then [`lexer`](self) tokens, then [`parse`]
//! into an [`Expr`], then [`evaluate`] against an [`EvalContext`].
//! Expressions are parsed once when a schema is compiled and evaluated many
//! times, which is where the performance comes from.
//!
//! See `spec/xpath.md` for the grammar and the semantics.
//!
//! # Examples
//!
//! ```
//! use schematron::xml::Document;
//! use schematron::xpath::{evaluate, parse, EvalContext, Namespaces, Variables};
//!
//! let doc = Document::from_str("<order><line qty='2'/><line qty='0'/></order>")?;
//! let expr = parse("count(line[@qty > 0])")?;
//!
//! let variables = Variables::new();
//! let namespaces = Namespaces::new();
//! let context = EvalContext::new(&doc, doc.document_element().unwrap(), &variables, &namespaces);
//!
//! assert_eq!(evaluate(&expr, &context)?.to_number(&doc), 1.0);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Surprises worth knowing
//!
//! - An unprefixed name matches **no namespace**. There is no default
//!   namespace in XPath 1.0, so a schema for a namespaced vocabulary must
//!   declare a prefix and use it everywhere.
//! - `a != b` is not `not(a = b)` when node-sets are involved: both can be
//!   true at once, because each is existential over the node-set.
//! - Every relational comparison converts to number, so `'x' > 0` is false
//!   rather than an error — the string becomes NaN.
//!
//! # Reaching other documents
//!
//! `document(uri)` returns the root nodes of external documents, so a
//! node-set can span documents. Because node handles are indices into one
//! arena, a loaded document is merged into that arena beside the instance,
//! keeping its own root — so `/` and `id()` still mean "this document" from
//! inside it.
//!
//! Loading is driven by the validator, which supplies a [`Documents`]
//! registry. Evaluating an expression directly through this module has no
//! registry, and `document()` is then an error rather than an empty node-set:
//! silently returning nothing would turn a broken lookup into a passing
//! assertion. See `spec/xpath.md`.

mod ast;
mod context;
mod eval;
mod functions;
mod lexer;
mod parser;
mod value;
mod version;

pub use ast::{
    Axis, BinaryOp, Expr, NameTest, NodeTest, PathExpr, PathStart, Quantifier, Step,
};
pub use context::{Documents, EvalContext, Namespaces, Variables};
pub use version::XPathVersion;
pub use eval::{evaluate, EvalError};
pub use functions::{check_function, check_regex, function_names, function_names_v2};
pub use parser::{parse, ParseError, MAX_RECURSION_DEPTH};
pub use value::{flatten_into_sequence, format_number, parse_number, Item, Value};
