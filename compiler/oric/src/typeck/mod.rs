//! Bridge from `oric` queries to `ori_types` module checking.
//!
//! `oric` resolves imports and the prelude, then supplies those surfaces through
//! the checker callback. `ori_types` remains independent of Salsa and file lookup.

mod metadata;

pub(crate) use metadata::{
    build_function_sigs, collect_metadata_from_results, collect_surfaces_from_results,
};

use std::path::{Path, PathBuf};

use ori_types::TypeCheckResult;

use crate::db::Db;
use crate::imports;
use crate::input::SourceFile;
use crate::ir::{Span, StringInterner};
use crate::parser::ParseOutput;

// Prelude Auto-Loading

/// Generate candidate paths for the prelude by walking up from the current file.
///
/// Search order:
/// 1. `$ORI_STDLIB/std/prelude.ori` (if env var set). A value pointing
///    directly at `library/std/` also resolves (parent treated as the root).
/// 2. Walk up from `current_file` looking for `<ancestor>/library/std/prelude.ori`
/// 3. User-local install: `~/.local/share/ori/library/std/prelude.ori`
/// 4. System locations: `/usr/local/lib/ori/stdlib/std/prelude.ori`
pub(crate) fn prelude_candidates(current_file: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(stdlib) = std::env::var("ORI_STDLIB") {
        for root in imports::ori_stdlib_library_roots(&stdlib) {
            candidates.push(root.join("std").join("prelude.ori"));
        }
    }

    let mut dir = current_file.parent();
    while let Some(d) = dir {
        candidates.push(d.join("library").join("std").join("prelude.ori"));
        dir = d.parent();
    }

    if let Ok(home) = std::env::var("HOME") {
        candidates.push(PathBuf::from(&home).join(".local/share/ori/library/std/prelude.ori"));
    }

    // 4. System locations
    for base in crate::imports::SYSTEM_STDLIB_ROOTS {
        candidates.push(PathBuf::from(base).join("std").join("prelude.ori"));
    }

    candidates
}

/// Check if a file is the prelude itself (to avoid recursive loading).
pub(crate) fn is_prelude_file(file_path: &Path) -> bool {
    file_path.ends_with("library/std/prelude.ori")
        || (file_path.file_name().is_some_and(|n| n == "prelude.ori")
            && file_path.parent().is_some_and(|p| p.ends_with("std")))
}

/// Type check a module with import support, returning both the result and the Pool.
///
/// This is the main entry point called by the `typed()` Salsa query and by
/// the evaluator's module loading for imported modules. It creates a
/// `ModuleChecker`, registers prelude and imported functions, then runs
/// all type checking passes.
///
/// # Cache Safety
///
/// Requires a [`CacheGuard`] proving that session-scoped side-caches have
/// been invalidated (or are not applicable for this module). This prevents
/// callers from accidentally using stale `PoolCache`/`CanonCache`/`ImportsCache`
/// entries after re-type-checking.
pub(crate) fn type_check_with_imports_and_pool(
    db: &dyn Db,
    parse_result: &ParseOutput,
    current_file: &Path,
    _guard: crate::query::CacheGuard,
) -> (TypeCheckResult, ori_types::Pool) {
    let interner = db.interner();

    let resolved = imports::resolve_imports(db, parse_result, current_file);

    // Use closure-based API: oric orchestrates import resolution,
    // ori_types handles type resolution internally.
    ori_types::check_module_with_imports(
        &parse_result.module,
        &parse_result.arena,
        interner,
        |checker| {
            register_builtins(interner, checker);
            register_resolved_imports(db, &resolved, current_file, checker, interner);
        },
    )
}

/// Register built-in functions that are implemented natively in the evaluator.
///
/// Compiler-provided functions such as type conversions, print, and panic are
/// available in every Ori program. Their registered type signatures let type
/// checking validate calls.
pub(crate) fn register_builtins(
    interner: &StringInterner,
    checker: &mut ori_types::ModuleChecker<'_>,
) {
    use ori_types::Idx;

    // Type conversion functions: T -> concrete_type
    // These accept any type (fresh type variable) and return the target type.
    let builtins: &[(&str, Idx)] = &[
        ("int", Idx::INT),
        ("float", Idx::FLOAT),
        ("str", Idx::STR),
        ("byte", Idx::BYTE),
    ];

    for &(name_str, return_type) in builtins {
        let name = interner.intern(name_str);
        let param = checker.pool_mut().fresh_var();
        let var_id = checker.pool().data(param);
        checker.register_builtin_function(name, &[param], return_type, &[var_id]);
    }

    // print(value: T) -> void — accepts any printable value
    {
        let name = interner.intern("print");
        let param = checker.pool_mut().fresh_var();
        let var_id = checker.pool().data(param);
        checker.register_builtin_function(name, &[param], Idx::UNIT, &[var_id]);
    }

    // thread_id() -> int — monomorphic
    {
        let name = interner.intern("thread_id");
        checker.register_builtin_function(name, &[], Idx::INT, &[]);
    }

    // Ordering values: Less, Equal, Greater
    // Must use the pre-interned Idx::ORDERING — pool.named() would create a
    // different Named idx that doesn't unify with return type annotations.
    {
        for variant in &["Less", "Equal", "Greater"] {
            let name = interner.intern(variant);
            checker.register_builtin_value(name, ori_types::Idx::ORDERING);
        }
    }
}

/// Type-check `sf` via `crate::query::typed`, or synthesize a poisoned
/// result carrying a `CircularImport` diagnostic when `sf` is already
/// mid-typecheck on the current call stack.
///
/// Salsa's own cycle detector fires at the CALL SITE (before a `#[salsa::tracked]`
/// query's body runs), so this check MUST happen before `crate::query::typed`
/// is invoked — checking inside `typed()`'s own body would be too late.
fn typed_or_poison(db: &dyn Db, sf: SourceFile) -> TypeCheckResult {
    let path = sf.path(db);
    match db.typing_stack().cycle_path(path) {
        Some(cycle) => poisoned_circular_import_result(&cycle),
        None => crate::query::typed(db, sf),
    }
}

/// Build a poisoned `TypeCheckResult` carrying one `CircularImport` error
/// naming the full cycle (per spec 18.7 "reports all cycles found").
fn poisoned_circular_import_result(cycle: &[PathBuf]) -> TypeCheckResult {
    let joined = cycle
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(" -> ");
    let error = ori_types::TypeCheckError::import_error(
        format!("circular import detected: {joined}"),
        Span::DUMMY,
        ori_types::ImportErrorKind::CircularImport,
    );
    ori_types::TypeCheckResult::from_typed(ori_types::TypedModule {
        errors: vec![error],
        ..ori_types::TypedModule::default()
    })
}

/// Copy every `CircularImport` diagnostic found among `results` into
/// `checker`'s own error list.
///
/// A cyclic import is discovered at whichever recursion level directly
/// attempts the doomed `typed()` call (via [`typed_or_poison`]'s poisoned
/// fallback); without this propagation step the diagnostic stays trapped in
/// that one result and never reaches the entry file the user actually asked
/// to check. Called uniformly at every level of the `register_resolved_imports`
/// recursion, so the diagnostic bubbles all the way up regardless of which
/// file in the cycle is the driver's entry point.
fn propagate_circular_import_errors<'a>(
    checker: &mut ori_types::ModuleChecker<'_>,
    results: impl Iterator<Item = &'a TypeCheckResult>,
) {
    for result in results {
        for error in result.errors() {
            if matches!(
                error.kind,
                ori_types::TypeErrorKind::ImportError {
                    kind: ori_types::ImportErrorKind::CircularImport,
                    ..
                }
            ) {
                checker.push_error(error.clone());
            }
        }
    }
}

/// Register public imported traits before their provider-owned impl templates.
///
/// The ordering makes foreign trait names available while each impl is
/// reconstructed in the consumer pool. The resolver-owned module path supplies
/// the stable producer identity used by later method realization.
fn register_imported_impl_templates(
    resolved: &imports::ResolvedImports,
    checker: &mut ori_types::ModuleChecker<'_>,
) {
    for module in &resolved.modules {
        checker.register_imported_traits(&module.parse_output.module, &module.parse_output.arena);
    }
    for module in &resolved.modules {
        checker.register_imported_impls(
            &module.parse_output.module,
            &module.parse_output.arena,
            &module.module_path.to_string_lossy(),
        );
    }
}

/// Register prelude and imported functions with the type checker from resolved imports.
///
/// Consumes a `ResolvedImports` produced by the unified import pipeline.
/// Uses `resolved.imported_functions` directly — each entry already tracks the
/// local name, original name, source module, and whether it's a module alias.
/// Register every public prelude function and trait, propagating any
/// circular-import errors surfaced while type-checking the prelude itself.
fn register_prelude(
    db: &dyn Db,
    resolved: &imports::ResolvedImports,
    checker: &mut ori_types::ModuleChecker<'_>,
) {
    let Some(ref prelude) = resolved.prelude else {
        return;
    };
    // Why: prelude signatures use the cached hash-first resolution path.
    let prelude_tcr = prelude.source_file.map(|sf| typed_or_poison(db, sf));
    propagate_circular_import_errors(checker, prelude_tcr.iter());

    for func in &prelude.parse_output.module.functions {
        if func.visibility.is_public() {
            let imported_sig = prelude_tcr
                .as_ref()
                .and_then(|r| r.typed.functions.iter().find(|s| s.name == func.name));
            checker.register_imported_function(func, &prelude.parse_output.arena, imported_sig);
        }
    }
    checker.register_imported_traits(&prelude.parse_output.module, &prelude.parse_output.arena);
}

/// Push every resolved-import error onto the checker, guarding the
/// invariant that `resolve_imports` always attaches a span.
fn push_import_errors(
    resolved: &imports::ResolvedImports,
    checker: &mut ori_types::ModuleChecker<'_>,
) {
    for error in &resolved.errors {
        debug_assert!(
            error.span.is_some(),
            "import errors from resolve_imports should always have spans"
        );
        let span = error.span.unwrap_or_else(|| {
            tracing::error!(
                message = %error.message,
                "import error missing span — resolve_imports invariant violated"
            );
            Span::DUMMY
        });
        checker.push_error(ori_types::TypeCheckError::import_error(
            error.message.clone(),
            span,
            error.kind,
        ));
    }
}

/// Type-check every imported module once (caching the result for signature
/// resolution) and collect each module's frozen pool alongside it.
fn collect_module_results_and_pools(
    db: &dyn Db,
    resolved: &imports::ResolvedImports,
    checker: &mut ori_types::ModuleChecker<'_>,
) -> (
    Vec<Option<TypeCheckResult>>,
    Vec<Option<std::sync::Arc<ori_types::Pool>>>,
) {
    let module_results: Vec<Option<TypeCheckResult>> = resolved
        .modules
        .iter()
        .map(|m| m.source_file.map(|sf| typed_or_poison(db, sf)))
        .collect();
    propagate_circular_import_errors(checker, module_results.iter().flatten());
    let module_pools: Vec<Option<std::sync::Arc<ori_types::Pool>>> = resolved
        .modules
        .iter()
        .map(|module| {
            module
                .source_file
                .and_then(|source_file| crate::query::typed_pool(db, source_file))
        })
        .collect();
    (module_results, module_pools)
}

/// Collect imported type metadata and collection surfaces for transitive
/// forwarding, from the prelude plus every imported module's results.
fn register_imported_metadata(
    db: &dyn Db,
    resolved: &imports::ResolvedImports,
    module_results: &[Option<TypeCheckResult>],
    checker: &mut ori_types::ModuleChecker<'_>,
) {
    let prelude_result = resolved
        .prelude
        .as_ref()
        .and_then(|p| p.source_file.map(|sf| typed_or_poison(db, sf)));
    propagate_circular_import_errors(checker, prelude_result.iter());

    let imported_metadata = collect_metadata_from_results(prelude_result.as_ref(), module_results);
    let imported_surfaces = collect_surfaces_from_results(prelude_result.as_ref(), module_results);

    if !imported_metadata.is_empty() {
        checker.set_imported_type_metadata(imported_metadata);
    }
    if !imported_surfaces.is_empty() {
        checker.set_imported_collection_surfaces(imported_surfaces);
    }
}

/// Register every explicitly-imported function (or whole-module alias)
/// against its resolved source module and cached signature.
fn register_imported_functions(
    resolved: &imports::ResolvedImports,
    module_results: &[Option<TypeCheckResult>],
    checker: &mut ori_types::ModuleChecker<'_>,
    interner: &StringInterner,
) {
    for func_ref in &resolved.imported_functions {
        let module = &resolved.modules[func_ref.module_index];
        let imported_parsed = &module.parse_output;

        if func_ref.is_module_alias {
            checker.register_module_alias(
                func_ref.local_name,
                &imported_parsed.module,
                &imported_parsed.arena,
            );
            continue;
        }

        let Some(func) = imported_parsed
            .module
            .functions
            .iter()
            .find(|f| f.name == func_ref.original_name)
        else {
            report_missing_import(checker, interner, func_ref, &module.module_path);
            continue;
        };

        let imported_sig = module_results[func_ref.module_index]
            .as_ref()
            .and_then(|r| {
                r.typed
                    .functions
                    .iter()
                    .find(|s| s.name == func_ref.original_name)
            });

        if func_ref.local_name == func_ref.original_name {
            checker.register_imported_function(func, &imported_parsed.arena, imported_sig);
        } else {
            checker.register_imported_function_as(
                func,
                &imported_parsed.arena,
                func_ref.local_name,
                imported_sig,
            );
        }
    }
}

fn register_resolved_imports(
    db: &dyn Db,
    resolved: &imports::ResolvedImports,
    current_file: &Path,
    checker: &mut ori_types::ModuleChecker<'_>,
    interner: &StringInterner,
) {
    register_prelude(db, resolved, checker);
    push_import_errors(resolved, checker);

    let (module_results, module_pools) = collect_module_results_and_pools(db, resolved, checker);

    // Imported impl selection is consumer-owned. Register the complete direct
    // import graph before local registration and derive closure run.
    register_imported_impl_templates(resolved, checker);

    register_imported_metadata(db, resolved, &module_results, checker);

    register_imported_constants(
        resolved,
        current_file,
        &module_results,
        &module_pools,
        checker,
        interner,
    );

    register_imported_functions(resolved, &module_results, checker, interner);
}

/// Register selected constant types in the consumer pool.
///
/// The provider owns inference of its initializer. The import boundary only
/// transfers that already-resolved type into the consumer pool, mirroring the
/// cross-pool signature handling for imported functions without moving
/// constant-value evaluation into `ori_types`.
fn register_imported_constants(
    resolved: &imports::ResolvedImports,
    current_file: &Path,
    module_results: &[Option<TypeCheckResult>],
    module_pools: &[Option<std::sync::Arc<ori_types::Pool>>],
    checker: &mut ori_types::ModuleChecker<'_>,
    interner: &StringInterner,
) {
    for constant_ref in &resolved.imported_constants {
        let module = &resolved.modules[constant_ref.module_index];
        let imported_module = &module.parse_output.module;
        let Some(constant) = imported_module
            .consts
            .iter()
            .find(|constant| constant.name == constant_ref.original_name)
        else {
            report_missing_constant(checker, interner, constant_ref, &module.module_path);
            continue;
        };

        let parent_test_access = imports::is_test_module(current_file)
            && imports::is_parent_module_import(current_file, &module.module_path);
        if !constant.visibility.is_public() && !parent_test_access {
            let name = interner.lookup(constant_ref.original_name);
            checker.push_error(ori_types::TypeCheckError::import_error(
                format!(
                    "constant '${name}' is private in module '{}'; add `pub` to the constant definition before importing it",
                    module.module_path.display()
                ),
                constant_ref.span,
                ori_types::ImportErrorKind::PrivateAccess,
            ));
            continue;
        }

        let Some(source_result) = module_results[constant_ref.module_index].as_ref() else {
            continue;
        };
        let Some(source_pool) = module_pools[constant_ref.module_index].as_deref() else {
            checker.push_error(ori_types::TypeCheckError::import_error(
                format!(
                    "cannot import constant '${}': the provider type pool is unavailable; check the provider module for type errors",
                    interner.lookup(constant_ref.original_name)
                ),
                constant_ref.span,
                ori_types::ImportErrorKind::ItemNotFound,
            ));
            continue;
        };
        let Some(source_ty) = source_result.typed.expr_type(constant.value.index()) else {
            checker.push_error(ori_types::TypeCheckError::import_error(
                format!(
                    "cannot import constant '${}': its initializer has no resolved type; fix errors in '{}' first",
                    interner.lookup(constant_ref.original_name),
                    module.module_path.display()
                ),
                constant_ref.span,
                ori_types::ImportErrorKind::ItemNotFound,
            ));
            continue;
        };

        let mut cache = rustc_hash::FxHashMap::default();
        let consumer_ty =
            ori_types::re_intern_type(source_pool, source_ty, checker.pool_mut(), &mut cache);
        checker.register_const_type(constant_ref.local_name, consumer_ty);
    }
}

#[cold]
fn report_missing_constant(
    checker: &mut ori_types::ModuleChecker<'_>,
    interner: &StringInterner,
    constant_ref: &imports::ImportedConstantRef,
    module_path: &Path,
) {
    let name = interner.lookup(constant_ref.original_name);
    checker.push_error(ori_types::TypeCheckError::import_error(
        format!(
            "constant '${name}' not found in module '{}'; export that constant or correct the name in the `use` list",
            module_path.display()
        ),
        constant_ref.span,
        ori_types::ImportErrorKind::ItemNotFound,
    ));
}

/// Report a missing imported function to the type checker.
#[cold]
fn report_missing_import(
    checker: &mut ori_types::ModuleChecker<'_>,
    interner: &StringInterner,
    func_ref: &imports::ImportedFunctionRef,
    module_path: &std::path::Path,
) {
    let func_name = interner.lookup(func_ref.original_name);
    checker.push_error(ori_types::TypeCheckError::import_error(
        format!(
            "function '{func_name}' not found in module '{}'",
            module_path.display()
        ),
        func_ref.span,
        ori_types::ImportErrorKind::ItemNotFound,
    ));
}

#[cfg(test)]
mod tests;
