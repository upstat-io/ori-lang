# Proposal: Parameterized and Property-Based Testing

**Status:** Draft
**Author:** Eric (with Claude)
**Created:** 2026-01-30

---

## Summary

Add parameterized and property-based testing to Ori via the `#test` attribute. Tests can declare parameters that are supplied by data lists, generators, or exhaustive enumeration.

```ori
#test(params: for limit in [0, 50, 100] yield
              for items in [[], small(), large()] yield
                  (limit, items))
@test_process tests _ (limit: int, items: [Item]) -> void =
    assert(condition: len(collection: process(items: items, limit: limit)) <= limit)
```

Key design decisions:
- Uses existing Ori expression syntax inside attributes (lambdas, `for...yield`)
- Only allowed on floating tests (`tests _`)
- Compile error if test parameters don't match attribute-provided values
- Desugars to wrapped test execution

---

## Motivation

### The Problem

Testing configurable behavior is painful. Consider:

```ori
@process_batch (items: [Item], limit: int) -> [Result] = ...
```

To test this thoroughly, you need to verify behavior across many `limit` values:
- `limit = 0` (edge case: nothing processed)
- `limit = 1` (edge case: single item)
- `limit = 50` (typical case)
- `limit = 1000` (large case)

Without parameterized testing, you either:
1. **Copy-paste tests** — Tedious, error-prone, hard to maintain
2. **Write a loop inside a test** — Poor failure reporting, all-or-nothing
3. **Skip thorough testing** — Bugs slip through

### The Solution

Parameterized testing separates test data from test logic:

```ori
#test(params: for limit in [0, 1, 50, 1000] yield (limit,))
@test_process tests _ (limit: int) -> void =
    assert(condition: process(items: sample_items(), limit: limit).is_valid())
```

One test definition, four executions, clear failure reporting per case.

### Why Floating Tests Only

Attached tests verify specific functions and run during compilation. They should be:
- **Fast** — No time for extensive parameterization
- **Deterministic** — Same inputs every time
- **Focused** — Test the specific contract

Parameterized and property-based tests are for thorough exploration:
- **Comprehensive** — Cover input space systematically
- **Slower** — May run many iterations
- **Explicit** — Run via `ori test`, not during compilation

This aligns with Ori's testing philosophy: attached tests for continuous verification, floating tests for deep exploration.

---

## Design

### The `#test` Attribute

The `#test` attribute configures test execution. It accepts these fields:

| Field | Type | Description |
|-------|------|-------------|
| `params` | Iterator expression | Parameterized test data |
| `property` | Generator lambda | Property-based testing |
| `exhaustive` | Tuple of types/values | Exhaustive enumeration |
| `runs` | int | Iterations for property tests (default: 100) |
| `seed` | int | RNG seed for reproducibility |
| `shrink` | bool | Enable shrinking on failure (default: true) |

Only one of `params`, `property`, or `exhaustive` may be specified.

### Parameterized Testing: `params`

The `params` field accepts any Ori expression that yields an iterator of tuples:

```ori
// Simple list
#test(params: [(0,), (50,), (100,)])
@test_limit tests _ (limit: int) -> void = ...

// For comprehension
#test(params: for x in [1, 2, 3] yield for y in [10, 20] yield (x, y))
@test_multiply tests _ (x: int, y: int) -> void = ...

// With filtering
#test(params: for a in [-10, 0, 10] yield
              for b in [-5, 0, 5] yield
              if b != 0 then (a, b))
@test_divide tests _ (a: int, b: int) -> void = ...

// Using helper functions
#test(params: generate_edge_cases())
@test_edge tests _ (input: str, expected: Result<Ast, Error>) -> void = ...
```

**Semantics:**

The expression is evaluated at test discovery time. Each yielded tuple becomes a test case. The test function runs once per tuple.

**Tuple Matching:**

The tuple elements must match the test function parameters by position:

```ori
#test(params: [(1, "a"), (2, "b")])
@test_example tests _ (n: int, s: str) -> void = ...
//                     ^^^^^   ^^^^^
//                     matches (int, str) tuple
```

### Property-Based Testing: `property`

The `property` field accepts a generator lambda that receives an RNG and returns a tuple:

```ori
#test(property: rng -> (
    x: rng.int(min: 0, max: 1000),
    y: rng.int(min: 0, max: 1000),
), runs: 100)
@test_commutative tests _ (x: int, y: int) -> void =
    assert_eq(actual: add(a: x, b: y), expected: add(a: y, b: x))
```

**Semantics:**

The generator runs `runs` times (default: 100) with different RNG states. Each generated tuple becomes a test case.

**Generator Combinators:**

Standard combinators are available on the `Rng` type:

```ori
rng.int(min: int, max: int) -> int
rng.float(min: float, max: float) -> float
rng.bool() -> bool
rng.char() -> char
rng.str(max_len: int) -> str
rng.list<T>(gen: Rng -> T, max_len: int) -> [T]
rng.option<T>(gen: Rng -> T) -> Option<T>
rng.one_of<T>(values: [T]) -> T
```

**Using `Arbitrary` trait:**

Types implementing `Arbitrary` can be generated directly:

```ori
#test(property: rng -> (
    item: Item.arbitrary(rng: rng),
    count: rng.int(min: 0, max: 100),
))
@test_process tests _ (item: Item, count: int) -> void = ...
```

### Exhaustive Testing: `exhaustive`

The `exhaustive` field enumerates all combinations:

```ori
#test(exhaustive: (mode: Mode, priority: Priority))
@test_all_modes tests _ (mode: Mode, priority: Priority) -> void = ...
```

For sum types, all variants are enumerated. For other types, explicit values must be provided:

```ori
#test(exhaustive: (
    mode: Mode,                    // Sum type: auto-enumerate variants
    count: [0, 1, 10, 100],        // Explicit values
    enabled: bool,                 // Bool: [false, true]
))
@test_combinations tests _ (mode: Mode, count: int, enabled: bool) -> void = ...
```

**Semantics:**

Computes cartesian product of all value sets. Test runs once per combination.

**Constraints:**

The compiler emits an error if the cartesian product exceeds a threshold (default: 10,000):

```
error: exhaustive test generates 50,000 combinations (limit: 10,000)
  --> src/test.ori:5:1
   |
 5 | #test(exhaustive: (a: [1..100], b: [1..100], c: [1..5]))
   | ^^^^^^^^^^^^^^^^^ too many combinations
   |
   = help: use #test(params: ...) with explicit cases instead
   = help: increase limit with #test(exhaustive: ..., max_combinations: 50000)
```

### The `Arbitrary` Trait

Types can implement `Arbitrary` to support property-based generation:

```ori
trait Arbitrary {
    // Generate a random value
    @arbitrary (rng: Rng) -> Self

    // Return simpler versions for shrinking (optional)
    @shrink (self) -> [Self] = []
}
```

**Built-in Implementations:**

```ori
impl Arbitrary for int {
    @arbitrary (rng: Rng) -> int = rng.int(min: int.MIN, max: int.MAX)
    @shrink (self) -> [int] = match self {
        0 -> []
        n if n > 0 -> [0, n / 2, n - 1]
        n -> [0, n / 2, n + 1]
    }
}

impl Arbitrary for bool {
    @arbitrary (rng: Rng) -> bool = rng.bool()
    @shrink (self) -> [bool] = if self then [false] else []
}

impl Arbitrary for str {
    @arbitrary (rng: Rng) -> str = rng.str(max_len: 100)
    @shrink (self) -> [str] = {
        let chars = self.chars();
        if is_empty(collection: chars) then []
        else [
            "",
            self.take(n: len(collection: self) / 2),
            self.drop(n: 1),
        ]
    }
}
```

**Deriving:**

Sum types can derive `Arbitrary`:

```ori
#derive(Arbitrary)
type Priority = Low | Medium | High

// Generates: randomly selects one variant
```

Structs with `Arbitrary` fields can derive it:

```ori
#derive(Arbitrary)
type Item = {
    id: int,
    name: str,
    priority: Priority,
}

// Generates: Item with arbitrary id, name, priority
```

### Shrinking

When a property test fails, the framework attempts to find a minimal failing case:

```
FAIL @test_sort
  Property failed after 47 runs.

  Original failing input:
    items = [583, -2941, 0, 17, -88, 42, 999, -1]

  Shrunk to minimal case (12 shrink steps):
    items = [1, -1]

  Assertion failed: expected sorted list
    at src/sort.test.ori:15:5
```

Shrinking uses the `shrink` method from `Arbitrary` to generate simpler inputs, then re-runs the test to verify failure persists.

### Seed and Reproducibility

Property tests use a random seed. On failure, the seed is reported:

```
FAIL @test_property (seed: 0x1a2b3c4d)
  ...
```

To reproduce:

```ori
#test(property: rng -> ..., seed: 0x1a2b3c4d)
```

By default, seed is derived from test name for deterministic CI behavior.

---

## Compile-Time Validation

### Parameter Matching

The compiler verifies that test parameters match the provided data:

```ori
#test(params: [(1, "a")])
@test_example tests _ (n: int) -> void = ...
//                     ^^^^^^
// error: test has 1 parameter but #test(params:) provides 2-element tuples
```

```ori
#test(params: [(1, "a")])
@test_example tests _ (n: int, s: int) -> void = ...
//                             ^^^^^^
// error: parameter `s` has type `int` but receives `str` from #test(params:)
```

### Floating Test Requirement

```ori
#test(params: [...])
@test_example tests @some_fn (x: int) -> void = ...
//            ^^^^^^^^^^^^^
// error: #test(params:) is only allowed on floating tests (use `tests _`)
```

### Expression Evaluation

The `params`, `property`, and `exhaustive` expressions must be evaluable at test discovery time. This means:
- No capabilities (no `uses Http`)
- No runtime-only values
- Const functions are allowed

---

## Desugaring

### Parameterized Tests

```ori
#test(params: [(0,), (50,), (100,)])
@test_limit tests _ (limit: int) -> void =
    assert(condition: process(limit: limit).is_valid())
```

Desugars to:

```ori
@test_limit tests _ () -> void = {
    let params = [(0,), (50,), (100,)];
    for (i, (limit,)) in params.enumerate() do
        {
            // Original test body
            assert(condition: process(limit: limit).is_valid())
        } |> catch_and_report(case: i, params: (limit,))
}
```

Or alternatively, generates separate test cases for better reporting:

```ori
@test_limit_0 tests _ () -> void = { let limit = 0; ... }
@test_limit_1 tests _ () -> void = { let limit = 50; ... }
@test_limit_2 tests _ () -> void = { let limit = 100; ... }
```

### Property Tests

```ori
#test(property: rng -> (x: rng.int(min: 0, max: 100)), runs: 100)
@test_prop tests _ (x: int) -> void =
    assert(condition: x >= 0)
```

Desugars to:

```ori
@test_prop tests _ () -> void = {
    let gen = rng -> (x: rng.int(min: 0, max: 100));
    let base_seed = hash("test_prop");
    for i in 0..100 do
        {
            let rng = Rng.seeded(seed: base_seed + i);
            let (x,) = gen(rng);
            assert(condition: x >= 0)
        } |> on_fail(shrink_and_report(gen:, seed: base_seed + i))
}
```

---

## Test Output

### Parameterized Test Output

```
Running @test_limit...
  [0/3] limit=0 ✓
  [1/3] limit=50 ✓
  [2/3] limit=100 ✗

FAIL @test_limit [2/3] (limit=100)
  assertion failed: expected valid result
    at src/test.ori:5:5
```

### Property Test Output

```
Running @test_sort...
  100 cases ✓

Running @test_commutative...
  FAIL after 47 cases

FAIL @test_commutative (seed: 0x1a2b3c4d)
  Original: (x=583, y=-2941)
  Shrunk:   (x=1, y=-1)

  assertion failed: expected equal
    actual:   0
    expected: 2
    at src/test.ori:12:5
```

### Exhaustive Test Output

```
Running @test_modes...
  9/9 combinations ✓
```

---

## CLI Integration

### Filtering Parameterized Tests

```bash
# Run all cases
ori test --filter "test_limit"

# Run specific case (by index)
ori test --filter "test_limit[2]"

# Run cases matching pattern
ori test --filter "test_limit[limit=100]"
```

### Property Test Options

```bash
# Override runs
ori test --property-runs 1000

# Set seed for reproducibility
ori test --property-seed 0x1a2b3c4d

# Disable shrinking (faster, less helpful)
ori test --no-shrink
```

---

## Examples

### Testing Edge Cases Systematically

```ori
#test(params: [
    (input: "", expected: Ok([])),
    (input: "a", expected: Ok(["a"])),
    (input: "a,b", expected: Ok(["a", "b"])),
    (input: "a,,b", expected: Err("empty element")),
    (input: ",", expected: Err("empty element")),
])
@test_parse_csv tests _ (input: str, expected: Result<[str], str>) -> void =
    assert_eq(actual: parse_csv(input: input), expected: expected)
```

### Property: Roundtrip

```ori
#test(property: rng -> (ast: Ast.arbitrary(rng: rng)), runs: 500)
@test_format_roundtrip tests _ (ast: Ast) -> void = {
    let formatted = format(ast: ast);
    let parsed = parse(input: formatted);
    assert_eq(actual: parsed, expected: Ok(ast))
}
```

### Property: Invariant

```ori
#test(property: rng -> (
    items: rng.list(gen: int.arbitrary, max_len: 100),
))
@test_sort_invariants tests _ (items: [int]) -> void = {
    let sorted = sort(items: items);

    // Length preserved
    assert_eq(actual: len(collection: sorted), expected: len(collection: items));

    // Elements preserved
    assert_eq(actual: sorted.to_set(), expected: items.to_set());

    // Actually sorted
    assert(condition: sorted.windows(size: 2).all(predicate: w -> w[0] <= w[1]))
}
```

### Exhaustive: State Machine

```ori
type State = Idle | Running | Paused | Stopped

#test(exhaustive: (from: State, event: Event))
@test_transitions tests _ (from: State, event: Event) -> void = {
    let to = transition(state: from, event: event);
    assert(condition: is_valid_transition(from: from, to: to, event: event))
}
```

---

## Alternatives Considered

### 1. Separate `@parameterized` Decorator

```ori
@parameterized([(0,), (50,), (100,)])
@test tests _ (limit: int) -> void = ...
```

Rejected: Adds another attribute. `#test(params:)` consolidates all test configuration.

### 2. Special Test Syntax

```ori
@test tests _ for limit in [0, 50, 100] do (limit: int) -> void = ...
```

Rejected: Requires new syntax. Attributes with Ori expressions are more consistent.

### 3. Allow on Attached Tests

Rejected: Attached tests run during compilation and should be fast and deterministic. Parameterized/property tests are exploratory.

---

## Implementation Plan

### Phase 1: `#test(params:)`
- Parse `#test` attribute with `params` field
- Validate parameter matching at compile time
- Implement desugaring to loop or multiple tests
- Update test runner output

### Phase 2: `#test(exhaustive:)`
- Implement sum type variant enumeration
- Implement cartesian product computation
- Add combination limit checking

### Phase 3: `Arbitrary` Trait
- Define `Arbitrary` trait in prelude
- Implement for primitives
- Implement `#derive(Arbitrary)`
- Add `Rng` type with combinators

### Phase 4: `#test(property:)`
- Implement property test execution
- Implement shrinking algorithm
- Implement seed handling and reproducibility
- Update test runner output

### Phase 5: CLI Integration
- Add `--property-runs`, `--property-seed`, `--no-shrink` flags
- Add case filtering for parameterized tests

---

## Summary

This proposal adds three testing modes to Ori:

| Mode | Attribute | Use Case |
|------|-----------|----------|
| Parameterized | `#test(params: ...)` | Test with explicit data sets |
| Property | `#test(property: ...)` | Test invariants with generated data |
| Exhaustive | `#test(exhaustive: ...)` | Test all combinations of small domains |

All use existing Ori expression syntax inside attributes. All are restricted to floating tests. All provide clear failure reporting with parameter values.

Combined with attached tests for compilation-time verification, this gives Ori a comprehensive testing story: fast continuous verification plus thorough exploration.
