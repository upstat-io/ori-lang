//! Function, test, and impl method body type checking.
//!
//! Provides the second-pass type checking that validates function bodies,
//! test bodies, and impl method bodies against their declared signatures.
//! The shared `check_callable` method consolidates the common logic between
//! function and test checking.

use ori_ir::{ExprId, Function, Module, Name};
use ori_types::Type;
use rustc_hash::FxHashSet;

use super::types::FunctionType;
use super::TypeChecker;
use crate::infer;

impl TypeChecker<'_> {
    /// Type check a callable body (shared logic for functions and tests).
    ///
    /// Sets up a child environment with the given parameter bindings, establishes
    /// the current function type (for `recurse`/`self()` support), runs inference
    /// on the body, and unifies the result with the declared return type.
    fn check_callable(
        &mut self,
        params: &[(Name, Type)],
        return_type: &Type,
        body: ExprId,
        capabilities: FxHashSet<Name>,
    ) {
        // Create scope for parameters
        let mut callable_env = if let Some(ref base) = self.inference.base_env {
            base.child()
        } else {
            self.inference.env.child()
        };

        // Bind parameters and collect types in single pass
        let param_types: Vec<Type> = params
            .iter()
            .map(|(name, ty)| {
                callable_env.bind(*name, ty.clone());
                ty.clone()
            })
            .collect();
        let current_fn_type = Type::Function {
            params: param_types,
            ret: Box::new(return_type.clone()),
        };

        // Use RAII guards for environment, function type, and capability scopes
        let return_type = return_type.clone();
        self.with_custom_env_scope(callable_env, |checker| {
            checker.with_function_type_scope(current_fn_type, |checker| {
                checker.with_capability_scope(capabilities, |checker| {
                    let body_type = infer::infer_expr(checker, body);

                    if let Err(e) = checker.inference.ctx.unify(&body_type, &return_type) {
                        let span = checker.context.arena.get_expr(body).span;
                        checker.report_type_error(&e, span);
                    }
                });
            });
        });
    }

    /// Type check a function body.
    pub(super) fn check_function(&mut self, func: &Function, func_type: &FunctionType) {
        let interner = self.inference.base_env.as_ref().map_or_else(
            || self.inference.env.interner(),
            ori_types::TypeEnv::interner,
        );

        // Build parameter bindings: (Name, Type) pairs
        let param_defs = self.context.arena.get_params(func.params);
        let params: Vec<(Name, Type)> = param_defs
            .iter()
            .zip(func_type.params.iter())
            .map(|(param, &type_id)| (param.name, interner.to_type(type_id)))
            .collect();

        // Convert return type from TypeId to Type
        let return_type = interner.to_type(func_type.return_type);

        // Collect capabilities
        let capabilities: FxHashSet<Name> = func.capabilities.iter().map(|c| c.name).collect();

        self.check_callable(&params, &return_type, func.body, capabilities);
    }

    /// Type check a test body.
    pub(super) fn check_test(&mut self, test: &ori_ir::TestDef) {
        // Infer parameter types
        let param_defs = self.context.arena.get_params(test.params);
        let params: Vec<(Name, Type)> = param_defs
            .iter()
            .map(|p| {
                let ty = match &p.ty {
                    Some(parsed_ty) => self.parsed_type_to_type(parsed_ty),
                    None => self.inference.ctx.fresh_var(),
                };
                (p.name, ty)
            })
            .collect();

        // Infer return type
        let return_type = match &test.return_ty {
            Some(parsed_ty) => self.parsed_type_to_type(parsed_ty),
            None => self.inference.ctx.fresh_var(),
        };

        // Tests don't declare capabilities
        self.check_callable(&params, &return_type, test.body, FxHashSet::default());
    }

    /// Type check all methods in an impl block.
    pub(super) fn check_impl_methods(&mut self, impl_def: &ori_ir::ImplDef) {
        let self_ty = self.parsed_type_to_type(&impl_def.self_ty);

        self.with_impl_scope(self_ty.clone(), |checker| {
            for method in &impl_def.methods {
                checker.check_impl_method(method, &self_ty);
            }
        });
    }

    /// Type check a single impl method.
    fn check_impl_method(&mut self, method: &ori_ir::ImplMethod, self_ty: &Type) {
        // Create scope for method parameters
        let mut method_env = if let Some(ref base) = self.inference.base_env {
            base.child()
        } else {
            self.inference.env.child()
        };

        // Bind parameters (first param is typically `self`)
        let params = self.context.arena.get_params(method.params);
        for param in params {
            let param_ty = if let Some(ref parsed_ty) = param.ty {
                self.parsed_type_to_type(parsed_ty)
            } else {
                // If first param is named `self`, bind to Self type
                let self_name = self.context.interner.intern("self");
                if param.name == self_name {
                    self_ty.clone()
                } else {
                    self.inference.ctx.fresh_var()
                }
            };
            method_env.bind(param.name, param_ty);
        }

        // Use RAII guard for environment scope
        let return_type = self.parsed_type_to_type(&method.return_ty);
        self.with_custom_env_scope(method_env, |checker| {
            let body_type = infer::infer_expr(checker, method.body);

            if let Err(e) = checker.inference.ctx.unify(&body_type, &return_type) {
                let span = checker.context.arena.get_expr(method.body).span;
                checker.report_type_error(&e, span);
            }
        });
    }

    /// Register config variable types.
    ///
    /// Infers the type of each config value and stores it for $name references.
    pub(super) fn register_configs(&mut self, module: &Module) {
        for config in &module.configs {
            let config_ty = infer::infer_expr(self, config.value);
            self.scope.config_types.insert(config.name, config_ty);
        }
    }

    /// Type check all methods in a def impl block.
    ///
    /// Unlike regular impl blocks, def impl methods don't have `self` parameter.
    /// They're stateless default implementations for capabilities.
    pub(super) fn check_def_impl_methods(&mut self, def_impl_def: &ori_ir::DefImplDef) {
        for method in &def_impl_def.methods {
            self.check_def_impl_method(method);
        }
    }

    /// Type check a single def impl method.
    ///
    /// Similar to `check_impl_method` but without `self` binding.
    fn check_def_impl_method(&mut self, method: &ori_ir::ImplMethod) {
        // Create scope for method parameters
        let mut method_env = if let Some(ref base) = self.inference.base_env {
            base.child()
        } else {
            self.inference.env.child()
        };

        // Bind all parameters (no special handling for self - there isn't one)
        let params = self.context.arena.get_params(method.params);
        for param in params {
            let param_ty = if let Some(ref parsed_ty) = param.ty {
                self.parsed_type_to_type(parsed_ty)
            } else {
                self.inference.ctx.fresh_var()
            };
            method_env.bind(param.name, param_ty);
        }

        // Use RAII guard for environment scope
        let return_type = self.parsed_type_to_type(&method.return_ty);
        self.with_custom_env_scope(method_env, |checker| {
            let body_type = infer::infer_expr(checker, method.body);

            if let Err(e) = checker.inference.ctx.unify(&body_type, &return_type) {
                let span = checker.context.arena.get_expr(method.body).span;
                checker.report_type_error(&e, span);
            }
        });
    }
}
