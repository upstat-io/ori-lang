// AST match expression definitions for Sigil
// Contains MatchExpr, MatchArm, and Pattern types

use super::Expr;

#[derive(Debug, Clone)]
pub struct MatchExpr {
    pub scrutinee: Expr,
    pub arms: Vec<MatchArm>,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    /// Wildcard: _
    Wildcard,

    /// Literal: 5, "hello", true
    Literal(Expr),

    /// Binding: x
    Binding(String),

    /// Variant: Ok { value }, Err { error }
    Variant {
        name: String,
        fields: Vec<(String, Pattern)>,
    },

    /// Condition: expr (for match guards)
    Condition(Expr),
}
