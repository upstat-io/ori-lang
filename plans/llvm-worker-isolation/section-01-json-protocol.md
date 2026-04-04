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

**IMPORTANT: stdout is not clean.** The LLVM backend's `ori_print` (in `ori_rt/src/io/mod.rs`) uses `println!()` which writes to stdout. If any Ori test calls `print()`, the output goes to the same stdout as the JSON. The JSON must be sentinel-framed (`---ORI_JSON_BEGIN---` / `---ORI_JSON_END---`) so the orchestrator can extract it reliably despite any `print()` pollution. <!-- reviewed: feasibility fix -->

**Reference implementations:**
- **Rust** `cargo test --format=json`: emits per-event JSON lines (one per test start/complete). Ori can use a simpler model — one JSON blob per file.
- **Zig** `Compilation.zig:6338-6343`: sidecar diagnostic file with structured format. Ori uses stdout instead (simpler, no temp file cleanup).

**Depends on:** Nothing.

---

## 01.1 BackendCrash Outcome Variant

**File(s):** `compiler/oric/src/test/result/mod.rs`

Add a new `TestOutcome::BackendCrash(String)` variant for tests whose LLVM worker process died by signal. This is distinct from `LlvmCompileFail` — compile failures are expected (codegen issues), but crashes are real failures that block the test gate.

- [ ] Add `BackendCrash(String)` variant to `TestOutcome` enum in `result/mod.rs:9-21`
  ```rust
  pub enum TestOutcome {
      Passed,
      Failed(String),
      Skipped(String),
      SkippedUnchanged,
      LlvmCompileFail(String),
      /// LLVM worker process crashed (SIGSEGV, SIGABRT, etc.).
      /// Distinct from LlvmCompileFail — crashes are real failures.
      BackendCrash(String),
  }
  ```
- [ ] Add `is_backend_crash()` predicate method alongside existing `is_passed()`, `is_failed()`, etc.
- [ ] Update `has_failures()` in BOTH `FileSummary` (line 159: `self.failed > 0 || (!self.errors.is_empty() && !self.llvm_compile_error)`) AND `TestSummary` (line 216: `self.failed > 0 || self.error_files > 0`) to include `BackendCrash` counts -- crashes are real failures that block the exit code. Also update `add_file()` in `TestSummary` (line 192) to aggregate the new `backend_crash` counter. <!-- reviewed: accuracy fix — both structs have has_failures(), with exact line numbers -->
- [ ] Add `backend_crash` counter to `FileSummary` and `TestSummary` structs, matching the pattern of `llvm_compile_fail`
- [ ] Update `add_result()` in `FileSummary` (line 138, the `match` on `result.outcome`) to add a `BackendCrash(_) => self.backend_crash += 1` arm
- [ ] Update `exit_code()` in `TestSummary` — `BackendCrash` counts toward `has_failures()`, producing exit code 1

---

## 01.2 Serde Derives on Result Types

**File(s):** `compiler/oric/src/test/result/mod.rs`, `compiler/oric/Cargo.toml`

Add `Serialize`/`Deserialize` derives to result types so they can be emitted as JSON. The `Name` type (interned identifier) needs special handling — it can't be deserialized without an interner. Use string representation for JSON.

- [ ] Add `serde` and `serde_json` dependencies to `compiler/oric/Cargo.toml`. `serde` is in workspace deps (root `Cargo.toml` line 74: `serde = { version = "1", features = ["derive"] }`). `serde_json` is NOT in workspace deps -- either add it to workspace deps first, or use a direct version: <!-- reviewed: accuracy fix — verified workspace deps -->
  ```toml
  [dependencies]
  serde = { workspace = true }
  serde_json = "1"
  ```
- [ ] Create a JSON-serializable mirror of the result types that replaces `Name` with `String`:
  ```rust
  /// JSON-serializable test result for worker→orchestrator protocol.
  #[derive(Serialize, Deserialize, Debug)]
  pub struct JsonTestResult {
      pub name: String,
      pub targets: Vec<String>,
      pub outcome: JsonTestOutcome,
      pub duration_ms: u64,
  }

  #[derive(Serialize, Deserialize, Debug, PartialEq)]
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
  #[derive(Serialize, Deserialize, Debug)]
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
  }
  ```
- [ ] Add `FileSummary::to_json()` conversion method that resolves `Name` values via the interner:
  ```rust
  impl FileSummary {
      pub fn to_json(&self, interner: &StringInterner) -> JsonFileSummary { ... }
  }
  ```
- [ ] Add `JsonFileSummary::into_file_summary()` reverse conversion for the orchestrator (re-interns names): <!-- reviewed: feasibility note — re-interning creates new Name values in the orchestrator's interner. This is correct: the worker's Name values are process-local and meaningless to the orchestrator. The orchestrator re-interns the string representations to get its own Name values, which it uses for display via its own interner. -->
  ```rust
  impl JsonFileSummary {
      pub fn into_file_summary(self, interner: &StringInterner) -> FileSummary { ... }
  }
  ```
- [ ] **Exit code 2 (no tests) representation**: When a file has no LLVM-eligible tests, the worker emits a `JsonFileSummary` with `results: []` and all counters at 0. The orchestrator treats this as a no-op (not a failure, not a crash).
- [ ] Rust unit test: serde round-trip for all `JsonTestOutcome` variants — serialize to JSON string, deserialize back, compare equal. Test file: `compiler/oric/src/test/result/tests.rs`
- [ ] Rust unit test: `FileSummary::to_json()` produces valid JSON with correct field values

---

## 01.3 --json Flag and JSON Emission

**File(s):** `compiler/oric/src/main.rs`, `compiler/oric/src/commands/test.rs`, `compiler/oric/src/test/runner/mod.rs`

Add `--json` flag to the test command that emits `JsonFileSummary` to stdout instead of human-readable output. When `--json` is active, all human-readable output (progress, errors, summaries) goes to stderr or is suppressed. Only the JSON blob goes to stdout.

- [ ] Add `json: bool` field to `TestRunnerConfig` in `runner/mod.rs:43-56` (struct starts at line 43 after `#[expect]` attribute) <!-- reviewed: accuracy fix — struct is at 43, not 38 -->
- [ ] Parse `--json` flag in `main.rs` test command block (lines 118-149, inside the `"test"` match arm):
  ```rust
  } else if arg == "--json" {
      config.json = true;
  }
  ```
- [ ] In `commands/test.rs:run_tests()`, when `config.json` is true:
  - Suppress human-readable output (no `print_test_summary`, no `print_summary_stats`)
  - After `runner.run()` completes, serialize the `TestSummary` (or per-file `FileSummary`) as JSON to stdout
  - For single-file mode: emit one `JsonFileSummary` object
  - For multi-file mode: emit `JsonFileSummary` array (one per file)
  ```rust
  if config.json {
      let interner = runner.interner();
      let json_summaries: Vec<JsonFileSummary> = summary.files
          .iter()
          .map(|f| f.to_json(interner))
          .collect();
      // Frame the JSON with sentinels so the orchestrator can extract it
      // even if Ori print() calls pollute stdout.
      println!("---ORI_JSON_BEGIN---");
      println!("{}", serde_json::to_string(&json_summaries).unwrap());
      println!("---ORI_JSON_END---");
  } else {
      // existing human-readable output
  }
  ```
  <!-- reviewed: feasibility fix — added sentinel framing because ori_print goes to stdout -->

- [ ] **CRITICAL: Redirect Ori `print()` away from stdout in `--json` mode.** Tracing output already goes to stderr (`tracing_setup.rs` uses `stderr()`), but `ori_print` in `ori_rt` uses `println!()` which writes to **stdout**. This WILL corrupt JSON output if any Ori test calls `print()`. Two options: <!-- reviewed: accuracy fix — print() goes to stdout via ori_rt, not stderr -->
  - **(A) Redirect worker stdout**: in `spawn_llvm_worker`, set worker's stdout to piped and stderr to inherit. But if an Ori test calls `print()`, its output goes to the piped stdout alongside the JSON. So the worker must emit JSON as the LAST thing to stdout, and the orchestrator must extract only the last line (or a framed JSON blob).
  - **(B) Emit JSON to a temp file**: worker writes `JsonFileSummary` to a temp file (path passed via env var or CLI arg), leaving stdout free for Ori `print()` output. Orchestrator reads the temp file. This is the Zig sidecar pattern (`Compilation.zig:6338-6343`).
  - **(C) Frame the JSON**: worker emits a unique sentinel line before and after the JSON blob (e.g., `---ORI_JSON_BEGIN---` / `---ORI_JSON_END---`). Orchestrator extracts the JSON between sentinels. Any `print()` output before/after is ignored.
  - **Recommended: (C)** — simplest, no temp file cleanup, robust against any stdout pollution. The orchestrator scans worker stdout for the sentinel-framed JSON block and ignores everything else.
- [ ] Integration test: `ori test --backend=llvm --json tests/spec/types/primitives.ori` produces sentinel-framed JSON with expected pass count. Run via `Command::new` in a Rust test. <!-- reviewed: accuracy fix — basic_types.ori doesn't exist, primitives.ori does -->
- [ ] Integration test: `ori test --backend=llvm --json tests/spec/inference/unification.ori` (a file with many tests) produces sentinel-framed JSON with per-test results.
- [ ] Integration test: stdout pollution resilience — create a test file that calls `print()`, run `ori test --backend=llvm --json <file>`, verify the sentinel-framed JSON block is extracted correctly despite Ori `print()` output on stdout. This is the critical robustness test. <!-- reviewed: feasibility fix — old test assumed stdout purity, which is impossible due to ori_print going to stdout -->

---

## 01.R Third Party Review Findings

- None.

---

## 01.N Completion Checklist

- [ ] `BackendCrash` variant added to `TestOutcome` with `is_backend_crash()` predicate
- [ ] `has_failures()` returns true for `BackendCrash` — crashes block the test gate
- [ ] `backend_crash` counter added to `FileSummary` and `TestSummary`
- [ ] JSON-serializable mirror types (`JsonTestResult`, `JsonTestOutcome`, `JsonFileSummary`) created
- [ ] `FileSummary::to_json()` and `JsonFileSummary::into_file_summary()` conversions work
- [ ] `--json` flag parsed in CLI and routed through `TestRunnerConfig`
- [ ] `ori test --backend=llvm --json <file>` emits sentinel-framed JSON to stdout (robust against Ori `print()` output)
- [ ] Serde round-trip test passes for all outcome variants
- [ ] Integration tests pass for JSON output on real test files
- [ ] `timeout 150 ./test-all.sh` passes — no regressions (JSON flag is opt-in, default behavior unchanged)
- [ ] `./clippy-all.sh` passes
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 01` returns 0 annotations
- [ ] **Plan sync** — update plan metadata
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** `ori test --backend=llvm --json tests/spec/types/primitives.ori` emits a sentinel-framed JSON `JsonFileSummary` to stdout with correct pass/fail/skip counts. The framing is robust against Ori `print()` output on stdout. `BackendCrash` variant exists and is counted as a real failure. All existing tests pass unchanged (JSON is opt-in). Serde round-trip test verifies all 6 `JsonTestOutcome` variants serialize and deserialize correctly. <!-- reviewed: accuracy fix — basic_types.ori doesn't exist; feasibility fix — sentinel framing required -->
