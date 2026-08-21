//! Pass 5 of schema compilation, and the public [`Schema`] type.
//!
//! Every XPath expression in the schema is parsed **once**, here, and cached.
//! Two things follow. A syntax error, an unknown function, or an undeclared
//! namespace prefix is reported when the schema loads, naming the element it
//! is in — not silently at validation time on somebody else's document. And
//! validating a thousand documents parses each expression once, not a
//! thousand times.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use super::expand::expand;
use super::include::resolve_includes;
use super::model::{
    Assertion, Content, Let, LetValue, Pattern, QueryBinding, Rule, SchemaModel,
};
use super::parse::parse_schema;
use super::resolver::{FileResolver, SharedResolver};
use crate::error::{Error, Result};
use crate::xml::Document;
use crate::xpath::{
    check_function, Axis, Expr, NameTest, Namespaces, NodeTest, PathStart, XPathVersion,
};

/// How to load a schema.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use schematron::schema::{MemoryResolver, SchemaOptions};
///
/// let options = SchemaOptions::new()
///     .with_resolver(Arc::new(MemoryResolver::new().with("common.sch", "<p/>")))
///     .with_max_include_depth(8);
/// assert_eq!(options.max_include_depth, 8);
/// ```
#[derive(Clone)]
pub struct SchemaOptions {
    /// The base URI that relative `include` hrefs resolve against.
    pub base_uri: Option<String>,
    /// How `include` and `extends href` fetch their targets.
    pub resolver: SharedResolver,
    /// How deeply includes may nest before it is treated as a runaway.
    pub max_include_depth: usize,
    /// Compile a schema whose `queryBinding` this crate does not implement.
    ///
    /// Off by default: running an `xslt2` schema through an XPath 1.0 engine
    /// gives wrong answers quietly. Turn it on for the common case of a
    /// schema that declares `xslt2` but only uses XPath 1.0 constructs — any
    /// construct that really is XPath 2.0 will still fail to compile, with a
    /// message naming it.
    pub allow_unknown_query_binding: bool,
}

impl std::fmt::Debug for SchemaOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SchemaOptions")
            .field("base_uri", &self.base_uri)
            .field("max_include_depth", &self.max_include_depth)
            .field(
                "allow_unknown_query_binding",
                &self.allow_unknown_query_binding,
            )
            .finish_non_exhaustive()
    }
}

impl Default for SchemaOptions {
    fn default() -> Self {
        Self {
            base_uri: None,
            resolver: Arc::new(FileResolver::new()),
            max_include_depth: 64,
            allow_unknown_query_binding: false,
        }
    }
}

impl SchemaOptions {
    /// Default options: filesystem resolution, strict query binding.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the base URI for relative includes.
    #[must_use]
    pub fn with_base_uri(mut self, uri: impl Into<String>) -> Self {
        self.base_uri = Some(uri.into());
        self
    }

    /// Sets the resolver.
    #[must_use]
    pub fn with_resolver(mut self, resolver: SharedResolver) -> Self {
        self.resolver = resolver;
        self
    }

    /// Sets the include depth limit.
    #[must_use]
    pub fn with_max_include_depth(mut self, depth: usize) -> Self {
        self.max_include_depth = depth;
        self
    }

    /// Allows an unsupported `queryBinding` to compile anyway.
    #[must_use]
    pub fn with_allow_unknown_query_binding(mut self, allow: bool) -> Self {
        self.allow_unknown_query_binding = allow;
        self
    }
}

/// A compiled Schematron schema, ready to validate documents.
///
/// Compiling is the expensive part; validating is not. Compile once and reuse.
/// `Schema` is immutable and `Send + Sync`, so one schema can validate
/// documents on many threads at the same time.
///
/// # Examples
///
/// ```
/// use schematron::{Document, Schema};
///
/// let schema = Schema::from_str(r#"
///     <schema xmlns="http://purl.oclc.org/dsdl/schematron">
///       <pattern>
///         <rule context="line">
///           <assert test="number(@qty) &gt; 0">Quantity must be positive.</assert>
///         </rule>
///       </pattern>
///     </schema>
/// "#)?;
///
/// let document = Document::from_str("<order><line qty='-1'/></order>")?;
/// let report = schema.validate(&document)?;
/// assert_eq!(report.count_failures(), 1);
/// # Ok::<(), schematron::Error>(())
/// ```
#[derive(Debug, Clone)]
pub struct Schema {
    pub(crate) model: SchemaModel,
    pub(crate) expressions: HashMap<String, Expr>,
    pub(crate) namespaces: Namespaces,
    /// Kept for `pattern/@documents` and XPath `document()`, both of which
    /// fetch documents at validation time rather than at compile time.
    pub(crate) resolver: SharedResolver,
    /// The XPath version every expression in this schema is compiled and
    /// evaluated as, taken from the query binding.
    pub(crate) version: XPathVersion,
    /// Whether any expression calls XPath `document()`.
    ///
    /// Recorded at compile time so that the overwhelmingly common case — a
    /// schema that does not — pays nothing for the machinery that supports
    /// it. See `spec/xpath.md`.
    pub(crate) uses_document_function: bool,
}

impl Schema {
    /// Compiles a schema from source text.
    ///
    /// Named `from_str` rather than implementing [`std::str::FromStr`]
    /// deliberately: the trait's signature has no room for
    /// [`SchemaOptions`], and a schema that cannot resolve its includes is
    /// not much use.
    ///
    /// # Errors
    ///
    /// Returns [`Error::XmlParse`] if the schema is not well-formed XML,
    /// [`Error::Schema`] if it is not a valid Schematron schema, and
    /// [`Error::XPathSyntax`] if any expression in it does not compile.
    #[allow(clippy::should_implement_trait)] // See the note above.
    pub fn from_str(source: &str) -> Result<Schema> {
        Schema::from_str_with(source, &SchemaOptions::default())
    }

    /// Compiles a schema from source text, with options.
    ///
    /// # Errors
    ///
    /// As [`Schema::from_str`], plus [`Error::Resolve`],
    /// [`Error::IncludeCycle`], and [`Error::IncludeDepth`] from include
    /// resolution.
    pub fn from_str_with(source: &str, options: &SchemaOptions) -> Result<Schema> {
        let mut document = Document::from_str(source)?;
        if let Some(base) = &options.base_uri {
            document.set_base_uri(base.clone());
        }
        Schema::from_document(&document, options)
    }

    /// Reads and compiles a schema from a file.
    ///
    /// The file's path becomes the base URI, so relative includes resolve
    /// against the directory the schema lives in.
    ///
    /// # Errors
    ///
    /// As [`Schema::from_str_with`], plus [`Error::Io`] if the file cannot be
    /// read.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Schema> {
        let path = path.as_ref();
        let source = std::fs::read_to_string(path).map_err(|source| Error::Io {
            path: path.display().to_string(),
            source,
        })?;
        let options = SchemaOptions::new().with_base_uri(path.display().to_string());
        Schema::from_str_with(&source, &options)
    }

    /// Compiles a schema from an already-parsed XML document.
    ///
    /// # Errors
    ///
    /// As [`Schema::from_str_with`].
    pub fn from_document(document: &Document, options: &SchemaOptions) -> Result<Schema> {
        let resolved = resolve_includes(
            document,
            options.resolver.as_ref(),
            options.max_include_depth,
        )?;
        let mut model = parse_schema(&resolved)?;

        if !model.query_binding.is_supported() && !options.allow_unknown_query_binding {
            return Err(Error::UnsupportedQueryBinding {
                binding: model
                    .query_binding
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            });
        }

        let version = model.query_binding.version();
        expand(&mut model)?;

        let mut namespaces = Namespaces::new();
        for ns in &model.namespaces {
            namespaces.insert(ns.prefix.clone(), ns.uri.clone());
        }

        let mut schema = Schema {
            model,
            expressions: HashMap::new(),
            namespaces,
            resolver: Arc::clone(&options.resolver),
            version,
            uses_document_function: false,
        };
        schema.compile_expressions()?;
        schema.check_references()?;
        schema.uses_document_function = schema
            .expressions
            .values()
            .any(calls_document_function);
        Ok(schema)
    }

    /// Parses every expression in the schema, once.
    fn compile_expressions(&mut self) -> Result<()> {
        let mut work: Vec<(String, String, bool)> = Vec::new();

        for binding in &self.model.lets {
            collect_let(binding, "schema", &mut work);
        }
        for phase in &self.model.phases {
            let location = format!("phase[@id='{}']", phase.id);
            for binding in &phase.lets {
                collect_let(binding, &location, &mut work);
            }
        }
        for (index, pattern) in self.model.patterns.iter().enumerate() {
            collect_pattern(pattern, index, &mut work);
        }
        for diagnostic in &self.model.diagnostics {
            let location = format!("diagnostic[@id='{}']", diagnostic.id);
            collect_content(&diagnostic.content, &location, &mut work);
        }
        for property in &self.model.properties {
            let location = format!("property[@id='{}']", property.id);
            collect_content(&property.content, &location, &mut work);
        }

        for (source, location, is_pattern) in work {
            let expr = self.compile_one(&source, &location)?;
            if is_pattern {
                check_match_pattern(&expr, &source, &location)?;
            }
            self.expressions.insert(source, expr);
        }
        Ok(())
    }

    fn compile_one(&self, source: &str, location: &str) -> Result<Expr> {
        let expr = crate::xpath::parse(source).map_err(|error| {
            Error::xpath_syntax(location, source, error.position, error.message)
        })?;
        self.check_expression(&expr, source, location)?;
        Ok(expr)
    }

    /// Walks an expression, checking what can be checked without a document:
    /// function names and arities, and namespace prefixes.
    fn check_expression(&self, expr: &Expr, source: &str, location: &str) -> Result<()> {
        match expr {
            // Literals need no checking. Nor does a variable reference:
            // variable scope is dynamic — a `let` in an enclosing scope may
            // bind it — so an unbound reference is caught at evaluation time,
            // where the scope is actually known.
            Expr::Literal(_) | Expr::Number(_) | Expr::Variable(_) => {}
            Expr::Negate(inner) => self.check_expression(inner, source, location)?,
            Expr::Binary(_, left, right) => {
                self.check_expression(left, source, location)?;
                self.check_expression(right, source, location)?;
            }
            Expr::Function { name, args } => {
                check_function(name, args.len(), self.version).map_err(|message| {
                    Error::xpath_syntax(location, source, 0, message)
                })?;
                check_literal_regex(name, args, source, location)?;
                for arg in args {
                    self.check_expression(arg, source, location)?;
                }
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.require_v2("`if (…) then … else …`", source, location)?;
                self.check_expression(condition, source, location)?;
                self.check_expression(then_branch, source, location)?;
                self.check_expression(else_branch, source, location)?;
            }
            Expr::Sequence(members) => {
                self.require_v2("a sequence written with `,`", source, location)?;
                for member in members {
                    self.check_expression(member, source, location)?;
                }
            }
            Expr::Range(from, to) => {
                self.require_v2("the `to` range operator", source, location)?;
                self.check_expression(from, source, location)?;
                self.check_expression(to, source, location)?;
            }
            Expr::For { input, body, .. } => {
                self.require_v2("`for … in … return …`", source, location)?;
                self.check_expression(input, source, location)?;
                self.check_expression(body, source, location)?;
            }
            Expr::Quantified {
                quantifier,
                input,
                test,
                ..
            } => {
                self.require_v2(
                    &format!("`{} … in … satisfies …`", quantifier.as_str()),
                    source,
                    location,
                )?;
                self.check_expression(input, source, location)?;
                self.check_expression(test, source, location)?;
            }
            Expr::Path(path) => {
                if let PathStart::Expr(start, predicates) = &path.start {
                    self.check_expression(start, source, location)?;
                    for predicate in predicates {
                        self.check_expression(predicate, source, location)?;
                    }
                }
                for step in &path.steps {
                    match &step.node_test {
                        NodeTest::Name(NameTest {
                            prefix: Some(prefix),
                            ..
                        })
                        | NodeTest::NamespaceWildcard(prefix) => {
                            self.check_prefix(prefix, source, location)?;
                        }
                        _ => {}
                    }
                    for predicate in &step.predicates {
                        self.check_expression(predicate, source, location)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Rejects an XPath 2.0 construct under an XPath 1.0 query binding.
    ///
    /// Keeping this in one place means every 2.0 construct refuses the same
    /// way and says the same thing about how to enable it.
    fn require_v2(&self, construct: &str, source: &str, location: &str) -> Result<()> {
        if self.version.is_v2() {
            return Ok(());
        }
        Err(Error::xpath_syntax(
            location,
            source,
            0,
            format!(
                "{construct} is XPath 2.0 syntax; this schema's query binding is \
                 XPath 1.0. Set queryBinding=\"xslt2\" to use it, and see \
                 spec/xpath2.md for what that enables."
            ),
        ))
    }

    fn check_prefix(&self, prefix: &str, source: &str, location: &str) -> Result<()> {
        if self.namespaces.resolve(prefix).is_some() {
            return Ok(());
        }
        let declared: Vec<&str> = self.model.namespaces.iter().map(|n| n.prefix.as_str()).collect();
        Err(Error::xpath_syntax(
            location,
            source,
            0,
            format!(
                "namespace prefix {prefix:?} is not declared; add <ns prefix=\"{prefix}\" uri=\"…\"/> \
                 to the schema{}",
                if declared.is_empty() {
                    ", which currently declares none".to_string()
                } else {
                    format!(", which declares: {}", declared.join(", "))
                }
            ),
        ))
    }

    /// Checks that every id an assertion references actually exists.
    fn check_references(&self) -> Result<()> {
        for pattern in &self.model.patterns {
            for rule in &pattern.rules {
                for assertion in rule.assertions() {
                    for id in &assertion.diagnostics {
                        if self.model.diagnostic(id).is_none() {
                            return Err(Error::schema(
                                assertion.kind.as_str(),
                                pattern.id.clone(),
                                format!("no <diagnostic> with @id={id:?}"),
                            ));
                        }
                    }
                    for id in &assertion.properties {
                        if self.model.property(id).is_none() {
                            return Err(Error::schema(
                                assertion.kind.as_str(),
                                pattern.id.clone(),
                                format!("no <property> with @id={id:?}"),
                            ));
                        }
                    }
                }
            }
        }
        for phase in &self.model.phases {
            for active in &phase.actives {
                if self.model.pattern(active).is_none() {
                    return Err(Error::schema(
                        "active",
                        Some(format!("phase[@id='{}']", phase.id)),
                        format!(
                            "no <pattern> with @id={active:?}; an abstract pattern \
                             cannot be activated either, since it never runs"
                        ),
                    ));
                }
            }
        }
        if let Some(default) = &self.model.default_phase {
            if self.model.phase(default).is_none() {
                return Err(Error::schema(
                    "schema",
                    None,
                    format!("@defaultPhase names {default:?}, which is not a declared <phase>"),
                ));
            }
        }
        Ok(())
    }

    /// Validates a document against this schema, using default options.
    ///
    /// # Errors
    ///
    /// Returns [`Error::XPathEval`] if an expression fails at runtime, and
    /// [`Error::UnknownPhase`] if a phase was named that does not exist. A
    /// document that simply breaks the rules is *not* an error: that is a
    /// [`Report`](crate::Report) with failures in it.
    pub fn validate(&self, document: &Document) -> Result<crate::Report> {
        self.validate_with(document, &crate::ValidateOptions::new())
    }

    /// Validates a document against this schema, with options.
    ///
    /// # Errors
    ///
    /// As [`Schema::validate`].
    pub fn validate_with(
        &self,
        document: &Document,
        options: &crate::ValidateOptions,
    ) -> Result<crate::Report> {
        crate::validate::validate(self, document, options)
    }

    /// The resolver used for `pattern/@documents`.
    pub(crate) fn resolver(&self) -> &dyn super::resolver::Resolver {
        self.resolver.as_ref()
    }

    /// A compiled expression, by its source text.
    pub(crate) fn expression(&self, source: &str) -> Result<&Expr> {
        self.expressions.get(source).ok_or_else(|| {
            Error::xpath_eval(
                source,
                "this expression was not compiled; this is a bug in the crate",
            )
        })
    }

    /// Whether any expression in this schema calls XPath `document()`.
    ///
    /// A schema that does not is validated against the caller's document
    /// directly; one that does needs a working copy of the tree that loaded
    /// documents can be merged into.
    #[must_use]
    pub fn uses_document_function(&self) -> bool {
        self.uses_document_function
    }

    /// The parsed model, after includes and abstractions are expanded.
    #[must_use]
    pub fn model(&self) -> &SchemaModel {
        &self.model
    }

    /// The namespace bindings every expression is evaluated against.
    #[must_use]
    pub fn namespaces(&self) -> &Namespaces {
        &self.namespaces
    }

    /// The schema's `@id`.
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        self.model.id.as_deref()
    }

    /// The schema's title.
    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.model.title.as_deref()
    }

    /// The schema's `@schemaVersion`.
    #[must_use]
    pub fn schema_version(&self) -> Option<&str> {
        self.model.schema_version.as_deref()
    }

    /// The XPath version this schema's expressions use.
    #[must_use]
    pub fn version(&self) -> XPathVersion {
        self.version
    }

    /// The query language binding the schema declares.
    #[must_use]
    pub fn query_binding(&self) -> &QueryBinding {
        &self.model.query_binding
    }

    /// The identifiers of the schema's phases, in document order.
    pub fn phases(&self) -> impl Iterator<Item = &str> {
        self.model.phases.iter().map(|p| p.id.as_str())
    }

    /// The schema's `@defaultPhase`.
    #[must_use]
    pub fn default_phase(&self) -> Option<&str> {
        self.model.default_phase.as_deref()
    }

    /// The patterns that will run, in order.
    #[must_use]
    pub fn patterns(&self) -> &[Pattern] {
        &self.model.patterns
    }
}

fn collect_let(binding: &Let, location: &str, work: &mut Vec<(String, String, bool)>) {
    match &binding.value {
        LetValue::Expression(expression) => work.push((
            expression.clone(),
            format!("{location}/let[@name='{}']", binding.name),
            false,
        )),
        LetValue::Content(content) => collect_content(
            content,
            &format!("{location}/let[@name='{}']", binding.name),
            work,
        ),
    }
}

fn collect_pattern(pattern: &Pattern, index: usize, work: &mut Vec<(String, String, bool)>) {
    let location = pattern.id.as_ref().map_or_else(
        || format!("pattern[{}]", index + 1),
        |id| format!("pattern[@id='{id}']"),
    );
    if let Some(documents) = &pattern.documents {
        work.push((documents.clone(), format!("{location}/@documents"), false));
    }
    for binding in &pattern.lets {
        collect_let(binding, &location, work);
    }
    for rule in &pattern.rules {
        collect_rule(rule, &location, work);
    }
}

fn collect_rule(rule: &Rule, pattern_location: &str, work: &mut Vec<(String, String, bool)>) {
    let location = match &rule.context {
        Some(context) => format!("{pattern_location}/rule[@context='{context}']"),
        None => format!("{pattern_location}/rule"),
    };
    if let Some(context) = &rule.context {
        work.push((context.clone(), format!("{location}/@context"), true));
    }
    if let Some(subject) = &rule.subject {
        work.push((subject.clone(), format!("{location}/@subject"), false));
    }
    for binding in &rule.lets {
        collect_let(binding, &location, work);
    }
    for (index, assertion) in rule.assertions().enumerate() {
        collect_assertion(assertion, &location, index, work);
    }
}

fn collect_assertion(
    assertion: &Assertion,
    rule_location: &str,
    index: usize,
    work: &mut Vec<(String, String, bool)>,
) {
    let location = format!("{rule_location}/{}[{}]", assertion.kind.as_str(), index + 1);
    work.push((assertion.test.clone(), format!("{location}/@test"), false));
    if let Some(subject) = &assertion.subject {
        work.push((subject.clone(), format!("{location}/@subject"), false));
    }
    collect_content(&assertion.content, &location, work);
}

fn collect_content(content: &[Content], location: &str, work: &mut Vec<(String, String, bool)>) {
    for fragment in content {
        match fragment {
            // Literal text and a bare `<name/>` hold no expression to compile.
            Content::Text(_) | Content::Name { path: None } => {}
            Content::ValueOf { select } => {
                work.push((select.clone(), format!("{location}/value-of/@select"), false));
            }
            Content::Name { path: Some(path) } => {
                work.push((path.clone(), format!("{location}/name/@path"), false));
            }
            Content::Emph(inner)
            | Content::Span { content: inner, .. }
            | Content::Dir { content: inner, .. } => collect_content(inner, location, work),
        }
    }
}

/// Compiles a literal regular expression argument, so that a malformed
/// pattern written into the schema fails when the schema loads rather than
/// part-way through validating somebody's document.
///
/// A pattern computed at runtime cannot be checked here, and is validated
/// when it is evaluated.
fn check_literal_regex(
    name: &str,
    args: &[Expr],
    source: &str,
    location: &str,
) -> Result<()> {
    // `matches(input, pattern, flags?)` and `replace(input, pattern, replacement, flags?)`.
    let (pattern_index, flags_index) = match name {
        "matches" => (1, 2),
        "replace" => (1, 3),
        _ => return Ok(()),
    };

    let Some(Expr::Literal(pattern)) = args.get(pattern_index) else {
        return Ok(());
    };
    // Only check flags that are literal too; a computed flag string is left
    // to evaluation time.
    let flags = match args.get(flags_index) {
        Some(Expr::Literal(flags)) => Some(flags.as_str()),
        Some(_) => return Ok(()),
        None => None,
    };

    crate::xpath::check_regex(pattern, flags)
        .map_err(|message| Error::xpath_syntax(location, source, 0, message))
}

/// Whether an expression calls `document()` anywhere inside it.
fn calls_document_function(expr: &Expr) -> bool {
    match expr {
        Expr::Literal(_) | Expr::Number(_) | Expr::Variable(_) => false,
        Expr::Negate(inner) => calls_document_function(inner),
        Expr::Binary(_, left, right) => {
            calls_document_function(left) || calls_document_function(right)
        }
        Expr::Function { name, args } => {
            name == "document" || args.iter().any(calls_document_function)
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            calls_document_function(condition)
                || calls_document_function(then_branch)
                || calls_document_function(else_branch)
        }
        Expr::Sequence(members) => members.iter().any(calls_document_function),
        Expr::Range(from, to) => {
            calls_document_function(from) || calls_document_function(to)
        }
        Expr::For { input, body, .. } => {
            calls_document_function(input) || calls_document_function(body)
        }
        Expr::Quantified { input, test, .. } => {
            calls_document_function(input) || calls_document_function(test)
        }
        Expr::Path(path) => {
            let start = match &path.start {
                PathStart::Expr(expr, predicates) => {
                    calls_document_function(expr)
                        || predicates.iter().any(calls_document_function)
                }
                PathStart::Root | PathStart::Context => false,
            };
            start
                || path
                    .steps
                    .iter()
                    .any(|step| step.predicates.iter().any(calls_document_function))
        }
    }
}

/// Rejects a `rule/@context` that is not a legal XSLT match pattern.
///
/// A match pattern selects downwards from an ancestor; a leading reverse axis
/// has no meaning in that reduction. Rejecting it beats guessing, because a
/// guess would produce a rule that silently never fires.
fn check_match_pattern(expr: &Expr, source: &str, location: &str) -> Result<()> {
    match expr {
        Expr::Binary(crate::xpath::BinaryOp::Union, left, right) => {
            check_match_pattern(left, source, location)?;
            check_match_pattern(right, source, location)
        }
        Expr::Path(path) => {
            for step in &path.steps {
                let allowed = matches!(
                    step.axis,
                    Axis::Child | Axis::Attribute | Axis::DescendantOrSelf | Axis::SelfAxis
                );
                if !allowed {
                    return Err(Error::xpath_syntax(
                        location,
                        source,
                        0,
                        format!(
                            "the {}:: axis is not allowed in a rule context, which must be an \
                             XSLT match pattern; express the constraint as a predicate instead, \
                             such as `a[ancestor::b]`",
                            step.axis.as_str()
                        ),
                    ));
                }
            }
            Ok(())
        }
        _ => Err(Error::xpath_syntax(
            location,
            source,
            0,
            "a rule context must be a location path or a union of location paths",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
        <schema xmlns="http://purl.oclc.org/dsdl/schematron">
          <pattern><rule context="a"><assert test="b">m</assert></rule></pattern>
        </schema>
    "#;

    fn error(source: &str) -> String {
        Schema::from_str(source).unwrap_err().to_string()
    }

    #[test]
    fn compiles_a_minimal_schema() {
        let schema = Schema::from_str(MINIMAL).unwrap();
        assert_eq!(schema.patterns().len(), 1);
        assert!(schema.expression("a").is_ok());
        assert!(schema.expression("b").is_ok());
    }

    #[test]
    fn reports_syntax_errors_at_compile_time_with_a_location() {
        let message = error(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern><rule context="a"><assert test="count(b">m</assert></rule></pattern>
               </schema>"#,
        );
        assert!(message.contains("XPath syntax error"), "{message}");
        assert!(message.contains("@test"), "{message}");
    }

    #[test]
    fn reports_unknown_functions_at_compile_time() {
        let message = error(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern><rule context="a"><assert test="nonsense()">m</assert></rule></pattern>
               </schema>"#,
        );
        assert!(message.contains("unknown function"), "{message}");
    }

    #[test]
    fn names_xpath_two_functions_as_such() {
        let message = error(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern><rule context="a">
                   <assert test="matches(., 'x')">m</assert>
                 </rule></pattern>
               </schema>"#,
        );
        assert!(message.contains("XPath 2.0"), "{message}");
    }

    #[test]
    fn reports_undeclared_prefixes_at_compile_time() {
        let message = error(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern><rule context="p:a"><assert test="b">m</assert></rule></pattern>
               </schema>"#,
        );
        assert!(message.contains("is not declared"), "{message}");
    }

    #[test]
    fn accepts_declared_prefixes() {
        let schema = Schema::from_str(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <ns prefix="p" uri="urn:p"/>
                 <pattern><rule context="p:a"><assert test="p:b">m</assert></rule></pattern>
               </schema>"#,
        )
        .unwrap();
        assert_eq!(schema.namespaces().resolve("p"), Some("urn:p"));
    }

    #[test]
    fn rejects_query_bindings_above_xpath_two() {
        let source = r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron" queryBinding="xslt3">
                          <pattern><rule context="a"><assert test="b">m</assert></rule></pattern>
                        </schema>"#;
        assert!(matches!(
            Schema::from_str(source).unwrap_err(),
            Error::UnsupportedQueryBinding { .. }
        ));

        let options = SchemaOptions::new().with_allow_unknown_query_binding(true);
        assert!(Schema::from_str_with(source, &options).is_ok());
    }

    #[test]
    fn accepts_the_xpath_two_bindings() {
        for binding in ["xslt2", "xpath2"] {
            let source = format!(
                r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron" queryBinding="{binding}">
                     <pattern><rule context="a"><assert test="b">m</assert></rule></pattern>
                   </schema>"#
            );
            let schema = Schema::from_str(&source)
                .unwrap_or_else(|e| panic!("{binding} should compile: {e}"));
            assert_eq!(schema.version(), crate::xpath::XPathVersion::V2);
        }
    }

    #[test]
    fn a_forced_unknown_binding_is_treated_as_xpath_one() {
        // `allow_unknown_query_binding` must not quietly grant 2.0 features.
        let source = r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron" queryBinding="xslt3">
                          <pattern><rule context="a"><assert test="b">m</assert></rule></pattern>
                        </schema>"#;
        let options = SchemaOptions::new().with_allow_unknown_query_binding(true);
        let schema = Schema::from_str_with(source, &options).unwrap();
        assert_eq!(schema.version(), crate::xpath::XPathVersion::V1);
    }

    #[test]
    fn rejects_reverse_axes_in_a_rule_context() {
        let message = error(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern><rule context="ancestor::a"><assert test="b">m</assert></rule></pattern>
               </schema>"#,
        );
        assert!(message.contains("not allowed in a rule context"), "{message}");
    }

    #[test]
    fn accepts_the_usual_match_pattern_shapes() {
        for context in ["a", "a/b", "//a", "/a", "@x", "a[@x]", "a | b", "*", "text()"] {
            let source = format!(
                r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                     <pattern><rule context="{context}"><assert test="true()">m</assert></rule></pattern>
                   </schema>"#
            );
            assert!(Schema::from_str(&source).is_ok(), "rejected context {context:?}");
        }
    }

    #[test]
    fn rejects_references_to_missing_diagnostics() {
        let message = error(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern><rule context="a">
                   <assert test="b" diagnostics="nope">m</assert>
                 </rule></pattern>
               </schema>"#,
        );
        assert!(message.contains("no <diagnostic>"), "{message}");
    }

    #[test]
    fn rejects_phases_that_activate_missing_patterns() {
        let message = error(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <phase id="p"><active pattern="nope"/></phase>
                 <pattern id="q"><rule context="a"><assert test="b">m</assert></rule></pattern>
               </schema>"#,
        );
        assert!(message.contains("no <pattern>"), "{message}");
    }

    #[test]
    fn rejects_a_default_phase_that_does_not_exist() {
        let message = error(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron" defaultPhase="nope">
                 <pattern><rule context="a"><assert test="b">m</assert></rule></pattern>
               </schema>"#,
        );
        assert!(message.contains("defaultPhase"), "{message}");
    }

    #[test]
    fn identical_expressions_compile_once() {
        let schema = Schema::from_str(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern>
                   <rule context="a"><assert test="b">one</assert></rule>
                   <rule context="c"><assert test="b">two</assert></rule>
                 </pattern>
               </schema>"#,
        )
        .unwrap();
        // "a", "b", "c" — three distinct sources, not four expressions.
        assert_eq!(schema.expressions.len(), 3);
    }

    #[test]
    fn schema_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Schema>();
    }
}
