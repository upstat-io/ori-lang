# Proposal: Dependency-Aware Test Execution

**Status:** Approved
**Author:** Eric (with Claude)
**Created:** 2026-01-24
**Approved:** 2026-01-25

---

## Summary

Implement reverse dependency closure for test execution. When a function changes, automatically run tests for that function AND tests for all functions that depend on it (callers up the dependency graph).

```
@foo changed
    ↓
run @test_foo (direct tests)
    ↓
find all functions that call @foo
    ↓
run their tests too
    ↓
recurse up the call graph
```

---

## Motivation

### The Problem with Traditional Test Runners

Traditional test runners have two modes:

1. **Run all tests** — Correct but slow. Doesn't scale.
2. **Run selected tests** — Fast but risky. May miss breakages.

Neither option is satisfactory:
- Running all tests on every change wastes time
- Running only direct tests misses cascading failures
- Manual test selection requires human judgment (error-prone)

### The Ori Advantage

Ori has information that traditional test runners lack:

1. **Tests are bound to functions** — `@test tests @target` creates an explicit relationship
2. **The compiler knows the dependency graph** — Which functions call which
3. **Tests are first-class** — They're in the module, not external files

This enables **dependency-aware test execution**: run exactly the tests that could be affected by a change.

### Why This Matters

```ori
@parse (input: str) -> Result<Ast, Error> = ...
@test_parse tests @parse () -> void = ...

@compile (input: str) -> Result<Binary, Error> = {
    let ast = parse(input: input)?
    generate_code(ast: ast)
}
@test_compile tests @compile () -> void = ...

@run_program (input: str) -> Result<Output, Error> = {
    let binary = compile(input: input)?
    execute(binary: binary)
}
@test_run tests @run_program () -> void = ...
```

If `@parse` changes:
- **Traditional runner (direct only):** Runs `@test_parse`. Misses breakage in `@compile` and `@run_program`.
- **Traditional runner (all):** Runs everything. Slow.
- **Ori (dependency-aware):** Runs `@test_parse`, `@test_compile`, `@test_run`. Fast AND correct.

---

## Design

### Test Execution Levels

| Level | What Runs | Use Case |
|-------|-----------|----------|
| **Direct** | Tests for the changed function only | Quick sanity check |
| **Closure** | Tests for changed function + all callers (recursive) | Default for incremental builds |
| **Full** | All tests in project | CI, release validation |

### Algorithm: Reverse Dependency Closure

```
function tests_to_run(changed_functions):
    affected = {}

    for func in changed_functions:
        affected.add(func)
        affected.union(reverse_transitive_closure(func))

    return tests where test.target in affected

function reverse_transitive_closure(func):
    callers = direct_callers(func)
    result = callers

    for caller in callers:
        result.union(reverse_transitive_closure(caller))

    return result
```

### Example Execution

Given this dependency graph:

```
@parse ← @compile ← @run_program ← @main
           ↑
       @optimize
```

And these test bindings:
- `@test_parse tests @parse`
- `@test_compile tests @compile`
- `@test_optimize tests @optimize`
- `@test_run tests @run_program`

If `@parse` changes:

```
Changed: @parse
Reverse closure: @compile, @run_program, @main
Tests to run:
  - @test_parse (direct)
  - @test_compile (calls @parse)
  - @test_run (calls @compile which calls @parse)
```

If `@optimize` changes:

```
Changed: @optimize
Reverse closure: @compile, @run_program, @main
Tests to run:
  - @test_optimize (direct)
  - @test_compile (calls @optimize)
  - @test_run (calls @compile)
```

### Integration with Incremental Compilation

The dependency graph already exists for incremental compilation. Test execution reuses it:

```
Compilation:
  source changed → recompile dependents (forward closure)

Testing:
  source changed → run tests for dependents (reverse closure)
```

Same graph, different traversal direction.

### CLI Interface

```bash
# Default: closure mode (changed + callers)
ori test

# Explicit modes
ori test --direct          # Only direct tests
ori test --closure         # Changed + callers (default)
ori test --full            # All tests

# Specify what changed (for CI integration)
ori test --changed=src/parser.ori
ori test --changed=@parse,@tokenize

# Show what would run without running
ori test --dry-run
```

### Output

```
$ ori test

Changes detected in: @parse

Running tests (closure mode):
  @test_parse ............ PASS (2ms)
  @test_compile .......... PASS (5ms)
  @test_run .............. PASS (8ms)

3 tests passed (15ms)
Skipped 47 unaffected tests
```

---

## Unit Tests vs Integration Tests

### Attached Tests (Unit Tests)

Tests with `tests @target` are unit tests:
- Attached to specific function
- Run when target or its callers change
- Should be fast (fully mocked via capabilities)
- Part of the dependency graph

```ori
@test_fetch_user tests @fetch_user () -> void =
    with Http = MockHttp(responses: {...}) in
    {...}
```

### Floating Tests (Integration Tests)

Tests without `tests @target` are integration tests:
- Not attached to specific function
- Run only in `--full` mode or explicitly
- May use real capabilities
- Not part of dependency closure

```ori
@test_end_to_end () -> void =
    with Http = RealHttp() in
    {
        let user = create_user(name: "Test")
        let fetched = fetch_user(id: user.id)
        assert_eq(fetched.name, "Test")
    }
```

### Execution Rules

| Test Type | Direct Mode | Closure Mode | Full Mode |
|-----------|-------------|--------------|-----------|
| Attached (changed target) | Run | Run | Run |
| Attached (caller of changed) | Skip | Run | Run |
| Attached (unaffected) | Skip | Skip | Run |
| Floating | Skip | Skip | Run |

---

## Implementation

### Compiler Changes

1. **Dependency Graph Extension**
   - Already tracks function → function dependencies
   - Add reverse lookup: function → callers
   - Index tests by their `tests @target` binding

2. **Test Discovery**
   - Parse `tests @target` bindings during compilation
   - Store in module metadata

3. **Closure Computation**
   - Given changed functions, compute reverse transitive closure
   - Filter to functions that have attached tests

### Data Structures

```rust
struct TestRegistry {
    // function -> tests that target it
    tests_for: HashMap<FunctionId, Vec<TestId>>,

    // function -> functions that call it
    callers: HashMap<FunctionId, Vec<FunctionId>>,
}

impl TestRegistry {
    fn tests_to_run(&self, changed: &[FunctionId]) -> Vec<TestId> {
        let affected = self.reverse_closure(changed);
        affected.iter()
            .flat_map(|f| self.tests_for.get(f))
            .flatten()
            .collect()
    }

    fn reverse_closure(&self, roots: &[FunctionId]) -> HashSet<FunctionId> {
        let mut result = HashSet::new();
        let mut queue: VecDeque<_> = roots.iter().collect();

        while let Some(func) = queue.pop_front() {
            if result.insert(*func) {
                if let Some(callers) = self.callers.get(func) {
                    queue.extend(callers);
                }
            }
        }

        result
    }
}
```

### Incremental Build Integration

The test runner integrates with incremental compilation:

1. Compiler detects changed source files
2. Compiler determines changed functions
3. Test runner computes closure
4. Test runner executes affected tests
5. Results cached for unchanged tests

---

## Performance Considerations

### Why This Is Fast

1. **Mocked tests** — Capabilities make unit tests fast (no I/O)
2. **Minimal set** — Only affected tests run
3. **Parallel execution** — Unrelated tests run concurrently
4. **Cached results** — Unchanged tests don't re-run

### Worst Case

In pathological cases (everything depends on a core function), closure mode approaches full mode. This is correct behavior — if you change a core function, you should test everything that uses it.

### Mitigation

- Core utilities should have thorough direct tests
- Changes to core functions are rare
- CI can run full mode; local dev uses closure mode

---

## Examples

### Development Workflow

```bash
# Edit @parse
$ vim src/parser.ori

# Run tests (closure mode by default)
$ ori test
Changes detected in: @parse
Running: @test_parse, @test_compile, @test_run
3 tests passed (15ms)

# Edit @fetch_user (leaf function)
$ vim src/api.ori

$ ori test
Changes detected in: @fetch_user
Running: @test_fetch_user
1 test passed (3ms)
```

### CI Workflow

```yaml
# .github/workflows/test.yml
jobs:
  test:
    steps:
      - uses: actions/checkout@v4
      - name: Run affected tests
        run: ori test --closure

      - name: Run full test suite
        run: ori test --full
```

---

## Future Extensions

### Test Impact Analysis

Show which tests would run for proposed changes:

```bash
$ ori test --analyze @parse
Changing @parse would trigger:
  - @test_parse (direct)
  - @test_compile (1 hop)
  - @test_run (2 hops)
  - @test_main (3 hops)
Total: 4 tests
```

### Smart Test Ordering

Run tests most likely to fail first:
1. Direct tests for changed function
2. Tests that failed recently
3. Tests closer in the dependency graph
4. Tests further away

### Coverage-Guided Closure

Extend closure based on actual code coverage data, not just static call graph.

---

## Summary

Dependency-aware test execution:

1. **Uses existing infrastructure** — Dependency graph already exists for incremental compilation
2. **Runs the right tests** — Not too few (missing failures), not too many (wasted time)
3. **Enables fast iteration** — Change, test, repeat in milliseconds
4. **Distinguishes test types** — Unit tests (attached) vs integration tests (floating)
5. **Makes mandatory testing practical** — Fast feedback makes requirements palatable

Combined with capabilities (trivial mocking) and mandatory testing (no untested code), this creates a system where code integrity is enforced automatically.
