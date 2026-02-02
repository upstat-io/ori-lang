//! Multi-File AOT Compilation
//!
//! Enables AOT compilation of Ori programs with imports. Handles the full
//! pipeline from dependency graph construction to linking multiple object files.
//!
//! # Architecture
//!
//! ```text
//! Entry File
//!     ↓
//! build_dependency_graph() ─── uses resolve_import()
//!     ↓
//! Topological Sort ─── dependencies compile before dependents
//!     ↓
//! compile_module_to_object() ─── each .ori → .o with mangled names
//!     ↓
//! Link All Objects ─── existing LinkerDriver
//!     ↓
//! Executable
//! ```
//!
//! # Key Design Decisions
//!
//! 1. **Separate object files per module** — Each `.ori` → one `.o` (enables parallel compilation)
//! 2. **Module-qualified name mangling** — `_ori_<module>$<function>` format
//! 3. **Linker-resolved imports** — Each module declares (not defines) imported symbols
//! 4. **Reuse existing infrastructure** — `DependencyGraph`, `resolve_import`, `LinkerDriver`

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::aot::incremental::deps::DependencyGraph;
use crate::aot::incremental::hash::{ContentHash, SourceHasher};

// Note: extract_imports uses HashSet for O(1) deduplication instead of Vec::contains

/// Error during multi-file compilation.
#[derive(Debug, Clone)]
pub enum MultiFileError {
    /// Import resolution failed.
    ImportError { message: String, path: PathBuf },
    /// Circular dependency detected.
    CyclicDependency { cycle: Vec<PathBuf> },
    /// Failed to read source file.
    IoError { path: PathBuf, message: String },
    /// Compilation error in a module.
    CompilationError { path: PathBuf, message: String },
    /// Linking error.
    LinkError { message: String },
}

impl std::fmt::Display for MultiFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ImportError { message, path } => {
                write!(f, "import error in '{}': {}", path.display(), message)
            }
            Self::CyclicDependency { cycle } => {
                write!(f, "circular dependency: ")?;
                for (i, path) in cycle.iter().enumerate() {
                    if i > 0 {
                        write!(f, " -> ")?;
                    }
                    write!(f, "{}", path.display())?;
                }
                Ok(())
            }
            Self::IoError { path, message } => {
                write!(f, "failed to read '{}': {}", path.display(), message)
            }
            Self::CompilationError { path, message } => {
                write!(f, "compilation error in '{}': {}", path.display(), message)
            }
            Self::LinkError { message } => {
                write!(f, "link error: {message}")
            }
        }
    }
}

impl std::error::Error for MultiFileError {}

/// Context for tracking files during dependency graph construction.
///
/// Uses a loading stack for cycle detection and a visited set to avoid
/// processing the same file twice.
#[derive(Debug, Default)]
struct GraphBuildContext {
    /// Stack of files currently being processed (for cycle detection).
    loading_stack: Vec<PathBuf>,
    /// Set of files currently in the loading stack (O(1) cycle check).
    loading_set: HashSet<PathBuf>,
    /// Files that have been fully processed.
    visited: HashSet<PathBuf>,
}

impl GraphBuildContext {
    fn new() -> Self {
        Self::default()
    }

    /// Check if loading this path would create a cycle.
    fn would_cycle(&self, path: &Path) -> bool {
        self.loading_set.contains(path)
    }

    /// Check if this path has already been processed.
    fn is_visited(&self, path: &Path) -> bool {
        self.visited.contains(path)
    }

    /// Start processing a file. Returns error if this would create a cycle.
    fn start_loading(&mut self, path: PathBuf) -> Result<(), MultiFileError> {
        if self.would_cycle(&path) {
            let mut cycle: Vec<PathBuf> = self.loading_stack.clone();
            cycle.push(path);
            return Err(MultiFileError::CyclicDependency { cycle });
        }
        self.loading_set.insert(path.clone());
        self.loading_stack.push(path);
        Ok(())
    }

    /// Finish processing a file.
    fn finish_loading(&mut self, path: PathBuf) {
        if let Some(popped) = self.loading_stack.pop() {
            self.loading_set.remove(&popped);
        }
        self.visited.insert(path);
    }
}

/// Derive a module name from a file path.
///
/// This converts a file path to a module name suitable for symbol mangling.
/// Examples:
/// - `./helper.ori` → `helper`
/// - `./http/client.ori` → `http$client`
/// - `/path/to/math.ori` → `math`
#[must_use]
pub fn derive_module_name(path: &Path, base_dir: Option<&Path>) -> String {
    // Get the file stem (filename without extension)
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");

    // If we have a base directory, compute relative path for nested modules
    if let Some(base) = base_dir {
        if let Ok(relative) = path.strip_prefix(base) {
            // Remove the filename and convert directory structure to module path
            if let Some(parent) = relative.parent() {
                if !parent.as_os_str().is_empty() {
                    let parent_str = parent.to_string_lossy();
                    // Replace path separators with module separators
                    let parent_normalized = parent_str.replace(['/', '\\'], "$");
                    return format!("{parent_normalized}${stem}");
                }
            }
        }
    }

    stem.to_string()
}

/// Result of building a dependency graph.
#[derive(Debug)]
pub struct DependencyBuildResult {
    /// The dependency graph mapping files to their imports.
    pub graph: DependencyGraph,
    /// All files in topological order (dependencies before dependents).
    pub compilation_order: Vec<PathBuf>,
    /// The base directory for computing relative module names.
    pub base_dir: PathBuf,
}

/// Configuration for multi-file compilation.
#[derive(Debug, Clone)]
pub struct MultiFileConfig {
    /// Directory for intermediate object files.
    pub obj_dir: PathBuf,
    /// Whether to print verbose output.
    pub verbose: bool,
}

impl Default for MultiFileConfig {
    fn default() -> Self {
        Self {
            obj_dir: std::env::temp_dir().join("ori_build"),
            verbose: false,
        }
    }
}

impl MultiFileConfig {
    /// Create a new configuration with the given object directory.
    #[must_use]
    pub fn with_obj_dir(mut self, obj_dir: PathBuf) -> Self {
        self.obj_dir = obj_dir;
        self
    }

    /// Enable verbose output.
    #[must_use]
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }
}

/// Build a dependency graph from an entry file.
///
/// Recursively resolves all imports and constructs a graph of dependencies.
/// Returns the graph along with a topological ordering for compilation.
///
/// # Arguments
///
/// * `entry_path` - Path to the entry file (e.g., main.ori)
/// * `resolve_import_fn` - Function to resolve import paths to file paths
///
/// # Type Parameters
///
/// * `F` - Import resolver function type: `Fn(&Path, &str) -> Result<PathBuf, String>`
///   Takes (`current_file`, `import_path_str`) and returns resolved path or error.
pub fn build_dependency_graph<F>(
    entry_path: &Path,
    resolve_import_fn: F,
) -> Result<DependencyBuildResult, MultiFileError>
where
    F: Fn(&Path, &str) -> Result<PathBuf, String>,
{
    let mut graph = DependencyGraph::new();
    let mut hasher = SourceHasher::new();
    let mut context = GraphBuildContext::new();

    // Canonicalize entry path for consistent comparison
    let entry_canonical = entry_path
        .canonicalize()
        .unwrap_or_else(|_| entry_path.to_path_buf());

    // Get the base directory (parent of entry file)
    let base_dir = entry_canonical
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);

    // Process entry file and all its dependencies
    process_file(
        &entry_canonical,
        &mut graph,
        &mut hasher,
        &mut context,
        &resolve_import_fn,
    )?;

    // Get topological order
    let compilation_order = graph.topological_order().ok_or_else(|| {
        // This shouldn't happen since we check for cycles during graph building,
        // but handle it gracefully
        MultiFileError::CyclicDependency {
            cycle: graph.files().cloned().collect(),
        }
    })?;

    Ok(DependencyBuildResult {
        graph,
        compilation_order,
        base_dir,
    })
}

/// Process a single file and its imports recursively.
fn process_file<F>(
    path: &Path,
    graph: &mut DependencyGraph,
    hasher: &mut SourceHasher,
    ctx: &mut GraphBuildContext,
    resolve_import_fn: &F,
) -> Result<(), MultiFileError>
where
    F: Fn(&Path, &str) -> Result<PathBuf, String>,
{
    // Skip if already processed
    if ctx.is_visited(path) {
        return Ok(());
    }

    // Start loading (checks for cycles)
    ctx.start_loading(path.to_path_buf())?;

    // Read and hash the file
    let content = std::fs::read_to_string(path).map_err(|e| MultiFileError::IoError {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;

    let hash = hasher
        .hash_file(path)
        .map_err(|e| MultiFileError::IoError {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;

    // Extract imports from file content
    let imports = extract_imports(&content, path, resolve_import_fn)?;

    // Process each import recursively
    for import_path in &imports {
        process_file(import_path, graph, hasher, ctx, resolve_import_fn)?;
    }

    // Add this file to the graph
    graph.add_file(path.to_path_buf(), hash, imports);

    // Finish loading
    ctx.finish_loading(path.to_path_buf());

    Ok(())
}

/// Extract import paths from source content.
///
/// Parses use statements from the source and resolves them to file paths.
/// Uses `HashSet` for O(1) deduplication instead of O(n) `Vec::contains`.
fn extract_imports<F>(
    content: &str,
    current_file: &Path,
    resolve_import_fn: &F,
) -> Result<Vec<PathBuf>, MultiFileError>
where
    F: Fn(&Path, &str) -> Result<PathBuf, String>,
{
    // Use HashSet for O(1) deduplication instead of O(n) Vec::contains
    let mut seen = HashSet::new();
    let mut imports = Vec::new();

    // Simple line-based extraction of use statements
    // Matches patterns like: use "./helper" { ... } or use "./helper" as alias
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("use ") {
            continue;
        }

        // Extract the path part (between use and the next space/brace)
        let after_use = &trimmed[4..].trim_start();

        // Skip module imports (std.xxx)
        if !after_use.starts_with('"') && !after_use.starts_with("\"./") {
            continue;
        }

        // Extract the quoted path
        if let Some(path_str) = extract_quoted_path(after_use) {
            // Only handle relative imports for now
            if path_str.starts_with("./") || path_str.starts_with("../") {
                match resolve_import_fn(current_file, path_str) {
                    Ok(resolved) => {
                        // Canonicalize for consistent comparison
                        let canonical = resolved.canonicalize().unwrap_or(resolved);
                        // O(1) HashSet insert + check instead of O(n) Vec::contains
                        if seen.insert(canonical.clone()) {
                            imports.push(canonical);
                        }
                    }
                    Err(msg) => {
                        return Err(MultiFileError::ImportError {
                            message: msg,
                            path: current_file.to_path_buf(),
                        });
                    }
                }
            }
        }
    }

    Ok(imports)
}

/// Extract a quoted string from the start of a line.
fn extract_quoted_path(s: &str) -> Option<&str> {
    if !s.starts_with('"') {
        return None;
    }

    let after_quote = &s[1..];
    let end = after_quote.find('"')?;
    Some(&after_quote[..end])
}

/// Resolve a relative import path, trying both file and directory module patterns.
///
/// For `./http`, tries:
/// 1. `./http.ori` (file-based module)
/// 2. `./http/mod.ori` (directory-based module)
///
/// This is the standard resolver function that should be passed to [`build_dependency_graph`].
///
/// # Arguments
///
/// * `current_file` - Path to the file containing the import statement
/// * `import_path` - The import path string (e.g., `./helper`, `../utils`)
///
/// # Returns
///
/// * `Ok(PathBuf)` - The resolved path to the module file
/// * `Err(String)` - Error message if neither file nor directory module was found
pub fn resolve_relative_import(current_file: &Path, import_path: &str) -> Result<PathBuf, String> {
    let dir = current_file.parent().unwrap_or(Path::new("."));
    let base_path = dir.join(import_path);

    // Generate candidates: file first, then directory module
    let candidates = if base_path.extension().is_none() {
        vec![base_path.with_extension("ori"), base_path.join("mod.ori")]
    } else {
        vec![base_path.clone()]
    };

    // Try each candidate
    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }

    // Format searched paths for error message
    let searched = candidates
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");

    Err(format!(
        "cannot find import '{import_path}'. Searched: {searched}"
    ))
}

/// Compiled module information.
#[derive(Debug)]
pub struct CompiledModule {
    /// Source file path.
    pub source_path: PathBuf,
    /// Object file path.
    pub object_path: PathBuf,
    /// Module name used for symbol mangling.
    pub module_name: String,
    /// Content hash of the source.
    pub hash: ContentHash,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_module_name_simple() {
        let path = Path::new("/project/src/helper.ori");
        assert_eq!(derive_module_name(path, None), "helper");
    }

    #[test]
    fn test_derive_module_name_nested() {
        let base = Path::new("/project/src");
        let path = Path::new("/project/src/http/client.ori");
        assert_eq!(derive_module_name(path, Some(base)), "http$client");
    }

    #[test]
    fn test_derive_module_name_deeply_nested() {
        let base = Path::new("/project/src");
        let path = Path::new("/project/src/net/http/json/parser.ori");
        assert_eq!(derive_module_name(path, Some(base)), "net$http$json$parser");
    }

    #[test]
    fn test_extract_quoted_path() {
        assert_eq!(extract_quoted_path("\"./helper\""), Some("./helper"));
        assert_eq!(
            extract_quoted_path("\"../utils\" { foo }"),
            Some("../utils")
        );
        assert_eq!(extract_quoted_path("no quotes"), None);
        assert_eq!(extract_quoted_path("\"unclosed"), None);
    }

    #[test]
    fn test_graph_build_context_cycle_detection() {
        let mut ctx = GraphBuildContext::new();
        let path_a = PathBuf::from("/a.ori");
        let path_b = PathBuf::from("/b.ori");

        // Start loading A
        ctx.start_loading(path_a.clone()).unwrap();
        assert!(ctx.would_cycle(&path_a));
        assert!(!ctx.would_cycle(&path_b));

        // Starting A again should error
        let result = ctx.start_loading(path_a.clone());
        assert!(matches!(
            result,
            Err(MultiFileError::CyclicDependency { .. })
        ));

        // Start loading B (should work)
        ctx.start_loading(path_b.clone()).unwrap();
        assert!(ctx.would_cycle(&path_b));

        // Finish B
        ctx.finish_loading(path_b.clone());
        assert!(!ctx.would_cycle(&path_b));
        assert!(ctx.is_visited(&path_b));
    }

    #[test]
    fn test_multi_file_error_display() {
        let err = MultiFileError::ImportError {
            message: "not found".to_string(),
            path: PathBuf::from("/test.ori"),
        };
        assert!(err.to_string().contains("import error"));
        assert!(err.to_string().contains("/test.ori"));

        let err = MultiFileError::CyclicDependency {
            cycle: vec![
                PathBuf::from("a.ori"),
                PathBuf::from("b.ori"),
                PathBuf::from("a.ori"),
            ],
        };
        assert!(err.to_string().contains("circular dependency"));
        assert!(err.to_string().contains("a.ori"));
        assert!(err.to_string().contains("b.ori"));
    }

    #[test]
    fn test_multi_file_config() {
        let config = MultiFileConfig::default()
            .with_obj_dir(PathBuf::from("/custom/obj"))
            .with_verbose(true);

        assert_eq!(config.obj_dir, PathBuf::from("/custom/obj"));
        assert!(config.verbose);
    }

    #[test]
    fn test_extract_imports_basic() {
        let content = r#"
use "./helper" { add }
use "./utils" as util

@main () -> void = print(msg: "hello")
"#;

        // Mock resolver that just appends .ori
        let resolver = |current: &Path, import: &str| {
            let dir = current.parent().unwrap_or(Path::new("."));
            let path = dir.join(import);
            let with_ext = if path.extension().is_none() {
                path.with_extension("ori")
            } else {
                path
            };
            Ok(with_ext)
        };

        let current = Path::new("/project/main.ori");
        let imports = extract_imports(content, current, &resolver).unwrap();

        assert_eq!(imports.len(), 2);
    }

    #[test]
    fn test_extract_imports_skips_module_imports() {
        let content = r#"
use std.math { sqrt }
use "./local" { foo }
"#;

        let resolver = |current: &Path, import: &str| {
            Ok(current.parent().unwrap().join(import).with_extension("ori"))
        };

        let current = Path::new("/project/main.ori");
        let imports = extract_imports(content, current, &resolver).unwrap();

        // Should only include the relative import, not std.math
        assert_eq!(imports.len(), 1);
    }

    #[test]
    fn test_resolve_relative_import_file_module() {
        // Create a temp directory with a file module
        let temp_dir = std::env::temp_dir().join("ori_test_resolve_file");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let helper_file = temp_dir.join("helper.ori");
        std::fs::write(&helper_file, "pub @foo () -> int = 42").unwrap();

        let current = temp_dir.join("main.ori");
        let result = resolve_relative_import(&current, "./helper");

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), helper_file);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_resolve_relative_import_directory_module() {
        // Create a temp directory with a directory module
        let temp_dir = std::env::temp_dir().join("ori_test_resolve_dir");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(temp_dir.join("http")).unwrap();

        let mod_file = temp_dir.join("http/mod.ori");
        std::fs::write(&mod_file, "pub @get () -> str = \"ok\"").unwrap();

        let current = temp_dir.join("main.ori");
        let result = resolve_relative_import(&current, "./http");

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mod_file);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_resolve_relative_import_prefers_file_over_directory() {
        // When both file and directory module exist, file takes precedence
        let temp_dir = std::env::temp_dir().join("ori_test_resolve_both");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(temp_dir.join("utils")).unwrap();

        let file_module = temp_dir.join("utils.ori");
        let dir_module = temp_dir.join("utils/mod.ori");
        std::fs::write(&file_module, "// file module").unwrap();
        std::fs::write(&dir_module, "// dir module").unwrap();

        let current = temp_dir.join("main.ori");
        let result = resolve_relative_import(&current, "./utils");

        assert!(result.is_ok());
        // File module should be preferred
        assert_eq!(result.unwrap(), file_module);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_resolve_relative_import_not_found() {
        let temp_dir = std::env::temp_dir().join("ori_test_resolve_notfound");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let current = temp_dir.join("main.ori");
        let result = resolve_relative_import(&current, "./nonexistent");

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("cannot find import"));
        assert!(err.contains("nonexistent.ori"));
        assert!(err.contains("nonexistent/mod.ori"));

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_resolve_relative_import_with_extension() {
        // When extension is provided, don't try directory module
        let temp_dir = std::env::temp_dir().join("ori_test_resolve_ext");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let helper_file = temp_dir.join("helper.ori");
        std::fs::write(&helper_file, "pub @foo () -> int = 42").unwrap();

        let current = temp_dir.join("main.ori");
        let result = resolve_relative_import(&current, "./helper.ori");

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), helper_file);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_resolve_relative_import_parent_path() {
        // Test ../path resolution
        let temp_dir = std::env::temp_dir().join("ori_test_resolve_parent");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(temp_dir.join("src")).unwrap();

        let utils_file = temp_dir.join("utils.ori");
        std::fs::write(&utils_file, "pub @helper () -> int = 1").unwrap();

        let current = temp_dir.join("src/main.ori");
        let result = resolve_relative_import(&current, "../utils");

        assert!(result.is_ok());
        // Compare canonicalized paths since ../utils resolves to parent dir
        let resolved = result.unwrap().canonicalize().unwrap();
        let expected = utils_file.canonicalize().unwrap();
        assert_eq!(resolved, expected);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
