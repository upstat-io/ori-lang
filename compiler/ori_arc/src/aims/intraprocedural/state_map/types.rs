//! Sparse events and alias/invoke side-table value types.

use rustc_hash::FxHashMap;

use crate::aims::lattice::AimsState;
use crate::ir::{ArcBlockId, ArcVarId};

// Per-invoke edge state

/// Per-edge demand state for Invoke terminators.
///
/// Normal and unwind are alternative paths (only one executes),
/// but they carry different variable sets: `dst` is defined only
/// on the normal path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvokeEdgeState {
    /// Demand flowing to the normal successor (`dst` IS defined here).
    pub normal: FxHashMap<ArcVarId, AimsState>,
    /// Demand flowing to the unwind successor (`dst` is NOT defined here).
    /// Used by RC emission to determine cleanup `RcDec` operations on
    /// the exception path.
    pub unwind: FxHashMap<ArcVarId, AimsState>,
}

// Sparse event table

/// A special-interest event detected during analysis.
///
/// Stored sparsely per block — most blocks have no events. These track
/// program points that don't fit the per-variable-per-block model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AimsEvent {
    /// A TRMC constructor-context region opens at this point.
    /// (Stage 3 — populated by normalize/ pass, consumed by reuse emission)
    ContextOpen {
        block: ArcBlockId,
        instr: usize,
        var: ArcVarId,
    },
    /// A TRMC constructor-context region closes at this point.
    ContextClose {
        block: ArcBlockId,
        instr: usize,
        var: ArcVarId,
    },
    /// A candidate reusable allocation site.
    ReusableAllocation {
        block: ArcBlockId,
        instr: usize,
        var: ArcVarId,
    },
    /// A lifetime-bounded value that a physical planner may consider for a
    /// local placement. AIMS supplies no size, storage, or ABI decision.
    PlacementEligibilityCandidate {
        block: ArcBlockId,
        instr: usize,
        var: ArcVarId,
    },
    /// A FIP gate: a call site where the callee's `FipContract` is
    /// `Conditional`, and the preconditions are satisfied at this point.
    FipGate { block: ArcBlockId, instr: usize },
    /// Per-branch allocation credit balance at a Switch terminator's successor.
    ///
    /// Records the allocation balance (constructs - consumed deaths) for each
    /// successor of a Switch terminator. FIP certification requires each branch
    /// to independently maintain non-negative credit balance (`FIPTree` DMATCH! rule).
    AllocCreditBalance {
        /// The Switch terminator's block.
        block: ArcBlockId,
        /// Index of the successor in the Switch's targets.
        successor_idx: usize,
        /// Balance: positive = more allocs than deaths (needs tokens),
        /// zero = balanced, negative = surplus deaths (provides tokens).
        balance: i32,
    },
}

impl AimsEvent {
    /// The block this event belongs to.
    pub(super) fn block(&self) -> ArcBlockId {
        match self {
            Self::ContextOpen { block, .. }
            | Self::ContextClose { block, .. }
            | Self::ReusableAllocation { block, .. }
            | Self::PlacementEligibilityCandidate { block, .. }
            | Self::FipGate { block, .. }
            | Self::AllocCreditBalance { block, .. } => *block,
        }
    }
}

// Apply-result alias side-table

/// Caller-side allocation-identity record for an Apply/Invoke destination.
///
/// When a callee's `MemoryContract` carries `ParamContract::return_alias =
/// Some(ReturnAliasShape::*)` for one or more params, the destination of an
/// Apply/Invoke at the caller IS the same logical allocation identity as the consumed
/// argument(s) — the callee transferred ownership through return. This
/// side-table records that identity so the caller's RC emission can avoid
/// double-decrementing the shared allocation.
///
/// Distinct from `borrow_sources` (Project-level borrow facts) and from
/// `project_alias_sources` (transitive Let/Jump alias chains) — see §1.9
/// Side-Table Domains. The three side-tables are composed at
/// `compute_project_alias_sources` so a single backward-walk query reaches
/// the canonical RC owner regardless of which mechanism produced the alias.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplyAliasSource {
    /// Apply dst aliases the consumed arg directly (callee returns the
    /// param unchanged: `@id<T>(x: T) -> T = x`). At the caller,
    /// `Apply dst = @callee(arg)` makes `dst` the same allocation as `arg`.
    Direct(ArcVarId),
    /// Apply dst aliases a single-field projection of the consumed arg
    /// (callee returns `arg.field`: `@unwrap<T>(b: Box<T>) -> T = b.inner`).
    /// Single-field only — matches `ReturnAliasShape::Project { field: u32 }`
    /// and `BorrowSource::Exact { field: Option<u32> }` precedent. Nested
    /// projections defer to a future extension.
    Project { arg: ArcVarId, field: u32 },
    /// Apply dst structurally CONTAINS the consumed arg as a transitive-drop
    /// variant payload (callee constructs a wrapping variant: `@wrap_ok(m: T)
    /// -> Result<T, E> = Ok(m)`). Distinct from `Direct`/`Project`: the
    /// dst is a SEPARATE allocation (the constructed wrapper), and arg lives
    /// inside dst's payload via the structural drop walk. PIN-2 ANALOGOUS:
    /// `Wrapped` does NOT trigger `uf.union` in `compute_ssa_alias_classes`
    /// (the wrapper and the wrapped allocation are DIFFERENT RC slots), and
    /// does NOT seed `project_alias_sources` Step 1b in
    /// `compute_project_alias_sources` (containment is NOT a projection-
    /// derived alias chain). The sole consumer is `should_suppress_apply
    /// _aliased_dec`: suppresses the redundant
    /// caller-side canonical dec on `arg` because `arg`'s ownership was
    /// transferred into dst's payload via the wrapping construct. Without
    /// this suppression, both arg's caller-side dec AND dst's structural
    /// drop walk dec the same allocation → double-free.
    Wrapped(ArcVarId),
    /// Apply dst aliases ONE OF the candidates at runtime — the exact alias
    /// is path-conditional. Arises when 2+ params of the callee have
    /// `return_alias != None` (e.g., `match x { A -> a, B -> b }` in the
    /// callee). Caller's RC emission suppresses scope-exit decs on EVERY
    /// candidate (their ownership transferred at the Apply site, regardless
    /// of which path is taken at runtime); dst's own RC ops are retained as
    /// the canonical owner of the returned allocation.
    Conditional { candidates: Vec<ArcVarId> },
}
