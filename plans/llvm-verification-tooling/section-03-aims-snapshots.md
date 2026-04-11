---
section: "03"
title: "AIMS Pass-Level Snapshot Tests"
status: not-started
reviewed: false
goal: "Capture .before.arc/.after.arc/.diff artifacts at configurable AIMS pipeline boundaries, enabling regression detection for RC elision, COW annotation, block merge, and reuse — invisible to behavioral tests when the LLVM optimizer papers over codegen quality issues"
success_criteria:
  - "Per-pass dump hooks exist for priority passes: realize_rc_reuse, merge_blocks, realize_annotations, normalize_function"
  - "compiler/ori_arc/tests/arc-opt/ contains ≥15 snapshot tests across the 4 priority passes"
  - "cargo test -p ori_arc --test aims_snapshots passes with all snapshots matching baselines"
  - "--bless mode updates snapshot baselines via shared harness (§02)"
  - "A deliberately introduced regression (e.g., disabling RC elision) causes snapshot diffs to fail"
inspired_by:
  - "Rust MIR-opt (src/tools/miropt-test-tools/src/lib.rs) — EMIT_MIR, .before/.after/.diff, pass names"
  - "Rust MIR-opt tests (tests/mir-opt/) — directive syntax, artifact naming, bless workflow"
depends_on: ["02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Per-Pass Dump Hooks in AIMS Pipeline"
    status: not-started
  - id: "03.2"
    title: "Snapshot Test Runner Integration"
    status: not-started
  - id: "03.3"
    title: "Initial Snapshot Corpus"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: AIMS Pass-Level Snapshot Tests

**Status:** Not Started
**Goal:** Capture `.before.arc`/`.after.arc`/`.diff` artifacts at configurable AIMS pipeline boundaries, enabling regression detection for RC elision, COW annotation, block merge, and reuse optimization changes. These changes are invisible to behavioral tests when the LLVM optimizer papers over codegen quality issues — the program produces correct output from terrible ARC IR. Snapshot tests catch this: if the IR shape changes, the snapshot diff catches it regardless of behavioral output.

**Success Criteria:**

- [ ] Per-pass dump hooks produce correct `.before.arc`/`.after.arc` artifacts — satisfies mission criterion: "AIMS pass regressions caught by snapshot diffs"
- [ ] ≥15 snapshot tests across priority passes — satisfies mission criterion: "snapshot corpus"
- [ ] Deliberate regression detected by snapshot failure — satisfies mission criterion: "regression detection"

**Context:** The AIMS pipeline runs 12 steps (see `.claude/rules/arc.md` §Pipeline). Currently, only the **final** ARC IR is observable via `ORI_DUMP_AFTER_ARC=1`. If a pass regresses (e.g., RC elision stops firing for a case it previously caught), the regression is invisible until it manifests as a runtime leak or wrong behavior. Per-pass snapshots, inspired by Rust's MIR-opt infrastructure, make each step's output observable and diffable.

**State Map Caveat (CRITICAL)**: `realize_annotations` (step 10) runs AFTER `merge_blocks` (step 9). Per `.claude/rules/arc.md`: "Position-keyed state maps (`entry_states`, `exit_states`, `instr_states`) are invalid after `merge_blocks()`." The snapshot harness for step 10 MUST NOT attempt to dump position-keyed state map fields — only ArcVarId-keyed lookups and the IR itself are safe post-merge.

**Reference implementations:**
- **Rust** `src/tools/miropt-test-tools/src/lib.rs`: `EMIT_MIR` directive triggers `.before.mir`/`.after.mir` capture around specific passes. Artifact naming: `{crate}.{function}.{pass}.{before|after}.mir`. Bless mode: `--bless` rewrites expected files.

**Depends on:** Section 02 (shared harness provides directive parsing, artifact naming, bless mode, diff generation).

---

## 03.1 Per-Pass Dump Hooks in AIMS Pipeline

**File(s):** `compiler/ori_arc/src/pipeline/aims_pipeline/mod.rs`, `compiler/ori_arc/src/pipeline/aims_pipeline/postprocess.rs`

Add configurable dump hooks at pipeline step boundaries. The hooks capture ARC IR before and after each pass, gated by configuration (not always-on — only when snapshot tests are running).

- [ ] Add a `SnapshotConfig` to `AimsPipelineConfig`:
  ```rust
  pub struct SnapshotConfig {
      /// Which passes to capture snapshots for
      pub capture_passes: Vec<String>,
      /// Output directory for snapshot artifacts
      pub output_dir: PathBuf,
  }
  ```

- [ ] Add snapshot capture before and after each priority pass in `run_aims_pipeline()`:
  - **Step 3a** (`normalize_function`): capture before/after
  - **Step 5** (`realize_rc_reuse`): capture before/after (highest value — RC elision)
  - **Step 8** (`detect/rewrite_tail_calls`): capture before/after
  - **Step 9** (`merge_blocks`): capture before/after
  - **Step 10** (`realize_annotations`): capture IR only (no state map dump — stale after merge)

- [ ] Implement `fn dump_arc_function(func: &ArcFunction, path: &Path)` that serializes ARC IR in a human-readable, diffable format. Use the existing `ORI_DUMP_AFTER_ARC` serialization format as the basis — it's already human-readable.

- [ ] Gate snapshot capture behind `SnapshotConfig` — zero overhead when not capturing.

- [ ] Add tests:
  - `test_snapshot_captures_before_and_after_realize_rc_reuse`
  - `test_snapshot_not_captured_when_config_empty`
  - `test_snapshot_ir_format_is_diffable`

- [ ] **Subsection close-out (03.1)** — MANDATORY before starting 03.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 03.2 Snapshot Test Runner Integration

**File(s):** `compiler/ori_arc/tests/aims_snapshots.rs` (new integration test), `compiler/ori_arc/tests/arc-opt/`

Wire the snapshot capture into cargo tests using the shared harness (§02). Tests use `// @test-arc-pass: realize_rc_reuse` directives to specify which pass to snapshot.

- [ ] Create `compiler/ori_arc/tests/aims_snapshots.rs` using `run_test_directory()` from the shared harness:
  ```rust
  //! AIMS pass-level snapshot tests.
  //!
  //! Uses the shared harness (ori_test_harness) for directive parsing,
  //! revision expansion, bless mode, and test orchestration.
  //! This crate provides only the AimsSnapshotStrategy — all orchestration
  //! logic lives in the shared harness.

  use ori_test_harness::runner::{run_test_directory, TestStrategy};
  use std::path::Path;

  #[test]
  fn aims_snapshot_tests() {
      let test_dir = Path::new("compiler/ori_arc/tests/arc-opt");
      let strategy = AimsSnapshotStrategy::new();
      let summary = run_test_directory(test_dir, &strategy);
      assert!(summary.is_success(), "AIMS snapshot failures:\n{}", summary.failures.join("\n"));
  }
  ```

- [ ] Implement `AimsSnapshotStrategy` that implements `TestStrategy`:
  - `execute()`: translates revision config into compiler flags, compiles through the AIMS pipeline with snapshot capture, interprets `Custom { key: "test-arc-pass", .. }` directives to select pass
  - `verify()`: compares captured artifacts against baselines using `bless::compare_or_bless()`

- [ ] Add `ori_test_harness` as dev-dependency of `ori_arc`.

- [ ] **Subsection close-out (03.2)** — MANDATORY before starting 03.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 03.3 Initial Snapshot Corpus

**File(s):** `compiler/ori_arc/tests/arc-opt/realize_rc_reuse/`, `compiler/ori_arc/tests/arc-opt/merge_blocks/`, `compiler/ori_arc/tests/arc-opt/realize_annotations/`, `compiler/ori_arc/tests/arc-opt/normalize_function/`

Create the initial corpus of snapshot tests covering the priority passes. Each test should cover a specific optimization behavior.

- [ ] **realize_rc_reuse** (Step 5, highest value):
  - `basic_elision.ori` — simple RC inc/dec pair elimination
  - `unique_owner_elision.ori` — unique owner skip dec
  - `borrowed_param_no_elision.ori` — borrowed params keep their RC ops
  - `closure_capture_rc.ori` — closure environment RC handling
  - `iterator_rc_lifecycle.ori` — iterator create/next/drop RC pattern

- [ ] **merge_blocks** (Step 9):
  - `linear_chain_merge.ori` — sequential blocks merged
  - `branch_no_merge.ori` — branches preserved
  - `empty_block_removal.ori` — empty cleanup blocks removed

- [ ] **realize_annotations** (Step 10):
  - `cow_annotation.ori` — COW uniqueness check placement
  - `drop_hint_placement.ori` — drop hint annotation after merge

- [ ] **realize_rc_reuse** (additional coverage):
  - `map_iteration_rc.ori` — RC lifecycle during map iteration
  - `nested_struct_rc.ori` — RC operations for nested struct access chains

- [ ] **merge_blocks** (additional coverage):
  - `loop_exit_merge.ori` — loop exit blocks merged correctly

- [ ] **normalize_function** (Step 3a):
  - `trmc_detection.ori` — TRMC context region detected
  - `no_trmc_passthrough.ori` — non-TRMC function passes through unchanged

- [ ] Bless all initial baselines: `ORI_BLESS=1 cargo test -p ori_arc --test aims_snapshots`

- [ ] Verify regression detection: temporarily disable one optimization, run tests, confirm snapshot failure.

- [ ] **Subsection close-out (03.3)** — MANDATORY before starting 03.R:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] Per-pass dump hooks work for all 4 priority passes
- [ ] ≥15 snapshot tests in `compiler/ori_arc/tests/arc-opt/` across priority passes
- [ ] `cargo test -p ori_arc --test aims_snapshots` passes
- [ ] `ORI_BLESS=1 cargo test -p ori_arc --test aims_snapshots` updates baselines
- [ ] Deliberate regression detected (snapshot diff fails)
- [ ] State map caveat respected (no position-keyed dumps after merge_blocks)
- [ ] No regressions: `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] Plan annotation cleanup
- [ ] **Plan sync** — update plan metadata
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` section-close sweep

**Exit Criteria:** `cargo test -p ori_arc --test aims_snapshots` runs ≥15 snapshot tests across 4 priority passes, all matching baselines. Deliberately introducing an optimization regression causes at least one snapshot diff to fail. Bless mode updates baselines. No regressions in `test-all.sh`.
