//! Trait and Impl Registration
//!
//! Registers trait definitions and implementations from modules.

use crate::ir::{Name, Module, TraitItem, ParamRange};
use crate::types::Type;
use super::TypeChecker;
use super::types::TypeCheckError;
use super::super::type_registry::{
    TraitEntry, TraitMethodDef, TraitAssocTypeDef, ImplEntry, ImplMethodDef,
};

impl<'a> TypeChecker<'a> {
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
                .map(|b| b.name())
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

            // Convert trait path to single name (for now, just use last segment)
            let trait_name = impl_def.trait_path.as_ref().map(|path| {
                *path.last().expect("trait path cannot be empty")
            });

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

            let entry = ImplEntry {
                trait_name,
                self_ty,
                span: impl_def.span,
                type_params,
                methods,
            };

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
