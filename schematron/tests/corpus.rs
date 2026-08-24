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

    // From the path, not the text: a case may `include` a sibling file, and
    // a relative href needs the schema's own location to resolve against.
    let schema = Schema::from_path(directory.join("schema.sch"))
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
fn every_expected_location_resolves_to_exactly_one_node() {
    // SVRL's `@location` exists so that a consumer can find the node the
    // finding is about. That only works if the value is an XPath the consumer
    // can evaluate — which, for the SVRL toolchain, means XPath 1.0.
    //
    // Each location is checked by evaluating `count(LOCATION) = 1` under the
    // default XPath 1.0 binding. That catches both halves at once: syntax
    // XPath 1.0 does not have fails to compile, and a location pointing at
    // the wrong number of nodes fails to report.
    //
    // This test reads the *expectation files*, so on its own it says nothing
    // about what `Document::location` produces — sabotage the generator and
    // this test still passes. It is `corpus_cases_all_pass` that ties the two
    // together, by comparing emitted locations against these same files. The
    // pair is what pins the format; neither does it alone.
    let mut failures = Vec::new();

    for case in cases() {
        let name = case.file_name().unwrap().to_string_lossy().to_string();
        let Ok(expected) = fs::read_to_string(case.join("expected.txt")) else {
            continue;
        };
        let locations: Vec<String> = parse_expected(&expected)
            .into_iter()
            .map(|e| e.location)
            .filter(|location| !location.is_empty())
            .collect();
        if locations.is_empty() {
            continue;
        }

        let mut schema = String::from(
            "<schema xmlns=\"http://purl.oclc.org/dsdl/schematron\">\n  <pattern>\n    <rule context=\"/\">\n",
        );
        for (index, location) in locations.iter().enumerate() {
            let escaped = location.replace('&', "&amp;").replace('<', "&lt;");
            schema.push_str(&format!(
                "      <report test=\"count({escaped}) = 1\">loc{index}</report>\n"
            ));
        }
        schema.push_str("    </rule>\n  </pattern>\n</schema>\n");

        let compiled = match Schema::from_str(&schema) {
            Ok(compiled) => compiled,
            Err(why) => {
                failures.push(format!("{name}: a location is not valid XPath 1.0: {why}"));
                continue;
            }
        };
        let Ok(document) = fs::read_to_string(case.join("input.xml"))
            .map_err(drop)
            .and_then(|s| Document::from_str(&s).map_err(drop))
        else {
            continue;
        };
        let report = compiled.validate(&document).expect("the probe schema runs");
        let fired: Vec<&str> = report
            .assertions()
            .map(|assertion| assertion.text.trim())
            .collect();
        for (index, location) in locations.iter().enumerate() {
            let label = format!("loc{index}");
            if !fired.iter().any(|text| *text == label) {
                failures.push(format!(
                    "{name}: location {location:?} does not select exactly one node"
                ));
            }
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_corpus_case_is_documented_and_every_documented_case_exists() {
    // spec/testing/ tabulates what each case pins down. Drift in either
    // direction is a defect: an undocumented case is invisible to a reader
    // deciding whether a behaviour is covered, and a documented case that
    // does not exist is a claim of coverage that is not there.
    let spec = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("spec/testing/index.md"),
    )
    .expect("spec/testing/ should exist");

    // Only the corpus table counts. The file holds other tables whose first
    // column is also a backticked lowercase name — the sabotage coverage
    // table, for one — and scanning the whole document read those as case
    // names, which is a false failure waiting for whoever adds the next
    // table. The section heading is the boundary.
    let table = {
        let start = spec
            .find("## Corpus tests")
            .expect("spec/testing/ should have a '## Corpus tests' section");
        let rest = &spec[start + "## Corpus tests".len()..];
        let end = rest.find("\n## ").unwrap_or(rest.len());
        &rest[..end]
    };

    for case in cases() {
        let name = case.file_name().unwrap().to_string_lossy().to_string();
        assert!(
            table.contains(&format!("| `{name}` |")),
            "corpus case {name:?} is not in the table under '## Corpus tests' \
             in spec/testing/"
        );
    }

    // And nothing in the table is fictional.
    for line in table.lines().filter(|line| line.starts_with("| `")) {
        let Some(name) = line.strip_prefix("| `").and_then(|r| r.split('`').next()) else {
            continue;
        };
        let path = corpus_root().join(name);
        assert!(
            path.is_dir(),
            "spec/testing/ documents corpus case {name:?}, which does not exist"
        );
    }
}

/// Clears the fields an SVRL round trip cannot preserve.
///
/// SVRL's `fired-rule` element has nowhere to record which node the rule
/// fired on, so that one field is compared as empty on both sides. See
/// `spec/svrl/`.
fn without_fired_rule_locations(mut report: schematron::Report) -> schematron::Report {
    for pattern in &mut report.patterns {
        for rule in &mut pattern.rules {
            rule.location = String::new();
        }
    }
    report
}

#[test]
fn every_corpus_report_survives_an_svrl_round_trip() {
    // A far stronger check on the SVRL writer than asserting on substrings of
    // its output: everything it emits must come back identical.
    let mut failures = Vec::new();

    for case in cases() {
        let name = case.file_name().unwrap().to_string_lossy().to_string();
        let Ok(schema) = Schema::from_path(case.join("schema.sch")).map_err(drop) else {
            failures.push(format!("{name}: schema did not compile"));
            continue;
        };
        let Ok(document) = fs::read_to_string(case.join("input.xml"))
            .map_err(drop)
            .and_then(|s| Document::from_str(&s).map_err(drop))
        else {
            failures.push(format!("{name}: document did not parse"));
            continue;
        };

        let mut options = ValidateOptions::new();
        if let Ok(phase) = fs::read_to_string(case.join("phase")) {
            options = options.with_phase(PhaseSelection::from(phase.trim()));
        }
        let Ok(original) = schema.validate_with(&document, &options) else {
            failures.push(format!("{name}: validation failed"));
            continue;
        };

        let svrl = original.to_svrl();
        match schematron::Report::from_svrl(&svrl) {
            Err(error) => failures.push(format!("{name}: SVRL did not parse back: {error}")),
            Ok(parsed) => {
                let expected = without_fired_rule_locations(original);
                let actual = without_fired_rule_locations(parsed);
                if expected != actual {
                    failures.push(format!(
                        "{name}: round trip changed the report\n  before: {expected:?}\n  after:  {actual:?}"
                    ));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} corpus case(s) failed the SVRL round trip:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
