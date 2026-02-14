//! Type Checking Bridge.
//!
//! Wires `ori_types::check_module_with_imports` into oric's Salsa query
//! pipeline. This module handles the oric-specific concerns (import resolution,
//! prelude loading, Salsa queries) while delegating type checking to `ori_types`.
//!
//! # Architecture
//!
//! ```text
//! typed() query
//!   └── type_check_with_imports_and_pool()
//!       └── resolve_imports()  ← unified import pipeline
//!           └── ori_types::check_module_with_imports(module, arena, interner, |checker| {
//!                   register_builtins()
//!                   register_resolved_imports()  ← consumes ResolvedImports
//!               })
//! ```
//!
//! The closure-based API decouples `ori_types` from oric-specific types
//! (Salsa, file resolution). oric orchestrates import resolution; `ori_types`
//! handles type resolution internally via `ModuleChecker`.

use std::path::{Path, PathBuf};

use ori_types::{FunctionSig, TypeCheckResult};

use crate::db::Db;
use crate::imports;
use crate::ir::{Name, Span, StringInterner};
use crate::parser::ParseOutput;

// Prelude Auto-Loading

/// Generate candidate paths for the prelude by walking up from the current file.
pub(crate) fn prelude_candidates(current_file: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut dir = current_file.parent();
    while let Some(d) = dir {
        candidates.push(d.join("library").join("std").join("prelude.ori"));
        dir = d.parent();
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

    // Resolve all imports via the unified pipeline.
    let resolved = imports::resolve_imports(db, parse_result, current_file);

    // Use closure-based API: oric orchestrates import resolution,
    // ori_types handles type resolution internally.
    ori_types::check_module_with_imports(
        &parse_result.module,
        &parse_result.arena,
        interner,
        |checker| {
            register_builtins(interner, checker);
            register_resolved_imports(&resolved, checker, interner);
        },
    )
}

/// Register built-in functions that are implemented natively in the evaluator.
///
/// These functions (type conversions, print, panic, etc.) are not defined in
/// the prelude `.ori` file but are available in every Ori program. Their type
/// signatures are registered here so type checking can validate calls.
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

/// Register prelude and imported functions with the type checker from resolved imports.
///
/// Consumes a `ResolvedImports` produced by the unified import pipeline.
/// Uses `resolved.imported_functions` directly — each entry already tracks the
/// local name, original name, source module, and whether it's a module alias.
fn register_resolved_imports(
    resolved: &imports::ResolvedImports,
    checker: &mut ori_types::ModuleChecker<'_>,
    interner: &StringInterner,
) {
    // 1. Register prelude functions (all public)
    if let Some(ref prelude) = resolved.prelude {
        for func in &prelude.parse_output.module.functions {
            if func.visibility.is_public() {
                checker.register_imported_function(func, &prelude.parse_output.arena);
            }
        }
    }

    // 2. Report any import resolution errors
    //
    // Span is guaranteed present: resolve_imports() always fills in the
    // use-statement span via `e.span.unwrap_or(imp.span)` (imports.rs:210).
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

    // 3. Register explicitly imported functions
    // Each imported_function ref maps directly to a resolved module and function.
    for func_ref in &resolved.imported_functions {
        let module = &resolved.modules[func_ref.module_index];
        let imported_parsed = &module.parse_output;

        // Module alias imports: register the entire module under an alias name
        if func_ref.is_module_alias {
            checker.register_module_alias(
                func_ref.local_name,
                &imported_parsed.module,
                &imported_parsed.arena,
            );
            continue;
        }

        // Find the function by its original name in the source module
        let Some(func) = imported_parsed
            .module
            .functions
            .iter()
            .find(|f| f.name == func_ref.original_name)
        else {
            report_missing_import(checker, interner, func_ref, &module.module_path);
            continue;
        };

        // Register with alias support
        if func_ref.local_name == func_ref.original_name {
            checker.register_imported_function(func, &imported_parsed.arena);
        } else {
            checker.register_imported_function_as(
                func,
                &imported_parsed.arena,
                func_ref.local_name,
            );
        }
    }
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

/// Build function signatures aligned with `module.functions` source order.
///
/// `typed.functions` is sorted by name (for Salsa determinism), while
/// `module.functions` is in source order. `FunctionCompiler::declare_all`
/// zips them, so they must be aligned.
#[cfg_attr(
    not(feature = "llvm"),
    expect(
        dead_code,
        reason = "consumed by #[cfg(feature = \"llvm\")] paths in compile_common and test runner"
    )
)]
pub(crate) fn build_function_sigs(
    parse_result: &ParseOutput,
    type_result: &TypeCheckResult,
) -> Vec<FunctionSig> {
    let sig_map: rustc_hash::FxHashMap<Name, &FunctionSig> = type_result
        .typed
        .functions
        .iter()
        .map(|ft| (ft.name, ft))
        .collect();

    parse_result
        .module
        .functions
        .iter()
        .map(|func| {
            sig_map
                .get(&func.name)
                .copied()
                .cloned()
                .unwrap_or_else(|| dummy_sig(func.name))
        })
        .collect()
}

/// Fallback signature for functions missing from the type check result.
///
/// Should never be reached after successful type checking — only exists to
/// prevent panics if the signature map is incomplete.
#[cold]
fn dummy_sig(name: Name) -> FunctionSig {
    use ori_types::Idx;

    debug_assert!(false, "function {name:?} has no type-checked signature");
    tracing::warn!(
        name = ?name,
        "function missing from type check result — using dummy signature"
    );
    FunctionSig {
        name,
        type_params: vec![],
        const_params: vec![],
        param_names: vec![],
        param_types: vec![],
        return_type: Idx::UNIT,
        capabilities: vec![],
        is_public: false,
        is_test: false,
        is_main: false,
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        required_params: 0,
        param_defaults: vec![],
    }
}
