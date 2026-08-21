//! Pass 4 of schema compilation: expanding abstractions.
//!
//! Two independent expansions, in this order: abstract rules, then abstract
//! patterns. Both happen on the model while expressions are still source
//! text, because abstract-pattern parameter substitution is textual — that is
//! how the standard defines it, and it is why `$parent` can stand for an
//! element name rather than only for a value.

use std::collections::HashMap;

use super::model::{Assertion, Content, Let, LetValue, Pattern, Rule, RuleChild, SchemaModel};
use crate::error::{Error, Result};

/// The maximum depth of `extends` chains, as a runaway guard.
const MAX_EXTENDS_DEPTH: usize = 64;

/// Expands abstract rules and abstract patterns, leaving only runnable ones.
pub(crate) fn expand(model: &mut SchemaModel) -> Result<()> {
    expand_rules(model)?;
    expand_patterns(model)?;
    Ok(())
}

/// Splices each `<extends rule="ID"/>` into the rule that references it.
///
/// The splice happens at the position of the `extends` element, so report
/// order matches document order in the schema.
fn expand_rules(model: &mut SchemaModel) -> Result<()> {
    for pattern in &mut model.patterns {
        let abstracts: HashMap<String, Rule> = pattern
            .rules
            .iter()
            .filter(|rule| rule.is_abstract)
            .filter_map(|rule| rule.id.clone().map(|id| (id, rule.clone())))
            .collect();

        for index in 0..pattern.rules.len() {
            if pattern.rules[index].is_abstract {
                continue;
            }
            let mut chain = Vec::new();
            let rule = pattern.rules[index].clone();
            pattern.rules[index] = splice(rule, &abstracts, &mut chain)?;
        }

        // Abstract rules exist only to be spliced; they never fire.
        pattern.rules.retain(|rule| !rule.is_abstract);
    }
    Ok(())
}

/// Replaces every `Extends` in a rule's body with the target's body.
fn splice(
    mut rule: Rule,
    abstracts: &HashMap<String, Rule>,
    chain: &mut Vec<String>,
) -> Result<Rule> {
    if chain.len() > MAX_EXTENDS_DEPTH {
        return Err(Error::schema(
            "extends",
            rule.id.clone(),
            format!("extends chains nested deeper than the limit of {MAX_EXTENDS_DEPTH}"),
        ));
    }

    let mut body = Vec::with_capacity(rule.body.len());
    let mut lets = Vec::new();

    for child in std::mem::take(&mut rule.body) {
        match child {
            RuleChild::Assertion(assertion) => body.push(RuleChild::Assertion(assertion)),
            RuleChild::Extends(target) => {
                if chain.iter().any(|seen| seen == &target) {
                    let mut chain = chain.clone();
                    chain.push(target);
                    return Err(Error::schema(
                        "extends",
                        rule.id.clone(),
                        format!("extends cycle: {}", chain.join(" -> ")),
                    ));
                }
                let base = abstracts.get(&target).cloned().ok_or_else(|| {
                    Error::schema(
                        "extends",
                        rule.id.clone(),
                        format!(
                            "no abstract rule with @id={target:?} in this pattern; \
                             an abstract rule must be in the same pattern as the rule \
                             that extends it"
                        ),
                    )
                })?;

                chain.push(target);
                let expanded = splice(base, abstracts, chain)?;
                chain.pop();

                // The extended rule's own variables come first, so the
                // extending rule can shadow them.
                lets.extend(expanded.lets);
                body.extend(expanded.body);
            }
        }
    }

    lets.extend(std::mem::take(&mut rule.lets));
    rule.lets = lets;
    rule.body = body;
    Ok(rule)
}

/// Replaces each `is-a` pattern with a copy of its abstract pattern's rules,
/// with `$param` placeholders substituted.
fn expand_patterns(model: &mut SchemaModel) -> Result<()> {
    let abstracts: HashMap<String, Pattern> = model
        .patterns
        .iter()
        .filter(|pattern| pattern.is_abstract)
        .filter_map(|pattern| pattern.id.clone().map(|id| (id, pattern.clone())))
        .collect();

    for index in 0..model.patterns.len() {
        let Some(target) = model.patterns[index].is_a.clone() else {
            continue;
        };
        let base = abstracts.get(&target).ok_or_else(|| {
            Error::schema(
                "pattern",
                model.patterns[index].id.clone(),
                format!("no abstract pattern with @id={target:?} to instantiate"),
            )
        })?;

        let parameters: HashMap<String, String> = model.patterns[index]
            .params
            .iter()
            .map(|param| (param.name.clone(), param.value.clone()))
            .collect();

        let instance = &mut model.patterns[index];
        instance.rules = base
            .rules
            .iter()
            .cloned()
            .map(|rule| substitute_rule(rule, &parameters))
            .collect();
        instance.lets = base
            .lets
            .iter()
            .cloned()
            .map(|binding| substitute_let(binding, &parameters))
            .collect();
        if instance.title.is_none() {
            instance.title.clone_from(&base.title);
        }
        if instance.documents.is_none() {
            instance.documents = base
                .documents
                .as_ref()
                .map(|d| substitute(d, &parameters));
        }
    }

    // Abstract patterns are templates; they never run.
    model.patterns.retain(|pattern| !pattern.is_abstract);
    Ok(())
}

fn substitute_rule(mut rule: Rule, parameters: &HashMap<String, String>) -> Rule {
    rule.context = rule.context.map(|c| substitute(&c, parameters));
    rule.subject = rule.subject.map(|s| substitute(&s, parameters));
    rule.lets = rule
        .lets
        .into_iter()
        .map(|binding| substitute_let(binding, parameters))
        .collect();
    rule.body = rule
        .body
        .into_iter()
        .map(|child| match child {
            RuleChild::Assertion(assertion) => {
                RuleChild::Assertion(substitute_assertion(assertion, parameters))
            }
            RuleChild::Extends(target) => RuleChild::Extends(target),
        })
        .collect();
    rule
}

fn substitute_let(mut binding: Let, parameters: &HashMap<String, String>) -> Let {
    binding.value = match binding.value {
        LetValue::Expression(expression) => {
            LetValue::Expression(substitute(&expression, parameters))
        }
        LetValue::Content(content) => LetValue::Content(substitute_content(content, parameters)),
    };
    binding
}

fn substitute_assertion(
    mut assertion: Assertion,
    parameters: &HashMap<String, String>,
) -> Assertion {
    assertion.test = substitute(&assertion.test, parameters);
    assertion.subject = assertion.subject.map(|s| substitute(&s, parameters));
    assertion.content = substitute_content(assertion.content, parameters);
    assertion
}

fn substitute_content(
    content: Vec<Content>,
    parameters: &HashMap<String, String>,
) -> Vec<Content> {
    content
        .into_iter()
        .map(|fragment| match fragment {
            Content::ValueOf { select } => Content::ValueOf {
                select: substitute(&select, parameters),
            },
            Content::Name { path } => Content::Name {
                path: path.map(|p| substitute(&p, parameters)),
            },
            Content::Emph(inner) => Content::Emph(substitute_content(inner, parameters)),
            Content::Span { class, content } => Content::Span {
                class,
                content: substitute_content(content, parameters),
            },
            Content::Dir { value, content } => Content::Dir {
                value,
                content: substitute_content(content, parameters),
            },
            // Literal text is not substituted: the standard substitutes in
            // expressions, and a `$` in prose should stay a `$`.
            Content::Text(text) => Content::Text(text),
        })
        .collect()
}

/// Replaces `$name` with its parameter value, textually.
///
/// Deliberately narrow, matching the reference implementation: a `$` followed
/// by a name that is a *declared* parameter. A `$` followed by anything else
/// is left alone, so it can still resolve as a `let` variable. Substitution
/// is single-pass, so a value containing `$x` does not expand again.
fn substitute(expression: &str, parameters: &HashMap<String, String>) -> String {
    if parameters.is_empty() || !expression.contains('$') {
        return expression.to_string();
    }

    let bytes = expression.as_bytes();
    let mut out = String::with_capacity(expression.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'$' {
            // Multi-byte characters copy whole, because slicing is by byte.
            let start = index;
            index += 1;
            while index < bytes.len() && !expression.is_char_boundary(index) {
                index += 1;
            }
            out.push_str(&expression[start..index]);
            continue;
        }

        let name_start = index + 1;
        let mut name_end = name_start;
        while name_end < bytes.len() && is_name_byte(bytes[name_end], name_end == name_start) {
            name_end += 1;
        }
        let name = &expression[name_start..name_end];
        if let Some(value) = parameters.get(name) {
            out.push_str(value);
            index = name_end;
        } else {
            out.push('$');
            index += 1;
        }
    }
    out
}

fn is_name_byte(byte: u8, first: bool) -> bool {
    let start = byte.is_ascii_alphabetic() || byte == b'_' || byte >= 0x80;
    if first {
        start
    } else {
        start || byte.is_ascii_digit() || byte == b'-' || byte == b'.'
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::model::AssertionKind;
    use crate::schema::parse::parse_schema;
    use crate::xml::Document;

    fn expanded(source: &str) -> SchemaModel {
        let document = Document::from_str(source).unwrap();
        let mut model = parse_schema(&document).unwrap();
        expand(&mut model).unwrap();
        model
    }

    fn expansion_error(source: &str) -> String {
        let document = Document::from_str(source).unwrap();
        let mut model = parse_schema(&document).unwrap();
        expand(&mut model).unwrap_err().to_string()
    }

    fn parameters(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn substitutes_declared_parameters_only() {
        let p = parameters(&[("parent", "invoice")]);
        assert_eq!(substitute("$parent/total", &p), "invoice/total");
        assert_eq!(substitute("$other/total", &p), "$other/total");
    }

    #[test]
    fn substitution_is_single_pass() {
        let p = parameters(&[("a", "$b"), ("b", "final")]);
        assert_eq!(substitute("$a", &p), "$b");
    }

    #[test]
    fn substitution_reaches_inside_string_literals() {
        // Textual substitution is what the standard specifies, and schemas
        // rely on it to build messages.
        let p = parameters(&[("x", "total")]);
        assert_eq!(substitute("concat('$x')", &p), "concat('total')");
    }

    #[test]
    fn substitution_stops_at_the_end_of_a_name() {
        let p = parameters(&[("a", "X")]);
        assert_eq!(substitute("$a/b", &p), "X/b");
        assert_eq!(substitute("$a", &p), "X");
        assert_eq!(substitute("count($a)", &p), "count(X)");
    }

    #[test]
    fn abstract_patterns_are_instantiated_and_removed() {
        let model = expanded(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern abstract="true" id="required">
                   <rule context="$parent">
                     <assert test="$child">missing</assert>
                   </rule>
                 </pattern>
                 <pattern is-a="required" id="invoice-total">
                   <param name="parent" value="invoice"/>
                   <param name="child" value="total"/>
                 </pattern>
               </schema>"#,
        );
        assert_eq!(model.patterns.len(), 1);
        let rule = &model.patterns[0].rules[0];
        assert_eq!(rule.context.as_deref(), Some("invoice"));
        assert_eq!(rule.assertions().next().unwrap().test, "total");
    }

    #[test]
    fn one_abstract_pattern_serves_several_instances() {
        let model = expanded(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern abstract="true" id="required">
                   <rule context="$parent"><assert test="$child">missing</assert></rule>
                 </pattern>
                 <pattern is-a="required" id="one">
                   <param name="parent" value="invoice"/><param name="child" value="total"/>
                 </pattern>
                 <pattern is-a="required" id="two">
                   <param name="parent" value="order"/><param name="child" value="date"/>
                 </pattern>
               </schema>"#,
        );
        assert_eq!(model.patterns.len(), 2);
        assert_eq!(model.patterns[0].rules[0].context.as_deref(), Some("invoice"));
        assert_eq!(model.patterns[1].rules[0].context.as_deref(), Some("order"));
    }

    #[test]
    fn missing_abstract_pattern_is_an_error() {
        let message = expansion_error(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern is-a="nope" id="p"><param name="a" value="b"/></pattern>
               </schema>"#,
        );
        assert!(message.contains("no abstract pattern"), "{message}");
    }

    #[test]
    fn abstract_rules_splice_in_at_their_position() {
        let model = expanded(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern>
                   <rule abstract="true" id="base">
                     <assert test="mid">middle</assert>
                   </rule>
                   <rule context="a">
                     <assert test="one">first</assert>
                     <extends rule="base"/>
                     <assert test="two">last</assert>
                   </rule>
                 </pattern>
               </schema>"#,
        );
        assert_eq!(model.patterns[0].rules.len(), 1);
        let tests: Vec<&str> = model.patterns[0].rules[0]
            .assertions()
            .map(|a| a.test.as_str())
            .collect();
        assert_eq!(tests, vec!["one", "mid", "two"]);
    }

    #[test]
    fn abstract_rules_extend_transitively() {
        let model = expanded(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern>
                   <rule abstract="true" id="inner"><assert test="i">i</assert></rule>
                   <rule abstract="true" id="outer">
                     <extends rule="inner"/>
                     <assert test="o">o</assert>
                   </rule>
                   <rule context="a"><extends rule="outer"/></rule>
                 </pattern>
               </schema>"#,
        );
        let tests: Vec<&str> = model.patterns[0].rules[0]
            .assertions()
            .map(|a| a.test.as_str())
            .collect();
        assert_eq!(tests, vec!["i", "o"]);
    }

    #[test]
    fn extends_cycles_are_reported() {
        let message = expansion_error(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern>
                   <rule abstract="true" id="a"><extends rule="b"/></rule>
                   <rule abstract="true" id="b"><extends rule="a"/></rule>
                   <rule context="x"><extends rule="a"/></rule>
                 </pattern>
               </schema>"#,
        );
        assert!(message.contains("cycle"), "{message}");
    }

    #[test]
    fn missing_extends_target_is_an_error() {
        let message = expansion_error(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern><rule context="x"><extends rule="nope"/></rule></pattern>
               </schema>"#,
        );
        assert!(message.contains("no abstract rule"), "{message}");
    }

    #[test]
    fn abstract_pattern_parameters_reach_assertion_content() {
        let model = expanded(
            r#"<schema xmlns="http://purl.oclc.org/dsdl/schematron">
                 <pattern abstract="true" id="t">
                   <rule context="$parent">
                     <assert test="$child">see <value-of select="$child"/></assert>
                   </rule>
                 </pattern>
                 <pattern is-a="t" id="i">
                   <param name="parent" value="a"/><param name="child" value="b"/>
                 </pattern>
               </schema>"#,
        );
        let assertion = model.patterns[0].rules[0].assertions().next().unwrap();
        assert_eq!(assertion.kind, AssertionKind::Assert);
        assert!(matches!(
            &assertion.content[1],
            Content::ValueOf { select } if select == "b"
        ));
    }
}
