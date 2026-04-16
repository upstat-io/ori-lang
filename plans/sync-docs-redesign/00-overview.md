---
plan: "sync-docs-redesign"
title: "Sync Docs Redesign: Cursor-Driven Nightly Batched Execution — Exhaustive Implementation Plan"
status: not-started
supersedes:
  - null
references:
  - ".claude/commands/sync-docs.md"
  - ".claude/skills/tpr-review/SKILL.md"
  - ".claude/skills/dual-tpr/transport.md"
  - ".claude/skills/sync-claude/SKILL.md"
  - ".claude/skills/continue-roadmap/SKILL.md"
  - ".claude/skills/fix-next-bug/SKILL.md"
---

# Sync Docs Redesign: Cursor-Driven Nightly Batched Execution — Exhaustive Implementation Plan

## Mission

Redesign `.claude/commands/sync-docs.md` from a single-session "run all 9 batches overnight" command into a **nightly-autonomous one-batch-per-invocation** model with a persistent cursor that advances through batches in strict round-robin rotation, produces durable morning-review artifacts, and never blocks on user input. The redesign resolves all 31 findings surfaced by the 2026-04-15 dual-source TPR review (22 codex + 10 gemini), closes the scope gap where ~98 `.md` files are declared in-scope but uncovered by any batch, sub-batches the two oversized batches (8 with 79 files and 9 with ~99 files) into context-fittable shards, and replaces the `AskUserQuestion`-dependent escalation paths with history-logged fallbacks so the command can truly run unattended every night.

## Mission Success Criteria

- [ ] `.claude/commands/sync-docs.md` contains zero "NEVER STOP UNTIL ALL N BATCHES" absolute-language prose (lines 9-24 of the current file fully replaced by cursor-driven flow instructions)
- [ ] A single nightly invocation of `/sync-docs` processes exactly one scheduled batch and exits — verified by reading history after a test run shows exactly one `batch_complete` event
- [ ] `.claude/state/sync-docs-cursor.json` persists across sessions; a mid-session simulated crash after commit + before cursor advance re-runs the same batch, not the next one (commit-before-cursor-update idempotency verified)
- [ ] `.claude/state/sync-docs-history.jsonl` contains one JSONL entry per nightly run; entry includes cycle/batch/branch/commit_sha/outcome/tpr_rounds/started_at/ended_at and (when applicable) failure_category
- [ ] `/commit-push --no-push` flag exists and skips Step 7 (git push) while preserving Steps 1-6 — verified by invoking with the flag on a clean repo and observing no `git push` call
- [ ] The canonical `Source` vocabulary in `.claude/skills/add-bug/SKILL.md` includes `sync-docs` — verified by grepping the file for `sync-docs` in the source list
- [ ] The 9 original batches expand to 15-25 shards such that no single shard contains more than ~25 markdown files — verified by batch catalog content
- [ ] All `.md` files currently in-scope per `sync-docs.md` lines 64-81 (excluding off-limits rows) are assigned to exactly one shard — no file is in two shards, no in-scope file is unassigned; verified by an assignment-audit script run by §08
- [ ] The TPR 10-iteration-cap, 3-wasted-rounds-cap, and transport-infra-failure paths all terminate the nightly run without any `AskUserQuestion` call — verified by inspecting the rewritten command file for zero AskUserQuestion references and by §09's wrapper logic exercising each cap-hit path with a log entry rather than a prompt
- [ ] When the cursor advances past the final batch in the catalog, the next invocation emits a `cycle_complete` history event, rolls `cycle_number` forward, and resets `next_batch_id` to the first batch — verified by §10's end-to-end cycle test
- [ ] All 31 TPR findings (22 codex + 10 gemini from the 2026-04-15 review) are resolved in a documented section — `/review-plan` verifies each finding maps to at least one plan section's completion checklist
- [ ] `python -m scripts.plan_corpus check plans/sync-docs-redesign/` returns green — the plan itself passes the corpus validator
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan sync-docs-redesign` returns 0 annotations across the repo — all `TPR-XX-NNN` and `§XX.Y` scaffolding stripped per CLAUDE.md §Plan annotations are temporary scaffolding
- [ ] `./test-all.sh` green after all sections complete — no regressions in compiler tests, spec tests, or plan-audit tests

## Architecture

```
Nightly autonomous flow (per invocation):

      ┌─────────────────────────────────────────────────────────┐
      │  cron / CronCreate / user-manual /sync-docs invocation  │
      └─────────────────────────┬───────────────────────────────┘
                                │
                    ┌───────────▼────────────┐
                    │  Read cursor           │   ← .claude/state/sync-docs-cursor.json
                    │  (pre-worktree)        │     (gitignored, atomic writes)
                    └───────────┬────────────┘
                                │
                    ┌───────────▼────────────┐
                    │  Enter worktree        │   ← sync-docs-c{C}-b{B}-{date}-a{A}
                    │  (branch from dev HEAD)│     (per-night branch)
                    └───────────┬────────────┘
                                │
                    ┌───────────▼────────────┐
                    │  Look up next batch    │   ← batch catalog SSOT (§05)
                    │  from cursor +         │     (format TBD in §05)
                    │  batch catalog         │
                    └───────────┬────────────┘
                                │
                    ┌───────────▼────────────┐
                    │  Run /tpr-review on    │   ← delegates to tpr-review skill
                    │  this batch (custom    │     findings file into THIS plan's
                    │  objective)            │     TPR block (resolves TPR-XX-001
                    └───────────┬────────────┘     because plan owns the section)
                                │
                   ┌────────────┴───────────┐
                   │                        │
                   ▼                        ▼
           ┌──────────────┐         ┌──────────────┐
           │  Convergence │         │  Cap-hit or  │
           │  (clean)     │         │  infra fail  │
           └──────┬───────┘         └──────┬───────┘
                  │                        │
                  ▼                        ▼
           ┌──────────────┐       ┌────────────────────┐
           │ Commit fixes │       │ Log cap_hit event  │
           │ (via         │       │ to history;         │
           │  /commit-push│       │ cursor.status =     │
           │  --no-push)  │       │ manual_attention;   │
           └──────┬───────┘       │ DO NOT advance;     │
                  │               │ exit clean          │
                  ▼               └──────┬──────────────┘
      ┌─────────────────────┐            │
      │ Advance cursor      │            │
      │ (atomic temp-rename)│            │
      │ Append history      │            │
      │ batch_complete event│            │
      └──────┬──────────────┘            │
             │                            │
             │  (if next_batch wraps      │
             │   past catalog end)        │
             ▼                            │
      ┌─────────────────────┐             │
      │ Emit cycle_complete │             │
      │ event; reset cursor │             │
      │ to batch 1; bump    │             │
      │ cycle_number        │             │
      └──────┬──────────────┘             │
             │                            │
             └──────────┬─────────────────┘
                        │
                        ▼
                ┌───────────────┐
                │  Exit worktree│  ← worktree kept (user merges when ready)
                │  (keep)       │    No push from worktree (enforced)
                └───────────────┘
```

## Design Principles

### 1. Cursor decoupled from worktree

The cursor file lives in `.claude/state/` outside any git branch, read via `git rev-parse --git-common-dir` before entering a worktree. Reason: if the cursor lived on a per-night branch, tomorrow's fresh worktree (from current `dev` HEAD) wouldn't see it unless the user merged last night's branch — breaking the autonomous rotation contract. Decoupling the cursor from the worktree means the user can let nightly branches pile up for a week before merging any of them, and rotation continues uninterrupted. (Resolves codex TPR-XX-003 `EXPOSURE: Fresh nightly worktrees need shared cursor state outside the review branches`.)

### 2. Commit-before-cursor-advance (at-least-once idempotency)

The order of durable writes is load-bearing: **commit the batch's fixes first, then atomically advance the cursor**. A crash after the commit but before cursor advance re-runs the same batch tomorrow (redoing verified work is cheap); a crash after cursor advance but before commit would silently skip a batch (the audit trail loses a night of verification — unacceptable). This is the "at-least-once" invariant. (Resolves codex TPR-XX-008 + gemini TPR-XX-004.)

### 3. Plan-owned TPR findings routing

By creating this plan (`plans/sync-docs-redesign/`), `/tpr-review` gains an owning plan for its custom-objective findings. Findings route to this plan's `§N.R Third Party Review Findings` blocks instead of `plans/bug-tracker/` — which resolves the conflict where `/sync-docs` marked `plans/**/*.md` off-limits but its delegated TPR skill wanted to write there. The resolution is architectural, not a carve-out. (Resolves codex TPR-XX-001.)

### 4. Autonomy over rigor at cap boundaries

Nightly runs have no user to prompt. When `/tpr-review` hits its 10-iteration cap, 3-wasted-rounds cap, or transport exhaustion, the nightly wrapper **must intercept before the `AskUserQuestion` call lands**. The replacement policy: log a `cap_hit` event with full context to history, set `cursor.status = manual_attention`, do NOT advance `next_batch_id`, exit clean. The user sees the manual-attention state in the morning and triages — but the nightly run doesn't stall. (Resolves codex TPR-XX-007, TPR-XX-010 + gemini TPR-XX-005.)

### 5. Fact-bound documentation preserved through the rewrite

The original `/sync-docs` §One Rule (FACT-BOUND) is not weakened by this redesign. Every doc change still cites verifiable facts (source line, spec clause, test file). The rewrite preserves this principle verbatim in the restructured command text. CLAUDE.md §Zero Deferral and §Correctness Above All also survive. What the rewrite removes is the "all-9-batches-in-one-session" prose that conflicts with the empirical reality of context limits.

## Section Dependency Graph

```
                    ┌──────────────────────┐
                    │  §01 Cursor + state/ │
                    │  bootstrap           │
                    └──────┬───────────────┘
                           │
          ┌────────────────┼────────────────┐
          │                │                │
          ▼                ▼                ▼
  ┌───────────────┐ ┌──────────────┐ ┌────────────────┐
  │ §02 History   │ │ §03 /commit- │ │ §04 /add-bug   │
  │ file schema   │ │ push --no-   │ │ sync-docs      │
  │               │ │ push flag    │ │ Source value   │
  └───────┬───────┘ └──────┬───────┘ └────────┬───────┘
          │                │                  │
          └────────┬───────┴──────────────────┘
                   │
                   ▼
          ┌────────────────────┐
          │ §05 Batch catalog  │
          │ SSOT (format pick) │
          └────────┬───────────┘
                   │
        ┌──────────┼──────────┐
        │          │          │
        ▼          ▼          ▼
  ┌─────────┐ ┌─────────┐ ┌─────────┐
  │ §06     │ │ §07     │ │ §08     │
  │ Batch 8 │ │ Batch 9 │ │ Scope   │
  │ shards  │ │ shards  │ │ gap     │
  │         │ │         │ │ closure │
  └────┬────┘ └────┬────┘ └────┬────┘
       │           │           │
       └───────────┼───────────┘
                   │
          ┌────────┴──────────┐
          │                   │
          ▼                   ▼
  ┌───────────────┐   ┌──────────────────┐
  │ §09 TPR cap   │   │ §10 Cycle-       │
  │ autonomous    │   │ complete         │
  │ fallback      │   │ detection        │
  └───────┬───────┘   └─────────┬────────┘
          │                     │
          └──────────┬──────────┘
                     │
                     ▼
             ┌───────────────┐
             │ §11 Full      │
             │ sync-docs.md  │
             │ rewrite       │
             └───────┬───────┘
                     │
                     ▼
             ┌───────────────┐
             │ §12 Migration │
             │ + verification│
             │ + cleanup     │
             └───────────────┘
```

**Parallelizable groups:**

- **§02, §03, §04** (after §01): all three touch independent files (history.jsonl definition, commit-push.md edit, add-bug/SKILL.md edit). Can be worked in any order.
- **§06, §07, §08** (after §05): all three are batch-catalog population work. Each defines independent shards.
- **§09, §10** (after §06-§08): independent concerns (cap-handling vs cycle-wrap detection).

**Critical path:**

§01 → §05 → §11 → §12 is the minimum path. The rest parallelize around it.

**Cross-section interactions (must be co-implemented):**

- **§01 + §02**: the cursor schema references history event types; the history schema references cursor state field values. They share a vocabulary (`status`, `outcome`, `failure_category`) that must agree. Co-review at §01/§02 close.
- **§05 + §11**: the command rewrite reads the batch catalog; §05's format choice dictates §11's read protocol. §05 must land with enough detail that §11 can implement the read cleanly.
- **§09 + §11**: the cap-fallback logic lives in the rewritten command file. §09 designs the behavior; §11 encodes it.

## Implementation Sequence

```
Phase 0 — Prerequisites (no behavioral changes)
  └─ §01: Cursor file + .claude/state/ bootstrap

Phase 1 — Foundation (parallel, independent)
  └─ §02: History file schema + append protocol
  └─ §03: /commit-push --no-push flag
  └─ §04: /add-bug sync-docs Source value
  Gate: All three pass their individual completion checklists; no integration needed yet

Phase 2 — Batch catalog
  └─ §05: Batch catalog SSOT (format decision + implementation)
  Gate: python -m scripts.plan_corpus check passes on the catalog artifact (whatever its form)

Phase 3 — Catalog population (parallel)
  └─ §06: Batch 8 sub-batching (design docs, 5-7 shards)
  └─ §07: Batch 9 sub-batching (guide/tooling/modules, 4-5 shards)
  └─ §08: Scope gap closure (remaining .md + commands + skills + rules)
  Gate: assignment-audit script confirms every in-scope .md file is in exactly one shard

Phase 4 — Runtime behaviors
  └─ §09: Autonomous TPR cap fallback
  └─ §10: Cycle-complete detection + reporting
  Gate: simulated cap-hit produces history cap_hit event without AskUserQuestion; simulated cycle wrap produces cycle_complete event

Phase 5 — Command rewrite [CRITICAL PATH]
  └─ §11: Full sync-docs.md rewrite
  Gate: /tpr-review on the rewrite converges clean; command file's NEVER-STOP prose eliminated

Phase 6 — Migration + verification
  └─ §12: First-cycle bootstrap walkthrough, cycle validation, annotation cleanup
  Gate: plan-annotations.sh returns 0; ./test-all.sh green
```

**Why this order:**

- §01 before everything else — the cursor schema is the foundational vocabulary every other section references.
- §03 early (not last) so the command rewrite in §11 can use it; deferring §03 means §11 has nothing to commit with.
- §05 sits in the middle as a synchronization point — §06-§08 are catalog population and can't start until the catalog format is decided.
- §11 is the critical path because all prior work is infrastructure; §11 is where behavior actually changes.
- §12 last to verify everything composed correctly and to strip scaffolding.

**Known failing tests (expected until plan completion):**

None — this plan touches no compiler code, no stdlib, no test harnesses. The only tests affected are the plan-audit tests (`tests/plan-audit/`), which should stay green throughout if the plan is written correctly. If plan-audit fails during section writing, it indicates a schema violation in the plan itself.

## Metrics (Current State)

| Artifact | Current LOC | Notes |
|---|---|---|
| `.claude/commands/sync-docs.md` | 398 | The file being rewritten. Post-rewrite target: ~200-250 lines (cursor flow is more compact than 9-batch prose). |
| `.claude/state/` | 0 (doesn't exist) | Creating this directory is part of §01. |
| `.claude/commands/commit-push.md` | 156 | Adding `--no-push` flag adds ~15-25 lines. |
| `.claude/skills/add-bug/SKILL.md` | 175 | Adding `sync-docs` source value adds 1 line. |
| Batch catalog | 0 | Entire artifact is new; size depends on §05's format choice. |
| `.gitignore` | ~50 | Adding `.claude/state/` line adds 1 line. |

## Estimated Effort

| Section | Est. New Lines | Complexity | Depends On |
|---|---|---|---|
| §01 Cursor + `.claude/state/` bootstrap | ~350 | Medium (new pattern for the codebase — atomic writes, schema design) | — |
| §02 History file schema | ~250 | Low (schema is well-defined from research; event types straightforward) | §01 |
| §03 `/commit-push --no-push` flag | ~200 | Low (add argument-conditional branch following `preview` precedent) | — |
| §04 `/add-bug` Source value | ~100 | Low (single-line addition + documentation update) | — |
| §05 Batch catalog SSOT | ~400 | High (format decision with evidence weighting; affects multiple downstream sections) | §01, §02 |
| §06 Batch 8 sub-batching | ~450 | Medium (7 shards, each needs a TPR objective paragraph) | §05 |
| §07 Batch 9 sub-batching | ~400 | Medium (5 shards) | §05 |
| §08 Scope gap closure | ~350 | Medium (4 new batches: error-docs, rules-remaining, commands, skills) | §05 |
| §09 Autonomous TPR cap fallback | ~400 | High (intercepts 3 `AskUserQuestion` points in the delegated skill — requires careful wrapper logic) | §02 |
| §10 Cycle-complete detection + reporting | ~300 | Medium (cursor semantics + history event + report template) | §01, §02 |
| §11 Full sync-docs.md rewrite | ~650 | High (rewrites the entire command file; preserves fact-bound contract while removing anti-patterns) | All prior |
| §12 Migration + verification + cleanup | ~300 | Medium (end-to-end test + annotation strip) | All prior |
| **Total new** | **~4150** | | |
| **Total deleted** | **~180** (NEVER-STOP prose, Phase 1/3 rewrites in `sync-docs.md`) | | |

## Known Bugs (Pre-existing, surfaced by TPR)

All 31 findings from the 2026-04-15 dual-source TPR (22 codex + 10 gemini, 0 surface agreements — see §Research Reconnaissance below) are tracked here with their resolution mapping.

| Finding ID | Severity | Title | Resolution Section | Status |
|---|---|---|---|---|
| TPR-XX-001-codex | high | /tpr-review routes to plans/bug-tracker — conflicts with plans/** off-limits | §05 (architectural: plan owns scope) + §11 | Not Started |
| TPR-XX-002-codex | high | /commit-push unconditionally pushes | §03 (add --no-push flag) + §11 (use it) | Not Started |
| TPR-XX-003-codex | high | Fresh nightly worktrees need shared cursor outside branches | §01 (cursor in .claude/state/ via git-common-dir) | Not Started |
| TPR-XX-004-codex | high | Define cursor schema and keep it out of plans/markdown scope | §01 | Not Started |
| TPR-XX-005-codex | high | Scope larger than 9 hardcoded batches | §08 (scope gap closure) | Not Started |
| TPR-XX-006-codex | high | Batches 8 and 9 too large for one night | §06 + §07 (sub-batching) | Not Started |
| TPR-XX-007-codex | high | Specify autonomous state machine for running/retry/manual-attention | §01 (state machine) + §09 (cap handling) | Not Started |
| TPR-XX-008-codex | high | Commit before cursor advance (idempotency) | §01 (design principle) + §11 (enforce) | Not Started |
| TPR-XX-009-codex | high | Remove codex-only fallback unless implemented | §09 (either delete promise or build transport support) | Not Started |
| TPR-XX-010-codex | high | Replace AskUserQuestion escalations with autonomous policy | §09 | Not Started |
| TPR-XX-011-codex | medium | Store stable batch IDs in a versioned catalog | §05 | Not Started |
| TPR-XX-012-codex | medium | Include batch and attempt in worktree name | §11 (branch naming scheme) | Not Started |
| TPR-XX-013-codex | medium | Document how user reconciles multiple unmerged branches | §10 (morning-review artifact) + §11 | Not Started |
| TPR-XX-014-codex | medium | Explicit overlap rule for /sync-claude and nightly /sync-docs | §11 (document starting-from-HEAD rule) | Not Started |
| TPR-XX-015-codex | medium | Canonical /add-bug provenance for spec/grammar | §04 (add sync-docs Source) | Not Started |
| TPR-XX-016-codex | medium | Cycle counters + wrap metadata (not invisible increment) | §10 | Not Started |
| TPR-XX-017-codex | medium | Persist morning-readable history outside /tmp | §02 (history file in .claude/state/) | Not Started |
| TPR-XX-018-codex | low | Keep round-robin; one-shot force_next_batch_id override | §01 (cursor schema includes force_next_batch_id) + §11 | Not Started |
| TPR-XX-019-codex | medium | Rewrite intro away from all-9-batches-or-bust | §11 | Not Started |
| TPR-XX-020-codex | medium | Rewrite batch execution and final-report sections | §11 | Not Started |
| TPR-XX-021-codex | medium | Define bootstrap and corruption recovery | §01 | Not Started |
| TPR-XX-022-codex | informational | /continue-roadmap is good gate template, bad persistence template | Acknowledged in §01 design principles | Not Started |
| TPR-XX-001-gemini | high | Define canonical cursor state location in tracked markdown | §01 (rejected "tracked markdown" — user picked gitignored JSON; documented in §01) | Not Started |
| TPR-XX-002-gemini | medium | Migrate batch definitions to state file to prevent drift | §05 | Not Started |
| TPR-XX-003-gemini | high | Specify worktree reuse policy across the cycle | §11 (per-night branch from HEAD; no reuse) | Not Started |
| TPR-XX-004-gemini | high | Enforce commit before cursor update for idempotency | §01 + §11 (same as codex-8) | Not Started |
| TPR-XX-005-gemini | medium | Define autonomous fallback for TPR thoroughness failures | §09 | Not Started |
| TPR-XX-006-gemini | high | Add autopilot flag to add-bug to prevent interactive stall | **REFUTED by research** (A.3): /add-bug is already non-interactive. §04 documents the verified-non-interactive finding. | Not Started (verification only) |
| TPR-XX-007-gemini | high | Split batch 9 to respect session context limits | §07 (same as codex-6 for batch 9) | Not Started |
| TPR-XX-008-gemini | medium | Create a cycle summary artifact for morning review | §10 | Not Started |
| TPR-XX-009-gemini | low | Remove obsolete NEVER STOP absolute directives | §11 | Not Started |
| TPR-XX-010-gemini | low | Define bootstrap behavior for missing cursor | §01 | Not Started |

**Verification-refuted findings:** TPR-XX-006-gemini claimed `/add-bug` may prompt interactively. Research (research-agent Part A.3) read the file end-to-end and confirmed `/add-bug` has NO `AskUserQuestion` call, its `allowed-tools` frontmatter explicitly omits `AskUserQuestion`, and Step 8 explicitly mandates resuming the caller without pause. The claim was gemini speculation (`basis: inference`) not backed by file inspection. Per the trust-tier rule, this is exactly the verification outcome that justifies the lower-trust stance on gemini findings.

## Research Reconnaissance

TPR review run 2026-04-15 (custom-objective mode, design review of current `.claude/commands/sync-docs.md`):

- Run directory: `/tmp/ori-tpr-6YT7xvRy/` (ephemeral — /tmp tmpfs)
- Codex envelope: 23550 bytes, 22 findings (21 actionable + 1 informational), `basis: direct_file_inspection`
- Gemini envelope: 8050 bytes, 10 findings (all actionable), `basis: inference`
- Attempts: succeeded on attempt 4/5 (3 prior attempts failed with `gemini_api_capacity`)
- Thoroughness judgment: MODERATE asymmetry (walltime 1.1x, events 1.3x comparable; bytes 14.6x explained by codex whole-file reads vs gemini targeted greps) — both reviewers thorough
- Verification result: 5 high-severity codex findings spot-verified against actual code (all confirmed). 1 high-severity gemini finding full-verified (REFUTED — /add-bug is autonomous).

Follow-up research (research-agent, 2026-04-15) verified: Batch 8 = 79 files exactly, Batch 9 = ~99 files, .claude/commands/ = 13 files, .claude/skills/*/SKILL.md = 21 files, .claude/state/ does not exist, .gitignore does NOT currently cover .claude/state/, /continue-roadmap and /fix-next-bug are both pure scan-based with zero cursor precedent (the cursor design is net-new pattern for the codebase), plan-corpus validator scans only plans/ (cursor file completely invisible to validation), atomic temp-rename writes are new to the .claude/ ecosystem (no existing precedent but pattern is correct), no existing env var/flag suppresses /tpr-review's AskUserQuestion escalations.

## Quick Reference

| ID | Title | File | Status |
|---|---|---|---|
| 01 | Cursor file + `.claude/state/` bootstrap | `section-01-cursor-state.md` | Not Started |
| 02 | History file schema + append protocol | `section-02-history-file.md` | Not Started |
| 03 | `/commit-push --no-push` flag | `section-03-commit-push-flag.md` | Not Started |
| 04 | `/add-bug` `sync-docs` Source value | `section-04-add-bug-source.md` | Not Started |
| 05 | Batch catalog SSOT (format decision) | `section-05-batch-catalog.md` | Not Started |
| 06 | Batch 8 sub-batching (design docs, 79 files) | `section-06-batch-8-shards.md` | Not Started |
| 07 | Batch 9 sub-batching (guide/tooling/modules, ~99 files) | `section-07-batch-9-shards.md` | Not Started |
| 08 | Scope gap closure (remaining .md + rules + commands + skills) | `section-08-scope-gap.md` | Not Started |
| 09 | Autonomous TPR cap fallback | `section-09-tpr-cap-fallback.md` | Not Started |
| 10 | Cycle-complete detection + reporting | `section-10-cycle-complete.md` | Not Started |
| 11 | Full `sync-docs.md` rewrite | `section-11-command-rewrite.md` | Not Started |
| 12 | Migration + verification + cleanup | `section-12-migration.md` | Not Started |
