//! Struct literal type inference and field lookup helpers.

use super::super::infer_expr;
use super::identifiers::infer_ident;
use super::substitute_type_params;
use crate::checker::TypeChecker;
use crate::registry::TypeKind;
use ori_ir::{FieldInitRange, Name, Span};
use ori_types::Type;
use std::collections::{HashMap, HashSet};

/// Result of looking up a struct field.
pub(super) enum FieldLookupResult {
    /// Field found with resolved type.
    Found(Type),
    /// Type is not a struct (is an enum).
    NotStruct,
    /// Field not found in struct.
    NoSuchField,
    /// Type alias (should have been resolved).
    Alias,
}

/// Look up a field in a struct type, optionally substituting type parameters.
pub(super) fn lookup_struct_field_in_entry(
    entry: &crate::registry::TypeEntry,
    field: Name,
    type_args: Option<&[Type]>,
    registry: &crate::registry::TypeRegistry,
) -> FieldLookupResult {
    match &entry.kind {
        TypeKind::Struct { fields } => {
            // Build type param map if we have type arguments
            let type_param_map: Option<HashMap<Name, Type>> = type_args.map(|args| {
                entry
                    .type_params
                    .iter()
                    .zip(args.iter())
                    .map(|(&param_name, arg)| (param_name, arg.clone()))
                    .collect()
            });

            let interner = registry.interner();
            for (field_name, field_ty_id) in fields {
                if *field_name == field {
                    let field_ty = interner.to_type(*field_ty_id);
                    let result_ty = match &type_param_map {
                        Some(map) => substitute_type_params(&field_ty, map),
                        None => field_ty,
                    };
                    return FieldLookupResult::Found(result_ty);
                }
            }
            FieldLookupResult::NoSuchField
        }
        TypeKind::Enum { .. } => FieldLookupResult::NotStruct,
        TypeKind::Alias { .. } => FieldLookupResult::Alias,
    }
}

/// Handle field access on a named or applied struct type.
pub(super) fn handle_struct_field_access(
    checker: &mut TypeChecker<'_>,
    type_name: Name,
    field: Name,
    type_args: Option<&[Type]>,
    span: Span,
) -> Type {
    let Some(entry) = checker.registries.types.get_by_name(type_name) else {
        checker.push_error(
            format!(
                "unknown type `{}`",
                checker.context.interner.lookup(type_name)
            ),
            span,
            ori_diagnostic::ErrorCode::E2003,
        );
        return Type::Error;
    };
    let entry = entry.clone();

    match lookup_struct_field_in_entry(&entry, field, type_args, &checker.registries.types) {
        FieldLookupResult::Found(ty) => ty,
        FieldLookupResult::NoSuchField => {
            checker.push_error(
                format!(
                    "struct `{}` has no field `{}`",
                    checker.context.interner.lookup(type_name),
                    checker.context.interner.lookup(field)
                ),
                span,
                ori_diagnostic::ErrorCode::E2001,
            );
            Type::Error
        }
        FieldLookupResult::NotStruct => {
            checker.push_error(
                format!(
                    "cannot access field `{}` on enum type `{}`",
                    checker.context.interner.lookup(field),
                    checker.context.interner.lookup(type_name)
                ),
                span,
                ori_diagnostic::ErrorCode::E2001,
            );
            Type::Error
        }
        FieldLookupResult::Alias => Type::Error,
    }
}

/// Infer type for a struct literal.
pub fn infer_struct(checker: &mut TypeChecker<'_>, name: Name, fields: FieldInitRange) -> Type {
    let type_entry = if let Some(entry) = checker.registries.types.get_by_name(name) {
        entry.clone()
    } else {
        let field_inits = checker.context.arena.get_field_inits(fields);
        let span = if let Some(first) = field_inits.first() {
            first.span
        } else {
            ori_ir::Span::new(0, 0)
        };

        checker.push_error(
            format!(
                "unknown struct type `{}`",
                checker.context.interner.lookup(name)
            ),
            span,
            ori_diagnostic::ErrorCode::E2003,
        );

        for init in field_inits {
            if let Some(value_id) = init.value {
                infer_expr(checker, value_id);
            }
        }
        return Type::Error;
    };

    // Get struct fields as TypeId, then convert to Type
    let expected_fields: Vec<(Name, Type)> = if let TypeKind::Struct { fields } = &type_entry.kind {
        let interner = checker.registries.types.interner();
        fields
            .iter()
            .map(|(name, ty_id)| (*name, interner.to_type(*ty_id)))
            .collect()
    } else {
        let field_inits = checker.context.arena.get_field_inits(fields);
        let span = if let Some(first) = field_inits.first() {
            first.span
        } else {
            ori_ir::Span::new(0, 0)
        };

        checker.push_error(
            format!(
                "`{}` is not a struct type",
                checker.context.interner.lookup(name)
            ),
            span,
            ori_diagnostic::ErrorCode::E2001,
        );
        return Type::Error;
    };

    let (expected_fields, type_args) = if type_entry.type_params.is_empty() {
        (expected_fields, Vec::new())
    } else {
        let type_args: Vec<Type> = type_entry
            .type_params
            .iter()
            .map(|_| checker.inference.ctx.fresh_var())
            .collect();

        let type_param_vars: HashMap<Name, Type> = type_entry
            .type_params
            .iter()
            .zip(type_args.iter())
            .map(|(&param_name, type_var)| (param_name, type_var.clone()))
            .collect();

        let substituted_fields = expected_fields
            .into_iter()
            .map(|(field_name, field_ty)| {
                let substituted_ty = substitute_type_params(&field_ty, &type_param_vars);
                (field_name, substituted_ty)
            })
            .collect();

        (substituted_fields, type_args)
    };

    let expected_map: HashMap<Name, Type> = expected_fields.iter().cloned().collect();

    let field_inits = checker.context.arena.get_field_inits(fields);
    let mut provided_fields: HashSet<Name> = HashSet::new();

    for init in field_inits {
        if !provided_fields.insert(init.name) {
            checker.push_error(
                format!(
                    "field `{}` specified more than once",
                    checker.context.interner.lookup(init.name)
                ),
                init.span,
                ori_diagnostic::ErrorCode::E2001,
            );
            continue;
        }

        if let Some(expected_ty) = expected_map.get(&init.name) {
            if let Some(value_id) = init.value {
                let actual_ty = infer_expr(checker, value_id);
                if let Err(e) = checker.inference.ctx.unify(&actual_ty, expected_ty) {
                    checker.report_type_error(&e, init.span);
                }
            } else {
                let var_ty = infer_ident(checker, init.name, init.span);
                if let Err(e) = checker.inference.ctx.unify(&var_ty, expected_ty) {
                    checker.report_type_error(&e, init.span);
                }
            }
        } else {
            checker.push_error(
                format!(
                    "struct `{}` has no field `{}`",
                    checker.context.interner.lookup(name),
                    checker.context.interner.lookup(init.name)
                ),
                init.span,
                ori_diagnostic::ErrorCode::E2001,
            );

            if let Some(value_id) = init.value {
                infer_expr(checker, value_id);
            }
        }
    }

    for (field_name, _) in &expected_fields {
        if !provided_fields.contains(field_name) {
            let span = if let Some(last) = field_inits.last() {
                last.span
            } else {
                ori_ir::Span::new(0, 0)
            };

            checker.push_error(
                format!(
                    "missing field `{}` in struct `{}`",
                    checker.context.interner.lookup(*field_name),
                    checker.context.interner.lookup(name)
                ),
                span,
                ori_diagnostic::ErrorCode::E2001,
            );
        }
    }

    if type_args.is_empty() {
        Type::Named(name)
    } else {
        Type::Applied {
            name,
            args: type_args,
        }
    }
}
