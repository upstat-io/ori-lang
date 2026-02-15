//! Derive trait processing for the evaluator.
//!
//! Processes `#[derive(Trait)]` attributes on type declarations and
//! generates derived method entries for the `UserMethodRegistry`.

use ori_ir::{
    DefaultFieldType, DerivedMethodInfo, DerivedTrait, Module, ParsedType, StringInterner,
    StructField, TypeDeclKind,
};

use crate::UserMethodRegistry;

/// Map a `ParsedType` to a `DefaultFieldType` for Default trait derivation.
fn parsed_type_to_default_field(ty: &ParsedType, interner: &StringInterner) -> DefaultFieldType {
    match ty {
        ParsedType::Primitive(id) => DefaultFieldType::Primitive(*id),
        ParsedType::Named { name, .. } => DefaultFieldType::Named(*name),
        ParsedType::List(_) | ParsedType::FixedList { .. } => {
            DefaultFieldType::Named(interner.intern("List"))
        }
        ParsedType::Map { .. } => DefaultFieldType::Named(interner.intern("Map")),
        ParsedType::Tuple(_) => DefaultFieldType::Named(interner.intern("Tuple")),
        ParsedType::Function { .. } => DefaultFieldType::Named(interner.intern("Function")),
        ParsedType::Infer
        | ParsedType::SelfType
        | ParsedType::AssociatedType { .. }
        | ParsedType::ConstExpr(_)
        | ParsedType::TraitBounds(_) => DefaultFieldType::Named(interner.intern("Unknown")),
    }
}

/// Extract field types from struct fields for Default derivation.
fn extract_field_types(fields: &[StructField], interner: &StringInterner) -> Vec<DefaultFieldType> {
    fields
        .iter()
        .map(|f| parsed_type_to_default_field(&f.ty, interner))
        .collect()
}

/// Process all derive attributes in a module.
///
/// For each type with `#[derive(...)]` attributes, generates derived method
/// entries in the user method registry.
pub fn process_derives(
    module: &Module,
    user_method_registry: &mut UserMethodRegistry,
    interner: &StringInterner,
) {
    for type_decl in &module.types {
        if !type_decl.derives.is_empty() {
            let type_name = type_decl.name;

            // Get field names and types based on type kind
            let (field_names, field_types) = match &type_decl.kind {
                TypeDeclKind::Struct(fields) => {
                    let names = fields.iter().map(|f| f.name).collect();
                    let types = extract_field_types(fields, interner);
                    (names, types)
                }
                TypeDeclKind::Sum(_variants) => {
                    // For sum types, we'll need variant-specific handling
                    // For now, use empty lists and handle variants in the evaluator
                    (Vec::new(), Vec::new())
                }
                TypeDeclKind::Newtype(_) => {
                    // Newtypes wrap a single value
                    (Vec::new(), Vec::new())
                }
            };

            // Process each derived trait
            for derive_name in &type_decl.derives {
                let trait_name_str = interner.lookup(*derive_name);

                if let Some(trait_kind) = DerivedTrait::from_name(trait_name_str) {
                    let method_name = interner.intern(trait_kind.method_name());
                    let info = if trait_kind == DerivedTrait::Default {
                        DerivedMethodInfo::with_field_types(
                            trait_kind,
                            field_names.clone(),
                            field_types.clone(),
                        )
                    } else {
                        DerivedMethodInfo::new(trait_kind, field_names.clone())
                    };

                    user_method_registry.register_derived(type_name, method_name, info);
                }
                // Unknown derive traits are ignored here (type checker may report an error)
            }
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests;
