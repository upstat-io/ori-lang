---
section: "11"
title: "CI Integration & ARC IR Parity"
status: not-started
reviewed: false
goal: "Consolidate all verification tools into the CI pipeline with tiered execution (every-commit, nightly, weekly), add ARC IR debug-vs-release parity checking, and create an opt-bisect diagnostic script — ensuring that no verification capability from Sections 01-10 exists only as a local tool without CI enforcement"
success_criteria:
  - ".github/workflows/ci.yml runs ORI_VERIFY_EACH=1, function-level verify, LLVM backend spec tests, and FileCheck tests on every PR"
  - "Nightly CI job runs sanitizers (ASan/UBSan), Alive2 curated corpus, AIMS snapshot tests, and ARC IR parity"
  - "Weekly CI job runs differential fuzzing, full Alive2 sweep, and full sanitizer matrix"
  - "ARC IR parity: debug-release-compare.sh extended with ORI_DUMP_AFTER_ARC=1 comparison"
  - "diagnostics/opt-bisect.sh wraps opt --opt-bisect-limit for LLVM pass bisection"
  - "Each section's CI gate was added incrementally (§11 consolidates, does not create from scratch)"
  - "LLVM crash escape hatch in test-all.sh NOT removed here (owned by plans/llvm-worker-isolation/)"
inspired_by:
  - "Rust CI (.github/workflows/ci*.yml) — tiered PR/nightly/weekly pipeline with separate jobs for different verification levels"
  - "Swift CI (swift/utils/build-script) — tiered validation: smoke (every commit), full (nightly), stress (weekly)"
  - "LLVM CI (llvm-project/.github/workflows/) — separate sanitizer, fuzz, and verification jobs"
depends_on: ["01", "02", "03", "04", "05", "06", "07", "08", "09", "10"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "11.1"
    title: "Every-Commit CI Tier"
    status: not-started
  - id: "11.2"
    title: "Nightly CI Tier"
    status: not-started
  - id: "11.3"
    title: "Weekly CI Tier"
    status: not-started
  - id: "11.4"
    title: "ARC IR Debug-vs-Release Parity"
    status: not-started
  - id: "11.5"
    title: "opt-bisect Diagnostic Script"
    status: not-started
  - id: "11.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "11.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 11: CI Integration & ARC IR Parity

**Status:** Not Started
**Goal:** Consolidate all verification tools from Sections 01-10 into a coherent CI pipeline with three tiers: every-commit (fast, high-signal), nightly (medium, comprehensive), and weekly (slow, exhaustive). Add ARC IR debug-vs-release parity checking (extending the existing `debug-release-compare.sh`) and an `opt-bisect` diagnostic script. This section does NOT create CI gates from scratch — per Codex feedback, each earlier section should already add its own CI gate. Section 11 verifies completeness, fills gaps, and ensures the tiered execution model is coherent.

**CRITICAL BLOCKER**: The LLVM crash escape hatch in `test-all.sh` (which masks LLVM backend crashes) is owned by `plans/llvm-worker-isolation/`. This section does NOT remove that escape hatch. If the escape hatch is still present when Section 11 starts, document it as a known limitation and add a `<!-- blocked-by: llvm-worker-isolation -->` annotation.

**Success Criteria:**

- [ ] Three CI tiers operational — satisfies mission criterion: "CI fully integrated"
- [ ] ARC IR parity catches structural divergences — satisfies mission criterion: "ARC IR debug-vs-release parity"
- [ ] opt-bisect script identifies failing LLVM passes — satisfies mission criterion: "diagnostic tooling"
- [ ] No verification tool exists only locally — satisfies mission criterion: "CI runs all verification"

**Context:** The current CI (`.github/workflows/ci.yml`) runs three test suites: Rust workspace tests (`cargo test --workspace`), Ori spec tests (interpreter only, via `cargo run -p oric -- test tests/`), and runtime tests (`cargo test -p ori_rt`). It is MISSING: LLVM backend spec tests (`ori test --backend=llvm tests/`), FileCheck IR tests, sanitizer instrumentation, AIMS snapshot verification, Alive2 refinement checking, and differential fuzzing. Each of Sections 01-10 adds its own CI gate incrementally; this section verifies that all gates are present and organizes them into the tiered execution model described in the research document.

**Reference implementations:**
- **Rust** `.github/workflows/`: Separate workflows for PR CI (fast), scheduled CI (nightly with extra sanitizer jobs), and weekly jobs (fuzzing, extensive testing).
- **Swift** `utils/build-script`: Tiered build modes with `--validation-test` (nightly) and `--stress-test` (weekly).
- **LLVM** `.github/workflows/`: Separate sanitizer, coverage, and fuzz jobs on different schedules.

**Depends on:** All previous sections (01-10). This is the integration section.

---

## 11.1 Every-Commit CI Tier

**File(s):** `.github/workflows/ci.yml`

The every-commit tier runs on every PR. It must be fast (add at most 3 minutes to current CI) and high-signal (catch the most common regressions). These gates should already exist from Sections 01-08 — this subsection audits and fills gaps.

- [ ] Audit existing CI for gates that should already be present from earlier sections:
  - **§01**: `ORI_VERIFY_EACH=1` and `ORI_VERIFY_ARC=1` in env block
  - **§01**: Function-level `fn_val.verify()` (implicit — runs during `cargo test --workspace`)
  - **§07**: FileCheck tests in `compiler/ori_llvm/tests/codegen/` (via `cargo test --workspace` if integrated as Rust tests, or explicit `ori test --backend=llvm compiler/ori_llvm/tests/codegen/`)
  - **§08**: Sanitizer smoke (if §08 added a smoke job — check)
  - **MISSING (known)**: `ori test --backend=llvm tests/` — LLVM backend spec tests

- [ ] Add LLVM backend spec tests to the every-commit CI if not already present:
  ```yaml
  - name: Ori LLVM backend tests
    run: |
      set -o pipefail
      cargo run -p oric --bin ori -- test --backend=llvm tests/ 2>&1 | tee llvm-test-output.txt
      LLVM_TESTS=$(grep -oE '[0-9]+ passed' llvm-test-output.txt | grep -oE '^[0-9]+' | tail -1 || echo "0")
      LLVM_FAILED=$(grep -oE '[0-9]+ failed' llvm-test-output.txt | grep -oE '^[0-9]+' | tail -1 || echo "0")
      echo "LLVM_TESTS=$LLVM_TESTS" >> $GITHUB_ENV
      echo "LLVM_FAILED=$LLVM_FAILED" >> $GITHUB_ENV
    env:
      LLVM_SYS_211_PREFIX: /usr/lib/llvm-21
      ORI_VERIFY_EACH: "1"
      ORI_VERIFY_ARC: "1"
  ```

- [ ] Verify the total CI time stays within the performance budget. Current `test` job timeout is 30 minutes. The every-commit additions should add at most 3 minutes:
  - `ORI_VERIFY_EACH=1` adds ~30-60% to LLVM test time (from research)
  - FileCheck tests: fast (simple pattern matching)
  - LLVM backend spec tests: medium (AOT compile + execute per test)
  - If the total exceeds 30 minutes, parallelize by splitting into multiple jobs

- [ ] Add the verification env vars to ALL test steps in the `test` job, not just the LLVM-specific one:
  ```yaml
  env:
    ORI_VERIFY_EACH: "1"
    ORI_VERIFY_ARC: "1"
    LLVM_SYS_211_PREFIX: /usr/lib/llvm-21
  ```

- [ ] Update the test results summary to include LLVM backend test counts in `test-results.json`.

- [ ] **Subsection close-out (11.1)** — MANDATORY before starting 11.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect on the debugging journey for 11.1 specifically: CI configuration debugging, workflow syntax issues, timing analysis. Implement every accepted improvement NOW and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`ci: ...`).

---

## 11.2 Nightly CI Tier

**File(s):** `.github/workflows/nightly-verification.yml` (new workflow, or extend existing `nightly.yml`)

The nightly tier runs on a schedule (e.g., 2:00 AM UTC). It runs more expensive verification that is too slow for every-commit CI but should catch issues before they accumulate.

- [ ] Create `.github/workflows/nightly-verification.yml` (separate from the existing `nightly.yml` which handles release PRs):
  ```yaml
  name: Nightly Verification

  on:
    schedule:
      - cron: '0 2 * * *'  # 2:00 AM UTC daily
    workflow_dispatch:       # Manual trigger

  jobs:
    sanitizers:
      name: Sanitizer Suite
      runs-on: ubuntu-latest
      timeout-minutes: 45
      steps:
        # ... LLVM + Rust setup
        - name: ASan/UBSan smoke
          run: # ... from §08's CI gate
          env:
            ORI_SANITIZE: "address,undefined"

    alive2:
      name: Alive2 Curated Corpus
      runs-on: ubuntu-latest
      timeout-minutes: 30
      steps:
        # ... from §09.5's nightly job

    aims-snapshots:
      name: AIMS Snapshot Verification
      runs-on: ubuntu-latest
      timeout-minutes: 15
      steps:
        # ... build + cargo test -p ori_arc --test aims_snapshots

    arc-parity:
      name: ARC IR Parity (Debug vs Release)
      runs-on: ubuntu-latest
      timeout-minutes: 20
      steps:
        # ... from §11.4 below
  ```

- [ ] Audit which nightly gates should already exist from earlier sections:
  - **§03**: AIMS snapshot tests (`cargo test -p ori_arc --test aims_snapshots`)
  - **§08**: Full sanitizer suite (ASan/UBSan on AOT smoke subset)
  - **§09**: Alive2 curated corpus (`diagnostics/alive2-verify.sh --corpus`)

- [ ] Add failure notification. Nightly failures must notify (unlike weekly, which is informational):
  ```yaml
  notify:
    name: Notify on Failure
    needs: [sanitizers, alive2, aims-snapshots, arc-parity]
    if: failure()
    runs-on: ubuntu-latest
    steps:
      - name: Create issue
        uses: actions/github-script@v7
        with:
          script: |
            github.rest.issues.create({
              owner: context.repo.owner,
              repo: context.repo.repo,
              title: `Nightly verification failed: ${new Date().toISOString().slice(0,10)}`,
              body: `Nightly verification pipeline failed. Check workflow run.`,
              labels: ['nightly-failure', 'verification']
            })
  ```

- [ ] **TPR checkpoint** — `/tpr-review` covering 11.1–11.2 implementation work

- [ ] **Subsection close-out (11.2)** — MANDATORY before starting 11.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — same protocol as 11.1's close-out, scoped to 11.2's debugging journey. Commit improvements separately using a valid conventional-commit type.

---

## 11.3 Weekly CI Tier

**File(s):** `.github/workflows/weekly-verification.yml` (new workflow)

The weekly tier runs expensive, exhaustive verification: full fuzzing campaigns, complete Alive2 sweeps, and full sanitizer matrices. Results are informational — failures create tracking issues but do not block development.

- [ ] Create `.github/workflows/weekly-verification.yml`:
  ```yaml
  name: Weekly Verification

  on:
    schedule:
      - cron: '0 4 * * 0'  # 4:00 AM UTC every Sunday
    workflow_dispatch:

  jobs:
    fuzz:
      name: Differential Fuzzing
      runs-on: ubuntu-latest
      timeout-minutes: 240  # 4 hours
      steps:
        # ... from §10.5's weekly job

    alive2-full:
      name: Alive2 Full Sweep
      runs-on: ubuntu-latest
      timeout-minutes: 120
      steps:
        # ... from §09.5's weekly job

    sanitizer-matrix:
      name: Full Sanitizer Matrix
      runs-on: ubuntu-latest
      timeout-minutes: 60
      strategy:
        matrix:
          sanitizer: [address, undefined]
          # MSan requires full-program instrumentation — separate job
      steps:
        # ... per-sanitizer full sweep from §08
  ```

- [ ] Upload all results as CI artifacts for historical comparison (consumed by §12):
  ```yaml
  - uses: actions/upload-artifact@v4
    with:
      name: weekly-verification-${{ github.run_id }}
      path: |
        build/alive2-results/
        fuzz/artifacts/
        build/sanitizer-results/
      retention-days: 90
  ```

- [ ] Weekly failures create tracking issues but do NOT block merges:
  ```yaml
  if: failure()
  steps:
    - name: Create tracking issue
      uses: actions/github-script@v7
      with:
        script: |
          github.rest.issues.create({
            owner: context.repo.owner,
            repo: context.repo.repo,
            title: `Weekly verification: findings on ${new Date().toISOString().slice(0,10)}`,
            body: `Weekly verification found issues. Triage required.`,
            labels: ['weekly-verification', 'triage-needed']
          })
  ```

- [ ] **Subsection close-out (11.3)** — MANDATORY before starting 11.4:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 11.4 ARC IR Debug-vs-Release Parity

**File(s):** `diagnostics/debug-release-compare.sh`

Extend the existing `debug-release-compare.sh` to capture and compare ARC IR between debug and release builds. Currently the script compares behavioral output (exit codes + stdout) and LLVM IR diffs — but not ARC IR. AIMS pipeline divergences between debug and release (due to different optimization flags or analysis precision) can cause subtle behavioral drift masked by LLVM optimization.

- [ ] Add `--arc-ir` flag to `debug-release-compare.sh` that enables ARC IR comparison:
  ```bash
  # In the debug build step:
  ORI_DUMP_AFTER_ARC=1 cargo run -- build "$file" -o "$debug_binary" 2>"$debug_arc_ir"

  # In the release build step:
  ORI_DUMP_AFTER_ARC=1 cargo run --release -- build "$file" -o "$release_binary" 2>"$release_arc_ir"

  # Compare ARC IR (structural diff, ignoring whitespace and variable IDs)
  diff_arc_ir "$debug_arc_ir" "$release_arc_ir"
  ```

- [ ] Implement ARC IR diff normalization. ARC IR uses generated variable IDs (`v0`, `v1`, ...) that may differ between debug and release builds due to different allocation patterns. The diff must normalize:
  - Variable IDs: `v<N>` → canonical renumbering based on first occurrence
  - Block IDs: `bb<N>` → canonical renumbering based on first occurrence
  - Whitespace: normalize to single spaces
  - Keep all instructions, RC operations, and control flow intact

- [ ] Add ARC IR parity to the nightly CI (§11.2) with a representative subset of test programs:
  ```bash
  # Run on key programs that exercise different ARC patterns:
  for f in tests/spec/traits/iterator/*.ori tests/spec/collections/cow/*.ori; do
      diagnostics/debug-release-compare.sh --arc-ir "$f" || PARITY_FAILURES=$((PARITY_FAILURES + 1))
  done
  ```

- [ ] **TPR checkpoint** — `/tpr-review` covering 11.3–11.4 implementation work

- [ ] **Subsection close-out (11.4)** — MANDATORY before starting 11.5:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 11.5 opt-bisect Diagnostic Script

**File(s):** `diagnostics/opt-bisect.sh`

Create a diagnostic script that wraps LLVM's `opt --opt-bisect-limit` to binary-search which LLVM optimization pass breaks a program. This is distinct from AIMS phase bisection (`diagnostics/bisect-passes.sh`, which bisects the 12-step AIMS pipeline) — `opt-bisect` bisects LLVM's own optimization passes (instcombine, GVN, SROA, etc.).

- [ ] Create `diagnostics/opt-bisect.sh` following diagnostic conventions (`--help`, `--no-color`, `--verbose`, `--json`, exit codes 0/1/2):
  ```bash
  # Usage: diagnostics/opt-bisect.sh <file.ori> [OPTIONS]
  #
  # Binary-searches which LLVM optimization pass breaks the program.
  # The "broken" condition is: the optimized binary produces different
  # output than the unoptimized binary, OR the optimized binary crashes.
  #
  # Options:
  #   --expected OUTPUT  Expected stdout (default: captured from -O0 build)
  #   --check-leaks      Also check ORI_CHECK_LEAKS divergence
  #   --verbose          Show each bisection step
  #   --json             Machine-readable output
  ```

- [ ] Implement the bisection algorithm:
  1. Build with `-O0` (no optimizations) — capture expected output
  2. Build with full optimization — verify the bug reproduces (different output or crash)
  3. Binary search using `opt --opt-bisect-limit=N`:
     - Set `LLVM_OPT_BISECT_LIMIT=N` environment variable (LLVM respects this)
     - Build and run with limit N
     - Compare output to expected
     - Narrow the range until the specific pass is identified
  4. Report: "Pass N ({pass_name}) at function {func_name} introduces the miscompile"

- [ ] Handle the Ori-specific integration: LLVM's opt-bisect-limit is read via the `LLVM_OPT_BISECT_LIMIT` environment variable by the LLVM optimization pipeline. Ori's `run_optimization_passes` must pass this through. Verify that the existing LLVM C API / Inkwell integration respects this env var.

- [ ] Add the script to `diagnostics/self-test.sh` with a positive test (a program that compiles correctly at all optimization levels — the script should report "no miscompile found").

- [ ] **Subsection close-out (11.5)** — MANDATORY before starting 11.R:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 11.R Third Party Review Findings

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

## 11.N Completion Checklist

- [ ] Every-commit CI runs: `ORI_VERIFY_EACH=1`, `ORI_VERIFY_ARC=1`, LLVM backend spec tests, FileCheck tests
- [ ] Every-commit CI total time within 30-minute budget
- [ ] Nightly CI runs: sanitizers, Alive2 curated corpus, AIMS snapshots, ARC IR parity
- [ ] Nightly failure creates GitHub issue automatically
- [ ] Weekly CI runs: differential fuzzing, Alive2 full sweep, sanitizer matrix
- [ ] Weekly results uploaded as CI artifacts with 90-day retention
- [ ] `debug-release-compare.sh --arc-ir` produces normalized ARC IR diffs
- [ ] ARC IR variable/block ID normalization prevents false positives
- [ ] `diagnostics/opt-bisect.sh` identifies failing LLVM optimization pass
- [ ] opt-bisect added to `diagnostics/self-test.sh`
- [ ] LLVM crash escape hatch status documented (blocked by `plans/llvm-worker-isolation/`)
- [ ] All §01-§10 CI gates verified present and functional
- [ ] No existing tests regressed: `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 11` returns 0 annotations
- [ ] All intermediate TPR checkpoint findings resolved
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] This section's frontmatter `status` → `complete`, subsection statuses updated
  - [ ] `00-overview.md` Quick Reference table status updated for this section
  - [ ] `00-overview.md` mission success criteria checkboxes updated
  - [ ] `index.md` section status updated
- [ ] `/tpr-review` passed (final, full-section)
- [ ] `/impl-hygiene-review` passed — AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` **section-close sweep** — verify per-subsection retrospectives ran, add cross-cutting items.

**Exit Criteria:** Three CI tiers operational and tested. Every-commit tier runs verification gates from §01 plus LLVM backend spec tests and FileCheck tests within the 30-minute budget. Nightly tier runs sanitizers, Alive2, AIMS snapshots, and ARC IR parity with automatic failure notification. Weekly tier runs differential fuzzing, full Alive2 sweep, and sanitizer matrix with artifact upload. `debug-release-compare.sh --arc-ir` catches ARC IR structural divergences with normalized diffing. `diagnostics/opt-bisect.sh` identifies failing LLVM passes via binary search. No verification tool from §01-§10 exists only locally without CI enforcement.
