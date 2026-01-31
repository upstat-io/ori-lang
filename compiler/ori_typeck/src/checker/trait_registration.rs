//! Trait and Impl Registration
//!
//! Registers trait definitions and implementations from modules.

use super::TypeChecker;
use crate::registry::{
    ImplAssocTypeDef, ImplEntry, ImplMethodDef, TraitAssocTypeDef, TraitEntry, TraitMethodDef,
};
use ori_ir::{
    ExprArena, GenericParam, GenericParamRange, Module, Name, ParamRange, TraitItem, TypeId,
};
use ori_types::Type;

/// Extract type parameter names from a generic parameter range.
fn extract_type_param_names(arena: &ExprArena, generics: GenericParamRange) -> Vec<Name> {
    arena
        .get_generic_params(generics)
        .iter()
        .map(|gp| gp.name)
        .collect()
}

/// Extract generic params for iteration.
fn get_generic_params(arena: &ExprArena, generics: GenericParamRange) -> Vec<GenericParam> {
    arena.get_generic_params(generics).to_vec()
}

impl TypeChecker<'_> {
    /// Register all trait definitions from a module.
    pub(crate) fn register_traits(&mut self, module: &Module) {
        for trait_def in &module.traits {
            let generic_params = get_generic_params(self.context.arena, trait_def.generics);
            let type_params: Vec<Name> = generic_params.iter().map(|gp| gp.name).collect();

            // Extract default types from generic params as ParsedType (unresolved).
            // We keep them as ParsedType because defaults may contain `Self`
            // which must be resolved at impl registration time, not here.
            let default_types: Vec<Option<ori_ir::ParsedType>> = generic_params
                .iter()
                .map(|gp| gp.default_type.clone())
                .collect();

            // Validate ordering: parameters with defaults must follow parameters without defaults
            self.validate_default_type_param_ordering(&generic_params, trait_def.span);

            // Convert super-traits to names
            let super_traits: Vec<Name> = trait_def
                .super_traits
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
                        assoc_types.push(TraitAssocTypeDef { name: at.name });
                    }
                }
            }

            let entry = TraitEntry::new(
                trait_def.name,
                trait_def.span,
                type_params,
                default_types,
                super_traits,
                methods,
                assoc_types,
                trait_def.visibility,
            );

            self.registries.traits.register_trait(entry);
        }
    }

    /// Register all implementation blocks from a module.
    pub(crate) fn register_impls(&mut self, module: &Module) {
        for impl_def in &module.impls {
            let type_params = extract_type_param_names(self.context.arena, impl_def.generics);

            // Use the last segment of the trait path as the trait name.
            let trait_name = impl_def
                .trait_path
                .as_ref()
                .and_then(|path| path.last().copied());

            // Convert self type
            let self_ty = self.parsed_type_to_type(&impl_def.self_ty);

            // For trait impls, resolve trait type arguments (filling in defaults as needed).
            // TODO: Use resolved_trait_type_args for method signature checking in a future PR.
            let _resolved_trait_type_args = if let Some(trait_name) = trait_name {
                self.resolve_trait_type_args(
                    trait_name,
                    impl_def.trait_type_args,
                    &self_ty,
                    impl_def.span,
                )
            } else {
                Vec::new()
            };

            // First collect all types (requires mutable self)
            let methods_as_types: Vec<_> = impl_def
                .methods
                .iter()
                .map(|m| {
                    let params = self.params_to_types(m.params);
                    let return_ty = self.parsed_type_to_type(&m.return_ty);
                    (m.name, params, return_ty)
                })
                .collect();

            let assoc_types_as_types: Vec<_> = impl_def
                .assoc_types
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
                .map(|(name, params, return_ty)| ImplMethodDef {
                    name,
                    params: params.iter().map(|ty| ty.to_type_id(interner)).collect(),
                    return_ty: return_ty.to_type_id(interner),
                })
                .collect();

            let assoc_types: Vec<ImplAssocTypeDef> = assoc_types_as_types
                .into_iter()
                .map(|(name, ty)| ImplAssocTypeDef {
                    name,
                    ty: ty.to_type_id(interner),
                })
                .collect();

            let entry = ImplEntry::new(
                trait_name,
                self_ty.clone(),
                impl_def.span,
                type_params,
                methods,
                assoc_types.clone(),
            );

            // For trait impls, validate that all required associated types are defined
            if let Some(trait_name) = trait_name {
                self.validate_associated_types(trait_name, &assoc_types, &self_ty, impl_def.span);
            }

            // Register impl, checking for coherence violations
            if let Err(coherence_err) = self.registries.traits.register_impl(entry) {
                self.push_error(
                    format!(
                        "{} (previous impl at {:?})",
                        coherence_err.message, coherence_err.existing_span
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
        use rustc_hash::FxHashSet;

        // Get the trait definition
        let trait_entry = match self.registries.traits.get_trait(trait_name) {
            Some(entry) => entry.clone(),
            None => return, // Trait not found - error reported elsewhere
        };

        // Build a set of provided associated types for O(1) lookup
        // This avoids O(n*m) nested iteration when validating multiple types
        let provided: FxHashSet<Name> = impl_assoc_types.iter().map(|at| at.name).collect();

        // Check each required associated type
        for required_at in &trait_entry.assoc_types {
            if !provided.contains(&required_at.name) {
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

    /// Resolve trait type arguments for an impl, filling in defaults as needed.
    ///
    /// Given `impl Add for Point` where `trait Add<Rhs = Self>`, this returns `[Point]`.
    /// Given `impl Add<int> for Point`, this returns `[int]`.
    ///
    /// # Arguments
    /// * `trait_name` - The trait being implemented
    /// * `provided_args` - Type arguments explicitly provided in the impl
    /// * `self_ty` - The implementing type (used to substitute `Self` in defaults)
    /// * `span` - Span for error reporting
    fn resolve_trait_type_args(
        &mut self,
        trait_name: Name,
        provided_args: ori_ir::ParsedTypeRange,
        self_ty: &Type,
        span: ori_ir::Span,
    ) -> Vec<Type> {
        // Get the trait definition
        let trait_entry = match self.registries.traits.get_trait(trait_name) {
            Some(entry) => entry.clone(),
            None => return Vec::new(), // Trait not found - error reported elsewhere
        };

        let required_params = trait_entry.type_params.len();
        let provided_arg_ids = self.context.arena.get_parsed_type_list(provided_args);
        let provided_count = provided_arg_ids.len();

        // Convert provided args to Types
        let mut result: Vec<Type> = provided_arg_ids
            .iter()
            .map(|&id| {
                let parsed_ty = self.context.arena.get_parsed_type(id).clone();
                self.parsed_type_to_type(&parsed_ty)
            })
            .collect();

        // Fill in remaining args with defaults
        for i in provided_count..required_params {
            if let Some(Some(default_parsed_ty)) = trait_entry.default_types.get(i) {
                // Resolve the ParsedType default, substituting Self with the implementing type
                let resolved_ty =
                    self.resolve_parsed_type_with_self_substitution(default_parsed_ty, self_ty);
                result.push(resolved_ty);
            } else {
                // No default provided and not enough args - error
                let trait_name_str = self.context.interner.lookup(trait_name);
                let param_name_str = self.context.interner.lookup(trait_entry.type_params[i]);
                self.push_error(
                    format!(
                        "impl of `{trait_name_str}` is missing type argument `{param_name_str}` which has no default"
                    ),
                    span,
                    ori_diagnostic::ErrorCode::E2016,
                );
            }
        }

        // Too many args provided
        if provided_count > required_params {
            let trait_name_str = self.context.interner.lookup(trait_name);
            self.push_error(
                format!(
                    "too many type arguments for `{trait_name_str}`: expected {required_params}, found {provided_count}"
                ),
                span,
                ori_diagnostic::ErrorCode::E2017,
            );
        }

        result
    }

    /// Resolve a `ParsedType` to a `Type`, substituting `Self` with a concrete type.
    ///
    /// This is used when resolving default type parameters that may contain `Self`.
    /// For example, in `trait Add<Rhs = Self>`, when we have `impl Add for Point`,
    /// `Self` becomes `Point`.
    fn resolve_parsed_type_with_self_substitution(
        &mut self,
        parsed: &ori_ir::ParsedType,
        self_ty: &Type,
    ) -> Type {
        match parsed {
            ori_ir::ParsedType::SelfType => self_ty.clone(),
            ori_ir::ParsedType::Primitive(type_id) => self.type_id_to_type(*type_id),
            ori_ir::ParsedType::Infer => self.inference.ctx.fresh_var(),
            ori_ir::ParsedType::Named { name, type_args } => {
                if type_args.is_empty() {
                    Type::Named(*name)
                } else {
                    // For named types with type args, resolve each arg
                    let arg_ids = self.context.arena.get_parsed_type_list(*type_args);
                    let resolved_args: Vec<Type> = arg_ids
                        .iter()
                        .map(|&id| {
                            let parsed_arg = self.context.arena.get_parsed_type(id).clone();
                            self.resolve_parsed_type_with_self_substitution(&parsed_arg, self_ty)
                        })
                        .collect();
                    Type::Applied {
                        name: *name,
                        args: resolved_args,
                    }
                }
            }
            ori_ir::ParsedType::List(inner_id) => {
                let parsed_inner = self.context.arena.get_parsed_type(*inner_id).clone();
                Type::List(Box::new(self.resolve_parsed_type_with_self_substitution(
                    &parsed_inner,
                    self_ty,
                )))
            }
            ori_ir::ParsedType::Tuple(elems) => {
                let elem_ids = self.context.arena.get_parsed_type_list(*elems);
                let resolved_elems: Vec<Type> = elem_ids
                    .iter()
                    .map(|&id| {
                        let parsed_elem = self.context.arena.get_parsed_type(id).clone();
                        self.resolve_parsed_type_with_self_substitution(&parsed_elem, self_ty)
                    })
                    .collect();
                Type::Tuple(resolved_elems)
            }
            ori_ir::ParsedType::Function { params, ret } => {
                let param_ids = self.context.arena.get_parsed_type_list(*params);
                let resolved_params: Vec<Type> = param_ids
                    .iter()
                    .map(|&id| {
                        let parsed_param = self.context.arena.get_parsed_type(id).clone();
                        self.resolve_parsed_type_with_self_substitution(&parsed_param, self_ty)
                    })
                    .collect();
                let parsed_ret = self.context.arena.get_parsed_type(*ret).clone();
                let resolved_ret =
                    self.resolve_parsed_type_with_self_substitution(&parsed_ret, self_ty);
                Type::Function {
                    params: resolved_params,
                    ret: Box::new(resolved_ret),
                }
            }
            ori_ir::ParsedType::Map { key, value } => {
                let parsed_key = self.context.arena.get_parsed_type(*key).clone();
                let parsed_value = self.context.arena.get_parsed_type(*value).clone();
                Type::Map {
                    key: Box::new(
                        self.resolve_parsed_type_with_self_substitution(&parsed_key, self_ty),
                    ),
                    value: Box::new(
                        self.resolve_parsed_type_with_self_substitution(&parsed_value, self_ty),
                    ),
                }
            }
            ori_ir::ParsedType::AssociatedType { base, assoc_name } => {
                let parsed_base = self.context.arena.get_parsed_type(*base).clone();
                let resolved_base =
                    self.resolve_parsed_type_with_self_substitution(&parsed_base, self_ty);
                Type::Projection {
                    base: Box::new(resolved_base),
                    trait_name: *assoc_name, // Placeholder
                    assoc_name: *assoc_name,
                }
            }
        }
    }

    /// Convert a parameter range to a vector of types.
    pub(crate) fn params_to_types(&mut self, params: ParamRange) -> Vec<Type> {
        self.context
            .arena
            .get_params(params)
            .iter()
            .map(|p| match &p.ty {
                Some(parsed_ty) => self.parsed_type_to_type(parsed_ty),
                None => self.inference.ctx.fresh_var(),
            })
            .collect()
    }

    /// Convert a parameter range to a vector of `TypeIds`.
    ///
    /// This is used when registering trait/impl methods where we want to
    /// store types as interned `TypeIds` for efficient equality comparisons.
    pub(crate) fn params_to_type_ids(&mut self, params: ParamRange) -> Vec<TypeId> {
        // First collect all types (requires mutable self for fresh_var)
        let types = self.params_to_types(params);
        // Then convert to TypeIds
        let interner = self.registries.traits.interner();
        types.iter().map(|ty| ty.to_type_id(interner)).collect()
    }

    /// Validate that generic parameters with defaults come after those without defaults.
    ///
    /// This enforces the constraint: `trait Foo<A, B = int, C = str>` is valid,
    /// but `trait Foo<A = int, B>` is invalid (non-default B after default A).
    fn validate_default_type_param_ordering(
        &mut self,
        generic_params: &[GenericParam],
        span: ori_ir::Span,
    ) {
        let mut seen_default = false;
        let mut first_default_name: Option<Name> = None;

        for param in generic_params {
            if param.default_type.is_some() {
                if !seen_default {
                    seen_default = true;
                    first_default_name = Some(param.name);
                }
            } else if seen_default {
                // Found a non-default parameter after a default parameter - error
                let non_default_name = self.context.interner.lookup(param.name);
                let default_name = first_default_name.map_or_else(
                    || "unknown".to_string(),
                    |n| self.context.interner.lookup(n).to_string(),
                );
                self.push_error(
                    format!(
                        "type parameter `{non_default_name}` without default must appear before type parameter `{default_name}` with default"
                    ),
                    span,
                    ori_diagnostic::ErrorCode::E2015,
                );
            }
        }
    }
}
