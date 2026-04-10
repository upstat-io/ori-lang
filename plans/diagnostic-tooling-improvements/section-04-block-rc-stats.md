---
section: "04"
title: "Block-level RC Stats"
status: not-started
reviewed: false
goal: "Create a new rc_histogram pass separate from rc_balance.rs that counts ALL RC ops (alloc/inc/dec/free/cow) per basic block, emit typed JSON via serde, and update rc-stats.sh to consume it — with migration safety net comparing awk totals to JSON totals before awk removal"
success_criteria:
  - "New rc_histogram.rs counts ALL 5 RC ops (alloc, inc, dec, free, cow) per basic block — rc_balance.rs untouched"
  - "RcOpKind classifier enum covers all 5 RC operation types as the raw counting vocabulary"
  - "JSON output uses typed Rust structs with serde::Serialize derives and a schema_version field"
  - "JSON emitted to stderr with 'codegen stats: json:' prefix (NOT 'codegen audit:' — avoids breaking codegen-audit.sh)"
  - "rc-stats.sh --block-level renders per-block table from compiler JSON"
  - "rc-stats.sh default mode migrated from awk to compiler JSON only AFTER migration tests confirm numeric equivalence"
  - "Per-block exit code is informational (exit 0) — only function-level imbalance triggers exit 1 (matching current behavior)"
  - "rc-stats.sh --optimized works via compiler JSON (post-optimization histogram) — awk parser completely removed"
  - "No LEAK:scattered-knowledge in ANY mode — all RC stats come from the compiler via JSON, awk parser deleted"
  - "Function names in JSON output are demangled (matching current awk output format)"
  - "Unnamed basic blocks have fallback labels (bb_0, bb_1, ...) in JSON output"
inspired_by:
  - "Swift ARC optimizer — tracks retain/release per SIL basic block"
  - "Lean 4 IR RC — per-block inc/dec analysis"
depends_on: []
third_party_review:
  status: resolved
  updated: 2026-04-09
sections:
  - id: "04.1"
    title: "Create RcOpKind enum and rc_histogram.rs counting pass"
    status: not-started
  - id: "04.2"
    title: "Typed JSON schema structs with serde"
    status: not-started
  - id: "04.3"
    title: "Wire histogram into audit pipeline and emit JSON"
    status: not-started
  - id: "04.4"
    title: "Update rc-stats.sh with --block-level and JSON migration"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Block-level RC Stats

**Status:** Not Started
**Goal:** Give developers the ability to localize RC leaks/over-releases to specific basic blocks within a function, not just the function as a whole. Currently `rc-stats.sh` reports per-function totals — "this function has +2 balance" — but cannot tell you WHICH loop or branch is responsible. The fix creates a **new** raw-counting pass (`rc_histogram.rs`) that is architecturally separate from the existing semantic lifecycle verifier (`rc_balance.rs`), emits typed JSON via serde, and updates the shell script to render it.

**Critical architectural constraint (from dual-source review):** `rc_balance.rs` is a **semantic lifecycle state machine** tracking pointer ownership transitions (Live/CowConsumed/Decremented). It deliberately tracks only `ori_rc_alloc`, `ori_rc_dec`, and COW calls — this is correct for its purpose. The new per-block counting is a **syntactic histogram** — it counts ALL 5 RC op types (`ori_rc_alloc`, `ori_rc_inc`, `ori_rc_dec`, `ori_rc_free`, COW) without tracking pointer identity or state transitions. **These MUST be separate modules.** Mixing histogram counting into the lifecycle tracker would corrupt the state machine's invariants. The awk parser in `rc-stats.sh` already tracks all 5 ops — the new pass must match.

**Success Criteria:**
- [ ] `rc_histogram.rs` counts all 5 RC ops per basic block per function, independent of `rc_balance.rs`
- [ ] `RcOpKind` enum in `verify/` classifies `Alloc | Inc | Dec | Free | Cow` as the raw counting vocabulary
- [ ] JSON schema struct `RcStatsReport` with `schema_version: u32` field, `serde::Serialize` derives, defined in `verify/rc_stats.rs`
- [ ] `ORI_AUDIT_CODEGEN=1` emits per-block JSON to stderr with `codegen stats: json:` prefix (separate from `codegen audit:` lines)
- [ ] `rc-stats.sh --block-level file.ori` renders a per-block table with balance per block
- [ ] Per-block imbalance is a localization aid — exit code remains 0. Only function-level imbalance triggers exit 1 (current behavior preserved)
- [ ] `rc-stats.sh` (no flag) backward compatible: identical output and exit behavior to today
- [ ] `rc-stats.sh --optimized` continues working via awk parser (compiler audit runs in codegen pipeline; it only sees unoptimized IR)
- [ ] Migration test matrix confirms awk totals match JSON totals before awk removal
- [ ] Satisfies mission criterion: "ORI_AUDIT_CODEGEN=1 emits per-block structured JSON; rc-stats.sh --block-level consumes it"

**Context:** Both Codex and Gemini independently identified critical issues with the original plan:
1. `rc_balance.rs` does NOT track `ori_rc_inc` or `ori_rc_free` — merging counting into it would silently corrupt the balance equation `(alloc + inc) - (dec + free)`.
2. The `codegen audit:` prefix is consumed by `codegen-audit.sh` via grep — adding JSON lines with that prefix would break it.
3. Per-block exit code 1 would produce false positives because RC ownership commonly crosses basic-block boundaries.
4. The `--optimized` flag path has no compiler-side equivalent — the audit pass sees only unoptimized IR.
5. Removing the awk parser without a migration safety net risks silent numeric divergence.

**Reference implementations:**
- **Swift** `ARC optimizer`: tracks retain/release per SIL basic block
- **Lean 4** `IR/RC.lean`: per-block inc/dec analysis in the RC insertion pass

**Depends on:** None.

---

## 04.1 Create RcOpKind enum and rc_histogram.rs counting pass

**File(s):**
- `compiler/ori_llvm/src/verify/rc_histogram.rs` (NEW — the raw counting pass)
- `compiler/ori_llvm/src/verify/mod.rs` (add `mod rc_histogram;` and wire into `audit_module_with_options`)

**Why a new file, not modifying rc_balance.rs:** `rc_balance.rs` is a semantic lifecycle verifier — it tracks pointer identity and state transitions (Live → CowConsumed → Decremented). The histogram is a syntactic counter — it counts instruction occurrences without tracking pointer identity. These are fundamentally different concerns. Mixing them would create a LEAK:phase-bleeding (histogram counting polluting the state machine) and risk corrupting `rc_balance.rs`'s invariants. The `RcOpKind` enum provides a shared vocabulary without coupling the implementations.

- [ ] Create `compiler/ori_llvm/src/verify/rc_histogram.rs` containing:
  - `RcOpKind` enum: `Alloc`, `Inc`, `Dec`, `Free`, `Cow` — with `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash` derives
  - `fn classify_rc_call(callee_name: &str) -> Option<RcOpKind>` — maps runtime RC functions to operation kinds. Must cover ALL RC operations emitted by the codegen, not just the 5 base names:
    - `Alloc`: `ori_rc_alloc`
    - `Inc`: `ori_rc_inc`, `ori_str_rc_inc`, `ori_list_rc_inc` (slice-aware typed RC inc)
    - `Dec`: `ori_rc_dec`, `ori_str_rc_dec`, `ori_buffer_rc_dec`, `ori_map_buffer_rc_dec`, `ori_set_buffer_rc_dec` (slice-aware typed RC dec)
    - `Free`: `ori_rc_free`
    - `Cow`: COW functions (via `super::is_cow_function`) — `ori_list_*_cow`, `ori_str_*_cow`, etc.
    - **Source of truth for the function list:** `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`. The classifier must cover all RC counting operations (functions that perform alloc/inc/dec/free on refcounts or COW mutations). NOT all functions containing `rc` — non-counting helpers (`ori_rc_live_count`, `ori_rc_reset_live_count`, `ori_rc_is_unique`, `ori_rc_is_unique_or_null`, `ori_rc_realloc`) must return `None` from `classify_rc_call`. Add an exhaustiveness test with an explicit allowlist of classified patterns (`*_rc_inc`, `*_rc_dec`, exact `ori_rc_alloc`, exact `ori_rc_free`, `*_cow`) and explicit `None` assertions for `ori_rc_is_unique`, `ori_rc_live_count`, `ori_rc_realloc`, and unrelated names like `ori_str_to_uppercase`.
    - **Note:** The current awk parser in `rc-stats.sh` only matches the 5 base patterns (`ori_rc_alloc/inc/dec/free` + COW). The new classifier intentionally EXPANDS coverage to typed RC operations that the awk parser missed (e.g., `ori_buffer_rc_dec`, `ori_str_rc_inc`). The migration test matrix (Phase B) must account for this — the old awk totals may be LOWER than JSON totals for files with typed RC calls.
  - `pub(super) struct BlockHistogram { pub label: String, pub counts: [u32; 5] }` (indexed by `RcOpKind` discriminant) — `pub(super)` so sibling module `rc_stats.rs` can read it
  - `pub(super) struct FunctionHistogram { pub name: String, pub blocks: Vec<BlockHistogram> }` — same visibility rationale
  - `pub fn collect_module_histogram(module: &Module<'_>, options: &AuditOptions) -> Vec<FunctionHistogram>` — walks all functions/blocks, calls `classify_rc_call` on every call/invoke instruction, accumulates counts per block
  - Uses `super::callee_name()` (shared with rc_balance) to extract callee names
  - Uses `super::rc_balance::should_audit_fn()` for function filtering (already `pub(super)`)
  - **Function name demangling:** The histogram MUST demangle Ori function names before storing them in `FunctionHistogram.name`, matching the EXACT format the current awk parser produces (rc-stats.sh lines 132-141). Add a `fn demangle_ori_name(llvm_name: &str) -> String` helper in `rc_histogram.rs` that applies these rules:
    - `_ori_drop$X` → `drop<X>` (e.g., `_ori_drop$Point` → `drop<Point>`)
    - `_ori_X$$Y$Z` → `@X impl Y.Z` (e.g., `_ori_Point$$Eq$equals` → `@Point impl Eq.equals`)
    - `_ori_X$Y` → `@X.Y` (e.g., `_ori_math$add` → `@math.add`)
    - `_ori_X` → `@X` (e.g., `_ori_main` → `@main`)
    - Non-`_ori_` names pass through unchanged
    - **Do NOT reuse the existing AOT demangler** (`aot/mangle/parse.rs`) — it produces a different format (`math.@add`, `int::Eq.@equals`) that does NOT match rc-stats.sh's output contract. A dedicated helper with the awk format is needed.
    - Add tests verifying output matches the awk format exactly for: module functions, trait impls, drop functions, and non-ori names
  - For block labels: use `BasicBlock::get_name()` if non-empty; otherwise generate `format!("bb_{}", block_index)` as a fallback. Many LLVM basic blocks are unnamed (empty `get_name()`) — the fallback ensures every block has a printable identifier in the JSON output and the rc-stats.sh table
- [ ] Add `mod rc_histogram;` to `compiler/ori_llvm/src/verify/mod.rs`
- [ ] **File size check:** `rc_histogram.rs` should be under 200 lines. The counting logic is simple — no state machine, just instruction classification and accumulation.
- [ ] Add Rust unit tests in `compiler/ori_llvm/src/verify/rc_histogram/tests.rs`:
  - `test_classify_rc_call_alloc_returns_alloc` — `"ori_rc_alloc"` → `Some(RcOpKind::Alloc)`
  - `test_classify_rc_call_inc_returns_inc` — `"ori_rc_inc"` → `Some(RcOpKind::Inc)`
  - `test_classify_rc_call_dec_returns_dec` — `"ori_rc_dec"` → `Some(RcOpKind::Dec)`
  - `test_classify_rc_call_free_returns_free` — `"ori_rc_free"` → `Some(RcOpKind::Free)`
  - `test_classify_rc_call_cow_function_returns_cow` — `"ori_list_push_cow"` → `Some(RcOpKind::Cow)`
  - `test_classify_rc_call_unrelated_returns_none` — `"puts"` → `None`
  - `test_empty_module_produces_empty_histogram` — synthetic inkwell module with no functions → empty vec
  - `test_module_with_rc_alloc_and_dec_counts_per_block` — synthetic module with `ori_rc_alloc` in entry block and `ori_rc_dec` in exit block → correct per-block counts
  - Test naming follows `<subject>_<scenario>_<expected>` (CLAUDE.md §Test function naming). No ephemeral identifiers.
- [ ] Run `timeout 150 cargo t -p ori_llvm -- rc_histogram` to verify tests pass

**CRITICAL: `rc_balance.rs` is NOT modified in this section.** The lifecycle verifier continues to track only alloc/dec/cow as before. If future work wants the lifecycle verifier to also track inc/free, that is a separate change with its own state-machine analysis.

- [ ] **Subsection close-out (04.1)** — MANDATORY before starting 04.2:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 04.2 Typed JSON schema structs with serde

**File(s):**
- `compiler/ori_llvm/src/verify/rc_stats.rs` (NEW — typed JSON schema)
- `compiler/ori_llvm/Cargo.toml` (add `serde_json` dependency)

**Why typed structs, not format!():** `ori_llvm/Cargo.toml` already depends on `serde = { version = "1", features = ["derive"] }` (line 26). Adding `serde_json` for serialization ensures correct JSON escaping, stable field ordering, and compile-time schema enforcement. Manual `format!()` JSON emission is a LEAK/EXPOSURE risk — escape handling for function names containing quotes/backslashes, format drift between emitter and consumer, no compile-time schema contract.

**Why a schema_version field:** The JSON schema will be consumed by `rc-stats.sh` and potentially other tools. A version field enables backward-compatible schema evolution without breaking consumers. Version 1 is the initial schema defined here.

- [ ] Add `serde_json = "1"` to `[dependencies]` in `compiler/ori_llvm/Cargo.toml` (serde is already present)
- [ ] Create `compiler/ori_llvm/src/verify/rc_stats.rs` containing typed structs:
  ```rust
  use serde::Serialize;

  /// Schema version for backward compatibility. Bump when adding fields.
  pub const SCHEMA_VERSION: u32 = 1;

  /// Top-level RC stats report emitted as JSON.
  #[derive(Debug, Clone, Serialize)]
  pub struct RcStatsReport {
      pub schema_version: u32,
      /// Whether this report covers optimized or unoptimized IR.
      pub optimized: bool,
      pub functions: Vec<FunctionStats>,
  }

  /// Per-function RC operation stats.
  #[derive(Debug, Clone, Serialize)]
  pub struct FunctionStats {
      pub name: String,
      pub blocks: Vec<BlockStats>,
      /// Function-level totals (sum of all blocks).
      pub totals: OpCounts,
  }

  /// Per-basic-block RC operation counts.
  #[derive(Debug, Clone, Serialize)]
  pub struct BlockStats {
      pub label: String,
      pub counts: OpCounts,
  }

  /// Raw RC operation counts.
  #[derive(Debug, Clone, Default, Serialize)]
  pub struct OpCounts {
      pub alloc: u32,
      pub inc: u32,
      pub dec: u32,
      pub free: u32,
      pub cow: u32,
  }
  ```
- [ ] Add `pub fn from_histograms(histograms: &[FunctionHistogram], optimized: bool) -> RcStatsReport` conversion that maps `FunctionHistogram` → `FunctionStats` with computed totals and sets `optimized` field accordingly
- [ ] Add `mod rc_stats;` to `compiler/ori_llvm/src/verify/mod.rs`
- [ ] **File size check:** `rc_stats.rs` should be under 100 lines — it is pure data types plus one conversion function.
- [ ] Add Rust unit tests in `compiler/ori_llvm/src/verify/rc_stats/tests.rs`:
  - `test_empty_histogram_produces_version_one_empty_functions` — empty input → `RcStatsReport { schema_version: 1, functions: [] }`
  - `test_histogram_to_report_computes_function_totals` — two blocks with known counts → totals are sums
  - `test_report_serializes_to_valid_json` — `serde_json::to_string(&report)` succeeds and contains `"schema_version":1`
  - `test_function_name_with_special_chars_serializes_correctly` — function name containing `"` and `\` serializes without corruption (proves serde handles escaping)
- [ ] Run `timeout 150 cargo t -p ori_llvm -- rc_stats` to verify tests pass

- [ ] **Subsection close-out (04.2)** — MANDATORY before starting 04.3:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 04.3 Wire histogram into audit pipeline and emit JSON

**File(s):**
- `compiler/ori_llvm/src/verify/mod.rs` (call histogram, emit JSON)
- `compiler/ori_llvm/src/verify/report.rs` (add `emit_rc_stats_json` method to `AuditReport` or as a standalone function)

**Prefix choice: `codegen stats: json:` NOT `codegen audit:`:** The existing `codegen-audit.sh` script (line 163) extracts lines via `grep "^codegen audit:"`. Adding a JSON line with that same prefix would cause `codegen-audit.sh` to parse it as a garbled audit finding. Using the distinct prefix `codegen stats: json:` cleanly separates the two output streams. `rc-stats.sh` will grep for `^codegen stats: json:` to extract its data.

- [ ] In `audit_module_with_options()` (`verify/mod.rs`), after the existing checks, call `rc_histogram::collect_module_histogram(module, options)` and convert to `RcStatsReport` via `rc_stats::RcStatsReport::from_histograms()`
- [ ] Store the `RcStatsReport` in `AuditReport` (add a field: `pub rc_stats: Option<rc_stats::RcStatsReport>`) or return it alongside the report
- [ ] **Optimized-IR histogram support (SSOT — eliminates awk parser entirely):** Add a `pub fn audit_module_histogram_only(module: &Module<'_>, options: &AuditOptions) -> RcStatsReport` entry point that runs ONLY the histogram pass (no lifecycle/COW/ABI/safety checks), calling `from_histograms(..., optimized: true)`. This enables callers to collect stats on the post-optimization module without running the full audit pass.
  - **Hook points (MUST be gated behind `if verify::audit_requested()`):** The histogram emission for optimized IR must be wrapped in an `if verify::audit_requested() { ... }` block — without this gate, every normal `ori build --release` would run the histogram and spam stderr. The specific hook points are:
    - `compiler/ori_llvm/src/aot/object.rs` in `verify_optimize_emit()` — after `run_optimization_passes()` completes but before object emission. This covers normal AOT object builds.
    - `compiler/oric/src/commands/build/single.rs` — after `optimize_module()` for `--emit=llvm-ir` builds.
  - Both paths emit a JSON line: `codegen stats: json: {..."optimized":true...}`
  - **Smoke test:** `ORI_AUDIT_CODEGEN=1 cargo run -p oric --bin ori -- build --release diagnostics/fixtures/clean.ori -o /tmp/test_bin 2>&1 | grep "codegen stats: json:"` must produce TWO JSON lines: one with `"optimized":false` and one with `"optimized":true`.
  - **Non-audit builds must NOT run the histogram:** `ori build --release diagnostics/fixtures/clean.ori` (without `ORI_AUDIT_CODEGEN=1`) must produce zero `codegen stats:` lines on stderr.
- [ ] In the `emit_to_stderr()` method of `AuditReport` (or a new `emit_rc_stats_json()` function), after the existing text output, emit the JSON:
  ```rust
  if let Some(ref stats) = self.rc_stats {
      if let Ok(json) = serde_json::to_string(stats) {
          eprintln!("codegen stats: json: {json}");
      }
  }
  ```
- [ ] **Verify `codegen-audit.sh` is unaffected:** The new line starts with `codegen stats:` not `codegen audit:` — the grep at line 163 of `codegen-audit.sh` (`grep "^codegen audit:"`) will not match it. Add a comment in the emitter noting this invariant.
- [ ] Add Rust unit tests (in `verify/tests.rs` or a dedicated test):
  - `test_audit_module_populates_rc_stats` — synthetic module with RC calls → `report.rc_stats` is `Some` with correct counts
  - `test_audit_empty_module_rc_stats_has_no_functions` — empty module → `rc_stats.functions` is empty, `schema_version` is 1
  - `test_rc_stats_json_prefix_does_not_match_codegen_audit_grep` — the output string starts with `"codegen stats: json:"` (proves it does not start with `"codegen audit:"`)
- [ ] **File size check:** `verify/mod.rs` is currently 135 lines. Adding the histogram call and JSON emission should keep it under 200 lines. If it exceeds, extract the JSON emission into `rc_stats.rs`.
- [ ] Run `timeout 150 cargo t -p ori_llvm` to verify no regressions
- [ ] Smoke test: `ORI_AUDIT_CODEGEN=1 cargo run -p oric --bin ori -- build diagnostics/fixtures/clean.ori -o /tmp/test_bin 2>&1 | grep "codegen stats: json:"` — should produce one JSON line

- [ ] **Subsection close-out (04.3)** — MANDATORY before starting 04.4:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 04.4 Update rc-stats.sh with --block-level and JSON migration

**File(s):** `diagnostics/rc-stats.sh`, `diagnostics/self-test.sh`

This subsection has three phases: (A) add `--block-level` using compiler JSON, (B) migration test matrix comparing awk vs JSON totals, (C) migrate default mode from awk to JSON and retain awk only for `--optimized`.

### Phase A: Add --block-level flag

- [ ] Add `--block-level` flag to `rc-stats.sh` argument parser
- [ ] When `--block-level` is passed (without `--optimized`):
  1. Compile with `ORI_AUDIT_CODEGEN=1`, capture stderr (do NOT fail on nonzero compiler exit — audit findings cause nonzero exit but stats JSON is still emitted to stderr before the exit code is set)
  2. Extract the `codegen stats: json:` line from stderr, strip the prefix. If no stats JSON line is found AND the compiler exit was nonzero, THEN exit 2 ("compilation failed before stats pass"). If stats JSON IS found, proceed regardless of compiler exit code.
  3. Parse JSON with `python3 -c 'import sys,json; ...'` (available on all target platforms; `jq` is optional)
  4. Render a hierarchical table: `Function > Block > alloc/inc/dec/free/cow/balance`
  5. Per-block balance shown for localization but does NOT affect exit code (RC ownership crosses block boundaries — a per-block imbalance is normal control flow, not a bug)
  6. Function-level balance (sum of all blocks) determines exit code: 0 = all functions balanced, 1 = any function imbalanced (matches current behavior exactly)
  7. **Audit-error resilience:** The stats pass runs before `has_errors()` triggers nonzero exit, so stats JSON is available even when audit findings exist. This makes `rc-stats.sh` useful on exactly the files developers need it for — files with RC issues.
- [ ] `--block-level --optimized` is supported: uses the optimized JSON (`"optimized": true`) to render per-block stats for the post-optimization IR. Both flags compose naturally since both modes consume compiler JSON.
- [ ] Add self-test entries:
  - `rc-stats.sh --block-level fixtures/clean.ori` produces per-block output containing block labels
  - `rc-stats.sh --block-level --optimized fixtures/clean.ori` produces per-block output from optimized IR
  - `rc-stats.sh --optimized fixtures/clean.ori` produces function-level output from optimized IR

### Phase B: Migration test matrix (awk vs JSON numeric equivalence)

**Why:** The awk parser counts RC ops by regex-matching LLVM IR text. The new histogram pass counts by walking inkwell's in-memory IR. These MUST agree before we remove the awk parser. Subtle differences (e.g., the awk parser counting `invoke` calls that the histogram misses, or the histogram counting inlined COW calls the awk regex doesn't match) would silently corrupt rc-stats.sh output.

- [ ] Create a temporary `--compare-awk` flag (or internal validation mode) that runs BOTH the awk parser (on IR text) and the JSON parser (from compiler output) on the same file, then compares per-function totals
- [ ] Test the comparison across multiple fixture files:
  - `diagnostics/fixtures/simple.ori` — minimal program, few RC ops
  - `diagnostics/fixtures/clean.ori` — program with RC operations
  - `diagnostics/fixtures/closure-capture.ori` (if exists from Section 06, or create a minimal one) — closures exercise inc/dec paths
  - At least one program with `ori_rc_free` calls (COW or drop path)
- [ ] For the 5 base RC operations (`ori_rc_alloc/inc/dec/free` + COW), awk and JSON totals should match. For typed RC operations (`ori_str_rc_inc`, `ori_buffer_rc_dec`, etc.), JSON totals will be HIGHER because the new classifier covers operations the awk parser never counted — this is correct and expected, not a regression.
- [ ] Document any divergence: if awk counts X and JSON counts X+N for a fixture, the N must be accounted for by typed RC operations visible in `ir-dump.sh --raw` output for that fixture.
- [ ] If any UNEXPECTED divergence is found (same operation counted differently): investigate and fix the histogram pass before proceeding to Phase C.

### Phase C: Migrate ALL modes to JSON, remove awk parser entirely

- [ ] In default mode (no `--block-level`, no `--optimized`): replace the awk parser with JSON consumption — compile with `ORI_AUDIT_CODEGEN=1`, extract the `codegen stats: json:` line where `"optimized": false`, compute per-function totals from the JSON, render the same table format as today
- [ ] In `--optimized` mode: compile with `ORI_AUDIT_CODEGEN=1`, extract the `codegen stats: json:` line where `"optimized": true` (emitted after the LLVM pass manager runs, per 04.3's optimized-IR histogram support). Render the same table format. No awk needed.
- [ ] **Remove the awk parser entirely.** Both `--optimized` and default modes now consume compiler JSON. The awk code (lines 116-208 of the current rc-stats.sh) is deleted. This eliminates the LEAK:scattered-knowledge — there is now exactly ONE source of truth for RC operation classification (`RcOpKind` in `rc_histogram.rs`), consumed by all modes via JSON.
- [ ] Verify output parity: `rc-stats.sh fixtures/clean.ori` produces identical output before and after migration (compare exact text, ignoring ANSI color codes). Note: For fixtures with typed RC ops (e.g., `ori_buffer_rc_dec`), the JSON output will show HIGHER totals than the old awk output because the classifier covers more operations — this is CORRECT behavior, not a regression.
- [ ] Verify exit code parity: any fixture that previously triggered exit 1 still triggers exit 1
- [ ] Add/update self-test entries:
  - `rc-stats.sh fixtures/clean.ori` (default mode) — works via JSON
  - `rc-stats.sh --optimized fixtures/clean.ori` — works via optimized JSON
  - `rc-stats.sh --block-level fixtures/clean.ori` — works via JSON
- [ ] Verify: `diagnostics/self-test.sh` passes

- [ ] **Subsection close-out (04.4)** — MANDATORY before starting 04.R:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 04.R Third Party Review Findings

- [x] `[TPR-04-001-codex][high]` `section-04-block-rc-stats.md:89` — Expand RcOpKind beyond the legacy awk baseline to cover typed RC ops (ori_str_rc_inc, ori_buffer_rc_dec, etc.).
  Resolved: Fixed on 2026-04-09. Expanded classifier spec to cover all runtime RC functions from runtime_functions.rs. Added exhaustiveness test requirement. Updated migration matrix to expect higher JSON totals for typed RC ops.
- [x] `[TPR-04-002-codex][medium]` `section-04-block-rc-stats.md:90` — Expose histogram data across verify submodules with pub(super) visibility.
  Resolved: Fixed on 2026-04-09. Made BlockHistogram and FunctionHistogram pub(super) with pub fields.
- [x] `[TPR-04-003-codex][medium]` `section-04-block-rc-stats.md:257` — Preserve stats output when audit errors abort codegen.
  Resolved: Fixed on 2026-04-09. Updated Phase A to capture stderr regardless of exit code, extract JSON even on nonzero exit, and reserve exit 2 only for missing JSON.
- [x] `[TPR-04-001-gemini][high]` `section-04-block-rc-stats.md:129` — Eliminate awk parser completely by supporting optimized-IR histogram in the compiler.
  Resolved: Fixed on 2026-04-09. Added audit_module_histogram_only entry point for post-optimization module, added optimized field to schema, updated Phase C to remove awk entirely, enabled --block-level --optimized composition.
- [x] `[TPR-04-002-gemini][high]` `section-04-block-rc-stats.md:128` — Preserve function name demangling in JSON output path.
  Resolved: Fixed on 2026-04-09. Added demangling requirement to rc_histogram.rs with explicit rules matching current awk demangling logic. Added test examples.
- [x] `[TPR-04-003-gemini][medium]` `section-04-block-rc-stats.md:73` — Handle unnamed BasicBlocks with fallback labels.
  Resolved: Fixed on 2026-04-09. Added fallback label generation (bb_0, bb_1, ...) for blocks where get_name() returns empty.
- [x] `[TPR-04-001-codex][medium]` (iter 2) `section-04-block-rc-stats.md:97` — Restrict RcOpKind exhaustiveness test to counting operations only.
  Resolved: Fixed on 2026-04-09. Replaced broad `*rc*` grep with explicit allowlist of counting patterns; added None assertions for non-counting helpers.
- [x] `[TPR-04-002-codex][high]` (iter 2) `section-04-block-rc-stats.md:185` — Pass optimized flag into from_histograms conversion.
  Resolved: Fixed on 2026-04-09. Updated function signature to `from_histograms(histograms, optimized: bool)`.
- [x] `[TPR-04-003-codex][high]` (iter 2) `section-04-block-rc-stats.md:212` — Anchor optimized histogram at specific AOT hook points.
  Resolved: Fixed on 2026-04-09. Named exact hook points (object.rs verify_optimize_emit, build/single.rs optimize_module). Added smoke test requiring both JSON lines.
- [x] `[TPR-04-004-codex][medium]` (iter 2) `section-04-block-rc-stats.md:106` — Remove unsafe existing-demangler reuse option.
  Resolved: Fixed on 2026-04-09. Deleted option (b), added explicit "do NOT reuse AOT demangler" note with format comparison.
- [x] `[TPR-04-001-gemini][high]` (iter 2) `section-04-block-rc-stats.md:212` — Gate optimized histogram behind audit_requested check.
  Resolved: Fixed on 2026-04-09. Added explicit audit_requested() gate at both hook points; added non-audit negative test.

---

## 04.N Completion Checklist

- [ ] All subsections (04.1, 04.2, 04.3, 04.4) complete
- [ ] `rc_histogram.rs` exists as a separate module from `rc_balance.rs` — no modifications to `rc_balance.rs`
- [ ] `RcOpKind` classifies all 5 RC ops (alloc, inc, dec, free, cow)
- [ ] JSON schema structs use `serde::Serialize` with `schema_version: u32` field
- [ ] JSON prefix is `codegen stats: json:` (NOT `codegen audit:`) — `codegen-audit.sh` unaffected
- [ ] Per-block exit code is 0 (localization aid only); function-level exit code matches current behavior
- [ ] `--optimized` mode works via compiler JSON (post-optimization histogram), awk parser completely removed
- [ ] Migration test matrix confirmed awk/JSON numeric equivalence before awk removal from default path
- [ ] `timeout 150 cargo t -p ori_llvm` passes
- [ ] `diagnostics/self-test.sh` passes
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] **Annotation cleanup:** remove any plan-annotation comments (e.g., `// Section 04`, `// 04.1`) from source files — plan annotations are temporary scaffolding per CLAUDE.md
- [ ] **`/improve-tooling` section-close sweep**
