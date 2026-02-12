//! Pattern resolution types for type-checker → evaluator communication.
//!
//! These types bridge the type checker and evaluator: the type checker produces
//! `PatternResolution` entries keyed by `PatternKey`, and the evaluator consumes
//! them during match evaluation to disambiguate `Binding` patterns.
//!
//! Lives in `ori_ir` (not `ori_types`) because both `ori_types` and `ori_eval`
//! need these types, and `ori_ir` is the shared vocabulary crate.

use crate::Name;

/// Key identifying a match pattern in the AST.
///
/// Used to look up whether a `Binding` pattern was resolved to a unit variant
/// by the type checker. Keys are either top-level arm patterns (indexed by
/// the arm's absolute position in the arena) or nested patterns (indexed by
/// their `MatchPatternId`).
///
/// # Salsa Compatibility
///
/// Derives all traits required for Salsa query results plus `Ord` for sorted
/// storage and binary search in `TypedModule::pattern_resolutions`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PatternKey {
    /// Top-level arm pattern. Value = `ArmRange.start + arm_index`.
    Arm(u32),
    /// Nested pattern stored via `MatchPatternId`. Value = `MatchPatternId::raw()`.
    Nested(u32),
}

/// Type-checker resolution of an ambiguous `Binding` pattern.
///
/// When the parser encounters `Pending` in a match arm, it creates
/// `MatchPattern::Binding("Pending")` because it lacks type context.
/// The type checker resolves this to a `UnitVariant` if the name matches
/// a unit variant of the scrutinee's enum type.
///
/// # Invariant
///
/// If a `PatternKey` has no entry in `pattern_resolutions`, the `Binding`
/// is a normal variable binding (the common case). Only resolved patterns
/// are stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PatternResolution {
    /// This `Binding` is actually a unit variant of the scrutinee's enum type.
    UnitVariant {
        /// The enum type's name (e.g., `Status`).
        type_name: Name,
        /// The variant's index in declaration order (= tag value in LLVM).
        variant_index: u8,
    },
}
