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
//!
//! Low-level path resolution (`resolve_import`, `is_test_module`, etc.) also
//! lives here — these are pure path-resolution utilities with no eval
//! dependencies, consumed by both `resolve_imports()` and the eval-side
//! `register_imports()`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::db::Db;
use crate::input::SourceFile;
use crate::ir::{ImportPath, Name, Span, StringInterner};
use crate::parser::ParseOutput;
use crate::query::parsed;
use crate::typeck::{is_prelude_file, prelude_candidates};

// Boundary types consumed by all backends

/// A resolved imported module: the parsed output and its source path.
pub(crate) struct ResolvedImportedModule {
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
    pub source_file: Option<SourceFile>,
    /// Index into the original `parse_result.module.imports` array.
    /// Enables consumers to map back to the source `UseDef` for
    /// visibility checking, alias handling, etc.
    pub import_index: usize,
}

/// Reference to a specific imported function within a resolved module.
pub(crate) struct ImportedFunctionRef {
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
    pub span: Span,
}

/// All resolved imports for a single file.
///
/// Produced by `resolve_imports()` and consumed by all backends.
pub(crate) struct ResolvedImports {
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
pub(crate) use ori_ir::ImportErrorKind;

/// An error encountered during import resolution.
#[derive(Debug, Clone)]
pub(crate) struct ImportError {
    /// Structured error kind for programmatic matching.
    pub kind: ImportErrorKind,
    /// Human-readable error message with context.
    pub message: String,
    /// Source span where the error occurred.
    /// `None` for errors without a specific location (e.g., module not found).
    pub span: Option<Span>,
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
    pub fn with_span(kind: ImportErrorKind, message: impl Into<String>, span: Span) -> Self {
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

// Low-level path resolution

/// Result of resolving an import through the Salsa database.
///
/// Contains both the loaded source file (a Salsa input) and the resolved path.
#[derive(Debug)]
struct ResolvedImport {
    /// The loaded source file as a Salsa input.
    pub file: SourceFile,
    /// The resolved file path (for error messages and cycle detection).
    pub path: PathBuf,
}

/// Build a module path from base directory and components, adding .ori extension.
fn build_module_path(base: PathBuf, components: &[&str]) -> PathBuf {
    let mut path = base;
    for component in components {
        path.push(component);
    }
    path.with_extension("ori")
}

/// Check if a file is a test module.
///
/// A test module is defined as:
/// 1. Being in a `_test/` directory, AND
/// 2. Having a `.test.ori` extension
///
/// Test modules can access private items from their parent module without
/// using the `::` prefix.
pub(crate) fn is_test_module(path: &Path) -> bool {
    // Check if the file has .test.ori extension
    let has_test_extension = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".test.ori"));

    if !has_test_extension {
        return false;
    }

    // Check if any parent directory is named _test
    path.parent().is_some_and(|parent| {
        parent
            .components()
            .any(|c| c.as_os_str().to_str() == Some("_test"))
    })
}

/// Check if the imported path is from the test module's parent module.
///
/// This is used to determine if a test module should have private access
/// to the imported module. A test module `src/_test/math.test.ori` should
/// have private access to `src/math.ori` (accessed via `../math`).
pub(crate) fn is_parent_module_import(current_file: &Path, import_path: &Path) -> bool {
    let current_dir = current_file.parent().unwrap_or(Path::new("."));

    // Check if current dir is named _test
    let is_in_test_dir = current_dir.file_name().and_then(|n| n.to_str()) == Some("_test");

    if !is_in_test_dir {
        return false;
    }

    // Get the parent directory of _test (e.g., src/_test -> src)
    let Some(test_parent) = current_dir.parent() else {
        return false;
    };

    // Get the directory containing the imported file
    let import_parent = import_path.parent().unwrap_or(Path::new("."));

    // Normalize both paths by removing .. components for comparison
    // For a path like "tests/spec/modules/_test/../use_imports.ori",
    // the parent is "tests/spec/modules/_test/.." which should equal "tests/spec/modules"
    let normalized_import_parent = normalize_path(import_parent);
    let normalized_test_parent = normalize_path(test_parent);

    normalized_import_parent == normalized_test_parent
}

/// Normalize a path by resolving . and .. components.
fn normalize_path(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                result.pop();
            }
            std::path::Component::CurDir => {
                // Skip current dir
            }
            _ => {
                result.push(component);
            }
        }
    }
    result
}

/// Resolve and load an import using the Salsa database.
///
/// This is the primary import resolution function. All file access goes through
/// `db.load_file()`, ensuring proper Salsa dependency tracking. When a file is
/// loaded, it becomes a Salsa input and content changes are tracked.
///
/// # Arguments
///
/// - `db`: The Salsa database for tracked file loading
/// - `import_path`: The import path from the AST
/// - `current_file`: Path to the file containing the import statement
///
/// # Returns
///
/// - `Ok(ResolvedImport)` with the loaded source file
/// - `Err(ImportError)` if the import cannot be resolved
///
/// # Salsa Tracking
///
/// This function creates proper Salsa inputs:
/// - Successful loads create tracked `SourceFile` inputs
/// - Content changes to imported files invalidate dependent queries
/// - File creation/deletion is detected on next query execution
///
/// # Directory Modules
///
/// For relative imports like `use "./http"`, the resolver tries:
/// 1. `./http.ori` (file-based module)
/// 2. `./http/mod.ori` (directory-based module)
///
/// The first successful load wins.
fn resolve_import(
    db: &dyn Db,
    import_path: &ImportPath,
    current_file: &Path,
    stdlib_override: Option<&str>,
) -> Result<ResolvedImport, ImportError> {
    let interner = db.interner();

    match import_path {
        ImportPath::Relative(name) => {
            resolve_relative_import_tracked(db, *name, current_file, interner)
        }
        ImportPath::Module(segments) => {
            resolve_module_import_tracked(db, segments, current_file, stdlib_override)
        }
    }
}

/// Resolve a relative import using tracked file loading.
///
/// Generates candidate paths and probes each via `db.load_file()`.
/// Tries file-based module first (`./http.ori`), then directory module (`./http/mod.ori`).
fn resolve_relative_import_tracked(
    db: &dyn Db,
    name: Name,
    current_file: &Path,
    interner: &StringInterner,
) -> Result<ResolvedImport, ImportError> {
    let candidates = generate_relative_candidates(name, current_file, interner);

    // Probe candidates by reference first to find the matching file,
    // then move the path out to avoid cloning on the success path.
    let found = candidates
        .iter()
        .enumerate()
        .find_map(|(i, path)| db.load_file(path).map(|file| (i, file)));

    if let Some((idx, file)) = found {
        // Move the matched path out of candidates via swap_remove (O(1), avoids clone).
        let mut candidates = candidates;
        let path = candidates.swap_remove(idx);
        return Ok(ResolvedImport { file, path });
    }

    // Defer string allocations to error path — the success path above
    // never needs the path display strings.
    let path_str = interner.lookup(name);
    let searched = candidates
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");

    Err(ImportError::new(
        ImportErrorKind::ModuleNotFound,
        format!("cannot find import '{path_str}'. Searched: {searched}"),
    ))
}

/// Generate candidate file paths for a relative import.
///
/// Returns paths to try in priority order:
/// 1. `<dir>/<path>.ori` (file-based module)
/// 2. `<dir>/<path>/mod.ori` (directory-based module)
fn generate_relative_candidates(
    name: Name,
    current_file: &Path,
    interner: &StringInterner,
) -> Vec<PathBuf> {
    let path_str = interner.lookup(name);
    let current_dir = current_file.parent().unwrap_or(Path::new("."));
    let resolved = current_dir.join(path_str);

    let mut candidates = Vec::with_capacity(2);

    if resolved.extension().is_none() {
        // No extension: try file module (./http.ori) then directory module (./http/mod.ori)
        candidates.push(resolved.with_extension("ori"));
        candidates.push(resolved.join("mod.ori"));
    } else {
        // Has extension: use exact path, no directory module variant
        candidates.push(resolved);
    }

    candidates
}

/// Resolve a module import using tracked file loading.
///
/// Generates candidate paths and probes each via `db.load_file()`.
/// The first successful load wins. All file access is tracked by Salsa.
fn resolve_module_import_tracked(
    db: &dyn Db,
    segments: &[Name],
    current_file: &Path,
    stdlib_override: Option<&str>,
) -> Result<ResolvedImport, ImportError> {
    if segments.is_empty() {
        return Err(ImportError::new(
            ImportErrorKind::EmptyModulePath,
            "empty module path",
        ));
    }

    let interner = db.interner();
    let components: Vec<&str> = segments.iter().map(|s| interner.lookup(*s)).collect();

    // Generate candidate paths and try each via db.load_file()
    for path in generate_module_candidates(&components, current_file, stdlib_override) {
        if let Some(file) = db.load_file(&path) {
            return Ok(ResolvedImport { file, path });
        }
    }

    // Defer module_name allocation to error path — the success path above
    // never needs the joined string.
    let module_name = components.join(".");
    Err(ImportError::new(
        ImportErrorKind::ModuleNotFound,
        format!("module '{module_name}' not found. Searched: ORI_STDLIB, ./library/, standard locations"),
    ))
}

/// Generate candidate file paths for a module import.
///
/// Returns paths to try in priority order:
/// 1. `$ORI_STDLIB/<module>.ori` (if override provided)
/// 2. `<ancestor>/library/<module>.ori` (walking up from current file)
/// 3. `<ancestor>/library/<module>/mod.ori` (directory module pattern)
/// 4. Standard system locations
///
/// This function is pure — all external state (env vars) is passed in
/// as parameters. IO is the caller's responsibility.
fn generate_module_candidates(
    components: &[&str],
    current_file: &Path,
    stdlib_override: Option<&str>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    // 1. Try ORI_STDLIB override (caller reads env var)
    if let Some(stdlib_path) = stdlib_override {
        candidates.push(build_module_path(PathBuf::from(stdlib_path), components));
    }

    // 2. Walk up directory tree looking for library/ directories
    let mut dir = current_file.parent();
    while let Some(d) = dir {
        // Build base path: library/comp1/comp2 (no extension yet).
        // Derive both candidates from this single allocation:
        //   file module: library/comp1/comp2.ori
        //   dir module:  library/comp1/comp2/mod.ori
        let mut base = d.join("library");
        for component in components {
            base.push(component);
        }
        candidates.push(base.with_extension("ori"));
        base.push("mod.ori");
        candidates.push(base);

        dir = d.parent();
    }

    // 3. Try standard system locations
    for base in ["/usr/local/lib/ori/stdlib", "/usr/lib/ori/stdlib"] {
        candidates.push(build_module_path(PathBuf::from(base), components));
    }

    candidates
}

// High-level entry point

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
pub(crate) fn resolve_imports(
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
    // Read ORI_STDLIB once for all module imports (avoids per-import syscall).
    let stdlib_override = std::env::var("ORI_STDLIB").ok();
    for (imp_idx, imp) in parse_result.module.imports.iter().enumerate() {
        let resolved = match resolve_import(db, &imp.path, file_path, stdlib_override.as_deref()) {
            Ok(resolved) => resolved,
            Err(mut e) => {
                // Ensure span is always present by falling back to the
                // use-statement span when the resolver didn't attach one.
                if e.span.is_none() {
                    e.span = Some(imp.span);
                }
                errors.push(e);
                continue;
            }
        };

        let source_file = resolved.file;
        let imported_parsed = parsed(db, source_file);

        let module_index = modules.len();
        modules.push(ResolvedImportedModule {
            parse_output: imported_parsed,
            module_path: resolved.path,
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
    db.imports_cache().store(file_path, result.clone());

    result
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;
    use std::collections::HashSet;

    use crate::db::CompilerDb;
    use crate::ir::SharedInterner;

    #[test]
    fn generate_relative_candidates_file_module() {
        let interner = SharedInterner::default();
        let name = interner.intern("./math");
        let current = PathBuf::from("/project/src/main.ori");

        let candidates = generate_relative_candidates(name, &current, &interner);

        // Should try file first, then directory module
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0], PathBuf::from("/project/src/math.ori"));
        assert_eq!(candidates[1], PathBuf::from("/project/src/math/mod.ori"));
    }

    #[test]
    fn generate_relative_candidates_parent_path() {
        let interner = SharedInterner::default();
        let name = interner.intern("../utils");
        let current = PathBuf::from("/project/src/main.ori");

        let candidates = generate_relative_candidates(name, &current, &interner);

        // Should try file first, then directory module
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0], PathBuf::from("/project/src/../utils.ori"));
        assert_eq!(
            candidates[1],
            PathBuf::from("/project/src/../utils/mod.ori")
        );
    }

    #[test]
    fn generate_relative_candidates_with_extension() {
        let interner = SharedInterner::default();
        let name = interner.intern("./helper.ori");
        let current = PathBuf::from("/project/src/main.ori");

        let candidates = generate_relative_candidates(name, &current, &interner);

        // Should only try the exact path when extension is provided
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0], PathBuf::from("/project/src/helper.ori"));
    }

    #[test]
    fn generate_relative_candidates_nested_directory() {
        let interner = SharedInterner::default();
        let name = interner.intern("./http/client");
        let current = PathBuf::from("/project/src/main.ori");

        let candidates = generate_relative_candidates(name, &current, &interner);

        // Should try file first, then directory module
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0], PathBuf::from("/project/src/http/client.ori"));
        assert_eq!(
            candidates[1],
            PathBuf::from("/project/src/http/client/mod.ori")
        );
    }

    #[test]
    fn resolve_module_path_not_found() {
        let db = CompilerDb::new();
        let interner = db.interner();
        let std = interner.intern("std");
        let math = interner.intern("math");
        let path = ImportPath::Module(vec![std, math]);
        let current = PathBuf::from("/nonexistent/project/src/main.ori");

        let result = resolve_import(&db, &path, &current, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("not found"));
    }

    #[test]
    fn import_error_display() {
        let err = ImportError::new(ImportErrorKind::ModuleNotFound, "test error");
        assert_eq!(format!("{err}"), "test error");
    }

    #[test]
    fn is_test_module_valid() {
        // Valid test module: in _test/ with .test.ori extension
        let path = PathBuf::from("/project/src/_test/math.test.ori");
        assert!(is_test_module(&path));
    }

    #[test]
    fn is_test_module_not_in_test_dir() {
        // Not in _test/ directory
        let path = PathBuf::from("/project/src/math.test.ori");
        assert!(!is_test_module(&path));
    }

    #[test]
    fn is_test_module_wrong_extension() {
        // In _test/ but wrong extension
        let path = PathBuf::from("/project/src/_test/math.ori");
        assert!(!is_test_module(&path));
    }

    #[test]
    fn is_test_module_nested() {
        // Nested _test/ directory
        let path = PathBuf::from("/project/src/utils/_test/helpers.test.ori");
        assert!(is_test_module(&path));
    }

    #[test]
    fn is_parent_module_import_valid() {
        // Test module importing from parent directory
        let current = PathBuf::from("/project/src/_test/math.test.ori");
        let import = PathBuf::from("/project/src/math.ori");
        assert!(is_parent_module_import(&current, &import));
    }

    #[test]
    fn is_parent_module_import_sibling() {
        // Importing from sibling, not parent
        let current = PathBuf::from("/project/src/_test/math.test.ori");
        let import = PathBuf::from("/project/src/_test/utils.ori");
        assert!(!is_parent_module_import(&current, &import));
    }

    #[test]
    fn is_parent_module_import_not_test() {
        // Not in _test directory
        let current = PathBuf::from("/project/src/main.ori");
        let import = PathBuf::from("/project/src/math.ori");
        assert!(!is_parent_module_import(&current, &import));
    }

    /// Test-only context for loading modules with cycle detection.
    ///
    /// Tracks which modules are currently being loaded to detect circular imports.
    /// In production, Salsa's query dependency tracking handles cycle detection.
    #[derive(Debug, Default)]
    struct LoadingContext {
        loading_stack: Vec<PathBuf>,
        loading_set: HashSet<PathBuf>,
        loaded: HashSet<PathBuf>,
    }

    impl LoadingContext {
        fn new() -> Self {
            LoadingContext {
                loading_stack: Vec::new(),
                loading_set: HashSet::new(),
                loaded: HashSet::new(),
            }
        }

        fn would_cycle(&self, path: &Path) -> bool {
            self.loading_set.contains(path)
        }

        fn is_loaded(&self, path: &Path) -> bool {
            self.loaded.contains(path)
        }

        fn start_loading(&mut self, path: PathBuf) -> Result<(), ImportError> {
            if self.would_cycle(&path) {
                let cycle: Vec<String> = self
                    .loading_stack
                    .iter()
                    .chain(std::iter::once(&path))
                    .map(|p| p.display().to_string())
                    .collect();
                return Err(ImportError::new(
                    ImportErrorKind::CircularImport,
                    format!("circular import detected: {}", cycle.join(" -> ")),
                ));
            }
            self.loading_set.insert(path.clone());
            self.loading_stack.push(path);
            Ok(())
        }

        fn finish_loading(&mut self, path: PathBuf) {
            if let Some(popped) = self.loading_stack.pop() {
                self.loading_set.remove(&popped);
            }
            self.loaded.insert(path);
        }
    }

    #[test]
    fn loading_context_cycle_detection() {
        let mut ctx = LoadingContext::new();
        let path1 = PathBuf::from("/a.ori");
        let path2 = PathBuf::from("/b.ori");

        assert!(!ctx.would_cycle(&path1));
        ctx.start_loading(path1.clone()).unwrap();
        assert!(ctx.would_cycle(&path1));
        assert!(!ctx.would_cycle(&path2));

        ctx.start_loading(path2.clone()).unwrap();
        assert!(ctx.would_cycle(&path2));

        ctx.finish_loading(path2.clone());
        assert!(!ctx.would_cycle(&path2)); // Not in stack anymore
        assert!(ctx.is_loaded(&path2)); // But marked as loaded
    }

    #[test]
    fn loading_context_cycle_error() {
        let mut ctx = LoadingContext::new();
        let path = PathBuf::from("/a.ori");

        ctx.start_loading(path.clone()).unwrap();
        let result = ctx.start_loading(path.clone());
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("circular import"));
    }
}
