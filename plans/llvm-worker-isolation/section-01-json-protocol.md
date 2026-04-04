---
section: "01"
title: "JSON Output Protocol"
status: not-started
reviewed: true
goal: "Add --json flag to ori test that emits structured FileSummary as JSON to stdout, plus a BackendCrash outcome variant"
success_criteria:
  - "ori test --backend=llvm --json <file> emits sentinel-framed JSON to stdout for passing files"
  - "ori test --backend=llvm --json <file> emits sentinel-framed JSON for files with test failures"
  - "ori test --backend=llvm --json <file> emits sentinel-framed JSON for files with LlvmCompileFail"
  - "Sentinel-framed JSON is extractable even when Ori print() output is on stdout"
  - "serde round-trip test: serialize → deserialize → compare for all TestOutcome variants"
  - "BackendCrash variant exists in TestOutcome with is_backend_crash() predicate and has_failures() returns true"
  - "Satisfies mission criterion: --json emits structured JSON for any input"
inspired_by:
  - "Zig Compilation.zig sidecar diagnostic file pattern (structured output alongside exit code)"
  - "Rust cargo test --format=json (per-event JSON lines)"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "BackendCrash outcome variant"
    status: not-started
  - id: "01.2"
    title: "Serde derives on result types"
    status: not-started
  - id: "01.3"
    title: "--json flag and JSON emission"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: JSON Output Protocol

**Status:** Not Started
**Goal:** Establish the structured communication protocol between the orchestrator (parent) and worker (child) processes. Add a `--json` flag to `ori test` that emits `FileSummary` as JSON to stdout. Add a `BackendCrash` variant to `TestOutcome` for worker signal deaths.

**Success Criteria:**

- [ ] `ori test --backend=llvm --json <file>` emits sentinel-framed JSON for any input state (pass, fail, compile error) -- framing ensures `print()` output doesn't corrupt JSON
- [ ] `BackendCrash(String)` variant in `TestOutcome` — `is_backend_crash()` returns true, `has_failures()` returns true
- [ ] Serde round-trip: `serialize(summary) |> deserialize == summary` for all outcome variants
- [ ] Satisfies mission criterion: structured JSON output for orchestrator consumption

**Context:** The orchestrator needs to parse per-file test results from the worker subprocess. The current output is human-readable text (`print_test_summary` in `commands/test.rs`) with format variations depending on `--verbose`. Parsing this is fragile -- pass lines are omitted unless verbose, LLVM compile errors are suppressed unless verbose, and the summary line is aggregate-only. A `--json` flag provides the structured protocol the orchestrator needs.

**IMPORTANT: stdout is not clean.** The LLVM backend's `ori_print` (in `ori_rt/src/io/mod.rs`) uses `println!()` which writes to stdout. If any Ori test calls `print()`, the output goes to the same stdout as the JSON. The JSON must be sentinel-framed (`---ORI_JSON_BEGIN---` / `---ORI_JSON_END---`) so the orchestrator can extract it reliably despite any `print()` pollution.

**Reference implementations:**
- **Rust** `cargo test --format=json`: emits per-event JSON lines (one per test start/complete). Ori can use a simpler model — one JSON blob per file.
- **Zig** `Compilation.zig:6338-6343`: sidecar diagnostic file with structured format. Ori uses stdout instead (simpler, no temp file cleanup).

**Depends on:** Nothing.

---

## 01.1 BackendCrash Outcome Variant

**File(s):** `compiler/oric/src/test/result/mod.rs`

Add a new `TestOutcome::BackendCrash(String)` variant for tests whose LLVM worker process died by signal. This is distinct from `LlvmCompileFail` — compile failures are expected (codegen issues), but crashes are real failures that block the test gate.

**TDD ordering: write tests FIRST (01.1.T below), verify they fail, then implement.**

- [ ] **01.1.T — Tests first** (in `compiler/oric/src/test/result/tests.rs`):
  - [ ] `test_backend_crash_is_backend_crash` — `BackendCrash("msg".into()).is_backend_crash()` returns true
  - [ ] `test_backend_crash_is_not_failed` — `BackendCrash("msg".into()).is_failed()` returns false (distinct from `Failed`)
  - [ ] `test_backend_crash_counted_in_file_summary` — `add_result(BackendCrash)` increments `backend_crash` counter, NOT `failed`
  - [ ] `test_backend_crash_file_has_failures` — `FileSummary` with only `BackendCrash` results returns `has_failures() == true`
  - [ ] `test_backend_crash_summary_has_failures` — `TestSummary` with `backend_crash > 0` returns `has_failures() == true`
  - [ ] `test_backend_crash_exit_code_1` — `TestSummary` with `BackendCrash` produces `exit_code() == 1`
  - [ ] Verify all 6 tests FAIL before implementing (they reference types/methods that don't exist yet)
- [ ] Add `BackendCrash(String)` variant to `TestOutcome` enum (`result/mod.rs` line 9-21):
  ```rust
  /// LLVM worker process crashed (SIGSEGV, SIGABRT, etc.).
  /// Distinct from LlvmCompileFail — crashes are real failures.
  BackendCrash(String),
  ```
- [ ] Add `is_backend_crash()` predicate method alongside existing `is_passed()`, `is_failed()`, etc. (line 23-43)
- [ ] Add `backend_crash: usize` field to `FileSummary` struct (line 107-128, after `llvm_compile_fail`)
- [ ] Add `backend_crash: usize` field to `TestSummary` struct (line 165-185, after `llvm_compile_fail`)
- [ ] Update `add_result()` in `FileSummary` (line 138, the `match` on `result.outcome`) to add `BackendCrash(_) => self.backend_crash += 1`
- [ ] Update `has_failures()` in `FileSummary` (line 159: `self.failed > 0 || (!self.errors.is_empty() && !self.llvm_compile_error)`) — add `|| self.backend_crash > 0`
- [ ] Update `has_failures()` in `TestSummary` (line 216: `self.failed > 0 || self.error_files > 0`) — add `|| self.backend_crash > 0`
- [ ] Update `add_file()` in `TestSummary` (line 192) — add `self.backend_crash += summary.backend_crash`
- [ ] Update `exit_code()` in `TestSummary` — `BackendCrash` counts flow through `has_failures()`, producing exit code 1 (no change needed if `has_failures()` is correctly updated above)
- [ ] Verify all 6 tests from 01.1.T now PASS

---

## 01.2 Serde Derives on Result Types

**File(s):** `compiler/oric/src/test/result/mod.rs` (new submodule `json_protocol.rs`), `compiler/oric/Cargo.toml`

Add `Serialize`/`Deserialize` derives to result types so they can be emitted as JSON. The `Name` type (interned identifier) needs special handling — it can't be deserialized without an interner. Use string representation for JSON.

**File size constraint:** `result/mod.rs` is currently 327 lines. Adding ~80 lines of JSON mirror types would bring it to ~407 — under the 500-line limit but getting close. Extract JSON protocol types to a new `compiler/oric/src/test/result/json_protocol.rs` submodule for separation of concerns. Declare `pub mod json_protocol;` in `result/mod.rs`.

**TDD ordering: write round-trip tests FIRST.**

- [ ] Add `serde` and `serde_json` dependencies to `compiler/oric/Cargo.toml`. `serde` is in workspace deps (root `Cargo.toml` line 74: `serde = { version = "1", features = ["derive"] }`). `serde_json` is NOT in workspace deps — add it to workspace `[workspace.dependencies]` first, then use `workspace = true` in oric:
  ```toml
  # Root Cargo.toml [workspace.dependencies]:
  serde_json = "1"
  
  # compiler/oric/Cargo.toml [dependencies]:
  serde = { workspace = true }
  serde_json = { workspace = true }
  ```
- [ ] Create `compiler/oric/src/test/result/json_protocol.rs` with JSON-serializable mirror types:
  ```rust
  //! JSON wire protocol types for worker→orchestrator communication.
  use serde::{Deserialize, Serialize};

  /// JSON-serializable test result for worker→orchestrator protocol.
  #[derive(Serialize, Deserialize, Debug, Clone)]
  pub struct JsonTestResult {
      pub name: String,
      pub targets: Vec<String>,
      pub outcome: JsonTestOutcome,
      pub duration_ms: u64,
  }

  #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
  #[serde(tag = "type", content = "message")]
  pub enum JsonTestOutcome {
      Passed,
      Failed(String),
      Skipped(String),
      SkippedUnchanged,
      LlvmCompileFail(String),
      BackendCrash(String),
  }

  /// JSON-serializable file summary.
  #[derive(Serialize, Deserialize, Debug, Clone)]
  pub struct JsonFileSummary {
      pub path: String,
      pub results: Vec<JsonTestResult>,
      pub passed: usize,
      pub failed: usize,
      pub skipped: usize,
      pub llvm_compile_fail: usize,
      pub backend_crash: usize,
      pub duration_ms: u64,
      pub errors: Vec<String>,
      pub llvm_compile_error: bool,
  }

  /// Sentinel markers for framing JSON in stdout (robust against Ori print() pollution).
  pub const JSON_BEGIN_SENTINEL: &str = "---ORI_JSON_BEGIN---";
  pub const JSON_END_SENTINEL: &str = "---ORI_JSON_END---";
  ```
- [ ] Declare `pub mod json_protocol;` in `result/mod.rs`
- [ ] **01.2.T — Tests first** (in `compiler/oric/src/test/result/tests.rs`):
  - [ ] `test_json_outcome_round_trip_all_variants` — serialize each of the 6 `JsonTestOutcome` variants to JSON, deserialize back, compare equal
  - [ ] `test_json_file_summary_round_trip` — serialize a `JsonFileSummary` with mixed outcomes, deserialize back, verify field equality
  - [ ] `test_file_summary_to_json_correct_fields` — create a `FileSummary` with known values, call `to_json()`, verify all fields match
  - [ ] `test_json_file_summary_into_file_summary` — round-trip `FileSummary` → `to_json()` → `into_file_summary()` → compare counters and outcome types
  - [ ] `test_empty_results_json` — file with 0 tests produces valid JSON with `results: []` and all counters at 0
  - [ ] Verify tests fail before implementing conversion methods
- [ ] Add `FileSummary::to_json(&self, interner: &StringInterner) -> JsonFileSummary` conversion in `json_protocol.rs` (resolves `Name` → `String` via interner)
- [ ] Add `JsonFileSummary::into_file_summary(self, interner: &StringInterner) -> FileSummary` reverse conversion (re-interns `String` → `Name`). Note: re-interning creates new `Name` values in the orchestrator's interner — this is correct since the worker's `Name` values are process-local.
- [ ] **Exit code 2 (no tests) representation**: When a file has no LLVM-eligible tests, the worker emits a `JsonFileSummary` with `results: []` and all counters at 0. The orchestrator treats this as a no-op (not a failure, not a crash).
- [ ] Verify all tests from 01.2.T now PASS

---

## 01.3 --json Flag and JSON Emission

**File(s):** `compiler/oric/src/main.rs` (lines 118-149), `compiler/oric/src/commands/test.rs`, `compiler/oric/src/test/runner/mod.rs` (lines 38-56)

Add `--json` flag to the test command that emits `JsonFileSummary` to stdout instead of human-readable output. When `--json` is active, all human-readable output (progress, errors, summaries) goes to stderr or is suppressed. Only the sentinel-framed JSON blob goes to stdout.

**Design decision: Sentinel framing (Option C).** Worker emits `---ORI_JSON_BEGIN---` / `---ORI_JSON_END---` sentinels around the JSON blob. Orchestrator extracts content between sentinels, ignoring any Ori `print()` output that pollutes stdout. This is simpler than temp files (Option B) and more robust than raw stdout parsing (Option A). Sentinel constants live in `json_protocol.rs` (defined in 01.2).

**TDD ordering: write integration tests, verify they fail (no `--json` flag yet), then implement.**

- [ ] Add `json: bool` field to `TestRunnerConfig` in `runner/mod.rs` (line 43, inside the struct). Also update `Default` impl (line 58-68) with `json: false`. The `#[expect(clippy::struct_excessive_bools)]` on line 39 remains valid (now 5 bools: `verbose`, `parallel`, `coverage`, `incremental`, `json`).
- [ ] Parse `--json` flag in `main.rs` test command block (lines 118-149, inside the `"test"` match arm):
  ```rust
  } else if arg == "--json" {
      config.json = true;
  }
  ```
- [ ] In `commands/test.rs:run_tests()` (line 10), when `config.json` is true:
  - Suppress human-readable output (no `print_test_summary`, no `print_summary_stats`)
  - After `runner.run()` completes, serialize per-file `FileSummary` as JSON to stdout
  - Use sentinel framing constants from `json_protocol.rs`:
  ```rust
  use oric::test::result::json_protocol::{JSON_BEGIN_SENTINEL, JSON_END_SENTINEL};
  
  if config.json {
      let interner = runner.interner();
      let json_summaries: Vec<JsonFileSummary> = summary.files
          .iter()
          .map(|f| f.to_json(interner))
          .collect();
      println!("{JSON_BEGIN_SENTINEL}");
      println!("{}", serde_json::to_string(&json_summaries).unwrap());
      println!("{JSON_END_SENTINEL}");
  } else {
      // existing human-readable output
  }
  ```
- [ ] **stdout pollution note**: Ori `print()` uses `println!()` in `ori_rt` which writes to stdout. Tracing already goes to stderr. The sentinel framing handles `print()` pollution — the orchestrator (Section 02) scans for sentinels and ignores everything else. No runtime changes needed.
- [ ] **01.3.T — Integration tests** (in `compiler/oric/tests/phases/` or `compiler/oric/src/test/runner/tests.rs`):
  - [ ] `test_json_flag_emits_sentinel_framed_json` — spawn `ori test --backend=llvm --json tests/spec/types/primitives.ori` via `Command::new(current_exe)`, capture stdout, verify `---ORI_JSON_BEGIN---` and `---ORI_JSON_END---` are present, extract content between them, parse as `Vec<JsonFileSummary>`, verify pass count > 0
  - [ ] `test_json_flag_multi_test_file` — use a file with many tests (`tests/spec/inference/unification.ori`), verify per-test results in JSON
  - [ ] `test_json_flag_stdout_pollution_resilience` — create/use a test file that calls `print()`, verify sentinel-framed JSON is extractable despite Ori `print()` output on stdout. This is the critical robustness test.
  - [ ] `test_json_flag_compile_error_file` — use a file with type errors, verify JSON contains `LlvmCompileFail` outcomes
  - [ ] `test_no_json_flag_unchanged` — verify `ori test --backend=llvm tests/spec/types/primitives.ori` (no `--json`) produces the same human-readable output as before (regression guard)
- [ ] Verify tests fail before implementing, then pass after

---

## 01.R Third Party Review Findings

- None.

---

## 01.N Completion Checklist

- [ ] `BackendCrash` variant added to `TestOutcome` with `is_backend_crash()` predicate
- [ ] `has_failures()` returns true for `BackendCrash` — crashes block the test gate
- [ ] `backend_crash` counter added to `FileSummary` and `TestSummary`
- [ ] JSON mirror types in `compiler/oric/src/test/result/json_protocol.rs` (new submodule, under 200 lines)
- [ ] `FileSummary::to_json()` and `JsonFileSummary::into_file_summary()` conversions work
- [ ] `--json` flag parsed in CLI and routed through `TestRunnerConfig`
- [ ] `ori test --backend=llvm --json <file>` emits sentinel-framed JSON to stdout (robust against Ori `print()` output)
- [ ] **TDD verified**: all tests written before implementation, verified failing, then passing
- [ ] Unit tests: 6 BackendCrash tests, 5 JSON round-trip/conversion tests (in `result/tests.rs`)
- [ ] Integration tests: 5 tests covering JSON output, pollution resilience, compile errors, regression (in `runner/tests.rs` or `tests/phases/`)
- [ ] `result/mod.rs` stays under 500 lines (currently 327 + ~10 lines for new variant/field/arm)
- [ ] `json_protocol.rs` stays under 200 lines
- [ ] `timeout 150 ./test-all.sh` passes — no regressions (JSON flag is opt-in, default behavior unchanged)
- [ ] `./clippy-all.sh` passes
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 01` returns 0 annotations
- [ ] **Plan sync** — update plan metadata
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** `ori test --backend=llvm --json tests/spec/types/primitives.ori` emits a sentinel-framed JSON `JsonFileSummary` to stdout with correct pass/fail/skip counts. The framing is robust against Ori `print()` output on stdout. `BackendCrash` variant exists and is counted as a real failure. All existing tests pass unchanged (JSON is opt-in). Serde round-trip test verifies all 6 `JsonTestOutcome` variants serialize and deserialize correctly.
