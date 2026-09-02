//! RFC 3986 URI-reference resolution, for XPath 2.0's `resolve-uri()`.
//!
//! A small, self-contained implementation rather than a dependency: the
//! crate already writes its own XML and XPath parsers, and a generic URI
//! crate would pull in far more than this one function needs (validation,
//! percent-encoding policy, IRI support). This implements exactly RFC 3986
//! §5.2's reference resolution algorithm, verified against the worked
//! examples in §5.4.1 and §5.4.2 (see the tests below) — nothing more.

/// The five components of a URI reference, per RFC 3986 §3 / Appendix B.
struct Parts<'a> {
    scheme: Option<&'a str>,
    authority: Option<&'a str>,
    path: &'a str,
    query: Option<&'a str>,
    fragment: Option<&'a str>,
}

/// Splits a URI reference into its components, applying the ABNF-equivalent
/// regular expression RFC 3986 Appendix B gives by hand rather than through
/// the `regex` crate: five bounded scans, no backtracking, and no pattern to
/// keep in sync with the spec text.
fn split(s: &str) -> Parts<'_> {
    let mut rest = s;

    let fragment = rest.find('#').map(|i| {
        let f = &rest[i + 1..];
        rest = &rest[..i];
        f
    });

    let query = rest.find('?').map(|i| {
        let q = &rest[i + 1..];
        rest = &rest[..i];
        q
    });

    // A colon only introduces a scheme if what precedes it actually matches
    // the scheme grammar — otherwise a relative reference like `a:b/c`,
    // whose first segment merely contains a colon, would be misread as
    // having scheme `a`. RFC 3986 §3.3 forbids exactly that ambiguity in a
    // relative-path reference's first segment; checking the grammar here
    // has the same effect without needing a separate "is this relative"
    // pre-pass.
    let scheme = rest.find(':').and_then(|i| {
        let candidate = &rest[..i];
        if is_scheme(candidate) {
            rest = &rest[i + 1..];
            Some(candidate)
        } else {
            None
        }
    });

    let authority = rest.strip_prefix("//").map(|after| {
        let end = after.find('/').unwrap_or(after.len());
        let a = &after[..end];
        rest = &after[end..];
        a
    });

    Parts {
        scheme,
        authority,
        path: rest,
        query,
        fragment,
    }
}

/// `ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )`, RFC 3986 §3.1.
fn is_scheme(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// RFC 3986 §5.2.4: removes `.` and `..` path segments.
fn remove_dot_segments(path: &str) -> String {
    let mut input = path.to_string();
    let mut output = String::new();

    while !input.is_empty() {
        if let Some(rest) = input.strip_prefix("../") {
            input = rest.to_string();
        } else if let Some(rest) = input.strip_prefix("./") {
            input = rest.to_string();
        } else if let Some(rest) = input.strip_prefix("/./") {
            input = format!("/{rest}");
        } else if input == "/." {
            input = "/".to_string();
        } else if let Some(rest) = input.strip_prefix("/../") {
            input = format!("/{rest}");
            remove_last_segment(&mut output);
        } else if input == "/.." {
            input = "/".to_string();
            remove_last_segment(&mut output);
        } else if input == "." || input == ".." {
            input.clear();
        } else {
            // Move the first path segment — the leading '/', if any, plus
            // everything up to (not including) the next '/' — from input
            // to output.
            let start = usize::from(input.starts_with('/'));
            let end = input[start..].find('/').map_or(input.len(), |i| start + i);
            output.push_str(&input[..end]);
            input = input[end..].to_string();
        }
    }

    output
}

/// Drops the last path segment and its preceding `/` (if any) from an
/// output buffer being built by [`remove_dot_segments`].
fn remove_last_segment(output: &mut String) {
    match output.rfind('/') {
        Some(i) => output.truncate(i),
        None => output.clear(),
    }
}

/// RFC 3986 §5.3: recomposes components into a URI string.
fn recompose(
    scheme: Option<&str>,
    authority: Option<&str>,
    path: &str,
    query: Option<&str>,
    fragment: Option<&str>,
) -> String {
    let mut out = String::new();
    if let Some(scheme) = scheme {
        out.push_str(scheme);
        out.push(':');
    }
    if let Some(authority) = authority {
        out.push_str("//");
        out.push_str(authority);
    }
    out.push_str(path);
    if let Some(query) = query {
        out.push('?');
        out.push_str(query);
    }
    if let Some(fragment) = fragment {
        out.push('#');
        out.push_str(fragment);
    }
    out
}

/// RFC 3986 §5.2.3: merges a relative reference's path onto a base URI.
fn merge(base: &Parts<'_>, reference_path: &str) -> String {
    if base.authority.is_some() && base.path.is_empty() {
        format!("/{reference_path}")
    } else {
        match base.path.rfind('/') {
            Some(i) => format!("{}{reference_path}", &base.path[..=i]),
            None => reference_path.to_string(),
        }
    }
}

/// RFC 3986 §5.2.2: resolves `reference` against `base`.
///
/// `resolve-uri()`'s own contract expects `base` to already be absolute;
/// this function does not check that, and resolving against a relative base
/// simply produces a relative result, the same as most implementations.
pub(crate) fn resolve(reference: &str, base: &str) -> String {
    let r = split(reference);
    let b = split(base);

    let (scheme, authority, path, query) = if let Some(scheme) = r.scheme {
        (scheme, r.authority, remove_dot_segments(r.path), r.query)
    } else if r.authority.is_some() {
        (
            b.scheme.unwrap_or_default(),
            r.authority,
            remove_dot_segments(r.path),
            r.query,
        )
    } else if r.path.is_empty() {
        (
            b.scheme.unwrap_or_default(),
            b.authority,
            b.path.to_string(),
            r.query.or(b.query),
        )
    } else if r.path.starts_with('/') {
        (
            b.scheme.unwrap_or_default(),
            b.authority,
            remove_dot_segments(r.path),
            r.query,
        )
    } else {
        (
            b.scheme.unwrap_or_default(),
            b.authority,
            remove_dot_segments(&merge(&b, r.path)),
            r.query,
        )
    };

    let scheme = (!scheme.is_empty()).then_some(scheme);
    recompose(scheme, authority, &path, query, r.fragment)
}

#[cfg(test)]
mod tests {
    use super::resolve;

    /// RFC 3986 §5.4.1, "Normal Examples", against the base URI its own
    /// worked examples use.
    #[test]
    fn rfc_3986_normal_examples() {
        let base = "http://a/b/c/d;p?q";
        for (reference, expected) in [
            ("g:h", "g:h"),
            ("g", "http://a/b/c/g"),
            ("./g", "http://a/b/c/g"),
            ("g/", "http://a/b/c/g/"),
            ("/g", "http://a/g"),
            ("//g", "http://g"),
            ("?y", "http://a/b/c/d;p?y"),
            ("g?y", "http://a/b/c/g?y"),
            ("#s", "http://a/b/c/d;p?q#s"),
            ("g#s", "http://a/b/c/g#s"),
            ("g?y#s", "http://a/b/c/g?y#s"),
            (";x", "http://a/b/c/;x"),
            ("g;x", "http://a/b/c/g;x"),
            ("g;x?y#s", "http://a/b/c/g;x?y#s"),
            ("", "http://a/b/c/d;p?q"),
            (".", "http://a/b/c/"),
            ("./", "http://a/b/c/"),
            ("..", "http://a/b/"),
            ("../", "http://a/b/"),
            ("../g", "http://a/b/g"),
            ("../..", "http://a/"),
            ("../../", "http://a/"),
            ("../../g", "http://a/g"),
        ] {
            assert_eq!(resolve(reference, base), expected, "resolving {reference:?}");
        }
    }

    /// RFC 3986 §5.4.2, "Abnormal Examples" — the cases that most often
    /// separate a correct implementation from one that merely handles the
    /// common cases. The RFC recommends this exact behavior even where the
    /// input itself is questionable, so that different implementations
    /// agree.
    #[test]
    fn rfc_3986_abnormal_examples() {
        let base = "http://a/b/c/d;p?q";
        for (reference, expected) in [
            ("../../../g", "http://a/g"),
            ("../../../../g", "http://a/g"),
            ("/./g", "http://a/g"),
            ("/../g", "http://a/g"),
            ("g.", "http://a/b/c/g."),
            (".g", "http://a/b/c/.g"),
            ("g..", "http://a/b/c/g.."),
            ("..g", "http://a/b/c/..g"),
            ("./../g", "http://a/b/g"),
            ("./g/.", "http://a/b/c/g/"),
            ("g/./h", "http://a/b/c/g/h"),
            ("g/../h", "http://a/b/c/h"),
            ("g;x=1/./y", "http://a/b/c/g;x=1/y"),
            ("g;x=1/../y", "http://a/b/c/y"),
            ("g?y/./x", "http://a/b/c/g?y/./x"),
            ("g?y/../x", "http://a/b/c/g?y/../x"),
            ("g#s/./x", "http://a/b/c/g#s/./x"),
            ("g#s/../x", "http://a/b/c/g#s/../x"),
        ] {
            assert_eq!(resolve(reference, base), expected, "resolving {reference:?}");
        }
    }

    #[test]
    fn a_reference_with_its_own_scheme_ignores_the_base_entirely() {
        assert_eq!(resolve("mailto:x@example.com", "http://a/b/"), "mailto:x@example.com");
    }
}
