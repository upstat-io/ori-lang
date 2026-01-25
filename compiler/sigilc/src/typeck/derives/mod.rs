//! Derive trait processing.
//!
//! Processes `#[derive(Trait)]` attributes on type declarations and
//! generates derived method entries for both:
//! - `UserMethodRegistry` (for evaluation)
//! - `TraitRegistry` (for type checking)

use crate::ir::{Module, Name, StringInterner, TypeDecl, TypeDeclKind};
use crate::types::Type;
use crate::typeck::type_registry::{
    ImplEntry, ImplMethodDef, TraitRegistry, TypeKind, TypeRegistry,
};
use sigil_eval::{DerivedMethodInfo, DerivedTrait, UserMethodRegistry};

/// Process all derive attributes in a module.
///
/// For each type with `#[derive(...)]` attributes, generates derived method
/// entries in the user method registry.
pub fn process_derives(
    module: &Module,
    type_registry: &TypeRegistry,
    user_method_registry: &mut UserMethodRegistry,
    interner: &StringInterner,
) {
    for type_decl in &module.types {
        if !type_decl.derives.is_empty() {
            process_type_derives(type_decl, type_registry, user_method_registry, interner);
        }
    }
}

/// Process derives for a single type declaration.
fn process_type_derives(
    type_decl: &TypeDecl,
    _type_registry: &TypeRegistry,
    user_method_registry: &mut UserMethodRegistry,
    interner: &StringInterner,
) {
    let type_name = interner.lookup(type_decl.name).to_string();

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
        let trait_name = interner.lookup(*derive_name);

        if let Some(trait_kind) = DerivedTrait::from_str(trait_name) {
            let method_name = trait_kind.method_name().to_string();
            let info = DerivedMethodInfo::new(trait_kind, field_names.clone());

            user_method_registry.register_derived(type_name.clone(), method_name, info);
        }
        // Unknown derive traits are ignored here (type checker may report an error)
    }
}

/// Register derived methods in the trait registry for type checking.
///
/// This creates `ImplEntry` objects for each derived trait so the type
/// checker can find the methods during method call inference.
pub fn register_derived_impls(
    module: &Module,
    trait_registry: &mut TraitRegistry,
    interner: &StringInterner,
) {
    for type_decl in &module.types {
        if !type_decl.derives.is_empty() {
            register_type_derived_impls(type_decl, trait_registry, interner);
        }
    }
}

/// Register derived impls for a single type declaration.
fn register_type_derived_impls(
    type_decl: &TypeDecl,
    trait_registry: &mut TraitRegistry,
    interner: &StringInterner,
) {
    let self_ty = Type::Named(type_decl.name);
    let mut methods = Vec::new();

    // Process each derived trait and collect methods
    for derive_name in &type_decl.derives {
        let trait_name_str = interner.lookup(*derive_name);

        if let Some(trait_kind) = DerivedTrait::from_str(trait_name_str) {
            let method_name = interner.intern(trait_kind.method_name());
            let method_def = create_derived_method_def(trait_kind, method_name, &self_ty);
            methods.push(method_def);
        }
    }

    // Register as an inherent impl (no trait_name)
    if !methods.is_empty() {
        let impl_entry = ImplEntry {
            trait_name: None,
            self_ty,
            span: type_decl.span,
            type_params: vec![],
            methods,
            assoc_types: vec![],
        };

        // Ignore coherence errors for derived methods - they're auto-generated
        let _ = trait_registry.register_impl(impl_entry);
    }
}

/// Create a method definition for a derived trait.
fn create_derived_method_def(
    trait_kind: DerivedTrait,
    method_name: Name,
    self_ty: &Type,
) -> ImplMethodDef {
    match trait_kind {
        DerivedTrait::Eq => ImplMethodDef {
            name: method_name,
            // eq(self, other: Self) -> bool
            params: vec![self_ty.clone(), self_ty.clone()],
            return_ty: Type::Bool,
        },
        DerivedTrait::Clone => ImplMethodDef {
            name: method_name,
            // clone(self) -> Self
            params: vec![self_ty.clone()],
            return_ty: self_ty.clone(),
        },
        DerivedTrait::Hashable => ImplMethodDef {
            name: method_name,
            // hash(self) -> int
            params: vec![self_ty.clone()],
            return_ty: Type::Int,
        },
        DerivedTrait::Printable => ImplMethodDef {
            name: method_name,
            // to_string(self) -> str
            params: vec![self_ty.clone()],
            return_ty: Type::Str,
        },
        DerivedTrait::Default => ImplMethodDef {
            name: method_name,
            // default() -> Self (static method, no self param)
            params: vec![],
            return_ty: self_ty.clone(),
        },
    }
}

/// Get variant information for sum types (enums).
///
/// This is used by the evaluator to handle derived methods for enum types.
pub fn get_variant_info(
    type_name: Name,
    type_registry: &TypeRegistry,
) -> Option<Vec<(Name, Vec<Name>)>> {
    let entry = type_registry.get_by_name(type_name)?;

    if let TypeKind::Enum { variants } = &entry.kind {
        Some(
            variants
                .iter()
                .map(|v| {
                    let field_names: Vec<Name> = v.fields.iter().map(|f| f.0).collect();
                    (v.name, field_names)
                })
                .collect(),
        )
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::SharedInterner;
    use crate::parser::parse;
    use sigil_lexer::lex;

    #[test]
    fn test_process_struct_derives() {
        let interner = SharedInterner::default();
        let source = r#"
#[derive(Eq)]
type Point = { x: int, y: int }

@main () -> void = print(msg: "test")
"#;

        let tokens = lex(source, &interner);
        let parse_result = parse(&tokens, &interner);
        assert!(!parse_result.has_errors(), "Parse errors: {:?}", parse_result.errors);

        let type_registry = TypeRegistry::new();
        let mut user_method_registry = UserMethodRegistry::new();

        process_derives(&parse_result.module, &type_registry, &mut user_method_registry, &interner);

        // Should have registered an eq method for Point
        assert!(user_method_registry.has_method("Point", "eq"));

        let info = user_method_registry.lookup_derived("Point", "eq").unwrap();
        assert_eq!(info.trait_kind, DerivedTrait::Eq);
        assert_eq!(info.field_names.len(), 2);
    }

    #[test]
    fn test_process_multiple_derives() {
        let interner = SharedInterner::default();
        let source = r#"
#[derive(Eq, Clone, Printable)]
type Point = { x: int, y: int }

@main () -> void = print(msg: "test")
"#;

        let tokens = lex(source, &interner);
        let parse_result = parse(&tokens, &interner);
        assert!(!parse_result.has_errors());

        let type_registry = TypeRegistry::new();
        let mut user_method_registry = UserMethodRegistry::new();

        process_derives(&parse_result.module, &type_registry, &mut user_method_registry, &interner);

        // Should have all three methods registered
        assert!(user_method_registry.has_method("Point", "eq"));
        assert!(user_method_registry.has_method("Point", "clone"));
        assert!(user_method_registry.has_method("Point", "to_string"));
    }

    #[test]
    fn test_ignore_unknown_derives() {
        let interner = SharedInterner::default();
        let source = r#"
#[derive(Unknown, Eq)]
type Point = { x: int }

@main () -> void = print(msg: "test")
"#;

        let tokens = lex(source, &interner);
        let parse_result = parse(&tokens, &interner);
        assert!(!parse_result.has_errors());

        let type_registry = TypeRegistry::new();
        let mut user_method_registry = UserMethodRegistry::new();

        process_derives(&parse_result.module, &type_registry, &mut user_method_registry, &interner);

        // Should have Eq but not Unknown
        assert!(user_method_registry.has_method("Point", "eq"));
        assert!(!user_method_registry.has_method("Point", "unknown"));
    }
}
