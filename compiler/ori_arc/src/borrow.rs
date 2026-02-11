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
use crate::ownership::{AnnotatedParam, AnnotatedSig, Ownership};
use crate::{ArcClassification, ArcClassifier};

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
    classifier: &ArcClassifier,
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
pub fn apply_borrows<S: std::hash::BuildHasher>(
    functions: &mut [ArcFunction],
    sigs: &std::collections::HashMap<Name, AnnotatedSig, S>,
) {
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
    classifier: &ArcClassifier,
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
                        let callee_params = callee_sig.params.clone();
                        for (i, &arg) in args.iter().enumerate() {
                            if i < callee_params.len()
                                && callee_params[i].ownership == Ownership::Owned
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
                    let callee_params = callee_sig.params.clone();
                    for (i, &arg) in args.iter().enumerate() {
                        if i < callee_params.len() && callee_params[i].ownership == Ownership::Owned
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
            let callee_params = callee_sig.params.clone();
            for (i, &arg) in args.iter().enumerate() {
                if i < callee_params.len() && callee_params[i].ownership == Ownership::Owned {
                    // If arg is a param that's currently Borrowed, promote it.
                    // This preserves the tail call: no Dec needed after the call.
                    changed |= try_mark_param_owned(arg, func, my_sig);
                }
            }
        }
    }

    changed
}

#[cfg(test)]
mod tests {
    use ori_ir::Name;
    use ori_types::{Idx, Pool};

    use crate::ir::{
        ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
        CtorKind, LitValue,
    };
    use crate::ownership::Ownership;
    use crate::ArcClassifier;

    use super::infer_borrows;

    /// Helper: build a minimal `ArcFunction`.
    fn make_func(
        name: Name,
        params: Vec<ArcParam>,
        return_type: Idx,
        blocks: Vec<ArcBlock>,
        var_types: Vec<Idx>,
    ) -> ArcFunction {
        let span_vecs: Vec<Vec<Option<ori_ir::Span>>> =
            blocks.iter().map(|b| vec![None; b.body.len()]).collect();
        ArcFunction {
            name,
            params,
            return_type,
            blocks,
            entry: ArcBlockId::new(0),
            var_types,
            spans: span_vecs,
        }
    }

    fn param(var: u32, ty: Idx) -> ArcParam {
        ArcParam {
            var: ArcVarId::new(var),
            ty,
            ownership: Ownership::Owned, // Default from lowering.
        }
    }

    fn v(n: u32) -> ArcVarId {
        ArcVarId::new(n)
    }

    fn b(n: u32) -> ArcBlockId {
        ArcBlockId::new(n)
    }

    // ── Pure function: all params should stay Borrowed ──────

    #[test]
    fn pure_function_all_borrowed() {
        // fn add(a: str, b: str) -> int
        //   let v2 = prim_op(a, b)  // just reads, doesn't store
        //   return v2
        let func = make_func(
            Name::from_raw(1),
            vec![param(0, Idx::STR), param(1, Idx::STR)],
            Idx::INT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::INT,
                    value: ArcValue::PrimOp {
                        op: crate::PrimOp::Binary(ori_ir::BinaryOp::Add),
                        args: vec![v(0), v(1)],
                    },
                }],
                terminator: ArcTerminator::Return { value: v(2) },
            }],
            vec![Idx::STR, Idx::STR, Idx::INT],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let sigs = infer_borrows(&[func], &classifier);

        let sig = &sigs[&Name::from_raw(1)];
        assert_eq!(sig.params[0].ownership, Ownership::Borrowed);
        assert_eq!(sig.params[1].ownership, Ownership::Borrowed);
    }

    // ── Return a parameter: must be Owned ───────────────────

    #[test]
    fn return_param_becomes_owned() {
        // fn identity(x: str) -> str
        //   return x
        let func = make_func(
            Name::from_raw(1),
            vec![param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::STR],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let sigs = infer_borrows(&[func], &classifier);

        let sig = &sigs[&Name::from_raw(1)];
        assert_eq!(sig.params[0].ownership, Ownership::Owned);
    }

    // ── Store param in Construct: must be Owned ─────────────

    #[test]
    fn construct_param_becomes_owned() {
        // fn wrap(x: str) -> (str,)
        //   let v1 = Tuple(x)
        //   return v1
        let func = make_func(
            Name::from_raw(1),
            vec![param(0, Idx::STR)],
            Idx::UNIT, // placeholder
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Construct {
                    dst: v(1),
                    ty: Idx::UNIT,
                    ctor: CtorKind::Tuple,
                    args: vec![v(0)],
                }],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::UNIT],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let sigs = infer_borrows(&[func], &classifier);

        let sig = &sigs[&Name::from_raw(1)];
        assert_eq!(sig.params[0].ownership, Ownership::Owned);
    }

    // ── Scalar params stay Owned (skipped by inference) ─────

    #[test]
    fn scalar_param_stays_owned() {
        // fn id_int(x: int) -> int
        //   return x
        let func = make_func(
            Name::from_raw(1),
            vec![param(0, Idx::INT)],
            Idx::INT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::INT],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let sigs = infer_borrows(&[func], &classifier);

        // Scalar params stay Owned (not Borrowed) since borrow inference
        // skips them — they have no RC regardless.
        let sig = &sigs[&Name::from_raw(1)];
        assert_eq!(sig.params[0].ownership, Ownership::Owned);
    }

    // ── ApplyIndirect: all args Owned ───────────────────────

    #[test]
    fn apply_indirect_all_args_owned() {
        // fn call_closure(f: fn, x: str) -> int
        //   let v2 = f(x)  // indirect call
        //   return v2
        let func = make_func(
            Name::from_raw(1),
            vec![param(0, Idx::STR), param(1, Idx::STR)], // both ref-typed
            Idx::INT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::ApplyIndirect {
                    dst: v(2),
                    ty: Idx::INT,
                    closure: v(0),
                    args: vec![v(1)],
                }],
                terminator: ArcTerminator::Return { value: v(2) },
            }],
            vec![Idx::STR, Idx::STR, Idx::INT],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let sigs = infer_borrows(&[func], &classifier);

        let sig = &sigs[&Name::from_raw(1)];
        // Both closure and arg must be Owned (unknown callee).
        assert_eq!(sig.params[0].ownership, Ownership::Owned);
        assert_eq!(sig.params[1].ownership, Ownership::Owned);
    }

    // ── PartialApply: captured args Owned ───────────────────

    #[test]
    fn partial_apply_captures_owned() {
        // fn make_adder(x: str) -> fn
        //   let v1 = partial_apply add(x)
        //   return v1
        let func = make_func(
            Name::from_raw(1),
            vec![param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::PartialApply {
                    dst: v(1),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![v(0)],
                }],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::STR],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let sigs = infer_borrows(&[func], &classifier);

        let sig = &sigs[&Name::from_raw(1)];
        assert_eq!(sig.params[0].ownership, Ownership::Owned);
    }

    // ── Projection propagation ──────────────────────────────

    #[test]
    fn projection_propagates_ownership() {
        // fn get_first(pair: (str, str)) -> str
        //   let v1 = pair.0  // project
        //   return v1         // returning v1 → v1 is owned → pair must be owned
        let func = make_func(
            Name::from_raw(1),
            vec![param(0, Idx::STR)], // pair type (simplified as STR for test)
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Project {
                    dst: v(1),
                    ty: Idx::STR,
                    value: v(0),
                    field: 0,
                }],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::STR],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let sigs = infer_borrows(&[func], &classifier);

        let sig = &sigs[&Name::from_raw(1)];
        // v1 is returned (owned), so pair (v0) must also be owned.
        assert_eq!(sig.params[0].ownership, Ownership::Owned);
    }

    // ── Mixed: some borrowed, some owned ────────────────────

    #[test]
    fn mixed_borrowed_and_owned() {
        // fn process(a: str, b: str) -> str
        //   let v2 = prim_op(a, b)  // reads both (borrowed ok)
        //   let v3 = Tuple(b)       // stores b (must be owned)
        //   return v2
        let func = make_func(
            Name::from_raw(1),
            vec![param(0, Idx::STR), param(1, Idx::STR)],
            Idx::INT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::INT,
                        value: ArcValue::PrimOp {
                            op: crate::PrimOp::Binary(ori_ir::BinaryOp::Add),
                            args: vec![v(0), v(1)],
                        },
                    },
                    ArcInstr::Construct {
                        dst: v(3),
                        ty: Idx::UNIT,
                        ctor: CtorKind::Tuple,
                        args: vec![v(1)],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(2) },
            }],
            vec![Idx::STR, Idx::STR, Idx::INT, Idx::UNIT],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let sigs = infer_borrows(&[func], &classifier);

        let sig = &sigs[&Name::from_raw(1)];
        assert_eq!(sig.params[0].ownership, Ownership::Borrowed); // a: only read
        assert_eq!(sig.params[1].ownership, Ownership::Owned); // b: stored
    }

    // ── Mutual recursion converges ──────────────────────────

    #[test]
    fn mutual_recursion_converges() {
        // fn f(x: str) -> str { g(x) }
        // fn g(y: str) -> str { f(y) }
        //
        // Neither stores — both should remain Borrowed... but they pass
        // to each other's owned position. However, initially both start
        // Borrowed, so the first iteration finds no owned positions.
        // They converge as Borrowed.
        let f = make_func(
            Name::from_raw(1),
            vec![param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(1),
                    ty: Idx::STR,
                    func: Name::from_raw(2), // calls g
                    args: vec![v(0)],
                }],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::STR],
        );

        let g = make_func(
            Name::from_raw(2),
            vec![param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(1),
                    ty: Idx::STR,
                    func: Name::from_raw(1), // calls f
                    args: vec![v(0)],
                }],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::STR],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let sigs = infer_borrows(&[f, g], &classifier);

        // Both just pass through to each other in tail position.
        // The Return { value: v(1) } makes v(1) owned (it's a local, always owned).
        // The Apply passes v(0) to a Borrowed position.
        // So both params remain Borrowed.
        let sig_f = &sigs[&Name::from_raw(1)];
        let sig_g = &sigs[&Name::from_raw(2)];
        assert_eq!(sig_f.params[0].ownership, Ownership::Borrowed);
        assert_eq!(sig_g.params[0].ownership, Ownership::Borrowed);
    }

    // ── Mutual recursion with storing ───────────────────────

    #[test]
    fn mutual_recursion_with_store_propagates() {
        // fn f(x: str) -> str { g(x) }
        // fn g(y: str) -> str { let t = Tuple(y); return t.0 }
        //
        // g stores y → g's param is Owned
        // f calls g with x → f's param must also be Owned
        let f = make_func(
            Name::from_raw(1),
            vec![param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(1),
                    ty: Idx::STR,
                    func: Name::from_raw(2), // calls g
                    args: vec![v(0)],
                }],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::STR],
        );

        let g = make_func(
            Name::from_raw(2),
            vec![param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Construct {
                        dst: v(1),
                        ty: Idx::UNIT,
                        ctor: CtorKind::Tuple,
                        args: vec![v(0)], // stores y
                    },
                    ArcInstr::Project {
                        dst: v(2),
                        ty: Idx::STR,
                        value: v(1),
                        field: 0,
                    },
                ],
                terminator: ArcTerminator::Return { value: v(2) },
            }],
            vec![Idx::STR, Idx::UNIT, Idx::STR],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let sigs = infer_borrows(&[f, g], &classifier);

        let sig_g = &sigs[&Name::from_raw(2)];
        assert_eq!(sig_g.params[0].ownership, Ownership::Owned); // g stores y

        let sig_f = &sigs[&Name::from_raw(1)];
        assert_eq!(sig_f.params[0].ownership, Ownership::Owned); // f passes to g's Owned
    }

    // ── Empty function (no params) ──────────────────────────

    #[test]
    fn empty_function_no_params() {
        let func = make_func(
            Name::from_raw(1),
            vec![],
            Idx::UNIT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(0),
                    ty: Idx::UNIT,
                    value: ArcValue::Literal(LitValue::Unit),
                }],
                terminator: ArcTerminator::Return { value: v(0) },
            }],
            vec![Idx::UNIT],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let sigs = infer_borrows(&[func], &classifier);

        let sig = &sigs[&Name::from_raw(1)];
        assert!(sig.params.is_empty());
    }

    // ── Tail call preservation ──────────────────────────────

    #[test]
    fn tail_call_promotes_borrowed_to_owned() {
        // fn f(x: str) -> str
        //   let v1 = g(x)   // g expects x as Owned
        //   return v1        // tail call — must promote x
        //
        // fn g(y: str) -> str
        //   let v1 = Tuple(y)  // stores y → Owned
        //   return v1
        let f = make_func(
            Name::from_raw(1),
            vec![param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(1),
                    ty: Idx::STR,
                    func: Name::from_raw(2), // calls g
                    args: vec![v(0)],
                }],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::STR],
        );

        let g = make_func(
            Name::from_raw(2),
            vec![param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Construct {
                    dst: v(1),
                    ty: Idx::UNIT,
                    ctor: CtorKind::Tuple,
                    args: vec![v(0)],
                }],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::UNIT],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let sigs = infer_borrows(&[f, g], &classifier);

        // g stores y → Owned
        assert_eq!(
            sigs[&Name::from_raw(2)].params[0].ownership,
            Ownership::Owned
        );
        // f's tail call to g promotes x → Owned
        assert_eq!(
            sigs[&Name::from_raw(1)].params[0].ownership,
            Ownership::Owned
        );
    }

    // ── Unknown callee (not in function set) ────────────────

    #[test]
    fn unknown_callee_marks_args_owned() {
        // fn f(x: str) -> str
        //   let v1 = external_fn(x)  // not in our function set
        //   return v1
        let func = make_func(
            Name::from_raw(1),
            vec![param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(1),
                    ty: Idx::STR,
                    func: Name::from_raw(999), // not in set
                    args: vec![v(0)],
                }],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::STR],
        );

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let sigs = infer_borrows(&[func], &classifier);

        // Unknown callee → assume all args Owned (conservative).
        let sig = &sigs[&Name::from_raw(1)];
        assert_eq!(sig.params[0].ownership, Ownership::Owned);
    }

    // ── apply_borrows writes results back to ArcFunction ────

    #[test]
    fn apply_borrows_updates_params() {
        use super::apply_borrows;

        // fn f(a: str, b: str) -> int
        //   let v2 = prim_op(a, b)  // reads both
        //   let v3 = Tuple(b)       // stores b
        //   return v2
        let func = make_func(
            Name::from_raw(1),
            vec![param(0, Idx::STR), param(1, Idx::STR)],
            Idx::INT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::INT,
                        value: ArcValue::PrimOp {
                            op: crate::PrimOp::Binary(ori_ir::BinaryOp::Add),
                            args: vec![v(0), v(1)],
                        },
                    },
                    ArcInstr::Construct {
                        dst: v(3),
                        ty: Idx::UNIT,
                        ctor: CtorKind::Tuple,
                        args: vec![v(1)],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(2) },
            }],
            vec![Idx::STR, Idx::STR, Idx::INT, Idx::UNIT],
        );

        // Both start as Owned (from lowering).
        assert_eq!(func.params[0].ownership, Ownership::Owned);
        assert_eq!(func.params[1].ownership, Ownership::Owned);

        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let sigs = infer_borrows(std::slice::from_ref(&func), &classifier);

        // Apply borrow results to a mutable copy.
        let mut funcs = vec![func];
        apply_borrows(&mut funcs, &sigs);

        assert_eq!(funcs[0].params[0].ownership, Ownership::Borrowed); // a: only read
        assert_eq!(funcs[0].params[1].ownership, Ownership::Owned); // b: stored
    }
}
