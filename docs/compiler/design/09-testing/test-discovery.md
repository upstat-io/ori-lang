---
title: "Test Discovery"
description: "Ori Compiler Design — Test Discovery"
order: 901
section: "Testing"
---

# Test Discovery

Test discovery finds all `.ori` files in a directory tree for test execution.

## Location

```
compiler/oric/src/test/discovery/mod.rs (85 lines)
```

## Architecture

Test discovery operates at the **filesystem level**, not the AST level. It finds files that may contain tests; actual test extraction happens during parsing.

```
discover_tests(root: &Path)
        |
        v
   filesystem scan
        |
        v
  Vec<TestFile>    <-- Just paths, no parsing yet
        |
        v
   runner parses each file
        |
        v
  extract tests from Module.tests
```

## TestFile Structure

```rust
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
```

## Discovery Functions

### discover_tests

Recursively finds all `.ori` files in a directory:

```rust
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
```

### discover_tests_in

Handles both file and directory paths:

```rust
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
```

## Filtering Rules

### Skipped Directories

| Directory | Reason |
|-----------|--------|
| `.hidden` | Hidden files/directories (starts with `.`) |
| `target` | Rust build output |
| `node_modules` | Node.js dependencies |
| `.git` | Version control |
| `__pycache__` | Python cache |

### File Extensions

Only `.ori` files are discovered. Test functions are extracted from these files during parsing.

## File Conventions

| Pattern | Description |
|---------|-------------|
| `foo.ori` | Source file (may contain tests) |
| `foo.test.ori` | Dedicated test file |
| `_test/` | Test directory convention |

All `.ori` files are scanned for tests, regardless of naming convention. The `_test/` directory convention is organizational, not enforced by discovery.

## Integration with Test Runner

The runner uses discovery to get file paths, then parses each file:

```rust
impl TestRunner {
    pub fn run(&self, path: &Path) -> TestSummary {
        // Discovery: filesystem scan
        let test_files = discover_tests_in(path);

        if self.config.parallel && test_files.len() > 1 {
            self.run_parallel(&test_files)
        } else {
            self.run_sequential(&test_files)
        }
    }

    fn run_file(&self, path: &Path) -> FileSummary {
        // Parsing: extract tests from Module
        let content = std::fs::read_to_string(path)?;
        let db = CompilerDb::new();
        let file = SourceFile::new(&db, path.to_path_buf(), content);
        let parse_result = parsed(&db, file);

        // Tests are in parse_result.module.tests
        for test in &parse_result.module.tests {
            // Execute test...
        }
    }
}
```

## Coverage Checking

Coverage checking (which functions have tests) is handled in the **runner**, not discovery:

```rust
// runner/mod.rs
fn coverage_report(&self, path: &Path) -> CoverageReport {
    // Examines which functions are targeted by tests
    // Reports functions without test coverage
}
```

Enable with `ori test --coverage`.
