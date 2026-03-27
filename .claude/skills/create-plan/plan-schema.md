# Plan Schema

The single source of truth for plan structure. All plans in `plans/` and `plans/completed/` must conform to this schema. Referenced by `/create-plan` (creation) and `/continue-roadmap` (validation).

---

## Directory Layout

```
plans/{plan-name}/
├── index.md           # Keyword clusters for quick finding
├── 00-overview.md     # Mission, architecture, dependencies, phasing, metrics
├── section-01-*.md    # First section
├── section-02-*.md    # Second section
└── ...
```

---

## Overview File Template (`00-overview.md`)

The overview is the master document. It answers: **what** is the goal, **why** does it matter, **how** do the pieces fit together, and **in what order** should they be built?

```markdown
---
plan: "{plan-name}"
title: "{Plan Title}: Exhaustive Implementation Plan"
status: not-started
supersedes:             # Plans this replaces (if any)
  - "plans/{old-plan}/"
references:             # Design docs, proposals, prior art
  - "plans/{related-doc}.md"
  - "docs/ori_lang/proposals/{proposal}.md"
---

# {Plan Title}: Exhaustive Implementation Plan

## Mission

{1-2 sentences. What is this plan accomplishing and why? Not "implement X" but "complete X as one cohesive system: from A through B to C." Establish scope and intent.}

## Architecture

\`\`\`
{ASCII diagram showing the pipeline/system being built or modified.
Show the flow of data through stages, the key types at each boundary,
and where this plan's sections fit in.}
\`\`\`

## Design Principles

{Name the core architectural principle(s) driving this plan's design.
Explain WHY these matter — cite concrete bugs or pain points that
motivated the principle. 2-3 principles max.}

\`\`\`
{Optional: show the information/data flow chain if applicable.
E.g., how each stage enriches IR for the next stage.}
\`\`\`

## Section Dependency Graph

\`\`\`
{ASCII graph showing section dependencies.
Use arrows to show what depends on what.
Note which sections are independent (parallelizable).}
\`\`\`

{Prose explanation:}
- Sections {X-Y} are independent and can be worked in any order.
- Section {Z} requires {X}. Section {W} requires all.

**Cross-section interactions (must be co-implemented):**
- **{Section A} + {Section B}**: {Why these must land together. Cite the
  specific bug or invariant that breaks if only one lands.}

## Implementation Sequence

{Resolve the dependency graph into a concrete build order. Each phase
gates the next; items within a phase can be parallelized.}

\`\`\`
Phase 0 - Prerequisites
  └─ {section}: {task description}

Phase 1 - Foundation
  └─ {section.subsection}: {task}
  └─ {section.subsection}: {task}

Phase 2 - Core implementation
  └─ {section.subsection}: {task}
  Gate: {testable condition proving this phase is complete}

Phase 3 - Integration  [CRITICAL PATH]
  └─ {section.subsection}: {task}
  Gate: {testable condition}

Phase N - Verification
  └─ {section}: {comprehensive testing}
\`\`\`

**Why this order:**
- Phase 0-1 are pure additions — no behavioral changes.
- Phase 2 must precede Phase 3 because {reason}.
- Phase 3 is the critical path because {reason}.

**Known failing tests (expected until plan completion):**

{List tests that are expected to fail and WHY. Prevents wasted effort
investigating "failures" that are symptoms of missing infrastructure.
Include root causes tied to specific phases.}

- **`test_name`** — {symptom}. Root cause: {Phase N} ({missing infrastructure}).

Do NOT attempt to fix these tests individually. They share infrastructure
dependencies that must be built bottom-up through Phases {X-Y}.

## Metrics (Current State)

{Baseline measurements before implementation begins. Establishes the
starting point so progress and regressions can be measured.}

| Crate | Production LOC | Test LOC | Total |
|-------|---------------|----------|-------|
| `{crate}` | ~{N} | ~{N} | ~{N} |
| **Total** | **~{N}** | **~{N}** | **~{N}** |

## Estimated Effort

| Section | Est. Lines | Complexity | Depends On |
|---------|-----------|------------|------------|
| {NN} {Title} | ~{N} | Low/Medium/High | — |
|   ↳ {NN.X} {Subsection} | ~{N} | Low | — |
| **Total new** | **~{N}** | | |
| **Total deleted** | **~{N}** | | |

## Known Bugs (Pre-existing)

{Bugs discovered during investigation that affect multiple sections.
Track root causes, fix locations, and status so they don't get lost.}

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| {Description} | {Root cause analysis} | Section {NN} | Not Started / Fixed / Guarded |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | {Title} | `section-01-{name}.md` | Not Started |
| 02 | {Title} | `section-02-{name}.md` | Not Started |
```

---

## Index File Template (`index.md`)

The index enables keyword-based discovery across all sections. If this plan is a
**reroute** (a parallel track alongside the main roadmap), add frontmatter to make
it discoverable by the website:

```yaml
---
reroute: true
name: "{Short Name}"
full_name: "{Full Plan Name}"
status: queued
order: N
---
```

- `reroute: true` — marks this plan as a reroute (omit for non-reroute plans)
- `name` — short display name for timeline pills (e.g., "LLVM Fixes")
- `full_name` — full display name for page titles (e.g., "LLVM Codegen Fixes")
- `status` — `active | queued | resolved`
- `order` — queue priority; lower value = promoted first when active reroute completes (default 999 if omitted)
- `key` and `dir` are derived at load time from the directory name

```markdown
# {Plan Name} Index

> **Maintenance Notice:** Update this index when adding/modifying sections.
> **Supersedes:** `plans/{old-plan}/` (if applicable)

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: {Title}
**File:** `section-01-{name}.md` | **Status:** Not Started

\`\`\`
keyword1, keyword2, keyword3
formal term, common alias, abbreviation
file_path.rs, function_name, TypeName
reference implementation term, prior art concept
\`\`\`

---

### Section 02: {Title}
**File:** `section-02-{name}.md` | **Status:** Not Started

\`\`\`
keywords here
\`\`\`

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | {Title} | `section-01-{name}.md` |
| 02 | {Title} | `section-02-{name}.md` |
```

---

## Section File Template

Each section file follows this structure. Sections range from focused (single subsection) to comprehensive (5+ subsections with deep analysis).

```markdown
---
section: "{NN}"
title: "{Title}"
status: not-started
reviewed: false
goal: "{One-line measurable goal}"
inspired_by:             # Reference implementations studied
  - "{Language/Tool} {pattern} ({file path})"
depends_on: ["{NN}"]     # Other sections required first
third_party_review:
  status: none           # none | findings | resolved
  updated: null          # YYYY-MM-DD when last touched
sections:
  - id: "{NN}.1"
    title: "{Subsection}"
    status: not-started
  - id: "{NN}.2"
    title: "{Subsection}"
    status: not-started
  - id: "{NN}.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "{NN}.N"
    title: "Completion Checklist"
    status: not-started
---

# Section {NN}: {Title}

**Status:** Not Started
**Goal:** {Expanded goal — what must be true when this section is complete.
Not "implement X" but "X works correctly under conditions A, B, C with
no regressions in Y."}

**Context:** {Why this section exists. What pain point, bug, or
architectural gap motivated it. Cite specific debugging sessions,
test failures, or design flaws. 2-4 sentences.}

**Reference implementations:**
- **{Language}** `{file path}`: {pattern name} — {what we learn from it}
- **{Language}** `{file path}`: {pattern name} — {what we learn from it}

**Depends on:** Section {NN} ({why}).

---

## {NN}.1 {Subsection Title}

**File(s):** `{file path(s) being modified}`

{Context paragraph: what this subsection does, what problem it solves,
and how it fits into the section's overall goal.}

- [ ] {Task description with enough detail to implement without ambiguity}
  \`\`\`rust
  // Code example showing the target design (types, signatures, key logic).
  // This is the SPEC — the implementation should match this.
  \`\`\`

- [ ] {Another task}
  - [ ] {Sub-task with specific file + function to modify}
  - [ ] {Sub-task}

- [ ] {Validation task — how to verify this subsection works}

---

## {NN}.2 {Subsection with Design Decisions}

**File(s):** `{file path(s)}`

**Context:** {The problem requiring a design decision.}

{Detailed analysis of the problem — what was tried, what failed, why.
Include debugging traces, root cause analysis, data from experiments.}

**Fix approach — {N} options:**

**(a) {Recommended approach}** (recommended — {why}):
{Detailed description with code examples.}

\`\`\`rust
// Target implementation
\`\`\`

**Why this is best:** {Justify against alternatives. Cite the
architectural principle it upholds.}

**Trade-off:** {What this approach costs or complicates.}

**(b) {Alternative approach}** ({characterization}):
{Description with code.}
**Downside:** {Why this is worse than (a).}

**(c) {Least recommended}** (not recommended):
{Brief description.}
**Downside:** {Why.}

**Recommended path:** Option (a) for {reason}, with option (b) as
acceptable interim if {condition}.

### {Sub-topic within the subsection}

**Discovery:** {What was learned during investigation that changes
the approach or adds requirements.}

**Implementation steps:**
1. {Specific, numbered, actionable step with file path}
2. {Step referencing specific functions to modify}
3. {Validation step — what test to run, what output to expect}

**Reference implementations:**
- **{Language}** `{file}`: {what it does} — {what we adopt from it}

**Co-implementation requirement with Section {NN} ({topic}):**
{Why this subsection and another section's work must land together.
What breaks if only one lands. Be specific about the failure mode.}

---

## {NN}.R Third Party Review Findings

<!-- Reserved for Codex or other external reviewers.
If unresolved findings exist here:
- section frontmatter `status` must be `in-progress`
- `third_party_review.status` must be `findings`

When all findings are triaged:
- accepted findings are integrated into the relevant implementation subsection(s)
- rejected findings are closed with rationale
- all items in this block are marked resolved
- `third_party_review.status` becomes `resolved` or `none`
-->

- None.

---

## {NN}.N Completion Checklist

- [ ] {Concrete, verifiable item — not "implement X" but "X passes test Y"}
- [ ] {Item with specific command to verify: `grep -r "pattern" path/` returns 0}
- [ ] {Behavioral verification: `test_name` passes without modification}
- [ ] {Regression check: `./test-all.sh` green}
- [ ] {No spurious warnings in normal compilation}
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan {NN}` returns 0 annotations — all temporary scaffolding (TPR, CROSS, BUG, §, Phase, section- refs) removed from `.rs` files

**Exit Criteria:** {Paragraph describing the measurable, testable condition
that proves this section is complete. Include specific commands, test names,
metric thresholds. Not "X works" but "X produces Y output when Z command
is run, with 0 regressions in test suite A (N tests) and test suite B (M tests)."}
```

---

## Verification Section Template

Every plan should include a verification section (typically the last section). This proves the system works as one cohesive whole.

```markdown
## {NN}.1 Test Matrix

Build a comprehensive test matrix covering every feature through the
pipeline being built/modified.

- [ ] **{Feature category}:** ({date started})
  - {Sub-feature} — {status: covered (file.rs) | FIXED (date) | gap: reason (#[ignore])}
  - {Sub-feature} — {status}

### {NN}.1.1 Discovered Gaps

| Gap | Roadmap Location | Test | Severity |
|-----|-----------------|------|----------|
| {Description} | {Section reference} | `test_name` | CRITICAL / Medium / Low |

---

## {NN}.2 Behavioral Equivalence (if applicable)

Verify that the new path produces identical results to the existing path.

- [ ] Build a test harness comparing outputs: {description}
- [ ] Apply to all relevant tests
- [ ] Track and investigate every mismatch
- [ ] Create a CI-runnable script

---

## {NN}.3 Code Journey (Pipeline Integration)

Run `/code-journey` to test the pipeline end-to-end with progressively
complex Ori programs. This catches issues that unit tests and spec tests
miss: silent wrong code generation, phase boundary mismatches, cascading
failures across compiler stages, and eval-vs-LLVM behavioral divergence.

- [ ] Run `/code-journey` — journeys escalate until the compiler breaks down
- [ ] All CRITICAL findings from journey results triaged (fixed or tracked)
- [ ] Eval and AOT paths produce identical results for all passing journeys
- [ ] Journey results archived in `plans/code-journeys/`

**Why this matters:** Unit tests verify individual phases in isolation.
Code journeys verify that phases compose correctly — data flows through
the full pipeline (lexer → parser → type checker → canonicalizer →
eval/LLVM) and produces correct results. They use differential testing
(eval path as oracle for LLVM path) and progressive complexity
escalation to map the exact boundary of what works.

**When to run:**
- After any change to phase boundaries (new IR nodes, new type variants)
- After changes to monomorphization, ARC pipeline, or codegen
- After adding new language features that affect multiple phases
- As final verification before marking a plan complete

---

## {NN}.4 Safety Verification (if applicable)

- [ ] **{Safety property}:** {How it's verified, what tool/technique}
- [ ] **Stress test:** {Scale — N allocations, N recursion depth, N elements}
- [ ] **{Tool} verification:** {Script path, what it catches}

---

## {NN}.5 Performance Validation

- [ ] **{Metric 1}:** Measured {what} ({conditions}):
  - {Workload A}: ~{value}
  - {Workload B}: ~{value}
  - Script: `{script path}`
  - Benchmark programs: `{path}`

- [ ] **{Metric 2}:** {comparison}:
  - {result with concrete numbers}

- [ ] **{Metric 3}:** {measurement}:
  - {result}

---

## {NN}.6 Documentation

- [ ] Update superseded plans to point to this plan
- [ ] Update CLAUDE.md if new commands/paths/patterns introduced
- [ ] Update relevant .claude/rules/*.md files
- [ ] Add architecture overview to key module docs

---

## {NN}.7 Completion Checklist

- [ ] Test matrix covers all features (every checkbox in {NN}.1)
- [ ] Behavioral equivalence verified ({script} passes — 0 mismatches)
- [ ] Code journey passes — eval/AOT match, no CRITICAL findings unaddressed
- [ ] Zero {safety violations} detected
- [ ] Stress tests pass ({N}/{M})
- [ ] Performance baselined
- [ ] All documentation updated
- [ ] Plan annotation cleanup: `plan-annotations.sh` returns 0 annotations for this plan's sections
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green

**Exit Criteria:** {Final measurable proof. Include test counts, metric
thresholds, and the specific commands that demonstrate completion.}
```

---

## Status Conventions

### Section and Subsection Status (section files, `00-overview.md`)

| YAML Status | Meaning | Notes |
|-------------|---------|-------|
| `not-started` | No work done | |
| `in-progress` | Partial completion | Include date + current state in header |
| `complete` | All done | Include completion date in header |

Sections AND subsections use the same values: `not-started`, `in-progress`, `complete`. Do NOT use `done` — always use `complete`.

### Plan-Level Status (`index.md` — website-facing)

| YAML Status | Meaning |
|-------------|---------|
| `active` | Currently being worked on |
| `queued` | Waiting in queue (lower `order` = promoted first) |
| `resolved` | Completed and archived |

Do NOT use `done` or `complete` in `index.md` — always use `resolved` for finished plans.

### Completed Plans

When all sections are `complete`, the plan is archived:
1. Set `index.md` status to `resolved`
2. Set `00-overview.md` status to `complete`
3. Move to `plans/completed/` via `git mv`

**Progress tracking conventions:**
- `[x]` — completed (include date: `(2026-02-24)`)
- `[ ]` — not started
- `**FIXED** (date)` — a bug discovered and fixed during implementation
- `#[ignore]` — test exists but is skipped due to known gap
- Commit references: `(committed c1c1b534)` for traceability
- Strikethrough `~~text~~` for gaps that were fixed (preserves history)

---

## Writing Principles

### Context Over Brevity
Each section should be self-contained enough that someone can understand
WHY the work exists, not just WHAT to do. Include the bug report, the
debugging session insight, the architectural principle that motivates it.

### Measurable Exit Criteria
"Implement X" is not an exit criterion. "{Command} produces {output}
with 0 failures across {N} tests" is. Every section ends with a
testable, verifiable condition.

### Design Decisions with Trade-offs
When there are multiple approaches, document all of them with pros/cons.
Mark the recommended approach and explain why. This prevents re-litigating
decisions and helps future readers understand the reasoning.

### Cross-References
Link sections that interact. When Section A depends on Section B,
explain the specific failure mode if only one lands. Use
"Co-implementation requirement" callouts for hard dependencies.

### Root Cause Analysis
When a bug or design flaw motivated a section, include the root cause
chain. "X broke because Y, which happened because Z, which is
fundamentally caused by W." This prevents surface-level fixes.

### Reference Implementations
Cite specific files from reference compilers/projects. Not "Rust does
this" but "Rust's `rustc_codegen_llvm/mir/operand.rs` uses the
`OperandValue` pattern where {description}." Include the path so the
reference can be consulted.

---

## Reference

See `plans/completed/codegen-purity/` for a canonical example:
- `00-overview.md` — Mission, architecture, dependency graph, phased sequence, metrics
- `index.md` — Keyword clusters for all 10 sections
- `section-01-block-merging.md` — Deep design decisions with options/trade-offs
- `section-10-verification.md` — Comprehensive test matrix and exit criteria
