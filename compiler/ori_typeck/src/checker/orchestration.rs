//! Module type checking orchestration.
//!
//! Contains the `check_module` implementation with its 4-pass logic.

use super::types::{FunctionType, TypedModule};
use super::TypeChecker;
use ori_ir::{Module, TypeId};
use ori_types::{Type, TypeData, TypeScheme};

impl TypeChecker<'_> {
    /// Type check a module.
    pub fn check_module(mut self, module: &Module) -> TypedModule {
        let mut function_types = Vec::new();

        // Pass 0a: Register built-in types (Ordering, etc.)
        // Must be done before user types, as user code may reference these.
        crate::registry::register_builtin_types(&mut self.registries.types, self.context.interner);

        // Pass 0b: Register user-defined types (structs, enums, newtypes)
        // Must be done before traits, as traits/impls may reference these types.
        self.register_types(module);

        // Pass 0c: Register traits and implementations
        self.register_traits(module);
        self.register_impls(module);
        self.register_def_impls(module);

        // Pass 0d: Register derived trait implementations
        // Must be done after register_types so we know the type structure,
        // but after register_impls so explicit impls take precedence.
        crate::derives::register_derived_impls(
            module,
            &mut self.registries.traits,
            self.context.interner,
        );

        // Pass 0e: Register config variables
        self.register_configs(module);

        // First pass: collect function signatures
        for func in &module.functions {
            let func_type = self.infer_function_signature(func);

            // Validate capabilities in uses clause
            self.validate_capabilities(func);

            // Store signature for constraint checking during calls (clone once)
            self.scope
                .function_sigs
                .insert(func.name, func_type.clone());

            // Bind function name to its type
            // For generic functions, create a polymorphic type scheme
            // so each call site gets fresh type variables
            let interner = self.inference.env.interner();
            let fn_type = Type::Function {
                params: func_type
                    .params
                    .iter()
                    .map(|&id| interner.to_type(id))
                    .collect(),
                ret: Box::new(interner.to_type(func_type.return_type)),
            };

            // Extract type vars from generic parameters
            let type_vars: Vec<_> = func_type
                .generics
                .iter()
                .filter_map(|g| {
                    if let TypeData::Var(tv) = interner.lookup(g.type_var) {
                        Some(tv)
                    } else {
                        None
                    }
                })
                .collect();

            if type_vars.is_empty() {
                self.inference.env.bind(func.name, fn_type);
            } else {
                let scheme = TypeScheme::poly(type_vars, fn_type);
                self.inference.env.bind_scheme(func.name, scheme);
            }

            // Move into vector at end (no clone needed)
            function_types.push(func_type);
        }

        // Freeze the base environment for child scope creation.
        // This avoids modifying the base during function checking.
        self.inference.base_env = Some(std::mem::take(&mut self.inference.env));

        // Second pass: type check function bodies
        for (func, func_type) in module.functions.iter().zip(function_types.iter()) {
            self.check_function(func, func_type);
        }

        // Third pass: type check test bodies
        for test in &module.tests {
            self.check_test(test);
        }

        // Fourth pass: type check impl method bodies
        for impl_def in &module.impls {
            self.check_impl_methods(impl_def);
        }

        // Fifth pass: type check def impl method bodies (default implementations)
        for def_impl_def in &module.def_impls {
            self.check_def_impl_methods(def_impl_def);
        }

        // Build expression types vector with resolved types
        // expr_types already stores TypeId, just need to resolve type variables
        let interner = self.inference.base_env.as_ref().map_or_else(
            || self.inference.env.interner(),
            ori_types::TypeEnv::interner,
        );
        let max_expr = self.inference.expr_types.keys().max().copied().unwrap_or(0);
        let mut expr_types = vec![interner.error(); max_expr + 1];
        for (id, type_id) in self.inference.expr_types {
            expr_types[id] = self.inference.ctx.resolve_id(type_id);
        }

        // Resolve function types (params and return_type are already TypeId)
        let resolved_function_types: Vec<FunctionType> = function_types
            .into_iter()
            .map(|ft| {
                // Resolve each param TypeId through the inference context
                let resolved_params: Vec<TypeId> = ft
                    .params
                    .iter()
                    .map(|&type_id| {
                        let ty = interner.to_type(type_id);
                        let resolved = self.inference.ctx.resolve(&ty);
                        resolved.to_type_id(interner)
                    })
                    .collect();
                let ret_ty = interner.to_type(ft.return_type);
                let resolved_ret = self.inference.ctx.resolve(&ret_ty);

                FunctionType {
                    name: ft.name,
                    generics: ft.generics,
                    where_constraints: ft.where_constraints,
                    params: resolved_params,
                    return_type: resolved_ret.to_type_id(interner),
                    capabilities: ft.capabilities,
                }
            })
            .collect();

        // Create the error guarantee from the error count
        let error_guarantee =
            ori_diagnostic::ErrorGuaranteed::from_error_count(self.diagnostics.errors.len());

        TypedModule {
            expr_types,
            function_types: resolved_function_types,
            errors: self.diagnostics.errors,
            error_guarantee,
        }
    }
}
