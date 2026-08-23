//! Differential testing against the ISO Schematron reference implementation.
//!
//! The crate's largest unproven claim is that it agrees with other Schematron
//! processors. This test settles it for the one that matters most: the
//! reference implementation, a set of XSLT stylesheets that compile a schema
//! into a validator and run it.
//!
//! It is `#[ignore]`d, because it needs two things this repository does not
//! carry: `xsltproc`, and the reference stylesheets themselves. Those are
//! third-party, carry their own licence and release cadence, and a vendored
//! copy here would rot. Fetch them and point the environment at them:
//!
//! ```sh
//! sh tests/differential/fetch-skeleton.sh /tmp/skeleton
//! SCHEMATRON_SKELETON=/tmp/skeleton cargo test --test differential -- --ignored
//! ```
//!
//! # What is compared, and what is not
//!
//! The **findings**: for each, whether it is a failed assertion or a
//! successful report, the test that produced it, the message, the `@flag` and
//! `@role` it carries, and the messages of its diagnostics — in order. That
//! is the substance of a Schematron report.
//!
//! And the **rule firings**, which findings alone cannot show: a rule whose
//! assertions all hold reports nothing, so without this the dispatch that
//! Schematron is built on goes unchecked wherever a schema passes.
//!
//! The `@location` attribute is compared, but by resolving it rather than by
//! string: the two write equally valid XPath in different shapes, and SVRL
//! prescribes neither. Two paths name the same node exactly when each selects
//! one node and their union still selects one, which is a question XPath 1.0
//! can answer about both syntaxes at once.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One finding, reduced to what both implementations should agree on.
#[derive(Debug, PartialEq, Eq, Clone)]
struct Finding {
    kind: String,
    test: String,
    text: String,
    /// The assertion's own `@flag` and `@role`, and the messages of the
    /// diagnostics it references. Comparing only kind, test and message left
    /// every one of these unchecked against the reference, though both
    /// implementations emit them.
    flag: Option<String>,
    role: Option<String>,
    diagnostics: Vec<String>,
}

/// Corpus cases where the two implementations legitimately differ.
///
/// Recorded individually so that the rest of the corpus stays a strict
/// comparison. A case listed here must have its reason written down, in
/// `spec/conformance.md` as well as here.
const KNOWN_DIVERGENCES: &[(&str, &str)] = &[
    (
        "message-inline-whitespace",
        "A whitespace-only text node between two inline elements in a message \
         — `<name/> <emph>e</emph>` — survives here and is lost by the \
         reference. Its generated validator is itself an XSLT stylesheet, and \
         XSLT 1.0 strips whitespace-only text nodes from a stylesheet, so the \
         space cannot survive the way it compiles a schema. Text with any \
         non-whitespace content is preserved by both. See \
         spec/conformance.md.",
    ),
    (
        "subject",
        "`@subject` moves the reported location to the node the assertion is \
         about; the reference reports the context node instead. Its own source \
         settles this one: the `linkableParms` template carries the comment \
         \"ISO SVRL does not have a subject attribute ... Instead, the \
         Schematron subject attribute is folded into the location attribute\", \
         and then never uses the `$subject` parameter it declares. The \
         reference documents this crate's behaviour and does not implement \
         it. See spec/conformance.md.",
    ),
    (
        "rich-metadata",
        "An assertion with no `@flag` or `@role` of its own inherits the \
         rule's here; the reference leaves both off. ISO does not state \
         inheritance either way, but its grammar does allow `rule/@flag`, and \
         under the reference that attribute has no observable effect at all — \
         it reaches neither the finding nor `fired-rule`. Making a permitted \
         attribute inert is the less defensible reading, and flags exist to \
         classify findings for filtering. See spec/conformance.md.",
    ),
    (
        "namespaced-attribute-context",
        "A rule on `@x` claims `@p:x` as well in the reference: libxslt's \
         template matcher ignores the namespace on an unprefixed attribute \
         name test. libxml2's own XPath contradicts it — `count(//@x)` is 1 \
         where `match=\"@x\"` matches both — and Java's XPath agrees with \
         libxml2, so this is a defect in the matcher rather than a reading of \
         the standard. Elements are unaffected, and `@p:x` correctly matches \
         only itself. See spec/conformance.md.",
    ),
    (
        "following-axis-from-attribute",
        "`@x/following::a` — the reference gives the element's following \
         nodes, excluding its children; this crate includes them. XPath 1.0 \
         orders an element's attributes before its children and excludes only \
         the *context node's* descendants, and an attribute has none, so the \
         children follow the attribute. Java's XPath engine agrees with this \
         crate, so the reference is the outlier rather than the arbiter. See \
         spec/conformance.md.",
    ),
    (
    "node-kinds",
    "The reference walks the tree with `select=\"@*|*\"`, so it visits only \
     elements and attributes and its generated text(), comment() and \
     processing-instruction() templates can never fire. This crate visits all \
     seven node kinds, so a rule with context=\"comment()\" works. See \
     spec/conformance.md.",
    ),
];

/// Corpus cases the reference cannot run at all.
const REFERENCE_CANNOT_RUN: &[(&str, &str)] = &[
    (
        "let-shadowing",
        "The reference compiles each <let> to an xsl:variable in one scope, so \
         a schema that shadows a name is an XSLT error. This crate implements \
         the four nested scopes the standard describes.",
    ),
    (
        "let-phase-scope",
        "Same cause: a phase-level <let> shadowing a schema-level one of the \
         same name is an XSLT variable redeclaration.",
    ),
];

fn skeleton() -> PathBuf {
    let path = std::env::var("SCHEMATRON_SKELETON").unwrap_or_else(|_| {
        panic!(
            "set SCHEMATRON_SKELETON to a directory holding the reference \
             stylesheets; see tests/differential/fetch-skeleton.sh"
        )
    });
    let path = PathBuf::from(path);
    assert!(
        path.join("iso_svrl_for_xslt1.xsl").exists(),
        "{} does not hold the reference stylesheets; run \
         tests/differential/fetch-skeleton.sh",
        path.display()
    );
    path
}

fn binary() -> PathBuf {
    let mut path = std::env::current_exe().expect("test executable path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join(format!("schematron{}", std::env::consts::EXE_SUFFIX))
}

/// Runs one stage of the reference pipeline.
fn xsltproc(stylesheet: &Path, input: &Path, params: &[(&str, &str)]) -> Result<String, String> {
    let mut command = Command::new("xsltproc");
    for (name, value) in params {
        command.arg("--stringparam").arg(name).arg(value);
    }
    let output = command
        .arg(stylesheet)
        .arg(input)
        .output()
        .map_err(|e| format!("xsltproc could not be run: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.lines().next().unwrap_or("xsltproc failed").to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Compiles a schema with the reference implementation and runs it.
fn reference_svrl(
    skeleton: &Path,
    work: &Path,
    schema: &Path,
    document: &Path,
    phase: Option<&str>,
) -> Result<String, String> {
    let write = |name: &str, body: &str| -> Result<PathBuf, String> {
        let path = work.join(name);
        std::fs::write(&path, body).map_err(|e| format!("cannot write {name}: {e}"))?;
        Ok(path)
    };

    // Three transforms turn a schema into a validator: resolve includes,
    // expand abstract patterns, then generate the SVRL-producing stylesheet.
    let included = xsltproc(&skeleton.join("iso_dsdl_include.xsl"), schema, &[])?;
    let included = write("included.sch", &included)?;

    let expanded = xsltproc(&skeleton.join("iso_abstract_expand.xsl"), &included, &[])?;
    let expanded = write("expanded.sch", &expanded)?;

    let params: Vec<(&str, &str)> = phase.map(|p| vec![("phase", p)]).unwrap_or_default();
    let validator = xsltproc(&skeleton.join("iso_svrl_for_xslt1.xsl"), &expanded, &params)?;
    let validator = write("validator.xsl", &validator)?;

    xsltproc(&validator, document, &[])
}

/// Collapses whitespace, which differs freely between the two and carries no
/// meaning in a message.
fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Every rule firing, in order, as `pattern/rule@context`.
///
/// A rule that fires but whose assertions all hold produces no finding at
/// all, so [`findings`] cannot see it. That is a large blind spot: the
/// first-matching-rule-wins dispatch is the central Schematron semantic, and
/// a schema whose tests happen to pass exercises it just as hard as one whose
/// tests fail. SVRL records each firing as `svrl:fired-rule`, so the two
/// implementations' dispatch can be compared directly.
///
/// SVRL has nowhere to record *which node* a rule fired on, so this compares
/// the sequence — how many times each rule fired, in what order, under which
/// pattern — rather than the nodes themselves.
fn fired_rules(svrl: &str) -> Result<Vec<String>, String> {
    let report = schematron::Report::from_svrl(svrl)
        .map_err(|e| format!("SVRL did not parse: {e}"))?;
    Ok(report
        .patterns
        .iter()
        .flat_map(|pattern| {
            let label = pattern.id.clone().unwrap_or_default();
            pattern.rules.iter().map(move |rule| {
                format!(
                    "{label}/{}@{}",
                    rule.id.clone().unwrap_or_default(),
                    rule.context
                )
            })
        })
        .collect())
}

/// The `@location` of each finding, in the same order as [`findings`].
fn locations(svrl: &str) -> Result<Vec<String>, String> {
    let report = schematron::Report::from_svrl(svrl)
        .map_err(|e| format!("SVRL did not parse: {e}"))?;
    Ok(report
        .assertions()
        .map(|assertion| assertion.location.clone())
        .collect())
}

/// Checks that each pair of locations identifies the same single node.
///
/// The two implementations write locations in different but equally valid
/// syntax, so comparing the strings is meaningless. Resolving them is not:
/// two paths pick the same one node exactly when each selects one node and
/// their union still selects one. That is pure XPath 1.0, so the crate's own
/// engine can answer it for both sides' syntax without either being favoured.
///
/// **This crate's location must always resolve to exactly one node** — that
/// is a hard failure, because a location that does not identify the node is
/// the one thing a location must not be. The reference's frequently does not
/// (see `spec/conformance.md`), and those pairs are counted and skipped
/// rather than failed, so its defects do not drown out a real disagreement.
///
/// Returns the number of pairs skipped for that reason.
fn compare_locations(
    document_text: &str,
    theirs: &[String],
    mine: &[String],
) -> Result<usize, String> {
    let escape = |v: &str| v.replace('&', "&amp;").replace('<', "&lt;");
    let mut probes = String::new();
    let mut pairs: Vec<usize> = Vec::new();

    for (index, (a, b)) in theirs.iter().zip(mine).enumerate() {
        if a.is_empty() || b.is_empty() {
            continue;
        }
        let (a, b) = (escape(a), escape(b));
        probes.push_str(&format!(
            "      <report test=\"count({a}) = 1\">T{index}</report>\n\
             \x20     <report test=\"count({b}) = 1\">M{index}</report>\n\
             \x20     <report test=\"count(({a}) | ({b})) = 1\">S{index}</report>\n"
        ));
        pairs.push(index);
    }
    if pairs.is_empty() {
        return Ok(0);
    }

    let schema = format!(
        "<schema xmlns=\"http://purl.oclc.org/dsdl/schematron\">\n  <pattern>\n    <rule context=\"/\">\n{probes}    </rule>\n  </pattern>\n</schema>\n"
    );
    let compiled = schematron::Schema::from_str(&schema)
        .map_err(|e| format!("the location probe schema did not compile: {e}"))?;
    let document = schematron::Document::from_str(document_text)
        .map_err(|e| format!("the document did not parse: {e}"))?;
    let report = compiled
        .validate(&document)
        .map_err(|e| format!("the location probe did not run: {e}"))?;

    let fired: Vec<String> = report
        .assertions()
        .map(|assertion| assertion.text.trim().to_string())
        .collect();
    let held = |label: String| fired.contains(&label);

    let mut unresolvable = 0;
    for index in pairs {
        if !held(format!("M{index}")) {
            return Err(format!(
                "this crate's location {:?} does not resolve to exactly one node",
                mine[index]
            ));
        }
        // The reference's position counter ignores namespaces while the
        // predicate it emits does not, so any location naming a namespaced
        // element may point at the wrong node or at none — see
        // `spec/conformance.md`. Those pairs are counted, not compared;
        // everything else stays strict, and this crate's side is checked
        // above either way.
        if !held(format!("T{index}")) || theirs[index].contains("local-name()") {
            unresolvable += 1;
            continue;
        }
        if !held(format!("S{index}")) {
            return Err(format!(
                "location {index} differs: the reference says {:?}, this crate says {:?}; \
                 both resolve, but to different nodes",
                theirs[index], mine[index]
            ));
        }
    }
    Ok(unresolvable)
}

/// Extracts the findings from an SVRL document, in order.
///
/// Parsed with the crate's own reader, which is what makes this test possible
/// at all — and which the corpus round-trip test independently checks.
fn findings(svrl: &str) -> Result<Vec<Finding>, String> {
    let report = schematron::Report::from_svrl(svrl)
        .map_err(|e| format!("SVRL did not parse: {e}"))?;

    Ok(report
        .assertions()
        .map(|assertion| Finding {
            kind: assertion.kind.as_str().to_string(),
            test: assertion.test.clone(),
            // Whitespace differs freely between the two, and carries no
            // meaning in a message.
            text: normalize(&assertion.text),
            flag: assertion.flag.clone(),
            role: assertion.role.clone(),
            diagnostics: assertion
                .diagnostics
                .iter()
                .map(|diagnostic| format!("{}: {}", diagnostic.id, normalize(&diagnostic.text)))
                .collect(),
        })
        .collect())
}

/// Runs this crate over a case and returns its findings.
fn our_svrl(schema: &Path, document: &Path, phase: Option<&str>) -> Result<String, String> {
    let mut command = Command::new(binary());
    command
        .arg("--schema")
        .arg(schema)
        .arg("--format")
        .arg("svrl");
    if let Some(phase) = phase {
        command.arg("--phase").arg(phase);
    }
    let output = command
        .arg(document)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("could not run the binary: {e}"))?;

    // Exit 1 means findings, which is not an error here.
    let code = output.status.code().unwrap_or(-1);
    if code != 0 && code != 1 {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// What comparing one case produced.
struct Comparison {
    /// The first difference found, if any: findings, then dispatch, then
    /// locations. `None` means the two implementations agree.
    difference: Option<String>,
    findings: usize,
    firings: usize,
    /// Locations skipped because the reference's could not be trusted.
    unresolvable: usize,
}

/// Compares one case's two SVRL documents on every axis this test checks.
///
/// Shared by the curated corpus and the generated cases so that the two
/// cannot drift into checking different things — which, when one of them is
/// the only cover for a behaviour, is how a gap opens without anyone noticing.
fn compare_case(reference: &str, ours: &str, document_text: &str) -> Comparison {
    let mut result = Comparison {
        difference: None,
        findings: 0,
        firings: 0,
        unresolvable: 0,
    };

    let (theirs, mine) = match (findings(reference), findings(ours)) {
        (Ok(a), Ok(b)) => (a, b),
        (a, b) => {
            result.difference = Some(format!("SVRL did not parse: {:?} {:?}", a.err(), b.err()));
            return result;
        }
    };
    result.findings = theirs.len();
    if theirs != mine {
        result.difference = Some(describe_difference(&theirs, &mine));
        return result;
    }

    match (fired_rules(reference), fired_rules(ours)) {
        (Ok(a), Ok(b)) => {
            result.firings = a.len();
            if a != b {
                let at = (0..a.len().max(b.len())).find(|&i| a.get(i) != b.get(i));
                result.difference = Some(format!(
                    "rule dispatch differs: the reference fired {} rules, this crate {}; \
                     at index {at:?} the reference has {:?} and this crate {:?}",
                    a.len(),
                    b.len(),
                    at.and_then(|i| a.get(i).cloned()),
                    at.and_then(|i| b.get(i).cloned()),
                ));
                return result;
            }
        }
        (Err(why), _) | (_, Err(why)) => {
            result.difference = Some(why);
            return result;
        }
    }

    match (locations(reference), locations(ours)) {
        (Ok(a), Ok(b)) => match compare_locations(document_text, &a, &b) {
            Ok(skipped) => result.unresolvable = skipped,
            Err(why) => result.difference = Some(why),
        },
        _ => result.difference = Some("locations did not parse".to_string()),
    }
    result
}

#[test]
#[ignore = "needs xsltproc and the reference stylesheets; see the module docs"]
fn we_agree_with_the_reference_implementation() {
    let skeleton = skeleton();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let work = std::env::temp_dir().join("schematron-differential");
    std::fs::create_dir_all(&work).expect("work directory");

    let known: BTreeMap<&str, &str> = KNOWN_DIVERGENCES.iter().copied().collect();
    let cannot_run: BTreeMap<&str, &str> = REFERENCE_CANNOT_RUN.iter().copied().collect();

    let mut cases: Vec<PathBuf> = std::fs::read_dir(root.join("tests/corpus"))
        .expect("tests/corpus should exist")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    cases.sort();

    let (mut agreed, mut diverged, mut unrunnable) = (0, 0, 0);
    let mut unresolvable = 0usize;
    let mut unexpected = Vec::new();
    let mut stale = Vec::new();

    for case in &cases {
        let name = case.file_name().unwrap().to_string_lossy().to_string();
        let schema = case.join("schema.sch");
        let document = case.join("input.xml");
        let phase = std::fs::read_to_string(case.join("phase")).ok();
        let phase = phase.as_deref().map(str::trim);

        let reference = match reference_svrl(&skeleton, &work, &schema, &document, phase) {
            Ok(svrl) => svrl,
            Err(why) => {
                if cannot_run.contains_key(name.as_str()) {
                    unrunnable += 1;
                } else {
                    unexpected.push(format!("{name}: the reference could not run it: {why}"));
                }
                continue;
            }
        };

        if cannot_run.contains_key(name.as_str()) {
            stale.push(format!("{name}: listed as unrunnable, but the reference ran it"));
        }

        let ours = match our_svrl(&schema, &document, phase) {
            Ok(svrl) => svrl,
            Err(why) => {
                unexpected.push(format!("{name}: this crate could not run it: {why}"));
                continue;
            }
        };

        let document_text = std::fs::read_to_string(&document).unwrap_or_default();
        let comparison = compare_case(&reference, &ours, &document_text);
        unresolvable += comparison.unresolvable;
        let difference = comparison.difference.map(|why| format!("{name}: {why}"));

        match (difference, known.contains_key(name.as_str())) {
            (None, false) => agreed += 1,
            (None, true) => stale.push(format!(
                "{name}: listed as a known divergence, but the two now agree"
            )),
            (Some(_), true) => diverged += 1,
            (Some(why), false) => unexpected.push(why),
        }
    }

    println!(
        "agreed: {agreed}, known divergences: {diverged}, \
         cases the reference cannot run: {unrunnable}, \
         locations the reference could not resolve: {unresolvable}"
    );

    // A known divergence that has gone away is as much a defect in this test
    // as an unexpected one: the list must describe reality.
    assert!(
        stale.is_empty(),
        "the divergence list is out of date:\n  {}",
        stale.join("\n  ")
    );
    assert!(
        unexpected.is_empty(),
        "{} unexpected difference(s) from the reference implementation:\n\n{}",
        unexpected.len(),
        unexpected.join("\n\n")
    );
    assert!(agreed > 0, "no case was actually compared");
}

// ---------------------------------------------------------------------------
// Generated cases
// ---------------------------------------------------------------------------
//
// The curated corpus above covers the constructs deliberately, one case per
// idea. It cannot cover their *combinations*, and XPath 1.0's conversion
// rules are where a subtle disagreement would hide: a node-set compared to a
// number, an empty node-set coerced to a boolean, a string that is not a
// number fed to a relational operator.
//
// So: generate schema and document pairs from a grammar of constructs both
// implementations support, run both, and require identical findings. A
// failure prints the seed, and the seed alone reproduces the case.
//
// The grammar deliberately excludes constructs already known to diverge or to
// be underspecified between the two, because a generator that rediscovers a
// documented divergence on every third case reports nothing:
//
// - `comment()`, `text()` and `processing-instruction()` contexts, per the
//   `node-kinds` divergence above.
// - `position()` and `last()`, which depend on the node list the reference's
//   `apply-templates` happens to build, not on the schema.
// - `let` shadowing, per the two `let-` divergences above.
// - `id()`, which needs a DTD to say which attributes are of type ID.

/// A deterministic PRNG — xorshift64*, chosen so a seed reproduces a case
/// exactly, on any platform and any run.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // Zero is a fixed point of xorshift, so move off it.
        Self(seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1))
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        // The modulus keeps the result below `n`, which is a `usize` already,
        // so the narrowing cannot lose anything.
        #[allow(clippy::cast_possible_truncation)]
        {
            (self.next() % n as u64) as usize
        }
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }

    /// True with probability `numerator / denominator`.
    fn chance(&mut self, numerator: usize, denominator: usize) -> bool {
        self.below(denominator) < numerator
    }
}

/// The namespaces a generated schema declares and a generated document uses.
///
/// Namespaces are where Schematron schemas go wrong most often in practice —
/// XPath 1.0 has no default namespace, so an unprefixed name matches nothing
/// in a namespaced document — which makes them worth generating rather than
/// only hand-testing.
const NAMESPACES: &[(&str, &str)] = &[
    ("p", "http://example.com/p"),
    ("q", "http://example.com/q"),
];

/// Element names, written the same way in a generated document and in the
/// XPath that addresses it. That works because the document declares these
/// prefixes and the schema declares matching `ns` elements, which is exactly
/// the correspondence a real schema has to get right.
const ELEMENTS: &[&str] = &["a", "b", "c", "p:a", "p:b", "q:a"];
// No namespaced attribute here on purpose. A document holding both `x` and
// `p:x` makes every unprefixed attribute test diverge, for the reference-side
// defect recorded in `KNOWN_DIVERGENCES`, which would bury anything new under
// the same finding case after case. The behaviour is pinned by name instead,
// in `tests/corpus/namespaced-attribute-context/`.
const ATTRIBUTES: &[&str] = &["x", "y"];
/// Values chosen to exercise conversion: integers, a decimal, a non-number,
/// and the empty string.
const VALUES: &[&str] = &["1", "2", "3.5", "foo", "", "10"];

/// Character data as it is *written* in a generated document.
///
/// Several of these resolve to a string in `VALUES`, so that a comparison
/// against `'foo'` is a real test of entity and CDATA handling rather than
/// only of string length: a parser that failed to resolve `&#102;oo` would
/// still report a plausible length.
const TEXT_PIECES: &[&str] = &[
    "1",
    "2",
    "3.5",
    "foo",
    "10",
    "&#102;oo",           // decimal character reference for "foo"
    "&#x66;oo",           // the same, in hex
    "f&#111;o",           // a reference in the middle
    "<![CDATA[foo]]>",    // CDATA that resolves to the same string again
    "<![CDATA[1]]>",
    "&amp;",
    "&lt;",
    "&gt;",
    "a&#32;b",            // an escaped space
    " ",
    "  ",
    "\n  ",
];

/// Builds a small document: element names and attribute values are drawn from
/// tiny alphabets so that rules actually match rather than missing silently.
fn generate_document(rng: &mut Rng) -> String {
    fn element(rng: &mut Rng, depth: usize, out: &mut String) {
        element_named(rng, None, depth, out);
    }

    fn element_named(rng: &mut Rng, forced: Option<&str>, depth: usize, out: &mut String) {
        let name = forced.unwrap_or_else(|| rng.pick(ELEMENTS));
        out.push('<');
        out.push_str(name);
        for attribute in ATTRIBUTES {
            if rng.chance(1, 2) {
                out.push_str(&format!(" {attribute}=\"{}\"", rng.pick(VALUES)));
            }
        }
        out.push('>');
        // Comments, processing instructions and stray text, interleaved with
        // the children. They change string-values, `node()` counts and text
        // node boundaries, none of which a document of bare elements reaches.
        let decoration = |rng: &mut Rng, out: &mut String| {
            match rng.below(12) {
                0 => out.push_str("<!-- c -->"),
                // The empty comment is `<!---->`. `<!--->` is not a comment
                // at all — XML forbids `--` inside one — and both this crate
                // and libxml2 reject it; see the parser tests.
                1 => out.push_str("<!---->"),
                2 => out.push_str("<?pi data?>"),
                3 => out.push_str("<?pi?>"),
                _ => {}
            }
        };
        decoration(rng, out);
        if depth > 0 && rng.chance(1, 2) {
            // Half the time every child shares one name, so that `b` is a
            // node-set of several nodes rather than nearly always one. The
            // existential comparison rules only differ from first-node rules
            // when there is more than one node, so without this the generator
            // cannot see the difference.
            let repeated = rng.chance(1, 2).then(|| rng.pick(ELEMENTS));
            for _ in 0..=rng.below(3) {
                // Mixed content: text either side of a child element, which
                // is what splits an element's text into several text nodes.
                if rng.chance(1, 4) {
                    out.push_str(rng.pick(TEXT_PIECES));
                }
                element_named(rng, repeated, depth - 1, out);
                decoration(rng, out);
            }
            if rng.chance(1, 4) {
                out.push_str(rng.pick(TEXT_PIECES));
            }
        } else if rng.chance(2, 3) {
            out.push_str(rng.pick(TEXT_PIECES));
        }
        out.push_str(&format!("</{name}>"));
    }

    let mut out = String::from("<root");
    for (prefix, uri) in NAMESPACES {
        out.push_str(&format!(" xmlns:{prefix}=\"{uri}\""));
    }
    out.push('>');
    for _ in 0..=rng.below(4) {
        element(rng, 2, &mut out);
    }
    out.push_str("</root>");
    out
}

/// The axes worth generating.
///
/// `namespace::` is left out on purpose. Namespace nodes are the corner of
/// the data model where XSLT 1.0 processors differ most, and a disagreement
/// there would say more about `xsltproc` than about this crate.
const AXES: &[&str] = &[
    "child::",
    "descendant::",
    "parent::",
    "ancestor::",
    "following-sibling::",
    "preceding-sibling::",
    "following::",
    "preceding::",
    "self::",
    "descendant-or-self::",
    "ancestor-or-self::",
    "attribute::",
];

/// A predicate.
///
/// `position()` and `last()` appear here but never on their own in a test.
/// Inside a predicate they are defined by the step's own node list, which
/// both implementations build the same way. Bare in a rule's test they depend
/// on the node list the reference's `apply-templates` happens to construct,
/// which is not something a schema controls.
fn generate_predicate(rng: &mut Rng) -> String {
    match rng.below(7) {
        0 => "1".to_string(),
        1 => "last()".to_string(),
        2 => "position() = 1".to_string(),
        3 => format!("position() > {}", rng.below(3)),
        4 => "position() = last()".to_string(),
        5 => format!("@{}", rng.pick(ATTRIBUTES)),
        _ => format!("@{} = '{}'", rng.pick(ATTRIBUTES), rng.pick(VALUES)),
    }
}

/// A location path, with an axis and optionally a predicate on each step.
fn generate_path(rng: &mut Rng) -> String {
    let mut path = String::new();
    let mut after_attribute = false;
    match rng.below(6) {
        0 => path.push('/'),
        1 => path.push_str("//"),
        _ => {}
    }
    for step in 0..=rng.below(2) {
        if step > 0 {
            path.push_str(if rng.chance(1, 5) { "//" } else { "/" });
        }
        // `following::` taken from an attribute node is a documented
        // divergence — see `KNOWN_DIVERGENCES` — and the corpus pins it by
        // name. Generating it here would rediscover it in case after case
        // and bury anything new, so the step after an attribute avoids it.
        let axis = loop {
            let candidate = rng.pick(AXES);
            if !(after_attribute && candidate == "following::") {
                break candidate;
            }
        };
        after_attribute = axis == "attribute::";
        // The attribute axis has attributes as its principal node type, so a
        // name test on it names an attribute, not an element.
        let name = if axis == "attribute::" {
            rng.pick(ATTRIBUTES)
        } else if rng.chance(1, 5) {
            rng.pick(&[
                "*",
                "p:*",
                "q:*",
                "node()",
                "text()",
                "comment()",
                "processing-instruction()",
            ])
        } else {
            rng.pick(ELEMENTS)
        };
        path.push_str(axis);
        path.push_str(name);
        if rng.chance(1, 3) {
            path.push_str(&format!("[{}]", generate_predicate(rng)));
        }
    }
    path
}

/// A numeric expression, so that arithmetic and its precedence are exercised
/// rather than only comparison.
fn generate_number(rng: &mut Rng, depth: usize) -> String {
    if depth > 0 && rng.chance(1, 2) {
        let operator = rng.pick(&["+", "-", "*", "div", "mod"]);
        return format!(
            "({} {operator} {})",
            generate_number(rng, depth - 1),
            generate_number(rng, depth - 1)
        );
    }
    match rng.below(10) {
        0 => rng.pick(&["0", "1", "2", "3.5", "-1", "10"]).to_string(),
        1 => format!("count({})", generate_path(rng)),
        2 => format!("sum({})", generate_path(rng)),
        3 => "string-length(.)".to_string(),
        4 => format!("number(@{})", rng.pick(ATTRIBUTES)),
        9 => format!("count({})", rng.pick(&[
            "text()", "comment()", "processing-instruction()", "node()",
            ".//text()", ".//comment()", ".//node()",
        ])),
        5 => format!("floor({})", generate_number(rng, 0)),
        6 => format!("ceiling({})", generate_number(rng, 0)),
        7 => format!("round({})", generate_number(rng, 0)),
        // Deliberately not `last()`. Inside a predicate it is well defined,
        // and `generate_predicate` uses it there; bare in a rule's test it
        // means the size of whatever node list the reference's
        // `apply-templates` built, which no schema controls. A generated case
        // using it reports a difference between two XSLT processors, not a
        // defect here.
        _ => format!("string-length(name({}))", generate_path(rng)),
    }
}

/// A string-valued expression.
fn generate_string(rng: &mut Rng) -> String {
    match rng.below(9) {
        0 => format!("'{}'", rng.pick(VALUES)),
        1 => "name()".to_string(),
        2 => "local-name()".to_string(),
        8 => "namespace-uri()".to_string(),
        3 => "string(.)".to_string(),
        4 => format!("concat(name(), '-', '{}')", rng.pick(VALUES)),
        5 => format!("substring-before(., '{}')", rng.pick(VALUES)),
        6 => format!("substring-after(., '{}')", rng.pick(VALUES)),
        _ => "normalize-space(.)".to_string(),
    }
}

/// An expression whose value both implementations must agree on.
fn generate_expression(rng: &mut Rng, depth: usize, variables: &[String]) -> String {
    if depth > 0 {
        match rng.below(8) {
            0 => {
                return format!(
                    "({}) and ({})",
                    generate_expression(rng, depth - 1, variables),
                    generate_expression(rng, depth - 1, variables)
                )
            }
            1 => {
                return format!(
                    "({}) or ({})",
                    generate_expression(rng, depth - 1, variables),
                    generate_expression(rng, depth - 1, variables)
                )
            }
            2 => return format!("not({})", generate_expression(rng, depth - 1, variables)),
            _ => {}
        }
    }

    let attribute = rng.pick(ATTRIBUTES);
    let element = rng.pick(ELEMENTS);
    let value = rng.pick(VALUES);
    let number = rng.pick(&["0", "1", "2", "3.5"]);
    let comparison = rng.pick(&["=", "!=", "<", ">", "<=", ">="]);
    let boolean = rng.pick(&["true()", "false()"]);
    let other_attribute = rng.pick(ATTRIBUTES);
    let other_element = rng.pick(ELEMENTS);

    // A variable is only in scope where it was declared, so the caller
    // passes exactly the names that are visible here.
    if !variables.is_empty() && rng.chance(1, 6) {
        {
            let name = &variables[rng.below(variables.len())];
            return match rng.below(3) {
                0 => format!("${name} {comparison} {number}"),
                1 => format!("${name} {comparison} '{value}'"),
                _ => format!("boolean(${name})"),
            };
        }
    }

    match rng.below(33) {
        0 => format!("@{attribute}"),
        1 => format!("@{attribute} {comparison} '{value}'"),
        2 => format!("@{attribute} {comparison} {number}"),
        3 => format!("count({element}) {comparison} {number}"),
        4 => format!("string-length(.) {comparison} {number}"),
        5 => format!("normalize-space(.) = '{value}'"),
        6 => format!(". {comparison} '{value}'"),
        7 => format!("number(@{attribute}) {comparison} {number}"),
        8 => element.to_string(),
        9 => format!(".//{element}"),
        10 => format!("sum(.//@{attribute}) {comparison} {number}"),
        11 => format!("starts-with(name(), '{element}')"),
        12 => format!("contains(., '{value}')"),
        13 => format!("substring(., 1, 2) = '{value}'"),
        14 => format!("count(ancestor::*) {comparison} {number}"),
        15 => "boolean(following-sibling::*)".to_string(),
        16 => format!("translate(., 'abc', 'ABC') = '{value}'"),
        17 => format!("string(@{attribute}) {comparison} string({element})"),

        // A node-set on one side. XPath 1.0 makes these existential — true if
        // *any* node satisfies the comparison — which is why `a = b` and
        // `a != b` can both hold.
        18 => format!("{element} {comparison} '{value}'"),
        19 => format!("{element} {comparison} {number}"),
        20 => format!(".//{element} {comparison} '{value}'"),
        21 => format!(".//@{attribute} {comparison} {number}"),
        22 => format!("{element} {comparison} {other_element}"),
        23 => format!("@{attribute} {comparison} @{other_attribute}"),

        // A node-set against a boolean converts the node-set to a boolean —
        // to whether it is non-empty — and never inspects a node's value.
        24 => format!("{element} {comparison} {boolean}"),
        25 => format!("boolean(.//{element}) {comparison} {boolean}"),

        // Node kinds. Only ever inside a test — never as a rule context,
        // which is the `node-kinds` divergence recorded above.
        26 => format!("{} {comparison} '{value}'", rng.pick(&[
            "text()", "normalize-space(text())", "string(comment())",
            "name(processing-instruction())", "string(node())",
        ])),
        27 => format!("count({}) {comparison} {number}", generate_path(rng)),
        31 => generate_path(rng),
        28 => format!("{} {comparison} '{value}'", generate_path(rng)),

        // A union is a node-set built from two, in document order and with
        // duplicates removed — which is where an implementation that simply
        // concatenates goes wrong.
        29 => format!(
            "count({} | {}) {comparison} {number}",
            generate_path(rng),
            generate_path(rng)
        ),

        // Arithmetic, including precedence and the IEEE 754 edges.
        30 => format!(
            "{} {comparison} {}",
            generate_number(rng, 2),
            generate_number(rng, 1)
        ),
        _ => format!("{} {comparison} {}", generate_string(rng), generate_string(rng)),
    }
}

/// An expression used inside `value-of`, where the string value is compared.
fn generate_value_of(rng: &mut Rng) -> String {
    match rng.below(5) {
        0 => format!("@{}", rng.pick(ATTRIBUTES)),
        1 => "name()".to_string(),
        2 => format!("count({})", rng.pick(ELEMENTS)),
        3 => "string-length(.)".to_string(),
        _ => "normalize-space(.)".to_string(),
    }
}

/// XML-escapes a value destined for an attribute.
fn escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;")
}

const CONTEXTS: &[&str] = &[
    "a", "b", "c", "*", "a/b", "//b", "a[@x]", "@x", "root", "root/*", "p:a",
    "p:b", "q:a", "p:*", "//p:a", "p:a/b",
];

/// One rule's body: an optional variable, then assertions.
///
/// `visible` are the variable names already in scope from outside the rule;
/// the rule may add one of its own, which the assertions can then use.
/// The mixed content of an assertion's message.
///
/// A message is not just text: Schematron instantiates `value-of`, `name`,
/// and the inline markup `emph`, `span` and `dir` into it. All of that lands
/// in the message the comparison already checks, so generating it costs
/// nothing extra to compare and covers a whole element group that plain text
/// never reaches.
fn generate_message(rng: &mut Rng, label: &str) -> String {
    let mut out = String::from(label);
    for part in 0..rng.below(3) {
        // After the label a plain space is part of a text node that already
        // has content. Between two elements it would be a whitespace-only
        // text node, which the reference cannot preserve — see
        // `KNOWN_DIVERGENCES` — so a word goes there instead.
        out.push_str(if part == 0 { " " } else { " and " });
        match rng.below(6) {
            0 => out.push_str(&format!(
                "<value-of select=\"{}\"/>",
                escape(&generate_value_of(rng))
            )),
            // `name` with no `path` is the context node's name; with one, the
            // name of whatever the path selects first.
            1 => out.push_str("<name/>"),
            2 => out.push_str(&format!(
                "<name path=\"{}\"/>",
                escape(&generate_path(rng))
            )),
            3 => out.push_str("<emph>emphasised</emph>"),
            4 => out.push_str("<span class=\"c\">spanned</span>"),
            _ => out.push_str("<dir value=\"ltr\">directed</dir>"),
        }
    }
    out
}

fn generate_rule_body(
    rng: &mut Rng,
    label: &str,
    visible: &[String],
    out: &mut String,
    diagnostics: &mut Vec<String>,
    allow_diagnostics: bool,
) {
    let mut variables = visible.to_vec();

    // Names are unique across the whole schema. Two `let`s of one name in
    // nested scopes is a case the reference cannot run at all — it compiles
    // every `let` into one XSLT scope and rejects the redeclaration — so the
    // generator stays out of that territory rather than rediscovering it.
    if rng.chance(1, 2) {
        let name = format!("v{label}");
        let value = match rng.below(3) {
            0 => generate_number(rng, 1),
            1 => generate_string(rng),
            _ => generate_path(rng),
        };
        out.push_str(&format!(
            "      <let name=\"{name}\" value=\"{}\"/>\n",
            escape(&value)
        ));
        variables.push(name);
    }

    for assertion in 0..=rng.below(3) {
        let kind = if rng.chance(1, 2) { "assert" } else { "report" };
        let test = escape(&generate_expression(rng, 2, &variables));
        let message = format!("{label}a{assertion}");

        // `@flag` and `@role` on the assertion itself. Never on the rule:
        // whether a rule's flag reaches its assertions is a documented
        // divergence — see `KNOWN_DIVERGENCES` — and generating one would
        // make every case with a rule-level flag differ.
        let mut attributes = String::new();
        if rng.chance(1, 3) {
            attributes.push_str(&format!(" flag=\"{}\"", rng.pick(&["error", "warning", "info"])));
        }
        if rng.chance(1, 3) {
            attributes.push_str(&format!(" role=\"{}\"", rng.pick(&["required", "structure"])));
        }
        // A diagnostic, whose message the comparison also checks.
        // A library fragment is spliced into another schema without the
        // `diagnostics` section it was written beside, so an assertion there
        // must not reference one — it would name an id the host schema has
        // never heard of.
        let diagnostic = (allow_diagnostics && rng.chance(1, 3))
            .then(|| format!("d{label}a{assertion}"));
        if let Some(id) = &diagnostic {
            attributes.push_str(&format!(" diagnostics=\"{id}\""));
        }

        let body = generate_message(rng, &message);
        out.push_str(&format!(
            "      <{kind} test=\"{test}\"{attributes}>{body}</{kind}>\n"
        ));
        if let Some(id) = diagnostic {
            diagnostics.push(id);
        }
    }
}

/// A library of parts for a generated schema to pull in by reference.
///
/// Written next to the schema, so a relative `href` resolves against the
/// schema's own location — which is the thing that actually has to work.
fn generate_library(rng: &mut Rng) -> String {
    let mut diagnostics = Vec::new();
    let mut out = String::from("<schema xmlns=\"http://purl.oclc.org/dsdl/schematron\">\n");
    for (prefix, uri) in NAMESPACES {
        out.push_str(&format!("  <ns prefix=\"{prefix}\" uri=\"{uri}\"/>\n"));
    }
    // A whole pattern, for `include` to splice as an element.
    out.push_str("  <pattern id=\"libp\">\n");
    out.push_str(&format!(
        "    <rule context=\"{}\">\n",
        CONTEXTS[rng.below(CONTEXTS.len())]
    ));
    generate_rule_body(rng, "libp", &[], &mut out, &mut diagnostics, false);
    out.push_str("    </rule>\n  </pattern>\n");

    // A bare rule, for `extends href` to splice the *children* of. It is not
    // in a pattern on purpose: nothing compiles this file as a schema, both
    // implementations only lift the element the fragment names out of it.
    out.push_str("  <rule id=\"libr\">\n");
    generate_rule_body(rng, "libr", &[], &mut out, &mut diagnostics, false);
    out.push_str("  </rule>\n");

    debug_assert!(diagnostics.is_empty(), "library parts reference no diagnostics");
    out.push_str("</schema>\n");
    out
}

/// Builds a schema, and names the phase to run it under, if any.
fn generate_schema(rng: &mut Rng) -> (String, Option<String>) {
    let mut diagnostics: Vec<String> = Vec::new();
    let mut out = String::from("<schema xmlns=\"http://purl.oclc.org/dsdl/schematron\">\n");
    for (prefix, uri) in NAMESPACES {
        out.push_str(&format!("  <ns prefix=\"{prefix}\" uri=\"{uri}\"/>\n"));
    }

    // A schema-level variable, visible to every rule below.
    let mut global: Vec<String> = Vec::new();
    if rng.chance(1, 3) {
        let value = generate_number(rng, 1);
        out.push_str(&format!(
            "  <let name=\"s0\" value=\"{}\"/>\n",
            escape(&value)
        ));
        global.push("s0".to_string());
    }

    // The concrete patterns, decided up front so a phase can name them.
    let concrete: Vec<String> = (0..=rng.below(3)).map(|i| format!("p{i}")).collect();
    let abstract_pattern = rng.chance(1, 3);
    // `include` splices the element the fragment names; `extends href`
    // splices that element's children into the rule holding it.
    let include_pattern = rng.chance(1, 3);
    let extends_href = rng.chance(1, 3);
    let instance = "i0".to_string();
    let mut activatable = concrete.clone();
    if abstract_pattern {
        activatable.push(instance.clone());
    }
    if include_pattern {
        activatable.push("libp".to_string());
    }

    // Phases. `#ALL` is the default when none is named, so generating a phase
    // is only interesting when it leaves something out.
    let phase = if rng.chance(1, 3) && activatable.len() > 1 {
        let active: Vec<&String> = activatable
            .iter()
            .enumerate()
            .filter(|(i, _)| *i == 0 || rng.chance(1, 2))
            .map(|(_, id)| id)
            .collect();
        out.push_str("  <phase id=\"ph0\">\n");
        for id in active {
            out.push_str(&format!("    <active pattern=\"{id}\"/>\n"));
        }
        out.push_str("  </phase>\n");
        Some("ph0".to_string())
    } else {
        None
    };

    // An abstract pattern, and the concrete pattern that instantiates it.
    // `$ctx` and `$val` are replaced by the `param` values before anything is
    // compiled, which is a text substitution the reference performs too.
    if abstract_pattern {
        out.push_str("  <pattern abstract=\"true\" id=\"abs0\">\n    <rule context=\"$ctx\">\n");
        out.push_str(&format!(
            "      <assert test=\"$val\">abs0a0</assert>\n      <report test=\"{}\">abs0a1</report>\n",
            escape(&generate_expression(rng, 1, &global))
        ));
        out.push_str("    </rule>\n  </pattern>\n");
        out.push_str(&format!("  <pattern id=\"{instance}\" is-a=\"abs0\">\n"));
        out.push_str(&format!(
            "    <param name=\"ctx\" value=\"{}\"/>\n",
            escape(CONTEXTS[rng.below(CONTEXTS.len())])
        ));
        out.push_str(&format!(
            "    <param name=\"val\" value=\"{}\"/>\n",
            escape(&generate_expression(rng, 1, &[]))
        ));
        out.push_str("  </pattern>\n");
    }

    if include_pattern {
        out.push_str("  <include href=\"lib.sch#libp\"/>\n");
    }

    for (index, id) in concrete.iter().enumerate() {
        out.push_str(&format!("  <pattern id=\"{id}\">\n"));

        // An abstract rule, spliced into a concrete one by `extends`. Its
        // assertions land at the position of the `extends` element, which is
        // what makes the ordering worth comparing.
        let abstract_rule = rng.chance(1, 3);
        if abstract_rule {
            out.push_str(&format!("    <rule abstract=\"true\" id=\"ar{index}\">\n"));
            generate_rule_body(rng, &format!("{id}ar"), &global, &mut out, &mut diagnostics, true);
            out.push_str("    </rule>\n");
        }

        for rule in 0..=rng.below(3) {
            let context = CONTEXTS[rng.below(CONTEXTS.len())];
            out.push_str(&format!("    <rule context=\"{context}\">\n"));
            if abstract_rule && rng.chance(1, 2) {
                out.push_str(&format!("      <extends rule=\"ar{index}\"/>\n"));
            }
            if extends_href && rng.chance(1, 2) {
                out.push_str("      <extends href=\"lib.sch#libr\"/>\n");
            }
            generate_rule_body(rng, &format!("{id}r{rule}"), &global, &mut out, &mut diagnostics, true);
            out.push_str("    </rule>\n");
        }
        out.push_str("  </pattern>\n");
    }

    if !diagnostics.is_empty() {
        out.push_str("  <diagnostics>\n");
        for id in &diagnostics {
            out.push_str(&format!(
                "    <diagnostic id=\"{id}\">help for {id}</diagnostic>\n"
            ));
        }
        out.push_str("  </diagnostics>\n");
    }
    out.push_str("</schema>\n");
    (out, phase)
}

/// The first place two finding lists differ, with counts, so a failure reads
/// as a diff rather than as two long dumps the reader must align by eye.
fn describe_difference(theirs: &[Finding], mine: &[Finding]) -> String {
    let show = |finding: Option<&Finding>| match finding {
        Some(f) => format!(
            "{} {:?} => {:?} flag={:?} role={:?} diagnostics={:?}",
            f.kind, f.test, f.text, f.flag, f.role, f.diagnostics
        ),
        None => "<nothing>".to_string(),
    };
    let at = (0..theirs.len().max(mine.len()))
        .find(|&i| theirs.get(i) != mine.get(i))
        .unwrap_or(0);
    format!(
        "findings differ: reference has {}, this crate has {}; \
         first difference at index {at}\n    reference: {}\n    ours:      {}",
        theirs.len(),
        mine.len(),
        show(theirs.get(at)),
        show(mine.get(at)),
    )
}

/// How many cases to generate.
///
/// The default was chosen by sabotage: breaking the existential node-set
/// comparison rule is caught somewhere between 200 and 300 cases, so 200 is
/// too few to claim coverage of it. Raise it when hunting; the run is linear
/// in the count.
fn generated_case_count() -> u64 {
    std::env::var("SCHEMATRON_FUZZ_CASES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(500)
}

/// The seed to start from, so a reported failure can be re-run alone.
fn generated_first_seed() -> u64 {
    std::env::var("SCHEMATRON_FUZZ_SEED")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1)
}

#[test]
#[ignore = "needs xsltproc and the reference stylesheets; see the module docs"]
fn generated_cases_agree_with_the_reference_implementation() {
    let skeleton = skeleton();
    let work = std::env::temp_dir().join("schematron-differential-generated");
    std::fs::create_dir_all(&work).expect("work directory");

    let first = generated_first_seed();
    let count = generated_case_count();

    let mut agreed = 0;
    let mut produced = 0usize;
    let mut unresolvable = 0usize;
    let mut firings = 0usize;
    let mut failures = Vec::new();

    for seed in first..first + count {
        let mut rng = Rng::new(seed);
        let (schema_text, phase) = generate_schema(&mut rng);
        let document_text = generate_document(&mut rng);

        let schema = work.join("schema.sch");
        let document = work.join("input.xml");
        std::fs::write(&schema, &schema_text).expect("write schema");
        std::fs::write(work.join("lib.sch"), generate_library(&mut rng)).expect("write library");
        std::fs::write(&document, &document_text).expect("write document");

        let report = |what: String| {
            format!(
                "seed {seed}: {what}\n  schema:\n{}\n  document:\n    {}\n",
                schema_text
                    .lines()
                    .map(|line| format!("    {line}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
                document_text
            )
        };

        let phase = phase.as_deref();
        let reference = match reference_svrl(&skeleton, &work, &schema, &document, phase) {
            Ok(svrl) => svrl,
            Err(why) => {
                // The generator emits only XPath 1.0 both sides implement, so
                // this means the generator is wrong, not the crate. Either
                // way it needs looking at rather than skipping.
                failures.push(report(format!("the reference could not run it: {why}")));
                continue;
            }
        };

        let ours = match our_svrl(&schema, &document, phase) {
            Ok(svrl) => svrl,
            Err(why) => {
                failures.push(report(format!("this crate could not run it: {why}")));
                continue;
            }
        };

        let comparison = compare_case(&reference, &ours, &document_text);
        unresolvable += comparison.unresolvable;
        firings += comparison.firings;
        match comparison.difference {
            None => {
                agreed += 1;
                produced += comparison.findings;
            }
            Some(why) => failures.push(report(why)),
        }

        // A wall of near-identical failures helps nobody; the first few carry
        // the information.
        if failures.len() >= 5 {
            break;
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} generated cases disagreed with the reference \
         (re-run one with SCHEMATRON_FUZZ_SEED=<seed> SCHEMATRON_FUZZ_CASES=1):\n\n{}",
        failures.len(),
        count,
        failures.join("\n")
    );
    // Agreement on nothing is not agreement. If the generator stopped
    // producing findings — a grammar change that makes every test true, say —
    // this test would pass while checking nothing at all.
    assert!(
        produced >= agreed,
        "the generated cases produced only {produced} findings across {agreed} \
         agreeing cases, which is too few to be checking anything"
    );
    println!(
        "generated cases agreed: {agreed}, findings compared: {produced}, \
         rule firings compared: {firings}, \
         locations the reference could not resolve: {unresolvable}"
    );
}
