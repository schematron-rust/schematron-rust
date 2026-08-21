//! Serialising a tree, and the escaping rules the SVRL writer needs.

/// Escapes text for use in element content.
///
/// `<` and `&` must be escaped; `>` need not be, but is, because the
/// three-character sequence `]]>` is forbidden in content and escaping every
/// `>` avoids having to detect it.
///
/// # Examples
///
/// ```
/// use schematron::xml::escape_text;
///
/// assert_eq!(escape_text("a < b & c"), "a &lt; b &amp; c");
/// ```
#[must_use]
pub fn escape_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            _ => out.push(c),
        }
    }
    out
}

/// Escapes text for use in a double-quoted attribute value.
///
/// Adds `"`, and the whitespace characters that attribute-value
/// normalisation would otherwise collapse into spaces on the way back in.
///
/// # Examples
///
/// ```
/// use schematron::xml::escape_attribute;
///
/// assert_eq!(escape_attribute("say \"hi\""), "say &quot;hi&quot;");
/// ```
#[must_use]
pub fn escape_attribute(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_markup_in_text() {
        assert_eq!(escape_text("<a>&"), "&lt;a&gt;&amp;");
    }

    #[test]
    fn escapes_quotes_in_attributes() {
        assert_eq!(escape_attribute(r#"a"b"#), "a&quot;b");
    }

    #[test]
    fn escapes_newlines_in_attributes() {
        assert_eq!(escape_attribute("a\nb"), "a&#10;b");
    }

    #[test]
    fn leaves_ordinary_text_alone() {
        assert_eq!(escape_text("plain text"), "plain text");
    }
}
