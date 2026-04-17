---
section: "07"
title: "Tooling & CLI"
status: in-progress
goal: "Track and resolve all known tooling and CLI bugs"
sections: []
---

# Section 07: Tooling & CLI

**Subsystem:** `compiler/oric/`, `compiler/ori_fmt/`, `compiler/ori_diagnostic/`

Bugs in the CLI (`ori run`, `ori check`, `ori test`, `ori fmt`), formatter, diagnostic output, test runner, and build tooling.

---

## Open Bugs

- [ ] `[BUG-07-014][low]` **`tests/plan-audit/test_quick.py::test_completes_under_5_seconds` flakes under concurrent suite execution** — found by /roadmap-work §02.3 close-out.
  Repro: `timeout 120 python -m pytest tests/plan-audit/` — `run_quick` on the full corpus takes ~5.0s consistently. In isolation the test passes at ~5.01s; under the concurrent suite load it sometimes finishes at ~5.04s, tripping the `assert elapsed < 5.0` budget. The 5s budget was set with zero margin against a workload that routinely lands right at the edge.
  Subsystem: `tests/plan-audit/test_quick.py::TestRunQuickPerformance::test_completes_under_5_seconds`, `scripts/plan_corpus/quick.py` (if the root cause is an actual slowdown).
  Found: 2026-04-17 | Source: /roadmap-work §02.3 close-out (`plans/plan-bug-dag-ingestion/`). Not a §02.3 regression — `run_quick` does not import `export_json`, so the body_preview addition cannot affect it.
  Fix options: (a) raise budget to a headroom-aware value like 6.0s and document the concurrent-load rationale; (b) profile `run_quick` to find a deterministic slowdown and eliminate it; (c) move the perf test to a dedicated isolated runner with `pytest-xdist` group control. Preference: (b) → (a) as fallback; NEVER just raise the budget without profiling first.

- [ ] `[BUG-07-013][medium]` **`roadmap_scan.py` shadow parser — still duplicates `plan_corpus`'s frontmatter parsing** — found by tpr-review (custom-objective final check of verify-roadmap-redesign close-out).
  Repro: Read `.claude/skills/continue-roadmap/roadmap_scan.py` lines ~326–348 (`read_text(errors="replace")`, `split_frontmatter` local helper) and ~470 / ~559 (`parse_section_file`, `parse_index_file`). These are ~600 lines of parsing logic that duplicate `scripts/plan_corpus/load_and_validate`. The `errors="replace"` + `{}` on YAMLError pattern explicitly swallows errors the `plan_corpus` package was designed to surface (LEAK:swallowed-error), meaning `/continue-roadmap` and `/verify-roadmap --quick` can disagree on corpus parse results.
  Subsystem: `.claude/skills/continue-roadmap/roadmap_scan.py`, `scripts/plan_corpus/` (consumer migration)
  Found: 2026-04-15 | Source: tpr-review (custom-objective: "final check on verify-roadmap-redesign close-out") | Reviewer: codex
  Fix: `plans/bug-tracker/fix-BUG-07-013.md` (via `/fix-bug`)
  Note: This is the follow-up anchor for the closed `plans/completed/verify-roadmap-redesign/section-05-validation.md:190` migration checkbox. The plan marked the migration `[x]` under the 2026-04-15 user-override close-out, but the actual code change was never made. Filing here to give the migration a real trackable home so `/continue-roadmap` can surface it for future work. Scope: refactor `roadmap_scan.py` to import `plan_corpus.load_and_validate` as its sole frontmatter parser; eliminate `split_frontmatter`, `parse_section_file`, `parse_index_file`; keep only `/continue-roadmap`-specific logic (section selection, focus plan, health signals).

- [ ] `[BUG-07-012][minor]` **dual-tpr transport discards codex's successful envelopes when gemini fails persistently — no `codex.final.envelope.json` preserved on infra failure** — found by improve-tooling.
  Repro: Run `/tpr-review` (or `/review-work`, `/tp-help`, `/review-plan`) during a period of persistent gemini-3.1-pro-preview capacity pressure. `.claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh` will retry 5 times, each attempt potentially receiving a full codex envelope (rc=0, 400+ event streams, valid findings) that gets discarded because the merge step in `.claude/skills/dual-tpr/scripts/merge-findings.py` requires both `$RUN/codex.envelope.json` and `$RUN/gemini.envelope.json`. When the transport exits non-zero on exhausted infra retries, the codex envelope buried in the jsonl stream is valuable review work that the operator must manually extract to consume.
  Subsystem: `.claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh`, `.claude/skills/dual-tpr/scripts/merge-findings.py`, `.claude/skills/dual-tpr/transport.md`
  Found: 2026-04-15 | Source: improve-tooling (§02 close-out section-sweep during `plans/empty-container-typeck-phase-contract`)
  Note: Surfaced during §02.N close-out. Codex emitted valid envelopes on attempts 1, 2, 3, 4 (each with the same DRIFT finding on gate order at `check/validators/mod.rs:161`). All four were discarded; I had to grep `codex.jsonl` for the `schema_version` sentinel and parse the envelope manually via Python to act on the finding. Proposed fix: when the transport exits due to `gemini_api_capacity` (or any gemini-only infra failure) AND at least one codex attempt returned rc=0 with a parseable envelope, save that final envelope to `$RUN/codex.final.envelope.json` and have the exit message print a clear operator instruction: "Codex review available at `$RUN/codex.final.envelope.json` — 1 of 2 reviewers completed. To accept as codex-only best-effort, read that file; to retry dual-source, re-run the skill in a new RUN." Keep the semantic loop's clean-pass contract strictly requiring BOTH envelopes by default — this is an informational affordance, not a bypass of the dual-source requirement. Blast radius: the fix affects every consumer skill that calls `dual-invoke-with-retry.sh` (tpr-review, review-work, tp-help, review-plan), so `/fix-bug BUG-07-012` should run full TPR + hygiene review on the transport change.

- [x] `[BUG-07-009][low]` **`tracing-tree` dependency always compiled into oric, regardless of `ORI_LOG_TREE` usage** — found by dual-tpr-gemini §07.3 Scenario 1 dual-source /tp-help (gemini-only).
  Resolved: Closed as "working as designed" on 2026-04-09. The unconditional dependency is an intentional ergonomic choice: `ORI_LOG_TREE=1` works immediately for any developer without recompilation. Making it a feature gate would require `--features tree` before `ORI_LOG_TREE=1` does anything — strictly worse developer experience for marginal binary size savings. CLAUDE.md documents `ORI_LOG_TREE=1` as an always-available debugging tool.
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

- [x] `[BUG-07-005][low]` **Orphan env vars `ORI_NO_REPR_OPT` and `ORI_VERIFY_ARC` are read in source but not registered in `compiler/oric/src/debug_flags.rs`** — found by continue-roadmap.
  Resolved: Fixed 2026-04-09. Registered both flags in `debug_flags.rs` `flags!` macro. Exported `NarrowingPolicy::ENV_NO_REPR_OPT` constant from `ori_repr` and added compile-time sync assertion (matches `ORI_AUDIT_*` pattern). Updated 3 `ORI_VERIFY_ARC` call sites in oric to use `debug_flags::ORI_VERIFY_ARC` constant instead of string literal. Documented both flags in CLAUDE.md. `check-debug-flags.sh` now reports 15 flags, 0 orphan, 0 undocumented. 16,922 tests passing.
  Subsystem: `compiler/oric/src/debug_flags.rs`, `compiler/ori_repr/src/plan/query.rs`, `compiler/oric/src/commands/codegen_pipeline.rs`, `compiler/oric/src/arc_dump/mod.rs`, `compiler/oric/src/arc_dot/mod.rs`, `CLAUDE.md`
  Found: 2026-04-07 | Source: continue-roadmap

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

- [ ] `[BUG-07-010][low]` **`repr_setup.rs`: duplicated name mangling and method lowering dispatch**
  Repro: `compiler/oric/src/commands/repr_setup.rs` — (1) `collect_unconstrained_fn_names` at lines 317,357 reimplements `__impl_{idx}_{method}_{ordinal}` formatting that `make_qualified_name` at line 128 already provides. (2) Method lowering dispatch block (~20 lines) is duplicated between `lower_impl_methods_for_analysis` and `lower_default_trait_methods`.
  Subsystem: `compiler/oric/src/commands/repr_setup.rs`
  Found: 2026-04-11 | Source: tpr-review
  Reviewer: gemini (TPR round 4: [TPR-03-001-gemini-impl-r4], [TPR-03-002-gemini-impl-r4])
  Note: Active work in repr-opt plan touches this area.

- [ ] `[BUG-07-011][low]` **`plan-annotations.sh --fix` produces grammatically broken output for prose-embedded annotation IDs**
  Repro: With `// Note: exercises list collect (not __collect_set) — Set<[int]> crashes (BUG-04-065).` where `BUG-04-065` is `[x]` resolved, running `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --fix` strips only the bare token `(BUG-04-065)`, leaving the surrounding sentence broken. Worse case from incident: `// E7 (Set<int>) is blocked by BUG-04-065 (Set<int> iteration crashes in AOT).` becomes `// E7 (Set<int>) is blocked by (Set<int> iteration crashes in AOT).` — doubled parenthetical, "blocked by (" is broken grammar. `// ...blocked by BUG-04-065; these exercise...` becomes `// ...blocked by ; these exercise...` — dangling "blocked by ;".
  Subsystem: `.claude/skills/impl-hygiene-review/plan-annotations.sh` + `.claude/skills/impl-hygiene-review/plan-annotations.py`
  Found: 2026-04-14 | Source: continue-roadmap
  Note: Discovered during query-intel-adoption section 01 pre-flight stale-annotation cleanup. The 5 stale BUG-04-065 refs (2 in `fat_ptr_iter/method_collect.rs`, 3 in `iter_rc_matrix.rs`) were cleaned by hand in commit 178f117b after rejecting the --fix preview. Expected: when bare token stripping would produce broken prose (ID preceded by "by ", "in ", "for ", followed by "; ", ". ", or `(ID)` adjacent to another paren group), the tool should either (a) skip the line and flag it for manual review, (b) strip the surrounding load-bearing prose fragment too, or (c) emit an informative diagnostic. Today --fix claims success but the output is unusable. Suggested direction: lightweight regex/heuristic detection of dependent prose, fall back to skip+warn; optional LLM-assisted rewrite for flagged cases.

---

## Resolved Bugs

- None.
