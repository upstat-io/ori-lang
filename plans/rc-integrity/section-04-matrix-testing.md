---
section: "04"
title: "Matrix Testing — Regression Guard"
status: not-started
goal: "Combinatorial test matrix covering value-type × operation × context — makes regressions progressively harder to introduce"
depends_on: ["01", "02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Matrix Design — Dimensions & Cross-Product"
    status: not-started
  - id: "04.2"
    title: "Value Type × Loop Pattern Matrix"
    status: not-started
  - id: "04.3"
    title: "Value Type × Scope Pattern Matrix"
    status: not-started
  - id: "04.4"
    title: "Nested & Composed Pattern Matrix"
    status: not-started
  - id: "04.5"
    title: "Journey Score Regression Guard"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Matrix Testing — Regression Guard

**Status:** Not Started
**Goal:** Build a combinatorial test matrix that covers the cross-product of (value type × operation × context). When any ARC pipeline change breaks leak-free behavior for a specific combination, the matrix test catches it immediately. The goal is to narrow the band of acceptable behavior so regressions become harder as the compiler grows.

**Context:** The FatValue PrimOp bug existed because no test exercised "string in loop" — there were string tests and loop tests but not the combination. Matrix testing prevents this class of gap by systematically covering the cross-product.

**Depends on:** Section 01 (leak detection), Section 02 (existing leaks fixed).

> **Warning: Test timing risk.** This section adds 66+ AOT tests, each compiling and running an Ori program via `compile_and_run_capture`. AOT tests currently run sequentially due to LLVM `Context::create()` contention. Adding 66 tests that each invoke `ori build` + execute could add 2-5 minutes to the AOT test suite. Verify that `timeout 150 ./test-all.sh` still passes after adding all matrix tests. If timing becomes an issue, consider batching multiple assertions into fewer test functions (e.g., one test function per matrix row instead of per cell).

---

## 04.1 Matrix Design — Dimensions & Cross-Product

Define the test matrix dimensions:

**Dimension 1: Value Types (RC-managed)**
- `str` (FatValue) — SSO and heap variants
- `[int]` (RcPointer) — empty and non-empty
- `[str]` (RcPointer with RC elements) — nested RC
- `{str: int}` (RcPointer with RC keys) — map with heap keys
- `type S = { items: [int], name: str }` (Aggregate with RC fields)

**Dimension 2: Operations**
- Construct (literal creation)
- Reassign in loop (`s = s + "x"`)
- Pass to function (owned, borrowed)
- Return from function
- Store in struct field
- Extract from struct field (Project)
- Compare (equality, ordering)
- Drop at end of scope

**Dimension 3: Context**
- Top-level local (simple scope)
- For-loop body
- While-loop body
- Loop-with-break body
- If-else branches (value live in one, dead in other)
- Match arms (value live in one arm, dead in others)
- Function call argument
- Function return value
- Closure capture (RC variable captured by closure, closure dropped)

- [ ] Document the full matrix dimensions in this section
- [ ] Identify which combinations are already covered by existing tests
- [ ] Identify which combinations are gaps (the cross-product minus existing coverage)
- [ ] Prioritize: high-risk combinations (loops + heap types, branches + struct drops) first

---

## 04.2 Value Type × Loop Pattern Matrix

**File(s):** `compiler/ori_llvm/tests/aot/arc.rs` (or new file `compiler/ori_llvm/tests/aot/rc_matrix.rs`)

Test every value type being reassigned inside every loop pattern. Each test uses `assert_aot_success` (which enables `ORI_CHECK_LEAKS=1`).

| | `for` loop | `while` loop | `loop`+`break` |
|---|---|---|---|
| `str` (SSO→heap) | `test_matrix_str_for_loop` | `test_matrix_str_while_loop` | `test_matrix_str_loop_break` |
| `[int]` push | `test_matrix_list_int_for_loop` | `test_matrix_list_int_while_loop` | `test_matrix_list_int_loop_break` |
| `[str]` push | `test_matrix_list_str_for_loop` | `test_matrix_list_str_while_loop` | `test_matrix_list_str_loop_break` |
| `{str: int}` insert | `test_matrix_map_for_loop` | `test_matrix_map_while_loop` | `test_matrix_map_loop_break` |
| Struct w/ heap | `test_matrix_struct_for_loop` | `test_matrix_struct_while_loop` | `test_matrix_struct_loop_break` |

- [ ] Create `compiler/ori_llvm/tests/aot/rc_matrix.rs`
- [ ] Add `pub mod rc_matrix;` to `compiler/ori_llvm/tests/aot/main.rs`
- [ ] Implement all 15 loop matrix tests (5 types × 3 loop patterns)
- [ ] Each test: 30 iterations, verify correct result AND zero leaks
- [ ] All 15 tests pass

---

## 04.3 Value Type × Scope Pattern Matrix

Test every value type in different scope contexts — ensures drops fire at the right points.

| | Simple scope | If-else | Match arms | Function arg | Function return |
|---|---|---|---|---|---|
| `str` (heap) | `test_matrix_str_scope` | `test_matrix_str_if_else` | `test_matrix_str_match` | `test_matrix_str_arg` | `test_matrix_str_return` |
| `[int]` | `test_matrix_list_scope` | `test_matrix_list_if_else` | `test_matrix_list_match` | `test_matrix_list_arg` | `test_matrix_list_return` |
| `[str]` | `test_matrix_list_str_scope` | `test_matrix_list_str_if_else` | `test_matrix_list_str_match` | `test_matrix_list_str_arg` | `test_matrix_list_str_return` |
| `{str: int}` | `test_matrix_map_scope` | `test_matrix_map_if_else` | `test_matrix_map_match` | `test_matrix_map_arg` | `test_matrix_map_return` |
| Struct w/ heap | `test_matrix_struct_scope` | `test_matrix_struct_if_else` | `test_matrix_struct_match` | `test_matrix_struct_arg` | `test_matrix_struct_return` |

- [ ] Implement all 25 scope matrix tests (5 types × 5 contexts)
- [ ] Each test verifies correct result AND zero leaks
- [ ] All 25 tests pass

---

## 04.4 Nested & Composed Pattern Matrix

Test combinations that compose multiple dimensions — the highest-risk patterns.

- [ ] `test_matrix_struct_with_list_in_loop` — Struct containing `[int]` reassigned in loop
- [ ] `test_matrix_list_of_strings_in_loop` — `[str]` with push in loop (nested RC: list + string elements)
- [ ] `test_matrix_string_in_if_else_in_loop` — String conditionally updated in loop
- [ ] `test_matrix_slice_in_scope` — Create slice, use, let both slice and original drop
- [ ] `test_matrix_slice_in_loop` — Create slices in a loop
- [ ] `test_matrix_multiple_heap_locals` — Multiple independent heap variables in one scope
- [ ] `test_matrix_heap_var_shadowing` — Shadow a heap variable with a new heap value
- [ ] `test_matrix_closure_captures_string` — Lambda capturing a heap string, called, then dropped
- [ ] `test_matrix_closure_captures_list` — Lambda capturing a `[int]`, called, then dropped
- [ ] `test_matrix_closure_in_loop` — Lambda created inside loop body capturing loop variable, used and dropped each iteration

---

## 04.5 Journey Score Regression Guard

Ensure the 10/10 code journey scores cannot regress.

- [ ] Create `compiler/ori_llvm/tests/aot/journey_guard.rs`
- [ ] Add `pub mod journey_guard;` to `compiler/ori_llvm/tests/aot/main.rs`
- [ ] For each of the 16 journeys (13 original + 3 new):
  - Compile the journey `.ori` file from `plans/code-journeys/NN-name.ori`
  - Run with `ORI_CHECK_LEAKS=1`
  - Verify exit code matches expected value (each journey's `@main` returns an `int` exit code; store expected values as named constants in `journey_guard.rs`)
  - Verify zero leaks (exit code != 2)
- [ ] Each test must hard-fail (not skip) if the journey `.ori` file is missing
- [ ] These tests run as part of `cargo test -p ori_llvm --test aot`
- [ ] Verify these tests are included in `./test-all.sh` via the existing `cargo test -p ori_llvm --test aot` invocation

---

### Cleanup (Applies to rc_matrix.rs and journey_guard.rs creation)

- [ ] **[BLOAT]** `compiler/ori_llvm/tests/aot/util.rs` (908 lines) — If adding AOT helpers for matrix tests, add them to a new submodule, not to `util.rs`. See Section 01.2 cleanup note for the split plan.

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] Matrix test file created (`rc_matrix.rs`) and registered in `main.rs`
- [ ] Journey guard file created (`journey_guard.rs`) and registered in `main.rs`
- [ ] 15 loop matrix tests pass (5 types × 3 loop patterns)
- [ ] 25 scope matrix tests pass (5 types × 5 contexts)
- [ ] 10 nested/composed matrix tests pass (7 original + 3 closure)
- [ ] 16 journey guard tests pass (13 original + 3 new)
- [ ] Total: 66+ new tests, all passing with zero leaks
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in existing tests

**Exit Criteria:** 66+ matrix tests covering the cross-product of value types, operations, and contexts. Every combination that could regress has an explicit test. Journey scores are guarded by automated tests that fail on any regression.
