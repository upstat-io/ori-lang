//! Function body type checking passes.
//!
//! This module implements Passes 2-5 of module type checking:
//! - Pass 2: Function bodies
//! - Pass 3: Test bodies
//! - Pass 4: Impl method bodies
//! - Pass 5: Def impl (default implementation) method bodies
//!
//! # Architecture
//!
//! Each function body is checked in a child environment that:
//! 1. Inherits from the frozen base environment (contains all function signatures)
//! 2. Has parameter bindings added
//! 3. Has function scope context set (`current_function`, capabilities)
//!
//! ```text
//! Base Environment (frozen after Pass 1)
//!     │
//!     ├─ child for function foo
//!     │   ├─ param: x -> int
//!     │   └─ param: y -> str
//!     │
//!     └─ child for function bar
//!         └─ param: n -> int
//! ```

use ori_ir::{Function, ImplMethod, Module, Name, TestDef, TraitDef, TraitItem};
use rustc_hash::FxHashSet;

use super::registration::{resolve_parsed_type_simple, resolve_type_with_self};
use super::ModuleChecker;
use crate::{check_expr, infer_expr, ContextKind, Expected, ExpectedOrigin, FunctionSig, Idx};

/// Check all function bodies.
///
/// This pass runs after signature collection (Pass 1). Each function body
/// is type-checked against its declared return type.
#[tracing::instrument(level = "debug", skip_all, fields(count = module.functions.len()))]
pub fn check_function_bodies(checker: &mut ModuleChecker<'_>, module: &Module) {
    for func in &module.functions {
        check_function(checker, func);
    }
}

/// Type check a single function body.
fn check_function(checker: &mut ModuleChecker<'_>, func: &Function) {
    // Look up the pre-collected signature
    let Some(sig) = checker.get_signature(func.name).cloned() else {
        // This should never happen if Pass 1 ran correctly
        checker.error_undefined(func.name, func.span);
        return;
    };

    // Create child environment from frozen base
    let Some(child_env) = checker.child_of_base() else {
        // Base env not frozen - internal error
        return;
    };

    // Bind parameters in the child environment
    let mut param_env = child_env;
    for (name, ty) in sig.param_names.iter().zip(&sig.param_types) {
        param_env.bind(*name, *ty);
    }

    // Bind const generic parameters as their declared type.
    // E.g., for `@f<$N: int>`, bind N -> int so the body can reference N.
    for cp in &sig.const_params {
        param_env.bind(cp.name, cp.const_type);
    }

    // Bind capability names as fresh type variables so the body can
    // reference them (e.g., `@f () -> int uses Value = Value`).
    // The concrete type is provided by the caller via `with...in`.
    for &cap_name in &sig.capabilities {
        let cap_ty = checker.pool_mut().fresh_var();
        param_env.bind(cap_name, cap_ty);
    }

    // Build function type for recursion support
    let fn_type = checker
        .pool_mut()
        .function(&sig.param_types, sig.return_type);

    // Extract capabilities for scope context
    let capabilities: FxHashSet<Name> = sig.capabilities.iter().copied().collect();

    // Get spans before entering the checking scope
    let guard_span = func.guard.map(|id| checker.arena().get_expr(id).span);
    let body_span = checker.arena().get_expr(func.body).span;

    // Check body with function scope context
    let (expr_types, errors, warnings, pat_resolutions) =
        checker.with_function_scope(fn_type, capabilities, |c| {
            // Get arena reference (lifetime 'a, not tied to c borrow)
            let arena = c.arena();

            // Create inference engine with prepared environment
            let mut engine = c.create_engine_with_env(param_env);

            // Set self type for recursive calls (self() in patterns like recurse)
            engine.set_self_type(fn_type);

            // Push context for better error messages
            engine.push_context(ContextKind::FunctionReturn {
                func_name: Some(func.name),
            });

            // Check guard expression if present
            if let (Some(guard_id), Some(span)) = (func.guard, guard_span) {
                let guard_ty = infer_expr(&mut engine, arena, guard_id);
                let expected_bool = Expected {
                    ty: Idx::BOOL,
                    origin: ExpectedOrigin::Context {
                        span,
                        kind: ContextKind::MatchArmGuard { arm_index: 0 }, // Reuse guard context
                    },
                };
                let _ = engine.check_type(guard_ty, &expected_bool, span);
            }

            // Check body against declared return type (bidirectional)
            let expected = Expected {
                ty: sig.return_type,
                origin: ExpectedOrigin::Context {
                    span: body_span,
                    kind: ContextKind::FunctionReturn {
                        func_name: Some(func.name),
                    },
                },
            };
            let _body_ty = check_expr(&mut engine, arena, func.body, &expected, body_span);

            engine.pop_context();

            // Return expression types, errors, warnings, and pattern resolutions
            (
                engine.take_expr_types(),
                engine.take_errors(),
                engine.take_warnings(),
                engine.take_pattern_resolutions(),
            )
        });

    // Store expression types
    for (expr_index, ty) in expr_types {
        checker.store_expr_type(expr_index, ty);
    }

    // Store errors and warnings
    for error in errors {
        checker.push_error(error);
    }
    for warning in warnings {
        checker.push_warning(warning);
    }

    // Accumulate pattern resolutions
    checker.pattern_resolutions.extend(pat_resolutions);
}

/// Check all test bodies.
///
/// Tests are similar to functions but:
/// - Always return unit (void)
/// - May have special test parameters
#[tracing::instrument(level = "debug", skip_all, fields(count = module.tests.len()))]
pub fn check_test_bodies(checker: &mut ModuleChecker<'_>, module: &Module) {
    for test in &module.tests {
        check_test(checker, test);
    }
}

/// Type check a single test body.
fn check_test(checker: &mut ModuleChecker<'_>, test: &TestDef) {
    // Look up pre-collected signature
    let Some(sig) = checker.get_signature(test.name).cloned() else {
        checker.error_undefined(test.name, test.span);
        return;
    };

    // Create child environment
    let Some(child_env) = checker.child_of_base() else {
        return;
    };

    // Bind parameters
    let mut param_env = child_env;
    for (name, ty) in sig.param_names.iter().zip(&sig.param_types) {
        param_env.bind(*name, *ty);
    }

    // Get arena reference (lifetime 'a, not tied to checker borrow)
    let arena = checker.arena();

    // Create inference engine and check body
    let fn_type = checker
        .pool_mut()
        .function(&sig.param_types, sig.return_type);
    let mut engine = checker.create_engine_with_env(param_env);
    engine.set_self_type(fn_type);

    // Push test context
    engine.push_context(ContextKind::TestBody);

    // Check body against declared return type (bidirectional)
    let body_span = arena.get_expr(test.body).span;
    let expected = Expected {
        ty: sig.return_type,
        origin: ExpectedOrigin::Context {
            span: body_span,
            kind: ContextKind::FunctionReturn {
                func_name: Some(test.name),
            },
        },
    };
    let _body_ty = check_expr(&mut engine, arena, test.body, &expected, body_span);

    engine.pop_context();

    // Extract results
    let expr_types = engine.take_expr_types();
    let errors = engine.take_errors();
    let warnings = engine.take_warnings();
    let pat_resolutions = engine.take_pattern_resolutions();

    // Store expression types
    for (expr_index, ty) in expr_types {
        checker.store_expr_type(expr_index, ty);
    }

    // Store errors and warnings
    for error in errors {
        checker.push_error(error);
    }
    for warning in warnings {
        checker.push_warning(warning);
    }

    // Accumulate pattern resolutions
    checker.pattern_resolutions.extend(pat_resolutions);
}

/// Check all impl method bodies.
///
/// For trait impls, this also checks unoverridden default methods from the trait
/// definition, registering their signatures for LLVM codegen.
#[tracing::instrument(level = "debug", skip_all, fields(count = module.impls.len()))]
pub fn check_impl_bodies(checker: &mut ModuleChecker<'_>, module: &Module) {
    for impl_def in &module.impls {
        check_impl_block(checker, impl_def, &module.traits);
    }
}

/// Type check methods in an impl block.
///
/// Processes explicit methods first, then unoverridden default methods from the
/// trait definition. Both register signatures via `register_impl_sig` for LLVM
/// codegen consumption (signatures are consumed positionally by `compile_impls`).
fn check_impl_block(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
    traits: &[TraitDef],
) {
    // Resolve the Self type for this impl block
    let arena = checker.arena();
    let self_type = resolve_parsed_type_simple(checker, &impl_def.self_ty, arena);

    // Collect generic params for type resolution within methods
    let generic_params: Vec<Name> = checker
        .arena()
        .get_generic_params(impl_def.generics)
        .iter()
        .map(|p| p.name)
        .collect();

    // Check explicitly defined methods
    for method in &impl_def.methods {
        check_impl_method(checker, method, self_type, &generic_params);
    }

    // For trait impls, also check unoverridden default methods.
    // This registers their signatures so LLVM codegen can compile them.
    if let Some(trait_path) = &impl_def.trait_path {
        if let Some(&trait_name) = trait_path.last() {
            let overridden: FxHashSet<Name> = impl_def.methods.iter().map(|m| m.name).collect();

            if let Some(trait_def) = traits.iter().find(|t| t.name == trait_name) {
                for item in &trait_def.items {
                    if let TraitItem::DefaultMethod(default) = item {
                        if !overridden.contains(&default.name) {
                            let as_impl = ImplMethod::from(default);
                            check_impl_method(checker, &as_impl, self_type, &generic_params);
                        }
                    }
                }
            }
        }
    }
}

/// Type check a single impl method body.
fn check_impl_method(
    checker: &mut ModuleChecker<'_>,
    method: &ImplMethod,
    self_type: Idx,
    type_params: &[Name],
) {
    // Create child environment from frozen base
    let Some(child_env) = checker.child_of_base() else {
        return;
    };

    // Resolve parameter types with Self substitution
    let params: Vec<_> = checker.arena().get_params(method.params).to_vec();
    let mut param_env = child_env;

    let mut param_types = Vec::with_capacity(params.len());
    for p in &params {
        let is_self = p.name == checker.well_known().self_kw;
        let ty = match &p.ty {
            Some(parsed_ty) => resolve_type_with_self(checker, parsed_ty, type_params, self_type),
            None if is_self => self_type,
            None => checker.pool_mut().fresh_var(),
        };
        param_env.bind(p.name, ty);
        param_types.push(ty);
    }

    // Resolve return type with Self substitution
    let return_type = resolve_type_with_self(checker, &method.return_ty, type_params, self_type);

    // Build function type for recursion support
    let fn_type = checker.pool_mut().function(&param_types, return_type);

    // Get body span before entering scope
    let body_span = checker.arena().get_expr(method.body).span;

    // Check body within impl scope + function scope
    let (expr_types, errors, warnings, pat_resolutions) = checker.with_impl_scope(self_type, |c| {
        c.with_function_scope(fn_type, FxHashSet::default(), |c| {
            let arena = c.arena();
            let mut engine = c.create_engine_with_env(param_env);

            engine.push_context(ContextKind::FunctionReturn {
                func_name: Some(method.name),
            });

            // Check body against declared return type (bidirectional)
            let expected = Expected {
                ty: return_type,
                origin: ExpectedOrigin::Context {
                    span: body_span,
                    kind: ContextKind::FunctionReturn {
                        func_name: Some(method.name),
                    },
                },
            };
            let _body_ty = check_expr(&mut engine, arena, method.body, &expected, body_span);

            engine.pop_context();

            (
                engine.take_expr_types(),
                engine.take_errors(),
                engine.take_warnings(),
                engine.take_pattern_resolutions(),
            )
        })
    });

    // Store results
    for (expr_index, ty) in expr_types {
        checker.store_expr_type(expr_index, ty);
    }
    for error in errors {
        checker.push_error(error);
    }
    for warning in warnings {
        checker.push_warning(warning);
    }
    checker.pattern_resolutions.extend(pat_resolutions);

    // Export impl method signature for codegen.
    // Codegen needs param_types, return_type, and type_params to compute ABI.
    let param_names: Vec<Name> = params.iter().map(|p| p.name).collect();
    let param_defaults: Vec<Option<ori_ir::ExprId>> = params.iter().map(|p| p.default).collect();
    let required_params = param_defaults.iter().filter(|d| d.is_none()).count();
    let sig = FunctionSig {
        name: method.name,
        type_params: type_params.to_vec(),
        const_params: vec![],
        param_names,
        param_types,
        return_type,
        capabilities: vec![],
        is_public: false,
        is_test: false,
        is_main: false,
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        required_params,
        param_defaults,
    };
    checker.register_impl_sig(method.name, sig);
}

// ============================================================================
// Pass 5: Def Impl (Default Implementation) Method Bodies
// ============================================================================

/// Check all def impl method bodies.
#[tracing::instrument(level = "debug", skip_all, fields(count = module.def_impls.len()))]
pub fn check_def_impl_bodies(checker: &mut ModuleChecker<'_>, module: &Module) {
    for def_impl in &module.def_impls {
        check_def_impl_block(checker, def_impl);
    }
}

/// Type check methods in a def impl block.
///
/// `def impl` methods are stateless (no `self` parameter) and don't have
/// a `for Type` clause. They provide default behavior for a capability trait.
fn check_def_impl_block(checker: &mut ModuleChecker<'_>, def_impl: &ori_ir::DefImplDef) {
    for method in &def_impl.methods {
        check_def_impl_method(checker, method);
    }
}

/// Type check a single def impl method body.
fn check_def_impl_method(checker: &mut ModuleChecker<'_>, method: &ImplMethod) {
    // Create child environment from frozen base
    let Some(child_env) = checker.child_of_base() else {
        return;
    };

    // Resolve parameter types (no Self substitution for def impl)
    let arena = checker.arena();
    let params: Vec<_> = arena.get_params(method.params).to_vec();
    let mut param_env = child_env;

    let mut param_types = Vec::with_capacity(params.len());
    for p in &params {
        let ty = match &p.ty {
            Some(parsed_ty) => resolve_parsed_type_simple(checker, parsed_ty, arena),
            None => checker.pool_mut().fresh_var(),
        };
        param_env.bind(p.name, ty);
        param_types.push(ty);
    }

    // Resolve return type
    let return_type = resolve_parsed_type_simple(checker, &method.return_ty, arena);

    // Build function type
    let fn_type = checker.pool_mut().function(&param_types, return_type);

    // Get body span
    let body_span = checker.arena().get_expr(method.body).span;

    // Check body with function scope only (no impl scope for def impl)
    let (expr_types, errors, warnings, pat_resolutions) =
        checker.with_function_scope(fn_type, FxHashSet::default(), |c| {
            let arena = c.arena();
            let mut engine = c.create_engine_with_env(param_env);

            engine.push_context(ContextKind::FunctionReturn {
                func_name: Some(method.name),
            });

            // Check body against declared return type (bidirectional)
            let expected = Expected {
                ty: return_type,
                origin: ExpectedOrigin::Context {
                    span: body_span,
                    kind: ContextKind::FunctionReturn {
                        func_name: Some(method.name),
                    },
                },
            };
            let _body_ty = check_expr(&mut engine, arena, method.body, &expected, body_span);

            engine.pop_context();

            (
                engine.take_expr_types(),
                engine.take_errors(),
                engine.take_warnings(),
                engine.take_pattern_resolutions(),
            )
        });

    // Store results
    for (expr_index, ty) in expr_types {
        checker.store_expr_type(expr_index, ty);
    }
    for error in errors {
        checker.push_error(error);
    }
    for warning in warnings {
        checker.push_warning(warning);
    }
    checker.pattern_resolutions.extend(pat_resolutions);
}

#[cfg(test)]
mod tests;
