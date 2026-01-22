// Module path resolver for Sigil
// Maps import paths (e.g., "std.io") to file paths

use std::path::{Path, PathBuf};

/// Resolved module information
#[derive(Debug, Clone)]
pub struct ResolvedModule {
    /// Absolute path to the source file
    pub path: PathBuf,
    /// The module's logical name (e.g., "std.io")
    pub name: String,
}

/// Module path resolver
pub struct ModuleResolver {
    /// Root directory for module resolution
    root: PathBuf,
    /// Library paths to search
    lib_paths: Vec<PathBuf>,
}

impl ModuleResolver {
    /// Create a new resolver with the given root directory
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();

        // Default library paths
        let mut lib_paths = Vec::new();

        // Add library/ subdirectory if it exists
        let lib_dir = root.join("library");
        if lib_dir.exists() {
            lib_paths.push(lib_dir);
        }

        // Add std library path relative to root
        let std_dir = root.join("library/std");
        if std_dir.exists() {
            lib_paths.push(std_dir);
        }

        ModuleResolver { root, lib_paths }
    }

    /// Add a library path to search
    pub fn add_lib_path(&mut self, path: impl AsRef<Path>) {
        self.lib_paths.push(path.as_ref().to_path_buf());
    }

    /// Canonicalize a path
    pub fn canonicalize(&self, path: &Path) -> Result<PathBuf, String> {
        if path.is_absolute() {
            path.canonicalize()
                .map_err(|e| format!("Cannot canonicalize '{}': {}", path.display(), e))
        } else {
            self.root
                .join(path)
                .canonicalize()
                .map_err(|e| format!("Cannot canonicalize '{}': {}", path.display(), e))
        }
    }

    /// Resolve an import path to a file path
    ///
    /// Resolution order:
    /// 1. Relative to the importing file's directory
    /// 2. In library paths
    /// 3. Relative to root
    pub fn resolve_import(&self, from: &Path, import_path: &[String]) -> Result<PathBuf, String> {
        if import_path.is_empty() {
            return Err("Empty import path".to_string());
        }

        // Check if this is a string path (starts with ./ or ../)
        let is_string_path = import_path.len() == 1
            && (import_path[0].starts_with("./") || import_path[0].starts_with("../"));

        if is_string_path {
            // For string paths, resolve directly relative to the importing file
            if let Some(parent) = from.parent() {
                let mut path = parent.join(&import_path[0]);
                // Add .si extension if not present
                if path.extension().is_none() {
                    path.set_extension("si");
                }
                if path.exists() {
                    // Use std::fs::canonicalize directly since path is relative to CWD
                    return path.canonicalize()
                        .map_err(|e| format!("Cannot canonicalize '{}': {}", path.display(), e));
                }
                return Err(format!(
                    "Cannot find module '{}' (looked for {})",
                    import_path[0],
                    path.display()
                ));
            }
        }

        // Convert import path to file path
        // e.g., ["std", "io"] -> "std/io.si"
        let relative_path = self.import_to_path(import_path);

        // 1. Try relative to the importing file's directory
        if let Some(parent) = from.parent() {
            // Handle _test directory convention
            let search_dir = if parent.ends_with("_test") {
                parent.parent().unwrap_or(parent)
            } else {
                parent
            };

            let candidate = search_dir.join(&relative_path);
            if candidate.exists() {
                return self.canonicalize(&candidate);
            }
        }

        // 2. Try library paths
        for lib_path in &self.lib_paths {
            let candidate = lib_path.join(&relative_path);
            if candidate.exists() {
                return self.canonicalize(&candidate);
            }
        }

        // 3. Try relative to root
        let candidate = self.root.join(&relative_path);
        if candidate.exists() {
            return self.canonicalize(&candidate);
        }

        // Build helpful error message
        let mut searched = vec![from
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default()];
        searched.extend(self.lib_paths.iter().map(|p| p.display().to_string()));
        searched.push(self.root.display().to_string());

        Err(format!(
            "Cannot find module '{}' (looked in: {})",
            import_path.join("."),
            searched.join(", ")
        ))
    }

    /// Convert an import path to a relative file path
    fn import_to_path(&self, import_path: &[String]) -> PathBuf {
        let mut path = PathBuf::new();
        for (i, segment) in import_path.iter().enumerate() {
            if i == import_path.len() - 1 {
                // Last segment gets .si extension
                path.push(format!("{}.si", segment));
            } else {
                path.push(segment);
            }
        }
        path
    }

    /// Get the logical module name from a file path
    pub fn path_to_module_name(&self, path: &Path) -> String {
        // Try to make the path relative to root
        let relative = path.strip_prefix(&self.root).unwrap_or(path);

        // Convert path components to module name
        let mut parts: Vec<&str> = relative
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect();

        // Remove .si extension from last component
        if let Some(last) = parts.last_mut() {
            if last.ends_with(".si") {
                *last = &last[..last.len() - 3];
            }
        }

        parts.join(".")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_import_to_path() {
        let temp_dir = TempDir::new().unwrap();
        let resolver = ModuleResolver::new(temp_dir.path());

        assert_eq!(
            resolver.import_to_path(&["math".to_string()]),
            PathBuf::from("math.si")
        );

        assert_eq!(
            resolver.import_to_path(&["std".to_string(), "io".to_string()]),
            PathBuf::from("std/io.si")
        );
    }

    #[test]
    fn test_resolve_relative() {
        let temp_dir = TempDir::new().unwrap();

        // Create test files
        fs::write(temp_dir.path().join("main.si"), "@main () -> void = nil").unwrap();
        fs::write(
            temp_dir.path().join("math.si"),
            "@add (a: int, b: int) -> int = a + b",
        )
        .unwrap();

        let resolver = ModuleResolver::new(temp_dir.path());
        let from = temp_dir.path().join("main.si");

        let resolved = resolver.resolve_import(&from, &["math".to_string()]);
        assert!(resolved.is_ok());
        assert!(resolved.unwrap().ends_with("math.si"));
    }

    #[test]
    fn test_resolve_from_test_dir() {
        let temp_dir = TempDir::new().unwrap();

        // Create test files
        fs::write(
            temp_dir.path().join("math.si"),
            "@add (a: int, b: int) -> int = a + b",
        )
        .unwrap();
        fs::create_dir(temp_dir.path().join("_test")).unwrap();
        fs::write(
            temp_dir.path().join("_test/math.test.si"),
            "use math { add }",
        )
        .unwrap();

        let resolver = ModuleResolver::new(temp_dir.path());
        let from = temp_dir.path().join("_test/math.test.si");

        let resolved = resolver.resolve_import(&from, &["math".to_string()]);
        assert!(resolved.is_ok());
        assert!(resolved.unwrap().ends_with("math.si"));
    }

    #[test]
    fn test_path_to_module_name() {
        let temp_dir = TempDir::new().unwrap();
        let resolver = ModuleResolver::new(temp_dir.path());

        let path = temp_dir.path().join("std/io.si");
        assert_eq!(resolver.path_to_module_name(&path), "std.io");
    }
}
