//! SCC-based borrow inference for ARC IR (Section 06.2).
//!
//! Determines which function parameters can be **borrowed** (no RC operations
//! at the call site) versus **owned** (caller must `rc_inc`, callee must
//! `rc_dec`).
//!
//! # Algorithm
//!
//! Follows Lean 4's approach (`src/Lean/Compiler/IR/Borrow.lean`):
//!
//! 1. **Decompose**: Build call graph, compute SCCs via Tarjan's algorithm.
//! 2. **Initialize**: All non-scalar parameters start as `Borrowed`.
//! 3. **Per-SCC analysis**: Process SCCs in topological order (callees first):
//!    - Non-recursive SCCs: single-pass analysis.
//!    - Recursive SCCs: fixed-point iteration within the SCC only.
//! 4. **Convergence**: Ownership is **monotonic** (Borrowed → Owned, never
//!    backwards). With N parameters per SCC, convergence is guaranteed in
//!    at most N iterations.
//!
//! # Entry Points
//!
//! - [`infer_borrows_scc`]: Full SCC-based pipeline (call graph → SCCs →
//!   per-SCC inference). Used by the JIT evaluator and standalone callers.
//! - [`infer_borrow_single`] / [`infer_borrow_fixed_point`]: Per-SCC building
//!   blocks used by both `infer_borrows_scc` and Salsa-tracked queries.
//!
//! # Projection Ownership Propagation
//!
//! When `Project { dst, value, .. }` extracts a field from a value and `dst`
//! becomes owned (returned or stored), the source `value` must also become
//! owned. Otherwise the caller might free the struct while the projected field
//! is still live. This propagation is transitive and handled naturally by the
//! fixed-point iteration.
//!
//! # Tail Call Preservation
//!
//! When a function tail-calls another function (or itself) and passes a
//! currently-borrowed parameter to an owned position, the parameter must be
//! promoted to owned. Without this, RC insertion would need to insert a `Dec`
//! after the tail call, which would break the tail call optimization (the
//! caller's stack frame must not exist after the tail call).

mod callees;
mod derived;
mod per_scc;

pub use callees::extract_callees;
pub use derived::infer_derived_ownership;
pub use per_scc::{infer_borrow_fixed_point, infer_borrow_single, initialize_single_borrowed};

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::graph::call_graph::CallGraph;
use crate::graph::scc::compute_sccs;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};
use crate::ownership::{AnnotatedSig, Ownership};
use crate::ArcClassification;

/// SCC-based borrow inference (replaces whole-program fixed-point).
///
/// Decomposes functions into SCCs via Tarjan's algorithm, then infers
/// borrow annotations per-SCC in topological order. Produces identical
/// results to the old whole-program fixed-point, but enables future
/// incrementality (each SCC can be cached independently).
///
/// # Arguments
///
/// * `functions` — ARC IR functions to analyze (typically one module's worth).
/// * `classifier` — type classifier for determining scalar vs ref types.
/// * `borrowing_builtins` — method names whose receiver is always borrowed
///   (e.g., `len`, `is_empty`). These are builtin methods compiled inline by
///   the LLVM emitter — they don't appear as user functions and are not in the
///   sigs map, but their args must NOT be forced to Owned.
#[allow(
    clippy::implicit_hasher,
    reason = "FxHashSet is the concrete type used throughout"
)]
pub fn infer_borrows_scc(
    functions: &[ArcFunction],
    classifier: &dyn ArcClassification,
    borrowing_builtins: &FxHashSet<Name>,
) -> FxHashMap<Name, AnnotatedSig> {
    let graph = CallGraph::build(functions);
    let sccs = compute_sccs(&graph);

    let func_by_name: FxHashMap<Name, &ArcFunction> =
        functions.iter().map(|f| (f.name, f)).collect();

    let mut all_sigs = FxHashMap::default();
    for scc in &sccs {
        if scc.is_recursive(&graph) {
            let scc_funcs: Vec<&ArcFunction> = scc
                .members
                .iter()
                .filter_map(|name| func_by_name.get(name).copied())
                .collect();
            let scc_sigs =
                infer_borrow_fixed_point(&scc_funcs, &all_sigs, classifier, borrowing_builtins);
            all_sigs.extend(scc_sigs);
        } else if let Some(&func) = func_by_name.get(&scc.members[0]) {
            let sig = infer_borrow_single(func, &all_sigs, classifier, borrowing_builtins);
            all_sigs.insert(func.name, sig);
        }
    }
    all_sigs
}

/// Apply borrow inference results back to `ArcFunction` parameters.
///
/// Updates each function's `ArcParam::ownership` in-place based on the
/// annotated signatures produced by [`infer_borrows_scc`]. This is the bridge
/// between analysis (Section 06.2) and downstream passes (Section 07).
#[expect(clippy::implicit_hasher, reason = "FxHashMap is the canonical hasher")]
pub fn apply_borrows(functions: &mut [ArcFunction], sigs: &FxHashMap<Name, AnnotatedSig>) {
    for func in functions {
        if let Some(sig) = sigs.get(&func.name) {
            for (param, annotated) in func.params.iter_mut().zip(&sig.params) {
                param.ownership = annotated.ownership;
            }
        }
    }
}

// Shared helpers used by per-SCC inference.

/// Returns the index into `func.params` if `var` is a function parameter.
fn param_index(var: ArcVarId, func: &ArcFunction) -> Option<usize> {
    func.params.iter().position(|p| p.var == var)
}

/// Check whether a variable is "owned" in the current analysis state.
///
/// A variable is owned if:
/// - It is a function parameter with `Ownership::Owned`, OR
/// - It is any non-parameter local variable (locals always own their values
///   from the point of definition).
///
/// Resolves alias chains before checking, so `v1 = Var(v0)` where `v0` is
/// a Borrowed param will correctly report `v1` as not owned.
fn is_owned_var(
    var: ArcVarId,
    func: &ArcFunction,
    sig: &AnnotatedSig,
    aliases: &FxHashMap<ArcVarId, ArcVarId>,
) -> bool {
    let resolved = resolve_alias(var, aliases);
    match param_index(resolved, func) {
        Some(pidx) => sig.params[pidx].ownership == Ownership::Owned,
        None => true, // Local variables are always owned.
    }
}

/// Try to mark a parameter as Owned. Returns `true` if it changed.
fn mark_owned(sig: &mut AnnotatedSig, pidx: usize) -> bool {
    if sig.params[pidx].ownership == Ownership::Borrowed {
        sig.params[pidx].ownership = Ownership::Owned;
        true
    } else {
        false
    }
}

/// Try to mark a variable as Owned if it is a Borrowed parameter.
/// Returns `true` if a parameter was promoted.
///
/// Resolves alias chains: if `var` is a `Let { value: Var(x) }` alias
/// of a parameter, the parameter is promoted. This prevents cases where
/// a parameter is aliased then consumed (e.g., `v1 = v0; Construct([v1])`)
/// from incorrectly leaving the param as Borrowed.
fn try_mark_param_owned(
    var: ArcVarId,
    func: &ArcFunction,
    sig: &mut AnnotatedSig,
    aliases: &FxHashMap<ArcVarId, ArcVarId>,
) -> bool {
    let resolved = resolve_alias(var, aliases);
    if let Some(pidx) = param_index(resolved, func) {
        mark_owned(sig, pidx)
    } else {
        false
    }
}

/// Build a mapping from alias variables to their source.
///
/// For each `Let { dst, value: Var(src) }`, records `dst → src`. This allows
/// ownership promotion to trace through alias chains like `v1 = v0` where `v0`
/// is a parameter and `v1` is consumed by a Construct or returned.
fn build_alias_map(func: &ArcFunction) -> FxHashMap<ArcVarId, ArcVarId> {
    let mut aliases = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                aliases.insert(*dst, *src);
            }
        }
    }
    aliases
}

/// Resolve an alias chain to the root variable.
///
/// Follows `Let { value: Var(x) }` chains: `v2 → v1 → v0 (param)`.
/// Terminates at the first non-alias variable or after a safety limit.
///
/// The 64-step limit is purely defensive: in practice, alias chains are 1–3
/// deep (one `Let { value: Var(x) }` per level). The limit guards against
/// pathological or cyclic alias maps — if hit, it indicates a bug in
/// `build_alias_map` or upstream lowering, not normal program structure.
fn resolve_alias(var: ArcVarId, aliases: &FxHashMap<ArcVarId, ArcVarId>) -> ArcVarId {
    let mut current = var;
    let mut steps = 0u32;
    while let Some(&src) = aliases.get(&current) {
        current = src;
        steps += 1;
        if steps > 64 {
            break;
        }
    }
    current
}

/// Core ownership update pass for a single function.
///
/// Scans all instructions and terminators, promoting Borrowed parameters to
/// Owned when required by usage (returned, stored, passed to owned position).
///
/// Callee signatures are looked up in two maps:
/// - `local_sigs`: SCC members currently being iterated (in-progress sigs).
/// - `external_sigs`: callees outside the SCC (already finalized).
///
/// Lookup order: `local_sigs` first, then `external_sigs`. This ensures
/// that within a recursive SCC, we see the latest in-progress signatures
/// of co-members, while external callees use their stable final sigs.
///
/// Returns `true` if any parameter's ownership changed.
pub(super) fn update_ownership_inner(
    func: &ArcFunction,
    my_sig: &mut AnnotatedSig,
    local_sigs: &FxHashMap<Name, AnnotatedSig>,
    external_sigs: &FxHashMap<Name, AnnotatedSig>,
    borrowing_builtins: &FxHashSet<Name>,
) -> bool {
    /// Look up a callee's signature in local then external maps.
    fn lookup_callee_sig<'a>(
        callee: Name,
        local: &'a FxHashMap<Name, AnnotatedSig>,
        external: &'a FxHashMap<Name, AnnotatedSig>,
    ) -> Option<&'a AnnotatedSig> {
        local.get(&callee).or_else(|| external.get(&callee))
    }

    let mut changed = false;
    let aliases = build_alias_map(func);

    for block in &func.blocks {
        // Scan instructions
        for instr in &block.body {
            match instr {
                ArcInstr::Apply {
                    args, func: callee, ..
                } => {
                    if let Some(callee_sig) = lookup_callee_sig(*callee, local_sigs, external_sigs)
                    {
                        for (i, &arg) in args.iter().enumerate() {
                            if i < callee_sig.params.len()
                                && callee_sig.params[i].ownership == Ownership::Owned
                            {
                                changed |= try_mark_param_owned(arg, func, my_sig, &aliases);
                            }
                        }
                    } else if !borrowing_builtins.contains(callee) {
                        for &arg in args {
                            changed |= try_mark_param_owned(arg, func, my_sig, &aliases);
                        }
                    }
                }

                ArcInstr::ApplyIndirect { closure, args, .. } => {
                    changed |= try_mark_param_owned(*closure, func, my_sig, &aliases);
                    for &arg in args {
                        changed |= try_mark_param_owned(arg, func, my_sig, &aliases);
                    }
                }

                ArcInstr::PartialApply { args, .. } | ArcInstr::Construct { args, .. } => {
                    for &arg in args {
                        changed |= try_mark_param_owned(arg, func, my_sig, &aliases);
                    }
                }

                ArcInstr::Project { dst, value, .. } => {
                    if is_owned_var(*dst, func, my_sig, &aliases) {
                        changed |= try_mark_param_owned(*value, func, my_sig, &aliases);
                    }
                }

                ArcInstr::Let { value, .. } => {
                    let _ = value;
                }

                // Select reads vars without consuming — no ownership propagation.
                // RC/reset/reuse instructions are handled by the ARC pass itself.
                ArcInstr::Select { .. }
                | ArcInstr::RcInc { .. }
                | ArcInstr::RcDec { .. }
                | ArcInstr::IsShared { .. }
                | ArcInstr::Set { .. }
                | ArcInstr::SetTag { .. }
                | ArcInstr::Reset { .. }
                | ArcInstr::Reuse { .. } => {}
            }
        }

        // Scan terminator
        match &block.terminator {
            ArcTerminator::Return { value } => {
                changed |= try_mark_param_owned(*value, func, my_sig, &aliases);
                changed |= check_tail_call(
                    block,
                    *value,
                    func,
                    my_sig,
                    local_sigs,
                    external_sigs,
                    &aliases,
                );
            }

            ArcTerminator::Jump { args, .. } => {
                let _ = args;
            }

            ArcTerminator::Branch { .. }
            | ArcTerminator::Switch { .. }
            | ArcTerminator::Unreachable
            | ArcTerminator::Resume => {}

            ArcTerminator::Invoke {
                args, func: callee, ..
            } => {
                if let Some(callee_sig) = lookup_callee_sig(*callee, local_sigs, external_sigs) {
                    for (i, &arg) in args.iter().enumerate() {
                        if i < callee_sig.params.len()
                            && callee_sig.params[i].ownership == Ownership::Owned
                        {
                            changed |= try_mark_param_owned(arg, func, my_sig, &aliases);
                        }
                    }
                } else if !borrowing_builtins.contains(callee) {
                    for &arg in args {
                        changed |= try_mark_param_owned(arg, func, my_sig, &aliases);
                    }
                }
            }
        }
    }

    changed
}

/// Check for tail call and promote borrowed params if needed.
///
/// A tail call is detected when the last instruction in a block is an
/// `Apply` whose `dst` is the same as the returned `value`. If the callee
/// expects an argument as Owned but the corresponding parameter in our
/// function is currently Borrowed, we must promote it to Owned to preserve
/// the tail call optimization.
fn check_tail_call(
    block: &crate::ir::ArcBlock,
    returned_value: ArcVarId,
    func: &ArcFunction,
    my_sig: &mut AnnotatedSig,
    local_sigs: &FxHashMap<Name, AnnotatedSig>,
    external_sigs: &FxHashMap<Name, AnnotatedSig>,
    aliases: &FxHashMap<ArcVarId, ArcVarId>,
) -> bool {
    let mut changed = false;

    let tail_apply = block
        .body
        .iter()
        .rev()
        .find(|instr| matches!(instr, ArcInstr::Apply { dst, .. } if *dst == returned_value));

    if let Some(ArcInstr::Apply {
        func: callee, args, ..
    }) = tail_apply
    {
        let callee_sig = local_sigs.get(callee).or_else(|| external_sigs.get(callee));
        if let Some(callee_sig) = callee_sig {
            for (i, &arg) in args.iter().enumerate() {
                if i < callee_sig.params.len() && callee_sig.params[i].ownership == Ownership::Owned
                {
                    changed |= try_mark_param_owned(arg, func, my_sig, aliases);
                }
            }
        }
    }

    changed
}

#[cfg(test)]
mod tests;
