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

#![expect(
    clippy::disallowed_types,
    reason = "Arc required for SharedArena API and shared captures"
)]

use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;

use ori_ir::canon::SharedCanonResult;
use ori_ir::{Module, Name, ParsedType, SharedArena, StringInterner, TypeDeclKind};

use crate::{Environment, FunctionValue, Mutability, UserMethod, UserMethodRegistry, Value};

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
    /// String interner for converting primitive `TypeId`s to `Name`s.
    ///
    /// Required for extracting `key_type_hint` from trait type arguments
    /// (e.g., `impl Index<int, str> for T` → hint = intern("int")).
    pub interner: &'a StringInterner,
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

    let captures = Arc::new(env.capture());

    // Build a lookup from function name → CanonRoot for canonical wiring.
    // Carries both body CanId and canonicalized defaults per function.
    let canon_lookup: FxHashMap<Name, Vec<&ori_ir::canon::CanonRoot>> = canon
        .map(|c| {
            let mut map: FxHashMap<Name, Vec<&ori_ir::canon::CanonRoot>> = FxHashMap::default();
            for root in &c.roots {
                map.entry(root.name).or_default().push(root);
            }
            map
        })
        .unwrap_or_default();

    for (name, funcs) in func_groups {
        // For multi-clause functions, `lower_module()` synthesizes a single
        // canonical match body. We always use the first clause's parameters.
        let first_func = funcs[0];
        let params_slice = arena.get_params(first_func.params);
        let params: Vec<_> = params_slice.iter().map(|p| p.name).collect();
        let capabilities: Vec<_> = first_func.capabilities.iter().map(|c| c.name).collect();

        let mut func_value = FunctionValue::with_shared_captures(
            params,
            Arc::clone(&captures),
            arena.clone(),
            capabilities,
        );

        // Attach canonical IR and defaults
        if let (Some(roots), Some(c)) = (canon_lookup.get(&name), canon) {
            if let Some(root) = roots.first() {
                func_value.set_canon(root.body, c.clone());
                if root.defaults.iter().any(Option::is_some) {
                    func_value.set_can_defaults(root.defaults.clone());
                }
            }
        }

        env.define(name, Value::Function(func_value), Mutability::Immutable);
    }
}

/// Collect methods from impl blocks into a registry.
///
/// Registers each method defined in `impl Type { ... }` blocks, including
/// default trait methods that weren't overridden.
pub fn collect_impl_methods_with_config(
    config: &MethodCollectionConfig<'_>,
    registry: &mut UserMethodRegistry,
) {
    collect_impl_methods(
        config.module,
        config.arena,
        &config.captures,
        config.canon,
        config.interner,
        registry,
    );
}

/// Collect methods from extend blocks into a registry.
///
/// Registers each method defined in `extend [T] { ... }` blocks.
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
/// For trait impls with type arguments (e.g., `impl Index<int, str> for T`),
/// extracts the first type argument as `key_type_hint` for runtime dispatch.
fn collect_impl_methods(
    module: &Module,
    arena: &SharedArena,
    captures: &Arc<FxHashMap<Name, Value>>,
    canon: Option<&SharedCanonResult>,
    interner: &StringInterner,
    registry: &mut UserMethodRegistry,
) {
    // First, build a map of trait names to their definitions for default method lookup
    let mut trait_map: FxHashMap<Name, &ori_ir::TraitDef> = FxHashMap::default();
    for trait_def in &module.traits {
        trait_map.insert(trait_def.name, trait_def);
    }

    // Track how many times each (type_name, method_name) pair has been seen,
    // so we can pick the correct canonical body when multiple impls define the
    // same method (e.g., two `impl Index<K,V> for T` blocks both defining `index`).
    // The canonicalization pass pushes method_roots in the same iteration order,
    // so the Nth occurrence here matches the Nth entry in `method_roots`.
    let mut method_canon_index: FxHashMap<(Name, Name), usize> = FxHashMap::default();

    for impl_def in &module.impls {
        // Get the type name from self_path (e.g., "Point" for `impl Point { ... }`)
        let Some(&type_name) = impl_def.self_path.last() else {
            continue;
        };

        // Extract key_type_hint from trait type args (first arg of Index<K, V> etc.)
        let key_type_hint = extract_key_type_hint(impl_def, arena, interner);

        // Collect names of methods explicitly defined in this impl
        let mut overridden_methods: FxHashSet<Name> = FxHashSet::default();

        // Register each explicitly defined method
        for method in &impl_def.methods {
            overridden_methods.insert(method.name);

            let params = arena.get_param_names(method.params);
            let mut user_method = UserMethod::new(params, Arc::clone(captures), arena.clone());
            user_method.key_type_hint = key_type_hint;

            if let Some(cr) = canon {
                let idx = method_canon_index
                    .entry((type_name, method.name))
                    .or_insert(0);
                if let Some(can_id) = cr.method_root_for_nth(type_name, method.name, *idx) {
                    user_method.set_canon(can_id, cr.clone());
                }
                *idx = idx.wrapping_add(1);
            }

            registry.register(type_name, method.name, user_method);
        }

        // For trait impls, also register default trait methods that weren't overridden
        if let Some(trait_path) = &impl_def.trait_path {
            if let Some(&trait_name) = trait_path.last() {
                if let Some(trait_def) = trait_map.get(&trait_name) {
                    for item in &trait_def.items {
                        if let ori_ir::TraitItem::DefaultMethod(default_method) = item {
                            if !overridden_methods.contains(&default_method.name) {
                                let params = arena.get_param_names(default_method.params);
                                let mut user_method =
                                    UserMethod::new(params, Arc::clone(captures), arena.clone());
                                user_method.key_type_hint = key_type_hint;

                                if let Some(cr) = canon {
                                    let idx = method_canon_index
                                        .entry((type_name, default_method.name))
                                        .or_insert(0);
                                    if let Some(can_id) =
                                        cr.method_root_for_nth(type_name, default_method.name, *idx)
                                    {
                                        user_method.set_canon(can_id, cr.clone());
                                    }
                                    *idx = idx.wrapping_add(1);
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

/// Extract the first trait type argument as a `Name` for runtime dispatch.
///
/// For `impl Index<int, str> for T`, this returns `Some(Name("int"))`.
/// Used to disambiguate multiple impls of the same trait on the same type.
fn extract_key_type_hint(
    impl_def: &ori_ir::ImplDef,
    arena: &SharedArena,
    interner: &StringInterner,
) -> Option<Name> {
    if impl_def.trait_type_args.is_empty() {
        return None;
    }
    let type_arg_ids = arena.get_parsed_type_list(impl_def.trait_type_args);
    let first_id = *type_arg_ids.first()?;
    let parsed_type = arena.get_parsed_type(first_id);
    let hint = match parsed_type {
        ParsedType::Primitive(type_id) => {
            let name_str = type_id.name()?;
            Some(interner.intern(name_str))
        }
        ParsedType::Named { name, .. } => Some(*name),
        _ => None,
    };
    tracing::debug!(
        ?hint,
        ?parsed_type,
        arg_count = type_arg_ids.len(),
        "extract_key_type_hint"
    );
    hint
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
fn collect_extend_methods(
    module: &Module,
    arena: &SharedArena,
    captures: &Arc<FxHashMap<Name, Value>>,
    canon: Option<&SharedCanonResult>,
    registry: &mut UserMethodRegistry,
) {
    // Track canonical body indices for duplicate method definitions,
    // mirroring the pattern from collect_impl_methods.
    let mut method_canon_index: FxHashMap<(Name, Name), usize> = FxHashMap::default();

    for extend_def in &module.extends {
        // Get the target type name (e.g., "list" for `extend [T] { ... }`)
        let type_name = extend_def.target_type_name;

        // Register each method
        for method in &extend_def.methods {
            let params = arena.get_param_names(method.params);
            let mut user_method = UserMethod::new(params, Arc::clone(captures), arena.clone());

            if let Some(cr) = canon {
                let idx = method_canon_index
                    .entry((type_name, method.name))
                    .or_insert(0);
                if let Some(can_id) = cr.method_root_for_nth(type_name, method.name, *idx) {
                    user_method.set_canon(can_id, cr.clone());
                }
                *idx = idx.wrapping_add(1);
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
fn collect_def_impl_methods(
    module: &Module,
    arena: &SharedArena,
    captures: &Arc<FxHashMap<Name, Value>>,
    canon: Option<&SharedCanonResult>,
    registry: &mut UserMethodRegistry,
) {
    // Track canonical body indices for duplicate method definitions,
    // mirroring the pattern from collect_impl_methods.
    let mut method_canon_index: FxHashMap<(Name, Name), usize> = FxHashMap::default();

    for def_impl_def in &module.def_impls {
        let trait_name = def_impl_def.trait_name;

        for method in &def_impl_def.methods {
            let params = arena.get_param_names(method.params);
            let mut user_method = UserMethod::new(params, Arc::clone(captures), arena.clone());

            if let Some(cr) = canon {
                let idx = method_canon_index
                    .entry((trait_name, method.name))
                    .or_insert(0);
                if let Some(can_id) = cr.method_root_for_nth(trait_name, method.name, *idx) {
                    user_method.set_canon(can_id, cr.clone());
                }
                *idx = idx.wrapping_add(1);
            }

            // Register under trait name for `TraitName.method(...)` capability dispatch
            registry.register(trait_name, method.name, user_method);
        }
    }
}

/// Collect methods from def impl blocks into a registry.
///
/// Default implementations provide stateless methods for capability traits.
/// Methods are registered under the trait name for `TraitName.method(...)` calls.
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
mod tests;
