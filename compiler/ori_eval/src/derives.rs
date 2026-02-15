//! Derive trait processing for the evaluator.
//!
//! Processes `#[derive(Trait)]` attributes on type declarations and
//! generates derived method entries for the `UserMethodRegistry`.

use ori_ir::{DerivedMethodInfo, DerivedTrait, Module, StringInterner, TypeDeclKind};

use crate::UserMethodRegistry;

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

            // Get field names based on type kind
            let field_names = match &type_decl.kind {
                TypeDeclKind::Struct(fields) => fields.iter().map(|f| f.name).collect(),
                TypeDeclKind::Sum(_variants) => {
                    // For sum types, we'll need variant-specific handling
                    // For now, use an empty list and handle variants in the evaluator
                    Vec::new()
                }
                TypeDeclKind::Newtype(_) => {
                    // Newtypes wrap a single value
                    Vec::new()
                }
            };

            // Process each derived trait
            for derive_name in &type_decl.derives {
                let trait_name_str = interner.lookup(*derive_name);

                if let Some(trait_kind) = DerivedTrait::from_name(trait_name_str) {
                    let method_name = interner.intern(trait_kind.method_name());
                    let info = DerivedMethodInfo::new(trait_kind, field_names.clone());

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
