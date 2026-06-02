//! Post-fixpoint demand propagation.
//!
//! After the SCC fixpoint converges, tighten a callee parameter's uniqueness
//! to `Unique` when every call site passes a fresh, single-use `Construct`
//! argument. BUG-04-069: the forwarded-`Owned`-param heuristic was removed as
//! the DP-10 / IC-8 fallacy; only fresh `Construct` + `count_var_uses == 1`
//! qualifies.

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use crate::ArcClassification;

use super::super::contract::MemoryContract;
use super::super::intraprocedural::{analyze_function, AimsStateMap};
use super::super::lattice::Uniqueness;
use super::use_count::count_var_uses;

/// Tighten parameter uniqueness when every call site passes a fresh,
/// single-use `Construct` argument.
///
/// After the main SCC fixpoint converges, this post-processing phase
/// re-analyzes each function to collect call-site argument states. For a
/// given (callee, `param_idx`), the callee's `ParamContract.uniqueness` is
/// tightened to `Unique` only when EVERY call site passes an argument that
/// is a fresh `Construct` value used exactly once in its function — RC == 1
/// by construction per TF-3.
///
/// # Soundness
///
/// A fresh `Construct` used exactly once has RC == 1 with no existing
/// aliases — the sole sound basis for `Unique`. The forwarded-`Owned`
/// parameter heuristic (`Owned + Linear + Once`) was removed: it proves
/// no-future-duplication, NOT no-existing-alias (the DP-10 / IC-8 fallacy).
/// A fresh `Construct` reaching the same (callee, `param_idx`) at two or
/// more sites is RC > 1 at the non-first use, so the single-use guard in
/// `arg_satisfies_uniqueness` is load-bearing.
///
/// # Implementation
///
/// `build_construct_set` identifies fresh-`Construct` variables; an argument
/// satisfies the condition iff it is in that set AND `count_var_uses == 1`.
/// The per-(callee, `param_idx`) fold tightens to `Unique` only when all
/// call sites satisfy the condition; any non-satisfying site leaves the
/// parameter at its conservative `MaybeShared` default.
pub(super) fn tighten_uniqueness_from_callers(
    functions: &[ArcFunction],
    classifier: &dyn ArcClassification,
    sigs: &mut FxHashMap<Name, MemoryContract>,
) {
    // Phase 1: Re-analyze each function with final contracts and collect
    // call-site argument states.
    //
    // For each (callee, param_idx), track whether ALL callers satisfy the
    // fresh-Construct + single-use condition. Start optimistically at `true`
    // and flip to `false` on any violation.
    let mut all_satisfy: FxHashMap<(Name, usize), bool> = FxHashMap::default();

    for func in functions {
        let state_map = analyze_function(func, classifier, sigs, &[], Vec::new());

        // Pre-compute Construct-defined vars for O(1) lookups in
        // arg_satisfies_uniqueness (avoids O(blocks×instrs) per argument).
        let construct_vars = build_construct_set(func);

        // Walk blocks to find Apply instructions.
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Apply {
                    func: callee, args, ..
                } = instr
                {
                    collect_call_site_uniqueness(
                        func,
                        &state_map,
                        &construct_vars,
                        *callee,
                        args,
                        sigs,
                        &mut all_satisfy,
                    );
                }
            }

            // Walk terminators for Invoke.
            if let ArcTerminator::Invoke {
                func: callee, args, ..
            } = &block.terminator
            {
                collect_call_site_uniqueness(
                    func,
                    &state_map,
                    &construct_vars,
                    *callee,
                    args,
                    sigs,
                    &mut all_satisfy,
                );
            }
        }
    }

    // Phase 2: Tighten uniqueness for parameters where ALL callers satisfy.
    let mut tightened = 0u32;
    for ((callee, param_idx), satisfies) in &all_satisfy {
        if !satisfies {
            continue;
        }
        if let Some(contract) = sigs.get_mut(callee) {
            if let Some(param) = contract.params.get_mut(*param_idx) {
                if param.uniqueness != Uniqueness::Unique {
                    param.uniqueness = Uniqueness::Unique;
                    tightened += 1;
                }
            }
        }
    }

    if tightened > 0 {
        tracing::debug!(
            tightened,
            "AIMS demand propagation: tightened parameter uniqueness"
        );
    }
}

/// Collect call-site argument uniqueness information for demand propagation.
///
/// For each non-scalar argument, folds per `(callee, param_idx)` whether the
/// argument is a fresh `Construct` used exactly once (RC==1 by TF-3) across all
/// call sites; the param tightens to `Unique` only when every site qualifies.
/// Only non-scalar arguments in callee contracts are checked.
fn collect_call_site_uniqueness(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    construct_vars: &FxHashSet<ArcVarId>,
    callee: Name,
    args: &[ArcVarId],
    sigs: &FxHashMap<Name, MemoryContract>,
    all_satisfy: &mut FxHashMap<(Name, usize), bool>,
) {
    let Some(callee_contract) = sigs.get(&callee) else {
        return;
    };

    for (i, &arg) in args.iter().enumerate() {
        if i >= callee_contract.params.len() {
            break;
        }

        if state_map.is_excluded(arg) {
            continue;
        }

        let satisfies = arg_satisfies_uniqueness(func, construct_vars, arg);

        let key = (callee, i);
        let entry = all_satisfy.entry(key).or_insert(true);
        if !satisfies {
            *entry = false;
        }
    }
}

/// Check whether a call-site argument is guaranteed unique (RC==1).
///
/// Only a fresh `Construct` value (RC==1 by TF-3) used EXACTLY ONCE qualifies.
/// The single-use guard is load-bearing: callers fold this per
/// `(callee, param_idx)` with logical AND, so a fresh value passed to the same
/// parameter at multiple sites is RC>1 at the non-first use and must not tighten.
/// A forwarded `Owned` parameter does NOT qualify — `Owned+Linear+Once` proves
/// no-future-duplication, NOT no-existing-alias (the removed IC-8 / DP-10 rule).
fn arg_satisfies_uniqueness(
    func: &ArcFunction,
    construct_vars: &FxHashSet<ArcVarId>,
    arg: ArcVarId,
) -> bool {
    // Fresh Construct (O(1) set lookup) used once across the whole function.
    construct_vars.contains(&arg) && count_var_uses(func, arg) == 1
}

/// Pre-compute the set of variables defined by `Construct` instructions.
///
/// A `Construct`-defined variable is a fresh allocation (RC == 1 at its
/// definition). Set membership alone does NOT prove call-site uniqueness:
/// a fresh value used more than once is RC > 1 at the non-first use. The
/// caller-side `count_var_uses == 1` guard in `arg_satisfies_uniqueness` is
/// what establishes RC == 1 at the call site. Computing this set once per
/// function (O(blocks × instrs)) then using O(1) lookups avoids a
/// per-argument scan in `arg_satisfies_uniqueness`.
fn build_construct_set(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    let mut set = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Construct { dst, .. } = instr {
                set.insert(*dst);
            }
        }
    }
    set
}
