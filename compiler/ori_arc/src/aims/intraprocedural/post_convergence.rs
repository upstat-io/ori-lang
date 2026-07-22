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
    is_transitive_drop_strategy, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId,
    ArgOwnership, CtorKind,
};

use super::super::contract::{MemoryContract, ReturnContract};
use super::super::lattice::{
    AccessClass, AimsState, Cardinality, Consumption, EffectClass, Locality, ShapeClass,
};
use super::super::transfer::transfer_def_resolved;
use super::state_map::{AimsEvent, AimsStateMap};

mod alias_forward;
mod transitive_drop;
mod trmc;

pub(crate) use alias_forward::propagate_alias_forward_state;
pub(crate) use trmc::{detect_trmc_candidates, populate_context_events};

use transitive_drop::{dst_strategy_of, materialize_payload_edge_classes};

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
    let yield_results: FxHashSet<_> = func
        .yield_allocations
        .iter()
        .map(|fact| fact.result)
        .collect();
    let returned_vars: FxHashSet<_> = func
        .blocks
        .iter()
        .filter_map(|block| match block.terminator {
            ArcTerminator::Return { value } => Some(value),
            _ => None,
        })
        .collect();
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
                // `ori_list_take` is an internal ownership transfer from an
                // untyped scratch handle to the typed list result. It has no
                // user-call contract; applying TF-5's unknown-call override
                // would erase the backward lifetime proof for this compiler-
                // generated identity. Stable lowering facts authorize the raw
                // lattice result only for that exact internal transfer.
                let effective_loc = if yield_results.contains(&dst) && !returned_vars.contains(&dst)
                {
                    state_map.var_state_at_block_exit(blk, dst).locality
                } else {
                    state_map.effective_locality_at_block_exit(blk, dst)
                };
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
