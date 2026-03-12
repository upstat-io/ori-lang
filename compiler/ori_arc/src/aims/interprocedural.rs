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
//! backward dataflow analysis, then [`extract_contract`] reads the
//! converged state map to produce a [`MemoryContract`].
//!
//! # References
//!
//! - Lean 4 `src/Lean/Compiler/IR/Borrow.lean`: `collect_O` + SCC loop
//! - `ori_arc` `borrow/per_scc.rs`: existing SCC borrow inference
//! - FP² (Lorenzen et al., ICFP 2023): FIP certification

#[cfg(test)]
mod tests;

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::borrow::BuiltinOwnershipSets;
use crate::graph::call_graph::CallGraph;
use crate::graph::scc::compute_sccs;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use crate::ArcClassification;

use super::contract::{
    ContextBehavior, EffectSummary, FipContract, MemoryContract, ParamContract, ReturnContract,
};
use super::intraprocedural::analyze_function;
use super::intraprocedural::AimsStateMap;
use super::lattice::Uniqueness;

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

    tracing::debug!(
        functions = all_sigs.len(),
        "AIMS interprocedural analysis complete"
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
    extract_contract(func, &state_map, classifier, all_sigs)
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
            let new_contract = extract_contract(func, &state_map, classifier, &combined_sigs);

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

// Contract extraction

/// Extract a [`MemoryContract`] from a converged intraprocedural state map.
///
/// Reads the backward-computed demand at the function entry point for each
/// parameter, and determines return value uniqueness from the function's
/// Return terminators.
fn extract_contract(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, MemoryContract>,
) -> MemoryContract {
    let params: Vec<ParamContract> = func
        .params
        .iter()
        .map(|param| {
            if classifier.is_scalar(param.ty) {
                // Scalar parameters don't participate in RC.
                // Use conservative access to avoid confusion — scalars
                // have no RC obligations regardless.
                return ParamContract::CONSERVATIVE;
            }
            let state = state_map.var_state_at_block_entry(func.entry, param.var);
            ParamContract {
                access: state.access,
                consumption: state.consumption,
                cardinality: state.cardinality,
                // v1: locality from backward demand
                locality_bound: state.locality,
                // v1 conservative — refined in 03.3 escape/share analysis
                may_escape: false,
                may_share: false,
            }
        })
        .collect();

    let return_info = extract_return_info(func, classifier, sigs);

    MemoryContract {
        params,
        return_info,
        // v1: effect summary conservative — refined in 03.3
        effects: EffectSummary::default(),
        context_behavior: ContextBehavior::default(),
        fip: FipContract::Never,
    }
}

/// Determine return value uniqueness from the function's Return terminators.
///
/// Walks all Return terminators, traces each returned variable to its
/// definition instruction, and determines uniqueness based on how the
/// value was produced. Results from all return paths are joined.
fn extract_return_info(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, MemoryContract>,
) -> ReturnContract {
    // Build definition map: var → defining instruction.
    let def_map = build_definition_map(func);

    // Build Invoke definition map: dst → callee name
    // (Invoke is a terminator, not an instruction).
    let invoke_defs = build_invoke_def_map(func);

    // Collect parameter variables for identity checks.
    let param_vars: rustc_hash::FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();

    let mut return_uniqueness = None::<Uniqueness>;
    let mut all_preserve_freshness = true;

    for block in &func.blocks {
        if let ArcTerminator::Return { value } = &block.terminator {
            let (uniq, preserves) = var_uniqueness(
                *value,
                &def_map,
                &invoke_defs,
                &param_vars,
                classifier,
                sigs,
            );

            return_uniqueness = Some(match return_uniqueness {
                None => uniq,
                Some(prev) => prev.join(uniq),
            });

            if !preserves {
                all_preserve_freshness = false;
            }
        }
    }

    match return_uniqueness {
        Some(uniqueness) => ReturnContract {
            uniqueness,
            preserves_freshness: all_preserve_freshness,
            ..ReturnContract::CONSERVATIVE
        },
        // No Return terminators (e.g., infinite loop) — conservative.
        None => ReturnContract::CONSERVATIVE,
    }
}

/// Determine the uniqueness of a variable based on its definition.
///
/// Returns `(Uniqueness, preserves_freshness)`.
fn var_uniqueness(
    var: ArcVarId,
    def_map: &FxHashMap<ArcVarId, &ArcInstr>,
    invoke_defs: &FxHashMap<ArcVarId, Name>,
    param_vars: &rustc_hash::FxHashSet<ArcVarId>,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, MemoryContract>,
) -> (Uniqueness, bool) {
    if let Some(instr) = def_map.get(&var) {
        match instr {
            // Fresh construction, closure, COW reuse, or scalar check → unique.
            ArcInstr::Construct { .. }
            | ArcInstr::Reuse { .. }
            | ArcInstr::PartialApply { .. }
            | ArcInstr::CollectionReuse { .. }
            | ArcInstr::IsShared { .. } => (Uniqueness::Unique, true),

            // Direct call → use callee's return contract.
            ArcInstr::Apply { func: callee, .. } => callee_return_uniqueness(*callee, sigs),

            // Let binding → trace through to source.
            ArcInstr::Let { value, ty, .. } => match value {
                crate::ir::ArcValue::Var(source) => {
                    if param_vars.contains(source) {
                        // Returning a parameter → preserves freshness
                        // (if caller passes unique, return is unique).
                        (Uniqueness::MaybeShared, true)
                    } else {
                        var_uniqueness(*source, def_map, invoke_defs, param_vars, classifier, sigs)
                    }
                }
                crate::ir::ArcValue::Literal(_) => (Uniqueness::Unique, true),
                crate::ir::ArcValue::PrimOp { .. } => {
                    if classifier.is_scalar(*ty) {
                        (Uniqueness::Unique, true)
                    } else {
                        (Uniqueness::MaybeShared, false)
                    }
                }
            },

            // Indirect call, projection, select, RC/mutation ops → conservative.
            ArcInstr::ApplyIndirect { .. }
            | ArcInstr::Project { .. }
            | ArcInstr::Select { .. }
            | ArcInstr::RcInc { .. }
            | ArcInstr::RcDec { .. }
            | ArcInstr::Set { .. }
            | ArcInstr::SetTag { .. }
            | ArcInstr::Reset { .. } => (Uniqueness::MaybeShared, false),
        }
    } else if let Some(callee) = invoke_defs.get(&var) {
        // Defined by an Invoke terminator → use callee's return contract.
        callee_return_uniqueness(*callee, sigs)
    } else if param_vars.contains(&var) {
        // Returning a parameter directly → uniqueness depends on caller.
        (Uniqueness::MaybeShared, true)
    } else {
        // Block parameter or unknown definition → conservative.
        (Uniqueness::MaybeShared, false)
    }
}

/// Look up a callee's return uniqueness from its contract.
fn callee_return_uniqueness(
    callee: Name,
    sigs: &FxHashMap<Name, MemoryContract>,
) -> (Uniqueness, bool) {
    if let Some(contract) = sigs.get(&callee) {
        (
            contract.return_info.uniqueness,
            contract.return_info.preserves_freshness,
        )
    } else {
        (Uniqueness::MaybeShared, false)
    }
}

/// Build a map from variable to its defining instruction.
///
/// In ARC IR's SSA form, each variable is defined exactly once.
/// Block parameters, function parameters, and Invoke-defined variables
/// are not included (Invoke is a terminator, handled separately).
fn build_definition_map(func: &ArcFunction) -> FxHashMap<ArcVarId, &ArcInstr> {
    let mut map = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let Some(dst) = instr.defined_var() {
                map.insert(dst, instr);
            }
        }
    }
    map
}

/// Build a map from Invoke-defined variables to their callee names.
///
/// Invoke terminators define a dst variable in the normal successor block.
/// This map captures those definitions separately from instruction definitions.
fn build_invoke_def_map(func: &ArcFunction) -> FxHashMap<ArcVarId, Name> {
    let mut map = FxHashMap::default();
    for block in &func.blocks {
        if let ArcTerminator::Invoke {
            dst, func: callee, ..
        } = &block.terminator
        {
            map.insert(*dst, *callee);
        }
    }
    map
}
