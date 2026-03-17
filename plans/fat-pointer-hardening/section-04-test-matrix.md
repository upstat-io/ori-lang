---
section: "04"
title: "Combinatorial Test Matrix"
status: not-started
goal: "Every cell of {type categories} x {language features} is covered by an AOT test, ensuring no intersection of fat pointers with any feature is untested"
depends_on: ["01", "02", "03"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Type Category Definitions"
    status: not-started
  - id: "04.2"
    title: "Feature Dimension Definitions"
    status: not-started
  - id: "04.3"
    title: "Matrix Implementation"
    status: not-started
  - id: "04.4"
    title: "Valgrind Verification Layer"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Combinatorial Test Matrix

**Status:** Not Started
**Goal:** Build a systematic test matrix covering `{type categories} x {language features}`. Every cell is an AOT test program that exercises a specific type in a specific feature context. All tests pass in both eval and AOT. All tests run clean under Valgrind.

**Context:** The original 13 code journeys all scored 10.0/10, yet 3 CRITICAL bugs lurked at feature intersections. The journeys tested features in isolation: J5 tested closures with `int` capture, J9 tested strings with `.length()`, but nobody tested closures capturing strings. The test matrix ensures this gap class is eliminated permanently — every type x feature intersection is tested.

**Design principle:** Tests target the **general type category**, not specific literal values. A test for "str x closures" proves that ALL string values work in closure capture, not just `"hello"`. The type categories and feature dimensions are defined below.

---

## 04.1 Type Category Definitions

These are the type categories that differ in LLVM representation, ARC strategy, or ABI treatment. Each category exercises a different codegen path.

| ID | Category | LLVM Type | RC Strategy | ABI | Example |
|----|----------|-----------|-------------|-----|---------|
| T1 | Scalar int | `i64` | None | Direct | `42` |
| T2 | Scalar float | `double` | None | Direct | `3.14` |
| T3 | Scalar bool | `i1` | None | Direct | `true` |
| T4 | String (SSO) | `{i64, i64, ptr}` | FatPointer (SSO skip) | Indirect (24B) | `"hello"` (<=23 bytes) |
| T5 | String (heap) | `{i64, i64, ptr}` | FatPointer (heap RC) | Indirect (24B) | `"abcdefghijklmnopqrstuvwxyz1234"` |
| T6 | List of scalars | `{i64, i64, ptr}` | HeapPointer | Indirect (24B) | `[1, 2, 3]` |
| T7 | List of fat ptrs | `{i64, i64, ptr}` | HeapPointer + elem RC | Indirect (24B) | `["a", "b"]` |
| T8 | Struct (scalar fields) | `{i64, i64}` | None | Direct (<=16B) or Indirect | `Point { x: 1, y: 2 }` |
| T9 | Struct (fat fields) | `{{i64,i64,ptr}, i64}` | AggregateFields | Indirect | `Named { name: "x", id: 1 }` |
| T10 | Sum type (unit variants) | `i64` (tag only) | None | Direct | `Red \| Green \| Blue` |
| T11 | Sum type (fat payload) | `{i64, {i64, i64, ptr}}` | InlineEnum | Indirect | `Some("hello")` / `None` |
| T12 | Closure (no capture) | `{ptr, ptr}` | Closure (null env) | Direct (16B) | `x -> x + 1` |
| T13 | Closure (scalar capture) | `{ptr, ptr}` | Closure (env RC) | Direct (16B) | `let n = 5; x -> x + n` |
| T14 | Closure (fat capture) | `{ptr, ptr}` | Closure (env RC + elem RC) | Direct (16B) | `let s = "hi"; x -> s.length() + x` |
| T15 | Option\<int\> | `{i64, i64}` | None | Direct | `Some(42)` / `None` |
| T16 | Option\<str\> | `{i64, {i64, i64, ptr}}` | InlineEnum + FatPointer | Indirect | `Some("hello")` / `None` |
| T17 | Map (str keys) | `{i64, i64, ptr}` | HeapPointer + key/val RC | Indirect (24B) | `{"a": 1, "b": 2}` |
| T18 | Tuple (mixed) | `{{i64, i64, ptr}, i64}` | AggregateFields | Indirect | `("hello", 42)` |

---

## 04.2 Feature Dimension Definitions

These are the language features that exercise different compiler paths (monomorphization, codegen patterns, ARC insertion, control flow).

| ID | Feature | What It Tests | Compiler Path |
|----|---------|---------------|---------------|
| F1 | Let binding | Value construction and binding | Value emission, alloca/store |
| F2 | Function parameter | Passing values to functions | ABI, borrow elision, RC inc/dec |
| F3 | Function return | Returning values from functions | Return ABI (sret vs register) |
| F4 | Closure capture | Capturing values in closure env | Env alloc, type propagation |
| F5 | Closure parameter | Passing values through closure call | Indirect call, trampoline |
| F6 | Pattern matching | Match expressions on values | Decision tree, extractvalue |
| F7 | If/else branching | Using values in conditionals | Select vs branch, phi merge |
| F8 | For loop iteration | Iterating over collections of values | Iterator protocol, element borrow |
| F9 | Loop accumulation | Accumulating values across iterations | Phi nodes, mutable binding |
| F10 | Generic instantiation | Using values as generic type params | Monomorphization |
| F11 | Struct field | Storing values in struct fields | GEP, aggregate construction |
| F12 | Sum type payload | Values as enum variant payloads | Tag + payload layout |
| F13 | Derived Eq | Equality comparison on values | `$eq` method codegen |
| F14 | List element | Values stored in list elements | Element-level RC, iteration |
| F15 | ? propagation | Using ? on Option/Result containing values | Early return, cleanup |
| F16 | Recursion | Passing values through recursive calls | Stack frames, RC across calls |
| F17 | Higher-order | Values passed through fn-typed params | Indirect call, type erasure |
| F18 | Multiple values | Multiple values of same type in scope | RC tracking, drop ordering |
| F19 | Break/continue | Early exit from loops with fat values in scope | Cleanup on break, continue semantics |
| F20 | Derived Clone | Cloning values containing fat pointer fields | Clone codegen, RC increment |

---

## 04.3 Matrix Implementation

**File(s):** `compiler/ori_llvm/tests/aot/fat_matrix/`, `tests/spec/fat_matrix/`

Not every cell in the 18x20 matrix (360 cells) needs a separate test file. Group tests by feature dimension — each test file exercises one feature across multiple type categories.

**Test file structure:**

```
compiler/ori_llvm/tests/aot/fat_matrix/
  f01_let_binding.rs        # T4-T18 in let bindings
  f02_function_param.rs     # T4-T18 as function params
  f03_function_return.rs    # T4-T18 as return values
  f04_closure_capture.rs    # T4-T18 as closure captures
  f05_closure_param.rs      # T4-T18 through closure calls
  f06_pattern_matching.rs   # T4-T18 in match expressions
  f07_branching.rs          # T4-T18 in if/else
  f08_for_loop.rs           # T6-T7, T17 as iteration sources; T4-T18 as elements
  f09_loop_accumulation.rs  # T4-T18 accumulated in loops
  f10_generics.rs           # T4-T18 through generic functions
  f11_struct_field.rs       # T4-T18 as struct fields
  f12_sum_payload.rs        # T4-T18 as sum type payloads
  f13_derived_eq.rs         # T4-T18 in derived Eq
  f14_list_element.rs       # T4-T18 as list elements
  f15_question_mark.rs      # T4-T18 in ? propagation
  f16_recursion.rs          # T4-T18 through recursive calls
  f17_higher_order.rs       # T4-T18 through higher-order functions
  f18_multiple_values.rs    # Multiple T4-T18 in same scope
  f19_break_continue.rs     # T4-T18 in loops with break/continue
  f20_derived_clone.rs      # T4-T18 cloned via derived Clone
```

Each test file is a Rust AOT test that:
1. Compiles an Ori program exercising the feature with each type
2. Runs it via eval AND AOT
3. Asserts identical exit codes
4. Runs under Valgrind for fat pointer types (T4-T18) -- this is mandatory per Section 04.4

- [ ] Create the `fat_matrix/` test directory structure:
  1. Create directory `compiler/ori_llvm/tests/aot/fat_matrix/`
  2. Add `pub mod fat_matrix;` to `compiler/ori_llvm/tests/aot/main.rs` (after existing module declarations)
  3. Create `compiler/ori_llvm/tests/aot/fat_matrix/mod.rs` declaring sub-modules (one per feature file)
  4. Each test file imports from `crate::util` for test helpers
- [ ] Implement F01 (let binding) tests for all fat pointer type categories (T4-T18)
- [ ] Implement F02 (function parameter) tests
- [ ] Implement F03 (function return) tests
- [ ] Implement F04 (closure capture) tests -- this is the J17 bug area
- [ ] Implement F05 (closure parameter) tests
- [ ] Implement F06 (pattern matching) tests
- [ ] Implement F07 (branching) tests
- [ ] Implement F08 (for loop iteration) tests -- this is the J15 bug area
- [ ] Implement F09 (loop accumulation) tests
- [ ] Implement F10 (generic instantiation) tests
- [ ] Implement F11 (struct field) tests
- [ ] Implement F12 (sum type payload) tests
- [ ] Implement F13 (derived Eq) tests
- [ ] Implement F14 (list element) tests -- this is also the J15 bug area
- [ ] Implement F15 (? propagation) tests
- [ ] Implement F16 (recursion) tests
- [ ] Implement F17 (higher-order) tests
- [ ] Implement F18 (multiple values) tests
- [ ] Implement F19 (break/continue) tests -- fat values in scope at break/continue must be cleaned up correctly
- [ ] Implement F20 (derived Clone) tests -- Clone of structs/sum types with fat fields
- [ ] All tests pass in both eval and AOT

**Priority ordering:** F04 (closure capture) and F08/F14 (iteration/list elements) first -- these are the known bug areas. Then F02/F03 (function param/return) as the most common fat pointer operations. Then the rest.

### Coverage Tracking

Maintain a coverage matrix in this file. Mark each cell as:
- `PASS` -- test exists and passes
- `FAIL` -- test exists and fails (with bug ID)
- `N/A` -- combination doesn't apply (e.g., T1 scalar int x F08 for loop iteration -- tested elsewhere)
- `---` -- not yet implemented

Initial state: all `---`. Target state: all `PASS` or `N/A`.

---

## 04.4 Valgrind Verification Layer

**File(s):** `tests/valgrind/fat_matrix/`

Spec tests and AOT tests verify behavioral correctness (right exit code). Valgrind verifies memory correctness (no leaks, no double-frees, no use-after-free).

For every test in the matrix that involves fat pointer types (T4-T18), create a corresponding Valgrind test:

- [ ] Create `tests/valgrind/fat_matrix/` directory
- [ ] Write Valgrind test runner that builds each `.ori` program and runs under `valgrind --leak-check=full --show-leak-kinds=all`
- [ ] All T4-T18 tests pass Valgrind with "0 errors from 0 contexts"
- [ ] Add to `diagnostics/valgrind-aot.sh` so the fat matrix is included in manual Valgrind runs

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] All 20 feature test files created
- [ ] All applicable type x feature cells are PASS
- [ ] No FAIL cells remain
- [ ] Valgrind clean on all fat pointer tests (T4-T18)
- [ ] `./test-all.sh` green (includes all new tests) -- debug AND release
- [ ] Coverage matrix in this file is fully populated
- [ ] No `---` (not yet implemented) cells remain for applicable combinations
- [ ] `diagnostics/dual-exec-verify.sh` passes on all fat matrix `.ori` programs (eval == AOT)
- [ ] `ORI_CHECK_LEAKS=1` reports 0 leaks on all fat matrix AOT binaries

**Exit Criteria:** `timeout 150 cargo test -p ori_llvm fat_matrix` passes all tests (0 failures) AND `diagnostics/valgrind-aot.sh tests/valgrind/fat_matrix/` reports "0 errors" for every test program AND `diagnostics/dual-exec-verify.sh` reports 0 mismatches.
