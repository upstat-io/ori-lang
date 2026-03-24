//! Query interface and policy types for `ReprPlan`.
//!
//! Provides convenience methods for common queries (width, triviality,
//! escape, RC strategy) and the `NarrowingPolicy` enum controlling
//! optimization aggressiveness.

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
    #[must_use]
    pub fn is_trivial(&self, idx: ori_types::Idx) -> bool {
        match self.get_repr(idx) {
            Some(repr) => is_trivial_machine_repr(repr),
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

/// Check if a `MachineRepr` is trivial (no RC operations needed).
fn is_trivial_machine_repr(repr: &MachineRepr) -> bool {
    match repr {
        MachineRepr::Int { .. }
        | MachineRepr::Float { .. }
        | MachineRepr::Bool
        | MachineRepr::Char
        | MachineRepr::Byte
        | MachineRepr::Duration
        | MachineRepr::Size
        | MachineRepr::Ordering
        | MachineRepr::Unit
        | MachineRepr::Never
        | MachineRepr::Range => true,
        MachineRepr::Struct(s) => s.trivial,
        MachineRepr::Tuple(t) => t.trivial,
        MachineRepr::Enum(e) => e
            .variants
            .iter()
            .all(|v| v.fields.iter().all(is_trivial_machine_repr)),
        MachineRepr::FatPointer(_)
        | MachineRepr::Closure(_)
        | MachineRepr::RcPointer(_)
        | MachineRepr::OpaquePtr
        | MachineRepr::StackPromoted { .. } => false,
    }
}
