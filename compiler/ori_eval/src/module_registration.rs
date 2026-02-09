//! Module registration - Salsa-free module loading for the interpreter.
//!
//! This module provides pure functions for registering module items (functions,
//! methods, constructors) into the interpreter environment. These functions are
//! decoupled from Salsa and work with parsed modules directly, enabling any client
//! to use the full Ori interpreter without query system dependencies.
//!
//! # Architecture
//!
//! The `oric` crate handles Salsa-tracked file loading and parsing, then delegates
//! to these functions for the actual registration. Clients that don't need Salsa
//! (WASM, embedded interpreters, testing) can call these functions directly after
//! parsing.

// Arc is required for SharedArena API
#![allow(clippy::disallowed_types)]

use crate::{Environment, FunctionValue, Mutability, UserMethod, UserMethodRegistry, Value};
use ori_ir::canon::SharedCanonResult;
use ori_ir::{Module, Name, SharedArena, TypeDeclKind};
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;

/// Configuration for method collection operations.
///
/// Groups the common parameters needed by `collect_impl_methods` and
/// `collect_extend_methods`, reducing parameter count and improving API clarity.
pub struct MethodCollectionConfig<'a> {
    /// The module containing methods to collect.
    pub module: &'a Module,
    /// Shared arena for expression lookup.
    pub arena: &'a SharedArena,
    /// Variable captures from the current environment, pre-wrapped in Arc for
    /// efficient sharing across multiple methods (O(1) cloning).
    pub captures: Arc<FxHashMap<Name, Value>>,
    /// Optional canonical IR for canonical method dispatch.
    pub canon: Option<&'a SharedCanonResult>,
}

/// Register all functions from a module into the environment.
///
/// Creates function values with proper captures and arena references, ensuring
/// correct evaluation when called from different contexts.
///
/// Functions with the same name are grouped as multi-clause functions, with
/// pattern matching determining which clause to execute at runtime.
///
/// When `canon` is provided, each function's `FunctionValue` is enriched with
/// canonical IR data (`can_body` + `SharedCanonResult`), enabling the evaluator
/// to dispatch on `CanExpr` instead of `ExprKind`.
///
/// # Arguments
///
/// * `module` - The module containing functions to register
/// * `arena` - Shared arena for expression lookup
/// * `env` - The environment to register functions into
/// * `canon` - Optional canonical IR (from `ori_canon::lower_module`)
pub fn register_module_functions(
    module: &Module,
    arena: &SharedArena,
    env: &mut Environment,
    canon: Option<&SharedCanonResult>,
) {
    // Group functions by name to support multi-clause definitions
    let mut func_groups: FxHashMap<Name, Vec<&ori_ir::Function>> = FxHashMap::default();
    for func in &module.functions {
        func_groups.entry(func.name).or_default().push(func);
    }

    let captures = env.capture();

    // Build a lookup from function name → CanId for canonical wiring.
    // A function may appear multiple times in roots (multi-clause), so we
    // collect all CanIds per name in order.
    let canon_lookup: FxHashMap<Name, Vec<ori_ir::canon::CanId>> = canon
        .map(|c| {
            let mut map: FxHashMap<Name, Vec<ori_ir::canon::CanId>> = FxHashMap::default();
            for &(name, can_id) in &c.roots {
                map.entry(name).or_default().push(can_id);
            }
            map
        })
        .unwrap_or_default();

    for (name, funcs) in func_groups {
        if funcs.len() == 1 {
            // Single function - create standard Value::Function
            let func = funcs[0];
            let params_slice = arena.get_params(func.params);
            let params: Vec<_> = params_slice.iter().map(|p| p.name).collect();
            let defaults: Vec<_> = params_slice.iter().map(|p| p.default).collect();
            let capabilities: Vec<_> = func.capabilities.iter().map(|c| c.name).collect();

            let mut func_value = FunctionValue::with_defaults(
                params,
                defaults,
                func.body,
                captures.clone(),
                arena.clone(),
                capabilities,
            );

            // Attach canonical IR if available
            if let (Some(can_ids), Some(c)) = (canon_lookup.get(&name), canon) {
                if let Some(&can_id) = can_ids.first() {
                    func_value.set_canon(can_id, c.clone());
                }
            }

            env.define(name, Value::Function(func_value), Mutability::Immutable);
        } else {
            // Multiple functions with same name — dispatch lowered to a single
            // canonical match body by `lower_module()`. Register using the first
            // clause's parameters (the match body handles clause selection).
            let first_func = funcs[0];
            let params_slice = arena.get_params(first_func.params);
            let params: Vec<_> = params_slice.iter().map(|p| p.name).collect();
            let defaults: Vec<_> = params_slice.iter().map(|p| p.default).collect();
            let capabilities: Vec<_> = first_func.capabilities.iter().map(|c| c.name).collect();

            let mut func_value = FunctionValue::with_defaults(
                params,
                defaults,
                first_func.body,
                captures.clone(),
                arena.clone(),
                capabilities,
            );

            // Attach the canonical match body (synthesized by lower_module).
            // The canon_lookup groups all same-name roots; the first entry
            // is the synthesized match body for the multi-clause group.
            if let (Some(can_ids), Some(c)) = (canon_lookup.get(&name), canon) {
                if let Some(&can_id) = can_ids.first() {
                    func_value.set_canon(can_id, c.clone());
                }
            }

            env.define(name, Value::Function(func_value), Mutability::Immutable);
        }
    }
}

/// Collect methods from impl blocks into a registry using a config struct.
///
/// Prefer this over `collect_impl_methods` for new code.
pub fn collect_impl_methods_with_config(
    config: &MethodCollectionConfig<'_>,
    registry: &mut UserMethodRegistry,
) {
    collect_impl_methods(
        config.module,
        config.arena,
        &config.captures,
        config.canon,
        registry,
    );
}

/// Collect methods from extend blocks into a registry using a config struct.
///
/// Prefer this over `collect_extend_methods` for new code.
pub fn collect_extend_methods_with_config(
    config: &MethodCollectionConfig<'_>,
    registry: &mut UserMethodRegistry,
) {
    collect_extend_methods(
        config.module,
        config.arena,
        &config.captures,
        config.canon,
        registry,
    );
}

/// Collect methods from impl blocks into a registry.
///
/// Takes explicit captures instead of borrowing from an interpreter, enabling
/// use from both CLI and WASM playground.
///
/// # Arguments
///
/// * `module` - The module containing impl blocks
/// * `arena` - Shared arena for expression lookup
/// * `captures` - Variable captures pre-wrapped in Arc for efficient sharing
/// * `registry` - Registry to store collected methods
#[expect(
    clippy::implicit_hasher,
    reason = "FxHashMap is used specifically for performance with small keys"
)]
pub fn collect_impl_methods(
    module: &Module,
    arena: &SharedArena,
    captures: &Arc<FxHashMap<Name, Value>>,
    canon: Option<&SharedCanonResult>,
    registry: &mut UserMethodRegistry,
) {
    // Arc is already provided by caller, no cloning needed

    // First, build a map of trait names to their definitions for default method lookup
    let mut trait_map: FxHashMap<Name, &ori_ir::TraitDef> = FxHashMap::default();
    for trait_def in &module.traits {
        trait_map.insert(trait_def.name, trait_def);
    }

    for impl_def in &module.impls {
        // Get the type name from self_path (e.g., "Point" for `impl Point { ... }`)
        let Some(&type_name) = impl_def.self_path.last() else {
            continue; // Skip if no type path
        };

        // Collect names of methods explicitly defined in this impl
        let mut overridden_methods: FxHashSet<Name> = FxHashSet::default();

        // Register each explicitly defined method
        for method in &impl_def.methods {
            overridden_methods.insert(method.name);

            // Get parameter names
            let params = arena.get_param_names(method.params);

            // Create user method with Arc-cloned captures (O(1) instead of O(n))
            let mut user_method =
                UserMethod::new(params, method.body, Arc::clone(captures), arena.clone());

            // Attach canonical IR when available.
            if let Some(cr) = canon {
                if let Some(can_id) = cr.method_root_for(type_name, method.name) {
                    user_method.set_canon(can_id, cr.clone());
                }
            }

            registry.register(type_name, method.name, user_method);
        }

        // For trait impls, also register default trait methods that weren't overridden
        if let Some(trait_path) = &impl_def.trait_path {
            if let Some(&trait_name) = trait_path.last() {
                if let Some(trait_def) = trait_map.get(&trait_name) {
                    for item in &trait_def.items {
                        if let ori_ir::TraitItem::DefaultMethod(default_method) = item {
                            // Only register if not overridden
                            if !overridden_methods.contains(&default_method.name) {
                                let params = arena.get_param_names(default_method.params);

                                let mut user_method = UserMethod::new(
                                    params,
                                    default_method.body,
                                    Arc::clone(captures),
                                    arena.clone(),
                                );

                                if let Some(cr) = canon {
                                    if let Some(can_id) =
                                        cr.method_root_for(type_name, default_method.name)
                                    {
                                        user_method.set_canon(can_id, cr.clone());
                                    }
                                }

                                registry.register(type_name, default_method.name, user_method);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Collect methods from extend blocks into a registry.
///
/// Takes explicit captures instead of borrowing from an interpreter, enabling
/// use from both CLI and WASM playground.
///
/// # Arguments
///
/// * `module` - The module containing extend blocks
/// * `arena` - Shared arena for expression lookup
/// * `captures` - Variable captures pre-wrapped in Arc for efficient sharing
/// * `registry` - Registry to store collected methods
#[expect(
    clippy::implicit_hasher,
    reason = "FxHashMap is used specifically for performance with small keys"
)]
pub fn collect_extend_methods(
    module: &Module,
    arena: &SharedArena,
    captures: &Arc<FxHashMap<Name, Value>>,
    canon: Option<&SharedCanonResult>,
    registry: &mut UserMethodRegistry,
) {
    // Arc is already provided by caller, no cloning needed

    for extend_def in &module.extends {
        // Get the target type name (e.g., "list" for `extend [T] { ... }`)
        let type_name = extend_def.target_type_name;

        // Register each method
        for method in &extend_def.methods {
            // Get parameter names
            let params = arena.get_param_names(method.params);

            // Create user method with Arc-cloned captures (O(1) instead of O(n))
            let mut user_method =
                UserMethod::new(params, method.body, Arc::clone(captures), arena.clone());

            if let Some(cr) = canon {
                if let Some(can_id) = cr.method_root_for(type_name, method.name) {
                    user_method.set_canon(can_id, cr.clone());
                }
            }

            registry.register(type_name, method.name, user_method);
        }
    }
}

/// Register variant constructors from sum type declarations.
///
/// For each sum type (enum), registers each variant as a constructor:
/// - Unit variants (no fields) are bound directly as `Value::Variant`
/// - Variants with fields are bound as constructor functions
///
/// # Arguments
///
/// * `module` - The module containing type declarations
/// * `env` - The environment to register constructors into
pub fn register_variant_constructors(module: &Module, env: &mut Environment) {
    for type_decl in &module.types {
        if let TypeDeclKind::Sum(variants) = &type_decl.kind {
            let type_name = type_decl.name;

            for variant in variants {
                if variant.fields.is_empty() {
                    // Unit variant: bind directly as Value::Variant
                    let value = Value::variant(type_name, variant.name, vec![]);
                    env.define_global(variant.name, value);
                } else {
                    // Variant with fields: create a constructor function
                    let value =
                        Value::variant_constructor(type_name, variant.name, variant.fields.len());
                    env.define_global(variant.name, value);
                }
            }
        }
    }
}

/// Collect methods from def impl blocks into a registry.
///
/// Default implementations provide stateless methods for capability traits.
/// Methods are registered under the trait name, allowing `TraitName.method(...)` calls.
///
/// # Arguments
///
/// * `module` - The module containing def impl blocks
/// * `arena` - Shared arena for expression lookup
/// * `captures` - Variable captures pre-wrapped in Arc for efficient sharing
/// * `registry` - Registry to store collected methods
#[expect(
    clippy::implicit_hasher,
    reason = "FxHashMap is used specifically for performance with small keys"
)]
pub fn collect_def_impl_methods(
    module: &Module,
    arena: &SharedArena,
    captures: &Arc<FxHashMap<Name, Value>>,
    canon: Option<&SharedCanonResult>,
    registry: &mut UserMethodRegistry,
) {
    // Arc is already provided by caller, no cloning needed

    for def_impl_def in &module.def_impls {
        let trait_name = def_impl_def.trait_name;

        for method in &def_impl_def.methods {
            // Get parameter names
            let params = arena.get_param_names(method.params);

            // Create user method with Arc-cloned captures (O(1) instead of O(n))
            let mut user_method =
                UserMethod::new(params, method.body, Arc::clone(captures), arena.clone());

            if let Some(cr) = canon {
                if let Some(can_id) = cr.method_root_for(trait_name, method.name) {
                    user_method.set_canon(can_id, cr.clone());
                }
            }

            // Register under trait name (trait_name -> method_name)
            // This enables `TraitName.method(...)` calls for capability dispatch
            registry.register(trait_name, method.name, user_method);
        }
    }
}

/// Collect methods from def impl blocks using a config struct.
///
/// Prefer this over `collect_def_impl_methods` for new code.
pub fn collect_def_impl_methods_with_config(
    config: &MethodCollectionConfig<'_>,
    registry: &mut UserMethodRegistry,
) {
    collect_def_impl_methods(
        config.module,
        config.arena,
        &config.captures,
        config.canon,
        registry,
    );
}

/// Register newtype constructors from type declarations.
///
/// For each newtype (e.g., `type UserId = str`), registers the type name
/// as a constructor that wraps the underlying value.
///
/// # Arguments
///
/// * `module` - The module containing type declarations
/// * `env` - The environment to register constructors into
pub fn register_newtype_constructors(module: &Module, env: &mut Environment) {
    for type_decl in &module.types {
        if let TypeDeclKind::Newtype(_) = &type_decl.kind {
            let type_name = type_decl.name;
            // Bind the newtype constructor to the type name
            let value = Value::newtype_constructor(type_name);
            env.define_global(type_name, value);
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "tests use unwrap for brevity")]
mod tests {
    use super::*;
    use ori_ir::SharedInterner;
    use ori_lexer::lex;
    use ori_parse::{parse, ParseOutput};

    fn parse_source(source: &str) -> (ParseOutput, SharedInterner) {
        let interner = SharedInterner::default();
        let tokens = lex(source, &interner);
        let result = parse(&tokens, &interner);
        (result, interner)
    }

    #[test]
    fn test_register_module_functions() {
        let (result, interner) = parse_source(
            r"
            @add (a: int, b: int) -> int = a + b
            @main () -> void = print(msg: str(add(a: 1, b: 2)))
        ",
        );

        let arena = SharedArena::new(result.arena.clone());
        let mut env = Environment::new();
        register_module_functions(&result.module, &arena, &mut env, None);

        let add_name = interner.intern("add");
        let main_name = interner.intern("main");

        assert!(env.lookup(add_name).is_some());
        assert!(env.lookup(main_name).is_some());
    }

    #[test]
    fn test_register_variant_constructors() {
        let (result, interner) = parse_source(
            r"
            type Status = Running | Done(result: int)
        ",
        );

        let mut env = Environment::new();
        register_variant_constructors(&result.module, &mut env);

        let running_name = interner.intern("Running");
        let done_name = interner.intern("Done");

        // Unit variant should be a Value::Variant
        let running = env.lookup(running_name);
        assert!(running.is_some());
        assert!(matches!(running.unwrap(), Value::Variant { .. }));

        // Variant with fields should be a constructor
        let done = env.lookup(done_name);
        assert!(done.is_some());
        assert!(matches!(done.unwrap(), Value::VariantConstructor { .. }));
    }

    #[test]
    fn test_register_newtype_constructors() {
        let (result, interner) = parse_source(
            r"
            type UserId = str
        ",
        );

        let mut env = Environment::new();
        register_newtype_constructors(&result.module, &mut env);

        let userid_name = interner.intern("UserId");

        let constructor = env.lookup(userid_name);
        assert!(constructor.is_some());
        assert!(matches!(
            constructor.unwrap(),
            Value::NewtypeConstructor { .. }
        ));
    }

    #[test]
    fn test_collect_impl_methods() {
        let (result, interner) = parse_source(
            r"
            type Point = { x: int, y: int }

            impl Point {
                @sum (self) -> int = self.x + self.y
            }
        ",
        );

        let arena = SharedArena::new(result.arena.clone());
        let mut registry = UserMethodRegistry::new();
        let captures = Arc::new(FxHashMap::default());

        collect_impl_methods(&result.module, &arena, &captures, None, &mut registry);

        let point_name = interner.intern("Point");
        let sum_name = interner.intern("sum");

        assert!(registry.lookup(point_name, sum_name).is_some());
    }

    #[test]
    fn test_collect_impl_methods_with_config() {
        let (result, interner) = parse_source(
            r"
            type Point = { x: int, y: int }

            impl Point {
                @sum (self) -> int = self.x + self.y
            }
        ",
        );

        let arena = SharedArena::new(result.arena.clone());
        let mut registry = UserMethodRegistry::new();
        let captures = Arc::new(FxHashMap::default());

        let config = MethodCollectionConfig {
            module: &result.module,
            arena: &arena,
            captures: Arc::clone(&captures),
            canon: None,
        };
        collect_impl_methods_with_config(&config, &mut registry);

        let point_name = interner.intern("Point");
        let sum_name = interner.intern("sum");

        assert!(registry.lookup(point_name, sum_name).is_some());
    }

    #[test]
    fn test_collect_extend_methods() {
        let (result, interner) = parse_source(
            r"
            extend [T] {
                @double (self) -> [T] = self + self
            }
        ",
        );

        let arena = SharedArena::new(result.arena.clone());
        let mut registry = UserMethodRegistry::new();
        let captures = Arc::new(FxHashMap::default());

        collect_extend_methods(&result.module, &arena, &captures, None, &mut registry);

        let list_name = interner.intern("list");
        let double_name = interner.intern("double");

        assert!(registry.lookup(list_name, double_name).is_some());
    }

    #[test]
    fn test_collect_extend_methods_with_config() {
        let (result, interner) = parse_source(
            r"
            extend [T] {
                @double (self) -> [T] = self + self
            }
        ",
        );

        let arena = SharedArena::new(result.arena.clone());
        let mut registry = UserMethodRegistry::new();
        let captures = Arc::new(FxHashMap::default());

        let config = MethodCollectionConfig {
            module: &result.module,
            arena: &arena,
            captures: Arc::clone(&captures),
            canon: None,
        };
        collect_extend_methods_with_config(&config, &mut registry);

        let list_name = interner.intern("list");
        let double_name = interner.intern("double");

        assert!(registry.lookup(list_name, double_name).is_some());
    }

    #[test]
    fn test_collect_def_impl_methods() {
        let (result, interner) = parse_source(
            r"
            def impl Http {
                @get (url: str) -> str = url
                @post (url: str, body: str) -> str = body
            }
        ",
        );

        let arena = SharedArena::new(result.arena.clone());
        let mut registry = UserMethodRegistry::new();
        let captures = Arc::new(FxHashMap::default());

        collect_def_impl_methods(&result.module, &arena, &captures, None, &mut registry);

        let http_name = interner.intern("Http");
        let get_name = interner.intern("get");
        let post_name = interner.intern("post");

        // Methods should be registered under the trait name
        assert!(registry.lookup(http_name, get_name).is_some());
        assert!(registry.lookup(http_name, post_name).is_some());
    }

    #[test]
    fn test_collect_def_impl_methods_with_config() {
        let (result, interner) = parse_source(
            r"
            pub def impl Http {
                @get (url: str) -> str = url
            }
        ",
        );

        let arena = SharedArena::new(result.arena.clone());
        let mut registry = UserMethodRegistry::new();
        let captures = Arc::new(FxHashMap::default());

        let config = MethodCollectionConfig {
            module: &result.module,
            arena: &arena,
            captures: Arc::clone(&captures),
            canon: None,
        };
        collect_def_impl_methods_with_config(&config, &mut registry);

        let http_name = interner.intern("Http");
        let get_name = interner.intern("get");

        assert!(registry.lookup(http_name, get_name).is_some());
    }
}
