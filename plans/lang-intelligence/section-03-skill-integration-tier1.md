---
section: "03"
title: "Skill Integration: TPR + Fix-Bug (Tier 1)"
status: not-started
reviewed: false
goal: "Insert intelligence pre-queries into the two highest-value skills: /tpr-review (evidence packets for reviewers) and /fix-bug (investigation phase)."
success_criteria:
  - "/tpr-review Step 2 evidence packets include an Intelligence Summary section with cross-language prior art"
  - "/fix-bug Phase 1 queries intelligence DB for similar bugs in reference compilers before root cause analysis"
  - "Both skills use scripts/intel-query.sh — no open-coded Neo4j logic"
  - "Both skills degrade gracefully when intelligence is unavailable"
  - "Evidence packet size is bounded (max 500 chars of intelligence summary to avoid noisy reviewers)"
depends_on: ["01", "02"]
third_party_review:
  status: none
  updated: null
---

# 03 Skill Integration: TPR + Fix-Bug (Tier 1)

## 03.0 Goal

Insert intelligence queries into the two highest-value Claude skills. These are Tier 1 because:
- `/tpr-review` runs after every non-trivial change — highest frequency
- `/fix-bug` runs for every bug — highest impact per invocation

The key insight from the TPR (Codex finding #1): **Claude must pre-query and inject results into the reviewer evidence packet.** Codex and Gemini run in sandboxed environments and cannot query Neo4j themselves. So Claude queries before launch and includes the results.

## 03.1 TPR Review Integration

**File**: `.claude/skills/tpr-review/SKILL.md`

**Insertion point**: New Step 0.75, between Step 0.5 (spec/grammar gate) and Step 1 (scratch dir). This runs BEFORE writing reviewer prompts.

**What to add**:

```markdown
## Step 0.75 — OPTIONAL: Intelligence Pre-Query

If the intelligence graph is available (check via `scripts/intel-query.sh status`),
query for cross-language prior art relevant to the code under review.

Identify the subsystem(s) being reviewed from the diff:
- ARC/memory code → `scripts/intel-query.sh ori-arc`
- Type inference → `scripts/intel-query.sh ori-inference`
- LLVM codegen → `scripts/intel-query.sh ori-codegen`
- Pattern matching → `scripts/intel-query.sh ori-patterns`
- Diagnostics → `scripts/intel-query.sh ori-diagnostics`
- General → `scripts/intel-query.sh search "<key terms from diff>"` --limit 10

Condense results into a bounded Intelligence Summary (max 500 chars):
  **Intelligence Summary (from cross-language graph):**
  - [rust#12345] Similar ARC bug in iterator early-exit (fixed, 45 comments)
  - [swift#6789] Protocol witness table leak on break (fixed, 12 comments)
  - Pattern appears in 3/10 reference compilers

Include this summary in BOTH reviewer prompts (Step 2) as part of the evidence packet,
after the "## Scope:" header. Reviewers should use it as a pointer to investigate,
not as authoritative evidence.

If intelligence is unavailable, skip silently — do not mention it in the prompts.
```

**Implementation checklist**:
- [ ] Add Step 0.75 to `.claude/skills/tpr-review/SKILL.md` after Step 0.5
- [ ] Map diff subsystems to intelligence presets (use file paths from `git diff --name-only`)
- [ ] Bound the intelligence summary to 500 chars max (condensed, not raw output)
- [ ] Include summary in both `codex.prompt.md` and `gemini.prompt.md` evidence packets
- [ ] Test: intelligence available → summary appears in prompts
- [ ] Test: intelligence unavailable → no mention in prompts, no error
- [ ] Test: empty results → no intelligence section in prompts (don't include "no results found")

### Subsection 03.1 close-out

**`/improve-tooling` retrospective**: Was the 500-char bound appropriate? Too much context makes reviewers noisy; too little wastes the query. Adjust based on actual TPR round results.

---

## 03.2 Fix-Bug Integration

**File**: `.claude/skills/fix-bug/SKILL.md`

**Insertion point**: Phase 1 (Investigation), after step 4 (Root cause analysis) and before step 5 (Check reference compilers). The intelligence query REPLACES the manual "check reference compilers" step with a structured graph query.

**What to add**:

```markdown
### Phase 1, Step 5: Intelligence Graph Query (replaces manual reference check)

If the intelligence graph is available, query for similar bugs:

  scripts/intel-query.sh search "<bug description keywords>" --limit 10
  scripts/intel-query.sh fixed "<bug category>" --repo rust,swift,koka,lean4

Look for:
- Same failure mode in other compilers (strongest signal if 2+ repos)
- How they fixed it (check the fix PR title/body)
- What regressions the fix introduced (follow FIXES edges)

Record relevant findings in the fix section's investigation notes.
If intelligence is unavailable, proceed with manual reference repo inspection as before.
```

**Implementation checklist**:
- [ ] Modify Phase 1 in `.claude/skills/fix-bug/SKILL.md` to include intelligence query
- [ ] Extract bug keywords from the bug entry title/repro for the search query
- [ ] Map bug subsystem to relevant reference repos (ARC bugs → rust,swift,koka,lean4)
- [ ] Record intelligence findings in the fix section file's investigation notes
- [ ] Test: intelligence available → query results inform investigation
- [ ] Test: intelligence unavailable → falls back to manual reference repo check

### Subsection 03.2 close-out

**`/improve-tooling` retrospective**: Did the intelligence query actually help find relevant prior art? Was the keyword extraction from bug descriptions effective? Any query patterns that should be added as presets?

---

## 03.R Third Party Review Findings

- None.

## Completion Checklist

- [ ] `/tpr-review` SKILL.md has Step 0.75 with intelligence pre-query
- [ ] `/fix-bug` SKILL.md has Phase 1 Step 5 with intelligence query
- [ ] Both use `scripts/intel-query.sh` exclusively
- [ ] Both degrade gracefully
- [ ] Evidence packet size bounded at 500 chars
- [ ] No test regressions: `timeout 150 ./test-all.sh`
- [ ] `/tpr-review` clean
- [ ] `/impl-hygiene-review` clean
- [ ] `/improve-tooling` section-close sweep
