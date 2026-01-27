//! Trait and Impl Registration
//!
//! Registers trait definitions and implementations from modules.

use ori_ir::{Name, Module, TraitItem, ParamRange, GenericParamRange, ExprArena, TypeId};
use ori_types::Type;
use super::TypeChecker;
use crate::registry::{
    TraitEntry, TraitMethodDef, TraitAssocTypeDef, ImplEntry, ImplMethodDef, ImplAssocTypeDef,
};

/// Extract type parameter names from a generic parameter range.
fn extract_type_param_names(arena: &ExprArena, generics: GenericParamRange) -> Vec<Name> {
    arena.get_generic_params(generics)
        .iter()
        .map(|gp| gp.name)
        .collect()
}

impl TypeChecker<'_> {
    /// Register all trait definitions from a module.
    pub(crate) fn register_traits(&mut self, module: &Module) {
        for trait_def in &module.traits {
            let type_params = extract_type_param_names(self.context.arena, trait_def.generics);

            // Convert super-traits to names
            let super_traits: Vec<Name> = trait_def.super_traits
                .iter()
                .map(ori_ir::TraitBound::name)
                .collect();

            // Convert trait items
            let mut methods = Vec::new();
            let mut assoc_types = Vec::new();

            for item in &trait_def.items {
                match item {
                    TraitItem::MethodSig(sig) => {
                        let params = self.params_to_type_ids(sig.params);
                        let return_ty = self.parsed_type_to_type(&sig.return_ty);
                        let return_ty_id = return_ty.to_type_id(self.registries.traits.interner());
                        methods.push(TraitMethodDef {
                            name: sig.name,
                            params,
                            return_ty: return_ty_id,
                            has_default: false,
                        });
                    }
                    TraitItem::DefaultMethod(method) => {
                        let params = self.params_to_type_ids(method.params);
                        let return_ty = self.parsed_type_to_type(&method.return_ty);
                        let return_ty_id = return_ty.to_type_id(self.registries.traits.interner());
                        methods.push(TraitMethodDef {
                            name: method.name,
                            params,
                            return_ty: return_ty_id,
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

            self.registries.traits.register_trait(entry);
        }
    }

    /// Register all implementation blocks from a module.
    pub(crate) fn register_impls(&mut self, module: &Module) {
        for impl_def in &module.impls {
            let type_params = extract_type_param_names(self.context.arena, impl_def.generics);

            // Use the last segment of the trait path as the trait name.
            let trait_name = impl_def.trait_path.as_ref().and_then(|path| path.last().copied());

            // Convert self type
            let self_ty = self.parsed_type_to_type(&impl_def.self_ty);

            // First collect all types (requires mutable self)
            let methods_as_types: Vec<_> = impl_def.methods
                .iter()
                .map(|m| {
                    let params = self.params_to_types(m.params);
                    let return_ty = self.parsed_type_to_type(&m.return_ty);
                    (m.name, params, return_ty)
                })
                .collect();

            let assoc_types_as_types: Vec<_> = impl_def.assoc_types
                .iter()
                .map(|at| {
                    let ty = self.parsed_type_to_type(&at.ty);
                    (at.name, ty)
                })
                .collect();

            // Then convert to TypeIds (immutable interner borrow)
            let interner = self.registries.traits.interner();
            let methods: Vec<ImplMethodDef> = methods_as_types
                .into_iter()
                .map(|(name, params, return_ty)| {
                    ImplMethodDef {
                        name,
                        params: params.iter().map(|ty| ty.to_type_id(interner)).collect(),
                        return_ty: return_ty.to_type_id(interner),
                    }
                })
                .collect();

            let assoc_types: Vec<ImplAssocTypeDef> = assoc_types_as_types
                .into_iter()
                .map(|(name, ty)| {
                    ImplAssocTypeDef {
                        name,
                        ty: ty.to_type_id(interner),
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
            if let Err(coherence_err) = self.registries.traits.register_impl(entry) {
                self.push_error(
                    format!(
                        "{} (previous impl at {:?})",
                        coherence_err.message,
                        coherence_err.existing_span
                    ),
                    coherence_err.span,
                    ori_diagnostic::ErrorCode::E2010,
                );
            }
        }
    }

    /// Validate that an impl block defines all required associated types from the trait.
    fn validate_associated_types(
        &mut self,
        trait_name: Name,
        impl_assoc_types: &[ImplAssocTypeDef],
        self_ty: &Type,
        span: ori_ir::Span,
    ) {
        // Get the trait definition
        let trait_entry = match self.registries.traits.get_trait(trait_name) {
            Some(entry) => entry.clone(),
            None => return, // Trait not found - error reported elsewhere
        };

        // Check each required associated type
        for required_at in &trait_entry.assoc_types {
            let defined = impl_assoc_types.iter().any(|at| at.name == required_at.name);
            if !defined {
                let trait_name_str = self.context.interner.lookup(trait_name);
                let assoc_name_str = self.context.interner.lookup(required_at.name);
                let type_name = self_ty.display(self.context.interner);
                self.push_error(
                    format!(
                        "impl of `{trait_name_str}` for `{type_name}` missing associated type `{assoc_name_str}`"
                    ),
                    span,
                    ori_diagnostic::ErrorCode::E2012,
                );
            }
        }
    }

    /// Convert a parameter range to a vector of types.
    pub(crate) fn params_to_types(&mut self, params: ParamRange) -> Vec<Type> {
        self.context.arena
            .get_params(params)
            .iter()
            .map(|p| {
                match &p.ty {
                    Some(parsed_ty) => self.parsed_type_to_type(parsed_ty),
                    None => self.inference.ctx.fresh_var(),
                }
            })
            .collect()
    }

    /// Convert a parameter range to a vector of TypeIds.
    ///
    /// This is used when registering trait/impl methods where we want to
    /// store types as interned TypeIds for efficient equality comparisons.
    pub(crate) fn params_to_type_ids(&mut self, params: ParamRange) -> Vec<TypeId> {
        // First collect all types (requires mutable self for fresh_var)
        let types = self.params_to_types(params);
        // Then convert to TypeIds
        let interner = self.registries.traits.interner();
        types.iter().map(|ty| ty.to_type_id(interner)).collect()
    }
}
