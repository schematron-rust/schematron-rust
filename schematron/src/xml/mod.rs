//! A pure Rust XML parser and XPath 1.0 data model.
//!
//! Schematron is defined over the XPath data model, so matching a rule
//! context or evaluating a test always needs a real tree with all seven
//! XPath node kinds, correct namespace scoping, and a stable document
//! order — never a bare stream of events with no tree behind it. This
//! module provides exactly that and nothing more; it is not a
//! general-purpose XML toolkit.
//!
//! That tree need not span the *whole* document at once, though: streaming
//! validation (`spec/streaming/`, `StreamingReader`) builds the same tree
//! one record at a time, in bounded memory, for schemas whose rules never
//! need more than a record's own subtree — never an event stream standing
//! in for the data model itself.
//!
//! See `spec/xml/` for the design and its deliberate limits.
//!
//! # Examples
//!
//! ```
//! use schematron::xml::Document;
//!
//! let doc = Document::from_str("<invoice><total>10.00</total></invoice>")?;
//! let invoice = doc.document_element().unwrap();
//! let total = doc.children(invoice)[0];
//! assert_eq!(doc.string_value(total), "10.00");
//! # Ok::<(), schematron::Error>(())
//! ```

mod document;
mod node;
mod parser;
mod streaming;
mod writer;

pub use document::Document;
pub use node::{NodeId, NodeKind, QName, XMLNS_NAMESPACE, XML_NAMESPACE};
pub use parser::MAX_DEPTH;
pub use writer::{escape_attribute, escape_text};

pub(crate) use node::NodeData;
pub(crate) use streaming::StreamingReader;
