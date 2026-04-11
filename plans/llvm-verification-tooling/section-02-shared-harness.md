---
section: "02"
title: "Shared Test Harness Infrastructure"
status: not-started
reviewed: false
goal: "Build a single workspace library (ori_test_harness) that provides directive parsing, artifact naming, bless mode, revision expansion, diff generation, and a canonical test runner loop — consumed by both AIMS snapshot tests (§03) and FileCheck IR tests (§07). Consuming crates provide only a TestStrategy callback; the harness owns the traverse→parse→expand→invoke→diff algorithm."
success_criteria:
  - "ori_test_harness crate exists in workspace with directive parser, bless mode, revision expansion, test runner loop"
  - "Directive parser handles // @<key>: <value> (generic Custom), // CHECK:, // @revisions: syntax via line-anchored regex (not the Ori lexer)"
  - "Bless mode controlled exclusively via ORI_BLESS=1 env var (only value '1' accepted); is_bless_enabled() is the single query point"
  - "Revision system extracts revision names and per-revision compile-flags directives; flag-to-env-var translation belongs to consumer TestStrategy, not the harness"
  - "run_test_directory(path, strategy) provides the canonical orchestration loop — consumers never duplicate traverse→parse→expand→invoke→diff"
  - "Diff generation produces readable .diff artifacts on failure using the similar crate"
  - "Seed tests use a mock TestStrategy proving harness orchestration without real compiler integration"
inspired_by:
  - "Rust compiletest (src/tools/compiletest/src/) — directive parsing, revision system, bless mode"
  - "Rust miropt-test-tools (src/tools/miropt-test-tools/src/lib.rs) — .before/.after/.diff artifact naming"
  - "Zig addCheckFile (test/src/LlvmIr.zig:45-73) — .matches/.exact assertion modes"
depends_on: []
third_party_review:
  status: resolved
  updated: 2026-04-10
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
    title: "Test Runner Orchestration (TestStrategy Trait)"
    status: not-started
  - id: "02.7"
    title: "Seed Tests with Mock TestStrategy"
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
**Goal:** Build a single workspace library (`ori_test_harness`) that provides directive parsing, artifact naming, `ORI_BLESS=1` mode, revision expansion, diff generation, and a canonical test runner orchestration loop — consumed by both AIMS snapshot tests (Section 03) and FileCheck IR tests (Section 07). Consuming crates provide only a `TestStrategy` callback; the harness owns the traverse-parse-expand-invoke-diff algorithm. This prevents the SSOT failure mode where two overlapping harnesses with duplicated logic drift apart (`impl-hygiene.md` §Algorithmic DRY).

**Success Criteria:**

- [ ] `ori_test_harness` crate exists in workspace — satisfies mission criterion: "Shared harness, not fragmented tools"
- [ ] Directive parser handles `// @<key>: <value>` (generic custom), `// CHECK:`, `// @revisions:` via line-anchored regex — satisfies §03 and §07 needs
- [ ] `ORI_BLESS=1` env var is the single bless control plane — satisfies §03 and §07 needs
- [ ] Revision system extracts names and per-revision `compile-flags` directives; flag translation is delegated to consumer `TestStrategy` — satisfies mission criterion: "FileCheck revision support"
- [ ] `run_test_directory(path, strategy)` is the canonical orchestration loop; §03 and §07 call it with their `TestStrategy` impl — prevents algorithmic duplication
- [ ] Seed tests use a `MockTestStrategy` to validate orchestration without real compiler integration

**Context:** The research identified a critical SSOT risk: AIMS pass-level snapshots (Tier 0.1) and FileCheck IR assertions (Tier 2.1) both need directive parsing, revision expansion, artifact naming, bless mode, and failure diffing. If built as separate harnesses, their duplicated logic will drift — the exact failure mode Rust avoided by having one `compiletest` tool for codegen, MIR-opt, and UI tests. The research proposes a shared "ori-check" runner binary, but the Codex+Gemini consensus (Round 1) recommends a workspace library + `oric` subcommand instead, to maintain SSOT for compiler behavior.

**Reference implementations:**
- **Rust** `src/tools/compiletest/src/directives.rs`: `//@` prefix parsing with `[revision]` gating, `name: value` syntax, forbidden revision names (line 610-618). Revision-specific CHECK prefixes.
- **Rust** `src/tools/miropt-test-tools/src/lib.rs`: `.before`/`.after`/`.diff` artifact naming (lines 48-137). `EMIT_MIR` directive syntax with pass name extraction.
- **Rust** `src/tools/compiletest/src/runtest.rs` (lines 2704-2821): Bless mode — delete old files, write actual output, clean up non-revision files.
- **Zig** `test/src/LlvmIr.zig` (lines 45-73): `.matches` mode (order-independent substring search) vs `.exact` mode (precise validation).

**Depends on:** Nothing — independent foundation section.

**Cross-section notes:**

- **MANDATORY: §03 and §07 MUST use `run_test_directory()` — no bespoke loops.** The entire point of §02 is that `run_test_directory(path, strategy)` is the SINGLE canonical orchestration loop. §03 and §07 must call it with their respective `TestStrategy` implementations (`AimsSnapshotStrategy`, `FileCheckStrategy`). They must NOT build their own file-walking, directive-parsing, revision-expanding, or bless-checking loops. Both consumer sections' current plan sketches include inline orchestration logic that must be replaced with `run_test_directory()` calls when those sections are reviewed. Similarly, bless mode must be queried exclusively via `bless::is_bless_enabled()` — no direct `std::env::var("ORI_BLESS")` in consumer code.
- **§07 `.ll` baselines vs §12 golden IR baselines:** §07's bless-to-`.ll` mechanism (per-test IR snapshots blessed via `ORI_BLESS=1`) is distinct from §12's `scripts/ir-baseline.sh` (whole-program golden IR for regression dashboarding). They serve different purposes: §07 pins specific codegen patterns, §12 detects any IR shape change. Both use `ORI_BLESS=1` as the control plane (via `ori_test_harness::bless::is_bless_enabled()`), but §12's script reads it independently. This is not an SSOT collision — it is complementary coverage at different granularities. Document this distinction in §07.1 and §12.1 when implementing.
- **`test-all.sh` CI wiring:** The current `test-all.sh` runs `cargo test -p ori_llvm --lib`, `--doc`, and `--test aot` but does NOT run custom integration test targets like `--test codegen_checks` (§07) or `--test aims_snapshots` (§03). Adding these test targets to `test-all.sh` is owned by §11 (CI Integration). §02 must NOT modify `test-all.sh`. Instead, each consumer section (§03, §07) documents the `cargo test` invocation needed, and §11 wires them into the pipeline.
- **Existing `aot.rs` helpers — REUSE REQUIRES EXTRACTION:** `compiler/ori_llvm/tests/aot/util/aot.rs` already contains `compile_and_capture_ir()`, `extract_function_ir()`, `compile_to_llvm_ir()`, and `ori_binary()`. However, these helpers live under the `aot` integration test target (`compiler/ori_llvm/tests/aot/main.rs`) — Cargo does not allow one integration test target to import another's modules. **Before §07 can reuse these helpers, they must be promoted to a shared location** — either a `compiler/ori_llvm/tests/test_util/` module visible to all integration test targets via `#[path]`, or a `compiler/ori_llvm/src/test_support.rs` module behind `#[cfg(test)]`. §07's plan must include this extraction as a prerequisite task. §02 does not own this extraction — it is a §07 dependency.
- **`cargo st` collision — RESOLVED: crate-local test directories (option a).** The Ori test runner (`ori test tests/`, invoked by `cargo st`) recursively discovers ALL `.ori` files under `tests/` (see `compiler/oric/src/test/discovery/mod.rs`). Files with FileCheck directives or snapshot-test patterns are NOT valid Ori test programs and would cause failures. **Canonical decision: test directories live inside compiler crates, not under top-level `tests/`.** This follows the existing pattern (`compiler/ori_llvm/tests/aot/` is already inside the compiler crate). Specifically:
  - §03 (AIMS snapshots): `compiler/ori_arc/tests/arc-opt/` (not `tests/arc-opt/`)
  - §07 (FileCheck IR): `compiler/ori_llvm/tests/codegen/` (not `tests/codegen/`)
  This ensures `cargo st` never discovers harness-managed test files. §03 and §07 must use these crate-local paths. All downstream sections (§03, §07, §09, §11) and the overview must be updated to reference the crate-local paths when those sections are reviewed.

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
  regex = "1"       # For line-anchored directive parsing (no Ori lexer dependency)
  walkdir = "2"     # For recursive test-file discovery in run_test_directory()

  [lints]
  workspace = true
  ```
  Do NOT depend on `ori_llvm`, `ori_arc`, `ori_types`, or any compiler crate — the harness is generic infrastructure. Compiler crates depend on it (as dev-dependencies), not the other way. This is critical: the harness sits below all compiler crates in the dependency graph.

- [ ] Add to workspace `Cargo.toml` `members` and `default-members` lists. **Requires explicit user permission per `.claude/rules/cargo.md`.**

- [ ] Create `compiler/ori_test_harness/src/lib.rs` as an index with submodules (per `impl-hygiene.md` — `lib.rs` is an index, no function bodies):
  ```rust
  //! Shared test harness for AIMS snapshot tests and FileCheck IR assertions.
  //!
  //! Provides directive parsing, artifact naming, bless mode, revision expansion,
  //! diff generation, and a canonical test runner loop. Consumed by `ori_arc`
  //! (AIMS snapshots) and `ori_llvm` (FileCheck IR tests) as a dev-dependency.
  //!
  //! **Design principle**: this crate knows nothing about the Ori compiler.
  //! It parses directives from text, names artifacts, diffs strings, and
  //! orchestrates a test loop via the `TestStrategy` trait. Compiler-specific
  //! behavior (compilation, IR capture, flag translation) lives in consumer
  //! crates' `TestStrategy` implementations.

  pub mod artifact;     // Artifact naming and storage
  pub mod bless;        // Bless mode (ORI_BLESS=1 env var)
  pub mod diff;         // Diff generation (similar crate)
  pub mod directive;    // Directive parsing (// @..., // CHECK:)
  pub mod revision;     // Revision expansion
  pub mod runner;       // Test runner orchestration (TestStrategy trait)
  ```

- [ ] Verify `cargo check -p ori_test_harness` compiles with the empty modules.

- [ ] **Subsection close-out (02.1)** — MANDATORY before starting 02.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 02.2 Directive Parser

**File(s):** `compiler/ori_test_harness/src/directive.rs`, `compiler/ori_test_harness/src/directive/tests.rs`

Parse test directives from `.ori` and `.rs` test files. Use **line-anchored regex** (`^//\s*@` or `^//\s*CHECK`), NOT the Ori lexer. The harness must not depend on any compiler crate (no `ori_lexer`, `ori_parse`, etc.). This is a line-based parser operating on plain text — it reads comment syntax, not Ori language syntax.

**Limitation acknowledgment:** Line-based parsing cannot handle multi-line directives or directives inside block comments. This is acceptable — Rust's compiletest has the same limitation, and all reference implementations (Rust, Zig, LLVM FileCheck) use line-based parsing.

- [ ] Define directive types:
  ```rust
  /// A parsed directive from a test file.
  ///
  /// The harness provides generic directives (revisions, compile-flags,
  /// CHECK variants) and a `Custom` variant for consumer-specific
  /// directives. This preserves the design principle that the harness
  /// "knows nothing about the Ori compiler" — consumer-specific
  /// directives like `// @test-arc-pass: realize_rc_reuse` are parsed
  /// as `Custom { key: "test-arc-pass", value: "realize_rc_reuse" }`
  /// and interpreted by the consumer's `TestStrategy` implementation.
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum Directive {
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
      /// `// @<key>: <value>` — consumer-specific directive.
      /// The harness parses the `key: value` structure; interpretation
      /// is delegated to the consumer's `TestStrategy`. Examples:
      /// `// @test-arc-pass: realize_rc_reuse` (§03 AIMS snapshots)
      Custom { key: String, value: String },
  }

  /// A directive line with source location and revision gate.
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct DirectiveLine {
      pub line_number: usize,
      pub revision: Option<String>,  // From [revision] prefix
      pub directive: Directive,
  }
  ```

- [ ] Define parse error type and result:
  ```rust
  /// An error encountered during directive parsing.
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct ParseError {
      pub line_number: usize,
      pub message: String,
  }

  /// Result of parsing directives from a test file.
  #[derive(Debug)]
  pub struct ParseResult {
      pub directives: Vec<DirectiveLine>,
      pub errors: Vec<ParseError>,
  }
  ```

- [ ] Implement `parse_directives(source: &str) -> ParseResult`:
  - Scan lines for `// @` prefix (line-anchored: must start at beginning of line after optional whitespace)
  - Handle `// @[revision_name] directive-name: value` syntax
  - Parse `// CHECK:`, `// CHECK-LABEL:`, etc. as FileCheck directives (also line-anchored)
  - Forbidden revision names: `true`, `false`, `CHECK`, `COM`, `NEXT`, `SAME`, `EMPTY`, `NOT`, `COUNT`, `DAG`, `LABEL` (from Rust compiletest) — produce a `ParseError` for each
  - Malformed directives (recognized prefix but unparseable value) → `ParseError` (not silent drop)
  - Return `ParseResult` with both successfully parsed directives and errors, with 1-based line numbers
  - Use `regex` crate for the line-anchored patterns. Compile patterns once via `LazyLock` (not per-call).

- [ ] **TDD: Write tests BEFORE implementing `parse_directives()`.** Verify tests fail first, then implement, then verify tests pass unchanged. Tests in `compiler/ori_test_harness/src/directive/tests.rs` (per `impl-hygiene.md` — sibling `tests.rs`, not inline):

  **Matrix dimensions:** directive_type × revision_gate × error_case

  **Positive (semantic pins — each verifies one directive type is parsed correctly):**
  - `test_parse_custom_directive_extracts_key_and_value` (e.g., `// @test-arc-pass: realize_rc_reuse` → `Custom { key: "test-arc-pass", value: "realize_rc_reuse" }`)
  - `test_parse_revisions_directive_splits_on_whitespace`
  - `test_parse_compile_flags_directive_collects_flags`
  - `test_parse_check_directive_preserves_pattern`
  - `test_parse_check_not_directive_preserves_pattern`
  - `test_parse_check_label_directive_preserves_pattern`
  - `test_parse_check_next_directive_preserves_pattern`
  - `test_parse_revision_gated_directive_records_revision_name`
  - `test_parse_mixed_directives_returns_source_order`
  - `test_parse_whitespace_before_comment_marker_accepted`

  **Negative pins (verify rejection/ignoring of invalid input):**
  - `test_parse_forbidden_revision_name_produces_error`
  - `test_parse_malformed_directive_produces_error`
  - `test_parse_non_directive_comment_ignored`
  - `test_parse_directive_inside_string_literal_not_matched` (line-based limitation acknowledgment)

- [ ] **Subsection close-out (02.2)** — MANDATORY before starting 02.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 02.3 Artifact Naming and Storage

**File(s):** `compiler/ori_test_harness/src/artifact.rs`, `compiler/ori_test_harness/src/artifact/tests.rs`

Define how test artifacts (`.before.arc`, `.after.arc`, `.diff`, `.ll`) are named, stored, and located. Follow Rust's MIR-opt pattern: expected baselines live alongside test source files.

- [ ] Define artifact types:
  ```rust
  /// Resolved paths for expected and actual artifact files.
  ///
  /// The harness provides generic path resolution and comparison.
  /// Artifact NAMING (what the path looks like) is the consumer's
  /// responsibility — the harness never decides whether an artifact
  /// is `.arc`, `.ll`, or something else. This preserves the design
  /// principle that the harness "knows nothing about the Ori compiler."
  #[derive(Debug, Clone)]
  pub struct ArtifactPaths {
      /// Expected baseline file (in source tree, alongside test file)
      pub expected: PathBuf,
      /// Actual output file (in build/temp directory)
      pub actual: PathBuf,
  }
  ```

- [ ] Implement generic artifact path resolution helpers:
  - `resolve_expected_path(test_path, suffix, revision)` — returns expected baseline path as sibling of test source file with revision inserted before extension
  - `resolve_actual_path(test_path, suffix, revision)` — returns actual output path under `target/test-harness/` (deterministic, not `$TMPDIR`, so artifacts survive for debugging)
  - **Revision suffix**: inserted before the consumer-provided extension: `test.debug.realize_rc_reuse.diff`
  - Expected files: same directory as test source
  - The harness provides path RESOLUTION (where baselines live, how revision suffixes are inserted). Artifact NAMING (what the suffix/extension is — `.arc`, `.ll`, `.diff`) is decided by the consumer's `TestStrategy::execute()` return value, not the harness. This preserves the "knows nothing about the compiler" boundary.

- [ ] **TDD: Write tests BEFORE implementing artifact path resolution.** Tests in `compiler/ori_test_harness/src/artifact/tests.rs`:
  - `test_expected_path_is_sibling_of_source_file`
  - `test_actual_path_is_under_target_test_harness`
  - `test_resolve_without_revision_omits_revision_suffix`
  - `test_resolve_with_revision_inserts_suffix_before_extension`
  - `test_revision_suffix_ordering_is_deterministic`

- [ ] **Subsection close-out (02.3)** — MANDATORY before starting 02.4:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 02.4 Bless Mode and Diff Generation

**File(s):** `compiler/ori_test_harness/src/bless.rs`, `compiler/ori_test_harness/src/diff.rs`, `compiler/ori_test_harness/src/bless/tests.rs`, `compiler/ori_test_harness/src/diff/tests.rs`

Implement bless mode and diff generation. **Bless mode is controlled exclusively via the `ORI_BLESS=1` environment variable.** There is no `--bless` CLI flag — `cargo test` rejects unrecognized CLI flags, so env var is the only viable control plane. The single query point is `bless::is_bless_enabled()`.

- [ ] Implement `bless::is_bless_enabled()` — the **single query point** for bless mode:
  ```rust
  /// Check if bless mode is active.
  ///
  /// Bless mode is controlled exclusively via the `ORI_BLESS=1` environment
  /// variable. There is no CLI flag — `cargo test` rejects unrecognized flags.
  /// All harness code queries this function; no other mechanism exists.
  pub fn is_bless_enabled() -> bool {
      std::env::var("ORI_BLESS").is_ok_and(|v| v == "1")
  }
  ```

- [ ] Implement `compare_or_bless()` (following Rust compiletest pattern):
  ```rust
  #[derive(Debug, PartialEq, Eq)]
  pub enum CompareOutcome {
      /// Expected matches actual.
      Match,
      /// Blessed: wrote new/updated baseline.
      Blessed,
      /// Blessed: removed empty baseline file.
      BlessedEmpty,
      /// Mismatch with diff.
      Mismatch { diff: String },
  }

  pub fn compare_or_bless(
      expected_path: &Path,
      actual: &str,
  ) -> Result<CompareOutcome, io::Error> {
      let bless = is_bless_enabled();
      if bless {
          if actual.is_empty() && expected_path.exists() {
              fs::remove_file(expected_path)?;
              return Ok(CompareOutcome::BlessedEmpty);
          }
          if !actual.is_empty() {
              // Ensure parent directory exists
              if let Some(parent) = expected_path.parent() {
                  fs::create_dir_all(parent)?;
              }
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
              diff: diff::generate_diff(&expected, actual),
          })
      }
  }
  ```

- [ ] Implement diff generation using `similar` crate:
  ```rust
  /// Generate a unified diff between expected and actual text.
  ///
  /// Output format: standard unified diff with context lines,
  /// line numbers, and +/- prefixes. Designed for terminal readability.
  pub fn generate_diff(expected: &str, actual: &str) -> String {
      // Use similar::TextDiff with unified_diff() formatter
      // Include 3 lines of context (standard unified diff default)
  }
  ```

- [ ] Bless mode must clean up old revision-specific files when revisions change (Rust compiletest deletes non-revision files when introducing revisions).

- [ ] **TDD: Write tests BEFORE implementing bless/diff.** Tests in `compiler/ori_test_harness/src/bless/tests.rs`:

  **Positive (semantic pins):**
  - `test_bless_writes_new_baseline_when_env_set_to_1`
  - `test_bless_deletes_empty_baseline_when_env_set`
  - `test_compare_returns_match_when_content_identical`
  - `test_compare_returns_mismatch_with_diff_when_content_differs`
  - `test_bless_creates_parent_directories`
  - `test_bless_cleans_old_revision_files`

  **Negative pins:**
  - `test_bless_disabled_when_env_is_zero` (`ORI_BLESS=0` → disabled)
  - `test_bless_disabled_when_env_is_false` (`ORI_BLESS=false` → disabled)
  - `test_bless_disabled_when_env_is_true` (`ORI_BLESS=true` → disabled; only `1` is accepted)
  - `test_bless_disabled_when_env_unset`

- [ ] Add tests in `compiler/ori_test_harness/src/diff/tests.rs`:
  - `test_diff_shows_added_lines_with_plus_prefix`
  - `test_diff_shows_removed_lines_with_minus_prefix`
  - `test_diff_includes_context_lines`
  - `test_diff_empty_expected_shows_all_actual_as_added`
  - `test_diff_identical_inputs_produces_empty_output`

- [ ] **TPR checkpoint** — `/tpr-review` covering 02.1-02.4 implementation work

- [ ] **Subsection close-out (02.4)** — MANDATORY before starting 02.5:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 02.5 Revision System

**File(s):** `compiler/ori_test_harness/src/revision.rs`, `compiler/ori_test_harness/src/revision/tests.rs`

Implement the revision expansion system. **Critical design boundary:** the harness extracts revision *names* and per-revision `// @[rev] compile-flags:` directives. It does NOT translate revision names into compiler flags or env vars — that is the consumer's responsibility inside `TestStrategy::execute()`. Hardcoding `--release` or `ORI_NO_REPR_OPT=1` in the harness would violate SSOT (the harness would encode compiler-specific knowledge).

- [ ] Define revision configuration:
  ```rust
  /// A single test revision extracted from directives.
  ///
  /// The harness extracts the revision name and any explicit
  /// `// @[name] compile-flags:` directives. Translation of
  /// revision names into actual compiler flags/env vars belongs
  /// in the consumer's `TestStrategy::execute()`.
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct RevisionConfig {
      /// Revision name (e.g., "debug", "release", "no-repr-opt")
      pub name: String,
      /// Explicit compile flags from `// @[name] compile-flags:` directives
      pub compile_flags: Vec<String>,
  }

  /// Expand revisions from parsed directives.
  ///
  /// - If no `// @revisions:` directive exists, returns a single
  ///   default revision with name "" (empty) and no flags.
  /// - If revisions are defined, returns one `RevisionConfig` per
  ///   revision name, with revision-gated compile-flags applied.
  pub fn expand_revisions(
      directives: &[DirectiveLine],
  ) -> Vec<RevisionConfig> {
      // Implementation
  }
  ```

- [ ] Implement `filter_directives_for_revision()` — given a list of directives and an active revision name, return only the directives that apply (ungated directives + directives gated to this revision):
  ```rust
  pub fn filter_directives_for_revision<'a>(
      directives: &'a [DirectiveLine],
      revision: &str,
  ) -> Vec<&'a DirectiveLine> {
      directives.iter().filter(|d| {
          d.revision.is_none()
              || d.revision.as_deref() == Some(revision)
      }).collect()
  }
  ```

- [ ] Revision-specific CHECK prefixes: when a revision named `debug` is active, `// @[debug] CHECK:` directives apply in addition to unprefixed `// CHECK:` directives. This is handled by `filter_directives_for_revision()` — no special prefix mechanism needed. (Simpler than Rust's approach of `// DEBUG-CHECK:` because our revision gating already covers this via `// @[debug] CHECK:`.)

- [ ] **TDD: Write tests BEFORE implementing revision expansion.** Tests in `compiler/ori_test_harness/src/revision/tests.rs`:

  **Positive:**
  - `test_no_revisions_directive_returns_single_default`
  - `test_revisions_directive_expands_to_one_config_per_name`
  - `test_revision_specific_compile_flags_applied_to_correct_revision`
  - `test_filter_directives_returns_ungated_plus_matching_revision`

  **Negative:**
  - `test_filter_directives_excludes_other_revision_directives`

- [ ] **Subsection close-out (02.5)** — MANDATORY before starting 02.6:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 02.6 Test Runner Orchestration (TestStrategy Trait)

**File(s):** `compiler/ori_test_harness/src/runner.rs`, `compiler/ori_test_harness/src/runner/tests.rs`

This is the most critical subsection. Without a canonical test runner loop, §03 and §07 will independently implement the traverse-read-parse-expand-invoke-diff algorithm, creating exactly the algorithmic DRY violation the harness exists to prevent.

The harness owns the orchestration algorithm. Consumer crates provide a `TestStrategy` callback that handles compiler-specific behavior (compilation, IR capture, flag/env-var translation). The harness never calls the compiler directly.

- [ ] Define the `TestStrategy` trait:
  ```rust
  /// Consumer-provided strategy for test execution.
  ///
  /// The harness orchestrates the test loop (discover → parse → expand →
  /// invoke → diff). The consumer implements this trait to provide
  /// compiler-specific behavior: compilation, IR capture, revision
  /// configuration, and result comparison.
  ///
  /// Implementations:
  /// - `ori_arc` provides `AimsSnapshotStrategy` (§03)
  /// - `ori_llvm` provides `FileCheckStrategy` (§07)
  pub trait TestStrategy {
      /// The type of error this strategy can produce.
      type Error: std::fmt::Display;

      /// Execute the test for a specific revision and produce output.
      ///
      /// The harness calls this once per revision. The strategy is
      /// responsible for: (1) translating the revision config into
      /// compiler flags/env vars, (2) compiling the test file, and
      /// (3) capturing the relevant output. Revision translation is
      /// done HERE so state is local to this call — no process-global
      /// side effects or interior mutation needed.
      ///
      /// Example: revision "release" → pass `--release` to compiler;
      /// revision "no-repr-opt" → set `ORI_NO_REPR_OPT=1` for this run.
      fn execute(
          &self,
          test_path: &Path,
          revision: &RevisionConfig,
          directives: &[DirectiveLine],
      ) -> Result<TestOutput, Self::Error>;

      /// Compare the actual output against expectations.
      ///
      /// For snapshot tests (§03): compare against baseline files.
      /// For FileCheck tests (§07): match CHECK directives against IR.
      /// Returns Ok(()) if the test passes, Err with details if it fails.
      fn verify(
          &self,
          test_path: &Path,
          revision: &RevisionConfig,
          directives: &[DirectiveLine],
          output: &TestOutput,
      ) -> Result<(), Self::Error>;
  }

  /// Output produced by a test execution.
  #[derive(Debug, Clone)]
  pub struct TestOutput {
      /// The captured output (IR text, snapshot text, etc.)
      pub content: String,
      /// Artifact paths produced (for bless mode)
      pub artifacts: Vec<ArtifactPaths>,
  }
  ```

- [ ] Implement `run_test_directory()` — the canonical orchestration loop:
  ```rust
  /// Run all tests in a directory using the given strategy.
  ///
  /// This is the SINGLE canonical test loop. Consumers (§03, §07) call
  /// this with their `TestStrategy` impl. They never duplicate the
  /// traverse → parse → expand → invoke → diff algorithm.
  ///
  /// Returns a summary of test results.
  pub fn run_test_directory<S: TestStrategy>(
      dir: &Path,
      strategy: &S,
  ) -> TestSummary {
      let mut summary = TestSummary::default();

      // 1. Discover test files (recursive walk, .ori extension)
      let test_files = discover_test_files(dir);
      if test_files.is_empty() {
          summary.failed += 1;
          summary.failures.push(format!(
              "no .ori test files found in {} (empty corpus = failure, not warning)",
              dir.display()
          ));
          return summary;
      }

      for test_path in &test_files {
          // 2. Read source and parse directives
          let source = match std::fs::read_to_string(test_path) {
              Ok(s) => s,
              Err(e) => {
                  summary.errors.push(format!(
                      "{}: read failed: {e}", test_path.display()
                  ));
                  continue;
              }
          };
          let parse_result = directive::parse_directives(&source);

          // 2b. Report parse errors and fail fast if any exist
          if !parse_result.errors.is_empty() {
              for err in &parse_result.errors {
                  summary.errors.push(format!(
                      "{}:{}: {}", test_path.display(),
                      err.line_number, err.message
                  ));
              }
              summary.failed += 1;
              summary.failures.push(format!(
                  "{}: {} parse error(s) — skipping execution",
                  test_path.display(), parse_result.errors.len()
              ));
              continue;
          }

          // 2c. Fail on zero actionable directives (orphan test prevention)
          if parse_result.directives.is_empty() {
              summary.failed += 1;
              summary.failures.push(format!(
                  "{}: no directives found (orphan test — check for typos in directive syntax)",
                  test_path.display()
              ));
              continue;
          }

          let directives = parse_result.directives;

          // 3. Expand revisions
          let revisions = revision::expand_revisions(&directives);

          // 4. For each revision: configure → execute → verify
          for rev in &revisions {
              let filtered = revision::filter_directives_for_revision(
                  &directives, &rev.name
              );

              match strategy.execute(test_path, rev, &filtered) {
                  Ok(output) => {
                      match strategy.verify(
                          test_path, rev, &filtered, &output
                      ) {
                          Ok(()) => summary.passed += 1,
                          Err(e) => {
                              summary.failed += 1;
                              summary.failures.push(format!(
                                  "{}[{}]: {e}",
                                  test_path.display(), rev.name
                              ));
                          }
                      }
                  }
                  Err(e) => {
                      summary.failed += 1;
                      summary.failures.push(format!(
                          "{}[{}]: execute failed: {e}",
                          test_path.display(), rev.name
                      ));
                  }
              }
          }
      }

      summary
  }
  ```

- [ ] Implement `discover_test_files()` using `walkdir` crate — simple recursive `.ori` file walker (do NOT import from `oric` — the harness must not depend on compiler crates):
  ```rust
  fn discover_test_files(dir: &Path) -> Vec<PathBuf> {
      use walkdir::WalkDir;
      let mut files: Vec<PathBuf> = WalkDir::new(dir)
          .into_iter()
          .filter_map(|e| e.ok())
          .filter(|e| e.file_type().is_file())
          .filter(|e| e.path().extension().is_some_and(|ext| ext == "ori"))
          .filter(|e| !e.path().components().any(|c| {
              c.as_os_str().to_str().is_some_and(|s| s.starts_with('.') || s == "target")
          }))
          .map(|e| e.into_path())
          .collect();
      files.sort();
      files
  }
  ```

- [ ] Define `TestSummary`:
  ```rust
  #[derive(Debug, Default)]
  pub struct TestSummary {
      pub passed: usize,
      pub failed: usize,
      pub failures: Vec<String>,
      pub warnings: Vec<String>,
      pub errors: Vec<String>,
  }

  impl TestSummary {
      pub fn is_success(&self) -> bool {
          self.failed == 0 && self.errors.is_empty()
      }
  }
  ```

- [ ] **TDD: Write tests BEFORE implementing `run_test_directory()`.** Tests in `compiler/ori_test_harness/src/runner/tests.rs` using `MockTestStrategy`:

  **Positive (semantic pins):**
  - `test_run_single_file_invokes_strategy_once`
  - `test_run_with_revisions_invokes_strategy_per_revision`
  - `test_run_summary_reports_correct_pass_fail_counts`

  **Negative pins:**
  - `test_run_empty_directory_fails_as_empty_corpus`
  - `test_run_file_with_zero_directives_fails_as_orphan`
  - `test_run_strategy_execute_error_counted_as_failure`
  - `test_run_strategy_verify_error_counted_as_failure`
  - `test_run_file_with_parse_errors_reports_them`

- [ ] **Subsection close-out (02.6)** — MANDATORY before starting 02.7:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 02.7 Seed Tests with Mock TestStrategy

**File(s):** `compiler/ori_test_harness/src/runner/mock.rs` (or inline in tests), seed `.ori` files

Validate that the harness orchestration, directive parsing, revision expansion, and bless mode work end-to-end without any compiler integration. This uses a `MockTestStrategy` that does not compile Ori code — it returns predetermined output based on the test file's directives.

**Why mock tests are necessary:** §03 and §07 cannot be started until §02 is complete. But §02's seed tests cannot exercise the full pipeline without §03/§07's `TestStrategy` implementations. A `MockTestStrategy` proves that the harness's orchestration algorithm is correct independently of compiler behavior. When §03 and §07 plug in their real strategies, they inherit a known-good orchestration loop.

- [ ] Implement `MockTestStrategy` for harness-only validation:
  ```rust
  /// A test strategy that returns predetermined output.
  ///
  /// Used to validate the harness orchestration loop without
  /// depending on the Ori compiler. The mock reads the test file,
  /// identifies directives, and returns synthetic output that either
  /// matches or mismatches expectations (controlled by test setup).
  #[cfg(test)]
  pub struct MockTestStrategy {
      /// Output to return from execute(). Keyed by (test_path, revision).
      pub outputs: HashMap<(PathBuf, String), String>,
  }
  ```

- [ ] Create seed test files in a temporary directory (not in `compiler/ori_llvm/tests/codegen/` or `compiler/ori_arc/tests/arc-opt/` — those are consumer directories created by §03/§07):
  - Seed file with `// @revisions: alpha beta` and `// @[alpha] compile-flags: --opt`
  - Seed file with `// @test-arc-pass: realize_rc_reuse`
  - Seed file with `// CHECK: some_pattern` and `// CHECK-NOT: bad_pattern`

- [ ] Write integration tests proving:
  - `test_mock_strategy_single_file_passes_when_output_matches`
  - `test_mock_strategy_revision_expansion_calls_execute_per_revision`
  - `test_mock_strategy_bless_mode_writes_baseline` (set `ORI_BLESS=1` in test env)
  - `test_mock_strategy_mismatch_produces_diff_in_failure`
  - `test_mock_strategy_directive_filtering_by_revision`

- [ ] **Subsection close-out (02.7)** — MANDATORY before starting 02.R:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 02.R Third Party Review Findings

- [x] `[TPR-02-001-codex][high]` `section-02-shared-harness.md:180` — parse_directives returns Vec with no error surface for forbidden/malformed directives.
  Resolved: Fixed on 2026-04-10. Changed return type to `ParseResult { directives, errors }` with `ParseError` type. Added negative pin tests for malformed and forbidden directives.
- [x] `[TPR-02-002-codex][high]` `section-02-shared-harness.md:82` — Test corpus paths undecided; downstream sections hardcode `tests/codegen/` and `tests/arc-opt/`.
  Resolved: Fixed on 2026-04-10. Made canonical decision: crate-local paths (`compiler/ori_llvm/tests/codegen/`, `compiler/ori_arc/tests/arc-opt/`). Noted downstream sections need updating when reviewed.
- [x] `[TPR-02-003-codex][high]` `section-02-shared-harness.md:81` — aot.rs helpers can't be imported by separate integration test targets; planned reuse is impossible as written.
  Resolved: Fixed on 2026-04-10. Updated cross-section note to explicitly state extraction is required before §07 can reuse; documented two concrete extraction approaches.
- [x] `[TPR-02-004-codex][medium]` `section-03-aims-snapshots.md:105` — §03 and §07 sketch bespoke loops bypassing the canonical `run_test_directory()` harness loop.
  Resolved: Fixed on 2026-04-10. Added MANDATORY cross-section note that §03/§07 MUST use `run_test_directory()` and `bless::is_bless_enabled()` exclusively.
- [x] `[TPR-02-005-codex][medium]` `section-02-shared-harness.md:676` — Missing TDD ordering, matrix dimensions, semantic/negative pins.
  Resolved: Fixed on 2026-04-10. Added explicit TDD ordering ("write tests BEFORE implementing"), matrix dimensions (directive_type × revision_gate × error_case, bless_mode × env_value, runner × directive_count × strategy_outcome), semantic pins, and negative pins to all test subsections and completion checklist.
- [x] `[TPR-02-001-gemini][medium]` `section-02-shared-harness.md:133` — Test names missing `test_` prefix and inconsistent 3-part naming.
  Resolved: Fixed on 2026-04-10. Added `test_` prefix to all test names across §02.2-§02.7. Fixed names not following `<subject>_<scenario>_<expected>` pattern.
- [x] `[TPR-02-002-gemini][medium]` `section-02-shared-harness.md:76` — Missing `walkdir` dependency for recursive test file discovery.
  Resolved: Fixed on 2026-04-10. Added `walkdir = "2"` to Cargo.toml dependencies. Updated `discover_test_files()` implementation sketch to use `WalkDir`.
- [x] `[TPR-02-003-gemini][high]` `section-02-shared-harness.md:254` — No validation for zero directives; test files with typos silently pass.
  Resolved: Fixed on 2026-04-10. Added orphan test prevention: `run_test_directory()` now fails files with zero parsed directives. Added `test_run_file_with_zero_directives_fails_as_orphan` negative pin.
- [x] `[TPR-02-004-gemini][medium]` `section-02-shared-harness.md:170` — `is_bless_enabled()` uses `.is_ok()` which enables bless for ANY env value including `0`/`false`.
  Resolved: Fixed on 2026-04-10. Changed to `.is_ok_and(|v| v == "1")` (only "1" accepted, per single-control-plane contract). Added negative pins for env=0, env=false, env=true, env=unset.
**--- Round 2 findings (iteration 2) ---**
- [x] `[TPR-02-001-codex-r2][high]` `00-overview.md:29` — Downstream sections still reference `tests/codegen/` and `tests/arc-opt/` instead of crate-local paths.
  Resolved: Fixed on 2026-04-10. Updated ALL plan files to crate-local paths: `00-overview.md`, `index.md`, `section-03`, `section-07`, `section-09`, `section-11`, `research.md`. `tests/codegen/` → `compiler/ori_llvm/tests/codegen/`, `tests/arc-opt/` → `compiler/ori_arc/tests/arc-opt/`.
- [x] `[TPR-02-002-codex-r2][high]` `section-03-aims-snapshots.md:105` — §03/§07 still sketch bespoke loops bypassing `run_test_directory()`.
  Resolved: Already documented in §02 cross-section notes as MANDATORY. Will be enforced when §03/§07 undergo their own /review-plan pass. §02 cannot edit sibling section files in single-section review mode.
- [x] `[TPR-02-003-codex-r2][high]` `section-07-filecheck.md:123` — §07 needs AOT helper extraction task before `codegen_checks`.
  Resolved: Already documented in §02 cross-section notes as a §07 dependency. Will be enforced when §07 undergoes its own /review-plan pass.
- [x] `[TPR-02-004-codex-r2][medium]` `section-02-shared-harness.md:301` — `is_bless_enabled()` accepts both "1" and "true" but prose says only "ORI_BLESS=1".
  Resolved: Fixed on 2026-04-10. Changed to accept only "1". Added `test_bless_disabled_when_env_is_true` negative pin.
- [x] `[TPR-02-001-gemini-r2][high]` `section-02-shared-harness.md:161` — `TestArcPass` and `ArtifactKind` variants are compiler-specific; violate "knows nothing about compiler" design principle.
  Resolved: Fixed on 2026-04-10. Replaced `TestArcPass` with generic `Custom { key, value }` directive. Removed `ArtifactKind` enum; artifact naming delegated to consumer `TestStrategy`. Harness provides only generic path resolution helpers.
- [x] `[TPR-02-002-gemini-r2][medium]` `section-02-shared-harness.md:585` — Files with parse errors still get executed with partial directive sets.
  Resolved: Fixed on 2026-04-10. Added fail-fast: `if !parse_result.errors.is_empty() { continue; }` after reporting errors. Added `test_run_file_with_parse_errors_reports_them` negative pin.
**--- Round 3 findings (iteration 3) ---**
- [x] `[TPR-02-001-codex-r3][high]` `section-03-aims-snapshots.md:101` — §03 still uses `tests/arc-opt/` and bespoke WalkDir loop.
  Resolved: Fixed on 2026-04-10. Updated all `tests/arc-opt/` paths to `compiler/ori_arc/tests/arc-opt/` across §03 and all other plan files. §03's bespoke loop will be replaced with `run_test_directory()` when §03 is reviewed (documented in §02 MANDATORY cross-section note).
- [x] `[TPR-02-002-codex-r3][high]` `section-07-filecheck.md:74` — §07 still uses `tests/codegen/` and bespoke discover loop.
  Resolved: Fixed on 2026-04-10. Updated all `tests/codegen/` paths to `compiler/ori_llvm/tests/codegen/` across §07, §09, §11, index.md, research.md, and 00-overview.md. §07's bespoke loop will be replaced when §07 is reviewed.
- [x] `[TPR-02-003-codex-r3][medium]` `section-02-shared-harness.md:785` — Round-2 path migration claim overstated; sibling files still had stale paths.
  Resolved: Fixed on 2026-04-10. Updated ALL sibling files. Round-2 TPR entry updated to reflect complete migration. Zero remaining `tests/codegen/` or `tests/arc-opt/` (without `compiler/` prefix) in plan files.
- [x] `[TPR-02-001-gemini-r3][medium]` `section-02-shared-harness.md:391` — Diff tests in §02.4 missing `test_` prefix.
  Resolved: Fixed on 2026-04-10. Added `test_` prefix to all 5 diff test names.
**--- Round 4 findings (iteration 4) ---**
- [x] `[TPR-02-001-codex-r4][high]` `section-02-shared-harness.md:499` — `configure_revision()` returns no config object; revision state leaks via side effects.
  Resolved: Fixed on 2026-04-10. Removed `configure_revision()` from `TestStrategy`; revision translation folded into `execute()` so state is local to each call. No process-global side effects.
- [x] `[TPR-02-002-codex-r4][medium]` `section-02-shared-harness.md:566` — Empty test directory treated as warning (is_success() ignores warnings).
  Resolved: Fixed on 2026-04-10. Empty corpus now fails (failed += 1). Test renamed to `test_run_empty_directory_fails_as_empty_corpus`.
- [x] `[TPR-02-003-codex-r4][low]` `section-02-shared-harness.md:234` — Parser test named for revision filtering; belongs in §02.5.
  Resolved: Fixed on 2026-04-10. Renamed to `test_parse_revision_gated_directive_records_revision_name` (asserts parser records the gate, not that filtering works).
- [x] `[TPR-02-001-gemini-r4][medium]` `section-02-shared-harness.md:745` — Stale `tests/codegen/` and `tests/arc-opt/` in §02.7.
  Resolved: Fixed on 2026-04-10 (mid-run). Updated to crate-local paths.

---

## 02.N Completion Checklist

- [ ] `ori_test_harness` crate exists in workspace, compiles, passes its own tests
- [ ] Directive parser uses line-anchored regex (no Ori lexer dependency)
- [ ] Directive parser handles all directive types (generic `Custom { key, value }`, revisions, compile-flags, CHECK variants)
- [ ] Forbidden revision names validated and rejected
- [ ] Malformed directives produce `ParseError` (not silent drop); files with parse errors are not executed
- [ ] Artifact path resolution produces correct sibling/target paths with revision suffixes
- [ ] Bless mode controlled exclusively via `ORI_BLESS=1` env var; `is_bless_enabled()` is the single query point
- [ ] Bless mode writes/deletes baselines correctly; creates parent directories
- [ ] Revision expansion extracts names and per-revision compile-flags
- [ ] Revision system does NOT hardcode compiler flags — flag translation delegated to `TestStrategy`
- [ ] `TestStrategy` trait defines `execute` (with revision translation) and `verify`
- [ ] `run_test_directory()` provides the canonical orchestration loop
- [ ] `MockTestStrategy` validates orchestration without compiler integration
- [ ] Seed tests demonstrate directive parsing, revision expansion, bless mode, and diff generation
- [ ] **TDD discipline verified**: all tests were written BEFORE their implementation; tests failed before code, passed after
- [ ] **Test matrix coverage**: directive_type × revision_gate × error_case dimensions covered; bless_mode × env_value × file_state dimensions covered; runner × directive_count × strategy_outcome dimensions covered
- [ ] **Semantic pins**: at least one test per subsection that ONLY passes with the new behavior
- [ ] **Negative pins**: forbidden revision names, malformed directives, zero directives (orphan), bless with env=0/false/unset
- [ ] File sizes: all source files < 500 lines (per `impl-hygiene.md`); split if approaching limit
- [ ] Tests in sibling `tests.rs` files, not inline (per `impl-hygiene.md`)
- [ ] Test names follow `test_<subject>_<scenario>_<expected>` convention (per `impl-hygiene.md` §Test Function Naming)
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

**Exit Criteria:** `ori_test_harness` crate compiles and passes all internal tests. Directive parsing, artifact naming, bless mode, revision expansion, and the `TestStrategy`-based runner loop all work. `MockTestStrategy` proves the orchestration algorithm is correct without compiler integration. Section 03 and Section 07 can consume the harness by implementing `TestStrategy` without building their own test loop. Bless mode is controlled exclusively via `ORI_BLESS=1`. Revision flag translation is delegated to consumer strategies, not hardcoded in the harness.
