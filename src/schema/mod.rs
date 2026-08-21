//! The Schematron document model and its compiler.
//!
//! A `.sch` file becomes a runnable [`Schema`] in five passes, described in
//! `spec/parsing.md`:
//!
//! 1. Parse the schema as XML.
//! 2. Resolve `include` and `extends href`.
//! 3. Build the [`SchemaModel`] and check Schematron's own content model.
//! 4. Expand abstract rules and abstract patterns.
//! 5. Parse every XPath expression, once, and check what can be checked
//!    statically: function names, arities, namespace prefixes, id references.
//!
//! The result is immutable and `Send + Sync`. Compile once, validate many.
//!
//! # Examples
//!
//! ```
//! use schematron::Schema;
//!
//! let schema = Schema::from_str(r#"
//!     <schema xmlns="http://purl.oclc.org/dsdl/schematron" defaultPhase="quick">
//!       <phase id="quick"><active pattern="structure"/></phase>
//!       <pattern id="structure">
//!         <rule context="invoice"><assert test="total">Needs a total.</assert></rule>
//!       </pattern>
//!     </schema>
//! "#)?;
//!
//! assert_eq!(schema.default_phase(), Some("quick"));
//! assert_eq!(schema.phases().collect::<Vec<_>>(), vec!["quick"]);
//! # Ok::<(), schematron::Error>(())
//! ```

mod compile;
mod expand;
mod include;
pub mod model;
mod parse;
mod resolver;

pub use compile::{Schema, SchemaOptions};
pub use model::{
    Assertion, AssertionKind, Content, Diagnostic, Let, LetValue, Ns, Paragraph, Param, Pattern,
    Phase, Property, QueryBinding, Rule, RuleChild, SchemaModel, SCHEMATRON_1_5_NAMESPACE,
    SCHEMATRON_NAMESPACE,
};
pub use resolver::{FileResolver, MemoryResolver, Resolver, SharedResolver};
