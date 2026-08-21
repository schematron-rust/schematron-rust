//! The corpus conformance suite.
//!
//! Each case is a directory under `tests/corpus/`:
//!
//! ```text
//! tests/corpus/<case>/
//!   schema.sch     the schema
//!   input.xml      the document to validate
//!   expected.txt   expected findings, one per line
//!   phase          optional, the phase to run
//! ```
//!
//! Each line of `expected.txt` is `KIND | location | text`, where `KIND` is
//! `assert` for a failed assertion or `report` for a successful report, and
//! `text` has its whitespace normalised. Blank lines and `#` comments are
//! ignored, so a case can explain itself.
//!
//! Adding a case is adding a directory. No Rust code changes.

use std::fs;
use std::path::{Path, PathBuf};

use schematron::validate::{PhaseSelection, ResultKind, ValidateOptions};
use schematron::{Document, Schema};

/// One expected finding.
#[derive(Debug, PartialEq, Eq)]
struct Expected {
    kind: String,
    location: String,
    text: String,
}

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

fn cases() -> Vec<PathBuf> {
    let mut cases: Vec<PathBuf> = fs::read_dir(corpus_root())
        .expect("tests/corpus should exist")
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    cases.sort();
    cases
}

/// Collapses whitespace, so a case file can wrap its expectations to a
/// readable width without that changing what is compared.
fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_expected(source: &str) -> Vec<Expected> {
    source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let parts: Vec<&str> = line.splitn(3, '|').collect();
            assert!(
                parts.len() == 3,
                "expected.txt lines must be `KIND | location | text`, got: {line}"
            );
            Expected {
                kind: parts[0].trim().to_string(),
                location: parts[1].trim().to_string(),
                text: normalize(parts[2]),
            }
        })
        .collect()
}

fn run_case(directory: &Path) -> Result<(), String> {
    let name = directory.file_name().unwrap().to_string_lossy().to_string();
    let read = |file: &str| -> Result<String, String> {
        fs::read_to_string(directory.join(file))
            .map_err(|e| format!("{name}: cannot read {file}: {e}"))
    };

    let schema = Schema::from_str(&read("schema.sch")?)
        .map_err(|e| format!("{name}: schema did not compile: {e}"))?;
    let document = Document::from_str(&read("input.xml")?)
        .map_err(|e| format!("{name}: document did not parse: {e}"))?;

    let mut options = ValidateOptions::new();
    if let Ok(phase) = fs::read_to_string(directory.join("phase")) {
        options = options.with_phase(PhaseSelection::from(phase.trim()));
    }

    let report = schema
        .validate_with(&document, &options)
        .map_err(|e| format!("{name}: validation failed: {e}"))?;

    let actual: Vec<Expected> = report
        .assertions()
        .map(|a| Expected {
            kind: match a.kind {
                ResultKind::FailedAssert => "assert".to_string(),
                ResultKind::SuccessfulReport => "report".to_string(),
            },
            location: a.location.clone(),
            text: normalize(&a.text),
        })
        .collect();

    let expected = parse_expected(&read("expected.txt")?);

    if actual == expected {
        return Ok(());
    }
    Err(format!(
        "{name}: findings did not match\n  expected ({}):\n{}\n  actual ({}):\n{}",
        expected.len(),
        render(&expected),
        actual.len(),
        render(&actual)
    ))
}

fn render(findings: &[Expected]) -> String {
    if findings.is_empty() {
        return "    (none)".to_string();
    }
    findings
        .iter()
        .map(|f| format!("    {} | {} | {}", f.kind, f.location, f.text))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn corpus_cases_all_pass() {
    let cases = cases();
    assert!(!cases.is_empty(), "the corpus should contain cases");

    let mut failures = Vec::new();
    for case in &cases {
        if let Err(message) = run_case(case) {
            failures.push(message);
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} corpus cases failed:\n\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n\n")
    );
}

#[test]
fn every_corpus_case_is_complete() {
    for case in cases() {
        for file in ["schema.sch", "input.xml", "expected.txt"] {
            assert!(
                case.join(file).exists(),
                "corpus case {} is missing {file}",
                case.display()
            );
        }
    }
}

#[test]
fn every_corpus_case_is_documented_and_every_documented_case_exists() {
    // spec/testing.md tabulates what each case pins down. Drift in either
    // direction is a defect: an undocumented case is invisible to a reader
    // deciding whether a behaviour is covered, and a documented case that
    // does not exist is a claim of coverage that is not there.
    let spec = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("spec/testing.md"),
    )
    .expect("spec/testing.md should exist");

    for case in cases() {
        let name = case.file_name().unwrap().to_string_lossy().to_string();
        assert!(
            spec.contains(&format!("| `{name}` |")),
            "corpus case {name:?} is not in the table in spec/testing.md"
        );
    }

    // And nothing in the table is fictional.
    for line in spec.lines().filter(|line| line.starts_with("| `")) {
        let Some(name) = line.strip_prefix("| `").and_then(|r| r.split('`').next()) else {
            continue;
        };
        // The table also lists non-case entries elsewhere in the file; only
        // rows naming a directory that looks like a case are checked.
        if !name.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
            continue;
        }
        let path = corpus_root().join(name);
        assert!(
            path.is_dir(),
            "spec/testing.md documents corpus case {name:?}, which does not exist"
        );
    }
}
