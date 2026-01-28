//! User-Defined Type Registration
//!
//! Registers struct, sum type (enum), and newtype declarations from modules.

use super::TypeChecker;
use ori_ir::{Module, TypeDecl, TypeDeclKind};
use ori_types::Type;

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
        let type_params: Vec<ori_ir::Name> = self
            .context
            .arena
            .get_generic_params(type_decl.generics)
            .iter()
            .map(|gp| gp.name)
            .collect();

        match &type_decl.kind {
            TypeDeclKind::Struct(fields) => {
                // Convert AST fields to (Name, Type) pairs
                let field_types: Vec<(ori_ir::Name, Type)> = fields
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
                // Convert AST variants to (Name, Vec<(Name, Type)>) tuples
                let variant_inputs: Vec<(ori_ir::Name, Vec<(ori_ir::Name, Type)>)> = variants
                    .iter()
                    .map(|v| {
                        let fields: Vec<(ori_ir::Name, Type)> = v
                            .fields
                            .iter()
                            .map(|f| {
                                let ty = self.parsed_type_to_type(&f.ty);
                                (f.name, ty)
                            })
                            .collect();
                        (v.name, fields)
                    })
                    .collect();

                self.registries.types.register_enum(
                    type_decl.name,
                    variant_inputs,
                    type_decl.span,
                    type_params,
                );
            }

            TypeDeclKind::Newtype(underlying_ty) => {
                // For newtypes, convert the underlying ParsedType to Type
                let underlying = self.parsed_type_to_type(underlying_ty);
                self.registries.types.register_newtype(
                    type_decl.name,
                    &underlying,
                    type_decl.span,
                    type_params,
                );
            }
        }
    }
}
