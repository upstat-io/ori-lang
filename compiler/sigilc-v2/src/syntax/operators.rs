//! Binary and unary operators.

use std::fmt;

/// Binary operators with precedence levels.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum BinaryOp {
    // Arithmetic (precedence 3-4)
    Add,         // +
    Sub,         // -
    Mul,         // *
    Div,         // /
    Mod,         // %
    FloorDiv,    // div

    // Shift (precedence 5)
    Shl,         // <<
    Shr,         // >>

    // Comparison (precedence 7-8)
    Eq,          // ==
    Ne,          // !=
    Lt,          // <
    Le,          // <=
    Gt,          // >
    Ge,          // >=

    // Logical (precedence 12-13)
    And,         // &&
    Or,          // ||

    // Bitwise (precedence 9-11)
    BitAnd,      // &
    BitOr,       // |
    BitXor,      // ^

    // Range (precedence 6)
    Range,       // ..
    RangeInc,    // ..=

    // Null coalescing (precedence 14)
    Coalesce,    // ??

    // String concatenation (same as Add)
    Concat,      // + (for strings)
}

impl BinaryOp {
    /// Get the precedence level (lower = tighter binding).
    /// Based on Sigil spec precedence table.
    pub fn precedence(self) -> u8 {
        match self {
            // 3: Multiplicative
            BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod | BinaryOp::FloorDiv => 3,

            // 4: Additive
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Concat => 4,

            // 5: Shift
            BinaryOp::Shl | BinaryOp::Shr => 5,

            // 6: Range
            BinaryOp::Range | BinaryOp::RangeInc => 6,

            // 7: Comparison
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => 7,

            // 8: Equality
            BinaryOp::Eq | BinaryOp::Ne => 8,

            // 9: Bitwise AND
            BinaryOp::BitAnd => 9,

            // 10: Bitwise XOR
            BinaryOp::BitXor => 10,

            // 11: Bitwise OR
            BinaryOp::BitOr => 11,

            // 12: Logical AND
            BinaryOp::And => 12,

            // 13: Logical OR
            BinaryOp::Or => 13,

            // 14: Coalesce
            BinaryOp::Coalesce => 14,
        }
    }

    /// Check if operator is left-associative.
    pub fn is_left_assoc(self) -> bool {
        // All binary operators are left-associative in Sigil
        true
    }

    /// Check if operator is comparison (for chaining).
    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            BinaryOp::Eq | BinaryOp::Ne |
            BinaryOp::Lt | BinaryOp::Le |
            BinaryOp::Gt | BinaryOp::Ge
        )
    }

    /// Check if operator short-circuits.
    pub fn is_short_circuit(self) -> bool {
        matches!(self, BinaryOp::And | BinaryOp::Or | BinaryOp::Coalesce)
    }

    /// Get the operator symbol.
    pub fn symbol(self) -> &'static str {
        match self {
            BinaryOp::Add | BinaryOp::Concat => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Mod => "%",
            BinaryOp::FloorDiv => "div",
            BinaryOp::Shl => "<<",
            BinaryOp::Shr => ">>",
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            BinaryOp::And => "&&",
            BinaryOp::Or => "||",
            BinaryOp::BitAnd => "&",
            BinaryOp::BitOr => "|",
            BinaryOp::BitXor => "^",
            BinaryOp::Range => "..",
            BinaryOp::RangeInc => "..=",
            BinaryOp::Coalesce => "??",
        }
    }
}

impl fmt::Debug for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.symbol())
    }
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.symbol())
    }
}

/// Unary operators.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum UnaryOp {
    /// Logical not: !x
    Not,
    /// Numeric negation: -x
    Neg,
    /// Bitwise not: ~x
    BitNot,
}

impl UnaryOp {
    /// Get the operator symbol.
    pub fn symbol(self) -> &'static str {
        match self {
            UnaryOp::Not => "!",
            UnaryOp::Neg => "-",
            UnaryOp::BitNot => "~",
        }
    }
}

impl fmt::Debug for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.symbol())
    }
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.symbol())
    }
}
