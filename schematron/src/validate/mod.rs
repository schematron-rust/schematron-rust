//! Validating documents, and the report that comes back.
//!
//! The algorithm is in `spec/validation/`. The one rule to carry in your
//! head: **within a single pattern, each node is processed by at most one
//! rule — the first whose context matches it.** Rules in one pattern compete
//! like the arms of a match expression; rules in different patterns do not.
//!
//! # Examples
//!
//! ```
//! use schematron::{Document, Schema};
//! use schematron::validate::{PhaseSelection, ValidateOptions};
//!
//! let schema = Schema::from_str(r#"
//!     <schema xmlns="http://purl.oclc.org/dsdl/schematron">
//!       <phase id="strict"><active pattern="totals"/></phase>
//!       <pattern id="totals">
//!         <rule context="invoice">
//!           <assert test="total">An invoice must have a total.</assert>
//!         </rule>
//!       </pattern>
//!     </schema>
//! "#)?;
//!
//! let document = Document::from_str("<invoice/>")?;
//! let options = ValidateOptions::new().with_phase(PhaseSelection::Named("strict".into()));
//! let report = schema.validate_with(&document, &options)?;
//!
//! assert_eq!(report.count_failures(), 1);
//! assert_eq!(report.failures().next().unwrap().location, "/invoice[1]");
//! # Ok::<(), schematron::Error>(())
//! ```

mod engine;
mod options;
mod report;

pub use options::{PhaseSelection, ValidateOptions};
pub use report::{
    ActivePattern, AssertionResult, DiagnosticResult, FiredRule, PropertyResult, Report, ResultKind,
};

pub(crate) use engine::validate;
