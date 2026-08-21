//! A pure Rust XML parser and XPath 1.0 data model.
//!
//! Schematron is defined over the XPath data model, so validating a document
//! needs a tree with all seven XPath node kinds, correct namespace scoping,
//! and a stable document order — not a stream of events. This module provides
//! exactly that and nothing more; it is not a general-purpose XML toolkit.
//!
//! See `spec/xml.md` for the design and its deliberate limits.
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
mod writer;

pub use document::Document;
pub use node::{NodeId, NodeKind, QName, XMLNS_NAMESPACE, XML_NAMESPACE};
pub use parser::MAX_DEPTH;
pub use writer::{escape_attribute, escape_text};

pub(crate) use node::NodeData;
