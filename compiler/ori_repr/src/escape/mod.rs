//! AIMS-backed escape facts consumed by representation planning.

use ori_arc::ir::{ArcVarId, YieldAllocationFact, YieldAllocationLocality};
use rustc_hash::FxHashSet;

/// Frozen per-function escape information for allocation-bearing variables.
///
/// Absence remains conservative: only identities explicitly proven local by
/// AIMS are members of `local_vars`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EscapeInfo {
    local_vars: FxHashSet<ArcVarId>,
}

impl EscapeInfo {
    /// Build an escape projection from the allocation identities frozen by AIMS.
    #[must_use]
    pub fn from_yield_allocations(facts: &[YieldAllocationFact]) -> Self {
        let mut local_vars = FxHashSet::default();
        for fact in facts {
            if fact.locality == YieldAllocationLocality::Local {
                local_vars.insert(fact.builder);
                local_vars.insert(fact.result);
            }
        }
        Self { local_vars }
    }

    /// Query one stable SSA identity. Missing facts conservatively escape.
    #[must_use]
    pub fn escapes(&self, var: ArcVarId) -> bool {
        !self.local_vars.contains(&var)
    }
}
