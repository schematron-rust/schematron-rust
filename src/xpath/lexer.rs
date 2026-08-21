//! The XPath 1.0 tokenizer.
//!
//! XPath's grammar is context-sensitive at the lexical level: whether `*` is
//! a wildcard or a multiply operator, and whether `div` is an element name or
//! an operator, depends on what came before. The three disambiguation rules
//! of XPath 1.0 section 3.7 are implemented here by tracking the previous
//! token, so the parser above can stay a plain recursive descent.

use std::fmt;

/// One lexical token, with the byte offset it started at.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Token {
    pub kind: TokenKind,
    pub position: usize,
}

/// The token kinds of XPath 1.0, after disambiguation.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TokenKind {
    /// A name used as a node test, possibly with a prefix.
    Name(String),
    /// A name followed by `(` that is not a node type: a function call.
    FunctionName(String),
    /// A name followed by `::`.
    AxisName(String),
    /// One of `node`, `text`, `comment`, `processing-instruction`, followed by `(`.
    NodeType(String),
    /// A quoted string.
    Literal(String),
    /// A numeric literal.
    Number(f64),
    /// `$name`.
    Variable(String),

    Slash,
    DoubleSlash,
    Dot,
    DoubleDot,
    At,
    Comma,
    ColonColon,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,

    /// `*` used as a wildcard node test.
    Star,
    /// `*` used as the multiply operator.
    Multiply,
    Plus,
    Minus,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Pipe,
    And,
    Or,
    Div,
    Mod,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Name(n) | TokenKind::AxisName(n) | TokenKind::NodeType(n) => {
                f.write_str(n)
            }
            TokenKind::FunctionName(n) => write!(f, "{n}()"),
            TokenKind::Literal(s) => write!(f, "'{s}'"),
            TokenKind::Number(n) => write!(f, "{n}"),
            TokenKind::Variable(v) => write!(f, "${v}"),
            TokenKind::Slash => f.write_str("/"),
            TokenKind::DoubleSlash => f.write_str("//"),
            TokenKind::Dot => f.write_str("."),
            TokenKind::DoubleDot => f.write_str(".."),
            TokenKind::At => f.write_str("@"),
            TokenKind::Comma => f.write_str(","),
            TokenKind::ColonColon => f.write_str("::"),
            TokenKind::LeftParen => f.write_str("("),
            TokenKind::RightParen => f.write_str(")"),
            TokenKind::LeftBracket => f.write_str("["),
            TokenKind::RightBracket => f.write_str("]"),
            TokenKind::Star | TokenKind::Multiply => f.write_str("*"),
            TokenKind::Plus => f.write_str("+"),
            TokenKind::Minus => f.write_str("-"),
            TokenKind::Equal => f.write_str("="),
            TokenKind::NotEqual => f.write_str("!="),
            TokenKind::Less => f.write_str("<"),
            TokenKind::LessEqual => f.write_str("<="),
            TokenKind::Greater => f.write_str(">"),
            TokenKind::GreaterEqual => f.write_str(">="),
            TokenKind::Pipe => f.write_str("|"),
            TokenKind::And => f.write_str("and"),
            TokenKind::Or => f.write_str("or"),
            TokenKind::Div => f.write_str("div"),
            TokenKind::Mod => f.write_str("mod"),
        }
    }
}

/// A lexical error, reported with the offset it occurred at.
#[derive(Debug, Clone)]
pub(crate) struct LexError {
    pub position: usize,
    pub message: String,
}

/// The node type names, which take precedence over function names.
const NODE_TYPES: [&str; 4] = ["node", "text", "comment", "processing-instruction"];

/// Tokenizes an XPath expression.
pub(crate) fn tokenize(input: &str) -> Result<Vec<Token>, LexError> {
    Lexer::new(input).run()
}

struct Lexer<'a> {
    input: &'a str,
    bytes: &'a [u8],
    position: usize,
    tokens: Vec<Token>,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            bytes: input.as_bytes(),
            position: 0,
            tokens: Vec::new(),
        }
    }

    fn error(position: usize, message: impl Into<String>) -> LexError {
        LexError {
            position,
            message: message.into(),
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.position + offset).copied()
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.position += 1;
        }
    }

    /// Whether the previous token allows an operator here.
    ///
    /// XPath 1.0 section 3.7: if there is a preceding token and it is not one
    /// of `@`, `::`, `(`, `[`, `,` or an operator, then `*` is the multiply
    /// operator and an `NCName` is an operator name.
    fn previous_allows_operator(&self) -> bool {
        match self.tokens.last().map(|t| &t.kind) {
            None => false,
            Some(kind) => !matches!(
                kind,
                TokenKind::At
                    | TokenKind::ColonColon
                    | TokenKind::LeftParen
                    | TokenKind::LeftBracket
                    | TokenKind::Comma
                    | TokenKind::Slash
                    | TokenKind::DoubleSlash
                    | TokenKind::Star
                    | TokenKind::Multiply
                    | TokenKind::Plus
                    | TokenKind::Minus
                    | TokenKind::Equal
                    | TokenKind::NotEqual
                    | TokenKind::Less
                    | TokenKind::LessEqual
                    | TokenKind::Greater
                    | TokenKind::GreaterEqual
                    | TokenKind::Pipe
                    | TokenKind::And
                    | TokenKind::Or
                    | TokenKind::Div
                    | TokenKind::Mod
            ),
        }
    }

    /// The next non-whitespace byte after `from`, without consuming.
    fn next_significant(&self, from: usize) -> Option<u8> {
        let mut i = from;
        while matches!(self.bytes.get(i), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            i += 1;
        }
        self.bytes.get(i).copied()
    }

    /// The two next non-whitespace bytes after `from`.
    fn next_two_significant(&self, from: usize) -> Option<(u8, u8)> {
        let mut i = from;
        while matches!(self.bytes.get(i), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            i += 1;
        }
        match (self.bytes.get(i), self.bytes.get(i + 1)) {
            (Some(&a), Some(&b)) => Some((a, b)),
            _ => None,
        }
    }

    fn push(&mut self, kind: TokenKind, position: usize) {
        self.tokens.push(Token { kind, position });
    }

    /// The tokenizing loop: one arm per leading character.
    #[allow(clippy::too_many_lines)] // A dispatch table; splitting it hides the flow.
    fn run(mut self) -> Result<Vec<Token>, LexError> {
        loop {
            self.skip_whitespace();
            let start = self.position;
            let Some(c) = self.peek() else { break };

            match c {
                b'(' => {
                    self.position += 1;
                    self.push(TokenKind::LeftParen, start);
                }
                b')' => {
                    self.position += 1;
                    self.push(TokenKind::RightParen, start);
                }
                b'[' => {
                    self.position += 1;
                    self.push(TokenKind::LeftBracket, start);
                }
                b']' => {
                    self.position += 1;
                    self.push(TokenKind::RightBracket, start);
                }
                b',' => {
                    self.position += 1;
                    self.push(TokenKind::Comma, start);
                }
                b'@' => {
                    self.position += 1;
                    self.push(TokenKind::At, start);
                }
                b'|' => {
                    self.position += 1;
                    self.push(TokenKind::Pipe, start);
                }
                b'+' => {
                    self.position += 1;
                    self.push(TokenKind::Plus, start);
                }
                b'-' => {
                    self.position += 1;
                    self.push(TokenKind::Minus, start);
                }
                b'=' => {
                    self.position += 1;
                    self.push(TokenKind::Equal, start);
                }
                b'!' => {
                    if self.peek_at(1) == Some(b'=') {
                        self.position += 2;
                        self.push(TokenKind::NotEqual, start);
                    } else {
                        return Err(Lexer::error(start, "'!' must be part of '!='"));
                    }
                }
                b'<' => {
                    if self.peek_at(1) == Some(b'=') {
                        self.position += 2;
                        self.push(TokenKind::LessEqual, start);
                    } else {
                        self.position += 1;
                        self.push(TokenKind::Less, start);
                    }
                }
                b'>' => {
                    if self.peek_at(1) == Some(b'=') {
                        self.position += 2;
                        self.push(TokenKind::GreaterEqual, start);
                    } else {
                        self.position += 1;
                        self.push(TokenKind::Greater, start);
                    }
                }
                b'/' => {
                    if self.peek_at(1) == Some(b'/') {
                        self.position += 2;
                        self.push(TokenKind::DoubleSlash, start);
                    } else {
                        self.position += 1;
                        self.push(TokenKind::Slash, start);
                    }
                }
                b':' => {
                    if self.peek_at(1) == Some(b':') {
                        self.position += 2;
                        self.push(TokenKind::ColonColon, start);
                    } else {
                        return Err(Lexer::error(start, "':' must be part of '::' or a prefixed name"));
                    }
                }
                b'*' => {
                    self.position += 1;
                    let kind = if self.previous_allows_operator() {
                        TokenKind::Multiply
                    } else {
                        TokenKind::Star
                    };
                    self.push(kind, start);
                }
                b'.' => {
                    if self.peek_at(1) == Some(b'.') {
                        self.position += 2;
                        self.push(TokenKind::DoubleDot, start);
                    } else if self.peek_at(1).is_some_and(|b| b.is_ascii_digit()) {
                        self.lex_number(start)?;
                    } else {
                        self.position += 1;
                        self.push(TokenKind::Dot, start);
                    }
                }
                b'\'' | b'"' => self.lex_literal(start)?,
                b'$' => self.lex_variable(start)?,
                c if c.is_ascii_digit() => self.lex_number(start)?,
                c if is_name_start(c) => self.lex_name(start),
                c => {
                    return Err(Lexer::error(
                        start,
                        format!("unexpected character {:?}", char::from(c)),
                    ))
                }
            }
        }
        Ok(self.tokens)
    }

    fn lex_literal(&mut self, start: usize) -> Result<(), LexError> {
        let quote = self.bytes[self.position];
        self.position += 1;
        let content_start = self.position;
        while let Some(c) = self.peek() {
            if c == quote {
                let text = self.input[content_start..self.position].to_string();
                self.position += 1;
                self.push(TokenKind::Literal(text), start);
                return Ok(());
            }
            self.position += 1;
        }
        Err(Lexer::error(start, "unterminated string literal"))
    }

    fn lex_number(&mut self, start: usize) -> Result<(), LexError> {
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.position += 1;
        }
        if self.peek() == Some(b'.') {
            self.position += 1;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.position += 1;
            }
        }
        let text = &self.input[start..self.position];
        let value = text
            .parse::<f64>()
            .map_err(|_| Lexer::error(start, format!("invalid number {text:?}")))?;
        self.push(TokenKind::Number(value), start);
        Ok(())
    }

    fn lex_variable(&mut self, start: usize) -> Result<(), LexError> {
        self.position += 1; // consume '$'
        let name_start = self.position;
        self.consume_qname();
        if self.position == name_start {
            return Err(Lexer::error(start, "'$' must be followed by a variable name"));
        }
        let name = self.input[name_start..self.position].to_string();
        self.push(TokenKind::Variable(name), start);
        Ok(())
    }

    /// Consumes an `NCName`, or a `prefix:local` pair.
    ///
    /// Stops before `::` so that an axis name is not swallowed as a prefix.
    fn consume_qname(&mut self) {
        if !self.peek().is_some_and(is_name_start) {
            return;
        }
        self.position += 1;
        while self.peek().is_some_and(is_name_char) {
            self.position += 1;
        }
        if self.peek() == Some(b':')
            && self.peek_at(1) != Some(b':')
            && self.peek_at(1).is_some_and(is_name_start)
        {
            self.position += 1;
            while self.peek().is_some_and(is_name_char) {
                self.position += 1;
            }
        }
    }

    /// Classifies a name token, applying XPath's three disambiguation rules.
    fn lex_name(&mut self, start: usize) {
        self.consume_qname();
        let name = self.input[start..self.position].to_string();

        // `prefix:*` is a node test, not a name followed by a multiply.
        if self.peek() == Some(b':') && self.peek_at(1) == Some(b'*') {
            self.position += 2;
            self.push(TokenKind::Name(format!("{name}:*")), start);
            return;
        }

        // Rule: two following characters are `::` -> axis name.
        if self.next_two_significant(self.position) == Some((b':', b':')) {
            self.push(TokenKind::AxisName(name), start);
            return;
        }

        // Rule: following character is `(` -> node type or function name.
        if self.next_significant(self.position) == Some(b'(') {
            let kind = if NODE_TYPES.contains(&name.as_str()) {
                TokenKind::NodeType(name)
            } else {
                TokenKind::FunctionName(name)
            };
            self.push(kind, start);
            return;
        }

        // Rule: in operator position, these names are operators.
        if self.previous_allows_operator() {
            let kind = match name.as_str() {
                "and" => Some(TokenKind::And),
                "or" => Some(TokenKind::Or),
                "div" => Some(TokenKind::Div),
                "mod" => Some(TokenKind::Mod),
                _ => None,
            };
            if let Some(kind) = kind {
                self.push(kind, start);
                return;
            }
        }

        self.push(TokenKind::Name(name), start);
    }
}

/// Whether a byte may start an XML name.
///
/// Restricted to the ASCII range plus every non-ASCII byte: full XML 1.0 name
/// character classification would need the Unicode tables, and accepting all
/// non-ASCII here errs toward accepting valid names rather than rejecting
/// them, with the XML parser doing the strict checking.
fn is_name_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c >= 0x80
}

/// Whether a byte may continue an XML name.
fn is_name_char(c: u8) -> bool {
    is_name_start(c) || c.is_ascii_digit() || c == b'-' || c == b'.'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(input: &str) -> Vec<TokenKind> {
        tokenize(input).unwrap().into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn star_is_a_wildcard_at_the_start() {
        assert_eq!(kinds("*"), vec![TokenKind::Star]);
    }

    #[test]
    fn star_is_multiply_after_a_name() {
        assert_eq!(
            kinds("a * b"),
            vec![
                TokenKind::Name("a".into()),
                TokenKind::Multiply,
                TokenKind::Name("b".into())
            ]
        );
    }

    #[test]
    fn star_is_a_wildcard_after_a_slash() {
        assert_eq!(kinds("a/*"), vec![
            TokenKind::Name("a".into()),
            TokenKind::Slash,
            TokenKind::Star
        ]);
    }

    #[test]
    fn div_is_a_name_in_name_position() {
        assert_eq!(kinds("div"), vec![TokenKind::Name("div".into())]);
    }

    #[test]
    fn div_is_an_operator_after_a_name() {
        assert_eq!(
            kinds("a div b"),
            vec![
                TokenKind::Name("a".into()),
                TokenKind::Div,
                TokenKind::Name("b".into())
            ]
        );
    }

    #[test]
    fn node_types_beat_function_names() {
        assert_eq!(kinds("text()"), vec![
            TokenKind::NodeType("text".into()),
            TokenKind::LeftParen,
            TokenKind::RightParen
        ]);
        assert_eq!(kinds("count(a)")[0], TokenKind::FunctionName("count".into()));
    }

    #[test]
    fn axis_names_are_recognised_before_the_double_colon() {
        assert_eq!(kinds("child::a")[0], TokenKind::AxisName("child".into()));
    }

    #[test]
    fn prefixed_names_stay_together() {
        assert_eq!(kinds("p:a"), vec![TokenKind::Name("p:a".into())]);
        assert_eq!(kinds("p:*"), vec![TokenKind::Name("p:*".into())]);
    }

    #[test]
    fn numbers_with_and_without_a_leading_digit() {
        assert_eq!(kinds("1"), vec![TokenKind::Number(1.0)]);
        assert_eq!(kinds(".5"), vec![TokenKind::Number(0.5)]);
        assert_eq!(kinds("1.5"), vec![TokenKind::Number(1.5)]);
    }

    #[test]
    fn dot_and_double_dot() {
        assert_eq!(kinds("."), vec![TokenKind::Dot]);
        assert_eq!(kinds(".."), vec![TokenKind::DoubleDot]);
    }

    #[test]
    fn literals_in_either_quote_style() {
        assert_eq!(kinds("'a'"), vec![TokenKind::Literal("a".into())]);
        assert_eq!(kinds("\"a'b\""), vec![TokenKind::Literal("a'b".into())]);
    }

    #[test]
    fn variables() {
        assert_eq!(kinds("$x"), vec![TokenKind::Variable("x".into())]);
        assert_eq!(kinds("$p:x"), vec![TokenKind::Variable("p:x".into())]);
    }

    #[test]
    fn comparison_operators() {
        assert_eq!(kinds("a<=b")[1], TokenKind::LessEqual);
        assert_eq!(kinds("a!=b")[1], TokenKind::NotEqual);
        assert_eq!(kinds("a>=b")[1], TokenKind::GreaterEqual);
    }

    #[test]
    fn unterminated_literal_is_an_error() {
        assert!(tokenize("'abc").is_err());
    }

    #[test]
    fn stray_characters_are_errors() {
        assert!(tokenize("a ; b").is_err());
        assert!(tokenize("a ! b").is_err());
    }
}
