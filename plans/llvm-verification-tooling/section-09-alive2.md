---
section: "09"
title: "Alive2 Formal Verification"
status: not-started
reviewed: false
goal: "Integrate Alive2's alive-tv translation validator to formally verify that LLVM optimization passes preserve the semantics of Ori's emitted IR — running a curated subset of pure/arithmetic functions on every CI nightly build and a full sweep weekly"
success_criteria:
  - "alive-tv binary is built from source and available in CI via cached artifact"
  - "diagnostics/alive2-verify.sh script verifies pre-opt vs post-opt IR refinement for a given .ori file"
  - "Curated test corpus of ≥15 pure/arithmetic-heavy Ori functions passes alive-tv refinement checking"
  - "False positive suppression list handles known-benign counterexamples (runtime calls, RC ops)"
  - "Nightly CI job runs alive-tv on the curated corpus with zero unresolved refinement failures"
  - "Weekly CI job runs alive-tv on all compiler/ori_llvm/tests/codegen/ IR with timeout per function"
inspired_by:
  - "Alive2 alive-tv (~/projects/reference_repos/verification_tools/alive2/tools/alive-tv.cpp) — standalone translation validation comparing two LLVM IR files"
  - "Alive2 opt plugin (~/projects/reference_repos/verification_tools/alive2/tv/tv.cpp) — per-pass translation validation integrated into LLVM opt pipeline"
  - "Rust LLVM CI (rust-lang/rust .github/workflows/) — nightly verification jobs separate from PR CI"
depends_on: ["07"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "09.1"
    title: "Build Alive2 and Z3 Dependencies"
    status: not-started
  - id: "09.2"
    title: "IR Capture Pipeline for Pre-Opt/Post-Opt Pairs"
    status: not-started
  - id: "09.3"
    title: "Diagnostic Script and Function Selection"
    status: not-started
  - id: "09.4"
    title: "False Positive Management"
    status: not-started
  - id: "09.5"
    title: "CI Integration (Nightly and Weekly)"
    status: not-started
  - id: "09.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "09.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 09: Alive2 Formal Verification

**Status:** Not Started
**Goal:** Integrate Alive2's `alive-tv` translation validator to formally verify that LLVM optimization passes preserve the semantics of Ori's emitted IR. Unlike behavioral testing (which checks "correct output for specific inputs"), Alive2 provides mathematical proof that the optimized IR is a valid refinement of the unoptimized IR — catching miscompiles that no finite set of test inputs can find. The integration targets a curated subset of pure/arithmetic functions (where Alive2 excels) and explicitly excludes RC operations and runtime calls (where Alive2's limitations produce false positives).

**Success Criteria:**

- [ ] `alive-tv` binary available in CI — satisfies mission criterion: "Alive2 refinement checking curated subset"
- [ ] ≥15 curated pure/arithmetic functions pass alive-tv — satisfies mission criterion: "nightly alive-tv green"
- [ ] False positive suppression prevents spurious failures — satisfies mission criterion: "zero unresolved failures"
- [ ] Nightly CI runs curated corpus; weekly CI runs full `compiler/ori_llvm/tests/codegen/` — satisfies mission criterion: "CI fully integrated"

**Context:** Alive2 is the standard tool for formal verification of LLVM transformations. It uses the Z3 SMT solver to prove that for ALL possible inputs, the post-optimization IR produces the same observable behavior as the pre-optimization IR (or strictly more defined behavior — "refinement"). This is strictly stronger than testing: a test checks one input, Alive2 proves all inputs. However, Alive2 has significant limitations: it does not support inter-procedural transformations, loops are unrolled to a configurable bound (~256 max), exception handling (`invoke`/`landingpad`) is not modeled, function calls are approximated conservatively, and memory operations can produce false positives. For Ori, this means Alive2 is best applied to pure/arithmetic functions — NOT to RC operations, runtime calls, or programs with complex control flow. The curated subset is guided by the FileCheck test corpus from Section 07, which identifies which functions have clean, self-contained LLVM IR.

**Reference implementations:**
- **Alive2** `tools/alive-tv.cpp`: Standalone translation validator. Takes two LLVM IR files (or one file with `-passes=` flag to optimize), compares function pairs for refinement. Uses Z3 for SMT solving. Supports `--src-fn`/`--tgt-fn` for per-function verification.
- **Alive2** `tv/tv.cpp`: LLVM opt plugin that intercepts every optimization pass and verifies each transform in-flight. Higher coverage but much slower — suitable for weekly sweeps.
- **Alive2** `README.md`: "Alive2 does not support inter-procedural transformations. Alive2 may produce spurious counterexamples if run with such passes."

**Depends on:** Section 07 (FileCheck test corpus provides the input selection guide — functions with clean IR patterns are the best candidates for Alive2 verification).

---

## 09.1 Build Alive2 and Z3 Dependencies

**File(s):** `scripts/build-alive2.sh`, `.github/workflows/ci.yml` (nightly job dependency)

Alive2 requires Z3 (SMT solver), re2c (lexer generator), cmake, and an LLVM build with RTTI enabled. The build must be reproducible and cacheable for CI.

- [ ] Create `scripts/build-alive2.sh` that:
  - Checks for Z3 installation (headers + library): `pkg-config --exists z3` or checks `/usr/include/z3.h`
  - Checks for re2c: `which re2c`
  - Clones or updates Alive2 from `~/projects/reference_repos/verification_tools/alive2/` (local) or GitHub (CI)
  - Builds Alive2 with cmake+ninja against the system LLVM 21:
    ```bash
    cmake -GNinja \
      -DCMAKE_BUILD_TYPE=Release \
      -DCMAKE_PREFIX_PATH=/usr/lib/llvm-21 \
      -DZ3_INCLUDE_DIR=/usr/include \
      -DZ3_LIBRARIES=/usr/lib/x86_64-linux-gnu/libz3.so \
      ..
    ninja alive-tv
    ```
  - Outputs `alive-tv` binary path to stdout for downstream consumption
  - Supports `--cached` flag that skips rebuild if `alive-tv` binary exists and is newer than source

- [ ] Document Z3 installation requirements in the script's `--help` output:
  ```
  # Ubuntu/Debian: sudo apt-get install libz3-dev re2c
  # macOS: brew install z3 re2c
  ```

- [ ] Add CI caching for the `alive-tv` binary. Since Alive2 builds against a specific LLVM version, the cache key must include the LLVM version:
  ```yaml
  - uses: actions/cache@v4
    with:
      path: build/alive2/alive-tv
      key: alive2-llvm21-${{ hashFiles('scripts/build-alive2.sh') }}
  ```

- [ ] Verify the built `alive-tv` works by running it on a trivial identity function:
  ```bash
  echo 'define i64 @f(i64 %x) { ret i64 %x }' > /tmp/id.ll
  ./alive-tv /tmp/id.ll --passes=instcombine
  # Should print: "Transformation seems to be correct!"
  ```

- [ ] **Subsection close-out (09.1)** — MANDATORY before starting 09.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect on the debugging journey for 09.1 specifically: which build errors were hit, what was confusing about the cmake/Z3 setup, where the script could be more helpful. Implement every accepted improvement NOW and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(scripts): ...`).
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 09.2 IR Capture Pipeline for Pre-Opt/Post-Opt Pairs

**File(s):** `compiler/ori_llvm/src/aot/passes/mod.rs`, `compiler/oric/src/commands/build/mod.rs`

Alive2's `alive-tv` needs two IR files: the pre-optimization LLVM IR and the post-optimization LLVM IR. Ori must capture these at the right pipeline boundary — after the ARC emitter produces LLVM IR but before and after the LLVM optimization pipeline runs.

- [ ] Add `ORI_DUMP_PREOPT_LLVM=1` env var (register in `debug_flags.rs`) that serializes the LLVM module to a `.preopt.ll` file immediately before `run_optimization_passes()` is called:
  ```rust
  if std::env::var("ORI_DUMP_PREOPT_LLVM").is_ok() {
      let preopt_path = output_path.with_extension("preopt.ll");
      module.print_to_file(&preopt_path)
          .map_err(|e| CodegenError::IrDumpFailed(e.to_string()))?;
  }
  ```

- [ ] Add `ORI_DUMP_POSTOPT_LLVM=1` env var that serializes the module after optimization passes complete but before object code emission. This is distinct from `ORI_DUMP_AFTER_LLVM=1` (which dumps during emission, not before optimization):
  ```rust
  if std::env::var("ORI_DUMP_POSTOPT_LLVM").is_ok() {
      let postopt_path = output_path.with_extension("postopt.ll");
      module.print_to_file(&postopt_path)
          .map_err(|e| CodegenError::IrDumpFailed(e.to_string()))?;
  }
  ```

- [ ] Create a combined convenience flag `ORI_ALIVE2_CAPTURE=1` that enables both dumps and places them in a structured output directory (`build/alive2-capture/`):
  ```
  build/alive2-capture/
    program_name.preopt.ll
    program_name.postopt.ll
  ```

- [ ] Add function-level IR extraction. `alive-tv` compares functions by name — to verify function `@_ori_main`, the pre-opt and post-opt IR must both contain that function. Verify that `module.print_to_file()` preserves all function definitions (it should — this is standard LLVM behavior, but confirm).

- [ ] **TPR checkpoint** — `/tpr-review` covering 09.1–09.2 implementation work

- [ ] **Subsection close-out (09.2)** — MANDATORY before starting 09.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — same protocol as 09.1's close-out, scoped to 09.2's debugging journey. Commit improvements separately using a valid conventional-commit type.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 09.3 Diagnostic Script and Function Selection

**File(s):** `diagnostics/alive2-verify.sh`, `tests/alive2/curated-corpus.txt`

Build the diagnostic script that orchestrates alive-tv verification and curate the initial function corpus.

- [ ] Create `diagnostics/alive2-verify.sh` following existing diagnostic conventions (`--help`, `--no-color`, `--verbose`, `--json`, exit codes 0/1/2):
  ```bash
  # Usage: diagnostics/alive2-verify.sh [OPTIONS] <file.ori | --corpus>
  #
  # Options:
  #   --corpus           Run against curated corpus (tests/alive2/curated-corpus.txt)
  #   --function NAME    Verify only the named function
  #   --timeout SECS     Per-function Z3 timeout (default: 60)
  #   --passes PASSES    LLVM passes to verify (default: O2)
  #   --verbose          Show alive-tv output for passing functions
  #   --json             Machine-readable output
  #   --suppress FILE    False positive suppression file
  ```

- [ ] Implement the verification pipeline in the script:
  1. Build the `.ori` file with `ORI_ALIVE2_CAPTURE=1`
  2. Extract function names from the pre-opt IR
  3. For each function (or `--function` target):
     - Run `alive-tv preopt.ll postopt.ll --src-fn @func --tgt-fn @func --smt-to=<timeout>`
     - Parse output for "correct", "incorrect", "timeout", "unknown"
     - Check against suppression list for known false positives
  4. Report summary: N verified, M timeouts, K suppressed, L failures

- [ ] Curate the initial function corpus (`tests/alive2/curated-corpus.txt`). Selection criteria:
  - **Include**: Pure arithmetic functions, simple control flow (no loops or loops with small bounds), no runtime calls (`_ori_rc_inc`, `_ori_rc_dec`, `_ori_alloc`, `_ori_panic`), no exception handling (`invoke`/`landingpad`)
  - **Exclude**: Functions with `call void @_ori_rc_*` (RC operations), functions with `invoke` (exception handling), functions calling external runtime (`_ori_*`), functions with large loop nests (>256 iterations), functions with `va_arg` or variadics
  - **Source**: Start from `compiler/ori_llvm/tests/codegen/` (Section 07's FileCheck tests) — functions that have clean CHECK patterns are good Alive2 candidates. Also include pure functions from `tests/spec/` that compile to small LLVM IR.
  - Format: one line per entry: `<ori_file_path> <function_name>` (or `<ori_file_path> *` for all functions in the file)

- [ ] Create `tests/alive2/` directory with the corpus file and a README explaining the selection criteria and how to add new entries.

- [ ] Add the script to `diagnostics/self-test.sh` with a minimal positive test (one known-good pure function).

- [ ] **Subsection close-out (09.3)** — MANDATORY before starting 09.4:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 09.4 False Positive Management

**File(s):** `tests/alive2/suppressed.json`, `diagnostics/alive2-verify.sh` (suppression logic)

Alive2 will produce false positives for Ori programs because it conservatively approximates function calls and does not model Ori's runtime semantics. A structured suppression system prevents false positives from blocking CI while keeping a clear audit trail.

- [ ] Define the suppression file format (`tests/alive2/suppressed.json`):
  ```json
  [
    {
      "function": "_ori_main",
      "file": "compiler/ori_llvm/tests/codegen/rc/basic_rc.ori",
      "reason": "Contains _ori_rc_inc/_ori_rc_dec runtime calls that Alive2 cannot model",
      "category": "runtime-call",
      "added": "2026-04-10",
      "alive2_output_hash": "abc123..."
    }
  ]
  ```

- [ ] Implement suppression matching in `alive2-verify.sh`:
  - Before reporting a failure, check if the function+file pair is in the suppression list
  - If suppressed, report as "suppressed" (not "passed" and not "failed")
  - If the alive2 output hash differs from the recorded hash, report as "suppression-stale" — the failure mode changed, requiring re-investigation
  - `--strict` flag ignores all suppressions (for manual deep verification)

- [ ] Define suppression categories:
  - `runtime-call` — function calls Ori runtime (`_ori_*`) which Alive2 cannot model
  - `memory-model` — Alive2's memory model disagrees with Ori's ARC semantics
  - `loop-bound` — loop exceeds Alive2's unroll limit
  - `invoke` — exception handling paths not modeled
  - `inter-procedural` — Alive2 README explicitly warns about this

- [ ] Add a `--review-suppressions` flag that checks whether suppressions are still needed — rerun alive-tv on each suppressed entry and report which ones now pass (can be removed from the suppression list).

- [ ] **TPR checkpoint** — `/tpr-review` covering 09.3–09.4 implementation work

- [ ] **Subsection close-out (09.4)** — MANDATORY before starting 09.5:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 09.5 CI Integration (Nightly and Weekly)

**File(s):** `.github/workflows/ci.yml` (nightly job), `.github/workflows/nightly.yml` (or new nightly-verification.yml)

Wire alive-tv into CI with tiered execution: nightly runs the curated corpus (fast, high-value), weekly runs the full FileCheck test set (slow, comprehensive).

- [ ] Add nightly CI job `alive2-verify`:
  ```yaml
  alive2-verify:
    name: Alive2 Verification (Nightly)
    runs-on: ubuntu-latest
    timeout-minutes: 30
    steps:
      - uses: actions/checkout@v4
      - name: Install dependencies
        run: sudo apt-get install -y libz3-dev re2c
      - name: Install LLVM 21
        run: # ... same as existing CI LLVM install
      - name: Cache alive-tv binary
        uses: actions/cache@v4
        with:
          path: build/alive2/alive-tv
          key: alive2-llvm21-${{ hashFiles('scripts/build-alive2.sh') }}
      - name: Build alive-tv
        run: ./scripts/build-alive2.sh --cached
      - name: Build Ori compiler
        run: cargo build
      - name: Run Alive2 on curated corpus
        run: diagnostics/alive2-verify.sh --corpus --json --timeout 60
  ```

- [ ] Add weekly CI job `alive2-full`:
  ```yaml
  alive2-full:
    name: Alive2 Full Sweep (Weekly)
    runs-on: ubuntu-latest
    timeout-minutes: 120
    # Only on schedule, not on every push
    steps:
      # ... same setup
      - name: Run Alive2 on all codegen tests
        run: |
          exit_code=0
          for f in compiler/ori_llvm/tests/codegen/**/*.ori; do
            diagnostics/alive2-verify.sh "$f" --timeout 120 --json \
              --suppress tests/alive2/suppressed.json || exit_code=1
          done
          # Suppressed (known) failures are non-blocking (handled by --suppress).
          # Any NEW/unsuppressed refinement failure sets exit_code=1 and fails the job.
          exit "$exit_code"
  ```

- [ ] Configure Z3 timeout appropriately:
  - Nightly (curated): 60 seconds per function — these are pre-selected to be fast
  - Weekly (full sweep): 120 seconds per function — allows more complex functions
  - Functions that timeout are reported but do not fail the job (they indicate candidates for the suppression list or corpus exclusion)

- [ ] Add CI artifact upload for alive2 results:
  ```yaml
  - uses: actions/upload-artifact@v4
    with:
      name: alive2-results
      path: build/alive2-results/
  ```

- [ ] **Subsection close-out (09.5)** — MANDATORY before starting 09.R:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 09.R Third Party Review Findings

<!-- Reserved for Codex or other external reviewers.
If unresolved findings exist here:
- section frontmatter `status` must be `in-progress`
- `third_party_review.status` must be `findings`

When all findings are triaged:
- accepted findings are integrated into the relevant implementation subsection(s)
- rejected findings are closed with rationale
- all items in this block are marked resolved
- `third_party_review.status` becomes `resolved` or `none`
-->

- None.

---

## 09.N Completion Checklist

- [ ] `alive-tv` binary builds reproducibly via `scripts/build-alive2.sh`
- [ ] `ORI_DUMP_PREOPT_LLVM` and `ORI_DUMP_POSTOPT_LLVM` registered in `debug_flags.rs`
- [ ] `ORI_ALIVE2_CAPTURE=1` produces `.preopt.ll` and `.postopt.ll` files
- [ ] `diagnostics/alive2-verify.sh` passes `--help` and follows diagnostic conventions
- [ ] Curated corpus in `tests/alive2/curated-corpus.txt` with ≥15 functions
- [ ] All curated corpus functions pass alive-tv refinement checking
- [ ] Suppression file `tests/alive2/suppressed.json` documents all false positives with categories
- [ ] `--review-suppressions` identifies stale suppressions
- [ ] Nightly CI job runs curated corpus with zero failures
- [ ] Weekly CI job runs full sweep with results uploaded as artifacts
- [ ] Script added to `diagnostics/self-test.sh`
- [ ] No existing tests regressed: `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 09` returns 0 annotations
- [ ] All intermediate TPR checkpoint findings resolved
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] This section's frontmatter `status` → `complete`, subsection statuses updated
  - [ ] `00-overview.md` Quick Reference table status updated for this section
  - [ ] `00-overview.md` mission success criteria checkboxes updated
  - [ ] `index.md` section status updated
- [ ] `/tpr-review` passed (final, full-section)
- [ ] `/impl-hygiene-review` passed — AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` **section-close sweep** — verify per-subsection retrospectives ran, add cross-cutting items.

**Exit Criteria:** `diagnostics/alive2-verify.sh --corpus` passes with zero unresolved failures on the curated corpus of ≥15 pure/arithmetic functions. All false positives are documented in the suppression file with categories and rationale. Nightly CI runs the curated corpus; weekly CI runs the full `compiler/ori_llvm/tests/codegen/` set. The `alive-tv` binary is cached in CI. Pre-opt and post-opt IR capture is gated behind `ORI_ALIVE2_CAPTURE=1` with zero overhead when disabled.
