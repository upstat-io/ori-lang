//! Expression Code Generation
//!
//! Generates C code for Ori expressions.

use ori_ir::{
    ast::{BinaryOp, ExprKind, UnaryOp},
    ExprArena, ExprId,
};

use crate::context::CodegenContext;
use super::types::CTypeMapper;

/// Generate C code for an expression.
///
/// Returns the C expression string that can be used in assignments, etc.
pub fn emit_expr(
    ctx: &mut CodegenContext<'_>,
    arena: &ExprArena,
    id: ExprId,
) -> String {
    let expr = arena.get_expr(id);

    match &expr.kind {
        // Literals
        ExprKind::Int(n) => format!("INT64_C({n})"),
        ExprKind::Float(bits) => {
            let f = f64::from_bits(*bits);
            if f.is_nan() {
                "NAN".to_string()
            } else if f.is_infinite() {
                if f.is_sign_positive() { "INFINITY" } else { "-INFINITY" }.to_string()
            } else {
                format!("{f:?}") // Use debug format to preserve precision
            }
        }
        ExprKind::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        ExprKind::String(name) => {
            let s = ctx.resolve_name(*name);
            emit_string_literal(s)
        }
        ExprKind::Char(c) => format!("UINT32_C({:#x})", *c as u32),
        ExprKind::Duration { value, unit } => {
            // Convert to milliseconds
            let ms = match unit {
                ori_ir::DurationUnit::Milliseconds => *value,
                ori_ir::DurationUnit::Seconds => value * 1_000,
                ori_ir::DurationUnit::Minutes => value * 60_000,
                ori_ir::DurationUnit::Hours => value * 3_600_000,
            };
            format!("INT64_C({ms})")
        }
        ExprKind::Size { value, unit } => {
            // Convert to bytes
            let bytes = match unit {
                ori_ir::SizeUnit::Bytes => *value,
                ori_ir::SizeUnit::Kilobytes => value * 1024,
                ori_ir::SizeUnit::Megabytes => value * 1024 * 1024,
                ori_ir::SizeUnit::Gigabytes => value * 1024 * 1024 * 1024,
            };
            format!("UINT64_C({bytes})")
        }
        ExprKind::Unit => "((void)0)".to_string(),

        // Variables
        ExprKind::Ident(name) => {
            let mangled = ctx.mangle(*name);
            let type_id = ctx.expr_type(id);
            // Primitives never need ARC, regardless of elision analysis
            if type_id.is_primitive() || ctx.can_elide_arc(id) {
                mangled
            } else {
                // Need to retain
                format!("ori_arc_retain({mangled})")
            }
        }
        ExprKind::Config(name) => {
            // Config values are compile-time constants
            ctx.mangle(*name)
        }
        ExprKind::FunctionRef(name) => {
            // Function reference
            format!("&{}", ctx.mangle(*name))
        }
        ExprKind::SelfRef => "self".to_string(),
        ExprKind::HashLength => {
            // # in index context - needs context to resolve
            "_ori_len".to_string()
        }

        // Binary operations
        ExprKind::Binary { op, left, right } => {
            let left_expr = emit_expr(ctx, arena, *left);
            let right_expr = emit_expr(ctx, arena, *right);
            emit_binary_op(ctx, *op, &left_expr, &right_expr, *left)
        }

        // Unary operations
        ExprKind::Unary { op, operand } => {
            let operand_expr = emit_expr(ctx, arena, *operand);
            emit_unary_op(*op, &operand_expr)
        }

        // Function calls
        ExprKind::Call { func, args } => {
            let func_expr = emit_callee(ctx, arena, *func);
            let arg_exprs: Vec<_> = arena
                .get_expr_list(*args)
                .iter()
                .map(|&arg| emit_expr(ctx, arena, arg))
                .collect();
            format!("{}({})", func_expr, arg_exprs.join(", "))
        }

        ExprKind::CallNamed { func, args } => {
            let func_expr = emit_callee(ctx, arena, *func);
            let arg_exprs: Vec<_> = arena
                .get_call_args(*args)
                .iter()
                .map(|arg| emit_expr(ctx, arena, arg.value))
                .collect();
            format!("{}({})", func_expr, arg_exprs.join(", "))
        }

        // Method calls
        ExprKind::MethodCall { receiver, method, args } => {
            let method_name = ctx.resolve_name(*method).to_string();
            let recv_expr = emit_expr(ctx, arena, *receiver);
            let args_list: Vec<ExprId> = arena.get_expr_list(*args).to_vec();
            let arg_exprs: Vec<_> = args_list
                .iter()
                .map(|&arg| emit_expr(ctx, arena, arg))
                .collect();

            // Emit as function call with receiver as first argument
            let all_args = std::iter::once(recv_expr.clone())
                .chain(arg_exprs)
                .collect::<Vec<_>>()
                .join(", ");

            format!("ori_{}({})", method_name, all_args)
        }

        ExprKind::MethodCallNamed { receiver, method, args } => {
            let method_name = ctx.resolve_name(*method).to_string();
            let recv_expr = emit_expr(ctx, arena, *receiver);
            let args_list: Vec<_> = arena.get_call_args(*args).iter().map(|a| a.value).collect();
            let arg_exprs: Vec<_> = args_list
                .iter()
                .map(|&value| emit_expr(ctx, arena, value))
                .collect();

            let all_args = std::iter::once(recv_expr)
                .chain(arg_exprs)
                .collect::<Vec<_>>()
                .join(", ");

            format!("ori_{}({})", method_name, all_args)
        }

        // Field access
        ExprKind::Field { receiver, field } => {
            let recv_expr = emit_expr(ctx, arena, *receiver);
            let field_name = ctx.resolve_name(*field);
            format!("{recv_expr}.{field_name}")
        }

        // Index access
        ExprKind::Index { receiver, index } => {
            let recv_expr = emit_expr(ctx, arena, *receiver);
            let index_expr = emit_expr(ctx, arena, *index);
            // Use bounds-checked indexing
            format!("ori_index({recv_expr}, {index_expr})")
        }

        // Conditionals
        ExprKind::If { cond, then_branch, else_branch } => {
            let cond_expr = emit_expr(ctx, arena, *cond);
            let then_expr = emit_expr(ctx, arena, *then_branch);

            if let Some(else_id) = else_branch {
                let else_expr = emit_expr(ctx, arena, *else_id);
                format!("({cond_expr} ? {then_expr} : {else_expr})")
            } else {
                // No else branch - only valid if then branch is void
                format!("({cond_expr} ? {then_expr} : ((void)0))")
            }
        }

        // Option constructors
        ExprKind::Some(inner) => {
            let inner_expr = emit_expr(ctx, arena, *inner);
            let inner_type = ctx.expr_type(*inner);

            // Use unboxed Some for primitives
            match inner_type {
                ori_ir::TypeId::INT => format!("ORI_SOME_INT({inner_expr})"),
                ori_ir::TypeId::FLOAT => format!("ORI_SOME_FLOAT({inner_expr})"),
                ori_ir::TypeId::BOOL => format!("ORI_SOME_BOOL({inner_expr})"),
                ori_ir::TypeId::CHAR => format!("ORI_SOME_CHAR({inner_expr})"),
                ori_ir::TypeId::BYTE => format!("ORI_SOME_BYTE({inner_expr})"),
                _ => format!("ori_some({inner_expr})"),
            }
        }

        ExprKind::None => {
            // Type determines which None to use
            let type_id = ctx.expr_type(id);
            let type_data = ctx.type_interner.lookup(type_id);

            if let ori_types::TypeData::Option(inner) = type_data {
                match inner {
                    ori_ir::TypeId::INT => "ORI_NONE_INT".to_string(),
                    ori_ir::TypeId::FLOAT => "ORI_NONE_FLOAT".to_string(),
                    ori_ir::TypeId::BOOL => "ORI_NONE_BOOL".to_string(),
                    ori_ir::TypeId::CHAR => "ORI_NONE_CHAR".to_string(),
                    ori_ir::TypeId::BYTE => "ORI_NONE_BYTE".to_string(),
                    _ => "ori_none()".to_string(),
                }
            } else {
                "ori_none()".to_string()
            }
        }

        // Result constructors
        ExprKind::Ok(inner) => {
            if let Some(inner_id) = inner {
                let inner_expr = emit_expr(ctx, arena, *inner_id);
                let inner_type = ctx.expr_type(*inner_id);

                match inner_type {
                    ori_ir::TypeId::INT => format!("ORI_OK_INT({inner_expr})"),
                    ori_ir::TypeId::FLOAT => format!("ORI_OK_FLOAT({inner_expr})"),
                    ori_ir::TypeId::BOOL => format!("ORI_OK_BOOL({inner_expr})"),
                    ori_ir::TypeId::VOID => "ORI_OK_VOID".to_string(),
                    _ => format!("ori_ok({inner_expr})"),
                }
            } else {
                "ORI_OK_VOID".to_string()
            }
        }

        ExprKind::Err(inner) => {
            if let Some(inner_id) = inner {
                let inner_expr = emit_expr(ctx, arena, *inner_id);
                // For now, assume Err<str>
                format!("ORI_ERR_INT({inner_expr})") // TODO: determine correct type
            } else {
                "ORI_ERR_VOID(ori_string_from_cstr(\"\"))".to_string()
            }
        }

        // List literal
        ExprKind::List(elems) => {
            let elem_exprs: Vec<_> = arena
                .get_expr_list(*elems)
                .iter()
                .map(|&e| emit_expr(ctx, arena, e))
                .collect();

            if elem_exprs.is_empty() {
                "ori_list_new(0, sizeof(void*))".to_string()
            } else {
                format!(
                    "ori_list_from_array((void*[]){{{}}}, {})",
                    elem_exprs.join(", "),
                    elem_exprs.len()
                )
            }
        }

        // Tuple literal
        ExprKind::Tuple(elems) => {
            let elem_exprs: Vec<_> = arena
                .get_expr_list(*elems)
                .iter()
                .map(|&e| emit_expr(ctx, arena, e))
                .collect();

            if elem_exprs.is_empty() {
                "((void)0)".to_string()
            } else {
                // Use compound literal for tuple
                let type_id = ctx.expr_type(id);
                let c_type = CTypeMapper::map_type_id(type_id, ctx.type_interner);
                format!("(({c_type}){{{}}})", elem_exprs.join(", "))
            }
        }

        // Map literal
        ExprKind::Map(entries) => {
            let entry_pairs: Vec<_> = arena
                .get_map_entries(*entries)
                .iter()
                .map(|e| {
                    let key = emit_expr(ctx, arena, e.key);
                    let val = emit_expr(ctx, arena, e.value);
                    format!("{key}, {val}")
                })
                .collect();

            if entry_pairs.is_empty() {
                "ori_map_new()".to_string()
            } else {
                format!(
                    "ori_map_from_pairs((void*[]){{{}}}, {})",
                    entry_pairs.join(", "),
                    entry_pairs.len()
                )
            }
        }

        // Struct literal
        ExprKind::Struct { name, fields } => {
            let struct_name = ctx.resolve_name(*name).to_string();
            // Collect field info first to avoid borrow issues
            let field_info: Vec<_> = arena
                .get_field_inits(*fields)
                .iter()
                .map(|f| (ctx.resolve_name(f.name).to_string(), f.name, f.value))
                .collect();

            let field_inits: Vec<_> = field_info
                .into_iter()
                .map(|(field_name, name, value)| {
                    // value is Option<ExprId> - None means shorthand (field name = variable name)
                    let value_expr = if let Some(v) = value {
                        emit_expr(ctx, arena, v)
                    } else {
                        // Shorthand: use field name as variable name
                        ctx.mangle(name)
                    };
                    format!(".{field_name} = {value_expr}")
                })
                .collect();

            format!("((ori_{struct_name}_t){{{}}})", field_inits.join(", "))
        }

        // Range expression
        ExprKind::Range { start, end, inclusive } => {
            let start_expr = start
                .map(|s| emit_expr(ctx, arena, s))
                .unwrap_or_else(|| "0".to_string());
            let end_expr = end
                .map(|e| emit_expr(ctx, arena, e))
                .unwrap_or_else(|| "INT64_MAX".to_string());
            let inclusive_int = i32::from(*inclusive);

            format!("ori_range_new({start_expr}, {end_expr}, {inclusive_int})")
        }

        // Lambda
        ExprKind::Lambda { params, body, .. } => {
            // Lambdas need to be lifted to top-level functions
            // For now, emit a placeholder
            let _ = arena.get_params(*params);
            let _ = body;
            "/* lambda */NULL".to_string()
        }

        // Block
        ExprKind::Block { stmts, result } => {
            // Blocks need statement-level codegen
            // For expression context, use GCC statement expression
            let _ = arena.get_stmt_range(*stmts);
            if let Some(res) = result {
                emit_expr(ctx, arena, *res)
            } else {
                "((void)0)".to_string()
            }
        }

        // Let binding (in expression context)
        ExprKind::Let { .. } => {
            // Let needs statement-level codegen
            "/* let */((void)0)".to_string()
        }

        // Match expression
        ExprKind::Match { scrutinee, arms } => {
            // Match needs statement-level codegen for proper control flow
            let _ = emit_expr(ctx, arena, *scrutinee);
            let _ = arena.get_arms(*arms);
            "/* match */0".to_string()
        }

        // For expression
        ExprKind::For { iter, body, .. } => {
            let _ = emit_expr(ctx, arena, *iter);
            let _ = emit_expr(ctx, arena, *body);
            "/* for */((void)0)".to_string()
        }

        // Loop expression
        ExprKind::Loop { body } => {
            let _ = emit_expr(ctx, arena, *body);
            "/* loop */((void)0)".to_string()
        }

        // Control flow
        ExprKind::Return(val) => {
            if let Some(v) = val {
                let val_expr = emit_expr(ctx, arena, *v);
                format!("return {val_expr}")
            } else {
                "return".to_string()
            }
        }

        ExprKind::Break(val) => {
            if let Some(v) = val {
                let val_expr = emit_expr(ctx, arena, *v);
                format!("{{ _break_value = {val_expr}; break; }}")
            } else {
                "break".to_string()
            }
        }

        ExprKind::Continue => "continue".to_string(),

        // Error propagation
        ExprKind::Try(inner) => {
            let inner_expr = emit_expr(ctx, arena, *inner);
            // Generate early return on error
            let tmp = ctx.fresh_temp();
            format!(
                "({{ __auto_type {tmp} = {inner_expr}; if (ORI_IS_ERR({tmp})) return {tmp}; ORI_UNWRAP_OK({tmp}); }})"
            )
        }

        ExprKind::Await(inner) => {
            let inner_expr = emit_expr(ctx, arena, *inner);
            // Await needs runtime support
            format!("ori_await({inner_expr})")
        }

        // Assignment
        ExprKind::Assign { target, value } => {
            let target_expr = emit_expr(ctx, arena, *target);
            let value_expr = emit_expr(ctx, arena, *value);
            format!("({target_expr} = {value_expr})")
        }

        // With capability
        ExprKind::WithCapability { provider, body, .. } => {
            let _ = emit_expr(ctx, arena, *provider);
            emit_expr(ctx, arena, *body)
        }

        // Function patterns
        ExprKind::FunctionSeq(seq) => {
            // Generate as statement expression
            match seq {
                ori_ir::ast::FunctionSeq::Run { bindings, result, .. }
                | ori_ir::ast::FunctionSeq::Try { bindings, result, .. } => {
                    let _ = arena.get_seq_bindings(*bindings);
                    emit_expr(ctx, arena, *result)
                }
                ori_ir::ast::FunctionSeq::Match { scrutinee, .. } => {
                    emit_expr(ctx, arena, *scrutinee)
                }
                ori_ir::ast::FunctionSeq::ForPattern { over, .. } => {
                    emit_expr(ctx, arena, *over)
                }
            }
        }

        ExprKind::FunctionExp(exp) => {
            // Named expressions - context dependent
            let _ = arena.get_named_exprs(exp.props);
            "/* function_exp */0".to_string()
        }

        // Error placeholder
        ExprKind::Error => "/* error */0".to_string(),
    }
}

/// Emit a callee expression (the function being called).
///
/// This is separate from `emit_expr` because callees don't need ARC -
/// function references are just pointers that don't need retain/release.
fn emit_callee(
    ctx: &mut CodegenContext<'_>,
    arena: &ExprArena,
    id: ExprId,
) -> String {
    let expr = arena.get_expr(id);

    match &expr.kind {
        // Direct function name - just mangle it
        ExprKind::Ident(name) => ctx.mangle(*name),
        // Function reference - emit address
        ExprKind::FunctionRef(name) => format!("&{}", ctx.mangle(*name)),
        // Anything else (function pointers, etc.) - use regular emit_expr
        _ => emit_expr(ctx, arena, id),
    }
}

/// Emit a string literal as a ori_string_t.
fn emit_string_literal(s: &str) -> String {
    // Escape special characters
    let escaped: String = s
        .chars()
        .flat_map(|c| match c {
            '\\' => vec!['\\', '\\'],
            '"' => vec!['\\', '"'],
            '\n' => vec!['\\', 'n'],
            '\r' => vec!['\\', 'r'],
            '\t' => vec!['\\', 't'],
            '\0' => vec!['\\', '0'],
            c => vec![c],
        })
        .collect();

    format!("ori_string_from_cstr(\"{escaped}\")")
}

/// Emit a binary operation.
fn emit_binary_op(
    ctx: &CodegenContext<'_>,
    op: BinaryOp,
    left: &str,
    right: &str,
    left_id: ExprId,
) -> String {
    match op {
        // Arithmetic
        BinaryOp::Add => {
            // Check if this is string concatenation
            let left_type = ctx.expr_type(left_id);
            if left_type == ori_ir::TypeId::STR {
                format!("ori_string_concat({left}, {right})")
            } else {
                format!("({left} + {right})")
            }
        }
        BinaryOp::Sub => format!("({left} - {right})"),
        BinaryOp::Mul => format!("({left} * {right})"),
        BinaryOp::Div => format!("({left} / {right})"),
        BinaryOp::Mod => format!("({left} % {right})"),
        BinaryOp::FloorDiv => format!("ori_floor_div({left}, {right})"),

        // Comparison
        BinaryOp::Eq => {
            let left_type = ctx.expr_type(left_id);
            if left_type == ori_ir::TypeId::STR {
                format!("ori_string_eq({left}, {right})")
            } else {
                format!("({left} == {right})")
            }
        }
        BinaryOp::NotEq => {
            let left_type = ctx.expr_type(left_id);
            if left_type == ori_ir::TypeId::STR {
                format!("(!ori_string_eq({left}, {right}))")
            } else {
                format!("({left} != {right})")
            }
        }
        BinaryOp::Lt => format!("({left} < {right})"),
        BinaryOp::LtEq => format!("({left} <= {right})"),
        BinaryOp::Gt => format!("({left} > {right})"),
        BinaryOp::GtEq => format!("({left} >= {right})"),

        // Logical (short-circuit)
        BinaryOp::And => format!("({left} && {right})"),
        BinaryOp::Or => format!("({left} || {right})"),

        // Bitwise
        BinaryOp::BitAnd => format!("({left} & {right})"),
        BinaryOp::BitOr => format!("({left} | {right})"),
        BinaryOp::BitXor => format!("({left} ^ {right})"),
        BinaryOp::Shl => format!("({left} << {right})"),
        BinaryOp::Shr => format!("({left} >> {right})"),

        // Coalesce (Option/Result default)
        BinaryOp::Coalesce => {
            format!("(ORI_IS_SOME({left}) ? ORI_UNWRAP({left}) : {right})")
        }

        // Range operators
        BinaryOp::Range => format!("ori_range_new({left}, {right}, 0)"),
        BinaryOp::RangeInclusive => format!("ori_range_new({left}, {right}, 1)"),
    }
}

/// Emit a unary operation.
fn emit_unary_op(op: UnaryOp, operand: &str) -> String {
    match op {
        UnaryOp::Neg => format!("(-{operand})"),
        UnaryOp::Not => format!("(!{operand})"),
        UnaryOp::BitNot => format!("(~{operand})"),
        UnaryOp::Try => format!("({operand})"), // Try is handled elsewhere
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_literal() {
        assert_eq!(emit_string_literal("hello"), "ori_string_from_cstr(\"hello\")");
        assert_eq!(emit_string_literal("line\nbreak"), "ori_string_from_cstr(\"line\\nbreak\")");
    }
}
