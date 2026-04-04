//! Interprocedural analysis for AIMS.
//!
//! Computes a [`MemoryContract`] for every function in the program via
//! SCC-based fixed-point iteration. The contract encodes per-parameter
//! access class, consumption, cardinality, and return value uniqueness.
//!
//! # Architecture
//!
//! 1. Build call graph + Tarjan SCCs (reusing `graph::call_graph` + `graph::scc`)
//! 2. Process SCCs in topological order (callees before callers)
//! 3. Non-recursive SCCs: single intraprocedural analysis pass
//! 4. Recursive SCCs: iterate until all contracts converge
//!
//! At each step, [`super::intraprocedural::analyze_function`] runs the
//! backward dataflow analysis, then [`extract::extract_contract`] reads the
//! converged state map to produce a [`MemoryContract`].
//!
//! # Module structure
//!
//! - [`extract`] — contract extraction from converged state maps
//!
//! # References
//!
//! - Lean 4 `src/Lean/Compiler/IR/Borrow.lean`: `collect_O` + SCC loop
//! - `ori_arc` `borrow/per_scc.rs`: existing SCC borrow inference
//! - FP² (Lorenzen et al., ICFP 2023): FIP certification

mod extract;

#[cfg(test)]
mod tests;

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::borrow::BuiltinOwnershipSets;
use crate::graph::call_graph::CallGraph;
use crate::graph::scc::compute_sccs;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use crate::ArcClassification;

use super::contract::{FipContract, MemoryContract};
use super::intraprocedural::analyze_function;
use super::intraprocedural::AimsStateMap;
use super::lattice::Uniqueness;

use crate::ownership::Ownership;

pub(crate) use extract::extract_contract;

/// Compute [`MemoryContract`] for all functions via SCC-based fixed-point.
///
/// Processes SCCs in topological order (callees before callers). Each SCC
/// is analyzed to convergence before moving to the next. Returns a map
/// from function name to its converged contract.
///
/// # Parameters
///
/// - `functions` — all ARC IR functions in the program
/// - `classifier` — type classification (scalar vs ref)
/// - `_builtins` — builtin ownership sets (Section 03.4 uses these)
/// - `_interner` — string interner for builtin name lookup (Section 03.4)
pub fn analyze_program(
    functions: &[ArcFunction],
    classifier: &dyn ArcClassification,
    builtins: &BuiltinOwnershipSets,
    interner: &ori_ir::StringInterner,
) -> FxHashMap<Name, MemoryContract> {
    let graph = CallGraph::build(functions);
    let sccs = compute_sccs(&graph);

    let func_by_name: FxHashMap<Name, &ArcFunction> =
        functions.iter().map(|f| (f.name, f)).collect();

    let mut all_sigs: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    // Pre-seed builtin contracts so call sites get accurate ownership info.
    super::builtins::seed_builtin_contracts(&mut all_sigs, builtins, interner);

    for scc in &sccs {
        if scc.is_recursive(&graph) {
            let scc_funcs: Vec<&ArcFunction> = scc
                .members
                .iter()
                .filter_map(|name| func_by_name.get(name).copied())
                .collect();
            if scc_funcs.is_empty() {
                continue;
            }
            let scc_sigs = analyze_scc_fixpoint(&scc_funcs, classifier, &all_sigs);
            all_sigs.extend(scc_sigs);
        } else if let Some(&func) = func_by_name.get(&scc.members[0]) {
            let contract = analyze_scc_single(func, classifier, &all_sigs);
            all_sigs.insert(func.name, contract);
        }
        // External/FFI functions not in `func_by_name` are skipped —
        // their contracts are looked up as conservative fallbacks
        // at call sites via `apply_callee_contract`.
    }

    // Section 09.1: Post-fixpoint demand propagation.
    // When ALL callers pass an argument with Owned+Linear+Once, tighten
    // the callee's parameter uniqueness to Unique. This is the callee-side
    // dual of COW-aware borrowing (07.3.1).
    tighten_uniqueness_from_callers(functions, classifier, &mut all_sigs);

    // FIP coverage reporting.
    let mut fip_certified = 0u32;
    let mut fip_conditional = 0u32;
    let mut fip_bounded = 0u32;
    let mut fip_never = 0u32;
    let mut fbip_count = 0u32;
    for contract in all_sigs.values() {
        match &contract.fip {
            FipContract::Certified => fip_certified += 1,
            FipContract::Conditional { .. } => fip_conditional += 1,
            FipContract::Bounded(_) => fip_bounded += 1,
            FipContract::Never => fip_never += 1,
        }
        if contract.is_fbip {
            fbip_count += 1;
        }
    }
    tracing::debug!(
        functions = all_sigs.len(),
        fip_certified,
        fip_conditional,
        fip_bounded,
        fip_never,
        fbip_count,
        "AIMS interprocedural analysis complete — FIP coverage"
    );

    all_sigs
}

/// Analyze a non-recursive function in a single pass.
fn analyze_scc_single(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    all_sigs: &FxHashMap<Name, MemoryContract>,
) -> MemoryContract {
    let state_map = analyze_function(func, classifier, all_sigs, &[], Vec::new());
    // Non-recursive: empty SCC peer set → has_unbounded_stack = false.
    // No context regions for non-recursive (TRMC requires recursion).
    let empty_peers = rustc_hash::FxHashSet::default();
    extract_contract(func, &state_map, classifier, all_sigs, &empty_peers, &[])
}

/// Analyze a mutually recursive SCC via fixed-point iteration.
///
/// Convergence: contracts are monotonic (params can only promote toward
/// conservative, return uniqueness can only weaken). Each iteration must
/// promote at least one dimension of one parameter, guaranteeing
/// termination in bounded iterations.
fn analyze_scc_fixpoint(
    scc_funcs: &[&ArcFunction],
    classifier: &dyn ArcClassification,
    external_sigs: &FxHashMap<Name, MemoryContract>,
) -> FxHashMap<Name, MemoryContract> {
    // Build the SCC peer set for constant-stack analysis (Section 12.2).
    let scc_peers: rustc_hash::FxHashSet<Name> = scc_funcs.iter().map(|f| f.name).collect();

    // Initialize all SCC members to most-optimistic contracts.
    let mut local_sigs: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    local_sigs.reserve(scc_funcs.len());
    for &func in scc_funcs {
        local_sigs.insert(
            func.name,
            MemoryContract::all_borrowed(func.params.len(), FipContract::Never),
        );
    }

    // Build a combined sig map: external (finalized) + local (iterating).
    // Local sigs shadow external ones for SCC members.
    //
    // NOTE: This clones `external_sigs` once per SCC. A layered lookup
    // (check local first, then external) would avoid the clone but
    // requires changing `analyze_function`'s `&FxHashMap` parameter to
    // a trait. Performance note: clone cost is O(depth) where depth is
    // the number of finalized contracts, negligible for typical programs.
    let mut combined_sigs = external_sigs.clone();

    let mut changed = true;
    let mut iterations = 0u32;
    while changed {
        changed = false;

        // Update combined with current local sigs.
        for (&name, contract) in &local_sigs {
            combined_sigs.insert(name, contract.clone());
        }

        for &func in scc_funcs {
            let state_map = analyze_function(func, classifier, &combined_sigs, &[], Vec::new());
            // Detect TRMC context regions (detection only — no rewrite during
            // interprocedural fixpoint; the rewrite runs in the per-function
            // pipeline after contracts converge).
            let context_regions = crate::aims::normalize::detect_context_regions(func);
            let new_contract = extract_contract(
                func,
                &state_map,
                classifier,
                &combined_sigs,
                &scc_peers,
                &context_regions,
            );

            let old_contract = &local_sigs[&func.name];
            if &new_contract != old_contract {
                // Join to ensure monotonicity: contracts can only grow
                // toward conservative.
                let joined = old_contract.join(&new_contract);
                if &joined != old_contract {
                    local_sigs.insert(func.name, joined);
                    changed = true;
                }
            }
        }
        iterations += 1;
    }

    // Convergence bound: each iteration promotes at least one lattice
    // dimension. Total dimensions per function = params × 6 + return × 4 + effects × 4.
    // (EffectSummary has 6 bool fields, but `may_deallocate` and
    // `has_unbounded_stack` don't change during fixpoint — they are set
    // post-emission and in extract_contract() respectively.)
    // In practice, convergence is much faster.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "param count per function bounded by u32::MAX in practice"
    )]
    let total_dimensions: u32 = scc_funcs
        .iter()
        .map(|f| {
            let param_dims = f.params.len() as u32 * 6;
            let return_dims = 4u32;
            let effect_dims = 4u32;
            param_dims + return_dims + effect_dims
        })
        .sum();
    debug_assert!(
        iterations <= total_dimensions.saturating_add(1),
        "AIMS fixed-point exceeded convergence bound: \
         {iterations} iterations for {total_dimensions} dimensions"
    );

    tracing::debug!(
        scc_size = scc_funcs.len(),
        iterations,
        "AIMS SCC fixed-point converged"
    );

    local_sigs
}

// Demand propagation (Section 09.1)

/// Tighten parameter uniqueness when ALL callers pass Owned+Linear+Once.
///
/// After the main SCC fixpoint converges, this post-processing phase
/// re-analyzes each function to collect call-site argument states, then
/// checks if every call site for a given (callee, `param_idx`) passes the
/// argument with `Owned + Linear + Once`. When all callers satisfy this
/// condition, the callee's `ParamContract.uniqueness` is tightened to
/// `Unique` — the callee can trust the argument's runtime RC == 1 at entry.
///
/// # Soundness (Marshall et al., ESOP 2022)
///
/// A single call site with `Owned + Linear + Once` proves that THIS caller
/// holds the sole live reference on THIS path. Global RC==1 is only proven
/// when the interprocedural fixpoint confirms ALL callers satisfy the
/// condition. Premature tightening from a single call site is unsound.
///
/// # Implementation
///
/// For each function, the converged backward demand at the function entry
/// block gives the total demand on each parameter. If a function parameter
/// is passed as an argument to a callee call site, the function entry state
/// captures whether the caller uses that argument with Owned+Linear+Once.
/// For local variables defined within the function, the entry state has
/// BOTTOM (Borrowed access), so they conservatively don't satisfy the
/// condition — this is correct but may miss optimization opportunities
/// for freshly constructed locals.
fn tighten_uniqueness_from_callers(
    functions: &[ArcFunction],
    classifier: &dyn ArcClassification,
    sigs: &mut FxHashMap<Name, MemoryContract>,
) {
    // Phase 1: Re-analyze each function with final contracts and collect
    // call-site argument states.
    //
    // For each (callee, param_idx), track whether ALL callers satisfy the
    // Owned+Linear+Once condition. Start optimistically at `true` and
    // flip to `false` on any violation.
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
/// For each argument at a call site, checks whether the argument is
/// guaranteed unique (RC==1) when passed to the callee:
///
/// 1. **Construct-defined variable**: fresh allocation, RC==1.
/// 2. **Owned function parameter with single use**: forwarded linearly.
///
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
/// Uses structural checks rather than backward analysis state, because the
/// backward analysis double-counts Apply arguments (contract demand + generic
/// backward demand) and uses `Affine` minimum consumption.
///
/// - **Construct-defined variables**: a `Construct` instruction produces a
///   fresh heap value with RC==1. The argument is unique at the call site.
/// - **Function parameters with `Owned` ownership and a single use**: the
///   parameter holds a real reference and is forwarded to exactly one call
///   without being shared.
/// - **Other variables**: conservatively treated as not unique.
fn arg_satisfies_uniqueness(
    func: &ArcFunction,
    construct_vars: &FxHashSet<ArcVarId>,
    arg: ArcVarId,
) -> bool {
    // Case 1: Locally defined by Construct — fresh unique value (RC==1).
    // O(1) lookup via pre-computed set (avoids O(blocks×instrs) scan).
    if construct_vars.contains(&arg) {
        return true;
    }

    // Case 2: Function parameter — check ownership and linear forwarding.
    let is_owned_param = func
        .params
        .iter()
        .any(|p| p.var == arg && p.ownership == Ownership::Owned);

    if !is_owned_param {
        return false;
    }

    // Count total uses of the variable across the function.
    // If it appears exactly once (at this call site), the parameter
    // is forwarded linearly — no other use retains a reference.
    let use_count = count_var_uses(func, arg);
    use_count == 1
}

/// Count how many times a variable appears as an operand in a function.
///
/// Counts uses in instruction operands and terminators. Does NOT count
/// the variable's definition (e.g., as `dst` in Construct or Apply).
fn count_var_uses(func: &ArcFunction, var: ArcVarId) -> usize {
    let mut count = 0;

    for block in &func.blocks {
        for instr in &block.body {
            count += instr_use_count(instr, var);
        }
        count += terminator_use_count(&block.terminator, var);
    }

    count
}

/// Count uses of a variable in a single instruction.
fn instr_use_count(instr: &ArcInstr, var: ArcVarId) -> usize {
    match instr {
        ArcInstr::Let { value, .. } => match value {
            crate::ir::ArcValue::Var(v) => usize::from(*v == var),
            crate::ir::ArcValue::Literal(_) => 0,
            crate::ir::ArcValue::PrimOp { args, .. } => args.iter().filter(|&&v| v == var).count(),
        },
        ArcInstr::Construct { args, .. }
        | ArcInstr::Apply { args, .. }
        | ArcInstr::PartialApply { args, .. } => args.iter().filter(|&&v| v == var).count(),
        ArcInstr::ApplyIndirect { closure, args, .. } => {
            usize::from(*closure == var) + args.iter().filter(|&&v| v == var).count()
        }
        ArcInstr::Project { value, .. } => usize::from(*value == var),
        ArcInstr::Select {
            cond,
            true_val,
            false_val,
            ..
        } => {
            usize::from(*cond == var)
                + usize::from(*true_val == var)
                + usize::from(*false_val == var)
        }
        ArcInstr::CollectionReuse { old_var, args, .. } => {
            usize::from(*old_var == var) + args.iter().filter(|&&v| v == var).count()
        }
        ArcInstr::IsShared { var: v, .. }
        | ArcInstr::Reset { var: v, .. }
        | ArcInstr::RcInc { var: v, .. }
        | ArcInstr::RcDec { var: v, .. } => usize::from(*v == var),
        ArcInstr::Set { base, value, .. } => usize::from(*base == var) + usize::from(*value == var),
        ArcInstr::SetTag { base, .. } => usize::from(*base == var),
        ArcInstr::Reuse { token, args, .. } => {
            usize::from(*token == var) + args.iter().filter(|&&v| v == var).count()
        }
    }
}

/// Count uses of a variable in a terminator.
fn terminator_use_count(term: &crate::ir::ArcTerminator, var: ArcVarId) -> usize {
    use crate::ir::ArcTerminator;
    match term {
        ArcTerminator::Return { value } => usize::from(*value == var),
        ArcTerminator::Jump { args, .. } => args.iter().filter(|&&v| v == var).count(),
        ArcTerminator::Branch { cond, .. } => usize::from(*cond == var),
        ArcTerminator::Switch { scrutinee, .. } => usize::from(*scrutinee == var),
        ArcTerminator::Invoke { args, .. } => args.iter().filter(|&&v| v == var).count(),
        ArcTerminator::InvokeIndirect { closure, args, .. } => {
            usize::from(*closure == var) + args.iter().filter(|&&v| v == var).count()
        }
        ArcTerminator::Resume | ArcTerminator::Unreachable => 0,
    }
}

/// Pre-compute the set of variables defined by `Construct` instructions.
///
/// A `Construct`-defined variable has RC==1 (fresh allocation), so it is
/// guaranteed unique at any call site. Computing this once per function
/// (O(blocks × instrs)) then using O(1) lookups avoids the previous
/// O(blocks × instrs) scan per argument in `arg_satisfies_uniqueness`.
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
