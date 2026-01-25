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
//! Files in `_test/` directories with `.test.si` extension can access private
//! items from their parent module without the `::` prefix.

use std::path::{Path, PathBuf};
use std::collections::{HashMap, HashSet};
use crate::ir::{Name, StringInterner, ImportPath, SharedArena};
use crate::parser::ParseResult;
use crate::eval::{Value, FunctionValue, Environment};

/// Error during import resolution.
#[derive(Debug, Clone)]
pub struct ImportError {
    pub message: String,
}

impl ImportError {
    pub fn new(message: impl Into<String>) -> Self {
        ImportError { message: message.into() }
    }
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ImportError {}

// =============================================================================
// Test Module Detection
// =============================================================================

/// Check if a file is a test module.
///
/// A test module is defined as:
/// 1. Being in a `_test/` directory, AND
/// 2. Having a `.test.si` extension
///
/// Test modules can access private items from their parent module without
/// using the `::` prefix.
pub fn is_test_module(path: &Path) -> bool {
    // Check if the file has .test.si extension
    let has_test_extension = path.file_name()
        .and_then(|n| n.to_str())
        .map_or(false, |n| n.ends_with(".test.si"));

    if !has_test_extension {
        return false;
    }

    // Check if any parent directory is named _test
    path.parent()
        .map_or(false, |parent| {
            parent.components().any(|c| {
                c.as_os_str().to_str().map_or(false, |s| s == "_test")
            })
        })
}

/// Check if the imported path is from the test module's parent module.
///
/// This is used to determine if a test module should have private access
/// to the imported module. A test module `src/_test/math.test.si` should
/// have private access to `src/math.si` (accessed via `../math`).
pub fn is_parent_module_import(current_file: &Path, import_path: &Path) -> bool {
    let current_dir = current_file.parent().unwrap_or(Path::new("."));

    // Check if current dir is named _test
    let is_in_test_dir = current_dir.file_name()
        .and_then(|n| n.to_str())
        .map_or(false, |n| n == "_test");

    if !is_in_test_dir {
        return false;
    }

    // Get the parent directory of _test (e.g., src/_test -> src)
    let test_parent = match current_dir.parent() {
        Some(p) => p,
        None => return false,
    };

    // Get the directory containing the imported file
    let import_parent = import_path.parent().unwrap_or(Path::new("."));

    // Normalize both paths by removing .. components for comparison
    // For a path like "tests/spec/modules/_test/../use_imports.si",
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

// =============================================================================
// Module Loading Context
// =============================================================================

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
            let cycle: Vec<String> = self.loading_stack.iter()
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

    /// Get the current loading depth (for debugging).
    #[allow(dead_code)]
    pub fn depth(&self) -> usize {
        self.loading_stack.len()
    }
}

// =============================================================================
// Path Resolution
// =============================================================================

/// Resolve an import path to a file path.
///
/// Handles relative paths (starting with './' or '../') and module paths.
///
/// Module paths are resolved by looking in:
/// 1. SIGIL_STDLIB environment variable (if set)
/// 2. ./library/ relative to project root
/// 3. Standard locations (/usr/local/lib/sigil/stdlib, etc.)
pub fn resolve_import_path(
    import_path: &ImportPath,
    current_file: &Path,
    interner: &StringInterner,
) -> Result<PathBuf, ImportError> {
    match import_path {
        ImportPath::Relative(name) => {
            let path_str = interner.lookup(*name);
            let current_dir = current_file.parent().unwrap_or(Path::new("."));

            // Handle relative paths like '../a_plus_b' or './math'
            let resolved = current_dir.join(path_str);

            // Add .si extension if not present
            let with_ext = if resolved.extension().is_none() {
                resolved.with_extension("si")
            } else {
                resolved
            };

            Ok(with_ext)
        }
        ImportPath::Module(segments) => {
            resolve_module_path(segments, current_file, interner)
        }
    }
}

/// Resolve a module path like `std.math` to a file path.
///
/// Search order:
/// 1. SIGIL_STDLIB environment variable
/// 2. ./library/ relative to project root (for development)
/// 3. Standard locations
fn resolve_module_path(
    segments: &[Name],
    current_file: &Path,
    interner: &StringInterner,
) -> Result<PathBuf, ImportError> {
    if segments.is_empty() {
        return Err(ImportError::new("empty module path"));
    }

    // Convert segments to path components
    let components: Vec<&str> = segments.iter()
        .map(|s| interner.lookup(*s))
        .collect();

    let module_name = components.join(".");

    // Try SIGIL_STDLIB first
    if let Ok(stdlib_path) = std::env::var("SIGIL_STDLIB") {
        let mut path = PathBuf::from(stdlib_path);
        for component in &components {
            path.push(component);
        }
        path.set_extension("si");
        if path.exists() {
            return Ok(path);
        }
    }

    // Try ./library/ relative to project root
    // Walk up from current file to find project root (contains Sigil.toml or library/)
    let mut project_root = current_file.parent();
    while let Some(dir) = project_root {
        let library_dir = dir.join("library");
        if library_dir.is_dir() {
            // Try direct file path: library/std/math.si
            let mut path = library_dir.clone();
            for component in &components {
                path.push(component);
            }
            path.set_extension("si");
            if path.exists() {
                return Ok(path);
            }
            // Also try with mod.si for directory modules: library/std/math/mod.si
            let mut mod_path = library_dir;
            for component in &components {
                mod_path.push(component);
            }
            mod_path.push("mod.si");
            if mod_path.exists() {
                return Ok(mod_path);
            }
            break; // Found library dir, no need to keep searching
        }
        project_root = dir.parent();
    }

    // Try standard locations
    let standard_locations = [
        "/usr/local/lib/sigil/stdlib",
        "/usr/lib/sigil/stdlib",
    ];

    for base in standard_locations {
        let base_path = Path::new(base);
        if base_path.is_dir() {
            let mut path = base_path.to_path_buf();
            for component in &components {
                path.push(component);
            }
            path.set_extension("si");
            if path.exists() {
                return Ok(path);
            }
        }
    }

    Err(ImportError::new(format!(
        "module '{}' not found. Searched: SIGIL_STDLIB, ./library/, standard locations",
        module_name
    )))
}

/// Load and parse an imported module.
///
/// Returns the parse result for the imported file.
pub fn load_imported_module(
    import_path: &Path,
    interner: &StringInterner,
) -> Result<ParseResult, ImportError> {
    // Read the imported file
    let content = std::fs::read_to_string(import_path)
        .map_err(|e| ImportError::new(format!("Failed to read '{}': {}", import_path.display(), e)))?;

    // Parse the imported file
    let tokens = crate::lexer::lex(&content, interner);
    let imported_result = crate::parser::parse(&tokens, interner);

    if imported_result.has_errors() {
        let errors: Vec<String> = imported_result.errors
            .iter()
            .map(|e| format!("{}: {}", e.span, e.message))
            .collect();
        return Err(ImportError::new(format!(
            "Errors in '{}': {}",
            import_path.display(),
            errors.join(", ")
        )));
    }

    Ok(imported_result)
}

/// Build a map of all functions in a module.
///
/// This allows imported functions to call other functions from their module.
pub fn build_module_functions(
    parse_result: &ParseResult,
    imported_arena: &SharedArena,
) -> HashMap<Name, Value> {
    let mut module_functions: HashMap<Name, Value> = HashMap::new();

    for func in &parse_result.module.functions {
        let params: Vec<_> = imported_arena.get_params(func.params)
            .iter()
            .map(|p| p.name)
            .collect();

        let func_value = FunctionValue::from_import(
            params,
            func.body,
            HashMap::new(),
            imported_arena.clone(),
        );
        module_functions.insert(func.name, Value::Function(func_value));
    }

    module_functions
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
    imported_result: &ParseResult,
    imported_arena: &SharedArena,
    module_functions: &HashMap<Name, Value>,
    env: &mut Environment,
    interner: &StringInterner,
    import_path: &Path,
    current_file: &Path,
) -> Result<(), ImportError> {
    // Check if this is a test module importing from its parent module
    let allow_private_access = is_test_module(current_file)
        && is_parent_module_import(current_file, import_path);

    for item in &import.items {
        let item_name_str = interner.lookup(item.name);

        // Find the function in the imported module
        let func = imported_result.module.functions
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

            let params: Vec<_> = imported_arena.get_params(func.params)
                .iter()
                .map(|p| p.name)
                .collect();

            // Captures include: current environment + all module functions
            let mut captures = env.capture();
            captures.extend(module_functions.clone());

            let func_value = FunctionValue::from_import(
                params,
                func.body,
                captures,
                imported_arena.clone(),
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
pub fn register_module_functions(
    parse_result: &ParseResult,
    env: &mut Environment,
) {
    for func in &parse_result.module.functions {
        let params: Vec<_> = parse_result.arena.get_params(func.params)
            .iter()
            .map(|p| p.name)
            .collect();
        let captures = env.capture();
        let func_value = FunctionValue::with_captures(params, func.body, captures);
        env.define(func.name, Value::Function(func_value), false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::SharedInterner;
    use std::path::PathBuf;

    #[test]
    fn test_resolve_relative_path() {
        let interner = SharedInterner::default();
        let name = interner.intern("./math");
        let path = ImportPath::Relative(name);
        let current = PathBuf::from("/project/src/main.si");

        let result = resolve_import_path(&path, &current, &interner).unwrap();
        assert_eq!(result, PathBuf::from("/project/src/math.si"));
    }

    #[test]
    fn test_resolve_parent_path() {
        let interner = SharedInterner::default();
        let name = interner.intern("../utils");
        let path = ImportPath::Relative(name);
        let current = PathBuf::from("/project/src/main.si");

        let result = resolve_import_path(&path, &current, &interner).unwrap();
        assert_eq!(result, PathBuf::from("/project/src/../utils.si"));
    }

    #[test]
    fn test_resolve_module_path_not_found() {
        let interner = SharedInterner::default();
        let std = interner.intern("std");
        let math = interner.intern("math");
        let path = ImportPath::Module(vec![std, math]);
        let current = PathBuf::from("/project/src/main.si");

        let result = resolve_import_path(&path, &current, &interner);
        // Module resolution searches paths but won't find std.math in tests
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("not found"));
    }

    #[test]
    fn test_import_error_display() {
        let err = ImportError::new("test error");
        assert_eq!(format!("{}", err), "test error");
    }

    // =========================================================================
    // Test Module Detection Tests
    // =========================================================================

    #[test]
    fn test_is_test_module_valid() {
        // Valid test module: in _test/ with .test.si extension
        let path = PathBuf::from("/project/src/_test/math.test.si");
        assert!(is_test_module(&path));
    }

    #[test]
    fn test_is_test_module_not_in_test_dir() {
        // Not in _test/ directory
        let path = PathBuf::from("/project/src/math.test.si");
        assert!(!is_test_module(&path));
    }

    #[test]
    fn test_is_test_module_wrong_extension() {
        // In _test/ but wrong extension
        let path = PathBuf::from("/project/src/_test/math.si");
        assert!(!is_test_module(&path));
    }

    #[test]
    fn test_is_test_module_nested() {
        // Nested _test/ directory
        let path = PathBuf::from("/project/src/utils/_test/helpers.test.si");
        assert!(is_test_module(&path));
    }

    #[test]
    fn test_is_parent_module_import_valid() {
        // Test module importing from parent directory
        let current = PathBuf::from("/project/src/_test/math.test.si");
        let import = PathBuf::from("/project/src/math.si");
        assert!(is_parent_module_import(&current, &import));
    }

    #[test]
    fn test_is_parent_module_import_sibling() {
        // Importing from sibling, not parent
        let current = PathBuf::from("/project/src/_test/math.test.si");
        let import = PathBuf::from("/project/src/_test/utils.si");
        assert!(!is_parent_module_import(&current, &import));
    }

    #[test]
    fn test_is_parent_module_import_not_test() {
        // Not in _test directory
        let current = PathBuf::from("/project/src/main.si");
        let import = PathBuf::from("/project/src/math.si");
        assert!(!is_parent_module_import(&current, &import));
    }

    // =========================================================================
    // Loading Context Tests
    // =========================================================================

    #[test]
    fn test_loading_context_cycle_detection() {
        let mut ctx = LoadingContext::new();
        let path1 = PathBuf::from("/a.si");
        let path2 = PathBuf::from("/b.si");

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
        let path = PathBuf::from("/a.si");

        ctx.start_loading(path.clone()).unwrap();
        let result = ctx.start_loading(path.clone());
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("circular import"));
    }
}
