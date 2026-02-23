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
    MatMul,

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
            Self::MatMul => "@",
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
            Self::Mul | Self::Div | Self::Mod | Self::FloorDiv | Self::MatMul => 3,
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

impl BinaryOp {
    /// Map this operator to its trait method name for operator overloading.
    ///
    /// Only operators with corresponding trait methods are mapped; comparison,
    /// logical, range, and coalesce operators return `None`.
    ///
    /// This is the **single source of truth** — `ori_types` (type checker) and
    /// `ori_llvm` (codegen) both call this instead of maintaining parallel mappings.
    pub const fn trait_method_name(self) -> Option<&'static str> {
        match self {
            Self::Add => Some("add"),
            Self::Sub => Some("subtract"),
            Self::Mul => Some("multiply"),
            Self::Div => Some("divide"),
            Self::FloorDiv => Some("floor_divide"),
            Self::Mod => Some("remainder"),
            Self::BitAnd => Some("bit_and"),
            Self::BitOr => Some("bit_or"),
            Self::BitXor => Some("bit_xor"),
            Self::Shl => Some("shift_left"),
            Self::Shr => Some("shift_right"),
            Self::MatMul => Some("mat_mul"),
            _ => None,
        }
    }

    /// Map this operator to its trait name for error messages and dispatch.
    ///
    /// Returns the trait name (e.g., `"Add"`, `"Sub"`) that a type must implement
    /// to support this operator. Same set of overloadable operators as
    /// `trait_method_name()`.
    ///
    /// This is the **single source of truth** — `ori_types` delegates to this
    /// instead of maintaining a parallel mapping.
    pub const fn trait_name(self) -> Option<&'static str> {
        match self {
            Self::Add => Some("Add"),
            Self::Sub => Some("Sub"),
            Self::Mul => Some("Mul"),
            Self::Div => Some("Div"),
            Self::FloorDiv => Some("FloorDiv"),
            Self::Mod => Some("Rem"),
            Self::BitAnd => Some("BitAnd"),
            Self::BitOr => Some("BitOr"),
            Self::BitXor => Some("BitXor"),
            Self::Shl => Some("Shl"),
            Self::Shr => Some("Shr"),
            Self::MatMul => Some("MatMul"),
            _ => None,
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

impl UnaryOp {
    /// Returns the source-level symbol for this operator.
    ///
    /// Used in error messages to show the exact operator that failed.
    pub const fn as_symbol(self) -> &'static str {
        match self {
            Self::Neg => "-",
            Self::Not => "!",
            Self::BitNot => "~",
            Self::Try => "?",
        }
    }
}

impl UnaryOp {
    /// Map this operator to its trait method name for operator overloading.
    ///
    /// `Try` is desugared before codegen and has no trait method.
    ///
    /// This is the **single source of truth** — `ori_types` (type checker) and
    /// `ori_llvm` (codegen) both call this instead of maintaining parallel mappings.
    pub const fn trait_method_name(self) -> Option<&'static str> {
        match self {
            Self::Neg => Some("negate"),
            Self::Not => Some("not"),
            Self::BitNot => Some("bit_not"),
            Self::Try => None,
        }
    }

    /// Map this operator to its trait name for error messages and dispatch.
    ///
    /// Returns the trait name (e.g., `"Neg"`, `"Not"`) that a type must implement
    /// to support this operator. Same set as `trait_method_name()`.
    ///
    /// This is the **single source of truth** — `ori_types` delegates to this
    /// instead of maintaining hardcoded strings in match arms.
    pub const fn trait_name(self) -> Option<&'static str> {
        match self {
            Self::Neg => Some("Neg"),
            Self::Not => Some("Not"),
            Self::BitNot => Some("BitNot"),
            Self::Try => None,
        }
    }
}
