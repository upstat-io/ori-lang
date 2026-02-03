//! Struct literal type inference and field lookup helpers.

use super::super::infer_expr;
use super::identifiers::infer_ident;
use super::substitute_type_params;
use crate::checker::TypeChecker;
use crate::registry::TypeKind;
use crate::suggest::{suggest_field, suggest_type};
use ori_ir::{FieldInitRange, Name, Span};
use ori_types::Type;
use std::collections::{HashMap, HashSet};

/// Result of looking up a struct field.
pub(super) enum FieldLookupResult {
    /// Field found with resolved type.
    Found(Type),
    /// Type is not a struct (is an enum or newtype).
    NotStruct,
    /// Field not found in struct.
    NoSuchField,
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
        TypeKind::Enum { .. } | TypeKind::Newtype { .. } => FieldLookupResult::NotStruct,
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
    // Perform lookup directly to avoid cloning the entire entry
    let lookup_result = {
        let Some(entry) = checker.registries.types.get_by_name(type_name) else {
            let type_name_str = checker.context.interner.lookup(type_name);
            let suggestion = suggest_type(checker, type_name);
            checker.error_unknown_struct(span, type_name_str, suggestion);
            return Type::Error;
        };
        lookup_struct_field_in_entry(entry, field, type_args, &checker.registries.types)
    };

    match lookup_result {
        FieldLookupResult::Found(ty) => ty,
        FieldLookupResult::NoSuchField => {
            let type_name_str = checker.context.interner.lookup(type_name);
            let field_name_str = checker.context.interner.lookup(field);
            let suggestion = suggest_field(checker, type_name, field);
            checker.error_no_such_field(span, type_name_str, field_name_str, suggestion);
            Type::Error
        }
        FieldLookupResult::NotStruct => {
            let type_name_str = checker.context.interner.lookup(type_name);
            let field_name_str = checker.context.interner.lookup(field);
            checker.error_field_access_not_supported(
                span,
                type_name_str,
                Some(format!(
                    "cannot access field `{field_name_str}` on non-struct type"
                )),
            );
            Type::Error
        }
    }
}

/// Infer type for a struct literal.
pub fn infer_struct(checker: &mut TypeChecker<'_>, name: Name, fields: FieldInitRange) -> Type {
    // Extract only the needed fields from the type entry to avoid cloning the entire entry
    let (expected_fields, type_params) = {
        let Some(entry) = checker.registries.types.get_by_name(name) else {
            let field_inits = checker.context.arena.get_field_inits(fields);
            let span = if let Some(first) = field_inits.first() {
                first.span
            } else {
                ori_ir::Span::new(0, 0)
            };

            let name_str = checker.context.interner.lookup(name);
            let suggestion = suggest_type(checker, name);
            checker.error_unknown_struct(span, name_str, suggestion);

            for init in field_inits {
                if let Some(value_id) = init.value {
                    infer_expr(checker, value_id);
                }
            }
            return Type::Error;
        };

        // Get struct fields as TypeId, then convert to Type
        let fields_vec: Vec<(Name, Type)> = if let TypeKind::Struct {
            fields: struct_fields,
        } = &entry.kind
        {
            let interner = checker.registries.types.interner();
            struct_fields
                .iter()
                .map(|(n, ty_id)| (*n, interner.to_type(*ty_id)))
                .collect()
        } else {
            let field_inits = checker.context.arena.get_field_inits(fields);
            let span = if let Some(first) = field_inits.first() {
                first.span
            } else {
                ori_ir::Span::new(0, 0)
            };

            let name_str = checker.context.interner.lookup(name);
            checker.error_not_a_struct(span, name_str);
            return Type::Error;
        };

        // Clone only the type_params, not the entire entry
        (fields_vec, entry.type_params.clone())
    };

    let (expected_fields, type_args) = if type_params.is_empty() {
        (expected_fields, Vec::new())
    } else {
        let type_args: Vec<Type> = type_params
            .iter()
            .map(|_| checker.inference.ctx.fresh_var())
            .collect();

        let type_param_vars: HashMap<Name, Type> = type_params
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

    let expected_map: HashMap<Name, &Type> =
        expected_fields.iter().map(|(n, ty)| (*n, ty)).collect();

    let field_inits = checker.context.arena.get_field_inits(fields);
    let mut provided_fields: HashSet<Name> = HashSet::new();

    for init in field_inits {
        if !provided_fields.insert(init.name) {
            let field_name = checker.context.interner.lookup(init.name);
            checker.error_duplicate_field(init.span, field_name);
            continue;
        }

        if let Some(&expected_ty) = expected_map.get(&init.name) {
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
            let struct_name = checker.context.interner.lookup(name);
            let field_name = checker.context.interner.lookup(init.name);
            let suggestion = suggest_field(checker, name, init.name);
            checker.error_no_such_field(init.span, struct_name, field_name, suggestion);

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

            let field_name_str = checker.context.interner.lookup(*field_name);
            let struct_name = checker.context.interner.lookup(name);
            checker.error_missing_field(span, struct_name, field_name_str);
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
