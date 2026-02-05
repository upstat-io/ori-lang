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
//!       └── ori_types::check_module_with_imports(module, arena, interner, |checker| {
//!               register_prelude()    ← loads prelude via Salsa
//!               register_imports()    ← resolves imports via Salsa
//!           })
//! ```
//!
//! The closure-based API decouples `ori_types` from oric-specific types
//! (Salsa, file resolution). oric orchestrates import resolution; `ori_types`
//! handles type resolution internally via `ModuleChecker`.

use std::path::{Path, PathBuf};

use ori_types::TypeCheckResult;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::db::Db;
use crate::eval::module::import::resolve_import;
use crate::ir::Name;
use crate::parser::ParseOutput;
use crate::query::parsed;

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

    // Use closure-based API: oric orchestrates import resolution,
    // ori_types handles type resolution internally.
    ori_types::check_module_with_imports(
        &parse_result.module,
        &parse_result.arena,
        interner,
        |checker| {
            register_builtins(interner, checker);
            register_prelude(db, current_file, checker);
            register_imports(db, parse_result, current_file, checker);
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

/// Register prelude functions with the type checker.
///
/// Uses `register_imported_function()` which handles type resolution internally.
fn register_prelude(db: &dyn Db, current_file: &Path, checker: &mut ori_types::ModuleChecker<'_>) {
    // Don't load prelude if we're type checking the prelude itself
    if is_prelude_file(current_file) {
        return;
    }

    // Find the prelude file via Salsa
    let prelude_file = prelude_candidates(current_file)
        .iter()
        .find_map(|candidate| db.load_file(candidate));

    let Some(prelude_file) = prelude_file else {
        // Prelude not found — okay for tests outside the project
        return;
    };

    let prelude_parsed = parsed(db, prelude_file);

    // Register all public prelude functions
    for func in &prelude_parsed.module.functions {
        if func.visibility.is_public() {
            checker.register_imported_function(func, &prelude_parsed.arena);
        }
    }
}

/// Register imported functions and module aliases with the type checker.
///
/// For each import in the module:
/// 1. Resolve the import path to a file (via Salsa's `resolve_import`)
/// 2. Parse the imported file (via Salsa's `parsed` query)
/// 3. Register functions using `register_imported_function()`
///
/// The `ModuleChecker` resolves types from the AST using its own Pool internally.
fn register_imports(
    db: &dyn Db,
    parse_result: &ParseOutput,
    current_file: &Path,
    checker: &mut ori_types::ModuleChecker<'_>,
) {
    for imp in &parse_result.module.imports {
        // Resolve import path to a file
        let resolved = match resolve_import(db, &imp.path, current_file) {
            Ok(resolved) => resolved,
            Err(e) => {
                // Push import error to checker
                let span = e.span.unwrap_or(imp.span);
                checker.push_error(ori_types::TypeCheckError::import_error(e.message, span));
                continue;
            }
        };

        // Parse the imported file via Salsa query
        let imported_parsed = parsed(db, resolved.file);

        // Handle module alias imports (use std.http as http)
        if let Some(alias) = imp.module_alias {
            checker.register_module_alias(alias, &imported_parsed.module, &imported_parsed.arena);
            continue;
        }

        // Handle individual item imports
        // Build a map of imported function names to their aliases
        let import_map: FxHashMap<Name, Option<Name>> = imp
            .items
            .iter()
            .map(|item| (item.name, item.alias))
            .collect();

        // Build a set of names that request private access
        let private_access: FxHashSet<Name> = imp
            .items
            .iter()
            .filter(|item| item.is_private)
            .map(|item| item.name)
            .collect();

        // Register each imported function
        for func in &imported_parsed.module.functions {
            // Only include functions that are actually imported
            let Some(&alias) = import_map.get(&func.name) else {
                continue;
            };

            // Note: Visibility enforcement for imports is not yet active.
            // V1 allowed importing any named function regardless of visibility.
            // When visibility enforcement is added (roadmap: Section 4 - Modules),
            // this should check: !func.visibility.is_public() && !private_access.contains(&func.name)
            let _ = &private_access; // suppress unused warning

            // Register with alias support: if aliased, we need to register
            // under the alias name. We do this by registering the function
            // and then updating the signature name if aliased.
            if let Some(alias_name) = alias {
                // For aliased imports, create a renamed copy of the function
                // We register the function first, then re-register under the alias
                let mut aliased_func = func.clone();
                aliased_func.name = alias_name;
                checker.register_imported_function(&aliased_func, &imported_parsed.arena);
            } else {
                checker.register_imported_function(func, &imported_parsed.arena);
            }
        }
    }
}
