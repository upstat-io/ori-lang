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

mod callees;
mod derived;
mod per_scc;

pub use callees::extract_callees;
pub use derived::infer_derived_ownership;
pub use per_scc::{infer_borrow_fixed_point, infer_borrow_single, initialize_single_borrowed};

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};
use crate::ownership::{AnnotatedParam, AnnotatedSig, Ownership};
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

// Shared helpers used by both whole-program and per-SCC inference.

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

#[cfg(test)]
mod tests;
