//! Per-instruction context accumulated by the Phase-5 burden-emission walker.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcVarId};
use crate::lower::burden::BurdenRef;

/// Per-instruction context accumulated by the emission walker.
///
/// Two storage axes (per-var and per-instruction transfer-point lookups
/// have distinct semantics):
/// - `collected` — per-`ArcVarId` `(var, BurdenSpec lookup)` from `var_types`
///   walk. Filtered by `ArcParam.ownership` for params.
/// - `transfer_points` — per-instruction `(consumed var, BurdenSpec lookup)`
///   for transfer points where ownership transfers (`Construct` with owned
///   arg; `Apply` / `Set` / etc.).
#[derive(Debug, Default)]
pub(crate) struct BurdenLowerCtx<'a> {
    pub(super) collected: Vec<(ArcVarId, Option<BurdenRef<'a>>)>,
    pub(super) transfer_points: Vec<(ArcVarId, Option<BurdenRef<'a>>)>,
    pub(super) last_use_points: Vec<(ArcVarId, usize, usize)>,
    /// Per-block block-LOCAL moved-field bitsets indexed by `block_idx`.
    /// Each entry maps `ArcVarId → set of moved field indices` for
    /// projections that occur within THIS block's body or terminator (the
    /// per-block transfer function output). Filled by Pass 2 of
    /// `populate_moved_out_fields`. `FieldId` is `u32` per
    /// `ArcInstr::Project.field`.
    pub(super) moved_out_fields_block_local: Vec<FxHashMap<ArcVarId, FxHashSet<u32>>>,
    /// Per-block ENTRY moved-field bitsets indexed by `block_idx`. Computed
    /// at fixpoint as `INTERSECT over P in predecessors(B): exit(P)` (or
    /// empty for entry block). Per `Spec: Annex E §AIMS RL-2`
    /// partial-transfer semantics, only fields moved on ALL incoming paths
    /// are "definitely moved" at block entry. When E2043 typeck rejection
    /// guarantees equal predecessor sets the INTERSECT degenerates to
    /// pick-any; INTERSECT remains the correct merge in both states.
    pub(super) moved_out_fields_block_entry: Vec<FxHashMap<ArcVarId, FxHashSet<u32>>>,
    /// Per-block EXIT moved-field bitsets indexed by `block_idx`. Computed
    /// at fixpoint as `entry(B) ∪ block_local(B)` (pointwise union: for each
    /// var, union field sets). The flow function for "field moves accumulate
    /// forward along reachable paths".
    pub(super) moved_out_fields_block_exit: Vec<FxHashMap<ArcVarId, FxHashSet<u32>>>,
    /// Cached union of `moved_out_fields_block_exit` populated at the end of
    /// `populate_moved_out_fields`. The accessor lends a reference into this
    /// field, preserving the `&FxHashMap<...>` accessor contract. Consumed
    /// by `compute_full_move_vars` / `compute_partial_move_vars`; both retain
    /// union-view semantics per `Spec: Annex E §AIMS RL-2` (a var's
    /// `BurdenDec` suppression / `BurdenDecPartial.skip_fields` is the union
    /// across all reachable CFG paths from definition to last use — exactly
    /// the exit-state union).
    pub(super) moved_out_fields_union: FxHashMap<ArcVarId, FxHashSet<u32>>,
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "accessors consumed by tests only; the returned ctx's accessors \
                  are not yet read by the production pipeline (the class_covered \
                  consumer is pending) — the walk reads the fields directly"
    )
)]
impl<'a> BurdenLowerCtx<'a> {
    /// Construct a fresh `BurdenLowerCtx` sized for `func`'s block count.
    /// All three per-block maps (`moved_out_fields_block_local`,
    /// `moved_out_fields_block_entry`, `moved_out_fields_block_exit`) are
    /// pre-allocated with `func.blocks.len()` empty maps so
    /// `populate_moved_out_fields` can index by `block_idx` without bounds
    /// checking. Other Vec fields (`collected`, `transfer_points`,
    /// `last_use_points`) stay empty; downstream walks populate them via
    /// `push`.
    pub(super) fn new(func: &ArcFunction) -> Self {
        let n = func.blocks.len();
        Self {
            collected: Vec::new(),
            transfer_points: Vec::new(),
            last_use_points: Vec::new(),
            moved_out_fields_block_local: vec![FxHashMap::default(); n],
            moved_out_fields_block_entry: vec![FxHashMap::default(); n],
            moved_out_fields_block_exit: vec![FxHashMap::default(); n],
            moved_out_fields_union: FxHashMap::default(),
        }
    }

    /// Read-only access to the accumulated `(var, burden lookup)` pairs.
    pub(crate) fn collected_burdens(&self) -> &[(ArcVarId, Option<BurdenRef<'a>>)] {
        &self.collected
    }

    /// Read-only access to the accumulated per-instruction transfer-point
    /// burden lookups for `Construct`, `Apply`, `ApplyIndirect`, `Invoke`,
    /// `InvokeIndirect`, `CollectionReuse`, `Set`, and `PartialApply` owned
    /// positions.
    pub(crate) fn transfer_points(&self) -> &[(ArcVarId, Option<BurdenRef<'a>>)] {
        &self.transfer_points
    }

    /// Read-only access to per-block last-use positions: `(var, block_idx,
    /// instr_idx)`. `BurdenDec(v)` emits immediately following EVERY last-use
    /// of `v` along every reachable CFG path; cross-block liveness flows via
    /// block-param handoffs.
    pub(crate) fn last_use_points(&self) -> &[(ArcVarId, usize, usize)] {
        &self.last_use_points
    }

    /// Read-only access to the moved-field bitset map (union-of-exit-states
    /// view). Populated at the end of `populate_moved_out_fields` from
    /// `moved_out_fields_block_exit`.
    pub(crate) fn moved_out_fields(&self) -> &FxHashMap<ArcVarId, FxHashSet<u32>> {
        &self.moved_out_fields_union
    }
}
