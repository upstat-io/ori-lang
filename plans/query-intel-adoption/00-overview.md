---
plan: "query-intel-adoption"
title: "Query-Intel Adoption: Exhaustive Implementation Plan"
status: in-progress
references:
  - "plans/query-intel-adoption/tpr-2026-04-14-merged.json"
  - "plans/completed/lang-intelligence/"
  - ".claude/rules/intelligence.md"
---

# Query-Intel Adoption: Exhaustive Implementation Plan

## Mission

Drive deep, ambient adoption of the Neo4j-backed intelligence graph (`/query-intel`, `scripts/intel-query.sh` → `../lang_intelligence/neo4j/query_graph.py`) across ALL Claude Code artifacts — skills, commands, rules, `CLAUDE.md`, the plan schema, hooks, and the tool's own output shapes. The graph houses 191K symbols, 505K CALLS edges, and 298K issues across 11 repos, synced on every commit, yet the code-symbol side (`file-symbols`/`callers`/`callees`/`similar`) is barely used despite scanning the Ori codebase ~100× faster than grep. This plan transforms intel from an opt-in capability into the default-on reconnaissance layer for design, bug investigation, review, planning, and tooling work.

## Mission Success Criteria

- [x] `/query-intel` is a Skill (`.claude/skills/query-intel/SKILL.md`) the harness can auto-trigger; the 14-line command file remains as a thin alias. (§01)
- [x] `CLAUDE.md` teaches the graph in the Commands, Key Paths, Reference Repos, Ownership & Deferral, and Compiler Coding Guidelines sections — all verified insertion points from the 2026-04-14 TPR. (§02)
- [x] Exactly ONE canonical intel-summary helper exists at `.claude/skills/dual-tpr/compose-intel-summary.md`; all 18 inlined copies (6 review-family + 12 wider-skill consumers discovered mid-execution) are replaced with `@`-includes. Zero LEAK:algorithmic-duplication for the intel-pre-query pattern. Invariant verified: `grep -l 'scripts/intel-query.sh status' .claude/ -r` returns exactly 3 files (SSOT + 2 legitimate teaching surfaces: `intelligence.md` rule, `query-intel.md` command). (§03)
- [x] All 10 rule files that cite cross-repo prior art (`arc.md`, `aims-rules.md`, `typeck.md`, `types.md`, `tests.md`, `impl-hygiene.md`, `canonicalization.md`, `patterns.md`, `compiler.md`, plus `intelligence.md` index refresh) include a graph-first paragraph before their manual-browsing guidance. (§04)
- [x] The 3 gap skills (`verify-tpr`, `sync-claude`, `fix-next-bug`) and 3 gap commands (`sync-spec`, `sync-grammar`, `verify-roadmap`) each include a concrete graph-query workflow step (not a token bullet) that `@`-includes §03's helper. (§05) — `tp-help` was already migrated in §03.
- [ ] `.claude/skills/create-plan/plan-schema.md` mandates an UNNUMBERED `## Intelligence Reconnaissance` block in every new `FileClass.PLAN_SECTION` file (roadmap and bug-tracker sections are explicitly out of scope); `python -m scripts.plan_corpus check` enforces it with a WARNING/ERROR outcome model gated by section `status` (not-started → HIGH; in-progress → MEDIUM; complete → exempt). `--strict-recon` promotes not-started missing-recon findings to ERROR. (§06)
- [ ] Retrofit of `status: not-started` sections across the active plan corpus completes via the permanent `python -m scripts.plan_corpus retrofit-recon` subcommand; `python -m scripts.plan_corpus discover` reports 100% coverage for the not-started slice of every active plan. §01-§05 of this plan (already `status: complete`, `reviewed: true`) are NOT touched — no historical-fiction retrospective-recon injected. (§09)
- [ ] `.claude/hooks/pre-review-intel.sh` fires on `UserPromptSubmit` for review-family slash-commands, injects a bounded Intelligence Summary via `hookSpecificOutput.additionalContext`, and degrades silently when the graph is unavailable. Registered in `.claude/settings.json`. (§07)
- [ ] `scripts/intel-query.sh` supports `--help`, defaults to `--human` on tty, has a `blast-radius` composite subcommand, and a `--format md` mode. `../lang_intelligence/neo4j/query_graph.py` emits ASCII call-trees for callers/callees, grouped output for file-symbols, and clickable `file:line` deep-links. (§08)
- [ ] `./test-all.sh` green — no regressions in Rust test suites from any side-effect of the plan's changes.
- [ ] Meta-validation: `/tpr-review` run on the plan's own aggregate changes with the custom objective "verify that this plan's output makes the graph ambient" observes `pre-review-intel.sh` injecting a summary into its own reviewer prompts (self-referential dogfood test).
- [ ] All section success criteria met (see each `section-NN-*.md` frontmatter `success_criteria`).

## Architecture

```
                    Operator / Claude session
                             │
                             ▼
        ┌────────────────────────────────────────────┐
        │                                            │
     auto-trigger                                 explicit
   (Skill description                             invocation
    + UserPromptSubmit hook)                       /query-intel
        │                                            │
        ▼                                            ▼
┌──────────────────────────────────────────────────────────┐
│  .claude/skills/query-intel/SKILL.md      [§01]          │
│  .claude/commands/query-intel.md (alias)  [§01]          │
└────────────────────────┬─────────────────────────────────┘
                         │
                         ▼
       ┌──────────────────────────────────┐
       │  scripts/intel-query.sh  [§08a]  │  ← tty-aware default,
       │  (wrapper, 206 lines today)      │    --help, blast-radius,
       └────────────────┬─────────────────┘    --format md
                        │
                        ▼
       ┌───────────────────────────────────────┐
       │  ../lang_intelligence/neo4j/          │
       │    query_graph.py  [§08b]             │  ← ASCII call-trees,
       │    (1240 lines)                       │    grouped file-symbols,
       │                                       │    deep-links
       └────────────────┬──────────────────────┘
                        │
                        ▼
              Neo4j (lang-intelligence container)
              11 repos · 191K symbols · 505K CALLS · 298K issues

Consumers of the SSOT helper (§03 fans out):

  .claude/skills/dual-tpr/compose-intel-summary.md   ← NEW SSOT [§03]
        │
        │ True @-include consumers (skill/command prompts harness-expanded):
        │
        ├── .claude/skills/tpr-review/SKILL.md                [§03]
        ├── .claude/skills/review-work/SKILL.md               [§03]
        ├── .claude/skills/review-plan/SKILL.md + step-*.md   [§03]
        ├── .claude/commands/review-work.md                   [§03]
        ├── .claude/commands/independent-review.md            [§03]
        ├── .claude/commands/review-bugs.md                   [§03]
        ├── .claude/skills/tp-help/SKILL.md                   [§03]
        ├── .claude/skills/verify-tpr/SKILL.md                [§05]
        ├── .claude/skills/sync-claude/SKILL.md               [§05]
        ├── .claude/skills/fix-next-bug/SKILL.md              [§05]
        ├── .claude/commands/sync-spec.md                     [§05]
        ├── .claude/commands/sync-grammar.md                  [§05]
        ├── .claude/commands/verify-roadmap.md                [§05]
        └── .claude/hooks/pre-review-intel.sh                 [§07]
        │
        │ Contract followers (follow the §03 format contract but are NOT
        │ harness-expanded — they are tools/templates that reference the
        │ SSOT's citation grammar and ≤500-char bound without @-including it):
        │
        ├── .claude/skills/create-plan/plan-schema.md         [§06]  ← template (markdown, not expanded)
        └── scripts/plan_corpus/retrofit_recon.py             [§09]  ← Python tool (no @-include expansion)

Ambient teaching surface (CLAUDE.md §02 + rule files §04):

  CLAUDE.md                                           [§02]
        ├─ Commands (line 140)           ← add /query-intel
        ├─ Key Paths (line 182)          ← add intel paths
        ├─ Reference Repos (line 186)    ← graph-first paragraph
        ├─ Ownership & Deferral (line 38)← strengthen fact-check rule
        └─ Compiler Coding Guidelines    ← graph-recon bullet

  .claude/rules/*.md                                  [§04]
        ├─ arc.md, aims-rules.md, typeck.md, types.md
        ├─ tests.md, impl-hygiene.md
        ├─ canonicalization.md, patterns.md
        └─ intelligence.md (workflow inventory refresh)
```

## Design Principles

**SSOT for the injection template.** The single most common pattern across the plan — "check availability → query file-symbols → query callers/callees → query similar → condense to ≤500 chars" — lives in ONE file (`compose-intel-summary.md`) and every consumer `@`-includes it. This mirrors how `.claude/skills/dual-tpr/polling-protocol.md` is the SSOT for dual-source polling and `compose-rules-brief.md` is the SSOT for rules-brief composition. No parallel copies, no drift.

**Graceful degradation.** Every new trigger (skill step, hook, rule-embedded guidance, plan-schema subsection) respects `scripts/intel-query.sh status` and silently yields when the graph is unavailable. Intelligence is ADDITIVE, never blocking — per the contract already established in `.claude/rules/intelligence.md`.

**Graph-first, not graph-only.** Results are for DISCOVERY, not replacement. The graph tells the reader WHERE to look; manual code reading still verifies. Every rule-file paragraph and skill step in §04/§05 reinforces this — "query first, read the code it points at, never cite a Neo4j result without verifying against actual source."

**Ambient over explicit.** The hook-heavy choice (§07) means review-family slash-commands get intel summaries INJECTED by default, not only when the operator remembers to invoke `/query-intel`. The SKILL promotion (§01) means the harness can auto-trigger even on non-review contexts where the description matches. Adoption flows from "always-on by default" rather than "available if you remember."

## Section Dependency Graph

```
            ┌────┐      ┌────┐
            │ 01 │      │ 02 │        (independent — parallel)
            └──┬─┘      └────┘
               │
               ▼
            ┌────┐
            │ 03 │                    (needs 01's Skill dir settled)
            └──┬─┘
               │
      ┌────────┼────────┬─────────┐
      │        │        │         │
      ▼        ▼        ▼         ▼
   ┌────┐  ┌────┐   ┌────┐    ┌────┐
   │ 04 │  │ 05 │   │ 06 │    │ 07 │   (all @-include §03;
   └────┘  └────┘   └─┬──┘    └────┘    §04 also parallel)
                     │
                     ▼
                  ┌────┐
                  │ 09 │                 (retrofit: depends on §06 validator)
                  └────┘
                                         ┌────┐
                                         │ 08 │   (fully independent —
                                         └────┘    scripts/ + ../lang_intelligence/)
```

- §01 and §02 are independent and parallelizable.
- §03 requires §01 (lives under `.claude/skills/dual-tpr/` — confirms Skill promotion structural decisions).
- §04, §05, §06, §07 all depend on §03 (they `@`-include the new SSOT helper).
- §04 is additionally non-blocking with respect to §05–§07 (mechanical paragraph inserts into 10 rule files).
- §09 depends on §06 — the retrofit tool enumerates `status: not-started` sections, writes stub recon blocks, and measures coverage via §06.2's per-plan reporter. §06 must land before §09 can start.
- §08 has no `.claude/` dependencies — touches only `scripts/intel-query.sh`, `.claude/commands/query-intel.md`, and `../lang_intelligence/neo4j/query_graph.py`.

**Cross-section interactions (must be co-implemented):**
- **§03 + §05**: The SSOT helper and the first consumers of it must land together so no skill is left with a half-migrated pattern. Writing §03 means completing §05's `@`-include swaps in the same work window.
- **§07 + §03**: The hook's output format MUST match §03's bounded summary template exactly — otherwise consumers see two slightly different summaries depending on whether the hook or the skill-embedded `@`-include fired.

## Implementation Sequence

```
Phase 0 - Foundation  (parallelizable)
  ├─ Section 01: Promote /query-intel to an auto-triggerable Skill
  └─ Section 02: CLAUDE.md expansion

Phase 1 - SSOT helper
  └─ Section 03: Create compose-intel-summary.md and migrate 6 consumers
  Gate: `grep -r "scripts/intel-query.sh.*status" .claude/skills .claude/commands | wc -l`
        returns 1 (the SSOT) or an @-include count — never inlined copies

Phase 2 - Fan-out  (parallelizable)  [CRITICAL PATH for adoption coverage]
  ├─ Section 04: Rule files graph-first paragraphs
  ├─ Section 05: Missing-trigger skills + commands
  ├─ Section 06: Plan-schema mandatory Intelligence Reconnaissance block + validator
  └─ Section 07: pre-review-intel.sh hook + settings.json registration

Phase 3 - Retrofit  (depends on §06)
  └─ Section 09: Retrofit active plans using status-gated severity

Phase 4 - Tool UX polish  (fully parallel)
  └─ Section 08: scripts/intel-query.sh + query_graph.py output shapes

Phase 5 - Meta-validation
  └─ /tpr-review on the plan's aggregate changes with custom objective:
     "verify this plan's output makes the graph ambient" — confirms the
     hook fires on its own reviewer prompts (self-referential dogfood).
  Gate: TPR envelope shows `pre-review-intel.sh` injection in both
        reviewers' context snapshots.
```

**Why this order:**
- Phase 0 is pure additions — new Skill file, new doc paragraphs. No behavioral change to existing workflows.
- Phase 1 is the hinge. Migrating the inlined copies to `@`-includes is where the LEAK is resolved; every later section depends on this SSOT existing.
- Phase 2 is the fan-out where adoption surface multiplies — 10 rule files + 4 skills + 3 commands + plan-schema + hook. Can be parallelized because each lands in a different file.
- Phase 3 is retrofit — §06's validator must be live before §09 can measure its own retrofit completion, so §09 sits downstream of §06.
- Phase 4 is tool UX — orthogonal to prompt-surface changes. Could ship earlier but benefits most when Phase 2 consumers are exercising the tool.
- Phase 5 is the integration test: does the whole thing actually make the graph ambient?

**Known failing tests (expected until plan completion):**

None. This plan does not modify compiler code; no test failures are expected. `./test-all.sh` must remain green throughout.

## Metrics (Current State)

| Surface | Intel references | Notes |
|---------|------------------|-------|
| `.claude/skills/` (19 skills) | 15 of 19 reference; 4 have none (`benchmark`, `verify-tpr`, `fix-next-bug`, `sync-claude`) | Depth varies; several mentions are token bullets |
| `.claude/commands/` (13 commands) | 6 of 13 reference; 7 have none (`tp-help`, `commit-push`, `pr-main`, `sync-spec`, `sync-grammar`, `zero-roadmap`, `verify-roadmap`) | Of the 6 that reference, 4 contain inlined pre-query patterns that should be `@`-includes |
| `.claude/rules/` (28 rules) | 1 of 28 (only `intelligence.md` itself) | Rule files that cite cross-repo prior art without graph-first guidance: 9 |
| `CLAUDE.md` | 1 mention (line 38) | No presence in Commands / Key Paths / Reference Repos sections |
| `scripts/intel-query.sh` | 206 lines; no `--help`; defaults to JSON | — |
| `../lang_intelligence/neo4j/query_graph.py` | 1240 lines | Output is flat lists; no ASCII trees, no grouped file-symbols, no clickable deep-links |
| `.claude/hooks/` | 4 hooks; none surface the graph | Registered matchers: PreToolUse for Bash commands |

## Estimated Effort

| Section | Est. plan LOC | Complexity | Depends On |
|---------|---------------|------------|------------|
| 01 Promote /query-intel to Skill | ~180 | Low | — |
| 02 CLAUDE.md expansion | ~150 | Low | — |
| 03 compose-intel-summary SSOT | ~200 | Medium | 01 |
| 04 Rules graph-first | ~220 | Low | 03 |
| 05 Missing-trigger skills | ~280 | Medium | 03 |
| 06 Plan-schema recon + validator | ~260 | Medium | 03 |
| 07 pre-review-intel hook | ~240 | Medium | 03 |
| 08 Tool UX + output | ~320 | High | — |
| 09 Retrofit active plans | ~200 | Medium | 06 |
| **Total plan text** | **~2050** | | |
| **Total files touched (plan execution)** | **~40** | | |

## Known Bugs (Pre-existing)

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|--------------|--------|
| `scripts/intel-query.sh --help` returns "Unknown command" (no help handling) | Wrapper lacks explicit `--help`/`-h` parsing; falls through to `unavailable` path | Section 08a | Not Started |
| `scripts/intel-query.sh` defaults to JSON even on tty | Wrapper does not check `[[ -t 1 ]]`; JSON is always default unless `--human` passed | Section 08a | Not Started |
| `query_graph.py` emits flat list for `callers`/`callees` — no tree structure | Output formatter treats all rows identically | Section 08b | Not Started |
| LEAK:algorithmic-duplication — intel pre-query pattern inlined in 18 consumer files (6 review-family + 12 wider skills) | No SSOT helper existed | Section 03 | Complete (2026-04-14) |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Promote /query-intel to an auto-triggerable Skill | `section-01-promote-to-skill.md` | Complete |
| 02 | CLAUDE.md expansion | `section-02-claude-md-expansion.md` | Complete |
| 03 | SSOT: compose-intel-summary helper | `section-03-compose-intel-summary-ssot.md` | Complete |
| 04 | Rule files: graph-first guidance | `section-04-rules-graph-first.md` | Complete |
| 05 | Missing-trigger skills & commands | `section-05-missing-trigger-skills.md` | Complete |
| 06 | Plan schema: mandatory Intelligence Reconnaissance block + validator | `section-06-plan-schema-recon.md` | In Progress |
| 07 | Hook-heavy ambient automation | `section-07-pre-review-intel-hook.md` | Not Started |
| 08 | Tool UX & output shapes | `section-08-tool-ux-and-output.md` | Not Started |
| 09 | Retrofit active plans — status-gated recon coverage | `section-09-retrofit.md` | Not Started |

## Provenance

This plan was generated from a dual-source `/tpr-review` (codex + gemini) run on 2026-04-14. The merged envelope — 45 findings, 40 actionable, 5 informational — is preserved at `plans/query-intel-adoption/tpr-2026-04-14-merged.json` (51 KB). Transport run: `/tmp/ori-tpr-JrlGnDIS`. Thoroughness: ASYMMETRY HIGH — codex invested 833s of deep investigation with a 37 KB envelope; gemini finished in 232s with a 10 KB envelope and admitted `verification_gaps` in its own output. Despite 0 title-level agreements, both reviewers converged on the same 8 topical clusters that map directly to the 8 sections above. Codex findings were spot-checked; gemini claims were independently verified against actual code per the project's reviewer-trust-tier rule before being incorporated.

The full `/create-plan` workflow's internal `/tp-help` consensus loop (Phase 1D) was deliberately skipped because running another dual-source consensus round on the same 8-cluster decomposition would be circular — the underlying consensus was already established by the parent TPR. Phase 2's multi-pass research was condensed because the subject is meta-tooling (`.claude/**/*.md`, `scripts/intel-query.sh`, `../lang_intelligence/neo4j/query_graph.py`), not compiler architecture. All schema requirements (frontmatter fields, per-subsection close-out blocks, `/improve-tooling` + `/sync-claude` at every subsection, completion checklists, `{NN}.R` Third Party Review blocks) are preserved exactly. Phase 5's `/review-plan` run on this directory remains available as an explicit operator opt-in before execution begins.
