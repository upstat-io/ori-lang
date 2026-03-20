---
section: "04"
title: "Combinatorial Test Matrix"
status: not-started
goal: "Write comprehensive cross-product tests: 9 type categories x 10 language features x 4 execution modes"
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
    title: "Language Features (F1-F10)"
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

**Warning — File size**: The existing `fat_ptr_iter.rs` is 184 lines with 6 tests. Adding 40+ tests will push it well past the 500-line limit. Plan to split into submodules by test category: `fat_ptr_iter/str_list.rs`, `fat_ptr_iter/nested_list.rs`, `fat_ptr_iter/map_set.rs`, `fat_ptr_iter/control_flow.rs`, etc. The main `fat_ptr_iter.rs` becomes a `mod.rs` that re-exports submodules.

---

## 04.1 Type Categories (T1-T9)

Each type category represents a collection element type that requires Drop semantics. All heap strings must exceed 23 bytes (SSO threshold).

- [ ] **T1: `[str]`** — list of heap-allocated strings (the original motivating case)
- [ ] **T2: `[[int]]`** — list of lists (inner lists are RC-managed buffers)
- [ ] **T3: `[[str]]`** — list of lists of strings (doubly-nested fat pointers)
- [ ] **T4: `[{name: str, age: int}]`** — list of structs with string fields
- [ ] **T5: `[Option<str>]`** — list of optional strings (sum type with fat pointer payload)
- [ ] **T6: `[Result<str, str>]`** — list of results with fat pointer in both variants
- [ ] **T7: `[(str, int)]`** — list of tuples with a string component
- [ ] **T8: `{str: int}`** — map with string keys (map iteration path)
- [ ] **T9: `Set<str>`** — set with string elements (set iteration path, shares list iter but uses hash table layout)

---

## 04.2 Language Features (F1-F10)

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

---

## 04.3 Execution Modes (M1-M4)

Each test runs in multiple modes. The AOT test (`assert_aot_success`) implicitly covers M1 + M2.

### M1: Correctness (exit code 0)

- [ ] All F1-F10 tests return correct results (exit code 0)

### M2: Leak Detection (`ORI_CHECK_LEAKS=1`)

- [ ] All F1-F10 tests report zero leaks (`assert_aot_success` already enables `ORI_CHECK_LEAKS=1`)

### M3: Behavioral Equivalence (interpreter vs AOT)

For a representative subset (T1-F1, T1-F2, T1-F5, T2-F6, T4-F1):
- [ ] Run with interpreter (`ori run`) and compare output to AOT binary
- [ ] Use `diagnostics/dual-exec-verify.sh` for automated comparison

### M4: Release Build

- [ ] Build with `cargo b --release` and re-run the full test matrix
- [ ] This is mandatory, not optional — debug and release LLVM IR differ due to FastISel behavior (see llvm.md)

---

## 04.4 Valgrind Verification

**File:** `tests/valgrind/fat_ptr_iter/` (new directory)

Create standalone `.ori` programs for Valgrind testing (separate from the Rust AOT tests).

- [ ] Create `tests/valgrind/fat_ptr_iter/` directory
- [ ] `str_list_full.ori` — T1-F1: full `[str]` iteration
- [ ] `str_list_break.ori` — T1-F2: partial `[str]` iteration with break
- [ ] `str_list_two_calls.ori` — T1-F5: `[str]` passed to function twice
- [ ] `nested_list.ori` — T2-F6: `[[int]]` nested iteration
- [ ] `nested_str_list.ori` — T3-F6: `[[str]]` nested iteration
- [ ] `struct_with_str.ori` — T4-F1: `[{name: str}]` iteration
- [ ] `option_str.ori` — T5-F1: `[Option<str>]` iteration
- [ ] `map_str_key.ori` — T8-F1: map with string keys iteration
- [ ] Run all with `diagnostics/valgrind-aot.sh tests/valgrind/fat_ptr_iter/` — zero errors, zero leaks

### Cleanup

- [ ] **[STYLE]** `compiler/ori_llvm/tests/aot/fat_ptr_iter.rs:15-17,111-113,135-137,159-161` — Remove 4 sets of decorative `// -----------------------------------------------------------------------` banners. Replace with plain section comments per hygiene rules (no decorative characters). Apply this to all new test code as well.
- [ ] **[BLOAT]** `compiler/ori_llvm/tests/aot/fat_ptr_iter.rs` — Split into directory module with subfiles by category when adding the test matrix (see warning above). Target structure: `fat_ptr_iter/mod.rs` + category files.

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] At least 40 AOT tests in `fat_ptr_iter/` covering T1-T9 x F1-F10 matrix
- [ ] All AOT tests pass with `ORI_CHECK_LEAKS=1`
- [ ] 8+ Valgrind test programs in `tests/valgrind/fat_ptr_iter/`
- [ ] All Valgrind tests report zero errors
- [ ] Dual-exec verification passes for representative subset
- [ ] All tests pass in release build (`cargo b --release && timeout 150 cargo test -p ori_llvm --test aot`)
- [ ] `timeout 150 ./test-all.sh` passes with zero failures
