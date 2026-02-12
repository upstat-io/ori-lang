//! Import resolution and module loading.
//!
//! Handles resolving import paths to file paths and loading imported modules.
//!
//! ## Import Types
//!
//! - Relative: `use './math' { add }` - resolves from current file
//! - Parent: `use '../utils' { helper }` - resolves from parent directory
//! - Module: `use std.math { sqrt }` - resolves from stdlib/packages
//!
//! ## Visibility
//!
//! - Public items (`pub @func`) can be imported normally
//! - Private items require `::` prefix: `use './mod' { ::private_func }`
//!
//! ## Test Modules
//!
//! Files in `_test/` directories with `.test.ori` extension can access private
//! items from their parent module without the `::` prefix.
//!
//! ## Salsa Integration
//!
//! All import resolution goes through [`resolve_import`], which provides proper
//! Salsa dependency tracking:
//! - All file access goes through `db.load_file()`, creating Salsa inputs
//! - File content changes are tracked and invalidate dependent queries

use crate::db::Db;
use crate::eval::{Environment, FunctionValue, Mutability, Value};
use crate::input::SourceFile;
use crate::ir::{ImportPath, Name, SharedArena, StringInterner};
use crate::parser::ParseOutput;
use ori_ir::canon::SharedCanonResult;
use rustc_hash::FxHashMap;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Re-export `ImportError` and `ImportErrorKind` from the canonical definition in `imports.rs`.
pub use crate::imports::{ImportError, ImportErrorKind};

/// Extract params and capabilities from a function definition.
///
/// This is a common pattern when building `FunctionValue` from AST.
fn extract_function_metadata(
    func: &crate::ir::Function,
    arena: &SharedArena,
) -> (Vec<Name>, Vec<Name>) {
    let params = arena.get_param_names(func.params);
    let capabilities = func.capabilities.iter().map(|c| c.name).collect();
    (params, capabilities)
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
pub fn is_test_module(path: &Path) -> bool {
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
pub fn is_parent_module_import(current_file: &Path, import_path: &Path) -> bool {
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

/// Result of resolving an import through the Salsa database.
///
/// Contains both the loaded source file (a Salsa input) and the resolved path.
#[derive(Debug)]
pub struct ResolvedImport {
    /// The loaded source file as a Salsa input.
    pub file: SourceFile,
    /// The resolved file path (for error messages and cycle detection).
    pub path: PathBuf,
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
/// Unlike [`resolve_import_path`], this function creates proper Salsa inputs:
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
pub fn resolve_import(
    db: &dyn Db,
    import_path: &ImportPath,
    current_file: &Path,
) -> Result<ResolvedImport, ImportError> {
    let interner = db.interner();

    match import_path {
        ImportPath::Relative(name) => {
            resolve_relative_import_tracked(db, *name, current_file, interner)
        }
        ImportPath::Module(segments) => resolve_module_import_tracked(db, segments, current_file),
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
    let path_str = interner.lookup(name);

    for path in &candidates {
        if let Some(file) = db.load_file(path) {
            return Ok(ResolvedImport {
                file,
                path: path.clone(),
            });
        }
    }

    // Format searched paths for error message
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

    // 1. Try file-based module: ./http.ori
    if resolved.extension().is_none() {
        candidates.push(resolved.with_extension("ori"));
    } else {
        candidates.push(resolved.clone());
    }

    // 2. Try directory module: ./http/mod.ori
    // Only if no extension was provided (don't try ./http.ori/mod.ori)
    if resolved.extension().is_none() {
        candidates.push(resolved.join("mod.ori"));
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
) -> Result<ResolvedImport, ImportError> {
    if segments.is_empty() {
        return Err(ImportError::new(
            ImportErrorKind::EmptyModulePath,
            "empty module path",
        ));
    }

    let interner = db.interner();
    let components: Vec<&str> = segments.iter().map(|s| interner.lookup(*s)).collect();
    let module_name = components.join(".");

    // Generate candidate paths and try each via db.load_file()
    for path in generate_module_candidates(&components, current_file) {
        if let Some(file) = db.load_file(&path) {
            return Ok(ResolvedImport { file, path });
        }
    }

    Err(ImportError::new(
        ImportErrorKind::ModuleNotFound,
        format!("module '{module_name}' not found. Searched: ORI_STDLIB, ./library/, standard locations"),
    ))
}

/// Generate candidate file paths for a module import.
///
/// Returns paths to try in priority order:
/// 1. `$ORI_STDLIB/<module>.ori` (if env var set)
/// 2. `<ancestor>/library/<module>.ori` (walking up from current file)
/// 3. `<ancestor>/library/<module>/mod.ori` (directory module pattern)
/// 4. Standard system locations
fn generate_module_candidates(components: &[&str], current_file: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    // 1. Try ORI_STDLIB environment variable
    if let Ok(stdlib_path) = std::env::var("ORI_STDLIB") {
        candidates.push(build_module_path(PathBuf::from(stdlib_path), components));
    }

    // 2. Walk up directory tree looking for library/ directories
    let mut dir = current_file.parent();
    while let Some(d) = dir {
        let library_dir = d.join("library");

        // Try library/std/math.ori pattern
        candidates.push(build_module_path(library_dir.clone(), components));

        // Try library/std/math/mod.ori pattern (directory modules)
        let mut mod_path = library_dir;
        for component in components {
            mod_path.push(component);
        }
        mod_path.push("mod.ori");
        candidates.push(mod_path);

        dir = d.parent();
    }

    // 3. Try standard system locations
    for base in ["/usr/local/lib/ori/stdlib", "/usr/lib/ori/stdlib"] {
        candidates.push(build_module_path(PathBuf::from(base), components));
    }

    candidates
}

/// Represents a parsed and loaded module ready for import registration.
///
/// Groups together the parse result, arena, and pre-built function map
/// to reduce parameter count in `register_imports`.
///
/// Uses `BTreeMap` for deterministic iteration order, which is important
/// for reproducible builds and Salsa query compatibility.
pub struct ImportedModule<'a> {
    /// The parse result containing the module's AST.
    pub result: &'a ParseOutput,
    /// The expression arena for the imported module.
    pub arena: &'a SharedArena,
    /// Pre-built map of all functions in the module.
    /// Uses `BTreeMap` for deterministic iteration order.
    pub functions: BTreeMap<Name, Value>,
}

impl<'a> ImportedModule<'a> {
    /// Create a new imported module from parse result and arena.
    ///
    /// Builds the function map automatically. When `canon` is provided,
    /// each function's `FunctionValue` is enriched with canonical IR data,
    /// enabling the evaluator to dispatch on `CanExpr` instead of `ExprKind`.
    pub fn new(
        result: &'a ParseOutput,
        arena: &'a SharedArena,
        canon: Option<&SharedCanonResult>,
    ) -> Self {
        let functions = Self::build_functions(result, arena, canon);
        ImportedModule {
            result,
            arena,
            functions,
        }
    }

    /// Build a map of all functions in a module.
    ///
    /// This allows imported functions to call other functions from their module.
    /// Uses `BTreeMap` for deterministic iteration order.
    ///
    /// When `canon` is provided, attaches canonical IR to each function via
    /// `set_canon()`, mirroring `register_module_functions` for local functions.
    fn build_functions(
        parse_result: &ParseOutput,
        imported_arena: &SharedArena,
        canon: Option<&SharedCanonResult>,
    ) -> BTreeMap<Name, Value> {
        let mut module_functions: BTreeMap<Name, Value> = BTreeMap::new();

        for func in &parse_result.module.functions {
            let (params, capabilities) = extract_function_metadata(func, imported_arena);
            let mut func_value = FunctionValue::with_capabilities(
                params,
                FxHashMap::default(),
                imported_arena.clone(),
                capabilities,
            );

            // Attach canonical IR when available
            if let Some(cr) = canon {
                if let Some(root) = cr.root_for(func.name) {
                    func_value.set_canon(root, cr.clone());
                }
            }

            module_functions.insert(func.name, Value::Function(func_value));
        }

        module_functions
    }
}

/// Build a map of all functions in a module.
///
/// This allows imported functions to call other functions from their module.
/// Uses `BTreeMap` for deterministic iteration order.
///
/// When `canon` is provided, attaches canonical IR to each function.
pub fn build_module_functions(
    parse_result: &ParseOutput,
    imported_arena: &SharedArena,
    canon: Option<&SharedCanonResult>,
) -> BTreeMap<Name, Value> {
    ImportedModule::build_functions(parse_result, imported_arena, canon)
}

/// Register imported items into the environment.
///
/// Looks up the requested items in the imported module and registers them
/// in the current environment with proper captures.
///
/// Visibility rules:
/// - Public items (`pub @func`) can be imported normally
/// - Private items (no `pub`) require `::` prefix: `use './mod' { ::private_func }`
/// - Test modules in `_test/` can access private items from parent module
///
/// Module alias imports:
/// - `use path as alias` imports the entire module as a namespace
/// - Only public items are included in the namespace
/// - Access via qualified syntax: `alias.function()`
pub fn register_imports(
    import: &crate::ir::UseDef,
    imported: &ImportedModule<'_>,
    env: &mut Environment,
    interner: &StringInterner,
    import_path: &Path,
    current_file: &Path,
    canon: Option<&SharedCanonResult>,
) -> Result<(), ImportError> {
    // Handle module alias: `use path as alias`
    if let Some(alias) = import.module_alias {
        return register_module_alias(import, imported, env, alias, import_path, canon);
    }

    // Check if this is a test module importing from its parent module
    let allow_private_access =
        is_test_module(current_file) && is_parent_module_import(current_file, import_path);

    // Build FxHashMap for O(1) function lookup instead of O(n) linear scan
    let func_by_name: FxHashMap<&str, &crate::ir::Function> = imported
        .result
        .module
        .functions
        .iter()
        .map(|f| (interner.lookup(f.name), f))
        .collect();

    // Build enriched captures once: current environment + all module functions.
    // Previously this was done per-item inside the loop, cloning the entire
    // environment N times for N imports. Now we build it once and share via Arc.
    let shared_captures: Arc<FxHashMap<Name, Value>> = {
        let mut captures = env.capture();
        for (name, value) in &imported.functions {
            captures.insert(*name, value.clone());
        }
        Arc::new(captures)
    };

    for item in &import.items {
        let item_name_str = interner.lookup(item.name);

        // Find the function in the imported module (O(1) lookup)
        if let Some(&func) = func_by_name.get(item_name_str) {
            // Check visibility: private items require :: prefix unless test module
            if !func.visibility.is_public() && !item.is_private && !allow_private_access {
                return Err(ImportError::new(
                    ImportErrorKind::PrivateAccess,
                    format!(
                        "'{}' is private in '{}'. Use '::{}' to import private items.",
                        item_name_str,
                        import_path.display(),
                        item_name_str
                    ),
                ));
            }

            let (params, capabilities) = extract_function_metadata(func, imported.arena);

            let mut func_value = FunctionValue::with_shared_captures(
                params,
                Arc::clone(&shared_captures),
                imported.arena.clone(),
                capabilities,
            );

            // Attach canonical IR when available
            if let Some(cr) = canon {
                if let Some(can_id) = cr.root_for(func.name) {
                    func_value.set_canon(can_id, cr.clone());
                }
            }

            // Use alias if provided, otherwise use original name
            let bind_name = item.alias.unwrap_or(item.name);
            env.define(
                bind_name,
                Value::Function(func_value),
                Mutability::Immutable,
            );
        } else {
            return Err(ImportError::new(
                ImportErrorKind::ItemNotFound,
                format!(
                    "'{}' not found in '{}'",
                    item_name_str,
                    import_path.display()
                ),
            ));
        }
    }

    Ok(())
}

/// Register a module alias import.
///
/// Creates a `ModuleNamespace` containing all public functions from the module
/// and binds it to the alias name.
fn register_module_alias(
    import: &crate::ir::UseDef,
    imported: &ImportedModule<'_>,
    env: &mut Environment,
    alias: Name,
    import_path: &Path,
    canon: Option<&SharedCanonResult>,
) -> Result<(), ImportError> {
    // Module alias imports should not have individual items
    if !import.items.is_empty() {
        return Err(ImportError::new(
            ImportErrorKind::ModuleAliasWithItems,
            format!(
                "module alias import cannot have individual items: '{}'",
                import_path.display()
            ),
        ));
    }

    // Collect all public functions into the namespace
    // Uses BTreeMap for deterministic iteration order
    let mut namespace: BTreeMap<Name, Value> = BTreeMap::new();

    // Clone captures once and wrap in Arc for sharing across all functions
    // Convert BTreeMap to FxHashMap (FunctionValue expects FxHashMap for captures)
    let shared_captures: Arc<FxHashMap<Name, Value>> = Arc::new(
        imported
            .functions
            .iter()
            .map(|(&k, v)| (k, v.clone()))
            .collect(),
    );

    for func in &imported.result.module.functions {
        if func.visibility.is_public() {
            let (params, capabilities) = extract_function_metadata(func, imported.arena);
            let mut func_value = FunctionValue::with_shared_captures(
                params,
                Arc::clone(&shared_captures),
                imported.arena.clone(),
                capabilities,
            );

            // Attach canonical IR when available
            if let Some(cr) = canon {
                if let Some(can_id) = cr.root_for(func.name) {
                    func_value.set_canon(can_id, cr.clone());
                }
            }

            namespace.insert(func.name, Value::Function(func_value));
        }
    }

    // Bind the namespace to the alias
    // (BTreeMap used for deterministic iteration order in Salsa queries)
    env.define(
        alias,
        Value::module_namespace(namespace),
        Mutability::Immutable,
    );

    Ok(())
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;
    use std::collections::HashSet;

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
    use crate::db::CompilerDb;
    use crate::ir::SharedInterner;
    use std::path::PathBuf;

    #[test]
    fn test_generate_relative_candidates_file_module() {
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
    fn test_generate_relative_candidates_parent_path() {
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
    fn test_generate_relative_candidates_with_extension() {
        let interner = SharedInterner::default();
        let name = interner.intern("./helper.ori");
        let current = PathBuf::from("/project/src/main.ori");

        let candidates = generate_relative_candidates(name, &current, &interner);

        // Should only try the exact path when extension is provided
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0], PathBuf::from("/project/src/helper.ori"));
    }

    #[test]
    fn test_generate_relative_candidates_nested_directory() {
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
    fn test_resolve_module_path_not_found() {
        let db = CompilerDb::new();
        let interner = db.interner();
        let std = interner.intern("std");
        let math = interner.intern("math");
        let path = ImportPath::Module(vec![std, math]);
        let current = PathBuf::from("/nonexistent/project/src/main.ori");

        let result = resolve_import(&db, &path, &current);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("not found"));
    }

    #[test]
    fn test_import_error_display() {
        let err = ImportError::new(ImportErrorKind::ModuleNotFound, "test error");
        assert_eq!(format!("{err}"), "test error");
    }

    #[test]
    fn test_is_test_module_valid() {
        // Valid test module: in _test/ with .test.ori extension
        let path = PathBuf::from("/project/src/_test/math.test.ori");
        assert!(is_test_module(&path));
    }

    #[test]
    fn test_is_test_module_not_in_test_dir() {
        // Not in _test/ directory
        let path = PathBuf::from("/project/src/math.test.ori");
        assert!(!is_test_module(&path));
    }

    #[test]
    fn test_is_test_module_wrong_extension() {
        // In _test/ but wrong extension
        let path = PathBuf::from("/project/src/_test/math.ori");
        assert!(!is_test_module(&path));
    }

    #[test]
    fn test_is_test_module_nested() {
        // Nested _test/ directory
        let path = PathBuf::from("/project/src/utils/_test/helpers.test.ori");
        assert!(is_test_module(&path));
    }

    #[test]
    fn test_is_parent_module_import_valid() {
        // Test module importing from parent directory
        let current = PathBuf::from("/project/src/_test/math.test.ori");
        let import = PathBuf::from("/project/src/math.ori");
        assert!(is_parent_module_import(&current, &import));
    }

    #[test]
    fn test_is_parent_module_import_sibling() {
        // Importing from sibling, not parent
        let current = PathBuf::from("/project/src/_test/math.test.ori");
        let import = PathBuf::from("/project/src/_test/utils.ori");
        assert!(!is_parent_module_import(&current, &import));
    }

    #[test]
    fn test_is_parent_module_import_not_test() {
        // Not in _test directory
        let current = PathBuf::from("/project/src/main.ori");
        let import = PathBuf::from("/project/src/math.ori");
        assert!(!is_parent_module_import(&current, &import));
    }

    #[test]
    fn test_loading_context_cycle_detection() {
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
    fn test_loading_context_cycle_error() {
        let mut ctx = LoadingContext::new();
        let path = PathBuf::from("/a.ori");

        ctx.start_loading(path.clone()).unwrap();
        let result = ctx.start_loading(path.clone());
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("circular import"));
    }
}
