//! The portability check: constructs that behave differently elsewhere.
//!
//! What makes this feature trustworthy is not that it reports something, but
//! that what it reports is *grounded*. Every kind corresponds to a divergence
//! recorded in `spec/conformance/`, each established by running this crate
//! and the ISO reference implementation against the same schema and comparing
//! their output.
//!
//! So the strongest test available is to point the check at the corpus cases
//! that **are** those divergences, and require it to notice them.

use std::fs;
use std::path::{Path, PathBuf};

use schematron::Schema;

fn corpus(case: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus")
        .join(case)
}

fn portability_of(case: &str) -> Vec<schematron::Lint> {
    let source = fs::read_to_string(corpus(case).join("schema.sch"))
        .unwrap_or_else(|e| panic!("corpus case {case:?} should have a schema: {e}"));
    let schema = Schema::from_str(&source)
        .unwrap_or_else(|e| panic!("corpus case {case:?} should compile: {e}"));
    schema.portability()
}

#[test]
fn the_cases_the_reference_cannot_compile_are_reported() {
    // These two are in the differential test's `REFERENCE_CANNOT_RUN` list:
    // the reference does not merely disagree, it refuses the schema. A schema
    // author running only this crate would never find out.
    for case in ["let-shadowing", "let-phase-scope"] {
        let found = portability_of(case);
        assert!(
            found
                .iter()
                .any(|lint| lint.kind == schematron::LintKind::VariableShadowsAnOuterScope),
            "{case} should report a shadowed variable, but reported {found:?}"
        );
    }
}

#[test]
fn the_documented_divergences_are_reported() {
    // Each of these is a numbered divergence in spec/conformance/.
    for (case, kind) in [
        ("node-kinds", schematron::LintKind::ContextSelectsANonElementKind),
        ("subject", schematron::LintKind::SubjectMovesTheLocation),
        ("rich-metadata", schematron::LintKind::FlagOrRoleOnTheRule),
    ] {
        let found = portability_of(case);
        assert!(
            found.iter().any(|lint| lint.kind == kind),
            "{case} should report {kind:?}, but reported {found:?}"
        );
    }
}

#[test]
fn the_remaining_detectable_divergences_are_reported() {
    for (case, kind) in [
        (
            "namespaced-attribute-context",
            schematron::LintKind::CollidingAttributeContexts,
        ),
        (
            "following-axis-from-attribute",
            schematron::LintKind::FollowingFromAnAttribute,
        ),
        (
            "message-inline-whitespace",
            schematron::LintKind::SpaceBetweenInlineElements,
        ),
    ] {
        let found = portability_of(case);
        assert!(
            found.iter().any(|lint| lint.kind == kind),
            "{case} should report {kind:?}, but reported {found:?}"
        );
    }
}

#[test]
fn a_schema_that_avoids_all_of_them_is_quiet() {
    // The check must be quiet on ordinary schemas, or it is noise. Nested
    // `let`s with distinct names are fine — the reference handles those.
    let schema = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <let name="base" value="10"/>
             <pattern>
               <let name="limit" value="$base * 2"/>
               <rule context="a">
                 <let name="count" value="count(b)"/>
                 <assert test="$count &lt;= $limit" flag="error">too many</assert>
               </rule>
             </pattern>
           </schema>"#,
    )
    .unwrap();
    assert_eq!(schema.portability(), Vec::new());
}

#[test]
fn sibling_scopes_binding_one_name_are_portable() {
    // Two rules each binding `$n` is ordinary: the reference compiles each
    // rule into its own template, so the bindings never meet. Reporting this
    // would make the check unusable on real schemas.
    let schema = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern>
               <rule context="a"><let name="n" value="1"/><assert test="$n">a</assert></rule>
               <rule context="b"><let name="n" value="2"/><assert test="$n">b</assert></rule>
             </pattern>
           </schema>"#,
    )
    .unwrap();
    assert_eq!(schema.portability(), Vec::new());
}

#[test]
fn portability_findings_are_kept_out_of_lint() {
    // The linter's rule is that a false positive costs more than a miss.
    // These constructs are correct, so `lint()` must stay silent about them.
    let schema = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern>
               <rule context="a" flag="warning" subject="b">
                 <assert test="b">needs a b</assert>
               </rule>
             </pattern>
           </schema>"#,
    )
    .unwrap();
    assert!(schema.lint().is_empty(), "{:?}", schema.lint());
    assert_eq!(schema.portability().len(), 2);
}

#[test]
fn no_message_or_help_carries_a_run_of_spaces() {
    // A Rust string split across source lines needs a trailing `\`, and
    // without one the indentation of the continuation lands *inside* the
    // string. The result reads fine in the source and prints as a gap in the
    // user's terminal. Checking the rendered text catches it wherever it
    // happens, which reviewing the source reliably does not.
    let schema = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <let name="dup" value="1"/>
             <pattern id="p">
               <let name="dup" value="2"/>
               <rule context="comment()" flag="warning" subject="x">
                 <assert test="a">m</assert>
               </rule>
             </pattern>
           </schema>"#,
    )
    .unwrap();

    let reported = schema
        .portability()
        .into_iter()
        .chain(schema.lint())
        .collect::<Vec<_>>();
    assert!(!reported.is_empty(), "the probe schema should report something");

    for lint in reported {
        assert!(
            !lint.message.contains("  "),
            "{:?} message has a run of spaces: {:?}",
            lint.kind,
            lint.message
        );
        if let Some(help) = &lint.help {
            assert!(
                !help.contains("  "),
                "{:?} help has a run of spaces: {:?}",
                lint.kind,
                help
            );
        }
    }
}
