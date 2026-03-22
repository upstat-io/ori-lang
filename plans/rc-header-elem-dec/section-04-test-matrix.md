---
section: "04"
title: "Combinatorial Test Matrix"
status: not-started
goal: "Write comprehensive cross-product tests: 9 type categories x 12 language features x 4 execution modes"
depends_on: ["03"]
reviewed: false
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Type Categories (T1-T9)"
    status: not-started
  - id: "04.2"
    title: "Language Features (F1-F12)"
    status: not-started
  - id: "04.3"
    title: "Execution Modes (M1-M4)"
    status: not-started
  - id: "04.4"
    title: "Valgrind Verification"
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
**Goal:** Write a comprehensive test matrix that covers fat pointer element cleanup across all type categories, language features, and execution modes. Every test must pass with `ORI_CHECK_LEAKS=1` and Valgrind.

**Depends on:** Section 03 (workarounds removed, clean codegen).

**Test file:** `compiler/ori_llvm/tests/aot/fat_ptr_iter.rs` (extend existing file)

**Warning -- File size**: The existing `fat_ptr_iter.rs` is 2211 lines (significantly expanded by iter-rc-contract plan tests). It is already well past the 500-line limit and MUST be split into submodules by test category: `fat_ptr_iter/str_list.rs`, `fat_ptr_iter/nested_list.rs`, `fat_ptr_iter/map_set.rs`, `fat_ptr_iter/control_flow.rs`, etc. The main `fat_ptr_iter.rs` becomes a `mod.rs` that re-exports submodules.

---

## 04.1 Type Categories (T1-T9)

Each type category represents a collection element type that requires Drop semantics. All heap strings must exceed 23 bytes (SSO threshold) to ensure they are heap-allocated and exercise the `elem_dec_fn` path.

- [ ] **T1: `[str]`** -- list of heap-allocated strings (the original motivating case)
- [ ] **T1b: `[str]` mixed SSO/heap** -- list containing both short strings (<= 23 bytes, SSO inline) and long strings (> 23 bytes, heap). Verifies that `elem_dec_fn` (`ori_str_rc_dec`) correctly skips SSO strings and only decs heap strings. This is a semantic pin: if the SSO check is broken, this test leaks or double-frees.
- [ ] **T2: `[[int]]`** — list of lists (inner lists are RC-managed buffers)
- [ ] **T3: `[[str]]`** — list of lists of strings (doubly-nested fat pointers)
- [ ] **T4: `[{name: str, age: int}]`** — list of structs with string fields
- [ ] **T5: `[Option<str>]`** — list of optional strings (sum type with fat pointer payload)
- [ ] **T6: `[Result<str, str>]`** — list of results with fat pointer in both variants
- [ ] **T7: `[(str, int)]`** — list of tuples with a string component
- [ ] **T8: `{str: int}`** — map with string keys (map iteration path)
- [ ] **T9: `Set<str>`** — set with string elements (set iteration path, shares list iter but uses hash table layout)

---

## 04.2 Language Features (F1-F12)

Each feature tests a different control flow or ownership pattern during iteration.

### F1: Full Iteration (`for x in coll do body`)

For each type T1-T9:
- [ ] Iterate the entire collection, use each element (e.g., `total = total + w.len()`)
- [ ] Verify correct result AND zero leaks

### F2: Partial Iteration with Break

For each type T1-T4:
- [ ] `for x in coll do { if condition then break; use(x); }`
- [ ] Verify un-consumed elements are correctly cleaned up

### F3: For-Yield

For each type T1-T4:
- [ ] `let derived = for x in coll yield transform(x);`
- [ ] Both the original collection and the derived collection must be leak-free

### F4: For with Guard

For each type T1-T2:
- [ ] `for x in coll if predicate(x) do body`
- [ ] Elements that fail the guard must be correctly cleaned up

### F5: Function Parameter Iteration

For each type T1-T4:
- [ ] Define `@f(coll: [T]) -> R` that iterates `coll`
- [ ] Call `f` TWICE with the same collection: `let a = f(coll: xs); let b = f(coll: xs);`
- [ ] Verify no double-free and correct results from both calls

### F6: Nested Iteration

For type T2 (`[[int]]`) and T3 (`[[str]]`):
- [ ] `for inner in outer do { for x in inner do body; }`
- [ ] Verify inner list cleanup happens correctly after each inner loop iteration

### F7: Continue with Value

For each type T1-T2:
- [ ] `for x in coll do { if skip_condition then { continue; }; use(x); }`
- [ ] Verify skipped elements are correctly cleaned up

### F8: Iteration in Match Arm

For type T1:
- [ ] `match some_option { Some(list) -> { for w in list do body; }, None -> 0 }`
- [ ] Verify correct cleanup regardless of which arm executes

### F9: Slice Iteration

For type T1 (`[str]`):
- [ ] Create `[str]`, take a slice, iterate the slice — verify the original buffer's elements are cleaned up when the last reference (slice or original) is dropped
- [ ] Verify `elem_dec_fn` is read from the ORIGINAL buffer's header (not the slice's data pointer)

### F10: For-Yield Producing Fat Pointers

For type T1 (`[str]`):
- [ ] `let derived = for w in words yield w;` — both `words` and `derived` are `[str]`, both need element cleanup
- [ ] Verify both the source and derived lists are leak-free

### F11: COW Mutation on Shared Collection

For type T1 (`[str]`), T8 (`{str: int}`), T9 (`Set<str>`):
- [ ] Create shared reference (let copy = original), then mutate copy via push/insert — verify both original and copy are leak-free
- [ ] Verify the COW slow path creates a new buffer with correct `elem_dec_fn` in header
- [ ] For sets: `Set<str>` union/intersection/difference on shared sets — verify new buffer cleanup

### F12: Collection Conversion (`map.keys()`, `set.to_list()`)

For type T8 (`{str: int}`) and T9 (`Set<str>`):
- [ ] `map.keys()` on `{str: int}` — exercises `write_array_to_list` producing `[str]`, verify output list has `elem_dec_fn` and zero leaks
- [ ] `map.values()` on `{int: str}` — exercises `write_array_to_list` producing `[str]`, verify zero leaks
- [ ] `str.split(sep:)` — exercises `write_array_to_list` producing `[str]`, verify zero leaks

---

## 04.3 Execution Modes (M1-M4)

Each test runs in multiple modes. The AOT test (`assert_aot_success`) implicitly covers M1 + M2.

### M1: Correctness (exit code 0)

- [ ] All F1-F12 tests return correct results (exit code 0)

### M2: Leak Detection (`ORI_CHECK_LEAKS=1`)

- [ ] All F1-F12 tests report zero leaks (`assert_aot_success` already enables `ORI_CHECK_LEAKS=1`)

### M3: Behavioral Equivalence (interpreter vs AOT)

For a representative subset (T1-F1, T1-F2, T1-F5, T2-F6, T4-F1):
- [ ] Run with interpreter (`ori run`) and compare output to AOT binary
- [ ] Use `diagnostics/dual-exec-verify.sh` for automated comparison

### M4: Release Build

- [ ] Build with `cargo b --release` and re-run the full test matrix
- [ ] This is mandatory, not optional — debug and release LLVM IR differ due to FastISel behavior (see llvm.md)
- [ ] Run `timeout 150 cargo test -p ori_llvm --test aot` with release binary -- ALL fat_ptr_iter tests pass
- [ ] Run Valgrind tests with release-compiled AOT binary -- zero errors, zero leaks

---

## 04.4 Valgrind Verification

**File:** `tests/valgrind/fat_ptr_iter/` (new directory)

Create standalone `.ori` programs for Valgrind testing (separate from the Rust AOT tests).

- [ ] Create `tests/valgrind/fat_ptr_iter/` directory
- [ ] `str_list_full.ori` — T1-F1: full `[str]` iteration, all strings > 23 bytes (heap)
- [ ] `str_list_mixed_sso.ori` -- T1b-F1: `[str]` with mixed SSO/heap strings -- verifies `elem_dec_fn` handles SSO correctly
- [ ] `str_list_break.ori` -- T1-F2: partial `[str]` iteration with break (un-consumed elements cleaned up)
- [ ] `str_list_two_calls.ori` — T1-F5: `[str]` passed to function twice (no double-free)
- [ ] `nested_list.ori` — T2-F6: `[[int]]` nested iteration
- [ ] `nested_str_list.ori` — T3-F6: `[[str]]` nested iteration (doubly-nested fat pointers)
- [ ] `struct_with_str.ori` — T4-F1: `[{name: str}]` iteration
- [ ] `option_str.ori` — T5-F1: `[Option<str>]` iteration
- [ ] `map_str_key.ori` — T8-F1: map with string keys iteration
- [ ] `cow_push_str.ori` -- `[str]` COW push on shared list -- verifies `elem_dec_fn` propagated to new buffer on slow path
- [ ] `collect_str.ori` -- `for w in words yield w` collecting into new `[str]` -- verifies `ori_iter_collect` output buffer
- [ ] `set_cow_insert.ori` -- `Set<str>` COW insert on shared set -- verifies `elem_dec_fn` propagation through set COW slow path
- [ ] `map_keys_str.ori` -- `map.keys()` on `{str: int}` -- verifies `ori_map_keys_to_list` output list buffer has correct `elem_dec_fn`
- [ ] `str_split.ori` -- `str.split(sep:)` returning `[str]` -- verifies `ori_str_split` output list buffer has correct `elem_dec_fn`
- [ ] `set_to_list.ori` -- `Set<str>` converted to `[str]` via `.to_list()` -- verifies `ori_set_to_list` output list buffer has correct `elem_dec_fn` and `elem_count`
- [ ] `args_str_list.ori` -- `@main(args: [str])` with command-line arguments -- verifies `ori_args_from_argv` list buffer cleanup (run with `valgrind-aot.sh -- arg1_longer_than_twenty_three_bytes arg2_longer_than_twenty_three_bytes` to force heap strings)
- [ ] Run all with `diagnostics/valgrind-aot.sh tests/valgrind/fat_ptr_iter/` -- zero errors, zero leaks
- [ ] Run all with `ORI_CHECK_LEAKS=1` -- zero leaks reported on stderr
- [ ] Run each Valgrind test 3x to verify stability (no intermittent failures)

### Cleanup

- [ ] **[STYLE]** `compiler/ori_llvm/tests/aot/fat_ptr_iter.rs` -- Remove all decorative `// -----------------------------------------------------------------------` banners. Replace with plain section comments. Apply during the directory-module split.
- [ ] **[BLOAT]** `compiler/ori_llvm/tests/aot/fat_ptr_iter.rs` -- Currently 2211 lines (4.4x the 500-line limit). Split into directory module with subfiles by category when adding the test matrix (see warning above). Target structure: `fat_ptr_iter/mod.rs` + category files. Split MUST happen BEFORE adding new tests, not after.

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] At least 50 AOT tests in `fat_ptr_iter/` covering T1-T9 x F1-F12 matrix (including T1b mixed SSO/heap, F11 COW mutation, F12 collection conversion)
- [ ] All AOT tests pass with `ORI_CHECK_LEAKS=1`
- [ ] 16+ Valgrind test programs in `tests/valgrind/fat_ptr_iter/` (8 original + SSO mix + COW push + collect + set_cow_insert + map_keys_str + str_split + set_to_list + args_str_list)
- [ ] All Valgrind tests report zero errors AND zero leaks
- [ ] All Valgrind tests verified stable (run 3x each, no intermittent failures)
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks on all Valgrind test programs
- [ ] Dual-exec verification passes for representative subset
- [ ] All tests pass in release build (`cargo b --release && timeout 150 cargo test -p ori_llvm --test aot`)
- [ ] `timeout 150 ./test-all.sh` passes with zero failures
