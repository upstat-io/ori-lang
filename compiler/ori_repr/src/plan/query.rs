//! Query interface and policy types for `ReprPlan`.
//!
//! Provides convenience methods for common queries (width, triviality,
//! escape, RC strategy) and the `NarrowingPolicy` enum controlling
//! optimization aggressiveness.

use crate::layout::is_trivial_repr;
use crate::repr::{FloatWidth, IntWidth, MachineRepr};

use super::ReprPlan;

/// Controls how aggressively the representation optimizer narrows types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NarrowingPolicy {
    /// Apply all safe narrowing optimizations (default).
    Aggressive,
    /// Apply only provably-safe narrowing (no heuristics).
    Conservative,
    /// No narrowing — canonical representations only (`--no-repr-opt`).
    Disabled,
}

impl NarrowingPolicy {
    /// Check if `ORI_NO_REPR_OPT` env var is explicitly enabled.
    ///
    /// Accepts `"1"`, `"true"`, `"yes"` (case-insensitive for string values).
    /// Does NOT activate on mere presence — `ORI_NO_REPR_OPT=0` and
    /// `ORI_NO_REPR_OPT=false` are treated as disabled. Unset variable
    /// returns `false`.
    ///
    /// This is the **single canonical check** for the env var — all call
    /// sites in `oric`, `ori_llvm`, etc. must use this method rather than
    /// checking the env var directly. (TPR-01-030)
    #[must_use]
    pub fn env_disabled() -> bool {
        std::env::var("ORI_NO_REPR_OPT")
            .ok()
            .is_some_and(|v| is_env_truthy(&v))
    }
}

/// Check if an environment variable value is explicitly truthy.
///
/// Accepts `"1"`, `"true"`, `"yes"` (case-insensitive for string values).
/// Returns `false` for `"0"`, `"false"`, `"no"`, empty, or any other value.
pub(crate) fn is_env_truthy(val: &str) -> bool {
    val == "1" || val.eq_ignore_ascii_case("true") || val.eq_ignore_ascii_case("yes")
}

/// RC strategy for a type — how its reference count is managed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RcStrategy {
    /// No RC needed (trivial type — all fields are scalars).
    None,
    /// Non-atomic RC (thread-local value, no cross-thread sharing).
    NonAtomic {
        /// Width of the reference count field.
        width: IntWidth,
    },
    /// Atomic RC (may be shared across threads).
    Atomic {
        /// Width of the reference count field.
        width: IntWidth,
    },
}

impl ReprPlan {
    /// Get the integer width for a type (default: `I64`).
    #[must_use]
    pub fn int_width(&self, idx: ori_types::Idx) -> IntWidth {
        match self.get_repr(idx) {
            Some(MachineRepr::Int { width, .. }) => *width,
            _ => IntWidth::I64,
        }
    }

    /// Get the float width for a type (default: `F64`).
    #[must_use]
    pub fn float_width(&self, idx: ori_types::Idx) -> FloatWidth {
        match self.get_repr(idx) {
            Some(MachineRepr::Float { width }) => *width,
            _ => FloatWidth::F64,
        }
    }

    /// Check if a type is trivial (no RC needed).
    ///
    /// Returns `false` by default — safe (never elides RC it shouldn't).
    /// Uses [`crate::layout::is_trivial_repr`] — the single canonical
    /// triviality check at the `MachineRepr` level.
    #[must_use]
    pub fn is_trivial(&self, idx: ori_types::Idx) -> bool {
        match self.get_repr(idx) {
            Some(repr) => is_trivial_repr(repr),
            None => false,
        }
    }

    /// Check if a variable escapes its function scope.
    ///
    /// Returns `true` by default — safe (never stack-promotes when unsure).
    #[must_use]
    pub fn escapes(&self, _func: ori_ir::Name, _var: ori_arc::ArcVarId) -> bool {
        // Until §08 populates escape_info, assume everything escapes.
        true
    }

    /// Get the RC strategy for a type.
    ///
    /// Returns `Atomic { I64 }` by default — matches current `ori_rt` behavior.
    /// Reads from the dedicated `rc_strategies` map, NOT from `MachineRepr`
    /// pattern-matching. This ensures canonical `OpaquePtr` types (iterators,
    /// channels) correctly return the safe default rather than `RcStrategy::None`
    /// (TPR-01-023).
    #[must_use]
    pub fn rc_strategy(&self, idx: ori_types::Idx) -> RcStrategy {
        self.rc_strategies
            .get(&idx)
            .copied()
            .unwrap_or(RcStrategy::Atomic {
                width: IntWidth::I64,
            })
    }

    /// Get the narrowing policy.
    #[must_use]
    pub fn narrowing_policy(&self) -> NarrowingPolicy {
        self.narrowing_policy
    }
}
