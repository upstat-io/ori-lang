//! Field and index access resolution for struct and tuple types.
//!
//! Handles `.field` member access, numeric tuple access (`.0`, `.1`),
//! and field resolution for struct update expressions.

use ori_ir::{ExprArena, ExprId, Name, Span};
use rustc_hash::FxHashMap;

use crate::infer::InferEngine;
use crate::pool::substitute::substitute_named_in_pool;
use crate::{ContextKind, Expected, ExpectedOrigin, Idx, Tag, TypeCheckError, TypeKind};

use super::super::infer_expr;

/// Infer the type of a field access expression: `receiver.field`.
///
/// Resolves tuple indices, generic struct fields, and module namespaces.
/// Unresolved receivers remain deferred; unsupported receivers return `ERROR`
/// so method or namespace resolution can diagnose them later.
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

        // Why: A fresh variable preserves unresolved access for later unification.
        Tag::Var => engine.fresh_var(),

        // Spec: Clause 8 defines `Error.message` as `str`.
        Tag::Error => {
            let field_str = engine.lookup_name(field).unwrap_or("");
            match field_str {
                "message" => Idx::STR,
                _ => Idx::ERROR,
            }
        }

        // Why: Method and namespace resolution diagnose unsupported receivers later.
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
/// a case with a definitive missing-field diagnosis.
pub(crate) fn infer_struct_field(
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
        return Idx::ERROR;
    };

    let TypeKind::Struct(struct_def) = &entry.kind else {
        return Idx::ERROR;
    };

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

    if let Some(args) = type_args {
        if !entry.type_params.is_empty() && args.len() == entry.type_params.len() {
            let subst: FxHashMap<Name, Idx> = entry
                .type_params
                .iter()
                .zip(args.iter())
                .map(|(&param, &arg)| (param, arg))
                .collect();
            return substitute_named_in_pool(engine.pool_mut(), field_def.ty, &subst);
        }
    }

    field_def.ty
}

/// Look up all field types for a struct, with optional generic substitution.
///
/// Returns a `Name -> Idx` map of field types if the type is a known struct
/// in the registry. Returns `None` for unknown or non-struct types.
#[must_use = "the absence of a value must be handled"]
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
            substitute_named_in_pool(engine.pool_mut(), field.ty, subst)
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
    expr_id: ExprId,
    receiver: ExprId,
    index: ExprId,
    span: Span,
) -> Idx {
    // INVARIANT: Every successful route replaces this fail-closed dispatch entry.
    engine.record_index_dispatch(expr_id, crate::IndexDispatchSelection::Error);
    let receiver_ty = infer_expr(engine, arena, receiver);
    let index_ty = infer_expr(engine, arena, index);
    let resolved = engine.resolve(receiver_ty);

    match engine.pool().tag(resolved) {
        Tag::List => {
            engine.record_index_dispatch(expr_id, crate::IndexDispatchSelection::Builtin);
            let elem_ty = engine.pool().list_elem(resolved);
            check_index_key_type(engine, arena, index, index_ty, Idx::INT, span);
            elem_ty
        }

        Tag::Map => {
            engine.record_index_dispatch(expr_id, crate::IndexDispatchSelection::Builtin);
            let key_ty = engine.pool().map_key(resolved);
            let value_ty = engine.pool().map_value(resolved);
            check_index_key_type(engine, arena, index, index_ty, key_ty, span);
            engine.pool_mut().option(value_ty)
        }

        Tag::Str => {
            engine.record_index_dispatch(expr_id, crate::IndexDispatchSelection::Builtin);
            check_index_key_type(engine, arena, index, index_ty, Idx::INT, span);
            Idx::STR
        }

        Tag::Var => {
            engine.record_index_dispatch(expr_id, crate::IndexDispatchSelection::Deferred);
            engine.fresh_var()
        }

        Tag::Error => Idx::ERROR,

        _ => resolve_index_via_trait(engine, arena, expr_id, resolved, index_ty, index, span),
    }
}

fn check_index_key_type(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    index: ExprId,
    actual: Idx,
    required: Idx,
    span: Span,
) {
    let expected = Expected {
        ty: required,
        origin: ExpectedOrigin::Context {
            span,
            kind: ContextKind::IndexKey,
        },
    };
    let _checked_type = engine.check_type(actual, &expected, arena.get_expr(index).span);
}

#[derive(Clone)]
struct IndexCandidate {
    signature: Idx,
    producer: crate::MethodProducer,
    has_self: bool,
}

/// Resolve subscript indexing through `Index` trait dispatch.
///
/// Candidate data is detached from the registry borrow before mutable type
/// checking, then key compatibility disambiguates multiple implementations.
fn resolve_index_via_trait(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    expr_id: ExprId,
    receiver_ty: Idx,
    index_ty: Idx,
    index: ExprId,
    span: Span,
) -> Idx {
    let Some(name) = engine.intern_name("index") else {
        return Idx::ERROR;
    };

    // Why: The registered producer preserves the key-specific impl identity.
    let candidates: Vec<IndexCandidate> = {
        let Some(trait_registry) = engine.trait_registry() else {
            return Idx::ERROR;
        };
        trait_registry
            .indexed_impls_for_type(receiver_ty)
            .filter_map(|(impl_index, impl_entry)| {
                let method = impl_entry.methods.get(&name)?;
                let producer = trait_registry.method_producer(impl_index, method)?;
                Some(IndexCandidate {
                    signature: method.signature,
                    producer,
                    has_self: method.has_self,
                })
            })
            .collect()
    };

    if candidates.is_empty() {
        engine.push_error(TypeCheckError::not_indexable(span, receiver_ty));
        return Idx::ERROR;
    }

    if candidates.len() == 1 {
        return check_index_signature(
            engine,
            arena,
            expr_id,
            candidates[0].clone(),
            index_ty,
            index,
            span,
        );
    }

    let resolved_index = engine.resolve(index_ty);
    let index_tag = engine.pool().tag(resolved_index);

    let matching: Vec<IndexCandidate> = candidates
        .into_iter()
        .filter(|candidate| {
            let resolved_sig = engine.resolve(candidate.signature);
            if engine.pool().tag(resolved_sig) != Tag::Function {
                return false;
            }
            let params = engine.pool().function_params(resolved_sig);
            let skip = usize::from(candidate.has_self);
            let key_params = &params[skip..];
            if key_params.len() != 1 {
                return false;
            }
            let key_resolved = engine.resolve(key_params[0]);
            let key_tag = engine.pool().tag(key_resolved);
            key_tag == index_tag || key_tag == Tag::Var || index_tag == Tag::Var
        })
        .collect();

    match matching.len() {
        0 => {
            engine.push_error(TypeCheckError::not_indexable(span, receiver_ty));
            Idx::ERROR
        }
        1 => check_index_signature(
            engine,
            arena,
            expr_id,
            matching[0].clone(),
            index_ty,
            index,
            span,
        ),
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
    expr_id: ExprId,
    candidate: IndexCandidate,
    index_ty: Idx,
    index: ExprId,
    span: Span,
) -> Idx {
    let resolved_sig = engine.resolve(candidate.signature);
    if engine.pool().tag(resolved_sig) != Tag::Function {
        return Idx::ERROR;
    }

    let params = engine.pool().function_params(resolved_sig);
    let ret = engine.pool().function_return(resolved_sig);

    let skip = usize::from(candidate.has_self);
    let method_params = &params[skip..];

    if method_params.len() != 1 {
        return Idx::ERROR;
    }

    check_index_key_type(engine, arena, index, index_ty, method_params[0], span);

    engine.record_index_dispatch(
        expr_id,
        crate::IndexDispatchSelection::Selected(candidate.producer),
    );

    ret
}
