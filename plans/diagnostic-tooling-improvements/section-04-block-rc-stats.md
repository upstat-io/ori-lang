---
section: "04"
title: "Block-level RC Stats"
status: not-started
reviewed: false
goal: "Extend ORI_AUDIT_CODEGEN to emit per-basic-block RC stats as structured JSON, and update rc-stats.sh to consume it via --block-level"
success_criteria:
  - "ORI_AUDIT_CODEGEN=1 emits per-block JSON to stderr (alongside existing text output)"
  - "JSON format includes function name, block label, and per-block alloc/inc/dec/free/cow counts"
  - "rc-stats.sh --block-level reads the JSON and displays per-block stats table"
  - "rc-stats.sh default mode migrated from awk IR parsing to compiler JSON — awk parser removed"
  - "No LEAK:scattered-knowledge — block-level stats come from the compiler, not shell regex"
inspired_by:
  - "Swift ARC optimizer — tracks retain/release per SIL basic block"
  - "Lean 4 IR RC — per-block inc/dec analysis"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Extend ORI_AUDIT_CODEGEN with per-block JSON"
    status: not-started
  - id: "04.2"
    title: "Update rc-stats.sh with --block-level"
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
**Goal:** Give developers the ability to localize RC leaks/over-releases to specific basic blocks within a function, not just the function as a whole. Currently `rc-stats.sh` reports per-function totals — "this function has +2 balance" — but can't tell you WHICH loop or branch is responsible. The fix extends the compiler's audit pass to emit per-block data, then updates the shell script to render it.

**Success Criteria:**
- [ ] `ORI_AUDIT_CODEGEN=1 ori build file.ori` emits JSON per-block data on stderr (new `codegen audit: json:` prefix line)
- [ ] JSON schema: `{"functions": [{"name": "...", "blocks": [{"label": "...", "alloc": N, "inc": N, "dec": N, "free": N, "cow": N}]}]}`
- [ ] `rc-stats.sh --block-level file.ori` renders a per-block table with balance per block
- [ ] `rc-stats.sh` (no flag) still works identically to today (backward compatible)
- [ ] Satisfies mission criterion: "ORI_AUDIT_CODEGEN=1 emits per-block structured JSON; rc-stats.sh --block-level consumes it"

**Context:** Both Codex and Gemini independently flagged the shell regex approach (parsing LLVM IR for `ori_rc_alloc` etc.) as LEAK:scattered-knowledge per impl-hygiene.md. The compiler already computes RC operation analysis in `ORI_AUDIT_CODEGEN` — the shell script shouldn't re-derive that. Gemini's pushback: "Do not settle for the shell-based block parser. It will break on the first complex match expression. Build the JSON bridge once; your shell scripts will be 10x simpler."

**Reference implementations:**
- **Swift** `ARC optimizer`: tracks retain/release per SIL basic block
- **Lean 4** `IR/RC.lean`: per-block inc/dec analysis in the RC insertion pass

**Depends on:** None.

---

## 04.1 Extend ORI_AUDIT_CODEGEN with per-block JSON

**File(s):** `compiler/ori_llvm/src/verify/mod.rs` (or wherever `ORI_AUDIT_CODEGEN` output is emitted)

This is a targeted Rust change. The audit pass already walks LLVM IR instructions and counts RC operations — it just doesn't group them by basic block or emit structured data.

- [ ] Find the codegen audit implementation: `grep -rn "ORI_AUDIT_CODEGEN\|codegen audit:" compiler/ori_llvm/src/`
- [ ] Add a `--json` or JSON-mode output path that emits per-block RC operation counts
  - Group counts by LLVM basic block label (use `BasicBlock::get_name()`)
  - Group blocks by function
  - Emit as a single JSON line to stderr with prefix `codegen audit: json: {...}`
  - The text-mode output (existing) is unaffected — JSON is an ADDITIONAL output line
- [ ] Add Rust unit tests in the appropriate `tests.rs`:
  - A simple program produces expected per-block JSON
  - An empty program produces `{"functions": []}`
  - A program with RC ops shows non-zero counts in the correct blocks
- [ ] Run `timeout 150 cargo t -p ori_llvm` to verify no regressions

**File size check:** Verify the audit file stays under 500 lines. If it would exceed, extract the JSON emitter into a submodule.

- [ ] **Subsection close-out (04.1)** — MANDATORY before starting 04.2:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 04.2 Update rc-stats.sh with --block-level

**File(s):** `diagnostics/rc-stats.sh`, `diagnostics/self-test.sh`

- [ ] Add `--block-level` flag to rc-stats.sh
  - When passed, compile with `ORI_AUDIT_CODEGEN=1`, extract the JSON line, parse with `python3 -c` or `jq`
  - Render a table showing: function > block > alloc/inc/dec/free/cow/balance
  - Flag imbalanced blocks with the same warning symbols as the existing function-level output
  - Exit code: 0 = all blocks balanced, 1 = imbalanced blocks found
- [ ] Default mode (no `--block-level`): migrate from awk-based LLVM IR parsing to compiler JSON. The JSON output from 04.1 includes function-level totals (sum of per-block counts), so the default mode can be rewritten to consume JSON instead of regex-matching IR. This eliminates the LEAK:scattered-knowledge identified in the overview. The awk parser is removed, not kept as a fallback.
- [ ] Add self-test entries:
  - `rc-stats.sh --block-level fixtures/clean.ori` produces non-empty per-block output
  - `rc-stats.sh fixtures/clean.ori` (no flag) still produces function-level output
- [ ] Verify: `diagnostics/self-test.sh` passes

- [ ] **Subsection close-out (04.2)** — MANDATORY before starting 04.R:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] All subsections (04.1, 04.2) complete
- [ ] `timeout 150 cargo t -p ori_llvm` passes
- [ ] `diagnostics/self-test.sh` passes
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] **`/improve-tooling` section-close sweep**
