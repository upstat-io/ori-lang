//! Uniqueness summary for interprocedural analysis.
//!
//! Captures the uniqueness of each parameter and the return value, enabling
//! callers to determine the uniqueness of call results without re-analyzing
//! the callee.

use super::Uniqueness;

/// Summary of a function's uniqueness behavior for interprocedural analysis.
///
/// Captures the uniqueness of each parameter and the return value, enabling
/// callers to determine the uniqueness of call results without re-analyzing
/// the callee.
///
/// Produced by [`super::inter::analyze_program`] and consumed by
/// [`super::intra::analyze_with_summaries`] to refine `Apply` results from the
/// conservative `MaybeShared` default to a precise uniqueness state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UniquenessSummary {
    /// Uniqueness of each parameter at function entry.
    ///
    /// Currently all `MaybeShared` (callers may share), but included for
    /// future interprocedural parameter refinement.
    ///
    /// **Note:** For builtin/COW summaries (from [`super::inter::build_cow_summaries`]),
    /// this is empty — only `return_val` is meaningful. Do not index into
    /// `params` without checking length first; use `.get(i)` or guard with
    /// `if i < summary.params.len()`.
    pub params: Vec<Uniqueness>,

    /// Uniqueness of the function's return value.
    ///
    /// - `Unique`: the function always returns a fresh allocation (e.g.,
    ///   constructor, COW operation result).
    /// - `MaybeShared`: the function may return a parameter or captured value.
    /// - `Shared`: the function always returns a known-shared value (rare).
    pub return_val: Uniqueness,

    /// Whether the function is "freshness-preserving": if all RC'd inputs
    /// are `Unique`, the output is guaranteed `Unique`.
    ///
    /// True for functions that only construct fresh values or apply COW
    /// operations. False for functions that may return a parameter directly.
    pub preserves_freshness: bool,
}

impl UniquenessSummary {
    /// Create a conservative summary where everything is `MaybeShared`.
    pub fn conservative(num_params: usize) -> Self {
        Self {
            params: vec![Uniqueness::MaybeShared; num_params],
            return_val: Uniqueness::MaybeShared,
            preserves_freshness: false,
        }
    }
}
