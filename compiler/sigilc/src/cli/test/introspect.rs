// Module introspection utilities

use sigilc::ast::{Item, Module};

/// Get all function names from a module
pub fn get_functions(module: &Module) -> Vec<String> {
    module
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Function(f) = item {
                Some(f.name.clone())
            } else {
                None
            }
        })
        .collect()
}

/// Get all tested function names from a module (from test targets)
pub fn get_tested_functions(module: &Module) -> Vec<String> {
    module
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Test(t) = item {
                Some(t.target.clone())
            } else {
                None
            }
        })
        .collect()
}
