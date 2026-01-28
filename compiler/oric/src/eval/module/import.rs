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
use crate::eval::{Environment, FunctionValue, Value};
use crate::input::SourceFile;
use crate::ir::{ImportPath, Name, SharedArena, StringInterner};
use crate::parser::ParseResult;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Error during import resolution.
#[derive(Debug, Clone)]
pub struct ImportError {
    pub message: String,
}

impl ImportError {
    pub fn new(message: impl Into<String>) -> Self {
        ImportError {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ImportError {}

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
pub fn resolve_import(
    db: &dyn Db,
    import_path: &ImportPath,
    current_file: &Path,
) -> Result<ResolvedImport, ImportError> {
    let interner = db.interner();

    match import_path {
        ImportPath::Relative(name) => {
            let path = resolve_relative_path_to_pathbuf(*name, current_file, interner);
            match db.load_file(&path) {
                Some(file) => Ok(ResolvedImport { file, path }),
                None => Err(ImportError::new(format!(
                    "cannot find import '{}' at '{}'",
                    interner.lookup(*name),
                    path.display()
                ))),
            }
        }
        ImportPath::Module(segments) => resolve_module_import_tracked(db, segments, current_file),
    }
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
        return Err(ImportError::new("empty module path"));
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

    Err(ImportError::new(format!(
        "module '{module_name}' not found. Searched: ORI_STDLIB, ./library/, standard locations"
    )))
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
        let mut path = PathBuf::from(stdlib_path);
        for component in components {
            path.push(component);
        }
        candidates.push(path.with_extension("ori"));
    }

    // 2. Walk up directory tree looking for library/ directories
    let mut dir = current_file.parent();
    while let Some(d) = dir {
        let library_dir = d.join("library");

        // Try library/std/math.ori pattern
        let mut path = library_dir.clone();
        for component in components {
            path.push(component);
        }
        candidates.push(path.with_extension("ori"));

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
        let mut path = PathBuf::from(base);
        for component in components {
            path.push(component);
        }
        candidates.push(path.with_extension("ori"));
    }

    candidates
}

/// Resolve a relative import path to a `PathBuf`.
///
/// Helper function for path computation without file access.
fn resolve_relative_path_to_pathbuf(
    name: Name,
    current_file: &Path,
    interner: &StringInterner,
) -> PathBuf {
    let path_str = interner.lookup(name);
    let current_dir = current_file.parent().unwrap_or(Path::new("."));
    let resolved = current_dir.join(path_str);

    if resolved.extension().is_none() {
        resolved.with_extension("ori")
    } else {
        resolved
    }
}

/// Context for loading modules with cycle detection.
///
/// Tracks which modules are currently being loaded to detect circular imports.
#[derive(Debug, Default)]
pub struct LoadingContext {
    /// Stack of modules currently being loaded (for cycle detection)
    loading_stack: Vec<PathBuf>,
    /// Cache of already loaded modules
    loaded: HashSet<PathBuf>,
}

impl LoadingContext {
    /// Create a new loading context.
    pub fn new() -> Self {
        LoadingContext {
            loading_stack: Vec::new(),
            loaded: HashSet::new(),
        }
    }

    /// Check if loading this path would create a cycle.
    pub fn would_cycle(&self, path: &Path) -> bool {
        self.loading_stack.iter().any(|p| p == path)
    }

    /// Check if this path has already been loaded.
    pub fn is_loaded(&self, path: &Path) -> bool {
        self.loaded.contains(path)
    }

    /// Start loading a module. Returns error if this would create a cycle.
    pub fn start_loading(&mut self, path: PathBuf) -> Result<(), ImportError> {
        if self.would_cycle(&path) {
            let cycle: Vec<String> = self
                .loading_stack
                .iter()
                .chain(std::iter::once(&path))
                .map(|p| p.display().to_string())
                .collect();
            return Err(ImportError::new(format!(
                "circular import detected: {}",
                cycle.join(" -> ")
            )));
        }
        self.loading_stack.push(path);
        Ok(())
    }

    /// Finish loading a module.
    pub fn finish_loading(&mut self, path: PathBuf) {
        self.loading_stack.pop();
        self.loaded.insert(path);
    }
}

/// Represents a parsed and loaded module ready for import registration.
///
/// Groups together the parse result, arena, and pre-built function map
/// to reduce parameter count in `register_imports`.
pub struct ImportedModule<'a> {
    /// The parse result containing the module's AST.
    pub result: &'a ParseResult,
    /// The expression arena for the imported module.
    pub arena: &'a SharedArena,
    /// Pre-built map of all functions in the module.
    pub functions: HashMap<Name, Value>,
}

impl<'a> ImportedModule<'a> {
    /// Create a new imported module from parse result and arena.
    ///
    /// Builds the function map automatically.
    pub fn new(result: &'a ParseResult, arena: &'a SharedArena) -> Self {
        let functions = Self::build_functions(result, arena);
        ImportedModule {
            result,
            arena,
            functions,
        }
    }

    /// Build a map of all functions in a module.
    ///
    /// This allows imported functions to call other functions from their module.
    fn build_functions(
        parse_result: &ParseResult,
        imported_arena: &SharedArena,
    ) -> HashMap<Name, Value> {
        let mut module_functions: HashMap<Name, Value> = HashMap::new();

        for func in &parse_result.module.functions {
            let params = imported_arena.get_param_names(func.params);
            let capabilities: Vec<_> = func.capabilities.iter().map(|c| c.name).collect();

            let func_value = FunctionValue::with_capabilities(
                params,
                func.body,
                HashMap::new(),
                imported_arena.clone(),
                capabilities,
            );
            module_functions.insert(func.name, Value::Function(func_value));
        }

        module_functions
    }
}

/// Build a map of all functions in a module.
///
/// This allows imported functions to call other functions from their module.
pub fn build_module_functions(
    parse_result: &ParseResult,
    imported_arena: &SharedArena,
) -> HashMap<Name, Value> {
    ImportedModule::build_functions(parse_result, imported_arena)
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
pub fn register_imports(
    import: &crate::ir::UseDef,
    imported: &ImportedModule<'_>,
    env: &mut Environment,
    interner: &StringInterner,
    import_path: &Path,
    current_file: &Path,
) -> Result<(), ImportError> {
    // Check if this is a test module importing from its parent module
    let allow_private_access =
        is_test_module(current_file) && is_parent_module_import(current_file, import_path);

    for item in &import.items {
        let item_name_str = interner.lookup(item.name);

        // Find the function in the imported module
        let func = imported
            .result
            .module
            .functions
            .iter()
            .find(|f| interner.lookup(f.name) == item_name_str);

        if let Some(func) = func {
            // Check visibility: private items require :: prefix unless test module
            if !func.is_public && !item.is_private && !allow_private_access {
                return Err(ImportError::new(format!(
                    "'{}' is private in '{}'. Use '::{}' to import private items.",
                    item_name_str,
                    import_path.display(),
                    item_name_str
                )));
            }

            let params = imported.arena.get_param_names(func.params);
            let capabilities: Vec<_> = func.capabilities.iter().map(|c| c.name).collect();

            // Captures include: current environment + all module functions
            // Iterate instead of cloning the entire HashMap to avoid intermediate allocation
            let mut captures = env.capture();
            for (name, value) in &imported.functions {
                captures.insert(*name, value.clone());
            }

            let func_value = FunctionValue::with_capabilities(
                params,
                func.body,
                captures,
                imported.arena.clone(),
                capabilities,
            );

            // Use alias if provided, otherwise use original name
            let bind_name = item.alias.unwrap_or(item.name);
            env.define(bind_name, Value::Function(func_value), false);
        } else {
            return Err(ImportError::new(format!(
                "'{}' not found in '{}'",
                item_name_str,
                import_path.display()
            )));
        }
    }

    Ok(())
}

/// Register all functions from a module into the environment.
///
/// IMPORTANT: All functions carry a `SharedArena` reference to ensure correct
/// evaluation when called from different contexts (e.g., from within a prelude
/// function or other imported code).
pub fn register_module_functions(parse_result: &ParseResult, env: &mut Environment) {
    // Create a shared arena for all functions in this module
    let shared_arena = SharedArena::new(parse_result.arena.clone());

    for func in &parse_result.module.functions {
        let params: Vec<_> = parse_result
            .arena
            .get_params(func.params)
            .iter()
            .map(|p| p.name)
            .collect();
        let capabilities: Vec<_> = func.capabilities.iter().map(|c| c.name).collect();
        let captures = env.capture();
        // Use from_import_with_capabilities to include the arena reference
        let func_value = FunctionValue::with_capabilities(
            params,
            func.body,
            captures,
            shared_arena.clone(),
            capabilities,
        );
        env.define(func.name, Value::Function(func_value), false);
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;
    use crate::db::CompilerDb;
    use crate::ir::SharedInterner;
    use std::path::PathBuf;

    #[test]
    fn test_resolve_relative_path_computation() {
        let interner = SharedInterner::default();
        let name = interner.intern("./math");
        let current = PathBuf::from("/project/src/main.ori");

        let result = resolve_relative_path_to_pathbuf(name, &current, &interner);
        assert_eq!(result, PathBuf::from("/project/src/math.ori"));
    }

    #[test]
    fn test_resolve_parent_path_computation() {
        let interner = SharedInterner::default();
        let name = interner.intern("../utils");
        let current = PathBuf::from("/project/src/main.ori");

        let result = resolve_relative_path_to_pathbuf(name, &current, &interner);
        assert_eq!(result, PathBuf::from("/project/src/../utils.ori"));
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
        let err = ImportError::new("test error");
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
