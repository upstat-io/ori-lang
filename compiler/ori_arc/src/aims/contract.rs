//! Memory contracts for interprocedural analysis.
//!
//! [`MemoryContract`] describes a function's memory behavior: how it uses
//! parameters, what it returns, and what effects it has. Produced by
//! interprocedural analysis (Section 03), consumed by intraprocedural
//! analysis (Section 02) at call sites and by emission (Section 04).
//!
//! # Stage 1
//!
//! In Stage 1, contracts are conservative: all params owned/unrestricted,
//! return value `MaybeShared`. Section 03 refines these via SCC fixed-point.

use super::lattice::{AccessClass, Cardinality, Consumption, Uniqueness};

/// Memory contract for a function.
///
/// Describes parameter ownership, return value uniqueness, and effects.
/// Produced by interprocedural analysis (Section 03).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryContract {
    /// Per-parameter contracts, in parameter order.
    pub params: Vec<ParamContract>,
    /// Return value information.
    pub return_info: ReturnInfo,
}

impl MemoryContract {
    /// Conservative contract: all params owned/unrestricted, return `MaybeShared`.
    ///
    /// Used as fallback when no interprocedural information is available.
    pub fn conservative(num_params: usize) -> Self {
        Self {
            params: vec![ParamContract::CONSERVATIVE; num_params],
            return_info: ReturnInfo::CONSERVATIVE,
        }
    }
}

/// Per-parameter memory contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParamContract {
    /// Whether the parameter is owned or borrowed.
    pub access: AccessClass,
    /// How the parameter is consumed.
    pub consumption: Consumption,
    /// How many times the parameter is used.
    pub cardinality: Cardinality,
}

impl ParamContract {
    /// Conservative: owned, unrestricted, many uses.
    pub const CONSERVATIVE: Self = Self {
        access: AccessClass::Owned,
        consumption: Consumption::Unrestricted,
        cardinality: Cardinality::Many,
    };
}

/// Return value information in a memory contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReturnInfo {
    /// Uniqueness of the return value.
    pub uniqueness: Uniqueness,
}

impl ReturnInfo {
    /// Conservative: return value may be shared.
    pub const CONSERVATIVE: Self = Self {
        uniqueness: Uniqueness::MaybeShared,
    };
}

/// TRMC constructor-context region metadata (Stage 3).
///
/// Describes a region of code where a self-recursive function builds
/// a constructor in tail context. Empty in Stage 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextRegion {
    /// Placeholder — populated in Stage 3.
    _private: (),
}
