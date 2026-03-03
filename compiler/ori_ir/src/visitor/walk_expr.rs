//! Expression walker — exhaustive child traversal for `ExprKind`.

use crate::ast::{Expr, ExprKind};
use crate::ExprArena;

use super::Visitor;

/// Walk an expression's children.
#[expect(
    clippy::too_many_lines,
    reason = "exhaustive ExprKind child-walking dispatch"
)]
pub fn walk_expr<'ast, V: Visitor<'ast> + ?Sized>(
    visitor: &mut V,
    expr: &Expr,
    arena: &'ast ExprArena,
) {
    match &expr.kind {
        // Literals - no children
        ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Char(_)
        | ExprKind::Duration { .. }
        | ExprKind::Size { .. }
        | ExprKind::Unit
        | ExprKind::Ident(_)
        | ExprKind::Const(_)
        | ExprKind::SelfRef
        | ExprKind::FunctionRef(_)
        | ExprKind::HashLength
        | ExprKind::None
        | ExprKind::TemplateFull(_)
        | ExprKind::Error => {}

        // Single child
        ExprKind::Unary { operand, .. } => {
            visitor.visit_expr_id(*operand, arena);
        }
        ExprKind::Try(inner)
        | ExprKind::Await(inner)
        | ExprKind::Some(inner)
        | ExprKind::Unsafe(inner) => {
            visitor.visit_expr_id(*inner, arena);
        }
        ExprKind::Cast { expr, .. } => {
            visitor.visit_expr_id(*expr, arena);
        }
        ExprKind::Loop { body, .. } => {
            visitor.visit_expr_id(*body, arena);
        }
        ExprKind::Break { value, .. } | ExprKind::Continue { value, .. } => {
            if value.is_present() {
                visitor.visit_expr_id(*value, arena);
            }
        }
        ExprKind::Ok(inner) | ExprKind::Err(inner) => {
            if inner.is_present() {
                visitor.visit_expr_id(*inner, arena);
            }
        }

        // Two children
        ExprKind::Binary { left, right, .. } => {
            visitor.visit_expr_id(*left, arena);
            visitor.visit_expr_id(*right, arena);
        }
        ExprKind::Index { receiver, index } => {
            visitor.visit_expr_id(*receiver, arena);
            visitor.visit_expr_id(*index, arena);
        }
        ExprKind::Assign { target, value } => {
            visitor.visit_expr_id(*target, arena);
            visitor.visit_expr_id(*value, arena);
        }

        // Field access
        ExprKind::Field { receiver, .. } => {
            visitor.visit_expr_id(*receiver, arena);
        }

        // Calls
        ExprKind::Call { func, args } => {
            visitor.visit_expr_id(*func, arena);
            for arg_id in arena.get_expr_list(*args).iter().copied() {
                visitor.visit_expr_id(arg_id, arena);
            }
        }
        ExprKind::CallNamed { func, args } => {
            visitor.visit_expr_id(*func, arena);
            for arg in arena.get_call_args(*args) {
                visitor.visit_call_arg(arg, arena);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            visitor.visit_expr_id(*receiver, arena);
            for arg_id in arena.get_expr_list(*args).iter().copied() {
                visitor.visit_expr_id(arg_id, arena);
            }
        }
        ExprKind::MethodCallNamed { receiver, args, .. } => {
            visitor.visit_expr_id(*receiver, arena);
            for arg in arena.get_call_args(*args) {
                visitor.visit_call_arg(arg, arena);
            }
        }

        // Control flow
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            visitor.visit_expr_id(*cond, arena);
            visitor.visit_expr_id(*then_branch, arena);
            if else_branch.is_present() {
                visitor.visit_expr_id(*else_branch, arena);
            }
        }
        ExprKind::Match { scrutinee, arms } => {
            visitor.visit_expr_id(*scrutinee, arena);
            for arm in arena.get_arms(*arms) {
                visitor.visit_match_arm(arm, arena);
            }
        }
        ExprKind::For {
            iter, guard, body, ..
        } => {
            visitor.visit_expr_id(*iter, arena);
            if guard.is_present() {
                visitor.visit_expr_id(*guard, arena);
            }
            visitor.visit_expr_id(*body, arena);
        }
        ExprKind::Block { stmts, result } => {
            for stmt in arena.get_stmt_range(*stmts) {
                visitor.visit_stmt(stmt, arena);
            }
            if result.is_present() {
                visitor.visit_expr_id(*result, arena);
            }
        }

        // Binding
        ExprKind::Let { pattern, init, .. } => {
            let pat = arena.get_binding_pattern(*pattern);
            visitor.visit_binding_pattern(pat);
            visitor.visit_expr_id(*init, arena);
        }
        ExprKind::Lambda { params, body, .. } => {
            for param in arena.get_params(*params) {
                visitor.visit_param(param, arena);
            }
            visitor.visit_expr_id(*body, arena);
        }

        // Collections
        ExprKind::List(items) | ExprKind::Tuple(items) => {
            for item_id in arena.get_expr_list(*items).iter().copied() {
                visitor.visit_expr_id(item_id, arena);
            }
        }
        ExprKind::Map(entries) => {
            for entry in arena.get_map_entries(*entries) {
                visitor.visit_map_entry(entry, arena);
            }
        }
        ExprKind::Struct { fields, .. } => {
            for init in arena.get_field_inits(*fields) {
                visitor.visit_field_init(init, arena);
            }
        }
        ExprKind::StructWithSpread { fields, .. } => {
            for field in arena.get_struct_lit_fields(*fields) {
                visitor.visit_struct_lit_field(field, arena);
            }
        }
        ExprKind::ListWithSpread(elements) => {
            for element in arena.get_list_elements(*elements) {
                visitor.visit_list_element(element, arena);
            }
        }
        ExprKind::MapWithSpread(elements) => {
            for element in arena.get_map_elements(*elements) {
                visitor.visit_map_element(element, arena);
            }
        }
        ExprKind::Range {
            start,
            end,
            step,
            inclusive: _,
        } => {
            if start.is_present() {
                visitor.visit_expr_id(*start, arena);
            }
            if end.is_present() {
                visitor.visit_expr_id(*end, arena);
            }
            if step.is_present() {
                visitor.visit_expr_id(*step, arena);
            }
        }

        // Capability provision
        ExprKind::WithCapability { provider, body, .. } => {
            visitor.visit_expr_id(*provider, arena);
            visitor.visit_expr_id(*body, arena);
        }

        // function_seq / function_exp (arena-allocated)
        ExprKind::FunctionSeq(id) => {
            let seq = arena.get_function_seq(*id);
            visitor.visit_function_seq(seq, arena);
        }
        ExprKind::FunctionExp(id) => {
            let exp = arena.get_function_exp(*id);
            visitor.visit_function_exp(exp, arena);
        }

        // Template literals with interpolation
        ExprKind::TemplateLiteral { parts, .. } => {
            for part in arena.get_template_parts(*parts) {
                visitor.visit_expr_id(part.expr, arena);
            }
        }
    }
}
