//! Type extraction helpers for match patterns.

use crate::checker::TypeChecker;
use crate::registry::{TypeKind, VariantDef};
use ori_ir::Name;
use ori_types::Type;

/// Get the inner type for a variant pattern (Some, Ok, Err, etc.).
pub fn get_variant_inner_type(
    checker: &mut TypeChecker<'_>,
    scrutinee_ty: &Type,
    variant_name: Name,
) -> Type {
    let variant_str = checker.context.interner.lookup(variant_name);

    match variant_str {
        "Some" => match scrutinee_ty {
            Type::Option(inner) => (**inner).clone(),
            _ => checker.inference.ctx.fresh_var(),
        },
        "None" => Type::Unit,
        "Ok" => match scrutinee_ty {
            Type::Result { ok, .. } => (**ok).clone(),
            _ => checker.inference.ctx.fresh_var(),
        },
        "Err" => match scrutinee_ty {
            Type::Result { err, .. } => (**err).clone(),
            _ => checker.inference.ctx.fresh_var(),
        },
        _ => {
            if let Type::Named(type_name) = scrutinee_ty {
                if let Some(entry) = checker.registries.types.get_by_name(*type_name) {
                    if let TypeKind::Enum { variants } = &entry.kind {
                        for variant in variants {
                            if variant.name == variant_name {
                                return get_variant_field_type(variant, &checker.registries.types);
                            }
                        }
                    }
                }
            }
            checker.inference.ctx.fresh_var()
        }
    }
}

/// Get the field type for a variant, converting `TypeId` to Type.
pub fn get_variant_field_type(
    variant: &VariantDef,
    registry: &crate::registry::TypeRegistry,
) -> Type {
    let interner = registry.interner();
    match variant.fields.len() {
        0 => Type::Unit,
        1 => interner.to_type(variant.fields[0].1),
        _ => Type::Tuple(
            variant
                .fields
                .iter()
                .map(|(_, ty_id)| interner.to_type(*ty_id))
                .collect(),
        ),
    }
}

/// Get field types for a struct pattern.
pub fn get_struct_field_types(
    checker: &mut TypeChecker<'_>,
    scrutinee_ty: &Type,
) -> Vec<(Name, Type)> {
    if let Type::Named(type_name) = scrutinee_ty {
        if let Some(entry) = checker.registries.types.get_by_name(*type_name) {
            if let TypeKind::Struct { fields } = &entry.kind {
                let interner = checker.registries.types.interner();
                return fields
                    .iter()
                    .map(|(name, ty_id)| (*name, interner.to_type(*ty_id)))
                    .collect();
            }
        }
    }
    vec![]
}
