//! Module type-checking entry points.
//!
//! Basic checks return [`TypeCheckResult`] and discard their fresh [`Pool`].
//! Pool-returning variants retain type data for diagnostics, evaluation, or codegen.

use ori_ir::{ExprArena, Module, StringInterner};

use super::bodies::{
    check_def_impl_bodies, check_function_bodies, check_impl_bodies, check_test_bodies,
};
use super::registration::{
    register_builtin_extensions, register_builtin_types, register_consts, register_derived_impls,
    register_extern_burdens, register_impls, register_object_safety_violations, register_traits,
    register_user_types,
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
/// ```rust
/// use ori_ir::StringInterner;
/// use ori_types::check_module;
///
/// let interner = StringInterner::new();
/// let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/doc_main.ori"));
/// let tokens = ori_lexer::lex(source, &interner);
/// let parse_output = ori_parse::parse(&tokens, &interner);
/// let result = check_module(&parse_output.module, &parse_output.arena, &interner);
///
/// assert!(!result.has_errors(), "{:?}", result.errors());
/// // Access typed module metadata.
/// assert_eq!(result.typed.function_count(), 1);
/// ```
#[tracing::instrument(level = "debug", skip_all)]
pub fn check_module(
    module: &Module,
    arena: &ExprArena,
    interner: &StringInterner,
) -> TypeCheckResult {
    ori_stack::ensure_sufficient_stack(|| {
        let mut checker = ModuleChecker::new(arena, interner);
        check_module_impl(&mut checker, module);
        checker.finish()
    })
}

/// Type check a module with pre-populated registries.
///
/// Use this when you have already resolved imports and need to
/// register their types/traits before checking.
///
/// # Example
///
/// ```rust
/// use ori_ir::StringInterner;
/// use ori_types::{check_module_with_registries, TraitRegistry, TypeRegistry};
///
/// let interner = StringInterner::new();
/// let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/doc_main.ori"));
/// let tokens = ori_lexer::lex(source, &interner);
/// let parsed = ori_parse::parse(&tokens, &interner);
/// let types = TypeRegistry::new();
/// let traits = TraitRegistry::new();
/// let result = check_module_with_registries(
///     &parsed.module, &parsed.arena, &interner, types, traits
/// );
/// assert!(!result.has_errors());
/// ```
#[tracing::instrument(level = "debug", skip_all)]
pub fn check_module_with_registries(
    module: &Module,
    arena: &ExprArena,
    interner: &StringInterner,
    types: TypeRegistry,
    traits: TraitRegistry,
) -> TypeCheckResult {
    ori_stack::ensure_sufficient_stack(|| {
        let mut checker = ModuleChecker::with_registries(arena, interner, types, traits);
        check_module_impl(&mut checker, module);
        checker.finish()
    })
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
    ori_stack::ensure_sufficient_stack(|| {
        let mut checker = ModuleChecker::new(arena, interner);
        check_module_impl(&mut checker, module);
        checker.intern_multi_clause_tuples(module);
        checker.finish_with_pool()
    })
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
/// ```rust
/// use ori_ir::StringInterner;
/// use ori_types::check_module_with_imports;
///
/// let interner = StringInterner::new();
/// let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/doc_main.ori"));
/// let tokens = ori_lexer::lex(source, &interner);
/// let parsed = ori_parse::parse(&tokens, &interner);
/// let (result, pool) = check_module_with_imports(
///     &parsed.module, &parsed.arena, &interner,
///     |_checker| {},
/// );
/// assert!(!result.has_errors());
/// assert_eq!(pool.format_type(ori_types::Idx::INT), "int");
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
    // Why: nested Salsa queries and body checking can exhaust macOS worker stacks.
    ori_stack::ensure_sufficient_stack(|| {
        let mut checker = ModuleChecker::new(arena, interner);
        register_fn(&mut checker);
        check_module_impl(&mut checker, module);
        checker.intern_multi_clause_tuples(module);
        checker.finish_with_pool()
    })
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
    register_builtin_types(checker);
    register_user_types(checker, module);

    // Spec: Annex E §FFI.
    register_extern_burdens(checker, module);

    // INVARIANT: Object-safety propagation precedes impl registration.
    // Spec: Clause 8.8.
    register_traits(checker, module);
    register_object_safety_violations(checker, module);
    register_impls(checker, module);
    // Why: Extension methods must be indexed before unknown-method diagnostics.
    register_builtin_extensions(checker, module);

    register_derived_impls(checker, module);
    register_consts(checker, module);
    tracing::debug!("registration passes complete");

    collect_signatures(checker, module);
    tracing::debug!("signature collection complete");

    check_function_bodies(checker, module);
    check_test_bodies(checker, module);
    check_impl_bodies(checker, module);
    check_def_impl_bodies(checker, module);
    tracing::debug!("body checking complete");
}

#[cfg(test)]
mod tests;
