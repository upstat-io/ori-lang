//! Binding pattern variable bindings.

use ori_ir::{BindingPattern, ExprArena, FieldBinding, Name};

use crate::infer::expr::structs::lookup_struct_field_types;
use crate::infer::InferEngine;
use crate::{Idx, Tag};

/// Bind a binding pattern to a type, introducing variables into scope.
pub(crate) fn bind_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    pattern: &BindingPattern,
    ty: Idx,
) {
    match pattern {
        BindingPattern::Name { name, mutable } => {
            engine.env_mut().bind_with_mutability(*name, ty, *mutable);
        }

        BindingPattern::Tuple(patterns) => {
            bind_tuple_pattern(engine, arena, patterns, ty);
        }

        BindingPattern::Struct { fields } => {
            bind_struct_pattern(engine, arena, fields, ty);
        }

        BindingPattern::List { elements, rest } => {
            bind_list_pattern(engine, arena, elements, *rest, ty);
        }

        BindingPattern::Wildcard => {}
    }
}

/// Binds tuple pattern variables against tuple element types, falling back
/// to fresh variables on type mismatch.
fn bind_tuple_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    patterns: &[BindingPattern],
    ty: Idx,
) {
    let resolved = engine.resolve(ty);
    if engine.pool().tag(resolved) == Tag::Tuple {
        let elem_types = engine.pool().tuple_elems(resolved);
        for (pat, elem_ty) in patterns.iter().zip(elem_types.iter()) {
            bind_pattern(engine, arena, pat, *elem_ty);
        }
    } else {
        // Why: Type mismatch.
        for pat in patterns {
            let var = engine.fresh_var();
            bind_pattern(engine, arena, pat, var);
        }
    }
}

/// Extracts struct field types from Named/Applied structures and binds field patterns.
fn bind_struct_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    fields: &[FieldBinding],
    ty: Idx,
) {
    let resolved = engine.resolve(ty);
    let field_type_map = match engine.pool().tag(resolved) {
        Tag::Named => {
            let type_name = engine.pool().named_name(resolved);
            lookup_struct_field_types(engine, type_name, None)
        }

        Tag::Applied => {
            let type_name = engine.pool().applied_name(resolved);
            let type_args = engine.pool().applied_args(resolved);
            lookup_struct_field_types(engine, type_name, Some(&type_args))
        }

        _ => None,
    };

    for field in fields {
        let field_ty = field_type_map
            .as_ref()
            .and_then(|m| m.get(&field.name).copied())
            .unwrap_or_else(|| engine.fresh_var());
        if let Some(sub_pat) = &field.pattern {
            bind_pattern(engine, arena, sub_pat, field_ty);
        } else {
            // Why: Shorthand: { x } or { $x } — use field's own mutability
            engine
                .env_mut()
                .bind_with_mutability(field.name, field_ty, field.mutable);
        }
    }
}

/// Recursively binds list pattern elements to the collection element type.
fn bind_list_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    elements: &[BindingPattern],
    rest: Option<(Name, ori_ir::Mutability)>,
    ty: Idx,
) {
    let resolved = engine.resolve(ty);
    if engine.pool().tag(resolved) == Tag::List {
        let elem_ty = engine.pool().list_elem(resolved);
        for pat in elements {
            bind_pattern(engine, arena, pat, elem_ty);
        }
        if let Some((rest_name, rest_mut)) = rest {
            // Why: Rest binding gets the full list type, respecting $ mutability
            engine
                .env_mut()
                .bind_with_mutability(rest_name, ty, rest_mut);
        }
    } else {
        // Why: Type mismatch.
        for pat in elements {
            let var = engine.fresh_var();
            bind_pattern(engine, arena, pat, var);
        }
        if let Some((rest_name, rest_mut)) = rest {
            engine
                .env_mut()
                .bind_with_mutability(rest_name, ty, rest_mut);
        }
    }
}
