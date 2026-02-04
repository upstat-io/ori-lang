---
title: "Test Runner"
description: "Ori Compiler Design — Test Runner"
order: 902
section: "Testing"
---

# Test Runner

The test runner executes discovered tests and reports results.

## Location

```
compiler/oric/src/test/runner.rs (~700 lines)
```

## Configuration

```rust
/// Backend for test execution.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Backend {
    /// Tree-walking interpreter (default).
    #[default]
    Interpreter,
    /// LLVM JIT compiler.
    LLVM,
}

/// Configuration for the test runner.
#[derive(Clone, Debug)]
pub struct TestRunnerConfig {
    /// Filter tests by name pattern (substring match).
    pub filter: Option<String>,
    /// Enable verbose output.
    pub verbose: bool,
    /// Run tests in parallel.
    pub parallel: bool,
    /// Generate coverage report.
    pub coverage: bool,
    /// Backend to use for execution.
    pub backend: Backend,
}

impl Default for TestRunnerConfig {
    fn default() -> Self {
        TestRunnerConfig {
            filter: None,
            verbose: false,
            parallel: true,
            coverage: false,
            backend: Backend::Interpreter,
        }
    }
}
```

## Runner Structure

The runner is minimal — configuration only, no registries:

```rust
/// Test runner.
pub struct TestRunner {
    config: TestRunnerConfig,
}

impl TestRunner {
    /// Create a new test runner with default config.
    pub fn new() -> Self {
        TestRunner {
            config: TestRunnerConfig::default(),
        }
    }

    /// Create a test runner with custom config.
    pub fn with_config(config: TestRunnerConfig) -> Self {
        TestRunner { config }
    }
}
```

## Execution Flow

### Entry Point

```rust
impl TestRunner {
    /// Run all tests in a path (file or directory).
    pub fn run(&self, path: &Path) -> TestSummary {
        let test_files = discover_tests_in(path);

        if self.config.parallel && test_files.len() > 1 {
            self.run_parallel(&test_files)
        } else {
            self.run_sequential(&test_files)
        }
    }
}
```

### Parallel Execution

Uses Rayon for work-stealing parallelism:

```rust
/// Run tests in parallel using rayon.
fn run_parallel(&self, files: &[TestFile]) -> TestSummary {
    let start = Instant::now();

    let file_summaries: Vec<_> = files
        .par_iter()
        .map(|file| self.run_file(&file.path))
        .collect();

    let mut summary = TestSummary::new();
    for file_summary in file_summaries {
        summary.add_file(file_summary);
    }

    summary.duration = start.elapsed();
    summary
}
```

### File Processing

Each file is parsed and tests are extracted:

```rust
fn run_file(&self, path: &Path) -> FileSummary {
    let mut summary = FileSummary::new(path.to_path_buf());

    // Read file
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            summary.add_error(format!("Failed to read file: {e}"));
            return summary;
        }
    };

    // Parse with Salsa
    let source = content.clone();
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, path.to_path_buf(), content);
    let parse_result = parsed(&db, file);

    if parse_result.has_errors() {
        for error in &parse_result.errors {
            summary.add_error(format!("{}: {}", error.span, error.message));
        }
        return summary;
    }

    // Check for tests
    if parse_result.module.tests.is_empty() {
        return summary;
    }

    // Separate compile_fail tests from regular tests
    let (compile_fail_tests, regular_tests): (Vec<_>, Vec<_>) =
        parse_result.module.tests.iter().partition(|t| t.has_compile_fail());

    // Run compile-fail tests (type check only)
    for test in compile_fail_tests {
        let result = self.run_compile_fail_test(&db, file, test, &source);
        summary.add_result(result);
    }

    // Run regular tests (full execution)
    for test in regular_tests {
        let result = self.run_test(&db, file, test);
        summary.add_result(result);
    }

    summary
}
```

## Test Results

### Per-Test Result

```rust
pub struct TestResult {
    pub name: String,
    pub targets: Vec<String>,
    pub outcome: TestOutcome,
    pub duration: Duration,
}

pub enum TestOutcome {
    Passed,              // Test passed (including matched compile_fail)
    Failed(String),      // Test failed (error message)
    Skipped(String),     // Test skipped (skip reason)
}
```

**Note:** Compile-fail tests map to `Passed` when errors match or `Failed` when they don't. The distinction between compile-fail and runtime tests is handled at the runner logic level, not in the outcome enum.

```rust
// Compile-fail test handling (simplified)
if expected_errors_matched {
    TestOutcome::Passed
} else {
    TestOutcome::Failed(format!("expected error '{}', got '{}'", expected, actual))
}
```

### File Summary

```rust
pub struct FileSummary {
    pub path: PathBuf,
    pub results: Vec<TestResult>,
    pub errors: Vec<String>,    // File-level errors (parse failures)
}
```

### Test Summary

```rust
pub struct TestSummary {
    pub files: Vec<FileSummary>,
    pub duration: Duration,
}

impl TestSummary {
    pub fn passed(&self) -> usize { ... }
    pub fn failed(&self) -> usize { ... }
    pub fn skipped(&self) -> usize { ... }
    pub fn total(&self) -> usize { ... }
    pub fn success(&self) -> bool { self.failed() == 0 }
}
```

## Backend Support

### Interpreter (Default)

```rust
fn run_test_interpreter(
    &self,
    evaluator: &mut Evaluator,
    test: &TestDef,
    interner: &StringInterner,
) -> TestResult {
    let start = Instant::now();

    match evaluator.eval_test(test, interner) {
        Ok(()) => TestResult {
            name: test.name.clone(),
            targets: test.targets.clone(),
            outcome: TestOutcome::Passed,
            duration: start.elapsed(),
        },
        Err(e) => TestResult {
            name: test.name.clone(),
            targets: test.targets.clone(),
            outcome: TestOutcome::Failed(e.message),
            duration: start.elapsed(),
        },
    }
}
```

### LLVM JIT

```rust
fn run_test_llvm(&self, test: &TestDef, ...) -> TestResult {
    // Compile to LLVM IR
    // JIT execute
    // Return result
}
```

Enable with `ori test --backend=llvm`.

## Compile-Fail Tests

Tests with `#compile_fail` attributes expect type errors:

```rust
fn run_compile_fail_test(
    &self,
    db: &CompilerDb,
    file: SourceFile,
    test: &TestDef,
    source: &str,
) -> TestResult {
    // Type check the test body
    let type_result = type_check_with_imports_and_source(db, file, source);

    if !type_result.has_errors() {
        // Expected error but compiled successfully
        return TestResult {
            outcome: TestOutcome::UnexpectedCompileSuccess,
            ...
        };
    }

    // Match errors against expectations
    let expectations = test.compile_fail_expectations();
    let errors = type_result.errors;

    if match_errors(&expectations, &errors) {
        TestResult {
            outcome: TestOutcome::CompileFailMatched,
            ...
        }
    } else {
        TestResult {
            outcome: TestOutcome::CompileFailMismatch {
                expected: format_expected(&expectations),
                actual: format_actual(&errors),
            },
            ...
        }
    }
}
```

## Filtering

Tests can be filtered by name substring:

```rust
// In run_file
if let Some(filter) = &self.config.filter {
    if !test.name.contains(filter) {
        continue;  // Skip non-matching tests
    }
}
```

## Coverage Report

When `--coverage` is enabled:

```rust
fn compute_coverage(&self, results: &[FileSummary]) -> CoverageReport {
    let mut report = CoverageReport::new();

    for file in results {
        for result in &file.results {
            // Track which functions are targeted
            for target in &result.targets {
                report.mark_covered(target);
            }
        }

        // Find functions without coverage
        // (requires parsing the source file again)
    }

    report
}
```

## CLI Usage

```bash
# Run all tests
ori test

# Run tests in specific path
ori test tests/spec/

# Filter by name pattern
ori test --filter="math"

# Verbose output
ori test --verbose

# Sequential execution
ori test --no-parallel

# Use LLVM backend
ori test --backend=llvm

# Generate coverage report
ori test --coverage
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | All tests passed |
| 1 | Some tests failed |
| 2 | Error (parse failure, etc.) |
