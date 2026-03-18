---
section: "05"
title: "Verification & Cleanup"
status: not-started
goal: "Full verification pass, unignore all tests, re-run code journeys, ensure zero regressions"
depends_on: ["04"]
reviewed: false
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Unignore Tests"
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

## 05.1 Unignore Tests

**File:** `compiler/ori_llvm/tests/aot/fat_ptr_iter.rs`

- [ ] Remove `#[ignore = "requires RC header extension..."]` from `test_str_list_passed_to_two_functions`
- [ ] Remove `#[ignore = "intermittent double-free..."]` from `test_nested_list_iteration`
- [ ] Run both tests 10 times each to verify no intermittent failures

---

## 05.2 Full Test Suite

- [ ] `timeout 150 ./test-all.sh` — all tests pass, 0 failures (Rust ~8,250 + spec tests) <!-- reviewed: accuracy fix — test count was stale -->
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
- [ ] Add CLAUDE.md memory entry for the RC Header V4 layout change
- [ ] Update `.claude/rules/runtime.md`: the "RefCount" row in the Functions table references "8-byte header, `drop_fn` for children" — update to "24-byte header (V4), `elem_dec_fn` in header for element cleanup" <!-- reviewed: completeness fix — runtime rule file has stale header size -->
- [ ] Update `docs/compiler/design/11-runtime/data-structures.md`: update layout diagram, header size, and add `elem_dec_fn` field documentation <!-- reviewed: completeness fix — design docs must reflect V4 -->
- [ ] Update `docs/compiler/design/11-runtime/reference-counting.md`: update all V3 references to V4, layout diagrams, size calculations <!-- reviewed: completeness fix -->
- [ ] Update `plans/value-semantics-optimization/section-05-seamless-slices.md`: references `RC_HEADER_SIZE = 16` at line 275 and `original_data - RC_HEADER_SIZE` throughout — update to 24 <!-- reviewed: completeness fix -->
- [ ] Update `plans/aims-literature-review/section-10-concurrent-rc.md` line 421: references "16-byte header" — update to 24 <!-- reviewed: completeness fix -->
- [ ] Update `plans/aims-literature-review/section-11-cyclic-rc.md` lines 119 and 151: references "16-byte header" and "V3 header uses 16 bytes" — update to V4/24 <!-- reviewed: completeness fix -->
- [ ] Update `plans/repr-opt/section-09-arc-header.md`: this plan proposes narrowing the RC header — it must be aware that V4 now has 3 fields, not 2. The narrowing target changes from 16 to 24 bytes baseline. <!-- reviewed: completeness fix — repr-opt plan has a dependency on header size -->

### Cleanup <!-- reviewed: hygiene fix -->

- [ ] **[DRIFT]** `.claude/rules/runtime.md` — The "RefCount" row says "8-byte header" which is already wrong for V3 (should be "16-byte header"). This is a pre-existing drift that would be compounded by V4. Fix the drift from V3 to V4 in one pass — don't just change 8 to 24.
- [ ] **[WASTE]** `compiler/ori_rt/src/rc/mod.rs` — After V4, verify the long prose block comment (lines 45-70) is updated and not duplicating information already in the module doc comment (lines 1-8). Consolidate if both describe the same layout.

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [ ] Zero `#[ignore]` tests related to fat pointer iteration
- [ ] All tests pass (`timeout 150 ./test-all.sh`)
- [ ] All tests pass in release mode (`cargo b --release && timeout 150 cargo test -p ori_llvm --test aot`)
- [ ] All code journeys re-run with improved or maintained scores
- [ ] Documentation updated (runtime rules, design docs, referenced plan files)
- [ ] No stale "16-byte header" references in codebase (run `grep -rn "16-byte header\|RC_HEADER_SIZE.*16\|header.*16 bytes" compiler/ docs/` and verify zero results) <!-- reviewed: completeness fix -->
- [ ] Plan `status: active` changed to `status: resolved` in `index.md` frontmatter
- [ ] If fat-pointer-hardening Section 01 is now fully complete, update its status
