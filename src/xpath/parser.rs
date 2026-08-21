//! Recursive-descent parser for XPath 1.0.
//!
//! The grammar is transcribed straight from the specification, one function
//! per production, so it can be checked against the standard by reading. The
//! only cleverness is in the [`lexer`](super::lexer), which has already
//! resolved XPath's context-sensitive token classes.

use super::ast::{
    Axis, BinaryOp, Expr, NameTest, NodeTest, PathExpr, PathStart, Quantifier, Step,
};
use super::lexer::{tokenize, Token, TokenKind};

/// The maximum nesting depth accepted while parsing.
///
/// Recursive descent on hostile input — a thousand nested parentheses — would
/// otherwise exhaust the stack. Exceeding this is an error, never a crash;
/// the `fuzz_xpath` target exists to keep that true.
///
/// The limit counts nested sub-expressions: parentheses, predicates,
/// function arguments, and unary minus. It does not count the length of a
/// location path, which is parsed iteratively, so `a/b/c/…` of any length is
/// fine. Sixty-four is far beyond any expression a person writes; the ceiling
/// is set by how much stack one nesting level costs in an unoptimised build,
/// where each level descends the whole precedence chain.
pub const MAX_RECURSION_DEPTH: usize = 64;

/// A parse failure, with the offset it occurred at.
///
/// The caller turns this into an [`Error::XPathSyntax`](crate::Error) with a
/// caret line pointing at `position`, once it knows which schema construct
/// the expression came from.
#[derive(Debug, Clone)]
pub struct ParseError {
    /// Byte offset into the expression.
    pub position: usize,
    /// What went wrong.
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at offset {}", self.message, self.position)
    }
}

impl std::error::Error for ParseError {}

/// Parses an XPath 1.0 expression.
///
/// # Errors
///
/// Returns a [`ParseError`] carrying the byte offset of the problem, which
/// the caller turns into an [`Error::XPathSyntax`](crate::Error::XPathSyntax)
/// with a caret line.
///
/// # Examples
///
/// ```
/// use schematron::xpath::parse;
///
/// assert!(parse("count(line[@qty > 0]) > 0").is_ok());
/// assert!(parse("count(line").is_err());
/// ```
pub fn parse(input: &str) -> Result<Expr, ParseError> {
    let tokens = tokenize(input).map_err(|e| ParseError {
        position: e.position,
        message: e.message,
    })?;
    let mut parser = Parser {
        tokens,
        index: 0,
        depth: 0,
        length: input.len(),
    };
    let expr = parser.parse_expr()?;
    if parser.index < parser.tokens.len() {
        let token = parser.tokens[parser.index].clone();
        return Err(ParseError {
            position: token.position,
            message: format!("unexpected {} after a complete expression", token.kind),
        });
    }
    Ok(expr)
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
    depth: usize,
    length: usize,
}

impl Parser {
    fn peek(&self) -> Option<&TokenKind> {
        self.tokens.get(self.index).map(|t| &t.kind)
    }

    fn position(&self) -> usize {
        self.tokens
            .get(self.index)
            .map_or(self.length, |t| t.position)
    }

    fn error<T>(&self, message: impl Into<String>) -> Result<T, ParseError> {
        Err(ParseError {
            position: self.position(),
            message: message.into(),
        })
    }

    fn advance(&mut self) -> Option<TokenKind> {
        let token = self.tokens.get(self.index).map(|t| t.kind.clone());
        if token.is_some() {
            self.index += 1;
        }
        token
    }

    /// Consumes a bare name token with this text, if it is next.
    ///
    /// XPath 2.0's keywords are not reserved words; `then` and `else` are
    /// ordinary names that only mean something in position.
    /// Whether the next token is this keyword.
    ///
    /// XPath 2.0's keywords — `in`, `return`, `satisfies`, `then`, `else`,
    /// `to` — are not reserved words, so the lexer classifies them by
    /// position like any other name. A keyword directly followed by `(`, as
    /// in `in (1 to 10)`, arrives as a function name; both spellings mean the
    /// keyword here.
    fn at_name(&self, text: &str) -> bool {
        matches!(
            self.peek(),
            Some(TokenKind::Name(name) | TokenKind::FunctionName(name)) if name == text
        )
    }

    fn eat_name(&mut self, text: &str) -> bool {
        if self.at_name(text) {
            self.index += 1;
            return true;
        }
        false
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.peek() == Some(kind) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: &TokenKind) -> Result<(), ParseError> {
        if self.eat(kind) {
            Ok(())
        } else {
            let found = self
                .peek()
                .map_or_else(|| "end of expression".to_string(), ToString::to_string);
            self.error(format!("expected {kind} but found {found}"))
        }
    }

    fn enter(&mut self) -> Result<(), ParseError> {
        self.depth += 1;
        if self.depth > MAX_RECURSION_DEPTH {
            return self.error(format!(
                "expression nested deeper than the limit of {MAX_RECURSION_DEPTH}"
            ));
        }
        Ok(())
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    /// `Expr := ExprSingle ("," ExprSingle)*`
    ///
    /// The comma builds an XPath 2.0 sequence. It is admitted only here —
    /// at the top level and inside parentheses — because function arguments
    /// and predicates take an `ExprSingle`, where a comma separates arguments
    /// rather than sequence members.
    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        let first = self.parse_expr_single()?;
        if self.peek() != Some(&TokenKind::Comma) {
            return Ok(first);
        }
        let mut members = vec![first];
        while self.eat(&TokenKind::Comma) {
            members.push(self.parse_expr_single()?);
        }
        Ok(Expr::Sequence(members))
    }

    /// `ExprSingle := ForExpr | QuantifiedExpr | IfExpr | OrExpr`
    fn parse_expr_single(&mut self) -> Result<Expr, ParseError> {
        self.enter()?;
        let expr = self.parse_expr_single_inner();
        self.leave();
        expr
    }

    fn parse_expr_single_inner(&mut self) -> Result<Expr, ParseError> {
        // `for` and `some`/`every` are ordinary names until a `$` follows
        // them, which is what tells them apart from an element called `for`.
        if let Some(TokenKind::Name(name) | TokenKind::FunctionName(name)) = self.peek().cloned() {
            let binds_a_variable =
                matches!(self.tokens.get(self.index + 1).map(|t| &t.kind), Some(TokenKind::Variable(_)));
            if binds_a_variable {
                match name.as_str() {
                    "for" => return self.parse_for(),
                    "some" => return self.parse_quantified(Quantifier::Some),
                    "every" => return self.parse_quantified(Quantifier::Every),
                    _ => {}
                }
            }
        }
        self.parse_or()
    }

    /// `for $v in ExprSingle return ExprSingle`
    fn parse_for(&mut self) -> Result<Expr, ParseError> {
        self.index += 1; // `for`
        let variable = self.expect_variable()?;
        if !self.eat_name("in") {
            return self.error("expected `in` after the variable of a `for` expression");
        }
        let input = self.parse_expr_single()?;
        if !self.eat_name("return") {
            return self.error("expected `return` after the sequence of a `for` expression");
        }
        let body = self.parse_expr_single()?;
        Ok(Expr::For {
            variable,
            input: Box::new(input),
            body: Box::new(body),
        })
    }

    /// `(some | every) $v in ExprSingle satisfies ExprSingle`
    fn parse_quantified(&mut self, quantifier: Quantifier) -> Result<Expr, ParseError> {
        self.index += 1; // `some` or `every`
        let variable = self.expect_variable()?;
        if !self.eat_name("in") {
            return self.error(format!(
                "expected `in` after the variable of a `{}` expression",
                quantifier.as_str()
            ));
        }
        let input = self.parse_expr_single()?;
        if !self.eat_name("satisfies") {
            return self.error(format!(
                "expected `satisfies` in a `{}` expression",
                quantifier.as_str()
            ));
        }
        let test = self.parse_expr_single()?;
        Ok(Expr::Quantified {
            quantifier,
            variable,
            input: Box::new(input),
            test: Box::new(test),
        })
    }

    fn expect_variable(&mut self) -> Result<NameTest, ParseError> {
        if let Some(TokenKind::Variable(name)) = self.advance() {
            return Ok(NameTest::parse(&name));
        }
        self.index = self.index.saturating_sub(1);
        self.error("expected a variable, written `$name`")
    }

    /// `OrExpr := AndExpr ('or' AndExpr)*`
    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and()?;
        while self.eat(&TokenKind::Or) {
            let right = self.parse_and()?;
            left = Expr::Binary(BinaryOp::Or, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    /// `AndExpr := EqualityExpr ('and' EqualityExpr)*`
    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_equality()?;
        while self.eat(&TokenKind::And) {
            let right = self.parse_equality()?;
            left = Expr::Binary(BinaryOp::And, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    /// `EqualityExpr := RelationalExpr (('=' | '!=') RelationalExpr)*`
    fn parse_equality(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_relational()?;
        loop {
            let op = match self.peek() {
                Some(TokenKind::Equal) => BinaryOp::Equal,
                Some(TokenKind::NotEqual) => BinaryOp::NotEqual,
                _ => break,
            };
            self.index += 1;
            let right = self.parse_relational()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    /// `RelationalExpr := RangeExpr (('<'|'>'|'<='|'>=') RangeExpr)*`
    fn parse_relational(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_range()?;
        loop {
            let op = match self.peek() {
                Some(TokenKind::Less) => BinaryOp::Less,
                Some(TokenKind::LessEqual) => BinaryOp::LessEqual,
                Some(TokenKind::Greater) => BinaryOp::Greater,
                Some(TokenKind::GreaterEqual) => BinaryOp::GreaterEqual,
                _ => break,
            };
            self.index += 1;
            let right = self.parse_range()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    /// `RangeExpr := AdditiveExpr ("to" AdditiveExpr)?`
    ///
    /// XPath 2.0 only. `to` is an ordinary name in XPath 1.0, and a 1.0
    /// binding rejects the resulting `Expr::Range` at compile time.
    fn parse_range(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_additive()?;
        if !self.at_name("to") {
            return Ok(left);
        }
        self.index += 1;
        let right = self.parse_additive()?;
        Ok(Expr::Range(Box::new(left), Box::new(right)))
    }

    /// `AdditiveExpr := MultiplicativeExpr (('+'|'-') MultiplicativeExpr)*`
    fn parse_additive(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                Some(TokenKind::Plus) => BinaryOp::Add,
                Some(TokenKind::Minus) => BinaryOp::Subtract,
                _ => break,
            };
            self.index += 1;
            let right = self.parse_multiplicative()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    /// `MultiplicativeExpr := UnaryExpr (('*'|'div'|'mod') UnaryExpr)*`
    fn parse_multiplicative(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Some(TokenKind::Multiply) => BinaryOp::Multiply,
                Some(TokenKind::Div) => BinaryOp::Divide,
                Some(TokenKind::Mod) => BinaryOp::Modulo,
                _ => break,
            };
            self.index += 1;
            let right = self.parse_unary()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    /// `UnaryExpr := '-'* UnionExpr`
    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if self.eat(&TokenKind::Minus) {
            self.enter()?;
            let operand = self.parse_unary();
            self.leave();
            return Ok(Expr::Negate(Box::new(operand?)));
        }
        self.parse_union()
    }

    /// `UnionExpr := PathExpr ('|' PathExpr)*`
    fn parse_union(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_path_expr()?;
        while self.eat(&TokenKind::Pipe) {
            let right = self.parse_path_expr()?;
            left = Expr::Binary(BinaryOp::Union, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    /// Whether the next token can begin a location path step.
    fn at_step_start(&self) -> bool {
        matches!(
            self.peek(),
            Some(
                TokenKind::Name(_)
                    | TokenKind::Star
                    | TokenKind::NodeType(_)
                    | TokenKind::AxisName(_)
                    | TokenKind::At
                    | TokenKind::Dot
                    | TokenKind::DoubleDot
            )
        )
    }

    /// `PathExpr := LocationPath | FilterExpr (('/'|'//') RelativeLocationPath)?`
    fn parse_path_expr(&mut self) -> Result<Expr, ParseError> {
        // An absolute path, or a relative path starting with a step.
        if matches!(self.peek(), Some(TokenKind::Slash | TokenKind::DoubleSlash)) {
            return self.parse_absolute_path();
        }
        if self.at_step_start() {
            let steps = self.parse_relative_path()?;
            return Ok(Expr::Path(Box::new(PathExpr {
                start: PathStart::Context,
                steps,
            })));
        }

        // Otherwise it starts with a primary expression: a literal, a number,
        // a variable, a parenthesised expression, or a function call.
        let primary = self.parse_primary()?;
        let predicates = self.parse_predicates()?;
        if matches!(self.peek(), Some(TokenKind::Slash | TokenKind::DoubleSlash)) {
            let steps = self.parse_path_tail()?;
            return Ok(Expr::Path(Box::new(PathExpr {
                start: PathStart::Expr(Box::new(primary), predicates),
                steps,
            })));
        }
        if predicates.is_empty() {
            Ok(primary)
        } else {
            Ok(Expr::Path(Box::new(PathExpr {
                start: PathStart::Expr(Box::new(primary), predicates),
                steps: Vec::new(),
            })))
        }
    }

    /// `'/' RelativeLocationPath? | '//' RelativeLocationPath`
    fn parse_absolute_path(&mut self) -> Result<Expr, ParseError> {
        let steps = if self.eat(&TokenKind::DoubleSlash) {
            let mut steps = vec![descendant_or_self_step()];
            steps.extend(self.parse_relative_path()?);
            steps
        } else {
            self.expect(&TokenKind::Slash)?;
            // A lone `/` is the root node itself.
            if self.at_step_start() {
                self.parse_relative_path()?
            } else {
                Vec::new()
            }
        };
        Ok(Expr::Path(Box::new(PathExpr {
            start: PathStart::Root,
            steps,
        })))
    }

    /// The `('/' Step)*` tail shared by paths that begin with an expression.
    fn parse_path_tail(&mut self) -> Result<Vec<Step>, ParseError> {
        let mut steps = Vec::new();
        loop {
            if self.eat(&TokenKind::DoubleSlash) {
                steps.push(descendant_or_self_step());
            } else if !self.eat(&TokenKind::Slash) {
                break;
            }
            steps.push(self.parse_step()?);
        }
        Ok(steps)
    }

    /// `RelativeLocationPath := Step (('/'|'//') Step)*`
    fn parse_relative_path(&mut self) -> Result<Vec<Step>, ParseError> {
        let mut steps = vec![self.parse_step()?];
        steps.extend(self.parse_path_tail()?);
        Ok(steps)
    }

    /// `Step := AxisSpecifier NodeTest Predicate* | '.' | '..'`
    fn parse_step(&mut self) -> Result<Step, ParseError> {
        if self.eat(&TokenKind::Dot) {
            return Ok(Step {
                axis: Axis::SelfAxis,
                node_test: NodeTest::AnyNode,
                predicates: Vec::new(),
            });
        }
        if self.eat(&TokenKind::DoubleDot) {
            return Ok(Step {
                axis: Axis::Parent,
                node_test: NodeTest::AnyNode,
                predicates: Vec::new(),
            });
        }

        let axis = if self.eat(&TokenKind::At) {
            Axis::Attribute
        } else if let Some(TokenKind::AxisName(name)) = self.peek().cloned() {
            self.index += 1;
            self.expect(&TokenKind::ColonColon)?;
            match Axis::from_name(&name) {
                Some(axis) => axis,
                None => return self.error(format!("unknown axis {name:?}")),
            }
        } else {
            Axis::Child
        };

        let node_test = self.parse_node_test(axis)?;
        let predicates = self.parse_predicates()?;
        Ok(Step {
            axis,
            node_test,
            predicates,
        })
    }

    fn parse_node_test(&mut self, axis: Axis) -> Result<NodeTest, ParseError> {
        match self.advance() {
            Some(TokenKind::Star) => Ok(NodeTest::Wildcard),
            Some(TokenKind::Name(name)) => {
                if let Some(prefix) = name.strip_suffix(":*") {
                    Ok(NodeTest::NamespaceWildcard(prefix.to_string()))
                } else {
                    Ok(NodeTest::Name(NameTest::parse(&name)))
                }
            }
            Some(TokenKind::NodeType(kind)) => {
                self.expect(&TokenKind::LeftParen)?;
                let test = match kind.as_str() {
                    "node" => NodeTest::AnyNode,
                    "text" => NodeTest::Text,
                    "comment" => NodeTest::Comment,
                    "processing-instruction" => {
                        if let Some(TokenKind::Literal(target)) = self.peek().cloned() {
                            self.index += 1;
                            NodeTest::ProcessingInstruction(Some(target))
                        } else {
                            NodeTest::ProcessingInstruction(None)
                        }
                    }
                    other => return self.error(format!("unknown node type {other}()")),
                };
                self.expect(&TokenKind::RightParen)?;
                Ok(test)
            }
            other => {
                // Rewind so the offset points at the offending token.
                self.index = self.index.saturating_sub(1);
                let found = other.map_or_else(
                    || "end of expression".to_string(),
                    |kind| kind.to_string(),
                );
                self.error(format!(
                    "expected a node test after {}:: but found {found}",
                    axis.as_str()
                ))
            }
        }
    }

    fn parse_predicates(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut predicates = Vec::new();
        while self.eat(&TokenKind::LeftBracket) {
            predicates.push(self.parse_expr_single()?);
            self.expect(&TokenKind::RightBracket)?;
        }
        Ok(predicates)
    }

    /// `PrimaryExpr := '$' QName | '(' Expr ')' | Literal | Number | FunctionCall`
    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        match self.advance() {
            Some(TokenKind::Variable(name)) => Ok(Expr::Variable(NameTest::parse(&name))),
            Some(TokenKind::Literal(text)) => Ok(Expr::Literal(text)),
            Some(TokenKind::Number(value)) => Ok(Expr::Number(value)),
            Some(TokenKind::LeftParen) => {
                // `()` is XPath 2.0's empty sequence, not an empty group.
                if self.eat(&TokenKind::RightParen) {
                    return Ok(Expr::Sequence(Vec::new()));
                }
                let inner = self.parse_expr()?;
                self.expect(&TokenKind::RightParen)?;
                Ok(inner)
            }
            Some(TokenKind::FunctionName(name)) if name == "if" => {
                // XPath 2.0's conditional. The lexer classifies `if` followed
                // by `(` as a function name, so the two are told apart by
                // what comes after the closing parenthesis: a `then` keyword
                // means a conditional, anything else means a call to a
                // function named `if` — which does not exist, and is reported
                // as such.
                self.expect(&TokenKind::LeftParen)?;
                let condition = self.parse_expr()?;
                self.expect(&TokenKind::RightParen)?;

                if !self.eat_name("then") {
                    return self.error(
                        "expected `then` after `if (…)`; XPath 2.0 conditionals are \
                         written `if (test) then value else value`",
                    );
                }
                let then_branch = self.parse_expr()?;
                if !self.eat_name("else") {
                    return self.error(
                        "expected `else`; an XPath 2.0 conditional must have both \
                         branches",
                    );
                }
                let else_branch = self.parse_expr()?;

                Ok(Expr::If {
                    condition: Box::new(condition),
                    then_branch: Box::new(then_branch),
                    else_branch: Box::new(else_branch),
                })
            }
            Some(TokenKind::FunctionName(name)) => {
                self.expect(&TokenKind::LeftParen)?;
                let mut args = Vec::new();
                if !self.eat(&TokenKind::RightParen) {
                    loop {
                        args.push(self.parse_expr_single()?);
                        if self.eat(&TokenKind::Comma) {
                            continue;
                        }
                        self.expect(&TokenKind::RightParen)?;
                        break;
                    }
                }
                Ok(Expr::Function { name, args })
            }
            other => {
                self.index = self.index.saturating_sub(1);
                let found = other.map_or_else(
                    || "end of expression".to_string(),
                    |kind| kind.to_string(),
                );
                self.error(format!("expected an expression but found {found}"))
            }
        }
    }
}

/// The step that `//` abbreviates: `descendant-or-self::node()`.
fn descendant_or_self_step() -> Step {
    Step {
        axis: Axis::DescendantOrSelf,
        node_test: NodeTest::AnyNode,
        predicates: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(input: &str) -> PathExpr {
        match parse(input).unwrap() {
            Expr::Path(p) => *p,
            other => panic!("expected a path, got {other:?}"),
        }
    }

    #[test]
    fn parses_a_simple_name_step() {
        let p = path("a");
        assert_eq!(p.start, PathStart::Context);
        assert_eq!(p.steps.len(), 1);
        assert_eq!(p.steps[0].axis, Axis::Child);
    }

    #[test]
    fn parses_an_absolute_path() {
        let p = path("/a/b");
        assert_eq!(p.start, PathStart::Root);
        assert_eq!(p.steps.len(), 2);
    }

    #[test]
    fn lone_slash_is_the_root_with_no_steps() {
        let p = path("/");
        assert_eq!(p.start, PathStart::Root);
        assert!(p.steps.is_empty());
    }

    #[test]
    fn double_slash_expands_to_descendant_or_self() {
        let p = path("//a");
        assert_eq!(p.steps[0].axis, Axis::DescendantOrSelf);
        assert_eq!(p.steps[0].node_test, NodeTest::AnyNode);
        assert_eq!(p.steps[1].axis, Axis::Child);
    }

    #[test]
    fn at_abbreviates_the_attribute_axis() {
        assert_eq!(path("@x").steps[0].axis, Axis::Attribute);
    }

    #[test]
    fn dot_and_double_dot_abbreviate_self_and_parent() {
        assert_eq!(path(".").steps[0].axis, Axis::SelfAxis);
        assert_eq!(path("..").steps[0].axis, Axis::Parent);
    }

    #[test]
    fn parses_explicit_axes() {
        assert_eq!(path("ancestor::a").steps[0].axis, Axis::Ancestor);
        assert_eq!(
            path("preceding-sibling::a").steps[0].axis,
            Axis::PrecedingSibling
        );
    }

    #[test]
    fn parses_node_type_tests() {
        assert_eq!(path("text()").steps[0].node_test, NodeTest::Text);
        assert_eq!(path("comment()").steps[0].node_test, NodeTest::Comment);
        assert_eq!(
            path("processing-instruction('t')").steps[0].node_test,
            NodeTest::ProcessingInstruction(Some("t".into()))
        );
    }

    #[test]
    fn parses_predicates_in_order() {
        let p = path("a[1][@x]");
        assert_eq!(p.steps[0].predicates.len(), 2);
    }

    #[test]
    fn respects_operator_precedence() {
        // `1 + 2 * 3` must group as `1 + (2 * 3)`.
        let expr = parse("1 + 2 * 3").unwrap();
        match expr {
            Expr::Binary(BinaryOp::Add, _, right) => {
                assert!(matches!(*right, Expr::Binary(BinaryOp::Multiply, _, _)));
            }
            other => panic!("unexpected shape: {other:?}"),
        }
    }

    #[test]
    fn or_binds_looser_than_and() {
        let expr = parse("a or b and c").unwrap();
        match expr {
            Expr::Binary(BinaryOp::Or, _, right) => {
                assert!(matches!(*right, Expr::Binary(BinaryOp::And, _, _)));
            }
            other => panic!("unexpected shape: {other:?}"),
        }
    }

    #[test]
    fn parses_function_calls_with_arguments() {
        let expr = parse("concat('a', 'b', 'c')").unwrap();
        match expr {
            Expr::Function { name, args } => {
                assert_eq!(name, "concat");
                assert_eq!(args.len(), 3);
            }
            other => panic!("unexpected shape: {other:?}"),
        }
    }

    #[test]
    fn parses_a_filter_expression_followed_by_a_path() {
        let p = path("id('x')/a");
        assert!(matches!(p.start, PathStart::Expr(_, _)));
        assert_eq!(p.steps.len(), 1);
    }

    #[test]
    fn parses_unions() {
        assert!(matches!(
            parse("a | b").unwrap(),
            Expr::Binary(BinaryOp::Union, _, _)
        ));
    }

    #[test]
    fn rejects_trailing_junk() {
        assert!(parse("a b").is_err());
    }

    #[test]
    fn rejects_unbalanced_parentheses() {
        assert!(parse("count(a").is_err());
        assert!(parse("(a").is_err());
        assert!(parse("a[1").is_err());
    }

    #[test]
    fn reports_the_offset_of_the_problem() {
        let e = parse("count(a").unwrap_err();
        assert_eq!(e.position, 7);
    }

    #[test]
    fn refuses_absurd_nesting_instead_of_overflowing() {
        let deep = format!("{}a{}", "(".repeat(5000), ")".repeat(5000));
        let error = parse(&deep).unwrap_err();
        assert!(error.message.contains("nested deeper"), "{}", error.message);
    }

    #[test]
    fn accepts_nesting_a_person_might_actually_write() {
        let depth = MAX_RECURSION_DEPTH / 2;
        let nested = format!("{}a{}", "(".repeat(depth), ")".repeat(depth));
        assert!(parse(&nested).is_ok());
    }

    #[test]
    fn long_location_paths_do_not_count_as_nesting() {
        // Paths are parsed iteratively, so length is not depth.
        let long = (0..500).map(|_| "a").collect::<Vec<_>>().join("/");
        assert!(parse(&long).is_ok());
    }
}
