//! Import resolution and module loading.
//!
//! Handles resolving import paths to file paths and loading imported modules.

use std::path::{Path, PathBuf};
use std::collections::HashMap;
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

/// Resolve an import path to a file path.
///
/// Handles relative paths (starting with './' or '../') and module paths.
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
            // Module paths like std.math - not implemented yet
            let path_str: String = segments
                .iter()
                .map(|s| interner.lookup(*s))
                .collect::<Vec<_>>()
                .join(".");
            Err(ImportError::new(format!("Module imports not yet implemented: {}", path_str)))
        }
    }
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
pub fn register_imports(
    import: &crate::ir::UseDef,
    imported_result: &ParseResult,
    imported_arena: &SharedArena,
    module_functions: &HashMap<Name, Value>,
    env: &mut Environment,
    interner: &StringInterner,
    import_path: &Path,
) -> Result<(), ImportError> {
    for item in &import.items {
        let item_name_str = interner.lookup(item.name);

        // Find the function in the imported module
        let func = imported_result.module.functions
            .iter()
            .find(|f| interner.lookup(f.name) == item_name_str);

        if let Some(func) = func {
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
    fn test_resolve_module_path_not_implemented() {
        let interner = SharedInterner::default();
        let std = interner.intern("std");
        let math = interner.intern("math");
        let path = ImportPath::Module(vec![std, math]);
        let current = PathBuf::from("/project/src/main.si");

        let result = resolve_import_path(&path, &current, &interner);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("not yet implemented"));
    }

    #[test]
    fn test_import_error_display() {
        let err = ImportError::new("test error");
        assert_eq!(format!("{}", err), "test error");
    }
}
