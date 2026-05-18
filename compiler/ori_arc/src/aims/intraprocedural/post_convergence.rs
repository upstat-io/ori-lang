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
    is_transitive_drop_strategy, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId,
    ArgOwnership, CtorKind, RcStrategy, ValueRepr,
};

use super::super::contract::{ContextRegion, MemoryContract, ReturnContract};
use super::super::lattice::{
    AccessClass, AimsState, Cardinality, Consumption, EffectClass, Locality, ReuseCtorKind,
    ShapeClass, Uniqueness,
};
use super::super::transfer::transfer_def;
use super::state_map::{AimsEvent, AimsStateMap};

/// §04A.3 ITEM-2 — populate `AimsStateMap::class_covered`.
///
/// `class_covered[C] = true` iff:
/// 1. Every var `v ∈ class_members[C]` has `func.burden_emitted[v.index()] = true`.
/// 2. Every payload class `P` transitively reachable from `C` via
///    `class_payload_of` is also in `class_covered`.
///
/// Fixed-point iteration over the finite class set — terminates per
/// `aims-rules.md §1.8 L-5` (`class_members` is finite; `class_covered`
/// only grows; iteration halts when one full pass adds zero classes).
///
/// Coexistence handshake per `plans/aims-burden-tracking/section-04A-minimal-
/// lattice-adaptation.md §04A.3 ITEM-2`. AIMS Invariant #5(c) — derives
/// purely from existing `class_members` + `class_payload_of` +
/// `func.burden_emitted` side tables; no parallel uniqueness tracker.
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
///    contract-derived narrowing is invisible. Plan TPR Round 0 F4.
///
/// Walks both `block.body` (covers Apply / Construct / etc.) AND
/// `block.terminator` (covers Invoke — the only terminator that defines
/// a variable). Plan TPR Round 4 F2 (gemini singleton medium GAP) added
/// the terminator walk after identifying that Invoke results were silently
/// skipped, leaving terminator-defined call results without
/// `LocalAllocCandidate` events even when their contract narrowed locality.
pub(crate) fn populate_sparse_events(state_map: &mut AimsStateMap, func: &ArcFunction) {
    for (block_idx, block) in func.blocks.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let blk = ArcBlockId::new(block_idx as u32);

        for (instr_idx, instr) in block.body.iter().enumerate() {
            // Reusable allocation candidates: Construct with reusable ctor.
            // Plan TPR Round 2 gemini F1: use `is_excluded` (skips both
            // scalars AND immortals) — `is_scalar` alone leaks immortal
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
            // Plan TPR Round 2 gemini F1: `is_excluded` excludes both
            // scalars and immortals from local-alloc candidacy.
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

        // Plan TPR Round 4 F2: Invoke terminator's dst is also a
        // forward-defined call result; treat symmetrically to body-Apply
        // per `populate_call_result_states`. The body loop above never
        // visits the terminator — Invoke results were silently skipped.
        // Plan TPR Round 2 gemini F1: `is_excluded` matches the body
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
/// `effective_locality_at_block_exit` (Plan TPR Round 0 F4 ordering).
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
///   `IsShared` check" semantics. Plan TPR Round 1 codex F2.
/// - **TF-5a / TF-6c** (indirect `ApplyIndirect` / InvokeIndirect): same
///   CONSERVATIVE per spec "Same as TF-5". Plan TPR Round 5 F2 corrected
///   prior round's "exclude indirect calls" treatment — the spec mandates
///   CONSERVATIVE for spec-symmetric handling between direct-no-contract
///   and indirect calls.
///
/// # Canonicalization
///
/// Plan TPR Round 5 F1 (codex critical): direct field writes from
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
/// override the optimistic lattice default. Plan TPR Round 1 F1.
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
/// Section 09.2 Shape Activation.
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
/// # Soundness gates (Section 13.2)
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
/// Section 09.2 Shape Activation — `ContextHole` detection.
pub(crate) fn detect_trmc_candidates(
    state_map: &mut AimsStateMap,
    func: &ArcFunction,
    may_share: bool,
) {
    // Collect variables defined by recursive calls (callee == func.name).
    // Uses the shared helper from normalize/detect.rs (Section 12.4a unification).
    let recursive_sites = crate::aims::normalize::collect_recursive_call_sites(func);
    let recursive_defs: FxHashSet<ArcVarId> = recursive_sites.into_keys().collect();
    if recursive_defs.is_empty() {
        return;
    }

    // Soundness gate 2 (Section 13.2): Effect purity — logged, not enforced.
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
/// # Soundness gates (Section 13.2)
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

    // Soundness gate 2 (Section 13.2): Effect purity — logged, not enforced.
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

// BUG-04-118 Option D — post-convergence class_payload_of population.
//
// Replaces the unsound population at step 4 (compute_ssa_alias_classes)
// which used syntactic ordering proxies that could not model
// path-sensitive lifetimes. Path-sensitive liveness from the converged
// AimsStateMap is the architecturally correct primitive per AIMS
// Invariant #5 (unified model).

/// Whether class A's lifetime extends past class B's destruction along some
/// CFG path, using the converged `AimsStateMap`'s path-sensitive liveness.
///
/// Two-tier check (Round 3 codex-F1 + gemini-F1 — intra-block gap close,
/// Round 4 gemini-F1 + Round 5 codex-F2 — terminator + defined-dead refinements):
/// 1. Block-exit tier: `is_live_at_exit` JOIN'd block-exit semantics.
/// 2. Intra-block tier: `precompute_block_uses` last-use comparison; only
///    fires when ALL B-members are dead-at-exit; defined-dead B detected
///    via `def_site_block`.
///
/// Witness-set widening (BUG-04-118 §05 Round 3 Option A — 3-of-3 reviewer
/// consensus): A's witness set is extended with Project-derived aliases of
/// B-members. Project apply-aliases live in a DIFFERENT class than their
/// source (PIN-2 at `ssa_alias_classes.rs:131` — "different RC slot"), so
/// `class_members(class_a)` cannot see them. The `project_alias_sources`
/// side-table on `AimsStateMap` records each alias-chain destination's root
/// sources; when any destination's sources include a B-member, the
/// destination's liveness extends A's effective lifetime.
///
/// Example (`apply_alias_result_strmap.ori`): `wrap_ok(m: m)` returns Result
/// wrapping `m`. In `@main`: `inner = Project result.payload[0]; extracted
/// = Let Var(inner)`. `project_alias_sources` records `inner → [result]` and
/// `extracted → [inner, result]`. Class B = `{result}`; class A = `{m}`.
/// Raw `a_members` is `{m}`, which is Dead post-Apply. But `extracted`
/// outlives `result`'s destructuring; treating `extracted`'s liveness as
/// part of A's witness set correctly identifies "A outlives B" and skips
/// the `class_payload_of` edge so PIN-6 does not over-suppress A's
/// canonical dec.
fn class_lifetime_extends_past_path_sensitive(
    class_a_id: u32,
    class_b_id: u32,
    def_site_block: usize,
    state_map: &AimsStateMap,
    func: &ArcFunction,
) -> bool {
    let Some(a_members) = state_map.class_members(class_a_id) else {
        return false;
    };
    let Some(b_members) = state_map.class_members(class_b_id) else {
        return false;
    };

    // BUG-04-118 §05 Round 3 Option A: build A's extended witness set by
    // walking project_alias_sources for any alias whose root sources include
    // a B-member. Witnesses are USED for "is A still alive?" checks (any_a
    // _live_exit, max_a_in_body, a_at_term) but NOT for B-related checks
    // (all_b_dead_exit stays based on real b_members per the lifetime check
    // semantic — B's destruction is what we're testing A's survival past).
    let extended_a_witnesses: FxHashSet<ArcVarId> = {
        let mut witnesses = a_members.iter().copied().collect::<FxHashSet<_>>();
        for (&alias_var, sources) in state_map.project_alias_sources() {
            if sources.iter().any(|src| b_members.contains(src)) {
                witnesses.insert(alias_var);
            }
        }
        witnesses
    };

    for blk_idx in 0..func.blocks.len() {
        let Ok(blk_u32) = u32::try_from(blk_idx) else {
            continue;
        };
        let blk = ArcBlockId::new(blk_u32);

        let any_a_live_exit = extended_a_witnesses
            .iter()
            .any(|m| crate::aims::emit_rc::is_live_at_exit(state_map, blk, *m));
        let all_b_dead_exit = b_members
            .iter()
            .all(|m| !crate::aims::emit_rc::is_live_at_exit(state_map, blk, *m));

        if any_a_live_exit && all_b_dead_exit {
            return true;
        }

        if !all_b_dead_exit {
            continue;
        }

        let use_info = crate::aims::emit_rc::precompute_block_uses(&func.blocks[blk_idx]);
        let any_b_used_in_block = b_members.iter().any(|m| use_info.contains_key(m));
        let any_b_defined_dead_in_block = def_site_block == blk_idx;
        if !any_b_used_in_block && !any_b_defined_dead_in_block {
            continue;
        }

        let max_a_in_body: Option<usize> = extended_a_witnesses
            .iter()
            .filter_map(|m| use_info.get(m))
            .filter_map(|(_count, lu)| match lu {
                crate::aims::emit_rc::LastUse::Body(i) => Some(*i),
                crate::aims::emit_rc::LastUse::Terminator => None,
            })
            .max();
        let max_b_in_body: Option<usize> = b_members
            .iter()
            .filter_map(|m| use_info.get(m))
            .filter_map(|(_count, lu)| match lu {
                crate::aims::emit_rc::LastUse::Body(i) => Some(*i),
                crate::aims::emit_rc::LastUse::Terminator => None,
            })
            .max();
        let a_at_term = extended_a_witnesses
            .iter()
            .filter_map(|m| use_info.get(m))
            .any(|(_, lu)| matches!(lu, crate::aims::emit_rc::LastUse::Terminator));
        let b_at_term = b_members
            .iter()
            .filter_map(|m| use_info.get(m))
            .any(|(_, lu)| matches!(lu, crate::aims::emit_rc::LastUse::Terminator));

        if let (Some(a_idx), Some(b_idx)) = (max_a_in_body, max_b_in_body) {
            if a_idx > b_idx && !b_at_term {
                return true;
            }
        }
        if a_at_term && !b_at_term && max_b_in_body.is_some() {
            return true;
        }
    }
    false
}

/// Centralized `class_payload_of` edge recording with Option D path-sensitive
/// lifetime check (BUG-04-118 §05.3).
fn record_payload_edge_lifetime(
    arg: ArcVarId,
    dst: ArcVarId,
    def_site_block: usize,
    func: &ArcFunction,
    state_map: &mut AimsStateMap,
    class_payload_of: &mut FxHashMap<u32, FxHashSet<u32>>,
) {
    if matches!(func.var_reprs.get(arg.index()), Some(&ValueRepr::Scalar)) {
        tracing::trace!(
            func = ?func.name,
            arg_var = arg.raw(),
            dst_var = dst.raw(),
            "BUG-04-118 record_payload_edge: skip — arg is scalar"
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
            "BUG-04-118 record_payload_edge: skip — self-loop"
        );
        return;
    }
    state_map.ensure_singleton_class(arg_class);
    state_map.ensure_singleton_class(dst_class);
    let outlives = class_lifetime_extends_past_path_sensitive(
        arg_class,
        dst_class,
        def_site_block,
        state_map,
        func,
    );
    tracing::debug!(
        func = ?func.name,
        arg_var = arg.raw(),
        dst_var = dst.raw(),
        arg_class,
        dst_class,
        a_outlives_b = outlives,
        action = if outlives { "SKIP" } else { "RECORD" },
        "BUG-04-118 record_payload_edge: predicate decision"
    );
    if outlives {
        return;
    }
    class_payload_of
        .entry(arg_class)
        .or_default()
        .insert(dst_class);
}

/// Return `Some(RcStrategy)` for non-scalar `dst`, `None` for scalar.
fn dst_strategy_of(func: &ArcFunction, dst: ArcVarId) -> Option<RcStrategy> {
    *func.var_rc_strategies.get(dst.index())?
}

/// Post-convergence `class_payload_of` population (BUG-04-118 §05.4).
///
/// Walks the 5 edge-recording sites (Construct/PartialApply/Apply/Set/Invoke)
/// AFTER `analyze_function`'s worklist returns the converged `AimsStateMap`.
/// For each candidate edge, applies the path-sensitive lifetime check from
/// `class_lifetime_extends_past_path_sensitive`; edges where A outlives B are
/// skipped. After collecting edges, materializes singleton class entries
/// (§05.4a) so PIN-6's `class_members(parent)` lookup succeeds for
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

    for (block_idx, block) in func.blocks.iter().enumerate() {
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
                            block_idx,
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
                        // BUG-04-118 path-c: edge eligibility = Owned-access
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
                            block_idx,
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
                        block_idx,
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
                        // BUG-04-118 path-c: see Apply branch above.
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
                            block_idx,
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
        "BUG-04-118 §05.4 populate_class_payload_of_with_liveness installed path-sensitive edge map"
    );

    state_map.set_class_payload_of(class_payload_of);
}

/// BUG-04-118 §05 Round 2 /tp-help — populate the same-class dec obligation
/// table.
///
/// Per multi-member SSA alias class C, per block B: identifies which class
/// members have last-use within B (intra-block obligations) and which class
/// members are live at B's exit (continuing into successor blocks). The
/// resulting `(B, C) → ClassObligationEntry` map is consumed by
/// `walk_dec.rs::class_alive_after` for path-sensitive same-slot dec dedup
/// across `Let{Var}` / `Jump` arg / `Conditional` alias chains.
///
/// Same-class semantics: members of one class share an RC slot via
/// `compute_ssa_alias_classes`'s `union_let_aliases` + `Jump` arg → block
/// param unioning. Emission must schedule one terminal dec per class at
/// the LAST obligation point. Singleton classes (one member) need no
/// dedup and are skipped.
///
/// Pipeline placement: post-convergence (Step 4.5) alongside
/// `populate_class_payload_of_with_liveness`. Both consume the converged
/// `AimsStateMap` lattice + path-sensitive liveness primitive
/// `is_live_at_exit`. Read-only thereafter (PL-5).
pub(crate) fn populate_class_dec_obligations(state_map: &mut AimsStateMap, func: &ArcFunction) {
    use super::state_map::ClassObligationEntry;

    // Build inverse: class_id → set of member vars (only multi-member
    // classes need dedup).
    let mut class_to_members: FxHashMap<u32, FxHashSet<ArcVarId>> = FxHashMap::default();
    for var_idx in 0..func.var_types.len() {
        let Ok(var_u32) = u32::try_from(var_idx) else {
            continue;
        };
        let var = ArcVarId::new(var_u32);
        if let Some(class_id) = state_map.ssa_alias_class_of(var) {
            class_to_members.entry(class_id).or_default().insert(var);
        }
    }

    let mut obligations: FxHashMap<(ArcBlockId, u32), ClassObligationEntry> = FxHashMap::default();

    for (class_id, members) in &class_to_members {
        if members.len() < 2 {
            continue; // singleton class — no same-class dedup needed.
        }
        for blk_idx in 0..func.blocks.len() {
            let Ok(blk_u32) = u32::try_from(blk_idx) else {
                continue;
            };
            let blk = ArcBlockId::new(blk_u32);
            let block = &func.blocks[blk_idx];

            let mut intra_block_obligations: Vec<(ArcVarId, usize)> = Vec::new();
            let mut block_exit_members: FxHashSet<ArcVarId> = FxHashSet::default();

            for &member in members {
                // Member live at block exit → obligation rolls forward to
                // successor block; no intra-block dec for this member here.
                if crate::aims::emit_rc::is_live_at_exit(state_map, blk, member) {
                    block_exit_members.insert(member);
                    continue;
                }

                // Member dies in this block: find last-use instr_idx.
                // Terminator usage takes priority (instr_idx = body.len());
                // otherwise scan body in reverse for first use.
                let last_use = if block.terminator.used_vars().contains(&member) {
                    Some(block.body.len())
                } else {
                    block
                        .body
                        .iter()
                        .enumerate()
                        .rev()
                        .find_map(|(idx, instr)| instr.used_vars().contains(&member).then_some(idx))
                };

                if let Some(idx) = last_use {
                    intra_block_obligations.push((member, idx));
                }
            }

            intra_block_obligations.sort_by_key(|&(_, idx)| idx);

            if !intra_block_obligations.is_empty() || !block_exit_members.is_empty() {
                obligations.insert(
                    (blk, *class_id),
                    ClassObligationEntry {
                        intra_block_obligations,
                        block_exit_members,
                    },
                );
            }
        }
    }

    tracing::debug!(
        target: "ori_arc::aims::intraprocedural::post_convergence",
        "BUG-04-118 §05 Round 2 populate_class_dec_obligations installed table with {} entries",
        obligations.len()
    );

    state_map.set_class_dec_obligations(obligations);
}
