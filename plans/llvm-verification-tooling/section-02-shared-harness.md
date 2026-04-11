---
section: "02"
title: "Shared Test Harness Infrastructure"
status: not-started
reviewed: false
goal: "Build a single workspace library (ori_test_harness) that provides directive parsing, artifact naming, --bless mode, revision expansion, and diff generation — consumed by both AIMS snapshot tests (§03) and FileCheck IR tests (§07)"
success_criteria:
  - "ori_test_harness crate exists in workspace with directive parser, bless mode, revision expansion"
  - "Directive parser handles // @test-arc-pass:, // CHECK:, // @revisions: syntax"
  - "--bless mode updates baselines for both .arc and .ll artifact types"
  - "Revision system runs tests against multiple flag sets (debug, release, no-repr-opt)"
  - "Diff generation produces readable .diff artifacts on failure"
  - "tests/codegen/ and tests/arc-opt/ directories created with canonical test-directory policy"
inspired_by:
  - "Rust compiletest (src/tools/compiletest/src/) — directive parsing, revision system, bless mode"
  - "Rust miropt-test-tools (src/tools/miropt-test-tools/src/lib.rs) — .before/.after/.diff artifact naming"
  - "Zig addCheckFile (test/src/LlvmIr.zig:45-73) — .matches/.exact assertion modes"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Create ori_test_harness Crate"
    status: not-started
  - id: "02.2"
    title: "Directive Parser"
    status: not-started
  - id: "02.3"
    title: "Artifact Naming and Storage"
    status: not-started
  - id: "02.4"
    title: "Bless Mode and Diff Generation"
    status: not-started
  - id: "02.5"
    title: "Revision System"
    status: not-started
  - id: "02.6"
    title: "Create Canonical Test Directories"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Shared Test Harness Infrastructure

**Status:** Not Started
**Goal:** Build a single workspace library (`ori_test_harness`) that provides directive parsing, artifact naming, `--bless` mode, revision expansion, and diff generation — consumed by both AIMS snapshot tests (Section 03) and FileCheck IR tests (Section 07). This prevents the SSOT failure mode where two overlapping harnesses with duplicated logic drift apart (`impl-hygiene.md` §Algorithmic DRY).

**Success Criteria:**

- [ ] `ori_test_harness` crate exists in workspace — satisfies mission criterion: "Shared harness, not fragmented tools"
- [ ] Directive parser handles `// @test-arc-pass:`, `// CHECK:`, `// @revisions:` — satisfies §03 and §07 needs
- [ ] `--bless` mode updates baselines for `.arc` and `.ll` artifacts — satisfies §03 and §07 needs
- [ ] Revision system runs tests against multiple flag sets — satisfies mission criterion: "FileCheck revision support"
- [ ] `tests/codegen/` and `tests/arc-opt/` directories created with policy docs — satisfies mission criterion: "New test directories canonical"

**Context:** The research identified a critical SSOT risk: AIMS pass-level snapshots (Tier 0.1) and FileCheck IR assertions (Tier 2.1) both need directive parsing, revision expansion, artifact naming, bless mode, and failure diffing. If built as separate harnesses, their duplicated logic will drift — the exact failure mode Rust avoided by having one `compiletest` tool for codegen, MIR-opt, and UI tests. The research proposes a shared "ori-check" runner binary, but the Codex+Gemini consensus (Round 1) recommends a workspace library + `oric` subcommand instead, to maintain SSOT for compiler behavior.

**Reference implementations:**
- **Rust** `src/tools/compiletest/src/directives.rs`: `//@` prefix parsing with `[revision]` gating, `name: value` syntax, forbidden revision names (line 610-618). Revision-specific CHECK prefixes.
- **Rust** `src/tools/miropt-test-tools/src/lib.rs`: `.before`/`.after`/`.diff` artifact naming (lines 48-137). `EMIT_MIR` directive syntax with pass name extraction.
- **Rust** `src/tools/compiletest/src/runtest.rs` (lines 2704-2821): Bless mode — delete old files, write actual output, clean up non-revision files.
- **Zig** `test/src/LlvmIr.zig` (lines 45-73): `.matches` mode (order-independent substring search) vs `.exact` mode (precise validation).

**Depends on:** Nothing — independent foundation section.

---

## 02.1 Create ori_test_harness Crate

**File(s):** `compiler/ori_test_harness/Cargo.toml`, `compiler/ori_test_harness/src/lib.rs`, `Cargo.toml` (workspace)

Create a new workspace crate that holds the shared test infrastructure. This crate is a dev-dependency of `ori_arc` (for AIMS snapshots) and `ori_llvm` (for FileCheck tests) — it is NOT a production dependency.

- [ ] Create `compiler/ori_test_harness/Cargo.toml`:
  ```toml
  [package]
  name = "ori_test_harness"
  version.workspace = true
  edition.workspace = true
  
  [dependencies]
  # Minimal — this is a test utility library
  similar = "2.5"  # For diff generation (used by insta, well-maintained)
  ```
  Do NOT depend on `ori_llvm`, `ori_arc`, or any compiler crate — the harness is generic infrastructure. Compiler crates depend on it, not the other way.

- [ ] Add to workspace `Cargo.toml` members list.

- [ ] Confirm user permission for workspace Cargo.toml edits per `.claude/rules/cargo.md`

  > **Note:** Editing `Cargo.toml` (workspace members, dependencies) requires explicit user permission per `.claude/rules/cargo.md`.

- [ ] Create `compiler/ori_test_harness/src/lib.rs` as an index with submodules:
  ```rust
  //! Shared test harness for AIMS snapshot tests and FileCheck IR assertions.
  //!
  //! Provides directive parsing, artifact naming, bless mode, revision expansion,
  //! and diff generation. Consumed by `ori_arc` (AIMS snapshots) and `ori_llvm`
  //! (FileCheck IR tests).
  
  pub mod directive;    // Directive parsing (// @..., // CHECK:)
  pub mod artifact;     // Artifact naming and storage
  pub mod bless;        // Bless mode (update baselines)
  pub mod diff;         // Diff generation
  pub mod revision;     // Revision expansion
  ```

- [ ] **Subsection close-out (02.1)** — MANDATORY before starting 02.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 02.2 Directive Parser

**File(s):** `compiler/ori_test_harness/src/directive.rs`

Parse test directives from `.ori` and `.rs` test files. Follow Rust's compiletest pattern (`//@` prefix) adapted for Ori's needs.

- [ ] Define directive types:
  ```rust
  /// A parsed directive from a test file.
  pub enum Directive {
      /// `// @test-arc-pass: realize_rc_reuse` — capture AIMS pass snapshot
      TestArcPass { pass_name: String },
      /// `// @revisions: debug release no-repr-opt` — define test revisions
      Revisions { names: Vec<String> },
      /// `// @compile-flags: --release` — extra flags for this revision
      CompileFlags { flags: Vec<String> },
      /// `// CHECK: <pattern>` — FileCheck-style assertion (substring match)
      Check { pattern: String },
      /// `// CHECK-LABEL: <pattern>` — FileCheck label assertion
      CheckLabel { pattern: String },
      /// `// CHECK-NOT: <pattern>` — FileCheck negative assertion
      CheckNot { pattern: String },
      /// `// CHECK-NEXT: <pattern>` — FileCheck next-line assertion
      CheckNext { pattern: String },
  }
  
  /// A directive line with source location and optional revision gate.
  pub struct DirectiveLine {
      pub line_number: usize,
      pub revision: Option<String>,  // From [revision] prefix
      pub directive: Directive,
  }
  ```

- [ ] Implement `parse_directives(source: &str) -> Vec<DirectiveLine>`:
  - Scan lines for `// @` prefix (Ori comment + directive marker)
  - Handle `// @[revision_name] directive-name: value` syntax
  - Parse `// CHECK:`, `// CHECK-LABEL:`, etc. as FileCheck directives
  - Forbidden revision names: `true`, `false`, `CHECK`, `COM`, `NEXT`, `SAME`, `EMPTY`, `NOT`, `COUNT`, `DAG`, `LABEL` (from Rust compiletest)
  - Return directives with line numbers for error reporting

- [ ] Add tests in `compiler/ori_test_harness/src/directive/tests.rs`:
  - `test_parse_test_arc_pass_directive`
  - `test_parse_revisions_directive`
  - `test_parse_check_directives`
  - `test_revision_gating_filters_correctly`
  - `test_forbidden_revision_names_rejected`
  - `test_mixed_directives_parsed_in_order`

- [ ] **Subsection close-out (02.2)** — MANDATORY before starting 02.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 02.3 Artifact Naming and Storage

**File(s):** `compiler/ori_test_harness/src/artifact.rs`

Define how test artifacts (`.before.arc`, `.after.arc`, `.diff`, `.ll`) are named, stored, and located. Follow Rust's MIR-opt pattern: expected baselines live alongside test source files.

- [ ] Define artifact types:
  ```rust
  pub enum ArtifactKind {
      /// AIMS pass snapshot: .before.arc / .after.arc / .diff
      ArcSnapshot { pass_name: String, function_name: String },
      /// LLVM IR dump: .ll
      LlvmIr { function_name: Option<String> },
  }
  
  pub struct ArtifactPaths {
      /// Expected baseline file (in source tree, alongside test file)
      pub expected: PathBuf,
      /// Actual output file (in build/temp directory)
      pub actual: PathBuf,
  }
  ```

- [ ] Implement artifact naming convention:
  - **AIMS snapshots**: `{test_name}.{function_name}.{pass_name}.before.arc`, `.after.arc`, `.diff`
  - **LLVM IR**: `{test_name}.{revision}.ll` (or `{test_name}.ll` without revisions)
  - **Revision suffix**: inserted before extension: `test.debug.realize_rc_reuse.diff`
  - Expected files: same directory as test source
  - Actual files: temp directory under `target/` or `$TMPDIR`

- [ ] Add tests:
  - `test_arc_snapshot_artifact_naming`
  - `test_llvm_ir_artifact_naming`
  - `test_revision_suffix_inserted_correctly`

- [ ] **Subsection close-out (02.3)** — MANDATORY before starting 02.4:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 02.4 Bless Mode and Diff Generation

**File(s):** `compiler/ori_test_harness/src/bless.rs`, `compiler/ori_test_harness/src/diff.rs`

Implement `--bless` mode that updates expected baselines, and diff generation that produces readable output on failure.

- [ ] Implement bless logic (following Rust compiletest pattern):
  ```rust
  pub fn compare_or_bless(
      expected_path: &Path,
      actual: &str,
      bless: bool,
  ) -> Result<CompareOutcome, io::Error> {
      if bless {
          if actual.is_empty() && expected_path.exists() {
              fs::remove_file(expected_path)?;
              return Ok(CompareOutcome::BlessedEmpty);
          }
          if !actual.is_empty() {
              fs::write(expected_path, actual)?;
              return Ok(CompareOutcome::Blessed);
          }
          return Ok(CompareOutcome::BlessedEmpty);
      }
      // Normal mode: compare
      let expected = fs::read_to_string(expected_path)
          .unwrap_or_default();
      if expected == actual {
          Ok(CompareOutcome::Match)
      } else {
          Ok(CompareOutcome::Mismatch {
              diff: generate_diff(&expected, actual),
          })
      }
  }
  ```

- [ ] Implement diff generation using `similar` crate:
  ```rust
  pub fn generate_diff(expected: &str, actual: &str) -> String {
      // Unified diff format with context lines
      // Show line numbers, +/- prefixes
  }
  ```

- [ ] Bless mode must clean up old revision-specific files when revisions change (Rust compiletest deletes non-revision files when introducing revisions).

- [ ] Add tests:
  - `test_bless_writes_new_baseline`
  - `test_bless_deletes_empty_baseline`
  - `test_compare_detects_mismatch`
  - `test_diff_output_readable`
  - `test_bless_cleans_old_revision_files`

- [ ] **TPR checkpoint** — `/tpr-review` covering 02.1–02.4 implementation work

- [ ] **Subsection close-out (02.4)** — MANDATORY before starting 02.5:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 02.5 Revision System

**File(s):** `compiler/ori_test_harness/src/revision.rs`

Implement the revision expansion system that runs tests against multiple configurations (debug, release, no-repr-opt).

- [ ] Define revision configuration:
  ```rust
  pub struct RevisionConfig {
      pub name: String,
      pub compile_flags: Vec<String>,
      pub env_vars: Vec<(String, String)>,
  }
  
  pub fn expand_revisions(
      directives: &[DirectiveLine],
  ) -> Vec<RevisionConfig> {
      // If no // @revisions: directive, return single default config
      // If revisions defined, return one config per revision with
      // revision-specific compile-flags applied
  }
  ```

- [ ] Support standard Ori revisions:
  - `debug`: default (no extra flags)
  - `release`: `--release` flag
  - `no-repr-opt`: `ORI_NO_REPR_OPT=1` env var
  - Custom revisions via `// @[name] compile-flags:` directives

- [ ] Revision-specific CHECK prefixes: when a revision named `DEBUG` is active, `// DEBUG-CHECK:` directives apply (in addition to unprefixed `// CHECK:` directives). Follow Rust compiletest pattern where revision name becomes a FileCheck prefix.

- [ ] Add tests:
  - `test_single_revision_when_no_directive`
  - `test_multiple_revisions_expanded`
  - `test_revision_specific_flags_applied`
  - `test_revision_specific_check_prefix`

- [ ] **Subsection close-out (02.5)** — MANDATORY before starting 02.6:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 02.6 Create Canonical Test Directories

**File(s):** `tests/codegen/`, `tests/arc-opt/`, `.claude/rules/tests.md`

Create the new test directories and update test-directory policy documentation so they are canonical, not ad-hoc.

- [ ] Create `tests/codegen/` with subdirectories:
  ```
  tests/codegen/
    rc/              # RC emission patterns
    cow/             # COW patterns
    closures/        # Closure codegen
    abi/             # ABI patterns
    iterator/        # Iterator codegen
    README.md        # Directory purpose and test conventions
  ```

- [ ] Create `tests/arc-opt/` with subdirectories:
  ```
  tests/arc-opt/
    realize_rc_reuse/     # Step 5 snapshots
    merge_blocks/         # Step 9 snapshots
    realize_annotations/  # Step 10 snapshots
    normalize_function/   # Step 3a snapshots
    README.md             # Directory purpose and conventions
  ```

- [ ] Update `.claude/rules/tests.md` to include the new directories in the test-directory taxonomy:
  - `tests/codegen/` — LLVM IR pattern tests (FileCheck-style, `ori_test_harness` directives)
  - `tests/arc-opt/` — AIMS pass snapshot tests (`.before.arc`/`.after.arc`/`.diff`)

- [ ] Add a seed test in each directory to validate the harness works end-to-end:
  - `tests/codegen/rc/basic_rc_inc_dec.ori` — basic RC pattern test
  - `tests/arc-opt/realize_rc_reuse/basic_elision.ori` — basic AIMS snapshot

- [ ] **Subsection close-out (02.6)** — MANDATORY before starting 02.R:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] `ori_test_harness` crate exists in workspace, compiles, passes its own tests
- [ ] Directive parser handles all directive types (arc-pass, revisions, compile-flags, CHECK variants)
- [ ] Artifact naming produces correct paths for AIMS and LLVM artifacts
- [ ] Bless mode writes/deletes baselines correctly
- [ ] Revision expansion produces correct configurations
- [ ] `tests/codegen/` and `tests/arc-opt/` directories exist with README and seed tests
- [ ] `.claude/rules/tests.md` updated with new test-directory taxonomy
- [ ] No existing tests regressed: `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 02` returns 0 annotations
- [ ] All intermediate TPR checkpoint findings resolved
- [ ] **Plan sync** — update plan metadata:
  - [ ] This section's frontmatter `status` → `complete`
  - [ ] `00-overview.md` Quick Reference updated
  - [ ] `index.md` section status updated
- [ ] `/tpr-review` passed (final, full-section)
- [ ] `/impl-hygiene-review` passed — AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` **section-close sweep** — verify per-subsection retrospectives ran, add cross-cutting items.

**Exit Criteria:** `ori_test_harness` crate compiles and passes all internal tests. Directive parsing, artifact naming, bless mode, and revision expansion work for both AIMS and LLVM artifact types. Seed tests in `tests/codegen/` and `tests/arc-opt/` demonstrate the full pipeline. Test-directory policy updated. Section 03 and Section 07 can consume the harness without building their own.
