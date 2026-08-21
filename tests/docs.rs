//! The documentation audit.
//!
//! Documentation that does not work is worse than none, because the reader
//! trusts it. These tests machine-check the claims that would otherwise rot
//! silently:
//!
//! - every schema shown in `spec/*.md` and `README.md` compiles
//! - every example `.sch` file compiles
//! - every relative link resolves
//! - every `spec/*.md` is reachable from `spec/index.md`
//! - the MSRV in the spec matches the one `Cargo.toml` enforces
//! - the XPath function list in the spec matches the engine's
//! - every runnable example is referenced from the documentation
//! - no documentation file exceeds its size budget
//!
//! The principle throughout is **single source of truth**: a fact lives in
//! one place, and anywhere it is repeated, a test ties the copies together.
//! See `spec/index.md`.

use std::fs;
use std::path::{Path, PathBuf};

use schematron::Schema;

/// Every Markdown file that might contain a schema.
fn markdown_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![root.join("README.md")];
    let mut specs: Vec<PathBuf> = fs::read_dir(root.join("spec"))
        .expect("spec/ should exist")
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "md"))
        .collect();
    specs.sort();
    files.extend(specs);
    files
}

/// Pulls fenced ```xml blocks out of Markdown.
fn xml_blocks(source: &str) -> Vec<(usize, String)> {
    let mut blocks = Vec::new();
    let mut current: Option<(usize, Vec<&str>)> = None;

    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        match &mut current {
            None => {
                if trimmed == "```xml" {
                    current = Some((index + 1, Vec::new()));
                }
            }
            Some((start, lines)) => {
                if trimmed == "```" {
                    blocks.push((*start, lines.join("\n")));
                    current = None;
                } else {
                    lines.push(line);
                }
            }
        }
    }
    blocks
}

/// Whether a block is a whole schema rather than a fragment.
///
/// Fragments — a lone `<pattern>`, a lone `<rule>` — are shown deliberately,
/// and cannot be compiled on their own.
fn is_whole_schema(block: &str) -> bool {
    let body = block.trim_start_matches(|c: char| c.is_whitespace());
    let body = if body.starts_with("<?xml") {
        body.split_once("?>").map_or(body, |(_, rest)| rest).trim_start()
    } else {
        body
    };
    body.starts_with("<schema") || body.starts_with("<!--")
}

#[test]
fn every_documented_schema_compiles() {
    let mut checked = 0;
    let mut failures = Vec::new();

    for file in markdown_files() {
        let source = fs::read_to_string(&file).expect("markdown file should be readable");
        let name = file
            .strip_prefix(env!("CARGO_MANIFEST_DIR"))
            .unwrap_or(&file)
            .display()
            .to_string();

        for (line, block) in xml_blocks(&source) {
            if !is_whole_schema(&block) {
                continue;
            }
            checked += 1;
            if let Err(error) = Schema::from_str(&block) {
                failures.push(format!("{name}:{line}\n    {error}\n"));
            }
        }
    }

    assert!(checked > 0, "no documented schemas were found to check");
    assert!(
        failures.is_empty(),
        "{} of {checked} documented schemas do not compile:\n\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn the_example_schema_files_compile() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut checked = 0;

    for entry in fs::read_dir(&examples).expect("examples/ should exist") {
        let path = entry.expect("directory entry").path();
        if path.extension().is_some_and(|e| e == "sch") {
            Schema::from_path(&path)
                .unwrap_or_else(|e| panic!("{} does not compile: {e}", path.display()));
            checked += 1;
        }
    }

    assert!(checked > 0, "no example schemas were found");
}

#[test]
fn every_spec_document_is_linked_from_the_index() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let index = fs::read_to_string(root.join("spec/index.md")).expect("spec/index.md");

    for entry in fs::read_dir(root.join("spec")).expect("spec/ should exist") {
        let path = entry.expect("directory entry").path();
        if path.extension().is_none_or(|e| e != "md") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if name == "index.md" {
            continue;
        }
        assert!(
            index.contains(&format!("({name})")),
            "spec/{name} is not linked from spec/index.md"
        );
    }
}

/// The declared MSRV, read out of `Cargo.toml`.
fn declared_msrv() -> String {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("Cargo.toml should be readable");
    manifest
        .lines()
        .find_map(|line| {
            let rest = line.trim().strip_prefix("rust-version")?;
            let value = rest.split_once('=')?.1;
            // Take what is inside the quotes, ignoring any trailing comment.
            let value = value.split('"').nth(1)?;
            Some(value.to_string())
        })
        .expect("Cargo.toml should declare rust-version")
}

#[test]
fn the_msrv_spec_agrees_with_cargo_toml() {
    // The MSRV policy is only worth anything if the number in the spec and the
    // number the toolchain actually enforces are the same one. Keeping them in
    // sync by hand is exactly the kind of bookkeeping that rots quietly.
    let msrv = declared_msrv();
    let spec = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("spec/rust-msrv-n-minus-3.md"),
    )
    .expect("spec/rust-msrv-n-minus-3.md should exist");

    assert!(
        spec.contains(&format!("rust-version = \"{msrv}\"")),
        "spec/rust-msrv-n-minus-3.md does not show rust-version = {msrv:?}, \
         which is what Cargo.toml declares"
    );
    assert!(
        spec.contains(&format!("**{msrv}**")),
        "spec/rust-msrv-n-minus-3.md does not name {msrv:?} as the current MSRV \
         in its value table"
    );
    assert!(
        spec.contains(&format!("cargo +{msrv} test")),
        "spec/rust-msrv-n-minus-3.md does not show how to verify {msrv:?}"
    );

    // The same command must be the one the testing spec tells people to run.
    let testing = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("spec/testing.md"))
        .expect("spec/testing.md should exist");
    assert!(
        testing.contains(&format!("cargo +{msrv} test")),
        "spec/testing.md does not list the MSRV check for {msrv:?}"
    );
}

#[test]
fn every_documented_toolchain_pin_is_the_declared_msrv() {
    // The MSRV appears in several documents, because a reader of any one of
    // them needs the command in front of them. Cargo.toml is the single
    // source; this ties every copy back to it, so a bump cannot leave a stale
    // `cargo +1.xx` behind in a file nobody thought to update.
    let msrv = declared_msrv();
    let mut stale = Vec::new();

    for file in documentation_files() {
        let source = fs::read_to_string(&file).expect("documentation should be readable");
        for (index, line) in source.lines().enumerate() {
            // Find `cargo +<version>` and check the version, ignoring
            // `+nightly` and `+stable`, which are not pins.
            let mut rest = line;
            while let Some(at) = rest.find("cargo +") {
                let after = &rest[at + "cargo +".len()..];
                let end = after
                    .find(|c: char| !c.is_ascii_digit() && c != '.')
                    .unwrap_or(after.len());
                let version = &after[..end];
                if !version.is_empty() && version != msrv {
                    stale.push(format!(
                        "{}:{}: pins {version}, but Cargo.toml declares {msrv}",
                        file.display(),
                        index + 1
                    ));
                }
                rest = &after[end..];
            }
        }
    }

    assert!(
        stale.is_empty(),
        "stale toolchain pin(s) in documentation:\n  {}",
        stale.join("\n  ")
    );
}

/// The size budget for any single documentation file.
///
/// Agent instruction files are read into a model's context on every session,
/// so an oversized one is a tax paid repeatedly. The budget applies to the
/// human-facing documentation too, on the theory that a file nobody can
/// finish reading is a file nobody reads.
const MAX_DOC_BYTES: u64 = 40_000;

/// Every Markdown file in the repository that counts as documentation.
fn documentation_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();

    for name in ["README.md", "index.md", "AGENTS.md", "CLAUDE.md"] {
        let path = root.join(name);
        if path.exists() {
            files.push(path);
        }
    }
    for directory in ["spec", "AGENTS"] {
        let directory = root.join(directory);
        if !directory.is_dir() {
            continue;
        }
        let mut found: Vec<PathBuf> = fs::read_dir(&directory)
            .expect("documentation directory should be readable")
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|e| e == "md"))
            .collect();
        found.sort();
        files.extend(found);
    }
    files
}

/// Extracts `[text](target)` links, skipping absolute URLs and anchors.
fn relative_links(source: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find("](") {
        let after = &rest[start + 2..];
        let Some(end) = after.find(')') else { break };
        let target = &after[..end];
        if !target.starts_with("http") && !target.starts_with('#') && !target.contains(' ') {
            links.push(target.to_string());
        }
        rest = &after[end..];
    }
    links
}

#[test]
fn every_relative_link_resolves() {
    let mut broken = Vec::new();

    for file in documentation_files() {
        let source = fs::read_to_string(&file).expect("documentation should be readable");
        let directory = file.parent().expect("a file has a parent directory");

        for link in relative_links(&source) {
            // Strip any `#anchor` before checking the path itself.
            let path = link.split('#').next().unwrap_or(&link);
            if path.is_empty() {
                continue;
            }
            if !directory.join(path).exists() {
                broken.push(format!("{}: {link}", file.display()));
            }
        }
    }

    assert!(
        broken.is_empty(),
        "{} broken link(s):\n  {}",
        broken.len(),
        broken.join("\n  ")
    );
}

#[test]
fn documentation_files_are_within_the_size_budget() {
    let mut oversized = Vec::new();

    for file in documentation_files() {
        let bytes = fs::metadata(&file)
            .expect("documentation should be readable")
            .len();
        if bytes > MAX_DOC_BYTES {
            oversized.push(format!(
                "{}: {bytes} bytes, over the {MAX_DOC_BYTES} budget",
                file.display()
            ));
        }
    }

    assert!(
        oversized.is_empty(),
        "{} file(s) over budget — split them:\n  {}",
        oversized.len(),
        oversized.join("\n  ")
    );
}

#[test]
fn the_xpath_function_list_in_the_spec_matches_the_engine() {
    // The engine is the source of truth; the spec must not drift from it.
    let spec = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("spec/xpath.md"))
        .expect("spec/xpath.md should exist");

    for function in schematron::xpath::function_names() {
        // Either spelling counts: the core functions are listed bare, while
        // `current()` is documented with parentheses in its own table.
        let bare = format!("`{function}`");
        let called = format!("`{function}(");
        assert!(
            spec.contains(&bare) || spec.contains(&called),
            "spec/xpath.md does not mention the `{function}` function"
        );
    }
}

#[test]
fn the_xpath_two_function_list_in_the_spec_matches_the_engine() {
    let spec = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("spec/xpath2.md"))
        .expect("spec/xpath2.md should exist");

    for function in schematron::xpath::function_names_v2() {
        let bare = format!("`{function}`");
        let called = format!("`{function}(");
        assert!(
            spec.contains(&bare) || spec.contains(&called),
            "spec/xpath2.md does not document the `{function}` function"
        );
    }
}

#[test]
fn every_runnable_example_is_referenced_from_the_documentation() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = fs::read_to_string(root.join("README.md")).expect("README.md");
    let tutorial =
        fs::read_to_string(root.join("spec/tutorial.md")).expect("spec/tutorial.md");
    let both = format!("{readme}{tutorial}");

    let mut unreferenced = Vec::new();
    for entry in fs::read_dir(root.join("examples")).expect("examples/ should exist") {
        let path = entry.expect("directory entry").path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        if !both.contains(&stem) {
            unreferenced.push(stem);
        }
    }

    assert!(
        unreferenced.is_empty(),
        "example(s) not referenced from README.md or spec/tutorial.md: {}",
        unreferenced.join(", ")
    );
}

#[test]
fn every_agent_document_is_reachable() {
    // A document nobody links to is a document nobody reads, and it rots
    // faster than the ones that are read. Everything under AGENTS/ must be
    // reachable from AGENTS.md or from the repository index.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let directory = root.join("AGENTS");
    if !directory.is_dir() {
        return;
    }

    let entry_points = format!(
        "{}{}",
        fs::read_to_string(root.join("AGENTS.md")).unwrap_or_default(),
        fs::read_to_string(root.join("index.md")).unwrap_or_default()
    );

    let mut orphans = Vec::new();
    for entry in fs::read_dir(&directory).expect("AGENTS/ should be readable") {
        let path = entry.expect("directory entry").path();
        if path.extension().is_none_or(|e| e != "md") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if !entry_points.contains(&format!("AGENTS/{name}")) {
            orphans.push(name);
        }
    }

    assert!(
        orphans.is_empty(),
        "AGENTS/ document(s) not linked from AGENTS.md or index.md: {}",
        orphans.join(", ")
    );
}

#[test]
fn documentation_does_not_hard_code_volatile_counts() {
    // A test count or a line count written into prose is stale within a day,
    // and a stale number is worse than no number because it is quietly
    // trusted. This caught a real one: the MSRV spec claimed "290 tests
    // passing" after the suite had grown to 297.
    //
    // Reference the command that produces the number instead.
    let pattern = regex_lite_tests_count();
    let mut offenders = Vec::new();

    for file in documentation_files() {
        let source = fs::read_to_string(&file).expect("documentation should be readable");
        for (index, line) in source.lines().enumerate() {
            if pattern(line) {
                offenders.push(format!("{}:{}: {}", file.display(), index + 1, line.trim()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "documentation hard-codes a test count; reference the command instead:\n  {}",
        offenders.join("\n  ")
    );
}

/// Matches prose that hard-codes a count of tests, without pulling in a regex
/// dependency for one check.
fn regex_lite_tests_count() -> impl Fn(&str) -> bool {
    |line: &str| {
        let lower = line.to_lowercase();
        for marker in [" tests", " tests.", " tests,", " tests:", " tests passing"] {
            let Some(position) = lower.find(marker) else {
                continue;
            };
            // Look at the token immediately before "tests".
            let before = lower[..position].trim_end();
            let token = before.rsplit(|c: char| c.is_whitespace()).next().unwrap_or("");
            let digits = token.trim_start_matches(|c: char| !c.is_ascii_digit());
            if digits.len() >= 2 && digits.chars().all(|c| c.is_ascii_digit()) {
                return true;
            }
        }
        false
    }
}
