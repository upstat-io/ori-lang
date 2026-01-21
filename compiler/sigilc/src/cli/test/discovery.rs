// Test file discovery

use std::fs;
use std::path::{Path, PathBuf};

/// Discover all test files in the current directory and subdirectories
pub fn discover_test_files() -> Vec<PathBuf> {
    let mut test_files = Vec::new();
    discover_test_files_recursive(Path::new("."), &mut test_files);
    test_files.sort();
    test_files
}

fn discover_test_files_recursive(dir: &Path, test_files: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip hidden directories and common non-source directories
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
        }

        if path.is_dir() {
            discover_test_files_recursive(&path, test_files);
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.ends_with(".test.si") {
                test_files.push(path);
            }
        }
    }
}
