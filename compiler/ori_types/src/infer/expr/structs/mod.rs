//! Struct inference — struct literals, field access, and index access.

mod field_access;

pub(crate) use field_access::{infer_field, infer_index, lookup_struct_field_types};

use ori_ir::{ExprArena, Name, Span};
use rustc_hash::{FxHashMap, FxHashSet};

use super::super::InferEngine;
use super::{find_similar_type_names, infer_expr, infer_ident};
use crate::{Idx, Pool, Tag, TypeCheckError, TypeKind};

/// Infer type for a struct literal: `Point { x: 1, y: 2 }`.
///
/// Performs:
/// 1. Type registry lookup to find the struct definition
/// 2. Fresh type variable creation for generic type parameters
/// 3. Type parameter substitution in field types
/// 4. Field validation (unknown fields, duplicate fields, missing fields)
/// 5. Unification of provided field values with expected field types
pub(crate) fn infer_struct(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    name: Name,
    fields: ori_ir::FieldInitRange,
    span: Span,
) -> Idx {
    // Step 1: Look up the struct type in the registry
    let Some(type_registry) = engine.type_registry() else {
        // No type registry — infer field values but can't validate
        let field_inits = arena.get_field_inits(fields);
        for init in field_inits {
            if let Some(value_id) = init.value {
                infer_expr(engine, arena, value_id);
            }
        }
        return Idx::ERROR;
    };

    let Some(entry) = type_registry.get_by_name(name).cloned() else {
        // Unknown type name — find similar type names for suggestions
        let similar = find_similar_type_names(engine, type_registry, name);
        engine.push_error(TypeCheckError::unknown_ident(span, name, similar));
        let field_inits = arena.get_field_inits(fields);
        for init in field_inits {
            if let Some(value_id) = init.value {
                infer_expr(engine, arena, value_id);
            }
        }
        return Idx::ERROR;
    };

    // Step 2: Verify it's a struct — move struct_def out of the already-owned entry
    let entry_idx = entry.idx;
    let type_params = entry.type_params;
    let TypeKind::Struct(struct_def) = entry.kind else {
        engine.push_error(TypeCheckError::not_a_struct(span, name));
        let field_inits = arena.get_field_inits(fields);
        for init in field_inits {
            if let Some(value_id) = init.value {
                infer_expr(engine, arena, value_id);
            }
        }
        return Idx::ERROR;
    };

    // Step 3: Create fresh type variables for generic params
    let type_param_subst: FxHashMap<Name, Idx> = type_params
        .iter()
        .map(|&param_name| (param_name, engine.fresh_var()))
        .collect();

    // Step 4: Build expected field types with substitution
    let expected_fields: Vec<(Name, Idx)> = struct_def
        .fields
        .iter()
        .map(|f| {
            let ty = if type_param_subst.is_empty() {
                f.ty
            } else {
                substitute_named_types(engine.pool_mut(), f.ty, &type_param_subst)
            };
            (f.name, ty)
        })
        .collect();

    let expected_map: FxHashMap<Name, Idx> = expected_fields.iter().copied().collect();

    // Step 5: Check provided fields
    let field_inits = arena.get_field_inits(fields);
    let mut provided_fields: FxHashSet<Name> =
        FxHashSet::with_capacity_and_hasher(field_inits.len(), rustc_hash::FxBuildHasher);

    for init in field_inits {
        // Check for duplicate fields
        if !provided_fields.insert(init.name) {
            engine.push_error(TypeCheckError::duplicate_field(init.span, name, init.name));
            continue;
        }

        if let Some(&expected_ty) = expected_map.get(&init.name) {
            // Known field — infer value and unify with expected type
            let actual_ty = if let Some(value_id) = init.value {
                infer_expr(engine, arena, value_id)
            } else {
                // Shorthand: `Point { x }` means `Point { x: x }`
                infer_ident(engine, init.name, init.span)
            };
            let _ = engine.unify_types(actual_ty, expected_ty);
        } else {
            // Unknown field — report error, still infer value
            let available: Vec<Name> = expected_fields.iter().map(|(n, _)| *n).collect();
            engine.push_error(TypeCheckError::undefined_field(
                init.span, entry_idx, init.name, available,
            ));
            if let Some(value_id) = init.value {
                infer_expr(engine, arena, value_id);
            }
        }
    }

    // Step 6: Check for missing fields
    let missing: Vec<Name> = expected_fields
        .iter()
        .filter(|(field_name, _)| !provided_fields.contains(field_name))
        .map(|(field_name, _)| *field_name)
        .collect();

    if !missing.is_empty() {
        engine.push_error(TypeCheckError::missing_fields(span, name, missing));
    }

    // Step 7: Return the struct type
    if type_param_subst.is_empty() {
        engine.pool_mut().named(name)
    } else {
        let type_args: Vec<Idx> = type_params
            .iter()
            .map(|param_name| type_param_subst[param_name])
            .collect();
        engine.pool_mut().applied(name, &type_args)
    }
}

/// Infer type for a struct literal with spread syntax: `Point { ...base, x: 10 }`.
#[expect(
    clippy::too_many_lines,
    reason = "multi-step struct spread inference: lookup, field validation, spread resolution, and type checking"
)]
pub(crate) fn infer_struct_spread(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    name: Name,
    fields: ori_ir::StructLitFieldRange,
    span: Span,
) -> Idx {
    let struct_lit_fields = arena.get_struct_lit_fields(fields);

    // Step 1: Look up the struct type in the registry
    let Some(type_registry) = engine.type_registry() else {
        for field in struct_lit_fields {
            match field {
                ori_ir::StructLitField::Field(init) => {
                    if let Some(value_id) = init.value {
                        infer_expr(engine, arena, value_id);
                    }
                }
                ori_ir::StructLitField::Spread { expr, .. } => {
                    infer_expr(engine, arena, *expr);
                }
            }
        }
        return Idx::ERROR;
    };

    let Some(entry) = type_registry.get_by_name(name).cloned() else {
        // Unknown type name — find similar type names for suggestions
        let similar = find_similar_type_names(engine, type_registry, name);
        engine.push_error(TypeCheckError::unknown_ident(span, name, similar));
        for field in struct_lit_fields {
            match field {
                ori_ir::StructLitField::Field(init) => {
                    if let Some(value_id) = init.value {
                        infer_expr(engine, arena, value_id);
                    }
                }
                ori_ir::StructLitField::Spread { expr, .. } => {
                    infer_expr(engine, arena, *expr);
                }
            }
        }
        return Idx::ERROR;
    };

    // Extract scalar fields before moving kind out of the owned entry
    let entry_idx = entry.idx;
    let type_params = entry.type_params;
    let TypeKind::Struct(struct_def) = entry.kind else {
        engine.push_error(TypeCheckError::not_a_struct(span, name));
        for field in struct_lit_fields {
            match field {
                ori_ir::StructLitField::Field(init) => {
                    if let Some(value_id) = init.value {
                        infer_expr(engine, arena, value_id);
                    }
                }
                ori_ir::StructLitField::Spread { expr, .. } => {
                    infer_expr(engine, arena, *expr);
                }
            }
        }
        return Idx::ERROR;
    };

    // Step 2: Create fresh type variables for generic params
    let type_param_subst: FxHashMap<Name, Idx> = type_params
        .iter()
        .map(|&param_name| (param_name, engine.fresh_var()))
        .collect();

    // Step 3: Build expected field types with substitution
    let expected_fields: Vec<(Name, Idx)> = struct_def
        .fields
        .iter()
        .map(|f| {
            let ty = if type_param_subst.is_empty() {
                f.ty
            } else {
                substitute_named_types(engine.pool_mut(), f.ty, &type_param_subst)
            };
            (f.name, ty)
        })
        .collect();

    let expected_map: FxHashMap<Name, Idx> = expected_fields.iter().copied().collect();

    // Build the target type for spread unification
    let target_type = if type_param_subst.is_empty() {
        engine.pool_mut().named(name)
    } else {
        let type_args: Vec<Idx> = type_params
            .iter()
            .map(|param_name| type_param_subst[param_name])
            .collect();
        engine.pool_mut().applied(name, &type_args)
    };

    // Step 4: Check provided fields
    let mut provided_fields: FxHashSet<Name> =
        FxHashSet::with_capacity_and_hasher(struct_lit_fields.len(), rustc_hash::FxBuildHasher);
    let mut has_spread = false;

    for field in struct_lit_fields {
        match field {
            ori_ir::StructLitField::Field(init) => {
                if !provided_fields.insert(init.name) {
                    engine.push_error(TypeCheckError::duplicate_field(init.span, name, init.name));
                    continue;
                }

                if let Some(&expected_ty) = expected_map.get(&init.name) {
                    let actual_ty = if let Some(value_id) = init.value {
                        infer_expr(engine, arena, value_id)
                    } else {
                        infer_ident(engine, init.name, init.span)
                    };
                    let _ = engine.unify_types(actual_ty, expected_ty);
                } else {
                    let available: Vec<Name> = expected_fields.iter().map(|(n, _)| *n).collect();
                    engine.push_error(TypeCheckError::undefined_field(
                        init.span, entry_idx, init.name, available,
                    ));
                    if let Some(value_id) = init.value {
                        infer_expr(engine, arena, value_id);
                    }
                }
            }
            ori_ir::StructLitField::Spread { expr, .. } => {
                has_spread = true;
                let spread_ty = infer_expr(engine, arena, *expr);
                // Spread expression must be the same struct type
                let _ = engine.unify_types(spread_ty, target_type);
            }
        }
    }

    // Step 5: Check for missing fields (only if no spread)
    if !has_spread {
        let missing: Vec<Name> = expected_fields
            .iter()
            .filter(|(field_name, _)| !provided_fields.contains(field_name))
            .map(|(field_name, _)| *field_name)
            .collect();

        if !missing.is_empty() {
            engine.push_error(TypeCheckError::missing_fields(span, name, missing));
        }
    }

    target_type
}

/// Substitute Named types that match type parameter names with replacement types.
///
/// Walks the pool type structure recursively. For a generic struct `type Box<T> = { value: T }`,
/// field type `Named(T)` is replaced with the fresh type variable allocated for T.
#[expect(
    clippy::too_many_lines,
    reason = "exhaustive Tag-based type substitution"
)]
pub(crate) fn substitute_named_types(
    pool: &mut Pool,
    ty: Idx,
    subst: &FxHashMap<Name, Idx>,
) -> Idx {
    match pool.tag(ty) {
        Tag::Named => {
            let name = pool.named_name(ty);
            if let Some(&replacement) = subst.get(&name) {
                replacement
            } else {
                ty
            }
        }

        Tag::List => {
            let elem = pool.list_elem(ty);
            let new_elem = substitute_named_types(pool, elem, subst);
            if new_elem == elem {
                ty
            } else {
                pool.list(new_elem)
            }
        }

        Tag::Option => {
            let elem = pool.option_inner(ty);
            let new_elem = substitute_named_types(pool, elem, subst);
            if new_elem == elem {
                ty
            } else {
                pool.option(new_elem)
            }
        }

        Tag::Set => {
            let elem = pool.set_elem(ty);
            let new_elem = substitute_named_types(pool, elem, subst);
            if new_elem == elem {
                ty
            } else {
                pool.set(new_elem)
            }
        }

        Tag::Channel => {
            let elem = pool.channel_elem(ty);
            let new_elem = substitute_named_types(pool, elem, subst);
            if new_elem == elem {
                ty
            } else {
                pool.channel(new_elem)
            }
        }

        Tag::Range => {
            let elem = pool.range_elem(ty);
            let new_elem = substitute_named_types(pool, elem, subst);
            if new_elem == elem {
                ty
            } else {
                pool.range(new_elem)
            }
        }

        Tag::Map => {
            let key = pool.map_key(ty);
            let value = pool.map_value(ty);
            let new_key = substitute_named_types(pool, key, subst);
            let new_value = substitute_named_types(pool, value, subst);
            if new_key == key && new_value == value {
                ty
            } else {
                pool.map(new_key, new_value)
            }
        }

        Tag::Result => {
            let ok = pool.result_ok(ty);
            let err = pool.result_err(ty);
            let new_ok = substitute_named_types(pool, ok, subst);
            let new_err = substitute_named_types(pool, err, subst);
            if new_ok == ok && new_err == err {
                ty
            } else {
                pool.result(new_ok, new_err)
            }
        }

        Tag::Function => {
            let params = pool.function_params(ty);
            let ret = pool.function_return(ty);

            let mut changed = false;
            let new_params: Vec<Idx> = params
                .iter()
                .map(|&p| {
                    let new_p = substitute_named_types(pool, p, subst);
                    if new_p != p {
                        changed = true;
                    }
                    new_p
                })
                .collect();

            let new_ret = substitute_named_types(pool, ret, subst);
            if new_ret != ret {
                changed = true;
            }

            if changed {
                pool.function(&new_params, new_ret)
            } else {
                ty
            }
        }

        Tag::Tuple => {
            let elems = pool.tuple_elems(ty);

            let mut changed = false;
            let new_elems: Vec<Idx> = elems
                .iter()
                .map(|&e| {
                    let new_e = substitute_named_types(pool, e, subst);
                    if new_e != e {
                        changed = true;
                    }
                    new_e
                })
                .collect();

            if changed {
                pool.tuple(&new_elems)
            } else {
                ty
            }
        }

        Tag::Applied => {
            let app_name = pool.applied_name(ty);
            let args = pool.applied_args(ty);

            let mut changed = false;
            let new_args: Vec<Idx> = args
                .iter()
                .map(|&a| {
                    let new_a = substitute_named_types(pool, a, subst);
                    if new_a != a {
                        changed = true;
                    }
                    new_a
                })
                .collect();

            if changed {
                pool.applied(app_name, &new_args)
            } else {
                ty
            }
        }

        // Primitives, Error, Var, BoundVar, RigidVar, etc. — no substitution needed
        _ => ty,
    }
}
