---
section: "02"
title: "Enhance diagnose-aot.sh"
status: complete
reviewed: true
goal: "Add codegen-audit, ARC IR dump, debug+release dual-build, and ORI_VERIFY_ARC to the primary AOT diagnostic tool"
success_criteria:
  - "diagnose-aot.sh runs codegen-audit.sh as a new section (6/9)"
  - "diagnose-aot.sh dumps ARC IR via arc-dump.sh as a new section"
  - "diagnose-aot.sh --release builds and tests against release binary"
  - "diagnose-aot.sh --both-builds runs the FULL battery against both debug and release"
  - "diagnose-aot.sh enables ORI_VERIFY_ARC=1 during compilation"
inspired_by:
  - "Swift SIL verifier -sil-verify-all — forces verification in all build modes"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Add codegen-audit and ARC IR sections"
    status: complete
  - id: "02.2"
    title: "Add release build and ORI_VERIFY_ARC support"
    status: complete
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 02: Enhance diagnose-aot.sh

**Status:** Complete
**Goal:** Make `diagnose-aot.sh` the definitive single-command AOT diagnostic — currently it misses codegen-audit, ARC IR, release builds, and ARC verification. After this section, running `diagnose-aot.sh --both-builds --valgrind file.ori` exercises every diagnostic layer available.

**Success Criteria:**
- [x] Codegen-audit runs as section 6 of the 9-section diagnostic battery
- [x] ARC IR dump runs as a section, saving ARC IR alongside LLVM IR
- [x] `--release` flag builds/runs against release binary
- [x] `--both-builds` runs the FULL battery twice (debug + release) and highlights divergences
- [x] `ORI_VERIFY_ARC=1` is enabled during compilation by default
- [x] Satisfies mission criterion: "diagnose-aot.sh runs codegen-audit, dumps ARC IR, supports --release and --both-builds"

**Context:** `diagnose-aot.sh` is documented as the "first tool to reach for on any AOT bug" (README.md line 11). But it never runs `codegen-audit.sh`, never dumps ARC IR (even though `arc-dump.sh` exists), never checks release builds (where FastISel bugs live), and never enables `ORI_VERIFY_ARC`. This means the "first tool" misses 3 of the 5 most common AOT failure modes.

**Depends on:** None.

---

## 02.1 Add codegen-audit and ARC IR sections

**File(s):** `diagnostics/diagnose-aot.sh`

Add two new sections to the 7-section diagnostic battery, making it 9 sections (numbered sequentially 1-9).

- [x] Add **Section 6: Codegen Audit** after LLVM IR (Section 5):
  - Run `codegen-audit.sh --no-color "$FILE"` (or `--color` based on color mode)
  - Capture exit code and output
  - Map: exit 0 = PASS (no findings), exit 1 = WARN (findings detected), exit 2 = FAIL (compilation or infrastructure failure — surface the error, do not skip)
  - Display findings inline
- [x] Add **Section 7: ARC IR** after Codegen Audit:
  - Run `arc-dump.sh --raw "$FILE"` to capture ARC IR
  - Save to `$tmpdir/arc-${basename_file%.ori}.txt`
  - Report line count (INFO status, same pattern as LLVM IR section)
- [x] Renumber existing Section 6 (Valgrind) → **Section 8** and Section 7 (Disassembly) → **Section 9** (sequential numbering, no gaps)
- [x] Update all `[N/M]` labels from `[N/7]` to `[N/9]` to reflect new total (9 sections)
- [x] Update `section_names` array in the summary block to include new sections and new numbering
- [x] Enable `ORI_VERIFY_ARC=1` during the compilation step (Section 1) to catch ARC IR verification failures
- [x] Verify: `diagnostics/diagnose-aot.sh diagnostics/fixtures/simple.ori` shows all sections including new ones

- [x] **Subsection close-out (02.1)** — MANDATORY before starting 02.2:
  - [x] All tasks above are `[x]` and verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 02.2 Add release build and ORI_VERIFY_ARC support

**File(s):** `diagnostics/diagnose-aot.sh`, `diagnostics/_common.sh`

- [x] Add `--release` flag: uses `target/release/ori` instead of `target/debug/ori`
  - Check that the release binary exists; if not, suggest `cargo b --release`
- [x] Add `--both-builds` flag: runs the FULL diagnostic battery twice (debug then release)
  - Print a clear separator between debug and release runs
  - On completion, highlight any sections where debug and release produced different results (different exit codes, different leak counts, different RC balance)
  - Exit code: 0 if both clean, 1 if either has failure
- [x] Use `find_ori_bin_profile()` and `require_both_builds()` from `_common.sh` (added by Section 01.2) for `--release` and `--both-builds` binary resolution — no new binary-discovery logic needed here
- [x] Update `diagnostics/self-test.sh` with tests for `--release` flag (at minimum: `--help` output includes `--release`)
- [x] Verify: `diagnostics/diagnose-aot.sh --both-builds diagnostics/fixtures/clean.ori` runs both builds and shows comparison

- [x] **Subsection close-out (02.2)** — MANDATORY before starting 02.R:
  - [x] All tasks above are `[x]` and verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [x] All subsections (02.1, 02.2) complete
- [x] `diagnostics/self-test.sh` passes
- [x] `diagnose-aot.sh --help` shows new options
- [x] `timeout 150 ./test-all.sh` green — no regressions
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] **`/improve-tooling` section-close sweep**
