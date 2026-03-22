---
section: "05"
title: "Verification & Cleanup"
status: not-started
goal: "Full verification pass, confirm test stability, re-run code journeys, update documentation, ensure zero regressions"
depends_on: ["04"]
reviewed: false
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Verify Test Stability"
    status: not-started
  - id: "05.2"
    title: "Full Test Suite"
    status: not-started
  - id: "05.3"
    title: "Code Journeys"
    status: not-started
  - id: "05.4"
    title: "Documentation"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Verification & Cleanup

**Status:** Not Started
**Goal:** Final verification pass. All tests green, all code journeys re-run, documentation updated.

**Depends on:** Section 04 (test matrix must pass first).

---

## 05.1 Verify Test Stability

**File:** `compiler/ori_llvm/tests/aot/fat_ptr_iter.rs`

- [ ] Verify `test_str_list_passed_to_two_functions` (currently active, not ignored) passes reliably
- [ ] Verify `test_nested_list_iteration` (currently active, not ignored) passes reliably
- [ ] Run both tests 10 times each to verify no intermittent failures

---

## 05.2 Full Test Suite

- [ ] `timeout 150 ./test-all.sh` — all tests pass, 0 failures
- [ ] `./clippy-all.sh` — zero warnings
- [ ] `./fmt-all.sh` — no formatting changes
- [ ] `cargo b --release` — release build succeeds
- [ ] `timeout 150 cargo test -p ori_llvm --test aot` with release binary — all AOT tests pass in release mode
- [ ] `diagnostics/valgrind-aot.sh` — zero errors on default test set
- [ ] `diagnostics/dual-exec-verify.sh` — behavioral equivalence for all spec tests

---

## 05.3 Code Journeys

Re-run code journeys J15-J17 to verify improved scores.

- [ ] Re-run J15 (string list iteration) — verify eval and AOT produce identical results, score improvement from previous run
- [ ] Re-run J16 (aggregate emission) — verify no regressions
- [ ] Re-run J17 (closure capture) — verify no regressions
- [ ] Update journey results files with new scores

---

## 05.4 Documentation

- [ ] Update CLAUDE.md memory entry for "Fat Pointer Bugs" — mark the iterator-collection ownership contract as RESOLVED
- [ ] Update `plans/fat-pointer-hardening/section-01-iterator-ownership.md` — check off items completed by this plan
- [ ] Update `plans/rc-integrity/section-02-leak-fixes.md` — check off items related to element cleanup
- [ ] Add CLAUDE.md memory entry for the RC Header V5 layout change (32 bytes, 4 fields: data_size, elem_dec_fn, elem_count, strong_count)
- [ ] Update `.claude/rules/runtime.md`: the "RefCount" row references "8-byte header, `drop_fn` for children" -- update to "32-byte header (V5), `elem_dec_fn` + `elem_count` in header for element cleanup". The current "8-byte" is wrong even for V3 (was 16 bytes) -- fix the full drift from V3 to V5 in one pass.
- [ ] Update `docs/compiler/design/11-runtime/data-structures.md`: update layout diagram, header size, and add `elem_dec_fn` + `elem_count` field documentation (V5 = 32 bytes)
- [ ] Update `docs/compiler/design/11-runtime/reference-counting.md`: update all V3 references to V5, layout diagrams, size calculations (32 bytes, 4 fields)
- [ ] Update `plans/value-semantics-optimization/section-05-seamless-slices.md`: references `RC_HEADER_SIZE = 16` at line 275 and `original_data - RC_HEADER_SIZE` throughout -- update to 32
- [ ] Update `plans/aims-literature-review/section-10-concurrent-rc.md` line 421: references "16-byte header" -- update to 32
- [ ] Update `plans/aims-literature-review/section-11-cyclic-rc.md` lines 119 and 151: references "16-byte header" and "V3 header uses 16 bytes" -- update to V5/32
- [ ] Update `plans/repr-opt/section-09-arc-header.md`: this plan proposes narrowing the RC header -- must note that V5 has 4 fields (not 2). The narrowing baseline changes from 16 to 32 bytes.
- [ ] Update `plans/iter-rc-contract/` plan files: multiple sections reference `__for_coll` as the active workaround mechanism (section-01, section-02, section-03, section-04, 00-overview). Add a note at the top of each that the `__for_coll` mechanism was removed by the `rc-header-elem-dec` plan and replaced with header-based element cleanup. These are historical references, not active code, but readers must not assume the mechanism still exists.

### Cleanup

- [ ] **[DRIFT]** `.claude/rules/runtime.md` line 29 -- The "RefCount" row says "8-byte header" which was already wrong for V3 (should be "16-byte header"). Fix the full drift from V3 to V5 in one pass. Update to: `ori_rc_alloc`, `ori_rc_inc`, `ori_rc_dec`, `ori_rc_free` (32-byte V5 header: `data_size`, `elem_dec_fn`, `elem_count`, `strong_count`; `drop_fn` for non-buffer RC objects). Also add `ori_buffer_store_elem_dec`, `ori_buffer_store_elem_count` to the Functions table.
- [ ] **[NOTE]** `compiler/ori_rt/src/rc/mod.rs` -- The V5 layout comment (lines 46-62) is already compact and matches the module doc (lines 1-8). No consolidation needed.

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [ ] Zero `#[ignore]` tests related to fat pointer iteration
- [ ] All tests pass (`timeout 150 ./test-all.sh`)
- [ ] All tests pass in release mode (`cargo b --release && timeout 150 cargo test -p ori_llvm --test aot`)
- [ ] Valgrind clean on all `tests/valgrind/fat_ptr_iter/` programs with release binary
- [ ] All code journeys re-run with improved or maintained scores
- [ ] Documentation updated (runtime rules, design docs, referenced plan files)
- [ ] No stale "16-byte header" or "24-byte header" references in codebase (run `grep -rn "16-byte header\|24-byte header\|RC_HEADER_SIZE.*16\|RC_HEADER_SIZE.*24\|header.*16 bytes\|header.*24 bytes" compiler/ docs/` and verify zero results)
- [ ] No stale "V3" or "V4" references in runtime or codegen code (run `grep -rn "V3 layout\|V4 layout\|V3 header\|V4 header" compiler/ori_rt/ compiler/ori_llvm/` and verify zero results or update to V5)
- [ ] No stale `__for_coll` references that describe the mechanism as active (run `grep -rn "__for_coll" compiler/ plans/` — compiler/ results should be zero; plan references should be annotated as historical)
- [ ] `write_array_to_list` has `elem_dec_fn` parameter and all callers pass it correctly
- [ ] No `ori_rc_alloc` call sites in `set/cow/`, `list/cow*.rs`, `list/query.rs`, `list/mod.rs`, `iterator/consumers.rs`, `string/ops.rs`, `map/mod.rs`, `lib.rs` that create collection buffers without propagating `elem_dec_fn` to the header (run `grep -rn "ori_rc_alloc\|alloc_set_hash_buffer\|rehash_set" compiler/ori_rt/src/{list,set,iterator,string,map}/ compiler/ori_rt/src/lib.rs | grep -v test` and verify each has corresponding header-store call or is in the excluded list)
- [ ] Plan `status: active` changed to `status: resolved` in `index.md` frontmatter
- [ ] If fat-pointer-hardening Section 01 is now fully complete, update its status
