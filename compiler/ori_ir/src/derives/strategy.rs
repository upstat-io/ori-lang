//! Derivation strategy types.
//!
//! Each derived trait has a [`DeriveStrategy`] that describes the logical
//! structure of the derivation algorithm: what to do per field, how to combine
//! results, and how to handle sum types. Backends (eval, LLVM) interpret these
//! strategies in their own representation (`Value` vs LLVM IR), eliminating
//! the need to duplicate composition logic.
//!
//! See `DerivedTrait::strategy()` for the mapping from trait to strategy.

/// The high-level strategy for deriving a trait.
///
/// Captures the composition logic (field iteration, result combination) that
/// is shared between eval and LLVM backends. The primitive operations (how to
/// compare two values, how to emit an `icmp`) remain backend-specific.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DeriveStrategy {
    /// How to handle struct fields.
    pub struct_body: StructBody,
    /// How to handle sum type variants.
    pub sum_body: SumBody,
}

/// Strategy for deriving a trait on a struct (product type).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum StructBody {
    /// Apply an operation to each field (pair), combine results.
    ///
    /// Used by: Eq (`AllTrue`), Comparable (`Lexicographic`), Hashable (`HashCombine`).
    ForEachField {
        /// The per-field operation.
        field_op: FieldOp,
        /// How to combine per-field results into a final value.
        combine: CombineOp,
    },

    /// Format fields into a string representation.
    ///
    /// Used by: Printable (`"TypeName(v1, v2)"`), Debug (`"TypeName { f1: v1, f2: v2 }"`).
    FormatFields {
        /// Opening delimiter style.
        open: FormatOpen,
        /// Text between fields.
        separator: &'static str,
        /// Text after all fields.
        suffix: &'static str,
        /// Whether to include `field_name: ` before each value.
        include_names: bool,
    },

    /// Produce a default value for each field and construct the struct.
    ///
    /// Used by: Default.
    DefaultConstruct,

    /// Clone/copy each field (identity for value types).
    ///
    /// Used by: Clone.
    CloneFields,
}

/// The per-field operation in a `ForEachField` strategy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FieldOp {
    /// `field.eq(other_field)` — binary, returns `bool`.
    Equals,
    /// `field.compare(other_field)` — binary, returns `Ordering`.
    Compare,
    /// `field.hash()` — unary, returns `int`.
    Hash,
}

/// How to combine per-field results in a `ForEachField` strategy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CombineOp {
    /// All fields must be `true`; short-circuit on first `false`.
    ///
    /// Initial: `true`. Used by Eq.
    AllTrue,
    /// Use the first non-`Equal` ordering; return `Equal` if all fields equal.
    ///
    /// Initial: `Ordering::Equal`. Used by Comparable.
    Lexicographic,
    /// FNV-1a: `hash = (hash ^ field_hash) * FNV_PRIME`.
    ///
    /// Initial: `FNV_OFFSET_BASIS`. Used by Hashable.
    HashCombine,
}

/// Opening delimiter style for `FormatFields`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FormatOpen {
    /// `"TypeName("` — used by Printable.
    TypeNameParen,
    /// `"TypeName { "` — used by Debug.
    TypeNameBrace,
}

/// How to handle sum types (enums) in a derivation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SumBody {
    /// Match on variant tag; same-variant pairs use struct strategy on payloads,
    /// different-variant pairs use tag ordering.
    MatchVariants,
    /// Not supported for this trait (e.g., Default on sum types).
    NotSupported,
}

#[cfg(test)]
mod tests;
