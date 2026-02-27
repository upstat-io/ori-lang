//! Field and index access resolution for struct and tuple types.
//!
//! Handles `.field` member access, numeric tuple access (`.0`, `.1`),
//! and field resolution for struct update expressions.

use ori_ir::{ExprArena, ExprId, Name, Span};
use rustc_hash::FxHashMap;

use super::super::infer_expr;
use super::substitute_named_types;
use crate::infer::InferEngine;
use crate::{ContextKind, Expected, ExpectedOrigin, Idx, Tag, TypeCheckError, TypeKind};

/// Infer the type of a field access expression: `receiver.field`.
///
/// Handles:
/// - Tuple field access by numeric index (`.0`, `.1`, etc.)
/// - Struct field access by name (`.x`, `.name`)
/// - Generic struct field access with type parameter substitution
/// - Module namespace access (`Counter.new`)
///
/// For unresolved type variables, returns a fresh variable to defer resolution.
/// For error types, propagates ERROR silently. For types where field access
/// is genuinely unsupported (primitives, functions, etc.), returns ERROR
/// without reporting an error — method resolution may handle these separately.
pub(crate) fn infer_field(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver: ExprId,
    field: Name,
    span: Span,
) -> Idx {
    let receiver_ty = infer_expr(engine, arena, receiver);
    let resolved = engine.resolve(receiver_ty);

    match engine.pool().tag(resolved) {
        Tag::Tuple => {
            // Tuple field access: `.0`, `.1`, etc.
            let Some(field_str) = engine.lookup_name(field) else {
                return Idx::ERROR;
            };
            if let Ok(index) = field_str.parse::<usize>() {
                let elems = engine.pool().tuple_elems(resolved);
                if index < elems.len() {
                    elems[index]
                } else {
                    engine.push_error(TypeCheckError::undefined_field(
                        span,
                        resolved,
                        field,
                        vec![],
                    ));
                    Idx::ERROR
                }
            } else {
                engine.push_error(TypeCheckError::undefined_field(
                    span,
                    resolved,
                    field,
                    vec![],
                ));
                Idx::ERROR
            }
        }

        Tag::Named => {
            let type_name = engine.pool().named_name(resolved);
            infer_struct_field(engine, type_name, None, field, span)
        }

        Tag::Applied => {
            let type_name = engine.pool().applied_name(resolved);
            let type_args = engine.pool().applied_args(resolved);
            infer_struct_field(engine, type_name, Some(type_args), field, span)
        }

        // Unresolved type variable — return fresh var to defer resolution
        // (following V1 pattern: the actual field type will be resolved later)
        Tag::Var => engine.fresh_var(),

        // Error type has field-like accessors (spec §6: Error = { message: str })
        Tag::Error => {
            let field_str = engine.lookup_name(field).unwrap_or("");
            match field_str {
                "message" => Idx::STR,
                _ => Idx::ERROR,
            }
        }

        // Unsupported types for field access — return ERROR silently.
        // Don't report errors here since module namespace access
        // (e.g., `Counter.new`) and other patterns may reach this point
        // and would require method/namespace resolution to diagnose properly.
        _ => Idx::ERROR,
    }
}

/// Look up a field on a struct type, with optional type argument substitution.
///
/// For types not in the registry or non-struct types, returns ERROR silently.
/// This avoids false positives for imported types or types that aren't yet
/// fully registered (e.g., from other modules).
///
/// Only reports errors when the struct is known but the field doesn't exist —
/// a case where we can give a definitive, useful error message.
fn infer_struct_field(
    engine: &mut InferEngine<'_>,
    type_name: Name,
    type_args: Option<Vec<Idx>>,
    field: Name,
    span: Span,
) -> Idx {
    let Some(type_registry) = engine.type_registry() else {
        return Idx::ERROR;
    };

    let Some(entry) = type_registry.get_by_name(type_name).cloned() else {
        return Idx::ERROR; // Not registered — likely imported
    };

    let TypeKind::Struct(struct_def) = &entry.kind else {
        return Idx::ERROR; // Enum/newtype/alias — not a struct
    };

    // Find the field
    let Some(field_def) = struct_def.fields.iter().find(|f| f.name == field).cloned() else {
        let available: Vec<Name> = struct_def.fields.iter().map(|f| f.name).collect();
        let receiver_idx = engine.pool_mut().named(type_name);
        engine.push_error(TypeCheckError::undefined_field(
            span,
            receiver_idx,
            field,
            available,
        ));
        return Idx::ERROR;
    };

    // Substitute type parameters for generic structs
    if let Some(args) = type_args {
        if !entry.type_params.is_empty() && args.len() == entry.type_params.len() {
            let subst: FxHashMap<Name, Idx> = entry
                .type_params
                .iter()
                .zip(args.iter())
                .map(|(&param, &arg)| (param, arg))
                .collect();
            return substitute_named_types(engine.pool_mut(), field_def.ty, &subst);
        }
    }

    field_def.ty
}

/// Look up all field types for a struct, with optional generic substitution.
///
/// Returns a `Name -> Idx` map of field types if the type is a known struct
/// in the registry. Returns `None` for unknown or non-struct types.
pub(crate) fn lookup_struct_field_types(
    engine: &mut InferEngine<'_>,
    type_name: Name,
    type_args: Option<&[Idx]>,
) -> Option<FxHashMap<Name, Idx>> {
    let type_registry = engine.type_registry()?;
    let entry = type_registry.get_by_name(type_name)?.clone();

    let TypeKind::Struct(struct_def) = &entry.kind else {
        return None;
    };

    let subst: Option<FxHashMap<Name, Idx>> = type_args.and_then(|args| {
        if !entry.type_params.is_empty() && args.len() == entry.type_params.len() {
            Some(
                entry
                    .type_params
                    .iter()
                    .zip(args.iter())
                    .map(|(&param, &arg)| (param, arg))
                    .collect(),
            )
        } else {
            None
        }
    });

    let mut field_types = FxHashMap::default();
    for field in &struct_def.fields {
        let ty = if let Some(ref subst) = subst {
            substitute_named_types(engine.pool_mut(), field.ty, subst)
        } else {
            field.ty
        };
        field_types.insert(field.name, ty);
    }
    Some(field_types)
}

/// Infer the type of an index access expression (e.g., `list[0]`, `map["key"]`).
///
/// Validates that the receiver is indexable and the index type matches:
/// - `[T]` indexed by `int` returns `T`
/// - `Map<K, V>` indexed by `K` returns `Option<V>`
/// - `str` indexed by `int` returns `str`
/// - User-defined types: dispatches via `Index<Key, Value>` trait
pub(crate) fn infer_index(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver: ExprId,
    index: ExprId,
    span: Span,
) -> Idx {
    let receiver_ty = infer_expr(engine, arena, receiver);
    let index_ty = infer_expr(engine, arena, index);
    let resolved = engine.resolve(receiver_ty);

    match engine.pool().tag(resolved) {
        Tag::List => {
            let elem_ty = engine.pool().list_elem(resolved);
            let _ = engine.unify_types(index_ty, Idx::INT);
            elem_ty
        }

        Tag::Map => {
            let key_ty = engine.pool().map_key(resolved);
            let value_ty = engine.pool().map_value(resolved);
            let _ = engine.unify_types(index_ty, key_ty);
            // Map indexing returns Option<V>
            engine.pool_mut().option(value_ty)
        }

        Tag::Str => {
            let _ = engine.unify_types(index_ty, Idx::INT);
            Idx::STR
        }

        // Unresolved type variable — return fresh var
        Tag::Var => engine.fresh_var(),

        // Error type — propagate silently
        Tag::Error => Idx::ERROR,

        // All other types: try Index trait dispatch
        _ => resolve_index_via_trait(engine, arena, resolved, index_ty, index, span),
    }
}

/// Try to resolve subscript indexing via `Index` trait dispatch.
///
/// Iterates all `Index` trait impls for the receiver type and filters by
/// key type compatibility. This handles the case where a type implements
/// `Index` for multiple key types (e.g., `Index<int, V>` + `Index<str, V>`).
///
/// Follows the borrow-dance pattern: scope the `trait_registry()` borrow
/// to extract candidate data, then use engine mutably for type checking.
fn resolve_index_via_trait(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver_ty: Idx,
    index_ty: Idx,
    index: ExprId,
    span: Span,
) -> Idx {
    let Some(name) = engine.intern_name("index") else {
        return Idx::ERROR;
    };

    // Scoped borrow: collect all Index impl candidates (signature, has_self).
    let candidates: Vec<(Idx, bool)> = {
        let Some(trait_registry) = engine.trait_registry() else {
            return Idx::ERROR;
        };
        trait_registry
            .impls_for_type(receiver_ty)
            .filter_map(|impl_entry| {
                let method = impl_entry.methods.get(&name)?;
                Some((method.signature, method.has_self))
            })
            .collect()
    };

    if candidates.is_empty() {
        engine.push_error(TypeCheckError::not_indexable(span, receiver_ty));
        return Idx::ERROR;
    }

    // Single candidate — use directly without key-type filtering
    if candidates.len() == 1 {
        return check_index_signature(engine, arena, candidates[0], index_ty, index, span);
    }

    // Multiple candidates — disambiguate by matching key type tags.
    let resolved_index = engine.resolve(index_ty);
    let index_tag = engine.pool().tag(resolved_index);

    let matching: Vec<(Idx, bool)> = candidates
        .into_iter()
        .filter(|&(sig_ty, has_self)| {
            let resolved_sig = engine.resolve(sig_ty);
            if engine.pool().tag(resolved_sig) != Tag::Function {
                return false;
            }
            let params = engine.pool().function_params(resolved_sig);
            let skip = usize::from(has_self);
            let key_params = &params[skip..];
            if key_params.len() != 1 {
                return false;
            }
            let key_resolved = engine.resolve(key_params[0]);
            let key_tag = engine.pool().tag(key_resolved);
            // Match if key tags equal, or if either is a type variable (deferred)
            key_tag == index_tag || key_tag == Tag::Var || index_tag == Tag::Var
        })
        .collect();

    match matching.len() {
        0 => {
            engine.push_error(TypeCheckError::not_indexable(span, receiver_ty));
            Idx::ERROR
        }
        1 => check_index_signature(engine, arena, matching[0], index_ty, index, span),
        _ => {
            engine.push_error(TypeCheckError::ambiguous_index(span, receiver_ty));
            Idx::ERROR
        }
    }
}

/// Check the signature of a resolved Index method against the index expression.
///
/// Validates the method signature is a function with exactly one non-self
/// parameter (the key), unifies the key type with the index expression type,
/// and returns the method's return type.
fn check_index_signature(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    candidate: (Idx, bool),
    index_ty: Idx,
    index: ExprId,
    span: Span,
) -> Idx {
    let (sig_ty, has_self) = candidate;
    let resolved_sig = engine.resolve(sig_ty);
    if engine.pool().tag(resolved_sig) != Tag::Function {
        return Idx::ERROR;
    }

    let params = engine.pool().function_params(resolved_sig);
    let ret = engine.pool().function_return(resolved_sig);

    let skip = usize::from(has_self);
    let method_params = &params[skip..];

    if method_params.len() != 1 {
        return Idx::ERROR;
    }

    let expected = Expected {
        ty: method_params[0],
        origin: ExpectedOrigin::Context {
            span,
            kind: ContextKind::IndexKey,
        },
    };
    let _ = engine.check_type(index_ty, &expected, arena.get_expr(index).span);

    ret
}
