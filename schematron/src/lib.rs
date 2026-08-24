//! Schematron: rule-based XML validation, in pure Rust.
//!
//! This crate implements ISO/IEC 19757-3 Schematron with no C dependency, no
//! XSLT processor, and no FFI. It contains its own XML parser, its own XPath
//! 1.0 engine, and its own validator, so a Schematron schema is *interpreted*
//! directly rather than transpiled into XSLT the way the reference
//! implementation does.
//!
//! # What Schematron is for
//!
//! Grammar languages — DTD, XML Schema, RELAX NG — describe the shape a
//! document may take. Schematron describes constraints the document must
//! satisfy, written as XPath expressions, so it can express things a grammar
//! cannot: co-occurrence rules, value relationships, cross-references, and
//! conditions that span documents.
//!
//! # Example
//!
//! ```
//! use schematron::{Document, Schema};
//!
//! let schema = Schema::from_str(r#"
//!     <schema xmlns="http://purl.oclc.org/dsdl/schematron">
//!       <pattern>
//!         <rule context="invoice">
//!           <assert test="total">An invoice must have a total.</assert>
//!         </rule>
//!       </pattern>
//!     </schema>
//! "#)?;
//!
//! let good = Document::from_str("<invoice><total>10</total></invoice>")?;
//! assert!(schema.validate(&good)?.is_valid());
//!
//! let bad = Document::from_str("<invoice/>")?;
//! let report = schema.validate(&bad)?;
//! assert!(!report.is_valid());
//! assert_eq!(report.failures().next().unwrap().text, "An invoice must have a total.");
//! # Ok::<(), schematron::Error>(())
//! ```
//!
//! # The one rule to internalise
//!
//! Within a single pattern, each node is processed by **at most one** rule:
//! the first whose context matches it. Rules in one pattern compete like the
//! arms of a match expression; rules in different patterns do not. Putting
//! two independent checks for the same element in one pattern silently
//! disables the second. See `spec/validation/`.
//!
//! # Layout
//!
//! - [`xml`] — the XML parser and XPath data model
//! - [`xpath`] — the XPath 1.0 engine
//! - [`schema`] — the Schematron document model and its compiler
//! - [`validate`] — the validator and its report types
//! - [`svrl`] — SVRL report output
//!
//! Most callers need only [`Schema`], [`Document`], and [`Report`].

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod lint;
pub mod schema;
pub mod svrl;
pub mod text;
pub mod validate;
pub mod xml;
pub mod xpath;

pub use error::{Error, Result};
pub use lint::{Lint, LintKind};
pub use schema::{Schema, SchemaOptions};
pub use svrl::SvrlOptions;
pub use text::TextOptions;
pub use validate::{PhaseSelection, Report, ValidateOptions};
pub use xml::Document;
