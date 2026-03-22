# Create Plan Command

Create a new plan directory with index and section files using the standard template. **Research-first**: deeply understand the existing codebase before writing anything.

## Usage

```
/create-plan <name> [description]
```

- `name`: Directory name for the plan (kebab-case, e.g., `error-recovery`, `lsp-integration`)
- `description`: Optional one-line description of the plan's goal

---

## Phase 1: Prerequisites

### Step 0: Read CLAUDE.md (ABSOLUTE FIRST — NO EXCEPTIONS)

**Before doing ANYTHING else**, read the ENTIRE CLAUDE.md file — every single word, top to bottom:

```
Read file: CLAUDE.md
```

This is mandatory. Do not skip, skim, or partially read. The rules in CLAUDE.md govern ALL behavior in this command. Proceed to Step 1 only after reading the complete file.

### Step 1: Gather Initial Scope

If not provided via arguments, use `AskUserQuestion` to ask:

1. **Plan name** — kebab-case directory name
2. **Plan title** — Human-readable title (e.g., "Error Recovery System")
3. **Goal** — One-line description of what this plan accomplishes
4. **Rough scope** — Which parts of the compiler/runtime/stdlib does this touch? (crates, subsystems, features)

Do NOT ask for sections yet. Sections emerge from research, not from guessing.

### Step 2: Read the Template & Hygiene Rules

Read `plans/_template/plan.md` for the structure reference.

The full rule set is embedded below (source of truth files — do not maintain separate copies). Use these rules when structuring plan sections to ensure plans account for registration sync points, file size limits, phase boundary discipline, and other hygiene requirements from the start.

**Hygiene Rules** (`.claude/rules/impl-hygiene.md`):
@.claude/rules/impl-hygiene.md

**Compiler Guidelines** (`.claude/rules/compiler.md`):
@.claude/rules/compiler.md

---

## Phase 2: Deep Codebase Research (MANDATORY — NO SHORTCUTS)

**THIS IS THE MOST IMPORTANT PHASE.** You MUST deeply understand the existing codebase before writing a single line of the plan. Every claim in the plan must be grounded to actual code — no assumptions, no guessing.

### Step 3: Launch Parallel Research Agents

Based on the user's stated scope, launch **3-5 parallel research agents** using the Agent tool. Each agent explores a different dimension of the codebase. Tailor the agents to the specific plan topic.

**Every agent MUST be instructed to:**
- Read actual source files (not just file names)
- Report exact file paths, line numbers, function signatures, type definitions
- Report what EXISTS today — not what they think should exist
- Flag anything ambiguous or surprising for user clarification
- NO assumptions — if something is unclear, say "UNCLEAR: {what}" rather than guessing

**Standard research agents** (adapt to the specific plan):

#### Agent A: Current Implementation Survey

```
You are researching the Ori compiler codebase for plan creation. Your job is to deeply understand the CURRENT state of the code related to: {topic/scope}.

Read CLAUDE.md first.

INSTRUCTIONS:
1. Find ALL files, types, functions, traits, and modules related to {topic}
   - Use Glob to find files by name patterns
   - Use Grep to find type/function/trait definitions
   - READ the actual source code of every file you find (not just names)
2. For each relevant file, report:
   - Full path
   - Line count (total, production, test)
   - Key types/structs/enums defined (with field signatures)
   - Key functions (with full signatures)
   - Imports and dependencies (what does this file depend on?)
   - Exports (what does this file expose to other crates?)
3. Map the data flow:
   - How does data enter this subsystem?
   - What transformations happen?
   - How does data leave?
   - What types cross crate boundaries?
4. Report ALL existing tests for this area:
   - Test file locations
   - What each test covers
   - Any #[ignore] tests and their reasons
   - Gaps in test coverage you notice
5. Report existing related plans/docs:
   - Check plans/ directory for related plans
   - Check docs/ for relevant spec or design docs
   - Check CLAUDE.md memory for relevant entries

OUTPUT FORMAT:
For each file, provide:
  PATH: {full path}
  LINES: {count}
  KEY TYPES: {list with signatures}
  KEY FUNCTIONS: {list with signatures}
  DEPENDENCIES: {what it imports}
  EXPORTS: {what it exposes}
  TESTS: {test file path and coverage summary}
  NOTES: {anything surprising, unclear, or noteworthy}

End with:
  UNCLEAR: {list of anything you couldn't determine}
  EXISTING_BUGS: {any bugs or issues you noticed while reading}
```

#### Agent B: Integration Points & Boundaries

```
You are researching the Ori compiler codebase for plan creation. Your job is to map every integration point and boundary that {topic/scope} touches.

Read CLAUDE.md first.

INSTRUCTIONS:
1. Identify every crate that {topic} touches or will need to touch
2. For each crate boundary:
   - What types cross the boundary? (Read the actual pub types)
   - What functions are called across the boundary? (Read actual call sites)
   - What registration/sync points exist? (enums, match arms, if-chains that must stay in sync)
3. Map the full pipeline flow for {topic}:
   - Lexer → Parser → IR → Types → Eval → LLVM → Runtime
   - At each stage, what representation does {topic} have?
   - Where are the hand-off points?
4. Identify existing patterns:
   - How are SIMILAR features implemented? (Find 2-3 analogous features)
   - Read their implementation end-to-end
   - Report the exact pattern (files, types, registration points)
   - This is the pattern the plan MUST follow
5. Check for registration sync requirements:
   - Enum variants that must be added in multiple places
   - Match arms that must stay in sync
   - Test arrays/lists that enumerate all variants
   - Registry entries that must be updated

OUTPUT FORMAT:
  CRATES_TOUCHED: {list}
  BOUNDARY_TYPES: {for each boundary, the types that cross it}
  PIPELINE_FLOW: {stage-by-stage representation}
  ANALOGOUS_FEATURES: {2-3 similar features with their implementation pattern}
  SYNC_POINTS: {every enum/match/registry that must stay in sync}
  UNCLEAR: {anything ambiguous}
  EXISTING_BUGS: {bugs noticed while reading}
```

#### Agent C: Existing Tests, Spec, & Prior Art

```
You are researching the Ori compiler codebase for plan creation. Your job is to understand the test infrastructure, spec requirements, and prior art for {topic/scope}.

Read CLAUDE.md first.

INSTRUCTIONS:
1. Find ALL existing tests related to {topic}:
   - Rust unit tests (tests.rs files)
   - Rust integration tests (ori_llvm/tests/aot/)
   - Ori spec tests (tests/spec/)
   - Valgrind tests (tests/valgrind/)
   - Read the actual test code, not just file names
2. Check the spec:
   - Read relevant sections of docs/ori_lang/v2026/spec/
   - Read grammar.ebnf for syntax rules
   - Read operator-rules.md if operators are involved
   - Report what the spec says about this topic
3. Check for prior art in reference repos:
   - Look at ~/projects/reference_repos/lang_repos/ for how other compilers handle this
   - Focus on Rust, Swift, Koka, Lean4 for ARC/memory topics
   - Focus on Gleam, Elm, Roc for type system topics
   - Report specific file paths and patterns from reference implementations
4. Check existing plans:
   - Read plans/ directory for related or superseded plans
   - Report any existing plan items that overlap with this topic
   - Report any completed plan items that this plan builds on
5. Check CLAUDE.md and memory for relevant context

OUTPUT FORMAT:
  EXISTING_TESTS: {list with paths and coverage}
  SPEC_REQUIREMENTS: {what the spec mandates}
  PRIOR_ART: {reference implementations with file paths}
  RELATED_PLANS: {existing plans that overlap}
  UNCLEAR: {anything ambiguous}
  EXISTING_BUGS: {bugs found in tests or spec compliance}
```

#### Agent D: Runtime & Codegen State (if the plan touches runtime/LLVM)

```
You are researching the Ori compiler codebase for plan creation. Your job is to understand the runtime and codegen state for {topic/scope}.

Read CLAUDE.md first.

INSTRUCTIONS:
1. Read the relevant runtime code in compiler/ori_rt/src/:
   - What C-ABI functions exist for this feature?
   - What data layouts are used?
   - What memory management patterns (RC inc/dec, COW, SSO)?
2. Read the relevant codegen code in compiler/ori_llvm/src/:
   - How is this feature lowered to LLVM IR?
   - What builtins are emitted?
   - How does the ARC pipeline interact?
3. Read the ARC pipeline if relevant (compiler/ori_arc/src/):
   - How does the optimizer analyze this feature?
   - What contracts/lattice states apply?
   - What rewrite rules fire?
4. Check for eval/LLVM divergence:
   - Compare ori_eval handling with ori_llvm handling
   - Are there known behavioral differences?
   - Run `grep -r "TODO\|FIXME\|HACK\|WORKAROUND" {relevant files}`
5. Check diagnostic scripts:
   - What diagnostic tools exist for this area?
   - What environment variables control debugging?

OUTPUT FORMAT:
  RUNTIME_FUNCTIONS: {C-ABI functions with signatures}
  CODEGEN_PATTERNS: {how LLVM IR is generated}
  ARC_INTERACTION: {optimizer analysis and rewrites}
  EVAL_LLVM_DIVERGENCE: {known differences}
  DEBUG_TOOLS: {relevant diagnostic scripts/env vars}
  UNCLEAR: {anything ambiguous}
  EXISTING_BUGS: {bugs found while reading}
```

#### Agent E: File Size & Hygiene Pre-Scan (ALWAYS include this agent)

```
You are researching the Ori compiler codebase for plan creation. Your job is to audit the hygiene state of all files that will be touched by {topic/scope}.

Read CLAUDE.md first, then read .claude/rules/impl-hygiene.md and .claude/rules/compiler.md.

INSTRUCTIONS:
1. Find all files that will likely be touched based on the scope: {topic}
2. For EACH file, report:
   - Full path
   - Line count (use wc -l via Bash)
   - Whether it exceeds the 500-line limit
   - Any existing TODOs, FIXMEs, HACKs, WORKAROUNDs
   - Any dead code or stale comments you notice
   - Any registration sync points that are already out of sync
3. Check for phase boundary violations:
   - Does any file import from a crate it shouldn't?
   - Is internal state leaking through boundary types?
4. Check test file conventions:
   - Are tests in sibling tests.rs files (not inline)?
   - Any #[cfg(test)] mod tests blocks that should be extracted?
5. Produce a hygiene summary:
   - Clean files (no issues)
   - Files with issues (categorized: BLOAT/WASTE/DRIFT/EXPOSURE/LEAK/STYLE)
   - Priority files that need splitting before the plan can proceed

OUTPUT FORMAT:
  FILES_TOUCHED: {list with line counts}
  OVER_LIMIT: {files > 500 lines}
  HYGIENE_ISSUES: {categorized findings with file:line}
  SYNC_VIOLATIONS: {any already-broken sync points}
  PRIORITY_SPLITS: {files that must be split before work begins}
  UNCLEAR: {anything ambiguous}
```

### Step 4: Synthesize Research Findings

After ALL research agents complete, synthesize their findings into a structured research summary. This summary is for YOUR reference to write the plan — do NOT write it to a file.

Compile:
1. **Complete file inventory** — every file that will be touched, with line counts and current state
2. **Data flow map** — how data moves through the system for this feature
3. **Analogous feature pattern** — the exact pattern (files, types, registrations) that similar features follow
4. **All sync points** — every enum, match, registry that must be updated together
5. **All existing tests** — what's covered, what's missing
6. **All unclear items** — things the research couldn't determine
7. **All existing bugs found** — bugs discovered during research (these go into the plan)
8. **Hygiene pre-scan** — files that need splitting or cleanup

### Step 5: User Clarification Round (MANDATORY)

**You MUST use `AskUserQuestion` here.** Present the research findings and ask targeted questions:

1. **Report what you found**: Summarize the current state of the codebase for this feature area. Include key types, files, patterns, and any surprises.
2. **Resolve unclear items**: For every "UNCLEAR" item from research, ask the user.
3. **Confirm the analogous pattern**: "Feature X follows this pattern: {pattern}. Should this plan follow the same pattern?"
4. **Propose sections**: Based on research, propose specific sections with rationale. "Based on the codebase, I propose these sections: {list with reasons}. Does this align with your vision?"
5. **Report existing bugs**: "During research, I found these existing issues: {list}. Per zero-deferral, these will be included in the plan."
6. **Ask about scope decisions**: If research revealed the scope is larger or smaller than expected, ask about adjustments.
7. **Ask about design trade-offs**: If there are multiple implementation approaches, present them with pros/cons from the research.

**Do NOT proceed to Phase 3 until the user responds.** Wait for their input. If they redirect or adjust scope, return to research agents for the new scope.

---

## Phase 3: Plan Writing (Research-Grounded)

**RULE: Every technical claim in the plan must trace back to a specific finding from Phase 2.** No assumptions. No guessing. If you can't ground a claim to actual code you read, you either need more research or the claim doesn't belong in the plan.

### Step 6: Create Directory Structure

Create the plan directory and files:

```
plans/{name}/
├── index.md           # Keyword index for discovery
├── 00-overview.md     # High-level goals and section summary
├── section-01-*.md    # First section
├── section-02-*.md    # Additional sections...
└── section-NN-*.md    # Final section
```

### Step 7: Generate 00-overview.md

Create overview following the template in `plans/_template/plan.md`. Ground every element in research:

- **Architecture diagram**: Based on the actual data flow map from research
- **Design principles**: Based on patterns observed in analogous features
- **Section dependency graph**: Based on actual crate dependencies and sync points found
- **Implementation sequence**: Based on the analogous feature pattern — follow the same order
- **Known bugs**: Include ALL bugs found during research Phase 2
- **Metrics**: Use actual line counts from the hygiene pre-scan

### Step 8: Generate index.md

Create the keyword index with:
- **Reroute frontmatter** (if this is a reroute plan — i.e., a parallel track alongside the main roadmap):
  ```yaml
  ---
  reroute: true
  name: "{Short Name}"
  full_name: "{Full Plan Name}"
  status: queued
  order: N
  ---
  ```
  The `name`, `full_name`, `status`, and `order` fields are the single source of truth for the website.
  `order` controls queue priority — lower value = promoted first (default 999 if omitted).
  `key` and `dir` are derived at load time from the directory name.
- Maintenance notice at the top
- How to use instructions
- Keyword cluster for each section — use REAL keywords from the research (actual type names, function names, file names, not placeholders)
- Quick reference table

### Step 9: Generate Section Files

For each section, create `section-{NN}-{name}.md` following the template. **Every section must be grounded:**

- **File paths**: Use EXACT paths from research (verified to exist)
- **Type signatures**: Use EXACT signatures from research (copy from source)
- **Function references**: Use EXACT function names from research
- **Registration sync points**: List ALL sync points from research for any new enum variant/type/entry
- **Analogous pattern**: Reference the analogous feature's implementation pattern — "Follow the same pattern as {feature} in {files}"
- **Code examples**: Show target implementation based on actual code patterns found during research, not invented patterns
- **Test strategy**: Based on existing test patterns found in Phase 2

**Frontmatter includes:**
- Section ID, title, status: not-started, goal
- `reviewed` field (see rules below)
- `inspired_by` with actual reference implementations found
- `depends_on` based on actual crate dependency chain
- `third_party_review: { status: none, updated: null }`
- `## {NN}.R Third Party Review Findings` block (empty, with `- None.`) before the completion checklist
- Completion checklist at the end

**`reviewed` field rules:**
- **Section 01**: `reviewed: true` — it is the starting point of implementation and was validated during plan creation against the research findings.
- **All other sections (02+)**: `reviewed: false` — they have NOT been validated against actual implementation reality. As Section 01 is implemented, assumptions in later sections may become stale or wrong.

### Step 10: Self-Check Before Review

Before proceeding to review, do a quick self-audit:

1. **Every file path in the plan** — verify it exists in the codebase (use Glob)
2. **Every function/type reference** — verify it exists (use Grep)
3. **Every registration sync point** — verify the list is complete
4. **No placeholder content** — no "TBD", no "placeholder keywords", no "to be determined"
5. **No assumptions** — every technical claim traces to research

Fix any issues found.

---

## Phase 4: Review

### Step 11: Report Progress

Show the user:
- Files created (with paths)
- Brief summary of what each section covers
- Note: "Running /review-plan for formal review..."

### Step 12: Run /review-plan (MANDATORY — USE THE ACTUAL SKILL)

**CRITICAL: Run the actual `/review-plan` skill using the Skill tool.** Do NOT reimplement the review logic. Do NOT spawn your own review agents. Use the Skill tool to invoke `/review-plan` with the plan directory path as the argument.

```
Skill: review-plan
Args: plans/{name}/
```

This runs the formal 4-agent review pipeline (Technical Accuracy, Completeness, Hygiene, Clarity) as defined in the `/review-plan` skill. It will edit the plan files directly to fix any issues.

### Step 13: Post-Review Summary

After `/review-plan` completes, report to the user:
- The review verdict
- What the review changed
- Any remaining concerns that need human judgement

### Step 14: Ask About Reroute Status

Use `AskUserQuestion` to ask the user whether this plan should be the active reroute. This determines the `reroute` frontmatter in `index.md`.

If the user says **yes**: add reroute frontmatter to `index.md` with `status: active` and `order: 1`.
If the user says **queued**: add reroute frontmatter with `status: queued` and ask for the `order` value.
If the user says **no**: do not add reroute frontmatter (plan is not a reroute).

---

## Example

**Input:** `/create-plan error-recovery "Improve compiler error messages and recovery"`

**Phase 1**: Read CLAUDE.md, ask user about scope ("Which crates? Which error types?")
**Phase 2**: Launch 5 parallel research agents exploring ori_diagnostic, ori_types errors, ori_parse recovery, spec error codes, reference compiler error systems. Synthesize findings. Ask user: "Found 117 error codes, 64 with docs. The pattern for adding errors follows {pattern}. Propose these sections: {list}."
**Phase 3**: Write plan with exact file paths, type signatures, and sync points from research.
**Phase 4**: Run `/review-plan plans/error-recovery/` via Skill tool.

**Creates:**
```
plans/error-recovery/
├── index.md
├── 00-overview.md
├── section-01-error-types.md
├── section-02-recovery-strategies.md
└── section-03-user-facing-messages.md
```

---

## Section Naming Conventions

| Section Type | Naming Pattern |
|--------------|----------------|
| Setup/Infrastructure | `section-01-setup.md` |
| Core Implementation | `section-02-core.md` |
| Integration | `section-03-integration.md` |
| Testing | `section-04-testing.md` |
| Documentation | `section-05-docs.md` |

---

## Anti-Deferral Rule for Plan Items

**Every checklist item in a plan must be implementable by the agent executing that section.** When writing plan items:

- Do NOT use soft language that invites skipping: "bonus", "future", "lower priority", "nice to have", "if time permits", "stretch goal".
- Do NOT label items "requires architectural change" — architectural changes are implementation tasks, not deferrals. If a 30-line change across 3 files is needed, describe the change and make it a checkbox.
- Do NOT create items that are descriptions of work rather than work itself. "Investigate whether X" is acceptable; "Document the approach for Y" when Y can be implemented is not.
- If an item genuinely cannot be done within the section (blocked by an unimplemented language feature, needs user decision), use `<!-- blocked-by:X -->` with a concrete blocker reference — not vague language.
- Every item must pass this test: "Can the implementing agent, with access to the codebase, complete this item in a single session?" If no, break it into items that can.

## Zero Assumptions Rule

**ABSOLUTE — NO EXCEPTIONS.** Every technical claim in the plan must be grounded to something found during research:

- **File paths**: Must exist in the codebase (verified by Glob/Read)
- **Type/function signatures**: Must match actual source (verified by reading the file)
- **Behavior descriptions**: Must match actual code behavior (verified by reading the implementation)
- **Registration sync points**: Must be the complete list (verified by Grep for all match arms / enum variants)
- **Patterns to follow**: Must reference actual analogous implementations (verified by reading them)

If you cannot verify a claim, it MUST be flagged as `<!-- UNVERIFIED: {reason} -->` and reported to the user in Step 5. Unverified claims are not acceptable in the final plan — they must be resolved before Phase 3 or removed.

## Reviewed Field Semantics

The `reviewed: true/false` field in section frontmatter is a **pre-implementation gate** — it tracks whether a section has been validated against the current codebase right before you start implementing it.

**Why this exists:** Plans are written with assumptions about how the code works. But as you implement Section 01, reality changes — deviations, discoveries, refactors, bug fixes. A section written before prior sections were implemented may reference stale file paths, wrong function signatures, or invalid approaches. `reviewed: false` means "not yet validated against implementation reality."

**Rules:**
- **Section 01** is always `reviewed: true` at creation — it's the starting point.
- **All other sections** are `reviewed: false` at creation — plans, not validated reality.
- **Single-section review** (`/review-plan plans/foo/section-03.md`): This is the pre-implementation gate. After confirming accuracy, flip to `reviewed: true`.
- **Whole-plan review** (`/review-plan plans/foo/`): Fixes issues, improves quality, but does NOT change `reviewed` values. You're improving the plan holistically, not gating specific sections.
- **`/continue-roadmap`** starting a `reviewed: false` section: triggers a single-section review first, which flips to `true` after validation.

---

## After Creation

Remind the user to:
1. Fill in any remaining section details with specific tasks
2. Update `00-overview.md` with dependencies and success criteria if not already complete
3. **If performance-sensitive** (lexer, parser, typeck, eval, codegen): Add `/benchmark` checkpoints to relevant sections

## Performance-Sensitive Plans

For plans touching hot paths, include a "Performance Validation" section in `index.md`:

```markdown
## Performance Validation

Use `/benchmark short` after modifying hot paths.

**When to benchmark:** [list specific sections]
**Skip benchmarks for:** [list non-perf sections]
```

See `plans/_template/plan.md` for full guidance.

---

## Template Reference

The command uses `plans/_template/plan.md` as the structure reference. See that file for:
- Complete index.md template
- Section file template
- Status conventions
- The roadmap (`plans/roadmap/`) as a working example
