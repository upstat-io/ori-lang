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

use crate::{Environment, FunctionValue, UserMethod, UserMethodRegistry, Value};
use ori_ir::{Module, Name, SharedArena, TypeDeclKind};
use std::collections::{HashMap, HashSet};

/// Register all functions from a module into the environment.
///
/// Creates function values with proper captures and arena references, ensuring
/// correct evaluation when called from different contexts.
///
/// # Arguments
///
/// * `module` - The module containing functions to register
/// * `arena` - Shared arena for expression lookup
/// * `env` - The environment to register functions into
pub fn register_module_functions(module: &Module, arena: &SharedArena, env: &mut Environment) {
    for func in &module.functions {
        let params = arena.get_param_names(func.params);
        let capabilities: Vec<_> = func.capabilities.iter().map(|c| c.name).collect();
        let captures = env.capture();

        let func_value = FunctionValue::with_capabilities(
            params,
            func.body,
            captures,
            arena.clone(),
            capabilities,
        );
        env.define(func.name, Value::Function(func_value), false);
    }
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
/// * `captures` - Variable captures from the current environment
/// * `registry` - Registry to store collected methods
#[expect(
    clippy::implicit_hasher,
    reason = "captures use default hasher throughout codebase"
)]
pub fn collect_impl_methods(
    module: &Module,
    arena: &SharedArena,
    captures: &HashMap<Name, Value>,
    registry: &mut UserMethodRegistry,
) {
    // First, build a map of trait names to their definitions for default method lookup
    let mut trait_map: HashMap<Name, &ori_ir::TraitDef> = HashMap::new();
    for trait_def in &module.traits {
        trait_map.insert(trait_def.name, trait_def);
    }

    for impl_def in &module.impls {
        // Get the type name from self_path (e.g., "Point" for `impl Point { ... }`)
        let Some(&type_name) = impl_def.self_path.last() else {
            continue; // Skip if no type path
        };

        // Collect names of methods explicitly defined in this impl
        let mut overridden_methods: HashSet<Name> = HashSet::new();

        // Register each explicitly defined method
        for method in &impl_def.methods {
            overridden_methods.insert(method.name);

            // Get parameter names
            let params = arena.get_param_names(method.params);

            // Create user method with captures and arena
            let user_method = UserMethod::new(params, method.body, captures.clone(), arena.clone());

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

                                let user_method = UserMethod::new(
                                    params,
                                    default_method.body,
                                    captures.clone(),
                                    arena.clone(),
                                );

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
/// * `captures` - Variable captures from the current environment
/// * `registry` - Registry to store collected methods
#[expect(
    clippy::implicit_hasher,
    reason = "captures use default hasher throughout codebase"
)]
pub fn collect_extend_methods(
    module: &Module,
    arena: &SharedArena,
    captures: &HashMap<Name, Value>,
    registry: &mut UserMethodRegistry,
) {
    for extend_def in &module.extends {
        // Get the target type name (e.g., "list" for `extend [T] { ... }`)
        let type_name = extend_def.target_type_name;

        // Register each method
        for method in &extend_def.methods {
            // Get parameter names
            let params = arena.get_param_names(method.params);

            // Create user method with captures and arena
            let user_method = UserMethod::new(params, method.body, captures.clone(), arena.clone());

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
        register_module_functions(&result.module, &arena, &mut env);

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
        let captures = HashMap::new();

        collect_impl_methods(&result.module, &arena, &captures, &mut registry);

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
        let captures = HashMap::new();

        collect_extend_methods(&result.module, &arena, &captures, &mut registry);

        let list_name = interner.intern("list");
        let double_name = interner.intern("double");

        assert!(registry.lookup(list_name, double_name).is_some());
    }
}
