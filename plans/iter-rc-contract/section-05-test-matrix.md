---
section: "05"
title: "Comprehensive Test Matrix"
status: not-started
goal: "Combinatorial test coverage: 7 element types x 8 iteration patterns x 2 loop variants, exercising every RC-relevant combination"
third_party_review: false
depends_on:
  - "02"
  - "03"
sections:
  - id: "05.1"
    title: "Matrix Definition & Valid Combinations"
    status: not-started
  - id: "05.2"
    title: "Test Implementation"
    status: not-started
  - id: "05.3"
    title: "Matrix Verification"
    status: not-started
---

# Section 05: Comprehensive Test Matrix

**Status:** Not Started
**Goal:** Build a combinatorial test matrix covering 7 element types, 8 iteration patterns, and 2 loop variants. Every valid combination must have an AOT test that verifies correct output AND correct RC behavior (no leaks, no double-frees).

**Context:** The bugs in Sections 02-03 were discovered through specific element type + pattern combinations. A comprehensive matrix prevents regression and catches interactions between element types and loop structures that unit tests miss.

---

## 05.1 Matrix Definition & Valid Combinations

### Element Types (7)

| ID | Type | elem_dec_fn | Notes |
|----|------|-------------|-------|
| E1 | `str` | `_ori_elem_dec$<idx>` (decs heap data ptr) | Fat pointer (24-byte SSO) |
| E2 | `[int]` | `_ori_elem_dec$<idx>` (decs nested buffer RC) | Nested list, scalar elements |
| E3 | `Option<str>` | `_ori_elem_dec$<idx>` (tag-switch) | InlineEnum layout |
| E4 | `(int) -> int` | `_ori_elem_dec$<idx>` (decs env_ptr RC) | Closure with captured env |
| E5 | `{name: str}` | `_ori_elem_dec$<idx>` (decs str field RC) | User-defined struct |
| E6 | `{str: int}` map | `key_dec_fn` via `get_or_generate_elem_dec_fn(str)` + `val_dec_fn` = NULL (int is scalar) via `ori_iter_from_map` | Map iteration uses `IterState::Map` -> `ori_map_buffer_rc_dec` |
| E7 | `Set<str>` | `_ori_elem_dec$<idx>` (same as E1 -- sets share `emit_list_iter` path via `builtins/mod.rs:371`) | Validates the shared emit_list_iter path |

### Iteration Patterns (8)

| ID | Pattern | Description | Applies to For-Do | Applies to For-Yield |
|----|---------|-------------|-------------------|---------------------|
| P1 | Full iteration | Complete traversal, all elements consumed | Yes | Yes |
| P2 | Break | Early exit via `break` (for-do) or `break`/`break value` (for-yield returns accumulated list) | Yes (`break` only -- `break value` is E0860) | Yes per spec (Clause 16.10) -- **but blocked: `lower_for_yield_iterator` has no `LoopContext` setup (see Section 03.5)** |
| P3 | Yield | Transform each element | No (for-do has no yield) | Yes |
| P4 | Two-call | Source collection used in TWO for-loops | Yes | Yes |
| P5 | Nested | `for x in outer do for y in x do ...` | Yes | Yes |
| P6 | Guard | `for x in list if pred do/yield body` | Yes | Yes |
| P7 | Unwind+catch | `catch(expr: () -> { for x in list do panic_or_body })` | Yes | Yes |
| P8 | Continue | `continue` in body (skip rest of body). For for-yield: `continue` skips yield, `continue value` substitutes. | Yes | Yes (same `LoopContext` blocker as P2 -- see Section 03.5) |

### Loop Variants (2)

| ID | Variant | Description |
|----|---------|-------------|
| L1 | `for-do` | Side-effect loop: `for x in list do body` |
| L2 | `for-yield` | List comprehension: `for x in list yield expr` |

### Valid Combinations

Not all combinations are valid. The matrix excludes:
- For-do + P3 (yield): `for-do` has no `yield` keyword.

Both `break` and `break value` are valid in for-yield (spec Clause 16.10). For for-do, only bare `break` is valid (`break value` is error E0860).

**Total valid combinations**: 7 element types x (7 for-do patterns + 8 for-yield patterns) = **105 tests**

Detailed breakdown:
```
For-Do (L1):    E1-E7 x {P1, P2, P4, P5, P6, P7, P8} = 7 x 7 = 49 tests
For-Yield (L2): E1-E7 x {P1, P2, P3, P4, P5, P6, P7, P8} = 7 x 8 = 56 tests
                                                   Total: 105 tests
```

N/A combinations (exclude from total):
- E6 (map) + P5 (nested): Maps cannot be directly nested like lists -- skip or use `{str: [int]}`.
- E7 (set) + P5 (nested): Sets cannot be directly nested like lists -- mark N/A.
- E4 (closure) + P7 (unwind): Valid but closure capture + panic is an edge case -- include but deprioritize.

**P2 (break) and P8 (continue) for-yield blocker:** `lower_for_yield_iterator` does not set up `LoopContext`, so `break`/`continue` in for-yield body will not compile in AOT. If Section 03.5 does not fix this, all P2 x L2 and P8 x L2 tests (14 tests total: 7 element types x 2 patterns) must use `#skip("for-yield break/continue not yet lowered in AOT")`. The skip must reference this plan as the tracking item.

### Test Naming Convention

```
test_iter_rc_{loop_variant}_{element_type}_{pattern}
```

Examples:
- `test_iter_rc_for_do_str_full`
- `test_iter_rc_for_yield_option_str_guard`
- `test_iter_rc_for_do_nested_list_break`
- `test_iter_rc_for_yield_closure_two_call`

### Test File Location

```
compiler/ori_llvm/tests/aot/iter_rc_matrix.rs
```

**Registration:** Add `pub mod iter_rc_matrix;` to `compiler/ori_llvm/tests/aot/main.rs` (alphabetical order, between `iterators` and `linking`). Without this, the test file will not be compiled.

**Directory creation:** Create `tests/spec/iterators/rc_matrix/` for individual test programs (`.ori` files). This directory does not exist yet.

Individual test programs (`.ori` files) in:
```
tests/spec/iterators/rc_matrix/
```

- [ ] Enumerate all 105 valid combinations explicitly (49 for-do + 56 for-yield), marking any N/A combinations with justification
- [ ] Create directory structure: `tests/spec/iterators/rc_matrix/`
- [ ] Verify test naming convention has no conflicts with existing tests in `compiler/ori_llvm/tests/aot/`
- [ ] Create Valgrind test programs in `tests/valgrind/` for key combinations: str for-yield, option_str for-yield, map str keys for-do

---

## 05.2 Test Implementation

### Test Template

Each test follows this pattern:

```rust
#[test]
fn test_iter_rc_for_yield_str_full() {
    // assert_aot_success compiles, runs with ORI_CHECK_LEAKS=1, and asserts exit code 0.
    // Exit code 2 = leak detected, non-zero = panic or failure.
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold",
        "third long string for good measure in the test"
    ]
    let result = for s in items yield s
    if len(collection: result) == 3 then 0 else 1
}
"#,
        "iter_rc_for_yield_str_full",
    );
}
```

### Implementation Priority

Implement tests in this order (highest risk first):

1. **Critical (the bugs)**: E3 (Option<str>) x {P1, P3, P6} x {L1, L2} = 6 tests
2. **Fat pointers**: E1 (str) x all valid patterns x {L1, L2} = up to 15 tests
3. **Nested collections**: E2 ([int]) x all valid patterns x {L1, L2} = up to 15 tests
4. **Map/Set (parallel bug fix)**: E6 ({str: int} map) + E7 (Set<str>) x {P1, P4, P6} x {L1, L2} = 12 tests (focused on the map NULL dec fn fix from Section 02.3)
5. **Other types**: E4 (closures), E5 (structs) x all valid patterns x {L1, L2} = remaining tests
6. **Edge cases**: Empty lists, single-element lists, very large lists

**Test pattern notes:**
- Use `@main () -> int` returning 0 for pass, non-zero for fail. This is simpler than `assert_eq` in AOT tests.
- Use `assert_aot_success(src, name)` from `crate::util` -- it automatically sets `ORI_CHECK_LEAKS=1`.
- Strings in test programs should exceed SSO threshold (23 bytes) to exercise heap allocation. Use strings like `"this is a very long string that exceeds SSO threshold"`.
- If `assert_eq` is needed, add `use std.testing { assert_eq }` at the top of the test program. `assert_eq` is NOT in the prelude.

### Test Assertions

Every test asserts:
1. **Correct output**: The program produces the expected result (values, not just no-crash)
2. **No leaks**: `ORI_CHECK_LEAKS=1` produces no output on stderr
3. **No crashes**: Exit code 0 (or expected panic for unwind tests)
4. **Debug+Release parity**: Both builds produce the same result

- [ ] Implement priority 1 tests (6 critical tests for Option<str>)
- [ ] Implement priority 2 tests (str element tests across all patterns)
- [ ] Implement priority 3 tests (nested list tests across all patterns)
- [ ] Implement priority 4 tests (map and set tests for key patterns)
- [ ] Implement priority 5 tests (closure and struct element tests)
- [ ] Implement priority 6 edge case tests (empty, single-element, large)
- [ ] Each test passes in both debug and release builds
- [ ] Each test passes with `ORI_CHECK_LEAKS=1` reporting zero leaks

---

## 05.3 Matrix Verification

After all tests are implemented, run the full matrix and capture results:

### Verification Protocol

1. **Debug build**: `timeout 150 cargo test -p ori_llvm -- iter_rc`
2. **Release build**: `timeout 150 cargo test -p ori_llvm --release -- iter_rc`
3. **Leak check**: Run each test program binary with `ORI_CHECK_LEAKS=1`
4. **Valgrind**: Run representative subset (E1, E2, E3 x P1, P3, P6 x L1, L2 = 18 programs) with `diagnostics/valgrind-aot.sh`
5. **Dual-exec**: Run all test programs through `diagnostics/dual-exec-verify.sh` for interpreter-vs-AOT parity

### Results Matrix

Generate a results table:
```
| Test | Debug | Release | Leaks | Valgrind | Dual-Exec |
|------|-------|---------|-------|----------|-----------|
| for_do_str_full | PASS | PASS | 0 | 0 errors | MATCH |
| ... | ... | ... | ... | ... | ... |
```

- [ ] Run full matrix in debug build -- all tests pass
- [ ] Run full matrix in release build -- all tests pass
- [ ] Run leak check on all test programs -- zero leaks
- [ ] Run Valgrind on representative subset (18 programs) -- zero memory errors
- [ ] Run dual-exec-verify on all test programs -- interpreter matches AOT
- [ ] Capture and store results matrix for reference

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [ ] All valid test combinations implemented (105 total: 49 for-do + 56 for-yield, minus documented N/A like E6/E7 x P5)
- [ ] N/A combinations documented with justification
- [ ] All tests pass in debug build
- [ ] All tests pass in release build
- [ ] Zero leaks across all tests with `ORI_CHECK_LEAKS=1`
- [ ] Valgrind clean on representative subset (18 programs)
- [ ] Dual-exec-verify confirms interpreter-vs-AOT parity on all programs
- [ ] Results matrix captured and stored
- [ ] `timeout 150 ./test-all.sh` green (no regressions from adding tests)

---

## Section 05 Exit Criteria

All valid combinations in the 7x8x2 matrix (105 tests: 49 for-do + 56 for-yield, minus any N/A combinations) have passing AOT tests. Every test verifies correct output, zero leaks, and debug/release parity. Valgrind confirms no memory errors on representative programs. The matrix provides comprehensive regression coverage for the iterator-collection RC ownership contract.
