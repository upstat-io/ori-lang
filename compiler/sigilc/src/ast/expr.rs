// AST expression definitions for Sigil
// Contains the main Expr enum representing all expression types

use super::matching::MatchExpr;
use super::operators::{BinaryOp, UnaryOp};
use super::patterns::PatternExpr;

/// Expressions
#[derive(Debug, Clone)]
pub enum Expr {
    /// Literals
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Nil,

    /// Identifier: x, user, etc.
    Ident(String),

    /// Config reference: $timeout
    Config(String),

    /// Length placeholder: # inside array index
    /// arr[# - 1] means arr[length - 1]
    LengthPlaceholder,

    /// List literal: [1, 2, 3]
    List(Vec<Expr>),

    /// Map literal: {"a": 1, "b": 2}
    MapLiteral(Vec<(Expr, Expr)>),

    /// Tuple: (a, b)
    Tuple(Vec<Expr>),

    /// Struct construction: User { id: x, name: y }
    Struct {
        name: String,
        fields: Vec<(String, Expr)>,
    },

    /// Field access: user.name
    Field(Box<Expr>, String),

    /// Index access: `arr[0]`
    Index(Box<Expr>, Box<Expr>),

    /// Function call: f(x, y)
    Call {
        func: Box<Expr>,
        args: Vec<Expr>,
    },

    /// Method call: x.method(y)
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<Expr>,
    },

    /// Binary operation: a + b
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },

    /// Unary operation: !x, -y
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },

    /// Lambda: x -> x + 1
    Lambda {
        params: Vec<String>,
        body: Box<Expr>,
    },

    /// Match expression
    Match(Box<MatchExpr>),

    /// If expression
    If {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
    },

    /// For loop
    For {
        binding: String,
        iterator: Box<Expr>,
        body: Box<Expr>,
    },

    /// Variable binding: let x = value or let mut x = value
    Let {
        name: String,
        mutable: bool,
        value: Box<Expr>,
    },

    /// Reassignment: x = value (only for mutable bindings)
    Reassign {
        target: String,
        value: Box<Expr>,
    },

    /// Block/sequence: run(expr1, expr2, ...)
    Block(Vec<Expr>),

    /// Range: 1..10
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
    },

    /// Pattern-based function calls
    Pattern(PatternExpr),

    /// Result constructors
    Ok(Box<Expr>),
    Err(Box<Expr>),
    Some(Box<Expr>),
    None_,

    /// Null coalesce: x ?? default
    Coalesce {
        value: Box<Expr>,
        default: Box<Expr>,
    },

    /// Unwrap: x.unwrap()
    Unwrap(Box<Expr>),
}
