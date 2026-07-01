//! Apply-result allocation-identity analysis.
//!
//! Pre-walk pass that computes the `apply_result_aliases` side-table on
//! [`AimsStateMap`] from converged callee `MemoryContract`s. The table records,
//! for each Apply/Invoke destination at the caller, which argument(s) the
//! destination shares an allocation with — the callee's contract declares
//! the alias structurally via `ParamContract::return_alias`, regardless of
//! the param's own access class.
//!
//! Three cases:
//!
//! * **Single-param Direct** (`@id<T>(x: T) -> T = x`): one param has
//!   `return_alias = Some(Direct)`. dst aliases that arg directly.
//! * **Single-param Project** (`@unwrap<T>(b: Box<T>) -> T = b.inner`): one
//!   param has `return_alias = Some(Project { field })`. dst aliases the
//!   single-field projection of the arg.
//! * **Multi-param Conditional** (`match x { A -> a, B -> b }`-style callee):
//!   2+ params have `return_alias != None`. dst aliases ONE OF the candidates
//!   path-conditionally; the caller suppresses scope-exit decs on every
//!   candidate when the caller's local arg classification triggers
//!   suppression in `realize/walk.rs`.
//!
//! Caller-side dec suppression fires based on the CALLER's local Access of
//! the arg (Owned → suppress; Borrowed → no-op), evaluated in
//! `should_suppress_apply_aliased_dec` in `realize/walk.rs`. The
//! callee's contract Access governs only the callee's own RC accounting
//! (the callee-side compensating Inc on Project'd dst is gated on
//! Owned-callee in `build_return_project_inc_targets`).
//!
//! Pipeline ordering — PL-5 (no-stale-summary invariant):
//!
//! 1. `populate_apply_result_aliases(func, sigs)` — pre-walk (this file).
//! 2. `compute_project_alias_sources(func, &apply_result_aliases)` — composes
//!    Step 1b that seeds the alias graph from this side-table; Rules 2/3/4/6
//!    transitivity propagates the alias through Let/Jump/CFG-merge/nested-
//!    Project chains.
//! 3. `analyze_function` worklist — sees fully composed alias graph; backward
//!    walk never reads stale state.
//!
//! Read-only after step 1 completes; matches the `borrow_sources` invariant
//! per §1.9 Side-Table Domains.

use rustc_hash::FxHashMap;

use ori_ir::Name;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

use super::super::contract::{MemoryContract, ReturnAliasShape};
use super::state_map::ApplyAliasSource;

/// Build a function-wide map of Let Var alias destinations to their immediate
/// source variable. Used by [`is_let_var_alias`] to detect when a consumed
/// Apply arg is a transparent alias rather than a fresh-allocation owner.
///
/// `Let { dst, value: Var(src),.. }` — backward analysis transfers `dst`'s
/// demand to `src` (transparent alias). The
/// alias HAS NO INDEPENDENT RC slot at the IR level — `src`'s RC ops cover
/// the shared allocation. Recording an `apply_result_aliases` entry keyed off
/// a Let Var alias would mislead downstream consumers (BUG-04-090 session
/// D regression on `arc::test_rc_alias_owned_call_then_root_use`).
///
/// Indirect Let aliases (`%c = Var(%b)` where `%b = Var(%a)`) need full chain
/// tracing — see [`is_let_var_alias`].
///
/// Crate-visible: shared with `realize::emit_unified::build_return_project_inc_targets`
/// (BUG-04-090 F-prj fix) — both consumers answer the same "let-alias chain
/// root" question, so the resolution has one canonical home here.
pub(crate) fn build_let_alias_map(func: &ArcFunction) -> FxHashMap<ArcVarId, ArcVarId> {
    let mut result = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                result.insert(*dst, *src);
            }
        }
    }
    result
}

/// Walk every Apply/Invoke site in `func`, look up the callee's
/// `MemoryContract` in `sigs`, and emit an [`ApplyAliasSource`] entry for the
/// destination when the callee's contract carries `return_alias != None` for
/// one or more params.
///
/// Indirect calls (`ApplyIndirect`/`InvokeIndirect`) are bridged via
/// `closure_resolve::resolve_to_partial_apply_idx_eq`: when the closure
/// resolves to a known `PartialApply { func, capture_args }`, the captured
/// args are prepended to the user-args (`[capture_args..., user_args...]`)
/// and that combined list is matched against the resolved target's contract
/// in `sigs`. Unresolvable closures (opaque parameter, conflicting merges,
/// cycles) yield no entry — fresh-allocation semantics per TF-5a / TF-6c.
/// (Closure bridge.)
///
/// Empty when no in-scope callee transfers ownership through return — the
/// returned map allocates nothing in the common case.
pub(crate) fn populate_apply_result_aliases(
    func: &ArcFunction,
    sigs: &FxHashMap<Name, MemoryContract>,
) -> FxHashMap<ArcVarId, ApplyAliasSource> {
    let mut result = FxHashMap::default();

    // Build the closure def-map ONCE per function. Used by ApplyIndirect /
    // InvokeIndirect resolution to trace closure vars through `Let Var` /
    // `Jump` block-param chains back to a `PartialApply` origin.
    let closure_def_map = crate::rc_insert::closure_resolve::build_closure_def_map(&func.blocks);

    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Apply {
                    dst,
                    func: callee,
                    args,
                    ..
                } => {
                    if let Some(contract) = sigs.get(callee) {
                        install_alias_entry(&mut result, *dst, args, contract);
                    }
                }
                ArcInstr::ApplyIndirect {
                    dst, closure, args, ..
                } => {
                    if let Some((target, capture_args)) =
                        crate::rc_insert::closure_resolve::resolve_to_partial_apply_idx_eq(
                            *closure,
                            &closure_def_map,
                            &func.var_types,
                        )
                    {
                        if let Some(contract) = sigs.get(&target) {
                            // Combined arg list mirrors the PartialApply'd
                            // function's signature: capture-params first,
                            // then user-params. Same ordering used by
                            // `rc_insert::annotate::resolve_indirect_arg_ownership`
                            // when reusing the direct-call ownership path.
                            let mut combined = capture_args;
                            combined.extend_from_slice(args);
                            install_indirect_alias_entry(&mut result, *dst, &combined, contract);
                        }
                    }
                }
                _ => {}
            }
        }
        match &block.terminator {
            ArcTerminator::Invoke {
                dst,
                func: callee,
                args,
                ..
            } => {
                if let Some(contract) = sigs.get(callee) {
                    install_alias_entry(&mut result, *dst, args, contract);
                }
            }
            ArcTerminator::InvokeIndirect {
                dst, closure, args, ..
            } => {
                if let Some((target, capture_args)) =
                    crate::rc_insert::closure_resolve::resolve_to_partial_apply_idx_eq(
                        *closure,
                        &closure_def_map,
                        &func.var_types,
                    )
                {
                    if let Some(contract) = sigs.get(&target) {
                        let mut combined = capture_args;
                        combined.extend_from_slice(args);
                        install_indirect_alias_entry(&mut result, *dst, &combined, contract);
                    }
                }
            }
            _ => {}
        }
    }
    result
}

/// Single-call-site dispatch: classify the callee's contract into Direct /
/// Project / Conditional / no-entry and install the corresponding map entry.
///
/// Let Var aliases of a consumed arg are admitted into the
/// `ApplyAliasSource` map and deduplicated at realize-walk emission time via
/// the SSA alias class table (`class_members`). The
/// downstream `should_suppress_apply_aliased_dec` consumer fires dec
/// suppression based on the CALLER'S local Access of the arg (Owned →
/// suppress; Borrowed → no-op), and class membership prevents double-suppression
/// across alias siblings. The earlier BUG-04-090 SKIP-rule
/// was superseded by the class-aware emission path; the regression
/// `arc::test_rc_alias_owned_call_then_root_use` is now guarded by the
/// class-membership check at the realize walk rather than by skipping at install
/// time.
fn install_alias_entry(
    result: &mut FxHashMap<ArcVarId, ApplyAliasSource>,
    dst: ArcVarId,
    args: &[ArcVarId],
    contract: &MemoryContract,
) {
    // Record alias entries for any param structurally flowing to a Direct
    // or Project Return, regardless of the param's own access. The realize
    // walk's `should_suppress_apply_aliased_dec` consumer fires the dec
    // suppression based on the CALLER'S local Access of the arg (Owned →
    // suppress; Borrowed → no-op). The callee's contract Access governs
    // the callee's own RC accounting (its scope-exit AggFields walk fires
    // only when access is Owned), not the caller's.
    //
    // Wrapped variant: handles two
    // distinct alias shapes via the same install path:
    // - Identity (return_alias = Some): callee returns the param itself
    //   (Direct) or a single-field projection (Project). `uf.union` fires
    //   in ssa_alias_classes for Direct (caller-arg and result are the
    //   SAME RC slot). BUG-04-090 shape.
    // - Containment (return_alias = None ∧ return_payload_contains_param =
    //   true): callee constructs a transitive-drop variant containing an
    //   alias of the param (`wrap_ok(m) = Ok(m)`). PIN-2 analogous: NO
    //   `uf.union` in ssa_alias_classes (Result and wrapped allocation are
    //   SEPARATE RC slots), and NO seeding of `project_alias_sources`
    //   Step 1b (containment is NOT a projection-derived alias chain).
    //   Suppresses ONLY the redundant caller-side canonical dec on arg via
    //   `should_suppress_apply_aliased_dec`; downstream apply-aliased
    //   projections of the result (e.g., `extracted = inner` from a match
    //   arm) stay in their own classes and keep their canonical decs.
    //
    // Multi-param Wrapped (e.g. `pair(a, b) = (a, b)` returning a tuple)
    // is NOT producible by `find_payload_containment_params`
    // (which only marks EnumVariant Construct + PartialApply per
    // `extract.rs`).
    let aliasing_params: Vec<usize> = contract
        .params
        .iter()
        .enumerate()
        .filter(|(_, p)| p.return_alias.is_some() || p.return_payload_contains_param)
        .map(|(i, _)| i)
        .collect();
    install_alias_entry_inner(result, dst, args, contract, &aliasing_params, false);
}

/// Closure-bridge install path for `ApplyIndirect` /
/// `InvokeIndirect`. Maps every aliasing param shape to `Wrapped(arg)`
/// regardless of `return_alias` discriminant.
///
/// **Why Wrapped, not Direct:** for direct calls, `Direct(arg)` triggers
/// `uf.union(dst, arg)` in `compute_ssa_alias_classes` so the caller's
/// arg and call result share an RC slot — correct for single-call
/// semantics like `let r = id(x)`. For indirect closure calls,
/// the closure can be invoked N times (`first = lookup`, `second =
/// lookup`); union'ing every result with the captured arg into one
/// class produces N scope-exit decs against a single underlying inc,
/// causing double-free on the captured allocation. The `Wrapped` variant
/// preserves the per-call result class while suppressing only the
/// caller's redundant canonical dec on the captured arg via
/// `should_suppress_apply_aliased_dec` — same shape used by the
/// `wrap_ok(m: m) = Ok(m)` containment case.
fn install_indirect_alias_entry(
    result: &mut FxHashMap<ArcVarId, ApplyAliasSource>,
    dst: ArcVarId,
    args: &[ArcVarId],
    contract: &MemoryContract,
) {
    let aliasing_params: Vec<usize> = contract
        .params
        .iter()
        .enumerate()
        .filter(|(_, p)| p.return_alias.is_some() || p.return_payload_contains_param)
        .map(|(i, _)| i)
        .collect();
    install_alias_entry_inner(result, dst, args, contract, &aliasing_params, true);
}

/// Shared install body. `force_wrapped = true` collapses every
/// single-param shape (`Direct` / `Project` / `containment`) to
/// `ApplyAliasSource::Wrapped(consumed_arg)` for the indirect-call
/// path; `false` preserves the discriminant per the original direct-
/// call install logic.
fn install_alias_entry_inner(
    result: &mut FxHashMap<ArcVarId, ApplyAliasSource>,
    dst: ArcVarId,
    args: &[ArcVarId],
    contract: &MemoryContract,
    aliasing_params: &[usize],
    force_wrapped: bool,
) {
    match aliasing_params.len() {
        0 => {
            // No entry — callee return is fresh w.r.t. caller-side aliasing.
        }
        1 => {
            let param_idx = aliasing_params[0];
            let Some(consumed_arg) = args.get(param_idx).copied() else {
                // Arg-count mismatch with contract param-count — should not
                // occur post type-check; skip defensively.
                return;
            };
            // Let aliases live in the union-find directly via
            // build_let_alias_map, so class structure unites caller-arg and
            // result classes for identity functions (e.g., id<T>(x: T) -> T),
            // preventing PIN-6 from spuriously recording a payload-of edge and
            // then over-suppressing the child class's dec.
            let alias_shape_opt = contract.params[param_idx].return_alias;
            let contains = contract.params[param_idx].return_payload_contains_param;
            let entry = if force_wrapped {
                // Closure-bridge: collapse every single-param
                // shape to `Wrapped(consumed_arg)`. See
                // `install_indirect_alias_entry` for the rationale (closure
                // can be called N times; union'ing every result with the
                // captured arg double-frees the underlying allocation).
                ApplyAliasSource::Wrapped(consumed_arg)
            } else {
                match alias_shape_opt {
                    Some(ReturnAliasShape::Direct) => ApplyAliasSource::Direct(consumed_arg),
                    Some(ReturnAliasShape::Project { field }) => ApplyAliasSource::Project {
                        arg: consumed_arg,
                        field,
                    },
                    None if contains => ApplyAliasSource::Wrapped(consumed_arg),
                    None => {
                        // Filter guarantees at least one of the two flags is true;
                        // None-and-not-contains contradicts the filter.
                        unreachable!(
                            "BUG: aliasing_params filter ensures return_alias.is_some() OR return_payload_contains_param; reached neither branch on param_idx={param_idx}"
                        )
                    }
                }
            };
            result.insert(dst, entry);
        }
        _ => {
            // Conditional case (E-mat): 2+ Owned params alias the return at
            // runtime. Suppress all candidates' scope-exit decs in `realize/walk.rs`;
            // dst's RC ops are retained as the canonical owner.
            //
            // Phase 4 #6: is_let_var_alias
            // filter removed — subsumed by class structure.
            let candidates: Vec<ArcVarId> = aliasing_params
                .iter()
                .filter_map(|&idx| args.get(idx).copied())
                .collect();
            if !candidates.is_empty() {
                result.insert(dst, ApplyAliasSource::Conditional { candidates });
            }
        }
    }
}
