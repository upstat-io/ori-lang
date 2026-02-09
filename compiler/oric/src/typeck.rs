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
//!   └── type_check_with_imports()
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

use ori_types::TypeCheckResult;

use crate::db::Db;
use crate::imports;
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

/// Type check a module with import support.
///
/// This is the main entry point called by the `typed()` Salsa query.
/// It creates a `ModuleChecker`, registers prelude and imported functions,
/// then runs all type checking passes.
///
/// The Pool is discarded — it's not part of the Salsa query result.
/// For scenarios that need the Pool (e.g., error rendering, evaluation),
/// use `type_check_with_imports_and_pool()` instead.
pub fn type_check_with_imports(
    db: &dyn Db,
    parse_result: &ParseOutput,
    current_file: &Path,
) -> TypeCheckResult {
    let (result, _pool) = type_check_with_imports_and_pool(db, parse_result, current_file);
    result
}

/// Type check a module, returning both the result and the Pool.
///
/// Unlike `type_check_with_imports()`, this retains the Pool so callers
/// can use it for error rendering, evaluation, or other Pool-dependent operations.
///
/// Used by `evaluated()` and the test runner where the Pool's type information
/// may be needed alongside the type checking result.
pub fn type_check_with_imports_and_pool(
    db: &dyn Db,
    parse_result: &ParseOutput,
    current_file: &Path,
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
            register_resolved_imports(&resolved, checker);
        },
    )
}

/// Register built-in functions that are implemented natively in the evaluator.
///
/// These functions (type conversions, print, panic, etc.) are not defined in
/// the prelude `.ori` file but are available in every Ori program. Their type
/// signatures are registered here so type checking can validate calls.
fn register_builtins(
    interner: &ori_ir::StringInterner,
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
    {
        let ordering_ty = checker.pool_mut().named(interner.intern("Ordering"));
        for variant in &["Less", "Equal", "Greater"] {
            let name = interner.intern(variant);
            checker.register_builtin_value(name, ordering_ty);
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
    for error in &resolved.errors {
        checker.push_error(ori_types::TypeCheckError::import_error(
            error.message.clone(),
            error.span,
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
            continue;
        };

        // Register with alias support
        if func_ref.local_name == func_ref.original_name {
            checker.register_imported_function(func, &imported_parsed.arena);
        } else {
            let mut aliased_func = func.clone();
            aliased_func.name = func_ref.local_name;
            checker.register_imported_function(&aliased_func, &imported_parsed.arena);
        }
    }
}
