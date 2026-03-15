---
section: "04"
title: "IR Quality Polish"
status: not-started
goal: "All journeys IR Quality score ≥ 9/10 by eliminating unjustified instructions"
depends_on: ["01", "03"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Unjustified Instruction Audit"
    status: not-started
  - id: "04.2"
    title: "Range Materialization Cleanup (J7)"
    status: not-started
  - id: "04.3"
    title: "SSO Branch Reduction (J9)"
    status: not-started
  - id: "04.4"
    title: "Parameter Materialization Cleanup (J10)"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: IR Quality Polish

**Status:** Not Started
**Goal:** Reduce unjustified instruction count to ≤ 1 across all journeys. Target: IR Quality score ≥ 9/10 for all.

**Context:** Several journeys have unjustified instructions — IR that cannot be explained by overflow checking or safety requirements. These are typically redundant constructions (build a range struct then immediately destructure it), unnecessary intermediate values, or stale materialization patterns.

**Current IR quality scores:**
| Journey | Score | Unjustified | Primary Issue |
|---------|-------|-------------|---------------|
| J3 | 9/10 | 1 | Redundant entry block instruction |
| J5 | 9/10 | 2 | Dead EH code (addressed in Section 01) |
| J7 | 8/10 | 2 | Range construct-then-destructure |
| J9 | 8/10 | 4 | SSO gating redundancies |
| J10 | 8/10 | varies | Parameter materialization overhead |

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

- [ ] **Investigate**: Is the range construction in ARC IR or LLVM emission?
- [ ] **Fix**: If in LLVM emission, optimize range iteration to use scalar SSA values directly
- [ ] **Test**: J7 `@sum_for` unjustified count = 0

### 04.3 SSO gating redundancies (J9)
String functions have redundant branches related to SSO (Small String Optimization) gating. These are `if sso then X else X` patterns where both paths do the same thing.

- [ ] **Investigate**: Are these emitted by the ARC pipeline or the string runtime dispatch?
- [ ] **Fix**: If both SSO and heap paths produce the same code, merge them
- [ ] **Test**: J9 unjustified count ≤ 1

### 04.4 Parameter materialization (J10)
`@count_items` has verbose parameter materialization that extracts fields from the list struct and repacks them.

- [ ] **Investigate**: Is this a by-value vs by-pointer ABI issue?
- [ ] **Investigate**: Check `FunctionAbi` for `@count_items` — if the list param uses `ParamPassing::Indirect`, the caller creates a stack alloca, stores the struct, and passes a pointer. If it uses `ParamPassing::Direct`, the struct is passed in registers (which for a 3-field list struct means 3 separate register values, causing the extract/repack pattern).
- [ ] **Fix**: If the callee receives the struct by pointer, avoid extract/repack
- [ ] **Fix alternative**: If Direct passing causes the extract/repack, check the ABI threshold — list structs (3x i64 = 24 bytes) should be passed Indirect on x86-64. If ABI is correct, the issue is in the LLVM IR emission, not the ABI computation.
- [ ] **Test**: J10 `@count_items` instruction count reduced

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] All journeys IR Quality score ≥ 9/10
- [ ] Total unjustified instructions across all 13 journeys ≤ 5
- [ ] No range construct-then-destructure patterns
- [ ] SSO gating code in J9 has no redundant branches (both arms identical = merge)
- [ ] Re-run metrics after Sections 01-03 to determine remaining unjustified count (many will be auto-fixed by EH block removal and empty block elimination)
- [ ] All journeys still PASS
- [ ] `./test-all.sh` green

**Exit Criteria:** All journeys have ≤ 1 unjustified instruction. IR Quality score ≥ 9/10 across the board. Zero regressions.
