use ori_ir::Name;
use ori_types::{Idx, Pool};

use crate::ir::{ArcBlock, ArcInstr, ArcTerminator, ArcValue, CtorKind, LitValue};
use crate::ownership::Ownership;
use crate::test_helpers::{b, make_func_named as make_func, owned_param as param, v};
use crate::ArcClassifier;

use super::infer_borrows;

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

// ── DerivedOwnership tests ──────────────────────────────────

use super::infer_derived_ownership;
use crate::ownership::DerivedOwnership;

/// Borrowed parameter → BorrowedFrom(self).
#[test]
fn derived_borrowed_param() {
    // fn f(x: str) -> int  -- x is borrowed (just read, not stored)
    //   v1 = prim_op(x)    -- reads x but doesn't take ownership
    //   return v1
    let func = make_func(
        Name::from_raw(1),
        vec![param(0, Idx::STR)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: v(1),
                ty: Idx::INT,
                value: ArcValue::Literal(LitValue::Int(42)),
            }],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::STR, Idx::INT],
    );

    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let sigs = infer_borrows(std::slice::from_ref(&func), &classifier);
    let ownership = infer_derived_ownership(&func, &sigs);

    // x is borrowed → BorrowedFrom(v(0))
    assert_eq!(ownership[0], DerivedOwnership::BorrowedFrom(v(0)));
    // v1 is a literal → Owned
    assert_eq!(ownership[1], DerivedOwnership::Owned);
}

/// Projection from Owned var produces `BorrowedFrom`.
#[test]
fn derived_projection_borrows_from_source() {
    // fn f() -> str
    //   v0 = apply g()         -- owned (call result)
    //   v1 = project(v0, 0)    -- borrows from v0
    //   return v1
    let func = make_func(
        Name::from_raw(1),
        vec![],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Apply {
                    dst: v(0),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![],
                },
                ArcInstr::Project {
                    dst: v(1),
                    ty: Idx::STR,
                    value: v(0),
                    field: 0,
                },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::STR, Idx::STR],
    );

    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let sigs = infer_borrows(std::slice::from_ref(&func), &classifier);
    let ownership = infer_derived_ownership(&func, &sigs);

    // v0 is owned (call result)
    assert_eq!(ownership[0], DerivedOwnership::Owned);
    // v1 = project(v0, 0) → borrows from v0
    assert_eq!(ownership[1], DerivedOwnership::BorrowedFrom(v(0)));
}

/// Transitive projection chain: project from a projection.
#[test]
fn derived_projection_chain() {
    // fn f() -> str
    //   v0 = apply g()          -- owned (call result)
    //   v1 = project(v0, 0)     -- borrows from v0
    //   v2 = project(v1, 0)     -- borrows transitively from v0
    //   return v2
    let func = make_func(
        Name::from_raw(1),
        vec![],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Apply {
                    dst: v(0),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![],
                },
                ArcInstr::Project {
                    dst: v(1),
                    ty: Idx::STR,
                    value: v(0),
                    field: 0,
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
        vec![Idx::STR, Idx::STR, Idx::STR],
    );

    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let sigs = infer_borrows(std::slice::from_ref(&func), &classifier);
    let ownership = infer_derived_ownership(&func, &sigs);

    assert_eq!(ownership[0], DerivedOwnership::Owned);
    // v1 borrows from v0
    assert_eq!(ownership[1], DerivedOwnership::BorrowedFrom(v(0)));
    // v2 borrows transitively from v0 (not v1)
    assert_eq!(ownership[2], DerivedOwnership::BorrowedFrom(v(0)));
}

/// Construct produces Fresh (refcount = 1).
#[test]
fn derived_construct_is_fresh() {
    // fn f() -> str
    //   v0: int = 42
    //   v1 = Construct(Struct, [v0])
    //   return v1
    let func = make_func(
        Name::from_raw(1),
        vec![],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: v(0),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(42)),
                },
                ArcInstr::Construct {
                    dst: v(1),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![v(0)],
                },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::INT, Idx::STR],
    );

    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let sigs = infer_borrows(std::slice::from_ref(&func), &classifier);
    let ownership = infer_derived_ownership(&func, &sigs);

    assert_eq!(ownership[0], DerivedOwnership::Owned); // literal
    assert_eq!(ownership[1], DerivedOwnership::Fresh); // construct
}

/// Apply result is Owned.
#[test]
fn derived_apply_result_is_owned() {
    // fn f(x: str) -> str
    //   v1 = apply g(x)
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
                func: Name::from_raw(99),
                args: vec![v(0)],
            }],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::STR, Idx::STR],
    );

    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let sigs = infer_borrows(std::slice::from_ref(&func), &classifier);
    let ownership = infer_derived_ownership(&func, &sigs);

    assert_eq!(ownership[1], DerivedOwnership::Owned); // call result
}

/// Block params are Owned (receive values via jump args).
#[test]
fn derived_block_params_are_owned() {
    // fn f(x: str) -> str
    //   jump b1(x)
    // b1(v1: str):
    //   return v1
    let func = make_func(
        Name::from_raw(1),
        vec![param(0, Idx::STR)],
        Idx::STR,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![v(0)],
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![(v(1), Idx::STR)],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
        ],
        vec![Idx::STR, Idx::STR],
    );

    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let sigs = infer_borrows(std::slice::from_ref(&func), &classifier);
    let ownership = infer_derived_ownership(&func, &sigs);

    // v1 is a block param → Owned
    assert_eq!(ownership[1], DerivedOwnership::Owned);
}

/// Let alias inherits from source.
#[test]
fn derived_let_alias_inherits() {
    // fn f(x: str) -> int
    //   v1 = x             -- alias, inherits ownership
    //   v2 = 42 : int
    //   return v2
    let func = make_func(
        Name::from_raw(1),
        vec![param(0, Idx::STR)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::STR,
                    value: ArcValue::Var(v(0)),
                },
                ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(42)),
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::STR, Idx::STR, Idx::INT],
    );

    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let sigs = infer_borrows(std::slice::from_ref(&func), &classifier);
    let ownership = infer_derived_ownership(&func, &sigs);

    // v0 is borrowed → BorrowedFrom(v(0))
    assert_eq!(ownership[0], DerivedOwnership::BorrowedFrom(v(0)));
    // v1 = Var(v0) → inherits BorrowedFrom(v(0))
    assert_eq!(ownership[1], DerivedOwnership::BorrowedFrom(v(0)));
}
