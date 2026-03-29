# Section 23: Full Evaluator Support -- Verification Results

**Verified**: 2026-03-28
**Status in roadmap**: in-progress
**Actual status**: MOSTLY COMPLETE -- test counts are massively stale (roadmap says 1983 passed/31 skipped; actual is 4181 passed/42 skipped). Many issues listed as broken are now fixed. Several test files remain entirely commented out. Queue/Stack not implemented.

## Test Execution

- `cargo st`: 4181 passed, 0 failed, 42 skipped (global test suite)
- Individual file results:
  - `tests/spec/expressions/coalesce.ori`: 31 passed, 0 failed, 0 skipped
  - `tests/spec/expressions/operators_comparison.ori`: 61 passed, 0 failed, 0 skipped
  - `tests/spec/expressions/operators_bitwise.ori`: 43 passed, 0 failed, 0 skipped
  - `tests/spec/expressions/loops.ori`: 34 passed, 0 failed, 1 skipped
  - `tests/spec/expressions/index_access.ori`: 34 passed, 0 failed, 1 skipped
  - `tests/spec/declarations/traits.ori`: 30 passed, 0 failed, 0 skipped
  - `tests/spec/declarations/attributes.ori`: 26 passed, 0 failed, 1 skipped
  - `tests/spec/types/existential.ori`: 8 passed, 0 failed, 0 skipped
  - `tests/spec/types/function_types.ori`: NO TESTS FOUND (all commented out)
  - `tests/spec/expressions/literals.ori`: NO TESTS FOUND (all commented out)
  - `tests/spec/expressions/field_access.ori`: 30 passed, 0 failed, 0 skipped
  - `tests/spec/expressions/with_expr.ori`: 12 passed, 0 failed, 3 skipped
  - `tests/spec/declarations/clause_params.ori`: NO TESTS FOUND (all commented out)
  - `tests/spec/types/const_generics.ori`: NO TESTS FOUND (all skipped)
  - `tests/spec/declarations/functions.ori`: NO TESTS FOUND (all commented out)

---

## 23.1 Operators

### 23.1.1 Null Coalesce Operator (`??`)

STALE ROADMAP: Roadmap says "26/31 tests pass" but actual count is 31/31 passing, 0 skipped.

- [done] `??` operator evaluation -- 31 tests pass, 0 fail, 0 skip
  - File: `tests/spec/expressions/coalesce.ori`
  - Short-circuit: VERIFIED -- `Some(42) ?? panic(msg: ...)` does not panic
  - Option coalescing: VERIFIED -- Some returns inner, None returns default
  - Result coalescing: VERIFIED -- Ok returns inner, Err returns default
  - Chaining: VERIFIED -- `a ?? b ?? c ?? default` works for all None/Some patterns (tests at lines 105-175)
  - Type inference: VERIFIED -- Option<int>, Option<str>, Option<[int]> all work
  - Precedence: VERIFIED -- `??` has lowest precedence

- STALE STATUS COMMENT in coalesce.ori: Says "26/31 tests pass" and "3 chaining tests fail" and "2 map tests depend on Section 23.3.1" -- ALL 31 TESTS PASS NOW

- STALE ROADMAP: Claims "Known Limitation: Chaining with Option variables fails" -- this is no longer true, all chaining tests pass

### 23.1.2 Comparison Operators for Option/Result

- [done] `<`, `<=`, `>`, `>=` for Option types
  - File: `tests/spec/expressions/operators_comparison.ori`
  - 61 tests pass, 0 fail, 0 skip
  - VERIFIED: `None < Some(x)` ordering works

### 23.1.3 Struct Equality with `#derive(Eq)`

- [done] Equality operators for derived structs
  - File: `tests/spec/expressions/operators_comparison.ori`
  - VERIFIED: `#derive(Eq) type Point = { x: int, y: int }` with `==` and `!=` works

### 23.1.4 Shift Overflow Behavior

STALE ROADMAP: Marked `[ ]` as "Fix: Left shift overflow should panic" -- but it ALREADY WORKS.

- [done] Left shift overflow panics correctly
  - File: `tests/spec/expressions/operators_bitwise.ori`
  - 43 tests pass, 0 fail, 0 skip
  - VERIFIED: `assert_panics(f: () -> 1 << 63)` passes (line 189-193)
  - VERIFIED: `assert_panics(f: () -> 1 << 64)` passes (line 196-201)
  - VERIFIED: `assert_panics(f: () -> 1 << -1)` passes (line 203-208)
  - VERIFIED: `assert_panics(f: () -> 1 >> 64)` passes (line 241-246)
  - VERIFIED: `assert_panics(f: () -> 1 >> -1)` passes (line 248-253)

---

## 23.2 Primitive Trait Methods

### 23.2.1 Printable Trait (`.to_str()`)

- [done] `.to_str()` on all primitive types
  - File: `tests/spec/declarations/traits.ori` (30 passed, 0 failed)
  - File: `tests/spec/types/existential.ori` (8 passed, 0 failed)
  - VERIFIED: int, str, bool, float all work

### 23.2.2 Clone Trait (`.clone()`)

- [done] `.clone()` on all primitive types
  - VERIFIED: Tests pass in traits.ori and existential.ori

### 23.2.3 Hashable Trait (`.hash()`)

- [done] `.hash()` on primitive types
  - VERIFIED: Tests pass in traits.ori

---

## 23.3 Type Coercion and Indexing

### 23.3.1 Map Index Return Type

STALE ROADMAP AND STATUS COMMENTS: Says "map returns value, not Option" but tests show map indexing returns `Option<V>`.

- [done] Map lookup works for existing keys
  - File: `tests/spec/expressions/index_access.ori` (34 passed, 1 skipped)
  - VERIFIED: `map["a"] ?? 0` works (line 128)
  - VERIFIED: `is_none(opt: map["missing"])` works (line 137)
  - VERIFIED: Empty map returns None (line 143-150)

- STALE STATUS at index_access.ori:122: Says "Evaluator [BROKEN] - map returns value, not Option" -- this is now fixed, map indexing returns `Option<V>`

### 23.3.2 Map Non-String Keys

- [todo] Status unclear from tests
  - No active test for non-string map keys
  - Tests in `literals.ori` are all commented out
  - Tests in `map_types.ori` have some commented-out items referencing this

### 23.3.3 String Index Return Type

STALE STATUS COMMENT: Says "returns char, should return str" but tests PASS with `expected: "h"`.

- [done] String indexing works and returns `str`
  - VERIFIED: `"hello"[0]` returns `"h"` (str, not char) -- test passes at line 208
  - VERIFIED: `s[# - 1]` returns `"o"` -- test passes at line 215
  - STALE STATUS at index_access.ori:203: Says "Evaluator [BROKEN] - returns char, should return str" -- this is now fixed

### 23.3.4 List Index Assignment

- [partial] `list[i] = value` syntax
  - One test exists but is SKIPPED: `#skip("index assignment not supported - pending design proposal")` at line 359
  - NOTE: The roadmap claims "Verified: works" but the test file has it skipped

---

## 23.4 Control Flow

### 23.4.1 Break with Value in Nested Loops

- [partial] `break value` works in simple cases but labeled breaks not implemented
  - VERIFIED: `loop { break 42 }` returns 42 (test at line 391, passes)
  - SKIPPED: `break x` inside `for` inside `loop` requires labeled breaks -- `#skip` at line 405
  - The roadmap describes this as "Returns 0 instead of break value" but the ACTUAL issue is about labeled breaks across nested loop boundaries, not basic break-value propagation

### 23.4.2 Function Field Calls

- [todo] Calling function stored in struct field
  - File: `tests/spec/types/function_types.ori`
  - ALL tests in this file are COMMENTED OUT (NO TESTS FOUND)
  - Comment at line 286: "Evaluator does not support calling function fields with method syntax (h.callback(x))"
  - Cannot verify whether bug still exists since no test is active
  - NOTE: Roadmap says "compiler crash (type_interner.rs:226)" -- unable to verify current status

---

## 23.5 Derived Traits

### 23.5.1 `#derive(Eq)` Implementation

- [done] Generated equality for structs
  - VERIFIED: `tests/spec/expressions/operators_comparison.ori` -- 61 tests pass
  - Tests include Point struct with `==` and `!=`

### 23.5.2 `#derive(Clone)` Implementation

- [done] Generated clone for structs
  - VERIFIED: `tests/spec/declarations/attributes.ori` -- 26 tests pass, 1 skipped
  - The 1 skip is for a pending feature, not clone

### 23.5.3 `#derive(Hashable)` Implementation

- [done] Generated hash for structs
  - VERIFIED: `tests/spec/declarations/attributes.ori` includes hash tests

---

## 23.6 Stdlib Types and Methods

### 23.6.1 Queue Type

- [todo] NOT implemented
  - Only listed as future design in `library/std/collections/mod.ori`
  - No Queue type in evaluator or stdlib

### 23.6.2 Stack Type

- [todo] NOT implemented
  - Not in evaluator or stdlib

### 23.6.3 String Slice

- [partial] `str.substring(start:, end:)` method exists (in prelude)
  - `str[start..end]` syntax -- unclear if implemented
  - No specific test file for string slicing

### 23.6.4 Stdlib Utilities

- [todo] `retry`/`validate` NOT implemented
  - Not in evaluator or stdlib

### 23.6.5 Async/Future Support

- [todo] NOT implemented
  - No async/await in evaluator

---

## 23.8 Parser Feature Support (Type Checker/Evaluator)

### 23.8.1 Guard Clauses

- [todo] Parser support exists but type checker and evaluator not implemented
  - `tests/spec/declarations/clause_params.ori` -- NO TESTS FOUND (all commented out)
  - Cannot verify guard clause behavior

### 23.8.2 List Patterns in Function Parameters

- [todo] Parser support exists but type checker and evaluator not implemented
  - `tests/spec/declarations/clause_params.ori` -- NO TESTS FOUND (all commented out)

### 23.8.3 Const Generics

- [todo] Parser support exists but type checker and evaluator not implemented
  - `tests/spec/types/const_generics.ori` -- NO TESTS FOUND (all 6 are `#skip`)
  - Skip reasons: "Array type not yet implemented", "fixed-capacity lists not yet implemented", "const bounds not yet implemented"

### 23.8.4 Variadic Parameters

- [todo] Parser support exists but evaluator not implemented
  - `tests/spec/declarations/variadic_params.ori` does NOT exist
  - `tests/spec/declarations/functions.ori` has variadic tests but all COMMENTED OUT
  - Comments say "variadic function calling not implemented in evaluator"

### 23.8.5 Function-Level Contract Enforcement (`pre()`/`post()`)

- [todo] Parser support exists but type checker and evaluator not implemented
  - No test file with `pre()` or `post()` assertions found in `tests/`
  - Roadmap references `tests/spec/patterns/run.ori` lines 140-288 but those lines contain NO contract tests

### 23.8.6 Spread in Function Calls

- [todo] Type checker and evaluator support not implemented
  - Tests in `tests/spec/expressions/function_calls.ori` may exist but depend on variadic implementation

---

## 23.7 Section Completion Checklist

STALE ROADMAP DATA: Says "1983 passed, 0 failed, 31 skipped" -- actual is 4181 passed, 0 failed, 42 skipped.

- [done] All operator evaluations (23.1) -- `??`, comparisons, equality, shift overflow ALL work
- [done] All primitive trait methods (23.2) -- `.to_str()`, `.clone()`, `.hash()` work
- [done] Most indexing behaviors (23.3) -- map returns `Option<V>`, string indexing returns `str`
- [partial] Control flow (23.4) -- basic break value works; labeled breaks not implemented
- [done] All derived traits (23.5) -- `#derive(Eq, Clone, Hashable)` all work
- [todo] Stdlib types (23.6) -- Queue/Stack not implemented
- [todo] Parser features (23.8) -- guard clauses, list patterns, const generics, variadics, contracts, spread all have parser support but no typechecker/evaluator implementation

---

## Findings Summary

### Items marked `[ ]` that should be `[x]` (STALE):

1. `??` operator (23.1.1) -- ALL 31 tests pass, not 26/31
2. Comparison operators for Option (23.1.2) -- works, 61 tests pass
3. Struct equality with `#derive(Eq)` (23.1.3) -- works
4. Shift overflow (23.1.4) -- panic behavior is CORRECT, all 43 bitwise tests pass
5. All primitive trait methods (23.2.1-23.2.3) -- all work
6. Map index return type (23.3.1) -- returns `Option<V>`, not raw value
7. String index return type (23.3.3) -- returns `str`, not `char`
8. All derived traits (23.5.1-23.5.3) -- all work

### STALE STATUS COMMENTS IN TEST FILES:

1. `coalesce.ori` line 4-9: Says "26/31 tests pass" and lists 5 failures -- ALL 31 PASS NOW
2. `index_access.ori` line 122: Says "Evaluator [BROKEN] - map returns value, not Option" -- FIXED
3. `index_access.ori` line 203: Says "Evaluator [BROKEN] - returns char, should return str" -- FIXED

### STALE ROADMAP DATA:

1. Test count "1983 passed, 31 skipped" at multiple locations -- actual is 4181 passed, 42 skipped
2. "Remaining Issues (verified 2026-02-04)" section lists 4 issues -- at least 3 of 4 are fixed
3. Section 23.1.4 claims shift overflow is broken -- it works correctly
4. Section 23.3.1 claims map lookup is broken -- it works correctly
5. Section 23.3.3 claims string indexing is broken -- it works correctly
6. Section 23.1.1 claims chaining fails -- it works correctly

### ITEMS GENUINELY NOT IMPLEMENTED:

1. Queue/Stack types (23.6.1, 23.6.2) -- no implementation
2. `retry`/`validate` stdlib utilities (23.6.4) -- no implementation
3. Async/Future support (23.6.5) -- no implementation
4. Guard clauses type checker/evaluator (23.8.1) -- parser only
5. List patterns type checker/evaluator (23.8.2) -- parser only
6. Const generics type checker/evaluator (23.8.3) -- parser only
7. Variadic parameters evaluator (23.8.4) -- parser only
8. Contract enforcement `pre()`/`post()` (23.8.5) -- parser only
9. Spread in function calls (23.8.6) -- parser only
10. Map non-string keys (23.3.2) -- no active tests
11. Function field calls (23.4.2) -- all tests commented out, status unclear
12. List index assignment (23.3.4) -- skipped test

### COMMENTED-OUT TEST FILES (cannot verify):

1. `tests/spec/types/function_types.ori` -- ALL tests commented out
2. `tests/spec/expressions/literals.ori` -- ALL tests commented out
3. `tests/spec/declarations/clause_params.ori` -- ALL tests commented out
4. `tests/spec/declarations/functions.ori` -- ALL tests commented out (variadic section)

### BUG FOUND: None new (existing issues are accurately tracked for genuinely unimplemented features)

### MATRIX COVERAGE ASSESSMENT:

- Operators: GOOD -- bitwise has 43 tests including overflow/sign/shift edge cases; comparison has 61 tests; coalesce has 31 tests
- Primitive traits: ADEQUATE -- tested via multiple test files but no systematic matrix
- Indexing: GOOD for lists and maps (including nested, variables, bounds); string indexing adequate
- Control flow: ADEQUATE for basic break/continue but labeled breaks not tested (skipped)
- Derived traits: ADEQUATE -- Eq, Clone, Hashable all tested but not with complex nested types
- Parser features (23.8): NO TESTS -- all commented out or skipped

### SEMANTIC PINS:

- Shift overflow: `assert_panics(f: () -> 1 << 63)` is a strong semantic pin
- Map indexing: `is_none(opt: map["missing"])` pins Option return type
- Coalesce short-circuit: `opt ?? panic(msg: ...)` pins short-circuit behavior
- Option ordering: `None < Some(x)` pins comparison semantics

### RISK AREAS:

1. 4 test files entirely commented out means significant spec behavior is UNVERIFIED
2. Function field calls may still crash but cannot be confirmed (tests commented out)
3. No contract tests exist anywhere -- `pre()`/`post()` semantics entirely untested
4. Variadic parameter behavior entirely untested
