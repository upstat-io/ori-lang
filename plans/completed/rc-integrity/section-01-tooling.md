---
section: "01"
title: "Tooling — Leak Detection Infrastructure"
status: complete
goal: "Every AOT binary checks for leaks at exit. test-all.sh fails on leaked memory. No silent leak passes."
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "ori_check_leaks() in Main Wrapper"
    status: complete
  - id: "01.2"
    title: "test-all.sh Integration"
    status: complete
  - id: "01.3"
    title: "Valgrind CI Script"
    status: complete
  - id: "01.4"
    title: "dual-exec-verify.sh Per-Test Timeouts"
    status: complete
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: complete
---

# Section 01: Tooling — Leak Detection Infrastructure

**Status:** In Progress
**Goal:** Every AOT-compiled binary checks for RC leaks at program exit. `test-all.sh` and `cargo test -p ori_llvm --test aot` fail when leaks are detected. No test can silently pass with leaked memory.

**Context:** `ORI_CHECK_LEAKS=1` was dead on Linux AOT because the LLVM-generated `main()` wrapper called `_ori_main()` directly with no leak check call afterward. (`ori_run_main()` is only used on Windows/MSVC for SEH, and contains a leak check — but that path is never used on Linux/Itanium.) The test harness (`compile_and_run_capture`) set the env var, but it had no effect. The fix was adding an explicit `ori_check_leaks()` call to the LLVM-generated main wrapper. This let 23+ tests pass for months while silently leaking memory.

---

## 01.1 ori_check_leaks() in Main Wrapper

**File(s):** `compiler/ori_rt/src/lib.rs`, `compiler/ori_llvm/src/codegen/function_compiler/entry_point.rs`, `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`

- [x] Add `ori_check_leaks()` FFI function to `ori_rt` — reads `ORI_CHECK_LEAKS` env var, checks `RC_LIVE_COUNT`, returns 0 (clean) or 2 (leaks) (2026-03-16)
- [x] Register `ori_check_leaks` in runtime function declarations (2026-03-16)
- [x] Modify `generate_main_wrapper()` to call `ori_check_leaks()` after `_ori_main()` returns — if non-zero, use leak exit code instead of main's (2026-03-16)
- [x] Verify: AOT binary with `ORI_CHECK_LEAKS=1` correctly reports leaks and exits with code 2 (2026-03-16)

---

## 01.2 test-all.sh Integration

**File(s):** `test-all.sh`, `compiler/ori_llvm/tests/aot/util/`

Ensure the test infrastructure properly reports leak failures.

- [x] Run a deliberately-leaking AOT test through `assert_aot_success` and confirm it fails with a message mentioning exit code 2 (leak), not a generic "non-zero exit" failure (2026-03-16)
- [x] Verify `compile_and_run_capture` sets `ORI_CHECK_LEAKS=1` for all AOT binary executions (2026-03-16)
- [x] Audit all AOT tests: ensure tests that should be leak-free use `assert_aot_success` (not bare `compile_and_run` which conflates exit code 2 with other failures) (2026-03-16)
- [x] Add explicit leak check to `test-all.sh` summary — report "X tests leaked memory" separately from "X tests failed" (2026-03-16)
- [x] Verify exit code precedence in `generate_main_wrapper`: if `_ori_main()` returns non-zero AND `ori_check_leaks()` returns 2, document which exit code the binary uses (current behavior: leak code overrides main code when non-zero) (2026-03-16)
- [x] Verify `./test-all.sh` reports 0 leaks after all Section 02 fixes — verified: 12,919 tests pass, 0 failures, 0 leaks (2026-03-17)

### Cleanup

- [x] **[BLOAT]** `compiler/ori_llvm/tests/aot/util.rs` (908 lines, exceeds 500-line limit) — Split into `util/mod.rs` (154 lines), `util/wasm.rs` (299 lines), `util/object.rs` (123 lines), `util/aot.rs` (376 lines). All existing imports via `crate::util::*` work unchanged via re-exports. (2026-03-16)

---

## 01.3 Valgrind CI Script

**File(s):** `diagnostics/valgrind-aot.sh`

Add a valgrind-based leak check that runs on code journey binaries as a secondary verification.

- [x] Update `diagnostics/valgrind-aot.sh` to auto-discover and run on all code journey binaries in `plans/code-journeys/` — added `--journeys` flag (2026-03-16)
- [x] Add `--leak-check=full --show-leak-kinds=all` flags (2026-03-16)
- [x] Script exits non-zero if any binary has "definitely lost" > 0 — uses `--errors-for-leak-kinds=definite --error-exitcode=42` (2026-03-16)
- [x] Add to `plans/code-journeys/` as a verification step (not in test-all.sh — too slow for CI) — added to `overview.md` verification steps (2026-03-16)

---

## 01.4 dual-exec-verify.sh Per-Test Timeouts

**File(s):** `diagnostics/dual-exec-verify.sh`

- [x] Add `PER_TEST_TIMEOUT=10` constant (2026-03-16)
- [x] Add `timeout -k 5` to interpreter run, AOT build, and AOT binary execution (2026-03-16)
- [x] Use file-based output capture instead of `$(timeout ...)` subshell — WSL2 abort hangs don't propagate SIGTERM through subshells (2026-03-16)
- [x] Handle exit codes 124 (SIGTERM) and 137 (SIGKILL) as timeout events (2026-03-16)
- [x] Add 120s timeout for batch `ori test` invocations (2026-03-16)

---

## 01.R Third Party Review Findings

- None.

---

## 01.N Completion Checklist

- [x] `ori_check_leaks()` callable from LLVM-generated main wrapper
- [x] AOT binaries report leaks when `ORI_CHECK_LEAKS=1` is set
- [x] `dual-exec-verify.sh` doesn't hang on crashing/aborting binaries
- [x] `test-all.sh` summary includes leak count (2026-03-16)
- [x] `diagnostics/valgrind-aot.sh` covers all code journeys — `--journeys` flag (2026-03-16)
- [x] All AOT tests use `assert_aot_success` (not bare `compile_and_run`) so leaks are reported distinctly (2026-03-16)
- [x] All AOT tests pass with leak detection enabled — 1315 AOT tests, 0 failures, 0 leaks via ORI_CHECK_LEAKS=1 (2026-03-17)

**Exit Criteria:** `ORI_CHECK_LEAKS=1` works for every AOT binary on every platform. `test-all.sh` fails loudly on any leaked memory. No test can silently pass with leaks.
