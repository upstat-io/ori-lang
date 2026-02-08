---
section: 23
title: Full Evaluator Support
status: not-started
tier: 0
goal: Complete evaluator support for entire Ori spec semantics
spec:
  - spec/grammar.ebnf
  - spec/06-types.md
  - spec/08-declarations.md
  - spec/09-expressions.md
  - spec/10-patterns.md
  - spec/11-traits.md
sections:
  - id: "23.1"
    title: Operators
    status: not-started
  - id: "23.2"
    title: Primitive Trait Methods
    status: not-started
  - id: "23.3"
    title: Type Coercion and Indexing
    status: not-started
  - id: "23.4"
    title: Control Flow
    status: not-started
  - id: "23.5"
    title: Derived Traits
    status: not-started
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

> **SPEC**: `spec/grammar.ebnf` (authoritative), `spec/06-types.md`, `spec/09-expressions.md`, `spec/11-traits.md`

**Status**: In Progress — Most features work! 1983 tests pass, 31 skipped. Only a few actual bugs remain (verified 2026-02-04).

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

> **SPEC**: `spec/09-expressions.md` § Operators

### 23.1.1 Null Coalesce Operator (`??`)

> **Test Status**: `STATUS: Lexer [OK], Parser [OK], Evaluator [PARTIAL]` in `tests/spec/expressions/coalesce.ori`
> **Progress**: 26/31 tests pass. Remaining 5 failures: 3 chaining tests (need type info), 2 map tests (separate bug).

- [ ] **Implement**: `??` operator evaluation — **26/31 tests pass**
  - [ ] **Location**: `ori_eval/src/interpreter/mod.rs` — added short-circuit logic in `eval_binary`
  - [ ] **Semantics**: `Option<T> ?? T -> T` — return inner value if Some, else right operand
  - [ ] **Semantics**: `Result<T, E> ?? T -> T` — return inner value if Ok, else right operand
  - [ ] **Short-circuit**: Right operand is NOT evaluated if left is Some/Ok
  - [ ] **Known Limitation**: Chaining (`a ?? b ?? c`) with Option variables fails without type info
    - **Workaround**: Use explicit `.unwrap_or()` or `.or()` methods for chaining
  - [ ] **Depends On**: Map lookup returning `Option<V>` (Section 23.3.1) for map tests to pass
  - [ ] **Ori Tests**: `tests/spec/expressions/coalesce.ori`

### 23.1.2 Comparison Operators for Option/Result

- [ ] **Implement**: `<`, `<=`, `>`, `>=` for Option types
  - [ ] **Spec**: `None < Some(x)` for all x — Works correctly
  - [ ] **Verified**: `let a: Option<int> = None; let b = Some(1); assert(eq: a < b)` passes
  - [ ] **Ori Tests**: `tests/spec/expressions/operators_comparison.ori`

### 23.1.3 Struct Equality with `#derive(Eq)`

- [ ] **Fix**: Equality operators for derived structs
  - [ ] **Verified**: `#derive(Eq) type Point = { x: int, y: int }` with `p1 == p2` works
  - [ ] **Ori Tests**: `tests/spec/expressions/operators_comparison.ori`

### 23.1.4 Shift Overflow Behavior

- [ ] **Fix**: Left shift overflow should panic
  - [ ] **Spec**: `1 << 63` should panic due to overflow
  - [ ] **Error**: Evaluator succeeds silently instead of panicking
  - [ ] **Ori Tests**: `tests/spec/expressions/operators_bitwise.ori`

---

## 23.2 Primitive Trait Methods

> **SPEC**: `spec/11-traits.md` § Built-in Traits
> **STATUS**: ALL IMPLEMENTED (verified 2026-02-04)

Primitives (int, str, bool, float, etc.) implement standard trait methods.

### 23.2.1 Printable Trait (`.to_str()`)

- [ ] **Implement**: `.to_str()` on primitive types
  - [ ] `int.to_str()` — Works: `42.to_str() == "42"`
  - [ ] `str.to_str()` — Works
  - [ ] `bool.to_str()` — Works: `true.to_str() == "true"`
  - [ ] `float.to_str()` — Works
  - [ ] **Ori Tests**: `tests/spec/declarations/traits.ori`, `tests/spec/types/existential.ori`

### 23.2.2 Clone Trait (`.clone()`)

- [ ] **Implement**: `.clone()` on primitive types
  - [ ] `int.clone()` — Works: `let y = x.clone()`
  - [ ] `str.clone()` — Works
  - [ ] All primitives are cloneable
  - [ ] **Ori Tests**: `tests/spec/declarations/traits.ori`, `tests/spec/types/existential.ori`

### 23.2.3 Hashable Trait (`.hash()`)

- [ ] **Implement**: `.hash()` on primitive types
  - [ ] `int.hash()` — Works
  - [ ] `str.hash()` — Works
  - [ ] **Ori Tests**: `tests/spec/declarations/traits.ori`

---

## 23.3 Type Coercion and Indexing

> **SPEC**: `spec/09-expressions.md` § Index Access
> **STATUS**: Mostly complete (verified 2026-02-04)

### 23.3.1 Map Index Return Type

- [ ] **Fix**: Map lookup works for existing keys
  - [ ] **Verified**: `let m = {"a": 1}; let val = m["a"]; assert(eq: val == 1)` works
  - [ ] **Pending**: Missing key behavior needs verification — spec says should return `Option<V>`
  - [ ] **Ori Tests**: `tests/spec/expressions/index_access.ori`, `tests/spec/expressions/literals.ori`

### 23.3.2 Map Non-String Keys

- [ ] **Fix**: Allow non-string map keys
  - [ ] **Spec**: `{int: str}` maps should work
  - [ ] **Error**: "map keys must be strings"
  - [ ] **Required**: Support any Hashable type as key
  - [ ] **Ori Tests**: `tests/spec/expressions/literals.ori`

### 23.3.3 String Index Return Type

- [ ] **Fix**: String indexing works
  - [ ] **Verified**: `let s = "hello"; let c = s[0]` compiles and runs
  - [ ] **Pending**: Verify return type matches spec (str vs char)
  - [ ] **Ori Tests**: `tests/spec/expressions/index_access.ori`

### 23.3.4 List Index Assignment

- [ ] **Implement**: `list[i] = value` syntax
  - [ ] **Verified**: `let list = [1, 2, 3]; list[0] = 99; assert(eq: list[0] == 99)` works
  - [ ] **Ori Tests**: `tests/spec/expressions/index_access.ori`

---

## 23.4 Control Flow

> **SPEC**: `spec/09-expressions.md` § Control Flow

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

> **SPEC**: `spec/08-declarations.md` § Attributes
> **STATUS**: ALL IMPLEMENTED (verified 2026-02-04)

### 23.5.1 `#derive(Eq)` Implementation

- [ ] **Fix**: Generated equality for structs
  - [ ] Compares all fields correctly
  - [ ] Works with `==` and `!=` operators
  - [ ] **Verified**: `#derive(Eq) type Point = {...}; assert(eq: p1 == p2)` works
  - [ ] **Ori Tests**: `tests/spec/expressions/operators_comparison.ori`

### 23.5.2 `#derive(Clone)` Implementation

- [ ] **Fix**: Generated clone for structs
  - [ ] Clones all fields correctly
  - [ ] **Verified**: `#derive(Clone) type Point = {...}; let p2 = p1.clone()` works
  - [ ] **Ori Tests**: `tests/spec/declarations/attributes.ori`

### 23.5.3 `#derive(Hashable)` Implementation

- [ ] **Fix**: Generated hash for structs
  - [ ] Combines hashes of all fields
  - [ ] **Verified**: `#derive(Hashable) type Point = {...}; let h = p.hash()` works
  - [ ] **Ori Tests**: `tests/spec/declarations/attributes.ori`

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

> **SPEC**: `spec/08-declarations.md` § Functions, `spec/09-expressions.md` § Calls

These features have working **parser support** (Section 0.9.1 complete), but need type checker and/or evaluator implementation.

### 23.8.1 Guard Clauses

> **Parser Status**: Parses correctly (`@f (n: int) -> int if n > 0 = n`)
> **Test File**: `tests/spec/declarations/clause_params.ori`

- [ ] **Type Checker**: Verify guard expression returns `bool`
  - [ ] **Location**: `ori_typeck/src/infer/` — check guard expression type
  - [ ] **Constraint**: Guard must be `bool`-typed
- [ ] **Evaluator**: Select matching clause based on guard evaluation
  - [ ] **Location**: `ori_eval/src/interpreter/` — function call resolution
  - [ ] **Semantics**: Clauses matched top-to-bottom; guard evaluated after pattern match
  - [ ] **Semantics**: If guard is false, try next clause

### 23.8.2 List Patterns in Function Parameters

> **Parser Status**: Parses correctly (`@len ([]: [T]) -> int = 0`)
> **Test File**: `tests/spec/declarations/clause_params.ori`

- [ ] **Type Checker**: Extract bindings from list patterns
  - [ ] **Location**: `ori_typeck/src/infer/` — pattern binding extraction
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
  - [ ] **Location**: `ori_typeck/src/infer/` — generic parameter handling
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
  - [ ] **Location**: `ori_typeck/src/infer/` — function signature handling
  - [ ] **Semantics**: `...T` in parameter position → receives as `[T]`
  - [ ] **Constraint**: Only one variadic param allowed per function
  - [ ] **Constraint**: Variadic must be last parameter
- [ ] **Evaluator**: Collect variadic arguments into list
  - [ ] **Location**: `ori_eval/src/interpreter/` — call argument handling
  - [ ] **Semantics**: All remaining args collected into `[T]`
  - [ ] **Semantics**: Zero args → empty list `[]`

### 23.8.5 Spread in Function Calls

> **Parser Status**: Parses correctly (`sum(...list)`)
> **Test File**: `tests/spec/expressions/function_calls.ori`

- [ ] **Type Checker**: Verify spread arg matches variadic param type
  - [ ] **Location**: `ori_typeck/src/infer/` — call type checking
  - [ ] **Constraint**: Spread only valid for variadic parameters
  - [ ] **Constraint**: `...expr` where `expr: [T]` spreads into `...T` param
- [ ] **Evaluator**: Expand spread arguments at call site
  - [ ] **Location**: `ori_eval/src/interpreter/` — call argument evaluation
  - [ ] **Semantics**: `...list` expands to individual elements
  - [ ] **Semantics**: Multiple spreads allowed: `fn(...a, ...b)`

---

## 23.7 Section Completion Checklist

> **STATUS**: MOSTLY COMPLETE — 1983 passed, 0 failed, 31 skipped (verified 2026-02-04)

- [ ] All operator evaluations implemented (23.1) — `??`, comparisons, equality work
- [ ] All primitive trait methods registered (23.2) — `.to_str()`, `.clone()`, `.hash()` work
- [ ] Most indexing behaviors correct per spec (23.3) — list/map/string indexing work
- [ ] Control flow semantics (23.4) — break value propagation, function field calls still broken
- [ ] All derived traits working (23.5) — `#derive(Eq, Clone, Hashable)` work
- [ ] Stdlib types (23.6) — Queue/Stack not implemented
- [ ] Run `cargo st tests/` — 1983 passed, 31 skipped (skips are mostly LLVM/capability issues)

**Exit Criteria**: Every Ori spec semantic is correctly implemented in the evaluator. All spec tests must pass — no skipped tests allowed.

**Remaining Issues (verified 2026-02-04):**
- Function field calls crash compiler (type_interner.rs:226 index out of bounds)
- Break value propagation in nested loops
- Map missing key behavior (needs Option<V>)
- Queue/Stack types not implemented

---

## Test Status Comments

Each failing test file has a status comment documenting the issue:

```
// STATUS: Lexer [OK], Parser [OK], Evaluator [BROKEN] - <specific issue>
```

Files with evaluator bugs:
- `tests/spec/expressions/coalesce.ori` — `??` operator
- `tests/spec/expressions/index_access.ori` — map/string indexing
- `tests/spec/expressions/operators_comparison.ori` — Option order, struct eq
- `tests/spec/expressions/operators_bitwise.ori` — shift overflow
- `tests/spec/expressions/loops.ori` — break value propagation
- `tests/spec/declarations/traits.ori` — primitive trait methods
- `tests/spec/types/existential.ori` — primitive trait methods
- `tests/spec/types/function_types.ori` — function field calls
- `tests/spec/expressions/literals.ori` — map issues
- `tests/spec/expressions/field_access.ori` — `??` operator

---

## Notes

- This section can be worked on in parallel with Section 0 (parser)
- Evaluator is the reference implementation; LLVM codegen validates against it
- Some evaluator work overlaps with type checker (Section 1, 2, 3)
- Stdlib types may be implemented in Ori itself once evaluator is complete
