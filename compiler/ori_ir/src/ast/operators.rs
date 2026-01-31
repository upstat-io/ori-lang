//! Binary and Unary Operators
//!
//! All operator types used in expressions.
//!
//! # Salsa Compatibility
//! All types have Copy, Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

/// Binary operators.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
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
}

/// Unary operators.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
    Try,
}
