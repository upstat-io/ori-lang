//! Function signature collection pass.
//!
//! This module implements Pass 1 of module type checking: collecting all function
//! signatures before body checking. This enables:
//!
//! - **Mutual recursion:** Function A can call B, and B can call A
//! - **Forward references:** Functions defined later in the file can be called
//! - **Let-polymorphism:** Generic functions get fresh type variables per call site
//!
//! # Architecture
//!
//! ```text
//! Module.functions
//!     ↓
//! infer_function_signature() ← Creates FunctionSig
//!     ↓
//! checker.signatures ← Stores for call resolution
//!     ↓
//! checker.base_env ← Binds function types in environment
//! ```

mod resolution;

use ori_ir::{ExprArena, Function, Module, Name, ParsedType, TestDef, Visibility as IrVisibility};
use rustc_hash::FxHashMap;

use super::ModuleChecker;
use crate::{ConstParamInfo, FnWhereClause, FunctionSig, Idx};
use resolution::resolve_and_check_type_with_vars;
pub(crate) use resolution::resolve_const_param_type;

// Pass 1: Signature Collection

/// Collect all function signatures.
///
/// This pass runs before body checking to enable mutual recursion and forward
/// references. After collection, the base environment is frozen.
#[tracing::instrument(level = "debug", skip_all, fields(
    functions = module.functions.len(),
    tests = module.tests.len(),
))]
pub fn collect_signatures(checker: &mut ModuleChecker<'_>, module: &Module) {
    // Create a child of the import environment so imported bindings are
    // visible as the parent scope. Local function bindings shadow imports.
    //
    // Environment chain after freeze:
    //   import_env → base_env (frozen) → child_env (per-function body)
    let mut env = checker.import_env().child();

    // Collect signatures for all regular functions
    for func in &module.functions {
        let (sig, var_ids) = infer_function_signature(checker, func);

        // Create function type for environment binding
        let fn_type = checker
            .pool_mut()
            .function(&sig.param_types, sig.return_type);

        // Generic functions must be wrapped in a type scheme so each call
        // site gets fresh type variables via instantiation.
        let bound_type = if var_ids.is_empty() {
            fn_type
        } else {
            checker.pool_mut().scheme(&var_ids, fn_type)
        };
        env.bind(func.name, bound_type);

        // Store signature for call resolution
        store_signature(checker, sig);
    }

    // Also collect test signatures (tests are function-like)
    for test in &module.tests {
        let sig = infer_test_signature(checker, test);
        let fn_type = checker
            .pool_mut()
            .function(&sig.param_types, sig.return_type);
        env.bind(test.name, fn_type);
        store_signature(checker, sig);
    }

    // Freeze the environment - body checking creates children from this base
    checker.freeze_base_env(env);
}

/// Store a function signature in the checker.
fn store_signature(checker: &mut ModuleChecker<'_>, sig: FunctionSig) {
    // Access signatures through a helper method
    checker.register_signature(sig);
}

// Signature Inference

/// Infer the signature of a function.
///
/// Creates fresh type variables for generic parameters and resolves
/// parameter/return types in that context.
fn infer_function_signature(
    checker: &mut ModuleChecker<'_>,
    func: &Function,
) -> (FunctionSig, Vec<u32>) {
    let arena = checker.arena();
    infer_function_signature_with_arena(checker, func, arena)
}

/// Infer the signature of a function from a foreign module's arena.
///
/// This is used during import registration to create signatures for imported
/// functions. The `foreign_arena` is used for all AST lookups (generic params,
/// parameters, parsed types), while the checker's local pool is used to create
/// fresh type variables.
///
/// Returns both the signature and the var IDs for generic type parameters,
/// which callers need to wrap the function type in a scheme.
pub(super) fn infer_function_signature_from(
    checker: &mut ModuleChecker<'_>,
    func: &Function,
    foreign_arena: &ExprArena,
) -> (FunctionSig, Vec<u32>) {
    infer_function_signature_with_arena(checker, func, foreign_arena)
}

/// Generic type/const-param collection results: declared type-param names,
/// declared const-param metadata, the name→fresh-var substitution map, and
/// the var ids parallel to `type_params` (NOT `FxHashMap` iteration order —
/// `scheme_var_ids[i]` must correspond to `type_params[i]` and
/// `generic_param_mapping[i]` for monomorphization substitution).
struct GenericParamCollection {
    type_params: Vec<Name>,
    const_params: Vec<ConstParamInfo>,
    type_param_vars: FxHashMap<Name, Idx>,
    var_ids: Vec<u32>,
}

/// Collect a function's generic type + const params (filtering const params
/// out of the type-param list — they are values, not types) and bind each
/// type param to a fresh unification variable.
fn collect_generic_params(
    checker: &mut ModuleChecker<'_>,
    generic_params: &[ori_ir::GenericParam],
) -> GenericParamCollection {
    let type_params: Vec<Name> = generic_params
        .iter()
        .filter(|p| !p.is_const)
        .map(|p| p.name)
        .collect();

    let const_params: Vec<ConstParamInfo> = generic_params
        .iter()
        .filter(|p| p.is_const)
        .map(|p| {
            let const_type = resolve_const_param_type(checker, p);
            ConstParamInfo {
                name: p.name,
                const_type,
                default_value: p.default_value,
            }
        })
        .collect();

    let type_param_vars: FxHashMap<Name, Idx> = type_params
        .iter()
        .map(|&name| {
            let var = checker.pool_mut().fresh_named_var(name);
            (name, var)
        })
        .collect();

    let var_ids: Vec<u32> = type_params
        .iter()
        .map(|name| checker.pool().data(type_param_vars[name]))
        .collect();

    GenericParamCollection {
        type_params,
        const_params,
        type_param_vars,
        var_ids,
    }
}

/// Extract a function's declared capabilities and, for each non-marker
/// capability, allocate a fresh provider-type variable retaining the ordered
/// provider schema in Pass 1, before any caller or body is checked.
fn collect_capability_params(
    checker: &mut ModuleChecker<'_>,
    func: &Function,
) -> (Vec<Name>, Vec<crate::CapabilityParam>) {
    let capabilities: Vec<Name> = func.capabilities.iter().map(|c| c.name).collect();
    let capability_params: Vec<crate::CapabilityParam> = capabilities
        .iter()
        .map(|&capability| {
            if crate::is_marker_capability(capability, checker.interner()) {
                crate::CapabilityParam::Marker { capability }
            } else {
                let provider_type = checker.pool_mut().fresh_named_var(capability);
                crate::CapabilityParam::Value {
                    capability,
                    provider_type,
                    provider_var_id: checker.pool().data(provider_type),
                }
            }
        })
        .collect();
    (capabilities, capability_params)
}

/// Collect per-type-param trait bounds, the function's where-clauses (type
/// bounds only; const bounds are deferred), and the map from each generic
/// type param to the function param that directly uses it.
fn collect_bounds_and_mapping(
    func: &Function,
    generic_params: &[ori_ir::GenericParam],
    type_params: &[Name],
    type_param_vars: &FxHashMap<Name, Idx>,
    param_types: &[Idx],
) -> (Vec<Vec<Name>>, Vec<FnWhereClause>, Vec<Option<usize>>) {
    let type_param_bounds: Vec<Vec<Name>> = generic_params
        .iter()
        .filter(|p| !p.is_const)
        .map(|p| p.bounds.iter().map(ori_ir::TraitBound::name).collect())
        .collect();

    let where_clauses: Vec<FnWhereClause> = func
        .where_clauses
        .iter()
        .filter_map(|wc| {
            let (param, projection, bounds, span) = wc.as_type_bound()?;
            Some(FnWhereClause {
                param,
                projection,
                bounds: bounds.iter().map(ori_ir::TraitBound::name).collect(),
                span,
            })
        })
        .collect();

    let generic_param_mapping: Vec<Option<usize>> = type_params
        .iter()
        .map(|tp_name| {
            let var_idx = type_param_vars[tp_name];
            param_types.iter().position(|&ty| ty == var_idx)
        })
        .collect();

    (type_param_bounds, where_clauses, generic_param_mapping)
}

/// Shared implementation for inferring a function signature from any arena.
///
/// Returns the signature and the var IDs of generic type parameters.
fn infer_function_signature_with_arena(
    checker: &mut ModuleChecker<'_>,
    func: &Function,
    arena: &ExprArena,
) -> (FunctionSig, Vec<u32>) {
    let generic_params = arena.get_generic_params(func.generics);
    let GenericParamCollection {
        type_params,
        const_params,
        type_param_vars,
        var_ids,
    } = collect_generic_params(checker, generic_params);

    let (param_names, param_types, param_defaults, required_params) =
        resolve_function_params(checker, func, arena, &type_param_vars);

    // Resolve return type
    let return_type = match &func.return_ty {
        Some(parsed_ty) => {
            resolve_and_check_type_with_vars(checker, parsed_ty, &type_param_vars, func.span, arena)
        }
        // No return type annotation: infer from the body.
        // Use a fresh type variable that will be unified with the body type
        // during Pass 2 (body checking).
        None => checker.pool_mut().fresh_var(),
    };

    // Detect an associated-type projection return over a generic type-param
    // (`-> C.Item` where `C` is a bound type-param). `return_type` is
    // `Idx::ERROR` poison (the projection cannot resolve until `C` is bound to a
    // concrete receiver at a call site); record `(base_param, assoc_name)` so
    // the call-site inference path can project the concrete result.
    let return_projection = func
        .return_ty
        .as_ref()
        .and_then(|rt| detect_type_param_projection(rt, &type_params, arena));

    let (capabilities, capability_params) = collect_capability_params(checker, func);

    let (type_param_bounds, where_clauses, generic_param_mapping) = collect_bounds_and_mapping(
        func,
        generic_params,
        &type_params,
        &type_param_vars,
        &param_types,
    );

    // Check for special function attributes
    let is_main = {
        let main_name = checker.interner().intern("main");
        func.name == main_name
    };

    // Compute Merkle hashes for cross-module type identity
    let param_hashes: Vec<u64> = param_types
        .iter()
        .map(|&idx| checker.pool().hash(idx))
        .collect();
    let return_hash = checker.pool().hash(return_type);

    let sig = FunctionSig {
        name: func.name,
        type_params,
        const_params,
        param_names,
        param_types,
        return_type,
        capabilities,
        capability_params,
        is_public: func.visibility == IrVisibility::Public,
        is_test: false,
        is_main,
        is_fbip: func.is_fbip,
        type_param_bounds,
        where_clauses,
        generic_param_mapping,
        scheme_var_ids: var_ids.clone(),
        required_params,
        param_defaults,
        param_hashes,
        return_hash,
        return_projection,
    };

    (sig, var_ids)
}

fn resolve_function_params(
    checker: &mut ModuleChecker<'_>,
    func: &Function,
    arena: &ExprArena,
    type_param_vars: &FxHashMap<Name, Idx>,
) -> (Vec<Name>, Vec<Idx>, Vec<Option<ori_ir::ExprId>>, usize) {
    let params = arena.get_params(func.params).to_vec();
    let names = params.iter().map(|param| param.name).collect();
    let types = params
        .iter()
        .map(|param| match &param.ty {
            Some(parsed) => resolve_and_check_type_with_vars(
                checker,
                parsed,
                type_param_vars,
                param.span,
                arena,
            ),
            None => checker.pool_mut().fresh_var(),
        })
        .collect();
    let defaults: Vec<_> = params.iter().map(|param| param.default).collect();
    let required = defaults.iter().filter(|default| default.is_none()).count();
    (names, types, defaults, required)
}

/// Detect a return-position associated-type projection over a generic
/// type-param: `C.Item` where `base` is a `Named(C)` and `C` is in
/// `type_params`. Returns `(base_param, assoc_name)`. `None` for any other
/// shape (a `Self.Item` projection, a non-type-param base, a non-projection).
fn detect_type_param_projection(
    parsed: &ParsedType,
    type_params: &[Name],
    arena: &ExprArena,
) -> Option<(Name, Name)> {
    let ParsedType::AssociatedType {
        base, assoc_name, ..
    } = parsed
    else {
        return None;
    };
    let ParsedType::Named { name, .. } = arena.get_parsed_type(*base) else {
        return None;
    };
    if type_params.contains(name) {
        Some((*name, *assoc_name))
    } else {
        None
    }
}

/// Infer the signature of a test function.
///
/// Tests are similar to functions but:
/// - Always return void/unit
/// - May have special test parameters
fn infer_test_signature(checker: &mut ModuleChecker<'_>, test: &TestDef) -> FunctionSig {
    // Tests don't have generic parameters
    let type_params = Vec::new();

    // Resolve parameter types
    // Clone params to avoid borrow conflicts
    let arena = checker.arena();
    let params: Vec<_> = arena.get_params(test.params).to_vec();
    let param_names: Vec<Name> = params.iter().map(|p| p.name).collect();

    let empty_vars = FxHashMap::default();
    let mut param_types = Vec::with_capacity(params.len());
    for p in &params {
        let ty = match &p.ty {
            Some(parsed_ty) => {
                resolve_and_check_type_with_vars(checker, parsed_ty, &empty_vars, p.span, arena)
            }
            None => checker.pool_mut().fresh_var(),
        };
        param_types.push(ty);
    }

    // Tests return their declared type, or unit if no annotation
    let return_type = match &test.return_ty {
        Some(parsed_ty) => {
            resolve_and_check_type_with_vars(checker, parsed_ty, &empty_vars, test.span, arena)
        }
        None => Idx::UNIT,
    };

    let param_defaults: Vec<Option<ori_ir::ExprId>> = params.iter().map(|p| p.default).collect();
    let required_params = param_defaults.iter().filter(|d| d.is_none()).count();

    // Compute Merkle hashes for cross-module type identity
    let param_hashes: Vec<u64> = param_types
        .iter()
        .map(|&idx| checker.pool().hash(idx))
        .collect();
    let return_hash = checker.pool().hash(return_type);

    FunctionSig {
        name: test.name,
        type_params,
        const_params: Vec::new(),
        param_names,
        param_types,
        return_type,
        capabilities: Vec::new(), // Tests don't declare capabilities
        capability_params: Vec::new(),
        is_public: false, // Tests are never public
        is_test: true,
        is_main: false,
        is_fbip: false, // Tests can't be fbip
        type_param_bounds: Vec::new(),
        where_clauses: Vec::new(),
        generic_param_mapping: Vec::new(),
        scheme_var_ids: Vec::new(),
        required_params,
        param_defaults,
        param_hashes,
        return_hash,
        return_projection: None,
    }
}

#[cfg(test)]
mod tests;
