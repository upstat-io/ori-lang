//! Trait and Impl Registration
//!
//! Registers trait definitions and implementations from modules.

use crate::ir::{Name, Module, TraitItem, ParamRange};
use crate::types::Type;
use super::TypeChecker;
use super::types::TypeCheckError;
use super::super::type_registry::{
    TraitEntry, TraitMethodDef, TraitAssocTypeDef, ImplEntry, ImplMethodDef, ImplAssocTypeDef,
};

impl TypeChecker<'_> {
    /// Register all trait definitions from a module.
    pub(crate) fn register_traits(&mut self, module: &Module) {
        for trait_def in &module.traits {
            // Convert generic params to names
            let type_params: Vec<Name> = self.arena
                .get_generic_params(trait_def.generics)
                .iter()
                .map(|gp| gp.name)
                .collect();

            // Convert super-traits to names
            let super_traits: Vec<Name> = trait_def.super_traits
                .iter()
                .map(sigil_ir::TraitBound::name)
                .collect();

            // Convert trait items
            let mut methods = Vec::new();
            let mut assoc_types = Vec::new();

            for item in &trait_def.items {
                match item {
                    TraitItem::MethodSig(sig) => {
                        let params = self.params_to_types(sig.params);
                        let return_ty = self.parsed_type_to_type(&sig.return_ty);
                        methods.push(TraitMethodDef {
                            name: sig.name,
                            params,
                            return_ty,
                            has_default: false,
                        });
                    }
                    TraitItem::DefaultMethod(method) => {
                        let params = self.params_to_types(method.params);
                        let return_ty = self.parsed_type_to_type(&method.return_ty);
                        methods.push(TraitMethodDef {
                            name: method.name,
                            params,
                            return_ty,
                            has_default: true,
                        });
                    }
                    TraitItem::AssocType(at) => {
                        assoc_types.push(TraitAssocTypeDef {
                            name: at.name,
                        });
                    }
                }
            }

            let entry = TraitEntry {
                name: trait_def.name,
                span: trait_def.span,
                type_params,
                super_traits,
                methods,
                assoc_types,
                is_public: trait_def.is_public,
            };

            self.trait_registry.register_trait(entry);
        }
    }

    /// Register all implementation blocks from a module.
    pub(crate) fn register_impls(&mut self, module: &Module) {
        for impl_def in &module.impls {
            // Convert generic params to names
            let type_params: Vec<Name> = self.arena
                .get_generic_params(impl_def.generics)
                .iter()
                .map(|gp| gp.name)
                .collect();

            // Use the last segment of the trait path as the trait name.
            let trait_name = impl_def.trait_path.as_ref().and_then(|path| path.last().copied());

            // Convert self type
            let self_ty = self.parsed_type_to_type(&impl_def.self_ty);

            // Convert methods
            let methods: Vec<ImplMethodDef> = impl_def.methods
                .iter()
                .map(|m| {
                    let params = self.params_to_types(m.params);
                    let return_ty = self.parsed_type_to_type(&m.return_ty);
                    ImplMethodDef {
                        name: m.name,
                        params,
                        return_ty,
                    }
                })
                .collect();

            // Convert associated type definitions
            let assoc_types: Vec<ImplAssocTypeDef> = impl_def.assoc_types
                .iter()
                .map(|at| {
                    let ty = self.parsed_type_to_type(&at.ty);
                    ImplAssocTypeDef {
                        name: at.name,
                        ty,
                    }
                })
                .collect();

            let entry = ImplEntry {
                trait_name,
                self_ty: self_ty.clone(),
                span: impl_def.span,
                type_params,
                methods,
                assoc_types: assoc_types.clone(),
            };

            // For trait impls, validate that all required associated types are defined
            if let Some(trait_name) = trait_name {
                self.validate_associated_types(trait_name, &assoc_types, &self_ty, impl_def.span);
            }

            // Register impl, checking for coherence violations
            if let Err(coherence_err) = self.trait_registry.register_impl(entry) {
                self.errors.push(TypeCheckError {
                    message: format!(
                        "{} (previous impl at {:?})",
                        coherence_err.message,
                        coherence_err.existing_span
                    ),
                    span: coherence_err.span,
                    code: crate::diagnostic::ErrorCode::E2010,
                });
            }
        }
    }

    /// Validate that an impl block defines all required associated types from the trait.
    fn validate_associated_types(
        &mut self,
        trait_name: Name,
        impl_assoc_types: &[ImplAssocTypeDef],
        self_ty: &Type,
        span: crate::ir::Span,
    ) {
        // Get the trait definition
        let trait_entry = match self.trait_registry.get_trait(trait_name) {
            Some(entry) => entry.clone(),
            None => return, // Trait not found - error reported elsewhere
        };

        // Check each required associated type
        for required_at in &trait_entry.assoc_types {
            let defined = impl_assoc_types.iter().any(|at| at.name == required_at.name);
            if !defined {
                let trait_name_str = self.interner.lookup(trait_name);
                let assoc_name_str = self.interner.lookup(required_at.name);
                let type_name = self_ty.display(self.interner);
                self.errors.push(TypeCheckError {
                    message: format!(
                        "impl of `{trait_name_str}` for `{type_name}` missing associated type `{assoc_name_str}`"
                    ),
                    span,
                    code: crate::diagnostic::ErrorCode::E2012,
                });
            }
        }
    }

    /// Convert a parameter range to a vector of types.
    pub(crate) fn params_to_types(&mut self, params: ParamRange) -> Vec<Type> {
        self.arena
            .get_params(params)
            .iter()
            .map(|p| {
                match &p.ty {
                    Some(parsed_ty) => self.parsed_type_to_type(parsed_ty),
                    None => self.ctx.fresh_var(),
                }
            })
            .collect()
    }
}
