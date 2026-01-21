// Module system for Sigil
// Handles multi-file programs with imports
//
// Key responsibilities:
// - Resolve import paths to file paths
// - Parse and cache modules
// - Build dependency graph
// - Detect circular imports
// - Provide imported symbols to type checker

mod resolver;

pub use resolver::{ModuleResolver, ResolvedModule};

use crate::ast::{Item, Module, UseDef};
use crate::errors::{Diagnostic, DiagnosticResult};
use crate::errors::codes::ErrorCode;
use crate::{lexer, parser};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A loaded module with its parsed AST and metadata
#[derive(Debug)]
pub struct LoadedModule {
    /// The module AST
    pub module: Module,
    /// Absolute path to the source file
    pub path: PathBuf,
    /// Direct dependencies (module paths)
    pub dependencies: Vec<Vec<String>>,
}

/// Module graph for dependency tracking
pub struct ModuleGraph {
    /// Loaded modules by their canonical path
    modules: HashMap<String, LoadedModule>,
    /// Module resolver for path resolution
    resolver: ModuleResolver,
    /// Currently loading stack (for cycle detection)
    loading_stack: Vec<String>,
}

impl ModuleGraph {
    /// Create a new module graph with the given root directory
    pub fn new(root_dir: impl AsRef<Path>) -> Self {
        ModuleGraph {
            modules: HashMap::new(),
            resolver: ModuleResolver::new(root_dir),
            loading_stack: Vec::new(),
        }
    }

    /// Load a module and all its dependencies
    pub fn load_module(&mut self, path: &Path) -> DiagnosticResult<&LoadedModule> {
        let canonical = self.resolver.canonicalize(path)
            .map_err(|e| Diagnostic::error(ErrorCode::E1001, format!("Cannot resolve path: {}", e)))?;

        let key = canonical.to_string_lossy().to_string();

        // Check for circular import
        if self.loading_stack.contains(&key) {
            let cycle = self.loading_stack.iter()
                .skip_while(|p| *p != &key)
                .cloned()
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(Diagnostic::error(
                ErrorCode::E1001,
                format!("Circular import detected: {} -> {}", cycle, key),
            ));
        }

        // Return if already loaded
        if self.modules.contains_key(&key) {
            return Ok(self.modules.get(&key).unwrap());
        }

        // Push to loading stack
        self.loading_stack.push(key.clone());

        // Load the module
        let loaded = self.load_single_module(&canonical)?;

        // Load dependencies recursively
        let deps = loaded.dependencies.clone();
        for dep_path in &deps {
            let resolved = self.resolver.resolve_import(&canonical, dep_path)
                .map_err(|e| Diagnostic::error(ErrorCode::E1001, e))?;
            self.load_module(&resolved)?;
        }

        // Pop from loading stack and store
        self.loading_stack.pop();
        self.modules.insert(key.clone(), loaded);

        Ok(self.modules.get(&key).unwrap())
    }

    /// Load a single module without loading dependencies
    fn load_single_module(&self, path: &Path) -> DiagnosticResult<LoadedModule> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| Diagnostic::error(
                ErrorCode::E1001,
                format!("Cannot read '{}': {}", path.display(), e),
            ))?;

        let filename = path.to_string_lossy().to_string();
        let tokens = lexer::tokenize(&source, &filename)
            .map_err(|e| Diagnostic::error(ErrorCode::E1001, e))?;
        let module = parser::parse(tokens, &filename)
            .map_err(|e| Diagnostic::error(ErrorCode::E2001, e))?;

        // Extract dependencies from use statements
        let dependencies: Vec<Vec<String>> = module.items.iter()
            .filter_map(|item| {
                if let Item::Use(use_def) = item {
                    Some(use_def.path.clone())
                } else {
                    None
                }
            })
            .collect();

        Ok(LoadedModule {
            module,
            path: path.to_path_buf(),
            dependencies,
        })
    }

    /// Get a loaded module by its canonical path
    pub fn get_module(&self, path: &Path) -> Option<&LoadedModule> {
        let key = path.to_string_lossy().to_string();
        self.modules.get(&key)
    }

    /// Get all loaded modules
    pub fn modules(&self) -> impl Iterator<Item = &LoadedModule> {
        self.modules.values()
    }

    /// Resolve an import path relative to a source file
    pub fn resolve_import(&self, from: &Path, import_path: &[String]) -> Result<PathBuf, String> {
        self.resolver.resolve_import(from, import_path)
    }
}

/// Get imported items from a use statement
pub fn get_imported_items<'a>(
    use_def: &UseDef,
    source_module: &'a Module,
) -> Result<Vec<&'a Item>, String> {
    let mut items = Vec::new();

    for use_item in &use_def.items {
        if use_item.name == "*" {
            // Import all public functions, configs, and types
            for item in &source_module.items {
                match item {
                    Item::Function(f) if f.public || f.name != "main" => {
                        items.push(item);
                    }
                    Item::Config(_) | Item::TypeDef(_) => {
                        items.push(item);
                    }
                    _ => {}
                }
            }
        } else {
            // Import specific item
            let found = source_module.items.iter().find(|item| match item {
                Item::Function(f) => f.name == use_item.name,
                Item::Config(c) => c.name == use_item.name,
                Item::TypeDef(t) => t.name == use_item.name,
                _ => false,
            });

            if let Some(item) = found {
                items.push(item);
            } else {
                return Err(format!(
                    "Cannot find '{}' in module '{}'",
                    use_item.name,
                    use_def.path.join(".")
                ));
            }
        }
    }

    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_load_single_module() {
        let temp_dir = TempDir::new().unwrap();
        let path = create_test_file(
            temp_dir.path(),
            "main.si",
            "@main () -> void = nil",
        );

        let mut graph = ModuleGraph::new(temp_dir.path());
        let loaded = graph.load_module(&path).unwrap();

        assert_eq!(loaded.module.items.len(), 1);
        assert!(loaded.dependencies.is_empty());
    }

    #[test]
    fn test_circular_import_detection() {
        let temp_dir = TempDir::new().unwrap();

        // Create a.si that imports b
        create_test_file(
            temp_dir.path(),
            "a.si",
            "use b { foo }\n@bar () -> int = foo()",
        );

        // Create b.si that imports a (circular!)
        create_test_file(
            temp_dir.path(),
            "b.si",
            "use a { bar }\n@foo () -> int = bar()",
        );

        let mut graph = ModuleGraph::new(temp_dir.path());
        let result = graph.load_module(&temp_dir.path().join("a.si"));

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Circular import"));
    }
}
