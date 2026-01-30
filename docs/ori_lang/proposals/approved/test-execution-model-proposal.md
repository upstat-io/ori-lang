# Proposal: Test Execution Model

**Status:** Approved
**Author:** Eric (with Claude)
**Created:** 2026-01-29
**Approved:** 2026-01-29
**Affects:** `compiler/oric/`, `compiler/ori_ir/`, CLI interface, `.ori/cache/`

## Summary

Define the complete test execution model for Ori: when tests run, which tests run, and how the compiler integrates test execution into the build process. This proposal consolidates and extends the approved [Dependency-Aware Testing](../approved/dependency-aware-testing-proposal.md) and [Incremental Test Execution](../approved/incremental-test-execution-proposal.md) proposals into an implementable specification.

The formal language specification ([Testing](../../spec/13-testing.md)) defines the semantics of test execution. This proposal provides the detailed implementation model—data structures, algorithms, and cache formats—that the compiler must implement to satisfy that specification.

## Motivation

Ori's core promise is **code that proves itself**: every function has tests, every change is verified, every effect is explicit. The testing system is not an afterthought—it's integral to compilation.

Current state:
- Test syntax is implemented (`@test tests @target`)
- Basic test runner exists
- Coverage enforcement exists
- **Missing**: Tests don't run automatically during compilation
- **Missing**: No dependency-aware test selection
- **Missing**: No incremental caching

This proposal specifies the complete execution model so that:
1. `ori check` compiles code AND runs affected tests
2. Changes to `@foo` automatically trigger tests for `@foo` and its callers
3. Unchanged code uses cached test results
4. Developers get immediate feedback without manual test invocation

## Design

### Core Principle: Tests Are Part of Compilation

In traditional languages, testing is separate from building:
```
build → (separate) → test
```

In Ori, testing is integrated into compilation:
```
parse → type check → test execution → codegen
```

A successful `ori check` means:
1. Code compiles
2. Affected tests pass (or are reported, in non-strict mode)

### Compilation Pipeline

```
Source Files
    │
    ▼
┌─────────────────┐
│     Parse       │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   Type Check    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Test Discovery │  ← Build test registry, compute affected set
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Test Execution  │  ← Run affected targeted tests
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│    Code Gen     │  (if requested)
└─────────────────┘
```

Tests run **after** type checking:
- A test's target must type-check before the test can execute
- Test failures are reported but don't block codegen (unless `--strict`)

### Test Registry

The compiler builds a test registry during compilation:

```rust
struct TestRegistry {
    /// Map from function → tests that target it
    tests_for: HashMap<FunctionId, Vec<TestId>>,

    /// Map from function → functions that call it (reverse deps)
    callers: HashMap<FunctionId, HashSet<FunctionId>>,

    /// Set of free-floating tests (target = _)
    free_floating: HashSet<TestId>,
}
```

Built during type checking:
1. For each `@test tests @target`: add to `tests_for[target]`
2. For each function call `f()` inside function `g`: add `g` to `callers[f]`
3. For each `@test tests _`: add to `free_floating`

### Change Detection

A function is **changed** if its content hash differs from the cached hash.

The content hash captures the function's definition—its body, signature, and constraints. When a function's content changes, the reverse closure algorithm (see below) propagates this to find all tests that may be affected.

Note: A function's hash does not change when its *dependencies* change. The test selection algorithm handles dependency propagation separately: if `@bar` changes and `@foo` calls `@bar`, then `@foo` is in `@bar`'s reverse closure, and tests for `@foo` will run—even though `@foo`'s hash is unchanged.

```rust
struct ChangeDetector {
    /// Previous compilation's hashes
    cached_hashes: HashMap<FunctionId, u64>,

    /// Current compilation's hashes
    current_hashes: HashMap<FunctionId, u64>,
}

impl ChangeDetector {
    fn is_changed(&self, func: FunctionId) -> bool {
        match (self.cached_hashes.get(&func), self.current_hashes.get(&func)) {
            (Some(old), Some(new)) => old != new,
            (None, Some(_)) => true,  // New function
            (Some(_), None) => true,  // Deleted function
            (None, None) => false,
        }
    }

    fn changed_functions(&self) -> HashSet<FunctionId> {
        self.current_hashes.keys()
            .filter(|f| self.is_changed(**f))
            .copied()
            .collect()
    }
}
```

Content hash includes:
- Function body AST (normalized: whitespace and comments stripped, source structure preserved)
- Parameter types and names
- Return type
- Capability requirements
- Generic constraints

Normalization ensures that formatting changes (whitespace, comments) do not invalidate the cache, while meaningful changes to code structure do.

### Reverse Transitive Closure

When a function changes, we need to find all functions that depend on it:

```rust
impl TestRegistry {
    /// Compute all functions affected by changes to `roots`
    fn reverse_closure(&self, roots: &HashSet<FunctionId>) -> HashSet<FunctionId> {
        let mut affected = roots.clone();
        let mut queue: VecDeque<_> = roots.iter().copied().collect();

        while let Some(func) = queue.pop_front() {
            if let Some(callers) = self.callers.get(&func) {
                for caller in callers {
                    if affected.insert(*caller) {
                        queue.push_back(*caller);
                    }
                }
            }
        }

        affected
    }

    /// Find tests to run for the given changed functions
    fn affected_tests(&self, changed: &HashSet<FunctionId>) -> Vec<TestId> {
        let affected = self.reverse_closure(changed);

        affected.iter()
            .filter_map(|f| self.tests_for.get(f))
            .flatten()
            .copied()
            .collect()
    }
}
```

### Example: Dependency Propagation

```ori
@helper (x: int) -> int = x * 2

@process (x: int) -> int = helper(x: x) + 1

@handle (x: int) -> int = process(x: x) + 10

@test_helper tests @helper () -> void = ...
@test_process tests @process () -> void = ...
@test_handle tests @handle () -> void = ...
```

Dependency graph:
```
@helper ← @process ← @handle
```

If `@helper` changes:
1. Changed set: `{@helper}`
2. Reverse closure: `{@helper, @process, @handle}`
3. Affected tests: `{@test_helper, @test_process, @test_handle}`

If `@handle` changes:
1. Changed set: `{@handle}`
2. Reverse closure: `{@handle}` (no callers)
3. Affected tests: `{@test_handle}`

### Test Result Caching

Test results are cached keyed by the hash of all inputs:

```rust
struct TestCache {
    /// Map from (test_id, inputs_hash) → result
    results: HashMap<(TestId, u64), TestResult>,
}

impl TestCache {
    fn inputs_hash(&self, test: TestId, registry: &TestRegistry) -> u64 {
        // Hash of all target functions' content hashes
        let mut hasher = DefaultHasher::new();
        for target in &test.targets {
            if let Some(hash) = registry.function_hash(target) {
                hash.hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    fn get_cached(&self, test: TestId, inputs_hash: u64) -> Option<&TestResult> {
        self.results.get(&(test, inputs_hash))
    }
}
```

Cache invalidation is automatic:
- If any target's hash changes, the inputs_hash changes
- Old cache entries become unreachable (can be pruned)

### Incremental Execution Flow

```
1. Load cache from .ori/cache/
2. Parse and type-check all files
3. Compute current function hashes
4. Detect changed functions (hash mismatch)
5. Compute reverse closure (affected set)
6. Find targeted tests for affected set
7. For each test:
   a. Compute inputs_hash
   b. If cached result exists with same inputs_hash → skip
   c. Otherwise → execute test, cache result
8. Report results
9. Save cache to .ori/cache/
```

### Full Compilation

On full compilation (no cache, or `--clean`):
1. All targeted tests execute
2. Results are cached
3. Free-floating tests do NOT execute (require `ori test`)

### Cache Storage

```
.ori/
├── cache/
│   ├── hashes.bin       # FunctionId → content hash
│   ├── deps.bin         # Dependency graph (callers map)
│   └── test-results/    # TestId → TestResult
└── ...
```

Format: Binary serialization (bincode or similar) for performance.

The `.ori/` directory should be in `.gitignore`.

### Cache Maintenance

**Pruning:** On successful build completion, the compiler removes cache entries for functions that no longer exist in the codebase. This prevents unbounded cache growth as code evolves.

**Invalidation:** Cache entries are never explicitly invalidated. Instead, the `inputs_hash` mechanism ensures stale entries are simply not matched—they become unreachable and are pruned on the next successful build.

### Test Result States

```rust
enum TestResult {
    Pass { duration: Duration },
    Fail { message: String, location: SourceLoc },
    Skip { reason: String },
    Error { message: String },  // Could not execute
}
```

### Non-Blocking vs Strict Mode

**Non-blocking (default):**
```
$ ori check src/

Compiling...
Running 3 affected tests...
  ✓ @test_helper (2ms)
  ✗ @test_process
    assertion failed: expected 5, got 6
    at src/lib.ori:25:5
  ✓ @test_handle (1ms)

Build succeeded with 1 test failure.
```

Compilation completes. Exit code 0. Developer can iterate.

**Strict mode (`--strict`):**
```
$ ori check --strict src/

Compiling...
Running 3 affected tests...
  ✓ @test_helper (2ms)
  ✗ @test_process
    assertion failed: expected 5, got 6

Build FAILED: 1 test failure.
```

Compilation fails. Exit code 1. For CI and pre-commit hooks.

### Performance Warning

Targeted tests run during compilation. Slow tests degrade the development experience.

```
warning: targeted test @test_large_parse took 350ms
  --> src/parser.ori:100:1
   |
100| @test_large_parse tests @parse () -> void = ...
   | ^^^^^^^^^^^^^^^^^ slow targeted test
   |
   = note: targeted tests run during compilation
   = help: consider making this a free-floating test: `tests _`
   = note: threshold is 100ms (configurable in ori.toml)
```

Configuration:
```toml
# ori.toml
[testing]
slow_test_threshold = "100ms"  # default
```

### Free-Floating Tests

Tests with `tests _` are explicitly excluded from compilation:

```ori
@test_integration tests _ () -> void = run(
    // Slow integration test with real I/O
    let result = full_pipeline(input: large_input),
    assert_ok(result: result),
)
```

Free-floating tests:
- Do NOT run during `ori check`
- Do NOT satisfy coverage requirements
- Only run via explicit `ori test`

Use cases:
- Integration tests
- Performance benchmarks
- Tests requiring external services
- Tests with large datasets

### CLI Interface

#### ori check

```
ori check [OPTIONS] <PATH>

Compile and run affected targeted tests.

Options:
    --no-test     Skip test execution (compile only)
    --strict      Fail build on any test failure
    --verbose     Show all test results, not just failures
    --clean       Ignore cache, run all targeted tests
```

#### ori test

```
ori test [OPTIONS] [PATH]

Run tests explicitly.

Options:
    --only-targeted    Skip free-floating tests
    --filter <PATTERN> Run only tests matching pattern
    --verbose          Show all test results
```

#### Execution Matrix

| Command | Targeted (affected) | Targeted (unaffected) | Free-floating |
|---------|--------------------|-----------------------|---------------|
| `ori check` | Run | Skip (cached) | Never |
| `ori check --no-test` | Never | Never | Never |
| `ori check --clean` | Run | Run | Never |
| `ori test` | Run | Run | Run |
| `ori test --only-targeted` | Run | Run | Never |

Note: `--clean` forces re-execution of all targeted tests (ignoring cache), but does not run free-floating tests. Free-floating tests always require explicit `ori test`, regardless of other flags.

## Implementation Plan

### Phase 1: Test Registry
- [ ] Add `TestRegistry` struct to compiler
- [ ] Build `tests_for` map during type checking
- [ ] Build `callers` map from call graph analysis
- [ ] Identify free-floating tests

### Phase 2: Change Detection
- [ ] Implement content hashing for functions
- [ ] Add `ChangeDetector` struct
- [ ] Integrate with existing incremental compilation (if any)

### Phase 3: Reverse Closure
- [ ] Implement `reverse_closure()` algorithm
- [ ] Implement `affected_tests()` lookup
- [ ] Add tests for closure computation

### Phase 4: Test Caching
- [ ] Define cache file format
- [ ] Implement cache loading/saving
- [ ] Implement inputs_hash computation
- [ ] Implement cache lookup during test execution

### Phase 5: CLI Integration
- [ ] Modify `ori check` to run tests after type checking
- [ ] Add `--no-test`, `--strict`, `--clean` flags
- [ ] Implement non-blocking result reporting
- [ ] Implement strict mode failure

### Phase 6: Performance Warnings
- [ ] Track test execution duration
- [ ] Emit warning for slow targeted tests
- [ ] Read threshold from `ori.toml`

### Phase 7: Polish
- [ ] Progress reporting during test execution
- [ ] Parallel test execution
- [ ] Cache pruning (remove stale entries)
- [ ] Documentation

## Testing the Implementation

The implementation should be verified with:

1. **Unit tests** for registry, closure, and cache logic
2. **Integration tests** for CLI behavior
3. **Spec tests** in `tests/spec/testing/`:
   - `incremental.ori` — verify only affected tests run
   - `closure.ori` — verify reverse closure is computed correctly
   - `caching.ori` — verify cache hits skip execution
   - `strict.ori` — verify `--strict` fails on test failure
   - `free-floating.ori` — verify `tests _` excluded from check

## Alternatives Considered

### 1. Tests Run Before Type Checking

Rejected: A test's target must type-check first. Running tests on invalid code is meaningless.

### 2. Tests Block Compilation by Default

Rejected: Would slow iteration. Developers want to see both compilation errors AND test failures together, then fix iteratively.

### 3. No Caching, Always Run All Tests

Rejected: Defeats the purpose of incremental compilation. Would be too slow for large codebases.

### 4. Forward Closure Instead of Reverse Closure

Rejected: Forward closure (dependencies of changed function) misses the critical case: a change to `@helper` should run tests for `@caller` because `@caller`'s behavior depends on `@helper`.

## Summary

This proposal defines Ori's test execution model:

1. **Tests run during compilation** — after type checking, before codegen
2. **Dependency-aware** — changes to `@foo` trigger tests for `@foo` and all callers
3. **Incremental** — unchanged functions use cached test results
4. **Non-blocking by default** — failures reported but don't block compilation
5. **Strict mode for CI** — `--strict` fails on any test failure
6. **Performance-conscious** — warnings for slow targeted tests

Combined with mandatory test coverage and capability-based mocking, this creates a system where **code integrity is enforced automatically** as a natural part of the development workflow.
