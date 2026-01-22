// Import resolution for Sigil
// Handles loading modules referenced by `use` statements

use sigilc::ast::{Item, Module};
use sigilc::{lexer, parser};
use std::fs;
use std::path::Path;

/// Parse a Sigil source file into a Module
pub fn parse_file(path: &str) -> Result<Module, String> {
    let source =
        fs::read_to_string(path).map_err(|e| format!("Error reading '{}': {}", path, e))?;
    let tokens = lexer::tokenize(&source, path)?;
    parser::parse(tokens, path)
}

/// Resolve an import path relative to the importing file
pub fn resolve_import_path(from_file: &str, import_path: &[String]) -> String {
    let from = Path::new(from_file);
    let dir = from.parent().unwrap_or(Path::new("."));

    // Check if this is a string path (starts with ./ or ../)
    if import_path.len() == 1 && (import_path[0].starts_with("./") || import_path[0].starts_with("../")) {
        // Relative string path like '../hello_world' or './math'
        let relative_path = &import_path[0];
        let mut path = dir.join(relative_path);
        // Add .si extension if not present
        if path.extension().is_none() {
            path.set_extension("si");
        }
        return path.to_str().unwrap_or_default().to_string();
    }

    // Convert dot-separated path to file path
    // e.g., "math" -> "../math.si" (relative to _test folder)
    // e.g., "utils.helpers" -> "../utils/helpers.si"
    let mut path = dir.to_path_buf();

    // Go up one level if we're in _test folder
    if dir.ends_with("_test") {
        path = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    }

    for (i, segment) in import_path.iter().enumerate() {
        if i == import_path.len() - 1 {
            path = path.join(format!("{}.si", segment));
        } else {
            path = path.join(segment);
        }
    }

    path.to_str().unwrap_or_default().to_string()
}

/// Load all items from modules referenced by `use` statements
pub fn load_imports(test_module: &Module, test_path: &str) -> Result<Vec<Item>, String> {
    let mut imported_items = Vec::new();

    for item in &test_module.items {
        if let Item::Use(use_def) = item {
            let source_path = resolve_import_path(test_path, &use_def.path);

            if !Path::new(&source_path).exists() {
                return Err(format!(
                    "Cannot find module '{}' (looked for {})",
                    use_def.path.join("."),
                    source_path
                ));
            }

            let source_module = parse_file(&source_path)?;

            // Import specified items or all if wildcard
            for use_item in &use_def.items {
                if use_item.name == "*" {
                    // Import all functions and configs
                    for item in &source_module.items {
                        match item {
                            Item::Function(_) | Item::Config(_) => {
                                imported_items.push(item.clone());
                            }
                            _ => {}
                        }
                    }
                } else {
                    // Import specific item
                    let found = source_module.items.iter().find(|item| match item {
                        Item::Function(f) => f.name == use_item.name,
                        Item::Config(c) => c.name == use_item.name,
                        _ => false,
                    });

                    if let Some(item) = found {
                        imported_items.push(item.clone());
                    } else {
                        return Err(format!(
                            "Cannot find '{}' in module '{}'",
                            use_item.name,
                            use_def.path.join(".")
                        ));
                    }
                }
            }
        }
    }

    Ok(imported_items)
}
