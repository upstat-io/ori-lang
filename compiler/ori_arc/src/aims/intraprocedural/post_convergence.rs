//! Post-convergence passes for intraprocedural analysis.
//!
//! After the backward dataflow analysis converges, these passes populate
//! side tables in the [`AimsStateMap`] using the converged state:
//!
//! - [`populate_borrow_sources`] — borrow source tracking for Project instructions
//! - [`populate_sparse_events`] — reusable logical shapes + placement-eligibility candidates
//! - [`populate_var_shapes`] — per-variable shape from definitions
//! - [`detect_trmc_candidates`] — TRMC `ContextHole` detection
//! - [`populate_context_events`] — ContextOpen/ContextClose from normalize metadata
//! - [`super::fip_balance::populate_fip_balance`] — FIP token balance (in `fip_balance.rs`)
//! - [`super::fip_balance::populate_fip_gate_events`] — FIP gate events (in `fip_balance.rs`)
//!
//! These passes are called from [`super::analyze_function`] after convergence.

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{
    is_transitive_drop_strategy, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue,
    ArcVarId, ArgOwnership, CtorKind, RcStrategy, ValueRepr,
};

use super::super::contract::{ContextRegion, MemoryContract, ReturnContract};
use super::super::lattice::{
    AccessClass, AimsState, Cardinality, Consumption, EffectClass, Locality, ShapeClass, Uniqueness,
};
use super::super::transfer::transfer_def_resolved;
use super::state_map::{AimsEvent, AimsStateMap};

/// Populate the borrow source side table after analysis converges.
///
/// Each variable defined by a `Project` instruction gets
/// `BorrowSource::exact_field(source_var, field)`. Block params that receive values
/// from multiple predecessors are left without a source (implicitly
/// `Unknown` if queried). In ARC IR's SSA form, each variable has exactly
/// one definition point, so this is per-variable, not per-point.
pub(crate) fn populate_borrow_sources(state_map: &mut AimsStateMap, func: &ArcFunction) {
    for block in &func.blocks {
        for instr in &block.body {
            if instr
                .defined_var()
                .is_some_and(|dst| state_map.is_scalar(dst))
            {
                continue;
            }

            // Use the converged state to compute the transfer result.
            let get_state = |v: ArcVarId| -> AimsState {
                if state_map.is_scalar(v) {
                    return AimsState::SCALAR;
                }
                state_map.var_state_at_block_entry(block.id, v)
            };

            if let Some(def) = transfer_def_resolved(func, instr, &get_state) {
                if let (Some(dst), Some(source)) = (instr.defined_var(), def.borrow_source) {
                    state_map.set_borrow_source(dst, source);
                }
            }
        }
    }
}

/// Populate the sparse event table after analysis converges.
///
/// Records two categories of events:
///
/// 1. **Reusable allocation candidates** (`AimsEvent::ReusableAllocation`):
///    `Construct` instructions with reusable constructor kinds (`Struct`,
///    `EnumVariant`) on non-scalar destinations. These mark allocation sites
///    that the `ReusePlanner` can match against death events for cross-block
///    reuse.
///
/// 2. **Placement eligibility** (`AimsEvent::PlacementEligibilityCandidate`):
///    Variables whose effective exit-locality (`effective_locality_at_block_exit`,
///    which JOINs the lattice value with the contract-narrowed value populated
///    by `populate_call_result_states`) shows `Locality::FunctionLocal` or
///    `BlockLocal`. The effective query is load-bearing for direct call results:
///    without it, a callee with `return_info.locality = FunctionLocal` would
///    not surface as a `PlacementEligibilityCandidate` because the lattice's BOTTOM
///    locality is `BlockLocal` (already FunctionLocal-eligible) but the
///    contract-derived narrowing is invisible.
///
/// Walks both `block.body` (covers Apply / Construct / etc.) AND
/// `block.terminator` (covers Invoke — the only terminator that defines
/// a variable). The terminator walk is required because Invoke results
/// would otherwise be silently skipped, leaving terminator-defined call
/// results without `PlacementEligibilityCandidate` events even when their contract
/// narrowed locality.
pub(crate) fn populate_sparse_events(state_map: &mut AimsStateMap, func: &ArcFunction) {
    for (block_idx, block) in func.blocks.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let blk = ArcBlockId::new(block_idx as u32);

        for (instr_idx, instr) in block.body.iter().enumerate() {
            // Reusable allocation candidates: Construct with reusable ctor.
            // Use `is_excluded` (skips both scalars AND immortals) —
            // `is_scalar` alone leaks immortal identities into reuse
            // candidates. Immortal identity and lifetime are stable
            // contracts, so destructive reset/reuse cannot acquire them
            // regardless of the target's physical encoding.
            if let ArcInstr::Construct { dst, ctor, .. } = instr {
                if !state_map.is_excluded(*dst)
                    && matches!(ctor, CtorKind::Struct(_) | CtorKind::EnumVariant { .. })
                {
                    state_map.record_event(AimsEvent::ReusableAllocation {
                        block: blk,
                        instr: instr_idx,
                        var: *dst,
                    });
                }
            }

            // Placement eligibility: variables with a bounded exit lifetime.
            // Uses `effective_locality_at_block_exit` (NOT raw lattice
            // locality) so contract-narrowed call results surface here.
            // `is_excluded` excludes both scalars and immortals from
            // placement eligibility.
            if let Some(dst) = instr.defined_var() {
                if state_map.is_excluded(dst) {
                    continue;
                }
                let effective_loc = state_map.effective_locality_at_block_exit(blk, dst);
                if matches!(
                    effective_loc,
                    Locality::FunctionLocal | Locality::BlockLocal
                ) {
                    state_map.record_event(AimsEvent::PlacementEligibilityCandidate {
                        block: blk,
                        instr: instr_idx,
                        var: dst,
                    });
                }
            }
        }

        // Invoke terminator's dst is also a forward-defined call result;
        // treat symmetrically to body-Apply per `populate_call_result_states`.
        // The body loop iterating `block.body` never visits the terminator —
        // Invoke results would otherwise be silently skipped. `is_excluded`
        // matches the body loop's exclusion criteria (scalars + immortals).
        if let ArcTerminator::Invoke { dst, .. } = &block.terminator {
            if !state_map.is_excluded(*dst) {
                let effective_loc = state_map.effective_locality_at_block_exit(blk, *dst);
                if matches!(
                    effective_loc,
                    Locality::FunctionLocal | Locality::BlockLocal
                ) {
                    state_map.record_event(AimsEvent::PlacementEligibilityCandidate {
                        block: blk,
                        // Terminator instruction index = body length
                        // (terminators are notionally at body-end).
                        instr: block.body.len(),
                        var: *dst,
                    });
                }
            }
        }
    }
}

/// Populate per-variable contract-narrowed call-result side tables on
/// [`AimsStateMap`] for every direct Apply/Invoke instruction.
///
/// Pipeline position: 1.5, between [`populate_borrow_sources`] (position 1)
/// and [`populate_sparse_events`] (position 2). The side tables MUST be
/// populated BEFORE `populate_sparse_events` reads them via
/// `effective_locality_at_block_exit`.
///
/// # Per-spec coverage
///
/// - **TF-6** (direct Apply/Invoke WITH contract): writes `contract.return_info`
///   dimensions (uniqueness, locality, shape) into the side tables, after
///   canonicalization. §3 TF-6 / TF-6a.
/// - **TF-5** (direct Apply WITHOUT contract): writes
///   `ReturnContract::CONSERVATIVE` (uniqueness=MaybeShared, locality=Unknown,
///   shape=NonReusable). NOT lattice BOTTOM — TF-5 says CONSERVATIVE, which
///   has `MaybeShared` uniqueness encoding the "unknown callee, runtime
///   `IsShared` check" semantics.
/// - **TF-5a / TF-6c** (indirect `ApplyIndirect` / InvokeIndirect): same
///   CONSERVATIVE per spec "Same as TF-5" — the spec mandates CONSERVATIVE
///   for spec-symmetric handling between direct-no-contract and indirect
///   calls.
///
/// # Canonicalization
///
/// Direct field writes from
/// `return_info` would bypass cross-dimensional feasibility invariants
/// (CN-3 Shared+ReusableCtor → `NonReusable`; CN-6 HeapEscaping+Unique →
/// `MaybeShared`). The pass builds a temporary `AimsState` from
/// CONSERVATIVE plus `return_info`, calls `canonicalize()` to enforce
/// CN-* rules, then extracts the canonicalized dimensions for side-table
/// writes.
///
/// # Sparse filter (BOTTOM-default)
///
/// `set_var_uniqueness` / `set_var_locality` skip BOTTOM values (Unique,
/// `BlockLocal`) — the side table stays sparse, and effective queries fall
/// through to the lattice (which is also BOTTOM by default) for those
/// values. CONSERVATIVE values (`MaybeShared`, Unknown) ARE stored — they
/// override the optimistic lattice default.
///
/// `set_var_shape` keeps its existing `NonReusable` filter (BOTTOM = CONSERVATIVE
/// for shape — they coincide).
///
/// # Excluded variables
///
/// Scalar and immortal variables are skipped via `is_excluded` — same
/// treatment as `populate_var_shapes`.
pub(crate) fn populate_call_result_states(
    state_map: &mut AimsStateMap,
    func: &ArcFunction,
    sigs: &FxHashMap<Name, MemoryContract>,
) {
    let canonicalize_contract_state = |return_info: ReturnContract| -> AimsState {
        let mut s = AimsState {
            // Call results are always Owned to caller; refine() does NOT
            // narrow access/consumption/cardinality/effect (per TF-6 spec).
            access: AccessClass::Owned,
            consumption: Consumption::Unrestricted,
            cardinality: Cardinality::Many,
            uniqueness: return_info.uniqueness,
            locality: return_info.locality,
            shape: return_info.shape,
            effect: EffectClass::ALL,
        };
        // Enforce CN-3 (Shared+ReusableCtor → NonReusable) and CN-6
        // (HeapEscaping+Unique → MaybeShared) before side-table writes.
        s.canonicalize();
        s
    };

    let write_canonicalized =
        |state_map: &mut AimsStateMap, dst: ArcVarId, return_info: ReturnContract| {
            let canonical = canonicalize_contract_state(return_info);
            state_map.set_var_uniqueness(dst, canonical.uniqueness);
            state_map.set_var_locality(dst, canonical.locality);
            state_map.set_var_shape(dst, canonical.shape);
        };

    for block in &func.blocks {
        // Body — direct Apply (with-or-without contract per TF-5/TF-6) AND
        // indirect ApplyIndirect (CONSERVATIVE per TF-5a).
        for instr in &block.body {
            match instr {
                ArcInstr::Apply {
                    dst, func: callee, ..
                } => {
                    if state_map.is_excluded(*dst) {
                        continue;
                    }
                    let return_info = sigs
                        .get(callee)
                        .map_or(ReturnContract::CONSERVATIVE, |c| c.return_info);
                    write_canonicalized(state_map, *dst, return_info);
                }
                ArcInstr::ApplyIndirect { dst, .. } => {
                    if !state_map.is_excluded(*dst) {
                        // TF-5a: indirect calls receive CONSERVATIVE
                        // (no contract available).
                        write_canonicalized(state_map, *dst, ReturnContract::CONSERVATIVE);
                    }
                }
                _ => {}
            }
        }
        // Terminator — direct Invoke (TF-6 / TF-6b) AND indirect
        // InvokeIndirect (TF-6c CONSERVATIVE).
        match &block.terminator {
            ArcTerminator::Invoke {
                dst, func: callee, ..
            } => {
                if !state_map.is_excluded(*dst) {
                    let return_info = sigs
                        .get(callee)
                        .map_or(ReturnContract::CONSERVATIVE, |c| c.return_info);
                    write_canonicalized(state_map, *dst, return_info);
                }
            }
            ArcTerminator::InvokeIndirect { dst, .. } => {
                if !state_map.is_excluded(*dst) {
                    write_canonicalized(state_map, *dst, ReturnContract::CONSERVATIVE);
                }
            }
            _ => {}
        }
    }
}

/// Populate the per-variable shape map from definition instructions.
///
/// Shape is a property of how a value was produced (its definition instruction),
/// not of backward demand. The backward analysis only carries shape within a
/// block's backward walk (set at `Construct`, killed at definition removal).
/// Cross-block, shape is lost because `add_backward_demand` initializes from
/// `BOTTOM` (which has `NonReusable` shape).
///
/// This post-convergence step makes shape available everywhere via
/// [`AimsStateMap::var_shape`], enabling:
/// - Death event filtering in reuse detection
/// - Cross-dimensional uniqueness proof (Once+ReusableCtor → static reuse)
/// - COW annotation for `CollectionBuffer` non-parameters
/// - TRMC candidate detection (`ContextHole`)
pub(crate) fn populate_var_shapes(state_map: &mut AimsStateMap, func: &ArcFunction) {
    for block in &func.blocks {
        for instr in &block.body {
            let Some(dst) = instr.defined_var() else {
                continue;
            };
            if state_map.is_excluded(dst) {
                continue;
            }
            let shape = match instr {
                ArcInstr::Construct { ctor, .. } | ArcInstr::Reuse { ctor, .. } => {
                    crate::aims::transfer::shape_from_ctor(ctor)
                }
                ArcInstr::CollectionReuse { .. } => ShapeClass::CollectionBuffer,
                _ => ShapeClass::NonReusable,
            };
            state_map.set_var_shape(dst, shape);
        }
    }
}

/// Propagate forward-state side-table entries (uniqueness, locality, shape)
/// across `Let { dst, value: Var(src) }` aliases.
///
/// TF-2 (`Let { Var(v) }` -> `dst.state := state(v)`): a var-binding inherits
/// its source's full lattice state. The side tables (`var_uniqueness`,
/// `var_locality`, `var_shapes`) carry the FORWARD dimensions of call results
/// because the backward dataflow re-initializes those dimensions from BOTTOM
/// (per `populate_var_shapes` doc). `populate_call_result_states` writes those
/// dimensions for `Apply`/`Invoke` dsts only; a `Let`-alias of such a result
/// inherits none, so its `effective_uniqueness_at_block_*` query falls through
/// to the BOTTOM lattice value (Unique). A seamless-slice result from
/// `ori_list_slice_drop` (`MaybeShared` on the call dst) is then read as Unique
/// at its alias's drop site, selecting the unique-owner free path on a shared
/// allocation. Spec: Annex E §AIMS (TF-2 variable-binding transfer).
///
/// Runs AFTER `populate_call_result_states` + `populate_var_shapes` so every
/// source side table is fully populated. Fixpoint over the alias edge set
/// handles transitive chains and arbitrary block ordering; monotone (each dst
/// dimension transitions unset -> set at most once) so it terminates.
/// Per-dst forward-alias propagation kind (TF-2/4/8/11).
///
/// - `Full` (TF-2 `Let { Var }`): the dst inherits the source's FULL lattice
///   state (uniqueness + locality + shape). Single source; first-write-wins.
/// - `View` (TF-4 `Project`): a projected field is a borrowed `NonReusable`
///   VIEW of the aggregate -- inherit uniqueness + locality ONLY (with a CN-6
///   re-check: `Unique` at wide locality demotes to `MaybeShared`), NEVER the
///   aggregate's shape. Single source; first-write-wins.
/// - `Join` (TF-8 `Select` / TF-11 Jump-arg -> block-param phi): the dst is the
///   lattice-JOIN (LUB) over ALL sources -- NEVER a meet, NEVER first-write-wins
///   (`Select(Unique, MaybeShared) -> MaybeShared`, the wider value). Re-joined
///   every fixpoint pass (monotone upward; terminates at finite lattice height),
///   so a loop back-edge phi arg converges. Shape NOT propagated (a Select/phi
///   merges DISTINCT allocations -- same-allocation reuse is the union-find's
///   job in `project_aliases`, kept distinct from this fact-join).
///
/// TF-15/15a `Set` / `SetTag` are EXCLUDED -- in-place mutations with no `dst`,
/// no forward transfer; mutation safety is TF-11-backward + DP-5 + RL-10.
enum AliasKind {
    Full,
    View,
    Join,
}

pub(crate) fn propagate_alias_forward_state(state_map: &mut AimsStateMap, func: &ArcFunction) {
    let edges = collect_alias_forward_edges(func);
    loop {
        let mut changed = false;
        for (dst, kind, sources) in &edges {
            let dst = *dst;
            if state_map.is_excluded(dst) {
                continue;
            }
            let step_changed = match kind {
                AliasKind::Full => step_full_alias(state_map, dst, sources[0]),
                AliasKind::View => step_view_alias(state_map, dst, sources[0]),
                AliasKind::Join => step_join_alias(state_map, dst, sources),
            };
            changed |= step_changed;
        }
        if !changed {
            break;
        }
    }
}

/// Build the per-dst forward-alias edge model `(dst, kind, sources)`. Multi-source
/// `Join` dsts LUB over all sources; single-source `Full`/`View` carry one src.
fn collect_alias_forward_edges(func: &ArcFunction) -> Vec<(ArcVarId, AliasKind, Vec<ArcVarId>)> {
    let mut edges: Vec<(ArcVarId, AliasKind, Vec<ArcVarId>)> = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } => edges.push((*dst, AliasKind::Full, vec![*src])),
                ArcInstr::Project { dst, value, .. } => {
                    edges.push((*dst, AliasKind::View, vec![*value]));
                }
                ArcInstr::Select {
                    dst,
                    true_val,
                    false_val,
                    ..
                } => edges.push((*dst, AliasKind::Join, vec![*true_val, *false_val])),
                // TF-15/15a Set/SetTag (+ every other variant): no forward edge.
                _ => {}
            }
        }
    }
    // TF-11 phi: each block-param joins the per-predecessor Jump arg at its
    // index. Reuse the `project_aliases` edge attribution (handles back-edges)
    // rather than re-deriving the predecessor scan; the same-allocation
    // union-find there stays DISTINCT from this forward fact-join.
    for (param, edge_args) in super::project_aliases::compute_param_edge_args(func) {
        let sources: Vec<ArcVarId> = edge_args.iter().map(|e| e.arg).collect();
        if !sources.is_empty() {
            edges.push((param, AliasKind::Join, sources));
        }
    }
    edges
}

/// TF-2 `Let { Var }`: dst inherits the source's FULL lattice state (uniqueness +
/// locality + shape). First-write-wins per dimension. Returns whether any
/// dimension changed.
fn step_full_alias(state_map: &mut AimsStateMap, dst: ArcVarId, src: ArcVarId) -> bool {
    let mut changed = false;
    if state_map.contract_uniqueness(dst).is_none() {
        if let Some(uniq) = state_map.contract_uniqueness(src) {
            state_map.set_var_uniqueness(dst, uniq);
            changed = true;
        }
    }
    if state_map.contract_locality(dst).is_none() {
        if let Some(loc) = state_map.contract_locality(src) {
            state_map.set_var_locality(dst, loc);
            changed = true;
        }
    }
    if matches!(state_map.var_shape(dst), ShapeClass::NonReusable) {
        let src_shape = state_map.var_shape(src);
        if !matches!(src_shape, ShapeClass::NonReusable) {
            state_map.set_var_shape(dst, src_shape);
            changed = true;
        }
    }
    changed
}

/// TF-4 `Project`: dst inherits uniqueness + locality ONLY (NO shape), with the
/// CN-6 re-check that demotes `Unique` to `MaybeShared` at wide locality.
/// First-write-wins per dimension. Returns whether any dimension changed.
fn step_view_alias(state_map: &mut AimsStateMap, dst: ArcVarId, src: ArcVarId) -> bool {
    let mut changed = false;
    if state_map.contract_uniqueness(dst).is_none() {
        if let Some(mut uniq) = state_map.contract_uniqueness(src) {
            // CN-6: Unique demotes to MaybeShared at wide locality.
            if uniq == Uniqueness::Unique
                && matches!(
                    state_map.contract_locality(src),
                    Some(Locality::HeapEscaping | Locality::Unknown)
                )
            {
                uniq = Uniqueness::MaybeShared;
            }
            state_map.set_var_uniqueness(dst, uniq);
            changed = true;
        }
    }
    if state_map.contract_locality(dst).is_none() {
        if let Some(loc) = state_map.contract_locality(src) {
            state_map.set_var_locality(dst, loc);
            changed = true;
        }
    }
    changed
}

/// TF-8 `Select` / TF-11 phi: LUB over all sources for uniqueness + locality,
/// re-joined every pass (monotone upward, NOT first-write-wins). No shape.
/// Returns whether any dimension changed.
fn step_join_alias(state_map: &mut AimsStateMap, dst: ArcVarId, sources: &[ArcVarId]) -> bool {
    let mut changed = false;
    if let Some(joined) = sources
        .iter()
        .filter_map(|s| state_map.contract_uniqueness(*s))
        .reduce(Uniqueness::join)
    {
        if state_map
            .contract_uniqueness(dst)
            .is_none_or(|cur| joined > cur)
        {
            state_map.set_var_uniqueness(dst, joined);
            changed = true;
        }
    }
    if let Some(joined) = sources
        .iter()
        .filter_map(|s| state_map.contract_locality(*s))
        .reduce(Locality::join)
    {
        if state_map
            .contract_locality(dst)
            .is_none_or(|cur| joined > cur)
        {
            state_map.set_var_locality(dst, joined);
            changed = true;
        }
    }
    changed
}

/// Detect TRMC (Tail Recursive Modulo Constructor) candidates.
///
/// A `Construct` instruction is a TRMC candidate when:
/// 1. It has `ReusableCtor` shape (struct or enum variant)
/// 2. At least one of its field arguments was defined by a recursive call
///    (`Apply` or `Invoke` where callee == current function)
/// 3. **Soundness** (Lemma 2, Leijen & Lorenzen JFP 2025):
///    The constructor destination is `Unique` — no other references exist
///    at the mutation point, so in-place hole fill is safe.
///
/// # Soundness gates
///
/// TRMC requires TWO soundness gates:
///
/// 1. **Per-variable uniqueness (enforced):** The context variable must have
///    `Uniqueness::Unique` at the mutation point (enforced by the
///    `state.uniqueness != Uniqueness::Unique` guard in this scan).
///
/// 2. **Effect purity (logged, not enforced):** In principle,
///    `may_resume_nonlinearly` (derived from `EffectSummary.may_share`)
///    guards against non-linear effect handler resumption capturing the
///    context variable. However, the current `HeapEscaping → may_share`
///    accumulation rule makes ALL TRMC candidates trigger `may_share`,
///    so enforcing this gate blocks all TRMC. The correct formulation
///    depends on effect-handler semantics (not yet implemented). Until
///    then, gate 1 alone is sound because no mechanism for non-linear
///    resumption exists in Ori v1. See `contract/mod.rs` `ContextBehavior` doc.
///
/// Locality is deliberately NOT checked: TRMC constructors are typically
/// returned (`HeapEscaping`), which is expected — the whole point is
/// building the result in place and returning it.
///
/// When all conditions hold, the constructor's shape is upgraded to
/// `ContextHole`, enabling Stage 3 TRMC normalization to rewrite the
/// recursive call into an in-place fill of the constructor's hole.
pub(crate) fn detect_trmc_candidates(
    state_map: &mut AimsStateMap,
    func: &ArcFunction,
    may_share: bool,
) {
    // Collect variables defined by recursive calls (callee == func.name).
    // Uses the shared helper from normalize/detect.rs.
    let recursive_sites = crate::aims::normalize::collect_recursive_call_sites(func);
    let recursive_defs: FxHashSet<ArcVarId> = recursive_sites.into_keys().collect();
    if recursive_defs.is_empty() {
        return;
    }

    // Soundness gate 2: Effect purity — logged, not enforced.
    // In Ori v1, no effect handlers exist, so non-linear resumption cannot
    // occur. When effect handlers are implemented, this must be enforced
    // (or refined to exclude self-sharing from returned Constructs).
    // See contract/mod.rs ContextBehavior doc for the full design rationale.
    if may_share {
        tracing::trace!(
            func = ?func.name,
            "TRMC effect gate: may_share=true (logged, not enforced — \
             no effect handlers in v1)"
        );
    }

    // Scan for Construct instructions with a recursive-call argument.
    for (block_idx, block) in func.blocks.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let blk = ArcBlockId::new(block_idx as u32);

        for instr in &block.body {
            let ArcInstr::Construct {
                dst, ctor, args, ..
            } = instr
            else {
                continue;
            };

            // Only struct/enum constructors are TRMC candidates.
            if !matches!(ctor, CtorKind::Struct(_) | CtorKind::EnumVariant { .. }) {
                continue;
            }

            if state_map.is_excluded(*dst) {
                continue;
            }

            // Check if any field argument was produced by a recursive call.
            let has_recursive_arg = args.iter().any(|arg| recursive_defs.contains(arg));
            if !has_recursive_arg {
                continue;
            }

            // Soundness (Lemma 2, Leijen & Lorenzen JFP 2025):
            // The constructor must be uniquely owned at the mutation point.
            // This ensures the in-place hole fill doesn't corrupt other viewers.
            //
            // Locality is NOT checked: TRMC constructors are typically returned
            // (HeapEscaping), which is expected — the whole point of TRMC is to
            // build the result in place and return it.
            //
            // Function-level may_share is NOT checked: it's too conservative
            // (any returned Construct triggers may_share via HeapEscaping → may_share
            // rule). The per-variable Unique guarantee is the actual soundness
            // condition — exactly one logical owner at the mutation point.
            let state = state_map.var_state_at_block_exit(blk, *dst);
            if state.uniqueness != Uniqueness::Unique {
                continue;
            }

            // All conditions met — upgrade shape to ContextHole.
            state_map.set_var_shape(*dst, ShapeClass::ContextHole);
            tracing::debug!(
                func = ?func.name,
                var = dst.raw(),
                block = blk.raw(),
                "TRMC candidate detected: ContextHole shape set"
            );
        }
    }
}

/// Record `ContextOpen`/`ContextClose` events from normalize-provided context regions.
///
/// For each [`ContextRegion`] where the context variable has `ContextHole` shape
/// (set by `detect_trmc_candidates`) and is `Unique` at the block exit, records
/// paired events in the sparse event table.
///
/// # Soundness gates
///
/// Same two-gate model as `detect_trmc_candidates`:
///
/// 1. **Per-variable uniqueness (enforced):** The context variable must be
///    `Unique` at the open block exit. Double-checked here even though
///    `detect_trmc_candidates` already verified it at `ContextHole` marking.
///
/// 2. **Effect purity (logged, not enforced):** `may_share` is logged for
///    diagnostics but not enforced. See `detect_trmc_candidates` doc for
///    the full rationale.
///
/// Stage 3: `context_regions` produced by `aims::normalize::normalize_function()`.
pub(crate) fn populate_context_events(
    state_map: &mut AimsStateMap,
    func: &ArcFunction,
    context_regions: &[ContextRegion],
    may_share: bool,
) {
    if context_regions.is_empty() {
        return;
    }

    // Soundness gate 2: Effect purity — logged, not enforced.
    if may_share {
        tracing::trace!(
            func = ?func.name,
            "TRMC context events: may_share=true (logged, not enforced — \
             no effect handlers in v1)"
        );
    }

    for region in context_regions {
        // Skip regions for excluded (scalar/immortal) variables.
        if state_map.is_excluded(region.context_var) {
            continue;
        }

        // Soundness gate: the context variable must have ContextHole shape
        // (set by detect_trmc_candidates, which already checks uniqueness).
        if !matches!(
            state_map.var_shape(region.context_var),
            ShapeClass::ContextHole
        ) {
            tracing::trace!(
                func = ?func.name,
                var = region.context_var.raw(),
                shape = ?state_map.var_shape(region.context_var),
                "skipping context event: not ContextHole shape"
            );
            continue;
        }

        // Double-check uniqueness at the open block exit.
        let state = state_map.var_state_at_block_exit(region.open_block, region.context_var);
        if state.uniqueness != Uniqueness::Unique {
            tracing::trace!(
                func = ?func.name,
                var = region.context_var.raw(),
                uniqueness = ?state.uniqueness,
                "skipping context event: not Unique"
            );
            continue;
        }

        // Record paired ContextOpen/ContextClose events.
        state_map.record_event(AimsEvent::ContextOpen {
            block: region.open_block,
            instr: region.open_instr,
            var: region.context_var,
        });
        state_map.record_event(AimsEvent::ContextClose {
            block: region.close_block,
            instr: region.close_instr,
            var: region.hole_var,
        });

        tracing::debug!(
            func = ?func.name,
            context_var = region.context_var.raw(),
            hole_var = region.hole_var.raw(),
            hole_field = region.hole_field,
            "recorded TRMC context events (open + close)"
        );
    }
}

// Post-convergence transitive-drop singleton-class materialization.
//
// INVARIANT: every transitive-drop payload edge (Construct/PartialApply/
// Apply/Set/Invoke arg → dst) materializes a singleton `class_members` entry
// for both endpoints so the `class_members(class_id)` lookups in
// `realize/cleanup_redundant.rs` succeed for singleton classes (AIMS
// Invariant #5, unified model).

/// Materialize a singleton `class_members` entry for both endpoints of a
/// transitive-drop payload edge.
fn materialize_payload_edge_classes(
    arg: ArcVarId,
    dst: ArcVarId,
    func: &ArcFunction,
    state_map: &mut AimsStateMap,
) {
    if matches!(func.var_reprs.get(arg.index()), Some(&ValueRepr::Scalar)) {
        tracing::trace!(
            func = ?func.name,
            arg_var = arg.raw(),
            dst_var = dst.raw(),
            "materialize_payload_edge: skip — arg is scalar"
        );
        return;
    }
    let arg_class = state_map.class_id_of(arg);
    let dst_class = state_map.class_id_of(dst);
    if arg_class == dst_class {
        tracing::trace!(
            func = ?func.name,
            arg_var = arg.raw(),
            dst_var = dst.raw(),
            class = arg_class,
            "materialize_payload_edge: skip — self-loop"
        );
        return;
    }
    state_map.ensure_singleton_class(arg_class);
    state_map.ensure_singleton_class(dst_class);
}

/// Return `Some(RcStrategy)` for non-scalar `dst`, `None` for scalar.
fn dst_strategy_of(func: &ArcFunction, dst: ArcVarId) -> Option<RcStrategy> {
    *func.var_rc_strategies.get(dst.index())?
}

/// Materialize singleton `class_members` entries for transitive-drop edges.
///
/// Walks the 5 edge-recording sites (Construct/PartialApply/Apply/Set/Invoke)
/// AFTER `analyze_function`'s worklist returns the converged `AimsStateMap`,
/// using the same Path-c edge-eligibility as the union-find pass. For each
/// eligible edge, materializes a singleton `class_members` entry for both the
/// arg and dst classes so the `class_members(class_id)` consumers in the
/// realize walk (`cleanup_redundant.rs`) succeed for singleton
/// parents/children. Records no inter-class relation — singleton
/// `class_members` entries only, no predicate-stack edge map.
#[expect(
    clippy::too_many_lines,
    reason = "five edge-recording sites with structurally similar logic must be enumerated explicitly to preserve preconditions"
)]
pub(crate) fn materialize_transitive_drop_singleton_classes(
    func: &ArcFunction,
    sigs: &FxHashMap<Name, MemoryContract>,
    state_map: &mut AimsStateMap,
) {
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Construct { dst, args, .. }
                | ArcInstr::PartialApply { dst, args, .. } => {
                    let Some(strat) = dst_strategy_of(func, *dst) else {
                        continue;
                    };
                    if !is_transitive_drop_strategy(strat) {
                        continue;
                    }
                    for arg in args {
                        materialize_payload_edge_classes(*arg, *dst, func, state_map);
                    }
                }
                ArcInstr::Apply {
                    dst,
                    func: callee,
                    args,
                    arg_ownership,
                    ..
                } => {
                    let Some(strat) = dst_strategy_of(func, *dst) else {
                        continue;
                    };
                    if !is_transitive_drop_strategy(strat) {
                        continue;
                    }
                    for (i, arg) in args.iter().enumerate() {
                        // Path-c: edge eligibility = Owned-access
                        // OR contract claims param flows into the returned
                        // transitive-drop variant payload (e.g.,
                        // `wrap_ok(m) = Ok(m)` — Borrowed access but
                        // structurally contained in Result.payload).
                        let edge_eligible = match sigs.get(callee) {
                            Some(contract) => contract.params.get(i).map_or_else(
                                || {
                                    arg_ownership
                                        .get(i)
                                        .is_none_or(|o| matches!(o, ArgOwnership::Owned))
                                },
                                |p| {
                                    matches!(p.access, AccessClass::Owned)
                                        || p.return_payload_contains_param
                                },
                            ),
                            None => arg_ownership
                                .get(i)
                                .is_none_or(|o| matches!(o, ArgOwnership::Owned)),
                        };
                        if !edge_eligible {
                            continue;
                        }
                        materialize_payload_edge_classes(*arg, *dst, func, state_map);
                    }
                }
                ArcInstr::Set { base, value, .. } => {
                    let Some(strat) = dst_strategy_of(func, *base) else {
                        continue;
                    };
                    if !is_transitive_drop_strategy(strat) {
                        continue;
                    }
                    materialize_payload_edge_classes(*value, *base, func, state_map);
                }
                _ => {}
            }
        }
        if let ArcTerminator::Invoke {
            dst,
            func: callee,
            args,
            arg_ownership,
            ..
        } = &block.terminator
        {
            if let Some(strat) = dst_strategy_of(func, *dst) {
                if is_transitive_drop_strategy(strat) {
                    for (i, arg) in args.iter().enumerate() {
                        // Path-c: same edge-eligibility rule as the Apply arm
                        // (Owned-access OR contract-claimed payload containment).
                        let edge_eligible = match sigs.get(callee) {
                            Some(contract) => contract.params.get(i).map_or_else(
                                || {
                                    arg_ownership
                                        .get(i)
                                        .is_none_or(|o| matches!(o, ArgOwnership::Owned))
                                },
                                |p| {
                                    matches!(p.access, AccessClass::Owned)
                                        || p.return_payload_contains_param
                                },
                            ),
                            None => arg_ownership
                                .get(i)
                                .is_none_or(|o| matches!(o, ArgOwnership::Owned)),
                        };
                        if !edge_eligible {
                            continue;
                        }
                        materialize_payload_edge_classes(*arg, *dst, func, state_map);
                    }
                }
            }
        }
    }

    tracing::debug!(
        func = ?func.name,
        "materialize_transitive_drop_singleton_classes materialized singleton classes"
    );
}
