# Sync Docs Redesign Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

## Keyword Clusters by Section

### Section 01: Cursor file + `.claude/state/` bootstrap
**File:** `section-01-cursor-state.md` | **Status:** Not Started

```
cursor, state file, .claude/state/, sync-docs-cursor.json, atomic writes
schema_version, status, cycle_number, next_batch_id, last_completed_batch_id
last_attempt, retry_streak, last_commit_sha, last_branch, force_next_batch_id
idle, running, manual_attention, completed, bootstrap, corruption recovery
git rev-parse --git-common-dir, temp-rename, JSON schema, gitignore
first-run, missing-cursor, malformed-cursor, timestamped-backup
```

---

### Section 02: History file schema + append protocol
**File:** `section-02-history-file.md` | **Status:** Not Started

```
history file, sync-docs-history.jsonl, JSONL, append-only
batch_complete, cycle_complete, cap_hit, transport_fail event types
cycle, batch_id, branch, commit_sha, outcome, tpr_rounds, started_at, ended_at
failure_category, run_dir, morning-review artifact, audit trail
```

---

### Section 03: `/commit-push --no-push` flag
**File:** `section-03-commit-push-flag.md` | **Status:** Not Started

```
commit-push, --no-push, local-only commit, $ARGUMENTS, preview flag precedent
Step 7, git push, conditional branch, backward compatibility
.claude/commands/commit-push.md, TPR-XX-002
```

---

### Section 04: `/add-bug` `sync-docs` Source value
**File:** `section-04-add-bug-source.md` | **Status:** Not Started

```
add-bug, canonical Source vocabulary, provenance, sync-docs value
.claude/skills/add-bug/SKILL.md, lines 112-124, SSOT for bug provenance
TPR-XX-015, verification: /add-bug is autonomous, no AskUserQuestion
```

---

### Section 05: Batch catalog SSOT (format decision)
**File:** `section-05-batch-catalog.md` | **Status:** Not Started

```
batch catalog, SSOT, format decision, catalog_version
plan-section-per-shard, tracked markdown catalog, JSON catalog
.claude/commands/sync-docs-batches.json, plans/sync-docs-catalog/
stable batch IDs, versioning, migration on version mismatch
TPR-XX-011, TPR-XX-002-gemini
```

---

### Section 06: Batch 8 sub-batching (design docs, 79 files)
**File:** `section-06-batch-8-shards.md` | **Status:** Not Started

```
batch 8, design docs sub-batching, docs/compiler/design/
01-architecture, 02-intermediate-representation, 03-lexer, 04-parser
05-type-system, 06-pattern-system, 07-canonicalization, 08-evaluator
09-aims, 10-llvm-backend, 11-runtime, 12-formatter, 13-diagnostics
14-testing, 15-platform-targets, appendices
shard 8a through 8g, 10-21 files per shard
TPR-XX-006-codex, TPR-XX-007-gemini (batch 9 counterpart)
```

---

### Section 07: Batch 9 sub-batching (guide/tooling/modules, ~99 files)
**File:** `section-07-batch-9-shards.md` | **Status:** Not Started

```
batch 9, guide docs, tooling docs, development docs, module docs
docs/guide/, docs/tooling/formatter/, docs/tooling/lsp/
docs/development/, docs/ori_lang/v2026/modules/, docs/ori_lang/*.md
READMEs, shard 9a through 9e, 15-25 files per shard
TPR-XX-006-codex, TPR-XX-007-gemini
```

---

### Section 08: Scope gap closure (remaining .md + rules + commands + skills)
**File:** `section-08-scope-gap.md` | **Status:** Not Started

```
scope gap, 98 uncovered .md files, error code docs
compiler/ori_diagnostic/src/errors/E*.md (74 files)
.claude/rules/patterns.md, .claude/rules/eval.md (missed rules)
.claude/commands/*.md (13 files), .claude/skills/*/SKILL.md (21 files)
.claude/skills/ non-SKILL internals (transport.md, polling-protocol.md, etc.)
assignment-audit script, every-file-in-exactly-one-shard invariant
TPR-XX-005-codex
```

---

### Section 09: Autonomous TPR cap fallback
**File:** `section-09-tpr-cap-fallback.md` | **Status:** Not Started

```
autonomous fallback, AskUserQuestion interception, wrapper logic
/tpr-review 10-iteration cap, 3-wasted-rounds cap, transport infra failure
cap_hit history event, cursor.status = manual_attention, do not advance
non-interactive mode, no user to prompt, nightly autonomous contract
TPR-XX-007-codex, TPR-XX-010-codex, TPR-XX-005-gemini, TPR-XX-009-codex
```

---

### Section 10: Cycle-complete detection + reporting
**File:** `section-10-cycle-complete.md` | **Status:** Not Started

```
cycle_complete event, cycle_number increment, wrap-run detection
morning-review artifact, nightly-batch-done vs cycle-N-complete
completed_in_cycle, last_cycle_completed_at, report template
unmerged branch reconciliation, per-night branch list
TPR-XX-016-codex, TPR-XX-013-codex, TPR-XX-008-gemini
```

---

### Section 11: Full `sync-docs.md` rewrite
**File:** `section-11-command-rewrite.md` | **Status:** Not Started

```
command rewrite, sync-docs.md restructure, NEVER-STOP prose deletion
Phase 1 Batch Execution Protocol rewrite, Phase 3 Final Report rewrite
cursor-read flow, select batch from cursor, run only that batch
per-night branch naming: sync-docs-c{C}-b{B}-{date}-a{A}
CronCreate integration guidance, /loop vs cron, starting-from-HEAD rule
fact-bound documentation preserved, zero AskUserQuestion in nightly path
TPR-XX-019-codex, TPR-XX-020-codex, TPR-XX-012-codex, TPR-XX-014-codex
TPR-XX-009-gemini, TPR-XX-003-gemini
```

---

### Section 12: Migration + verification + cleanup
**File:** `section-12-migration.md` | **Status:** Not Started

```
first-cycle bootstrap walkthrough, end-to-end cycle validation
plan-annotations.sh, strip TPR-XX-NNN scaffolding, §XX.Y references
./test-all.sh green, plan-audit green, all 31 TPR findings resolved
migration path, deployment checklist, rollback plan
```

---

## Quick Reference

| ID | Title | File |
|---|---|---|
| 01 | Cursor file + `.claude/state/` bootstrap | `section-01-cursor-state.md` |
| 02 | History file schema + append protocol | `section-02-history-file.md` |
| 03 | `/commit-push --no-push` flag | `section-03-commit-push-flag.md` |
| 04 | `/add-bug` `sync-docs` Source value | `section-04-add-bug-source.md` |
| 05 | Batch catalog SSOT (format decision) | `section-05-batch-catalog.md` |
| 06 | Batch 8 sub-batching (design docs, 79 files) | `section-06-batch-8-shards.md` |
| 07 | Batch 9 sub-batching (guide/tooling/modules, ~99 files) | `section-07-batch-9-shards.md` |
| 08 | Scope gap closure (remaining .md + rules + commands + skills) | `section-08-scope-gap.md` |
| 09 | Autonomous TPR cap fallback | `section-09-tpr-cap-fallback.md` |
| 10 | Cycle-complete detection + reporting | `section-10-cycle-complete.md` |
| 11 | Full `sync-docs.md` rewrite | `section-11-command-rewrite.md` |
| 12 | Migration + verification + cleanup | `section-12-migration.md` |
