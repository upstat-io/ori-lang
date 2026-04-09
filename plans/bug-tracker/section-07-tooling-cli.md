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

- [ ] `[BUG-07-009][low]` **`tracing-tree` dependency always compiled into oric, regardless of `ORI_LOG_TREE` usage** — found by dual-tpr-gemini §07.3 Scenario 1 dual-source /tp-help (gemini-only).
  **Repro**: `compiler/oric/Cargo.toml` lists `tracing-tree` as an unconditional dependency, but it's only used inside the `if use_tree { ... }` branch in `compiler/oric/src/tracing_setup.rs:25-36`, which only fires when `ORI_LOG_TREE` is set at runtime (rare developer use case). The crate and its transitive deps (`ansi_term`, etc.) are compiled and linked into every `oric` release binary regardless.
  **Impact**: Low — marginal binary size increase for a rarely-used diagnostic feature. Not a runtime correctness issue.
  **Suggested fix**: Make `tracing-tree` an optional Cargo dependency gated behind a `tree` feature. `tracing_setup.rs` gates the tree branch with `#[cfg(feature = "tree")]` and `ORI_LOG_TREE` becomes a no-op unless the feature is enabled at build time. Document the feature in CLAUDE.md. Alternative: close as "working as designed" if binary-size minimalism isn't a current priority — the footprint is small and always-available `ORI_LOG_TREE=1` aids developer ergonomics.
  Subsystem: `compiler/oric/Cargo.toml` + `compiler/oric/src/tracing_setup.rs:25-36`
  Found: 2026-04-08 | Source: dual-tpr-gemini §07.3 Scenario 1 (gemini only)

- [x] `[BUG-07-008][medium]` **`tracing_setup.rs` uses `.init()` which panics if a global subscriber is already set** — found by dual-tpr-gemini §07.3 Scenario 1 dual-source /tp-help (gemini-only).
  Resolved: Fixed 2026-04-09 as part of the BUG-07-006 cluster fix. Replaced `.init()` with `.try_init().ok()` (via `let _ = ...try_init()`) at both subscriber-init sites in `tracing_setup.rs`. `try_init` returns `Err` instead of panicking when a subscriber is already set, and `let _` discards the result. Preserves first-wins semantics while tolerating concurrent subscribers.
  **Repro**: Set a global tracing subscriber before calling `oric::tracing_setup::init()` — e.g., from a test runner that wires up `tracing_subscriber::fmt::init()` for its own diagnostics, OR after `ori_llvm::init_tracing()` runs (see BUG-07-007). The second `.init()` call crashes the process with `"global default trace dispatcher has already been set"`.
  **Root cause**: `compiler/oric/src/tracing_setup.rs:36` and `tracing_setup.rs:46` both call `.init()` (the panicking variant from `SubscriberInitExt`). The `OnceLock<()>` guard prevents double-init from *oric itself*, but not from other subscribers set before oric's first call.
  **Impact**: Medium — setup-time crash in test runners or any composition scenario where multiple tracing consumers initialize. Not a runtime correctness issue but a stability/composability issue. Classified as `EXPOSURE`.
  **Suggested fix**: Replace `.init()` with `.try_init().ok()` — `try_init` returns `Err` instead of panicking when a subscriber is already set, and `.ok()` discards the result. Preserves the first-wins semantics (whoever initialized first owns the global subscriber) while tolerating concurrent subscribers. Aligns with `CLAUDE.md` stabilization discipline.
  Subsystem: `compiler/oric/src/tracing_setup.rs:36, 46`
  Found: 2026-04-08 | Source: dual-tpr-gemini §07.3 Scenario 1 (gemini only)

- [x] `[BUG-07-007][medium]` **Parallel tracing subscriber initialization in `ori_llvm::init_tracing` violates SSOT "One System One Owner"** — found by dual-tpr-gemini §07.3 Scenario 1 dual-source /tp-help (BOTH codex and gemini — high confidence via convergence).
  Resolved: Fixed 2026-04-09 as part of the BUG-07-006 cluster fix. Removed `init_tracing()` from `compiler/ori_llvm/src/init.rs` entirely (it had zero callers — verified via grep) and removed the `init_tracing` re-export from `compiler/ori_llvm/src/lib.rs`. The sole canonical owner of tracing initialization is `oric::tracing_setup::init()`. Also removed the `TRACING_INIT: Once` static and the `tracing_subscriber::{fmt, prelude::*, EnvFilter}` imports that were only used by the deleted function.
  **Repro**: Grep for `init_tracing` across the workspace — two parallel initializers exist:
  - `compiler/oric/src/tracing_setup.rs::init()` — supports `ORI_LOG`, `RUST_LOG` fallback, `ORI_LOG_TREE`, default `warn`
  - `compiler/ori_llvm/src/init.rs::init_tracing()` — supports only `RUST_LOG`, no tree mode, no `warn` default, uses its own `Once` guard
  Codex verified `ori_llvm::init_tracing` currently has NO in-repo caller (latent), but it's still a real SSOT violation waiting to bite if/when a caller lands.
  **Root cause**: `compiler/ori_llvm/src/init.rs:41-57` duplicates tracing setup logic that should live only in `oric` (per `.claude/rules/compiler.md`: "IO only in `oric`; core crates pure"). Two sources of truth = inevitable drift. Classified as `LEAK:shadow-home` (codex) / `DRIFT` + "One System One Owner" violation (gemini) — same finding, different category labels from the two reviewers, identical root cause.
  **Impact**: Medium-high if a caller lands. Latent today but compounds BUG-07-008: if any code calls `ori_llvm::init_tracing()` after `oric::tracing_setup::init()`, the second `.init()` call panics. Removing the `ori_llvm` initializer fixes both bugs.
  **Suggested fix**: Remove `ori_llvm::init_tracing()` entirely — `ori_llvm` should never configure global subscribers. If `ori_llvm` integration tests need tracing, they should either (a) rely on `oric::tracing_setup::init()` as the sole entry point via the oric binary, or (b) use `tracing-subscriber`'s `try_init()` in test-only code paths with `#[cfg(test)]`.
  Subsystem: `compiler/ori_llvm/src/init.rs:41-57` (remove) + `compiler/oric/src/tracing_setup.rs` (sole owner, no changes)
  Found: 2026-04-08 | Source: dual-tpr-gemini §07.3 Scenario 1 (dual-source convergence: both codex AND gemini independently surfaced)

- [x] `[BUG-07-006][high]` **`tracing_setup.rs`: silent fallback on malformed `ORI_LOG` parse error swallows user configuration** — found by dual-tpr-gemini §07.3 Scenario 1 dual-source /tp-help (BOTH codex and gemini — high confidence via convergence).
  Resolved: Fixed 2026-04-09. Extracted `build_filter()` helper that checks `std::env::var("ORI_LOG").is_ok()` to distinguish parse errors from not-present: if ORI_LOG is set but `EnvFilter::try_from_env` fails, emits `eprintln!("warning: ORI_LOG parse error: {e}; falling back to RUST_LOG or default filter")`. Verified: `ORI_LOG="[[[invalid" ori check /dev/null` now shows the warning on stderr and falls back to default. Also fixes BUG-07-007 (removed dead `init_tracing()` from `ori_llvm::init.rs` + removed re-export from `lib.rs`) and BUG-07-008 (replaced `.init()` with `.try_init().ok()` at both subscriber init sites in `tracing_setup.rs`). 16,921 tests passing.
  Subsystem: `compiler/oric/src/tracing_setup.rs`, `compiler/ori_llvm/src/init.rs`, `compiler/ori_llvm/src/lib.rs`
  Found: 2026-04-08 | Source: dual-tpr-gemini §07.3 Scenario 1 (dual-source convergence: both codex AND gemini independently surfaced)

- [ ] `[BUG-07-005][low]` **Orphan env vars `ORI_NO_REPR_OPT` and `ORI_VERIFY_ARC` are read in source but not registered in `compiler/oric/src/debug_flags.rs`** — found by continue-roadmap.
  **Repro**: `diagnostics/check-debug-flags.sh` reports two ORPHAN entries:
  - `ORI_NO_REPR_OPT` — read at `compiler/ori_repr/src/plan/query.rs:36`
  - `ORI_VERIFY_ARC` — read at `compiler/oric/src/commands/codegen_pipeline.rs:381`, `compiler/oric/src/arc_dump/mod.rs:68`, `compiler/oric/src/arc_dot/mod.rs:60`
  **Impact**: Low — neither flag is broken at runtime; the consistency check fails (`diagnostics/self-test.sh` shows `check-debug-flags.sh FAIL`) and both flags are undocumented in CLAUDE.md. New users can't discover them.
  **Suggested fix**: Either (a) add both flags to `compiler/oric/src/debug_flags.rs` (`Flag` enum + `from_env_var` mapping + CLAUDE.md doc), or (b) remove the orphan call sites if the flags are obsolete. Surfaced during TPR-07-019 retrospective when running `diagnostics/self-test.sh` to verify the new `arc-dump.sh` script — neither flag is related to TPR-07-019 itself.
  Subsystem: `compiler/oric/src/debug_flags.rs` (registry) + the listed orphan call sites
  Found: 2026-04-07 | Source: continue-roadmap
  Note: `ORI_NO_REPR_OPT` is in active repr-opt territory; coordinate with the repr-opt reroute owner before removing.

- [x] `[BUG-07-004][low]` **AOT test harness does not invalidate stale binaries when cross-crate deps change** — found by tpr-review.
  Resolved: OBE on 2026-04-09. The fix is already in place: `ensure_ori_binary_fresh()` in `compiler/ori_llvm/tests/aot/util/aot.rs:73-110` runs `cargo build -p oric --bin ori` exactly once per test process via `OnceLock`, matching the test profile. `ori_binary()` calls it before returning the binary path. This is option (b) from the suggested fix. Implemented as part of §07 TPR-07-017 work (doc comment at line 57-60 references the same symptom).
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
