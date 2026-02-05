//! Public API for module-level type checking.
//!
//! Provides the main entry points for checking modules.
//!
//! # Pool Lifecycle
//!
//! Each `check_module*` call creates a fresh [`Pool`] inside a [`ModuleChecker`].
//! The Pool owns all type data for that module. After checking:
//!
//! - **`check_module()`**: Pool is discarded. Use when only the [`TypeCheckResult`]
//!   is needed (e.g., Salsa `typed()` query).
//! - **`check_module_with_pool()`** / **`check_module_with_imports()`**: Returns
//!   `(TypeCheckResult, Pool)`. Use when the Pool is needed for:
//!   - Error rendering (`Pool::format_type()` resolves `Idx` to type names)
//!   - Code generation (LLVM codegen needs full type information)
//!   - LSP features (hover, completion)
//!
//! The evaluator (`ori_eval`) only needs `&[Idx]` (expression types) for
//! type-directed dispatch — it does NOT need the Pool at runtime.
//!
//! In the Salsa pipeline (`oric/src/query/mod.rs`):
//! ```text
//! typed()     → calls type_check_with_imports() → discards Pool
//! evaluated() → calls type_check_with_imports_and_pool() → passes &[Idx] to evaluator
//! ori check   → calls type_check_with_imports_and_pool() → uses Pool for error rendering
//! ```

use ori_ir::{ExprArena, Module, StringInterner};

use super::bodies::{
    check_def_impl_bodies, check_function_bodies, check_impl_bodies, check_test_bodies,
};
use super::registration::{
    register_builtin_types, register_consts, register_derived_impls, register_impls,
    register_traits, register_user_types,
};
use super::signatures::collect_signatures;
use super::ModuleChecker;
use crate::{Pool, TraitRegistry, TypeCheckResult, TypeRegistry};

/// Type check a module and return the typed representation.
///
/// This is the main entry point for type checking.
///
/// # Example
///
/// ```ignore
/// let parse_output = parse_module(source);
/// let result = check_module(&parse_output.module, &parse_output.arena, &interner);
///
/// if result.has_errors() {
///     for error in result.errors() {
///         eprintln!("{}", error);
///     }
/// }
///
/// // Access expression types
/// let expr_ty = result.typed.expr_type(expr_index);
/// ```
#[tracing::instrument(level = "debug", skip_all)]
pub fn check_module(
    module: &Module,
    arena: &ExprArena,
    interner: &StringInterner,
) -> TypeCheckResult {
    let mut checker = ModuleChecker::new(arena, interner);
    check_module_impl(&mut checker, module);
    checker.finish()
}

/// Type check a module with pre-populated registries.
///
/// Use this when you have already resolved imports and need to
/// register their types/traits before checking.
///
/// # Example
///
/// ```ignore
/// // Resolve imports first
/// let (types, traits) = resolve_imports(&imports, db);
///
/// let result = check_module_with_registries(
///     &module, &arena, &interner, types, traits
/// );
/// ```
#[tracing::instrument(level = "debug", skip_all)]
pub fn check_module_with_registries(
    module: &Module,
    arena: &ExprArena,
    interner: &StringInterner,
    types: TypeRegistry,
    traits: TraitRegistry,
) -> TypeCheckResult {
    let mut checker = ModuleChecker::with_registries(arena, interner, types, traits);
    check_module_impl(&mut checker, module);
    checker.finish()
}

/// Type check a module and return both the result and the pool.
///
/// Use this when you need access to the pool for type resolution
/// after checking (e.g., for code generation or LSP features).
#[tracing::instrument(level = "debug", skip_all)]
pub fn check_module_with_pool(
    module: &Module,
    arena: &ExprArena,
    interner: &StringInterner,
) -> (TypeCheckResult, Pool) {
    let mut checker = ModuleChecker::new(arena, interner);
    check_module_impl(&mut checker, module);
    checker.finish_with_pool()
}

/// Type check a module with imports registered via a closure.
///
/// The `register_fn` closure receives a mutable reference to the
/// `ModuleChecker` and should call `register_imported_function()` and/or
/// `register_module_alias()` to wire imported functions into the type checker.
///
/// This closure-based API decouples `ori_types` from `oric`-specific types
/// (Salsa, file resolution, etc.), letting `oric` orchestrate import resolution
/// while `ori_types` provides the registration mechanism.
///
/// # Example
///
/// ```ignore
/// let (result, pool) = check_module_with_imports(
///     &module, &arena, &interner,
///     |checker| {
///         // Register functions from another module
///         for func in &other_module.functions {
///             checker.register_imported_function(func, &other_arena);
///         }
///     },
/// );
/// ```
#[tracing::instrument(level = "debug", skip_all)]
pub fn check_module_with_imports<F>(
    module: &Module,
    arena: &ExprArena,
    interner: &StringInterner,
    register_fn: F,
) -> (TypeCheckResult, Pool)
where
    F: FnOnce(&mut ModuleChecker<'_>),
{
    let mut checker = ModuleChecker::new(arena, interner);
    register_fn(&mut checker);
    check_module_impl(&mut checker, module);
    checker.finish_with_pool()
}

/// Internal implementation of module checking.
///
/// Runs all passes in order:
/// 1. Registration passes (0a-0e)
/// 2. Function signature collection
/// 3. Function body checking
/// 4. Test body checking
/// 5. Impl method body checking
#[tracing::instrument(level = "debug", skip_all, fields(
    functions = module.functions.len(),
    tests = module.tests.len(),
    impls = module.impls.len(),
))]
fn check_module_impl(checker: &mut ModuleChecker<'_>, module: &Module) {
    // Pass 0a: Register built-in types
    register_builtin_types(checker);

    // Pass 0b: Register user-defined types
    register_user_types(checker, module);

    // Pass 0c: Register traits and implementations
    register_traits(checker, module);
    register_impls(checker, module);

    // Pass 0d: Register derived implementations
    register_derived_impls(checker, module);

    // Pass 0e: Register config variables
    register_consts(checker, module);
    tracing::debug!("registration passes complete");

    // Pass 1: Collect function signatures
    collect_signatures(checker, module);
    tracing::debug!("signature collection complete");

    // Pass 2: Check function bodies
    check_function_bodies(checker, module);

    // Pass 3: Check test bodies
    check_test_bodies(checker, module);

    // Pass 4: Check impl method bodies
    check_impl_bodies(checker, module);

    // Pass 5: Check def impl method bodies
    check_def_impl_bodies(checker, module);
    tracing::debug!("body checking complete");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_empty_module() {
        let arena = ExprArena::new();
        let interner = StringInterner::new();
        let module = Module::default();

        let result = check_module(&module, &arena, &interner);

        assert!(!result.has_errors());
        assert!(result.typed.functions.is_empty());
    }

    #[test]
    fn check_module_with_pool_returns_pool() {
        let arena = ExprArena::new();
        let interner = StringInterner::new();
        let module = Module::default();

        let (result, pool) = check_module_with_pool(&module, &arena, &interner);

        assert!(!result.has_errors());
        // Pool should have pre-interned primitives
        assert_eq!(pool.tag(crate::Idx::INT), crate::Tag::Int);
    }
}
