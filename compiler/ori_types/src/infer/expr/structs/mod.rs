//! Struct inference — struct literals, field access, and index access.

mod field_access;

pub(crate) use field_access::{
    infer_field, infer_index, infer_struct_field, lookup_struct_field_types,
};

use ori_ir::{ExprArena, Name, ParsedType, ParsedTypeId, Span};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::pool::substitute::substitute_named_in_pool;
use crate::{ContextKind, Expected, Idx, TypeCheckError, TypeKind};

use super::super::InferEngine;
use super::type_resolution::resolve_parsed_type;
use super::{find_similar_type_names, infer_expr, infer_ident};

/// Infer the value expressions of a field-init range for error recovery.
///
/// Called when a struct literal cannot be validated (no type registry, unknown
/// type name, or non-struct target) so each field value still surfaces its own
/// type errors before the caller returns `Idx::ERROR`.
fn infer_field_init_values(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    fields: ori_ir::FieldInitRange,
) {
    for init in arena.get_field_inits(fields) {
        if let Some(value_id) = init.value {
            infer_expr(engine, arena, value_id);
        }
    }
}

/// Infer the value expressions of a struct-literal (spread) field range for
/// error recovery.
///
/// Called when a spread struct literal cannot be validated (no type registry,
/// unknown type name, or non-struct target) so each field value and spread
/// expression still surfaces its own type errors before the caller returns
/// `Idx::ERROR`.
fn infer_struct_lit_field_values(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    fields: ori_ir::StructLitFieldRange,
) {
    for field in arena.get_struct_lit_fields(fields) {
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
}

/// Selects which `ContextKind` a provided-field check carries on its E2001
/// type-mismatch note: a struct-literal field vs a spread-override field.
enum FieldCheckContext {
    /// `Point { x: 1 }` — struct-literal field.
    StructLiteral,
    /// `{ ...base, x: 10 }` — spread-override field.
    SpreadUpdate,
}

/// The fixed context for checking each provided field of one struct literal:
/// the struct's name + registry index, its expected field set, and which
/// `ContextKind` the E2001 type-mismatch note carries. Constructed once per
/// struct literal; `check_field` runs per provided field.
struct ProvidedFieldCheck<'a> {
    name: Name,
    entry_idx: Idx,
    expected_fields: &'a [(Name, Idx)],
    expected_map: &'a FxHashMap<Name, Idx>,
    ctx: FieldCheckContext,
}

impl ProvidedFieldCheck<'_> {
    fn context_kind(&self, field_name: Name) -> ContextKind {
        match self.ctx {
            FieldCheckContext::StructLiteral => ContextKind::StructField {
                struct_name: self.name,
                field_name,
            },
            FieldCheckContext::SpreadUpdate => ContextKind::RecordUpdate { field_name },
        }
    }

    /// Check one provided field against its expected type, reporting a duplicate
    /// field, an unknown field, or a value/type mismatch. `provided_fields`
    /// accumulates the field names seen so far (for the duplicate check).
    fn check_field(
        &self,
        engine: &mut InferEngine<'_>,
        arena: &ExprArena,
        init: &ori_ir::FieldInit,
        provided_fields: &mut FxHashSet<Name>,
    ) {
        // Check for duplicate fields
        if !provided_fields.insert(init.name) {
            engine.push_error(TypeCheckError::duplicate_field(
                init.span, self.name, init.name,
            ));
            return;
        }

        if let Some(&expected_ty) = self.expected_map.get(&init.name) {
            // Known field — infer value and unify with expected type
            let actual_ty = if let Some(value_id) = init.value {
                infer_expr(engine, arena, value_id)
            } else {
                // Shorthand: `Point { x }` means `Point { x: x }`
                infer_ident(engine, init.name, init.span)
            };
            let _ = engine.check_type(
                actual_ty,
                &Expected::from_context(expected_ty, init.span, self.context_kind(init.name)),
                init.span,
            );
        } else {
            // Unknown field — report error, still infer value
            let available: Vec<Name> = self.expected_fields.iter().map(|(n, _)| *n).collect();
            engine.push_error(TypeCheckError::undefined_field(
                init.span,
                self.entry_idx,
                init.name,
                available,
            ));
            if let Some(value_id) = init.value {
                infer_expr(engine, arena, value_id);
            }
        }
    }
}

/// Check each provided field of a struct literal against its expected type,
/// reporting duplicate fields and unknown fields. Returns the set of field
/// names actually provided, for the caller's missing-field check.
fn check_provided_fields(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    name: Name,
    entry_idx: Idx,
    fields: ori_ir::FieldInitRange,
    expected_fields: &[(Name, Idx)],
    expected_map: &FxHashMap<Name, Idx>,
) -> FxHashSet<Name> {
    let field_inits = arena.get_field_inits(fields);
    let mut provided_fields: FxHashSet<Name> =
        FxHashSet::with_capacity_and_hasher(field_inits.len(), rustc_hash::FxBuildHasher);

    let check = ProvidedFieldCheck {
        name,
        entry_idx,
        expected_fields,
        expected_map,
        ctx: FieldCheckContext::StructLiteral,
    };
    for init in field_inits {
        check.check_field(engine, arena, init, &mut provided_fields);
    }

    provided_fields
}

/// Resolved registration data for a struct-literal target shared by
/// `infer_struct` and `infer_struct_spread`: the registry index, the
/// substituted expected field set, and the applied/named result type.
struct StructLiteralTarget {
    name: Name,
    entry_idx: Idx,
    expected_fields: Vec<(Name, Idx)>,
    expected_map: FxHashMap<Name, Idx>,
    target_type: Idx,
}

/// Resolve the struct-literal `type_path` head to its registered definition and
/// build the per-literal layout data: fresh type-parameter substitution,
/// substituted expected field types, and the applied/named result type.
///
/// A bare `Named` head resolves by name; a module-qualified `AssociatedType`
/// head (`module.Type`) resolves through the shared `resolve_parsed_type` SSOT
/// then back to its registry entry via `get_by_idx`, so a qualified path never
/// falls back to a same-named local type. Module-qualified type registration is
/// tracked by BUG-02-101; until it lands the qualified head misses cleanly.
///
/// Pushes the unknown-type-name / non-struct-target diagnostic and returns
/// `None` on a missing type registry, unknown name, or non-struct target. The
/// caller performs error-recovery inference over its own field range
/// (`FieldInitRange` vs `StructLitFieldRange`) on the `None` path.
fn resolve_struct_literal_target(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    type_path: ParsedTypeId,
    span: Span,
) -> Option<StructLiteralTarget> {
    // Step 1: Resolve the type-path head to its registered type entry.
    let entry = match arena.get_parsed_type(type_path) {
        ParsedType::Named { name, .. } => {
            let name = *name;
            let type_registry = engine.type_registry()?;
            let Some(entry) = type_registry.get_by_name(name).cloned() else {
                let similar = find_similar_type_names(engine, type_registry, name);
                engine.push_error(TypeCheckError::unknown_ident(span, name, similar));
                return None;
            };
            entry
        }
        ParsedType::AssociatedType { assoc_name, .. } => {
            let assoc_name = *assoc_name;
            let parsed = arena.get_parsed_type(type_path);
            let resolved = resolve_parsed_type(engine, arena, parsed);
            let Some(entry) = engine
                .type_registry()
                .and_then(|registry| registry.get_by_idx(resolved).cloned())
            else {
                engine.push_error(TypeCheckError::unknown_ident(span, assoc_name, Vec::new()));
                return None;
            };
            entry
        }
        _ => return None,
    };

    let name = entry.name;

    // Step 2: Verify it's a struct — move struct_def out of the already-owned entry
    let entry_idx = entry.idx;
    let type_params = entry.type_params;
    let TypeKind::Struct(struct_def) = entry.kind else {
        engine.push_error(TypeCheckError::not_a_struct(span, name));
        return None;
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
                substitute_named_in_pool(engine.pool_mut(), f.ty, &type_param_subst)
            };
            (f.name, ty)
        })
        .collect();

    let expected_map: FxHashMap<Name, Idx> = expected_fields.iter().copied().collect();

    // Step 5: Build the applied/named result type — the spread-unification target
    // and the struct literal's own type
    let target_type = if type_param_subst.is_empty() {
        engine.pool_mut().named(name)
    } else {
        let type_args: Vec<Idx> = type_params
            .iter()
            .map(|param_name| type_param_subst[param_name])
            .collect();
        engine.pool_mut().applied(name, &type_args)
    };

    Some(StructLiteralTarget {
        name,
        entry_idx,
        expected_fields,
        expected_map,
        target_type,
    })
}

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
    type_path: ParsedTypeId,
    fields: ori_ir::FieldInitRange,
    span: Span,
) -> Idx {
    // Resolve the struct target; on failure infer field values for error
    // recovery and bail with the error type.
    let Some(target) = resolve_struct_literal_target(engine, arena, type_path, span) else {
        infer_field_init_values(engine, arena, fields);
        return Idx::ERROR;
    };
    let name = target.name;

    // Check provided fields
    let provided_fields = check_provided_fields(
        engine,
        arena,
        name,
        target.entry_idx,
        fields,
        &target.expected_fields,
        &target.expected_map,
    );

    // Check for missing fields
    let missing: Vec<Name> = target
        .expected_fields
        .iter()
        .filter(|(field_name, _)| !provided_fields.contains(field_name))
        .map(|(field_name, _)| *field_name)
        .collect();

    if !missing.is_empty() {
        engine.push_error(TypeCheckError::missing_fields(span, name, missing));
    }

    target.target_type
}

/// Infer type for a struct literal with spread syntax: `Point { ...base, x: 10 }`.
pub(crate) fn infer_struct_spread(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    type_path: ParsedTypeId,
    fields: ori_ir::StructLitFieldRange,
    span: Span,
) -> Idx {
    let struct_lit_fields = arena.get_struct_lit_fields(fields);

    // Resolve the struct target; on failure infer field values (and spread
    // expressions) for error recovery and bail with the error type.
    let Some(target) = resolve_struct_literal_target(engine, arena, type_path, span) else {
        infer_struct_lit_field_values(engine, arena, fields);
        return Idx::ERROR;
    };
    let name = target.name;

    // Check provided fields
    let mut provided_fields: FxHashSet<Name> =
        FxHashSet::with_capacity_and_hasher(struct_lit_fields.len(), rustc_hash::FxBuildHasher);
    let mut has_spread = false;

    let check = ProvidedFieldCheck {
        name,
        entry_idx: target.entry_idx,
        expected_fields: &target.expected_fields,
        expected_map: &target.expected_map,
        ctx: FieldCheckContext::SpreadUpdate,
    };
    for field in struct_lit_fields {
        match field {
            ori_ir::StructLitField::Field(init) => {
                check.check_field(engine, arena, init, &mut provided_fields);
            }
            ori_ir::StructLitField::Spread { expr, span, .. } => {
                has_spread = true;
                let spread_ty = infer_expr(engine, arena, *expr);
                // Spread expression must be the same struct type
                let _ = engine.check_type(
                    spread_ty,
                    &Expected::from_context(
                        target.target_type,
                        *span,
                        ContextKind::StructConstruction { struct_name: name },
                    ),
                    *span,
                );
            }
        }
    }

    // Check for missing fields (only if no spread)
    if !has_spread {
        let missing: Vec<Name> = target
            .expected_fields
            .iter()
            .filter(|(field_name, _)| !provided_fields.contains(field_name))
            .map(|(field_name, _)| *field_name)
            .collect();

        if !missing.is_empty() {
            engine.push_error(TypeCheckError::missing_fields(span, name, missing));
        }
    }

    target.target_type
}
