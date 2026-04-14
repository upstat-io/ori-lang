---
name: review-plan
description: Review and improve a plan for accuracy, correctness, feasibility, strategic cohesion, executability, and testing rigor — expand to fulfill the mission, never scope down. Runs blind spot analysis, a merged 4-lens Opus editing agent, then /tpr-review (using review-plan skill) until clean.
allowed-tools: Read, Grep, Glob, Agent, AskUserQuestion, Bash, Edit, Write, LSP, Skill
---

# Review Plan Command

Review and improve a plan using a **4-phase pipeline**: mechanical pre-check, blind spot analysis, a merged 4-lens Opus editing agent, and adversarial `/tpr-review` convergence using the plan-specific reviewer skill.

**Design rationale:** Plans are upstream of code — a flawed plan multiplies into flawed code across every section. The Opus agent does the volume work (all 4 review lenses merged into one pass), then `/tpr-review` applies adversarial pressure with the reviewer-side `review-plan` skill — which understands mission criteria, cross-section coherence, and executability rather than code correctness.

## Reviewed Field Semantics — CRITICAL

The `reviewed: true/false` field in section frontmatter is a **pre-implementation gate** — it tracks whether a section has been validated against the current codebase right before implementation begins.

**Two modes — the mode determines whether `reviewed` gets flipped:**

**Single-section review** (`/review-plan plans/foo/section-03.md`):
This is the pre-implementation gate. After the FULL pipeline completes — editing agent AND `/tpr-review` clean pass — flip `reviewed: true` in a final step. Do NOT flip it inside the editing agent. Exception: if issues remain that could NOT be resolved (requiring human judgement), leave `reviewed: false`.

**Whole-plan review** (`/review-plan plans/foo/`):
Improves quality across all sections, but does **NOT** change any `reviewed` values. Fix content issues, but leave every section's `reviewed` field as-is — including missing fields (do not add `reviewed: false` to sections that lack the field).

## Usage

```
/review-plan <plan-path>
```

- `plan-path`: **Required.** Path to the plan directory or a specific plan file.
  - If a directory: whole-plan review mode
  - If a single file: single-section review mode (reads sibling files for context)

---

## Workflow

### Step 0: Read CLAUDE.md (ABSOLUTE FIRST — NO EXCEPTIONS)

**Before doing ANYTHING else**, read the ENTIRE CLAUDE.md file:

```
Read file: CLAUDE.md
```

### Step 1: Determine Review Mode and Normalize Paths

Inspect `$ARGUMENTS`:

- **Single-file input** (e.g. `plans/foo/section-03.md`):
  - Mode: **single-section**
  - `{plan_dir}` = parent directory (e.g. `plans/foo/`)
  - `{target_section}` = the specific file (e.g. `section-03.md`)
  - Agent scope: read ALL sibling files for context, but edits are limited to `{target_section}` only unless structural necessity requires touching `00-overview.md` or `index.md`
  - `reviewed` flip: happens AFTER Phase 4 (tpr-review) clean pass — NOT inside the agent

- **Directory input** (e.g. `plans/foo/`):
  - Mode: **whole-plan**
  - `{plan_dir}` = the directory
  - `{target_section}` = N/A (all sections)
  - Agent scope: read and edit ALL files in `{plan_dir}/`
  - `reviewed` flip: NEVER — whole-plan review does not touch `reviewed` fields

If the path doesn't exist, report the error and stop.

### Step 2: Plan-Wide Accuracy Pre-Check

**Only the heuristic/semantic check** — the mechanical STATUS_DRIFT check is handled by Phase 1 (`plan-audit.py`).

Read every section file's frontmatter and identify **"effectively complete" sections**: sections where all own implementation work is done but marked `in-progress` because of external blockers. If a section's remaining unchecked items are ALL blocked by external issues (not the section's own work), fix the status to `complete` with a blocker note.

Report to the user: "Pre-check: fixed N effectively-complete sections. Running Phase 1..."

### Step 3: Phase 1 — Static Analysis via `plan-audit.py`

Run the mechanical audit script. Auto-fixes deterministic metadata drift and produces a structured JSON packet.

```bash
python3 .claude/skills/plan-audit/plan-audit.py {plan_dir} --fix-safe --apply --json > /tmp/plan-audit-output.json 2>/tmp/plan-audit-fixes.log
```

Read both outputs:
```
Read file: /tmp/plan-audit-fixes.log
Read file: /tmp/plan-audit-output.json
```

**Capture the following for Phase 3 handoff:**
- Total findings: N critical, M major, K minor
- Auto-fixed: list what was corrected
- Remaining findings (not auto-fixable): verbatim list with location and message — these are passed to the agent

Report to the user: "Phase 1: N findings (X critical, Y major, Z minor), M auto-fixed. Running intelligence pre-query..."

### Step 3.5: CONDITIONAL — Intelligence pre-query

If the intelligence graph is available (`scripts/intel-query.sh status` returns `"ok"`):
1. Extract 3-5 high-signal file paths and symbol names from the plan's checklist items.
2. Run `scripts/intel-query.sh --human file-symbols "<path>" --repo ori` for each.
3. Run `scripts/intel-query.sh --human callers "<symbol>" --repo ori` and `callees` for key symbols.
4. Run `scripts/intel-query.sh --human similar "<symbol>" --repo rust,swift,go --limit 5` for cross-repo context.
5. Pass results into subsequent steps as intelligence context.

If unavailable or empty, skip silently and proceed.

Report to the user: "Intelligence pre-query complete. Running /tp-help..."

### Step 4: Phase 2 — `/tp-help` Blind Spot Analysis

Invoke `/tp-help` and wait for it to complete before proceeding:

```
Skill: tp-help
```

Provide a prompt that includes:
- The plan's mission/goal (from overview)
- The section list with their goals and statuses
- A brief summary of the plan's scope (which crates, which subsystems)
- Whether this is a single-section or whole-plan review

Ask specifically:
- "Given this plan's scope, what are the most likely failure modes the review should watch for?"
- "What architectural risks or blind spots would you flag?"
- "Are there cross-cutting concerns that might fall between section boundaries?"

**Capture the following for Phase 3 handoff:**
- Key blind spots identified (bullet list, ≤10 items)
- Architectural risks flagged
- Cross-cutting concerns

Report to the user: "Phase 2 complete. Running editing agent..."

### Step 5: Phase 3 — Editing Agent (All 4 Lenses Merged)

Spawn an agent with full edit authority. This agent merges the scope of the original 4 sequential review agents into one pass.

**IMPORTANT**: Use `model: "opus"`. Do NOT flip `reviewed` fields inside this agent — that happens after Phase 4.

```
Agent (model: opus):

You are reviewing a plan for the Ori compiler at {plan_dir}/.

**Review mode: {single-section: edit {target_section} only (read siblings for context) | whole-plan: edit all files}**

You have FULL AUTHORITY to make ANY structural change within your edit scope:
- **Add new sections** if coverage gaps exist
- **Remove sections** that are redundant or misguided
- **Merge sections** that are artificially split
- **Split sections** that try to do too much (especially 20+ checklist items)
- **Reorder sections** if the dependency flow is wrong
- **Rewrite the overview and index** to match structural changes
- **Restructure the entire plan** if the current organization doesn't serve the mission
- **Rewrite checklist items** that are vague, wrong, or missing the point
- **Change section boundaries** — move items between sections if they belong elsewhere

The plan exists to serve the mission. If the structure fights the mission, change the structure. Never scope down.

**DO NOT touch `reviewed` fields** — those are handled by a separate final step.

## CRITICAL PREREQUISITE: Read CLAUDE.md (every word)

```
Read file: CLAUDE.md
```

Then load the hygiene and testing rules:
```
Read file: .claude/rules/impl-hygiene.md
Read file: .claude/rules/compiler.md
Read file: .claude/rules/tests.md
```

## Context from Prior Phases

### Phase 1 — plan-audit.py remaining findings (trust these — deterministic):
{Paste verbatim: location + message for each remaining finding not auto-fixed}

### Phase 2 — /tp-help blind spots:
{Paste: bullet list of blind spots, risks, cross-cutting concerns}

---

## Part 1: Technical Accuracy & Feasibility

1. Read ALL files in {plan_dir}/
2. Cross-reference every technical claim against the actual codebase:
   - Do referenced files, types, functions, modules exist?
   - Are crate dependency assumptions correct? (`ori_lexer → ori_parse → ori_ir → ori_types → ori_eval → ori_llvm → oric`)
   - Are described code patterns accurate?
3. Check claims against the spec in `docs/ori_lang/v2026/spec/` (`grammar.ebnf`, `operator-rules.md`, clause files)
4. For every inaccuracy found, EDIT the plan files directly to fix them
5. For each section, assess whether the described implementation approach will actually work:
   - Can each checklist item be implemented as described?
   - Are there hidden prerequisites or dependencies not mentioned?
   - Does the approach handle the full problem space, or only a subset?
   - Are there architectural constraints (file size limits, phase boundaries, crate deps) that would block the approach?
6. If a step is infeasible:
   - Do NOT remove it or mark it as "future work"
   - EXPAND the approach: add prerequisite steps, restructure the section, or add a new section that addresses the blocker
7. Structural assessment — step back and assess the plan AS A WHOLE:
   - Is this the right set of sections? Would a different decomposition serve the mission better?
   - Are sections at the right granularity?
   - Does the section ordering reflect actual implementation dependencies?
   - If you see a better structure, IMPLEMENT IT — don't just note it

Add a brief comment near each fix: `<!-- reviewed: accuracy/feasibility fix -->`

---

## Part 2: Mission Fulfillment & Strategic Cohesion

8. Identify the plan's stated mission/goal and mission success criteria (from `00-overview.md`)
9. Verify the success criteria hierarchy:
   - Does `00-overview.md` have a "Mission Success Criteria" section with concrete, testable criteria?
   - Does every section have `success_criteria` in its frontmatter AND a "Success Criteria" block in its body?
   - Does every mission criterion trace to at least one section that delivers it?
   - Does every section criterion connect upward to at least one mission criterion?
   - Are all criteria concrete and testable — not "X works" but "X produces Y when Z is run"?
   - If missing or vague, ADD them. A section without success criteria is not executable.
10. For each aspect of the mission, verify there is at least one section that addresses it. If the plan ends at 70% of the mission, add sections for the remaining 30%.
11. Blocker resolution — verify the plan identifies and resolves ALL blockers between the current codebase state and the mission's goals:
    - Are there UNIDENTIFIED blockers? Search the codebase, roadmap, bug-tracker, and other plans.
    - If a blocker is tracked elsewhere, this plan must include resolving it with cross-links (`<!-- resolves: plans/... -->`)
12. Verify that sections can be worked in order (section N before section N+1):
    - Does each section's output provide what the next needs?
    - Are there circular dependencies? (Resolve by reordering or splitting)
13. Flag and fix deferral traps: "bonus", "future", "lower priority", "nice to have", "stretch goal", "requires architectural change" → concrete mandatory tasks or explicit `<!-- blocked-by:X -->`. Remove all soft deferral language.

Add a brief comment near each addition: `<!-- reviewed: cohesion fix -->`

---

## Part 3: Section Executability & Codebase Hygiene

14. For each section, assess executability — could an implementer sit down and work through every checklist item in order?
    - Is each checklist item a concrete, verifiable task (WHAT + WHERE)?
    - Are there hidden steps between checklist items?
    - Would an implementer need to make design decisions not covered by the plan?
15. For vague or under-specified items, EXPAND them:
    - Break into specific sub-items with file paths and approach
    - Add "WHERE:" annotations when the location isn't obvious
16. If a section is too thin (fewer than 3 substantive items), expand it — research the codebase to add concrete items.
17. If a section is too large (20+ items, or mixes unrelated concerns), SPLIT IT.
18. Reorder items within sections if they violate crate dependency ordering.
19. **Rules weaving** — CLAUDE.md and `.claude/rules/*.md` constraints must be embedded organically in checklist items, not assumed:
    - "Add variant X, update match arms at `file.rs:123` and `other.rs:456`" NOT "Add variant (remember sync points)"
    - Key rules: TDD discipline (tests.md), file size limits, crate ordering, registration sync, ARC invariants, phase boundaries, test conventions
    - If a section touches a subsystem but doesn't embed its rules, ADD the relevant constraints inline
20. **Codebase scan** — extract from the plan every file path, crate, and module that will be touched. READ those files (up to 30 files; prioritize files mentioned in multiple sections). Look for issues the plan should address:
    - **BLOAT**: Files over 500 lines the plan will touch but doesn't plan to split
    - **WASTE**: Dead code, stale comments, unnecessary clones in touched files
    - **DRIFT**: Registration sync points already out of sync
    - **EXPOSURE/LEAK**: Phase bleeding in files the plan modifies
    - NOTE: Do NOT re-verify file existence or line-level accuracy — Phase 1 already covered DEAD_PATH deterministically. Focus on architectural issues requiring judgment.
21. Weave "fix along the way" checklist items for real findings (file:line required — no fabrication):
    - `- [ ] **[BLOAT]** file:line — Split into submodules (currently N lines)`
    - `- [ ] **[DRIFT]** file:line — Sync missing variant at other_file:line`
    - Group under "Cleanup" sub-heading if 3+ findings per section.

Add a brief comment near each change: `<!-- reviewed: executability/hygiene fix -->`

---

## Part 4: Testing Rigor & Final Integration

22. For EVERY section that modifies compiler code, verify it has a test strategy meeting CLAUDE.md requirements:
    - **Matrix tests**: type × pattern dimensions explicitly named
    - Semantic pin: at least one test that ONLY passes with the new semantics
    - TDD ordering: failing tests FIRST, debug+release verification LAST
    - If missing → ADD concrete test checklist items:
      - `- [ ] Write failing test matrix BEFORE implementation` (FIRST item)
      - `- [ ] Verify all tests pass in both debug and release` (LAST item)
      - `- [ ] Add semantic pin test that only passes with new behavior`
23. Review for clarity and internal consistency:
    - Is terminology consistent across sections?
    - Does the overview accurately reflect the section contents?
    - Are there contradictions between sections?
    - Fix inconsistencies and update overview/index to reflect current structure.
24. Remove all `<!-- reviewed: ... -->` comments left during prior parts of this review.
25. **Plan-sync line items** — verify every section's completion checklist includes:
    - Section frontmatter status update
    - `00-overview.md` Quick Reference table and mission success criteria checkboxes
    - `index.md` section status
    - Cross-links to other plans (if section resolves external blockers)
    - Next section's `depends_on` verification
    - If missing → ADD from the template in `plan-schema.md`
26. **Final coherence check**: read through the entire plan one more time. Does it tell a complete, sequential story? Is this the RIGHT plan for the mission?

---

## Critical Rules

- NEVER scope down — always expand. "Requires architectural change" is not a reason to defer — it IS the work.
- No deferral traps — every checkbox must be implementable.
- Be specific — every change needs evidence: a spec clause, a file:line, or concrete reasoning.
- Cross-reference, don't guess — read spec files and source code.
- Do NOT dismiss TPR findings as "unrelated" or "out of scope" — per CLAUDE.md there is no such thing. Only reject findings that are factually incorrect.
- Testing rigor is non-negotiable — matrix tests, semantic pins, TDD ordering, debug+release.
- Success criteria mandatory at both mission and section levels, connected bidirectionally.
- Rules woven in, not assumed — plans are self-contained execution documents.
- Plan-sync on section completion — frontmatter, overview, index, cross-links, next section's depends_on.
- **DO NOT touch `reviewed` fields** — handled by a separate final step after tpr-review.

After editing, list what you changed and why.
```

Read the agent's output. Note what changes were made.

### Step 6: Phase 4 — Run `/tpr-review` with `review-plan` Skill (MANDATORY)

**CRITICAL: Run the actual `/tpr-review` skill using the Skill tool with plan-review context.** Do NOT reimplement the review logic. The reviewers will use their `review-plan` skill (not `review-work`) which is specifically designed for plan analysis — mission criteria, cross-section coherence, executability.

Pass `--skill review-plan` so that `/tpr-review` uses the plan-specific reviewer preambles and transport label:

```
Skill: tpr-review
Args: --skill review-plan
```

`/tpr-review` will:
- Use `review-plan` activation preambles (codex: `Run the /review-plan skill in envelope-only mode.`, gemini: `Activate the review-plan skill and follow its instructions exactly.`)
- Pass `--skill review-plan` to the transport (correct `round.log` attribution)
- Launch Codex and Gemini in parallel using the `review-plan` reviewer skill
- Merge findings from both reviewers
- Fix actionable findings directly
- Re-run until both reviewers return zero actionable findings (max 10 iterations)

Wait for `/tpr-review` to complete. Capture for the verdict:
- Iterations to converge (or "max reached")
- Per-iteration finding counts

### Step 7: Flip `reviewed` Field (Single-Section Mode Only)

**Only in single-section mode.** After Phase 4 returns a clean pass:

- Set `reviewed: true` in `{target_section}`'s frontmatter
- Exception: if Phase 4 surfaced unfixable issues requiring human judgement, leave `reviewed: false` and report to user

**In whole-plan mode: skip this step entirely.**

### Step 8: Post-Edit Verification (Loop Until Clean)

Run the audit script and loop until no critical findings remain:

```bash
python3 .claude/skills/plan-audit/plan-audit.py {plan_dir} --verify --json > /tmp/plan-audit-verify.json 2>&1
```

Read the results. If critical or major findings remain:
1. Fix them
2. Re-run the audit
3. Repeat until the result is clean (zero critical, zero major)

### Step 8.5: Cross-Plan Review Invalidation

**When to run:** Only when the review made significant changes — changes that alter which files, types, or subsystems the plan's sections reference. Skip if only cosmetic/formatting changes.

#### 8.5a: Run invalidation detection

```bash
python3 .claude/skills/plan-audit/plan-invalidate.py {plan_dir} --json > /tmp/plan-invalidate-output.json
```

Read output. If `status` is `"clean"`, skip to Step 9.

#### 8.5b: Present findings to user

If stale sections are found, present via `AskUserQuestion`:

> **Cross-plan review invalidation detected.**
>
> This plan review changed scope that overlaps with **N reviewed sections** across **M other plans**.
>
> **High-impact overlaps** (weight ≥ 4): {list}
> **Lower-impact overlaps** (weight 2-3): {list}
>
> Options:
> 1. **Apply all** — invalidate all N sections
> 2. **Apply high-impact only** — invalidate only weight ≥ 4
> 3. **Skip** — leave reviews as-is

If approved:

```bash
python3 .claude/skills/plan-audit/plan-invalidate.py {plan_dir} --apply [--min-weight 4]
```

### Step 9: Present Verdict

```
## Plan Review: {plan name}

### Pipeline Summary
- **Pre-check**: {N effectively-complete sections corrected}
- **Phase 1** (plan-audit.py): {N} findings, {M} auto-fixed; {K} remaining passed to agent
- **Phase 2** (/tp-help): {N} blind spots identified
- **Phase 3** (Opus agent):
  - Structural changes: {sections added/removed/merged/split/reordered}
  - Accuracy fixes: {N}
  - Cohesion fixes: {N}
  - Hygiene items woven: {N by category}
  - Test strategy gaps filled: {N}
- **Phase 4** (/tpr-review with review-plan skill): {clean on iteration N | max reached with N remaining}
  - Iteration 1: {N} findings
  - Iteration 2: {N} findings (if applicable)
- **Post-edit verification**: {CLEAN | N remaining findings after fixes}

### Review Status
| Section | `reviewed` Before | `reviewed` After | Reason |
|---------|------------------|-----------------|--------|
| ... | ... | ... | ... |

### Cross-Plan Invalidation
{Results or "Skipped — changes were cosmetic."}

### Remaining Concerns
{Issues requiring human judgement, ranked Critical > Major > Minor}

---

## Verdict

**{CLEAN | MINOR FIXES APPLIED | SIGNIFICANT REWORK APPLIED | RESTRUCTURED | NEEDS MANUAL ATTENTION}**

{2-3 sentence assessment. Total edits across all phases.}
```

**Verdict definitions:**
- **CLEAN**: No issues found. Plan is ready for implementation.
- **MINOR FIXES APPLIED**: Small corrections. Plan is ready.
- **SIGNIFICANT REWORK APPLIED**: Substantial edits (reordered steps, added missing sections, fixed incorrect assumptions). Review the diff before proceeding.
- **RESTRUCTURED**: Plan structure fundamentally changed. Review the new structure before proceeding.
- **NEEDS MANUAL ATTENTION**: Issues requiring human judgement. Cannot be auto-fixed.

---

## Important Rules

1. **`/tpr-review` is MANDATORY** — every plan review runs it. No exceptions. Uses `review-plan` reviewer skill.
2. **Skill invocations are self-contained** — `Skill: tp-help` and `Skill: tpr-review` manage their own transport, polling, and convergence. Do NOT add foreground/background/polling directives around them.
3. **Agent edits directly** — this is not a report-only review. The agent fixes what it finds.
4. **Sequential phases** — each phase completes before the next starts. Phases feed each other via captured outputs.
5. **`reviewed` flip is LAST** — only in single-section mode, only after tpr-review clean pass, never inside the agent.
6. **Whole-plan mode never touches `reviewed` fields** — not even to add missing ones.
7. **Be specific** — every change needs evidence: a spec clause, a file:line, or concrete reasoning.
8. **NEVER scope down — always expand** — grow the plan if it doesn't fulfill its mission.
9. **No deferral traps** — "bonus", "future", "lower priority" → concrete mandatory tasks or `<!-- blocked-by:X -->`.
10. **Testing rigor is non-negotiable** — matrix tests (type × pattern dimensions), semantic pins, TDD ordering, debug+release.
11. **Success criteria mandatory** at both mission and section levels, connected bidirectionally.
12. **Rules woven in, not assumed** — plans are self-contained execution documents.
13. **Plan-sync on section completion** — frontmatter, overview, index, cross-links, next section's depends_on.
14. **No dismissing findings as "unrelated"** — per CLAUDE.md there is no "unrelated", "pre-existing", or "out of scope". Only reject findings that are factually incorrect.
