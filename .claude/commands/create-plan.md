# Create Plan Command

Create a new plan directory with index and section files using the standard template. **Research-first, architecture-second, sections-last**: deeply understand the existing codebase, design the architecture, then write sections sequentially.

## Usage

```
/create-plan <name> [description]
/create-plan <add xyz to roadmap>
```

- `name`: Directory name for the plan (kebab-case, e.g., `error-recovery`, `lsp-integration`)
- `description`: Optional one-line description of the plan's goal
- **Roadmap mode**: If the name/description indicates adding to the roadmap (e.g., "add pattern matching to roadmap", "roadmap: closures"), this command operates in **Roadmap Mode** — see the dedicated section below.

---

## Mode Detection

**New Plan Mode** (default): The argument names a new plan directory. Creates `plans/{name}/` from scratch.

**Roadmap Mode**: The argument indicates adding a section to the existing roadmap. Detected when the input contains "roadmap" or references an existing roadmap section. Operates on `plans/roadmap/` instead of creating a new directory.

Both modes follow the SAME research rigor, the SAME iterative deepening, the SAME sequential writing discipline. The difference is the target: a new plan vs. an existing one.

---

## Design Principles

These principles govern the entire plan creation process. When in doubt, consult these:

1. **Research depth > research breadth** — One agent that reads 15 files thoroughly beats 5 agents that scan 50 files superficially. Understanding invariants, control flow, and edge cases matters more than listing type signatures.

2. **Architecture before sections** — The overview isn't boilerplate. It's the load-bearing design document. Sections are *implementations of* the architecture, not independent documents. Design first, detail second.

3. **Sequential section writing is non-negotiable** — Sections depend on each other. Section 3 references decisions made in Section 2. Parallel writing forces each section to *guess* what other sections decided, producing contradictions. Write one section at a time, in order.

4. **User checkpoints at design-level decisions** — Don't ask the user to review 8 completed sections. Ask them to review the architecture *first*, then write sections they've already conceptually agreed to.

5. **Iterative deepening over parallel breadth** — Start wide, then go deep on what matters. Each research pass builds on the findings of the prior pass.

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

## Phase 2: Multi-Pass Research (MANDATORY — NO SHORTCUTS)

**THIS IS THE MOST IMPORTANT PHASE.** You MUST deeply understand the existing codebase before designing architecture or writing sections. Every claim in the plan must be grounded to actual code — no assumptions, no guessing.

Research uses **iterative deepening** — four sequential passes, each building on the findings of the prior pass. Passes 1 and 2 may use parallel agents for breadth. Passes 3 and 4 are focused, sequential deep-dives.

### Step 3: Pass 1 — Breadth Scan (parallel agents)

Launch **2-4 parallel agents** to build an inventory of everything relevant. This pass answers: **what exists?**

**Every agent MUST be instructed to:**
- Read actual source files (not just file names)
- Report exact file paths, line numbers, function signatures, type definitions
- Report what EXISTS today — not what they think should exist
- Flag anything ambiguous or surprising as `UNCLEAR: {what}`
- NO assumptions — if something is unclear, say so rather than guessing

Tailor agents to the specific plan topic. Standard agents:

#### Agent 1: Implementation & Boundary Survey

```
You are researching the Ori compiler codebase for plan creation. Your job is to build a complete inventory of everything related to: {topic/scope}.

Read CLAUDE.md first.

PART A — Implementation Inventory:
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
3. Report ALL existing tests for this area:
   - Test file locations and what each test covers
   - Any #[ignore] tests and their reasons
   - Gaps in test coverage you notice

PART B — Integration Points & Boundaries:
1. Identify every crate that {topic} touches or will need to touch
2. For each crate boundary:
   - What types cross the boundary? (Read the actual pub types)
   - What functions are called across the boundary? (Read actual call sites)
   - What registration/sync points exist? (enums, match arms, if-chains that must stay in sync)
3. Map the full pipeline flow for {topic}:
   - Lexer → Parser → IR → Types → Eval → LLVM → Runtime
   - At each stage, what representation does {topic} have?
   - Where are the hand-off points?
4. Check for registration sync requirements:
   - Enum variants that must be added in multiple places
   - Match arms that must stay in sync
   - Test arrays/lists that enumerate all variants
   - Registry entries that must be updated

OUTPUT FORMAT:
For each file:
  PATH: {full path}
  LINES: {count}
  KEY TYPES: {list with signatures}
  KEY FUNCTIONS: {list with signatures}
  DEPENDENCIES: {what it imports}
  EXPORTS: {what it exposes}
  TESTS: {test file path and coverage summary}
  NOTES: {anything surprising, unclear, or noteworthy}

Then:
  CRATES_TOUCHED: {list}
  BOUNDARY_TYPES: {for each boundary, the types that cross it}
  PIPELINE_FLOW: {stage-by-stage representation}
  SYNC_POINTS: {every enum/match/registry that must stay in sync}
  UNCLEAR: {list of anything you couldn't determine}
  EXISTING_BUGS: {any bugs or issues you noticed while reading}
```

#### Agent 2: Tests, Spec, & Hygiene Audit

```
You are researching the Ori compiler codebase for plan creation. Your job is to understand the test landscape, spec requirements, and hygiene state for {topic/scope}.

Read CLAUDE.md first, then read .claude/rules/impl-hygiene.md and .claude/rules/compiler.md.

PART A — Tests & Spec:
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
3. Check existing plans:
   - Read plans/ directory for related or superseded plans
   - Report any existing plan items that overlap with this topic
   - Report any completed plan items that this plan builds on
4. Check CLAUDE.md and memory for relevant context

PART B — Hygiene Audit:
1. Find all files that will likely be touched based on the scope: {topic}
2. For EACH file, report:
   - Full path and line count
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
  EXISTING_TESTS: {list with paths and coverage}
  SPEC_REQUIREMENTS: {what the spec mandates}
  RELATED_PLANS: {existing plans that overlap}
  FILES_TOUCHED: {list with line counts}
  OVER_LIMIT: {files > 500 lines}
  HYGIENE_ISSUES: {categorized findings with file:line}
  SYNC_VIOLATIONS: {any already-broken sync points}
  PRIORITY_SPLITS: {files that must be split before work begins}
  UNCLEAR: {anything ambiguous}
  EXISTING_BUGS: {bugs found in tests, spec compliance, or hygiene}
```

#### Agent 3: Runtime & Codegen State (if the plan touches runtime/LLVM)

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
   - Grep for TODO|FIXME|HACK|WORKAROUND in relevant files
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

### Step 4: Pass 2 — Deep Read (sequential, focused)

**After Pass 1 agents complete**, identify the **10-15 most critical files** from their findings. These are the files where the plan's core logic lives — not periphery.

**You (the main agent) or a single focused agent MUST now read these files thoroughly.** Not scan for signatures — read the actual logic. Understand:

1. **Invariants**: What properties does this code maintain? What `debug_assert!`s exist? What would break if those invariants were violated?
2. **Control flow**: How does execution actually flow through this code? What are the error paths? What are the edge cases?
3. **State mutations**: What state changes? Where? In what order? What are the pre/post conditions?
4. **Why it works this way**: Look for comments explaining design decisions. Look at git blame for recent changes. Understand the *reasoning*, not just the *structure*.
5. **What would break**: If you changed X, what else would need to change? What tests would fail? What invariants would be violated?

**Output**: For each critical file, write a paragraph (not a list) explaining how the code works, what invariants it maintains, and what would break if changed. This understanding is what grounds the plan.

**This step cannot be parallelized.** Each file read may inform what to look for in the next file. If reading file A reveals that it delegates to file B in a non-obvious way, read file B next.

### Step 5: Pass 3 — Pattern Study (single focused agent)

Launch **one agent** to trace 2-3 analogous features end-to-end through the compiler pipeline. These are features that already exist and follow the same structural pattern that the new plan will need.

```
You are studying implementation patterns in the Ori compiler. Your job is to trace analogous features end-to-end to discover the exact implementation pattern that {topic/scope} should follow.

Read CLAUDE.md first.

INSTRUCTIONS:
1. Identify 2-3 features ALREADY IMPLEMENTED in the compiler that are structurally similar to {topic}. Examples:
   - If adding a new collection type: trace how Map or Set was implemented
   - If adding a new trait: trace how Comparable or Hashable was implemented
   - If adding a new expression form: trace how match or for-yield was implemented
   - If adding codegen support: trace how an existing feature flows through ori_llvm

2. For EACH analogous feature, trace the COMPLETE implementation through every compiler phase:
   a. Lexer: What tokens? (compiler/ori_lexer/src/)
   b. Parser: What AST nodes? (compiler/ori_parse/src/)
   c. IR: What IR representation? (compiler/ori_ir/src/)
   d. Type checker: What type rules? (compiler/ori_types/src/)
   e. Registry: What method/type registrations? (compiler/ori_registry/src/)
   f. Evaluator: What evaluation logic? (compiler/ori_eval/src/)
   g. ARC pipeline: What memory analysis? (compiler/ori_arc/src/)
   h. LLVM codegen: What IR generation? (compiler/ori_llvm/src/)
   i. Runtime: What C-ABI support? (compiler/ori_rt/src/)
   j. Stdlib: What library support? (library/std/)
   k. Tests: What test files and patterns? (tests/spec/, */tests.rs)

3. For each phase, READ THE ACTUAL CODE. Report:
   - Exact file path and function/type names
   - How data enters and leaves that phase
   - What registration/sync points were needed
   - What the implementation pattern is (not just "it exists" but "here's how it works")

4. Synthesize the pattern:
   - What is the exact sequence of files to create/modify?
   - What is the exact sequence of types/enums/match-arms to add?
   - What is the order of operations? (What must come first?)
   - Where did the analogous feature deviate from the expected pattern, and why?

OUTPUT FORMAT:
For each analogous feature:
  FEATURE: {name}
  PIPELINE TRACE:
    LEXER: {file, tokens, how it works}
    PARSER: {file, AST nodes, how it works}
    IR: {file, IR types, how it works}
    TYPECK: {file, type rules, how it works}
    REGISTRY: {file, registrations, how it works}
    EVAL: {file, eval logic, how it works}
    ARC: {file, analysis, how it works}
    LLVM: {file, codegen, how it works}
    RUNTIME: {file, C-ABI, how it works}
    STDLIB: {file, library support, how it works}
    TESTS: {files, patterns, coverage}
  SYNC_POINTS: {all registration points that had to stay in sync}
  ORDER_OF_OPERATIONS: {what was built first, second, third}
  DEVIATIONS: {where this feature broke the expected pattern}

Then:
  RECOMMENDED_PATTERN: {the pattern the new plan should follow}
  RECOMMENDED_ORDER: {the order in which phases should be implemented}
  PATTERN_RISKS: {where the new feature might need to deviate from the pattern}
```

### Step 6: Pass 4 — Prior Art Study (single focused agent)

Launch **one agent** to study reference compilers for the specific design decisions this plan will face. Not "how does Rust work generally" — "how does Rust solve *this specific problem*."

```
You are studying prior art in reference compiler implementations. Your job is to find how other compilers handle the specific design decisions that {topic/scope} will face.

Read CLAUDE.md first for reference repo locations.

INSTRUCTIONS:
1. Identify the 2-4 specific DESIGN DECISIONS this plan will need to make. Examples:
   - "Should X use static dispatch or dynamic dispatch?"
   - "Should X be represented in the IR or desugared earlier?"
   - "How should X interact with the ARC pipeline?"
   - "What error messages should X produce?"

2. For EACH design decision, check the reference repos at ~/projects/reference_repos/lang_repos/:
   - Rust, Swift, Koka, Lean4 for ARC/memory topics
   - Gleam, Elm, Roc for type system topics
   - Go, Zig, TypeScript for general patterns

3. For each reference implementation you find:
   - Read the ACTUAL CODE (not just file names)
   - Understand their design choice and WHY they made it
   - Note the trade-offs they accepted
   - Note any bugs or limitations in their approach

4. Synthesize design recommendations:
   - For each design decision, recommend an approach with evidence
   - Cite specific files and patterns from reference implementations
   - Explain which reference implementation's approach best fits Ori's constraints

OUTPUT FORMAT:
For each design decision:
  DECISION: {what needs to be decided}
  REFERENCE IMPLEMENTATIONS:
    {Language}: {file path} — {their approach and why}
    {Language}: {file path} — {their approach and why}
  RECOMMENDATION: {what Ori should do}
  EVIDENCE: {why, citing specific reference impl trade-offs}
  RISKS: {what could go wrong with this approach}
```

**Note**: Passes 3 and 4 CAN run in parallel with each other (they are independent), but both MUST wait for Passes 1-2 to complete (they depend on knowing what files and code are relevant).

---

## Phase 3: Architecture Design (REQUIRED BEFORE SECTION WRITING)

This phase synthesizes all research into a cohesive architecture. **No sections are written until the architecture is designed and the user approves it.**

### Step 7: Synthesize Research into Architecture

After ALL research passes complete, synthesize findings into a structured architecture. Compile:

1. **Complete file inventory** — every file that will be touched, with line counts and current state
2. **Deep understanding summary** — for each critical file, how the code works, what invariants it maintains, what would break (from Pass 2)
3. **Implementation pattern** — the exact pattern that analogous features follow, and how this plan should follow it (from Pass 3)
4. **Design decisions** — for each decision, the recommended approach with evidence from prior art (from Pass 4)
5. **All sync points** — every enum, match, registry that must be updated together
6. **All existing tests** — what's covered, what's missing
7. **All unclear items** — things the research couldn't determine
8. **All existing bugs found** — bugs discovered during research (these go into the plan)
9. **Hygiene pre-scan** — files that need splitting or cleanup
10. **Dependency chain** — what must be built first, what gates what, what can be parallelized

### Step 8: Write `00-overview.md` FIRST

The overview is the **load-bearing design document**. It is NOT boilerplate filled in after sections are written — it is the architectural blueprint that DRIVES section content.

Write `00-overview.md` following the template in `plans/_template/plan.md`, grounding every element in research:

- **Mission**: Based on the actual problem discovered during research — what exists, what's broken, what's missing
- **Architecture diagram**: Based on the actual data flow map from Pass 2's deep read — show how data enters, transforms, and exits
- **Design principles**: Based on patterns observed in analogous features (Pass 3) and prior art (Pass 4) — cite the specific evidence
- **Section dependency graph**: Based on actual crate dependencies and sync points found in Pass 1 — show which sections gate others
- **Implementation sequence**: Based on the analogous feature pattern from Pass 3 — follow the same order that worked before
- **Design decisions**: Include the key design decisions from Pass 4 with recommended approaches and evidence
- **Known bugs**: Include ALL bugs found during research passes
- **Metrics**: Use actual line counts from the hygiene pre-scan

**Also create `index.md`** with keyword clusters using REAL keywords from the research (actual type names, function names, file names — not placeholders).

### Step 9: User Review of Architecture (MANDATORY — DO NOT SKIP)

**You MUST use `AskUserQuestion` here.** Present the architecture and get explicit buy-in before writing sections.

Present:
1. **The architecture**: Summarize the design from `00-overview.md` — mission, data flow, key design decisions
2. **The proposed sections**: List each section with its goal, what files it touches, and what it depends on. Explain WHY these sections and WHY this order.
3. **Design decisions**: For each key design decision, present the recommended approach with evidence. Ask if the user agrees or wants a different approach.
4. **Analogous pattern**: "Feature X follows this pattern: {pattern}. This plan will follow the same pattern. Does this align with your vision?"
5. **Resolve unclear items**: For every `UNCLEAR` item from research, ask the user.
6. **Report existing bugs**: "During research, I found these existing issues: {list}. Per zero-deferral, these will be included in the plan."
7. **Scope adjustments**: If research revealed the scope is larger or smaller than expected, propose adjustments with rationale.

**Do NOT proceed to Phase 4 until the user responds and approves the architecture.** If they redirect or adjust scope, update the overview and re-present. If they change design decisions, update accordingly. The architecture must be agreed upon before sections are detailed.

---

## Phase 4: Sequential Section Writing (MANDATORY SEQUENTIAL — NO PARALLELISM)

**CRITICAL RULE: Write sections ONE AT A TIME, IN ORDER.** Do NOT launch parallel agents to write sections. Each section depends on decisions and details from prior sections. Section N is not written until Section N-1 is complete.

### Step 10: Create Directory Structure

Create the plan directory:

```
plans/{name}/
├── index.md           # Already created in Step 8
├── 00-overview.md     # Already created in Step 8
├── section-01-*.md    # Written sequentially starting here
├── section-02-*.md    # Written after section-01 is complete
└── section-NN-*.md    # Written after all prior sections are complete
```

### Step 11: Write Sections Sequentially

For each section, in order from 01 to N:

**Before writing the section**, re-read:
- The `00-overview.md` architecture (to stay aligned with the design)
- ALL previously written sections (to reference their decisions and avoid contradictions)
- The relevant research findings for this section's scope

**Write the section** following the template in `plans/_template/plan.md`. Every section must be grounded:

- **File paths**: Use EXACT paths from research (verified to exist)
- **Type signatures**: Use EXACT signatures from research (copy from source)
- **Function references**: Use EXACT function names from research
- **Registration sync points**: List ALL sync points from research for any new enum variant/type/entry
- **Analogous pattern**: Reference the analogous feature's implementation pattern — "Follow the same pattern as {feature} in {files}"
- **Code examples**: Show target implementation based on actual code patterns found during research, not invented patterns
- **Test strategy**: Based on existing test patterns found in Phase 2
- **Dependencies on prior sections**: Explicitly reference what earlier sections provide. "This section uses the {type} defined in Section {N} ({file path})."
- **What this section provides to later sections**: State what downstream sections will depend on. "Section {M} will use the {API/type/pattern} established here."

**Frontmatter includes:**
- Section ID, title, status: not-started, goal
- `reviewed` field (see rules below)
- `inspired_by` with actual reference implementations found
- `depends_on` based on actual crate dependency chain AND section content dependencies
- `third_party_review: { status: none, updated: null }`
- `## {NN}.R Third Party Review Findings` block (empty, with `- None.`) before the completion checklist
- Completion checklist at the end

**`reviewed` field rules:**
- **Section 01**: `reviewed: true` — it is the starting point of implementation and was validated during plan creation against the research findings.
- **All other sections (02+)**: `reviewed: false` — they have NOT been validated against actual implementation reality. As Section 01 is implemented, assumptions in later sections may become stale or wrong.

**After writing each section**, briefly verify:
- File paths referenced in this section exist
- Type/function names referenced exist
- References to prior sections are accurate (re-read the referenced section if needed)
- No contradictions with prior sections

Then proceed to the next section.

### Step 12: Update Overview and Index

After all sections are written:
- Update `00-overview.md` with the final section list, dependency graph, and any adjustments that emerged during sequential writing
- Update `index.md` with complete keyword clusters for all sections — using actual type names, function names, and file names from the written sections

---

## Phase 5: Cohesion Review & Finalization

### Step 13: Cohesion Check (NEW — before /review-plan)

Launch **one agent** to read the ENTIRE plan front-to-back and check for internal coherence:

```
You are reviewing a newly created plan for internal coherence. Read EVERY file in the plan directory: {plan_dir}/

Check for:
1. CONTRADICTIONS: Does Section X say one thing and Section Y say another? (e.g., Section 2 says "add variant Foo to enum Bar" but Section 5 says "add variant Baz to enum Bar" for the same purpose)
2. GAPS: Is there work that falls between sections? (e.g., Section 2 produces a type that Section 4 consumes, but no section handles the transformation between them)
3. REDUNDANCY: Do multiple sections do the same work? (e.g., both Section 3 and Section 5 add the same match arm)
4. BROKEN REFERENCES: Does Section X reference a type/file/function from Section Y that Section Y doesn't actually define?
5. ORDERING ISSUES: Does Section X depend on work described in Section Y, but X comes before Y?
6. SYNC POINT COMPLETENESS: Are ALL sync points (enum variants, match arms, registry entries) accounted for across all sections? Is any sync point mentioned in one section but forgotten in its counterpart section?
7. OVERVIEW ALIGNMENT: Does the overview's architecture diagram, dependency graph, and implementation sequence still match what the sections actually describe?

For each issue found, report:
  ISSUE TYPE: {contradiction/gap/redundancy/broken-ref/ordering/sync-gap/overview-drift}
  SECTIONS: {which sections are involved}
  DETAILS: {what the issue is}
  FIX: {how to resolve it}
```

Fix all issues found by the cohesion check before proceeding.

### Step 14: Self-Check Before Review

Do a quick self-audit:

1. **Every file path in the plan** — verify it exists in the codebase (use Glob)
2. **Every function/type reference** — verify it exists (use Grep)
3. **Every registration sync point** — verify the list is complete
4. **No placeholder content** — no "TBD", no "placeholder keywords", no "to be determined"
5. **No assumptions** — every technical claim traces to research
6. **No contradictions** — cohesion check passed clean

Fix any issues found.

### Step 15: Report Progress

Show the user:
- Files created (with paths)
- Brief summary of what each section covers
- Any issues found and fixed during cohesion/self-check
- Note: "Running /review-plan for formal review..."

### Step 16: Run /review-plan (MANDATORY — USE THE ACTUAL SKILL)

**CRITICAL: Run the actual `/review-plan` skill using the Skill tool.** Do NOT reimplement the review logic. Do NOT spawn your own review agents. Use the Skill tool to invoke `/review-plan` with the plan directory path as the argument.

```
Skill: review-plan
Args: plans/{name}/
```

This runs the formal review pipeline as defined in the `/review-plan` skill. It will edit the plan files directly to fix any issues.

### Step 17: Post-Review Summary

After `/review-plan` completes, report to the user:
- The review verdict
- What the review changed
- Any remaining concerns that need human judgement

### Step 18: Ask About Reroute Status

Use `AskUserQuestion` to ask the user whether this plan should be the active reroute. This determines the `reroute` frontmatter in `index.md`.

If the user says **yes**: add reroute frontmatter to `index.md` with `status: active` and `order: 1`.
If the user says **queued**: add reroute frontmatter with `status: queued` and ask for the `order` value.
If the user says **no**: do not add reroute frontmatter (plan is not a reroute).

---

## Example

**Input:** `/create-plan error-recovery "Improve compiler error messages and recovery"`

**Phase 1**: Read CLAUDE.md. Ask user about scope ("Which crates? Which error types?").

**Phase 2**:
- *Pass 1*: Launch 2 parallel agents — (1) survey `ori_diagnostic`, `ori_types` errors, `ori_parse` recovery, all error-related files; (2) audit tests, spec error codes, hygiene state.
- *Pass 2*: Deep-read the 12 most critical files. Understand how `DiagnosticQueue` dedup works, how `ErrorGuaranteed` propagates, how recovery tokens are chosen.
- *Pass 3*: Trace how `E2029` (Hashable-without-Eq) was implemented end-to-end — from type checker detection through diagnostic emission to test coverage. Trace how `E0860` (break-value-in-while) was implemented. Document the exact pattern.
- *Pass 4*: Study Elm's error diffing (`Reporting/Error/Type.hs`), Roc's `to_diff` pattern, Rust's `DiagnosticBuilder` chain pattern. Recommend approaches for Ori.

**Phase 3**: Design architecture. Write `00-overview.md` with data flow, design decisions (Elm-style diffing vs Rust-style chaining), dependency graph. Present to user: "Found 117 error codes, 64 with docs. The E2029 pattern shows {pattern}. Propose these sections in this order: {list}. The key design decision is {X} — I recommend {Y} because {evidence}."

**Phase 4**: After user approves architecture, write sections sequentially:
- Section 01 (error types) → read it → write Section 02 (recovery strategies, building on 01's types) → read both → write Section 03 (user-facing messages, building on 01+02).

**Phase 5**: Cohesion check → self-check → report → run `/review-plan plans/error-recovery/`.

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

If you cannot verify a claim, it MUST be flagged as `<!-- UNVERIFIED: {reason} -->` and reported to the user in Step 9. Unverified claims are not acceptable in the final plan — they must be resolved before Phase 4 or removed.

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

---

## Roadmap Mode

When the input indicates adding to the roadmap (e.g., `/create-plan add closures to roadmap`, `/create-plan roadmap: pattern matching`), this command operates on `plans/roadmap/` instead of creating a new plan directory.

**Same rigor, different target.** Every phase applies identically — the research depth, the iterative deepening, the sequential writing, the cohesion review. The only differences are structural: you're inserting into an existing plan, not creating a fresh one.

### Roadmap Mode: How It Differs

#### Phase 1 Differences

- **Step 1**: Instead of asking for a plan name, identify:
  1. **What feature/section** to add to the roadmap
  2. **Where it fits** — after which existing section? What does it depend on?
  3. **What it might affect** — which existing sections reference related code?

- **Step 2**: In addition to the template and hygiene rules, **read the entire roadmap**:
  - `plans/roadmap/00-overview.md` — understand the mission, architecture, dependency graph
  - `plans/roadmap/index.md` — understand the keyword structure and section numbering
  - **Every existing section file** — understand what's already planned, what's complete, what's in progress
  - Pay attention to: section dependencies, implementation sequence, cross-section interactions

#### Phase 2 Differences

Research is identical in rigor, but adds a roadmap-specific dimension:

- **Pass 1**: In addition to the standard inventory, identify:
  - Which existing roadmap sections touch the same files/types/crates
  - Which existing sections might need updates due to the new section
  - Whether any completed sections already partially cover the new scope

- **Pass 2**: In addition to deep-reading critical files, deep-read:
  - The 2-3 existing roadmap sections most related to the new one
  - Any completed sections that the new section builds on (to understand what was actually implemented vs. what was planned)

#### Phase 3 Differences

- **Step 7**: Synthesis must include:
  - **Impact analysis**: How does the new section affect the existing roadmap? Does it change dependencies? Does it invalidate assumptions in other sections?
  - **Insertion point**: Where in the section numbering does it go? (May require renumbering)
  - **Dependency updates**: Which existing sections need `depends_on` updates?

- **Step 8**: Instead of writing a new `00-overview.md`:
  - **Update** the existing `00-overview.md` — add the new section to the architecture diagram, dependency graph, implementation sequence, quick reference table, and estimated effort
  - **Update** `index.md` — add keyword clusters for the new section
  - If the overview or index format has drifted from the current template (`plans/_template/plan.md`), bring them up to date while you're editing them

- **Step 9**: Present to the user:
  - The proposed new section with its goals and scope
  - The impact on existing sections (what changes, what doesn't)
  - The updated dependency graph showing where the new section fits
  - Any existing sections that need updates and what those updates are

#### Phase 4 Differences

- **Step 11**: Write the new section(s) following the same sequential discipline. If multiple new sections are needed, write them in order.

- **After writing the new section(s)**: Update any existing sections that are affected:
  - Update `depends_on` in sections that now depend on the new section
  - Update cross-references in sections that reference related code
  - Update `00-overview.md` dependency graph and implementation sequence
  - Update `index.md` with the new section's keywords
  - If any existing section's content is now stale or contradicted by the new section, fix it. Flag the section as `reviewed: false` if you changed its assumptions.

#### Phase 5 Differences

- **Step 13**: The cohesion check reads the ENTIRE roadmap (all sections, not just new ones), checking that:
  - The new section is consistent with all existing sections
  - No existing section contradicts the new section
  - The dependency graph in `00-overview.md` is accurate
  - The implementation sequence still makes sense with the new section inserted
  - Cross-references between sections are all valid

- **Step 16**: Run `/review-plan plans/roadmap/` (the full roadmap, not just the new section)

- **Step 18**: Skip the reroute question (the roadmap is the roadmap, not a reroute)

### Roadmap Mode: The "Leave It Better" Rule

**You MUST leave the roadmap in better shape than you found it.** When operating in roadmap mode:

1. **Format drift**: If the roadmap's existing sections don't match the current template format (`plans/_template/plan.md`), update them to match. This includes frontmatter fields, section structure, completion checklists, and third-party review blocks.
2. **Stale content**: If you encounter stale file paths, outdated type signatures, or references to code that no longer exists, fix them.
3. **Missing cross-references**: If sections reference each other implicitly but lack explicit `depends_on` or co-implementation callouts, add them.
4. **Incomplete hygiene**: If sections lack completion checklists, exit criteria, or test strategies, add them.
5. **Overview accuracy**: The overview's architecture diagram, dependency graph, and implementation sequence must accurately reflect the current state of the roadmap after your changes.

This is not optional cleanup — it's a mandatory part of roadmap mode. Every touch of the roadmap is an opportunity to improve its coherence and accuracy.

### Roadmap Mode: Example

**Input:** `/create-plan add pattern matching exhaustiveness to roadmap`

**Phase 1**: Read CLAUDE.md. Read the entire roadmap (overview + all sections). Identify that this relates to type checker work, probably depends on existing Section 07 (type inference), and might affect Section 12 (verification).

**Phase 2**:
- *Pass 1*: Survey exhaustiveness checking code in `ori_types`, find that `ori_types/src/check/exhaustiveness.rs` exists with 340 lines. Find that Section 07 touches `ori_types/src/check/` but doesn't cover exhaustiveness.
- *Pass 2*: Deep-read `exhaustiveness.rs` and the 3 existing roadmap sections most related. Discover that Section 07's completion assumes exhaustiveness works, but the current implementation has gaps for nested patterns.
- *Pass 3*: Trace how Gleam's exhaustiveness checker works end-to-end (`compiler-core/src/exhaustiveness.rs`).
- *Pass 4*: Compare Elm's exhaustiveness approach (algebraic, provably complete) vs Rust's (witness-based).

**Phase 3**: Design the new section. Determine it should be Section 08 (after type inference, before integration). Update `00-overview.md` dependency graph. Present to user: "The new section depends on 07, and Section 12 should depend on it. Here's the impact..."

**Phase 4**: Write Section 08 sequentially. Then update Section 07 (add forward reference), Section 12 (add dependency), and `00-overview.md` (updated graph + sequence).

**Phase 5**: Cohesion check on full roadmap. Fix any format drift found in older sections. Run `/review-plan plans/roadmap/`.

---

## Template Reference

The command uses `plans/_template/plan.md` as the structure reference. See that file for:
- Complete index.md template
- Section file template
- Status conventions
- The roadmap (`plans/roadmap/`) as a working example
