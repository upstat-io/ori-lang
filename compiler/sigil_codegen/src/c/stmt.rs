//! Statement Code Generation
//!
//! Generates C code for Sigil statements.

use sigil_ir::{
    ast::{BindingPattern, ExprKind, SeqBinding, Stmt, StmtKind},
    ExprArena, ExprId, StmtRange, TypeId,
};

use crate::context::CodegenContext;
use super::expr::emit_expr;
use super::types::CTypeMapper;

/// Emit C code for a statement range.
pub fn emit_stmts(
    ctx: &mut CodegenContext<'_>,
    arena: &ExprArena,
    range: StmtRange,
) {
    for stmt in arena.get_stmt_range(range) {
        emit_stmt(ctx, arena, stmt);
    }
}

/// Emit C code for a single statement.
pub fn emit_stmt(
    ctx: &mut CodegenContext<'_>,
    arena: &ExprArena,
    stmt: &Stmt,
) {
    match &stmt.kind {
        StmtKind::Expr(expr_id) => {
            emit_expr_stmt(ctx, arena, *expr_id);
        }
        StmtKind::Let { pattern, ty, init, mutable: _ } => {
            emit_let_binding_from_stmt(ctx, arena, pattern, *ty, *init);
        }
    }
}

/// Emit a let binding from a StmtKind::Let.
fn emit_let_binding_from_stmt(
    ctx: &mut CodegenContext<'_>,
    arena: &ExprArena,
    pattern: &BindingPattern,
    ty: Option<TypeId>,
    init: ExprId,
) {
    let init_expr = emit_expr(ctx, arena, init);

    match pattern {
        BindingPattern::Name(name) => {
            let var_name = ctx.mangle(*name);
            let type_str = if let Some(type_id) = ty {
                CTypeMapper::map_type_id(type_id, ctx.type_interner)
            } else {
                // Use type inference result
                let type_id = ctx.expr_type(init);
                CTypeMapper::map_type_id(type_id, ctx.type_interner)
            };

            ctx.writeln(&format!("{type_str} {var_name} = {init_expr};"));
        }

        BindingPattern::Tuple(patterns) => {
            // Destructure tuple
            let tmp = ctx.fresh_temp();
            let type_id = ty.unwrap_or_else(|| ctx.expr_type(init));
            let type_str = CTypeMapper::map_type_id(type_id, ctx.type_interner);
            ctx.writeln(&format!("{type_str} {tmp} = {init_expr};"));

            for (i, pat) in patterns.iter().enumerate() {
                if let BindingPattern::Name(name) = pat {
                    let var_name = ctx.mangle(*name);
                    ctx.writeln(&format!("__auto_type {var_name} = {tmp}._{i};"));
                }
            }
        }

        BindingPattern::Struct { fields } => {
            // Destructure struct
            let tmp = ctx.fresh_temp();
            let type_id = ty.unwrap_or_else(|| ctx.expr_type(init));
            let type_str = CTypeMapper::map_type_id(type_id, ctx.type_interner);
            ctx.writeln(&format!("{type_str} {tmp} = {init_expr};"));

            for (field_name, opt_pattern) in fields {
                if let Some(BindingPattern::Name(name)) = opt_pattern {
                    let var_name = ctx.mangle(*name);
                    let field_str = ctx.resolve_name(*field_name);
                    ctx.writeln(&format!("__auto_type {var_name} = {tmp}.{field_str};"));
                } else {
                    // Shorthand: { x } means { x: x }
                    let var_name = ctx.mangle(*field_name);
                    let field_str = ctx.resolve_name(*field_name);
                    ctx.writeln(&format!("__auto_type {var_name} = {tmp}.{field_str};"));
                }
            }
        }

        BindingPattern::List { elements, rest } => {
            // Destructure list
            let tmp = ctx.fresh_temp();
            ctx.writeln(&format!("sigil_list_t {tmp} = {init_expr};"));

            for (i, pat) in elements.iter().enumerate() {
                if let BindingPattern::Name(name) = pat {
                    let var_name = ctx.mangle(*name);
                    ctx.writeln(&format!(
                        "__auto_type {var_name} = sigil_list_get({tmp}, {i});"
                    ));
                }
            }

            if let Some(rest_name) = rest {
                let var_name = ctx.mangle(*rest_name);
                let start = elements.len();
                ctx.writeln(&format!(
                    "sigil_list_t {var_name} = sigil_list_slice({tmp}, {start}, -1);"
                ));
            }
        }

        BindingPattern::Wildcard => {
            // Evaluate for side effects but discard
            ctx.writeln(&format!("(void){init_expr};"));
        }
    }
}

/// Emit an expression as a statement.
pub fn emit_expr_stmt(
    ctx: &mut CodegenContext<'_>,
    arena: &ExprArena,
    id: ExprId,
) {
    let expr = arena.get_expr(id);

    match &expr.kind {
        // Let binding (expression form)
        ExprKind::Let { pattern, ty, init, mutable: _ } => {
            emit_let_binding(ctx, arena, pattern, ty.as_ref(), *init);
        }

        // If statement (without else, or void result)
        ExprKind::If { cond, then_branch, else_branch } => {
            emit_if_stmt(ctx, arena, *cond, *then_branch, *else_branch);
        }

        // Match statement
        ExprKind::Match { scrutinee, arms } => {
            emit_match_stmt(ctx, arena, *scrutinee, *arms);
        }

        // For loop
        ExprKind::For { binding, iter, guard, body, is_yield } => {
            emit_for_stmt(ctx, arena, *binding, *iter, *guard, *body, *is_yield);
        }

        // While-style loop
        ExprKind::Loop { body } => {
            emit_loop_stmt(ctx, arena, *body);
        }

        // Block
        ExprKind::Block { stmts, result } => {
            emit_block(ctx, arena, *stmts, *result);
        }

        // Return
        ExprKind::Return(val) => {
            if let Some(v) = val {
                let val_expr = emit_expr(ctx, arena, *v);
                ctx.writeln(&format!("return {val_expr};"));
            } else {
                ctx.writeln("return;");
            }
        }

        // Break
        ExprKind::Break(val) => {
            if let Some(v) = val {
                let val_expr = emit_expr(ctx, arena, *v);
                ctx.writeln(&format!("_break_value = {val_expr};"));
            }
            ctx.writeln("break;");
        }

        // Continue
        ExprKind::Continue => {
            ctx.writeln("continue;");
        }

        // Assignment
        ExprKind::Assign { target, value } => {
            let target_expr = emit_expr(ctx, arena, *target);
            let value_expr = emit_expr(ctx, arena, *value);
            ctx.writeln(&format!("{target_expr} = {value_expr};"));
        }

        // FunctionSeq (run/try blocks)
        ExprKind::FunctionSeq(seq) => {
            emit_function_seq(ctx, arena, seq);
        }

        // Other expressions
        _ => {
            let expr_code = emit_expr(ctx, arena, id);
            // Don't emit void expressions as statements
            if expr_code != "((void)0)" {
                ctx.writeln(&format!("{expr_code};"));
            }
        }
    }
}

/// Emit a let binding (from expression form).
fn emit_let_binding(
    ctx: &mut CodegenContext<'_>,
    arena: &ExprArena,
    pattern: &BindingPattern,
    ty: Option<&sigil_ir::ParsedType>,
    init: ExprId,
) {
    let init_expr = emit_expr(ctx, arena, init);

    match pattern {
        BindingPattern::Name(name) => {
            let var_name = ctx.mangle(*name);
            let type_str = if let Some(parsed_ty) = ty {
                CTypeMapper::map_parsed_type(parsed_ty, ctx.interner)
            } else {
                // Use type inference result
                let type_id = ctx.expr_type(init);
                CTypeMapper::map_type_id(type_id, ctx.type_interner)
            };

            ctx.writeln(&format!("{type_str} {var_name} = {init_expr};"));
        }

        BindingPattern::Tuple(patterns) => {
            // Destructure tuple
            let tmp = ctx.fresh_temp();
            let type_id = ctx.expr_type(init);
            let type_str = CTypeMapper::map_type_id(type_id, ctx.type_interner);
            ctx.writeln(&format!("{type_str} {tmp} = {init_expr};"));

            for (i, pat) in patterns.iter().enumerate() {
                if let BindingPattern::Name(name) = pat {
                    let var_name = ctx.mangle(*name);
                    ctx.writeln(&format!("__auto_type {var_name} = {tmp}._{i};"));
                }
            }
        }

        BindingPattern::Struct { fields } => {
            // Destructure struct
            let tmp = ctx.fresh_temp();
            let type_id = ctx.expr_type(init);
            let type_str = CTypeMapper::map_type_id(type_id, ctx.type_interner);
            ctx.writeln(&format!("{type_str} {tmp} = {init_expr};"));

            for (field_name, opt_pattern) in fields {
                if let Some(BindingPattern::Name(name)) = opt_pattern {
                    let var_name = ctx.mangle(*name);
                    let field_str = ctx.resolve_name(*field_name);
                    ctx.writeln(&format!("__auto_type {var_name} = {tmp}.{field_str};"));
                } else {
                    // Shorthand: { x } means { x: x }
                    let var_name = ctx.mangle(*field_name);
                    let field_str = ctx.resolve_name(*field_name);
                    ctx.writeln(&format!("__auto_type {var_name} = {tmp}.{field_str};"));
                }
            }
        }

        BindingPattern::List { elements, rest } => {
            // Destructure list
            let tmp = ctx.fresh_temp();
            ctx.writeln(&format!("sigil_list_t {tmp} = {init_expr};"));

            for (i, pat) in elements.iter().enumerate() {
                if let BindingPattern::Name(name) = pat {
                    let var_name = ctx.mangle(*name);
                    ctx.writeln(&format!(
                        "__auto_type {var_name} = sigil_list_get({tmp}, {i});"
                    ));
                }
            }

            if let Some(rest_name) = rest {
                let var_name = ctx.mangle(*rest_name);
                let start = elements.len();
                ctx.writeln(&format!(
                    "sigil_list_t {var_name} = sigil_list_slice({tmp}, {start}, -1);"
                ));
            }
        }

        BindingPattern::Wildcard => {
            // Evaluate for side effects but discard
            ctx.writeln(&format!("(void){init_expr};"));
        }
    }
}

/// Emit an if statement.
fn emit_if_stmt(
    ctx: &mut CodegenContext<'_>,
    arena: &ExprArena,
    cond: ExprId,
    then_branch: ExprId,
    else_branch: Option<ExprId>,
) {
    let cond_expr = emit_expr(ctx, arena, cond);
    ctx.writeln(&format!("if ({cond_expr}) {{"));
    ctx.indent();
    emit_expr_stmt(ctx, arena, then_branch);
    ctx.dedent();

    if let Some(else_id) = else_branch {
        // Check if else is another if (else if chain)
        let else_expr = arena.get_expr(else_id);
        if let ExprKind::If { cond: else_cond, then_branch: else_then, else_branch: else_else } = &else_expr.kind {
            ctx.writeln("} else");
            emit_if_stmt(ctx, arena, *else_cond, *else_then, *else_else);
            return;
        }

        ctx.writeln("} else {");
        ctx.indent();
        emit_expr_stmt(ctx, arena, else_id);
        ctx.dedent();
    }

    ctx.writeln("}");
}

/// Emit a match statement (simplified - generates if-else chain).
fn emit_match_stmt(
    ctx: &mut CodegenContext<'_>,
    arena: &ExprArena,
    scrutinee: ExprId,
    arms: sigil_ir::ArmRange,
) {
    let scrutinee_expr = emit_expr(ctx, arena, scrutinee);
    let tmp = ctx.fresh_temp();
    let type_id = ctx.expr_type(scrutinee);
    let type_str = CTypeMapper::map_type_id(type_id, ctx.type_interner);

    ctx.writeln(&format!("{type_str} {tmp} = {scrutinee_expr};"));

    let arms_slice = arena.get_arms(arms);
    for (i, arm) in arms_slice.iter().enumerate() {
        // Simplified pattern matching - just use true for now
        // Full pattern matching would require more complex codegen
        let condition = "true"; // Placeholder

        if i == 0 {
            ctx.writeln(&format!("if ({condition}) {{"));
        } else {
            ctx.writeln(&format!("}} else if ({condition}) {{"));
        }
        ctx.indent();

        // Check guard
        if let Some(guard) = arm.guard {
            let guard_expr = emit_expr(ctx, arena, guard);
            ctx.writeln(&format!("if ({guard_expr}) {{"));
            ctx.indent();
            emit_expr_stmt(ctx, arena, arm.body);
            ctx.dedent();
            ctx.writeln("}");
        } else {
            emit_expr_stmt(ctx, arena, arm.body);
        }

        ctx.dedent();
    }
    ctx.writeln("}");
}

/// Emit a for loop.
fn emit_for_stmt(
    ctx: &mut CodegenContext<'_>,
    arena: &ExprArena,
    binding: sigil_ir::Name,
    iter: ExprId,
    guard: Option<ExprId>,
    body: ExprId,
    is_yield: bool,
) {
    let iter_expr = emit_expr(ctx, arena, iter);
    let iter_tmp = ctx.fresh_temp();
    let idx_tmp = ctx.fresh_temp();
    let var_name = ctx.mangle(binding);

    // For now, assume iterating over a list
    ctx.writeln(&format!("sigil_list_t {iter_tmp} = {iter_expr};"));
    ctx.writeln(&format!("for (uint64_t {idx_tmp} = 0; {idx_tmp} < {iter_tmp}.len; {idx_tmp}++) {{"));
    ctx.indent();
    ctx.writeln(&format!("__auto_type {var_name} = sigil_list_get({iter_tmp}, {idx_tmp});"));

    if let Some(guard_id) = guard {
        let guard_expr = emit_expr(ctx, arena, guard_id);
        ctx.writeln(&format!("if (!({guard_expr})) continue;"));
    }

    if is_yield {
        // Collect results into a list
        ctx.writeln("// yield expression - collect results");
    }

    emit_expr_stmt(ctx, arena, body);

    ctx.dedent();
    ctx.writeln("}");
}

/// Emit a loop statement.
fn emit_loop_stmt(
    ctx: &mut CodegenContext<'_>,
    arena: &ExprArena,
    body: ExprId,
) {
    ctx.writeln("while (true) {");
    ctx.indent();
    emit_expr_stmt(ctx, arena, body);
    ctx.dedent();
    ctx.writeln("}");
}

/// Emit a block.
fn emit_block(
    ctx: &mut CodegenContext<'_>,
    arena: &ExprArena,
    stmts: StmtRange,
    result: Option<ExprId>,
) {
    ctx.writeln("{");
    ctx.indent();

    emit_stmts(ctx, arena, stmts);

    if let Some(res_id) = result {
        emit_expr_stmt(ctx, arena, res_id);
    }

    ctx.dedent();
    ctx.writeln("}");
}

/// Emit a function_seq (run/try block).
fn emit_function_seq(
    ctx: &mut CodegenContext<'_>,
    arena: &ExprArena,
    seq: &sigil_ir::ast::FunctionSeq,
) {
    match seq {
        sigil_ir::ast::FunctionSeq::Run { bindings, result, .. }
        | sigil_ir::ast::FunctionSeq::Try { bindings, result, .. } => {
            for binding in arena.get_seq_bindings(*bindings) {
                match binding {
                    SeqBinding::Let { pattern, ty, value, .. } => {
                        let init_expr = emit_expr(ctx, arena, *value);
                        if let BindingPattern::Name(name) = pattern {
                            let var_name = ctx.mangle(*name);
                            let type_str = if let Some(parsed_ty) = ty {
                                CTypeMapper::map_parsed_type(parsed_ty, ctx.interner)
                            } else {
                                let type_id = ctx.expr_type(*value);
                                CTypeMapper::map_type_id(type_id, ctx.type_interner)
                            };
                            ctx.writeln(&format!("{type_str} {var_name} = {init_expr};"));
                        }
                    }
                    SeqBinding::Stmt { expr, .. } => {
                        emit_expr_stmt(ctx, arena, *expr);
                    }
                }
            }
            // Emit result expression
            emit_expr_stmt(ctx, arena, *result);
        }
        sigil_ir::ast::FunctionSeq::Match { scrutinee, arms, .. } => {
            emit_match_stmt(ctx, arena, *scrutinee, *arms);
        }
        sigil_ir::ast::FunctionSeq::ForPattern { over, arm, default, .. } => {
            // Simplified for-pattern
            let _ = emit_expr(ctx, arena, *over);
            emit_expr_stmt(ctx, arena, arm.body);
            let _ = emit_expr(ctx, arena, *default);
        }
    }
}

#[cfg(test)]
mod tests {
    // Statement tests would require a full AST setup
    // For now, just verify the module compiles
    #[test]
    fn test_module_compiles() {
        assert!(true);
    }
}
