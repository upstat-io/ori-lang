# /fix-next-bug — Queue Scan & Mode Selection (Sonnet Sub-agent)

**This file is read by the Sonnet sub-agent dispatched from `SKILL.md`.** Execute Steps 1-5 and return the structured handoff. Do NOT invoke `/fix-bug` or make code changes — the parent (Opus) manages the fix loop.

**You do NOT:**
- Edit `.rs`, `.ori`, or anything under `compiler/`, `library/`, `tests/`
- Invoke `/fix-bug` or fix any bug
- Run `git commit` directly

**You DO:**
- Read `plans/bug-tracker/section-*.md` files
- Run `scripts/intel-query.sh` for blast-radius preview
- Use `AskUserQuestion` for mode selection
- Return the structured handoff block

---

## Step 1: Scan All Open Bugs

Read all section files to collect every `- [ ]` entry:

```
plans/bug-tracker/section-01-parser-lexer.md
plans/bug-tracker/section-02-typeck.md
plans/bug-tracker/section-03-eval.md
plans/bug-tracker/section-04-codegen-llvm.md
plans/bug-tracker/section-05-runtime-arc.md
plans/bug-tracker/section-06-stdlib.md
plans/bug-tracker/section-07-tooling-cli.md
plans/bug-tracker/section-08-spec-docs.md
```

For each `- [ ]` entry, extract:
- **ID**: `BUG-{section}-{ordinal}`
- **Severity**: critical, high, medium, or low
- **Title**: the bold text after severity
- **Repro**: repro line if present
- **Subsystem**: subsystem line if present
- **Lifecycle markers**: check for `Escalated to plan:`, `Blocked:`, `**Blocked**:`, `**Blocked:**`, or `<!-- blocked-by:` in the entry body

**Exclude non-fixable entries** — remove from the candidate list any `- [ ]` entry whose body contains ANY of these lifecycle markers (check case-insensitively):
- `Escalated to plan:` or `Escalated:` — promoted to a plan; not an inline fix candidate
- `Blocked:` or `**Blocked**:` or `**Blocked:**` — has a prerequisite not yet met
- `<!-- blocked-by:` — cross-section dependency tracking

**Implementation note**: lifecycle markers can appear on the `- [ ]` checkbox line itself OR in the indented body lines below it. Scan the ENTIRE multi-line entry — checkbox line plus all indented continuation lines — before classifying it.

## Step 2: Sort by Priority

Sort all open (non-excluded) bugs using this priority ordering:
1. **Severity** — `critical` > `high` > `medium` > `low`
2. **Pipeline position** — lower section number first (01 Parser & Lexer → 02 Type Checker → 03 Evaluator → 04 Codegen & LLVM → 05 Runtime & ARC → 06 Stdlib → 07 Tooling & CLI → 08 Spec & Docs)
3. **Ordinal** — lower bug number first within the same section and severity

## Step 3: Check for Empty Queue

If there are no open (non-excluded) bugs:
- Set `queue_empty: true` in the handoff
- Return immediately — no mode selection needed

## Step 4: Present the Selected Bug + Blast-Radius Preview

The selected bug is the first entry after Step 2 sorting. Collect context for the queue display:

```
Selected: [BUG-{section}-{ordinal}][{severity}] {title}
  Repro: {repro}
  Subsystem: {subsystem}

Remaining queue ({N-1} bugs):
  1. [BUG-...][severity] title
  2. [BUG-...][severity] title
  ...
```

**Blast-radius preview**: Before the mode question, add blast-radius context. This helps the user gauge whether the bug is localized or cross-cutting.

Follow the canonical intel-summary injection protocol:

@.claude/skills/dual-tpr/compose-intel-summary.md

Per SSOT Step F — /fix-next-bug uses `callers "<repro symbol>" --repo ori` for a lightweight blast-radius preview on the selected bug's repro symbol. If the intelligence graph is unavailable, skip this and omit the blast-radius line.

Target the bug's repro symbol (from the Repro or Subsystem field). Append a one-line note:
```
  Blast radius: <symbol> called by N sites across M modules
```

This is a PREVIEW only — `/fix-bug` Phase 1 (investigation) runs its own full intelligence queries.

## Step 5: Choose Mode

Use `AskUserQuestion`:

- **Question**: `Ready to start with: [BUG-{section}-{ordinal}][{severity}] {title}\n\nHow would you like to proceed?`
- **Options**:
  - `One at a time` — Fix this bug, then ask before each next bug
  - `Fix all bugs non-stop` — Loop through ALL open bugs automatically with zero interaction. No questions, no pauses, no stops.

Record the choice for the handoff.

---

## Return the Handoff

Return this EXACT format:

```
## Handoff to parent (Opus) — fix-next-bug scan

**Queue empty**: {true | false}
**Selected bug**: {BUG-XX-NNN or "n/a"}
**Selected title**: {title or "n/a"}
**Selected severity**: {severity or "n/a"}
**Selected repro**: {repro or "n/a"}
**Selected subsystem**: {subsystem or "n/a"}
**Mode**: {interactive | autopilot | n/a (queue empty)}
**Blast radius**: {symbol called by N sites across M modules | unavailable}

### Queue display (verbatim, for parent to show user)
{The full formatted queue text from Step 4, with the Selected bug and Remaining queue sections}

### Full sorted queue
{List of BUG-IDs in priority order: "BUG-XX-NNN, BUG-XX-NNN, ..."}
```
