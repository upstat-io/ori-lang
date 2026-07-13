use ori_ir::canon::{CanArena, CanExpr, CanNode, CanonResult};
use ori_ir::{Name, Span, StringInterner, TypeId};
use ori_types::{Idx, Pool};

use crate::ir::{ArcInstr, CtorKind};

/// Wrap a built arena into a single-root `CanonResult` and lower it.
fn lower_root(
    arena: CanArena,
    root: ori_ir::canon::CanId,
    interner: &StringInterner,
    pool: &Pool,
    ret_ty: Idx,
) {
    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
        mono_dispatch_map_can: vec![],
    };
    let mut problems = Vec::new();
    let _ = super::super::super::lower_function_can(
        Name::from_raw(1),
        &[],
        ret_ty,
        root,
        &canon,
        interner,
        pool,
        &mut problems,
        false,
        None,
    );
}

/// Negative pin: a field name resolving in neither the struct's fields nor
/// as a tuple ordinal is an internal invariant violation (typeck resolves
/// every field access per PC-2). Lowering must fail loudly — silently
/// projecting field 0 reads the WRONG FIELD in release builds.
#[test]
#[should_panic(expected = "PC-2 violation")]
fn resolve_field_index_unknown_struct_field_panics_not_field_zero() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let struct_name = interner.intern("Point");
    let x_field = interner.intern("x");
    let bogus_field = interner.intern("definitely_not_a_field");
    let struct_ty = pool.struct_type(struct_name, &[(x_field, Idx::INT)]);

    let mut arena = CanArena::with_capacity(200);
    let recv = arena.push(CanNode::new(
        CanExpr::Int(0),
        Span::new(0, 1),
        TypeId::from_raw(struct_ty.raw()),
    ));
    let access = arena.push(CanNode::new(
        CanExpr::Field {
            receiver: recv,
            field: bogus_field,
        },
        Span::new(0, 3),
        TypeId::from_raw(Idx::INT.raw()),
    ));

    lower_root(arena, access, &interner, &pool, Idx::INT);
}

/// Negative pin: `?` applied to a non-Option/Result scrutinee is rejected
/// by typeck — reaching `lower_try` with such a tag is an internal
/// invariant violation that must fail loudly, never silently return the
/// scrutinee.
#[test]
#[should_panic(expected = "PC-2 violation")]
fn lower_try_non_option_result_scrutinee_panics_not_returns_scrutinee() {
    let interner = StringInterner::new();
    let pool = Pool::new();

    let mut arena = CanArena::with_capacity(200);
    let inner = arena.push(CanNode::new(
        CanExpr::Int(7),
        Span::new(0, 1),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let try_expr = arena.push(CanNode::new(
        CanExpr::Try(inner),
        Span::new(0, 2),
        TypeId::from_raw(Idx::INT.raw()),
    ));

    lower_root(arena, try_expr, &interner, &pool, Idx::INT);
}

/// Positive control: `expr?` on `Result<int, Error>` where `Error` is the
/// registered error struct MUST inject `__ori_inject_trace` on the Err
/// payload before re-wrapping — the dual-identity guard (`is_error_struct_receiver`)
/// accepting the genuine Error identity.
#[test]
fn lower_try_injects_trace_on_genuine_error_err_payload() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let error_name = interner.intern("Error");
    let error_struct_idx = pool.named(error_name);
    pool.set_error_struct_idx(error_struct_idx);
    let result_ty = pool.result(Idx::INT, error_struct_idx);

    let mut arena = CanArena::with_capacity(200);
    let inner = arena.push(CanNode::new(
        CanExpr::Int(0),
        Span::new(0, 1),
        TypeId::from_raw(result_ty.raw()),
    ));
    let try_expr = arena.push(CanNode::new(
        CanExpr::Try(inner),
        Span::new(0, 2),
        TypeId::from_raw(Idx::INT.raw()),
    ));

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: try_expr,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
        mono_dispatch_map_can: vec![],
    };
    let mut problems = Vec::new();
    let (func, _) = super::super::super::lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::INT,
        try_expr,
        &canon,
        &interner,
        &pool,
        &mut problems,
        false,
        None,
    );

    assert!(problems.is_empty());
    let inject_fn = interner.intern("__ori_inject_trace");
    let injected = func.blocks.iter().any(|block| {
        block
            .body
            .iter()
            .any(|instr| matches!(instr, ArcInstr::Apply { func, .. } if *func == inject_fn))
    });
    assert!(
        injected,
        "lower_try must inject __ori_inject_trace on a genuine Error Err payload"
    );
}

/// Negative control: `expr?` on `Result<int, MyError>` where `MyError` is a
/// newtype over `Error` (`type MyError = Error`) MUST NOT inject
/// `__ori_inject_trace` — the newtype's distinct idx never matches
/// `is_error_struct_receiver` (`chase_var_links` never crosses the
/// resolutions map), preserving newtype nominal typing (TI-5).
#[test]
fn lower_try_does_not_inject_trace_on_newtype_over_error_err_payload() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let error_name = interner.intern("Error");
    let error_struct_idx = pool.named(error_name);
    pool.set_error_struct_idx(error_struct_idx);
    let myerror_name = interner.intern("MyError");
    let myerror_idx = pool.named(myerror_name);
    pool.set_resolution(myerror_idx, error_struct_idx);
    let result_ty = pool.result(Idx::INT, myerror_idx);

    let mut arena = CanArena::with_capacity(200);
    let inner = arena.push(CanNode::new(
        CanExpr::Int(0),
        Span::new(0, 1),
        TypeId::from_raw(result_ty.raw()),
    ));
    let try_expr = arena.push(CanNode::new(
        CanExpr::Try(inner),
        Span::new(0, 2),
        TypeId::from_raw(Idx::INT.raw()),
    ));

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: try_expr,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
        mono_dispatch_map_can: vec![],
    };
    let mut problems = Vec::new();
    let (func, _) = super::super::super::lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::INT,
        try_expr,
        &canon,
        &interner,
        &pool,
        &mut problems,
        false,
        None,
    );

    assert!(problems.is_empty());
    let inject_fn = interner.intern("__ori_inject_trace");
    let injected = func.blocks.iter().any(|block| {
        block
            .body
            .iter()
            .any(|instr| matches!(instr, ArcInstr::Apply { func, .. } if *func == inject_fn))
    });
    assert!(
        !injected,
        "lower_try must NOT inject __ori_inject_trace on a newtype-over-Error Err payload"
    );
}

#[test]
fn lower_tuple() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut arena = CanArena::with_capacity(200);

    let a = arena.push(CanNode::new(
        CanExpr::Int(1),
        Span::new(1, 2),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let b = arena.push(CanNode::new(
        CanExpr::Int(2),
        Span::new(4, 5),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let exprs = arena.push_expr_list(&[a, b]);
    let tup = arena.push(CanNode::new(
        CanExpr::Tuple(exprs),
        Span::new(0, 6),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: tup,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
        mono_dispatch_map_can: vec![],
    };

    let mut problems = Vec::new();
    let (func, _) = super::super::super::lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::UNIT,
        tup,
        &canon,
        &interner,
        &pool,
        &mut problems,
        false,
        None,
    );

    assert!(problems.is_empty());
    let last = &func.blocks[0].body[2];
    assert!(matches!(
        last,
        ArcInstr::Construct {
            ctor: CtorKind::Tuple,
            ..
        }
    ));
}

#[test]
fn lower_none() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut arena = CanArena::with_capacity(200);

    let none_id = arena.push(CanNode::new(
        CanExpr::None,
        Span::new(0, 4),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: none_id,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
        mono_dispatch_map_can: vec![],
    };

    let mut problems = Vec::new();
    let (func, _) = super::super::super::lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::UNIT,
        none_id,
        &canon,
        &interner,
        &pool,
        &mut problems,
        false,
        None,
    );

    assert!(problems.is_empty());
    let last = &func.blocks[0].body[0];
    assert!(matches!(
        last,
        ArcInstr::Construct {
            ctor: CtorKind::EnumVariant { variant: 1, .. },
            ..
        }
    ));
}
