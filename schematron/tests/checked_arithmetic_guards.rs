//! The refusals of the checked arithmetic in the XPath engine (RCA-TEST-2).
//!
//! Each case is the first input at which an operation does not fit, reached
//! through the public evaluator. The result is an error naming the figure,
//! never a clamped, rounded or wrapped value (RCA-P-8).

use schematron::xml::Document;
use schematron::xpath::{evaluate, parse, EvalContext, Namespaces, Variables, XPathVersion};
use schematron::{Schema, ValidateOptions};

/// The error message of evaluating `source` as XPath 3.0, with the run's instant set to `now`.
fn failure_at(source: &str, now: f64) -> String {
    let document = Document::from_str("<a/>").unwrap();
    let variables = Variables::new();
    let namespaces = Namespaces::new();
    let context = EvalContext::new(
        &document,
        document.document_element().unwrap(),
        &variables,
        &namespaces,
    )
    .with_version(XPathVersion::V3)
    .with_current_time(now);
    let expression = parse(source).unwrap_or_else(|error| panic!("{source} should parse: {error}"));
    match evaluate(&expression, &context) {
        Ok(value) => panic!("{source} should be refused, but gave {value:?}"),
        Err(error) => error.to_string(),
    }
}

fn failure(source: &str) -> String {
    failure_at(source, 0.0)
}

const FAR_DATE: &str = "xs:date('9223372036854775807-01-01')";
const FAR_DATE_TIME: &str = "xs:dateTime('9223372036854775807-01-01T00:00:00Z')";
const HUGE_YEARS: &str = "xs:yearMonthDuration('P700000000000000000Y')";
const MIN_MONTHS: &str = "(xs:yearMonthDuration('-P4611686018427387904M') + \
                          xs:yearMonthDuration('-P4611686018427387904M'))";

#[test]
fn a_month_count_above_2_pow_53_cannot_be_divided_or_scaled_exactly() {
    for source in [
        format!("{HUGE_YEARS} div xs:yearMonthDuration('P1M')"),
        format!("xs:yearMonthDuration('P1M') div {HUGE_YEARS}"),
        format!("{HUGE_YEARS} * 2"),
        "xs:yearMonthDuration('P1Y') * 1000000000000000000000000000000".to_string(),
    ] {
        assert!(
            failure(&source).contains("year-month duration is too large"),
            "{source}"
        );
    }
}

#[test]
fn month_arithmetic_that_overflows_is_refused() {
    let sum = "xs:yearMonthDuration('P4611686018427387904M') + \
               xs:yearMonthDuration('P4611686018427387904M')";
    assert!(failure(sum).contains("year-month duration is too large"));
    // Negating the smallest month count has no `i64`.
    for source in [
        format!("xs:date('2000-01-01') - {MIN_MONTHS}"),
        format!("xs:yearMonthDuration('P1M') - {MIN_MONTHS}"),
    ] {
        assert!(
            failure(&source).contains("year-month duration is too large"),
            "{source}"
        );
    }
}

#[test]
fn dates_too_far_from_the_epoch_cannot_be_subtracted_or_moved() {
    let subtraction = "xs:date('1000000000000000-01-01') - xs:date('2000-01-01')";
    assert!(failure(subtraction).contains("outside the range"));
    for source in [
        format!("{FAR_DATE} + xs:yearMonthDuration('P1Y')"),
        format!("{FAR_DATE} + xs:dayTimeDuration('P1D')"),
        format!("xs:yearMonthDuration('P1Y') + {FAR_DATE}"),
        format!("xs:dayTimeDuration('P1D') + {FAR_DATE}"),
    ] {
        assert!(failure(&source).contains("outside the range"), "{source}");
    }
}

#[test]
fn function_results_that_have_no_exact_double_or_date_are_refused() {
    let adjusted =
        format!("adjust-dateTime-to-timezone({FAR_DATE_TIME}, xs:dayTimeDuration('PT1H'))");
    assert!(failure(&adjusted).contains("outside the range"));
    assert!(
        failure("years-from-duration(xs:yearMonthDuration('P700000000000000000Y'))")
            .contains("too large")
    );
    assert!(failure("year-from-date(xs:date('10000000000000000-01-01'))").contains("too large"));
    // An instant that is not a calendar date cannot be today.
    assert!(failure_at("current-date()", 1e300).contains("outside the range"));
}

#[test]
fn a_function_reference_arity_that_does_not_fit_is_refused() {
    let error = parse("name#99999999999999999999").unwrap_err();
    assert!(error.to_string().contains("arity"), "{error}");
}

#[test]
fn invalid_options_are_refused_before_a_streaming_run_reads_anything() {
    let schema = Schema::from_str(
        r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
             <pattern><rule context="record"><assert test="@id">id</assert></rule></pattern>
           </schema>"#,
    )
    .unwrap();
    let options = ValidateOptions::new().with_implicit_timezone(24 * 60);
    let error = schema
        .validate_streaming("<records><record id='1'/></records>".as_bytes(), &options)
        .unwrap_err();
    assert!(error.to_string().contains("implicit_timezone"), "{error}");
}
