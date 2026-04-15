# /continue-roadmap — Sub-agent Workflow

**This file is read by the sub-agent dispatched from `SKILL.md`.** The protocol is deliberately thin: the scanner (`roadmap_scan.py --json`) pre-computes every gate decision, focus-context field, and next-unblocked pointer. The sub-agent's job is to run the scanner once, apply mechanical auto-fixes, and fill a handoff template from the JSON.

**You do NOT:**
- Read `CLAUDE.md` (not needed for JSON transcription)
- Read section files (their fields are in `focus_context` / `next_unblocked`)
- Run intelligence-graph queries (belongs to `/review-plan` / `/roadmap-work`, not scan+gates)
- Validate blockers by opening sibling plan files (scanner already classifies them)
- Edit `.rs`, `.ori`, or any file under `compiler/`, `library/`, `tests/`
- Run `git add` / `git commit` directly — commits go through `/commit-push`

**You DO:**
- Run the scanner once, parse JSON
- Apply mechanical auto-fixes to stale frontmatter (plan-doc-only writes)
- Build the handoff block from JSON fields
- Stop and escalate when any gate fires — the parent handles escalations

---

## Step 1 — Run the Scanner

```bash
python3 .claude/skills/continue-roadmap/roadmap_scan.py --json
```

If args were passed to `/continue-roadmap` (e.g., `section-4`, `4`, keyword), append the plan dir and/or section number as positional args. Without args, the scanner auto-selects via reroute priority then first-incomplete.

Parse stdout as JSON. All subsequent steps read this object; do not open plan files by hand.

`render_json` (in `roadmap_scan.py`) emits exactly three top-level fields — nothing else exists in the JSON envelope:

- `focus_context` — plan full name (`plan_full_name`), description (`plan_description`), section goal (`section_goal`), subsection list (`subsections`), progress text (`plan_progress_pct` / `plan_progress_text` / `section_progress_text`). Feeds the handoff's focus-context block verbatim.
- `next_unblocked` — `{subsection_id, item_content, item_lineno, unblocked_count, blocked_count}` for the first actionable `- [ ]` item. May be `null` if the focus section has no unblocked items.
- `gates` — 7 pre-computed gate entries, each `{fires: bool, severity, payload}`. Unfired gates have an empty `payload: {}`; firing gates carry `options` + `question` where user interaction is needed.

If you need cross-plan diagnostics (mismatches, orphan blockers, health signals), invoke `roadmap_scan.py` WITHOUT `--json` — the rich-text mode includes them. The `--json` envelope is intentionally minimal; do not expect `focus.plan` / `focus.section` / `health.*` top-level keys to exist.

If `scanner exit code != 0` or JSON parse fails: return `<escalate-to-parent>` with the raw stderr. Do not try to recover.

## Step 2 — Auto-fix Cleanup (mechanical, silent)

**Cleanup is never user-facing.** Any `auto-fix` gate that fires runs inline without asking, without surfacing options, without emitting an escalation. The user has pre-approved all cleanup — scanner-detected cruft is mechanical by definition.

### 2a. Stale frontmatter

If `gates.stale_frontmatter.fires` is true, iterate `payload.focus_plan_mismatches` and apply these rules via the Edit tool on the relevant plan/section/overview files:

| Scanner issue | Fix |
|---|---|
| `frontmatter=complete but N unchecked` | set status `in-progress` (or `not-started` if 0 checked) |
| `frontmatter=not-started but N checked` | set status `in-progress` (or `complete` if 0 unchecked) |
| `frontmatter=in-progress but all items checked` | set status `complete` |
| `frontmatter=in-progress but 0 items checked` | set status `not-started` |
| TPR status mismatch | set `third_party_review.status` to match (`findings` if unchecked TPR items exist, else `resolved`) |

**Do NOT fix** any mismatch outside the focus plan. Report those in the handoff's Gate results as informational only.

### 2b. Stale plan annotations

If `gates.stale_plan_annotations.fires` is true (count > 0), run the mechanical cleanup tool scoped to the focus plan:

```bash
bash .claude/skills/impl-hygiene-review/plan-annotations.sh --cleanup-only --plan <focus_context.plan_name>
```

This is a shell invocation of an idempotent tool — the "never edit .rs" rule applies to manual edits, not to calling a dedicated cleanup script. The tool strips stale plan-specific annotations (TPR, CROSS, BUG, §, Phase, section-NN refs) from `.rs` files in the focus plan's scope. Spec references (`Spec: Clause N.M`) are permanent and are NOT removed.

After the tool returns, verify the new count via `--count`. Include the "before → after" numbers in the handoff.

### 2c. Commit all cleanup together

If ANY fix was applied in 2a or 2b, commit everything via `Skill: commit-push` with a message like `chore(plans): auto-fix stale frontmatter + plan annotations`. Do not create separate commits per fix type — one cleanup commit covers the pass.

## Step 3 — Evaluate Remaining Gates

After Step 2, only `block`-severity and `info`-severity gates remain. For each entry with `fires: true`, act per the table below. If ANY gate with `severity: "block"` fires, stop and return `<escalate-to-parent>` — do NOT proceed to the normal handoff. The parent will invoke `AskUserQuestion` with the `payload.options` array.

| Gate key | Severity when fires | Action |
|---|---|---|
| `stale_frontmatter` | `auto-fix` | Handled silently in Step 2a — never escalates. |
| `stale_plan_annotations` | `auto-fix` | Handled silently in Step 2b — never escalates. |
| `unreviewed_plan` | `block` | Escalate. `payload.options` offers `/review-plan`, proceed-anyway, pick-different. |
| `tpr_findings` | `block` | Escalate. Parent invokes `/verify-tpr` with `payload.next_skill_arg`. |
| `critical_bugs` | `block` | Escalate. Parent invokes `/fix-bug` with the bug IDs from `payload.bugs`. |
| `high_bugs` | `info` | Include bug IDs in handoff summary. Not blocking. |
| `dirty_tree` | `block` | Escalate. `payload.options` offers `/commit-push` or proceed-dirty. **NEVER** run destructive git commands to clean up. |

When multiple block-severity gates fire together (e.g., unreviewed + dirty tree), escalate with ALL of them listed — the parent will ask the user each one via sequential `AskUserQuestion` calls.

## Step 4 — Pacing Question (only when no block-gates fire)

If Step 3 resolved cleanly, ask the user via `AskUserQuestion`:

- **Question**: "How should I pace Section {focus_context.section_number}?"
- **Options**:
  - `subsection-by-subsection` (recommended) — pause after each subsection completes
  - `full-section` — run all subsections continuously without pausing

Record the choice for the handoff. The subsection close-out retrospective (`/improve-tooling`) is mandatory regardless of pacing — pacing only controls pause-for-review, not gate skipping.

## Step 5 — Emit Handoff

Return the handoff block in this EXACT format (filling from JSON fields):

```
## Handoff to parent

**Focus plan**: {focus_context.plan_dir}
**Focus section file**: {focus_context.section_file}
**Focus subsection id**: {next_unblocked.subsection_id}
**Pacing choice**: {from Step 4}

### Focus context
## Focus: {focus_context.plan_full_name} — Section {focus_context.section_number}: {focus_context.section_title}

**Plan**: {focus_context.plan_description}
**Section goal**: {focus_context.section_goal}
**Plan progress**: {focus_context.plan_progress_pct}% ({focus_context.plan_progress_text})
**Section progress**: {focus_context.section_progress_text}

Subsections:
  {each subsection: "  {id}  {title}  [{status}]"}

### Gate results
- Stale frontmatter: {fixed count or "none"}
- Stale plan annotations: {count or "none"}
- Unreviewed plan: {pass | block}
- TPR findings: {none | N open}
- Critical bugs: {none | N}
- High bugs: {none | N (list IDs)}
- Dirty tree: {clean | N files}

### Next unblocked item
Subsection {next_unblocked.subsection_id}: {next_unblocked.item_content}
({next_unblocked.unblocked_count} unblocked, {next_unblocked.blocked_count} blocked remaining)

### Next command for the parent
Skill: roadmap-work {focus_context.section_file} {next_unblocked.subsection_id}
```

**If escalating**, keep ALL prior blocks (`## Handoff to parent` header, `### Focus context`, `### Gate results`, `### Next unblocked item`) EXACTLY as above — the Focus Context block is MANDATORY on every return, escalation included. Only the final `### Next command for the parent` block is replaced with:

```
### <escalate-to-parent>
**Gates fired**:
{list each firing block-severity gate with its payload}

**User questions for AskUserQuestion**:
{for each block-gate, emit a question + the payload.options array verbatim so the parent can call AskUserQuestion without re-deriving options}

**Relevant paths**:
{collected from gate payloads: section_path, files list, bug source sections, etc.}
```

Rationale: the parent's Step A prints the Focus Context block to the user before any `AskUserQuestion` prompt so the user knows which plan/section the gate decisions apply to. Omitting Focus Context on escalation forces the parent to re-query the sub-agent (wasted tokens) or fire gates blind (wrong answer likely).

---

## Return Format Example (escalation path)

When `unreviewed_plan` and `dirty_tree` both fire, the sub-agent returns the FULL handoff — Focus Context and all — with the final block swapped for `<escalate-to-parent>`:

```
## Handoff to parent

**Focus plan**: plans/empty-container-typeck-phase-contract
**Focus section file**: plans/empty-container-typeck-phase-contract/section-03-bodies-pass-integration.md
**Focus subsection id**: 03.1
**Pacing choice**: (not asked — blocked on gates)

### Focus context
## Focus: Empty Container Typeck Phase Contract — Section 03: Bodies Pass Integration

**Plan**: {plan_description from scanner}
**Section goal**: {section_goal from scanner}
**Plan progress**: 40% (2/5 sections complete)
**Section progress**: 0/4 subsections complete

Subsections:
  03.1  Wire bodies pass into typeck driver   [not-started]
  03.2  Propagate empty-container constraints [not-started]
  03.3  Integration tests                     [not-started]
  03.4  Plan-annotation cleanup               [not-started]

### Gate results
- Stale frontmatter: none
- Stale plan annotations: none
- Unreviewed plan: block
- TPR findings: none
- Critical bugs: none
- High bugs: none
- Dirty tree: 3 files

### Next unblocked item
Subsection 03.1: Wire bodies pass into typeck driver
(4 unblocked, 0 blocked remaining)

### <escalate-to-parent>
**Gates fired**: unreviewed_plan (block), dirty_tree (block)

**User questions for AskUserQuestion**:

Q1 (unreviewed_plan):
  question: "Section 03 has `reviewed: false`. Its assumptions have not been validated against the current codebase. How do you want to proceed?"
  options:
    - "Run /review-plan now (recommended)"  → next_skill: review-plan, arg: {section_path}
    - "Proceed anyway"  → next_skill: null
    - "Pick a different section"  → next_skill: null

Q2 (dirty_tree):
  question: "Working tree has 3 pending files from other sessions. How do you want to proceed?"
  options:
    - "Run /commit-push (recommended)"  → next_skill: commit-push
    - "Proceed with dirty tree"  → next_skill: null

**Relevant paths**: {section_path}, {each file in dirty_tree.files}
```

The parent surfaces the Focus Context block verbatim (per `SKILL.md` Step A), emits a one-line gate summary, then invokes `AskUserQuestion` with Q1, waits for the answer, dispatches the chosen skill if any, then does the same for Q2. After both resolve, the parent re-dispatches `/continue-roadmap` for a fresh scan — do NOT reuse this scan's output across the escalation boundary.
