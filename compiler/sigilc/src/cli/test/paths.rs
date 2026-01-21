// Test file path utilities

use std::path::Path;

/// Get the test file path for a given source file
pub fn get_test_file_path(source_path: &str) -> String {
    let path = Path::new(source_path);
    let parent = path.parent().unwrap_or(Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    // Try _test/name.test.si first
    let test_dir = parent.join("_test");
    let test_file = test_dir.join(format!("{}.test.si", stem));
    if test_file.exists() {
        return test_file.to_str().unwrap_or_default().to_string();
    }

    // Fall back to name.test.si in same directory
    let test_file = parent.join(format!("{}.test.si", stem));
    test_file.to_str().unwrap_or_default().to_string()
}
