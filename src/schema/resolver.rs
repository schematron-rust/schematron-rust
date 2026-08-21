//! Fetching the documents that `include` and `extends href` point at.
//!
//! Resolution is behind a trait so that a caller can supply schemas from a
//! bundle, a database, or a test fixture rather than the filesystem. The
//! default reads local files and refuses network URIs: fetching over the
//! network is a decision an application makes, not something a validation
//! library should do behind the caller's back.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::error::{Error, Result};

/// Turns an `href` into document source text.
pub trait Resolver: fmt::Debug + Send + Sync {
    /// Fetches the document at `href`, resolved relative to `base`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Resolve`] when the target cannot be fetched, or when
    /// the resolver declines to fetch it.
    fn resolve(&self, href: &str, base: Option<&str>) -> Result<String>;

    /// The base URI to use for references *inside* the fetched document.
    ///
    /// The default joins `href` onto `base` the way a relative path joins a
    /// directory, so that nested includes resolve relative to the file that
    /// contains them rather than to the top-level schema.
    fn rebase(&self, href: &str, base: Option<&str>) -> Option<String> {
        Some(join(href, base).to_string_lossy().into_owned())
    }
}

/// Joins a relative `href` onto the directory containing `base`.
fn join(href: &str, base: Option<&str>) -> PathBuf {
    let path = Path::new(href);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    match base.map(Path::new).and_then(Path::parent) {
        Some(directory) => directory.join(path),
        None => path.to_path_buf(),
    }
}

/// Reads included documents from the local filesystem.
///
/// Relative hrefs resolve against the directory of the including document.
/// `http:` and `https:` URIs are refused with a message that says why, rather
/// than being silently skipped or silently fetched.
///
/// # Examples
///
/// ```
/// use schematron::schema::{FileResolver, Resolver};
///
/// let resolver = FileResolver::new();
/// let error = resolver.resolve("https://example.com/x.sch", None).unwrap_err();
/// assert!(error.to_string().contains("network"));
/// ```
#[derive(Debug, Clone, Default)]
pub struct FileResolver;

impl FileResolver {
    /// A resolver that reads the local filesystem.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Resolver for FileResolver {
    fn resolve(&self, href: &str, base: Option<&str>) -> Result<String> {
        if href.starts_with("http://") || href.starts_with("https://") {
            return Err(Error::Resolve {
                href: href.to_string(),
                message: "this resolver does not perform network access; \
                          fetch the document yourself and supply it through a \
                          custom Resolver, or vendor it next to the schema"
                    .to_string(),
            });
        }
        let href = href.strip_prefix("file://").unwrap_or(href);
        let path = join(href, base);
        std::fs::read_to_string(&path).map_err(|source| Error::Resolve {
            href: path.display().to_string(),
            message: source.to_string(),
        })
    }
}

/// Serves documents from an in-memory map, keyed by href.
///
/// Intended for tests and for embedding a schema and its includes in a
/// binary, where reaching the filesystem is neither possible nor wanted.
///
/// # Examples
///
/// ```
/// use schematron::schema::{MemoryResolver, Resolver};
///
/// let resolver = MemoryResolver::new().with("common.sch", "<p/>");
/// assert_eq!(resolver.resolve("common.sch", None).unwrap(), "<p/>");
/// assert!(resolver.resolve("missing.sch", None).is_err());
/// ```
#[derive(Debug, Clone, Default)]
pub struct MemoryResolver {
    documents: std::collections::HashMap<String, String>,
}

impl MemoryResolver {
    /// An empty resolver.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a document, returning the resolver so calls can be chained.
    #[must_use]
    pub fn with(mut self, href: impl Into<String>, source: impl Into<String>) -> Self {
        self.documents.insert(href.into(), source.into());
        self
    }
}

impl Resolver for MemoryResolver {
    fn resolve(&self, href: &str, _base: Option<&str>) -> Result<String> {
        self.documents
            .get(href)
            .cloned()
            .ok_or_else(|| Error::Resolve {
                href: href.to_string(),
                message: "not present in this MemoryResolver".to_string(),
            })
    }

    fn rebase(&self, href: &str, _base: Option<&str>) -> Option<String> {
        // Keys are opaque, so an included document's own hrefs are looked up
        // by their literal keys too.
        Some(href.to_string())
    }
}

/// A shared resolver handle.
pub type SharedResolver = Arc<dyn Resolver>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_resolver_refuses_network_uris() {
        let error = FileResolver::new()
            .resolve("https://example.com/a.sch", None)
            .unwrap_err();
        assert!(error.to_string().contains("network"), "{error}");
    }

    #[test]
    fn relative_hrefs_join_the_base_directory() {
        assert_eq!(
            join("b.sch", Some("/schemas/a.sch")),
            PathBuf::from("/schemas/b.sch")
        );
        assert_eq!(
            join("sub/b.sch", Some("/schemas/a.sch")),
            PathBuf::from("/schemas/sub/b.sch")
        );
    }

    #[test]
    fn absolute_hrefs_ignore_the_base() {
        assert_eq!(
            join("/other/b.sch", Some("/schemas/a.sch")),
            PathBuf::from("/other/b.sch")
        );
    }

    #[test]
    fn memory_resolver_serves_and_reports_misses() {
        let resolver = MemoryResolver::new().with("a", "<x/>");
        assert_eq!(resolver.resolve("a", None).unwrap(), "<x/>");
        assert!(resolver.resolve("b", None).is_err());
    }
}
