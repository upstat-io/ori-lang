//! Type extraction helpers for match patterns.

use std::collections::HashMap;

use crate::checker::TypeChecker;
use crate::registry::TypeKind;
use ori_ir::Name;
use ori_types::Type;

/// Get field types for a struct pattern as a HashMap for O(1) lookup.
pub fn get_struct_field_types(
    checker: &mut TypeChecker<'_>,
    scrutinee_ty: &Type,
) -> HashMap<Name, Type> {
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
    HashMap::new()
}

/// Get the field types for a variant pattern as a Vec.
///
/// Returns a list of types for multi-field variant patterns like `Click(x, y)`.
/// For built-in types (Option, Result), returns a single-element vector.
pub fn get_variant_field_types(
    checker: &mut TypeChecker<'_>,
    scrutinee_ty: &Type,
    variant_name: Name,
) -> Vec<Type> {
    let variant_str = checker.context.interner.lookup(variant_name);

    match variant_str {
        "Some" => match scrutinee_ty {
            Type::Option(inner) => vec![(**inner).clone()],
            _ => vec![checker.inference.ctx.fresh_var()],
        },
        "None" => vec![],
        "Ok" => match scrutinee_ty {
            Type::Result { ok, .. } => vec![(**ok).clone()],
            _ => vec![checker.inference.ctx.fresh_var()],
        },
        "Err" => match scrutinee_ty {
            Type::Result { err, .. } => vec![(**err).clone()],
            _ => vec![checker.inference.ctx.fresh_var()],
        },
        _ => {
            // User-defined variant
            if let Type::Named(type_name) = scrutinee_ty {
                if let Some(entry) = checker.registries.types.get_by_name(*type_name) {
                    if let TypeKind::Enum { variants } = &entry.kind {
                        for variant in variants {
                            if variant.name == variant_name {
                                let interner = checker.registries.types.interner();
                                return variant
                                    .fields
                                    .iter()
                                    .map(|(_, ty_id)| interner.to_type(*ty_id))
                                    .collect();
                            }
                        }
                    }
                }
            }
            vec![]
        }
    }
}
