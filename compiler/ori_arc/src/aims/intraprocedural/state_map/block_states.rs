//! Block-boundary state access, mutation, and convergence tracking.

use super::{AimsState, AimsStateMap, ArcBlockId, ArcVarId, FxHashMap, FxHashSet};

impl AimsStateMap {
    // Block state accessors

    /// Get the state of a variable at a block's exit (after terminator).
    ///
    /// Returns `SCALAR` for scalar variables, `BOTTOM` for variables
    /// not present in the state map (no demand from successors).
    ///
    /// # Invoke-terminator dsts
    ///
    /// For variables defined by an `ArcTerminator::Invoke` in `block`,
    /// `block_exit_states[block][var]` is BOTTOM because the normal
    /// successor's strip in `compute_block_entry_state` erases the var
    /// from its entry state before the predecessor's exit JOIN reads
    /// it. The `invoke_def_demand` side table captures the pre-strip
    /// demand and is consulted FIRST for any `(block, var)` pair,
    /// recovering the post-def demand (e.g., Return-widened
    /// `HeapEscaping` locality from a successor that returns the dst).
    /// Non-Invoke vars never have entries in `invoke_def_demand`, so
    /// the fallthrough to standard `block_exit_states` covers all
    /// other queries.
    #[must_use]
    pub fn var_state_at_block_exit(&self, block: ArcBlockId, var: ArcVarId) -> AimsState {
        if self.is_scalar(var) || self.is_immortal(var) {
            return AimsState::SCALAR;
        }
        if let Some(&state) = self.invoke_def_demand.get(&(block, var)) {
            return state;
        }
        self.block_exit_states
            .get(block.index())
            .and_then(|states| states.get(&var))
            .copied()
            .unwrap_or(AimsState::BOTTOM)
    }

    /// Get the converged BACKWARD-demand state at a variable's DEFINITION.
    ///
    /// Consults `def_demand` (intra-block instruction definitions) FIRST, then
    /// `invoke_def_demand` (Invoke-terminator definitions), then falls back to
    /// `block_exit_states`. Unlike `var_state_at_block_exit`, this recovers the
    /// pre-strip demand for a var defined+consumed within one block (where
    /// block-exit returns BOTTOM), giving DP-3 / DP-2 the proven TF-11
    /// `seqAdd`-accumulated cardinality (`Once` single-use, `Many` multi-use).
    #[must_use]
    pub fn var_state_at_definition(&self, block: ArcBlockId, var: ArcVarId) -> AimsState {
        if self.is_scalar(var) || self.is_immortal(var) {
            return AimsState::SCALAR;
        }
        if let Some(&state) = self.def_demand.get(&(block, var)) {
            return state;
        }
        if let Some(&state) = self.invoke_def_demand.get(&(block, var)) {
            return state;
        }
        self.block_exit_states
            .get(block.index())
            .and_then(|states| states.get(&var))
            .copied()
            .unwrap_or(AimsState::BOTTOM)
    }

    /// Record the converged pre-strip demand for an intra-block
    /// instruction-defined dst.
    ///
    /// Called by `analyze_function` after `compute_block_entry_state` returns
    /// the captured demand for the block's stripped instruction-defined vars.
    /// Keyed by `(defining_block, dst)`. See `def_demand` field doc.
    pub(crate) fn set_def_demand(&mut self, block: ArcBlockId, var: ArcVarId, state: AimsState) {
        self.def_demand.insert((block, var), state);
    }

    /// Record the pre-strip demand for an Invoke-terminator-defined dst.
    ///
    /// Called by `analyze_function` after `compute_block_entry_state` returns
    /// the captured demand for the normal successor's stripped vars. Keyed by
    /// `(invoke_owner_block, invoke_dst)` — the owner block is the predecessor
    /// whose terminator is the Invoke that defines `var`.
    ///
    /// See `invoke_def_demand` field doc for the full mechanism.
    pub(crate) fn set_invoke_def_demand(
        &mut self,
        block: ArcBlockId,
        var: ArcVarId,
        state: AimsState,
    ) {
        self.invoke_def_demand.insert((block, var), state);
    }

    /// Clear the invoke-def demand side table.
    ///
    /// Called at the start of each `analyze_function` iteration so the
    /// captured demand reflects the current iteration's successor entry
    /// states (which converge across iterations).
    pub(crate) fn clear_invoke_def_demand(&mut self) {
        self.invoke_def_demand.clear();
        self.def_demand.clear();
    }

    /// Get the state of a variable at a block's entry (before first instruction).
    ///
    /// Returns `SCALAR` for scalar variables, `BOTTOM` for variables
    /// not present in the state map.
    #[must_use]
    pub fn var_state_at_block_entry(&self, block: ArcBlockId, var: ArcVarId) -> AimsState {
        if self.is_scalar(var) || self.is_immortal(var) {
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

    /// Get L-9-excluded scalar liveness at a block's entry.
    pub(crate) fn scalar_live_at_entry(&self, block: ArcBlockId) -> Option<&FxHashSet<ArcVarId>> {
        self.scalar_live_at_entry.get(block.index())
    }

    /// Get L-9-excluded scalar liveness at a block's exit.
    pub(crate) fn scalar_live_at_exit(&self, block: ArcBlockId) -> Option<&FxHashSet<ArcVarId>> {
        self.scalar_live_at_exit.get(block.index())
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

    /// Join scalar liveness at a block entry as part of the same fixed point
    /// as the managed product state.
    pub(crate) fn update_scalar_live_at_entry(
        &mut self,
        block: ArcBlockId,
        live: FxHashSet<ArcVarId>,
    ) -> bool {
        let Some(current) = self.scalar_live_at_entry.get_mut(block.index()) else {
            return false;
        };
        let previous_len = current.len();
        current.extend(live);
        if current.len() == previous_len {
            return false;
        }
        self.changed = true;
        true
    }

    /// Join scalar liveness at a block exit as part of the same fixed point
    /// as the managed product state.
    pub(crate) fn update_scalar_live_at_exit(
        &mut self,
        block: ArcBlockId,
        live: FxHashSet<ArcVarId>,
    ) -> bool {
        let Some(current) = self.scalar_live_at_exit.get_mut(block.index()) else {
            return false;
        };
        let previous_len = current.len();
        current.extend(live);
        if current.len() == previous_len {
            return false;
        }
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

    /// Whether cross-dimension canonicalize chaining was detected during
    /// analysis (any canonicalize call required more than one round).
    ///
    /// With current rules, this should always be `false`.
    /// A `true` value indicates a new rule created a cross-dimension chain.
    #[must_use]
    pub fn cross_dimension_detected(&self) -> bool {
        self.cross_dimension_detected
    }

    /// Record that cross-dimension chaining was detected.
    pub fn set_cross_dimension_detected(&mut self) {
        self.cross_dimension_detected = true;
    }

    /// Count cross-dimension canonicalize rule effects on converged states.
    ///
    /// Examines all block exit states and counts variable-block pairs where
    /// the converged state shows evidence of cross-dimensional rule effects:
    /// - Cross-dim: `BlockLocal + Owned + ≤Once + Unique` (from FRESH/transfer)
    /// - Rule 6: `HeapEscaping/Unknown + MaybeShared` where Unique was demoted
    /// - Rule 8: `Borrowed + ≤FunctionLocal` where locality was capped
    ///
    /// Returns total count of cross-dim influenced variable-block pairs.
    #[must_use]
    pub fn count_cross_dim_states(&self) -> usize {
        use crate::aims::lattice::{AccessClass, Cardinality, Locality, Uniqueness};

        let mut count = 0;
        for exit_map in &self.block_exit_states {
            for state in exit_map.values() {
                if state.is_scalar() {
                    continue;
                }
                // Cross-dim evidence: state has Unique + BlockLocal + Owned + ≤Once.
                // Reachable from FRESH allocation or transfer functions.
                if state.uniqueness == Uniqueness::Unique
                    && state.locality == Locality::BlockLocal
                    && state.access == AccessClass::Owned
                    && state.cardinality <= Cardinality::Once
                {
                    count += 1;
                    continue;
                }
                // Rule 8 evidence: Borrowed + ≤FunctionLocal.
                // The locality cap is from cross-dim reasoning.
                if state.access == AccessClass::Borrowed
                    && state.locality <= Locality::FunctionLocal
                    && state.locality != Locality::BlockLocal
                {
                    count += 1;
                }
            }
        }
        count
    }
}
