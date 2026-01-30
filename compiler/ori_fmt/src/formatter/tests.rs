//! Formatter Core Tests
//!
//! Tests for the formatting engine's inline vs broken decisions.

use crate::context::{FormatContext, MAX_LINE_WIDTH};
use crate::formatter::format_expr;
use ori_ir::{
    ast::{Expr, ExprKind},
    BinaryOp, ExprArena, Span, StringInterner, UnaryOp,
};

/// Helper to create a test expression in the arena.
fn make_expr(arena: &mut ExprArena, kind: ExprKind) -> ori_ir::ExprId {
    arena.alloc_expr(Expr::new(kind, Span::new(0, 1)))
}

/// Helper to format an expression and return the result.
fn format_to_string(
    arena: &ExprArena,
    interner: &StringInterner,
    expr_id: ori_ir::ExprId,
) -> String {
    format_expr(arena, interner, expr_id)
}

// =============================================================================
// Idempotency Tests
// =============================================================================

/// Verify that formatting the same AST twice produces identical output.
/// This is AST-level idempotency - formatting is deterministic.
#[test]
fn format_idempotent_int_literal() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let expr = make_expr(&mut arena, ExprKind::Int(42));

    let first = format_to_string(&arena, &interner, expr);
    let second = format_to_string(&arena, &interner, expr);
    assert_eq!(first, second);
}

#[test]
fn format_idempotent_binary_expression() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let left = make_expr(&mut arena, ExprKind::Int(1));
    let right = make_expr(&mut arena, ExprKind::Int(2));
    let expr = make_expr(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Add,
            left,
            right,
        },
    );

    let first = format_to_string(&arena, &interner, expr);
    let second = format_to_string(&arena, &interner, expr);
    assert_eq!(first, second);
}

#[test]
fn format_idempotent_nested_expression() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    // Build: (1 + 2) * 3
    let one = make_expr(&mut arena, ExprKind::Int(1));
    let two = make_expr(&mut arena, ExprKind::Int(2));
    let add = make_expr(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Add,
            left: one,
            right: two,
        },
    );
    let three = make_expr(&mut arena, ExprKind::Int(3));
    let expr = make_expr(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Mul,
            left: add,
            right: three,
        },
    );

    let first = format_to_string(&arena, &interner, expr);
    let second = format_to_string(&arena, &interner, expr);
    assert_eq!(first, second);
}

// =============================================================================
// Literal Formatting Tests
// =============================================================================

#[test]
fn format_int_literal() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let expr = make_expr(&mut arena, ExprKind::Int(42));
    assert_eq!(format_to_string(&arena, &interner, expr), "42\n");
}

#[test]
fn format_negative_int() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let expr = make_expr(&mut arena, ExprKind::Int(-123));
    assert_eq!(format_to_string(&arena, &interner, expr), "-123\n");
}

#[test]
fn format_float_literal() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let expr = make_expr(&mut arena, ExprKind::Float(3.14f64.to_bits()));
    assert_eq!(format_to_string(&arena, &interner, expr), "3.14\n");
}

#[test]
fn format_float_whole_number() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let expr = make_expr(&mut arena, ExprKind::Float(42.0f64.to_bits()));
    assert_eq!(format_to_string(&arena, &interner, expr), "42.0\n");
}

#[test]
fn format_bool_true() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let expr = make_expr(&mut arena, ExprKind::Bool(true));
    assert_eq!(format_to_string(&arena, &interner, expr), "true\n");
}

#[test]
fn format_bool_false() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let expr = make_expr(&mut arena, ExprKind::Bool(false));
    assert_eq!(format_to_string(&arena, &interner, expr), "false\n");
}

#[test]
fn format_string_literal() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let name = interner.intern("hello");
    let expr = make_expr(&mut arena, ExprKind::String(name));
    assert_eq!(format_to_string(&arena, &interner, expr), "\"hello\"\n");
}

#[test]
fn format_string_with_escapes() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    // String containing newline and tab
    let name = interner.intern("line1\nline2\ttab");
    let expr = make_expr(&mut arena, ExprKind::String(name));
    assert_eq!(
        format_to_string(&arena, &interner, expr),
        "\"line1\\nline2\\ttab\"\n"
    );
}

#[test]
fn format_char_literal() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let expr = make_expr(&mut arena, ExprKind::Char('x'));
    assert_eq!(format_to_string(&arena, &interner, expr), "'x'\n");
}

#[test]
fn format_char_escape() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let expr = make_expr(&mut arena, ExprKind::Char('\n'));
    assert_eq!(format_to_string(&arena, &interner, expr), "'\\n'\n");
}

#[test]
fn format_unit() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let expr = make_expr(&mut arena, ExprKind::Unit);
    assert_eq!(format_to_string(&arena, &interner, expr), "()\n");
}

// =============================================================================
// Operator Formatting Tests
// =============================================================================

#[test]
fn format_binary_add() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let left = make_expr(&mut arena, ExprKind::Int(1));
    let right = make_expr(&mut arena, ExprKind::Int(2));
    let expr = make_expr(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Add,
            left,
            right,
        },
    );

    assert_eq!(format_to_string(&arena, &interner, expr), "1 + 2\n");
}

#[test]
fn format_unary_neg() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let operand = make_expr(&mut arena, ExprKind::Int(5));
    let expr = make_expr(
        &mut arena,
        ExprKind::Unary {
            op: UnaryOp::Neg,
            operand,
        },
    );

    assert_eq!(format_to_string(&arena, &interner, expr), "-5\n");
}

#[test]
fn format_unary_not() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let operand = make_expr(&mut arena, ExprKind::Bool(true));
    let expr = make_expr(
        &mut arena,
        ExprKind::Unary {
            op: UnaryOp::Not,
            operand,
        },
    );

    assert_eq!(format_to_string(&arena, &interner, expr), "!true\n");
}

// =============================================================================
// Duration and Size Formatting Tests
// =============================================================================

#[test]
fn format_duration() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    use ori_ir::DurationUnit;

    let ms = make_expr(
        &mut arena,
        ExprKind::Duration {
            value: 100,
            unit: DurationUnit::Milliseconds,
        },
    );
    assert_eq!(format_to_string(&arena, &interner, ms), "100ms\n");

    let s = make_expr(
        &mut arena,
        ExprKind::Duration {
            value: 30,
            unit: DurationUnit::Seconds,
        },
    );
    assert_eq!(format_to_string(&arena, &interner, s), "30s\n");
}

#[test]
fn format_size() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    use ori_ir::SizeUnit;

    let kb = make_expr(
        &mut arena,
        ExprKind::Size {
            value: 4,
            unit: SizeUnit::Kilobytes,
        },
    );
    assert_eq!(format_to_string(&arena, &interner, kb), "4kb\n");
}

// =============================================================================
// Identifier Formatting Tests
// =============================================================================

#[test]
fn format_identifier() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let name = interner.intern("foo");
    let expr = make_expr(&mut arena, ExprKind::Ident(name));
    assert_eq!(format_to_string(&arena, &interner, expr), "foo\n");
}

#[test]
fn format_config_identifier() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let name = interner.intern("timeout");
    let expr = make_expr(&mut arena, ExprKind::Config(name));
    assert_eq!(format_to_string(&arena, &interner, expr), "$timeout\n");
}

#[test]
fn format_function_ref() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let name = interner.intern("main");
    let expr = make_expr(&mut arena, ExprKind::FunctionRef(name));
    assert_eq!(format_to_string(&arena, &interner, expr), "@main\n");
}

#[test]
fn format_self_ref() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let expr = make_expr(&mut arena, ExprKind::SelfRef);
    assert_eq!(format_to_string(&arena, &interner, expr), "self\n");
}

// =============================================================================
// Control Flow Formatting Tests
// =============================================================================

#[test]
fn format_return_void() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let expr = make_expr(&mut arena, ExprKind::Return(None));
    assert_eq!(format_to_string(&arena, &interner, expr), "return\n");
}

#[test]
fn format_return_value() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let value = make_expr(&mut arena, ExprKind::Int(42));
    let expr = make_expr(&mut arena, ExprKind::Return(Some(value)));
    assert_eq!(format_to_string(&arena, &interner, expr), "return 42\n");
}

#[test]
fn format_break_void() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let expr = make_expr(&mut arena, ExprKind::Break(None));
    assert_eq!(format_to_string(&arena, &interner, expr), "break\n");
}

#[test]
fn format_continue() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let expr = make_expr(&mut arena, ExprKind::Continue);
    assert_eq!(format_to_string(&arena, &interner, expr), "continue\n");
}

// =============================================================================
// Option/Result Formatting Tests
// =============================================================================

#[test]
fn format_some() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let inner = make_expr(&mut arena, ExprKind::Int(42));
    let expr = make_expr(&mut arena, ExprKind::Some(inner));
    assert_eq!(format_to_string(&arena, &interner, expr), "Some(42)\n");
}

#[test]
fn format_none() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let expr = make_expr(&mut arena, ExprKind::None);
    assert_eq!(format_to_string(&arena, &interner, expr), "None\n");
}

#[test]
fn format_ok_with_value() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let inner = make_expr(&mut arena, ExprKind::Int(42));
    let expr = make_expr(&mut arena, ExprKind::Ok(Some(inner)));
    assert_eq!(format_to_string(&arena, &interner, expr), "Ok(42)\n");
}

#[test]
fn format_ok_void() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let expr = make_expr(&mut arena, ExprKind::Ok(None));
    assert_eq!(format_to_string(&arena, &interner, expr), "Ok()\n");
}

#[test]
fn format_err() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let name = interner.intern("error message");
    let inner = make_expr(&mut arena, ExprKind::String(name));
    let expr = make_expr(&mut arena, ExprKind::Err(Some(inner)));
    assert_eq!(
        format_to_string(&arena, &interner, expr),
        "Err(\"error message\")\n"
    );
}

// =============================================================================
// Range Formatting Tests
// =============================================================================

#[test]
fn format_range_exclusive() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let start = make_expr(&mut arena, ExprKind::Int(0));
    let end = make_expr(&mut arena, ExprKind::Int(10));
    let expr = make_expr(
        &mut arena,
        ExprKind::Range {
            start: Some(start),
            end: Some(end),
            inclusive: false,
        },
    );

    assert_eq!(format_to_string(&arena, &interner, expr), "0..10\n");
}

#[test]
fn format_range_inclusive() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let start = make_expr(&mut arena, ExprKind::Int(0));
    let end = make_expr(&mut arena, ExprKind::Int(10));
    let expr = make_expr(
        &mut arena,
        ExprKind::Range {
            start: Some(start),
            end: Some(end),
            inclusive: true,
        },
    );

    assert_eq!(format_to_string(&arena, &interner, expr), "0..=10\n");
}

#[test]
fn format_context_fits_boundary() {
    let mut ctx = FormatContext::new();

    // Emit exactly MAX_LINE_WIDTH - 10 characters
    ctx.emit(&"x".repeat(MAX_LINE_WIDTH - 10));
    assert_eq!(ctx.column(), MAX_LINE_WIDTH - 10);

    // Should fit exactly 10 more
    assert!(ctx.fits(10));
    assert!(!ctx.fits(11));
}

#[test]
fn format_context_indentation_tracking() {
    let mut ctx = FormatContext::new();

    ctx.emit("fn main");
    ctx.emit_newline();
    assert_eq!(ctx.column(), 0);

    ctx.indent();
    ctx.emit_indent();
    assert_eq!(ctx.column(), 4);

    ctx.emit("body");
    assert_eq!(ctx.column(), 8);

    ctx.emit_newline();
    ctx.indent();
    ctx.emit_indent();
    assert_eq!(ctx.column(), 8);

    ctx.emit("nested");
    assert_eq!(ctx.column(), 14);

    ctx.dedent();
    ctx.dedent();
    assert_eq!(ctx.indent_level(), 0);
}

#[test]
fn format_context_with_indent_scope() {
    let mut ctx = FormatContext::new();

    assert_eq!(ctx.indent_level(), 0);

    let result = ctx.with_indent(|inner| {
        assert_eq!(inner.indent_level(), 1);
        inner.emit("inside");
        42
    });

    assert_eq!(result, 42);
    assert_eq!(ctx.indent_level(), 0);
}

#[test]
fn string_emitter_finalization() {
    let mut ctx = FormatContext::new();
    ctx.emit("content");
    ctx.emit_newline();
    ctx.emit_newline();
    ctx.emit_newline();

    let output = ctx.finalize();
    assert_eq!(output, "content\n");
}

#[test]
fn string_emitter_empty_finalization() {
    let ctx = FormatContext::new();
    let output = ctx.finalize();
    assert_eq!(output, "\n");
}

#[test]
fn format_context_column_after_indent() {
    let mut ctx = FormatContext::new();
    ctx.indent();
    ctx.indent();
    ctx.emit_indent();

    assert_eq!(ctx.column(), 8);

    ctx.emit("x");
    assert_eq!(ctx.column(), 9);
}

#[test]
fn format_context_newline_indent_combined() {
    let mut ctx = FormatContext::new();
    ctx.indent();
    ctx.emit("first");
    ctx.emit_newline_indent();
    ctx.emit("second");

    assert_eq!(ctx.as_str(), "first\n    second");
}
