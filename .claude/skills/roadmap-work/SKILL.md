---
name: roadmap-work
description: Execute a roadmap subsection on Opus. Invoked by /continue-roadmap (Sonnet) at its Step 6 handoff after the user confirms which subsection to work on. Reads code on Opus, writes code on Opus, invokes /fix-bug / /tpr-review / /impl-hygiene-review as nested skills. Not invoked directly by the user — always chained from /continue-roadmap.
argument-hint: "<plan-path>/<section-file> <subsection-id>"
model: opus
---

# Roadmap Work — Execute a Subsection on Opus

**Invoked by `/continue-roadmap` at its Step 6 handoff.** The parent Sonnet skill has already:

1. Re-read CLAUDE.md.
2. Scanned the roadmap, run all gates (schema, stale-frontmatter, unreviewed-plan, TPR triage, bug tracker, clean working tree).
3. Resolved blockers and impediments.
4. Presented the focus section summary to the user.
5. Got the user's pacing choice (full-section / subsection-by-subsection).
6. Identified the specific subsection to execute.

Your job is the code-execution body that used to live in `/continue-roadmap` Step 6 — but running on Opus so the code reads and code writes benefit from Opus's judgment on Ori compiler invariants (ARC soundness, phase purity, type-system rules, spec conformance).

## Rule of model usage

**Opus for:**
- Reading affected source files before editing
- Writing code (Rust compiler code, Ori stdlib, Ori tests, Rust tests)
- Triage decisions surfaced by `/tpr-review` (nested — inherits its own Opus triage phase)
- Root-cause analysis in `/fix-bug` (nested)
- `/impl-hygiene-review` findings interpretation (nested)

**Sonnet for (delegate via `Agent(model: "sonnet")` subagents):**
- Updating plan checkbox flips (`- [ ]` → `- [x]`) at subsection close-out
- Frontmatter metadata updates (`updated:`, `status:`, `reviewed:`)
- Progress summaries / retrospective report text
- Running mechanical scripts (`./test-all.sh`, `diagnostics/*.sh`, `roadmap-scan.sh`)

**Shell, nested skill invocations, and `/commit-push`** inherit their own model policies — no Opus vs Sonnet choice needed here.

## Protocol

### Step 0: Re-read CLAUDE.md (MANDATORY even though /continue-roadmap just did it)

Context compression between skill invocations can drop rules. Read CLAUDE.md in full before executing.

### Step 1: Load subsection detail

Read the target section file at `<plan-path>/<section-file>` in full. Identify the specific `- [ ]` items under `<subsection-id>` (e.g., `§04.2 Phase B`, subsection `1.1A`, etc.).

### Step 2: Intelligence recon (CONDITIONAL — per `.claude/rules/intelligence.md`)

Follow the canonical intel-summary injection protocol:

@.claude/skills/dual-tpr/compose-intel-summary.md

Per SSOT Step F / `/continue-roadmap` extension — use `file-symbols`, `callers`/`callees`, `similar` on section-body symbols to map blast radius before editing.

### Step 3: Read affected source code (Opus-mandatory)

Read the code paths the subsection will touch **before** modifying anything. This read feeds directly into your edit decisions — per the user's empirical experience, Opus produces materially better Ori compiler code, and pre-edit reads are inseparable from the edit quality. Do not delegate this read to a Sonnet subagent; same-model-read-and-write is the correctness invariant.

### Step 4: Execute the subsection's checkboxes

Follow the **Implementation Guidelines** in `.claude/skills/continue-roadmap/SKILL.md` — specifically the sections after Step 6:

- ZERO DEFERRAL — Implement, Don't Document For Later
- ALL Deferrals Must Have Implementation Anchors
- Plan Boundary Integrity
- Scope Rule: ALL Checkboxes in the Section Are In Scope
- Verification Rule: Empty Checkboxes Must Be Verified
- Matrix Testing Rule (delegate to `.claude/rules/tests.md`)
- TDD for Bugs (delegate to CLAUDE.md §TDD for Bugs)

These guidelines are authored in `/continue-roadmap` and read here by reference rather than duplicated — the content is long and drift between two copies is an SSOT violation.

### Step 5: Run tests

```
timeout 150 ./test-all.sh
```

### Step 6: Nested skill invocations (as needed)

- If a bug surfaces during Step 4, invoke `Skill: fix-bug BUG-XX-NNN` (inherits its own Opus judgment phases).
- After code changes that touch more than the subsection's narrow slice, invoke `Skill: tpr-review` (inherits its own dispatch/triage split — triage is Opus).
- At subsection close-out (per `/continue-roadmap` §Step 5.5), invoke `Skill: impl-hygiene-review` **after** TPR is clean.

### Step 7: Subsection close-out (per `/continue-roadmap` §Step 5.5 close-out sequence)

1. Verify all subsection tasks are `[x]` and behavior is verified.
2. Update subsection `status` in section frontmatter to `complete` — **delegate the frontmatter edit to an `Agent(model: "sonnet")` subagent** (mechanical-writing, Sonnet-safe).
3. Invoke `Skill: improve-tooling` retrospectively on THIS subsection.
4. Run `diagnostics/repo-hygiene.sh --check` (and `--clean` if needed).
5. Invoke `Skill: commit-push` for the subsection's implementation work.

### Step 8: Return control to `/continue-roadmap`

Your skill exits. The Sonnet parent resumes for the next subsection's pacing decision (full-section mode → next subsection; subsection-by-subsection mode → AskUserQuestion).

## What this skill does NOT do

- **Does not run the gates** (schema, stale-frontmatter, unreviewed-plan, TPR triage, bug tracker, clean working tree) — those are `/continue-roadmap` Steps 1–2 and already ran on Sonnet before this skill was invoked.
- **Does not decide which subsection to execute** — the parent skill picked it; this skill receives the ID as args.
- **Does not loop to the next subsection** — control returns to `/continue-roadmap` after close-out, which then decides whether to invoke `/roadmap-work` again (full-section mode) or prompt the user (subsection-by-subsection mode).

## Invocation contract

Called as:
```
Skill: roadmap-work <plan-path>/<section-file> <subsection-id>
```

- `<plan-path>/<section-file>`: e.g., `plans/roadmap/section-04-aims.md`
- `<subsection-id>`: e.g., `4.2`, `4.2B`, `Phase-A`

Optional third arg: freeform note from `/continue-roadmap` about what the user specifically asked for (e.g., "focus on LLVM Rust tests only", "resolve impediment first"). When present, honor the note before starting general subsection execution.

## Related

- `.claude/skills/continue-roadmap/SKILL.md` — parent skill, contains the gate logic and Implementation Guidelines this skill references.
- `.claude/skills/fix-bug/SKILL.md` — nested for bug fixes.
- `.claude/skills/tpr-review/SKILL.md` — nested for dual-source review.
- `.claude/skills/impl-hygiene-review/SKILL.md` — nested for hygiene sweep.
- `.claude/skills/commit-push/SKILL.md` — nested for close-out commits.
- Memory `project_skill_model_policy.md` — cross-skill Model Policy index.
