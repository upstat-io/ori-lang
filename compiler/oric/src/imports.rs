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

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::db::Db;
use crate::eval::module::import::resolve_import;
use crate::ir::Name;
use crate::parser::ParseOutput;
use crate::query::parsed;
use crate::typeck::{is_prelude_file, prelude_candidates};

/// A resolved imported module: the parsed output and its source path.
pub struct ResolvedImportedModule {
    /// Full parsed module (functions, types, arena, etc.).
    pub parse_output: ParseOutput,
    /// Resolved file path (e.g., `library/std/testing.ori`).
    ///
    /// Uses `PathBuf` (not `String`) for consistent key types across all
    /// three side-caches (`PoolCache`, `CanonCache`, `ImportsCache`).
    pub module_path: PathBuf,
    /// The Salsa `SourceFile` input for this module.
    ///
    /// Enables consumers to use Salsa queries (`typed()`, `typed_pool()`) instead
    /// of bypassing the query pipeline. This ensures type check results are cached
    /// in Salsa's dependency graph and the Pool is stored in `PoolCache`.
    pub source_file: Option<crate::input::SourceFile>,
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
    /// Source span of the `use` statement this import came from.
    /// Used for error reporting when the imported item is not found.
    pub span: ori_ir::Span,
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

/// Re-export the canonical `ImportErrorKind` from `ori_ir`.
///
/// Single source of truth shared by both the import resolver and the
/// type checker, eliminating the lossy mapping that previously collapsed
/// `EmptyModulePath | ModuleAliasWithItems` into `Other`.
pub use ori_ir::ImportErrorKind;

/// An error encountered during import resolution.
#[derive(Debug, Clone)]
pub struct ImportError {
    /// Structured error kind for programmatic matching.
    pub kind: ImportErrorKind,
    /// Human-readable error message with context.
    pub message: String,
    /// Source span where the error occurred.
    /// `None` for errors without a specific location (e.g., module not found).
    pub span: Option<ori_ir::Span>,
}

impl ImportError {
    /// Create an error without a source span.
    #[cold]
    pub fn new(kind: ImportErrorKind, message: impl Into<String>) -> Self {
        ImportError {
            kind,
            message: message.into(),
            span: None,
        }
    }

    /// Create an error with a source span.
    #[cold]
    pub fn with_span(
        kind: ImportErrorKind,
        message: impl Into<String>,
        span: ori_ir::Span,
    ) -> Self {
        ImportError {
            kind,
            message: message.into(),
            span: Some(span),
        }
    }
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ImportError {}

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
) -> Arc<ResolvedImports> {
    // Check session-scoped cache first — avoids re-resolving imports when
    // multiple consumers (type checker, evaluator, LLVM backend) need the
    // same file's imports within a single compilation session.
    if let Some(cached) = db.imports_cache().get(file_path) {
        return cached;
    }

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
                module_path: PathBuf::from("std/prelude"),
                source_file: Some(prelude_file),
                import_index: 0, // Not used for prelude (stored separately)
            });
        }
    }

    // 2. Resolve explicit imports
    for (imp_idx, imp) in parse_result.module.imports.iter().enumerate() {
        let resolved = match resolve_import(db, &imp.path, file_path) {
            Ok(resolved) => resolved,
            Err(e) => {
                errors.push(ImportError {
                    kind: e.kind,
                    message: e.message,
                    span: Some(e.span.unwrap_or(imp.span)),
                });
                continue;
            }
        };

        let source_file = resolved.file;
        let imported_parsed = parsed(db, source_file);

        let module_index = modules.len();
        modules.push(ResolvedImportedModule {
            parse_output: imported_parsed,
            module_path: resolved.path.clone(),
            source_file: Some(source_file),
            import_index: imp_idx,
        });

        // Handle module alias imports (use std.http as http)
        if let Some(alias) = imp.module_alias {
            imported_functions.push(ImportedFunctionRef {
                local_name: alias,
                original_name: alias,
                module_index,
                is_module_alias: true,
                span: imp.span,
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
                span: imp.span,
            });
        }
    }

    let result = Arc::new(ResolvedImports {
        prelude,
        modules,
        imported_functions,
        errors,
    });

    // Cache for subsequent calls (evaluator, LLVM backend, etc.)
    db.imports_cache()
        .store(file_path.to_path_buf(), result.clone());

    result
}
