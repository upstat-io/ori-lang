//! Per-variable facts, sparse events, effect summaries, and FIP summaries.

use super::{
    AimsEvent, AimsStateMap, ArcBlockId, ArcVarId, EffectSummary, Locality, ShapeClass, Uniqueness,
};

impl AimsStateMap {
    // Per-variable shape

    /// Get the shape classification for a variable from its definition.
    ///
    /// Returns `NonReusable` for variables without a recorded shape
    /// (block parameters, function parameters, or variables defined by
    /// non-shaping instructions).
    ///
    /// This is a per-variable property (set at the definition point),
    /// NOT a per-block state. Unlike the backward-computed lattice dimensions,
    /// shape doesn't change across block boundaries.
    #[must_use]
    pub fn var_shape(&self, var: ArcVarId) -> ShapeClass {
        self.var_shapes
            .get(&var)
            .copied()
            .unwrap_or(ShapeClass::NonReusable)
    }

    /// Record the shape for a variable, derived from its definition instruction.
    ///
    /// Called by `populate_var_shapes()` post-convergence.
    pub fn set_var_shape(&mut self, var: ArcVarId, shape: ShapeClass) {
        if !matches!(shape, ShapeClass::NonReusable) {
            self.var_shapes.insert(var, shape);
        }
    }

    // Per-variable contract-narrowed call-result side tables

    /// Record the contract-narrowed uniqueness for a call-result variable.
    ///
    /// BOTTOM-default sparse filter: `Uniqueness::Unique` is the lattice
    /// BOTTOM and is NOT stored — effective queries fall through to lattice
    /// (which is also Unique by default), giving identical behavior. The
    /// filter SHALL skip `Unique` (NOT `MaybeShared`); skipping `MaybeShared`
    /// would erase the load-bearing `ori_list_slice_drop` case where
    /// `return_info.uniqueness = MaybeShared` overrides the optimistic
    /// lattice default — that override is what prevents the slice-rest double-free.
    pub fn set_var_uniqueness(&mut self, var: ArcVarId, uniq: Uniqueness) {
        if !matches!(uniq, Uniqueness::Unique) {
            self.var_uniqueness.insert(var, uniq);
        }
    }

    /// Record the contract-narrowed locality for a call-result variable.
    ///
    /// BOTTOM-default sparse filter: `Locality::BlockLocal` is the lattice
    /// BOTTOM and is NOT stored. `FunctionLocal`, `HeapEscaping`, and
    /// `Unknown` (the CONSERVATIVE default for direct-no-contract calls)
    /// ARE stored. The filter SHALL skip BOTTOM, NOT CONSERVATIVE — the
    /// asymmetry is the same as `set_var_uniqueness` and serves the same
    /// architectural purpose.
    pub fn set_var_locality(&mut self, var: ArcVarId, loc: Locality) {
        if !matches!(loc, Locality::BlockLocal) {
            self.var_locality.insert(var, loc);
        }
    }

    /// Get the contract-narrowed uniqueness if the side table has an entry
    /// for `var`. Returns `None` when no contract narrowing applies.
    ///
    /// Distinguishes "unset" from "set to BOTTOM" (also None — BOTTOM never
    /// inserts). This presence-awareness is load-bearing for the
    /// `effective_uniqueness_at_block_*` JOIN semantics — an unset variable
    /// is NOT semantically equivalent to one set to `MaybeShared`, despite
    /// both differing from Unique. The override-pattern alternative is an AIMS
    /// Invariant 5 violation; presence-aware lookup + JOIN is the correct fix.
    #[must_use]
    pub fn contract_uniqueness(&self, var: ArcVarId) -> Option<Uniqueness> {
        self.var_uniqueness.get(&var).copied()
    }

    /// Get the contract-narrowed locality if the side table has an entry
    /// for `var`. Returns `None` when no contract narrowing applies.
    #[must_use]
    pub fn contract_locality(&self, var: ArcVarId) -> Option<Locality> {
        self.var_locality.get(&var).copied()
    }

    /// Effective uniqueness combining contract-narrowed forward state with
    /// the lattice's block-entry value for a call-result variable.
    ///
    /// Semantics: presence-aware lookup with lattice JOIN. When the side
    /// table is unset, returns the lattice value directly (no contract
    /// narrowing). When set, JOINs the contract value with the lattice
    /// value.
    ///
    /// JOIN preserves lattice widening: a contract claiming Unique that
    /// conflicts with backward demand's `MaybeShared` converges to `MaybeShared`,
    /// not Unique. This is the unified-model semantics
    /// Invariant 5 — the side table FEEDS INTO the lattice via JOIN, never
    /// overrides it. The override alternative (returning the side-table
    /// value when present, ignoring the lattice) suppresses backward demand
    /// widening and is rejected.
    #[must_use]
    pub fn effective_uniqueness_at_block_entry(
        &self,
        block: ArcBlockId,
        var: ArcVarId,
    ) -> Uniqueness {
        let lattice = self.var_state_at_block_entry(block, var).uniqueness;
        join_contract_over_lattice(lattice, self.contract_uniqueness(var), Uniqueness::join)
    }

    /// Effective uniqueness combining contract-narrowed forward state with
    /// the lattice's block-exit value. See [`effective_uniqueness_at_block_entry`]
    /// for JOIN semantics; this variant queries the exit-side lattice value.
    ///
    /// Entry-side and exit-side variants are NOT interchangeable — consumer
    /// sites that read different sides (COW reads entry, `drop_hints` read
    /// exit) MUST call the matching helper.
    #[must_use]
    pub fn effective_uniqueness_at_block_exit(
        &self,
        block: ArcBlockId,
        var: ArcVarId,
    ) -> Uniqueness {
        let lattice = self.var_state_at_block_exit(block, var).uniqueness;
        join_contract_over_lattice(lattice, self.contract_uniqueness(var), Uniqueness::join)
    }

    /// Effective locality combining contract-narrowed forward state with
    /// the lattice's block-entry value. JOIN semantics (`max` per
    /// §1.5: `BlockLocal` < `FunctionLocal` < `HeapEscaping` <
    /// Unknown — shipped 4-value chain; the spec's 5-value `ArgEscaping`
    /// is target-only per the spec's vocabulary-changes preamble).
    #[must_use]
    pub fn effective_locality_at_block_entry(&self, block: ArcBlockId, var: ArcVarId) -> Locality {
        let lattice = self.var_state_at_block_entry(block, var).locality;
        join_contract_over_lattice(lattice, self.contract_locality(var), Locality::join)
    }

    /// Effective locality combining contract-narrowed forward state with
    /// the lattice's block-exit value. See [`effective_locality_at_block_entry`].
    #[must_use]
    pub fn effective_locality_at_block_exit(&self, block: ArcBlockId, var: ArcVarId) -> Locality {
        let lattice = self.var_state_at_block_exit(block, var).locality;
        join_contract_over_lattice(lattice, self.contract_locality(var), Locality::join)
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

    // Effect summary

    /// Get the accumulated function-level effect summary.
    ///
    /// Populated by `populate_effect_summary()` after convergence.
    /// Returns `EffectSummary::default()` (all false) if not yet populated.
    #[must_use]
    pub fn effect_summary(&self) -> EffectSummary {
        self.effect_summary
    }

    /// Join an effect into the function-level accumulator.
    ///
    /// Each flag is OR'd: once set, it stays set.
    ///
    /// Note: `has_unbounded_stack` is NOT set during per-block accumulation.
    /// It remains `false` here; `extract_contract()` sets it from SCC
    /// membership and syntactic tail-position analysis.
    pub fn accumulate_effect(&mut self, effect: EffectSummary) {
        self.effect_summary = self.effect_summary.join(effect);
    }

    // FIP token balance

    /// Set the FIP allocation balance counts from post-convergence analysis.
    ///
    /// `construct_count`: non-scalar `Construct` instructions with reusable ctor kinds.
    /// `consumed_count`: consumed values with `ReusableCtor` shape (provide reuse tokens).
    pub fn set_fip_balance(&mut self, construct_count: u32, consumed_count: u32) {
        self.fip_construct_count = construct_count;
        self.fip_consumed_count = consumed_count;
    }

    /// Number of non-scalar `Construct` instructions with reusable ctor kinds.
    #[must_use]
    pub fn fip_construct_count(&self) -> u32 {
        self.fip_construct_count
    }

    /// Number of consumed non-scalar values with reusable shape (provide reuse
    /// tokens).
    #[must_use]
    pub fn fip_consumed_count(&self) -> u32 {
        self.fip_consumed_count
    }

    /// Whether the function's allocations are token-balanced by consumed values.
    ///
    /// `true` means consumed values with reusable shape >= construct allocations,
    /// so every Construct can potentially reuse memory from a consumed value.
    /// This is a necessary condition for FIP certification.
    #[must_use]
    pub fn fip_token_balanced(&self) -> bool {
        self.fip_consumed_count >= self.fip_construct_count
    }

    /// Net allocation count: constructs beyond what consumed values can supply.
    ///
    /// Returns 0 when balanced (FIP), positive when the function needs more
    /// allocations than it can reuse. Used for `FipContract::Bounded(n)`.
    #[must_use]
    pub fn fip_net_allocation(&self) -> u32 {
        self.fip_construct_count
            .saturating_sub(self.fip_consumed_count)
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

/// Shared JOIN semantics behind every `effective_*_at_block_*` query
/// (Invariant 5 — the side table feeds the lattice via JOIN, never
/// overrides it). `None` (no contract narrowing) returns `lattice`
/// unchanged; `Some(contract)` returns `join(contract, lattice)`.
fn join_contract_over_lattice<T: Copy>(
    lattice: T,
    contract: Option<T>,
    join: impl FnOnce(T, T) -> T,
) -> T {
    match contract {
        Some(contract) => join(contract, lattice),
        None => lattice,
    }
}
