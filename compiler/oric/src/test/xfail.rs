//! Expected failure (XFAIL) tracking for backend-specific test gaps.
//!
//! Loads a list of tests and files expected to fail for a given backend.
//! Matching failures become "expected failure" (no exit code impact).
//! Unexpected passes (XPASS) produce warnings so stale entries get removed.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Set of tests and files expected to fail for a specific backend.
#[derive(Debug)]
pub struct XFailSet {
    /// Test names expected to fail (e.g., `test_list_map_type`).
    tests: HashSet<String>,
    /// File paths expected to error during compilation (relative to project root).
    files: HashSet<PathBuf>,
}

impl XFailSet {
    /// Create an empty set (no expected failures).
    pub fn empty() -> Self {
        XFailSet {
            tests: HashSet::new(),
            files: HashSet::new(),
        }
    }

    /// Load expected failures from `xfail-{backend}.txt`.
    ///
    /// Searches `test_dir` and its ancestors for the xfail file, so running
    /// `ori test tests/spec/types/foo.ori` still finds `tests/xfail-llvm.txt`.
    ///
    /// If the file doesn't exist, returns an empty set (backward compatible).
    ///
    /// Format:
    /// - Lines starting with `#` are comments
    /// - Blank lines are ignored
    /// - `test:name` — individual test expected to fail
    /// - `file:path` — file expected to error during compilation
    pub fn load(test_dir: &Path, backend: &str) -> Self {
        let filename = format!("xfail-{backend}.txt");

        // Walk up from test_dir to find the xfail file
        let mut dir = Some(test_dir);
        while let Some(d) = dir {
            let path = d.join(&filename);
            if let Ok(content) = std::fs::read_to_string(&path) {
                return Self::parse(&content);
            }
            dir = d.parent();
        }

        Self::empty()
    }

    /// Parse xfail file content.
    ///
    /// File paths in the xfail list are stored as-is from the file.
    /// The test runner passes paths in the same form they appear in
    /// `FileSummary` (relative from CWD, e.g., `tests/spec/types/foo.ori`).
    fn parse(content: &str) -> Self {
        let mut tests = HashSet::new();
        let mut files = HashSet::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip comments and blank lines
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if let Some(name) = trimmed.strip_prefix("test:") {
                let name = name.trim();
                if !name.is_empty() {
                    tests.insert(name.to_string());
                }
            } else if let Some(path) = trimmed.strip_prefix("file:") {
                let path = path.trim();
                if !path.is_empty() {
                    files.insert(PathBuf::from(path));
                }
            }
            // Unknown prefixes are silently ignored (forward compatible)
        }

        XFailSet { tests, files }
    }

    /// Check if a test name is expected to fail.
    pub fn is_expected_test_failure(&self, test_name: &str) -> bool {
        self.tests.contains(test_name)
    }

    /// Check if a file path is expected to error during compilation.
    ///
    /// Compares canonicalized paths to handle relative/absolute differences.
    pub fn is_expected_file_error(&self, file_path: &Path) -> bool {
        // Try direct match first
        if self.files.contains(file_path) {
            return true;
        }
        // Try canonicalized match for path normalization
        if let Ok(canonical) = file_path.canonicalize() {
            self.files.iter().any(|expected| {
                expected
                    .canonicalize()
                    .is_ok_and(|exp_canonical| exp_canonical == canonical)
            })
        } else {
            false
        }
    }

    /// Returns true if this set has no expected failures.
    pub fn is_empty(&self) -> bool {
        self.tests.is_empty() && self.files.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_set_has_no_failures() {
        let set = XFailSet::empty();
        assert!(!set.is_expected_test_failure("anything"));
        assert!(!set.is_expected_file_error(Path::new("anything.ori")));
        assert!(set.is_empty());
    }

    #[test]
    fn parse_test_entries() {
        let content = "test:test_list_map\ntest:test_filter\n";
        let set = XFailSet::parse(content);
        assert!(set.is_expected_test_failure("test_list_map"));
        assert!(set.is_expected_test_failure("test_filter"));
        assert!(!set.is_expected_test_failure("test_other"));
    }

    #[test]
    fn parse_file_entries() {
        let content = "file:tests/spec/types/args.ori\n";
        let set = XFailSet::parse(content);
        assert!(set.is_expected_file_error(Path::new("tests/spec/types/args.ori")));
        assert!(!set.is_expected_file_error(Path::new("tests/spec/other.ori")));
    }

    #[test]
    fn parse_skips_comments_and_blanks() {
        let content = "\
# This is a comment
test:test_one

# Another comment

test:test_two
";
        let set = XFailSet::parse(content);
        assert!(set.is_expected_test_failure("test_one"));
        assert!(set.is_expected_test_failure("test_two"));
        assert!(!set.is_empty());
    }

    #[test]
    fn parse_trims_whitespace() {
        let content = "  test:test_padded  \n  file:tests/padded.ori  \n";
        let set = XFailSet::parse(content);
        assert!(set.is_expected_test_failure("test_padded"));
        assert!(set.is_expected_file_error(Path::new("tests/padded.ori")));
    }

    #[test]
    fn parse_ignores_unknown_prefixes() {
        let content = "unknown:something\ntest:valid\n";
        let set = XFailSet::parse(content);
        assert!(set.is_expected_test_failure("valid"));
        // No panic or error for unknown prefix
    }

    #[test]
    fn load_missing_file_returns_empty() {
        let set = XFailSet::load(Path::new("/nonexistent"), "llvm");
        assert!(set.is_empty());
    }
}
