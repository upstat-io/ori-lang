---
title: "Testing System Overview"
description: "Ori Compiler Design — Testing System Overview"
order: 900
section: "Testing"
---

# Testing System Overview

The Ori test system provides test discovery, parallel execution, and coverage tracking. Testing is mandatory in Ori - every function requires tests.

## Location

```
compiler/oric/src/test/
├── mod.rs            # Module exports
├── runner.rs         # Test execution
├── discovery.rs      # Test finding
├── result.rs         # Test result types
└── error_matching.rs # ExpectedError matching for compile_fail tests

compiler/oric/src/testing/
├── mod.rs     # Testing utilities
├── harness.rs # Test harness
└── mocks.rs   # Mock implementations
```

## Design Goals

1. **Mandatory testing** - Functions without tests fail compilation
2. **Parallel execution** - Tests run concurrently
3. **Targeted tests** - Tests declare what they test
4. **Fast feedback** - Quick test discovery and execution

## Test Types

### Targeted Tests

Test a specific function:

```ori
@add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void =
    assert_eq(actual: add(2, 3), expected: 5)
```

### Free-Floating Tests

Test multiple things or integration:

```ori
@test_integration () -> void = {
    let result = process_data(input);
    assert(cond: result.is_valid);
}
```

### Multi-Target Tests

Test multiple functions:

```ori
@test_math tests @add tests @subtract () -> void = {
    assert_eq(actual: add(1, 2), expected: 3);
    assert_eq(actual: subtract(5, 3), expected: 2);
}
```

## Test Attributes

### #[skip("reason")]

Skip a test:

```ori
#[skip("not implemented yet")]
@test_future_feature () -> void = ...
```

### #[compile_fail("error")]

Expect compilation to fail:

```ori
#[compile_fail("type mismatch")]
@test_type_error () -> void = {
    let x: int = "not an int";
}
```

### Extended compile_fail Syntax

The `compile_fail` attribute supports rich error specifications:

```ori
// Simple message matching (legacy)
#[compile_fail("type mismatch")]

// Error code matching
#[compile_fail(code: "E2001")]

// Combined matching
#[compile_fail(code: "E2001", message: "type mismatch")]

// Position-specific matching
#[compile_fail(message: "error", line: 5)]
#[compile_fail(message: "error", line: 5, column: 10)]

// Multiple expected errors (multiple attributes)
#[compile_fail("type mismatch")]
#[compile_fail("unknown identifier")]
@test_multiple_errors () -> void = ...
```

### ExpectedError Structure

```rust
#[derive(Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct ExpectedError {
    pub message: Option<Name>,   // Substring match
    pub code: Option<Name>,      // Error code (e.g., "E2001")
    pub line: Option<u32>,       // Expected line (1-based)
    pub column: Option<u32>,     // Expected column (1-based)
}
```

The error matching module (`error_matching.rs`) provides:

```rust
/// Convert byte offset to (line, column).
pub fn offset_to_line_col(source: &str, offset: u32) -> (usize, usize);

/// Check if actual error matches expected specification.
pub fn matches_expected(
    actual: &TypeCheckError,
    expected: &ExpectedError,
    source: &str,
    interner: &StringInterner,
) -> bool;
```

### #[fail("message")]

Expect test to fail at runtime:

```ori
#[fail("assertion failed")]
@test_expected_failure () -> void =
    assert(cond: false)
```

## Test Output

```
Running 42 tests...

test @test_add ... ok (2ms)
test @test_subtract ... ok (1ms)
test @test_multiply ... FAILED (5ms)

  assertion failed: expected 6, got 5
    at src/mathsi:15:5

test @test_divide ... ok (1ms)
test @test_skip ... skipped (not implemented yet)

Results: 40 passed, 1 failed, 1 skipped
Coverage: 95% of functions tested
```

## Test Outcomes

Each test produces one of four outcomes:

| Outcome | Meaning | Counts as failure? |
|---------|---------|-------------------|
| `Passed` | Test passed (including matched `compile_fail` expectations) | No |
| `Failed(String)` | Test failed with error message | Yes |
| `Skipped(String)` | Test skipped with reason (via `#[skip]`) | No |
| `LlvmCompileFail(String)` | LLVM compilation of file failed — test could not run | No (tracked separately) |

`LlvmCompileFail` is distinct from `Failed`: it indicates the LLVM backend could not compile the file, not that the test logic is wrong. These are tracked separately in the summary and displayed as LLVM compilation issues rather than test failures.

## Test Runner Architecture

### Shared Interner Pattern

All test files share one `SharedInterner` (Arc-wrapped) so `Name` values are comparable across files:

```rust
let interner = SharedInterner::new();  // Arc-wrapped

for file in files {
    let db = CompilerDb::with_interner(interner.clone());
    let parsed = parsed(&db, file);
    // All Name values now comparable across files
}
```

### Backend Support

| Backend | Description | Parallelization |
|---------|-------------|-----------------|
| Interpreter | Tree-walking evaluator | Parallel (rayon scoped thread pool) |
| LLVM | JIT compilation | Sequential (global lock contention) |

### LLVM "Compile Once, Run Many"

For LLVM tests, a single compilation pass generates all test wrappers:

```rust
let compiled = evaluator.compile_module_with_tests(tests);  // ONE pass

for test in tests {
    compiled.run_test(test.name);  // N calls, NO recompilation
}
```

This provides O(N + M) performance vs O(N × M) where N=functions, M=tests.

### Test Execution Flow

```
discover_tests_in(path)
  → for each file:
    → parse (→ ParseOutput)
    → separate compile_fail vs regular tests
    → type_check_with_imports_source_and_interner
    → run_compile_fail_tests (error matching only, no eval)
    → run_regular_tests (interpreter or LLVM)
    → apply_fail_wrapper for tests with #[fail]
```

### Error Matching Algorithm

For `compile_fail` tests, errors are matched using multi-criteria matching:

1. For each expectation, find first unmatched error satisfying all criteria
2. Greedy 1:1 matching (one error satisfies one expectation)
3. Report unmatched expectations and unmatched errors

**Span Isolation:** For multiple compile_fail tests in the same file, errors are first filtered to those within the test's span, falling back to all module errors if none found.

## Phase Tests vs Spec Tests

| Scenario | Location |
|----------|----------|
| Internal compiler behavior | `tests/phases/` |
| Inline unit test < 200 lines | `compiler/<crate>/src/` |
| Needs multiple compiler internals | `tests/phases/` |
| User-facing language feature | `tests/spec/` |
| Backend-independent behavior | `tests/spec/` |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | All tests passed |
| 1 | Test failures exist |
| 2 | No tests found |

## Related Documents

- [Test Discovery](test-discovery.md) - Finding tests
- [Test Runner](test-runner.md) - Executing tests
