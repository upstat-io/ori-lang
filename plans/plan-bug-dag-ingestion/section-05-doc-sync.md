---
section: "05"
title: "Doc sync + consumer wiring"
status: complete
reviewed: true
goal: "Integrate the 5 new intel-query.sh subcommands (plan-status, blocks, bugs-for, symbol-plans, dag-ascii) into the project's documentation surface: .claude/rules/intelligence.md (When to Query + How to Query), .claude/skills/query-intel/compose-intel-summary.md (Step F consumer registry), CLAUDE.md (§Commands + §Intelligence paragraph), and ~/projects/lang_intelligence/CLAUDE.md (pipeline documentation)."
success_criteria:
  - ".claude/rules/intelligence.md §When to Query lists the new query patterns: 'Plan/bug graph queries (plan-status/blocks/bugs-for/symbol-plans/dag-ascii) — project state, dependency chains, blast-radius from symbols to plans'."
  - ".claude/rules/intelligence.md §How to Query shows at least 3 example invocations per subcommand (grouped under a new 'Plan/bug graph queries' heading)."
  - ".claude/skills/query-intel/compose-intel-summary.md Step F has a new 'Plan/bug graph consumers' subsection listing the subcommands and which existing consumer skills (e.g. /continue-roadmap, /fix-next-bug, /review-plan, /review-bugs) can leverage them; bounded extension rules still apply (2-3 bullet cap)."
  - "CLAUDE.md §Commands (or §Intelligence graph) paragraph mentions the 5 new subcommand names with a one-line description each."
  - "~/projects/lang_intelligence/CLAUDE.md documents the plan/bug graph pipeline alongside the existing code-graph pipeline: schema surface, ingestion path (exporter → importer), sync cadence, query subcommands."
  - "All four files pass /sync-claude verification: no broken references, no stale TOC entries, no missing cross-links."
  - "grep -l 'plan-status\\|symbol-plans\\|dag-ascii' .claude/rules/intelligence.md CLAUDE.md ~/projects/lang_intelligence/CLAUDE.md .claude/skills/query-intel/compose-intel-summary.md returns all four paths."
  - "Satisfies mission criterion: '.claude/rules/intelligence.md \"When to Query\" and \"How to Query\" sections list the new subcommands and their use cases...'."
inspired_by:
  - "plans/query-intel-adoption/section-03-compose-intel-summary-ssot.md — SSOT pattern for intel-query helper docs"
  - "plans/query-intel-adoption/section-04-rules-graph-first.md — rules file extension pattern for graph-first guidance"
depends_on: ["04"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Update .claude/rules/intelligence.md — When to Query + How to Query"
    status: complete
  - id: "05.2"
    title: "Update .claude/skills/query-intel/compose-intel-summary.md Step F consumer registry"
    status: complete
  - id: "05.3"
    title: "Update CLAUDE.md §Commands / §Intelligence paragraph"
    status: complete
  - id: "05.4"
    title: "Update ~/projects/lang_intelligence/CLAUDE.md pipeline docs"
    status: complete
  - id: "05.N"
    title: "Completion Checklist"
    status: complete
---

# Section 05: Doc sync + consumer wiring

**Status:** Complete
**Goal:** Integrate the 5 new `intel-query.sh` subcommands (`plan-status`, `blocks`, `bugs-for`, `symbol-plans`, `dag-ascii`) into every documentation surface that an Ori session or skill might consult when deciding whether and how to query the plan/bug graph. When this section completes, any developer reading `.claude/rules/intelligence.md`, any skill reading `compose-intel-summary.md`, any reviewer reading `CLAUDE.md`, or any `lang_intelligence` contributor reading its `CLAUDE.md` will find the new subcommands documented with working examples — zero stale gaps.

**Success Criteria:**

- [x] `.claude/rules/intelligence.md` §When to Query has a new bullet for plan/bug graph queries listing all 5 subcommands and the skills that benefit (see §05.1).
- [x] `.claude/rules/intelligence.md` §How to Query has a new "Plan/bug graph queries (Ori plan corpus)" block after the existing "Code symbol queries" block, with ≥3 example invocations per subcommand (see §05.1).
- [x] `.claude/skills/query-intel/compose-intel-summary.md` Step F registry has a new "Plan/bug graph consumers" group with entries for `/continue-roadmap`, `/review-plan`, and `/fix-next-bug`, each capped at 2-3 bullets (see §05.2).
- [x] `CLAUDE.md` §Intelligence graph paragraph (and optionally §Commands block) mentions all 5 new subcommand names with one-line descriptions each (see §05.3).
- [x] `~/projects/lang_intelligence/CLAUDE.md` has a "Plan/Bug Graph Pipeline" section documenting schema surface, ingestion path, sync cadence, query subcommands, graceful degradation, and test files (see §05.4).
- [x] `grep -l 'plan-status\|symbol-plans\|dag-ascii' .claude/rules/intelligence.md CLAUDE.md ~/projects/lang_intelligence/CLAUDE.md .claude/skills/query-intel/compose-intel-summary.md` returns all four paths with exit 0.
- [x] `/sync-claude` run clean across all four touched files — no broken cross-links, no stale TOC entries.
- [x] Satisfies mission criterion: "`.claude/rules/intelligence.md` 'When to Query' and 'How to Query' sections list the new subcommands and their use cases...".

**Context:** §04 delivered the 5 new `query_graph.py` handlers and wired them through `intel-query.sh`. But those subcommands are invisible to sessions unless the doc surfaces that skills and developers consult are updated. Three different audiences need coverage: (1) sessions running skills — they read `.claude/rules/intelligence.md` to know when/how to query; (2) skill authors — they read `compose-intel-summary.md` Step F to know which consumer extensions exist; (3) developers and tooling maintainers — they read `CLAUDE.md` and `lang_intelligence/CLAUDE.md`. All four files must be updated atomically before §05 is closed, because a partial update (e.g. `intelligence.md` updated but `CLAUDE.md` not) leaves one audience without guidance and fails the `grep -l` gate in the success criteria. Per `impl-hygiene.md §SSOT`, `compose-intel-summary.md` is the SSOT for all intel-query consumer extensions — additions must preserve the SSOT shape (no forking, no parallel list). Per `impl-hygiene.md §Fact-Bound Documentation Sync`, every change must cite a verifiable code location (the `query_graph.py` handler function name, the `schema.cypher` label names from §02, the `lefthook.yml` post-commit hook from §03).

**Reference implementations:**
- **`plans/query-intel-adoption/section-03-compose-intel-summary-ssot.md`**: SSOT convergence pattern — shows how to add a new consumer group to Step F without forking the SSOT.
- **`plans/query-intel-adoption/section-04-rules-graph-first.md`**: rules file extension pattern — how `intelligence.md` §When to Query grows a new bullet without invalidating existing ones.

**Depends on:** Section 04 (the 5 subcommand handlers — `cmd_plan_status`, `cmd_blocks`, `cmd_bugs_for`, `cmd_symbol_plans`, `cmd_dag_ascii` — must exist in `query_graph.py` and pass their unit tests before doc references to them are accurate).

---

## Intelligence Reconnaissance

Queries run 2026-04-17:

- `scripts/intel-query.sh --human search "documentation drift detection" --limit 5` — 5 results (go#24489 clock drift, go#42747 CPU feature detection, typescript#57267 organizeImports change detection). None related to doc-sync patterns; documentation drift in Ori context is a meta-tooling problem not tracked in reference-repo issue trackers.
- `scripts/intel-query.sh --human symbols "sync_claude" --repo ori` — 0 results. `/sync-claude` is a skill (Markdown/shell), not a Rust symbol; the graph indexes compiled symbols only. Expected absence.
- `scripts/intel-query.sh --human similar "command documentation" --repo rust,go,typescript --limit 5` — no embedding found for the phrase. Phrase-level embeddings require prior indexing; freeform phrases are not embedded. No cross-repo prior art found via vector search for this specific meta-concern.
- All four queries confirm: this section's work is documentation-only, covering Python and Markdown files not indexed by the graph. No Rust symbols change. Intel graph blast-radius: zero.

Results summary (≤500 chars) [ori]: Graph available (Neo4j 5.26.24, 32K+ Ori symbols, 505K+ CALLS edges). All four queries returned zero signal: `sync_claude` and `cmd_*` doc-sync patterns are meta-tooling (Python/Markdown), absent from the Rust-symbol graph. Reference-repo search for "documentation drift" returned unrelated issues. This confirms §05 is a pure doc/meta-tooling section; no code blast-radius, no cross-repo prior art applies. Implementation grounded entirely by reading the four target files directly.

See `.claude/skills/query-intel/compose-intel-summary.md` for the full query protocol (SSOT — do NOT `@`-include in plan files; plan markdown is not harness-expanded, so the include would be a dead literal).

---

## 05.1 Update `.claude/rules/intelligence.md` — When to Query + How to Query

**File:** `.claude/rules/intelligence.md`

This subsection adds two blocks to `intelligence.md`: (1) a new bullet in §When to Query naming the 5 new subcommands and the skills that benefit from them; (2) a new "Plan/bug graph queries" example block in §How to Query after the existing "Code symbol queries" block. The update must be fact-bound: every subcommand name must resolve to a `query_graph.py` handler verified in §04; every skill name must exist in the skills directory.

### What to add: §When to Query

The current §When to Query list ends with a **Tooling** bullet. Add a new bullet immediately before **Tooling** (after **Roadmap** entries), since plan/bug graph queries are most relevant to roadmap-family and bug-triage workflows:

**Proposed diff (not yet applied):**

```markdown
-**Tooling** (/improve-tooling): `symbols` to check if similar tools already exist before creating new ones
+**Plan/bug graph queries** (/continue-roadmap, /fix-next-bug, /review-plan, /review-bugs): the 5 plan-corpus subcommands answer project-state questions without Neo4j/Cypher knowledge:
+  - `plan-status "<plan-name>"` — health check (section counts, blocker counts, bug counts) before starting work on a plan
+  - `blocks "<section-id>"` — transitive BLOCKED_BY + DEPENDS_ON chain to root blocker; use before picking the next section
+  - `bugs-for "<plan-name>"` — all open bugs blocking any section of the plan, sorted by severity; use for pre-section triage
+  - `symbol-plans "<symbol-name>"` — all plans/sections/bugs referencing a symbol via MENTIONS_CODE → CodeReference → RESOLVES_TO; use for cross-plan refactor impact analysis
+  - `dag-ascii "<plan-name>"` — ASCII tree of plan hierarchy + blocker edges (or `--format dot` for Graphviz); use for plan visualization and onboarding
+**Tooling** (/improve-tooling): `symbols` to check if similar tools already exist before creating new ones
```

### What to add: §How to Query

After the existing "Code symbol queries" block (the `# Cross-repo semantic similarity` block ends the existing examples), add a new "Plan/bug graph queries" heading with 3+ example invocations per subcommand:

**Proposed diff (not yet applied):**

```markdown
+# Plan/bug graph queries (Ori plan corpus — requires §02 schema and §03 sync to be deployed)
+scripts/intel-query.sh plan-status plan-bug-dag-ingestion
+scripts/intel-query.sh plan-status empty-container-typeck-phase-contract --json
+scripts/intel-query.sh plan-status plans/roadmap
+
+scripts/intel-query.sh blocks plan-bug-dag-ingestion/section-02-neo4j-schema-importer.md
+scripts/intel-query.sh blocks BUG-04-057 --json
+scripts/intel-query.sh blocks plan-empty-container/section-03-bodies-pass-integration.md
+
+scripts/intel-query.sh bugs-for empty-container-typeck-phase-contract
+scripts/intel-query.sh bugs-for plan-bug-dag-ingestion --json
+scripts/intel-query.sh bugs-for plans/roadmap
+
+scripts/intel-query.sh symbol-plans eval_iter_next --repo ori
+scripts/intel-query.sh symbol-plans check_map_key_hashable --json
+scripts/intel-query.sh symbol-plans IteratorValue
+
+scripts/intel-query.sh dag-ascii plan-bug-dag-ingestion
+scripts/intel-query.sh dag-ascii empty-container-typeck-phase-contract --format dot
+scripts/intel-query.sh dag-ascii plan-bug-dag-ingestion --json
```

### Use-by-workflow mapping

This table clarifies which skill benefits from which subcommand. It must be woven into the examples or a note block adjacent to the new "Plan/bug graph queries" heading:

| Subcommand | Primary skill(s) | When to use |
|---|---|---|
| `plan-status` | `/continue-roadmap` (Step 2.1), `/roadmap-work` | Health check before committing a session to a plan section |
| `blocks` | `/continue-roadmap` (gate check), `/fix-next-bug` | Unblock chain before picking the next item; expose hidden transitive dependencies |
| `bugs-for` | `/review-bugs`, `/continue-roadmap` | Pre-section bug triage; find all bugs that must be fixed before a section can close |
| `symbol-plans` | `/impl-hygiene-review`, `/fix-bug` (blast-radius) | Cross-plan refactor impact: find all plans/bugs that will be affected by renaming or changing a symbol |
| `dag-ascii` | `/review-plan`, `/create-plan` | Plan structure visualization; onboarding; identifying dependency bottlenecks |

### Tasks

- [x] Read `.claude/rules/intelligence.md` §When to Query (current bullet list) and identify the insertion point for the new "Plan/bug graph queries" bullet (after the last roadmap/roadmap-verification bullet, before "Tooling").
- [x] Verify all 5 subcommand names against `query_graph.py`'s `commands` dict (§04 success criterion) before writing them into the rules file — fact-bound sync per `impl-hygiene.md §Fact-Bound Documentation Sync`.
- [x] Add the new "Plan/bug graph queries" bullet to §When to Query, using the proposed diff above as the authoritative shape.
- [x] Read `.claude/rules/intelligence.md` §How to Query (current `# Code symbol queries` block and the block that follows it) to confirm the insertion point.
- [x] Add the new "Plan/bug graph queries" example block after the `# Cross-repo semantic similarity` block, using the proposed diff above as the authoritative shape.
- [x] Add the use-by-workflow table (or inline equivalent) adjacent to the new examples heading.
- [x] Run `grep -c 'plan-status\|symbol-plans\|dag-ascii' .claude/rules/intelligence.md` — assert count ≥ 5 (one per subcommand, 3 examples each = ≥15 occurrences, but the grep -c counts lines, so ≥ 5 distinct lines containing any of the three terms).
- [x] `/commit-push` with `docs(rules): add plan/bug graph query docs to intelligence.md — §05.1`.

- [x] **Subsection close-out (05.1)** — MANDATORY before starting 05.2:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any temp files.

---

## 05.2 Update `.claude/skills/query-intel/compose-intel-summary.md` Step F consumer registry

**File:** `.claude/skills/query-intel/compose-intel-summary.md`

`compose-intel-summary.md` is the SSOT (`impl-hygiene.md §SSOT`) for all intel-query consumer extensions. Step F is the canonical registry of per-consumer queries beyond the base A-E protocol. This subsection adds a new "Plan/bug graph consumers" group to Step F, listing which skills use which plan-corpus subcommands — without exceeding the 2-3 bullet cap per consumer entry, and without forking the SSOT shape.

**SSOT invariant (must be preserved):** Step F is the single source of truth for consumer extensions. Drift between Step F and a consumer's own file is a `DRIFT:intel-extension-registry` finding. The new entries must reflect the ACTUAL queries the listed skills will invoke — not aspirational, not placeholder. Any skill that gains a plan-corpus subcommand call must register here in the same commit that adds the call to the skill file.

### Proposed Step F addition

The new group goes at the end of the "Planning/proposal consumers" block and before the "Analysis/maintenance consumers" block (preserving the logical grouping from the current layout):

**Proposed diff (not yet applied):**

```markdown
+**Plan/bug graph consumers:**
+
+- **`/continue-roadmap`** (Step 2.1 extension) — plan health check before section execution:
+  - `plan-status "<plan-name>"` (verify plan is in-progress, not blocked by external constraints)
+  - `bugs-for "<plan-name>"` (enumerate open blockers before committing to a section)
+
+- **`/review-plan`** (cross-plan scope overlap detection):
+  - `symbol-plans "<touched symbol>" --repo ori` (find other plans/bugs referencing the same symbols; flag their `reviewed: true` as potentially stale)
+
+- **`/fix-next-bug`** (Step 4.5 blast-radius preview extension):
+  - `symbol-plans "<bug repro symbol>"` (see which plans are impacted by this bug before choosing interactive vs. autopilot mode)
+
+- **`/review-bugs`** (Step 5.5 plan-context enrichment):
+  - `bugs-for "<plan-name>"` (show which plan sections the bug blocks, providing triage context)
```

**Cap enforcement:** Each entry above has exactly 1-2 bullets — within the 2-3 bullet cap per `compose-intel-summary.md §Registry contract`. No entry exceeds 2 bullets.

**Co-commit constraint:** The Step F entry for `/continue-roadmap` must be committed in the same PR/commit that extends `continue-roadmap/workflow.md` Step 2.1 to actually invoke `plan-status`/`bugs-for`. If that skill extension is deferred (to `plans/query-intel-adoption` or another plan), the Step F entries must also be deferred until the consumer actually calls the queries. Do NOT add a Step F entry for a query the consumer does not yet invoke — that is the definition of `DRIFT:intel-extension-registry`.

**Note on consumer skill updates:** This section's scope is documentation — it updates Step F to register the extensions, but it does NOT extend the actual skill files (`continue-roadmap/workflow.md`, `fix-next-bug/SKILL.md`, etc.) to invoke the new subcommands. That wiring belongs to the skill's own plan or to `plans/query-intel-adoption/section-08`. The Step F entries here should be written as "planned/documented extensions" with a note indicating the wiring is pending its own plan section, OR deferred until the wiring is live. The implementing agent must choose: (a) wire the skills NOW in §05 and write the Step F entry; (b) defer the Step F entry until the skill wiring lands. **Do not write a Step F entry for a query the skill does not yet invoke.**

### Tasks

- [x] Read `.claude/skills/query-intel/compose-intel-summary.md` Step F in full to identify current group boundaries and insertion point (after "Planning/proposal consumers", before "Analysis/maintenance consumers").
- [x] Determine for each listed consumer skill (`/continue-roadmap`, `/review-plan`, `/fix-next-bug`, `/review-bugs`) whether the skill currently invokes ANY plan-corpus subcommand. If yes, write the Step F entry + commit both the skill change and the Step F entry together. If no, defer the Step F entry to the plan/section that wires the skill.
- [x] Write the "Plan/bug graph consumers" group block using the proposed diff above as the authoritative shape — adjusting to reflect only the queries the skills ACTUALLY invoke at commit time.
- [x] Verify bullet cap: each consumer entry has ≤ 3 bullets (the cap is per `§Registry contract`).
- [x] Update the consumer count at the top of `compose-intel-summary.md` if any new `@`-include relationships are added (note: Step F registry entries do NOT require a new `@`-include — they document existing subcommands; count update only needed if a skill starts including the SSOT for the first time).
- [x] Run `grep -c 'plan-status\|symbol-plans\|dag-ascii\|bugs-for\|blocks' .claude/skills/query-intel/compose-intel-summary.md` — assert count ≥ 4.
- [x] `/commit-push` with `docs(skills): register plan/bug graph consumers in compose-intel-summary.md Step F — §05.2`.

- [x] **Subsection close-out (05.2)** — MANDATORY before starting 05.3:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any temp files.

---

## 05.3 Update `CLAUDE.md` §Commands / §Intelligence paragraph

**File:** `/home/eric/projects/ori_lang/CLAUDE.md`

`CLAUDE.md` is the primary onboarding document for any Claude session. The §Intelligence graph paragraph (near line 380) describes the graph as covering "191K+ symbols and 505K+ CALLS edges across Ori + 10 reference compilers". After §01-§04, the graph also covers the plan/bug corpus (`:Plan`, `:PlanSection`, `:Bug`, `:FixSection`, `:Subsection`, `:Overview`, `:BugTrackerSection` nodes with `BLOCKED_BY`, `DEPENDS_ON`, `HAS_SECTION`, `MENTIONS_CODE`, `RESOLVES_TO` edges). This must be reflected in CLAUDE.md so sessions know the graph's extended scope and can use the 5 new subcommands without consulting a separate file.

### §Intelligence graph paragraph update

The current paragraph at the end of the §Intelligence graph entry in §Commands reads approximately:

> **Intelligence graph**: `/query-intel status` (health) | `/query-intel --human symbols "<name>" --repo ori` | `callers`/`callees`/`file-symbols`/`similar` subcommands. The graph indexes 191K+ symbols and 505K+ CALLS edges across Ori + 10 reference compilers — ~100x faster than grep for blast-radius and cross-repo prior art. Degrades silently when `scripts/intel-query.sh status` is not ok. See `.claude/rules/intelligence.md` for the full workflow inventory, `.claude/skills/query-intel/SKILL.md` for the capability reference.

**Proposed diff (not yet applied):**

```markdown
-**Intelligence graph**: `/query-intel status` (health) | `/query-intel --human symbols "<name>" --repo ori` | `callers`/`callees`/`file-symbols`/`similar` subcommands. The graph indexes 191K+ symbols and 505K+ CALLS edges across Ori + 10 reference compilers — ~100x faster than grep for blast-radius and cross-repo prior art. Degrades silently when `scripts/intel-query.sh status` is not ok. See `.claude/rules/intelligence.md` for the full workflow inventory, `.claude/skills/query-intel/SKILL.md` for the capability reference.
+**Intelligence graph**: `/query-intel status` (health) | `/query-intel --human symbols "<name>" --repo ori` | `callers`/`callees`/`file-symbols`/`similar` subcommands. The graph indexes 191K+ symbols and 505K+ CALLS edges across Ori + 10 reference compilers — plus the full plan/bug corpus (plans, sections, subsections, bugs, fix-sections as typed DAG nodes) — ~100x faster than grep for blast-radius and cross-repo prior art. **Plan/bug graph subcommands:** `plan-status "<plan>"` (health: section + blocker + bug counts) | `blocks "<section-id>"` (transitive BLOCKED_BY chain to root blocker) | `bugs-for "<plan>"` (open bugs blocking any section, sorted by severity) | `symbol-plans "<symbol>"` (all plans/sections/bugs referencing a symbol via CodeReference bridge) | `dag-ascii "<plan>"` (ASCII hierarchy tree; `--format dot` for Graphviz). Degrades silently when `scripts/intel-query.sh status` is not ok. See `.claude/rules/intelligence.md` for the full workflow inventory, `.claude/skills/query-intel/SKILL.md` for the capability reference.
```

### Tasks

- [x] Read the actual `CLAUDE.md` §Intelligence graph entry to confirm its current text and line number (the 191K+ figure and the subcommand list).
- [x] Verify the plan/bug corpus counts (plan count, section count, bug count) from the deployed Neo4j after §03 sync completes — use `scripts/intel-query.sh cypher "MATCH (p:Plan) RETURN count(p)"` etc. to get real numbers; update the paragraph with concrete counts (not placeholders).
- [x] Update the §Intelligence graph paragraph using the proposed diff above, with real counts substituted.
- [x] Verify the 5 subcommand names in the update match `query_graph.py` commands dict (fact-bound sync).
- [x] Check whether CLAUDE.md §Commands has a separate "Intelligence graph" line (the short pipe-separated list); if so, add the 5 subcommand names there too (consistent with the existing format: `plan-status | blocks | bugs-for | symbol-plans | dag-ascii`).
- [x] Run `grep -c 'plan-status\|symbol-plans\|dag-ascii' CLAUDE.md` — assert count ≥ 3.
- [x] `/commit-push` with `docs(project): add plan/bug graph subcommands to CLAUDE.md §Intelligence graph — §05.3`.

- [x] **Subsection close-out (05.3)** — MANDATORY before starting 05.4:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any temp files.

---

## 05.4 Update `~/projects/lang_intelligence/CLAUDE.md` pipeline docs

**File:** `~/projects/lang_intelligence/CLAUDE.md`

The `lang_intelligence` repo's `CLAUDE.md` documents the existing "Code Graph Pipeline" (GitHub issue/PR fetch → `import_graph.py` → Neo4j). After §01-§03, a second pipeline exists: ori_lang plan corpus → `export_json.py` → `import_plan_bug_graph.py` → Neo4j. This must be documented alongside the code-graph pipeline so any `lang_intelligence` contributor (or a Claude session working in that repo) knows the second pipeline exists, how it is triggered, and how to debug it.

### Proposed "Plan/Bug Graph Pipeline" section

The new section goes immediately after the existing "Architecture" block and before the "Running" block (or after the full "Running" block if that fits better with the file's current structure — the implementing agent must read the file and choose the correct insertion point). The draft below is authoritative in content; the implementing agent adjusts the heading level to match the file's existing structure.

**Proposed new section (complete draft):**

```markdown
## Plan/Bug Graph Pipeline

Alongside the code-graph (GitHub issues + PRs), a second pipeline ingests the ori_lang plan/bug corpus into Neo4j as a typed DAG. This enables cross-plan queries, plan-to-symbol joins, and dependency-chain analysis via the same `intel-query.sh` interface.

### Schema surface

New node labels (all with uniqueness constraints on their `id` property):

| Label | `id` convention | Key properties |
|-------|----------------|----------------|
| `:Plan` | Directory name (e.g. `plan-bug-dag-ingestion`) | `title`, `status`, `file_path` |
| `:PlanSection` | `plans/<dir>/section-<NN>-<slug>.md` | `section`, `title`, `status`, `reviewed` |
| `:Subsection` | `<section_id>/<subsection_id>` (e.g. `plans/.../section-05.md/05.1`) | `subsection_id`, `title`, `status` |
| `:Overview` | `plans/<dir>/00-overview.md` | `plan_status`, `file_path` |
| `:BugTrackerSection` | `plans/bug-tracker/section-NN-<slug>.md` | `section`, `title` |
| `:Bug` | `BUG-NN-NNN` | `severity`, `title`, `status`, `subsystem`, `found` |
| `:FixSection` | `fix-BUG-NN-NNN` | `bug_id`, `title`, `status`, `severity` |

New relationship types:

| Relationship | Meaning |
|-------------|---------|
| `(:Plan)-[:HAS_SECTION]->(:PlanSection)` | Plan contains section |
| `(:Plan)-[:HAS_OVERVIEW]->(:Overview)` | Plan's overview file |
| `(:PlanSection)-[:HAS_SUBSECTION]->(:Subsection)` | Section contains subsection |
| `(:PlanSection)-[:DEPENDS_ON]->(:PlanSection)` | Section dependency (from frontmatter `depends_on:`) |
| `(:PlanSection)-[:BLOCKED_BY]->(:Bug\|:PlanSection)` | Blocking relationship |
| `(:PlanSection)-[:SUPERSEDES]->(:Plan\|:PlanSection)` | Supersession (from frontmatter `supersedes:`) |
| `(:BugTrackerSection)-[:HAS_BUG]->(:Bug)` | Bug-tracker section contains bug |
| `(:Bug)-[:FIXED_BY]->(:FixSection)` | Bug is resolved by fix section |
| `(:FixSection)-[:RESOLVES]->(:Bug)` | Fix section resolves bug |
| `(any)-[:MENTIONS_CODE]->(:CodeReference)-[:RESOLVES_TO]->(:Symbol\|:File)` | Plan/bug references code symbol (reuses existing bridge) |

### Ingestion path

```
ori_lang/plans/**/*.md
  └── python -m scripts.plan_corpus export          # runs in ori_lang/
        │  (discovery → schemas → dag → export_json)
        │  stdout: {"nodes": [...], "relationships": [...]}
        ▼
~/projects/lang_intelligence/scripts/sync-plan-bug-graph.sh
  └── neo4j/sync_plan_bug_graph.py
        └── neo4j/import_plan_bug_graph.py
              │  Phase 1: MERGE nodes (plan → section → subsection → bug → fix-section)
              │  Phase 2: MERGE edges (DEPENDS_ON, BLOCKED_BY, MENTIONS_CODE, ...)
              │  Stale cleanup: DETACH DELETE nodes no longer in corpus
              ▼
        Neo4j (same instance as code graph)
```

### Sync cadence

- **Automatic (post-commit):** `ori_lang/lefthook.yml` `post-commit` hook entry `intel-plan-sync` triggers on any change to `plans/**`. The hook invokes `sync-plan-bug-graph.sh` in the background — hook returns immediately (<100ms), sync completes within 10s.
- **Manual full rebuild:** `~/projects/lang_intelligence/scripts/sync-plan-bug-graph.sh --full`
- **Log:** `~/projects/lang_intelligence/logs/plan-bug-sync.log` (10k-line rotation, same as code-graph log)

### Query subcommands

Five subcommands in `neo4j/query_graph.py` serve plan/bug graph queries via `scripts/intel-query.sh`:

| Subcommand | Function | Description |
|---|---|---|
| `plan-status <plan>` | `cmd_plan_status` | Aggregate health: section counts, blocker counts, bug counts |
| `blocks <node-id>` | `cmd_blocks` | Transitive BLOCKED_BY + DEPENDS_ON chain to root blocker(s); prints indented tree in human mode |
| `bugs-for <plan>` | `cmd_bugs_for` | Open bugs blocking any section of the plan, sorted critical→high→medium→low |
| `symbol-plans <symbol>` | `cmd_symbol_plans` | All plans/sections/bugs referencing the symbol via MENTIONS_CODE bridge |
| `dag-ascii <plan>` | `cmd_dag_ascii` | ASCII plan hierarchy + blocker edges; `--format dot` emits Graphviz DOT |

All five support `--json` (raw dict) and `--human` (formatted text) modes. `intel-query.sh` passes subcommand names through unchanged — no wrapper changes required.

### Graceful degradation

`sync-plan-bug-graph.sh` exits 0 on any failure path (no Neo4j connection, missing venv, parse error, import error). Sync failures are logged to `logs/plan-bug-sync.log` but never propagate to `git commit` — the commit completes regardless of graph state. `intel-query.sh status` degrades silently; all downstream workflows (`./test-all.sh`, `/tpr-review`, `/continue-roadmap`) continue working when the graph is unavailable.

### Testing

- `~/projects/lang_intelligence/tests/test_import_plan_bug_graph.py` — unit tests for the importer using an in-memory MagicMock Neo4j driver: node MERGE correctness, edge MERGE correctness, stale node DETACH DELETE, two-phase ordering.
- `~/projects/lang_intelligence/tests/test_query_plan_bug.py` — unit tests for all 5 query handlers using fixture graph state: each subcommand × (json_mode=True, json_mode=False) × (populated, empty, edge cases).
```

### Tasks

- [x] Read `~/projects/lang_intelligence/CLAUDE.md` in full to identify: (1) current section structure and heading levels; (2) where "Architecture" block ends and "Running" block begins; (3) whether any plan-pipeline content already exists (it should not, unless a prior session added it prematurely — if so, diff against the proposed draft and reconcile).
- [x] Identify the correct insertion point for "Plan/Bug Graph Pipeline" section (after "Architecture" block or after "Running" block based on file structure).
- [x] Verify the schema table node labels against `schema.cypher` (from §02 deliverable) — fact-bound: every label name and relationship type in the draft must match the deployed schema.
- [x] Verify subcommand names against `query_graph.py` commands dict (from §04 deliverable).
- [x] Verify ingestion path description against actual script names: `sync-plan-bug-graph.sh`, `sync_plan_bug_graph.py`, `import_plan_bug_graph.py` (from §02 and §03 deliverables).
- [x] Write the new "Plan/Bug Graph Pipeline" section using the proposed draft above, with adjustments for correct heading level and insertion point.
- [x] Run `grep -c 'plan-status\|symbol-plans\|dag-ascii' ~/projects/lang_intelligence/CLAUDE.md` — assert count ≥ 3.
- [x] `/commit-push` with `docs(project): add Plan/Bug Graph Pipeline section to lang_intelligence/CLAUDE.md — §05.4`.

- [x] **Subsection close-out (05.4)** — MANDATORY before starting 05.R:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any temp files.

---

## 05.N Completion Checklist

- [x] `.claude/rules/intelligence.md` §When to Query has the new "Plan/bug graph queries" bullet listing all 5 subcommands and their primary consumer skills.
- [x] `.claude/rules/intelligence.md` §How to Query has a new "Plan/bug graph queries (Ori plan corpus)" block with ≥3 example invocations per subcommand.
- [x] `.claude/skills/query-intel/compose-intel-summary.md` Step F has a "Plan/bug graph consumers" group with entries for `/continue-roadmap`, `/review-plan`, `/fix-next-bug`, and `/review-bugs`; each entry has ≤3 bullets (cap enforced).
- [x] `CLAUDE.md` §Intelligence graph paragraph mentions all 5 new subcommand names with one-line descriptions.
- [x] `~/projects/lang_intelligence/CLAUDE.md` has a "Plan/Bug Graph Pipeline" section documenting schema labels, relationships, ingestion path, sync cadence, query subcommands, graceful degradation, and test files.
- [x] `grep -l 'plan-status\|symbol-plans\|dag-ascii' .claude/rules/intelligence.md CLAUDE.md ~/projects/lang_intelligence/CLAUDE.md .claude/skills/query-intel/compose-intel-summary.md` returns all 4 paths.
- [x] `grep -c 'plan-status\|symbol-plans\|dag-ascii' .claude/rules/intelligence.md` ≥ 5 (lines containing any of the three terms).
- [x] `grep -c 'plan-status\|symbol-plans\|dag-ascii' CLAUDE.md` ≥ 3.
- [x] `grep -c 'plan-status\|symbol-plans\|dag-ascii' ~/projects/lang_intelligence/CLAUDE.md` ≥ 3.
- [x] `grep -c 'plan-status\|symbol-plans\|dag-ascii\|bugs-for\|blocks' .claude/skills/query-intel/compose-intel-summary.md` ≥ 4.
- [x] All Step F entries in `compose-intel-summary.md` reflect ACTUAL queries the skills invoke (no `DRIFT:intel-extension-registry` — verified by reading each listed skill's workflow against Step F).
- [x] All facts in the four updated files are fact-bound: subcommand names verified against `query_graph.py` commands dict; schema labels verified against `schema.cypher`; script names verified against the scripts in `~/projects/lang_intelligence/`; skill names verified against `.claude/skills/` directory.
- [x] **`/sync-claude` section-close doc sync** — run `/sync-claude` across all commits in §05 (use `git diff --name-only <section-start>..HEAD` to identify all changed files). Verify: (1) CLAUDE.md §Commands, §Intelligence graph, §Key Paths current; (2) `canon.md` unaffected (no pipeline phase changes); (3) `ori-syntax.md` unaffected (no prelude/keyword/operator changes). All four target files ARE the doc surfaces — the sync verifies they are internally consistent and that no other doc surfaces reference the new subcommands without being updated. Document: "Claude artifact sync §05: doc-only changes; four target files updated; no compiler phase changes; `canon.md` and `ori-syntax.md` unaffected."
- [x] **Plan sync** — update plan metadata to reflect this section's completion:
  - [x] This section's frontmatter `status` → `complete`, all subsection statuses → `complete`
  - [x] `00-overview.md` Quick Reference table status updated for §05
  - [x] `00-overview.md` mission success criteria: check off the criterion "`.claude/rules/intelligence.md` 'When to Query' and 'How to Query' sections list the new subcommands..." (mission criterion line 30 in `00-overview.md`)
  - [x] `index.md` §05 status updated
  - [x] Cross-links verified: §06 `depends_on: ["05"]` assumption holds — §05 must be complete before §06's verification run.
- [x] `diagnostics/repo-hygiene.sh --check` clean — no temp/scratch files in working tree.

**Exit Criteria:** `grep -l 'plan-status\|symbol-plans\|dag-ascii' .claude/rules/intelligence.md CLAUDE.md ~/projects/lang_intelligence/CLAUDE.md .claude/skills/query-intel/compose-intel-summary.md` returns exactly 4 paths (all four files contain the new subcommand terms). `./test-all.sh` green (zero regressions — §05 is doc-only; any test failure indicates an unrelated regression that must be investigated and fixed before closing §05).
