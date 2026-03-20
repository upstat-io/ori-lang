---
section: "05"
title: "Verification & Merge Gate"
status: complete
goal: "Zero leaks, zero regressions, all 20 journeys correct, all matrix tests green — branch merge-ready"
depends_on: ["01", "02", "03", "04"]
third_party_review:
  status: resolved
  updated: 2026-03-20
sections:
  - id: "05.1"
    title: "Full Test Suite"
    status: complete
  - id: "05.2"
    title: "Leak Verification"
    status: complete
  - id: "05.3"
    title: "Journey Score Verification"
    status: complete
  - id: "05.4"
    title: "Release Build"
    status: complete
  - id: "05.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "05.N"
    title: "Completion Checklist"
    status: complete
---

# Section 05: Verification & Merge Gate

**Status:** In Progress
**Goal:** Comprehensive verification that all fixes, tests, and journeys are complete. Zero leaks, zero regressions, all 20 journeys correct, all matrix tests pass. Branch is merge-ready.

**Depends on:** All of Sections 01-04.

---

## 05.1 Full Test Suite

- [x] `timeout 150 ./test-all.sh` — 13,460 passed, 0 failures
- [x] `./clippy-all.sh` — zero warnings
- [x] `./fmt-all.sh` — no formatting changes (auto-reformatted)
- [x] `cargo test -p ori_llvm --test aot` — 1,803 passed, 0 failed, 17 ignored (all pre-existing)
- [x] `diagnostics/dual-exec-verify.sh` — 11 mismatches, all pre-existing AOT gaps (closures/list printing, not regressions)
- [x] Verify no new `#[ignore]` attributes — 17 remaining (4 un-ignored as now passing), all pre-existing
- [x] Verify no new `#skip` attributes — all pre-existing (unimplemented features: variadics, pattern params, typed constants)
- [x] **Bonus:** 4 previously-ignored tests now pass and were un-ignored: `test_generic_option_match_leak`, `test_mono_nounwind_callee_uses_call_not_invoke`, `test_aot_catch_panic`, `test_mem_deep_recursion_200_with_strings`

---

## 05.2 Leak Verification

- [x] **Positive control:** Verified by `test_matrix_str_if_else` which correctly detected a 27-byte heap string leak (exit code 2) before the select-fold fix. The infrastructure detects leaks at the granularity of individual RC allocations with pointer + size attribution.
- [x] All 20 code journey binaries run with `ORI_CHECK_LEAKS=1` — zero leaks, correct exit codes
- [x] All 70 matrix tests pass with `ORI_CHECK_LEAKS=1` — zero leaks (enforced by `assert_aot_success`)
- [x] All 20 journey guard tests verify zero leaks (enforced by `assert_journey`)

---

## 05.3 Journey Score Verification

- [x] All 20 journeys produce correct exit codes via journey_guard.rs (J01–J20)
- [x] Journey guard tests are part of `cargo test -p ori_llvm --test aot` and `./test-all.sh`
- [x] All exit codes verified: J01=33, J02=17, J03=61, J04=57, J05=27, J06=41, J07=30, J08=57, J09=13, J10=33, J11=33, J12=33, J13=55, J14=65, J15=18, J16=42, J17=10, J18=67, J19=51, J20=105

---

## 05.4 Release Build

- [x] `cargo b && timeout 150 ./test-all.sh` — debug build, 13,460 passed, 0 failures
- [x] `cargo b --release` — release build succeeds with no warnings
- [x] `cargo b --release && timeout 150 ./test-all.sh` — release build, 13,460 passed, 0 failures
- [x] All 20 journey `.ori` files built with release binary + execute — correct exit codes
- [x] All 20 journey release binaries run with `ORI_CHECK_LEAKS=1` — zero leaks

---

## 05.R Third Party Review Findings

- [x] `[TPR-05-001][high]` `plans/rc-integrity/section-05-verification.md:45` — Section 05 marks the branch merge-ready even though the same verification log records unresolved AOT gaps.
  Resolved: Validated and integrated on 2026-03-20. All 17 ignored AOT tests are now explicitly tracked with ownership in the main roadmap — not waived as "pre-existing":
  - 12 tests (`iter_rc_matrix.rs`): catch() type inference bug → Section 10 § catch BUG item
  - 2 tests (`tuples.rs`): parser chained tuple field `.0.1` → Section 0 § 0.9 parser bugs
  - 1 test (`cli.rs`): incremental compilation → Section 21B § 21B.6
  - 1 test (`spec.rs`): inline panic in catch → Section 21A § 21A.5 catch codegen
  - 1 test (`generics.rs`): nounwind monomorphized callees → Section 21A § nounwind gap
  The "merge-ready" claim in 05.N is qualified: RC integrity work is complete, but the branch has pre-existing AOT gaps tracked in the main roadmap. Updated 05.N item below.
- [x] `[TPR-05-002][medium]` `plans/rc-integrity/section-05-verification.md:54` — The leak-detector positive-control checkbox is checked with a historical observation, not a reproducible current verification artifact.
  Resolved: Implemented on 2026-03-20. Added `leak_detection_positive_control` test in `ori_rt/src/tests.rs` — a permanent, reproducible positive control that:
  (1) Allocates via `ori_rc_alloc()` without `ori_rc_free()` (deliberate leak)
  (2) Asserts `RC_LIVE_COUNT` increments (the mechanism `check_leaks_and_exit()` reads)
  (3) Documents the full chain: `RC_LIVE_COUNT > 0` → `check_leaks_and_exit()` returns 2 → process exits with code 2
  Also updated AOT stub tests in `arc.rs` to reference this positive control.
  Run: `cargo test -p ori_rt leak_detection_positive_control`

---

## 05.N Completion Checklist

- [x] `./test-all.sh` green (13,460 tests, 0 failures)
- [x] `./clippy-all.sh` green
- [x] `cargo test -p ori_llvm --test aot` — 1,803 tests, 0 failures, 0 leaks
- [x] All 20 journeys produce correct exit codes and zero leaks
- [x] `cargo b --release && ./test-all.sh` green
- [x] `diagnostics/dual-exec-verify.sh` — 11 mismatches, all pre-existing AOT gaps
- [x] No new `#[ignore]` or `#skip` attributes added to suppress failures — 4 previously-ignored tests un-ignored (now pass)
- [x] Branch `experiment/aims` is merge-ready — RC integrity work complete; 17 pre-existing `#[ignore]` AOT tests tracked in main roadmap (Sections 0, 10, 21A, 21B)

**Exit Criteria:** All 70 matrix tests + 20 journey guards pass with zero leaks in both debug and release. No regressions. 4 previously-broken tests now pass. The select-fold leak and slice double-free bugs are fixed with semantic pin tests. Branch is ready to merge.
