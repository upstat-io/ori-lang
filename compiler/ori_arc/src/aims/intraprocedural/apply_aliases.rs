//! Apply-result allocation-identity analysis (BUG-04-090).
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
//! `should_suppress_apply_aliased_dec` at File 13 (`realize/walk.rs`). The
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
/// `Let { dst, value: Var(src), .. }` — backward analysis transfers `dst`'s
/// demand to `src` per `aims-rules.md §IA-5 step 1` (transparent alias). The
/// alias HAS NO INDEPENDENT RC slot at the IR level — `src`'s RC ops cover
/// the shared allocation. Recording an `apply_result_aliases` entry keyed off
/// a Let Var alias would mislead downstream consumers (BUG-04-090 §05 session
/// D regression on `arc::test_rc_alias_owned_call_then_root_use`).
///
/// Indirect Let aliases (`%c = Var(%b)` where `%b = Var(%a)`) need full chain
/// tracing — see [`is_let_var_alias`].
///
/// Crate-visible: shared with `realize::emit_unified::build_return_project_inc_targets`
/// (BUG-04-090 §05 F-prj fix) per `LEAK:algorithmic-duplication` — both consumers
/// answer the same "let-alias chain root" question.
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

/// Whether `var` is defined by a `Let { value: Var(_) }` alias instruction.
/// Returns false for fresh-allocation roots (`Construct` / `Apply` result /
/// `Reuse` / `PartialApply` / `Project` / function parameter).
///
/// BUG-04-090 §05 Hypothesis D component #2: caller-side allocation-identity
/// suppression must skip Let Var aliases. The alias has no independent RC
/// slot; recording an `ApplyAliasSource` entry keyed on it provides no
/// information the existing alias machinery does not already track, AND
/// trips downstream consumers into suppressing decs that balance Let-Var
/// emitted incs upstream (per `aims-rules.md §IA-5 step 1` Let-Var demand
/// transfer + cardinality `seq_add`).
fn is_let_var_alias(let_alias_map: &FxHashMap<ArcVarId, ArcVarId>, var: ArcVarId) -> bool {
    let_alias_map.contains_key(&var)
}

/// Walk every Apply/Invoke site in `func`, look up the callee's
/// `MemoryContract` in `sigs`, and emit an [`ApplyAliasSource`] entry for the
/// destination when the callee's contract carries `return_alias != None` for
/// one or more params.
///
/// Indirect calls (`ApplyIndirect`/`InvokeIndirect`) have no contract and
/// produce no entry — fresh-allocation semantics per the conservative TF-5a /
/// TF-6c default.
///
/// Empty when no in-scope callee transfers ownership through return — the
/// returned map allocates nothing in the common case.
pub(crate) fn populate_apply_result_aliases(
    func: &ArcFunction,
    sigs: &FxHashMap<Name, MemoryContract>,
) -> FxHashMap<ArcVarId, ApplyAliasSource> {
    let let_alias_map = build_let_alias_map(func);
    let mut result = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst,
                func: callee,
                args,
                ..
            } = instr
            {
                if let Some(contract) = sigs.get(callee) {
                    install_alias_entry(&mut result, *dst, args, contract, &let_alias_map);
                }
            }
        }
        if let ArcTerminator::Invoke {
            dst,
            func: callee,
            args,
            ..
        } = &block.terminator
        {
            if let Some(contract) = sigs.get(callee) {
                install_alias_entry(&mut result, *dst, args, contract, &let_alias_map);
            }
        }
    }
    result
}

/// Single-call-site dispatch: classify the callee's contract into Direct /
/// Project / Conditional / no-entry and install the corresponding map entry.
///
/// Per BUG-04-090 §05 Hypothesis D component #2 (gemini F1 + opencode F2):
/// SKIP entries when the consumed arg is a Let Var alias. The alias has no
/// independent RC slot at the IR level — recording an `ApplyAliasSource`
/// keyed on it would mislead downstream consumers into suppressing
/// non-existent decs (no-op suppression) OR worse, suppressing decs on the
/// canonical RC owner (regressing `arc::test_rc_alias_owned_call_then_root_use`
/// per §05 session D postmortem).
fn install_alias_entry(
    result: &mut FxHashMap<ArcVarId, ApplyAliasSource>,
    dst: ArcVarId,
    args: &[ArcVarId],
    contract: &MemoryContract,
    let_alias_map: &FxHashMap<ArcVarId, ArcVarId>,
) {
    // Record alias entries for any param structurally flowing to a Direct
    // or Project Return, regardless of the param's own access. The realize
    // walk's `should_suppress_apply_aliased_dec` consumer fires the dec
    // suppression based on the CALLER'S local Access of the arg (Owned →
    // suppress; Borrowed → no-op). The callee's contract Access governs
    // the callee's own RC accounting (its scope-exit AggFields walk fires
    // only when access is Owned), not the caller's.
    let aliasing_params: Vec<usize> = contract
        .params
        .iter()
        .enumerate()
        .filter(|(_, p)| p.return_alias.is_some())
        .map(|(i, _)| i)
        .collect();

    match aliasing_params.len() {
        0 => {
            // No entry — callee return is fresh w.r.t. caller-side aliasing.
        }
        1 => {
            let param_idx = aliasing_params[0];
            // Filter guarantees Some; let-else avoids expect().
            let Some(alias_shape) = contract.params[param_idx].return_alias else {
                return;
            };
            let Some(consumed_arg) = args.get(param_idx).copied() else {
                // Arg-count mismatch with contract param-count — should not
                // occur post type-check; skip defensively.
                return;
            };
            // Hypothesis D component #2: skip if the consumed arg is a Let
            // Var alias. The alias has no RC slot; downstream suppression
            // gates that match this entry would either no-op (alias has no
            // dec) or misfire on the canonical RC owner via traversal.
            if is_let_var_alias(let_alias_map, consumed_arg) {
                return;
            }
            let entry = match alias_shape {
                ReturnAliasShape::Direct => ApplyAliasSource::Direct(consumed_arg),
                ReturnAliasShape::Project { field } => ApplyAliasSource::Project {
                    arg: consumed_arg,
                    field,
                },
            };
            result.insert(dst, entry);
        }
        _ => {
            // Conditional case (E-mat): 2+ Owned params alias the return at
            // runtime. Suppress all candidates' scope-exit decs at File 13;
            // dst's RC ops are retained as the canonical owner.
            //
            // Hypothesis D component #2: per-candidate Let-alias filter.
            // Drop any candidate whose consumed arg is a Let Var alias
            // (no independent RC slot). If all candidates filter out, skip
            // the entry entirely.
            let candidates: Vec<ArcVarId> = aliasing_params
                .iter()
                .filter_map(|&idx| args.get(idx).copied())
                .filter(|&arg| !is_let_var_alias(let_alias_map, arg))
                .collect();
            if !candidates.is_empty() {
                result.insert(dst, ApplyAliasSource::Conditional { candidates });
            }
        }
    }
}
