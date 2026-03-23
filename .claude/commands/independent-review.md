---
name: independent-review
description: Deep code review of recent changes with minimal context to avoid biases — sequential cold-start agents, each expanding on prior findings, with full-context final triage.
allowed-tools: Read, Grep, Glob, Agent, AskUserQuestion, Bash
---

# Independent Review Command

Perform an unbiased code review via a 4-agent sequential pipeline. Each agent starts cold (no project context, no conversation history) and receives only the raw materials needed to do its job. Findings flow forward — each agent sees the previous agent's output and expands the search. The main context (which has full project knowledge) performs the final triage, filtering out false positives and writing validated findings into the plan's Third Party Review section.

## Why Sequential, Not Parallel

Parallel agents find the same surface issues independently. Sequential agents **build on each other**:
- Agent 1 finds the obvious issues cold
- Agent 2 validates those AND searches deeper, now knowing where to look
- Agent 3 validates the accumulated set AND widens scope further
- Agent 4 does a final cold pass with the full finding set, verifying and adding anything missed

Each agent's search radius is strictly larger than the previous one's.

## Usage

```
/independent-review [plan-path] [commit-range]
```

- `plan-path`: Optional. Path to the plan directory (e.g., `plans/aims/`). If omitted, uses the most recently modified plan directory under `plans/`.
- `commit-range`: Optional. Git revision range (e.g., `HEAD~5..HEAD`, `abc123..def456`). If omitted, auto-detects: all commits on the current branch since diverging from `master`.

---

## Workflow

### Step 1: Gather Raw Materials

Collect inputs mechanically. Do NOT summarize or editorialize.

#### 1a. Determine commit range

```bash
# If commit-range provided, use it directly
# Otherwise, find the merge base with master:
MERGE_BASE=$(git merge-base master HEAD)
# Range is: $MERGE_BASE..HEAD
```

#### 1b. Extract raw data

```bash
git diff $RANGE --stat          # file-level summary
git log $RANGE --oneline        # commit list (for scope, not rationale)
```

Save the `--stat` output as `$DIFF_STAT` and the log as `$COMMIT_LOG`.

#### 1c. Extract plan materials (stripped)

If a plan path is available, read the plan files and extract ONLY:
- The plan's **mission statement** (the 1-2 sentence Mission from `00-overview.md`)
- The plan's **completion criteria / exit criteria** (from each section's Completion Checklist and Exit Criteria)
- Section titles and checklist items (the `- [ ]` / `- [x]` lines)
- File paths mentioned in checklist items

**Strip ALL prose, motivation paragraphs, design rationale, and explanatory text.** The agents see "what it's supposed to do" and "what success looks like" — never "why."

Save this as `$PLAN_STRIPPED`.

#### 1d. Report scope to user

```
Independent Review: {N} commits, {M} files changed
Plan: {plan name} ({L} checklist items, {K} completion criteria)
Pipeline: 4 sequential agents (cold start) -> full-context triage
```

---

### Step 2: Agent 1 — Cold Discovery

**Launch as Agent** (not background — we need results before Agent 2).

This agent knows NOTHING about the project beyond what's in the prompt. It receives only the mission statement, completion criteria, and the diff stat. It reads the actual code files to form its own conclusions.

```
You are performing a code review. You have ZERO context about this project, its history, or
why these changes were made. You are reviewing a Rust project. That's all you know going in.

## Mission Statement

{$PLAN_STRIPPED mission statement — the 1-2 sentence "what this plan accomplishes"}

## Success Criteria

{$PLAN_STRIPPED completion criteria / exit criteria}

## What Changed

{$DIFF_STAT}

## Commits

{$COMMIT_LOG}

## Your Instructions

1. For EVERY changed file in the diff stat above, READ THE FULL FILE (use the Read tool).
   Do not skip any file. Do not read only the diff — read the whole file for context.

2. For each file, evaluate against these dimensions:

   **Correctness**
   - Is the logic correct? Trace edge cases mentally.
   - Off-by-one errors, missing bounds checks, unhandled match arms?
   - Error paths handled? Could this panic unexpectedly?
   - Integer overflow/underflow/truncation risks?
   - Empty inputs, None values, zero-length collections handled?

   **Safety**
   - Any unsafe blocks — are they sound?
   - Memory: use-after-free, double-free, dangling references?
   - Resource leaks on error paths?
   - Unbounded growth (Vec/HashMap/String without limits)?
   - ARC/RC balance — increments and decrements matched on all paths?
   - FFI: pointer casts correct? Null checks present? ABI correct?

   **Consistency**
   - Does new code match patterns in the surrounding file?
   - Read 1-2 sibling files — does it match the directory's conventions?
   - Naming, error handling, abstraction level consistent?

   **Test Quality**
   - For each changed source file: is there a corresponding test?
   - Do tests actually exercise the changed code? (trace the call path)
   - Are assertions checking the right thing, or just "doesn't panic"?
   - Edge cases and error paths tested?
   - Could any test pass even if the change were reverted?

3. Evaluate against the mission statement and success criteria:
   - Does the code achieve what the mission describes?
   - Are the success criteria met by what you see in the code?
   - Any gaps between stated goals and actual implementation?

## Output Format

For each finding:
```
**[{CATEGORY}]** `file:line` — severity: {critical|major|minor}
Description of the issue.
What could go wrong in practice.
```

Categories: CORRECTNESS, SAFETY, CONSISTENCY, TEST, GOAL-GAP

For clean files:
```
`file` — CLEAN (reviewed N regions, M lines)
```

End with:
```
## Summary
- Files reviewed: N
- Findings: X critical, Y major, Z minor
- Goal alignment: {ALIGNED | PARTIAL | MISALIGNED}
- Test coverage: {STRONG | ADEQUATE | WEAK | MISSING}
```
```

---

### Step 3: Agent 2 — Verify and Expand

**Wait for Agent 1 to complete.** Then launch Agent 2 with Agent 1's findings.

Agent 2 receives everything Agent 1 received PLUS Agent 1's findings. Its job: validate each finding (confirm or reject) and search deeper in the areas where Agent 1 found issues.

```
You are the second reviewer in a sequential code review pipeline. A previous reviewer
(who had no project context) examined this Rust project and produced findings. Your job is:

1. VERIFY each finding — read the code yourself and confirm or reject it
2. EXPAND — in areas where findings were found, search deeper and wider
3. FIND NEW — look for issues the first reviewer missed entirely

You also have ZERO project context beyond what's provided here.

## Mission Statement

{$PLAN_STRIPPED mission statement}

## Success Criteria

{$PLAN_STRIPPED completion criteria / exit criteria}

## What Changed

{$DIFF_STAT}

## Commits

{$COMMIT_LOG}

## Previous Reviewer's Findings

{Agent 1's full output}

## Your Instructions

### Phase A: Verify Previous Findings

For EACH finding from the previous reviewer:
1. READ THE FILE yourself (do not trust the previous reviewer's reading)
2. Confirm or reject:
   - **CONFIRMED** — you independently see the same issue
   - **REJECTED** — the finding is incorrect (explain why: the code is actually correct because...)
   - **ESCALATED** — the finding is real AND worse than stated (explain the additional impact)
   - **DOWNGRADED** — the finding is real but less severe than stated (explain why)

### Phase B: Expand Search

For each file/area where findings were CONFIRMED or ESCALATED:
1. Read additional files in the same module/directory
2. Check if the same pattern (the bug, the inconsistency, the safety issue) appears elsewhere
3. Check callers and callees of the affected functions — does the issue propagate?
4. Check if related test files adequately cover the confirmed issues

### Phase C: New Discoveries

Independently review any files you haven't examined yet. The first reviewer may have
focused on the most obvious files. Check:
- Configuration files, build files, test infrastructure
- Files that import/use the changed files (downstream impact)
- Files that the changed files import (upstream assumptions)

## Output Format

### Verification Results
For each previous finding:
```
**[{PREV-ID}]** `file:line` — {CONFIRMED|REJECTED|ESCALATED|DOWNGRADED}
{Your independent assessment. If rejected, explain why.}
```

### New Findings
```
**[{CATEGORY}]** `file:line` — severity: {critical|major|minor}
Description.
```

### Summary
- Previous findings verified: N confirmed, M rejected, P escalated, Q downgraded
- New findings: X critical, Y major, Z minor
- Expanded search covered: N additional files
```

---

### Step 4: Agent 3 — Deep Verification

**Wait for Agent 2 to complete.** Then launch Agent 3 with the accumulated findings.

Agent 3 receives everything previous agents received PLUS both sets of findings. Its focus: the confirmed findings are real, but are they complete? Are there systemic patterns? What's the blast radius?

```
You are the third reviewer in a sequential code review pipeline. Two previous reviewers
have examined this Rust project. Findings have been confirmed or rejected. Your job is:

1. VALIDATE the confirmed findings one more time (triple-check)
2. ASSESS SYSTEMIC RISK — are confirmed findings isolated or symptomatic of deeper issues?
3. CHECK BLAST RADIUS — for each confirmed issue, how far does the impact reach?
4. FINAL SWEEP — anything everyone missed?

You have ZERO project context beyond what's provided.

## Mission Statement

{$PLAN_STRIPPED mission statement}

## Success Criteria

{$PLAN_STRIPPED completion criteria / exit criteria}

## What Changed

{$DIFF_STAT}

## Accumulated Findings (from 2 previous reviewers)

{Agent 2's full output, which includes Agent 1's findings and verification results}

## Your Instructions

### Phase A: Triple-Check Confirmed Findings

For each CONFIRMED or ESCALATED finding:
1. Read the file yourself
2. If you ALSO confirm it: it's now HIGH CONFIDENCE (3/3 reviewers agree)
3. If you disagree: mark as DISPUTED and explain

### Phase B: Systemic Analysis

Look at the PATTERN of confirmed findings:
- Are multiple findings symptoms of one root cause?
- Is there an architectural issue that the individual findings are pointing at?
- Group related findings and identify if there's a deeper structural problem

### Phase C: Blast Radius

For each high-confidence finding:
1. Trace callers: who calls this code? Are they affected?
2. Trace data flow: does bad data propagate to other subsystems?
3. Check production paths: is this code exercised in normal operation, or only edge cases?
4. Estimate user impact: would this cause a crash? Wrong output? Silent corruption?

### Phase D: Final Sweep

Scan the FULL diff one more time with fresh eyes. Previous reviewers may have
anchored on certain files. Look at:
- The smallest changes (1-2 line diffs are often the most dangerous)
- Deleted code — was anything removed that shouldn't have been?
- Changes to public APIs — are they backward compatible?
- Changes to error messages — do they still make sense?

## Output Format

### High-Confidence Findings (confirmed by 3 reviewers)
```
**[HC-{N}]** `file:line` — severity: {critical|major|minor}
{Description}
Blast radius: {who/what is affected}
Root cause: {if part of a systemic pattern}
```

### Disputed Findings
```
**[DISPUTED-{N}]** `file:line`
Previous assessment: {what prior reviewers said}
My assessment: {why I disagree}
```

### Systemic Patterns
```
**[SYSTEMIC-{N}]** — severity: {critical|major|minor}
{Description of the pattern}
Individual findings: {list of related finding IDs}
Root cause: {the underlying issue}
```

### New Findings (final sweep)
```
**[{CATEGORY}]** `file:line` — severity: {critical|major|minor}
Description.
```

### Summary
- High-confidence findings: N (confirmed by all 3 reviewers)
- Disputed findings: M
- Systemic patterns identified: P
- New findings from final sweep: Q
- Overall risk assessment: {LOW | MODERATE | HIGH | CRITICAL}
```

---

### Step 5: Agent 4 — Final Cold Consolidation

**Wait for Agent 3 to complete.** Then launch Agent 4.

Agent 4 is the consolidator. It receives the complete finding chain and produces the final structured report. It does NOT add new findings — it organizes, de-duplicates, and produces the clean output that the main context will triage.

```
You are the final consolidator in a 4-agent code review pipeline. Three previous reviewers
have examined a Rust project, verified each other's findings, and assessed systemic risk.

Your job is NOT to find new issues. Your job is to produce a clean, organized, actionable
final report from the accumulated findings.

## Mission Statement

{$PLAN_STRIPPED mission statement}

## Success Criteria

{$PLAN_STRIPPED completion criteria / exit criteria}

## Plan Checklist

{$PLAN_STRIPPED checklist items}

## Full Finding Chain

{Agent 3's full output, which includes all previous findings}

## Your Instructions

1. **De-duplicate**: Merge findings that describe the same issue from different angles.
   Note how many reviewers independently flagged each issue.

2. **Classify**: Assign final severity based on reviewer consensus:
   - 3/3 confirmed + critical = CRITICAL (must fix)
   - 3/3 confirmed + major = MAJOR (should fix)
   - 2/3 confirmed = use higher severity reviewer's assessment
   - 1/3 only (not rejected by others) = as-stated but flagged as lower confidence
   - DISPUTED = include both sides, let the project maintainer decide

3. **Plan Fidelity**: Compare the checklist items against the actual code changes:
   - For each checklist item: DONE / PARTIAL / MISSING / SKIPPED
   - For each changed file not in checklist: UNPLANNED (note if it seems related)
   - Fidelity score: HIGH (>90% done, few unplanned) / MEDIUM / LOW

4. **Produce the final report** in this exact format:

```
## Independent Review Report

**Scope**: {N} commits, {M} files changed
**Reviewers**: 4 sequential agents (cold start pipeline)
**Confidence**: {how many findings survived all 3 verification passes}

---

### CRITICAL Findings
{Each finding: ID, file:line, description, blast radius, reviewer consensus (e.g., 3/3)}
{Empty section = none found}

### MAJOR Findings
{Same format}

### MINOR Findings
{Same format}

### Systemic Patterns
{From Agent 3's systemic analysis}

### Disputed Findings
{Both sides presented — for maintainer to decide}

### Plan Fidelity
| # | Item (abbreviated) | Status | Notes |
|---|-------------------|--------|-------|
{Table}

Fidelity: {HIGH|MEDIUM|LOW}
Unplanned changes: {list}

### Test Assessment
Coverage: {STRONG|ADEQUATE|WEAK|MISSING}
Gaps: {specific untested scenarios}

---

### Statistics
- Total findings surfaced across pipeline: {N}
- Survived to final report: {M} ({X} critical, {Y} major, {Z} minor)
- Rejected during pipeline: {P}
- Disputed: {Q}
```
```

---

### Step 6: Full-Context Triage (Main Context)

**This step is performed by YOU (the main context), NOT by an agent.** You have full project knowledge — CLAUDE.md, memory, conversation history, codebase familiarity — that the cold agents did not have.

#### 6a. Read Agent 4's Final Report

Read the consolidated report carefully.

#### 6b. Triage Each Finding

For each finding in the report, apply your full project context to determine:

- **VALID** — The finding is real and actionable. The cold reviewers correctly identified a problem.
- **FALSE POSITIVE** — The finding looks wrong in isolation but is actually correct given project context the reviewers didn't have. **You must explain WHY** — cite the specific context (spec clause, architectural decision, CLAUDE.md rule) that makes it a false positive.
- **ALREADY TRACKED** — The finding describes a known issue that's already in the plan or roadmap. Note where.
- **WON'T FIX** — The finding is technically correct but intentional / acceptable. Explain why.

**Bias check on yourself**: If you find yourself rejecting most findings, STOP and reconsider. The whole point of this command is to catch things you missed. A high rejection rate may mean you're rationalizing.

#### 6c. Present Triage to User

```
## Independent Review — Triage Results

**Pipeline produced**: {X} findings ({A} critical, {B} major, {C} minor)
**After full-context triage**: {Y} valid, {Z} false positives, {W} already tracked

### Valid Findings (will be written to plan TPR section)

{For each valid finding:}
**[TPR-{section}-{NNN}][{severity}]** `file:line` — Description
Reviewer consensus: {N}/3
Action: {what needs to happen}

### False Positives (rejected with explanation)

{For each false positive:}
**REJECTED**: `file:line` — {finding description}
**Reason**: {specific context that makes this a false positive}

### Already Tracked

{For each already-tracked finding:}
**KNOWN**: `file:line` — {finding description}
**Tracked at**: {plan section / roadmap item / issue}

### Plan Fidelity
{Pass through from Agent 4}

### Test Assessment
{Pass through from Agent 4}

---

## Verdict

**{PASS | PASS WITH CONCERNS | NEEDS WORK | FAIL}**

{2-3 sentence factual assessment. State the numbers.}
```

**Verdict definitions:**
- **PASS**: No valid critical findings. Few or no valid major findings. Tests adequate.
- **PASS WITH CONCERNS**: No valid critical findings, but valid major findings exist. Tests may have gaps.
- **NEEDS WORK**: Valid critical findings, or multiple valid major findings. Action required.
- **FAIL**: Multiple valid critical findings, or fundamental goal misalignment. Significant rework needed.

#### 6d. Write Findings to Plan

For each VALID finding, write it into the appropriate plan section's `## {NN}.R Third Party Review Findings` block using the standard TPR format:

```markdown
- [ ] `[TPR-{section}-{NNN}][{severity}]` `{file:line}` — {Description}.
```

Update the section's frontmatter:
- `third_party_review.status: findings`
- `third_party_review.updated: {today's date}`

This integrates the findings into the normal `/continue-roadmap` workflow — the TPR Triage Gate (Step 1.9) will ensure findings are addressed before new work begins on that section.

---

### Step 7: Verify Roadmap

**After all findings are written to the plan, run `/verify-roadmap`.**

This is mandatory — not optional. The independent review may have changed section statuses (TPR findings force sections back to `in-progress`), added new checklist items, or revealed plan/code drift. `/verify-roadmap` catches any inconsistencies introduced by the review itself and ensures the plan is in a clean, consistent state before anyone acts on the findings.

If `/verify-roadmap` surfaces additional issues (stale frontmatter, status mismatches), fix them before concluding.

---

## Important Rules

1. **Agents are sequential, not parallel.** Each agent MUST complete before the next launches. The finding chain is the point.
2. **Cold start is sacred.** Agents receive ONLY what's specified in their prompts. No CLAUDE.md, no memory, no conversation context, no "helpful" additions. More context = more bias.
3. **Mission + criteria, not rationale.** Agents see WHAT the code should accomplish and HOW to measure success. They never see WHY decisions were made. This is the key anti-bias mechanism.
4. **Findings flow forward.** Agent 1's output goes to Agent 2. Agent 2's goes to Agent 3. Agent 3's goes to Agent 4. Nothing is filtered between agents.
5. **Main context is the final arbiter.** Only the main context (Step 6) has full project knowledge to distinguish valid findings from false positives. Agents cannot do this — they don't have the context.
6. **Rejection requires explanation.** You cannot reject a finding without citing specific context the agents lacked. "It's fine" is not a rejection reason.
7. **Valid findings go into the plan.** This isn't a read-only report. Valid findings are written to the TPR section and enter the normal triage workflow.
8. **Bias self-check.** If you reject >60% of findings, explicitly acknowledge this and reconsider whether you're rationalizing.
