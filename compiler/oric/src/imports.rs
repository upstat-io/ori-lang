//! Unified Import Resolution.
//!
//! Provides a single `resolve_imports()` function that resolves all imports
//! for a module — prelude and explicit `use` statements — into a
//! `ResolvedImports` structure. This structure is consumed by all backends
//! (type checker, interpreter, LLVM JIT, AOT) so resolution happens once
//! and backends just consume the result.
//!
//! # Architecture
//!
//! ```text
//! resolve_imports(db, parse_result, file_path)
//!   ├── prelude resolution (walk-up search → load_file → parsed)
//!   └── use-statement resolution (resolve_import → parsed)
//!         ↓
//!   ResolvedImports { prelude, modules, imported_functions }
//!         ↓
//!   ├── type checker: register_resolved_imports()
//!   ├── interpreter: load_module() consumes prelude + modules
//!   └── LLVM JIT:    compile imported function bodies
//! ```

use crate::db::Db;
use crate::eval::module::import::resolve_import;
use crate::ir::Name;
use crate::parser::ParseOutput;
use crate::query::parsed;
use crate::typeck::{is_prelude_file, prelude_candidates};

use std::path::Path;

/// A resolved imported module: the parsed output and its source path.
pub struct ResolvedImportedModule {
    /// Full parsed module (functions, types, arena, etc.).
    pub parse_output: ParseOutput,
    /// Resolved file path (e.g., `library/std/testing.ori`).
    pub module_path: String,
    /// Index into the original `parse_result.module.imports` array.
    /// Enables consumers to map back to the source `UseDef` for
    /// visibility checking, alias handling, etc.
    pub import_index: usize,
}

/// Reference to a specific imported function within a resolved module.
pub struct ImportedFunctionRef {
    /// Name in the importing scope (may be aliased via `as`).
    pub local_name: Name,
    /// Name in the source module.
    pub original_name: Name,
    /// Index into `ResolvedImports::modules`.
    pub module_index: usize,
    /// Whether this is a module alias import (`use std.http as http`).
    pub is_module_alias: bool,
}

/// All resolved imports for a single file.
///
/// Produced by `resolve_imports()` and consumed by all backends.
pub struct ResolvedImports {
    /// Prelude module (if found and applicable).
    pub prelude: Option<ResolvedImportedModule>,
    /// Explicitly imported modules (from `use` statements), in source order.
    pub modules: Vec<ResolvedImportedModule>,
    /// Mapping of imported functions to their source modules.
    /// Each entry tracks the local name, original name, and which module it comes from.
    pub imported_functions: Vec<ImportedFunctionRef>,
    /// Import errors encountered during resolution.
    /// These are collected rather than failing fast so all errors are reported.
    pub errors: Vec<ImportError>,
}

/// An error encountered during import resolution.
pub struct ImportError {
    pub message: String,
    pub span: ori_ir::Span,
}

/// Resolve all imports for a file.
///
/// This is the single entry point for import resolution. It uses Salsa queries
/// internally (`load_file`, `parsed`, `resolve_import`) for caching and
/// dependency tracking.
///
/// # What it resolves
///
/// 1. **Prelude** — walks up from `file_path` looking for `library/std/prelude.ori`
/// 2. **Explicit imports** — resolves each `use` statement to a parsed module
///
/// # What it does NOT do
///
/// - Does not register functions with any checker/environment (that's the backend's job)
/// - Does not type-check imported modules (caller does this if needed)
/// - Does not build interpreter `FunctionValue`s (interpreter does this)
pub fn resolve_imports(
    db: &dyn Db,
    parse_result: &ParseOutput,
    file_path: &Path,
) -> ResolvedImports {
    let mut prelude = None;
    let mut modules = Vec::new();
    let mut imported_functions = Vec::new();
    let mut errors = Vec::new();

    // 1. Resolve prelude
    if !is_prelude_file(file_path) {
        let prelude_file = prelude_candidates(file_path)
            .iter()
            .find_map(|candidate| db.load_file(candidate));

        if let Some(prelude_file) = prelude_file {
            let prelude_parsed = parsed(db, prelude_file);
            prelude = Some(ResolvedImportedModule {
                parse_output: prelude_parsed,
                module_path: "std/prelude".to_string(),
                import_index: 0, // Not used for prelude (stored separately)
            });
        }
    }

    // 2. Resolve explicit imports
    for (imp_idx, imp) in parse_result.module.imports.iter().enumerate() {
        let resolved = match resolve_import(db, &imp.path, file_path) {
            Ok(resolved) => resolved,
            Err(e) => {
                let span = e.span.unwrap_or(imp.span);
                errors.push(ImportError {
                    message: e.message,
                    span,
                });
                continue;
            }
        };

        let imported_parsed = parsed(db, resolved.file);
        let module_path = resolved.path.display().to_string();

        let module_index = modules.len();
        modules.push(ResolvedImportedModule {
            parse_output: imported_parsed,
            module_path,
            import_index: imp_idx,
        });

        // Handle module alias imports (use std.http as http)
        if let Some(alias) = imp.module_alias {
            imported_functions.push(ImportedFunctionRef {
                local_name: alias,
                original_name: alias,
                module_index,
                is_module_alias: true,
            });
            continue;
        }

        // Handle individual item imports
        for item in &imp.items {
            imported_functions.push(ImportedFunctionRef {
                local_name: item.alias.unwrap_or(item.name),
                original_name: item.name,
                module_index,
                is_module_alias: false,
            });
        }
    }

    ResolvedImports {
        prelude,
        modules,
        imported_functions,
        errors,
    }
}
