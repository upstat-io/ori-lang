//! Test file discovery.
//!
//! Finds all test files in a given directory tree.
//! Convention: All .ori files can contain tests (functions with `tests` keyword).

use std::fs;
use std::path::{Path, PathBuf};

/// A discovered test file.
#[derive(Clone, Debug)]
pub struct TestFile {
    /// Path to the test file.
    pub path: PathBuf,
}

impl TestFile {
    pub fn new(path: PathBuf) -> Self {
        TestFile { path }
    }
}

/// Discover all test files in a directory tree.
///
/// # Arguments
/// * `root` - Root directory to search
///
/// # Returns
/// Vector of discovered test files, sorted by path.
pub fn discover_tests(root: &Path) -> Vec<TestFile> {
    let mut files = Vec::new();
    discover_recursive(root, &mut files);
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

fn discover_recursive(dir: &Path, files: &mut Vec<TestFile>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip hidden files and directories
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') {
                continue;
            }
        }

        if path.is_dir() {
            // Skip common non-source directories
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if matches!(name, "target" | "node_modules" | ".git" | "__pycache__") {
                    continue;
                }
            }
            discover_recursive(&path, files);
        } else if path.extension().is_some_and(|e| e == "ori") {
            files.push(TestFile::new(path));
        }
    }
}

/// Discover tests in a specific file or directory.
///
/// If `path` is a file, returns just that file.
/// If `path` is a directory, discovers all .ori files recursively.
pub fn discover_tests_in(path: &Path) -> Vec<TestFile> {
    if path.is_file() {
        if path.extension().is_some_and(|e| e == "ori") {
            vec![TestFile::new(path.to_path_buf())]
        } else {
            vec![]
        }
    } else if path.is_dir() {
        discover_tests(path)
    } else {
        vec![]
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests;
