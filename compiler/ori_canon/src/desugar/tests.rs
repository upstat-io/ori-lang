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

// Non-primitive `{expr:spec}` FormatWith desugar

/// Lower `` `{x:spec}` `` (single interpolation, with a format spec) and return
/// the lowered result.
fn lower_single_format_with(
    arena: &mut ExprArena,
    interner: &SharedInterner,
    type_result: &TypeCheckResult,
    pool: &ori_types::Pool,
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
    lower(arena, type_result, pool, root, interner)
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
    let pool = ori_types::Pool::new();
    let result = lower_single_format_with(&mut arena, &interner, &type_result, &pool);

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
    let mut pool = ori_types::Pool::new();
    let non_primitive = pool.named(interner.intern("PrintableOnly"));
    // expr_types: [0]=Ident(x):<non-primitive>, [1]=TemplateLiteral:str
    let type_result = test_type_result(vec![non_primitive, Idx::STR]);
    let result = lower_single_format_with(&mut arena, &interner, &type_result, &pool);

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
    let mut pool = ori_types::Pool::new();
    let non_primitive = pool.named(interner.intern("ExplicitFormattable"));
    let mut typed = TypedModule::new();
    typed.expr_types.push(non_primitive); // [0]=Ident(x)
    typed.expr_types.push(Idx::STR); // [1]=TemplateLiteral
    typed.formattable_impl_types.push(non_primitive);
    let type_result = TypeCheckResult::ok(typed);
    let result = lower_single_format_with(&mut arena, &interner, &type_result, &pool);

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
fn desugar_map_with_spread_chains_via_merge() {
    // {k1: v1, ...base} -> {k1: v1}.merge(base)
    // Positive pin: the canonical shape after MapWithSpread desugaring is a
    // MethodCall(merge) chain, never a residual MapWithSpread sugar variant.
    let mut arena = ExprArena::new();
    let interner = test_interner();

    let base_name = interner.intern("base");
    let key = interner.intern("k1");

    let key_expr = arena.alloc_expr(Expr::new(ExprKind::String(key), Span::new(1, 5)));
    let val_expr = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(7, 8)));
    let base = arena.alloc_expr(Expr::new(ExprKind::Ident(base_name), Span::new(13, 17)));

    let elements = arena.alloc_map_elements([
        ori_ir::MapElement::Entry(ori_ir::MapEntry {
            key: key_expr,
            value: val_expr,
            span: Span::new(1, 8),
        }),
        ori_ir::MapElement::Spread {
            expr: base,
            span: Span::new(10, 17),
        },
    ]);

    let root = arena.alloc_expr(Expr::new(
        ExprKind::MapWithSpread(elements),
        Span::new(0, 18),
    ));

    let type_result = test_type_result(vec![Idx::STR, Idx::INT, Idx::INT, Idx::INT]);

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);
    assert!(result.root.is_valid());

    // Root is a MethodCall(merge); no MapWithSpread sugar survives (PHASE-34 Canon->All).
    match result.arena.kind(result.root) {
        CanExpr::MethodCall {
            method, receiver, ..
        } => {
            assert_eq!(
                *method,
                interner.intern("merge"),
                "spread desugars via merge"
            );
            // The receiver of the merge is the {k1: v1} literal Map segment.
            assert!(
                matches!(result.arena.kind(*receiver), CanExpr::Map(_)),
                "merge receiver is the leading Map literal segment"
            );
        }
        other => panic!("expected MethodCall(merge), got {other:?}"),
    }
}

#[test]
fn desugar_struct_with_spread_flattens_to_struct() {
    // Point { ...base, x: 10 } -> Struct { x: base.x (overridden by 10), y: base.y }
    // Positive pin: the canonical shape is a flat CanExpr::Struct with every
    // declared field resolved; "later wins" — the explicit `x: 10` overrides
    // the spread's `base.x`. No StructWithSpread sugar survives.
    let mut arena = ExprArena::new();
    let interner = test_interner();

    let point = interner.intern("Point");
    let base_name = interner.intern("base");
    let fx = interner.intern("x");
    let fy = interner.intern("y");

    let base = arena.alloc_expr(Expr::new(ExprKind::Ident(base_name), Span::new(11, 15)));
    let ten = arena.alloc_expr(Expr::new(ExprKind::Int(10), Span::new(20, 22)));

    let fields = arena.alloc_struct_lit_fields([
        ori_ir::StructLitField::Spread {
            expr: base,
            span: Span::new(8, 15),
        },
        ori_ir::StructLitField::Field(ori_ir::FieldInit {
            name: fx,
            value: Some(ten),
            span: Span::new(17, 22),
        }),
    ]);

    let root = arena.alloc_expr(Expr::new(
        ExprKind::StructWithSpread {
            name: point,
            fields,
        },
        Span::new(0, 24),
    ));

    // Register the Point struct so resolve_struct_fields returns [x, y].
    let mut typed = TypedModule::new();
    typed.expr_types.push(Idx::INT); // [0] base -> int placeholder
    typed.expr_types.push(Idx::INT); // [1] ten
    typed.expr_types.push(Idx::INT); // [2] StructWithSpread root
    let point_idx = Idx::from_raw(64);
    typed.types.push(ori_types::TypeEntry {
        name: point,
        idx: point_idx,
        kind: ori_types::TypeKind::Struct(ori_types::StructDef {
            fields: vec![
                ori_types::FieldDef {
                    name: fx,
                    ty: Idx::INT,
                    span: Span::DUMMY,
                    visibility: ori_types::Visibility::Public,
                },
                ori_types::FieldDef {
                    name: fy,
                    ty: Idx::INT,
                    span: Span::DUMMY,
                    visibility: ori_types::Visibility::Public,
                },
            ],
            category: ori_types::ValueCategory::default(),
        }),
        span: Span::DUMMY,
        type_params: Vec::new(),
        visibility: ori_types::Visibility::Public,
        merkle_hash: 0,
        repr: None,
        burden: None,
    });
    let type_result = TypeCheckResult::ok(typed);

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);
    assert!(result.root.is_valid());

    match result.arena.kind(result.root) {
        CanExpr::Struct { name, fields } => {
            assert_eq!(*name, point);
            let field_list = result.arena.get_fields(*fields);
            assert_eq!(field_list.len(), 2, "every declared field resolved");
            // Field x: "later wins" — the explicit Int(10), not base.x.
            assert_eq!(field_list[0].name, fx);
            assert_eq!(
                *result.arena.kind(field_list[0].value),
                CanExpr::Int(10),
                "explicit field overrides the spread (later wins)"
            );
            // Field y: from the spread -> base.y Field access.
            assert_eq!(field_list[1].name, fy);
            match result.arena.kind(field_list[1].value) {
                CanExpr::Field { field, .. } => assert_eq!(*field, fy),
                other => panic!("expected Field(base.y), got {other:?}"),
            }
        }
        other => panic!("expected flat Struct, got {other:?}"),
    }
}

#[test]
fn desugar_method_call_named_drops_to_positional() {
    // No method signature available -> args stay in source order, and the sugar
    // variant lowers to a positional CanExpr::MethodCall. Positive pin: no
    // MethodCallNamed survives canonicalization.
    let mut arena = ExprArena::new();
    let interner = test_interner();

    let recv_name = interner.intern("x");
    let method = interner.intern("scale");
    let arg_factor = interner.intern("factor");

    let receiver = arena.alloc_expr(Expr::new(ExprKind::Ident(recv_name), Span::new(0, 1)));
    let arg_val = arena.alloc_expr(Expr::new(ExprKind::Int(3), Span::new(15, 16)));

    let args = arena.alloc_call_args([CallArg {
        name: Some(arg_factor),
        value: arg_val,
        is_spread: false,
        span: Span::new(8, 16),
    }]);

    let root = arena.alloc_expr(Expr::new(
        ExprKind::MethodCallNamed {
            receiver,
            method,
            args,
        },
        Span::new(0, 17),
    ));

    let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);
    assert!(result.root.is_valid());

    match result.arena.kind(result.root) {
        CanExpr::MethodCall {
            method: m,
            receiver: r,
            args,
        } => {
            assert_eq!(*m, method);
            assert_eq!(*result.arena.kind(*r), CanExpr::Ident(recv_name));
            let arg_list = result.arena.get_expr_list(*args);
            assert_eq!(arg_list.len(), 1);
            assert_eq!(*result.arena.kind(arg_list[0]), CanExpr::Int(3));
        }
        other => panic!("expected positional MethodCall, got {other:?}"),
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
        .insert(target, AssignDesugar { level_types });
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

#[test]
fn module_alias_qualified_call_lowers_to_function_ref_call() {
    // Positive pin: an alias-qualified call `math.add(a: 5)` whose
    // typeck side-table (`module_alias_call_map`) records the resolved qualified
    // name lowers to a free `Call(FunctionRef("math.add"))`, NOT a MethodCall on
    // the namespace placeholder receiver. Fails if the canon module-alias rewrite
    // (desugar_method_call_named -> lower_module_alias_call) is reverted.
    let mut arena = ExprArena::new();
    let interner = test_interner();

    let math = interner.intern("math");
    let add = interner.intern("add");
    let qualified = interner.intern("math.add");
    let arg_name = interner.intern("a");

    let recv = arena.alloc_expr(Expr::new(ExprKind::Ident(math), Span::new(0, 4)));
    let val = arena.alloc_expr(Expr::new(ExprKind::Int(5), Span::new(11, 12)));
    let args = arena.alloc_call_args([CallArg {
        name: Some(arg_name),
        value: val,
        is_spread: false,
        span: Span::new(8, 12),
    }]);
    let root = arena.alloc_expr(Expr::new(
        ExprKind::MethodCallNamed {
            receiver: recv,
            method: add,
            args,
        },
        Span::new(0, 13),
    ));

    let mut typed = TypedModule::new();
    typed.expr_types.push(Idx::INT); // [0] recv (namespace placeholder)
    typed.expr_types.push(Idx::INT); // [1] arg value
    typed.expr_types.push(Idx::INT); // [2] MethodCallNamed root
    typed.module_alias_call_map.insert(root, qualified);
    let type_result = TypeCheckResult::ok(typed);

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);

    match result.arena.kind(result.root) {
        CanExpr::Call { func, .. } => match result.arena.kind(*func) {
            CanExpr::FunctionRef(name) => assert_eq!(
                *name, qualified,
                "alias call must lower to Call(FunctionRef(\"math.add\"))"
            ),
            other => panic!("expected FunctionRef(math.add) callee, got {other:?}"),
        },
        other => panic!("expected free Call for alias-qualified call, got {other:?}"),
    }
}

#[test]
fn module_alias_positional_call_lowers_to_function_ref_call() {
    // Positive pin (positional calling convention): an alias-qualified
    // positional call `math.add(5)` whose typeck side-table records the
    // resolved qualified name lowers to a free `Call(FunctionRef("math.add"))`
    // via the `lower/collections.rs` MethodCall rewrite — the parallel branch
    // to the named-arg rewrite (`desugar/calls.rs`) pinned above. Fails if the
    // positional module-alias rewrite branch is reverted.
    let mut arena = ExprArena::new();
    let interner = test_interner();

    let math = interner.intern("math");
    let add = interner.intern("add");
    let qualified = interner.intern("math.add");

    let recv = arena.alloc_expr(Expr::new(ExprKind::Ident(math), Span::new(0, 4)));
    let val = arena.alloc_expr(Expr::new(ExprKind::Int(5), Span::new(9, 10)));
    let args = arena.alloc_expr_list([val]);
    let root = arena.alloc_expr(Expr::new(
        ExprKind::MethodCall {
            receiver: recv,
            method: add,
            args,
        },
        Span::new(0, 11),
    ));

    let mut typed = TypedModule::new();
    typed.expr_types.push(Idx::INT); // [0] recv (namespace placeholder)
    typed.expr_types.push(Idx::INT); // [1] arg value
    typed.expr_types.push(Idx::INT); // [2] MethodCall root
    typed.module_alias_call_map.insert(root, qualified);
    let type_result = TypeCheckResult::ok(typed);

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);

    match result.arena.kind(result.root) {
        CanExpr::Call { func, .. } => match result.arena.kind(*func) {
            CanExpr::FunctionRef(name) => assert_eq!(
                *name, qualified,
                "positional alias call must lower to Call(FunctionRef(\"math.add\"))"
            ),
            other => panic!("expected FunctionRef(math.add) callee, got {other:?}"),
        },
        other => panic!("expected free Call for positional alias call, got {other:?}"),
    }
}

#[test]
fn ordinary_method_call_not_rewritten_to_function_ref() {
    // Negative pin: an ordinary method call `x.add(a: 5)` with NO
    // `module_alias_call_map` entry must stay a MethodCall, never a free
    // Call(FunctionRef). Clamps the alias-rewrite branch so it fires ONLY on a
    // recorded alias-qualified call — guards against the rewrite wrongly firing
    // on every method call.
    let mut arena = ExprArena::new();
    let interner = test_interner();

    let x = interner.intern("x");
    let add = interner.intern("add");
    let arg_name = interner.intern("a");

    let recv = arena.alloc_expr(Expr::new(ExprKind::Ident(x), Span::new(0, 1)));
    let val = arena.alloc_expr(Expr::new(ExprKind::Int(5), Span::new(8, 9)));
    let args = arena.alloc_call_args([CallArg {
        name: Some(arg_name),
        value: val,
        is_spread: false,
        span: Span::new(5, 9),
    }]);
    let root = arena.alloc_expr(Expr::new(
        ExprKind::MethodCallNamed {
            receiver: recv,
            method: add,
            args,
        },
        Span::new(0, 10),
    ));

    // module_alias_call_map left empty -> no rewrite.
    let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);

    assert!(
        matches!(result.arena.kind(result.root), CanExpr::MethodCall { .. }),
        "ordinary method call must stay a MethodCall, got {:?}",
        result.arena.kind(result.root)
    );
}
