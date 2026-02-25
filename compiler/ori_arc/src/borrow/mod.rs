//! Iterative borrow inference for ARC IR (Section 06.2).
//!
//! Determines which function parameters can be **borrowed** (no RC operations
//! at the call site) versus **owned** (caller must `rc_inc`, callee must
//! `rc_dec`).
//!
//! # Algorithm
//!
//! Follows Lean 4's approach (`src/Lean/Compiler/IR/Borrow.lean`):
//!
//! 1. **Initialize**: All non-scalar parameters start as `Borrowed`.
//! 2. **Scan**: Walk every instruction in every block. When a parameter is
//!    used in a way that requires ownership (returned, stored, passed to an
//!    owning position), mark it `Owned`.
//! 3. **Iterate**: Repeat step 2 until no parameter changes (fixed point).
//!
//! The fixed point converges because ownership is **monotonic** — parameters
//! can only transition from `Borrowed` to `Owned`, never backwards. With N
//! parameters, convergence is guaranteed in at most N iterations.
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
//!
//! # Per-SCC Inference (Sections 12.5–12.6)
//!
//! [`infer_borrow_single`] and [`infer_borrow_fixed_point`] decompose borrow
//! inference by SCC (strongly connected component). Non-recursive SCCs use a
//! single pass; recursive SCCs iterate to a fixed point within the SCC only.
//! Both accept pre-resolved external callee signatures, enabling Salsa-tracked
//! incremental queries where changing one function only re-analyzes its SCC.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};
use crate::ownership::{AnnotatedParam, AnnotatedSig, DerivedOwnership, Ownership};
use crate::ArcClassification;

/// Infer borrow annotations for a set of (possibly mutually recursive) functions.
///
/// Returns a map from function name to its annotated signature. Scalar
/// parameters are always effectively borrowed (no RC) and are marked as
/// `Owned` in the output — they are simply skipped by RC insertion because
/// their [`ArcClass`](crate::ArcClass) is `Scalar`.
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
pub fn infer_borrows(
    functions: &[ArcFunction],
    classifier: &dyn ArcClassification,
    borrowing_builtins: &FxHashSet<Name>,
) -> FxHashMap<Name, AnnotatedSig> {
    let mut sigs = initialize_all_borrowed(functions, classifier);

    let mut changed = true;
    while changed {
        changed = false;
        for func in functions {
            if update_ownership(func, &mut sigs, borrowing_builtins) {
                changed = true;
            }
        }
    }

    sigs
}

/// Apply borrow inference results back to `ArcFunction` parameters.
///
/// Updates each function's `ArcParam::ownership` in-place based on the
/// annotated signatures produced by [`infer_borrows`]. This is the bridge
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

/// Initialize all non-scalar parameters as `Borrowed`.
///
/// Scalar parameters (int, float, bool, etc.) don't need RC and are
/// initialized as `Owned` — borrow inference ignores them entirely.
fn initialize_all_borrowed(
    functions: &[ArcFunction],
    classifier: &dyn ArcClassification,
) -> FxHashMap<Name, AnnotatedSig> {
    let mut sigs = FxHashMap::default();
    sigs.reserve(functions.len());

    for func in functions {
        let params: Vec<AnnotatedParam> = func
            .params
            .iter()
            .map(|p| {
                let ownership = if classifier.is_scalar(p.ty) {
                    // Scalar: no RC needed regardless of usage.
                    Ownership::Owned
                } else {
                    // Ref-typed: start as Borrowed (optimistic).
                    Ownership::Borrowed
                };
                AnnotatedParam {
                    name: Name::from_raw(p.var.raw()),
                    ty: p.ty,
                    ownership,
                }
            })
            .collect();

        sigs.insert(
            func.name,
            AnnotatedSig {
                params,
                return_type: func.return_type,
            },
        );
    }

    sigs
}

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

/// Single pass over one function, checking all parameter uses.
///
/// Returns `true` if any parameter's ownership changed.
///
/// Delegates to [`update_ownership_inner`] with the full sigs map as
/// both local and external (whole-program mode).
fn update_ownership(
    func: &ArcFunction,
    sigs: &mut FxHashMap<Name, AnnotatedSig>,
    borrowing_builtins: &FxHashSet<Name>,
) -> bool {
    // Clone this function's sig to avoid simultaneous &/&mut borrow of `sigs`.
    let mut my_sig = match sigs.get(&func.name) {
        Some(sig) => sig.clone(),
        None => return false,
    };

    let empty = FxHashMap::default();
    let changed = update_ownership_inner(func, &mut my_sig, &empty, sigs, borrowing_builtins);

    if changed {
        sigs.insert(func.name, my_sig);
    }
    changed
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
fn update_ownership_inner(
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

                ArcInstr::RcInc { .. }
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

/// Initialize a single function's parameters as Borrowed (non-scalar) or Owned (scalar).
///
/// Same logic as [`initialize_all_borrowed`] but for a single function,
/// returning a standalone `AnnotatedSig` rather than inserting into a map.
pub fn initialize_single_borrowed(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
) -> AnnotatedSig {
    let params: Vec<AnnotatedParam> = func
        .params
        .iter()
        .map(|p| {
            let ownership = if classifier.is_scalar(p.ty) {
                Ownership::Owned
            } else {
                Ownership::Borrowed
            };
            AnnotatedParam {
                name: Name::from_raw(p.var.raw()),
                ty: p.ty,
                ownership,
            }
        })
        .collect();

    AnnotatedSig {
        params,
        return_type: func.return_type,
    }
}

/// Infer borrow annotations for a single non-recursive function (Section 12.5).
///
/// Unlike [`infer_borrows`] which runs whole-program fixed-point iteration,
/// this function analyzes ONE function in a single pass. Callee signatures
/// are provided via `external_sigs` (already finalized by earlier SCCs).
///
/// Suitable for non-recursive SCCs (single function, no self-call).
///
/// # Arguments
///
/// * `func` — the ARC IR function to analyze.
/// * `external_sigs` — pre-resolved callee signatures from other SCCs.
/// * `classifier` — type classifier for scalar vs ref types.
/// * `borrowing_builtins` — method names whose receiver is always borrowed.
#[expect(
    clippy::implicit_hasher,
    reason = "FxHashMap/FxHashSet are the concrete types used throughout"
)]
pub fn infer_borrow_single(
    func: &ArcFunction,
    external_sigs: &FxHashMap<Name, AnnotatedSig>,
    classifier: &dyn ArcClassification,
    borrowing_builtins: &FxHashSet<Name>,
) -> AnnotatedSig {
    let mut sig = initialize_single_borrowed(func, classifier);
    let empty_local = FxHashMap::default();
    update_ownership_inner(
        func,
        &mut sig,
        &empty_local,
        external_sigs,
        borrowing_builtins,
    );
    sig
}

/// Infer borrow annotations for a mutually recursive SCC (Section 12.6).
///
/// Runs fixed-point iteration over the SCC members only, using pre-resolved
/// `external_sigs` for callees outside the SCC. Convergence is guaranteed
/// in at most `sum_of_params + 1` iterations (monotonic: Borrowed → Owned).
///
/// # Arguments
///
/// * `scc_functions` — ARC IR functions in this SCC (mutually recursive group).
/// * `external_sigs` — pre-resolved callee signatures from other SCCs.
/// * `classifier` — type classifier for scalar vs ref types.
/// * `borrowing_builtins` — method names whose receiver is always borrowed.
#[expect(
    clippy::implicit_hasher,
    reason = "FxHashMap/FxHashSet are the concrete types used throughout"
)]
pub fn infer_borrow_fixed_point(
    scc_functions: &[&ArcFunction],
    external_sigs: &FxHashMap<Name, AnnotatedSig>,
    classifier: &dyn ArcClassification,
    borrowing_builtins: &FxHashSet<Name>,
) -> FxHashMap<Name, AnnotatedSig> {
    // Initialize all SCC members.
    let mut local_sigs = FxHashMap::default();
    local_sigs.reserve(scc_functions.len());
    for &func in scc_functions {
        local_sigs.insert(func.name, initialize_single_borrowed(func, classifier));
    }

    // Fixed-point iteration within the SCC.
    let mut changed = true;
    let mut iterations = 0u32;
    while changed {
        changed = false;
        for &func in scc_functions {
            let mut my_sig = local_sigs[&func.name].clone();
            if update_ownership_inner(
                func,
                &mut my_sig,
                &local_sigs,
                external_sigs,
                borrowing_builtins,
            ) {
                local_sigs.insert(func.name, my_sig);
                changed = true;
            }
        }
        iterations += 1;
    }

    // Convergence bound: each iteration promotes at least one param.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "param count per function bounded by u32::MAX in practice"
    )]
    let total_params: u32 = scc_functions.iter().map(|f| f.params.len() as u32).sum();
    debug_assert!(
        iterations <= total_params.saturating_add(1),
        "fixed-point exceeded convergence bound: {iterations} iterations for {total_params} params"
    );

    local_sigs
}

/// Extract callee [`Name`]s from a function's instructions.
///
/// Scans `Apply`, `PartialApply`, and `Invoke` instructions for direct callees.
/// Indirect calls (`ApplyIndirect`) are excluded — their callees are unknown.
///
/// This duplicates `CallGraph::build`'s per-function logic as a standalone
/// helper, avoiding the need to reconstruct the full call graph in per-SCC
/// queries.
pub fn extract_callees(func: &ArcFunction) -> FxHashSet<Name> {
    let mut callees = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Apply { func: callee, .. }
                | ArcInstr::PartialApply { func: callee, .. } => {
                    callees.insert(*callee);
                }
                _ => {}
            }
        }
        if let ArcTerminator::Invoke { func: callee, .. } = &block.terminator {
            callees.insert(*callee);
        }
    }
    callees
}

/// Infer per-variable ownership from SSA data flow.
///
/// Unlike [`infer_borrows`] which classifies only function parameters via
/// fixed-point iteration, this function classifies **every** variable in a
/// single forward pass (no fixed-point needed — SSA guarantees each variable
/// is defined exactly once).
///
/// The result is a `Vec<DerivedOwnership>` indexed by `ArcVarId::raw()`,
/// enabling RC insertion to skip `RcInc`/`RcDec` for:
/// - Variables borrowed from a still-live owner (`BorrowedFrom`)
/// - Freshly constructed values with refcount = 1 (`Fresh`)
///
/// # Arguments
///
/// * `func` — the ARC IR function to analyze.
/// * `sigs` — annotated signatures from borrow inference (for callee param ownership).
/// * `classifier` — type classifier for determining scalar vs ref types.
#[expect(clippy::implicit_hasher, reason = "FxHashMap is the canonical hasher")]
pub fn infer_derived_ownership(
    func: &ArcFunction,
    sigs: &FxHashMap<Name, AnnotatedSig>,
) -> Vec<DerivedOwnership> {
    let num_vars = func.var_types.len();
    let mut ownership = vec![DerivedOwnership::Owned; num_vars];

    // Function parameters: inherit from AnnotatedSig.
    if let Some(sig) = sigs.get(&func.name) {
        for (i, param) in func.params.iter().enumerate() {
            let idx = param.var.index();
            if idx < num_vars {
                ownership[idx] = match sig.params.get(i).map(|p| p.ownership) {
                    Some(Ownership::Borrowed) => DerivedOwnership::BorrowedFrom(param.var),
                    _ => DerivedOwnership::Owned,
                };
            }
        }
    }

    // Forward pass over all blocks in order.
    // SSA form: each variable is defined exactly once, so a single forward
    // pass is sufficient (no iteration needed).
    for block in &func.blocks {
        // Block parameters receive values via jump args — they're owned
        // (the caller transfers ownership through the jump).
        for &(param_var, _ty) in &block.params {
            let idx = param_var.index();
            if idx < num_vars {
                ownership[idx] = DerivedOwnership::Owned;
            }
        }

        for instr in &block.body {
            match instr {
                ArcInstr::Project { dst, value, .. } => {
                    // A projection borrows from the source variable.
                    let dst_idx = dst.index();
                    if dst_idx < num_vars {
                        let source_idx = value.index();
                        ownership[dst_idx] = if source_idx < num_vars {
                            // Transitively resolve: if `value` borrows from X,
                            // the projection also borrows from X.
                            match ownership[source_idx] {
                                DerivedOwnership::BorrowedFrom(root) => {
                                    DerivedOwnership::BorrowedFrom(root)
                                }
                                _ => DerivedOwnership::BorrowedFrom(*value),
                            }
                        } else {
                            DerivedOwnership::BorrowedFrom(*value)
                        };
                    }
                }

                ArcInstr::Let { dst, value, .. } => {
                    let dst_idx = dst.index();
                    if dst_idx < num_vars {
                        ownership[dst_idx] = match value {
                            // Var alias inherits from source.
                            crate::ir::ArcValue::Var(src) => {
                                let src_idx = src.index();
                                if src_idx < num_vars {
                                    ownership[src_idx]
                                } else {
                                    DerivedOwnership::Owned
                                }
                            }
                            // Literals and PrimOps produce owned values.
                            crate::ir::ArcValue::Literal(_)
                            | crate::ir::ArcValue::PrimOp { .. } => DerivedOwnership::Owned,
                        };
                    }
                }

                ArcInstr::Construct { dst, .. } => {
                    // A newly constructed value has refcount = 1.
                    let dst_idx = dst.index();
                    if dst_idx < num_vars {
                        ownership[dst_idx] = DerivedOwnership::Fresh;
                    }
                }

                ArcInstr::PartialApply { dst, .. } => {
                    // A new closure has refcount = 1.
                    let dst_idx = dst.index();
                    if dst_idx < num_vars {
                        ownership[dst_idx] = DerivedOwnership::Fresh;
                    }
                }

                ArcInstr::Apply { dst, .. } | ArcInstr::ApplyIndirect { dst, .. } => {
                    // Call results are owned (callee returns an owned value).
                    let dst_idx = dst.index();
                    if dst_idx < num_vars {
                        ownership[dst_idx] = DerivedOwnership::Owned;
                    }
                }

                // RC/reuse ops don't define new variables (or their dst
                // is a token which is always Owned).
                ArcInstr::RcInc { .. }
                | ArcInstr::RcDec { .. }
                | ArcInstr::IsShared { .. }
                | ArcInstr::Set { .. }
                | ArcInstr::SetTag { .. }
                | ArcInstr::Reset { .. }
                | ArcInstr::Reuse { .. } => {}
            }
        }
    }

    ownership
}

#[cfg(test)]
mod tests;
