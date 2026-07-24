//! Borrow, allocation-alias, SSA-class, and invoke-edge side tables.

use super::{
    AimsStateMap, ApplyAliasSource, ArcBlockId, ArcVarId, BorrowSource, FxHashMap, FxHashSet,
    InvokeEdgeState, ProjectSources,
};

impl AimsStateMap {
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

    /// Find all borrows from a given source variable.
    ///
    /// Returns an iterator of `(borrow_var, field)` pairs, where `field` is
    /// `Some(idx)` for field-level borrows (from `Project`) and `None` for
    /// whole-object borrows. Used by the disjoint-field COW optimization
    /// to check whether a mutation conflicts with live borrows.
    pub fn borrows_from_source(
        &self,
        source: ArcVarId,
    ) -> impl Iterator<Item = (ArcVarId, Option<u32>)> + '_ {
        self.borrow_sources.iter().filter_map(move |(var, bs)| {
            if let BorrowSource::Exact { source: src, field } = bs {
                if *src == source {
                    return Some((*var, *field));
                }
            }
            None
        })
    }

    /// Merge provenance at control flow join points.
    ///
    /// Same source → keep `Exact`; different sources → `Unknown`.
    pub fn join_borrow_sources(&mut self, var: ArcVarId, other: BorrowSource) {
        self.borrow_sources
            .entry(var)
            .and_modify(|existing| *existing = existing.join(other))
            .or_insert(other);
    }

    // Apply-result allocation-identity provenance

    /// Look up the Apply-result allocation-identity record for a variable.
    ///
    /// Returns `Some(_)` only when `var` is an Apply/Invoke destination AND
    /// the callee's `MemoryContract` carried `ParamContract::return_alias !=
    /// None` for one or more Owned params at the time
    /// `populate_apply_result_aliases` ran. Returns `None` for fresh
    /// allocations, indirect calls, and Apply/Invoke destinations whose
    /// callees do not transfer ownership through return.
    #[must_use]
    pub fn apply_result_alias(&self, var: ArcVarId) -> Option<&ApplyAliasSource> {
        self.apply_result_aliases.get(&var)
    }

    /// Read-only borrow of the entire Apply-result alias map.
    ///
    /// Consumed by `compute_project_alias_sources` Step 1b (composition with
    /// the project alias graph) and by the `realize/walk.rs` forward-walk
    /// `is_ownership_transfer` / `is_owned_call_position` classification.
    #[must_use]
    pub fn apply_result_aliases(&self) -> &FxHashMap<ArcVarId, ApplyAliasSource> {
        &self.apply_result_aliases
    }

    /// Bulk-install the pre-computed Apply-result alias map.
    ///
    /// Called once per function during `analyze_function`'s pre-walk setup,
    /// BEFORE `compute_project_alias_sources` and BEFORE the worklist loop.
    /// The map is read-only after this point per PL-5 (no-stale-summary
    /// invariant).
    pub fn set_apply_result_aliases(&mut self, aliases: FxHashMap<ArcVarId, ApplyAliasSource>) {
        self.apply_result_aliases = aliases;
    }

    // Project-derived alias graph

    /// Bulk-install the pre-computed Project-derived alias source map.
    ///
    /// Called once per function during `analyze_function`'s pre-walk setup,
    /// AFTER `compute_project_alias_sources` runs. The local map is also
    /// kept by `analyze_function` for `propagate_project_source_demand`
    /// during the worklist; this setter persists a clone on the state map
    /// so the post-convergence pass can consume it after lattice
    /// convergence. Read-only after this point per PL-5.
    pub(crate) fn set_project_alias_sources(
        &mut self,
        sources: FxHashMap<ArcVarId, ProjectSources>,
    ) {
        self.project_alias_sources = sources;
    }

    /// Whether `var` is the destination of an Apply/Invoke whose callee
    /// `MemoryContract` carried `return_alias != None` for one or more
    /// Owned params. O(1) lookup against the pre-walk-populated
    /// `apply_result_aliases` map.
    ///
    /// Consumed for narrowing `should_suppress_return_transfer_dec`
    /// interactions on apply-alias destinations.
    #[must_use]
    pub fn is_apply_alias_destination(&self, var: ArcVarId) -> bool {
        self.apply_result_aliases.contains_key(&var)
    }

    // SSA-alias equivalence-class accessors

    /// Return the equivalence-class id for `var` if it participates in a
    /// multi-member class; `None` for singletons.
    #[must_use]
    pub fn ssa_alias_class_of(&self, var: ArcVarId) -> Option<u32> {
        self.ssa_alias_classes.get(&var).copied()
    }

    /// Return the set of class members for `class_id`, if any.
    #[must_use]
    pub fn class_members(&self, class_id: u32) -> Option<&FxHashSet<ArcVarId>> {
        self.class_members.get(&class_id)
    }

    /// Return the set of source-candidate vars recorded for `class_id`.
    /// Used by `should_suppress_apply_aliased_dec` to detect apply-source
    /// roles for caller-side dec suppression.
    #[must_use]
    pub fn class_apply_alias_source_candidates(
        &self,
        class_id: u32,
    ) -> Option<&FxHashSet<ArcVarId>> {
        self.class_apply_alias_source_candidates.get(&class_id)
    }

    /// Bulk-install the pre-computed SSA-alias-class output. Read-only after
    /// this point per PL-5 (no-stale-summary invariant).
    pub fn set_ssa_alias_output(
        &mut self,
        class_table: FxHashMap<ArcVarId, u32>,
        class_members: FxHashMap<u32, FxHashSet<ArcVarId>>,
        class_apply_alias_source_candidates: FxHashMap<u32, FxHashSet<ArcVarId>>,
    ) {
        self.ssa_alias_classes = class_table;
        self.class_members = class_members;
        self.class_apply_alias_source_candidates = class_apply_alias_source_candidates;
    }

    /// Materialize a singleton `class_members` entry for `class_id` if absent.
    ///
    /// Singleton id == `ArcVarId::raw()` per the existing scheme; recovers
    /// the var via `ArcVarId::new(class_id)` and inserts both the
    /// class-members and ssa-alias-classes entries idempotently.
    /// Required by `materialize_transitive_drop_singleton_classes` so the
    /// `class_members(class_id)` lookups in the realize walk
    /// (`cleanup_redundant.rs`) succeed for singleton parents/children.
    pub(crate) fn ensure_singleton_class(&mut self, class_id: u32) {
        if let std::collections::hash_map::Entry::Vacant(entry) = self.class_members.entry(class_id)
        {
            let var = ArcVarId::new(class_id);
            let mut singleton: FxHashSet<ArcVarId> = FxHashSet::default();
            singleton.insert(var);
            entry.insert(singleton);
            self.ssa_alias_classes.entry(var).or_insert(class_id);
        }
    }

    /// Resolve a var's class id, falling back to its raw u32 (singleton id).
    ///
    /// `ssa_alias_class_of(var)` returns `Some(class_id)` for vars in a
    /// multi-member class and `None` for singletons. Singleton class id
    /// equals `var.raw()` per the existing materialization scheme; this
    /// helper consolidates the lookup so callers don't repeat the fallback.
    /// Used by the post-convergence edge recorder to resolve arg/dst class ids
    /// without re-running the local `UnionFind` from `compute_ssa_alias_classes`.
    pub(crate) fn class_id_of(&self, var: ArcVarId) -> u32 {
        self.ssa_alias_class_of(var).unwrap_or_else(|| var.raw())
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
}
