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

## Mission Success Criteria

{The mission is complete when ALL of these are true. Each criterion must be concrete, testable, and verifiable — not "X works" but "X produces Y when Z is run." Section success criteria are the building blocks — when every section meets its own criteria, the mission criteria must follow. Every mission criterion must be traceable to at least one section that delivers it.}

- [ ] {Criterion 1 — specific, measurable, verifiable condition}
- [ ] {Criterion 2 — with command or test that proves it}
- [ ] {Criterion 3 — connects to section(s) that deliver it}
- [ ] `./test-all.sh` green — no regressions
- [ ] All section success criteria met

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
success_criteria:        # Concrete conditions proving this section is done
  - "{Criterion 1 — testable, verifiable}"
  - "{Criterion 2 — with command or observable result}"
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
# ── TPR Checkpoint Placement ──
# For sections with 3+ implementation subsections, add intermediate
# `/tpr-review` checkpoints after every 2-3 completed subsections.
# Mark these as `- [ ] **TPR checkpoint** — ...` items at the END of
# the last subsection in each group. The final `/tpr-review` in the
# Completion Checklist ({NN}.N) still runs — it catches integration
# issues across the full section.
#
# Example for a 6-subsection section:
#   {NN}.1  Implementation A
#   {NN}.2  Implementation B
#   {NN}.3  Implementation C  ← TPR checkpoint here (covers .1-.3)
#   {NN}.4  Implementation D
#   {NN}.5  Implementation E  ← TPR checkpoint here (covers .4-.5)
#   {NN}.R  Third Party Review Findings
#   {NN}.N  Completion Checklist  ← final TPR (full section)
---

# Section {NN}: {Title}

**Status:** Not Started
**Goal:** {Expanded goal — what must be true when this section is complete.
Not "implement X" but "X works correctly under conditions A, B, C with
no regressions in Y."}

**Success Criteria:**
{Concrete, testable conditions that prove this section's work is done.
Each criterion should be independently verifiable. Together, these criteria
contribute to one or more mission success criteria in `00-overview.md`.}

- [ ] {Criterion — specific behavioral outcome with verification method}
- [ ] {Criterion — test name, command, or observable result}
- [ ] {Criterion — connects upward to mission criterion: "Satisfies mission criterion N"}

**Context:** {Why this section exists. What pain point, bug, or
architectural gap motivated it. Cite specific debugging sessions,
test failures, or design flaws. 2-4 sentences.}

**Reference implementations:**
- **{Language}** `{file path}`: {pattern name} — {what we learn from it}
- **{Language}** `{file path}`: {pattern name} — {what we learn from it}

**Depends on:** Section {NN} ({why}).

---

<!-- ── MANDATORY SUBSECTION STRUCTURE ──
EVERY subsection ({NN}.1, {NN}.2, ...) MUST end with a **Subsection close-out**
block containing the per-subsection `/improve-tooling` retrospective. This is
non-negotiable: pain memory decays within hours, so the look-back must fire
while the subsection's debugging journey is still hot — NOT at section close.

The close-out block goes AFTER the subsection's implementation/validation
tasks and BEFORE the `---` separator. See {NN}.1 below for the canonical form;
every subsequent subsection in this section MUST repeat the same close-out
shape (only the subsection ID changes). Plans that omit the per-subsection
close-out will fail `/continue-roadmap` validation.
-->

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

- [ ] **Subsection close-out ({NN}.1)** — MANDATORY before starting {NN}.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the debugging journey for {NN}.1 specifically: which `diagnostics/`
        scripts you ran, where you added `dbg!`/`tracing` calls (and what each
        was looking for), where output was hard to interpret, where test
        failures gave unhelpful messages, where you ran the same command
        sequence repeatedly. Forward-look: what tool/log/diagnostic would
        shorten the next regression in {NN}.1's code path by 10 minutes?
        Implement every accepted improvement NOW (zero deferral) and commit
        each via SEPARATE `/commit-push` (e.g., `build(diagnostics): add X to
        Y.sh — surfaced by {plan}/section-{NN}.1 retrospective`). Use a valid
        conventional-commit type — `build` for dev/diagnostic scripts, `test`
        for test-harness changes, `chore` for general tooling, `ci` for CI
        config, `docs` for tool docs. Do NOT use `tools(...)` — the lefthook
        commit-msg hook rejects any type outside the standard set. Mandatory
        even when nothing felt painful — that is exactly when blind spots
        accumulate. If genuinely no gaps, document briefly: "Retrospective
        {NN}.1: no tooling gaps — relied on existing scripts X, Y." Do not
        silently skip. See `.claude/skills/improve-tooling/SKILL.md`
        "Per-Subsection Workflow" for the full protocol.

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

- [ ] **TPR checkpoint** — `/tpr-review` covering {NN}.1–{NN}.2 implementation work
  <!-- For sections with 3+ implementation subsections, place intermediate
       TPR checkpoints after every 2-3 completed subsections. This catches
       design drift, missed edge cases, and hygiene issues BEFORE they
       compound across the remaining subsections. The checkpoint item lives
       at the end of the last subsection in the group. -->

- [ ] **Subsection close-out ({NN}.2)** — MANDATORY before starting {NN}.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — same
        protocol as {NN}.1's close-out, scoped to {NN}.2's debugging journey.
        Commit improvements separately using a valid conventional-commit type:
        `build(diagnostics): ... — surfaced by {plan}/section-{NN}.2
        retrospective` (or `test(...)`, `chore(...)`, etc — see {NN}.1's
        close-out for the type rules).

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
- [ ] All intermediate TPR checkpoint findings resolved (see checkpoint items in subsections above)
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] This section's frontmatter `status` → `complete`, subsection statuses updated
  - [ ] `00-overview.md` Quick Reference table status updated for this section
  - [ ] `00-overview.md` mission success criteria checkboxes updated (check off any now satisfied)
  - [ ] `index.md` section status updated
  - [ ] Cross-links to other plans updated if this section resolved external blockers (`<!-- resolved-by: ... -->`)
  - [ ] Next section's `depends_on` verified — no stale assumptions from this section's work
- [ ] `/tpr-review` passed (final, full-section) — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review found no critical or major findings (or all findings triaged and fixed). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` **section-close sweep** — MANDATORY safety net after both reviews are clean. The PRIMARY tooling capture happens per-subsection (see each subsection's close-out block above) — by section close those captures should already be committed. The sweep does TWO things: (1) **Verify** every subsection in this section has either an "improvements made" entry (with commits) or a documented "no gaps" negative finding from its own per-subsection retrospective; if any subsection skipped its retrospective, STOP and run it now — the sweep cannot substitute for missed per-subsection captures. (2) **Look for cross-subsection patterns** invisible at per-item scope: command sequences repeated when transitioning between *different* subsections, integration test failures with worse messages than within-subsection failures, mental cross-referencing across files no tool combined, instrumentation that only became obvious after seeing all subsections together. Add ONLY new items that emerged from these cross-cutting patterns — do not duplicate per-subsection findings. Implement immediately (zero deferral), commit separately using a valid conventional-commit type (`build(diagnostics): add X — surfaced by section-{NN} close sweep` — use `build` for dev/diagnostic scripts, `test` for test-harness, `chore` for general tooling, `ci` for CI, `docs` for tool docs; the lefthook commit-msg hook rejects any non-standard type), verify against the original scenario. Most sweeps produce zero new findings when per-subsection captures are thorough — that is the expected, healthy outcome and must be documented: "Section-close sweep: per-subsection retrospectives covered everything; no cross-subsection patterns required new tooling." Do not silently skip.

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
- [ ] `/tpr-review` passed — independent Codex review clean
- [ ] `/impl-hygiene-review` passed — hygiene review clean. MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` **section-close sweep** — MANDATORY after both reviews are clean. Per-subsection captures from {NN}.1–{NN}.6 should already be committed via each subsection's own close-out block; the sweep verifies they ran (no skips) and adds only NEW cross-cutting items invisible at per-item scope. Verification sections especially benefit from cross-cutting capture because they exercise the full diagnostic surface — but the *primary* tooling growth still happens per-subsection. Look for: diagnostic scripts that were run during multiple subsections with the same output-interpretation friction, manual cross-referencing across dumps that no tool combined, stress-test or perf instrumentation that became obvious only after seeing the full verification picture. Implement immediately, commit separately using a valid conventional-commit type (`build(diagnostics): add X — surfaced by section-{NN} verification close sweep` — see the regular section-close sweep above for the type rules; the lefthook commit-msg hook rejects non-standard types like `tools(...)`), verify against the original scenario. Document the negative finding if there are no cross-cutting gaps. Do not silently skip.

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

### Rules Woven In, Not Assumed
Plans cannot assume the implementer has CLAUDE.md or `.claude/rules/*.md`
loaded in context. Every section must embed the specific rules that
govern its work — woven organically into checklist items, constraints,
and callouts. If a rule applies, it appears in the task description
itself: "Add `FooVariant` — update ALL match arms (`file.rs:123`,
`other.rs:456`)" rather than "Add variant (check sync points)." The
plan is a self-contained execution document. Relevant rule files:
`tests.md` (TDD, matrix testing), `compiler.md` (file size, crate
ordering, API design), `registry.md` (sync points), `arc.md` (AIMS
invariants), `impl-hygiene.md` (bloat/waste/drift categories),
`runtime.md`, `llvm.md`, `eval.md`, `parse.md`, `ir.md`, etc.

### Measurable Exit Criteria
"Implement X" is not an exit criterion. "{Command} produces {output}
with 0 failures across {N} tests" is. Every section ends with a
testable, verifiable condition.

### Success Criteria Hierarchy
The plan has mission-level success criteria in `00-overview.md`. Each
section has its own success criteria. Section criteria are the building
blocks — when every section meets its criteria, the mission criteria
must follow. Every mission criterion must trace to at least one section
that delivers it, and every section criterion must trace upward to at
least one mission criterion it contributes to. A section without
success criteria is not executable. A mission criterion that no section
delivers is a gap in the plan.

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

### TPR Cadence — Review Early, Review Often
Don't save `/tpr-review` for the very end of a section. For sections
with 3+ implementation subsections, place **intermediate TPR checkpoints**
after every 2-3 completed subsections of finished work. This catches
design drift, hygiene violations, and missed edge cases *before* they
compound across remaining subsections — fixing an issue in subsection 2
is cheap; discovering it after subsection 6 means rework across 4
subsections. The final TPR in the Completion Checklist still runs as a
full-section integration review. Larger, more complex subsections
(high estimated lines, cross-crate changes, new data flow paths) should
trigger a checkpoint sooner rather than later.

### Reference Implementations
Cite specific files from reference compilers/projects. Not "Rust does
this" but "Rust's `rustc_codegen_llvm/mir/operand.rs` uses the
`OperandValue` pattern where {description}." Include the path so the
reference can be consulted.

---

## Bug Fix Section Template

Bug fixes use a lighter-weight section file that lives in the bug tracker (`plans/bug-tracker/fix-BUG-XX-NNN.md`). Created by the `/fix-bug` command, these follow the same rigor as plan sections but are scoped to a single bug or cluster of related bugs.

```markdown
---
bug: "BUG-{section}-{ordinal}"
title: "{Bug title}"
severity: "{critical|high|medium|low}"
status: not-started
goal: "{One-line measurable goal}"
success_criteria:
  - "{Criterion 1 — specific, testable}"
  - "{Criterion 2 — with verification command}"
subsystem: "{crate/file path}"
found: "{YYYY-MM-DD}"
source: "{tpr-review|code-journey|manual|continue-roadmap|review-work}"
third_party_review:
  status: none
  updated: null
---

# Fix: BUG-{section}-{ordinal} — {Title}

**Status:** Not Started
**Severity:** {severity}
**Goal:** {Expanded goal — not 'fix X' but 'X correctly handles Y under conditions Z'}

**Success Criteria:**
- [ ] {Criterion — specific behavioral outcome with verification}
- [ ] {Criterion — test name or command}

**Context:** {Why this bug exists. How it was discovered. 2-4 sentences.}

---

## 1. Root Cause Analysis

- **Symptom**: {What was observed}
- **Proximate cause**: {What code produced the wrong behavior}
- **Root cause**: {Why — the architectural/logical flaw}
- **Blast radius**: {What else is affected}
- **Affected files**:
  - `{file}` — {what changes and why}

**Reference implementations** (if applicable):
- **{Language}** `{file}`: {How they handle this case}

---

## 2. TDD — Test Matrix

### Exact failing case
- [ ] {From the repro}

### Edge cases
- [ ] {Boundary conditions}

### Cross-type coverage (if type-dependent)
- [ ] {Test each relevant type}

### Cross-pattern coverage (if pattern-dependent)
- [ ] {Test each relevant pattern}

### Cross-feature interactions
- [ ] {Test interaction with other features}

### Semantic pin
- [ ] {Test that ONLY passes with correct semantics}

### Negative pin
- [ ] {Test that REJECTS old/broken behavior}

### Verify tests fail before fix
- [ ] All new tests fail against current code

---

## 3. Implementation

- [ ] {Fix approach with code examples}

---

## 4. Completion Checklist

- [ ] All new tests pass unchanged after fix
- [ ] Matrix completeness verified
- [ ] Debug AND release builds pass
- [ ] Interpreter and LLVM produce identical results (dual-execution parity)
- [ ] `ORI_CHECK_LEAKS=1` zero leaks (for memory-touching fixes)
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `cargo test -p {affected_crate}` green
- [ ] `/commit-push` — commit all changes before review
- [ ] Bug entry updated: `- [x]` with resolution
- [ ] Fix section status → `complete`
- [ ] Bug-tracker overview open bug count updated
- [ ] `/tpr-review` passed (critical/high: MANDATORY; medium: expected; low: recommended but not required)
- [ ] `/impl-hygiene-review` passed — AFTER TPR (critical/high: MANDATORY; medium: recommended; low: optional)
- [ ] `/improve-tooling` retrospective completed — MANDATORY at fix close, after both reviews are clean. Reflect on the bug-finding journey: which `diagnostics/` scripts you ran, where you added ad-hoc `dbg!`/`tracing` calls during root cause analysis, where the test failure messages were unhelpful, where the matrix tests were tedious to write because helpers were missing. Bug fixes are the richest source of tooling gaps because you've just spent time fighting the diagnostic surface — capture every gap you noticed. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. Especially valuable: instrumentation/logging that would have made the root cause obvious in 1 minute instead of 30, and matrix-test helpers that future fix sections will reuse.

**Exit Criteria:** {Measurable proof of completion with test names and commands.}
```

### Key Differences from Plan Sections

| Aspect | Plan Section | Bug Fix Section |
|--------|-------------|-----------------|
| Location | `plans/{plan}/section-NN-*.md` | `plans/bug-tracker/fix-BUG-XX-NNN.md` |
| Scope | Feature/subsystem | Single bug or cluster |
| Subsections | Multiple ({NN}.1, {NN}.2, ...) | Four fixed sections (RCA, TDD, Impl, Checklist) |
| Research | Multi-pass (4 passes, agents) | Focused investigation |
| TPR checkpoints | After every 2-3 subsections | Final only (unless complex) |
| {NN}.R section | Reserved for TPR findings | TPR findings go in completion checklist |
| Overview sync | Mission criteria, dependency graph | Bug count in overview, entry marked resolved |

### When to Escalate to a Full Plan

If during `/fix-bug` investigation you discover the bug requires:
- Changes to 5+ files across 3+ crates
- New data types, pipeline stages, or architectural changes
- Work that naturally decomposes into 3+ distinct subsections
- Changes that affect multiple other plans

...then escalate to `/create-plan` instead. The fix section becomes the research input for the plan.

---

## Reference

See `plans/completed/codegen-purity/` for a canonical example:
- `00-overview.md` — Mission, architecture, dependency graph, phased sequence, metrics
- `index.md` — Keyword clusters for all 10 sections
- `section-01-block-merging.md` — Deep design decisions with options/trade-offs
- `section-10-verification.md` — Comprehensive test matrix and exit criteria
