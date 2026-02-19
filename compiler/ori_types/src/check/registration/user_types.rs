//! User-defined type registration (Pass 0b).
//!
//! Registers struct, enum (sum type), and newtype declarations from user code
//! into both the Pool (for type interning) and TypeRegistry (for field access
//! and type checking).

use super::type_resolution::{collect_generic_params, convert_visibility, resolve_field_type};
use crate::{
    EnumVariant, FieldDef, Idx, ModuleChecker, TypeCheckError, VariantDef, VariantFields,
    Visibility,
};

/// Register user-defined types (structs, enums, newtypes).
pub fn register_user_types(checker: &mut ModuleChecker<'_>, module: &ori_ir::Module) {
    for type_decl in &module.types {
        register_type_decl(checker, type_decl);
    }
}

/// Register a single type declaration.
#[expect(
    clippy::too_many_lines,
    reason = "exhaustive type declaration kind registration — struct, enum, newtype, alias"
)]
fn register_type_decl(checker: &mut ModuleChecker<'_>, decl: &ori_ir::TypeDecl) {
    // Collect generic parameters
    let type_params = collect_generic_params(checker.arena(), decl.generics);

    // Create pool index for this type
    let idx = checker.pool_mut().named(decl.name);

    // Convert visibility
    let visibility = convert_visibility(decl.visibility);

    // Build and register based on declaration kind
    match &decl.kind {
        ori_ir::TypeDeclKind::Struct(fields) => {
            let field_defs: Vec<FieldDef> = fields
                .iter()
                .map(|f| {
                    let ty = resolve_field_type(checker, &f.ty, &type_params);
                    FieldDef {
                        name: f.name,
                        ty,
                        span: f.span,
                        visibility: Visibility::Public,
                    }
                })
                .collect();

            // E2019: Never type cannot appear as a struct field.
            // Direct comparison (not resolve_fully) — aliases may not be registered yet.
            for f in &field_defs {
                if f.ty == Idx::NEVER {
                    checker.push_error(TypeCheckError::uninhabited_struct_field(
                        f.span, decl.name, f.name,
                    ));
                }
            }

            // Create Pool struct entry BEFORE moving field_defs to TypeRegistry.
            // Extract (Name, Idx) pairs for the Pool's compact representation.
            let pool_fields: Vec<(ori_ir::Name, Idx)> =
                field_defs.iter().map(|f| (f.name, f.ty)).collect();
            let struct_idx = checker.pool_mut().struct_type(decl.name, &pool_fields);
            checker.pool_mut().set_resolution(idx, struct_idx);

            checker.type_registry_mut().register_struct(
                decl.name,
                idx,
                type_params,
                field_defs,
                decl.span,
                visibility,
            );
        }

        ori_ir::TypeDeclKind::Sum(variants) => {
            let variant_defs: Vec<VariantDef> = variants
                .iter()
                .map(|v| {
                    let fields = if v.fields.is_empty() {
                        VariantFields::Unit
                    } else {
                        let field_defs: Vec<FieldDef> = v
                            .fields
                            .iter()
                            .map(|f| {
                                let ty = resolve_field_type(checker, &f.ty, &type_params);
                                FieldDef {
                                    name: f.name,
                                    ty,
                                    span: f.span,
                                    visibility: Visibility::Public,
                                }
                            })
                            .collect();
                        VariantFields::Record(field_defs)
                    };

                    VariantDef {
                        name: v.name,
                        fields,
                        span: v.span,
                    }
                })
                .collect();

            // Create Pool enum entry BEFORE moving variant_defs to TypeRegistry.
            // Extract variant info for the Pool's compact representation.
            let pool_variants: Vec<EnumVariant> = variant_defs
                .iter()
                .map(|v| {
                    let field_types = match &v.fields {
                        VariantFields::Unit => vec![],
                        VariantFields::Tuple(types) => types.clone(),
                        VariantFields::Record(field_defs) => {
                            field_defs.iter().map(|f| f.ty).collect()
                        }
                    };
                    EnumVariant {
                        name: v.name,
                        field_types,
                    }
                })
                .collect();
            let enum_idx = checker.pool_mut().enum_type(decl.name, &pool_variants);
            checker.pool_mut().set_resolution(idx, enum_idx);

            checker.type_registry_mut().register_enum(
                decl.name,
                idx,
                type_params,
                variant_defs,
                decl.span,
                visibility,
            );
        }

        ori_ir::TypeDeclKind::Newtype(underlying) => {
            let underlying_ty = resolve_field_type(checker, underlying, &type_params);
            checker.type_registry_mut().register_newtype(
                decl.name,
                idx,
                type_params,
                underlying_ty,
                decl.span,
                visibility,
            );
        }
    }
}
