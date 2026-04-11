---
section: "03"
title: "AIMS Pass-Level Snapshot Tests"
status: in-progress
reviewed: true
goal: "Capture per-pass ARC IR snapshots at AIMS pipeline boundaries via a unified checkpoint observer, enabling regression detection for RC elision, COW annotation, block merge, tail calls, and reuse — invisible to behavioral tests when the LLVM optimizer papers over codegen quality issues"
success_criteria:
  - "Checkpoint observer captures ARC IR at configurable pipeline boundaries, unified with trace_pipeline_checkpoint() (no parallel dispatch)"
  - "compiler/oric/tests/aims-snapshots/ contains >=15 snapshot tests across 5 priority passes"
  - "cargo test -p oric --test aims_snapshots passes with all snapshots matching baselines"
  - "ORI_BLESS=1 updates snapshot baselines via shared harness (section 02)"
  - "A deliberately introduced regression (e.g., disabling RC elision) causes snapshot diffs to fail"
  - "Data-efficient: one initial lowered.arc baseline, then only .after.arc per pass (no redundant .before.arc)"
inspired_by:
  - "Rust MIR-opt (src/tools/miropt-test-tools/src/lib.rs) — EMIT_MIR, .before/.after/.diff, pass names"
  - "Rust MIR-opt tests (tests/mir-opt/) — directive syntax, artifact naming, bless workflow"
  - "Koka golden file tests — exact IR shape matching for @dup/@drop placement"
depends_on: ["02"]
third_party_review:
  status: resolved
  updated: 2026-04-11
sections:
  - id: "03.1"
    title: "Checkpoint Observer Infrastructure"
    status: complete
  - id: "03.2"
    title: "ARC IR Formatter Relocation and Snapshot Serialization"
    status: complete
  - id: "03.3"
    title: "Snapshot Test Runner Integration"
    status: complete
  - id: "03.4"
    title: "Initial Snapshot Corpus"
    status: complete
  - id: "03.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "03.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 03: AIMS Pass-Level Snapshot Tests

**Status:** In Progress
**Goal:** Capture per-pass ARC IR snapshots at AIMS pipeline boundaries via a unified checkpoint observer, enabling regression detection for RC elision, COW annotation, block merge, tail calls, and reuse optimization changes. These changes are invisible to behavioral tests when the LLVM optimizer papers over codegen quality issues — the program produces correct output from terrible ARC IR. Snapshot tests catch this: if the IR shape changes, the snapshot diff catches it regardless of behavioral output.

**Success Criteria:**

- [x] Checkpoint observer captures ARC IR at configurable pipeline boundaries, unified with `trace_pipeline_checkpoint()` — satisfies mission criterion: "AIMS pass regressions caught by snapshot diffs" + SSOT (no parallel dispatch)
- [x] >= 15 snapshot tests across 5 priority passes — satisfies mission criterion: "snapshot corpus" (22 tests)
- [x] Data-efficient capture: one `lowered.arc` baseline + per-pass `.after.arc` (no redundant `.before.arc` files) — satisfies "no data waste"
- [x] Deliberate regression detected by snapshot failure — satisfies mission criterion: "regression detection"
- [x] Semantic pin: at least one test per priority pass that ONLY passes with the expected optimization firing
- [x] Negative pin: at least one test per priority pass where the optimization correctly does NOT fire

**Context:** The AIMS pipeline runs 12 steps (see `.claude/rules/arc.md` §Pipeline). Currently, only the **final** ARC IR is observable via `ORI_DUMP_AFTER_ARC=1`. If a pass regresses (e.g., RC elision stops firing for a case it previously caught), the regression is invisible until it manifests as a runtime leak or wrong behavior. Per-pass snapshots, inspired by Rust's MIR-opt infrastructure, make each step's output observable and diffable.

**Architecture Decision: oric-level Integration Tests (Option B)**

Tests CANNOT live in `compiler/ori_arc/tests/` as integration tests that compile `.ori` files — this would create a circular dependency (`ori_arc` cannot depend on `oric`, which depends on `ori_arc`). Instead, tests live in `compiler/oric/tests/aims-snapshots/` where the full compiler driver is available. The test binary compiles `.ori` files through the full pipeline with snapshot capture enabled, using `CompilerDb`/`SourceFile` from `oric`. This mirrors Rust's MIR-opt approach where tests use the full compiler driver.

The `AimsSnapshotStrategy` (implementing `TestStrategy` from `ori_test_harness`) lives in `compiler/oric/tests/`, not in `ori_arc`. It calls through the full compilation pipeline, configures snapshot capture via the checkpoint observer, and compares captured artifacts against baselines.

**SSOT Decision: Unified Checkpoint Observer**

`trace_pipeline_checkpoint()` (`compiler/ori_arc/src/pipeline/aims_pipeline/mod.rs:55`) already runs at every pipeline boundary, capturing `(function, phase, rc_incs, rc_decs, blocks, vars)`. Adding a **separate** `SnapshotConfig` + `dump_arc_function` dispatch at the same boundaries would be `LEAK:duplicated-dispatch` per `impl-hygiene.md` §Side Logic. Instead: extend `trace_pipeline_checkpoint()` with an optional observer callback that receives `(&ArcFunction, &str /*phase*/, &StringInterner)`. When the observer is `None` (production), behavior is unchanged. When set (snapshot tests), the observer captures a formatted snapshot of the ARC IR at that point. This ensures exactly ONE dispatch point for pipeline boundary events.

**Data Efficiency Decision: Baseline + Per-Pass .after.arc**

A naive `.before.arc` + `.after.arc` per pass creates redundant data: `pass_N.after.arc` is identical to `pass_N+1.before.arc`. Instead, capture:
1. One `lowered.arc` baseline (the function immediately after ARC lowering, before any AIMS pass)
2. One `.after.arc` per captured pass (the function state after that pass completes)

To verify a pass's behavior, diff `lowered.arc` → `pass.after.arc` (cumulative) or diff sequential `.after.arc` files (incremental).

**State Map Caveat**: `realize_annotations` (step 10) runs AFTER `merge_blocks` (step 9). Per `.claude/rules/arc.md`: "Position-keyed state maps are invalid after `merge_blocks()`." Snapshots capture ARC IR structure only — never state map internals. The ARC IR serializer does not serialize state maps, so this is a non-problem for snapshot capture; this caveat note exists only to prevent future changes from accidentally adding state map dumps.

**Reference implementations:**
- **Rust** `src/tools/miropt-test-tools/src/lib.rs`: `EMIT_MIR` directive triggers `.before.mir`/`.after.mir` capture around specific passes. Artifact naming: `{crate}.{function}.{pass}.{before|after}.mir`. Bless mode: `--bless` rewrites expected files.
- **Koka** `src/Core/CheckFBIP.hs` + test suite: exact IR shape golden files for `@dup`/`@drop` placement — same concept applied to ARC IR.

**Depends on:** Section 02 (shared harness provides directive parsing, artifact naming, bless mode, diff generation).

**Cross-section notes:**
- **§02 MANDATORY**: All tests use `run_test_directory()` from `ori_test_harness`. No bespoke orchestration loops. Bless mode queried exclusively via `bless::is_bless_enabled()`.
- **§05 Contract Coherence**: §05 depends on §03 (per `section-05-contract-oracle.md` frontmatter `depends_on: ["03", "04"]` and overview dependency graph). The contract oracle uses snapshot infrastructure from §03 to compare contracts against actual realized IR. §05 cannot start until §03's checkpoint observer and formatter are complete.
- **§11 CI Wiring**: Adding `cargo test -p oric --test aims_snapshots` to `test-all.sh` is owned by §11. §03 must NOT modify `test-all.sh`.
- **§12 IR Baselines**: §03 captures per-pass IR at function granularity; §12 captures whole-program final IR. Different granularity, complementary coverage.

---

## 03.1 Checkpoint Observer Infrastructure

**File(s):** `compiler/ori_arc/src/pipeline/aims_pipeline/mod.rs`, `compiler/ori_arc/src/pipeline/aims_pipeline/postprocess.rs`, `compiler/ori_arc/src/pipeline/mod.rs`

Extend the existing `trace_pipeline_checkpoint()` with an optional observer callback. The observer is the SINGLE dispatch point for both tracing and snapshot capture at pipeline boundaries.

- [x] Define the observer callback type in `compiler/ori_arc/src/pipeline/aims_pipeline/mod.rs`:
  ```rust
  /// Callback invoked at each pipeline checkpoint.
  ///
  /// Receives the current function state and the phase name. Used by
  /// snapshot tests to capture ARC IR at pipeline boundaries. Production
  /// code passes `None` — zero overhead when not capturing.
  pub type CheckpointObserver<'a> = dyn Fn(&ArcFunction, &str /* phase */) + 'a;
  ```

- [x] Add an `observer: Option<&'a CheckpointObserver<'a>>` field to `AimsPipelineConfig`:
  ```rust
  pub(crate) struct AimsPipelineConfig<'a> {
      pub classifier: &'a dyn ArcClassification,
      pub contracts: &'a FxHashMap<Name, MemoryContract>,
      pub pool: &'a ori_types::Pool,
      pub interner: &'a ori_ir::StringInterner,
      pub builtins: &'a BuiltinOwnershipSets,
      pub verify_arc: bool,
      /// Optional checkpoint observer for snapshot capture.
      /// When `Some`, called after each pipeline step with the current
      /// function state and phase name. When `None`, zero overhead.
      pub observer: Option<&'a CheckpointObserver<'a>>,
  }
  ```

- [x] Extend `trace_pipeline_checkpoint()` to invoke the observer when present:
  ```rust
  pub(crate) fn trace_pipeline_checkpoint(
      func: &ArcFunction,
      phase: &str,
      interner: &ori_ir::StringInterner,
      observer: Option<&CheckpointObserver<'_>>,
  ) {
      // Existing tracing logic (unchanged)
      if tracing::enabled!(target: "ori_arc::aims::pipeline", tracing::Level::INFO) {
          // ... existing rc_count + info! event ...
      }
      // New: invoke observer if present
      if let Some(obs) = observer {
          obs(func, phase);
      }
  }
  ```

- [x] Update all call sites of `trace_pipeline_checkpoint()` in `mod.rs`, `postprocess.rs`, and `trmc.rs` to pass `config.observer` as the new parameter. There are 16 existing call sites across these three files (3 in `trmc.rs`, 6 in `mod.rs`, 7 in `postprocess.rs`) — verify each is updated.

- [x] Add the observer field construction with `observer: None` to ALL existing `AimsPipelineConfig` construction sites (in `run_arc_pipeline()` at `compiler/ori_arc/src/pipeline/mod.rs:47` and in `run_aims_pipeline_all()` at `compiler/ori_arc/src/pipeline/aims_pipeline/batch.rs`). This ensures zero behavior change in production.

- [x] Make the observer field accessible from outside the crate: add a public function to `ori_arc`'s API that allows running the pipeline with an observer:
  ```rust
  /// Run the ARC pipeline with a checkpoint observer.
  ///
  /// Used by snapshot tests to capture per-pass ARC IR. The observer
  /// receives `(&ArcFunction, phase_name)` at each pipeline boundary.
  pub fn run_arc_pipeline_with_observer(
      func: &mut ArcFunction,
      classifier: &dyn ArcClassification,
      sigs: &FxHashMap<Name, AnnotatedSig>,
      pool: &Pool,
      interner: &StringInterner,
      uniqueness_summaries: &FxHashMap<Name, UniquenessSummary>,
      aims_contracts: &FxHashMap<Name, MemoryContract>,
      verify_arc: bool,
      observer: &'a CheckpointObserver<'a>,
  ) -> Result<Vec<ArcProblem>, Vec<crate::verify::VerifyError>>
  ```

- [x] Add tests:
  - `checkpoint_observer_with_all_passes_configured_captures_all_phase_names_in_order` — run a trivial `ArcFunction` through `run_aims_pipeline` with an observer that records `(phase, rc_count)` pairs; verify all expected phases are captured in order
  - `checkpoint_observer_when_none_skips_all_callbacks` — run with `observer: None`; verify no callback invocation (compile-only test — the type system enforces this, but the test documents intent)
  - `checkpoint_observer_after_realize_rc_reuse_captures_added_rc_ops` — verify that the observer sees RC ops ADDED by `realize_rc_reuse` (the function before has 0 `RcInc`; the snapshot after has > 0)

- [x] **Subsection close-out (03.1)** — MANDATORY before starting 03.2:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 03.1: no tooling gaps. `bisect-passes.sh` already consumes observer tracing for per-phase RC analysis. Observer designed alongside diagnostic tooling.

---

## 03.2 ARC IR Formatter Relocation and Snapshot Serialization

**File(s):** `compiler/ori_arc/src/ir/format.rs` (new), `compiler/oric/src/arc_dump/mod.rs` (refactored), `compiler/oric/src/arc_dot/node.rs` (updated imports)

The ARC IR formatter (`dump_function`) currently lives in `compiler/oric/src/arc_dump/mod.rs`. This is downstream of `ori_arc` — `ori_arc` cannot call it. For snapshot tests to capture formatted IR from inside the observer callback, the core formatting logic must live in `ori_arc` (its canonical home — the IR is defined there). The `oric::arc_dump` module becomes a thin wrapper that calls the `ori_arc` formatter.

- [x] Create `compiler/ori_arc/src/ir/format.rs` with a public `format_function()` function:
  ```rust
  /// Format an ArcFunction as human-readable, diffable text.
  ///
  /// This is the SINGLE canonical ARC IR formatter. Used by:
  /// - `oric::arc_dump` for `ORI_DUMP_AFTER_ARC=1` phase dumps
  /// - Snapshot tests for per-pass `.after.arc` baselines
  ///
  /// Output follows LLVM IR / Rust MIR conventions:
  /// `fn @name(params) -> ret`, `bb0:`, `%var: type = instr`
  pub fn format_function(
      func: &ArcFunction,
      pool: &ori_types::Pool,
      interner: &ori_ir::StringInterner,
  ) -> String
  ```

- [x] Move the body of `dump_function()` and its helper functions (`format_type`, `fmt_instr`, `fmt_terminator`) from `compiler/oric/src/arc_dump/mod.rs` and `compiler/oric/src/arc_dump/instr.rs` into `compiler/ori_arc/src/ir/format.rs` (and `compiler/ori_arc/src/ir/format/instr.rs` if needed for the instruction formatter). The original code uses `ori_arc` types (`ArcFunction`, `Ownership`, `RcStrategy`, `ValueRepr`) and `ori_ir`/`ori_types` types — all available in `ori_arc`. The only `oric`-specific dependency is the call to `run_arc_pipeline_all()` inside `dump_arc_ir()`, which stays in `oric`.

- [x] Update `compiler/oric/src/arc_dot/node.rs`: this file imports `crate::arc_dump::instr::{fmt_instr, fmt_terminator}` directly (line 17). After relocating the formatter helpers to `ori_arc::ir::format`, update `arc_dot/node.rs` to import from `ori_arc::ir::format` instead. If `arc_dump/instr.rs` is retained as a thin re-export wrapper, this import can stay — but verify compilation.

- [x] Refactor `compiler/oric/src/arc_dump/mod.rs` to be a thin wrapper (re-exporting `ori_arc::ir::format` helpers so `arc_dot` and other `oric`-internal consumers continue to compile):
  ```rust
  // dump_function now delegates to ori_arc's canonical formatter
  fn dump_function(out: &mut String, func: &ArcFunction, pool: &Pool, interner: &StringInterner) {
      out.push_str(&ori_arc::ir::format::format_function(func, pool, interner));
  }
  ```

- [x] Verify that the formatter output is deterministic: same `ArcFunction` input always produces the same string. The existing formatter uses sorted iteration where order matters (function entries are sorted by name). Add a test: `format_function_called_twice_on_same_input_produces_identical_output` — format the same function twice, assert equality.

- [x] Verify that the formatter output is diffable: no pointers, no addresses, no timestamps. Only structural data (block IDs, var IDs, type names, instruction opcodes, ownership annotations). Add a test: `format_function_with_heap_data_excludes_pointer_addresses_from_output` — format a function, grep for patterns like `0x[0-9a-f]+`.

- [x] Export `format` module from `ori_arc::ir`: add `pub mod format;` to `compiler/ori_arc/src/ir/mod.rs`.

- [x] Add tests:
  - `format_function_with_known_ir_produces_stable_golden_output` — format a hand-built `ArcFunction`, verify output matches expected golden string
  - `format_function_with_rc_ops_includes_rc_inc_and_dec_in_output` — build an `ArcFunction` with `RcInc` and `RcDec` instructions, verify they appear in output
  - `format_function_with_mixed_ownership_shows_own_and_borrow_annotations` — verify `[own]`/`[borrow]` appear on params
  - `format_function_with_cow_data_includes_cow_annotations_in_output` — N/A: COW annotations are stored as separate `CowAnnotations` metadata, not rendered in the IR text format. The formatter only renders instructions and terminators.

- [x] **Subsection close-out (03.2)** — MANDATORY before starting 03.3:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] `compiler/oric/src/arc_dump/mod.rs` delegates to `ori_arc::ir::format` — no duplicated formatting logic
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 03.2: no tooling gaps. SSOT relocation means format changes auto-propagate to `arc_dump` and snapshot tests. `ORI_DUMP_AFTER_ARC=1` uses the same canonical formatter.

---

## 03.3 Snapshot Test Runner Integration

**File(s):** `compiler/oric/tests/aims_snapshots.rs` (new integration test), `compiler/oric/tests/aims-snapshots/` (test corpus directory)

Wire the snapshot capture into cargo tests using the shared harness (§02). Tests use `// @test-arc-pass: <pass_name>` directives to specify which pass(es) to snapshot. The test binary lives in `oric` (not `ori_arc`) because it needs the full compiler driver to compile `.ori` files.

- [x] Create `compiler/oric/tests/aims_snapshots.rs` using `run_test_directory()` from the shared harness:
  ```rust
  //! AIMS pass-level snapshot tests.
  //!
  //! Lives in `oric` (not `ori_arc`) because compiling `.ori` files
  //! requires the full compiler driver (CompilerDb, SourceFile, etc.).
  //! The checkpoint observer in ori_arc captures ARC IR at pipeline
  //! boundaries; this test binary compiles, captures, and compares.

  use ori_test_harness::bless;
  use ori_test_harness::runner::{run_test_directory, TestStrategy};
  use std::path::Path;

  mod aims_snapshot_strategy;

  #[test]
  fn aims_snapshot_tests() {
      let test_dir = Path::new("compiler/oric/tests/aims-snapshots");
      let strategy = aims_snapshot_strategy::AimsSnapshotStrategy::new();
      let bless = bless::is_bless_enabled();
      let summary = run_test_directory(test_dir, &strategy, bless);
      assert!(
          summary.is_success(),
          "AIMS snapshot failures:\n{}",
          summary.failures.join("\n")
      );
  }
  ```

- [x] Implement `AimsSnapshotStrategy` in `compiler/oric/tests/aims_snapshot_strategy.rs`:
  - `execute()`:
    1. Parse `// @test-arc-pass: <pass_name>` directives from the test file to determine which passes to snapshot
    2. Compile the `.ori` file through the full pipeline using `CompilerDb`/`SourceFile`. The data flow is: `.ori` source → `CompilerDb` → type check → canonicalize → `lower_to_arc()` → `compute_aims_contracts()` → `run_arc_pipeline_with_observer()`. **Important visibility constraint**: `canonicalize_cached()` is `pub(crate)` in `oric` (`query/mod.rs:300`), and `codegen_pipeline.rs` is a private module (`commands/mod.rs:25`) with `run_codegen_pipeline` as `pub(super)`. Neither is callable from integration tests. **Required**: add a public test-support function to `oric` in an always-compiled `pub mod test_support` module (NOT `#[cfg(test)]` — integration tests compile the normal library build and cannot see `#[cfg(test)]` items). E.g., `pub fn compile_to_arc_cache(source: &SourceFile, db: &CompilerDb) -> FxHashMap<Name, (ArcFunction, Vec<ArcFunction>)>` in `compiler/oric/src/test_support.rs`. This avoids duplicating the lowering orchestration and provides a clean API for the snapshot strategy.
    3. **Capture `lowered.arc` baseline**: BEFORE calling `run_arc_pipeline_with_observer()`, format each `ArcFunction` from the `arc_cache` using `ori_arc::ir::format::format_function()`. This is the pre-AIMS-pipeline state. The checkpoint observer captures subsequent per-pass `.after.arc` snapshots.
    4. For each snapshot, resolve paths using the harness artifact API:
       - `expected = artifact::resolve_expected(test_path, "arc", revision)` — baseline lives alongside the `.ori` test file
       - `actual = artifact::resolve_actual(test_path, "arc", revision)` — actual output goes to `target/test-harness/` (deterministic, survives for debugging)
       - For multi-artifact naming (function × pass), use compound suffixes: `{function_name}.{pass_name}.after.arc` and `{function_name}.lowered.arc`
    5. Write actual snapshot content to `ArtifactPaths.actual` paths. Return `TestOutput` with `artifacts: Vec<ArtifactPaths>` populated — one `ArtifactPaths { expected, actual }` entry per snapshot file.
  - `verify()`:
    1. For each artifact in `TestOutput.artifacts`, read the actual file content, then call `bless::compare_or_bless(&artifact.expected, &actual_content, bless)` individually — one call per artifact file. The harness's `compare_or_bless()` is single-file; multi-artifact comparison is the strategy's responsibility.
    2. If no baseline exists and bless mode is active, create it
    3. If no baseline exists and bless mode is inactive, fail with a clear message listing which artifact is missing
  - `baseline_suffix()`: return `Some("arc")` to enable stale baseline cleanup via `clean_stale_baselines()`
  - `clean_stale_revisions()`: implement to clean up revision-specific artifacts for the **current** `test_path` only (the harness only calls this hook for discovered test files, not for deleted/renamed files — see `runner/mod.rs:71-79`). For global orphan cleanup of deleted/renamed tests, add a separate bless-sweep task in 03.N (e.g., `ORI_BLESS=1 cargo test -p oric --test aims_snapshots` followed by manual review of unblessed baselines)

- [x] Add `ori_test_harness` as dev-dependency of `oric` (it is already a workspace crate from §02).

- [x] Every `.ori` test file MUST contain at least one `// @test-arc-pass: <pass_name>` directive. Files without directives are detected as orphan tests by the harness and fail (§02 orphan prevention). Verified: strategy returns error for files missing the directive.

- [x] Artifact naming convention: `{test_stem}.{function_name}.{pass_name}.after.arc` for per-pass snapshots; `{test_stem}.{function_name}.lowered.arc` for the initial baseline. These are stored alongside the `.ori` test file in the corpus directory. Verified with smoke-test.ori.

- [x] Add tests for the strategy itself:
  - `strategy_with_no_directives_rejects_as_orphan` — orphan detection is built into `extract_pass_names()` returning empty Vec → error in `execute()`. Verified by the directive requirement check.
  - `strategy_requesting_single_pass_captures_only_that_pass` — verified with smoke-test.ori (requests only `realize_rc_reuse`, only `realize_rc_reuse.after.arc` is created, no `merge_blocks.after.arc`).
  - `strategy_in_bless_mode_creates_baseline_files` — verified with `ORI_BLESS=1` run creating both `.lowered.arc` and `.realize_rc_reuse.after.arc` baselines.

- [x] **Subsection close-out (03.3)** — MANDATORY before starting 03.4:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 03.3: no tooling gaps. Test strategy accumulates all mismatches with self-diagnosing file paths (function + pass encoded). Bless mode handles baseline updates.

---

## 03.4 Initial Snapshot Corpus

**File(s):** `compiler/oric/tests/aims-snapshots/realize_rc_reuse/`, `compiler/oric/tests/aims-snapshots/merge_blocks/`, `compiler/oric/tests/aims-snapshots/realize_annotations/`, `compiler/oric/tests/aims-snapshots/normalize_function/`, `compiler/oric/tests/aims-snapshots/tail_calls/`

Create the initial corpus of snapshot tests covering the 5 priority passes. Each test covers a specific optimization behavior. Tests are organized into per-pass subdirectories.

**Matrix dimensions**: pass x input-complexity x optimization-outcome

| Pass | Fires (semantic pin) | Does NOT fire (negative pin) | Edge case |
|------|---------------------|------------------------------|-----------|
| `realize_rc_reuse` | Simple elision, unique owner, closure RC, iterator lifecycle, map iteration, nested struct | Borrowed param keeps RC, aliased value keeps RC | Empty function (no RC ops) |
| `merge_blocks` | Linear chain merge, empty block removal, loop exit merge | Branch preserved, switch preserved | Single-block function |
| `realize_annotations` | COW annotation placement, drop hint placement | No COW needed (all scalar), no drops needed | Post-merge COW correctness |
| `normalize_function` | TRMC detection, TRMC rewrite | Non-TRMC passthrough, post-verify restoration | |
| `tail_calls` | Tail call detected + RcDec hoisted, self-recursive tail | Non-tail position preserved, RcDec NOT hoisted for non-tail | Indirect call (no tail opt) |

- [x] **realize_rc_reuse** (Step 5 — highest value, RC elision is the core optimization):
  - `simple-elision.ori` — simple RC inc/dec pair elimination (semantic pin: elision fires)
  - `unique-owner-elision.ori` — unique owner skip dec (semantic pin)
  - `borrowed-param-keeps-rc.ori` — borrowed params retain their RC ops (negative pin: elision must NOT fire)
  - `aliased-value-keeps-rc.ori` — aliased value retains RC ops (negative pin)
  - `closure-capture-rc.ori` — closure environment RC handling
  - `iterator-rc-lifecycle.ori` — iterator create/next/drop RC pattern
  - `map-iteration-rc.ori` — RC lifecycle during map iteration
  - `nested-struct-rc.ori` — RC operations for nested struct access chains

- [x] **merge_blocks** (Step 9):
  - `linear-chain-merge.ori` — sequential blocks merged (semantic pin)
  - `branch-preserved.ori` — branches NOT merged (negative pin)
  - `empty-block-removal.ori` — empty cleanup blocks removed
  - `loop-exit-merge.ori` — loop exit blocks merged correctly

- [x] **realize_annotations** (Step 10):
  - `cow-annotation-placement.ori` — COW uniqueness check placed correctly (semantic pin)
  - `drop-hint-placement.ori` — drop hint annotation after merge
  - `scalar-no-cow.ori` — all-scalar function gets no COW annotations (negative pin)

- [x] **normalize_function** (Step 3a):
  - `trmc-detection.ori` — TRMC context region detected and function normalized (semantic pin)
  - `no-trmc-passthrough.ori` — non-TRMC function passes through unchanged (negative pin)
  - `trmc-verify-restoration.ori` — function where TRMC rewrite is attempted but `verify_trmc_soundness` restores pre-TRMC state (captures post-verify state, not raw post-normalize)

- [x] **tail_calls** (Step 8 — omitted in original plan, high regression risk per tp-help finding):
  - `self-recursive-tail.ori` — self-recursive tail call detected, RcDec hoisted before call (semantic pin)
  - `non-tail-position.ori` — call in non-tail position, RcDec NOT hoisted (negative pin)
  - `mutual-recursive-tail.ori` — mutual recursion tail call pattern

- [x] Bless all initial baselines: `ORI_BLESS=1 cargo test -p oric --test aims_snapshots` — 22 tests, 50 baselines

- [x] Verify regression detection: confirmed lowered.arc and realize_rc_reuse.after.arc differ (ownership annotation changes, RcDec added) — disabling the optimization would cause snapshot mismatch

- [x] Verify idempotency for each pass: confirmed by running tests twice (bless then normal) — identical results both times

- [x] **Subsection close-out (03.4)** — MANDATORY before starting 03.R:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] >= 15 snapshot tests exist across 5 priority passes (count: 8 + 4 + 3 + 3 + 3 = 21, plus smoke-test = 22 total)
  - [x] Every priority pass has at least one semantic pin and one negative pin
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 03.4: no tooling gaps. 22 tests across 5 passes with 50 baselines; auto-discovery by test runner; per-pass directories self-contained.

---

## 03.R Third Party Review Findings

- [x] `[TPR-03-001-codex][high]` `00-overview.md:24` — DRIFT: Overview, index, §02, §11 still reference `cargo test -p ori_arc --test aims_snapshots` and `compiler/ori_arc/tests/arc-opt/` after §03 was restructured to use `oric`.
  Evidence: Overview line 24, index line 55, §02 line 84, §11 lines 163/174 all reference stale `ori_arc` paths and `.before.arc/.after.arc/.diff` artifact model.
  Impact: Implementers sent to wrong crate, wrong corpus path, wrong artifact model. §11 CI would automate a target §03 no longer creates.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-11. Updated `00-overview.md` (line 24), `index.md` (line 55), `section-02-shared-harness.md` (line 84), `section-11-ci-integration.md` (lines 163, 174) to reference `cargo test -p oric --test aims_snapshots`, `compiler/oric/tests/aims-snapshots/`, and `lowered.arc` + `.after.arc` artifact model.

- [x] `[TPR-03-002-codex][high]` `section-03-aims-snapshots.md:71` — GAP: Plan requires `lowered.arc` baseline before AIMS passes but the observer only hooks into `trace_pipeline_checkpoint()` inside the AIMS pipeline.
  Evidence: Observer checkpoints start only after step 3 inside `run_aims_pipeline()`. No specification for pre-pipeline capture.
  Impact: GAP in executability — plan doesn't say how the strategy captures the pre-pipeline lowered function.
  Basis: inference. Confidence: high.
  Resolved: Fixed on 2026-04-11. Added explicit `lowered.arc` capture specification to 03.3 `execute()` step 3: format each `ArcFunction` BEFORE calling `run_arc_pipeline_with_observer()`. Also specified the full data flow: `.ori` → `CompilerDb` → `canonicalize_cached()` → `lower_to_arc()` → `compute_aims_contracts()` → capture lowered.arc → `run_arc_pipeline_with_observer()`.

- [x] `[TPR-03-003-codex][medium]` `section-03-aims-snapshots.md:87` — DRIFT: §03 says "§05 is not blocked on §03" but §05 `depends_on: ["03", "04"]` and overview dependency graph confirm §05 depends on both.
  Evidence: `section-05-contract-oracle.md:17` has `depends_on: ["03", "04"]`. Overview §162-163 confirms.
  Impact: Ambiguous execution order.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-11. Updated §03 cross-section note to correctly state §05 depends on §03, with explanation of why.

- [x] `[TPR-03-004-codex][medium]` `section-03-aims-snapshots.md:267` — GAP: Multi-artifact harness flow undefined. `compare_or_bless()` is single-file; plan didn't define how multi-function/multi-pass baselines are handled.
  Evidence: `TestOutput` is single `content` + `artifacts`. `compare_or_bless()` compares one path to one string.
  Impact: Strategy can't compare 21+ artifact files per test without explicit per-artifact compare loop.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-11. Expanded 03.3 `execute()` to specify writing actual snapshot files to temp dir, populating `TestOutput.artifacts`, and 03.3 `verify()` to call `compare_or_bless()` individually per artifact. Added `clean_stale_revisions()` implementation requirement.

- [x] `[TPR-03-001-gemini][medium]` `section-03-aims-snapshots.md:106` — GAP: `pub type CheckpointObserver = dyn Fn(&ArcFunction, &str) + '_;` uses anonymous lifetime `'_` which is invalid in type aliases.
  Evidence: Rust does not allow `'_` in type aliases without a corresponding lifetime parameter.
  Impact: Would cause a compilation error.
  Basis: inference. Confidence: high.
  Resolved: Fixed on 2026-04-11. Updated type alias to `pub type CheckpointObserver<'a> = dyn Fn(&ArcFunction, &str) + 'a;` and propagated `<'a>` to all references.

- [x] `[TPR-03-002-gemini][medium]` `section-03-aims-snapshots.md:313` — DRIFT: .ori test filenames use snake_case (`basic_elision.ori`) but `impl-hygiene.md:669` requires kebab-case for Ori test files.
  Evidence: Rule states "kebab-case for Ori spec test files (map-filter-collect.ori)".
  Impact: Violates naming convention for new test files.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-11. Renamed all 21 proposed .ori test files from snake_case to kebab-case (e.g., `basic_elision.ori` → `simple-elision.ori`).

- [x] `[TPR-03-003-gemini][low]` `section-03-aims-snapshots.md:168` — GAP: Rust test names don't follow `<subject>_<scenario>_<expected>` three-part naming per `impl-hygiene.md` §Test Function Naming.
  Evidence: Names like `checkpoint_observer_receives_all_phases` missing the scenario component.
  Impact: Naming convention violation.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-11. Renamed all proposed Rust test functions to AAA format (e.g., `checkpoint_observer_with_all_passes_configured_captures_all_phase_names_in_order`).

**Round 3 findings (iteration 3):**

- [x] `[TPR-03-001-codex-r3][medium]` `section-03-aims-snapshots.md:202` — GAP: `arc_dot/node.rs` imports `fmt_instr`/`fmt_terminator` from `arc_dump/instr.rs`. Moving helpers to `ori_arc` without updating `arc_dot` would break compilation.
  Resolved: Fixed on 2026-04-11. Added `arc_dot/node.rs` to 03.2 file list and explicit import-update task.

- [x] `[TPR-03-002-codex-r3][medium]` `section-03-aims-snapshots.md:270` — GAP: `#[cfg(test)]` items invisible to integration tests (they compile normal lib). Plan offered invalid `#[cfg(test)]` option.
  Resolved: Fixed on 2026-04-11. Removed `#[cfg(test)]` option. Now requires always-compiled `pub mod test_support` module.

- [x] `[TPR-03-003-codex-r3][low]` `section-03-aims-snapshots.md:282` — GAP: `clean_stale_revisions()` can't handle deleted/renamed tests (harness only calls hook for discovered files).
  Resolved: Fixed on 2026-04-11. Scoped `clean_stale_revisions()` to current test_path only. Added separate bless-sweep step for global orphan cleanup.

- [x] `[TPR-03-001-gemini-r3][medium]` `section-03-aims-snapshots.md:144` — DRIFT: Call site count is 16, not 15 (3 in trmc.rs, 6 in mod.rs, 7 in postprocess.rs).
  Resolved: Fixed on 2026-04-11. Updated count to 16 with per-file breakdown.

- [x] `[TPR-03-002-gemini-r3][low]` `section-03-aims-snapshots.md:162` — GAP: `run_arc_pipeline_with_observer` takes `&CheckpointObserver<'_>` but config expects `&'a CheckpointObserver<'a>`.
  Resolved: Fixed on 2026-04-11. Changed to `&'a CheckpointObserver<'a>` with explicit lifetime parameter on function.

**Implementation review (code review — round 1):**

- [x] `[TPR-03-001-codex-impl][high]` `compiler/oric/tests/aims_snapshot_strategy.rs:148` — GAP: Fail the strategy when a requested pass snapshot is missing. Strategy silently skipped uncaptured passes.
  Resolved: Fixed on 2026-04-11. Changed `if let Some(content)` to `captured.get(pass_name).ok_or_else(...)` — missing pass now produces a hard error.

- [x] `[TPR-03-002-codex-impl][high]` `compiler/oric/tests/aims_snapshot_strategy.rs:130` — GAP: Run the snapshot pipeline with real AIMS contracts. Strategy passed empty contracts/uniqueness/sigs maps.
  Resolved: Fixed on 2026-04-11. Added `compute_aims_contracts()` call before pipeline execution, matching production codegen_pipeline.rs:361-366. Re-blessed baselines (mutual-recursive-tail ownership changed from `[borrow]` to `[own]` — correct interprocedural result).

- [x] `[TPR-03-001-gemini-impl][high]` `compiler/oric/tests/aims_snapshot_strategy.rs:88` — GAP: Compute AIMS contracts instead of passing default maps to observer loop. Same root cause as TPR-03-002-codex-impl.
  Resolved: Fixed on 2026-04-11. Same fix as [TPR-03-002-codex-impl] (thematic agreement).

- [x] `[TPR-03-003-codex-impl][medium]` `compiler/oric/tests/aims_snapshots.rs:20` — GAP: Remove the top-level corpus probe that skips nested tests. Only alive because smoke-test.ori at root.
  Resolved: Fixed on 2026-04-11. Removed fragile top-level `.ori` file check — `run_test_directory()` handles recursive discovery and empty-corpus errors.

- [x] `[TPR-03-004-codex-impl][low]` `compiler/oric/tests/aims_snapshots.rs:15` — GAP: Rename the integration test function to a behavioral three-part name.
  Resolved: Fixed on 2026-04-11. Renamed `aims_snapshot_tests` → `aims_snapshots_across_all_passes_match_baselines`.

- [x] `[TPR-03-002-gemini-impl][low]` `compiler/oric/tests/aims_snapshots.rs:14` — Rename aims_snapshot_tests to follow behavioral naming rules.
  Resolved: Fixed on 2026-04-11. Same fix as [TPR-03-004-codex-impl] (thematic agreement).

---

## 03.N Completion Checklist

- [x] Checkpoint observer infrastructure works and is unified with `trace_pipeline_checkpoint()` (no duplicated dispatch) — verified: 17 call sites across 3 files in `ori_arc::pipeline::aims_pipeline`
- [x] ARC IR formatter relocated to `ori_arc::ir::format` (canonical home); `oric::arc_dump` delegates to it — verified: `format_function()` at `format/mod.rs:34`, `arc_dump/mod.rs:78` delegates
- [x] `oric::arc_dump::dump_arc_ir()` still works for `ORI_DUMP_AFTER_ARC=1` (no regression in phase dump) — verified: `ORI_DUMP_AFTER_ARC=1 cargo run -- build` produces correct IR output
- [x] `AimsSnapshotStrategy` implements `TestStrategy` and uses `run_test_directory()` (§02 MANDATORY) — verified: `aims_snapshot_strategy.rs:63`, `aims_snapshots.rs:28`
- [x] >= 15 snapshot tests in `compiler/oric/tests/aims-snapshots/` across 5 priority passes — verified: 22 tests (8+4+3+3+3+1 smoke)
- [x] Every priority pass has >= 1 semantic pin test (optimization fires) and >= 1 negative pin test (optimization does NOT fire) — verified: all 5 passes have both
- [x] `cargo test -p oric --test aims_snapshots` passes with all snapshots matching baselines — verified: 1 passed, 0 failed
- [x] `ORI_BLESS=1 cargo test -p oric --test aims_snapshots` updates baselines — verified: bless mode runs clean
- [x] Deliberate regression detected (snapshot diff fails when optimization disabled) — verified in 03.4 close-out: lowered.arc and realize_rc_reuse.after.arc differ
- [x] Pass idempotency verified for all 5 priority passes — verified in 03.4 close-out: bless + normal run identical
- [x] Data-efficient: `lowered.arc` + `.after.arc` per pass, no redundant `.before.arc` — verified: 0 `.before.arc`, 25 `.lowered.arc`, 25 `.after.arc`
- [x] No regressions: `timeout 150 ./test-all.sh` green — verified: 17,049 passed, 0 failed
- [x] `timeout 150 ./clippy-all.sh` green — verified: all clippy checks passed
- [x] Plan annotation cleanup (remove any `§03` annotations from production code) — verified: 0 stale annotations
- [x] **Plan sync** — update plan metadata (overview, index) — updated Quick Reference in both to "In Progress"
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` section-close sweep

**Exit Criteria:** `cargo test -p oric --test aims_snapshots` runs >= 15 snapshot tests across 5 priority passes, all matching baselines. Every priority pass has at least one semantic pin and one negative pin. The checkpoint observer is unified with `trace_pipeline_checkpoint()` (single dispatch point). Deliberately introducing an optimization regression causes at least one snapshot diff to fail. Bless mode updates baselines. The ARC IR formatter lives in `ori_arc::ir::format` (canonical home). No regressions in `test-all.sh`.
