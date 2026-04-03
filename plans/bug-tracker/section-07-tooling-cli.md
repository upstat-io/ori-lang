---
section: "07"
title: "Tooling & CLI"
status: open
goal: "Track and resolve all known tooling and CLI bugs"
sections: []
---

# Section 07: Tooling & CLI

**Subsystem:** `compiler/oric/`, `compiler/ori_fmt/`, `compiler/ori_diagnostic/`

Bugs in the CLI (`ori run`, `ori check`, `ori test`, `ori fmt`), formatter, diagnostic output, test runner, and build tooling.

---

## Open Bugs

- [x] `[BUG-07-001][medium]` **`--target` with missing/invalid value should show valid targets** — found by manual.
  Resolved: OBE on 2026-04-02. Target validation now active — `ori build --target=foo` emits `error[E5004]: target 'foo' is not supported` with full list of supported targets.
  Repro: `ori build hello.ori --target=` or `ori build hello.ori --target=foo`
  Expected: error listing valid targets (from `SUPPORTED_TARGETS` / `list_targets()`)
  Actual: proceeds with invalid target, fails cryptically at link time
  Subsystem: `compiler/oric/src/commands/build_options/parse_args.rs` (line 20 — accepts any string), `compiler/oric/src/commands/targets/mod.rs` (has `list_targets()` and `SUPPORTED_TARGETS`)
  Found: 2026-03-28 | Source: manual
  Note: Related to BUG-04-001 (cross-compilation failure). Early validation here would prevent the confusing linker error.

- [x] `[BUG-07-002][medium]` **`dual-exec-verify.sh` exits 0 with zero verified tests** — found by tpr-review.
  Resolved: Fixed on 2026-04-02. Added `TOTAL_VERIFIED` global counter that accumulates verified tests from both @test comparison (runtime PASS:PASS + compile-fail verified) and @main comparison. Final summary now distinguishes three states: mismatches (exit 1), zero verifications (exit 3, yellow warning), success (exit 0, shows count). New exit code 3 documented in script header.
  Subsystem: `diagnostics/dual-exec-verify.sh`
  Found: 2026-03-29 | Source: tpr-review (TPR-05-016)

- [x] `[BUG-07-003][medium]` **`dual-exec-verify.sh` compile_fail_verified counter inflates LLVM PASS totals as verified** — found by tpr-review.
  Resolved: Fixed on 2026-04-02. Removed the `compile_fail_verified = llvm_total - VERIFIED` calculation that incorrectly counted all non-compared LLVM passes as verified. `TOTAL_VERIFIED` now only accumulates `VERIFIED` (runtime cross-compared) and `MAIN_VERIFIED`. The `total_verified` display line uses `VERIFIED` directly. Verified: `operators_comparison.ori` now correctly shows "0 / 5 (0%)" and triggers the zero-verification warning (exit code 3) instead of falsely claiming "ALL VERIFIED (5 tests)".
  Subsystem: `diagnostics/dual-exec-verify.sh`
  Found: 2026-04-02 | Source: tpr-review
  Note: BUG-07-002 fix (exit code 3 for zero verifications) was incomplete — the `TOTAL_VERIFIED` counter was still inflated by the compile_fail_verified miscalculation. Now fixed.

---

## Resolved Bugs

- None.
