//! Operator strategy definitions for the Ori type registry.
//!
//! [`OpDefs`] holds the [`OpStrategy`] for every operator applicable to a
//! type. It is a fixed-field struct so that Rust's exhaustiveness checking
//! catches missing operator entries when a new operator is added.

use crate::tags::OpStrategy;

/// Operator strategies for a single type.
///
/// Every field corresponds to one operator. Each field is an [`OpStrategy`]
/// declaring how that operation is lowered. [`OpStrategy::Unsupported`] means
/// the operator is invalid for this type (caught by the type checker, never
/// reaches codegen).
///
/// # Consumer Usage
///
/// Consumers map their own `BinOp` enum to these fields directly:
///
/// ```ignore
/// let type_def = find_type(tag)?;
/// let strategy = match op {
///     BinOp::Add => type_def.operators.add,
///     BinOp::Sub => type_def.operators.sub,
///     // ...
/// };
/// ```
///
/// No wrapper function exists because `BinOp` lives in `ori_ir` and this
/// crate has zero dependencies. The exhaustive match on `BinOp` combined
/// with the required struct fields ensures compile-time coverage.
///
/// Adding a new field here is a compile error in every `TypeDef` definition
/// (Sections 03-07) and every backend match arm that reads `OpDefs`,
/// enforcing full coverage.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct OpDefs {
    // Arithmetic operators
    /// `+` (binary addition, or string concatenation for `str`).
    pub add: OpStrategy,
    /// `-` (binary subtraction).
    pub sub: OpStrategy,
    /// `*` (binary multiplication).
    pub mul: OpStrategy,
    /// `/` (binary division).
    pub div: OpStrategy,
    /// `%` (remainder/modulo).
    pub rem: OpStrategy,
    /// `div` keyword (integer/floor division).
    pub floor_div: OpStrategy,

    // Comparison operators (one field per operator)
    /// `==` (equality).
    pub eq: OpStrategy,
    /// `!=` (inequality).
    pub neq: OpStrategy,
    /// `<` (less than).
    pub lt: OpStrategy,
    /// `>` (greater than).
    pub gt: OpStrategy,
    /// `<=` (less than or equal).
    pub lt_eq: OpStrategy,
    /// `>=` (greater than or equal).
    pub gt_eq: OpStrategy,

    // Unary operators
    /// `-x` (unary negation).
    pub neg: OpStrategy,
    /// `!x` (logical NOT).
    pub not: OpStrategy,

    // Bitwise operators
    /// `&` (bitwise AND).
    pub bit_and: OpStrategy,
    /// `|` (bitwise OR).
    pub bit_or: OpStrategy,
    /// `^` (bitwise XOR).
    pub bit_xor: OpStrategy,
    /// `~` (bitwise NOT / complement).
    pub bit_not: OpStrategy,
    /// `<<` (left shift).
    pub shl: OpStrategy,
    /// `>>` (right shift, arithmetic for signed types).
    pub shr: OpStrategy,
}

// 20 fields x sizeof(OpStrategy).
// 64-bit: 20 x 24 = 480 bytes.
// 32-bit (WASM): 20 x 12 = 240 bytes.
#[cfg(target_pointer_width = "64")]
const _: () = assert!(core::mem::size_of::<OpDefs>() == 480);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(core::mem::size_of::<OpDefs>() == 240);

impl OpDefs {
    /// All operators unsupported.
    ///
    /// Useful as a starting point for types with no operator support,
    /// or as a base to override specific fields.
    pub const UNSUPPORTED: Self = Self {
        add: OpStrategy::Unsupported,
        sub: OpStrategy::Unsupported,
        mul: OpStrategy::Unsupported,
        div: OpStrategy::Unsupported,
        rem: OpStrategy::Unsupported,
        floor_div: OpStrategy::Unsupported,
        eq: OpStrategy::Unsupported,
        neq: OpStrategy::Unsupported,
        lt: OpStrategy::Unsupported,
        gt: OpStrategy::Unsupported,
        lt_eq: OpStrategy::Unsupported,
        gt_eq: OpStrategy::Unsupported,
        neg: OpStrategy::Unsupported,
        not: OpStrategy::Unsupported,
        bit_and: OpStrategy::Unsupported,
        bit_or: OpStrategy::Unsupported,
        bit_xor: OpStrategy::Unsupported,
        bit_not: OpStrategy::Unsupported,
        shl: OpStrategy::Unsupported,
        shr: OpStrategy::Unsupported,
    };
}

#[cfg(test)]
mod tests;
