---
section: "04"
title: "IR Quality Polish"
status: complete
goal: "All journeys IR Quality score ≥ 9/10 by eliminating unjustified instructions"
depends_on: ["01", "03"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Unjustified Instruction Audit"
    status: complete
  - id: "04.2"
    title: "Range Materialization Cleanup (J7)"
    status: complete
  - id: "04.3"
    title: "SSO Branch Reduction (J9)"
    status: complete
  - id: "04.4"
    title: "Parameter Materialization Cleanup (J10)"
    status: complete
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: complete
---

# Section 04: IR Quality Polish

**Status:** Not Started
**Goal:** Reduce unjustified instruction count to ≤ 1 across all journeys. Target: IR Quality score ≥ 9/10 for all.

**Context:** Several journeys have unjustified instructions — IR that cannot be explained by overflow checking or safety requirements. These are typically redundant constructions (build a range struct then immediately destructure it), unnecessary intermediate values, or stale materialization patterns.

**Post-audit IR quality scores (2026-03-16):**
| Journey | Score | Unjustified | Status |
|---------|-------|-------------|--------|
| J1-J13 | 10/10 | 0 | All OPTIMAL |

All previously flagged issues resolved by Sections 01-03 fixes + metrics script correction (entry block preheader classification).

**Depends on:** Section 01 (J5 EH fix reduces unjustified count), Section 03 (empty block removal reduces counts).

---

## 04.1 Unjustified Instruction Audit

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/value_emission.rs` (range/struct construction), `compiler/ori_llvm/src/codegen/arc_emitter/operators/mod.rs` (overflow check patterns), `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/string_builtins.rs` (SSO gating), `compiler/ori_llvm/src/codegen/arc_emitter/apply_helpers.rs` (parameter materialization)

After Sections 01 and 03 are complete, re-run `.claude/skills/code-journey/extract-metrics.py` on all journeys to see which unjustified instructions remain. Many will be eliminated by the earlier fixes.

```bash
# Re-run metrics extraction after Sections 01-03
for i in $(seq 1 13); do
  python3 .claude/skills/code-journey/extract-metrics.py plans/code-journeys/$(printf '%02d' $i)-*-results.md
done
```

**Known patterns to investigate:**

### 04.2 Range construct-then-destructure (J7)
`@sum_for` builds a `Range { start, end, step }` struct, then immediately destructures it to extract start/end/step for the loop. This could be lowered directly to scalar SSA values.

- [x] **N/A** — J7 scores 10/10 (0 unjustified) in fresh IR after Sections 01-03. Verified 2026-03-16.

### 04.3 SSO gating redundancies (J9)
String functions have redundant branches related to SSO (Small String Optimization) gating. These are `if sso then X else X` patterns where both paths do the same thing.

- [x] **N/A** — J9 scores 10/10 (0 unjustified) in fresh IR after Sections 01-03. Verified 2026-03-16.

### 04.4 Parameter materialization (J10)
`@count_items` has verbose parameter materialization that extracts fields from the list struct and repacks them.

- [x] **N/A** — J10 scores 10/10 (0 unjustified) in fresh IR after Sections 01-03. Verified 2026-03-16.

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [x] All journeys IR Quality score ≥ 9/10 (all 13 score 10/10)
- [x] Total unjustified instructions across all 13 journeys ≤ 5 (actual: 0)
- [x] No range construct-then-destructure patterns (J7 scores OPTIMAL)
- [x] SSO gating code in J9 has no redundant branches (J9 scores OPTIMAL)
- [x] Re-run metrics after Sections 01-03: all 0 unjustified. Metrics script updated to correctly classify entry block preheaders. 173 metrics tests pass.
- [x] All journeys still PASS (eval + AOT produce correct exit codes on all 13)
- [x] `cargo t` green, `cargo clippy --workspace` green

**Exit Criteria:** All journeys have ≤ 1 unjustified instruction. IR Quality score ≥ 9/10 across the board. Zero regressions.
