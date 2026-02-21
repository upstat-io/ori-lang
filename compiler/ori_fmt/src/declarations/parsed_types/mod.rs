//! Parsed Type Formatting
//!
//! Formatting for type expressions (`ParsedType`) used in function signatures,
//! type declarations, and other contexts.

use ori_ir::{ExprArena, ExprId, ExprKind, ParsedType, StringLookup, TypeId};

use crate::context::FormatContext;
use crate::emitter::Emitter;

/// Format a parsed type expression.
pub(crate) fn format_parsed_type<I: StringLookup, E: Emitter>(
    ty: &ParsedType,
    arena: &ExprArena,
    interner: &I,
    ctx: &mut FormatContext<E>,
) {
    match ty {
        ParsedType::Primitive(type_id) => {
            ctx.emit(type_id_to_str(*type_id));
        }
        ParsedType::Named { name, type_args } => {
            ctx.emit(interner.lookup(*name));
            let args = arena.get_parsed_type_list(*type_args);
            if !args.is_empty() {
                ctx.emit("<");
                for (i, arg_id) in args.iter().enumerate() {
                    if i > 0 {
                        ctx.emit(", ");
                    }
                    let arg = arena.get_parsed_type(*arg_id);
                    format_parsed_type(arg, arena, interner, ctx);
                }
                ctx.emit(">");
            }
        }
        ParsedType::List(elem) => {
            ctx.emit("[");
            let elem_ty = arena.get_parsed_type(*elem);
            format_parsed_type(elem_ty, arena, interner, ctx);
            ctx.emit("]");
        }
        ParsedType::FixedList { elem, capacity } => {
            ctx.emit("[");
            let elem_ty = arena.get_parsed_type(*elem);
            format_parsed_type(elem_ty, arena, interner, ctx);
            ctx.emit(", max ");
            format_const_expr(*capacity, arena, interner, ctx);
            ctx.emit("]");
        }
        ParsedType::Tuple(elems) => {
            ctx.emit("(");
            let elem_list = arena.get_parsed_type_list(*elems);
            for (i, elem_id) in elem_list.iter().enumerate() {
                if i > 0 {
                    ctx.emit(", ");
                }
                let elem = arena.get_parsed_type(*elem_id);
                format_parsed_type(elem, arena, interner, ctx);
            }
            ctx.emit(")");
        }
        ParsedType::Function { params, ret } => {
            ctx.emit("(");
            let param_list = arena.get_parsed_type_list(*params);
            for (i, param_id) in param_list.iter().enumerate() {
                if i > 0 {
                    ctx.emit(", ");
                }
                let param = arena.get_parsed_type(*param_id);
                format_parsed_type(param, arena, interner, ctx);
            }
            ctx.emit(") -> ");
            let ret_ty = arena.get_parsed_type(*ret);
            format_parsed_type(ret_ty, arena, interner, ctx);
        }
        ParsedType::Map { key, value } => {
            ctx.emit("{");
            let key_ty = arena.get_parsed_type(*key);
            format_parsed_type(key_ty, arena, interner, ctx);
            ctx.emit(": ");
            let value_ty = arena.get_parsed_type(*value);
            format_parsed_type(value_ty, arena, interner, ctx);
            ctx.emit("}");
        }
        ParsedType::Infer => {
            ctx.emit("_");
        }
        ParsedType::SelfType => {
            ctx.emit("Self");
        }
        ParsedType::AssociatedType { base, assoc_name } => {
            let base_ty = arena.get_parsed_type(*base);
            format_parsed_type(base_ty, arena, interner, ctx);
            ctx.emit(".");
            ctx.emit(interner.lookup(*assoc_name));
        }
        ParsedType::ConstExpr(expr_id) => {
            format_const_expr(*expr_id, arena, interner, ctx);
        }
        ParsedType::TraitBounds(bounds) => {
            let bound_ids = arena.get_parsed_type_list(*bounds);
            for (i, bound_id) in bound_ids.iter().enumerate() {
                if i > 0 {
                    ctx.emit(" + ");
                }
                let bound = arena.get_parsed_type(*bound_id);
                format_parsed_type(bound, arena, interner, ctx);
            }
        }
    }
}

/// Calculate the width of a parsed type expression.
pub(super) fn calculate_type_width<I: StringLookup>(
    ty: &ParsedType,
    arena: &ExprArena,
    interner: &I,
) -> usize {
    match ty {
        ParsedType::Primitive(type_id) => type_id_to_str(*type_id).len(),
        ParsedType::Named { name, type_args } => {
            let mut width = interner.lookup(*name).len();
            let args = arena.get_parsed_type_list(*type_args);
            if !args.is_empty() {
                width += 2; // "<>"
                for (i, arg_id) in args.iter().enumerate() {
                    if i > 0 {
                        width += 2; // ", "
                    }
                    let arg = arena.get_parsed_type(*arg_id);
                    width += calculate_type_width(arg, arena, interner);
                }
            }
            width
        }
        ParsedType::List(elem) => {
            let elem_ty = arena.get_parsed_type(*elem);
            2 + calculate_type_width(elem_ty, arena, interner) // "[]"
        }
        ParsedType::FixedList { elem, .. } => {
            let elem_ty = arena.get_parsed_type(*elem);
            // "[" + elem + ", max " + expr + "]" — estimate expr width as 10
            2 + calculate_type_width(elem_ty, arena, interner) + 6 + 10
        }
        ParsedType::Tuple(elems) => {
            let elem_list = arena.get_parsed_type_list(*elems);
            let mut width = 2; // "()"
            for (i, elem_id) in elem_list.iter().enumerate() {
                if i > 0 {
                    width += 2; // ", "
                }
                let elem = arena.get_parsed_type(*elem_id);
                width += calculate_type_width(elem, arena, interner);
            }
            width
        }
        ParsedType::Function { params, ret } => {
            let param_list = arena.get_parsed_type_list(*params);
            let mut width = 2; // "()"
            for (i, param_id) in param_list.iter().enumerate() {
                if i > 0 {
                    width += 2; // ", "
                }
                let param = arena.get_parsed_type(*param_id);
                width += calculate_type_width(param, arena, interner);
            }
            width += 4; // " -> "
            let ret_ty = arena.get_parsed_type(*ret);
            width += calculate_type_width(ret_ty, arena, interner);
            width
        }
        ParsedType::Map { key, value } => {
            let key_ty = arena.get_parsed_type(*key);
            let value_ty = arena.get_parsed_type(*value);
            2 + calculate_type_width(key_ty, arena, interner)
                + 2
                + calculate_type_width(value_ty, arena, interner)
            // "{" + key + ": " + value + "}"
        }
        ParsedType::Infer => 1,    // "_"
        ParsedType::SelfType => 4, // "Self"
        ParsedType::AssociatedType { base, assoc_name } => {
            let base_ty = arena.get_parsed_type(*base);
            calculate_type_width(base_ty, arena, interner) + 1 + interner.lookup(*assoc_name).len()
        }
        ParsedType::ConstExpr(_) => 10, // Estimate for const expressions
        ParsedType::TraitBounds(bounds) => {
            let bound_ids = arena.get_parsed_type_list(*bounds);
            let mut width = 0;
            for (i, bound_id) in bound_ids.iter().enumerate() {
                if i > 0 {
                    width += 3; // " + "
                }
                let bound = arena.get_parsed_type(*bound_id);
                width += calculate_type_width(bound, arena, interner);
            }
            width
        }
    }
}

/// Format a const expression (used in type positions like `$N`, `42`, `$N + 1`).
pub(crate) fn format_const_expr<I: StringLookup, E: Emitter>(
    expr_id: ExprId,
    arena: &ExprArena,
    interner: &I,
    ctx: &mut FormatContext<E>,
) {
    let expr = arena.get_expr(expr_id);
    match &expr.kind {
        ExprKind::Int(n) => ctx.emit(&n.to_string()),
        ExprKind::Const(name) => {
            ctx.emit("$");
            ctx.emit(interner.lookup(*name));
        }
        ExprKind::Ident(name) => ctx.emit(interner.lookup(*name)),
        ExprKind::Binary { op, left, right } => {
            format_const_expr(*left, arena, interner, ctx);
            ctx.emit(" ");
            ctx.emit(op.as_symbol());
            ctx.emit(" ");
            format_const_expr(*right, arena, interner, ctx);
        }
        _ => ctx.emit("<const>"),
    }
}

/// Convert a [`TypeId`] to its string representation.
pub(crate) fn type_id_to_str(id: TypeId) -> &'static str {
    match id {
        TypeId::INT => "int",
        TypeId::FLOAT => "float",
        TypeId::BOOL => "bool",
        TypeId::STR => "str",
        TypeId::CHAR => "char",
        TypeId::BYTE => "byte",
        TypeId::VOID => "void",
        TypeId::NEVER => "Never",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests;
