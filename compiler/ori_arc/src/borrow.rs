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

use rustc_hash::FxHashMap;

use ori_ir::Name;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
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
pub fn infer_borrows(
    functions: &[ArcFunction],
    classifier: &dyn ArcClassification,
) -> FxHashMap<Name, AnnotatedSig> {
    let mut sigs = initialize_all_borrowed(functions, classifier);

    let mut changed = true;
    while changed {
        changed = false;
        for func in functions {
            if update_ownership(func, &mut sigs) {
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
fn is_owned_var(var: ArcVarId, func: &ArcFunction, sig: &AnnotatedSig) -> bool {
    match param_index(var, func) {
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
fn try_mark_param_owned(var: ArcVarId, func: &ArcFunction, sig: &mut AnnotatedSig) -> bool {
    if let Some(pidx) = param_index(var, func) {
        mark_owned(sig, pidx)
    } else {
        false
    }
}

/// Single pass over one function, checking all parameter uses.
///
/// Returns `true` if any parameter's ownership changed.
fn update_ownership(func: &ArcFunction, sigs: &mut FxHashMap<Name, AnnotatedSig>) -> bool {
    let mut changed = false;

    // Clone this function's sig to avoid simultaneous &/&mut borrow of `sigs`.
    // The clone is cheap (Vec of small Copy structs).
    let mut my_sig = match sigs.get(&func.name) {
        Some(sig) => sig.clone(),
        None => return false,
    };

    for block in &func.blocks {
        // Scan instructions
        for instr in &block.body {
            match instr {
                ArcInstr::Apply {
                    args, func: callee, ..
                } => {
                    // If a parameter is passed to an owned position in
                    // the callee, it must become owned.
                    if let Some(callee_sig) = sigs.get(callee) {
                        for (i, &arg) in args.iter().enumerate() {
                            if i < callee_sig.params.len()
                                && callee_sig.params[i].ownership == Ownership::Owned
                            {
                                changed |= try_mark_param_owned(arg, func, &mut my_sig);
                            }
                        }
                    } else {
                        // Unknown callee (external/runtime) — all args must be owned.
                        for &arg in args {
                            changed |= try_mark_param_owned(arg, func, &mut my_sig);
                        }
                    }
                }

                ArcInstr::ApplyIndirect { closure, args, .. } => {
                    // Unknown callee — all arguments and closure must be owned.
                    changed |= try_mark_param_owned(*closure, func, &mut my_sig);
                    for &arg in args {
                        changed |= try_mark_param_owned(arg, func, &mut my_sig);
                    }
                }

                ArcInstr::PartialApply { args, .. } => {
                    // All captured args stored in closure env — must be owned.
                    for &arg in args {
                        changed |= try_mark_param_owned(arg, func, &mut my_sig);
                    }
                }

                ArcInstr::Construct { args, .. } => {
                    // Args stored into a data structure — must be owned.
                    for &arg in args {
                        changed |= try_mark_param_owned(arg, func, &mut my_sig);
                    }
                }

                ArcInstr::Project { dst, value, .. } => {
                    // Bidirectional propagation: if the projected result is
                    // owned, the source must also be owned (prevents
                    // use-after-free on the projected field).
                    if is_owned_var(*dst, func, &my_sig) {
                        changed |= try_mark_param_owned(*value, func, &mut my_sig);
                    }
                }

                ArcInstr::Let { value, .. } => {
                    // Let { dst, value: Var(x) } is an alias — no ownership
                    // transfer implied. RC insertion handles liveness.
                    // PrimOp and Literal are scalar — no RC concern.
                    let _ = value;
                }

                // RC and reuse operations are not present after initial
                // lowering (they're inserted by later passes). Skip them.
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
                // Returning a parameter transfers ownership to the caller.
                changed |= try_mark_param_owned(*value, func, &mut my_sig);

                // Tail call preservation: if this return immediately follows
                // an Apply whose result is the returned value, check whether
                // any arguments need ownership promotion.
                changed |= check_tail_call(block, *value, func, &mut my_sig, sigs);
            }

            ArcTerminator::Jump { args, .. } => {
                // Jump args flow into block parameters — no ownership
                // concern for the jump itself. Block params are locals
                // in the target block and handled there.
                let _ = args;
            }

            ArcTerminator::Branch { .. }
            | ArcTerminator::Switch { .. }
            | ArcTerminator::Unreachable
            | ArcTerminator::Resume => {}

            ArcTerminator::Invoke {
                args, func: callee, ..
            } => {
                // Same as Apply — check callee param ownership.
                if let Some(callee_sig) = sigs.get(callee) {
                    for (i, &arg) in args.iter().enumerate() {
                        if i < callee_sig.params.len()
                            && callee_sig.params[i].ownership == Ownership::Owned
                        {
                            changed |= try_mark_param_owned(arg, func, &mut my_sig);
                        }
                    }
                } else {
                    for &arg in args {
                        changed |= try_mark_param_owned(arg, func, &mut my_sig);
                    }
                }
            }
        }
    }

    // Write back the (possibly-updated) signature.
    if changed {
        sigs.insert(func.name, my_sig);
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
    sigs: &FxHashMap<Name, AnnotatedSig>,
) -> bool {
    let mut changed = false;

    // Find the last Apply in the block whose dst matches the returned value.
    let tail_apply = block
        .body
        .iter()
        .rev()
        .find(|instr| matches!(instr, ArcInstr::Apply { dst, .. } if *dst == returned_value));

    if let Some(ArcInstr::Apply {
        func: callee, args, ..
    }) = tail_apply
    {
        // Get the callee's param ownership info.
        if let Some(callee_sig) = sigs.get(callee) {
            for (i, &arg) in args.iter().enumerate() {
                if i < callee_sig.params.len() && callee_sig.params[i].ownership == Ownership::Owned
                {
                    // If arg is a param that's currently Borrowed, promote it.
                    // This preserves the tail call: no Dec needed after the call.
                    changed |= try_mark_param_owned(arg, func, my_sig);
                }
            }
        }
    }

    changed
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
