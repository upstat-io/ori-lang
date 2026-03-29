---
section: 23
title: Full Evaluator Support
status: in-progress
reviewed: false
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
    status: not-started
  - id: "23.5"
    title: Derived Traits
    status: done
  - id: "23.6"
    title: Stdlib Types and Methods
    status: not-started
  - id: "23.8"
    title: Parser Feature Support (Type Checker/Evaluator)
    status: not-started
  - id: "23.7"
    title: Section Completion Checklist
    status: not-started
---

# Section 23: Full Evaluator Support

**Goal**: Complete evaluator support for entire Ori spec semantics (parsing assumed working — see Section 0)

> **SPEC**: `spec/grammar.ebnf` (authoritative), `spec/08-types.md`, `spec/14-expressions.md`, `spec/09-properties-of-types.md`

**Status**: In Progress — Most features work! 4181 tests pass, 42 skipped (verified 2026-03-28). Only a few actual bugs remain.

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

> **Test Status**: ALL PASSING — 31/31 tests pass (verified 2026-03-28)

- [x] **Implement**: `??` operator evaluation — **31/31 tests pass** [done] (verified 2026-03-28)
  - [x] **Location**: `ori_eval/src/interpreter/mod.rs` — short-circuit logic in `eval_binary`
  - [x] **Semantics**: `Option<T> ?? T -> T` — return inner value if Some, else right operand
  - [x] **Semantics**: `Result<T, E> ?? T -> T` — return inner value if Ok, else right operand
  - [x] **Short-circuit**: Right operand is NOT evaluated if left is Some/Ok
  - [x] **Chaining**: `a ?? b ?? c ?? default` works for all None/Some patterns
  - [x] **Ori Tests**: `tests/spec/expressions/coalesce.ori` — 31 passed, 0 failed

### 23.1.2 Comparison Operators for Option/Result

- [x] **Implement**: `<`, `<=`, `>`, `>=` for Option types [done] (verified 2026-03-28)
  - [x] **Spec**: `None < Some(x)` for all x — works correctly
  - [x] **Verified**: 61 tests pass in `tests/spec/expressions/operators_comparison.ori`
  - [x] **Ori Tests**: `tests/spec/expressions/operators_comparison.ori`

### 23.1.3 Struct Equality with `#derive(Eq)`

- [x] **Fix**: Equality operators for derived structs [done] (verified 2026-03-28)
  - [x] **Verified**: `#derive(Eq) type Point = { x: int, y: int }` with `p1 == p2` works
  - [x] **Ori Tests**: `tests/spec/expressions/operators_comparison.ori` — 61 tests pass

### 23.1.4 Shift Overflow Behavior

- [x] **Fix**: Left shift overflow panics correctly [done] (verified 2026-03-28)
  - [x] **Spec**: `1 << 63` panics due to overflow
  - [x] **Verified**: `assert_panics(f: () -> 1 << 63)`, `assert_panics(f: () -> 1 << 64)`, `assert_panics(f: () -> 1 << -1)` all pass
  - [x] **Ori Tests**: `tests/spec/expressions/operators_bitwise.ori` — 43 tests pass

---

## 23.2 Primitive Trait Methods

> **SPEC**: `spec/09-properties-of-types.md` § Built-in Traits
> **STATUS**: ALL IMPLEMENTED (verified 2026-03-28)

Primitives (int, str, bool, float, etc.) implement standard trait methods.

### 23.2.1 Printable Trait (`.to_str()`)

- [x] **Implement**: `.to_str()` on primitive types [done] (verified 2026-03-28)
  - [x] `int.to_str()` — Works: `42.to_str() == "42"`
  - [x] `str.to_str()` — Works
  - [x] `bool.to_str()` — Works: `true.to_str() == "true"`
  - [x] `float.to_str()` — Works
  - [x] **Ori Tests**: `tests/spec/declarations/traits.ori` (30 passed), `tests/spec/types/existential.ori` (8 passed)

### 23.2.2 Clone Trait (`.clone()`)

- [x] **Implement**: `.clone()` on primitive types [done] (verified 2026-03-28)
  - [x] `int.clone()` — Works: `let y = x.clone()`
  - [x] `str.clone()` — Works
  - [x] All primitives are cloneable
  - [x] **Ori Tests**: `tests/spec/declarations/traits.ori`, `tests/spec/types/existential.ori`

### 23.2.3 Hashable Trait (`.hash()`)

- [x] **Implement**: `.hash()` on primitive types [done] (verified 2026-03-28)
  - [x] `int.hash()` — Works
  - [x] `str.hash()` — Works
  - [x] **Ori Tests**: `tests/spec/declarations/traits.ori`

---

## 23.3 Type Coercion and Indexing

> **SPEC**: `spec/14-expressions.md` § Index Access
> **STATUS**: Mostly complete — map returns Option<V>, string indexing returns str (verified 2026-03-28)

### 23.3.1 Map Index Return Type

- [x] **Fix**: Map lookup returns `Option<V>` per spec [done] (verified 2026-03-28)
  - [x] **Verified**: `map["a"] ?? 0` works; `is_none(opt: map["missing"])` works; empty map returns None
  - [x] **Ori Tests**: `tests/spec/expressions/index_access.ori` — 34 passed, 1 skipped

### 23.3.2 Map Non-String Keys

- [ ] **Fix**: Allow non-string map keys
  - [ ] **Spec**: `{int: str}` maps should work
  - [ ] **Error**: "map keys must be strings"
  - [ ] **Required**: Support any Hashable type as key
  - [ ] **Ori Tests**: `tests/spec/expressions/literals.ori`

### 23.3.3 String Index Return Type

- [x] **Fix**: String indexing returns `str` per spec [done] (verified 2026-03-28)
  - [x] **Verified**: `"hello"[0]` returns `"h"` (str, not char); `s[# - 1]` returns `"o"`
  - [x] **Ori Tests**: `tests/spec/expressions/index_access.ori`

### 23.3.4 List Index Assignment

- [ ] **Implement**: `list[i] = value` syntax
  - [ ] **Verified**: `let list = [1, 2, 3]; list[0] = 99; assert(eq: list[0] == 99)` works
  - [ ] **Ori Tests**: `tests/spec/expressions/index_access.ori`

---

## 23.4 Control Flow

> **SPEC**: `spec/14-expressions.md` § Control Flow

### 23.4.1 Break with Value in Nested Loops

- [ ] **Fix**: `break value` inside for loop inside loop
  - [ ] **Error**: Returns 0 instead of break value
  - [ ] **Cause**: Break value not propagating through nested constructs
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`

### 23.4.2 Function Field Calls

- [ ] **Implement**: Calling function stored in struct field
  - [ ] **Syntax**: `handler.callback(42)` where `callback: (int) -> str`
  - [ ] **Error**: Compiler crash (index out of bounds in type_interner.rs:226)
  - [ ] **Required**: Recognize field as callable, invoke it
  - [ ] **Ori Tests**: `tests/spec/types/function_types.ori`
  - [ ] **Note**: This causes a compiler panic, not just a type error (verified 2026-02-04)

---

## 23.5 Derived Traits

> **SPEC**: `spec/10-declarations.md` § Attributes
> **STATUS**: ALL IMPLEMENTED (verified 2026-03-28)

### 23.5.1 `#derive(Eq)` Implementation

- [x] **Fix**: Generated equality for structs [done] (verified 2026-03-28)
  - [x] Compares all fields correctly
  - [x] Works with `==` and `!=` operators
  - [x] **Verified**: `#derive(Eq) type Point = {...}; assert(eq: p1 == p2)` works
  - [x] **Ori Tests**: `tests/spec/expressions/operators_comparison.ori` — 61 tests pass

### 23.5.2 `#derive(Clone)` Implementation

- [x] **Fix**: Generated clone for structs [done] (verified 2026-03-28)
  - [x] Clones all fields correctly
  - [x] **Verified**: `#derive(Clone) type Point = {...}; let p2 = p1.clone()` works
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — 26 tests pass

### 23.5.3 `#derive(Hashable)` Implementation

- [x] **Fix**: Generated hash for structs [done] (verified 2026-03-28)
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

These features have working **parser support** (Section 0.9.1 complete), but need type checker and/or evaluator implementation.

### 23.8.1 Guard Clauses

> **Parser Status**: Parses correctly (`@f (n: int) -> int if n > 0 = n`)
> **Test File**: `tests/spec/declarations/clause_params.ori`

- [ ] **Type Checker**: Verify guard expression returns `bool`
  - [ ] **Location**: `ori_types/src/infer/` — check guard expression type
  - [ ] **Constraint**: Guard must be `bool`-typed
- [ ] **Evaluator**: Select matching clause based on guard evaluation
  - [ ] **Location**: `ori_eval/src/interpreter/` — function call resolution
  - [ ] **Semantics**: Clauses matched top-to-bottom; guard evaluated after pattern match
  - [ ] **Semantics**: If guard is false, try next clause

### 23.8.2 List Patterns in Function Parameters

> **Parser Status**: Parses correctly (`@len ([]: [T]) -> int = 0`)
> **Test File**: `tests/spec/declarations/clause_params.ori`

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

> **STATUS**: MOSTLY COMPLETE — 4181 passed, 0 failed, 42 skipped (verified 2026-03-28)

- [x] All operator evaluations implemented (23.1) — `??`, comparisons, equality, shift overflow all work [done] (verified 2026-03-28)
- [x] All primitive trait methods registered (23.2) — `.to_str()`, `.clone()`, `.hash()` work [done] (verified 2026-03-28)
- [x] Most indexing behaviors correct per spec (23.3) — map returns `Option<V>`, string indexing returns `str` [done] (verified 2026-03-28)
- [ ] Control flow semantics (23.4) — basic break value works; labeled breaks and function field calls still broken
- [x] All derived traits working (23.5) — `#derive(Eq, Clone, Hashable)` work [done] (verified 2026-03-28)
- [ ] Stdlib types (23.6) — Queue/Stack not implemented
- [ ] Run `cargo st tests/` — 4181 passed, 42 skipped (skips are mostly LLVM/capability issues)
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)

**Exit Criteria**: Every Ori spec semantic is correctly implemented in the evaluator. All spec tests must pass — no skipped tests allowed.

**Remaining Issues (verified 2026-03-28):**
- Function field calls crash compiler (all tests in function_types.ori commented out)
- Labeled break value propagation in nested loops
- Queue/Stack types not implemented
- Parser features (23.8) — guard clauses, const generics, variadics, contracts, spread all parser-only

---

## Test Status Comments

Most previously-broken test files now pass (verified 2026-03-28). Status comments in some test files are STALE and should be cleaned up.

**RESOLVED** (all tests pass, status comments may be stale):
- `tests/spec/expressions/coalesce.ori` — 31/31 pass (was: `??` operator partial)
- `tests/spec/expressions/index_access.ori` — 34/34 pass (was: map/string indexing broken)
- `tests/spec/expressions/operators_comparison.ori` — 61/61 pass (was: Option order, struct eq)
- `tests/spec/expressions/operators_bitwise.ori` — 43/43 pass (was: shift overflow)
- `tests/spec/declarations/traits.ori` — 30/30 pass (was: primitive trait methods)
- `tests/spec/types/existential.ori` — 8/8 pass (was: primitive trait methods)
- `tests/spec/expressions/field_access.ori` — 30/30 pass (was: `??` operator)

**STILL BROKEN** (tests commented out or skipped):
- `tests/spec/types/function_types.ori` — ALL tests commented out (function field calls)
- `tests/spec/expressions/literals.ori` — ALL tests commented out
- `tests/spec/declarations/clause_params.ori` — ALL tests commented out
- `tests/spec/declarations/functions.ori` — ALL tests commented out (variadic section)
- `tests/spec/expressions/loops.ori` — labeled break value propagation skipped

---

## Notes

- This section can be worked on in parallel with Section 0 (parser)
- Evaluator is the reference implementation; LLVM codegen validates against it
- Some evaluator work overlaps with type checker (Section 1, 2, 3)
- Stdlib types may be implemented in Ori itself once evaluator is complete
