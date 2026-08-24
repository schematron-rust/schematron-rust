//! Integration tests for the command line tool.
//!
//! These run the built binary, so they check the parts the library tests
//! cannot: argument parsing, output selection, and — most importantly — the
//! exit codes, which is what a build pipeline actually depends on.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use assertables::*;

/// The path of the binary Cargo built for this test run.
fn binary() -> PathBuf {
    // The test executable lives in target/<profile>/deps/; the binary is two
    // levels up from there.
    let mut path = std::env::current_exe().expect("test executable path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join(format!("schematron{}", std::env::consts::EXE_SUFFIX))
}

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples")
}

fn run(args: &[&str]) -> Output {
    Command::new(binary())
        .args(args)
        .output()
        .expect("the schematron binary should run")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn code(output: &Output) -> i32 {
    output.status.code().expect("the process should exit normally")
}

fn schema_path() -> String {
    examples().join("invoice.sch").display().to_string()
}

fn good() -> String {
    examples().join("invoice-good.xml").display().to_string()
}

fn bad() -> String {
    examples().join("invoice-bad.xml").display().to_string()
}

#[test]
fn a_valid_document_exits_zero() {
    let output = run(&["--schema", &schema_path(), "--phase", "#ALL", &good()]);
    assert_eq!(code(&output), 0);
    assert_contains!(stdout(&output), "no findings");
}

#[test]
fn an_invalid_document_exits_one() {
    let output = run(&["--schema", &schema_path(), "--phase", "#ALL", &bad()]);
    assert_eq!(code(&output), 1);
    assert_contains!(stdout(&output), "An invoice must have an id.");
}

#[test]
fn no_documents_is_a_usage_error() {
    let output = run(&["--schema", &schema_path()]);
    assert_eq!(code(&output), 2);
}

#[test]
fn an_unreadable_schema_exits_three() {
    let output = run(&["--schema", "/nonexistent/rules.sch", &good()]);
    assert_eq!(code(&output), 3);
}

#[test]
fn an_unreadable_document_exits_four() {
    let output = run(&["--schema", &schema_path(), "/nonexistent/data.xml"]);
    assert_eq!(code(&output), 4);
}

#[test]
fn the_default_phase_applies_when_none_is_named() {
    // The default phase runs only the structure pattern, so the totals
    // warning from the bad invoice does not appear.
    let output = run(&["--schema", &schema_path(), &bad()]);
    let text = stdout(&output);
    assert_contains!(text, "An invoice must have an id.");
    assert_not_contains!(text, "the lines plus tax");
}

#[test]
fn svrl_output_is_well_formed_and_carries_findings() {
    let output = run(&[
        "--schema", &schema_path(), "--phase", "#ALL", "--format", "svrl", &bad(),
    ]);
    let text = stdout(&output);
    assert!(text.starts_with("<?xml"), "{text}");
    assert_contains!(text, "svrl:schematron-output");
    assert_contains!(text, "svrl:failed-assert");
    assert_contains!(text, "</svrl:schematron-output>");
}

#[test]
fn json_output_parses_as_json() {
    let output = run(&[
        "--schema", &schema_path(), "--phase", "#ALL", "--format", "json", &bad(),
    ]);
    let text = stdout(&output);
    assert_contains!(text, "\"patterns\"");
    assert_contains!(text, "\"FailedAssert\"");
}

#[test]
fn flag_filtering_narrows_both_output_and_exit_code() {
    // Only the warning-flagged finding survives, so it is still an exit 1.
    let output = run(&[
        "--schema", &schema_path(), "--phase", "#ALL", "--flag", "warning", &bad(),
    ]);
    let text = stdout(&output);
    assert_contains!(text, "the lines plus tax");
    assert_not_contains!(text, "An invoice must have an id.");
    assert_eq!(code(&output), 1);
}

#[test]
fn quiet_suppresses_output_but_keeps_the_exit_code() {
    let output = run(&["--schema", &schema_path(), "--phase", "#ALL", "--quiet", &bad()]);
    assert_eq!(code(&output), 1);
    assert_is_empty!(stdout(&output));
}

#[test]
fn max_failures_stops_early() {
    let output = run(&[
        "--schema", &schema_path(), "--phase", "#ALL", "--max-failures", "1", &bad(),
    ]);
    let text = stdout(&output);
    assert_contains!(text, "1 finding");
    assert_eq!(code(&output), 1);
}

#[test]
fn list_phases_prints_the_phases_and_the_default() {
    let output = run(&["--schema", &schema_path(), "--list-phases"]);
    let text = stdout(&output);
    assert_contains!(text, "basic");
    assert_contains!(text, "strict");
    assert_contains!(text, "(default)");
    assert_eq!(code(&output), 0);
}

#[test]
fn explain_shows_the_compiled_schema() {
    let output = run(&["--schema", &schema_path(), "--explain"]);
    let text = stdout(&output);
    assert_contains!(text, "pattern structure");
    assert_contains!(text, "context invoice");
    assert_contains!(text, "no earlier rule claimed");
    assert_eq!(code(&output), 0);
}

#[test]
fn verbose_shows_tests_and_rules() {
    let output = run(&[
        "--schema", &schema_path(), "--phase", "#ALL", "--verbose", &bad(),
    ]);
    let text = stdout(&output);
    assert_contains!(text, "test: @id");
    assert_contains!(text, "rule: invoice");
}

#[test]
fn several_documents_are_validated_in_one_run() {
    let output = run(&["--schema", &schema_path(), "--phase", "#ALL", &good(), &bad()]);
    let text = stdout(&output);
    assert_contains!(text, "invoice-good.xml");
    assert_contains!(text, "invoice-bad.xml");
    // One bad document is enough to fail the run.
    assert_eq!(code(&output), 1);
}

#[test]
fn output_can_be_written_to_a_file() {
    let target = std::env::temp_dir().join("schematron-cli-test.svrl");
    let _ = std::fs::remove_file(&target);
    let output = run(&[
        "--schema", &schema_path(), "--phase", "#ALL",
        "--format", "svrl",
        "--output", &target.display().to_string(),
        &bad(),
    ]);
    assert_eq!(code(&output), 1);
    let written = std::fs::read_to_string(&target).expect("the report should have been written");
    assert_contains!(written, "svrl:schematron-output");
    let _ = std::fs::remove_file(&target);
}

#[test]
fn help_and_version_work() {
    let help = run(&["--help"]);
    assert_eq!(code(&help), 0);
    assert_contains!(stdout(&help), "EXIT CODES");
    assert_contains!(stdout(&help), "--phase");

    let version = run(&["--version"]);
    assert_eq!(code(&version), 0);
    assert_contains!(stdout(&version), "schematron");
}

/// Extracts long option names (`--like-this`) from a body of text.
fn long_flags(source: &str) -> std::collections::BTreeSet<String> {
    let mut flags = std::collections::BTreeSet::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i + 2 < bytes.len() {
        if bytes[i] == b'-' && bytes[i + 1] == b'-' && bytes[i + 2].is_ascii_alphabetic() {
            let start = i + 2;
            let mut end = start;
            while end < bytes.len()
                && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'-')
            {
                end += 1;
            }
            // Trim a trailing hyphen left by a line break or an em-dash.
            let name = source[start..end].trim_end_matches('-');
            if !name.is_empty() {
                flags.insert(name.to_string());
            }
            i = end;
            continue;
        }
        i += 1;
    }
    flags
}

#[test]
fn every_cli_flag_is_documented_and_every_documented_flag_exists() {
    // The binary is the source of truth; spec/cli/ must match it exactly.
    // Drift in either direction is a defect: an undocumented flag is hidden,
    // and a documented flag that does not exist is a promise the tool breaks.
    let help = stdout(&run(&["--help"]));
    let mut actual = long_flags(&help);
    // `--help` and `--version` are clap's, and are described in prose rather
    // than in the options table.
    actual.remove("help");
    actual.remove("version");

    let spec_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("spec/cli/index.md");
    let spec = std::fs::read_to_string(&spec_path).expect("spec/cli/ should exist");
    let documented = long_flags(&spec);

    let undocumented: Vec<&String> = actual.difference(&documented).collect();
    assert!(
        undocumented.is_empty(),
        "flag(s) the tool accepts but spec/cli/ does not document: {undocumented:?}"
    );

    // Anything in the spec that looks like a flag must actually exist. The
    // spec also names flags of *other* tools in its examples, so only check
    // the ones listed in its options table.
    let table: String = spec
        .lines()
        .filter(|line| line.starts_with("| `-"))
        .collect::<Vec<_>>()
        .join("\n");
    let promised = long_flags(&table);
    let missing: Vec<&String> = promised.difference(&actual).collect();
    assert!(
        missing.is_empty(),
        "spec/cli/ documents flag(s) the tool does not accept: {missing:?}"
    );
}

#[test]
fn every_documented_exit_code_is_described_consistently() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let help = stdout(&run(&["--help"]));
    let spec = std::fs::read_to_string(root.join("spec/cli/index.md")).expect("spec/cli/");
    let readme = std::fs::read_to_string(root.join("README.md")).expect("README.md");

    // All three places must agree on what each code means.
    for (code, meaning) in [
        (0, "valid"),
        (1, "failed assertion"),
        (2, "usage error"),
        (3, "schema error"),
        (4, "document error"),
    ] {
        for (name, text) in [("--help", &help), ("spec/cli/", &spec), ("README.md", &readme)] {
            // Case-insensitive: a table cell reasonably starts with a capital.
            let text = text.to_lowercase();
            assert!(
                text.contains(&code.to_string()) && text.contains(meaning),
                "{name} does not describe exit code {code} as {meaning:?}"
            );
        }
    }
}

/// Writes a schema to a temporary file and returns its path.
fn temp_schema(name: &str, source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("schematron-cli-{name}.sch"));
    std::fs::write(&path, source).expect("temporary schema should be writable");
    path
}

#[test]
fn lint_reports_nothing_for_a_clean_schema() {
    let output = run(&["--schema", &schema_path(), "--lint"]);
    assert_eq!(code(&output), 0);
    assert_contains!(stdout(&output), "no lints");
}

#[test]
fn lint_reports_problems_and_exits_one() {
    // Exit 1 so a build can gate on the schema being clean.
    let path = temp_schema(
        "lint-dirty",
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern>
               <rule context="*"><assert test="@id">m</assert></rule>
               <rule context="invoice"><assert test="total">m</assert></rule>
             </pattern>
             <diagnostics><diagnostic id="unused">text</diagnostic></diagnostics>
           </schema>"#,
    );

    let output = run(&["--schema", &path.display().to_string(), "--lint"]);
    assert_eq!(code(&output), 1);

    let text = stdout(&output);
    assert_contains!(text, "unreachable-rule");
    assert_contains!(text, "unreferenced-diagnostic");
    // Every lint must carry a location and actionable help.
    assert_contains!(text, "pattern[1]/rule[2]");
    assert_contains!(text, "help:");
    assert_contains!(text, "2 lints");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn lint_needs_no_document() {
    // The schema is the whole input; passing a document is not required.
    let output = run(&["--schema", &schema_path(), "--lint"]);
    assert_eq!(code(&output), 0);
}

#[test]
fn lint_catches_the_missing_namespace_prefix_mistake() {
    // The single most common reason a schema silently does nothing.
    let path = temp_schema(
        "lint-namespace",
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <ns prefix="inv" uri="urn:invoice"/>
             <pattern>
               <rule context="invoice"><assert test="total">m</assert></rule>
             </pattern>
           </schema>"#,
    );

    let output = run(&["--schema", &path.display().to_string(), "--lint"]);
    assert_eq!(code(&output), 1);
    let text = stdout(&output);
    assert_contains!(text, "unprefixed-name");
    assert_contains!(text, "no default namespace");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn the_bundled_example_schema_is_lint_clean() {
    // The crate's own worked example must not model bad practice.
    let output = run(&["--schema", &schema_path(), "--lint"]);
    assert_eq!(code(&output), 0, "{}", stdout(&output));
}

#[test]
fn parallel_produces_the_same_output_as_sequential() {
    // The flag is a performance switch, not a behaviour switch. If the two
    // ever diverge, that is the bug this catches.
    let sequential = run(&["--schema", &schema_path(), "--phase", "#ALL", &bad()]);
    let parallel = run(&[
        "--schema", &schema_path(), "--phase", "#ALL", "--parallel", &bad(),
    ]);

    assert_eq!(stdout(&sequential), stdout(&parallel));
    assert_eq!(code(&sequential), code(&parallel));
}

#[test]
fn parallel_produces_the_same_svrl_as_sequential() {
    let schema = schema_path();
    let document = bad();
    let base = ["--schema", &schema, "--phase", "#ALL", "--format", "svrl"];

    let mut sequential_args = base.to_vec();
    sequential_args.push(&document);
    let sequential = run(&sequential_args);

    let mut parallel_args = base.to_vec();
    parallel_args.push("--parallel");
    parallel_args.push(&document);
    let parallel = run(&parallel_args);

    assert_eq!(stdout(&sequential), stdout(&parallel));
}
