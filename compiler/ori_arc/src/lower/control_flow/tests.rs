use ori_ir::canon::{CanArena, CanBindingPattern, CanExpr, CanNode, CanonResult};
use ori_ir::{Mutability, Name, Span, StringInterner, TypeId};
use ori_types::Idx;
use ori_types::Pool;

use super::pool_type_store_size;

use crate::ir::ArcTerminator;

#[test]
fn lower_block_with_let() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut arena = CanArena::with_capacity(200);

    // { let x = 1; x + 2 }
    let lit1 = arena.push(CanNode::new(
        CanExpr::Int(1),
        Span::new(10, 11),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let x_name = Name::from_raw(100);
    let pat = arena.push_binding_pattern(CanBindingPattern::Name {
        name: x_name,
        mutable: Mutability::Immutable,
    });

    let let_expr = arena.push(CanNode::new(
        CanExpr::Let {
            pattern: pat,
            init: lit1,
            mutable: Mutability::Immutable,
        },
        Span::new(2, 12),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));

    let x_ref = arena.push(CanNode::new(
        CanExpr::Ident(x_name),
        Span::new(14, 15),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let lit2 = arena.push(CanNode::new(
        CanExpr::Int(2),
        Span::new(18, 19),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let add = arena.push(CanNode::new(
        CanExpr::Binary {
            op: ori_ir::BinaryOp::Add,
            left: x_ref,
            right: lit2,
        },
        Span::new(14, 19),
        TypeId::from_raw(Idx::INT.raw()),
    ));

    let stmts = arena.push_expr_list(&[let_expr]);
    let block = arena.push(CanNode::new(
        CanExpr::Block { stmts, result: add },
        Span::new(0, 20),
        TypeId::from_raw(Idx::INT.raw()),
    ));

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: block,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let mut problems = Vec::new();
    let (func, _) = super::super::super::lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::INT,
        block,
        &canon,
        &interner,
        &pool,
        &mut problems,
        false,
        None,
    );

    assert!(problems.is_empty(), "problems: {problems:?}");
    assert!(func.blocks[0].body.len() >= 3);
    assert_jump_args_match_params(&func);
}

#[test]
fn lower_if_else_produces_four_blocks() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut arena = CanArena::with_capacity(200);

    let cond = arena.push(CanNode::new(
        CanExpr::Bool(true),
        Span::new(3, 7),
        TypeId::from_raw(Idx::BOOL.raw()),
    ));
    let then_val = arena.push(CanNode::new(
        CanExpr::Int(1),
        Span::new(10, 11),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let else_val = arena.push(CanNode::new(
        CanExpr::Int(2),
        Span::new(17, 18),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let if_expr = arena.push(CanNode::new(
        CanExpr::If {
            cond,
            then_branch: then_val,
            else_branch: else_val,
        },
        Span::new(0, 19),
        TypeId::from_raw(Idx::INT.raw()),
    ));

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: if_expr,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let mut problems = Vec::new();
    let (func, _) = super::super::super::lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::INT,
        if_expr,
        &canon,
        &interner,
        &pool,
        &mut problems,
        false,
        None,
    );

    assert!(problems.is_empty());
    assert_eq!(func.blocks.len(), 4);
    assert!(matches!(
        func.blocks[0].terminator,
        ArcTerminator::Branch { .. }
    ));
    assert!(!func.blocks[3].params.is_empty());
    assert_jump_args_match_params(&func);
}

#[test]
fn lower_loop_produces_header_and_exit() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut arena = CanArena::with_capacity(200);

    // loop { break 42 }
    let lit42 = arena.push(CanNode::new(
        CanExpr::Int(42),
        Span::new(14, 16),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let break_expr = arena.push(CanNode::new(
        CanExpr::Break {
            label: Name::EMPTY,
            value: lit42,
        },
        Span::new(8, 16),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));
    let loop_expr = arena.push(CanNode::new(
        CanExpr::Loop {
            label: Name::EMPTY,
            body: break_expr,
        },
        Span::new(0, 18),
        TypeId::from_raw(Idx::INT.raw()),
    ));

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: loop_expr,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let mut problems = Vec::new();
    let (func, _) = super::super::super::lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::INT,
        loop_expr,
        &canon,
        &interner,
        &pool,
        &mut problems,
        false,
        None,
    );

    assert!(problems.is_empty(), "problems: {problems:?}");
    assert!(func.blocks.len() >= 3);
    assert_jump_args_match_params(&func);
}

// -----------------------------------------------------------------------
// SSA well-formedness: jump args match block params
// -----------------------------------------------------------------------

/// Verify that every `Jump` terminator passes exactly as many args as the
/// target block has params. This catches SSA param ordering mismatches
/// between `break`/`continue`/`exit_prep` paths and their target blocks.
fn assert_jump_args_match_params(func: &crate::ir::ArcFunction) {
    for block in &func.blocks {
        if let ArcTerminator::Jump { target, args } = &block.terminator {
            let target_block = &func.blocks[target.index()];
            assert_eq!(
                args.len(),
                target_block.params.len(),
                "Jump from bb{} → bb{}: {} args but {} params",
                block.id.raw(),
                target.raw(),
                args.len(),
                target_block.params.len(),
            );
        }
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "SSA integration test requires building a full AST with 2 mutable vars + for-range loop"
)]
fn for_range_with_mutable_vars_ssa_well_formed() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let mut arena = CanArena::with_capacity(400);

    let x_name = interner.intern("x");
    let y_name = interner.intern("y");

    // let mut x: int = 0
    let lit0 = arena.push(CanNode::new(
        CanExpr::Int(0),
        Span::new(0, 1),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let pat_x = arena.push_binding_pattern(CanBindingPattern::Name {
        name: x_name,
        mutable: Mutability::Mutable,
    });
    let let_x = arena.push(CanNode::new(
        CanExpr::Let {
            pattern: pat_x,
            init: lit0,
            mutable: Mutability::Mutable,
        },
        Span::new(0, 10),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));

    // let mut y: int = 0
    let lit0b = arena.push(CanNode::new(
        CanExpr::Int(0),
        Span::new(12, 13),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let pat_y = arena.push_binding_pattern(CanBindingPattern::Name {
        name: y_name,
        mutable: Mutability::Mutable,
    });
    let let_y = arena.push(CanNode::new(
        CanExpr::Let {
            pattern: pat_y,
            init: lit0b,
            mutable: Mutability::Mutable,
        },
        Span::new(12, 22),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));

    // Range: 0..10
    let start = arena.push(CanNode::new(
        CanExpr::Int(0),
        Span::new(30, 31),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let end = arena.push(CanNode::new(
        CanExpr::Int(10),
        Span::new(33, 35),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let step = arena.push(CanNode::new(
        CanExpr::Int(1),
        Span::new(33, 35),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let range_ty = pool.range(Idx::INT);
    let range = arena.push(CanNode::new(
        CanExpr::Range {
            start,
            end,
            step,
            inclusive: false,
        },
        Span::new(30, 35),
        TypeId::from_raw(range_ty.raw()),
    ));

    // Body: x = x + 1 (simplified as just an int literal for testing)
    let body_lit = arena.push(CanNode::new(
        CanExpr::Int(1),
        Span::new(40, 41),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let assign_target = arena.push(CanNode::new(
        CanExpr::Ident(x_name),
        Span::new(38, 39),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let assign_val = arena.push(CanNode::new(
        CanExpr::Assign {
            target: assign_target,
            value: body_lit,
        },
        Span::new(38, 41),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));

    let pat_i = arena.push_binding_pattern(CanBindingPattern::Name {
        name: interner.intern("i"),
        mutable: Mutability::Immutable,
    });
    let for_expr = arena.push(CanNode::new(
        CanExpr::For {
            label: Name::EMPTY,
            pattern: pat_i,
            iter: range,
            guard: ori_ir::canon::CanId::INVALID,
            body: assign_val,
            is_yield: false,
        },
        Span::new(25, 42),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));

    // Block: { let mut x = 0; let mut y = 0; for i in 0..10 do x = 1 }
    let stmts = arena.push_expr_list(&[let_x, let_y]);
    let block = arena.push(CanNode::new(
        CanExpr::Block {
            stmts,
            result: for_expr,
        },
        Span::new(0, 50),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: block,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let mut problems = Vec::new();
    let (func, _) = super::super::super::lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::UNIT,
        block,
        &canon,
        &interner,
        &pool,
        &mut problems,
        false,
        None,
    );

    assert!(problems.is_empty(), "problems: {problems:?}");
    // The for-range loop should produce header, body, latch, exit, exit_prep blocks.
    assert!(
        func.blocks.len() >= 6,
        "expected at least 6 blocks for for-range with mutable vars, got {}",
        func.blocks.len()
    );

    // Core invariant: every Jump's args must match the target block's params.
    assert_jump_args_match_params(&func);
}

// -----------------------------------------------------------------------
// pool_type_store_size — cross-phase size agreement
// -----------------------------------------------------------------------
//
// These values must match `TypeLayoutResolver::type_store_size()` in ori_llvm.
// If a new type is added and these constants differ, for-yield element
// buffers will be mis-sized, causing memory corruption.

#[test]
fn type_store_size_primitives() {
    let pool = Pool::new();
    // Scalar primitives — must match LLVM int/float type widths
    assert_eq!(pool_type_store_size(Idx::INT, &pool, 0), 8, "int = i64");
    assert_eq!(pool_type_store_size(Idx::FLOAT, &pool, 0), 8, "float = f64");
    assert_eq!(pool_type_store_size(Idx::BOOL, &pool, 0), 1, "bool = i1");
    assert_eq!(pool_type_store_size(Idx::CHAR, &pool, 0), 4, "char = i32");
    assert_eq!(pool_type_store_size(Idx::BYTE, &pool, 0), 1, "byte = i8");
    assert_eq!(pool_type_store_size(Idx::UNIT, &pool, 0), 0, "unit = void");
    assert_eq!(
        pool_type_store_size(Idx::STR, &pool, 0),
        16,
        "str = {{i64, ptr}}"
    );
    assert_eq!(
        pool_type_store_size(Idx::DURATION, &pool, 0),
        8,
        "Duration = i64"
    );
    assert_eq!(pool_type_store_size(Idx::SIZE, &pool, 0), 8, "Size = i64");
}

#[test]
fn type_store_size_containers() {
    let mut pool = Pool::new();

    // List<int> = {i64 len, i64 cap, ptr data} = 24
    let list_int = pool.list(Idx::INT);
    assert_eq!(pool_type_store_size(list_int, &pool, 0), 24, "list");

    // Option<int> = {i64 tag, int payload} = 8 + 8 = 16
    let opt_int = pool.option(Idx::INT);
    assert_eq!(pool_type_store_size(opt_int, &pool, 0), 16, "Option<int>");

    // Option<bool> = {i64 tag, bool payload} = 8 + 1 = 9
    let opt_bool = pool.option(Idx::BOOL);
    assert_eq!(pool_type_store_size(opt_bool, &pool, 0), 9, "Option<bool>");

    // Result<int, str> = {i64 tag, max(8, 16)} = 8 + 16 = 24
    let res = pool.result(Idx::INT, Idx::STR);
    assert_eq!(pool_type_store_size(res, &pool, 0), 24, "Result<int, str>");

    // Tuple (int, bool) = 8 + 1 = 9
    let tup = pool.tuple(&[Idx::INT, Idx::BOOL]);
    assert_eq!(pool_type_store_size(tup, &pool, 0), 9, "(int, bool)");

    // Tuple (int, int, int) = 24
    let tup3 = pool.tuple(&[Idx::INT, Idx::INT, Idx::INT]);
    assert_eq!(pool_type_store_size(tup3, &pool, 0), 24, "(int, int, int)");
}
