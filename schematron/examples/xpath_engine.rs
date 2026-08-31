//! Use the XPath 1.0 engine on its own, without Schematron.
//!
//! The engine is public because it is useful independently — and because the
//! surprising parts of XPath 1.0 are easier to understand when you can poke
//! at them directly. This example demonstrates three of them.
//!
//! ```sh
//! cargo run --example xpath_engine
//! ```

use schematron::xml::Document;
use schematron::xpath::{evaluate, parse, EvalContext, Namespaces, NumericType, Value, Variables};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let document = Document::from_str(
        r#"<order id="A-1">
             <line qty="2" amount="10.00"/>
             <line qty="3" amount="15.00"/>
             <line qty="0" amount="0.00"/>
           </order>"#,
    )?;
    let node = document.document_element().expect("a document element");

    let mut variables = Variables::new();
    variables.bind("minimum", Value::Number(1.0, NumericType::Double));
    let namespaces = Namespaces::new();

    let evaluate_one = |expression: &str| -> Result<Value, Box<dyn std::error::Error>> {
        let expr = parse(expression)?;
        let context = EvalContext::new(&document, node, &variables, &namespaces);
        Ok(evaluate(&expr, &context)?)
    };

    println!("=== ordinary queries ===");
    for expression in [
        "count(line)",
        "sum(line/@amount)",
        "line[number(@qty) >= $minimum]/@amount",
        "normalize-space(@id)",
        "count(line[position() > 1])",
    ] {
        let value = evaluate_one(expression)?;
        println!("  {expression:<40} => {}", value.to_xpath_string(&document));
    }

    println!("\n=== XPath 1.0 surprises ===");

    // 1. Node-set comparison is existential, so `=` and `!=` can both be true.
    let equal = evaluate_one("line/@qty = 2")?.to_boolean();
    let not_equal = evaluate_one("line/@qty != 2")?.to_boolean();
    println!("  line/@qty  = 2  => {equal}");
    println!("  line/@qty != 2  => {not_equal}");
    println!("  both true at once, because each asks whether SOME node satisfies it");

    // 2. Relational comparison always converts to number, so a non-numeric
    //    string becomes NaN and every comparison against it is false.
    println!("\n  '@id > 0'       => {}", evaluate_one("@id > 0")?.to_boolean());
    println!("  '@id <= 0'      => {}", evaluate_one("@id <= 0")?.to_boolean());
    println!("  both false, because 'A-1' converts to NaN");

    // 3. Number formatting is XPath's, not Rust's: no exponent, ever.
    println!(
        "\n  string(1 div 3) => {}",
        evaluate_one("1 div 3")?.to_xpath_string(&document)
    );
    println!(
        "  string(1 div 0) => {}",
        evaluate_one("1 div 0")?.to_xpath_string(&document)
    );

    Ok(())
}
