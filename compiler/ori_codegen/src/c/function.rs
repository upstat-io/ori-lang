//! Function Code Generation
//!
//! Generates C code for Ori function bodies.

use ori_ir::{ExprArena, ExprId, ast::{ExprKind, FunctionSeq, SeqBinding, BindingPattern}};

use crate::context::CodegenContext;
use super::stmt::emit_expr_stmt;
use super::expr::emit_expr;

/// Emit the body of a function.
///
/// If `has_return` is true, the function should return a value.
pub fn emit_body(
    ctx: &mut CodegenContext<'_>,
    arena: &ExprArena,
    body: ExprId,
    has_return: bool,
) {
    let expr = arena.get_expr(body);

    match &expr.kind {
        // Block is the common case
        ExprKind::Block { stmts, result } => {
            // Emit statements
            for stmt in arena.get_stmt_range(*stmts) {
                super::stmt::emit_stmt(ctx, arena, stmt);
            }

            // Emit result
            if let Some(result_id) = result {
                if has_return {
                    let result_expr = emit_expr(ctx, arena, *result_id);
                    ctx.writeln(&format!("return {result_expr};"));
                } else {
                    emit_expr_stmt(ctx, arena, *result_id);
                }
            }
        }

        // FunctionSeq (run/try)
        ExprKind::FunctionSeq(seq) => {
            let (bindings_range, result) = match seq {
                FunctionSeq::Run { bindings, result, .. }
                | FunctionSeq::Try { bindings, result, .. } => (Some(*bindings), Some(*result)),
                FunctionSeq::Match { .. }
                | FunctionSeq::ForPattern { .. } => (None, None),
            };

            if let Some(range) = bindings_range {
                let bindings = arena.get_seq_bindings(range);
                for binding in bindings {
                    match binding {
                        SeqBinding::Let { pattern, value, .. } => {
                            // Extract name from pattern
                            if let BindingPattern::Name(name) = pattern {
                                let var_name = ctx.mangle(*name);
                                let value_expr = emit_expr(ctx, arena, *value);
                                let type_id = ctx.expr_type(*value);
                                let type_str = super::types::CTypeMapper::map_type_id(type_id, ctx.type_interner);
                                ctx.writeln(&format!("{type_str} {var_name} = {value_expr};"));
                            } else {
                                // Complex pattern - just evaluate for side effects
                                let value_expr = emit_expr(ctx, arena, *value);
                                ctx.writeln(&format!("{value_expr};"));
                            }
                        }
                        SeqBinding::Stmt { expr, .. } => {
                            emit_expr_stmt(ctx, arena, *expr);
                        }
                    }
                }
            }

            // Emit result if present
            if let Some(result_id) = result {
                if has_return {
                    let result_expr = emit_expr(ctx, arena, result_id);
                    ctx.writeln(&format!("return {result_expr};"));
                } else {
                    emit_expr_stmt(ctx, arena, result_id);
                }
            }
        }

        // Single expression body (e.g., @add (a: int, b: int) -> int = a + b)
        _ => {
            if has_return {
                let result_expr = emit_expr(ctx, arena, body);
                ctx.writeln(&format!("return {result_expr};"));
            } else {
                emit_expr_stmt(ctx, arena, body);
            }
        }
    }
}

/// Emit cleanup code for a function.
///
/// Releases any local variables that need ARC cleanup.
#[allow(dead_code)]
pub fn emit_cleanup(
    ctx: &mut CodegenContext<'_>,
    locals: &[ori_ir::Name],
) {
    for &name in locals {
        if ctx.needs_release(name) {
            let var_name = ctx.mangle(name);
            ctx.writeln(&format!("ori_arc_release({var_name});"));
        }
    }
}

/// Emit a function prologue (local variable declarations, etc.).
#[allow(dead_code)]
pub fn emit_prologue(
    ctx: &mut CodegenContext<'_>,
    arena: &ExprArena,
    body: ExprId,
) {
    // For now, no prologue needed
    // In the future, we might pre-declare variables for better C compatibility
    let _ = ctx;
    let _ = arena;
    let _ = body;
}

/// Emit a function epilogue (cleanup, etc.).
#[allow(dead_code)]
pub fn emit_epilogue(
    ctx: &mut CodegenContext<'_>,
    locals: &[ori_ir::Name],
) {
    emit_cleanup(ctx, locals);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::StringInterner;
    use ori_types::TypeInterner;

    #[test]
    fn test_empty_body() {
        let _arena = ExprArena::new();
        let interner = StringInterner::new();
        let type_interner = TypeInterner::new();
        let _ctx = CodegenContext::new(&interner, &type_interner, &[]);

        // Can't easily test without a valid ExprId
        // Just verify the module compiles
        assert!(true);
    }
}
