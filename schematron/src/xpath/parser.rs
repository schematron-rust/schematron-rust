//! Recursive-descent parser for XPath 1.0.
//!
//! The grammar is transcribed straight from the specification, one function
//! per production, so it can be checked against the standard by reading. The
//! only cleverness is in the [`lexer`](super::lexer), which has already
//! resolved XPath's context-sensitive token classes.

use super::ast::{
    Axis, BinaryOp, Expr, ItemType, NameTest, NodeTest, Occurrence, PathExpr, PathStart,
    Quantifier, SequenceType, Step, TypeOp,
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
        // `for`, `let`, and `some`/`every` are ordinary names until a `$`
        // follows them, which is what tells them apart from an element
        // called `for`.
        if let Some(TokenKind::Name(name) | TokenKind::FunctionName(name)) = self.peek().cloned() {
            let binds_a_variable =
                matches!(self.tokens.get(self.index + 1).map(|t| &t.kind), Some(TokenKind::Variable(_)));
            if binds_a_variable {
                match name.as_str() {
                    "for" => return self.parse_for(),
                    "let" => return self.parse_let(),
                    "some" => return self.parse_quantified(Quantifier::Some),
                    "every" => return self.parse_quantified(Quantifier::Every),
                    _ => {}
                }
            }
        }
        self.parse_or()
    }

    /// `let $v := ExprSingle return ExprSingle`
    ///
    /// XPath 3.0 only; a 1.0/2.0 binding rejects the resulting `Expr::Let`
    /// at compile time. See `Expr::Let`'s doc comment for how this differs
    /// from `for`.
    fn parse_let(&mut self) -> Result<Expr, ParseError> {
        self.index += 1; // `let`
        let variable = self.expect_variable()?;
        if !self.eat(&TokenKind::Assign) {
            return self.error("expected `:=` after the variable of a `let` expression");
        }
        let value = self.parse_expr_single()?;
        if !self.eat_name("return") {
            return self.error("expected `return` after the value of a `let` expression");
        }
        let body = self.parse_expr_single()?;
        Ok(Expr::Let {
            variable,
            value: Box::new(value),
            body: Box::new(body),
        })
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

    /// A left-associative chain of one precedence level: `Operand (Op
    /// Operand)*`.
    ///
    /// Shared by every repeating binary-operator production in this
    /// grammar (`or`, `and`, the comparisons, `||`, `+`/`-`, `*`/`div`/
    /// `mod`, and `|`), so each counts its chain length against the same
    /// recursion budget a dynamic call's or arrow's `(args)(args)…` chain
    /// already does — see `parse_path_expr`'s and `parse_arrow`'s doc
    /// comments. It has to: each repetition nests one more `Expr::Binary`
    /// inside the last, and `evaluate_binary` unwraps that recursively, so
    /// — unlike a location path's steps, which `evaluate_path` walks in a
    /// plain loop and are therefore exempt — an unbounded chain here is a
    /// stack-overflow risk at evaluation time, not merely a parse-time
    /// one. Found by fuzzing `fuzz_xpath` on nothing more exotic than a
    /// few hundred `|`s in a row.
    fn parse_binary_chain(
        &mut self,
        operand: fn(&mut Self) -> Result<Expr, ParseError>,
        op_of: fn(&TokenKind) -> Option<BinaryOp>,
    ) -> Result<Expr, ParseError> {
        let mut left = operand(self)?;
        let mut chained = 0usize;
        while let Some(op) = self.peek().and_then(op_of) {
            self.index += 1;
            self.enter()?;
            chained += 1;
            match operand(self) {
                Ok(right) => left = Expr::Binary(op, Box::new(left), Box::new(right)),
                Err(error) => {
                    for _ in 0..chained {
                        self.leave();
                    }
                    return Err(error);
                }
            }
        }
        for _ in 0..chained {
            self.leave();
        }
        Ok(left)
    }

    /// `OrExpr := AndExpr ('or' AndExpr)*`
    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary_chain(Self::parse_and, |kind| {
            matches!(kind, TokenKind::Or).then_some(BinaryOp::Or)
        })
    }

    /// `AndExpr := EqualityExpr ('and' EqualityExpr)*`
    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary_chain(Self::parse_equality, |kind| {
            matches!(kind, TokenKind::And).then_some(BinaryOp::And)
        })
    }

    /// `EqualityExpr := RelationalExpr (('=' | '!=') RelationalExpr)*`
    fn parse_equality(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary_chain(Self::parse_relational, |kind| match kind {
            TokenKind::Equal => Some(BinaryOp::Equal),
            TokenKind::NotEqual => Some(BinaryOp::NotEqual),
            TokenKind::ValueEqual => Some(BinaryOp::ValueEqual),
            TokenKind::ValueNotEqual => Some(BinaryOp::ValueNotEqual),
            TokenKind::NodeIs => Some(BinaryOp::NodeIs),
            TokenKind::NodeBefore => Some(BinaryOp::NodeBefore),
            TokenKind::NodeAfter => Some(BinaryOp::NodeAfter),
            _ => None,
        })
    }

    /// `RelationalExpr := StringConcatExpr (('<'|'>'|'<='|'>=') StringConcatExpr)*`
    fn parse_relational(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary_chain(Self::parse_concat, |kind| match kind {
            TokenKind::Less => Some(BinaryOp::Less),
            TokenKind::LessEqual => Some(BinaryOp::LessEqual),
            TokenKind::Greater => Some(BinaryOp::Greater),
            TokenKind::GreaterEqual => Some(BinaryOp::GreaterEqual),
            TokenKind::ValueLess => Some(BinaryOp::ValueLess),
            TokenKind::ValueLessEqual => Some(BinaryOp::ValueLessEqual),
            TokenKind::ValueGreater => Some(BinaryOp::ValueGreater),
            TokenKind::ValueGreaterEqual => Some(BinaryOp::ValueGreaterEqual),
            _ => None,
        })
    }

    /// `StringConcatExpr := RangeExpr ("||" RangeExpr)*`
    ///
    /// XPath 3.0 only; a 1.0/2.0 binding rejects the resulting
    /// `BinaryOp::Concat` at compile time, same as every other 3.0-only
    /// construct.
    fn parse_concat(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary_chain(Self::parse_range, |kind| {
            matches!(kind, TokenKind::DoublePipe).then_some(BinaryOp::Concat)
        })
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
        self.parse_binary_chain(Self::parse_multiplicative, |kind| match kind {
            TokenKind::Plus => Some(BinaryOp::Add),
            TokenKind::Minus => Some(BinaryOp::Subtract),
            _ => None,
        })
    }

    /// `MultiplicativeExpr := UnaryExpr (('*'|'div'|'mod') UnaryExpr)*`
    fn parse_multiplicative(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary_chain(Self::parse_unary, |kind| match kind {
            TokenKind::Multiply => Some(BinaryOp::Multiply),
            TokenKind::Div => Some(BinaryOp::Divide),
            TokenKind::Mod => Some(BinaryOp::Modulo),
            _ => None,
        })
    }

    /// `UnaryExpr := '-'* TypeExpr`
    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if self.eat(&TokenKind::Minus) {
            self.enter()?;
            let operand = self.parse_unary();
            self.leave();
            return Ok(Expr::Negate(Box::new(operand?)));
        }
        self.parse_type_operators()
    }

    /// The XPath 2.0 type operators, innermost binding first.
    ///
    /// `cast as` binds tightest and `instance of` loosest, which is the order
    /// XPath 2.0 gives them, so `$x cast as xs:string instance of xs:string`
    /// means what it reads as.
    fn parse_type_operators(&mut self) -> Result<Expr, ParseError> {
        let mut value = self.parse_arrow()?;

        // `cast as` and `castable as` take a single type; the other two take
        // a sequence type.
        for (first, second, op) in [
            ("cast", "as", TypeOp::CastAs),
            ("castable", "as", TypeOp::CastableAs),
            ("treat", "as", TypeOp::TreatAs),
            ("instance", "of", TypeOp::InstanceOf),
        ] {
            if !self.at_name(first) {
                continue;
            }
            // The keyword is only a keyword when its partner follows; `cast`
            // on its own is an ordinary name.
            let partner = self
                .tokens
                .get(self.index + 1)
                .map(|token| &token.kind);
            let follows = matches!(
                partner,
                Some(TokenKind::Name(name) | TokenKind::FunctionName(name)) if name == second
            );
            if !follows {
                continue;
            }
            self.index += 2;
            let sequence_type = self.parse_sequence_type(op)?;
            value = Expr::TypeOp {
                op,
                value: Box::new(value),
                sequence_type,
            };
        }
        Ok(value)
    }

    /// `SequenceType := 'empty-sequence()' | ItemType OccurrenceIndicator?`
    fn parse_sequence_type(&mut self, op: TypeOp) -> Result<SequenceType, ParseError> {
        let item_type = self.parse_item_type()?;

        if item_type == ItemType::EmptySequence {
            return Ok(SequenceType {
                item_type,
                occurrence: Occurrence::ZeroOrMore,
            });
        }

        let occurrence = if self.eat(&TokenKind::QuestionMark) {
            Occurrence::ZeroOrOne
        } else if op.takes_single_type() {
            // `cast as` and `castable as` admit only `?`.
            Occurrence::One
        } else if self.eat(&TokenKind::Star) || self.eat(&TokenKind::Multiply) {
            Occurrence::ZeroOrMore
        } else if self.eat(&TokenKind::Plus) {
            Occurrence::OneOrMore
        } else {
            Occurrence::One
        };

        if op.takes_single_type() && !matches!(item_type, ItemType::Atomic(_)) {
            return self.error(format!(
                "`{}` takes an atomic type such as xs:date, because casting a node \
                 or a sequence has no meaning",
                op.as_str()
            ));
        }

        Ok(SequenceType {
            item_type,
            occurrence,
        })
    }

    /// `ItemType := KindTest | 'item()' | AtomicType`
    fn parse_item_type(&mut self) -> Result<ItemType, ParseError> {
        use crate::xml::NodeKind;

        match self.advance() {
            // A kind test: a node type name followed by parentheses.
            Some(TokenKind::NodeType(name) | TokenKind::FunctionName(name)) => {
                self.expect(&TokenKind::LeftParen)?;
                let kind = match name.as_str() {
                    "item" => {
                        self.expect(&TokenKind::RightParen)?;
                        return Ok(ItemType::AnyItem);
                    }
                    "empty-sequence" => {
                        self.expect(&TokenKind::RightParen)?;
                        return Ok(ItemType::EmptySequence);
                    }
                    "node" => None,
                    "element" => Some(NodeKind::Element),
                    "attribute" => Some(NodeKind::Attribute),
                    "text" => Some(NodeKind::Text),
                    "comment" => Some(NodeKind::Comment),
                    "processing-instruction" => Some(NodeKind::ProcessingInstruction),
                    "document-node" => Some(NodeKind::Root),
                    "namespace-node" => Some(NodeKind::Namespace),
                    other => {
                        return self.error(format!("{other}() is not a known item type"))
                    }
                };
                // `element(name)` and `attribute(name)` narrow by name.
                let named = if let Some(TokenKind::Name(name)) = self.peek().cloned() {
                    self.index += 1;
                    Some(NameTest::parse(&name))
                } else {
                    None
                };
                self.expect(&TokenKind::RightParen)?;
                Ok(ItemType::Node { kind, name: named })
            }

            // An atomic type name, such as `xs:date`.
            Some(TokenKind::Name(name)) => Ok(ItemType::Atomic(name)),

            other => {
                self.index = self.index.saturating_sub(1);
                let found = other
                    .map_or_else(|| "end of expression".to_string(), |kind| kind.to_string());
                self.error(format!("expected a type but found {found}"))
            }
        }
    }

    /// `ArrowExpr := UnionExpr ("=>" ArrowFunctionSpecifier ArgumentList)*`
    ///
    /// XPath 3.0 only; a 1.0/2.0 binding rejects the resulting `Expr::Arrow`
    /// at compile time. Each `=>` pipes the value built so far in as the
    /// call's first argument, ahead of whatever `ArgumentList` supplies —
    /// see `parse_arrow_call`.
    ///
    /// Chained the same way a dynamic call's trailing `(args)(args)…` is
    /// (see `parse_path_expr`): each link is charged against the shared
    /// recursion depth, released only once the whole chain is done, so an
    /// unbounded chain of `=>` is a parse error rather than a risk at
    /// evaluation time.
    fn parse_arrow(&mut self) -> Result<Expr, ParseError> {
        let mut value = self.parse_union()?;
        let mut chained = 0usize;
        while self.peek() == Some(&TokenKind::Arrow) {
            self.index += 1; // `=>`
            self.enter()?;
            chained += 1;
            match self.parse_arrow_call(value) {
                Ok(called) => value = Expr::Arrow(Box::new(called)),
                Err(error) => {
                    for _ in 0..chained {
                        self.leave();
                    }
                    return Err(error);
                }
            }
        }
        for _ in 0..chained {
            self.leave();
        }
        Ok(value)
    }

    /// `ArrowFunctionSpecifier ArgumentList`, with `input` prepended as the
    /// call's first argument.
    ///
    /// `ArrowFunctionSpecifier := EQName | VarRef | ParenthesizedExpr`. A
    /// bare `EQName` always arrives as a `FunctionName` token here, never a
    /// plain `Name` — the lexer classifies any name immediately followed by
    /// `(` that way, and an `ArrowFunctionSpecifier` is always followed by
    /// one.
    fn parse_arrow_call(&mut self, input: Expr) -> Result<Expr, ParseError> {
        match self.advance() {
            Some(TokenKind::FunctionName(name)) => {
                let mut args = vec![input];
                args.extend(self.parse_arguments()?);
                Ok(Expr::Function { name, args })
            }
            Some(TokenKind::Variable(name)) => {
                let target = Expr::Variable(NameTest::parse(&name));
                let mut args = vec![input];
                args.extend(self.parse_arguments()?);
                Ok(Expr::DynamicCall {
                    function: Box::new(target),
                    args,
                })
            }
            Some(TokenKind::LeftParen) => {
                let target = self.parse_expr()?;
                self.expect(&TokenKind::RightParen)?;
                let mut args = vec![input];
                args.extend(self.parse_arguments()?);
                Ok(Expr::DynamicCall {
                    function: Box::new(target),
                    args,
                })
            }
            other => {
                self.index = self.index.saturating_sub(1);
                let found = other.map_or_else(
                    || "end of expression".to_string(),
                    |kind| kind.to_string(),
                );
                self.error(format!(
                    "expected a function name, a variable, or a parenthesized \
                     expression after '=>', but found {found}"
                ))
            }
        }
    }

    /// `UnionExpr := PathExpr ('|' PathExpr)*`
    fn parse_union(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary_chain(Self::parse_simple_map, |kind| {
            matches!(kind, TokenKind::Pipe).then_some(BinaryOp::Union)
        })
    }

    /// `SimpleMapExpr := PathExpr ("!" PathExpr)*`
    ///
    /// XPath 3.0 only; a 1.0/2.0 binding rejects the resulting
    /// `BinaryOp::SimpleMap` at compile time, same as every other 3.0-only
    /// construct. Binds tighter than `|` — real XPath 3.0's grammar nests
    /// `UnionExpr` around `SimpleMapExpr`, not the other way round, so
    /// `a ! b | c` groups as `(a ! b) | c`.
    fn parse_simple_map(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary_chain(Self::parse_path_expr, |kind| {
            matches!(kind, TokenKind::Bang).then_some(BinaryOp::SimpleMap)
        })
    }

    /// Whether the next token can begin a location path step.
    fn at_step_start(&self) -> bool {
        // `name#N` is XPath 3.0's named function reference — a primary
        // expression, not a location step — even though a bare name alone
        // always starts one.
        if matches!(self.peek(), Some(TokenKind::Name(_)))
            && matches!(
                self.tokens.get(self.index + 1).map(|t| &t.kind),
                Some(TokenKind::Hash)
            )
        {
            return false;
        }
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
        let mut primary = self.parse_primary()?;

        // XPath 3.0's dynamic call: `(args)` directly after a primary,
        // chainable (`$f(1)(2)`). This is new syntax with no prior meaning —
        // a name directly followed by `(` was always a `FunctionName` token,
        // never reaching here — so a 1.0 or 2.0 binding rejects the
        // resulting `Expr::DynamicCall` at compile time rather than the
        // parser needing to know the version.
        //
        // Unlike a location path's steps, which `evaluate_path` walks in a
        // plain loop, each chained call nests one `Expr::DynamicCall` inside
        // the last and `evaluate` unwraps that recursively — so, unlike a
        // path of any length, an unbounded chain here would risk a stack
        // overflow at evaluation time rather than a clean parse error. Each
        // call in the chain is charged against the shared recursion depth,
        // released only once the whole chain is done, so the chain's total
        // length is bounded exactly like any other nesting.
        let mut chained = 0usize;
        while self.peek() == Some(&TokenKind::LeftParen) {
            self.enter()?;
            chained += 1;
            let args = match self.parse_arguments() {
                Ok(args) => args,
                Err(error) => {
                    for _ in 0..chained {
                        self.leave();
                    }
                    return Err(error);
                }
            };
            primary = Expr::DynamicCall {
                function: Box::new(primary),
                args,
            };
        }
        for _ in 0..chained {
            self.leave();
        }

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

        let mut written = true;
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
            written = false;
            Axis::Child
        };

        let node_test = self.parse_node_test(axis)?;

        // XPath 2.0 section 3.2.1.1: a step whose node test is an attribute
        // kind test defaults to the attribute axis, not the child axis. So
        // `b/attribute()` means `b/attribute::attribute()` and selects b's
        // attributes — otherwise the test could never match anything, since
        // the child axis yields no attributes at all.
        let axis = if !written
            && matches!(
                node_test,
                NodeTest::Kind {
                    kind: crate::xml::NodeKind::Attribute,
                    ..
                }
            ) {
            Axis::Attribute
        } else {
            axis
        };

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
                    // XPath 2.0 kind tests. A name may be given, and `*`
                    // means the same as no name at all.
                    "element" | "attribute" | "document-node" => {
                        let node_kind = match kind.as_str() {
                            "element" => crate::xml::NodeKind::Element,
                            "attribute" => crate::xml::NodeKind::Attribute,
                            _ => crate::xml::NodeKind::Root,
                        };
                        let mut name = None;
                        if node_kind != crate::xml::NodeKind::Root {
                            match self.peek().cloned() {
                                Some(TokenKind::Star) => self.index += 1,
                                Some(TokenKind::Name(written)) => {
                                    self.index += 1;
                                    name = Some(NameTest::parse(&written));
                                }
                                _ => {}
                            }
                        }
                        NodeTest::Kind {
                            kind: node_kind,
                            name,
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
            Some(TokenKind::Number(value, numeric_type)) => Ok(Expr::Number(value, numeric_type)),
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
            Some(TokenKind::FunctionName(name)) if name == "function" => {
                self.parse_inline_function()
            }
            Some(TokenKind::FunctionName(name)) => {
                let args = self.parse_arguments()?;
                Ok(Expr::Function { name, args })
            }
            // XPath 3.0's named function reference: `name#arity`. Reached
            // only when `at_step_start` has already ruled out this being a
            // location step — see its doc comment.
            Some(TokenKind::Name(name)) if self.peek() == Some(&TokenKind::Hash) => {
                self.index += 1; // `#`
                match self.advance() {
                    Some(TokenKind::Number(value, super::value::NumericType::Integer))
                        if value >= 0.0 =>
                    {
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        Ok(Expr::NamedFunctionRef {
                            name,
                            arity: value as usize,
                        })
                    }
                    other => {
                        self.index = self.index.saturating_sub(1);
                        let found = other.map_or_else(
                            || "end of expression".to_string(),
                            |kind| kind.to_string(),
                        );
                        self.error(format!(
                            "expected a non-negative integer arity after `{name}#`, \
                             but found {found}"
                        ))
                    }
                }
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

    /// `ArgumentList := "(" (ExprSingle ("," ExprSingle)*)? ")"`
    ///
    /// Shared by an ordinary function call, a dynamic call, and — indirectly,
    /// since it takes the same shape — nothing else, but kept as its own
    /// production because two call sites need it identically.
    fn parse_arguments(&mut self) -> Result<Vec<Expr>, ParseError> {
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
        Ok(args)
    }

    /// `InlineFunctionExpr := "function" "(" ParamList? ")" EnclosedExpr`
    ///
    /// `EnclosedExpr := "{" Expr? "}"`. Neither a parameter's nor the
    /// function's own return type may be annotated in this phase — see the
    /// doc comment on `Expr::InlineFunction`.
    fn parse_inline_function(&mut self) -> Result<Expr, ParseError> {
        // The `function` token itself was already consumed by `parse_primary`'s
        // `self.advance()`, which is how it dispatched here.
        self.expect(&TokenKind::LeftParen)?;
        let mut params = Vec::new();
        if !self.eat(&TokenKind::RightParen) {
            loop {
                params.push(self.expect_variable()?.to_string());
                if self.eat(&TokenKind::Comma) {
                    continue;
                }
                self.expect(&TokenKind::RightParen)?;
                break;
            }
        }
        self.expect(&TokenKind::LeftBrace)?;
        self.enter()?;
        let body = if self.peek() == Some(&TokenKind::RightBrace) {
            Expr::Sequence(Vec::new())
        } else {
            self.parse_expr()?
        };
        self.leave();
        self.expect(&TokenKind::RightBrace)?;
        Ok(Expr::InlineFunction {
            params,
            body: std::sync::Arc::new(body),
        })
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

    #[test]
    fn a_long_chain_of_dynamic_calls_is_bounded_unlike_a_path() {
        // Unlike a path's steps, each chained call nests one `DynamicCall`
        // inside the last, and `evaluate` unwraps that recursively — so,
        // unlike `long_location_paths_do_not_count_as_nesting` above, this
        // one genuinely must count, or a long enough chain would risk a
        // stack overflow at evaluation time instead of a clean parse error.
        let chain = format!("a{}", "()".repeat(5000));
        let error = parse(&chain).unwrap_err();
        assert!(error.message.contains("nested deeper"), "{}", error.message);

        let shallow = format!("a{}", "()".repeat(MAX_RECURSION_DEPTH / 2));
        assert!(parse(&shallow).is_ok());
    }

    #[test]
    fn a_long_chain_of_any_repeating_binary_operator_is_bounded() {
        // Found by fuzzing `fuzz_xpath`: a few hundred `|` in a row parsed
        // fine (the loop in `parse_union` counted no depth at all) and then
        // overflowed the stack in `evaluate`, which unwraps the resulting
        // left-degenerate `Expr::Binary` tree recursively — the same risk
        // `a_long_chain_of_dynamic_calls_is_bounded_unlike_a_path` above
        // already guards for `(args)(args)…`, just not yet extended to
        // `or`, `and`, the comparisons, `||`, `+`/`-`, `*`/`div`/`mod`, `|`,
        // and (added when `!` itself was written, sharing the same
        // `parse_binary_chain`) `!`. One fix covers all nine.
        for operator in ["or", "and", "=", "<", "||", "+", "*", "|", "!"] {
            let chain = format!("a{}", format!(" {operator} a").repeat(5000));
            let error = parse(&chain).unwrap_err();
            assert!(
                error.message.contains("nested deeper"),
                "operator {operator:?}: {}",
                error.message
            );

            let shallow = format!("a{}", format!(" {operator} a").repeat(MAX_RECURSION_DEPTH / 2));
            assert!(parse(&shallow).is_ok(), "operator {operator:?}");
        }
    }

    #[test]
    fn parses_string_concatenation() {
        assert!(matches!(
            parse("'a' || 'b'").unwrap(),
            Expr::Binary(BinaryOp::Concat, _, _)
        ));
    }

    #[test]
    fn string_concat_binds_tighter_than_equality_but_looser_than_additive() {
        // `'a' || 'b' = 'ab'` must group as `('a' || 'b') = 'ab'` — `||`
        // binds inside `=`.
        let expr = parse("'a' || 'b' = 'ab'").unwrap();
        match expr {
            Expr::Binary(BinaryOp::Equal, left, _) => {
                assert!(matches!(*left, Expr::Binary(BinaryOp::Concat, _, _)));
            }
            other => panic!("unexpected shape: {other:?}"),
        }

        // `1 || 2 + 3` must group as `1 || (2 + 3)` — additive binds inside
        // `||`.
        let expr = parse("1 || 2 + 3").unwrap();
        match expr {
            Expr::Binary(BinaryOp::Concat, _, right) => {
                assert!(matches!(*right, Expr::Binary(BinaryOp::Add, _, _)));
            }
            other => panic!("unexpected shape: {other:?}"),
        }
    }

    #[test]
    fn parses_an_arrow_call_to_a_named_function() {
        // `$x => f(1)` is sugar for `f($x, 1)`.
        let expr = parse("$x => f(1)").unwrap();
        match expr {
            Expr::Arrow(called) => match *called {
                Expr::Function { name, args } => {
                    assert_eq!(name, "f");
                    assert_eq!(args.len(), 2);
                    assert!(matches!(args[0], Expr::Variable(_)));
                }
                other => panic!("unexpected call shape: {other:?}"),
            },
            other => panic!("unexpected shape: {other:?}"),
        }
    }

    #[test]
    fn parses_an_arrow_call_to_a_dynamic_target() {
        for source in ["$x => $f(1)", "$x => (g#1)(1)"] {
            let expr = parse(source).unwrap();
            match expr {
                Expr::Arrow(called) => {
                    assert!(matches!(*called, Expr::DynamicCall { .. }), "{source}");
                }
                other => panic!("unexpected shape for {source}: {other:?}"),
            }
        }
    }

    #[test]
    fn arrow_calls_chain() {
        // `$x => f() => g()` is sugar for `g(f($x))`.
        let expr = parse("$x => f() => g()").unwrap();
        match expr {
            Expr::Arrow(outer) => match *outer {
                Expr::Function { name, args } => {
                    assert_eq!(name, "g");
                    assert!(matches!(args[0], Expr::Arrow(_)));
                }
                other => panic!("unexpected shape: {other:?}"),
            },
            other => panic!("unexpected shape: {other:?}"),
        }
    }

    #[test]
    fn cast_as_binds_looser_than_arrow() {
        // `$x => f() cast as xs:string` must group as
        // `($x => f()) cast as xs:string`.
        let expr = parse("$x => f() cast as xs:string").unwrap();
        match expr {
            Expr::TypeOp { value, .. } => {
                assert!(matches!(*value, Expr::Arrow(_)));
            }
            other => panic!("unexpected shape: {other:?}"),
        }
    }

    #[test]
    fn a_long_chain_of_arrow_calls_is_bounded() {
        let chain = format!("a{}", " => f()".repeat(5000));
        let error = parse(&chain).unwrap_err();
        assert!(error.message.contains("nested deeper"), "{}", error.message);

        let shallow = format!("a{}", " => f()".repeat(MAX_RECURSION_DEPTH / 2));
        assert!(parse(&shallow).is_ok());
    }

    #[test]
    fn parses_the_simple_map_operator() {
        assert!(matches!(
            parse("a ! b").unwrap(),
            Expr::Binary(BinaryOp::SimpleMap, _, _)
        ));
    }

    #[test]
    fn simple_map_chains_left_associatively() {
        // `a ! b ! c` is `(a ! b) ! c`.
        let expr = parse("a ! b ! c").unwrap();
        match expr {
            Expr::Binary(BinaryOp::SimpleMap, left, _) => {
                assert!(matches!(*left, Expr::Binary(BinaryOp::SimpleMap, _, _)));
            }
            other => panic!("unexpected shape: {other:?}"),
        }
    }

    #[test]
    fn simple_map_binds_tighter_than_union() {
        // `a ! b | c` must group as `(a ! b) | c` — real XPath 3.0's grammar
        // nests `UnionExpr` around `SimpleMapExpr`, not the other way round.
        let expr = parse("a ! b | c").unwrap();
        match expr {
            Expr::Binary(BinaryOp::Union, left, _) => {
                assert!(matches!(*left, Expr::Binary(BinaryOp::SimpleMap, _, _)));
            }
            other => panic!("unexpected shape: {other:?}"),
        }
    }

    #[test]
    fn range_binds_looser_than_simple_map() {
        // `1 to 2 ! f()` must group as `1 to (2 ! f())` — `to` sits above
        // `!` in this grammar exactly as `RangeExpr` sits above
        // `SimpleMapExpr` in real XPath 3.0's.
        let expr = parse("1 to 2 ! f()").unwrap();
        match expr {
            Expr::Range(_, right) => {
                assert!(matches!(*right, Expr::Binary(BinaryOp::SimpleMap, _, _)));
            }
            other => panic!("unexpected shape: {other:?}"),
        }
    }

    #[test]
    fn a_long_chain_of_simple_maps_is_bounded() {
        // Shares `parse_binary_chain` with the other repeating binary
        // operators — see `a_long_chain_of_any_repeating_binary_operator_is_bounded`
        // — but kept as its own test too, since `!` is the one of the nine
        // whose right operand is evaluated with a shifted context rather
        // than the shared one, which is a different enough evaluation path
        // to be worth its own coverage of the same parse-time bound.
        let chain = format!("a{}", " ! a".repeat(5000));
        let error = parse(&chain).unwrap_err();
        assert!(error.message.contains("nested deeper"), "{}", error.message);

        let shallow = format!("a{}", " ! a".repeat(MAX_RECURSION_DEPTH / 2));
        assert!(parse(&shallow).is_ok());
    }

    #[test]
    fn parses_a_let_expression() {
        let expr = parse("let $x := 1 return $x").unwrap();
        match expr {
            Expr::Let {
                variable,
                value,
                body,
            } => {
                assert_eq!(variable.to_string(), "x");
                assert!(matches!(*value, Expr::Number(1.0, _)));
                assert!(matches!(*body, Expr::Variable(_)));
            }
            other => panic!("unexpected shape: {other:?}"),
        }
    }

    #[test]
    fn let_is_still_a_name_without_a_following_variable() {
        // Same disambiguation `for`/`some`/`every` already use: `let` is
        // only a keyword directly before `$`, so an element named `let`
        // parses as an ordinary step.
        let p = path("let");
        assert_eq!(p.steps[0].node_test, NodeTest::Name(NameTest::parse("let")));
    }

    #[test]
    fn a_let_expression_needs_colon_equal() {
        let error = parse("let $x 1 return $x").unwrap_err();
        assert!(error.message.contains(":="), "{}", error.message);
    }
}
