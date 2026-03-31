---
section: 23
title: Full Evaluator Support
status: in-progress
reviewed: true
last_verified: "2026-03-29"
tier: 0
goal: Complete evaluator support for entire Ori spec semantics
spec:
  - spec/grammar.ebnf
  - spec/08-types.md
  - spec/10-declarations.md
  - spec/14-expressions.md
  - spec/15-patterns.md
  - spec/09-properties-of-types.md
sections:
  - id: "23.1"
    title: Operators
    status: done
  - id: "23.2"
    title: Primitive Trait Methods
    status: done
  - id: "23.3"
    title: Type Coercion and Indexing
    status: in-progress
  - id: "23.4"
    title: Control Flow
    status: in-progress
  - id: "23.5"
    title: Derived Traits
    status: done
  - id: "23.6"
    title: Stdlib Types and Methods
    status: not-started
  - id: "23.7"
    title: Section Completion Checklist
    status: not-started
  - id: "23.8"
    title: Parser Feature Support (Type Checker/Evaluator)
    status: in-progress
---

# Section 23: Full Evaluator Support

**Goal**: Complete evaluator support for entire Ori spec semantics (parsing assumed working — see Section 0)

> **SPEC**: `spec/grammar.ebnf` (authoritative), `spec/08-types.md`, `spec/14-expressions.md`, `spec/09-properties-of-types.md`

**Status**: In Progress — Most features work! 4181 tests pass, 42 skipped (verified 2026-03-29). Only a few actual bugs remain.

---

## OVERVIEW

This section ensures the evaluator (interpreter) correctly implements all Ori language semantics. It assumes the parser works correctly (Section 0). The evaluator is in `compiler/ori_eval/`.

**Why this matters**: The evaluator is the reference implementation for Ori semantics. It must correctly implement every language feature before LLVM codegen can be validated against it.

**Approach**:
1. Audit current evaluator against spec semantics
2. Implement missing features
3. Fix incorrect behaviors
4. Validate with spec tests

---

## 23.1 Operators

> **SPEC**: `spec/14-expressions.md` § Operators

### 23.1.1 Null Coalesce Operator (`??`)

> **Test Status**: ALL PASSING — 31/31 tests pass (verified 2026-03-29)
> WEAK TESTS: No `#compile_fail` negative pins for invalid `??` usage (e.g., `int ?? int`)
> STALE TEST COMMENTS: `coalesce.ori` header says "26/31 pass" — all 31/31 pass

- [x] **Implement**: `??` operator evaluation — **31/31 tests pass** [done] (verified 2026-03-29)
  - [x] **Location**: `ori_eval/src/interpreter/mod.rs` — short-circuit logic in `eval_binary`
  - [x] **Semantics**: `Option<T> ?? T -> T` — return inner value if Some, else right operand
  - [x] **Semantics**: `Result<T, E> ?? T -> T` — return inner value if Ok, else right operand
  - [x] **Short-circuit**: Right operand is NOT evaluated if left is Some/Ok
  - [x] **Chaining**: `a ?? b ?? c ?? default` works for all None/Some patterns
  - [x] **Ori Tests**: `tests/spec/expressions/coalesce.ori` — 31 passed, 0 failed

### 23.1.2 Comparison Operators for Option/Result

- [x] **Implement**: `<`, `<=`, `>`, `>=` for Option types [done] (verified 2026-03-29)
  - [x] **Spec**: `None < Some(x)` for all x — works correctly
  - [x] **Verified**: 61 tests pass in `tests/spec/expressions/operators_comparison.ori`
  - [x] **Ori Tests**: `tests/spec/expressions/operators_comparison.ori`

> WEAK TESTS: No `#compile_fail` for comparing incompatible types
> STALE TEST COMMENTS: `operators_comparison.ori` lines 365, 456 say `STATUS: Evaluator [BROKEN]` for Option ordering and struct equality — both work

### 23.1.3 Struct Equality with `#derive(Eq)`

- [x] **Fix**: Equality operators for derived structs [done] (verified 2026-03-29)
  - [x] **Verified**: `#derive(Eq) type Point = { x: int, y: int }` with `p1 == p2` works
  - [x] **Ori Tests**: `tests/spec/expressions/operators_comparison.ori` — 61 tests pass

### 23.1.4 Shift Overflow Behavior

- [x] **Fix**: Left shift overflow panics correctly [done] (verified 2026-03-29)
  - [x] **Spec**: `1 << 63` panics due to overflow
  - [x] **Verified**: `assert_panics(f: () -> 1 << 63)`, `assert_panics(f: () -> 1 << 64)`, `assert_panics(f: () -> 1 << -1)` all pass
  - [x] **Ori Tests**: `tests/spec/expressions/operators_bitwise.ori` — 43 tests pass

---

## 23.2 Primitive Trait Methods

> **SPEC**: `spec/09-properties-of-types.md` § Built-in Traits
> **STATUS**: ALL IMPLEMENTED (verified 2026-03-29)

Primitives (int, str, bool, float, etc.) implement standard trait methods.

### 23.2.1 Printable Trait (`.to_str()`)

- [x] **Implement**: `.to_str()` on primitive types [done] (verified 2026-03-29)
  - [x] `int.to_str()` — Works: `42.to_str() == "42"`
  - [x] `str.to_str()` — Works
  - [x] `bool.to_str()` — Works: `true.to_str() == "true"`
  - [x] `float.to_str()` — Works
  - [x] **Ori Tests**: `tests/spec/declarations/traits.ori` (30 passed), `tests/spec/types/existential.ori` (8 passed)

### 23.2.2 Clone Trait (`.clone()`)

- [x] **Implement**: `.clone()` on primitive types [done] (verified 2026-03-29)
  - [x] `int.clone()` — Works: `let y = x.clone()`
  - [x] `str.clone()` — Works
  - [x] All primitives are cloneable
  - [x] **Ori Tests**: `tests/spec/declarations/traits.ori`, `tests/spec/types/existential.ori`

### 23.2.3 Hashable Trait (`.hash()`)

- [x] **Implement**: `.hash()` on primitive types [done] (verified 2026-03-29)
  - [x] `int.hash()` — Works
  - [x] `str.hash()` — Works
  - [x] **Ori Tests**: `tests/spec/declarations/traits.ori`

---

## 23.3 Type Coercion and Indexing

> **SPEC**: `spec/14-expressions.md` § Index Access
> **STATUS**: Mostly complete — map returns Option<V>, string indexing returns str, non-string keys work (verified 2026-03-29)
> STALE TEST COMMENTS: `index_access.ori` lines 122-124 say map returns value not Option (fixed); lines 203-204 say returns char not str (fixed)

### 23.3.1 Map Index Return Type

- [x] **Fix**: Map lookup returns `Option<V>` per spec [done] (verified 2026-03-29)
  - [x] **Verified**: `map["a"] ?? 0` works; `is_none(opt: map["missing"])` works; empty map returns None
  - [x] **Ori Tests**: `tests/spec/expressions/index_access.ori` — 34 passed, 1 skipped

### 23.3.2 Map Non-String Keys

- [x] **Fix**: Allow non-string map keys [done] (verified 2026-03-29)
  - [x] **Verified**: `{1: "one", 2: "two"}` works; `map[1] ?? "none"` returns `"one"`; int key None detection works
  - [x] **Ori Tests**: `tests/spec/expressions/index_access.ori` — `test_map_int_key` passes

### 23.3.3 String Index Return Type

- [x] **Fix**: String indexing returns `str` per spec [done] (verified 2026-03-29)
  - [x] **Verified**: `"hello"[0]` returns `"h"` (str, not char); `s[# - 1]` returns `"o"`
  - [x] **Ori Tests**: `tests/spec/expressions/index_access.ori`

### 23.3.4 List Index Assignment

- [ ] **Implement**: `list[i] = value` syntax
  - [ ] **Error**: `E6080: index assignment (list[i] = x) is not supported; use functional update patterns instead` (verified 2026-03-29)
  - [ ] **Note**: Needs design decision — current error message suggests functional updates
  - [ ] **Ori Tests**: `tests/spec/expressions/index_access.ori`

---

## 23.4 Control Flow

> **SPEC**: `spec/14-expressions.md` § Control Flow
> **STATUS**: Partial — function field calls work; labeled breaks remain (verified 2026-03-29)

### 23.4.1 Labeled Break with Value in Nested Loops

- [ ] **Implement**: Labeled `break:name value` for nested loops
  - [ ] **Note**: Unlabeled `break` correctly breaks innermost loop only (not a bug). The original description ("break value not propagating") was misleading — this is a missing feature (labeled breaks), not broken behavior. (verified 2026-03-29)
  - [ ] **Required**: `loop:name`, `break:name value`, `continue:name` support in evaluator
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori` — 1 skip: `#skip("requires labeled breaks (loop:name, break:name)")`

### 23.4.2 Function Field Calls

- [x] **Implement**: Calling function stored in struct field [done] (verified 2026-03-29)
  - [x] **Syntax**: `handler.callback(42)` where `callback: (int) -> str` — works correctly
  - [x] **Verified**: Both `ori check` and `ori run` succeed; no crash. Output correct.
  - [x] **Ori Tests**: `tests/spec/types/function_types.ori`

> DEAD TEST CODE: `tests/spec/types/function_types.ori` has ALL 355 lines commented out. Function field calls now work; these tests should be revived.

---

## 23.5 Derived Traits

> **SPEC**: `spec/10-declarations.md` § Attributes
> **STATUS**: ALL IMPLEMENTED (verified 2026-03-29)
> NEEDS TESTS: Only 3 of 7 derivable traits tested (Eq, Clone, Hashable). Missing: Printable, Debug, Default, Comparable.

### 23.5.1 `#derive(Eq)` Implementation

- [x] **Fix**: Generated equality for structs [done] (verified 2026-03-29)
  - [x] Compares all fields correctly
  - [x] Works with `==` and `!=` operators
  - [x] **Verified**: `#derive(Eq) type Point = {...}; assert(eq: p1 == p2)` works
  - [x] **Ori Tests**: `tests/spec/expressions/operators_comparison.ori` — 61 tests pass

### 23.5.2 `#derive(Clone)` Implementation

- [x] **Fix**: Generated clone for structs [done] (verified 2026-03-29)
  - [x] Clones all fields correctly
  - [x] **Verified**: `#derive(Clone) type Point = {...}; let p2 = p1.clone()` works
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — 26 tests pass

### 23.5.3 `#derive(Hashable)` Implementation

- [x] **Fix**: Generated hash for structs [done] (verified 2026-03-29)
  - [x] Combines hashes of all fields
  - [x] **Verified**: `#derive(Hashable) type Point = {...}; let h = p.hash()` works
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori`

---

## 23.6 Stdlib Types and Methods

> **SPEC**: Various stdlib specs

### 23.6.1 Queue Type

- [ ] **Implement**: Queue data structure — **6 tests skipped**
  - [ ] `Queue.enqueue(value:)`
  - [ ] `Queue.dequeue()` -> `Option<T>`
  - [ ] `Queue.peek()` -> `Option<T>`
  - [ ] `Queue.len()` -> `int`
  - [ ] `Queue.is_empty()` -> `bool`
  - [ ] `Queue.clear()`
  - [ ] **Location**: `library/std/` or evaluator built-ins

### 23.6.2 Stack Type

- [ ] **Implement**: Stack data structure — **6 tests skipped**
  - [ ] `Stack.push(value:)`
  - [ ] `Stack.pop()` -> `Option<T>`
  - [ ] `Stack.peek()` -> `Option<T>`
  - [ ] `Stack.len()` -> `int`
  - [ ] `Stack.is_empty()` -> `bool`
  - [ ] `Stack.clear()`
  - [ ] **Location**: `library/std/` or evaluator built-ins

### 23.6.3 String Slice

- [ ] **Implement**: String slicing — **2 tests skipped**
  - [ ] `str.slice(start:, end:)` method
  - [ ] `str[start..end]` syntax
  - [ ] **Location**: Evaluator string operations

### 23.6.4 Stdlib Utilities

- [ ] **Implement**: retry/validate — **5 tests skipped**
  - [ ] `retry(attempts:, delay:, op:)`
  - [ ] `validate(value:, rules:)`
  - [ ] **Location**: `library/std/`

### 23.6.5 Async/Future Support

- [ ] **Implement**: Future handling — **1 test skipped**
  - [ ] Async/await or Future handling
  - [ ] **Location**: Evaluator async support

---

## 23.8 Parser Feature Support (Type Checker/Evaluator)

> **SPEC**: `spec/10-declarations.md` § Functions, `spec/14-expressions.md` § Calls
> **STATUS**: Partial — guard clauses work end-to-end; remaining items need type checker + evaluator (verified 2026-03-29)

These features have working **parser support** (Section 0.9.1 complete), but need type checker and/or evaluator implementation.

### 23.8.1 Guard Clauses

> **Parser Status**: Parses correctly (`@f (n: int) -> int if n > 0 = n`)
> **Test File**: `tests/spec/declarations/clause_params.ori`

- [x] **Type Checker**: Verify guard expression returns `bool` [done] (verified 2026-03-29)
  - [x] **Verified**: Guard type checking works correctly
- [x] **Evaluator**: Select matching clause based on guard evaluation [done] (verified 2026-03-29)
  - [x] **Verified**: `@f (n: int) -> int if n > 0 = n * 2; @f (n: int) -> int = 0;` dispatches correctly: `f(n: 5)` returns 10, `f(n: -1)` returns 0
  - [x] **Semantics**: Clauses matched top-to-bottom; guard evaluated after pattern match
  - [x] **Semantics**: If guard is false, try next clause

> DEAD TEST CODE: `tests/spec/declarations/clause_params.ori` has ALL tests commented out. Guard clauses now work; these tests should be revived.

### 23.8.2 List Patterns in Function Parameters

> **Parser Status**: Parses correctly (`@len ([]: [T]) -> int = 0`)
> **Test File**: `tests/spec/declarations/clause_params.ori`
> **Verified not working** (2026-03-29): `E2003: unknown identifier 'tail'` — type checker does not extract bindings from list patterns in function parameters

- [ ] **Type Checker**: Extract bindings from list patterns
  - [ ] **Location**: `ori_types/src/infer/` — pattern binding extraction
  - [ ] **Bindings**: `[x, ..tail]` creates `x: T` and `tail: [T]`
  - [ ] **Empty**: `[]` pattern matches empty list only
- [ ] **Evaluator**: Destructure list into pattern bindings
  - [ ] **Location**: `ori_eval/src/interpreter/` — parameter binding
  - [ ] **Semantics**: Match list structure, bind named elements
  - [ ] **Failure**: If pattern doesn't match, try next clause

### 23.8.3 Const Generics

> **Parser Status**: Parses correctly (`@f<$N: int>`, `@f<$N: int = 10>`)
> **Test File**: `tests/spec/declarations/generics.ori`
> **Verified not working** (2026-03-29): `$N` not recognized in expression context — fails at parse level

- [ ] **Type Checker**: Make const generic params available in scope
  - [ ] **Location**: `ori_types/src/infer/` — generic parameter handling
  - [ ] **Binding**: `$N` available as compile-time constant in function body
  - [ ] **Type**: Const param has the declared type (`int`, `bool`, etc.)
- [ ] **Type Checker**: Evaluate const generic default values
  - [ ] **Constraint**: Default must be const-evaluable
- [ ] **Type Checker**: Support const generic constraints in `where` clauses
  - [ ] **Syntax**: `where N > 0`, `where N > 0 && N <= 100`
  - [ ] **Evaluation**: Constraints checked at monomorphization time
- [ ] **Evaluator**: Substitute const values at call sites
  - [ ] **Location**: `ori_eval/src/interpreter/` — generic instantiation

### 23.8.4 Variadic Parameters

> **Parser Status**: Parses correctly (`@sum (nums: ...int)`)
> **Test File**: `tests/spec/declarations/variadic_params.ori` (needs creation)
> **Verified not working** (2026-03-29): `E2004: expected 1 arguments, found 3` — type checker does not handle variadic expansion

- [ ] **Type Checker**: Handle variadic parameter types
  - [ ] **Location**: `ori_types/src/infer/` — function signature handling
  - [ ] **Semantics**: `...T` in parameter position → receives as `[T]`
  - [ ] **Constraint**: Only one variadic param allowed per function
  - [ ] **Constraint**: Variadic must be last parameter
- [ ] **Evaluator**: Collect variadic arguments into list
  - [ ] **Location**: `ori_eval/src/interpreter/` — call argument handling
  - [ ] **Semantics**: All remaining args collected into `[T]`
  - [ ] **Semantics**: Zero args → empty list `[]`

### 23.8.5 Function-Level Contract Enforcement (`pre()`/`post()`)  <!-- unblocks:0.6.1 -->

> **Parser Status**: Parses correctly (`pre(cond | "msg")`, `post(r -> cond | "msg")`) [done] (2026-02-14)
> **IR**: `CheckExpr` struct, `CheckRange`, stored on function definition node
> **Test File**: `tests/spec/patterns/run.ori` (commented-out tests at lines 140-288)
> **Verified not working** (2026-03-29): `pre()` is parsed but NOT evaluated at runtime — `safe_div(a: 10, b: 0)` produces `E6001: division by zero` instead of contract violation panic

- [ ] **Type Checker**: Verify `pre()` condition is `bool`-typed
  - [ ] **Location**: `ori_types/src/infer/expr.rs` — function contract handling
  - [ ] **Semantics**: `pre(expr)` — expr must be `bool`
  - [ ] **Semantics**: `pre(expr | "msg")` — msg must be `str`
- [ ] **Type Checker**: Verify `post()` lambda returns `bool`
  - [ ] **Semantics**: `post(r -> expr)` — lambda `(T) -> bool` where T is result type
  - [ ] **Semantics**: `post((a, b) -> expr)` — tuple destructuring in lambda
- [ ] **Evaluator**: Execute `pre()` contracts before function body
  - [ ] **Location**: `ori_eval/src/interpreter/` — function call evaluation
  - [ ] **Semantics**: Evaluate condition; if false, panic with message (or default)
  - [ ] **Semantics**: Multiple `pre()` contracts evaluated in order; first failure panics
- [ ] **Evaluator**: Execute `post()` contracts after function body
  - [ ] **Semantics**: Evaluate lambda with result value; if returns false, panic
  - [ ] **Semantics**: Multiple `post()` contracts evaluated in order; first failure panics

### 23.8.6 Spread in Function Calls

> **Parser Status**: Parses correctly (`sum(...list)`)
> **Test File**: `tests/spec/expressions/function_calls.ori`
> **Verified not working** (2026-03-29): `E2001: type mismatch: expected 'int', found '[int]'` — type checker does not expand spread

- [ ] **Type Checker**: Verify spread arg matches variadic param type
  - [ ] **Location**: `ori_types/src/infer/` — call type checking
  - [ ] **Constraint**: Spread only valid for variadic parameters
  - [ ] **Constraint**: `...expr` where `expr: [T]` spreads into `...T` param
- [ ] **Evaluator**: Expand spread arguments at call site
  - [ ] **Location**: `ori_eval/src/interpreter/` — call argument evaluation
  - [ ] **Semantics**: `...list` expands to individual elements
  - [ ] **Semantics**: Multiple spreads allowed: `fn(...a, ...b)`

---

## 23.7 Section Completion Checklist

> **STATUS**: MOSTLY COMPLETE — 4181 passed, 0 failed, 42 skipped (verified 2026-03-29)

- [x] All operator evaluations implemented (23.1) — `??`, comparisons, equality, shift overflow all work [done] (verified 2026-03-29)
- [x] All primitive trait methods registered (23.2) — `.to_str()`, `.clone()`, `.hash()` work [done] (verified 2026-03-29)
- [x] Most indexing behaviors correct per spec (23.3) — map returns `Option<V>`, string indexing returns `str`, non-string keys work [done] (verified 2026-03-29)
- [ ] Control flow semantics (23.4) — function field calls work [done]; labeled breaks still missing
- [x] All derived traits working (23.5) — `#derive(Eq, Clone, Hashable)` work [done] (verified 2026-03-29)
- [ ] Stdlib types (23.6) — Queue/Stack not implemented (aspirational, not spec types)
- [ ] Run `cargo st tests/` — 4181 passed, 42 skipped (skips are mostly LLVM/capability issues)
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review last commit` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.

**Exit Criteria**: Every Ori spec semantic is correctly implemented in the evaluator. All spec tests must pass — no skipped tests allowed.

**Remaining Issues (verified 2026-03-29):**
- Labeled break/continue with values in nested loops (23.4.1)
- List index assignment needs design decision (23.3.4) — currently returns E6080
- Parser features (23.8) — const generics, variadics, contracts, spread need type checker + evaluator
- List patterns in function parameters (23.8.2) — type checker binding extraction
- 4 commented-out test files (~500+ lines) need revival: `function_types.ori`, `clause_params.ori`, `literals.ori`, `functions.ori`
- 5 stale status comments in test files need cleanup (coalesce.ori, index_access.ori x2, operators_comparison.ori x2)
- Missing negative pins (`#compile_fail`) across all operator test suites

---

## Test Status Comments

Most previously-broken test files now pass (verified 2026-03-29). Status comments in some test files are STALE and should be cleaned up.

**RESOLVED** (all tests pass, STALE STATUS COMMENTS need cleanup):
- `tests/spec/expressions/coalesce.ori` — 31/31 pass | STALE: header says "26/31 pass", lists chaining/map failures
- `tests/spec/expressions/index_access.ori` — 34/34 pass | STALE: lines 122-124 say map returns value not Option; lines 203-204 say returns char not str
- `tests/spec/expressions/operators_comparison.ori` — 61/61 pass | STALE: line 365 says Option `<` not implemented; line 456 says struct eq broken
- `tests/spec/expressions/operators_bitwise.ori` — 43/43 pass
- `tests/spec/declarations/traits.ori` — 30/30 pass
- `tests/spec/types/existential.ori` — 8/8 pass
- `tests/spec/expressions/field_access.ori` — 30/30 pass

**DEAD TEST CODE** (commented-out files, features now work — should be revived):
- `tests/spec/types/function_types.ori` — ALL 355 lines commented out; function field calls now work
- `tests/spec/declarations/clause_params.ori` — ALL tests commented out; guard clauses now work
- `tests/spec/expressions/literals.ori` — ALL tests commented out; many literal patterns likely work
- `tests/spec/declarations/functions.ori` — ALL tests commented out; basic function tests should work (variadic section won't)

**GENUINELY INCOMPLETE** (features not yet implemented):
- `tests/spec/expressions/loops.ori` — labeled break value propagation skipped

---

## Notes

- This section can be worked on in parallel with Section 0 (parser)
- Evaluator is the reference implementation; LLVM codegen validates against it
- Some evaluator work overlaps with type checker (Section 1, 2, 3)
- Stdlib types may be implemented in Ori itself once evaluator is complete
