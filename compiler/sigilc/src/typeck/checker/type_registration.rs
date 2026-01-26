//! User-Defined Type Registration
//!
//! Registers struct, sum type (enum), and newtype declarations from modules.

use crate::ir::{Module, TypeDecl, TypeDeclKind};
use crate::types::Type;
use super::TypeChecker;
use super::super::type_registry::VariantDef;

impl TypeChecker<'_> {
    /// Register all user-defined type declarations from a module.
    ///
    /// This must be called before trait registration, as traits and impls
    /// may reference user-defined types.
    pub(crate) fn register_types(&mut self, module: &Module) {
        for type_decl in &module.types {
            self.register_type_decl(type_decl);
        }
    }

    /// Register a single type declaration.
    fn register_type_decl(&mut self, type_decl: &TypeDecl) {
        // Convert generic params to names
        let type_params: Vec<crate::ir::Name> = self.context.arena
            .get_generic_params(type_decl.generics)
            .iter()
            .map(|gp| gp.name)
            .collect();

        match &type_decl.kind {
            TypeDeclKind::Struct(fields) => {
                // Convert AST fields to (Name, Type) pairs
                let field_types: Vec<(crate::ir::Name, Type)> = fields
                    .iter()
                    .map(|f| {
                        let ty = self.parsed_type_to_type(&f.ty);
                        (f.name, ty)
                    })
                    .collect();

                self.registries.types.register_struct(
                    type_decl.name,
                    field_types,
                    type_decl.span,
                    type_params,
                );
            }

            TypeDeclKind::Sum(variants) => {
                // Convert AST variants to VariantDef
                let variant_defs: Vec<VariantDef> = variants
                    .iter()
                    .map(|v| {
                        let fields: Vec<(crate::ir::Name, Type)> = v.fields
                            .iter()
                            .map(|f| {
                                let ty = self.parsed_type_to_type(&f.ty);
                                (f.name, ty)
                            })
                            .collect();
                        VariantDef {
                            name: v.name,
                            fields,
                        }
                    })
                    .collect();

                self.registries.types.register_enum(
                    type_decl.name,
                    variant_defs,
                    type_decl.span,
                    type_params,
                );
            }

            TypeDeclKind::Newtype(target_ty) => {
                // For newtypes, convert the target ParsedType to Type
                let target = self.parsed_type_to_type(target_ty);
                self.registries.types.register_alias(
                    type_decl.name,
                    target,
                    type_decl.span,
                    type_params,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ir::StringInterner;
    use crate::parser::parse;
    use crate::typeck::checker::TypeChecker;
    use crate::typeck::type_registry::TypeKind;

    fn check_types(source: &str) -> (TypeChecker<'static>, StringInterner) {
        let interner = StringInterner::new();
        let tokens = sigil_lexer::lex(source, &interner);
        let parse_result = parse(&tokens, &interner);
        assert!(
            !parse_result.has_errors(),
            "parse errors: {:?}",
            parse_result.errors
        );

        // Leak the arena to get 'static lifetime for testing
        let arena = Box::leak(Box::new(parse_result.arena));
        let interner = Box::leak(Box::new(interner));

        let mut checker = TypeChecker::new(arena, interner);
        checker.register_types(&parse_result.module);

        // Safety: We're leaking memory here, but it's fine for tests
        (checker, StringInterner::new())
    }

    #[test]
    fn test_register_struct_type() {
        let source = r#"
type Point = { x: int, y: int }

@main () -> void = print(msg: "test")
"#;

        let (checker, _interner) = check_types(source);
        assert_eq!(checker.registries.types.len(), 1);

        // Find Point type
        let entry = checker.registries.types.iter().next().unwrap();
        if let TypeKind::Struct { fields } = &entry.kind {
            assert_eq!(fields.len(), 2);
        } else {
            panic!("Expected struct type");
        }
    }

    #[test]
    fn test_register_sum_type() {
        let source = r#"
type Status = Pending | Running | Done

@main () -> void = print(msg: "test")
"#;

        let (checker, _interner) = check_types(source);
        assert_eq!(checker.registries.types.len(), 1);

        let entry = checker.registries.types.iter().next().unwrap();
        if let TypeKind::Enum { variants } = &entry.kind {
            assert_eq!(variants.len(), 3);
        } else {
            panic!("Expected enum type");
        }
    }

    #[test]
    fn test_register_newtype() {
        let source = r#"
type UserId = int

@main () -> void = print(msg: "test")
"#;

        let (checker, _interner) = check_types(source);
        assert_eq!(checker.registries.types.len(), 1);

        let entry = checker.registries.types.iter().next().unwrap();
        if let TypeKind::Alias { target } = &entry.kind {
            assert_eq!(*target, crate::types::Type::Int);
        } else {
            panic!("Expected alias type");
        }
    }
}
