//! The `schematron` command line tool.
//!
//! A thin shell over the library: parse arguments, compile the schema once,
//! validate each document, render the report, and choose an exit code. See
//! `spec/cli.md`.

#![forbid(unsafe_code)]

use std::io::Read;
use std::io::Write;
use std::process::ExitCode;

use clap::{Parser, ValueEnum};

use schematron::schema::QueryBinding;
use schematron::svrl::SvrlOptions;
use schematron::text::TextOptions;
use schematron::validate::{PhaseSelection, Report, ValidateOptions};
use schematron::{Document, Lint, Schema, SchemaOptions};

/// Exit code: every document was valid.
const EXIT_VALID: u8 = 0;
/// Exit code: at least one assertion failed.
const EXIT_INVALID: u8 = 1;
/// Exit code: the arguments did not make sense.
const EXIT_USAGE: u8 = 2;
/// Exit code: the schema could not be compiled.
const EXIT_SCHEMA: u8 = 3;
/// Exit code: an input document could not be read or parsed.
const EXIT_DOCUMENT: u8 = 4;

/// Validate XML documents against a Schematron schema.
///
/// Schematron is a rule-based schema language: instead of describing the
/// shape a document may take, it asserts conditions the document must satisfy,
/// written as `XPath` expressions. This tool implements ISO/IEC 19757-3 in pure
/// Rust, with no XSLT processor and no C library.
// Command line flags are independent switches by nature.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Parser)]
#[command(
    name = "schematron",
    version,
    about = "Validate XML documents against a Schematron schema",
    long_about = None,
    after_help = "\
EXIT CODES:
  0  every document valid
  1  at least one failed assertion
  2  usage error
  3  schema error
  4  document error

EXAMPLES:
  schematron --schema rules.sch data.xml
  schematron -s rules.sch -p strict -f svrl -o report.svrl data.xml
  schematron -s rules.sch --flag error docs/*.xml
  cat data.xml | schematron -s rules.sch -
  schematron -s rules.sch --explain
"
)]
struct Cli {
    /// Schematron schema file, usually with a .sch extension.
    #[arg(short, long, value_name = "PATH")]
    schema: String,

    /// XML documents to validate; `-` reads standard input.
    #[arg(value_name = "DOCUMENT")]
    documents: Vec<String>,

    /// Phase to run: a phase id, or `#ALL`, or `#DEFAULT`.
    #[arg(short, long, value_name = "NAME")]
    phase: Option<String>,

    /// Output format.
    #[arg(short, long, value_enum, default_value_t = Format::Text)]
    format: Format,

    /// Write the report here instead of to standard output.
    #[arg(short, long, value_name = "PATH")]
    output: Option<String>,

    /// Report only findings carrying this flag; repeatable.
    #[arg(long, value_name = "FLAG")]
    flag: Vec<String>,

    /// Stop after this many failed assertions.
    #[arg(long, value_name = "N")]
    max_failures: Option<usize>,

    /// Evaluate the schema's patterns on separate threads.
    ///
    /// The report is unchanged; only the wall-clock time differs. Worth it
    /// for a schema with many patterns over a large document, and no help at
    /// all for a schema with one. Ignored when --max-failures is given.
    #[arg(long)]
    parallel: bool,

    /// Omit `fired-rule` events from SVRL output.
    #[arg(long)]
    svrl_findings_only: bool,

    /// Compile a schema whose queryBinding this crate does not implement.
    ///
    /// `XPath` 2.0 and later are a different language, so this is best effort:
    /// any construct that really is `XPath` 2.0 will still fail to compile.
    #[arg(long)]
    allow_unknown_query_binding: bool,

    /// Print the schema's phases and exit.
    #[arg(long)]
    list_phases: bool,

    /// Check the schema for constructs that are legal but probably wrong,
    /// then exit. No document is needed.
    #[arg(long)]
    lint: bool,

    /// Print the compiled schema — patterns, rules, contexts, tests — and exit.
    #[arg(long)]
    explain: bool,

    /// Suppress the report; use the exit code only.
    #[arg(short, long)]
    quiet: bool,

    /// Include successful reports, fired rules, and tests in text output.
    #[arg(short, long)]
    verbose: bool,
}

/// The report formats the tool can write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    /// Human-readable text.
    Text,
    /// Schematron Validation Report Language, the standard XML report format.
    Svrl,
    /// JSON, keeping the report's tree structure.
    Json,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(code) => ExitCode::from(code),
        Err(exit) => {
            eprintln!("schematron: {}", exit.message);
            ExitCode::from(exit.code)
        }
    }
}

/// A fatal error, paired with the exit code it should produce.
struct Exit {
    code: u8,
    message: String,
}

impl Exit {
    fn new(code: u8, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

fn run(cli: &Cli) -> Result<u8, Exit> {
    let options = SchemaOptions::new()
        .with_allow_unknown_query_binding(cli.allow_unknown_query_binding);
    let source = std::fs::read_to_string(&cli.schema)
        .map_err(|e| Exit::new(EXIT_SCHEMA, format!("cannot read schema {}: {e}", cli.schema)))?;
    let options = options.with_base_uri(cli.schema.clone());
    let schema = Schema::from_str_with(&source, &options)
        .map_err(|e| Exit::new(EXIT_SCHEMA, e.to_string()))?;

    if cli.list_phases {
        print_phases(&schema);
        return Ok(EXIT_VALID);
    }
    if cli.explain {
        print!("{}", explain(&schema));
        return Ok(EXIT_VALID);
    }
    if cli.lint {
        let lints = schema.lint();
        if !cli.quiet {
            write_output(cli.output.as_deref(), &render_lints(&lints))?;
        }
        // Exit 1 when anything was reported, so a build can gate on it.
        return Ok(if lints.is_empty() {
            EXIT_VALID
        } else {
            EXIT_INVALID
        });
    }
    if cli.documents.is_empty() {
        return Err(Exit::new(
            EXIT_USAGE,
            "no documents given; pass one or more file paths, or `-` for standard input",
        ));
    }

    let mut validate_options = ValidateOptions::new()
        .with_phase(
            cli.phase
                .as_deref()
                .map_or(PhaseSelection::Default, PhaseSelection::from),
        )
        // SVRL carries `fired-rule` events, so they are recorded for it even
        // when the run is not verbose.
        .with_record_fired_rules(cli.verbose || cli.format == Format::Svrl)
        .with_parallel_patterns(cli.parallel);
    if let Some(limit) = cli.max_failures {
        validate_options = validate_options.with_max_failures(limit);
    }

    let mut rendered = String::new();
    let mut failures = 0;

    for path in &cli.documents {
        let document = read_document(path)?;
        let mut report = schema
            .validate_with(&document, &validate_options)
            .map_err(|e| Exit::new(EXIT_SCHEMA, e.to_string()))?;

        if !cli.flag.is_empty() {
            retain_flags(&mut report, &cli.flag);
        }
        failures += report.count_failures();

        if !cli.quiet {
            rendered.push_str(&render(&report, cli, path));
        }
    }

    if !cli.quiet {
        write_output(cli.output.as_deref(), &rendered)?;
    }

    Ok(if failures == 0 {
        EXIT_VALID
    } else {
        EXIT_INVALID
    })
}

fn read_document(path: &str) -> Result<Document, Exit> {
    if path == "-" {
        let mut bytes = Vec::new();
        std::io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|e| Exit::new(EXIT_DOCUMENT, format!("cannot read standard input: {e}")))?;
        return Document::from_bytes(&bytes)
            .map_err(|e| Exit::new(EXIT_DOCUMENT, format!("standard input: {e}")));
    }
    Document::from_path(path).map_err(|e| Exit::new(EXIT_DOCUMENT, e.to_string()))
}

/// Keeps only the findings whose flag is in `flags`.
///
/// Filtering happens on the report, so it changes both what is printed and
/// what the exit code counts — which is what makes `--flag error` mean "fail
/// the build on errors, and leave warnings to a different run".
fn retain_flags(report: &mut Report, flags: &[String]) {
    for pattern in &mut report.patterns {
        for rule in &mut pattern.rules {
            rule.assertions
                .retain(|a| a.flag.as_ref().is_some_and(|f| flags.contains(f)));
        }
    }
}

fn render(report: &Report, cli: &Cli, path: &str) -> String {
    match cli.format {
        Format::Text => {
            let mut options = if cli.verbose {
                TextOptions::verbose()
            } else {
                TextOptions::new()
            };
            options.label = Some(path.to_string());
            report.to_text_with(&options)
        }
        Format::Svrl => {
            let options = if cli.svrl_findings_only {
                SvrlOptions::findings_only()
            } else {
                SvrlOptions::new()
            };
            report.to_svrl_with(&options)
        }
        Format::Json => report
            .to_json()
            .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
    }
}

fn write_output(path: Option<&str>, text: &str) -> Result<(), Exit> {
    match path {
        None => {
            let mut stdout = std::io::stdout().lock();
            stdout
                .write_all(text.as_bytes())
                .map_err(|e| Exit::new(EXIT_USAGE, format!("cannot write output: {e}")))
        }
        Some(path) => std::fs::write(path, text)
            .map_err(|e| Exit::new(EXIT_USAGE, format!("cannot write {path}: {e}"))),
    }
}

/// Renders lints for a person, one block each.
fn render_lints(lints: &[Lint]) -> String {
    if lints.is_empty() {
        return "no lints\n".to_string();
    }

    let mut out = String::new();
    for lint in lints {
        out.push_str(&format!("{:<24} {}\n", lint.kind.as_str(), lint.location));
        out.push_str(&format!("    {}\n", lint.message));
        if let Some(help) = &lint.help {
            // Wrap the help text, labelling the first line only so that the
            // continuation reads as one paragraph rather than a list.
            for (index, line) in wrap(help, 68).into_iter().enumerate() {
                let label = if index == 0 { "    help: " } else { "          " };
                out.push_str(&format!("{label}{line}\n"));
            }
        }
        out.push('\n');
    }
    out.push_str(&format!(
        "{} lint{}\n",
        lints.len(),
        if lints.len() == 1 { "" } else { "s" }
    ));
    out
}

/// Greedy word wrap, so help text does not run off the terminal.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if !current.is_empty() && current.len() + 1 + word.len() > width {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn print_phases(schema: &Schema) {
    let phases: Vec<&str> = schema.phases().collect();
    if phases.is_empty() {
        println!("This schema declares no phases; every pattern always runs.");
        return;
    }
    println!("Phases:");
    for phase in phases {
        let marker = if schema.default_phase() == Some(phase) {
            "  (default)"
        } else {
            ""
        };
        println!("  {phase}{marker}");
    }
    println!("\nAlso accepted: #ALL, #DEFAULT");
}

/// Renders the compiled schema, which is the fastest way to see what a schema
/// will actually do — including which rules compete inside a pattern.
fn explain(schema: &Schema) -> String {
    let mut out = String::new();
    if let Some(title) = schema.title() {
        out.push_str(&format!("{title}\n\n"));
    }
    out.push_str(&format!(
        "query binding: {}\n",
        schema.query_binding().as_str().unwrap_or("(default, XPath 1.0)")
    ));
    if matches!(schema.query_binding(), QueryBinding::Other(_)) {
        out.push_str("  warning: this binding is not implemented; expressions run as XPath 1.0\n");
    }
    let namespaces: Vec<String> = schema
        .namespaces()
        .iter()
        .map(|(prefix, uri)| format!("{prefix}={uri}"))
        .collect();
    if namespaces.is_empty() {
        out.push_str("namespaces: none declared\n");
    } else {
        out.push_str(&format!("namespaces: {}\n", namespaces.join(" ")));
    }
    out.push('\n');

    for pattern in schema.patterns() {
        let id = pattern.id.as_deref().unwrap_or("(unnamed)");
        out.push_str(&format!("pattern {id}\n"));
        if let Some(title) = &pattern.title {
            out.push_str(&format!("  {title}\n"));
        }
        for (index, rule) in pattern.rules.iter().enumerate() {
            out.push_str(&format!(
                "  rule {} context {}\n",
                index + 1,
                rule.context.as_deref().unwrap_or("(none)")
            ));
            if index > 0 {
                out.push_str("       (only fires on nodes no earlier rule claimed)\n");
            }
            for assertion in rule.assertions() {
                out.push_str(&format!(
                    "    {:<7} {}\n",
                    assertion.kind.as_str(),
                    assertion.test
                ));
            }
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> Schema {
        Schema::from_str(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron" defaultPhase="quick">
                 <phase id="quick"><active pattern="p"/></phase>
                 <pattern id="p">
                   <rule context="a"><assert test="b" flag="error">m</assert></rule>
                   <rule context="*"><assert test="true()">n</assert></rule>
                 </pattern>
               </schema>"#,
        )
        .unwrap()
    }

    #[test]
    fn explain_lists_patterns_rules_and_tests() {
        let text = explain(&schema());
        assert!(text.contains("pattern p"), "{text}");
        assert!(text.contains("context a"), "{text}");
        assert!(text.contains("assert  b"), "{text}");
    }

    #[test]
    fn explain_warns_that_later_rules_only_get_unclaimed_nodes() {
        let text = explain(&schema());
        assert!(text.contains("no earlier rule claimed"), "{text}");
    }

    #[test]
    fn flag_filtering_removes_other_findings() {
        let schema = Schema::from_str(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern><rule context="a">
                   <assert test="b" flag="error">e</assert>
                   <assert test="c" flag="warning">w</assert>
                 </rule></pattern>
               </schema>"#,
        )
        .unwrap();
        let document = Document::from_str("<a/>").unwrap();
        let mut report = schema.validate(&document).unwrap();
        assert_eq!(report.count_failures(), 2);
        retain_flags(&mut report, &["error".to_string()]);
        assert_eq!(report.count_failures(), 1);
    }
}
