# Review Plan Command

Iteratively review and refine any plan through 10 sequential analysis passes. Uses a hierarchical agent architecture to prevent context overflow: the main context only handles Q&A, coordinator agents assign work, and worker agents each handle a semantic group of files (derived from the plan's own structure via `index.md`).

## Usage

```
/review-plan <plan-path>
```

- `plan-path`: Path to the plan directory (e.g., `plans/lexer_v2`, `plans/llvm_v2`) or a single plan file

## Overview

This command runs **10 sequential refinement passes** over a plan. Each pass:

1. **Setup agent** reads `index.md` → derives semantic groups and **allocates passes by group**
2. **Each pass focuses on a specific group** (Round 1: deep review, Round 2: refinement) or cross-group integration
3. **Worker agent(s)** analyze the focus group's files, cross-reference source code, return issues
4. **Main context** presents questions to user via interactive Q&A
5. **Update worker(s)** apply fixes to affected plan files
6. **Next pass** moves to the next group or review phase

Each pass builds on the previous one's changes, creating iterative refinement. Early passes catch major issues; later passes catch subtleties, inconsistencies introduced by earlier edits, and polish.

## Architecture: 3-Level Agent Hierarchy

```
Main Context (Q&A only — stays lean)
├── Setup Agent: reads index.md → derives groups + allocates passes
└── Passes 1-10 (allocated dynamically from plan structure):
    │
    ├── GROUP-FOCUSED PASS (Round 1: review, Round 2: refinement):
    │   ├── Analysis Worker ("{focus group}")   ← 1-5 plan files + source code
    │   ├── [Main Context: Q&A with user]
    │   └── Update Worker ("{focus group}")     ← edits plan files in that group
    │
    └── CROSS-GROUP PASS (integration, final validation):
        ├── Analysis Coordinator Agent
        │   ├── Worker ("{group 1}")            ← cross-group interfaces
        │   ├── Worker ("{group 2}")            ← cross-group interfaces
        │   └── ... (consolidates results)
        ├── [Main Context: Q&A with user]
        └── Update Coordinator Agent
            ├── Worker (group with issues)
            └── Worker (group with issues)
```

**Why this matters**: A single agent reading 10+ plan files AND cross-referencing source code will exhaust its context window. By splitting into workers based on the plan's own semantic groups (tiers, themes, dependency clusters from `index.md`), every agent stays focused on a coherent domain. The main context never reads plan files or source code — it only handles user interaction.

---

## Workflow

### Step 0: Setup (Sub-Agent)

Launch a **general-purpose agent** to discover the plan structure and derive semantic worker groups:

```
You are the setup agent for a plan review process.

1. Resolve the plan path: {plan-path}
   - If it's a directory: list all files (index.md, 00-overview.md, section-*.md, or similar)
   - If it's a single file: use that file (skip to step 4)

2. **Read the index file FIRST** (index.md, 00-overview.md, or similar manifest):
   - This is your PRIMARY source of plan structure. Do NOT read every section file.
   - Extract: section titles, file paths, tiers/phases/groups, dependency relationships,
     keyword clusters, and any other organizational structure the plan defines.

3. **Derive semantic groups from the index.** Group files by the plan's OWN structure:
   - Use tiers, phases, or thematic clusters defined in the index
   - Use dependency relationships (sections that reference each other belong together)
   - Use the plan's own naming/numbering to identify natural groupings
   - Give each group a short descriptive name based on what those sections cover
   - Target 2-5 groups total. Each group should have 1-5 files (prefer 3-4).
   - If a group would exceed 5 files, split it into sub-groups along natural boundaries.
   - Always include the index/overview file in the first group.
   - Example for a plan with tiers: "Core Foundation (Tier 1)", "ARC System (Tier 2)", etc.
   - Example for a plan with phases: "Phase 1: Parsing", "Phase 2: Type Checking", etc.

4. For each file, produce a ONE-LINE summary (from index keywords/descriptions — do NOT
   read the full section files just for summaries).

Return your result in this EXACT format:

GROUPS:
### Group 1: {descriptive name based on plan structure}
- {absolute-path-1}: {one-line summary}
- {absolute-path-2}: {one-line summary}

### Group 2: {descriptive name based on plan structure}
- {absolute-path-3}: {one-line summary}
- {absolute-path-4}: {one-line summary}
...

TOTAL_FILES: {count}
TOTAL_GROUPS: {count}

STRUCTURE_SUMMARY:
{2-5 sentence overview of what this plan covers, the grouping rationale, and how groups relate}

PASS_ALLOCATION:
Given your N groups, allocate 10 passes as follows:
- Passes 1–N: One per group (deep review). List each: "Pass {i}: Review — {group name}"
- Passes N+1–2N (max pass 8): One per group (refinement). List each: "Pass {i}: Refine — {group name}"
- Pass 9 (or 2N+1): "Cross-group integration & dependency chains"
- Pass 10 (or last): "Final validation & polish"
If 2N > 8, compress Round 2 by combining smaller groups.
If N >= 8, skip Round 2 and use passes 9-10 for cross-group + final.

Example for 4 groups (Tier 1, Tier 2, Tier 3, Tier 4):
Pass 1: Review — Tier 1: Core Foundation (TypeInfo, IrBuilder, Expr, ABI)
Pass 2: Review — Tier 2: ARC System (Classification, Borrow, RC, FBIP)
Pass 3: Review — Tier 3: Codegen Patterns (Decision Trees, LLVM Passes, Incremental)
Pass 4: Review — Tier 4: Infrastructure (Debug Info, Tests, Diagnostics)
Pass 5: Refine — Tier 1: Core Foundation
Pass 6: Refine — Tier 2: ARC System
Pass 7: Refine — Tier 3: Codegen Patterns
Pass 8: Refine — Tier 4: Infrastructure
Pass 9: Cross-group integration & dependency chains
Pass 10: Final validation & polish

FALLBACK: If there is no index file or the plan has no clear internal structure,
group files sequentially in batches of 3-4 and name groups by their content themes
(you may skim file headings to determine themes, but do NOT read full files).
```

After the setup agent returns:

1. **Parse the groups and file list** from the agent's output
2. **Create a todo list** with 10 tasks (see Todo List Structure below)
3. **Do NOT read any plan files yourself** — only store the groups, file list, and structure summary

### Steps 1-10: Review Passes (Sequential)

For each pass N (1 through 10), execute the following phases:

#### Phase A: Analysis (Coordinator + Workers)

Launch a **general-purpose agent** as the **Analysis Coordinator** with:

```
You are the ANALYSIS COORDINATOR for review pass {N}/10 of a plan.
Your job is to assign pre-defined semantic groups to focused worker agents, then consolidate results.

SEMANTIC GROUPS (derived from the plan's own structure):
{groups from setup — each group has a name and file list}

STRUCTURE SUMMARY:
{structure summary from setup}

PASS {N} CONTEXT:
{Use the pass allocation from setup to determine the pass type and focus:}

{If Round 1 (review) pass — focus group: "{group name}":
  "First deep review of {group name}. Focus on structural accuracy, design soundness,
  factual correctness, and source code verification for this group's sections."}
{If Round 2 (refinement) pass — focus group: "{group name}":
  "Refinement pass for {group name}. Round 1 found issues that were fixed.
  Focus on: remaining inaccuracies, edge cases missed in Round 1, problems
  introduced by Round 1 edits, and detail-level correctness."}
{If cross-group integration pass:
  "Cross-group integration review. Focus on: interfaces between groups,
  dependency consistency across sections, terminology alignment,
  and cross-references between plan sections."}
{If final validation pass:
  "Final validation. Focus on: overall coherence, readability, any remaining
  inconsistencies, and issues introduced by previous rounds of editing."}

PASS TYPE: {from pass allocation — "group-focused" or "cross-group"}
FOCUS GROUP (if group-focused): {the specific group name and its files}

INSTRUCTIONS:

1. **Group-focused passes** (Round 1 review, Round 2 refinement):
   - Launch ONLY ONE worker agent — the worker for the focus group.
   - Do NOT launch workers for other groups. They are not being reviewed this pass.

2. **Cross-group passes** (integration, final validation):
   - Launch one worker per semantic group, all in parallel.
   - Each worker focuses on cross-group interfaces, not internal group issues.

   General rules:
   - Do NOT re-batch or re-group the files — the setup agent already grouped them
     based on the plan's own structure (tiers, phases, themes).
   - If a group has more than 5 files, you may split it into sub-groups along
     natural boundaries, but preserve the group's thematic name.

3. For each applicable group, launch a general-purpose WORKER AGENT with this prompt:

   ---
   You are a review worker for pass {N}/10 of a plan review.
   You are reviewing the "{group name}" group (not the entire plan).
   This group covers: {group's thematic focus, derived from its name and file summaries}

   FILES TO REVIEW (read ALL of these):
   {group file paths with one-line summaries}

   CONTEXT (do NOT read these — this is provided for reference):
   The overall plan covers: {structure summary}
   Other groups in the plan (not your responsibility): {other group names + file paths}

   REVIEW FOCUS FOR PASS {N}:
   {pass context from above}

   REVIEW CHECKLIST:
   1. Is the design in these files accurate and sound?
   2. Are all facts correct? Cross-reference against actual source code in compiler/
   3. Does it utilize existing Ori systems correctly? (lexer: compiler/ori_lexer/,
      parser: compiler/ori_parse/, types: compiler/ori_types/, IR: compiler/ori_ir/,
      eval: compiler/ori_eval/, diagnostic: compiler/ori_diagnostic/)
   4. Do referenced files, types, functions, and modules actually exist? CHECK by reading source.
   5. Are implementation steps in the right order with correct dependencies?
   6. Any gaps — missing edge cases, unaddressed error handling?
   7. Is terminology consistent with the Ori codebase and spec (docs/ori_lang/v2026/spec/)?

   OUTPUT FORMAT (strict):

   ## Issues Found

   ### Issue {number}: {title}
   - **File**: {which plan file}
   - **Location**: {section/heading in that file}
   - **Problem**: {what's wrong}
   - **Evidence**: {what you found in source code or spec — include file path + relevant detail}
   - **Suggested Fix**: {how to fix it}
   - **Confidence**: HIGH / MEDIUM / LOW

   ## Questions for User

   ### Question {number}: {title}
   - **File**: {which plan file}
   - **Context**: {why this question matters}
   - **Question**: {the actual question}
   - **Recommended Answer**: {your recommendation}
   - **Reasoning**: {why you recommend this}
   - **Alternatives**: {other valid options}

   ## Batch Summary
   - Issues found: {count}
   - Questions for user: {count}
   - Files reviewed: {count}

   If you find NO issues and have NO questions, say so explicitly.
   ---

4. After ALL worker agents return, consolidate their results:
   - Merge all issues into one numbered list (re-number sequentially)
   - Merge all questions into one numbered list (re-number sequentially)
   - De-duplicate: if two workers found the same issue, keep the more detailed one
   - For group-focused passes with only one worker, pass through results directly

5. Return the consolidated result in this format:

   ## Consolidated Issues (Pass {N})

   {all issues, re-numbered}

   ## Consolidated Questions (Pass {N})

   {all questions, re-numbered}

   ## Pass {N} Analysis Summary
   - Total issues found: {count}
   - Total questions for user: {count}
   - Groups processed: {count}
   - Overall plan health: {assessment}
```

#### Phase B: User Q&A (Main Context — Interactive)

After the analysis coordinator returns:

1. **If the coordinator found questions**: Present them to the user using **AskUserQuestion**, one at a time or grouped logically (max 4 per AskUserQuestion call). For each question:
   - Show the context and reasoning
   - Offer the agent's recommended answer as the first option (marked as recommended)
   - Include alternatives as other options
   - Allow "Other" for custom input

2. **If no questions**: Skip to Phase C.

**Important**: The main context should only store a compact representation of issues and answers — do not paste the full analysis output into Phase C's prompt. Instead, summarize each issue as one line: `"{issue number}. {title}: {suggested fix} [File: {path}]"`.

#### Phase C: Plan Update (Coordinator + Workers)

Launch a **general-purpose agent** as the **Update Coordinator** with:

```
You are the UPDATE COORDINATOR for review pass {N}/10.
Your job is to distribute plan edits across focused worker agents.

ISSUES TO FIX:
{compact issue list — one line per issue with file path, problem, and fix}

USER ANSWERS TO QUESTIONS:
{compact answers — one line per question with the user's decision}

SEMANTIC GROUPS WITH CHANGES NEEDED:
{only list groups (with their files) that have issues or are affected by user answers}

INSTRUCTIONS:

1. Use the semantic groups above as your worker assignments. Each group with changes
   becomes one worker. Skip groups with no issues.
   - If a group has only 1-2 minor issues, it still gets its own worker (domain coherence
     matters more than minimizing worker count).

2. For each affected group, launch a general-purpose WORKER AGENT in parallel with:

   ---
   You are an update worker for pass {N}/10 of a plan review.

   FILES TO MODIFY (read each one first, then edit):
   {group file paths}

   ISSUES TO FIX IN THESE FILES:
   {only issues relevant to this group's files}

   USER ANSWERS RELEVANT TO THESE FILES:
   {only answers relevant to this group's files}

   INSTRUCTIONS:
   1. Read each plan file in your group
   2. Apply fixes for the issues listed above
   3. Incorporate user answers where relevant
   4. Use the Edit tool for precise, targeted changes
   5. Do NOT rewrite entire files — make surgical edits
   6. Do NOT change anything that wasn't flagged as an issue
   7. After all edits, re-read modified files to verify correctness

   IMPORTANT:
   - Only modify plan files, NEVER modify source code
   - Preserve existing formatting and structure
   - If an issue fix conflicts with a user answer, the user answer takes priority

   REPORT what you changed:
   - {file}: {brief description of change}
   ---

3. After all workers return, consolidate their reports.

4. Return a summary:

   ## Update Summary (Pass {N})
   - Files modified: {list}
   - Changes made: {count}
   - Per-file changes:
     - {file}: {brief description}
```

#### Phase D: Progress Report (Main Context)

After the update coordinator returns, briefly report to the user:

```
Pass {N}/10 complete.
- Issues fixed: {count}
- Questions resolved: {count}
- Files modified: {list}

Moving to pass {N+1}...
```

Mark the todo item as completed.

---

## Worker Group Guidelines

Workers are assigned **semantic groups derived from the plan's own structure**, not arbitrary file batches.

### Primary: Index-Driven Grouping (preferred)

The setup agent reads `index.md` and extracts the plan's own organizational structure:
- **Tiers** (e.g., Tier 1: Foundation, Tier 2: ARC, Tier 3: Optimization, Tier 4: Infrastructure)
- **Phases** (e.g., Phase 1: Parsing, Phase 2: Type Checking)
- **Themes** (e.g., sections that share keywords, cross-references, or dependency chains)

Each group maps to one worker agent. Workers understand their domain because the group has a meaningful name and focused scope.

| Group Size | Action |
|------------|--------|
| 1-5 files | One worker handles the whole group |
| 6-8 files | Split into 2 sub-groups along natural boundaries |
| 9+ files | Split into 3+ sub-groups, preserving thematic names |

### Fallback: Content-Based Grouping

If the plan has no index or manifest file, the setup agent skims file headings to identify themes, then groups files by content similarity. Always include overview/index files in the first group.

---

## Todo List Structure

**Do NOT use hardcoded generic names.** After the setup agent returns, use its PASS_ALLOCATION to generate the todo list dynamically. Each task name must reference the specific plan group being reviewed.

### Pass Allocation Algorithm

Given N semantic groups from the setup agent:

| Round | Passes | Focus | Naming Pattern |
|-------|--------|-------|----------------|
| **Round 1** (deep review) | 1 through N | One group per pass | `Review plan — {group name}` |
| **Round 2** (refinement) | N+1 through min(2N, 8) | One group per pass | `Refine plan — {group name}` |
| **Integration** | 9 (or 2N+1) | All groups | `Review plan — Cross-group integration` |
| **Final** | 10 (or last) | All groups | `Review plan — Final validation` |

**Scaling rules:**
- If 2N > 8: Compress Round 2 by combining smaller groups or skipping groups with zero issues from Round 1
- If N >= 8: Skip Round 2 entirely; use passes 9–10 for cross-group + final
- If N <= 2: Add extra rounds (Round 3: source verification per group) to fill 10 passes

### Example: Plan with 4 Tiers (e.g., LLVM V2)

| # | Subject | Active Form |
|---|---------|-------------|
| 1 | Review plan — Tier 1: Core Foundation (TypeInfo, IrBuilder, Expr, ABI) | Reviewing Tier 1: Core Foundation |
| 2 | Review plan — Tier 2: ARC System (Classification, Borrow, RC, FBIP) | Reviewing Tier 2: ARC System |
| 3 | Review plan — Tier 3: Codegen Patterns (Decision Trees, LLVM Passes, Incremental) | Reviewing Tier 3: Codegen Patterns |
| 4 | Review plan — Tier 4: Infrastructure (Debug Info, Tests, Diagnostics) | Reviewing Tier 4: Infrastructure |
| 5 | Refine plan — Tier 1: Core Foundation | Refining Tier 1: Core Foundation |
| 6 | Refine plan — Tier 2: ARC System | Refining Tier 2: ARC System |
| 7 | Refine plan — Tier 3: Codegen Patterns | Refining Tier 3: Codegen Patterns |
| 8 | Refine plan — Tier 4: Infrastructure | Refining Tier 4: Infrastructure |
| 9 | Review plan — Cross-group integration & dependency chains | Reviewing cross-group integration |
| 10 | Review plan — Final validation & polish | Final validation |

### Example: Plan with 2 Phases

| # | Subject | Active Form |
|---|---------|-------------|
| 1 | Review plan — Phase 1: Parsing & Lexing | Reviewing Phase 1: Parsing |
| 2 | Review plan — Phase 2: Type Checking | Reviewing Phase 2: Type Checking |
| 3 | Refine plan — Phase 1: Parsing & Lexing | Refining Phase 1: Parsing |
| 4 | Refine plan — Phase 2: Type Checking | Refining Phase 2: Type Checking |
| 5 | Deep review — Phase 1 source verification | Verifying Phase 1 source refs |
| 6 | Deep review — Phase 2 source verification | Verifying Phase 2 source refs |
| 7 | Refine plan — Phase 1 (edge cases & gaps) | Refining Phase 1 edge cases |
| 8 | Refine plan — Phase 2 (edge cases & gaps) | Refining Phase 2 edge cases |
| 9 | Review plan — Cross-phase integration | Reviewing cross-phase integration |
| 10 | Review plan — Final validation & polish | Final validation |

Each task (except #1) is **blocked by** the previous task. Process them strictly in order.

---

## Important Constraints

### DO:
- Delegate ALL file reading to sub-agents — main context never reads plan files
- Derive worker groups from the plan's own structure (index.md tiers, phases, themes)
- Keep worker agents to 1-5 plan files each (one semantic group per worker)
- Launch worker agents in parallel within each coordinator
- Read actual source code in worker agents to verify plan claims
- Check that referenced types, functions, modules exist
- Present questions with recommended answers and clear reasoning
- Make surgical edits to plan files
- Track all changes across passes
- Pass compact summaries between phases (not full analysis text)

### DO NOT:
- Read plan files or source code in the main context
- Re-batch files into generic groups — always use the plan-derived semantic groups
- Use hardcoded generic pass names ("Major structural issues", "Detail accuracy", etc.) — names MUST come from the plan's own structure via PASS_ALLOCATION
- Give a single agent more than 5 plan files to read
- Launch workers for ALL groups during a group-focused pass — only the focus group's worker
- Modify any source code files (only plan files)
- Run passes in parallel (passes must be strictly sequential)
- Skip the Q&A phase (even if there are no questions, report that)
- Make changes not supported by the analysis
- Rewrite entire plan files (surgical edits only)
- Let agents hallucinate file paths or type names — verify everything
- Pass verbose analysis output between phases — summarize to one line per issue

### Q&A Guidelines:
- Group related questions when presenting to user (max 4 per AskUserQuestion call)
- Always provide a recommended answer with reasoning
- If the agent's recommended answer seems wrong, flag that to the user
- Keep Q&A focused — avoid philosophical questions, focus on concrete decisions
- If a question was already answered in a previous pass, don't re-ask it

---

## Completion

After all 10 passes:

```
Plan review complete (10/10 passes).

Summary:
- Total issues fixed: {sum across all passes}
- Total questions resolved: {sum}
- Files modified: {list with modification counts}

The plan has been iteratively refined through 10 review passes.
Review the final state of the plan at: {plan-path}
```
