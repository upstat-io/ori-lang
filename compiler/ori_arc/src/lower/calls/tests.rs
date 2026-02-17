use ori_ir::canon::{CanArena, CanExpr, CanNode, CanonResult};
use ori_ir::{Name, Span, StringInterner, TypeId};
use ori_types::Idx;
use ori_types::Pool;

use crate::ir::{ArcInstr, ArcTerminator};

/// Helper: lower a Call expression and return the resulting function.
fn lower_call_expr(
    interner: &StringInterner,
    func_name: Name,
    arg_val: i64,
) -> crate::ir::ArcFunction {
    let pool = Pool::new();
    let mut arena = CanArena::with_capacity(100);

    let func_ref = arena.push(CanNode::new(
        CanExpr::Ident(func_name),
        Span::new(0, 1),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let arg = arena.push(CanNode::new(
        CanExpr::Int(arg_val),
        Span::new(2, 4),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let args = arena.push_expr_list(&[arg]);
    let call = arena.push(CanNode::new(
        CanExpr::Call {
            func: func_ref,
            args,
        },
        Span::new(0, 5),
        TypeId::from_raw(Idx::INT.raw()),
    ));

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: call,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let mut problems = Vec::new();
    let (func, _) = super::super::super::lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::INT,
        call,
        &canon,
        interner,
        &pool,
        &mut problems,
    );
    assert!(problems.is_empty());
    func
}

#[test]
fn user_call_emits_invoke() {
    let interner = StringInterner::new();
    let func_name = interner.intern("my_function");

    let func = lower_call_expr(&interner, func_name, 42);

    let has_invoke = func.blocks.iter().any(|b| {
        matches!(
            &b.terminator,
            ArcTerminator::Invoke { func, .. } if *func == func_name
        )
    });
    assert!(has_invoke, "expected Invoke terminator for user call");

    let has_apply = func.blocks[0]
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::Apply { func, .. } if *func == func_name));
    assert!(!has_apply, "user call should not emit Apply");
}

#[test]
fn runtime_call_emits_apply() {
    let interner = StringInterner::new();
    let func_name = interner.intern("ori_print_int");

    let func = lower_call_expr(&interner, func_name, 42);

    let has_apply = func.blocks[0]
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::Apply { func, .. } if *func == func_name));
    assert!(has_apply, "expected Apply for runtime call");

    let has_invoke = func.blocks.iter().any(|b| {
        matches!(
            &b.terminator,
            ArcTerminator::Invoke { func, .. } if *func == func_name
        )
    });
    assert!(!has_invoke, "runtime call should not emit Invoke");
}

#[test]
fn compiler_intrinsic_call_emits_apply() {
    let interner = StringInterner::new();
    let func_name = interner.intern("__index");

    let func = lower_call_expr(&interner, func_name, 0);

    let has_apply = func.blocks[0]
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::Apply { func, .. } if *func == func_name));
    assert!(has_apply, "expected Apply for compiler intrinsic");
}

#[test]
fn invoke_creates_normal_and_unwind_blocks() {
    let interner = StringInterner::new();
    let func_name = interner.intern("my_function");

    let func = lower_call_expr(&interner, func_name, 42);

    assert!(
        func.blocks.len() >= 3,
        "expected at least 3 blocks (entry + normal + unwind), got {}",
        func.blocks.len()
    );

    let invoke_block = func
        .blocks
        .iter()
        .find(|b| matches!(&b.terminator, ArcTerminator::Invoke { .. }));
    assert!(invoke_block.is_some(), "expected an Invoke terminator");

    let has_resume = func
        .blocks
        .iter()
        .any(|b| matches!(&b.terminator, ArcTerminator::Resume));
    assert!(has_resume, "expected Resume terminator in unwind block");
}

#[test]
fn invoke_dst_is_valid_variable() {
    let interner = StringInterner::new();
    let func_name = interner.intern("my_function");

    let func = lower_call_expr(&interner, func_name, 42);

    if let Some(block) = func
        .blocks
        .iter()
        .find(|b| matches!(&b.terminator, ArcTerminator::Invoke { .. }))
    {
        if let ArcTerminator::Invoke { dst, normal, .. } = &block.terminator {
            let normal_block = &func.blocks[normal.index()];
            assert!(
                matches!(&normal_block.terminator, ArcTerminator::Return { value } if *value == *dst),
                "expected normal block to return the invoke dst"
            );
        }
    }
}

#[test]
fn lower_method_call_user_defined() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut arena = CanArena::with_capacity(100);

    let method_name = interner.intern("to_str");
    let receiver = arena.push(CanNode::new(
        CanExpr::Int(1),
        Span::new(0, 1),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let args = arena.push_expr_list(&[]);
    let method_call = arena.push(CanNode::new(
        CanExpr::MethodCall {
            receiver,
            method: method_name,
            args,
        },
        Span::new(0, 10),
        TypeId::from_raw(Idx::STR.raw()),
    ));

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: method_call,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let mut problems = Vec::new();
    let (func, _) = super::super::super::lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::STR,
        method_call,
        &canon,
        &interner,
        &pool,
        &mut problems,
    );

    assert!(problems.is_empty());
    let has_invoke = func.blocks.iter().any(|b| {
        matches!(
            &b.terminator,
            ArcTerminator::Invoke { func, .. } if *func == method_name
        )
    });
    assert!(has_invoke, "expected Invoke for user-defined method call");
}
