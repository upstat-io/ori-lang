//! Post-convergence passes for intraprocedural analysis.
//!
//! After the backward dataflow analysis converges, these passes populate
//! side tables in the [`AimsStateMap`] using the converged state:
//!
//! - [`populate_borrow_sources`] — borrow source tracking for Project instructions
//! - [`populate_sparse_events`] — reusable allocations + local-alloc candidates
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
    AccessClass, AimsState, Cardinality, Consumption, EffectClass, Locality, ReuseCtorKind,
    ShapeClass, Uniqueness,
};
use super::super::transfer::transfer_def;
use super::state_map::{AimsEvent, AimsStateMap};

/// Populate `AimsStateMap::class_covered` for the coexistence handshake.
///
/// `class_covered[C] = true` iff:
/// 1. Every var `v ∈ class_members[C]` has `func.burden_emitted[v.index()] = true`.
/// 2. Every payload class `P` transitively reachable from `C` via
///    `class_payload_of` is also in `class_covered`.
///
/// Fixed-point iteration over the finite class set — terminates per L-5
/// (`class_members` is finite; `class_covered` only grows; iteration halts
/// when one full pass adds zero classes).
///
/// AIMS Invariant #5(c) — derives purely from existing `class_members` +
/// `class_payload_of` + `func.burden_emitted` side tables; no parallel
/// uniqueness tracker.
pub(crate) fn populate_class_covered(state_map: &mut AimsStateMap, func: &ArcFunction) {
    // Empty burden_emitted means the burden walker never ran or emitted
    // nothing — no class can be covered (DP-2/DP-3 elimination would still
    // see zero burden ops). Short-circuit.
    if func.burden_emitted.is_empty() || !func.burden_emitted.iter().any(|b| *b) {
        return;
    }

    // Step 1: candidate set — classes whose every member has burden_emitted=true.
    // Computed once; subsequent fixed-point passes only check transitive
    // payload coverage, never re-check membership.
    let candidate_classes: Vec<(u32, Vec<u32>)> = state_map
        .class_members_iter()
        .filter_map(|(class_id, members)| {
            let all_emitted = members
                .iter()
                .all(|v| func.burden_emitted.get(v.index()).copied().unwrap_or(false));
            if !all_emitted {
                return None;
            }
            // Pre-collect transitive payload class ids per `class_payload_of`.
            let payloads: Vec<u32> = state_map
                .class_payload_of(class_id)
                .map(|s| s.iter().copied().collect())
                .unwrap_or_default();
            Some((class_id, payloads))
        })
        .collect();

    if candidate_classes.is_empty() {
        return;
    }

    // Step 2: fixed-point iteration. A class is covered iff it is a candidate
    // AND every payload class is covered. Terminates because `covered` only
    // grows; bounded by `candidate_classes.len()`.
    let mut covered: FxHashSet<u32> = FxHashSet::default();
    loop {
        let mut grew = false;
        for (class_id, payloads) in &candidate_classes {
            if covered.contains(class_id) {
                continue;
            }
            let payloads_covered = payloads.iter().all(|p| covered.contains(p));
            if payloads_covered {
                covered.insert(*class_id);
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }

    state_map.set_class_covered(covered);
}

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
            // Use the converged state to compute the transfer result.
            let get_state = |v: ArcVarId| -> AimsState {
                if state_map.is_scalar(v) {
                    return AimsState::SCALAR;
                }
                state_map.var_state_at_block_entry(block.id, v)
            };

            if let Some(def) = transfer_def(instr, &get_state) {
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
/// 2. **Local-allocation eligibility** (`AimsEvent::LocalAllocCandidate`):
///    Variables whose effective exit-locality (`effective_locality_at_block_exit`,
///    which JOINs the lattice value with the contract-narrowed value populated
///    by `populate_call_result_states`) shows `Locality::FunctionLocal` or
///    `BlockLocal`. The effective query is load-bearing for direct call results:
///    without it, a callee with `return_info.locality = FunctionLocal` would
///    not surface as a `LocalAllocCandidate` because the lattice's BOTTOM
///    locality is `BlockLocal` (already FunctionLocal-eligible) but the
///    contract-derived narrowing is invisible.
///
/// Walks both `block.body` (covers Apply / Construct / etc.) AND
/// `block.terminator` (covers Invoke — the only terminator that defines
/// a variable). The terminator walk is required because Invoke results
/// would otherwise be silently skipped, leaving terminator-defined call
/// results without `LocalAllocCandidate` events even when their contract
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
            // `is_scalar` alone leaks immortal
            // heap-allocated constants into reuse candidates, where they
            // cannot be reused because their MAX_REFCOUNT prevents the
            // reset/reuse pipeline from acquiring the allocation.
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

            // Local-allocation eligibility: variables with local exit state.
            // Uses `effective_locality_at_block_exit` (NOT raw lattice
            // locality) so contract-narrowed call results surface here.
            // `is_excluded` excludes both scalars and immortals from
            // local-alloc candidacy.
            if let Some(dst) = instr.defined_var() {
                if state_map.is_excluded(dst) {
                    continue;
                }
                let effective_loc = state_map.effective_locality_at_block_exit(blk, dst);
                if matches!(
                    effective_loc,
                    Locality::FunctionLocal | Locality::BlockLocal
                ) {
                    state_map.record_event(AimsEvent::LocalAllocCandidate {
                        block: blk,
                        instr: instr_idx,
                        var: dst,
                    });
                }
            }
        }

        // Invoke terminator's dst is also a forward-defined call result;
        // treat symmetrically to body-Apply per `populate_call_result_states`.
        // The body loop above never visits the terminator — Invoke results
        // would otherwise be silently skipped. `is_excluded` matches the body
        // loop's exclusion criteria (scalars + immortals).
        if let ArcTerminator::Invoke { dst, .. } = &block.terminator {
            if !state_map.is_excluded(*dst) {
                let effective_loc = state_map.effective_locality_at_block_exit(blk, *dst);
                if matches!(
                    effective_loc,
                    Locality::FunctionLocal | Locality::BlockLocal
                ) {
                    state_map.record_event(AimsEvent::LocalAllocCandidate {
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
///
/// Shape Activation.
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
                ArcInstr::Construct { ctor, .. } | ArcInstr::Reuse { ctor, .. } => match ctor {
                    CtorKind::Struct(_) => ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
                    CtorKind::EnumVariant { .. } => {
                        ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant)
                    }
                    CtorKind::ListLiteral | CtorKind::SetLiteral | CtorKind::MapLiteral => {
                        ShapeClass::CollectionBuffer
                    }
                    CtorKind::Tuple | CtorKind::Closure { .. } => ShapeClass::NonReusable,
                },
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
/// allocation. Spec: aims-rules.md §3 TF-2.
///
/// Runs AFTER `populate_call_result_states` + `populate_var_shapes` so every
/// source side table is fully populated. Fixpoint over the alias edge set
/// handles transitive chains and arbitrary block ordering; monotone (each dst
/// dimension transitions unset -> set at most once) so it terminates.
pub(crate) fn propagate_alias_forward_state(state_map: &mut AimsStateMap, func: &ArcFunction) {
    let mut aliases: Vec<(ArcVarId, ArcVarId)> = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                aliases.push((*dst, *src));
            }
        }
    }

    loop {
        let mut changed = false;
        for &(dst, src) in &aliases {
            if state_map.is_excluded(dst) {
                continue;
            }
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
        }
        if !changed {
            break;
        }
    }
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
///    `Uniqueness::Unique` at the mutation point. Checked below.
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
///
/// Shape Activation — `ContextHole` detection.
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
            // condition — refcount == 1 at the mutation point.
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

// Post-convergence class_payload_of population.
//
// INVARIANT: payload edges are populated from path-sensitive liveness in
// the converged AimsStateMap — syntactic ordering proxies cannot model
// path-sensitive lifetimes (AIMS Invariant #5, unified model).

/// Centralized `class_payload_of` edge recording.
fn record_payload_edge_lifetime(
    arg: ArcVarId,
    dst: ArcVarId,
    func: &ArcFunction,
    state_map: &mut AimsStateMap,
    class_payload_of: &mut FxHashMap<u32, FxHashSet<u32>>,
) {
    if matches!(func.var_reprs.get(arg.index()), Some(&ValueRepr::Scalar)) {
        tracing::trace!(
            func = ?func.name,
            arg_var = arg.raw(),
            dst_var = dst.raw(),
            "record_payload_edge: skip — arg is scalar"
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
            "record_payload_edge: skip — self-loop"
        );
        return;
    }
    state_map.ensure_singleton_class(arg_class);
    state_map.ensure_singleton_class(dst_class);
    // Path-sensitive predicate-stack-soundness patches (container-destructure
    // skip + class-lifetime "outlives" skip) retired: the burden path is the
    // sole RC emitter and the predicate-stack consumers of this edge map (PIN-6)
    // are retired, so the plain payload-of edge is recorded unconditionally.
    class_payload_of
        .entry(arg_class)
        .or_default()
        .insert(dst_class);
}

/// Return `Some(RcStrategy)` for non-scalar `dst`, `None` for scalar.
fn dst_strategy_of(func: &ArcFunction, dst: ArcVarId) -> Option<RcStrategy> {
    *func.var_rc_strategies.get(dst.index())?
}

/// Post-convergence `class_payload_of` population.
///
/// Walks the 5 edge-recording sites (Construct/PartialApply/Apply/Set/Invoke)
/// AFTER `analyze_function`'s worklist returns the converged `AimsStateMap`.
/// For each candidate edge, applies the path-sensitive lifetime check from
/// `class_lifetime_extends_past_path_sensitive`; edges where A outlives B are
/// skipped. After collecting edges, materializes singleton class entries
/// so PIN-6's `class_members(parent)` lookup succeeds for
/// singleton parents/children, then installs via `set_class_payload_of`.
#[expect(
    clippy::too_many_lines,
    reason = "five edge-recording sites with structurally similar logic must be enumerated explicitly to preserve preconditions"
)]
pub(crate) fn populate_class_payload_of_with_liveness(
    func: &ArcFunction,
    sigs: &FxHashMap<Name, MemoryContract>,
    state_map: &mut AimsStateMap,
) {
    let mut class_payload_of: FxHashMap<u32, FxHashSet<u32>> = FxHashMap::default();

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
                        record_payload_edge_lifetime(
                            *arg,
                            *dst,
                            func,
                            state_map,
                            &mut class_payload_of,
                        );
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
                        record_payload_edge_lifetime(
                            *arg,
                            *dst,
                            func,
                            state_map,
                            &mut class_payload_of,
                        );
                    }
                }
                ArcInstr::Set { base, value, .. } => {
                    let Some(strat) = dst_strategy_of(func, *base) else {
                        continue;
                    };
                    if !is_transitive_drop_strategy(strat) {
                        continue;
                    }
                    record_payload_edge_lifetime(
                        *value,
                        *base,
                        func,
                        state_map,
                        &mut class_payload_of,
                    );
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
                        // Path-c: see Apply branch above.
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
                        record_payload_edge_lifetime(
                            *arg,
                            *dst,
                            func,
                            state_map,
                            &mut class_payload_of,
                        );
                    }
                }
            }
        }
    }

    let class_ids_to_materialize: Vec<u32> = class_payload_of
        .iter()
        .flat_map(|(&child_class, parent_classes)| {
            std::iter::once(child_class).chain(parent_classes.iter().copied())
        })
        .collect();
    for class_id in class_ids_to_materialize {
        state_map.ensure_singleton_class(class_id);
    }

    tracing::debug!(
        func = ?func.name,
        edges = class_payload_of.len(),
        "populate_class_payload_of_with_liveness installed path-sensitive edge map"
    );

    state_map.set_class_payload_of(class_payload_of);
}
