use ori_ir::canon::{CanArena, CanBindingPattern, CanExpr, CanNode, CanonResult};
use ori_ir::{Mutability, Name, Span, StringInterner, TypeId};
use ori_types::Idx;
use ori_types::Pool;

use super::pool_type_store_size;

use crate::ir::ArcTerminator;
use crate::lower::ArcProblem;

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

// SSA well-formedness: jump args match block params

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

#[test]
fn lower_index_assignment_reports_internal_error_instead_of_panicking() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let mut arena = CanArena::with_capacity(64);

    let list_int = pool.list(Idx::INT);
    let receiver = arena.push(CanNode::new(
        CanExpr::Ident(Name::from_raw(100)),
        Span::new(0, 1),
        TypeId::from_raw(list_int.raw()),
    ));
    let index = arena.push(CanNode::new(
        CanExpr::Int(0),
        Span::new(2, 3),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let target = arena.push(CanNode::new(
        CanExpr::Index { receiver, index },
        Span::new(0, 3),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let value = arena.push(CanNode::new(
        CanExpr::Int(42),
        Span::new(6, 8),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let assign = arena.push(CanNode::new(
        CanExpr::Assign { target, value },
        Span::new(0, 8),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: assign,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let mut problems = Vec::new();
    let _ = super::super::super::lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::UNIT,
        assign,
        &canon,
        &interner,
        &pool,
        &mut problems,
        false,
        None,
    );

    assert!(
        problems.iter().any(|p| matches!(
            p,
            ArcProblem::InternalError {
                message,
                ..
            } if message.contains("index assignment")
        )),
        "expected internal error for index assignment, got: {problems:?}"
    );
}

#[test]
fn lower_field_assignment_reports_internal_error_instead_of_panicking() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut arena = CanArena::with_capacity(64);

    let receiver = arena.push(CanNode::new(
        CanExpr::Ident(Name::from_raw(101)),
        Span::new(0, 1),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let target = arena.push(CanNode::new(
        CanExpr::Field {
            receiver,
            field: Name::from_raw(102),
        },
        Span::new(0, 3),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let value = arena.push(CanNode::new(
        CanExpr::Int(7),
        Span::new(6, 7),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let assign = arena.push(CanNode::new(
        CanExpr::Assign { target, value },
        Span::new(0, 7),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: assign,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let mut problems = Vec::new();
    let _ = super::super::super::lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::UNIT,
        assign,
        &canon,
        &interner,
        &pool,
        &mut problems,
        false,
        None,
    );

    assert!(
        problems.iter().any(|p| matches!(
            p,
            ArcProblem::InternalError {
                message,
                ..
            } if message.contains("field assignment")
        )),
        "expected internal error for field assignment, got: {problems:?}"
    );
}

// pool_type_store_size — cross-phase size agreement
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
        24,
        "str = {{i64, i64, ptr}} (SSO)"
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

    // Option<bool> = {i64 tag, bool payload} = round_up(8 + 1, 8) = 16
    let opt_bool = pool.option(Idx::BOOL);
    assert_eq!(pool_type_store_size(opt_bool, &pool, 0), 16, "Option<bool>");

    // Result<int, str> = {i64 tag, max(8, 24)} = 8 + 24 = 32
    let res = pool.result(Idx::INT, Idx::STR);
    assert_eq!(pool_type_store_size(res, &pool, 0), 32, "Result<int, str>");

    // Tuple (int, bool) = round_up(8 + 1, 8) = 16 (padded to max align)
    let tup = pool.tuple(&[Idx::INT, Idx::BOOL]);
    assert_eq!(pool_type_store_size(tup, &pool, 0), 16, "(int, bool)");

    // Tuple (int, int, int) = 24
    let tup3 = pool.tuple(&[Idx::INT, Idx::INT, Idx::INT]);
    assert_eq!(pool_type_store_size(tup3, &pool, 0), 24, "(int, int, int)");

    // Function = {fn_ptr: ptr, env_ptr: ptr} = 16 (fat pointer closure)
    let func = pool.function(&[Idx::INT], Idx::INT);
    assert_eq!(
        pool_type_store_size(func, &pool, 0),
        16,
        "Function = {{ptr, ptr}}"
    );

    // Ordering = i8 = 1 byte
    assert_eq!(
        pool_type_store_size(Idx::ORDERING, &pool, 0),
        1,
        "Ordering = i8"
    );

    // Never = uninhabited = 0 bytes
    assert_eq!(
        pool_type_store_size(Idx::NEVER, &pool, 0),
        0,
        "Never = void"
    );
}

#[test]
fn type_store_size_extended_types() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();

    // Function = {fn_ptr: ptr, env_ptr: ptr} = 16 (fat pointer closure)
    let func = pool.function(&[Idx::INT], Idx::INT);
    assert_eq!(
        pool_type_store_size(func, &pool, 0),
        16,
        "Function = {{ptr, ptr}}"
    );

    // Ordering = i8 = 1 byte
    assert_eq!(
        pool_type_store_size(Idx::ORDERING, &pool, 0),
        1,
        "Ordering = i8"
    );

    // Never = uninhabited = 0 bytes
    assert_eq!(
        pool_type_store_size(Idx::NEVER, &pool, 0),
        0,
        "Never = void"
    );

    // Payload enum: A | B(x: int) → {i8 tag (padded to 8), [1 x i64]} = 8 + 8 = 16
    let variant_a_name = interner.intern("A");
    let variant_b_name = interner.intern("B");
    let enum_name = interner.intern("SimpleEnum");
    let simple_enum = pool.enum_type(
        enum_name,
        &[
            ori_types::EnumVariant {
                name: variant_a_name,
                field_types: vec![],
            },
            ori_types::EnumVariant {
                name: variant_b_name,
                field_types: vec![Idx::INT],
            },
        ],
    );
    assert_eq!(
        pool_type_store_size(simple_enum, &pool, 0),
        16,
        "A | B(int) = {{i8 tag padded to 8, [1 x i64]}} = 16"
    );

    // Payload enum: A(s: str) | B → {i8 tag (padded to 8), max(24, 0)} = 8 + 24 = 32
    let enum2_name = interner.intern("StrEnum");
    let str_enum = pool.enum_type(
        enum2_name,
        &[
            ori_types::EnumVariant {
                name: variant_a_name,
                field_types: vec![Idx::STR],
            },
            ori_types::EnumVariant {
                name: variant_b_name,
                field_types: vec![],
            },
        ],
    );
    assert_eq!(
        pool_type_store_size(str_enum, &pool, 0),
        32,
        "A(str) | B = {{i8 tag padded to 8, max(24, 0)}} = 32"
    );
}

/// Regression: Option/Result trailing alignment padding.
/// `Option<T>` is `{i64 tag, T payload}` in LLVM. Store size must include
/// trailing alignment padding to match LLVM's `size_of()`. Without it,
/// outer aggregates containing Option/Result fields are undersized.
#[test]
fn type_store_size_option_result_trailing_padding() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();

    // Option<bool> = {i64 tag, i1 payload} → LLVM pads to 16 bytes (alignment 8)
    let opt_bool = pool.option(Idx::BOOL);
    assert_eq!(
        pool_type_store_size(opt_bool, &pool, 0),
        16,
        "Option<bool> must include trailing padding to alignment 8"
    );

    // Option<int> = {i64 tag, i64 payload} → 16, already aligned
    let opt_int = pool.option(Idx::INT);
    assert_eq!(
        pool_type_store_size(opt_int, &pool, 0),
        16,
        "Option<int> is 16 (no trailing padding needed)"
    );

    // Option<char> = {i64 tag, i32 payload} → round_up(12, 8) = 16
    let opt_char = pool.option(Idx::CHAR);
    assert_eq!(
        pool_type_store_size(opt_char, &pool, 0),
        16,
        "Option<char> must pad to 16"
    );

    // Result<bool, bool> = {i64 tag, max(1, 1) payload} → round_up(9, 8) = 16
    let res_bb = pool.result(Idx::BOOL, Idx::BOOL);
    assert_eq!(
        pool_type_store_size(res_bb, &pool, 0),
        16,
        "Result<bool, bool> must pad to 16"
    );

    // Result<int, str> = {i64 tag, max(8, 24) = 24} → round_up(32, 8) = 32 (already aligned)
    let res_is = pool.result(Idx::INT, Idx::STR);
    assert_eq!(
        pool_type_store_size(res_is, &pool, 0),
        32,
        "Result<int, str> is 32 (no trailing padding needed)"
    );

    // Nested: (Option<bool>, bool) — Option<bool>=16, then bool at 16 → 17, pad to 24
    let tup_opt_bool = pool.tuple(&[opt_bool, Idx::BOOL]);
    assert_eq!(
        pool_type_store_size(tup_opt_bool, &pool, 0),
        24,
        "(Option<bool>, bool) must use padded Option size"
    );

    // Struct { left: Option<bool>, right: bool } — same as tuple: 24
    let left_name = interner.intern("left");
    let right_name = interner.intern("right");
    let s_name = interner.intern("OptStruct");
    let opt_struct = pool.struct_type(s_name, &[(left_name, opt_bool), (right_name, Idx::BOOL)]);
    assert_eq!(
        pool_type_store_size(opt_struct, &pool, 0),
        24,
        "Struct {{Option<bool>, bool}} must use padded Option size"
    );

    // Option<Option<bool>> — inner=16, outer=round_up(8+16, 8)=24
    let opt_opt_bool = pool.option(opt_bool);
    assert_eq!(
        pool_type_store_size(opt_opt_bool, &pool, 0),
        24,
        "Option<Option<bool>> must compose correctly"
    );
}

/// Regression: inter-field alignment padding was missing.
/// `pool_type_store_size()` summed field sizes without aligning each field,
/// undercounting aggregates with mixed-alignment fields.
#[test]
fn type_store_size_inter_field_padding() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();

    // (bool, str, int, bool):
    //   bool at 0 (1B), pad to 8, str at 8 (24B), int at 32 (8B), bool at 40 (1B)
    //   → offset 41, round_up(41, 8) = 48
    let tup_mixed = pool.tuple(&[Idx::BOOL, Idx::STR, Idx::INT, Idx::BOOL]);
    assert_eq!(
        pool_type_store_size(tup_mixed, &pool, 0),
        48,
        "(bool, str, int, bool) needs inter-field padding"
    );

    // (bool, int) — bool→pad to 8→int: offset 16, round_up(16, 8) = 16
    let tup_bi = pool.tuple(&[Idx::BOOL, Idx::INT]);
    assert_eq!(
        pool_type_store_size(tup_bi, &pool, 0),
        16,
        "(bool, int) needs padding between bool and int"
    );

    // (char, int) — char at 0 (4B), pad to 8, int at 8: round_up(16, 8) = 16
    let tup_ci = pool.tuple(&[Idx::CHAR, Idx::INT]);
    assert_eq!(
        pool_type_store_size(tup_ci, &pool, 0),
        16,
        "(char, int) needs padding between char and int"
    );

    // (bool, int, bool, str):
    //   bool at 0 (1B), pad to 8, int at 8 (8B), bool at 16 (1B), pad to 8, str at 24 (24B)
    //   → offset 48, round_up(48, 8) = 48
    let tup_bibs = pool.tuple(&[Idx::BOOL, Idx::INT, Idx::BOOL, Idx::STR]);
    assert_eq!(
        pool_type_store_size(tup_bibs, &pool, 0),
        48,
        "(bool, int, bool, str) needs two inter-field pads"
    );

    // Struct { a: bool, b: str } — same padding as tuple:
    //   bool at 0, pad to 8, str at 8 (24B) → round_up(32, 8) = 32
    let a_name = interner.intern("a");
    let b_name = interner.intern("b");
    let c_name = interner.intern("c");
    let d_name = interner.intern("d");
    let s_name = interner.intern("PaddedStruct");
    let padded_struct = pool.struct_type(s_name, &[(a_name, Idx::BOOL), (b_name, Idx::STR)]);
    assert_eq!(
        pool_type_store_size(padded_struct, &pool, 0),
        32,
        "Struct {{bool, str}} needs padding"
    );

    // Struct { a: bool, b: str, c: int, d: bool } — same as tuple case: 48
    let s2_name = interner.intern("BigPaddedStruct");
    let big_struct = pool.struct_type(
        s2_name,
        &[
            (a_name, Idx::BOOL),
            (b_name, Idx::STR),
            (c_name, Idx::INT),
            (d_name, Idx::BOOL),
        ],
    );
    assert_eq!(
        pool_type_store_size(big_struct, &pool, 0),
        48,
        "Struct {{bool, str, int, bool}} needs inter-field padding"
    );
}

/// Regression: + enum payload i64-slot sizing.
/// Enum payloads use `[M x i64]` layout where each field occupies at
/// least one full i64 slot (8 bytes), regardless of natural alignment.
#[test]
fn type_store_size_enum_payload_slots() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();

    // A(bool, str) | B → payload = 8+24 = 32, total = 40
    let padded_enum = pool.enum_type(
        interner.intern("PaddedEnum"),
        &[
            ori_types::EnumVariant {
                name: interner.intern("A"),
                field_types: vec![Idx::BOOL, Idx::STR],
            },
            ori_types::EnumVariant {
                name: interner.intern("B"),
                field_types: vec![],
            },
        ],
    );
    assert_eq!(
        pool_type_store_size(padded_enum, &pool, 0),
        40,
        "Enum A(bool, str) | B: bool slot(8) + str(24) = 32 payload"
    );

    // A(bool, bool, int) | B → payload = 8+8+8 = 24 (3 slots), total = 32
    let slot_enum = pool.enum_type(
        interner.intern("SlotEnum"),
        &[
            ori_types::EnumVariant {
                name: interner.intern("A"),
                field_types: vec![Idx::BOOL, Idx::BOOL, Idx::INT],
            },
            ori_types::EnumVariant {
                name: interner.intern("B"),
                field_types: vec![],
            },
        ],
    );
    assert_eq!(
        pool_type_store_size(slot_enum, &pool, 0),
        32,
        "Enum A(bool, bool, int) | B: each bool takes a full i64 slot"
    );

    // A(bool, int, bool, str) | B → payload = 8+8+8+24 = 48 (6 slots), total = 56
    let big_enum = pool.enum_type(
        interner.intern("BigSlotEnum"),
        &[
            ori_types::EnumVariant {
                name: interner.intern("A"),
                field_types: vec![Idx::BOOL, Idx::INT, Idx::BOOL, Idx::STR],
            },
            ori_types::EnumVariant {
                name: interner.intern("B"),
                field_types: vec![],
            },
        ],
    );
    assert_eq!(
        pool_type_store_size(big_enum, &pool, 0),
        56,
        "Enum A(bool, int, bool, str) | B: 6-slot payload = 48 + tag 8 = 56"
    );
}

/// Regression: nested aggregates with sub-8-byte alignment.
/// `pool_type_alignment()` must recurse into struct/tuple fields to compute
/// max field alignment, matching `type_alignment()` in `ori_llvm`. Without
/// recursion, all struct/tuple types default to alignment 8, which over-sizes
/// aggregates like `((char, char), bool)` (12 bytes, not 16).
#[test]
fn type_store_size_nested_low_alignment() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();

    // (char, char) — alignment 4, size 8
    let tup_cc = pool.tuple(&[Idx::CHAR, Idx::CHAR]);
    assert_eq!(
        pool_type_store_size(tup_cc, &pool, 0),
        8,
        "(char, char) = 8"
    );

    // ((char, char), bool) — alignment should be 4 (max of inner fields)
    // offset 0: (char,char)=8, offset 8: bool=1 → 9, round_up(9, 4) = 12
    let tup_ccb = pool.tuple(&[tup_cc, Idx::BOOL]);
    assert_eq!(
        pool_type_store_size(tup_ccb, &pool, 0),
        12,
        "((char, char), bool) should be 12 (alignment 4), not 16"
    );

    // (bool, bool) — alignment 1, size 2
    let tup_bb = pool.tuple(&[Idx::BOOL, Idx::BOOL]);
    assert_eq!(
        pool_type_store_size(tup_bb, &pool, 0),
        2,
        "(bool, bool) = 2"
    );

    // ((bool, bool), char) — alignment should be 4 (from char)
    // offset 0: (bool,bool)=2, pad to 4 (char alignment), offset 4: char=4 → 8, round_up(8, 4) = 8
    let tup_bbc = pool.tuple(&[tup_bb, Idx::CHAR]);
    assert_eq!(
        pool_type_store_size(tup_bbc, &pool, 0),
        8,
        "((bool, bool), char) should be 8"
    );

    // Struct { inner: (char, char), flag: bool } — same as nested tuple: 12
    let inner_name = interner.intern("inner");
    let flag_name = interner.intern("flag");
    let s_name = interner.intern("NestedCharStruct");
    let nested_struct = pool.struct_type(s_name, &[(inner_name, tup_cc), (flag_name, Idx::BOOL)]);
    assert_eq!(
        pool_type_store_size(nested_struct, &pool, 0),
        12,
        "Struct {{ inner: (char, char), flag: bool }} should be 12"
    );

    // Mixed: ((char, char), int) — alignment 8 (from int), so unchanged from before
    let tup_cci = pool.tuple(&[tup_cc, Idx::INT]);
    assert_eq!(
        pool_type_store_size(tup_cci, &pool, 0),
        16,
        "((char, char), int) is 16 — int dominates alignment"
    );
}

/// Regression: all-unit enums use narrowed i8 tags after §07.1.
///
/// `pool_type_store_size()` was hardcoding 8-byte (i64) enum tags for all enums,
/// but §07.1 narrowed all-unit enums to i8 (1 byte). This caused `for...yield`
/// over all-unit enums to allocate 8 bytes per element in `ori_list_new` /
/// `ori_list_push`, but LLVM lowered them as 1-byte `{ i8 }` structs — resulting
/// in out-of-bounds reads/writes and segfaults.
#[test]
fn type_store_size_all_unit_enum_narrowed_tag() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();

    // North | South | East | West — 4 variants, all unit → i8 tag, no payload.
    // LLVM: %ori.Dir = type { i8 } → size 1
    let dir_enum = pool.enum_type(
        interner.intern("Dir"),
        &[
            ori_types::EnumVariant {
                name: interner.intern("North"),
                field_types: vec![],
            },
            ori_types::EnumVariant {
                name: interner.intern("South"),
                field_types: vec![],
            },
            ori_types::EnumVariant {
                name: interner.intern("East"),
                field_types: vec![],
            },
            ori_types::EnumVariant {
                name: interner.intern("West"),
                field_types: vec![],
            },
        ],
    );
    assert_eq!(
        pool_type_store_size(dir_enum, &pool, 0),
        1,
        "All-unit enum (4 variants) = {{i8}} = 1 byte after §07.1 narrowing"
    );

    // Semantic pin: 2-variant all-unit enum → still i8 tag → 1 byte
    let bool_enum = pool.enum_type(
        interner.intern("MyBool"),
        &[
            ori_types::EnumVariant {
                name: interner.intern("True"),
                field_types: vec![],
            },
            ori_types::EnumVariant {
                name: interner.intern("False"),
                field_types: vec![],
            },
        ],
    );
    assert_eq!(
        pool_type_store_size(bool_enum, &pool, 0),
        1,
        "All-unit enum (2 variants) = {{i8}} = 1 byte"
    );

    // Single-variant enum (newtype-like, no payload) → i8 tag → 1 byte
    let single = pool.enum_type(
        interner.intern("Single"),
        &[ori_types::EnumVariant {
            name: interner.intern("Only"),
            field_types: vec![],
        }],
    );
    assert_eq!(
        pool_type_store_size(single, &pool, 0),
        1,
        "Single-variant unit enum = {{i8}} = 1 byte"
    );

    // Payload enum: A | B(int) → i8 tag + [1 x i64] payload.
    // LLVM: { i8, [1 x i64] } → size = 8 (tag padded) + 8 (payload) = 16
    // Same as before — payload alignment dominates.
    let payload_enum = pool.enum_type(
        interner.intern("PayloadEnum"),
        &[
            ori_types::EnumVariant {
                name: interner.intern("A"),
                field_types: vec![],
            },
            ori_types::EnumVariant {
                name: interner.intern("B"),
                field_types: vec![Idx::INT],
            },
        ],
    );
    assert_eq!(
        pool_type_store_size(payload_enum, &pool, 0),
        16,
        "Payload enum A | B(int) = {{i8, [1 x i64]}} = 16 (tag padded to 8)"
    );
}

/// Regression: all-unit enum as field in struct/tuple.
///
/// `pool_type_alignment_inner()` returned 8 for all enums, but all-unit enums
/// with narrowed tags have alignment 1. This over-sizes containing aggregates.
#[test]
fn type_store_size_all_unit_enum_in_aggregate() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();

    // All-unit enum: { i8 } → size 1, alignment 1
    let dir_enum = pool.enum_type(
        interner.intern("Dir2"),
        &[
            ori_types::EnumVariant {
                name: interner.intern("N"),
                field_types: vec![],
            },
            ori_types::EnumVariant {
                name: interner.intern("S"),
                field_types: vec![],
            },
        ],
    );

    // (Dir, Dir) — both alignment 1, so size = 1 + 1 = 2
    let tup_dd = pool.tuple(&[dir_enum, dir_enum]);
    assert_eq!(
        pool_type_store_size(tup_dd, &pool, 0),
        2,
        "(Dir, Dir) = 2 bytes (alignment 1 for all-unit enums)"
    );

    // (Dir, char) — Dir=1 align=1, pad to 4 (char align), char=4 → total 8
    let tup_dc = pool.tuple(&[dir_enum, Idx::CHAR]);
    assert_eq!(
        pool_type_store_size(tup_dc, &pool, 0),
        8,
        "(Dir, char) = 8 bytes"
    );

    // (Dir, int) — Dir=1 align=1, pad to 8 (int align), int=8 → total 16
    let tup_di = pool.tuple(&[dir_enum, Idx::INT]);
    assert_eq!(
        pool_type_store_size(tup_di, &pool, 0),
        16,
        "(Dir, int) = 16 bytes"
    );
}
