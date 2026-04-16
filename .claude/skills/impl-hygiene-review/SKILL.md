---
name: impl-hygiene-review
description: Deep, wide implementation hygiene review — multi-pass analysis across the full compiler with third-party cross-checking. Dispatches phases as sub-agents (Sonnet for orchestration/static analysis/formatting; Opus for multi-lens deep analysis and plan authorship).
allowed-tools: Read, Grep, Glob, Agent, Bash, Skill, AskUserQuestion
---

# Implementation Hygiene Review

Deep, wide-angle review of implementation hygiene against `.claude/rules/impl-hygiene.md`. Multi-pass, multi-lens analysis that traces data flow end-to-end, detects algorithmic duplication, and cross-checks findings via third-party review.

**Implementation hygiene is NOT architecture** (design decisions are made). It covers the full plumbing layer — phase boundaries, data flow, error propagation, abstraction discipline, algorithmic DRY, file organization, naming, comments, visibility, and lint discipline.

## How this skill runs

SKILL.md is a thin coordinator. The review runs as a pipeline of sub-agents, each dispatched via `Agent({})` with its own model. The coordinator parses the target mode, chains phases sequentially, and threads handoff JSON files between them.

**FOREGROUND MANDATORY — ALL Agent dispatches.** Every `Agent({})` call below MUST run in the foreground (do NOT set `run_in_background: true`). The pipeline is sequential: each phase's output informs the next dispatch. No independent work to parallelize.

- **Phases 0, 1, 2** (Sonnet) — gather static-analysis output, load rules/context, map the landscape. Produce the context packet.
- **Phase 3** (Opus) — the one judgment-heavy phase. Multi-lens deep analysis reads code, traces data flow, compares function bodies for algorithmic DRY, and produces findings.
- **Phase 4** (Sonnet) — dispatches `/tp-help` or `/tpr-review` to cross-check. Orchestration-only.
- **Phase 5** (Sonnet) — formats findings into the report template. Mechanical-writing.
- **Phase 6** (Opus, CONDITIONAL) — authors a plan to address the findings when scope exceeds inline fixes. Skipped when findings are few or small enough to fix directly.

The coordinator reads the tiny `/tmp/impl-hygiene-{run}/phase-N.json` summary from each sub-agent, not the full finding payloads. Main context stays under ~15K tokens per invocation.

## Target

`$ARGUMENTS` specifies the boundary or scope to review. **If empty or blank, default to Auto Mode** — the skill autoscopes to the current active work arc (uncommitted changes plus recent commits, expanded to full crates per the dependency map). This is the recommended default; explicit modes below are for special cases.

### Auto Mode (default — no arguments) ★ recommended

When called with no arguments, the review autoscopes to the current "session of work" by combining uncommitted changes and recent commits. This is the recommended default for **every** plan section, fix-bug section, and roadmap completion checklist — multi-commit work arcs (feature implementations, bug-fix sequences, plan section work) typically span several commits, and a single-commit view is too narrow to catch the cross-commit drift this review is designed to catch.

**Auto-scoping procedure** (coordinator runs inline — it's just git commands):
1. List uncommitted changes: `git diff --name-only HEAD` and `git diff --name-only --cached`
2. Determine the active commit range:
   - If on a non-default branch: `git log --name-only --pretty=format: $(git merge-base HEAD origin/master)..HEAD`
   - If on `master`/`main`: fall back to the last 5 commits
3. Union, deduplicate, expand to full crates per the dependency map below
4. Apply dependency expansion (downstream consumers also enter scope)

> **Why no `/impl-hygiene-review last commit`**: a single-commit view is intentionally not supported as a scope. Use Auto Mode (no arguments) for the normal case, or `last N commits` for explicit narrow scoping when you genuinely want N=2 or N=3.

### Path Mode (explicit crate/directory targets)

- `/impl-hygiene-review compiler/ori_lexer compiler/ori_parse` — review lexer→parser boundary
- `/impl-hygiene-review compiler/ori_types` — review internal phase boundaries within a crate
- `/impl-hygiene-review compiler/ori_arc` — review ARC pass composition

### Commit Mode

- `/impl-hygiene-review last 3 commits` — review files touched by the last N commits
- `/impl-hygiene-review <commit-hash>` — review files touched by a specific commit
- `/impl-hygiene-review <commit-A>..<commit-B>` — review the commit range

### Full Project Mode (landscape survey)

- `/impl-hygiene-review full` — review the entire compiler across all crates
- `/impl-hygiene-review full --focus=dry` — emphasis on algorithmic duplication
- `/impl-hygiene-review full --focus=leaks` — emphasis on side logic and SSOT

Full project mode is the widest sweep. Use this when you want the complete landscape picture.

**CRITICAL: Commits are scope selectors, NOT content filters.** The commit determines WHICH files and areas to review. Once the files are identified, review them completely — report ALL hygiene findings in those files. Do NOT deprioritize or exclude findings because they predate the commit.

### Dependency map for expansion

```
ori_lexer      → consumed by: ori_parse
ori_parse      → consumed by: ori_types
ori_ir         → consumed by: ori_types, ori_eval, ori_llvm, ori_arc
ori_diagnostic → consumed by: all compiler crates
ori_registry   → consumed by: ori_types, ori_eval, ori_llvm
ori_types      → consumed by: ori_eval, ori_llvm, ori_arc
ori_patterns   → consumed by: ori_eval
ori_eval       → consumed by: oric
ori_arc        → consumed by: ori_llvm
ori_llvm       → consumed by: oric
ori_rt         → consumed by: ori_llvm (FFI contract)
```

If any `.rs` file in a crate is touched, the whole crate is in scope; its downstream consumers also enter scope.

## Pipeline — Sequential Phase Dispatch

```
run_id = /tmp/impl-hygiene-{generated}
scope = <resolved scope paths from target-mode parsing>
mkdir -p {run_id}

# Write initial context for sub-agents to read
echo '{"scope": [...], "mode": "auto|path|commit|full", "run_id": "{run_id}"}' \
  > {run_id}/context.json

# ── PHASE 0 (Sonnet): Automated Static Analysis ─────────────
Agent({
  subagent_type: "general-purpose",
  model: "sonnet",
  description: "impl-hygiene phase 0 static analysis",
  prompt: `Read .claude/skills/impl-hygiene-review/phase-0-static-analysis.md
           and execute. run_id: {run_id}. Context at {run_id}/context.json.
           Write {run_id}/phase-0.json per the protocol.`
})

# ── PHASE 1 (Sonnet): Load Rules & Context ──────────────────
Agent({
  subagent_type: "general-purpose",
  model: "sonnet",
  description: "impl-hygiene phase 1 context load",
  prompt: `Read .claude/skills/impl-hygiene-review/phase-1-context.md
           and execute. run_id: {run_id}. Prior phase outputs at
           {run_id}/phase-0.json. Write {run_id}/phase-1.json.`
})

# ── PHASE 2 (Sonnet): Map the Landscape ─────────────────────
Agent({
  subagent_type: "general-purpose",
  model: "sonnet",
  description: "impl-hygiene phase 2 landscape",
  prompt: `Read .claude/skills/impl-hygiene-review/phase-2-landscape.md
           and execute. run_id: {run_id}. Write {run_id}/phase-2.json.`
})

# ── PHASE 3 (Opus): Deep Multi-Lens Analysis ────────────────
# THIS is the heavy, judgment-critical phase. Opus mandatory.
Agent({
  subagent_type: "general-purpose",
  model: "opus",
  description: "impl-hygiene phase 3 deep analysis",
  prompt: `Read .claude/skills/impl-hygiene-review/phase-3-analysis.md
           and execute. run_id: {run_id}. Consume {run_id}/phase-{0,1,2}.json.
           Write {run_id}/phase-3.json with the findings list.`
})

# ── PHASE 4 (Sonnet): Third-Party Cross-Check ───────────────
Agent({
  subagent_type: "general-purpose",
  model: "sonnet",
  description: "impl-hygiene phase 4 cross-check",
  prompt: `Read .claude/skills/impl-hygiene-review/phase-4-cross-check.md
           and execute. run_id: {run_id}. Write {run_id}/phase-4.json.`
})

# ── PHASE 5 (Sonnet): Compile & Present Findings ────────────
Agent({
  subagent_type: "general-purpose",
  model: "sonnet",
  description: "impl-hygiene phase 5 present",
  prompt: `Read .claude/skills/impl-hygiene-review/phase-5-present.md
           and execute. run_id: {run_id}. Write {run_id}/phase-5.json
           with the formatted report text and per-category counts.`
})

# Read phase-5.json's summary and emit the report to the user

# ── PHASE 6 (Opus, CONDITIONAL): Generate Plan ──────────────
# Only when findings count or scope exceeds inline-fix capacity.
# Coordinator decides based on phase-5.json's counts + severity distribution.
if findings_warrant_a_plan:
    Agent({
      subagent_type: "general-purpose",
      model: "opus",
      description: "impl-hygiene phase 6 plan authoring",
      prompt: `Read .claude/skills/impl-hygiene-review/phase-6-plan.md
               and execute. run_id: {run_id}. Consume {run_id}/phase-5.json.
               Write {run_id}/phase-6.json with plan directory + outline.`
    })
```

**Escalation:** if any phase returns `"escalate": true`, invoke `AskUserQuestion` with the provided `question` + `options` verbatim. Never dump escalations as prose.

## Important Rules

1. **No architecture changes** — Don't propose new phases, new IRs, or restructured crate graphs.
2. **Full scope** — Phase boundaries, data flow, naming, comments, visibility, file organization, lint discipline, unsafe hygiene, algorithmic DRY, and code fixes are all in scope. Only new phases, IRs, or crate graph restructures are out of scope (that's architecture).
3. **Trace, don't grep** — Follow actual data flow through the code, don't just search for patterns.
4. **Read both sides** — Always read both the producer and consumer of a boundary.
5. **Compare function bodies** — For algorithmic DRY, read pairs/groups of structurally similar functions side by side. Compare control-flow skeletons, not just names.
6. **Understand before flagging** — Some apparent violations are intentional (e.g., lexer tracking nesting depth for nested comments is acceptable phase-local state).
7. **Be specific** — Every finding must have `file:line`, the boundary it violates, and a concrete fix.
8. **Compare to reference compilers** — When in doubt, check `~/projects/reference_repos/lang_repos/`.
9. **Cross-check with /tp-help** — For full project mode: MANDATORY. For other modes: RECOMMENDED.
10. **Follow the algorithm, not the name** — Two functions named differently but with identical control-flow skeletons are duplicated.

## Files in this skill

- `SKILL.md` (this file) — coordinator + target modes + dependency map + important rules.
- `phase-0-static-analysis.md` — Sonnet: run hygiene-lint.py, enum-drift.py, fn-rename.py, plan-annotations.py.
- `phase-1-context.md` — Sonnet: load CLAUDE.md + `.claude/rules/*.md` + active plan.
- `phase-2-landscape.md` — Sonnet: intel queries, symbol + call-graph inventory.
- `phase-3-analysis.md` — **Opus**: multi-lens deep analysis (the heavy, judgment-critical phase). Includes finding targets.
- `phase-4-cross-check.md` — Sonnet: dispatches `/tp-help` or `/tpr-review`.
- `phase-5-present.md` — Sonnet: format findings into report template + cleanup notes.
- `phase-6-plan.md` — **Opus**, CONDITIONAL: author a plan from the findings when scope warrants it. Includes plan-section format.

Tooling lives alongside (unchanged): `hygiene-lint.py/.sh`, `enum-drift.py/.sh`, `fn-rename.py/.sh`, `hygiene-fix.py/.sh`, `plan-annotations.py/.sh`, `test_plan_annotations_fix.py`.
