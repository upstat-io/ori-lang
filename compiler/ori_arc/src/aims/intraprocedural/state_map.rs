//! State map data structure for intraprocedural analysis.
//!
//! [`AimsStateMap`] stores the computed [`AimsState`] for every variable at block
//! boundaries (entry and exit). Per-instruction states are NOT stored — they are
//! re-derived during emission by replaying transfer functions within each block.
//!
//! The state map is the **analysis fact source** for all downstream consumers:
//! RC emission, reuse emission, COW annotation, drop hints, and FIP certification.

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "tests use expect for clearer failure messages"
)]
mod tests;

use rustc_hash::FxHashMap;

use crate::ir::{ArcBlockId, ArcFunction, ArcVarId};

use super::super::lattice::{AimsState, BorrowSource};

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
    /// A local-allocation eligibility point (`Locality::FunctionLocal`
    /// or `BlockLocal` for a non-scalar allocation).
    LocalAllocCandidate {
        block: ArcBlockId,
        instr: usize,
        var: ArcVarId,
    },
    /// A FIP gate: a call site where the callee's `FipContract` is
    /// `Conditional`, and the preconditions are satisfied at this point.
    FipGate { block: ArcBlockId, instr: usize },
}

impl AimsEvent {
    /// The block this event belongs to.
    fn block(&self) -> ArcBlockId {
        match self {
            Self::ContextOpen { block, .. }
            | Self::ContextClose { block, .. }
            | Self::ReusableAllocation { block, .. }
            | Self::LocalAllocCandidate { block, .. }
            | Self::FipGate { block, .. } => *block,
        }
    }
}

// State map

/// Complete analysis result for a function.
///
/// Maps (block-boundary, variable) → [`AimsState`].
/// Computed by backward dataflow, consumed by emission passes.
/// Per-instruction states are NOT stored — they are re-derived by
/// emission passes via backward replay within each block.
///
/// Also contains:
/// - Borrow provenance side table (per-variable, not per-point)
/// - Per-invoke edge states (normal vs unwind demand)
/// - Sparse event table for special-interest program points
///
/// # Memory layout
///
/// Uses `Vec<FxHashMap<ArcVarId, AimsState>>` (sparse) rather than
/// `Vec<Vec<AimsState>>` (dense). In backward demand analysis, most
/// variables are `BOTTOM` (no demand) at most blocks — sparse storage
/// avoids allocating `num_vars` entries per block when only a fraction
/// have non-BOTTOM state. For functions with very few variables where
/// dense storage would win on cache locality, the hash map overhead is
/// negligible (small maps fit in a single cache line).
pub struct AimsStateMap {
    /// State at the END of each block (after terminator).
    /// Indexed by `ArcBlockId::index()`.
    block_exit_states: Vec<FxHashMap<ArcVarId, AimsState>>,

    /// State at the ENTRY of each block (before first instruction).
    /// Computed by applying transfer functions backward through the block's
    /// instructions, starting from the block's exit state.
    /// Indexed by `ArcBlockId::index()`.
    block_entry_states: Vec<FxHashMap<ArcVarId, AimsState>>,

    /// Per-invoke edge states. Stored sparsely — only for blocks
    /// ending in Invoke.
    invoke_edge_states: FxHashMap<ArcBlockId, InvokeEdgeState>,

    /// Borrow provenance side table (solutions.md Decision 1/5).
    /// Sparse: only entries for variables currently in `AccessClass::Borrowed`.
    /// Per-variable (not per-point) — see module doc for precision trade-off.
    borrow_sources: FxHashMap<ArcVarId, BorrowSource>,

    /// Sparse event table: special-interest program points, indexed by block.
    events: FxHashMap<ArcBlockId, Vec<AimsEvent>>,

    /// Variables permanently marked as SCALAR (excluded from analysis).
    /// Indexed by `ArcVarId::index()`. True = scalar (never analyzed).
    scalars: Vec<bool>,

    /// Tracks whether any state changed in the last iteration.
    /// Reset to `false` at the start of each iteration; set to `true`
    /// by `update_block_entry` when a state changes.
    changed: bool,
}

impl AimsStateMap {
    /// Create a new state map for the given function.
    ///
    /// All variables start at `BOTTOM` (most optimistic). Scalar variables
    /// must be marked separately via [`set_permanent_scalar`](Self::set_permanent_scalar).
    pub fn new(func: &ArcFunction) -> Self {
        let num_blocks = func.blocks.len();
        let num_vars = func.var_types.len();
        Self {
            block_exit_states: vec![FxHashMap::default(); num_blocks],
            block_entry_states: vec![FxHashMap::default(); num_blocks],
            invoke_edge_states: FxHashMap::default(),
            borrow_sources: FxHashMap::default(),
            events: FxHashMap::default(),
            scalars: vec![false; num_vars],
            changed: false,
        }
    }

    // Scalar management

    /// Mark a variable as permanently SCALAR (excluded from analysis).
    ///
    /// Scalar variables never need RC operations, COW checks, or reuse.
    /// This is irreversible — once marked, the variable returns `SCALAR`
    /// from all state queries.
    pub fn set_permanent_scalar(&mut self, var: ArcVarId) {
        if let Some(entry) = self.scalars.get_mut(var.index()) {
            *entry = true;
        }
    }

    /// Whether a variable is permanently SCALAR.
    #[inline]
    pub fn is_scalar(&self, var: ArcVarId) -> bool {
        self.scalars.get(var.index()).copied().unwrap_or(false)
    }

    // Block state accessors

    /// Get the state of a variable at a block's exit (after terminator).
    ///
    /// Returns `SCALAR` for scalar variables, `BOTTOM` for variables
    /// not present in the state map (no demand from successors).
    #[must_use]
    pub fn var_state_at_block_exit(&self, block: ArcBlockId, var: ArcVarId) -> AimsState {
        if self.is_scalar(var) {
            return AimsState::SCALAR;
        }
        self.block_exit_states
            .get(block.index())
            .and_then(|states| states.get(&var))
            .copied()
            .unwrap_or(AimsState::BOTTOM)
    }

    /// Get the state of a variable at a block's entry (before first instruction).
    ///
    /// Returns `SCALAR` for scalar variables, `BOTTOM` for variables
    /// not present in the state map.
    #[must_use]
    pub fn var_state_at_block_entry(&self, block: ArcBlockId, var: ArcVarId) -> AimsState {
        if self.is_scalar(var) {
            return AimsState::SCALAR;
        }
        self.block_entry_states
            .get(block.index())
            .and_then(|states| states.get(&var))
            .copied()
            .unwrap_or(AimsState::BOTTOM)
    }

    /// Get the full entry state map for a block (all variables with non-BOTTOM state).
    ///
    /// Returns `None` for out-of-bounds block indices.
    #[must_use]
    pub fn block_entry_states(&self, block: ArcBlockId) -> Option<&FxHashMap<ArcVarId, AimsState>> {
        self.block_entry_states.get(block.index())
    }

    /// Get the full exit state map for a block (all variables with non-BOTTOM state).
    ///
    /// Returns `None` for out-of-bounds block indices.
    #[must_use]
    pub fn block_exit_states(&self, block: ArcBlockId) -> Option<&FxHashMap<ArcVarId, AimsState>> {
        self.block_exit_states.get(block.index())
    }

    // Block state mutation

    /// Update the entry state for a block. Returns `true` if any state changed.
    ///
    /// Called by the worklist loop: if this returns `true`, predecessors
    /// need to be re-analyzed.
    pub fn update_block_entry(
        &mut self,
        block: ArcBlockId,
        new_entry: FxHashMap<ArcVarId, AimsState>,
    ) -> bool {
        let idx = block.index();
        if idx >= self.block_entry_states.len() {
            return false;
        }
        let current = &self.block_entry_states[idx];
        if *current == new_entry {
            return false;
        }
        self.block_entry_states[idx] = new_entry;
        self.changed = true;
        true
    }

    /// Update the exit state for a block. Returns `true` if any state changed.
    pub fn update_block_exit(
        &mut self,
        block: ArcBlockId,
        new_exit: FxHashMap<ArcVarId, AimsState>,
    ) -> bool {
        let idx = block.index();
        if idx >= self.block_exit_states.len() {
            return false;
        }
        let current = &self.block_exit_states[idx];
        if *current == new_exit {
            return false;
        }
        self.block_exit_states[idx] = new_exit;
        self.changed = true;
        true
    }

    // Convergence tracking

    /// Whether the analysis has converged (no state changed in last iteration).
    #[must_use]
    pub fn is_converged(&self) -> bool {
        !self.changed
    }

    /// Reset the change tracker for a new iteration.
    pub fn reset_changed(&mut self) {
        self.changed = false;
    }

    // Borrow provenance

    /// Get the borrow provenance for a variable.
    ///
    /// Returns `None` if the variable is Owned or not tracked.
    #[must_use]
    pub fn borrow_source(&self, var: ArcVarId) -> Option<&BorrowSource> {
        self.borrow_sources.get(&var)
    }

    /// Update borrow provenance during transfer function application.
    ///
    /// Called by `Project` and pattern binding transfers.
    pub fn set_borrow_source(&mut self, var: ArcVarId, source: BorrowSource) {
        self.borrow_sources.insert(var, source);
    }

    /// Remove provenance when a variable transitions to `AccessClass::Owned`.
    pub fn clear_borrow_source(&mut self, var: ArcVarId) {
        self.borrow_sources.remove(&var);
    }

    /// Merge provenance at control flow join points.
    ///
    /// Same source → keep `Exact`; different sources → `Unknown`.
    pub fn join_borrow_sources(&mut self, var: ArcVarId, other: BorrowSource) {
        match self.borrow_sources.get(&var) {
            Some(existing) => {
                let joined = existing.join(other);
                self.borrow_sources.insert(var, joined);
            }
            None => {
                self.borrow_sources.insert(var, other);
            }
        }
    }

    // Invoke edge states

    /// Get the per-edge demand state for a block ending in Invoke.
    ///
    /// Returns `None` for blocks that don't end in Invoke.
    #[must_use]
    pub fn invoke_edge_state(&self, block: ArcBlockId) -> Option<&InvokeEdgeState> {
        self.invoke_edge_states.get(&block)
    }

    /// Store per-edge state during analysis when processing an Invoke terminator.
    pub fn set_invoke_edge_state(&mut self, block: ArcBlockId, state: InvokeEdgeState) {
        self.invoke_edge_states.insert(block, state);
    }

    // Sparse event table

    /// Get the event slice for a specific block.
    ///
    /// Returns an empty slice if no events recorded for that block.
    #[must_use]
    pub fn events_in_block(&self, block: ArcBlockId) -> &[AimsEvent] {
        self.events
            .get(&block)
            .map_or(&[], |events| events.as_slice())
    }

    /// Append a sparse event to the block's event list.
    pub fn record_event(&mut self, event: AimsEvent) {
        let block = event.block();
        self.events.entry(block).or_default().push(event);
    }

    // Summary queries

    /// Number of blocks in the state map.
    #[must_use]
    pub fn num_blocks(&self) -> usize {
        self.block_entry_states.len()
    }

    /// Number of variables tracked (including scalars).
    #[must_use]
    pub fn num_vars(&self) -> usize {
        self.scalars.len()
    }
}
