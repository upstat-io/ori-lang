use super::*;
use crate::lower::lower;
use ori_ir::ast::{CallArg, Expr, TemplatePart};
use ori_ir::{ExprArena, ExprId, ExprKind, SharedInterner, Span};
use ori_types::{Idx, TypeCheckResult, TypedModule};

fn test_type_result(expr_types: Vec<Idx>) -> TypeCheckResult {
    let mut typed = TypedModule::new();
    for idx in expr_types {
        typed.expr_types.push(idx);
    }
    TypeCheckResult::ok(typed)
}

fn test_interner() -> SharedInterner {
    SharedInterner::new()
}

#[test]
fn desugar_call_named_source_order_fallback() {
    // When no function signature is available, args stay in source order.
    let mut arena = ExprArena::new();
    let interner = test_interner();

    let func_name = interner.intern("unknown_fn");
    let arg_a_name = interner.intern("a");
    let arg_b_name = interner.intern("b");

    let func = arena.alloc_expr(Expr::new(ExprKind::Ident(func_name), Span::new(0, 3)));
    let val1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(4, 5)));
    let val2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(8, 9)));

    let args = arena.alloc_call_args([
        CallArg {
            name: Some(arg_b_name),
            value: val2,
            is_spread: false,
            span: Span::new(7, 10),
        },
        CallArg {
            name: Some(arg_a_name),
            value: val1,
            is_spread: false,
            span: Span::new(4, 6),
        },
    ]);

    let root = arena.alloc_expr(Expr::new(
        ExprKind::CallNamed { func, args },
        Span::new(0, 11),
    ));

    // No function sig → source order (b=2, a=1).
    let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT, Idx::INT]);

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Call { args, .. } => {
            let arg_list = result.arena.get_expr_list(*args);
            assert_eq!(arg_list.len(), 2);
            // Source order preserved: b=2 first, a=1 second.
            assert_eq!(*result.arena.kind(arg_list[0]), CanExpr::Int(2));
            assert_eq!(*result.arena.kind(arg_list[1]), CanExpr::Int(1));
        }
        other => panic!("expected Call, got {other:?}"),
    }
}

#[test]
fn desugar_template_literal_simple() {
    // `hello {name}!` → "hello".concat(name.to_str()).concat("!")
    let mut arena = ExprArena::new();
    let interner = test_interner();

    let head = interner.intern("hello ");
    let tail = interner.intern("!");
    let var_name = interner.intern("name");

    let expr = arena.alloc_expr(Expr::new(ExprKind::Ident(var_name), Span::new(8, 12)));

    let parts = arena.alloc_template_parts([TemplatePart {
        expr,
        format_spec: Name::EMPTY,
        text_after: tail,
    }]);

    let root = arena.alloc_expr(Expr::new(
        ExprKind::TemplateLiteral { head, parts },
        Span::new(0, 14),
    ));

    // expr_types: [0]=Ident(name):str, [1]=TemplateLiteral:str
    let type_result = test_type_result(vec![Idx::STR, Idx::STR]);

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);
    assert!(result.root.is_valid());

    // The root should be a concat chain. Since `name` is already str,
    // no to_str wrapping needed. Result is:
    // "hello ".concat(name).concat("!")
    // The final concat("!") is the root.
    match result.arena.kind(result.root) {
        CanExpr::MethodCall { method, .. } => {
            let concat = interner.intern("concat");
            assert_eq!(*method, concat);
        }
        other => panic!("expected MethodCall(concat), got {other:?}"),
    }
}

// Non-primitive `{expr:spec}` FormatWith desugar (BUG-04-193)

/// Lower `` `{x:spec}` `` (single interpolation, with a format spec) and return
/// the lowered result.
fn lower_single_format_with(
    arena: &mut ExprArena,
    interner: &SharedInterner,
    type_result: &TypeCheckResult,
) -> ori_ir::canon::CanonResult {
    let x_name = interner.intern("x");
    let spec = interner.intern(">10");
    let x = arena.alloc_expr(Expr::new(ExprKind::Ident(x_name), Span::new(1, 2)));
    let parts = arena.alloc_template_parts([TemplatePart {
        expr: x,
        format_spec: spec,
        text_after: Name::EMPTY,
    }]);
    let root = arena.alloc_expr(Expr::new(
        ExprKind::TemplateLiteral {
            head: Name::EMPTY,
            parts,
        },
        Span::new(0, 8),
    ));
    let pool = ori_types::Pool::new();
    lower(arena, type_result, &pool, root, interner)
}

/// The format-result node is the single arg of the root `concat` `MethodCall`
/// (`"".concat(<format-result>)`).
fn format_result_node(
    result: &ori_ir::canon::CanonResult,
    interner: &SharedInterner,
) -> ori_ir::canon::CanId {
    match result.arena.kind(result.root) {
        CanExpr::MethodCall { method, args, .. } => {
            assert_eq!(*method, interner.intern("concat"), "root is concat chain");
            let arg_ids = result.arena.get_expr_list(*args);
            assert_eq!(arg_ids.len(), 1);
            arg_ids[0]
        }
        other => panic!("expected concat MethodCall root, got {other:?}"),
    }
}

#[test]
fn desugar_primitive_format_with_stays_bare_formatwith() {
    // `{x:spec}` where x: int → `FormatWith { x, spec }` (the ori_format_* fast path).
    let mut arena = ExprArena::new();
    let interner = test_interner();
    // expr_types: [0]=Ident(x):int, [1]=TemplateLiteral:str
    let type_result = test_type_result(vec![Idx::INT, Idx::STR]);
    let result = lower_single_format_with(&mut arena, &interner, &type_result);

    let fmt_node = format_result_node(&result, &interner);
    match result.arena.kind(fmt_node) {
        CanExpr::FormatWith { expr, .. } => {
            // The wrapped expr is the bare Ident(x), NOT a to_str MethodCall.
            assert!(
                matches!(result.arena.kind(*expr), CanExpr::Ident(_)),
                "primitive FormatWith wraps the value directly"
            );
        }
        other => panic!("expected bare FormatWith for primitive, got {other:?}"),
    }
}

#[test]
fn desugar_printable_only_format_with_routes_through_to_str() {
    // `{x:spec}` where x is a non-primitive WITHOUT an explicit Formattable impl
    // → `FormatWith { <x.to_str()>, spec }`.
    let mut arena = ExprArena::new();
    let interner = test_interner();
    // Use a non-primitive idx (>= FIRST_DYNAMIC) so it is not treated as primitive.
    let non_primitive = Idx::from_raw(64);
    // expr_types: [0]=Ident(x):<non-primitive>, [1]=TemplateLiteral:str
    let type_result = test_type_result(vec![non_primitive, Idx::STR]);
    let result = lower_single_format_with(&mut arena, &interner, &type_result);

    let fmt_node = format_result_node(&result, &interner);
    match result.arena.kind(fmt_node) {
        CanExpr::FormatWith { expr, .. } => {
            // The wrapped expr is `x.to_str()`, NOT the bare value.
            match result.arena.kind(*expr) {
                CanExpr::MethodCall { method, .. } => {
                    assert_eq!(*method, interner.intern("to_str"));
                }
                other => panic!("expected to_str MethodCall under FormatWith, got {other:?}"),
            }
        }
        other => panic!("expected FormatWith for Printable-only, got {other:?}"),
    }
}

#[test]
fn desugar_explicit_formattable_format_with_emits_format_call() {
    // `{x:spec}` where x has an explicit `impl T: Formattable` →
    // `x.format(<FormatSpec struct>)` MethodCall.
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let non_primitive = Idx::from_raw(64);
    let mut typed = TypedModule::new();
    typed.expr_types.push(non_primitive); // [0]=Ident(x)
    typed.expr_types.push(Idx::STR); // [1]=TemplateLiteral
    typed.formattable_impl_types.push(non_primitive);
    let type_result = TypeCheckResult::ok(typed);
    let result = lower_single_format_with(&mut arena, &interner, &type_result);

    let fmt_node = format_result_node(&result, &interner);
    match result.arena.kind(fmt_node) {
        CanExpr::MethodCall { method, args, .. } => {
            assert_eq!(*method, interner.intern("format"));
            // Exactly one arg: the synthesized FormatSpec struct.
            let arg_ids = result.arena.get_expr_list(*args);
            assert_eq!(arg_ids.len(), 1);
            match result.arena.kind(arg_ids[0]) {
                CanExpr::Struct { name, .. } => {
                    assert_eq!(*name, interner.intern("FormatSpec"));
                }
                other => panic!("expected FormatSpec struct arg, got {other:?}"),
            }
        }
        other => panic!("expected format() MethodCall for explicit Formattable, got {other:?}"),
    }
}

#[test]
fn desugar_list_with_spread_simple() {
    // [1, ...xs, 2] → [1].concat(xs).concat([2])
    let mut arena = ExprArena::new();
    let interner = test_interner();

    let xs_name = interner.intern("xs");

    let e1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(1, 2)));
    let xs = arena.alloc_expr(Expr::new(ExprKind::Ident(xs_name), Span::new(7, 9)));
    let e2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(11, 12)));

    let elements = arena.alloc_list_elements([
        ori_ir::ListElement::Expr {
            expr: e1,
            span: Span::new(1, 2),
        },
        ori_ir::ListElement::Spread {
            expr: xs,
            span: Span::new(4, 9),
        },
        ori_ir::ListElement::Expr {
            expr: e2,
            span: Span::new(11, 12),
        },
    ]);

    let root = arena.alloc_expr(Expr::new(
        ExprKind::ListWithSpread(elements),
        Span::new(0, 13),
    ));

    let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT, Idx::INT]);

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);
    assert!(result.root.is_valid());

    // Root should be: [1].concat(xs).concat([2])
    // That's a chain of MethodCall(concat).
    match result.arena.kind(result.root) {
        CanExpr::MethodCall { method, .. } => {
            let concat = interner.intern("concat");
            assert_eq!(*method, concat);
        }
        other => panic!("expected MethodCall(concat), got {other:?}"),
    }
}

// Index/Field-Assignment Desugar

use ori_ir::AccessStep;
use ori_types::AssignDesugar;

/// Build a `TypeCheckResult` with `expr_types` sized to `n` (all `Idx::INT`)
/// and one `assign_desugar_map` entry keyed by `target` with the given
/// per-level types.
fn assign_type_result(n: usize, target: ExprId, level_types: Vec<Idx>) -> TypeCheckResult {
    let mut typed = TypedModule::new();
    for _ in 0..n {
        typed.expr_types.push(Idx::INT);
    }
    typed
        .assign_desugar_map
        .push((target, AssignDesugar { level_types }));
    typed.assign_desugar_map.sort_by_key(|(eid, _)| eid.raw());
    TypeCheckResult::ok(typed)
}

#[test]
fn desugar_index_assign_rewrites_to_updated_reassignment() {
    // `xs[0] = 9` -> `xs = xs.updated(key: 0, value: 9)`.
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let xs = interner.intern("xs");

    let root = arena.alloc_expr(Expr::new(ExprKind::Ident(xs), Span::new(0, 2)));
    let idx = arena.alloc_expr(Expr::new(ExprKind::Int(0), Span::new(3, 4)));
    let value = arena.alloc_expr(Expr::new(ExprKind::Int(9), Span::new(8, 9)));
    let steps = arena.alloc_access_steps([AccessStep::Index(idx)]);
    let target = arena.alloc_expr(Expr::new(
        ExprKind::AssignTarget { root, steps },
        Span::new(0, 4),
    ));
    let assign = arena.alloc_expr(Expr::new(
        ExprKind::Assign { target, value },
        Span::new(0, 9),
    ));

    let type_result = assign_type_result(assign.index() + 1, target, vec![Idx::INT, Idx::INT]);
    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, assign, &interner);
    assert!(result.root.is_valid());

    // Root: Assign { target: Ident(xs), value: MethodCall(updated, [key, value]) }.
    match result.arena.kind(result.root) {
        CanExpr::Assign { target, value } => {
            match result.arena.kind(*target) {
                CanExpr::Ident(name) => assert_eq!(*name, xs),
                other => panic!("expected reassignment target Ident(xs), got {other:?}"),
            }
            match result.arena.kind(*value) {
                CanExpr::MethodCall { method, args, .. } => {
                    assert_eq!(*method, interner.intern("updated"));
                    assert_eq!(result.arena.get_expr_list(*args).len(), 2);
                }
                other => panic!("expected MethodCall(updated), got {other:?}"),
            }
        }
        other => panic!("expected Assign reassignment, got {other:?}"),
    }
}

#[test]
fn desugar_compound_index_assign_hoists_index_to_single_temp() {
    // `xs[i + 0] = xs[i + 0] + 1` (compound-expanded): the shared
    // side-effecting index ExprId (`i + 0`, a Binary — non-trivial) hoists to
    // ONE `let $__assign_idx_0` temp -> Block { [let], Assign{..} }; the index
    // evaluates exactly once across read-copy and write-copy.
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let xs = interner.intern("xs");
    let i = interner.intern("i");

    let root = arena.alloc_expr(Expr::new(ExprKind::Ident(xs), Span::new(0, 2)));
    // A non-trivial (side-effecting-shaped) SHARED index ExprId — appears in
    // the write-target steps AND the read-copy inside `value`.
    let i_ref = arena.alloc_expr(Expr::new(ExprKind::Ident(i), Span::new(3, 4)));
    let zero = arena.alloc_expr(Expr::new(ExprKind::Int(0), Span::new(5, 6)));
    let idx = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: ori_ir::BinaryOp::Add,
            left: i_ref,
            right: zero,
        },
        Span::new(3, 6),
    ));
    let steps = arena.alloc_access_steps([AccessStep::Index(idx)]);
    let target = arena.alloc_expr(Expr::new(
        ExprKind::AssignTarget { root, steps },
        Span::new(0, 4),
    ));

    let read_root = arena.alloc_expr(Expr::new(ExprKind::Ident(xs), Span::new(0, 2)));
    let read = arena.alloc_expr(Expr::new(
        ExprKind::Index {
            receiver: read_root,
            index: idx,
        },
        Span::new(0, 4),
    ));
    let one = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(7, 8)));
    let value = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: ori_ir::BinaryOp::Add,
            left: read,
            right: one,
        },
        Span::new(0, 8),
    ));
    let assign = arena.alloc_expr(Expr::new(
        ExprKind::Assign { target, value },
        Span::new(0, 8),
    ));

    let type_result = assign_type_result(assign.index() + 1, target, vec![Idx::INT, Idx::INT]);
    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, assign, &interner);
    assert!(result.root.is_valid());

    match result.arena.kind(result.root) {
        CanExpr::Block {
            stmts,
            result: block_result,
        } => {
            assert_eq!(result.arena.get_expr_list(*stmts).len(), 1);
            let stmt = result.arena.get_expr_list(*stmts)[0];
            assert!(matches!(result.arena.kind(stmt), CanExpr::Let { .. }));
            assert!(matches!(
                result.arena.kind(*block_result),
                CanExpr::Assign { .. }
            ));
        }
        other => panic!("expected Block wrapping the hoisted reassignment, got {other:?}"),
    }
}

#[test]
fn desugar_nested_index_assign_wraps_right_to_left() {
    // `grid[0][1] = 5` -> `grid = grid.updated(0, grid[0].updated(1, 5))`.
    // The outer reassignment's value is the OUTER updated; its `value` arg is
    // the INNER updated (right-to-left wrap).
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let grid = interner.intern("grid");

    let root = arena.alloc_expr(Expr::new(ExprKind::Ident(grid), Span::new(0, 4)));
    let i0 = arena.alloc_expr(Expr::new(ExprKind::Int(0), Span::new(5, 6)));
    let i1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(8, 9)));
    let value = arena.alloc_expr(Expr::new(ExprKind::Int(5), Span::new(13, 14)));
    let steps = arena.alloc_access_steps([AccessStep::Index(i0), AccessStep::Index(i1)]);
    let target = arena.alloc_expr(Expr::new(
        ExprKind::AssignTarget { root, steps },
        Span::new(0, 9),
    ));
    let assign = arena.alloc_expr(Expr::new(
        ExprKind::Assign { target, value },
        Span::new(0, 14),
    ));

    let type_result = assign_type_result(
        assign.index() + 1,
        target,
        vec![Idx::INT, Idx::INT, Idx::INT],
    );
    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, assign, &interner);
    assert!(result.root.is_valid());

    let updated = interner.intern("updated");
    // Trivial literal indices need no hoist temp -> bare Assign reassignment.
    let outer_value = match result.arena.kind(result.root) {
        CanExpr::Assign { value, .. } => *value,
        other => panic!("expected Assign reassignment, got {other:?}"),
    };
    let inner = match result.arena.kind(outer_value) {
        CanExpr::MethodCall { method, args, .. } => {
            assert_eq!(*method, updated);
            let arg_ids = result.arena.get_expr_list(*args);
            assert_eq!(arg_ids.len(), 2);
            arg_ids[1]
        }
        other => panic!("expected OUTER MethodCall(updated), got {other:?}"),
    };
    match result.arena.kind(inner) {
        CanExpr::MethodCall { method, .. } => assert_eq!(*method, updated),
        other => panic!("expected INNER MethodCall(updated), got {other:?}"),
    }
}
