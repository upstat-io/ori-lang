use ori_ir::canon::{CanArena, CanExpr, CanNode, CanonResult};
use ori_ir::{Name, Span, StringInterner, TypeId};
use ori_types::{Idx, Pool};

use crate::ir::{ArcInstr, ArcTerminator, ArcValue, CtorKind, LitValue};
use crate::lower::ArcLoweringInput;

/// Wrap a built arena into a single-root `CanonResult` and lower it.
fn lower_root(
    arena: CanArena,
    root: ori_ir::canon::CanId,
    interner: &StringInterner,
    pool: &Pool,
    ret_ty: Idx,
) {
    let canon = CanonResult::new(arena, root);
    let mut problems = Vec::new();
    let _ = super::super::super::lower_function_can(
        ArcLoweringInput {
            name: Name::from_raw(1),
            params: &[],
            return_type: ret_ty,
            body: root,
            canon: &canon,
            interner,
            pool,
            type_subst: None,
            const_bindings: None,
            is_fbip: false,
        },
        &mut problems,
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

    let canon = CanonResult::new(arena, try_expr);
    let mut problems = Vec::new();
    let (func, _) = super::super::super::lower_function_can(
        ArcLoweringInput {
            name: Name::from_raw(1),
            params: &[],
            return_type: Idx::INT,
            body: try_expr,
            canon: &canon,
            interner: &interner,
            pool: &pool,
            type_subst: None,
            const_bindings: None,
            is_fbip: false,
        },
        &mut problems,
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

    let canon = CanonResult::new(arena, try_expr);
    let mut problems = Vec::new();
    let (func, _) = super::super::super::lower_function_can(
        ArcLoweringInput {
            name: Name::from_raw(1),
            params: &[],
            return_type: Idx::INT,
            body: try_expr,
            canon: &canon,
            interner: &interner,
            pool: &pool,
            type_subst: None,
            const_bindings: None,
            is_fbip: false,
        },
        &mut problems,
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
    let mut pool = Pool::new();
    let tuple_ty = pool.tuple(&[Idx::INT, Idx::INT]);
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
        TypeId::from_raw(tuple_ty.raw()),
    ));

    let canon = CanonResult::new(arena, tup);

    let mut problems = Vec::new();
    let (func, _) = super::super::super::lower_function_can(
        ArcLoweringInput {
            name: Name::from_raw(1),
            params: &[],
            return_type: tuple_ty,
            body: tup,
            canon: &canon,
            interner: &interner,
            pool: &pool,
            type_subst: None,
            const_bindings: None,
            is_fbip: false,
        },
        &mut problems,
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
fn lower_empty_tuple_as_unit_literal() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut arena = CanArena::with_capacity(1);
    let exprs = arena.push_expr_list(&[]);
    let unit = arena.push(CanNode::new(
        CanExpr::Tuple(exprs),
        Span::new(0, 2),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));
    let canon = CanonResult::new(arena, unit);
    let mut problems = Vec::new();

    let (function, _) = super::super::super::lower_function_can(
        ArcLoweringInput {
            name: Name::from_raw(1),
            params: &[],
            return_type: Idx::UNIT,
            body: unit,
            canon: &canon,
            interner: &interner,
            pool: &pool,
            type_subst: None,
            const_bindings: None,
            is_fbip: false,
        },
        &mut problems,
    );

    assert!(problems.is_empty());
    assert!(matches!(
        function.blocks[0].body.as_slice(),
        [ArcInstr::Let {
            ty: Idx::UNIT,
            value: ArcValue::Literal(LitValue::Unit),
            ..
        }]
    ));
}

#[test]
fn list_index_retains_unwind_carrier_without_lexical_catch() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let list_ty = pool.list(Idx::INT);
    let mut arena = CanArena::with_capacity(8);

    let items: Vec<_> = [10, 20, 30]
        .into_iter()
        .map(|value| {
            arena.push(CanNode::new(
                CanExpr::Int(value),
                Span::new(0, 1),
                TypeId::from_raw(Idx::INT.raw()),
            ))
        })
        .collect();
    let item_range = arena.push_expr_list(&items);
    let receiver = arena.push(CanNode::new(
        CanExpr::List(item_range),
        Span::new(0, 8),
        TypeId::from_raw(list_ty.raw()),
    ));
    let index = arena.push(CanNode::new(
        CanExpr::Int(-1),
        Span::new(9, 11),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let root = arena.push(CanNode::new(
        CanExpr::Index {
            receiver,
            index,
            producer: None,
        },
        Span::new(0, 12),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let canon = CanonResult::new(arena, root);
    let mut problems = Vec::new();

    let (function, _) = super::super::super::lower_function_can(
        ArcLoweringInput {
            name: Name::from_raw(1),
            params: &[],
            return_type: Idx::INT,
            body: root,
            canon: &canon,
            interner: &interner,
            pool: &pool,
            type_subst: None,
            const_bindings: None,
            is_fbip: false,
        },
        &mut problems,
    );

    assert!(problems.is_empty());
    let index_name = interner.intern("__index");
    assert!(function.blocks.iter().any(|block| matches!(
        block.terminator,
        ArcTerminator::Invoke { func, .. } if func == index_name
    )));
    assert!(function
        .blocks
        .iter()
        .any(|block| matches!(block.terminator, ArcTerminator::Resume)));
}

/// A generalized lambda receiver reaches ARC as a quantified `BoundVar`.
/// Index lowering must preserve the protocol call until specialization rather
/// than misclassifying the parametric receiver as a concrete user type.
#[test]
fn bound_var_index_without_selected_producer_retains_protocol_call() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let receiver_name = interner.intern("values");
    let receiver_ty = pool.bound_var(0);
    let mut arena = CanArena::with_capacity(3);
    let receiver = arena.push(CanNode::new(
        CanExpr::Ident(receiver_name),
        Span::DUMMY,
        TypeId::from_raw(receiver_ty.raw()),
    ));
    let index = arena.push(CanNode::new(
        CanExpr::Int(0),
        Span::DUMMY,
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let root = arena.push(CanNode::new(
        CanExpr::Index {
            receiver,
            index,
            producer: None,
        },
        Span::DUMMY,
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let canon = CanonResult::new(arena, root);
    let params = [(receiver_name, receiver_ty)];
    let mut problems = Vec::new();

    let (function, _) = super::super::super::lower_function_can(
        ArcLoweringInput {
            name: interner.intern("first"),
            params: &params,
            return_type: Idx::INT,
            body: root,
            canon: &canon,
            interner: &interner,
            pool: &pool,
            type_subst: None,
            const_bindings: None,
            is_fbip: false,
        },
        &mut problems,
    );

    assert!(
        problems.is_empty(),
        "parametric index must remain a protocol call: {problems:?}"
    );
    let index_name = interner.intern("__index");
    assert!(function.blocks.iter().any(|block| matches!(
        block.terminator,
        ArcTerminator::Invoke { func, .. } if func == index_name
    )));
}

/// A concrete user receiver without type-checker-selected provenance remains
/// an internal contract violation; the parametric exception must stay narrow.
#[test]
fn concrete_user_index_without_selected_producer_reports_internal_error() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let receiver_name = interner.intern("value");
    let type_name = interner.intern("Indexable");
    let field_name = interner.intern("value");
    let receiver_ty = pool.struct_type(type_name, &[(field_name, Idx::INT)]);
    let mut arena = CanArena::with_capacity(3);
    let receiver = arena.push(CanNode::new(
        CanExpr::Ident(receiver_name),
        Span::DUMMY,
        TypeId::from_raw(receiver_ty.raw()),
    ));
    let index = arena.push(CanNode::new(
        CanExpr::Int(0),
        Span::DUMMY,
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let root = arena.push(CanNode::new(
        CanExpr::Index {
            receiver,
            index,
            producer: None,
        },
        Span::DUMMY,
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let canon = CanonResult::new(arena, root);
    let params = [(receiver_name, receiver_ty)];
    let mut problems = Vec::new();

    let _ = super::super::super::lower_function_can(
        ArcLoweringInput {
            name: interner.intern("read"),
            params: &params,
            return_type: Idx::INT,
            body: root,
            canon: &canon,
            interner: &interner,
            pool: &pool,
            type_subst: None,
            const_bindings: None,
            is_fbip: false,
        },
        &mut problems,
    );

    assert!(matches!(
        problems.as_slice(),
        [super::super::ArcProblem::InternalError { message, .. }]
            if message.contains("no type-checker-selected method producer")
    ));
}

#[test]
fn user_index_lowers_as_may_unwind_call_with_selected_producer() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let receiver_name = interner.intern("value");
    let type_name = interner.intern("Indexable");
    let field_name = interner.intern("value");
    let receiver_ty = pool.struct_type(type_name, &[(field_name, Idx::INT)]);
    let selected = ori_ir::canon::MethodProducerId::new(7);
    let mut arena = CanArena::with_capacity(3);
    let receiver = arena.push(CanNode::new(
        CanExpr::Ident(receiver_name),
        Span::DUMMY,
        TypeId::from_raw(receiver_ty.raw()),
    ));
    let index = arena.push(CanNode::new(
        CanExpr::Int(0),
        Span::DUMMY,
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let root = arena.push(CanNode::new(
        CanExpr::Index {
            receiver,
            index,
            producer: Some(selected),
        },
        Span::DUMMY,
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let canon = CanonResult::new(arena, root);
    let params = [(receiver_name, receiver_ty)];
    let mut problems = Vec::new();

    let (function, _) = super::super::super::lower_function_can(
        ArcLoweringInput {
            name: interner.intern("read"),
            params: &params,
            return_type: Idx::INT,
            body: root,
            canon: &canon,
            interner: &interner,
            pool: &pool,
            type_subst: None,
            const_bindings: None,
            is_fbip: false,
        },
        &mut problems,
    );

    assert!(
        problems.is_empty(),
        "unexpected lowering problems: {problems:?}"
    );
    let index_name = interner.intern("index");
    assert!(function.blocks.iter().any(|block| matches!(
        block.terminator,
        ArcTerminator::Invoke { func, .. } if func == index_name
    )));
    assert!(matches!(
        function.method_call_facts.as_slice(),
        [fact]
            if fact.receiver_type == receiver_ty
                && fact.producer.is_none()
                && fact.selected_producer == Some(selected)
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

    let canon = CanonResult::new(arena, none_id);

    let mut problems = Vec::new();
    let (func, _) = super::super::super::lower_function_can(
        ArcLoweringInput {
            name: Name::from_raw(1),
            params: &[],
            return_type: Idx::UNIT,
            body: none_id,
            canon: &canon,
            interner: &interner,
            pool: &pool,
            type_subst: None,
            const_bindings: None,
            is_fbip: false,
        },
        &mut problems,
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
