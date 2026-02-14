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

use ori_ir::{ExprArena, Function, Module, Name, ParsedType, TestDef, Visibility as IrVisibility};
use rustc_hash::FxHashMap;

use super::ModuleChecker;
use crate::{FnWhereClause, FunctionSig, Idx};

// ============================================================================
// Pass 1: Signature Collection
// ============================================================================

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

// ============================================================================
// Signature Inference
// ============================================================================

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

/// Shared implementation for inferring a function signature from any arena.
///
/// Returns the signature and the var IDs of generic type parameters.
fn infer_function_signature_with_arena(
    checker: &mut ModuleChecker<'_>,
    func: &Function,
    arena: &ExprArena,
) -> (FunctionSig, Vec<u32>) {
    // Collect generic parameter names (filter out const params — they are values, not types)
    let generic_params = arena.get_generic_params(func.generics);
    let type_params: Vec<Name> = generic_params
        .iter()
        .filter(|p| !p.is_const)
        .map(|p| p.name)
        .collect();

    // Create a mapping from generic param names to fresh type variables
    let type_param_vars: FxHashMap<Name, Idx> = type_params
        .iter()
        .map(|&name| {
            let var = checker.pool_mut().fresh_named_var(name);
            (name, var)
        })
        .collect();

    // Collect var IDs for scheme creation (sorted for determinism)
    let mut var_ids: Vec<u32> = type_param_vars
        .values()
        .map(|idx| checker.pool().data(*idx))
        .collect();
    var_ids.sort_unstable();

    // Resolve parameter types using the generic param mapping
    // Clone params to avoid borrow conflicts - params slice borrows arena,
    // but we need mutable pool access during type resolution
    let params: Vec<_> = arena.get_params(func.params).to_vec();
    let param_names: Vec<Name> = params.iter().map(|p| p.name).collect();

    let mut param_types = Vec::with_capacity(params.len());
    for p in &params {
        let ty = match &p.ty {
            Some(parsed_ty) => resolve_type_with_vars(checker, parsed_ty, &type_param_vars, arena),
            // Parameter without type annotation gets a fresh variable
            None => checker.pool_mut().fresh_var(),
        };
        param_types.push(ty);
    }

    // Resolve return type
    let return_type = match &func.return_ty {
        Some(parsed_ty) => resolve_type_with_vars(checker, parsed_ty, &type_param_vars, arena),
        // No return type annotation: infer from the body.
        // Use a fresh type variable that will be unified with the body type
        // during Pass 2 (body checking).
        None => checker.pool_mut().fresh_var(),
    };

    // Extract capabilities
    let capabilities: Vec<Name> = func.capabilities.iter().map(|c| c.name).collect();

    // Collect trait bounds for each generic type parameter (matching type_params order)
    let type_param_bounds: Vec<Vec<Name>> = generic_params
        .iter()
        .filter(|p| !p.is_const)
        .map(|p| p.bounds.iter().map(ori_ir::TraitBound::name).collect())
        .collect();

    // Collect where-clauses (only type bounds; const bounds are deferred)
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

    // Map each generic type param to the function param that directly uses it
    let generic_param_mapping: Vec<Option<usize>> = type_params
        .iter()
        .map(|tp_name| {
            let var_idx = type_param_vars[tp_name];
            param_types.iter().position(|&ty| ty == var_idx)
        })
        .collect();

    // Collect default expressions and count required params.
    let param_defaults: Vec<Option<ori_ir::ExprId>> = params.iter().map(|p| p.default).collect();
    let required_params = param_defaults.iter().filter(|d| d.is_none()).count();

    // Check for special function attributes
    let main_name = checker.interner().intern("main");
    let is_main = func.name == main_name;

    let sig = FunctionSig {
        name: func.name,
        type_params,
        param_names,
        param_types,
        return_type,
        capabilities,
        is_public: func.visibility == IrVisibility::Public,
        is_test: false,
        is_main,
        type_param_bounds,
        where_clauses,
        generic_param_mapping,
        required_params,
        param_defaults,
    };

    (sig, var_ids)
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
            Some(parsed_ty) => resolve_type_with_vars(checker, parsed_ty, &empty_vars, arena),
            None => checker.pool_mut().fresh_var(),
        };
        param_types.push(ty);
    }

    // Tests return their declared type, or unit if no annotation
    let return_type = match &test.return_ty {
        Some(parsed_ty) => resolve_type_with_vars(checker, parsed_ty, &empty_vars, arena),
        None => Idx::UNIT,
    };

    let param_defaults: Vec<Option<ori_ir::ExprId>> = params.iter().map(|p| p.default).collect();
    let required_params = param_defaults.iter().filter(|d| d.is_none()).count();

    FunctionSig {
        name: test.name,
        type_params,
        param_names,
        param_types,
        return_type,
        capabilities: Vec::new(), // Tests don't declare capabilities
        is_public: false,         // Tests are never public
        is_test: true,
        is_main: false,
        type_param_bounds: Vec::new(),
        where_clauses: Vec::new(),
        generic_param_mapping: Vec::new(),
        required_params,
        param_defaults,
    }
}

// ============================================================================
// Type Resolution with Generic Parameters
// ============================================================================

/// Resolve a parsed type with generic parameter variables in scope.
///
/// This differs from `resolve_parsed_type_simple` in that it:
/// 1. Looks up type parameter names in the provided mapping
/// 2. Returns the corresponding type variable for generic params
///
/// The `arena` parameter allows resolving types from either the local module's
/// arena or a foreign module's arena, enabling import signature inference.
fn resolve_type_with_vars(
    checker: &mut ModuleChecker<'_>,
    parsed: &ParsedType,
    type_param_vars: &FxHashMap<Name, Idx>,
    arena: &ExprArena,
) -> Idx {
    match parsed {
        // Primitive types - unchanged
        ParsedType::Primitive(type_id) => match type_id.raw() & 0x0FFF_FFFF {
            0 => Idx::INT,
            1 => Idx::FLOAT,
            2 => Idx::BOOL,
            3 => Idx::STR,
            4 => Idx::CHAR,
            5 => Idx::BYTE,
            6 => Idx::UNIT,
            7 => Idx::NEVER,
            _ => Idx::ERROR,
        },

        // List type: [T]
        ParsedType::List(elem_id) => {
            let elem = arena.get_parsed_type(*elem_id);
            let elem_ty = resolve_type_with_vars(checker, elem, type_param_vars, arena);
            checker.pool_mut().list(elem_ty)
        }

        // Map type: {K: V}
        ParsedType::Map { key, value } => {
            let key_parsed = arena.get_parsed_type(*key);
            let value_parsed = arena.get_parsed_type(*value);
            let key_ty = resolve_type_with_vars(checker, key_parsed, type_param_vars, arena);
            let value_ty = resolve_type_with_vars(checker, value_parsed, type_param_vars, arena);
            checker.pool_mut().map(key_ty, value_ty)
        }

        // Tuple type: (T, U, V)
        ParsedType::Tuple(elems) => {
            let elem_ids = arena.get_parsed_type_list(*elems);
            let elem_types: Vec<Idx> = elem_ids
                .iter()
                .map(|&elem_id| {
                    let elem = arena.get_parsed_type(elem_id);
                    resolve_type_with_vars(checker, elem, type_param_vars, arena)
                })
                .collect();
            checker.pool_mut().tuple(&elem_types)
        }

        // Function type: fn(T) -> U
        ParsedType::Function { params, ret } => {
            let param_ids = arena.get_parsed_type_list(*params);
            let param_types: Vec<Idx> = param_ids
                .iter()
                .map(|&param_id| {
                    let param = arena.get_parsed_type(param_id);
                    resolve_type_with_vars(checker, param, type_param_vars, arena)
                })
                .collect();
            let ret_parsed = arena.get_parsed_type(*ret);
            let ret_ty = resolve_type_with_vars(checker, ret_parsed, type_param_vars, arena);
            checker.pool_mut().function(&param_types, ret_ty)
        }

        // Named type: Could be a type parameter or a user-defined type
        ParsedType::Named { name, type_args } => {
            // First, check if this is a type parameter
            if let Some(&var) = type_param_vars.get(name) {
                // It's a generic type parameter - return the variable
                return var;
            }

            // Resolve type arguments if present
            let type_arg_ids = arena.get_parsed_type_list(*type_args);
            let resolved_args: Vec<Idx> = type_arg_ids
                .iter()
                .map(|&arg_id| {
                    let arg = arena.get_parsed_type(arg_id);
                    resolve_type_with_vars(checker, arg, type_param_vars, arena)
                })
                .collect();

            // Check for well-known generic types that have dedicated Pool tags.
            // These must be constructed with their specific Pool methods to ensure
            // unification works correctly (e.g., Option<int> from a type annotation
            // must produce the same Tag::Option as pool.option(int) from inference).
            if !resolved_args.is_empty() {
                let name_str = checker.interner().lookup(*name);
                match (name_str, resolved_args.len()) {
                    ("Option", 1) => return checker.pool_mut().option(resolved_args[0]),
                    ("Result", 2) => {
                        return checker
                            .pool_mut()
                            .result(resolved_args[0], resolved_args[1]);
                    }
                    ("Set", 1) => return checker.pool_mut().set(resolved_args[0]),
                    ("Channel" | "Chan", 1) => {
                        return checker.pool_mut().channel(resolved_args[0]);
                    }
                    ("Range", 1) => return checker.pool_mut().range(resolved_args[0]),
                    _ => {
                        // User-defined generic type: create Applied type
                        return checker.pool_mut().applied(*name, &resolved_args);
                    }
                }
            }

            // No type args — check for built-in primitive type names
            let name_str = checker.interner().lookup(*name);
            match name_str {
                "int" => return Idx::INT,
                "float" => return Idx::FLOAT,
                "bool" => return Idx::BOOL,
                "str" => return Idx::STR,
                "char" => return Idx::CHAR,
                "byte" => return Idx::BYTE,
                "void" | "()" => return Idx::UNIT,
                "never" | "Never" => return Idx::NEVER,
                "Duration" | "duration" => return Idx::DURATION,
                "Size" | "size" => return Idx::SIZE,
                "Ordering" | "ordering" => return Idx::ORDERING,
                _ => {}
            }

            // User-defined bare named type
            checker.pool_mut().named(*name)
        }

        // Fixed-size list: treat as regular list for now
        ParsedType::FixedList { elem, capacity: _ } => {
            let elem_parsed = arena.get_parsed_type(*elem);
            let elem_ty = resolve_type_with_vars(checker, elem_parsed, type_param_vars, arena);
            checker.pool_mut().list(elem_ty)
        }

        // Infer type: fresh variable
        ParsedType::Infer => checker.pool_mut().fresh_var(),

        // Self type: handled specially in impl blocks
        ParsedType::SelfType => {
            // In impl blocks, Self refers to the implementing type
            // For now, return error - this should be resolved by the caller
            Idx::ERROR
        }

        // Associated type: T::Item
        ParsedType::AssociatedType { .. } | ParsedType::ConstExpr(_) => {
            // Associated types require trait resolution; const expressions require const evaluation.
            // For now, return error - will be implemented with trait/const support
            Idx::ERROR
        }

        // Bounded trait object: resolve first bound as primary type
        ParsedType::TraitBounds(bounds) => {
            let bound_ids = arena.get_parsed_type_list(*bounds);
            if let Some(&first_id) = bound_ids.first() {
                let first = arena.get_parsed_type(first_id);
                resolve_type_with_vars(checker, first, type_param_vars, arena)
            } else {
                Idx::ERROR
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::{ExprArena, StringInterner};

    #[test]
    fn collect_signatures_empty_module() {
        let arena = ExprArena::new();
        let interner = StringInterner::new();
        let mut checker = ModuleChecker::new(&arena, &interner);
        let module = Module::default();

        collect_signatures(&mut checker, &module);

        // Base env should be frozen even with empty module
        assert!(checker.base_env().is_some());
    }
}
