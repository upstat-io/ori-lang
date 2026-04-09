---
name: review-plan
description: Review and improve a plan for accuracy, correctness, feasibility, strategic cohesion, executability, and testing rigor — expand to fulfill the mission, never scope down. Uses a convergence loop to pressure-test solutions until they are systematic and cross-section-coherent.
allowed-tools: Read, Grep, Glob, Agent, AskUserQuestion, Bash, Edit, Write, LSP, Skill
---

# Review Plan Command

Review and improve a plan using a **4-tier pipeline** that combines cheap mechanical verification (Tiers 0–1), grounded architectural editing (Tier 2), and adversarial convergence review (Tier 3). The convergence loop runs dual-source reviewers (Codex + Gemini) against the Opus architect's output and iterates — fix findings, re-review — until both reviewers return clean.

**Design rationale:** Plans are upstream of code — a flawed plan multiplies into flawed code across every section. One-shot consultation (`/tp-help`) gives two first impressions; a convergence loop (`/tpr-review`-style) produces solutions that survive adversarial pressure from multiple angles across multiple rounds. Each iteration forces deeper analysis: round 1 catches obvious issues, round 2 tests the quality of your fixes, round 3+ filters out everything except systematic, holistic, cross-cutting solutions.

**Cross-section coherence is the primary review target.** Each section is a chapter in a story whose thesis is the plan's mission statement. The hardest failure mode is sections that are locally correct but globally incoherent — they solve their own problem without accounting for constraints from prior sections or obligations to downstream sections. The convergence loop specifically targets this: reviewers are grounded in the full plan narrative (mission + all completed sections) and must evaluate how each section fits the whole, not just whether it's internally sound.

## Reviewed Field Semantics — CRITICAL

The `reviewed: true/false` field in section frontmatter is a **pre-implementation gate** — it tracks whether a section has been validated against the current codebase right before implementation begins.

**Two modes — the mode determines whether `reviewed` gets flipped:**

**Single-section review** (`/review-plan plans/foo/section-03.md`):
This is the pre-implementation gate. After the pipeline completes (including convergence), flip `reviewed: true` — unless issues remain that could NOT be resolved (requiring human judgement), in which case leave `reviewed: false`.

**Whole-plan review** (`/review-plan plans/foo/`):
Improves quality across all sections, but does **NOT** change any `reviewed` values. Fix content issues, but leave every section's `reviewed` field as-is.

## Usage

```
/review-plan <plan-path>
```

- `plan-path`: **Required.** Path to the plan directory or a specific plan file.
  - If a directory: reviews all files in the directory
  - If a single file: reviews that file (and reads siblings for context)

## Workflow

### Step 0: Read CLAUDE.md (ABSOLUTE FIRST — NO EXCEPTIONS)

**Before doing ANYTHING else**, read the ENTIRE CLAUDE.md file:

```
Read file: CLAUDE.md
```

### Step 1: Tier 0 — Static Analysis via `plan-audit.py`

Run the mechanical audit script. This auto-fixes deterministic metadata drift and produces a structured JSON packet for downstream agents.

```
Bash:
  python3 .claude/skills/plan-audit/plan-audit.py {plan_dir} --fix-safe --apply --json > /tmp/plan-audit-output.json 2>/tmp/plan-audit-fixes.log
```

Read the fix log first to see what metadata was auto-corrected:
```
Read file: /tmp/plan-audit-fixes.log
```

Then read the JSON output:
```
Read file: /tmp/plan-audit-output.json
```

Report to the user:
- How many metadata fixes were applied
- Summary: "Tier 0 found N findings (X critical, Y major, Z minor)"
- Note: "Running Tier 1 semantic audit..."

### Step 2: Tier 1 — Sonnet Semantic Auditor (READ-ONLY)

Spawn a **Sonnet** agent that consumes the Tier 0 JSON packet and uses LSP to semantically verify plan references. This agent has **NO edit authority** — it produces findings only.

**IMPORTANT**: Use `model: "sonnet"` to ensure this runs on the cheaper model.

```
Agent (model: sonnet):
  You are a semantic auditor for Ori compiler plans. Your job is to verify plan accuracy using LSP tools and produce findings. You have NO permission to edit any plan files — you are read-only.

  CRITICAL PREREQUISITE: Read CLAUDE.md first (every word):
  ```
  Read file: CLAUDE.md
  ```

  Then read the Tier 0 audit results:
  ```
  Read file: /tmp/plan-audit-output.json
  ```

  ## Your Task

  Read all plan files in {plan_dir}/. Then perform these verification passes:

  ### Pass 1: Classify Tier 0 Findings
  Review each finding in the JSON. For DEAD_PATH findings, check if the path refers to a file that WILL be created by the plan (false positive) or is genuinely missing (true positive). For DEFERRAL_LANG findings, check if the context is actually deferral or just descriptive language. Output a classified list.

  ### Pass 2: LSP Semantic Verification
  For the most critical symbols and file references in the plan (prioritize WHERE: anchors and symbols being modified), use LSP to verify:

  1. **Symbol existence** — use `workspaceSymbol` for bare symbol names to confirm they exist
  2. **Canonical home** — use `goToDefinition` to verify symbols are where the plan says
  3. **Kind/signature** — use `hover` to check the plan's claims (enum vs struct, function signatures)
  4. **Blast radius** — for symbols being modified, use `findReferences` to check how many call sites are affected
  5. **Phase bleeding** — use `incomingCalls` on critical functions to verify no cross-phase violations
  6. **BLOAT check** — use `documentSymbol` on files the plan will modify that Tier 0 flagged as near the 500-line limit

  Priority order for LSP checks:
  - Explicit file:line references from `file_line_refs` in the JSON
  - Symbols being modified (mentioned in unchecked `- [ ]` items)
  - Repeated symbols across sections (from `symbols` list in JSON)
  - Section-boundary symbols (in depends_on chains)

  Do NOT verify every symbol — focus on the 15-20 most critical.

  ### Pass 3: Checklist Quality Assessment
  For each incomplete section:
  - Are checklist items concrete and actionable (not vague like "improve X")?
  - Does the section have a test strategy with matrix dimensions?
  - Does the completion checklist have mandatory items (test-all.sh, TPR, hygiene)?
  - Are CLAUDE.md rules embedded in items, not just referenced?

  ### Pass 4: Cross-Section Dependency Analysis
  Using Tier 0's FILE_CONTENTION findings plus LSP call hierarchy, identify:
  - Implicit dependencies between sections (Section 2 modifies a function Section 5 calls)
  - Missing depends_on links
  - Ordering risks

  ## Output Format

  Produce a structured findings report as a single markdown document. Group by severity (Critical > Major > Minor). Each finding must include:
  - Category (DRIFT/GAP/WASTE/BLOAT/LEAK/EXPOSURE per impl-hygiene.md)
  - Location (file:line or section reference)
  - What's wrong and what should change
  - Whether this needs Opus attention or is informational

  End with a "Needs Opus Attention" section listing only the findings that require architectural judgment to resolve.
```

Read the Sonnet agent's output. Extract the "Needs Opus Attention" items.

### Step 3: Tier 2 — Opus Architect (SOLE WRITER, GROUNDED IN FULL NARRATIVE)

Spawn an **Opus** agent that receives the pre-digested findings and has **full restructuring authority**. This is the only agent that edits plan files.

**CRITICAL — Grounding mandate:** Before making ANY edits, the Opus architect MUST read the full plan narrative to understand the story. This is what prevents locally-correct-but-globally-incoherent edits:

1. Read the plan's **overview/index** — understand the mission statement, success criteria, and the section dependency graph
2. Read **ALL completed sections** (status: complete or in-progress with work already done) — understand what decisions were already made, what constraints they impose, what APIs/representations were chosen
3. Read the **current section(s) under review** — understand how they claim to fit the narrative
4. Articulate (in the agent's own reasoning): "The mission is X. Sections 1–N decided Y. This section must therefore Z." This forces coherence before editing begins.

```
Agent:
  You are the architect reviewing and improving an Ori compiler plan at {plan_dir}/.

  You have FULL AUTHORITY to make ANY structural change: add, remove, merge, split, reorder sections, rewrite the overview/index, restructure the entire plan. The plan exists to serve the mission — if the structure fights the mission, change the structure.

  CRITICAL PREREQUISITE: Read CLAUDE.md first (every word):
  ```
  Read file: CLAUDE.md
  ```

  Then load the hygiene rules:
  ```
  Read file: .claude/rules/impl-hygiene.md
  Read file: .claude/rules/compiler.md
  Read file: .claude/rules/tests.md
  ```

  ## GROUNDING — Full Plan Narrative (MANDATORY BEFORE ANY EDITS)

  Before you write a single character, build a mental model of the entire plan:

  1. Read the plan's overview/index file — understand the MISSION, success criteria, and section dependency graph
  2. Read ALL completed or in-progress sections — understand what decisions have been made, what constraints they impose, what representations/APIs were chosen, what invariants were established
  3. Read the section(s) under review
  4. Before editing, write down (in your reasoning) a brief narrative summary:
     - "The mission is: ..."
     - "Prior sections decided: ..."
     - "This section must therefore: ..."
     - "This section sets up downstream sections by: ..."

  This grounding step is NON-NEGOTIABLE. Edits made without understanding the full narrative produce locally-correct-but-globally-incoherent plans — the exact failure mode this pipeline exists to prevent.

  ## Pre-Digested Context

  ### Tier 0 Audit (deterministic findings — trust these):
  {Insert Tier 0 JSON summary: finding counts, critical/major findings, section manifest}

  ### Tier 1 Semantic Audit (verify critical findings before acting):
  {Insert Sonnet agent's "Needs Opus Attention" section}

  ## Your Mission

  Read ALL plan files in {plan_dir}/. Then:

  ### Part 1: Technical Accuracy & Feasibility
  - Cross-reference technical claims against the actual codebase
  - For inaccuracies, EDIT the plan files directly
  - If a step is infeasible, EXPAND the approach — never scope down
  - Use LSP (goToDefinition, hover) for spot-checks on any symbol you're unsure about

  ### Part 2: Strategic Cohesion & Mission Fulfillment
  - Verify the plan works as ONE cohesive strategy — not N independent sections
  - Every mission criterion must trace to at least one section
  - Flag and fix deferral traps: "bonus", "future", "nice to have" → concrete mandatory tasks
  - Verify depends_on chains and sequential flow
  - **Cross-section coherence**: does each section's approach respect the constraints imposed by prior sections AND properly set up downstream sections? A section that contradicts a prior decision or fails to provide what a later section needs is a COHERENCE violation — the highest-priority finding category.

  ### Part 3: Section Executability & Testing Rigor
  - Every checklist item must be concrete and verifiable
  - Every code-modifying section needs: matrix tests, semantic pins, TDD ordering
  - Sections with <3 items: expand. Sections with 20+ items: split.
  - Embed CLAUDE.md rules in items, not just reference them
  - Verify completion checklists have: test-all.sh, TPR, hygiene, plan-sync

  ### Part 4: Codebase Scan (targeted, not exhaustive)
  - For files flagged by Tier 0/1 (DEAD_PATH, BLOAT_RISK, FILE_CONTENTION), verify and fix
  - Weave "fix along the way" items for BLOAT/WASTE/DRIFT findings

  ### Part 5: Final Integration
  - Verify `reviewed` field:
    - Single-section review: set `reviewed: true` after confirming accuracy (unless unfixable issues remain)
    - Whole-plan review: do NOT change any `reviewed` values
  - Verify success criteria hierarchy (mission → section → checklist)
  - Update overview/index to match any structural changes
  - Remove all `<!-- reviewed: ... -->` comment markers

  ## CRITICAL RULES
  1. NEVER scope down — always expand
  2. No deferral traps — every checkbox must be implementable
  3. Testing rigor is non-negotiable (matrix, semantic pins, TDD)
  4. Rules woven in, not assumed — plans are self-contained execution documents
  5. Verify Tier 1 critical findings before acting on them — Sonnet can be wrong
  6. Plan-sync on section completion: frontmatter, overview, index, cross-links
  7. COHERENCE is king — every edit must be evaluated against the full plan narrative, not just the local section

  After editing, list what you changed and why.
```

Read the Opus agent's output. Note what changes were made.

### Step 4: Tier 3 — Convergence Review Loop (Dual-Source)

This is the core quality gate. After the Opus architect has edited the plan, run dual-source adversarial review with a convergence loop — fix findings and re-review until both Codex and Gemini return zero actionable findings.

**This replaces the former one-shot `/tp-help` consultation.** The convergence loop ensures solutions survive adversarial pressure across multiple rounds. Round 1 catches obvious issues; round 2 tests the quality of your fixes; round 3+ filters out everything except systematic, holistic solutions.

#### State Machine

```
iteration_counter = 0
while iteration_counter < 5:
    RUN = scratch-dir.sh
    write codex.prompt.md and gemini.prompt.md into RUN (plan-review-specific)
    if dual-invoke-with-retry.sh fails:
        surface failure category + $RUN to user via AskUserQuestion
        EXIT
    else:
        merged = merge-findings.py(codex.envelope.json, gemini.envelope.json)
        if merged has zero actionable findings:
            CLEAN PASS — exit with iteration_counter for the report
        for each actionable finding in merged:
            fix the plan (edit plan files directly)
            run plan-audit.py --verify to confirm structural integrity
        iteration_counter += 1
# After 5 iterations without clean:
surface remaining findings to user via AskUserQuestion
```

**Max iterations: 5** (not 10 like code TPR — plan edits are cheaper and faster than code fixes, so fewer iterations are needed to converge; if 5 rounds don't converge, there's likely a fundamental design disagreement that needs human judgement).

#### 4a. Create a per-run scratch directory

```
Bash:
  RUN=$(.claude/skills/dual-tpr/scripts/scratch-dir.sh)
  echo "$RUN"
```

#### 4b. Write both reviewer prompts (PLAN-REVIEW-SPECIFIC)

The reviewer prompts are the critical differentiator from code TPR. Reviewers must be grounded in the **full plan narrative** and evaluate **cross-section coherence**, not just local correctness.

```
Bash:
  cat > "$RUN/codex.prompt.md" <<'PROMPT'
  Run the /review-work skill in envelope-only mode. Emit the JSON
  envelope per .claude/skills/dual-tpr/findings-schema.json; do NOT
  write findings to plan files.

  ## HARD RULES — ABSOLUTE, NO EXCEPTIONS
  1. You are READ-ONLY. Do NOT edit, create, or delete ANY file. Your output is the JSON envelope ONLY.
  2. Every finding must use the finding categories from impl-hygiene.md (LEAK, DRIFT, GAP, WASTE, COHERENCE, BLOAT, NOTE).
  3. You MUST ground yourself in the full plan narrative before reviewing any section.

  ## Grounding — read these files FIRST before reviewing

  Before you look at any plan content, read these files in full:

  1. CLAUDE.md (project root)
  2. .claude/rules/impl-hygiene.md
  3. .claude/rules/tests.md
  4. .claude/rules/compiler.md

  ## Scope: Plan review — {plan_dir}

  You are reviewing a PLAN, not code. Your job is to evaluate the plan's proposed solutions for:

  ### 1. Cross-Section Coherence (HIGHEST PRIORITY)
  Read the plan's overview/index to understand the mission. Then read ALL sections (completed and pending). For each section under review, ask:
  - Does this section's approach RESPECT constraints imposed by prior completed sections?
  - Does this section properly SET UP what downstream sections need?
  - Does the solution serve the plan's mission, or just solve a local problem?
  - If a prior section chose representation X, does this section honor that choice or silently contradict it?
  - Would implementing this section in sequence after prior sections actually work, or would it require rework?

  A section that is internally correct but globally incoherent is a COHERENCE finding — the most severe category.

  ### 2. Solution Quality
  - Are proposed solutions systematic and holistic, or narrow and local?
  - Do they account for cross-cutting concerns (type checker + evaluator + LLVM + tests all need updating)?
  - Are there simpler correct approaches the plan missed?
  - Would this solution survive implementation, or will it hit walls the plan doesn't anticipate?

  ### 3. Technical Accuracy
  - Are file paths, symbol names, and function signatures correct?
  - Are the claims about how the codebase works actually true?
  - Do the depends_on chains reflect real implementation dependencies?

  ### 4. Testing & Verification Strategy
  - Are test matrices genuinely comprehensive, or do they just look comprehensive?
  - Are semantic pins and negative pins specified for each section?
  - Does the test strategy cover cross-section interactions?

  {Additional context: Tier 0/1 summary findings, specific areas of concern}
  PROMPT

  cat > "$RUN/gemini.prompt.md" <<'PROMPT'
  Activate the review-work skill and follow its instructions exactly.
  Emit the JSON envelope per .claude/skills/dual-tpr/findings-schema.json;
  do NOT write findings to plan files.

  ## HARD RULES — ABSOLUTE, NO EXCEPTIONS
  1. You are READ-ONLY. Do NOT edit, create, or delete ANY file. Your output is the JSON envelope ONLY.
  2. Every finding must use the finding categories from impl-hygiene.md (LEAK, DRIFT, GAP, WASTE, COHERENCE, BLOAT, NOTE).
  3. You MUST ground yourself in the full plan narrative before reviewing any section.

  ## Grounding — read these files FIRST before reviewing

  Before you look at any plan content, read these files in full:

  1. CLAUDE.md (project root)
  2. .claude/rules/impl-hygiene.md
  3. .claude/rules/tests.md
  4. .claude/rules/compiler.md

  ## Scope: Plan review — {plan_dir}

  You are reviewing a PLAN, not code. Your job is to evaluate the plan's proposed solutions for:

  ### 1. Cross-Section Coherence (HIGHEST PRIORITY)
  Read the plan's overview/index to understand the mission. Then read ALL sections (completed and pending). For each section under review, ask:
  - Does this section's approach RESPECT constraints imposed by prior completed sections?
  - Does this section properly SET UP what downstream sections need?
  - Does the solution serve the plan's mission, or just solve a local problem?
  - If a prior section chose representation X, does this section honor that choice or silently contradict it?
  - Would implementing this section in sequence after prior sections actually work, or would it require rework?

  A section that is internally correct but globally incoherent is a COHERENCE finding — the most severe category.

  ### 2. Solution Quality
  - Are proposed solutions systematic and holistic, or narrow and local?
  - Do they account for cross-cutting concerns (type checker + evaluator + LLVM + tests all need updating)?
  - Are there simpler correct approaches the plan missed?
  - Would this solution survive implementation, or will it hit walls the plan doesn't anticipate?

  ### 3. Technical Accuracy
  - Are file paths, symbol names, and function signatures correct?
  - Are the claims about how the codebase works actually true?
  - Do the depends_on chains reflect real implementation dependencies?

  ### 4. Testing & Verification Strategy
  - Are test matrices genuinely comprehensive, or do they just look comprehensive?
  - Are semantic pins and negative pins specified for each section?
  - Does the test strategy cover cross-section interactions?

  {Additional context: Tier 0/1 summary findings, specific areas of concern}
  PROMPT
```

#### 4c. Launch the dual-source transport in the background

```
Bash (run_in_background: true):
  .claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh \
    --run "$RUN" \
    --skill review-work \
    --codex-prompt "$RUN/codex.prompt.md" \
    --gemini-prompt "$RUN/gemini.prompt.md" \
    --schema .claude/skills/dual-tpr/findings-schema.json
```

**DO NOT:**
- Run the transport in the Bash foreground
- Set a `timeout:` parameter on the Bash call
- Wrap the transport in an Agent subagent
- Poll `$RUN/*.envelope.json` or `$RUN/merged.json` mid-stream (atomic-write files)

#### Polling Protocol — Canonical SSOT

Follow `.claude/skills/dual-tpr/polling-protocol.md` verbatim. Poll `status-check.sh "$RUN" --events 5` at ~75-second intervals with absolute wall-clock timestamps. See the polling protocol file for full rules.

@.claude/skills/dual-tpr/polling-protocol.md

#### 4d. On success: merge and verify findings

When the completion notification arrives AND the transport exited 0:

```
Bash:
  .claude/skills/dual-tpr/scripts/merge-findings.py \
    --codex "$RUN/codex.envelope.json" \
    --gemini "$RUN/gemini.envelope.json" \
    --section "XX" \
    --out "$RUN/merged.json"
```

Read `$RUN/merged.json`. For EVERY finding, independently verify the claim against the actual plan files and codebase before acting on it — reviewer findings are hypotheses, not facts. Trust tiers: Codex = HIGH (spot-check); Gemini = LOWER (full verification needed, confabulation-prone).

#### 4e. If zero actionable findings → CLEAN PASS (EXIT to Step 5)

Report: "Convergence review passed clean on iteration N — both reviewers returned zero actionable findings."

#### 4f. If actionable findings exist → fix plan and re-run

For each verified actionable finding:
1. **Edit the plan files directly** — this is plan review, not code review. Fixes are plan text edits: restructured sections, rewritten approaches, added cross-references, expanded test matrices, corrected technical claims.
2. **For COHERENCE findings** (cross-section issues): read both the offending section AND the section(s) it conflicts with. Fix the incoherence at the right level — sometimes it's the current section that needs to change, sometimes it's a prior section's constraint that was wrong.
3. Run `plan-audit.py --verify` to confirm structural integrity after edits.

Then **go back to Step 4a** (create fresh scratch dir, re-run both reviewers on the fixed plan).

#### 4g. After 5 iterations without clean pass → user escalation

Surface remaining merged findings to the user via `AskUserQuestion`:
- Summary of iterations run and findings per iteration
- Whether progress is being made (findings decreasing) or oscillating (likely a fundamental design tension)
- The current finding list
- Ask: continue past the cap, file remaining findings and proceed, or discuss the recurring design tension?

### Step 5: Tier 0 Post-Edit Verification

Run the audit script again to verify structural integrity after all edits (Opus + convergence loop fixes):

```
Bash:
  python3 .claude/skills/plan-audit/plan-audit.py {plan_dir} --verify --json > /tmp/plan-audit-verify.json 2>&1
```

Read the results. If critical findings remain, report them to the user.

### Step 5.5: Cross-Plan Review Invalidation

**When to run:** This step runs when the review made **significant changes** to the plan — specifically, changes that alter which files, types, or subsystems the plan's sections reference. Skip this step if the review only made cosmetic/formatting changes.

**Purpose:** If the review changed a plan's scope (added/removed file references, changed implementation approach, restructured sections), those changes may invalidate `reviewed: true` sections in OTHER plans that overlap with the changed plan's scope. This is the same cache coherence problem as `/create-plan` Step 19, but triggered by review edits instead of plan creation.

#### 5.5a: Run invalidation detection

```
Bash:
  python3 .claude/skills/plan-audit/plan-invalidate.py {plan_dir} --json > /tmp/plan-invalidate-output.json
```

Read `/tmp/plan-invalidate-output.json`. If `status` is `"clean"`, skip to Step 6.

#### 5.5b: Present findings to user

If stale sections are found, present them to the user via `AskUserQuestion` using the same approval model as `/create-plan` Step 19 — the mutation policy for cross-plan invalidation is IDENTICAL regardless of which command triggers it:

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

If the user approves:

```
Bash:
  python3 .claude/skills/plan-audit/plan-invalidate.py {plan_dir} --apply [--min-weight 4]
```

Include the results in the Step 6 verdict under the Cross-Plan Invalidation heading.

### Step 6: Present Verdict

Consolidate findings into a summary:

```
## Plan Review: {plan name}

### Pipeline Summary
- **Tier 0** (static analysis): {N} findings, {M} auto-fixed
- **Tier 1** (Sonnet semantic audit): {N} findings, {M} needing Opus attention
- **Tier 2** (Opus architect): {changes made}
- **Tier 3** (convergence review): {iterations to clean | max iterations reached}
  - Iteration 1: {N} findings ({M} COHERENCE, {K} other)
  - Iteration 2: {N} findings ...
  - ...
- **Post-edit verification**: {CLEAN | N remaining findings}

### Changes Made
{List of edits by category: accuracy fixes, structural changes, coherence fixes, test strategy additions, hygiene weaves}

### Convergence History
{For each iteration: what findings were surfaced, what was fixed, what changed between rounds. This is the audit trail showing how the plan was pressure-tested.}

### Review Status
| Section | `reviewed` Before | `reviewed` After | Reason |
|---------|------------------|-----------------|--------|
| ... | ... | ... | ... |

### Cross-Plan Invalidation
{If Step 5.5 ran: report how many sections in other plans were flipped from reviewed: true → false, and why.}
{If Step 5.5 was skipped (cosmetic changes only): "No cross-plan invalidation needed — review changes were cosmetic."}
{If Step 5.5 found no overlaps: "No cross-plan invalidation needed — no overlapping scopes."}

### Remaining Concerns
{Issues requiring human judgement, ranked Critical > Major > Minor}

---

## Verdict

**{CLEAN | MINOR FIXES APPLIED | SIGNIFICANT REWORK APPLIED | RESTRUCTURED | CONVERGED AFTER N ROUNDS | NEEDS MANUAL ATTENTION}**

{2-3 sentence assessment. Total edits across all tiers. Convergence behavior (clean on round 1 = high confidence; clean on round 3+ = the plan needed real work and got it).}
```

## Important Rules

1. **Tier 2 (Opus) has FULL AUTHORITY** — add, remove, merge, split, reorder sections, restructure entirely.
2. **Tier 1 (Sonnet) is READ-ONLY** — findings and annotations only, no edits.
3. **Tier 0 auto-fixes are limited to --fix-safe** — only deterministic metadata corrections.
4. **Tier 3 reviewers are READ-ONLY** — they produce findings envelopes only; Claude fixes the plan.
5. **Cross-section coherence is the primary target** — locally correct but globally incoherent is the worst failure mode.
6. **Grounding is mandatory** — Opus and both reviewers MUST read the full plan narrative before reviewing any section.
7. **Be specific** — every change needs evidence: a spec clause, a file:line, or concrete reasoning.
8. **Cross-reference, don't guess** — use LSP and file reads to verify claims.
9. **NEVER scope down — always expand** — grow the plan if it doesn't fulfill its mission.
10. **No deferral traps** — "bonus", "future", "lower priority" → concrete mandatory tasks or explicit `<!-- blocked-by:X -->`.
11. **Testing rigor is non-negotiable** — matrix tests, semantic pins, TDD ordering, debug+release.
12. **Success criteria mandatory** at both mission and section levels, connected bidirectionally.
13. **Rules woven in, not assumed** — plans are self-contained execution documents.
14. **Verify Tier 1 findings** — Sonnet can confabulate. Trust deterministic Tier 0 findings; verify Tier 1 judgment calls.
15. **Plan-sync on section completion** — frontmatter, overview, index, cross-links, next section's depends_on.
16. **Convergence loop infra retries are invisible to semantic iteration budget** — transport failures don't consume iteration count.
