//! Binary and Unary Operators
//!
//! All operator types used in expressions.
//!
//! # Salsa Compatibility
//! All types have Copy, Clone, Eq, PartialEq, Hash, Debug for Salsa requirements.

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

/// Unary operators.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
    Try,
}
