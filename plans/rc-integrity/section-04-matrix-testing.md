---
section: "04"
title: "Matrix Testing — Regression Guard"
status: in-progress
goal: "Combinatorial test matrix covering value-type × operation × context — makes regressions progressively harder to introduce"
depends_on: ["01", "02"]
third_party_review:
  status: findings
  updated: 2026-03-20
sections:
  - id: "04.1"
    title: "Matrix Design — Dimensions & Cross-Product"
    status: complete
  - id: "04.2"
    title: "Value Type × Loop Pattern Matrix"
    status: complete
  - id: "04.3"
    title: "Value Type × Scope Pattern Matrix"
    status: complete
  - id: "04.4"
    title: "Nested & Composed Pattern Matrix"
    status: complete
  - id: "04.5"
    title: "Journey Score Regression Guard"
    status: complete
  - id: "04.R"
    title: "Third Party Review Findings"
    status: in-progress
  - id: "04.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 04: Matrix Testing — Regression Guard

**Status:** In Progress
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

- [x] Document the full matrix dimensions in this section
- [x] Identify which combinations are already covered by existing tests — extensive coverage in `iter_rc_matrix.rs` (iteration patterns), `fat_matrix/` (20 categories), `fat_ptr_iter.rs`; gaps in reassignment-in-loop and scope-context patterns
- [x] Identify which combinations are gaps (the cross-product minus existing coverage) — gaps: no `while` keyword in Ori (foreign keyword, use `loop`+`if`/`break`), no AOT while-loop tests existed
- [x] Prioritize: high-risk combinations (loops + heap types, branches + struct drops) first

---

## 04.2 Value Type × Loop Pattern Matrix

**File(s):** `compiler/ori_llvm/tests/aot/arc.rs` (or new file `compiler/ori_llvm/tests/aot/rc_matrix.rs`)

Test every value type being reassigned inside every loop pattern. Each test uses `assert_aot_success` (which enables `ORI_CHECK_LEAKS=1`).

Note: `while` is a foreign keyword in Ori — use `loop { if !cond then break; body }` equivalent.

| | `for` range | `loop`+`if`/`break` (≈while) | `for`+early `break` |
|---|---|---|---|
| `str` (SSO→heap) | `test_matrix_str_for_loop` | `test_matrix_str_while_loop` | `test_matrix_str_loop_break` |
| `[int]` push | `test_matrix_list_int_for_loop` | `test_matrix_list_int_while_loop` | `test_matrix_list_int_loop_break` |
| `[str]` push | `test_matrix_list_str_for_loop` | `test_matrix_list_str_while_loop` | `test_matrix_list_str_loop_break` |
| `{str: int}` insert | `test_matrix_map_for_loop` | `test_matrix_map_while_loop` | `test_matrix_map_loop_break` |
| Struct w/ heap | `test_matrix_struct_for_loop` | `test_matrix_struct_while_loop` | `test_matrix_struct_loop_break` |

- [x] Create `compiler/ori_llvm/tests/aot/rc_matrix.rs`
- [x] Add `pub mod rc_matrix;` to `compiler/ori_llvm/tests/aot/main.rs`
- [x] Implement all 15 loop matrix tests (5 types × 3 loop patterns)
- [x] Each test: 30 iterations, verify correct result AND zero leaks
- [x] All 15 tests pass

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

- [x] Implement all 25 scope matrix tests (5 types × 5 contexts)
- [x] Each test verifies correct result AND zero leaks
- [x] All 25 tests pass
- [x] **FIXED: `test_matrix_str_if_else` leak** — select-fold optimization eagerly materialized both if-else branches for heap types. Fix: `detect_select_diamond` now rejects diamonds with non-scalar merge params (Criterion 7). Semantic pin: `select_not_folded_non_scalar_merge_param` in `ori_arc/src/block_merge/tests.rs`.

---

## 04.4 Nested & Composed Pattern Matrix

Test combinations that compose multiple dimensions — the highest-risk patterns.

- [x] `test_matrix_struct_with_list_in_loop` — Struct containing `[int]` reassigned in loop
- [x] `test_matrix_list_of_strings_in_loop` — `[str]` with push in loop (nested RC: list + string elements)
- [x] `test_matrix_string_in_if_else_in_loop` — String conditionally updated in loop
- [x] `test_matrix_slice_in_scope` — Create slice, use, let both slice and original drop — **FIXED: double-free** — `emit_auto_iter` and `emit_rc_inc_clone` called `ori_rc_inc(data)` directly instead of slice-aware `ori_list_rc_inc(data, cap)`. Fixed via `emit_slice_aware_rc_inc` helper.
- [x] `test_matrix_slice_in_loop` — Create slices in a loop
- [x] `test_matrix_multiple_heap_locals` — Multiple independent heap variables in one scope
- [x] `test_matrix_heap_var_shadowing` — Shadow a heap variable with a new heap value
- [x] `test_matrix_closure_captures_string` — Lambda capturing a heap string, called, then dropped
- [x] `test_matrix_closure_captures_list` — Lambda capturing a `[int]`, called, then dropped
- [x] `test_matrix_closure_in_loop` — Lambda created inside loop body capturing loop variable, used and dropped each iteration

---

## 04.5 Journey Score Regression Guard

Ensure the 10/10 code journey scores cannot regress.

- [x] Create `compiler/ori_llvm/tests/aot/journey_guard.rs`
- [x] Add `pub mod journey_guard;` to `compiler/ori_llvm/tests/aot/main.rs`
- [x] For each of the 20 journeys (13 original + 7 new):
  - Compile the journey `.ori` file from `plans/code-journeys/NN-name.ori`
  - Run with `ORI_CHECK_LEAKS=1`
  - Verify exit code matches expected value (each journey's `@main` returns an `int` exit code; stored as named constants in `journey_guard.rs`)
  - Verify zero leaks (exit code != 2)
- [x] Each test must hard-fail (not skip) if the journey `.ori` file is missing
- [x] These tests run as part of `cargo test -p ori_llvm --test aot`
- [x] Verify these tests are included in `./test-all.sh` via the existing `cargo test -p ori_llvm --test aot` invocation — confirmed: 1797 AOT tests pass

---

### Cleanup (Applies to rc_matrix.rs and journey_guard.rs creation)

- [x] **[BLOAT]** `compiler/ori_llvm/tests/aot/util.rs` — No new helpers added to `util.rs`. Journey guard uses its own `run_journey`/`assert_journey` helpers inline. Matrix tests use only `assert_aot_success` from util.

---

## 04.R Third Party Review Findings

- [ ] `[TPR-04-001][medium]` `plans/rc-integrity/index.md:1` — RC Integrity status metadata drifted into contradictory states before review.
  Evidence: the current tree marked the plan index `status: resolved`, while `plans/rc-integrity/00-overview.md` remained `status: in-progress`; this section's frontmatter was `status: complete` while the body still said `**Status:** Not Started`.
  Impact: downstream readers cannot trust whether the matrix/verification work is actually closed, and new third-party review findings would be hidden behind a resolved plan state.
  Required plan update: keep the plan/index active until Section 04 and Section 05 findings are cleared, and keep section body/frontmatter status text synchronized in the same edit pass.

---

## 04.N Completion Checklist

- [x] Matrix test file created (`rc_matrix.rs`) and registered in `main.rs`
- [x] Journey guard file created (`journey_guard.rs`) and registered in `main.rs`
- [x] 15 loop matrix tests pass (5 types × 3 loop patterns)
- [x] 25 scope matrix tests pass (5 types × 5 contexts) — all 25 pass after select-fold fix
- [x] 10 nested/composed matrix tests pass (7 original + 3 closure) — all 10 pass after slice RC fix
- [x] 20 journey guard tests pass (all 20 journeys)
- [x] Total: 70 new tests, all 70 passing with zero leaks
- [x] `timeout 150 ./test-all.sh` green — 13,456 passed, 0 failed
- [x] `./clippy-all.sh` green
- [x] No regressions in existing tests — 1799 AOT tests pass (was 1797 before, +2 new passing)

**Exit Criteria:** 66+ matrix tests covering the cross-product of value types, operations, and contexts. Every combination that could regress has an explicit test. Journey scores are guarded by automated tests that fail on any regression.
