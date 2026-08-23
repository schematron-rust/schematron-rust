//! The XPath 1.0 abstract syntax tree.
//!
//! Expressions are parsed once, when the schema is compiled, and evaluated
//! many times, so the tree is built for evaluation speed rather than for
//! round-tripping back to source.

use std::fmt;

/// A parsed XPath expression.
#[derive(Debug, Clone, PartialEq)]
// Variants will be added: XPath 2.0 phase 2b adds `cast as` and `instance of`.
// Marking it non-exhaustive now means that will not be a breaking change.
#[non_exhaustive]
pub enum Expr {
    /// A binary operation.
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    /// Arithmetic negation.
    Negate(Box<Expr>),
    /// A location path, or a filter expression followed by one.
    Path(Box<PathExpr>),
    /// A string literal.
    Literal(String),
    /// A numeric literal.
    Number(f64),
    /// A variable reference, `$name`.
    Variable(NameTest),
    /// A function call.
    Function {
        /// The function name, possibly prefixed.
        name: String,
        /// The argument expressions.
        args: Vec<Expr>,
    },
    /// `(E, E, E)` — an XPath 2.0 sequence.
    ///
    /// Nested sequences flatten when the value is built, so this holds the
    /// operands as written.
    Sequence(Vec<Expr>),
    /// `E to E` — an ascending range of integers.
    ///
    /// A descending range yields the empty sequence, as XPath 2.0 specifies.
    Range(Box<Expr>, Box<Expr>),
    /// `for $v in E return E` — iterates, yielding a sequence.
    For {
        /// The variable bound on each iteration.
        variable: NameTest,
        /// The sequence or node-set to iterate.
        input: Box<Expr>,
        /// Evaluated once per item; its results concatenate.
        body: Box<Expr>,
    },
    /// `some $v in E satisfies E` and `every $v in E satisfies E`.
    Quantified {
        /// Which quantifier.
        quantifier: Quantifier,
        /// The variable bound on each iteration.
        variable: NameTest,
        /// The sequence or node-set to iterate.
        input: Box<Expr>,
        /// The test applied to each item.
        test: Box<Expr>,
    },
    /// `E instance of T`, `E cast as T`, `E castable as T`, `E treat as T`.
    TypeOp {
        /// Which operator.
        op: TypeOp,
        /// The value being tested or converted.
        value: Box<Expr>,
        /// The type it is tested or converted against.
        sequence_type: SequenceType,
    },
    /// `if (E) then E else E`.
    ///
    /// XPath 2.0 only; an XPath 1.0 binding rejects it at compile time. Both
    /// branches are required, as XPath 2.0 requires.
    If {
        /// The condition, converted to boolean.
        condition: Box<Expr>,
        /// The value when the condition holds.
        then_branch: Box<Expr>,
        /// The value when it does not.
        else_branch: Box<Expr>,
    },
}

/// Which of XPath 2.0's four type operators an [`Expr::TypeOp`] applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TypeOp {
    /// `instance of` — whether the value matches the type.
    InstanceOf,
    /// `castable as` — whether a cast would succeed, without performing it.
    CastableAs,
    /// `cast as` — converts, or raises an error.
    CastAs,
    /// `treat as` — passes the value through, or raises an error.
    TreatAs,
}

impl TypeOp {
    /// The operator as written, for error messages.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            TypeOp::InstanceOf => "instance of",
            TypeOp::CastableAs => "castable as",
            TypeOp::CastAs => "cast as",
            TypeOp::TreatAs => "treat as",
        }
    }

    /// Whether this operator takes a single type rather than a sequence type.
    ///
    /// Casting a sequence has no meaning, so `cast as` and `castable as`
    /// accept only an atomic type with an optional `?`.
    #[must_use]
    pub const fn takes_single_type(self) -> bool {
        matches!(self, TypeOp::CastAs | TypeOp::CastableAs)
    }
}

/// How many items a [`SequenceType`] admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Occurrence {
    /// Exactly one, written with no indicator.
    #[default]
    One,
    /// `?` — zero or one.
    ZeroOrOne,
    /// `*` — zero or more.
    ZeroOrMore,
    /// `+` — one or more.
    OneOrMore,
}

impl Occurrence {
    /// Whether a sequence of this length is admitted.
    #[must_use]
    pub const fn admits(self, count: usize) -> bool {
        match self {
            Occurrence::One => count == 1,
            Occurrence::ZeroOrOne => count <= 1,
            Occurrence::ZeroOrMore => true,
            Occurrence::OneOrMore => count >= 1,
        }
    }

    /// The indicator as written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Occurrence::One => "",
            Occurrence::ZeroOrOne => "?",
            Occurrence::ZeroOrMore => "*",
            Occurrence::OneOrMore => "+",
        }
    }
}

/// What a single item must be for a [`SequenceType`] to match it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ItemType {
    /// `item()` — anything.
    AnyItem,
    /// `node()`, `element()`, `text()` and the rest, optionally named.
    Node {
        /// Which kind of node, or `None` for `node()`.
        kind: Option<crate::xml::NodeKind>,
        /// The name the node must have, when one was written.
        name: Option<NameTest>,
    },
    /// An atomic type, written with its prefix as in `xs:date`.
    Atomic(String),
    /// `empty-sequence()` — matches only the empty sequence.
    EmptySequence,
}

/// A type as written after `instance of`, `cast as`, and their companions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceType {
    /// What each item must be.
    pub item_type: ItemType,
    /// How many items are admitted.
    pub occurrence: Occurrence,
}

impl SequenceType {
    /// The type as written, for error messages.
    #[must_use]
    pub fn as_written(&self) -> String {
        let base = match &self.item_type {
            ItemType::AnyItem => "item()".to_string(),
            ItemType::EmptySequence => "empty-sequence()".to_string(),
            ItemType::Atomic(name) => name.clone(),
            ItemType::Node { kind, name } => {
                let base = match kind {
                    None => "node",
                    Some(crate::xml::NodeKind::Element) => "element",
                    Some(crate::xml::NodeKind::Attribute) => "attribute",
                    Some(crate::xml::NodeKind::Text) => "text",
                    Some(crate::xml::NodeKind::Comment) => "comment",
                    Some(crate::xml::NodeKind::ProcessingInstruction) => {
                        "processing-instruction"
                    }
                    Some(crate::xml::NodeKind::Root) => "document-node",
                    Some(crate::xml::NodeKind::Namespace) => "namespace-node",
                };
                match name {
                    Some(name) => format!("{base}({name})"),
                    None => format!("{base}()"),
                }
            }
        };
        format!("{base}{}", self.occurrence.as_str())
    }
}

/// Which quantifier an [`Expr::Quantified`] uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantifier {
    /// `some` — true when any item satisfies the test.
    Some,
    /// `every` — true when every item does, and for an empty input.
    Every,
}

impl Quantifier {
    /// The keyword, for error messages.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Quantifier::Some => "some",
            Quantifier::Every => "every",
        }
    }
}

/// The binary operators, at their XPath precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// Variants will be added: XPath 2.0 phase 2b adds the value comparisons.
// Marking it non-exhaustive now means that will not be a breaking change.
#[non_exhaustive]
pub enum BinaryOp {
    /// `or`
    Or,
    /// `and`
    And,
    /// `=`
    Equal,
    /// `!=`
    NotEqual,
    /// `<`
    Less,
    /// `<=`
    LessEqual,
    /// `>`
    Greater,
    /// `>=`
    GreaterEqual,
    /// `+`
    Add,
    /// `-`
    Subtract,
    /// `*`
    Multiply,
    /// `div`
    Divide,
    /// `mod`
    Modulo,
    /// `|`
    Union,
    /// `eq` — XPath 2.0 value comparison: exactly one item each, no coercion.
    ValueEqual,
    /// `ne`
    ValueNotEqual,
    /// `lt`
    ValueLess,
    /// `le`
    ValueLessEqual,
    /// `gt`
    ValueGreater,
    /// `ge`
    ValueGreaterEqual,
    /// `is` — whether both operands select the same node.
    NodeIs,
    /// `<<` — whether the left node precedes the right in document order.
    NodeBefore,
    /// `>>` — whether it follows.
    NodeAfter,
}

impl BinaryOp {
    /// Whether this is one of XPath 2.0's node comparisons.
    #[must_use]
    pub const fn is_node_comparison(self) -> bool {
        matches!(
            self,
            BinaryOp::NodeIs | BinaryOp::NodeBefore | BinaryOp::NodeAfter
        )
    }

    /// Whether this is one of XPath 2.0's value comparisons.
    #[must_use]
    pub const fn is_value_comparison(self) -> bool {
        matches!(
            self,
            BinaryOp::ValueEqual
                | BinaryOp::ValueNotEqual
                | BinaryOp::ValueLess
                | BinaryOp::ValueLessEqual
                | BinaryOp::ValueGreater
                | BinaryOp::ValueGreaterEqual
        )
    }

    /// The operator as written, for error messages.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            BinaryOp::Or => "or",
            BinaryOp::And => "and",
            BinaryOp::Equal => "=",
            BinaryOp::NotEqual => "!=",
            BinaryOp::Less => "<",
            BinaryOp::LessEqual => "<=",
            BinaryOp::Greater => ">",
            BinaryOp::GreaterEqual => ">=",
            BinaryOp::Add => "+",
            BinaryOp::Subtract => "-",
            BinaryOp::Multiply => "*",
            BinaryOp::Divide => "div",
            BinaryOp::Modulo => "mod",
            BinaryOp::Union => "|",
            BinaryOp::ValueEqual => "eq",
            BinaryOp::ValueNotEqual => "ne",
            BinaryOp::ValueLess => "lt",
            BinaryOp::ValueLessEqual => "le",
            BinaryOp::ValueGreater => "gt",
            BinaryOp::ValueGreaterEqual => "ge",
            BinaryOp::NodeIs => "is",
            BinaryOp::NodeBefore => "<<",
            BinaryOp::NodeAfter => ">>",
        }
    }
}

/// A location path: a starting point plus a sequence of steps.
#[derive(Debug, Clone, PartialEq)]
pub struct PathExpr {
    /// Where the path starts.
    pub start: PathStart,
    /// The steps to walk.
    pub steps: Vec<Step>,
}

/// The starting node-set of a path.
#[derive(Debug, Clone, PartialEq)]
pub enum PathStart {
    /// `/…` — start from the root node.
    Root,
    /// `a/b` — start from the context node.
    Context,
    /// `f(x)/b` or `(expr)[1]/b` — start from another expression's node-set.
    Expr(Box<Expr>, Vec<Expr>),
}

/// One step of a location path.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// The axis to walk.
    pub axis: Axis,
    /// The node test to apply to each node on the axis.
    pub node_test: NodeTest,
    /// The predicates to filter with, in order.
    pub predicates: Vec<Expr>,
}

/// The thirteen XPath axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// `ancestor::`
    Ancestor,
    /// `ancestor-or-self::`
    AncestorOrSelf,
    /// `attribute::`, abbreviated `@`
    Attribute,
    /// `child::`, the default axis
    Child,
    /// `descendant::`
    Descendant,
    /// `descendant-or-self::`, the expansion of `//`
    DescendantOrSelf,
    /// `following::`
    Following,
    /// `following-sibling::`
    FollowingSibling,
    /// `namespace::`
    Namespace,
    /// `parent::`, abbreviated `..`
    Parent,
    /// `preceding::`
    Preceding,
    /// `preceding-sibling::`
    PrecedingSibling,
    /// `self::`, abbreviated `.`
    SelfAxis,
}

impl Axis {
    /// Whether the axis runs backwards through the document.
    ///
    /// Predicates on a reverse axis number positions from the context node
    /// outwards, not in document order, which is the difference between
    /// `preceding-sibling::a[1]` meaning "nearest" and "first".
    #[must_use]
    pub const fn is_reverse(self) -> bool {
        matches!(
            self,
            Axis::Ancestor
                | Axis::AncestorOrSelf
                | Axis::Preceding
                | Axis::PrecedingSibling
                | Axis::Parent
        )
    }

    /// The name as written in an expression.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Axis::Ancestor => "ancestor",
            Axis::AncestorOrSelf => "ancestor-or-self",
            Axis::Attribute => "attribute",
            Axis::Child => "child",
            Axis::Descendant => "descendant",
            Axis::DescendantOrSelf => "descendant-or-self",
            Axis::Following => "following",
            Axis::FollowingSibling => "following-sibling",
            Axis::Namespace => "namespace",
            Axis::Parent => "parent",
            Axis::Preceding => "preceding",
            Axis::PrecedingSibling => "preceding-sibling",
            Axis::SelfAxis => "self",
        }
    }

    /// Parses an axis name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Axis> {
        Some(match name {
            "ancestor" => Axis::Ancestor,
            "ancestor-or-self" => Axis::AncestorOrSelf,
            "attribute" => Axis::Attribute,
            "child" => Axis::Child,
            "descendant" => Axis::Descendant,
            "descendant-or-self" => Axis::DescendantOrSelf,
            "following" => Axis::Following,
            "following-sibling" => Axis::FollowingSibling,
            "namespace" => Axis::Namespace,
            "parent" => Axis::Parent,
            "preceding" => Axis::Preceding,
            "preceding-sibling" => Axis::PrecedingSibling,
            "self" => Axis::SelfAxis,
            _ => return None,
        })
    }
}

/// A name that may carry a prefix, resolved against the schema's `ns`
/// declarations at evaluation time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameTest {
    /// The prefix, if the name was written with one.
    pub prefix: Option<String>,
    /// The local part.
    pub local: String,
}

impl NameTest {
    /// Splits `prefix:local` into its parts.
    #[must_use]
    pub fn parse(raw: &str) -> NameTest {
        match raw.split_once(':') {
            Some((prefix, local)) => NameTest {
                prefix: Some(prefix.to_string()),
                local: local.to_string(),
            },
            None => NameTest {
                prefix: None,
                local: raw.to_string(),
            },
        }
    }
}

impl fmt::Display for NameTest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.prefix {
            Some(p) => write!(f, "{p}:{}", self.local),
            None => f.write_str(&self.local),
        }
    }
}

/// The node test applied to each node an axis yields.
#[derive(Debug, Clone, PartialEq)]
// Variants will be added: XPath 2.0 phase 2b adds the kind tests. Marking it
// non-exhaustive now means that will not be a breaking change.
#[non_exhaustive]
pub enum NodeTest {
    /// A qualified name: matches nodes of the axis's principal type whose
    /// expanded name matches.
    Name(NameTest),
    /// `*`: any node of the axis's principal type.
    Wildcard,
    /// `prefix:*`: any node of the principal type in that namespace.
    NamespaceWildcard(String),
    /// `node()`
    AnyNode,
    /// `text()`
    Text,
    /// `comment()`
    Comment,
    /// `processing-instruction()` or `processing-instruction('target')`
    ProcessingInstruction(Option<String>),
    /// An XPath 2.0 kind test used as a node test: `element()`,
    /// `element(name)`, `attribute()`, `attribute(name)`, `document-node()`.
    ///
    /// Distinct from [`NodeTest::Wildcard`], which selects the axis's
    /// principal node type: `element()` selects elements on any axis.
    Kind {
        /// The kind of node selected.
        kind: crate::xml::NodeKind,
        /// The name it must have, when one was written. `element(*)` is
        /// written with a wildcard and means the same as `element()`.
        name: Option<NameTest>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_axes_are_marked() {
        assert!(Axis::Ancestor.is_reverse());
        assert!(Axis::PrecedingSibling.is_reverse());
        assert!(!Axis::Child.is_reverse());
        assert!(!Axis::Following.is_reverse());
    }

    #[test]
    fn axis_names_round_trip() {
        for axis in [Axis::Ancestor, Axis::DescendantOrSelf, Axis::SelfAxis] {
            assert_eq!(Axis::from_name(axis.as_str()), Some(axis));
        }
        assert_eq!(Axis::from_name("nonsense"), None);
    }

    #[test]
    fn name_test_splits_prefix() {
        let n = NameTest::parse("p:a");
        assert_eq!(n.prefix.as_deref(), Some("p"));
        assert_eq!(n.local, "a");
        assert_eq!(NameTest::parse("a").prefix, None);
    }
}
