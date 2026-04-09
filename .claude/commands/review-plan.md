---
name: review-plan
description: Review and improve a plan for accuracy, correctness, feasibility, strategic cohesion, executability, and testing rigor — expand to fulfill the mission, never scope down.
allowed-tools: Read, Grep, Glob, Agent, AskUserQuestion, Bash, Edit, Write, LSP, Skill
---

# Review Plan Command

Review and improve a plan using a **3-tier "Filter-then-Architect" pipeline** that minimizes expensive Opus context while maximizing review quality. The pipeline uses static analysis (Tier 0) and a Sonnet semantic auditor (Tier 1) to pre-digest mechanical findings, so the Opus architect (Tier 2) focuses only on judgment-intensive work: technical soundness, strategic cohesion, restructuring, and expansion.

**Design rationale:** The prior 4-sequential-Opus-agent design consumed ~200K Opus tokens per review, most of it on mechanical verification (checking file paths, counting checkboxes, validating line numbers). The tiered design pushes mechanical work to Python scripts (0 tokens) and semantic verification to Sonnet (~40-60K cheap tokens), leaving Opus with only the judgment work (~30-40K Opus tokens). Both Codex and Gemini independently validated this architecture (2026-04-09).

## Reviewed Field Semantics — CRITICAL

The `reviewed: true/false` field in section frontmatter is a **pre-implementation gate** — it tracks whether a section has been validated against the current codebase right before implementation begins.

**Two modes — the mode determines whether `reviewed` gets flipped:**

**Single-section review** (`/review-plan plans/foo/section-03.md`):
This is the pre-implementation gate. After the pipeline completes, flip `reviewed: true` — unless the Opus agent flagged issues it could NOT fix (requiring human judgement), in which case leave `reviewed: false`.

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

### Step 3: Third-Party Blind Spot Check via /tp-help

**SEQUENTIAL & FOREGROUND — MANDATORY.**

Call `/tp-help` with:
- The plan's mission (one line)
- Tier 0 summary (finding counts by category)
- Tier 1's top findings (the "Needs Opus Attention" list)
- The question: "What architectural risks or blind spots would you flag for the Opus review?"

Use the response to add focus areas to the Opus prompt.

### Step 4: Tier 2 — Opus Architect (SOLE WRITER)

Spawn an **Opus** agent that receives the pre-digested findings and has **full restructuring authority**. This is the only agent that edits plan files.

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

  ## Pre-Digested Context

  ### Tier 0 Audit (deterministic findings — trust these):
  {Insert Tier 0 JSON summary: finding counts, critical/major findings, section manifest}

  ### Tier 1 Semantic Audit (verify critical findings before acting):
  {Insert Sonnet agent's "Needs Opus Attention" section}

  ### Third-Party Blind Spots:
  {Insert relevant /tp-help insights}

  ## Your Mission

  Read ALL plan files in {plan_dir}/. Then:

  ### Part 1: Technical Accuracy & Feasibility
  - Cross-reference technical claims against the actual codebase
  - For inaccuracies, EDIT the plan files directly
  - If a step is infeasible, EXPAND the approach — never scope down
  - Use LSP (goToDefinition, hover) for spot-checks on any symbol you're unsure about

  ### Part 2: Strategic Cohesion & Mission Fulfillment
  - Verify the plan works as ONE cohesive strategy
  - Every mission criterion must trace to at least one section
  - Flag and fix deferral traps: "bonus", "future", "nice to have" → concrete mandatory tasks
  - Verify depends_on chains and sequential flow

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

  After editing, list what you changed and why.
```

### Step 5: Tier 0 Post-Edit Verification

Run the audit script again to verify structural integrity after Opus edits:

```
Bash:
  python3 .claude/skills/plan-audit/plan-audit.py {plan_dir} --verify --json > /tmp/plan-audit-verify.json 2>&1
```

Read the results. If critical findings remain, report them to the user.

### Step 6: Present Verdict

Consolidate findings into a summary:

```
## Plan Review: {plan name}

### Pipeline Summary
- **Tier 0** (static analysis): {N} findings, {M} auto-fixed
- **Tier 1** (Sonnet semantic audit): {N} findings, {M} needing Opus attention
- **Tier 2** (Opus architect): {changes made}
- **Post-edit verification**: {CLEAN | N remaining findings}

### Changes Made
{List of edits by category: accuracy fixes, structural changes, test strategy additions, hygiene weaves}

### Review Status
| Section | `reviewed` Before | `reviewed` After | Reason |
|---------|------------------|-----------------|--------|
| ... | ... | ... | ... |

### Remaining Concerns
{Issues requiring human judgement, ranked Critical > Major > Minor}

---

## Verdict

**{CLEAN | MINOR FIXES APPLIED | SIGNIFICANT REWORK APPLIED | RESTRUCTURED | NEEDS MANUAL ATTENTION}**

{2-3 sentence assessment. Total edits across all tiers.}
```

## Important Rules

0. **ALL external consultations (`/tp-help`) are SEQUENTIAL and FOREGROUND** — complete fully before proceeding.
1. **Tier 2 (Opus) has FULL AUTHORITY** — add, remove, merge, split, reorder sections, restructure entirely.
2. **Tier 1 (Sonnet) is READ-ONLY** — findings and annotations only, no edits.
3. **Tier 0 auto-fixes are limited to --fix-safe** — only deterministic metadata corrections.
4. **Be specific** — every change needs evidence: a spec clause, a file:line, or concrete reasoning.
5. **Cross-reference, don't guess** — use LSP and file reads to verify claims.
6. **NEVER scope down — always expand** — grow the plan if it doesn't fulfill its mission.
7. **No deferral traps** — "bonus", "future", "lower priority" → concrete mandatory tasks or explicit `<!-- blocked-by:X -->`.
8. **Testing rigor is non-negotiable** — matrix tests, semantic pins, TDD ordering, debug+release.
9. **Success criteria mandatory** at both mission and section levels, connected bidirectionally.
10. **Rules woven in, not assumed** — plans are self-contained execution documents.
11. **Verify Tier 1 findings** — Sonnet can confabulate. Trust deterministic Tier 0 findings; verify Tier 1 judgment calls.
12. **Plan-sync on section completion** — frontmatter, overview, index, cross-links, next section's depends_on.
