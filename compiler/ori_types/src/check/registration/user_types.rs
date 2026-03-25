//! User-defined type registration (Pass 0b).
//!
//! Registers struct, enum (sum type), and newtype declarations from user code
//! into both the Pool (for type interning) and `TypeRegistry` (for field access
//! and type checking).

use super::type_resolution::{collect_generic_params, convert_visibility, resolve_field_type};
use crate::{
    EnumVariant, FieldDef, Idx, ModuleChecker, TypeCheckError, VariantDef, VariantFields,
    Visibility,
};
use ori_ir::ReprAttrKind;

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
                    let ty = resolve_field_type(checker, &f.ty);
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

            let hash = checker.pool().hash(idx);
            checker.type_registry_mut().register_struct(
                decl.name,
                idx,
                type_params,
                field_defs,
                decl.span,
                visibility,
                hash,
                None, // repr set after validation below
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
                                let ty = resolve_field_type(checker, &f.ty);
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

            let hash = checker.pool().hash(idx);
            checker.type_registry_mut().register_enum(
                decl.name,
                idx,
                type_params,
                variant_defs,
                decl.span,
                visibility,
                hash,
                None, // repr set after validation below
            );
        }

        ori_ir::TypeDeclKind::Newtype(underlying) => {
            let underlying_ty = resolve_field_type(checker, underlying);
            let hash = checker.pool().hash(idx);
            checker.type_registry_mut().register_newtype(
                decl.name,
                idx,
                type_params,
                underlying_ty,
                decl.span,
                visibility,
                hash,
                None, // repr set after validation below
            );
        }
    }

    // Validate and merge #repr attributes (E2041).
    let resolved_repr = validate_and_merge_repr_attrs(checker, decl);

    // Re-register the resolved repr if validation produced a merged value.
    // The registration above passed the raw list; update with the resolved single attr.
    if let Some(repr) = resolved_repr {
        checker.type_registry_mut().set_repr(idx, Some(repr));
    } else if !decl.repr_attrs.is_empty() {
        // Had attrs but all were rejected — clear any that were set during registration.
        checker.type_registry_mut().set_repr(idx, None);
    }
}

/// Validate and merge `#repr` attributes into a single resolved `ReprAttrKind`.
///
/// Enforces:
/// - Type-kind validation: `c`/`packed`/`aligned` only on structs; `transparent` on
///   structs (field-count checked) or newtypes (redundant, allowed).
/// - Combination rules: `c + aligned(N)` → `CAligned(N)`; `packed + aligned`,
///   `c + packed`, `transparent + anything` are errors.
/// - Aligned constraints: N > 0, N is a power of two.
///
/// Returns the merged `ReprAttrKind` if valid, `None` if no attrs or all invalid.
fn validate_and_merge_repr_attrs(
    checker: &mut ModuleChecker<'_>,
    decl: &ori_ir::TypeDecl,
) -> Option<ReprAttrKind> {
    if decl.repr_attrs.is_empty() {
        return None;
    }

    let is_struct = matches!(decl.kind, ori_ir::TypeDeclKind::Struct(_));
    let is_newtype = matches!(decl.kind, ori_ir::TypeDeclKind::Newtype(_));

    // Step 1: Validate each individual attr for type-kind and value constraints.
    let mut valid_attrs: Vec<ReprAttrKind> = Vec::new();
    for attr in &decl.repr_attrs {
        match attr {
            ReprAttrKind::Transparent => {
                if let ori_ir::TypeDeclKind::Struct(fields) = &decl.kind {
                    if fields.is_empty() {
                        checker.push_error(TypeCheckError::invalid_repr_attribute(
                            decl.span,
                            decl.name,
                            "`#repr(\"transparent\")` requires exactly one field, but this struct has none",
                        ));
                        continue;
                    } else if fields.len() > 1 {
                        checker.push_error(TypeCheckError::invalid_repr_attribute(
                            decl.span,
                            decl.name,
                            format!(
                                "`#repr(\"transparent\")` requires exactly one field, but this struct has {}",
                                fields.len()
                            ),
                        ));
                        continue;
                    }
                    valid_attrs.push(*attr);
                } else if is_newtype {
                    // Spec §26.4.9: "#repr applies only to struct types."
                    // Newtypes are implicitly transparent; explicit #repr is an error.
                    checker.push_error(TypeCheckError::invalid_repr_attribute(
                        decl.span,
                        decl.name,
                        "`#repr(\"transparent\")` can only be applied to structs, not newtypes (newtypes are implicitly transparent)",
                    ));
                } else {
                    checker.push_error(TypeCheckError::invalid_repr_attribute(
                        decl.span,
                        decl.name,
                        "`#repr(\"transparent\")` can only be applied to structs, not sum types",
                    ));
                }
            }
            ReprAttrKind::Aligned(n) => {
                if *n == 0 {
                    checker.push_error(TypeCheckError::invalid_repr_attribute(
                        decl.span,
                        decl.name,
                        "alignment must be greater than zero",
                    ));
                    continue;
                }
                if !n.is_power_of_two() {
                    checker.push_error(TypeCheckError::invalid_repr_attribute(
                        decl.span,
                        decl.name,
                        format!("alignment must be a power of two, but got {n}"),
                    ));
                    continue;
                }
                if !is_struct {
                    checker.push_error(TypeCheckError::invalid_repr_attribute(
                        decl.span,
                        decl.name,
                        "`#repr(\"aligned\", N)` can only be applied to structs, not newtypes or sum types",
                    ));
                    continue;
                }
                valid_attrs.push(*attr);
            }
            ReprAttrKind::C => {
                if !is_struct {
                    checker.push_error(TypeCheckError::invalid_repr_attribute(
                        decl.span,
                        decl.name,
                        "`#repr(\"c\")` can only be applied to structs, not newtypes or sum types",
                    ));
                    continue;
                }
                valid_attrs.push(*attr);
            }
            ReprAttrKind::Packed => {
                if !is_struct {
                    checker.push_error(TypeCheckError::invalid_repr_attribute(
                        decl.span,
                        decl.name,
                        "`#repr(\"packed\")` can only be applied to structs, not newtypes or sum types",
                    ));
                    continue;
                }
                valid_attrs.push(*attr);
            }
            ReprAttrKind::CAligned(_) => {
                // CAligned is only produced by merging — should not appear in raw input.
                // If it somehow does, just pass it through.
                valid_attrs.push(*attr);
            }
        }
    }

    merge_repr_attrs(checker, decl, &valid_attrs)
}

/// Merge multiple validated `#repr` attributes into a single `ReprAttrKind`.
///
/// Only `c + aligned(N) → CAligned(N)` is a valid combination; all other
/// multi-attribute stacks are rejected with E2041.
fn merge_repr_attrs(
    checker: &mut ModuleChecker<'_>,
    decl: &ori_ir::TypeDecl,
    valid_attrs: &[ReprAttrKind],
) -> Option<ReprAttrKind> {
    if valid_attrs.is_empty() {
        return None;
    }

    if valid_attrs.len() == 1 {
        return Some(valid_attrs[0]);
    }

    let has_c = valid_attrs.iter().any(|a| matches!(a, ReprAttrKind::C));
    let has_packed = valid_attrs
        .iter()
        .any(|a| matches!(a, ReprAttrKind::Packed));
    let has_transparent = valid_attrs
        .iter()
        .any(|a| matches!(a, ReprAttrKind::Transparent));
    let aligned_n = valid_attrs.iter().find_map(|a| match a {
        ReprAttrKind::Aligned(n) => Some(*n),
        _ => None,
    });

    // Reject invalid combinations.
    if has_packed && aligned_n.is_some() {
        checker.push_error(TypeCheckError::invalid_repr_attribute(
            decl.span,
            decl.name,
            "cannot combine `#repr(\"packed\")` and `#repr(\"aligned\", N)` — they are contradictory",
        ));
        return None;
    }
    if has_c && has_packed {
        checker.push_error(TypeCheckError::invalid_repr_attribute(
            decl.span,
            decl.name,
            "cannot combine `#repr(\"c\")` and `#repr(\"packed\")` — use `#repr(\"c\")` with explicit padding",
        ));
        return None;
    }
    if has_transparent && valid_attrs.len() > 1 {
        checker.push_error(TypeCheckError::invalid_repr_attribute(
            decl.span,
            decl.name,
            "`#repr(\"transparent\")` cannot be combined with other repr attributes",
        ));
        return None;
    }

    // Valid combination: c + aligned(N) → CAligned(N).
    if let (true, Some(n)) = (has_c, aligned_n) {
        // Reject if there are extra attrs beyond the c+aligned pair.
        if valid_attrs.len() > 2 {
            checker.push_error(TypeCheckError::invalid_repr_attribute(
                decl.span,
                decl.name,
                "only `#repr(\"c\")` + `#repr(\"aligned\", N)` is a valid combination — extra `#repr` attributes are not permitted",
            ));
            return None;
        }
        return Some(ReprAttrKind::CAligned(n));
    }

    // Reject duplicate same-kind attrs. The only valid multi-attr case is c+aligned
    // (handled above). Any other multi-attr combination reaching here is a duplicate.
    checker.push_error(TypeCheckError::invalid_repr_attribute(
        decl.span,
        decl.name,
        "duplicate `#repr` attributes are not permitted — each `#repr` kind may only appear once",
    ));
    None
}
