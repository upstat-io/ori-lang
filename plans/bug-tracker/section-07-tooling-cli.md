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

- [ ] `[BUG-07-005][low]` **Orphan env vars `ORI_NO_REPR_OPT` and `ORI_VERIFY_ARC` are read in source but not registered in `compiler/oric/src/debug_flags.rs`** — found by continue-roadmap.
  **Repro**: `diagnostics/check-debug-flags.sh` reports two ORPHAN entries:
  - `ORI_NO_REPR_OPT` — read at `compiler/ori_repr/src/plan/query.rs:36`
  - `ORI_VERIFY_ARC` — read at `compiler/oric/src/commands/codegen_pipeline.rs:381`, `compiler/oric/src/arc_dump/mod.rs:68`, `compiler/oric/src/arc_dot/mod.rs:60`
  **Impact**: Low — neither flag is broken at runtime; the consistency check fails (`diagnostics/self-test.sh` shows `check-debug-flags.sh FAIL`) and both flags are undocumented in CLAUDE.md. New users can't discover them.
  **Suggested fix**: Either (a) add both flags to `compiler/oric/src/debug_flags.rs` (`Flag` enum + `from_env_var` mapping + CLAUDE.md doc), or (b) remove the orphan call sites if the flags are obsolete. Surfaced during TPR-07-019 retrospective when running `diagnostics/self-test.sh` to verify the new `arc-dump.sh` script — neither flag is related to TPR-07-019 itself.
  Subsystem: `compiler/oric/src/debug_flags.rs` (registry) + the listed orphan call sites
  Found: 2026-04-07 | Source: continue-roadmap
  Note: `ORI_NO_REPR_OPT` is in active repr-opt territory; coordinate with the repr-opt reroute owner before removing.

- [ ] `[BUG-07-004][low]` **AOT test harness does not invalidate stale binaries when cross-crate deps change** — found by tpr-review.
  **Repro**: Make a change in `ori_arc` (e.g., the `apply_consuming_overrides` logic) that affects generated code. Run `cargo test -p ori_llvm --test aot <specific_test>` without touching `tests/aot/main.rs`. The test binary is rebuilt but `assert_aot_success` produces stale results — touching `tests/aot/main.rs` to force a full rebuild picks up transitive crate changes and tests pass. Observed twice during TPR-07-008 investigation: first with the initial triviality flip, again after the `annotate.rs` type-aware override.
  **Impact**: False "test failures" when iterating on cross-crate fixes. Wastes time chasing regressions that were already fixed.
  **Suggested fix**: AOT test util should either (a) include a build timestamp in the binary path so stale binaries are never reused, (b) invoke a cache-busting `cargo build -p oric` before each test run, or (c) use `cargo metadata` or file mtimes to detect dep-graph changes.
  Subsystem: `compiler/ori_llvm/tests/aot/util/aot.rs`
  Found: 2026-04-06 | Source: tpr-review (TPR-07-008)

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
