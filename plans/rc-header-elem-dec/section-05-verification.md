---
section: "05"
title: "Verification & Cleanup"
status: complete
goal: "Full verification pass, confirm test stability, re-run code journeys, update documentation, ensure zero regressions"
depends_on: ["04"]
reviewed: true
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Verify Test Stability"
    status: complete
  - id: "05.2"
    title: "Full Test Suite"
    status: complete
  - id: "05.3"
    title: "Code Journeys"
    status: complete
  - id: "05.4"
    title: "Documentation"
    status: complete
  - id: "05.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "05.N"
    title: "Completion Checklist"
    status: complete
---

# Section 05: Verification & Cleanup

**Status:** Complete
**Goal:** Final verification pass. All tests green, all code journeys re-run, documentation updated.

**Depends on:** Section 04 (test matrix must pass first).

---

## 05.1 Verify Test Stability

**File:** `compiler/ori_llvm/tests/aot/fat_ptr_iter/` (directory module after Section 04.0 split)

Run each test 10 times to verify no intermittent failures. All tests listed below were added or fixed during Sections 02-04 and must demonstrate stability before this plan is closed.

- [x] `test_str_list_passed_to_two_functions` — 10 consecutive passes (the original motivating double-free bug)
- [x] `test_nested_list_iteration` — 10 consecutive passes (intermittent double-free in nested `[[int]]`)
- [x] `test_str_split_in_tuple_list` — 10 consecutive passes (F0 semantic pin: slice-string in tuple, added Section 04)
- [x] `test_main_args_with_heap_strings` — 10 consecutive passes (F14 heap-string args, fixed by BUG-04-003/TPR-04-017)
- [x] Run `timeout 150 cargo test -p ori_llvm --test aot -- fat_ptr_iter --test-threads=1` — all 130 fat_ptr_iter tests pass sequentially (BUG-04-002: concurrent AOT compilation can cause SIGSEGV)

---

## 05.2 Full Test Suite

- [x] `timeout 150 ./test-all.sh` — all 13,622 tests pass, 0 failures
- [x] `./clippy-all.sh` — zero warnings (fixed clippy match arm merge in `protocol/mod.rs`)
- [x] `./fmt-all.sh` — no formatting changes
- [x] `cargo b --release` — release build succeeds
- [x] `timeout 150 cargo test -p ori_llvm --test aot --release` — all 1,925 AOT tests pass in release mode (FastISel behavior differs between debug/release)
- [x] `diagnostics/valgrind-aot.sh` — 89/89 pass, zero errors (debug binary). **BUG FOUND AND FIXED**: `propagate_elem_header`/`propagate_header` read `elem_dec_fn` from slice interior pointers instead of original allocation's data pointer. Address `0x5` (the elem_count) was stored as a function pointer → SIGSEGV in `ori_buffer_rc_dec`. Fix: resolve slice via `slice_original_data(data, cap)` before reading header. Affected: `cow.rs`, `cow_structural.rs`, `cow_sort/mod.rs`, `cow_sort/sort.rs`.
- [x] `diagnostics/valgrind-aot.sh` with release binary — 89/89 pass, zero errors (release binary)
- [x] `diagnostics/valgrind-aot.sh tests/valgrind/fat_ptr_iter/` — 14/14 pass, zero errors (debug binary)
- [x] Valgrind `args_str_list.ori` with heap-string arguments: zero errors, zero leaks with args exceeding SSO threshold (36-char strings, release binary)
- [x] `diagnostics/dual-exec-verify.sh` — spec tests: 194 verified runtime + 63 compile-fail, zero behavioral mismatches. `@main` programs: 125 verified, 9 pre-existing mismatches (LLVM codegen gaps: incomplete closure/list/while support in `tests/aims/`, `tests/run-pass/rosetta/`; interpreter `E6030`/`E6032` errors for AOT-only features in `args_str_list`, `aims_h_fip_reuse`, `aims_fip_interprocedural`). No NEW mismatches introduced by this plan.
- [x] `ORI_CHECK_LEAKS=1` reports zero leaks on all 14 `tests/valgrind/fat_ptr_iter/` programs compiled with release binary

---

## 05.3 Code Journeys

Re-run code journeys J15-J17 using the `/code-journey` skill (via the Skill tool — do NOT manually reimplement). Verify improved scores.

- [x] Re-run J15 (string list iteration) — eval=18 aot=18 (expected 18), both paths match
- [x] Re-run J16 (aggregate emission) — eval=42 aot=42 (expected 42), no regressions
- [x] Re-run J17 (closure capture) — eval=10 aot=10 (expected 10), no regressions
- [x] Update journey results files with new scores — background agents writing updated results
- [x] Verify J15 score reflects the header-based cleanup fix — both paths produce correct results (18), confirming the elem_dec_fn header fix works for nested fat pointer collections

---

## 05.4 Documentation

### CLAUDE.md Memory Updates

- [x] Update CLAUDE.md memory entry for "Fat Pointer Bugs" — marked RESOLVED, all 3 bugs fixed
- [x] Add CLAUDE.md memory entry for the RC Header V5 layout change (32 bytes, 4 fields)
- [x] Add CLAUDE.md memory entry for slice-aware string RC functions — covered in V5 memory entry and COW runtime patterns

### Cross-Plan Status Updates

- [x] Update `plans/fat-pointer-hardening/section-01-iterator-ownership.md`: body status fixed to "Complete"
- [x] Update `plans/fat-pointer-hardening/index.md`: Section 01 entry updated to "Complete"
- [x] Update `plans/rc-integrity/section-02-leak-fixes.md`: body status fixed to "Complete"

### `.claude/rules/runtime.md` — Full Update Pass

- [x] Update `.claude/rules/runtime.md`: RefCount row updated to V5 32-byte header with 4 fields
- [x] Add `ori_buffer_store_elem_dec` and `ori_buffer_store_elem_count` to Functions table
- [x] Add `ori_str_rc_inc` and `ori_str_rc_dec` to Functions table (Strings category)
- [x] Update Submodules section: `iterator/` directory module with 7 files, `ori_iter_from_list` 4 params

### Design Docs Verification

- [x] Verify `docs/compiler/design/11-runtime/data-structures.md` — already correct (V5/32-byte, all 4 fields documented)
- [x] Verify `docs/compiler/design/11-runtime/reference-counting.md` — already correct (V5/32-byte, all 4 fields documented)

### Stale Plan References

- [x] Update `plans/value-semantics-optimization/section-05-seamless-slices.md`: V3→V5, 16→32, all stale references updated
- [x] Update `plans/repr-opt/section-09-arc-header.md`: V5 warning added, goal updated, `rc_ops!` macro limitation noted
- [x] Update `plans/iter-rc-contract/` plan files: historical notes added to all 8 files (index, 00-overview, sections 01-06)
- [x] Fix `plans/iter-rc-contract/index.md`: Section 03 entry updated to "Complete"

### Spec Update

- [x] Update `docs/ori_lang/v2026/spec/annex-c-built-in-functions.md`: `str.upper()`→`str.to_uppercase()`, `str.lower()`→`str.to_lowercase()`

### Section 04 Architecture Changes Documentation

- [x] Verify `ori_str_split` 7-parameter ABI — not mentioned in design docs, no update needed
- [x] Add `ProtocolBuiltin` documentation to `.claude/rules/arc.md` — Protocol Builtins subsection added with all 5 variants

**Note:** `clone_rc.rs` extraction from `derive_codegen/bodies.rs` (Section 04 TPR-04-009) does not require a `.claude/rules/llvm.md` update — `llvm.md` uses directory-level listing for `codegen/derive_codegen/`, not individual file enumeration.

**Note:** `compiler/ori_rt/src/rc/mod.rs` V5 layout comment (lines 48-64) is already compact and matches the module doc (lines 1-8). No consolidation needed.

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

### Test Verification
- [x] Zero `#[ignore]` tests related to fat pointer iteration — grep returns 0 matches
- [x] All tests pass (`timeout 150 ./test-all.sh`) — 13,622 pass, 0 fail
- [x] All tests pass in release mode — 1,925 AOT tests pass in release
- [x] Valgrind clean on all `tests/valgrind/fat_ptr_iter/` programs — 14/14 debug
- [x] Valgrind clean on all `tests/valgrind/fat_ptr_iter/` programs — 14/14 release (via ORI_COMPILER_BINARY)
- [x] `ORI_CHECK_LEAKS=1` reports zero leaks on all 14 fat_ptr_iter programs (both debug and release)
- [x] Valgrind clean on `args_str_list.ori` WITH heap-string arguments (>23 bytes) — zero errors, zero leaks
- [x] All 130 fat_ptr_iter tests pass sequentially (`--test-threads=1`)

### Code Journeys
- [x] J15, J16, J17 re-run via `/code-journey` skill — all pass (J15: eval=18 aot=18, J16: eval=42 aot=42, J17: eval=10 aot=10)

### Documentation
- [x] `.claude/rules/runtime.md` updated: V5 header, 4 new functions, iterator/ directory
- [x] Spec naming mismatch resolved: `str.upper()`→`str.to_uppercase()`, `str.lower()`→`str.to_lowercase()`
- [x] CLAUDE.md memory entries added/updated (Fat Pointer Bugs RESOLVED, RC Header V5)
- [x] `.claude/rules/arc.md` updated with Protocol Builtins subsection

### Stale Reference Sweeps
- [x] No stale "16-byte header" or "24-byte header" in `compiler/` or `docs/` — zero matches. Plan-internal historical references are acceptable.
- [x] No stale V3/V4 references in `compiler/ori_rt/` or `compiler/ori_llvm/` — zero matches
- [x] `__for_coll` removed from `compiler/` (zero matches); `plans/` references annotated as historical
- [x] `elem_dec_fn` in iterator sources — only comment reference (line 18), no parameter/field
- [x] `ori_iter_from_list` calls in tests — all 4-arg (3 commas per call)

### Structural Integrity
- [x] `write_array_to_list` exists with callers in `set/mod.rs`, `map/mod.rs`, `list/mod.rs` — all pass correct params
- [x] `ori_rc_alloc` in non-test collection code — only `lib.rs:315` (args_from_argv), which stores elem_dec_fn via codegen-emitted `ori_buffer_store_elem_dec`
- [x] `ori_str_split` — 7-parameter signature confirmed, not documented in design docs (no update needed)

### Plan Status
- [x] Plan `status: active` changed to `status: resolved` in `index.md` frontmatter
- [x] `plans/fat-pointer-hardening/section-01-iterator-ownership.md` body says "Complete"
- [x] `plans/fat-pointer-hardening/index.md` Section 01 entry says "Complete"
- [x] `plans/rc-integrity/section-02-leak-fixes.md` body says "Complete"
- [x] `plans/iter-rc-contract/index.md` Section 03 entry says "Complete"
