//! Binary and Unary Operators
//!
//! All operator types used in expressions.
//!
//! # Specification
//!
//! - Syntax: `docs/ori_lang/0.1-alpha/spec/grammar.ebnf` § EXPRESSIONS
//! - Semantics: `docs/ori_lang/0.1-alpha/spec/operator-rules.md`
//! - Precedence: `docs/ori_lang/0.1-alpha/spec/operator-rules.md` § Precedence Table
//!
//! # Salsa Compatibility
//! All types have Copy, Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

/// Binary operators.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum BinaryOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    FloorDiv,

    // Comparison
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,

    // Logical
    And,
    Or,

    // Bitwise
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,

    // Other
    Range,
    RangeInclusive,
    Coalesce,
}

impl BinaryOp {
    /// Returns the source-level symbol for this operator.
    ///
    /// Used in error messages to show the exact operator that failed.
    pub const fn as_symbol(self) -> &'static str {
        match self {
            // Arithmetic
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Mod => "%",
            Self::FloorDiv => "div",
            // Comparison
            Self::Eq => "==",
            Self::NotEq => "!=",
            Self::Lt => "<",
            Self::LtEq => "<=",
            Self::Gt => ">",
            Self::GtEq => ">=",
            // Logical
            Self::And => "&&",
            Self::Or => "||",
            // Bitwise
            Self::BitAnd => "&",
            Self::BitOr => "|",
            Self::BitXor => "^",
            Self::Shl => "<<",
            Self::Shr => ">>",
            // Other
            Self::Range => "..",
            Self::RangeInclusive => "..=",
            Self::Coalesce => "??",
        }
    }

    /// Returns the precedence level of this operator.
    ///
    /// Higher number = lower precedence (binds less tightly).
    /// Used by the formatter to determine when parentheses are needed.
    ///
    /// Precedence levels (from operator-rules.md):
    /// - 3: `*` `/` `%` `div`
    /// - 4: `+` `-`
    /// - 5: `<<` `>>`
    /// - 6: `..` `..=`
    /// - 7: `<` `>` `<=` `>=`
    /// - 8: `==` `!=`
    /// - 9: `&`
    /// - 10: `^`
    /// - 11: `|`
    /// - 12: `&&`
    /// - 13: `||`
    /// - 14: `??`
    pub const fn precedence(self) -> u8 {
        match self {
            // Multiplicative (highest binary precedence)
            Self::Mul | Self::Div | Self::Mod | Self::FloorDiv => 3,
            // Additive
            Self::Add | Self::Sub => 4,
            // Shift
            Self::Shl | Self::Shr => 5,
            // Range
            Self::Range | Self::RangeInclusive => 6,
            // Comparison
            Self::Lt | Self::LtEq | Self::Gt | Self::GtEq => 7,
            // Equality
            Self::Eq | Self::NotEq => 8,
            // Bitwise AND
            Self::BitAnd => 9,
            // Bitwise XOR
            Self::BitXor => 10,
            // Bitwise OR
            Self::BitOr => 11,
            // Logical AND
            Self::And => 12,
            // Logical OR
            Self::Or => 13,
            // Coalesce (lowest binary precedence)
            Self::Coalesce => 14,
        }
    }
}

/// Unary operators.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
    Try,
}
