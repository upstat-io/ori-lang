---
plan: "plan-bug-dag-ingestion"
title: "Plan & Bug DAG Ingestion: Exhaustive Implementation Plan"
status: in-progress
references:
  - "plans/query-intel-adoption/00-overview.md"
  - "plans/completed/lang-intelligence/"
  - ".claude/rules/intelligence.md"
  - ".claude/skills/query-intel/compose-intel-summary.md"
  - "scripts/plan_corpus/dag.py"
---

# Plan & Bug DAG Ingestion: Exhaustive Implementation Plan

## Mission

Project the ori_lang plan/bug corpus (plans, sections, subsections, bug-tracker entries, fix-BUG files) into the existing `lang_intelligence` Neo4j graph as a typed DAG, joined to the existing 191K+ code-symbol graph via the already-established `CodeReference` bridge, and keep it synchronized with the same commit-triggered, fire-and-forget cadence used by the code graph today — so that any session can answer "what plans touch symbol X", "full blocked-by chain to ship section Y", "what bugs block plan Z", and "impact radius of fix-BUG-XX-NNN" with a single `scripts/intel-query.sh` subcommand or one Cypher query.

## Mission Success Criteria

The mission is complete when ALL of the following are true:

- [x] Neo4j `schema.cypher` declares typed node labels `:Plan`, `:PlanSection`, `:Subsection`, `:Bug`, `:FixSection`, `:BugTrackerSection`, `:Overview` with uniqueness constraints on stable IDs (plan directory name, `BUG-XX-NNN`, `fix-BUG-XX-NNN`, `plans/<dir>/section-<NN>-<slug>.md`) — verified by `cypher-shell < schema.cypher` idempotent re-run producing zero schema changes on second invocation. (§02)
- [x] `scripts/plan_corpus/schemas.py` exposes optional `touches: list[str] | None = None` on `PlanSectionSchema` and `FixBugSchema`; `scripts/plan_corpus/types.py` exposes `SourceKind.EXPLICIT_SUPERSEDES` and `SourceKind.EXPLICIT_REFERENCES`; `scripts/plan_corpus/dag.py` promotes frontmatter `supersedes:` entries to typed edges and `references:` entries to typed references — verified by `python -m scripts.plan_corpus docgen --check` returning exit 0 after regenerating `docs/internal/plan-schema-reference.md`. (§01)
- [x] `scripts/plan_corpus/export_json.py` serializes `Corpus + Dag` to a Neo4j-flavored JSON envelope `{"nodes": [...], "relationships": [...]}` with stable IDs and full provenance (`source_kind`, `source_line`, `raw_text`); `python -m scripts.plan_corpus export` emits the same envelope on stdout — verified by a fixture-corpus round-trip test in `tests/plan-audit/test_export_json.py`. (§01)
- [x] `~/projects/lang_intelligence/neo4j/import_plan_bug_graph.py` consumes the JSON envelope and, in two phases (nodes → relationships), MERGEs nodes and edges into Neo4j; stale nodes are detected via the `all_incoming_ids - existing_db_ids` diff and removed via `DETACH DELETE` mirroring `import_code_graph.py:536-557` — verified by a Python unit test in `~/projects/lang_intelligence/tests/test_import_plan_bug_graph.py` that uses an in-memory mock driver. (§02)
- [x] Plan/bug nodes are joined to code symbols via the existing bridge: `(node:Plan|PlanSection|Bug|FixSection)-[:MENTIONS_CODE]->(:CodeReference)-[:RESOLVES_TO]->(:Symbol|:File)`. Declarative `touches:` entries produce direct `(:MENTIONS_CODE)` edges with `mention_kind: "declared"`; scraped backtick mentions produce `(:MENTIONS_CODE)` edges with `mention_kind: "inferred"`; ambiguous resolutions produce `:UnresolvedSymbol` stubs — all via reuse of `resolve_code_refs.py`'s pipeline. Verified by: `scripts/intel-query.sh cypher "MATCH (p:PlanSection)-[:MENTIONS_CODE]->(cr)-[:RESOLVES_TO]->(s:Symbol) RETURN count(DISTINCT s) > 0"` returns true after sync. (§02)
- [x] `~/projects/lang_intelligence/scripts/sync-plan-bug-graph.sh` and `~/projects/lang_intelligence/neo4j/sync_plan_bug_graph.py` implement commit-triggered full corpus rebuild with `flock`-based lock discipline, 10k-line log rotation, env-var driven Neo4j connection, and fire-and-forget exit semantics; `ori_lang/lefthook.yml` `post-commit` gains a second entry `intel-plan-sync` scoped to `plans/**` that invokes the wrapper in the background — verified by: (1) `touch plans/test.md && git add plans/test.md && git commit -m "test"` completes in under 100ms (hook returns immediately), (2) `~/projects/lang_intelligence/logs/plan-bug-sync.log` shows the sync completing within 10s, (3) the test file appears as a parse error in the next corpus scan. (§03)
- [ ] `~/projects/lang_intelligence/neo4j/query_graph.py` exposes five new subcommand handlers — `plan-status`, `blocks`, `bugs-for`, `symbol-plans`, `dag-ascii` — each supporting `--json` and `--human` modes with consistent output envelope matching existing handlers; `ori_lang/scripts/intel-query.sh` routes these verbatim — verified by unit tests in `~/projects/lang_intelligence/tests/test_query_plan_bug.py` using fixture graph state. (§04)
- [ ] `.claude/rules/intelligence.md` "When to Query" and "How to Query" sections list the new subcommands and their use cases; `.claude/skills/query-intel/compose-intel-summary.md` Step F registry lists the new queries under plan-family consumers; `CLAUDE.md` §Commands includes the new subcommand syntax; `~/projects/lang_intelligence/CLAUDE.md` documents the new pipeline alongside the existing code-graph pipeline — verified by `grep -l "plan-status\|symbol-plans\|dag-ascii" .claude/rules/intelligence.md CLAUDE.md ../lang_intelligence/CLAUDE.md` returning all three paths. (§05)
- [ ] Full-corpus rebuild (`sync-plan-bug-graph.sh --full`) produces the exact same graph state as an incremental rebuild on a fully-clean start, verified by: `diff <(cypher-shell -f dump-plan-bug-graph.cypher | sort) <(rebuild; cypher-shell -f dump-plan-bug-graph.cypher | sort)` returns empty. (§06)
- [ ] Graceful degradation preserved: when `docker stop lang-intelligence` is active, every downstream workflow continues working — `./test-all.sh` passes, `/continue-roadmap` proceeds. Verified by a CI-runnable script `diagnostics/verify-plan-bug-degraded.sh` that kills the container, runs the checks, and restarts it. (§06)
- [ ] `./test-all.sh` green — no regressions in Rust test suites from any side-effect of the plan's schema/dag changes.
- [ ] `python -m scripts.plan_corpus check plans/plan-bug-dag-ingestion/` returns exit 0 with zero recon-related findings on the complete sections.
- [ ] All section success criteria met (see each `section-NN-*.md` frontmatter `success_criteria`).

## Architecture

```
┌─────────────────────────────── ori_lang ─────────────────────────────────┐
│                                                                          │
│  plans/**/*.md ─── filesystem SSOT (YAML frontmatter + body markdown)    │
│     │                                                                    │
│     ▼                                                                    │
│  scripts/plan_corpus/  ─── discovery.discover_corpus()                   │
│     │                 ─── schemas.py + schema.py (validate frontmatter)  │
│     │                 ─── dag.py (NodeKind/Edge/Reference + classifiers) │
│     │                 ─── export_json.py  ← NEW in §01                   │
│     │                                                                    │
│     ▼                                                                    │
│  python -m scripts.plan_corpus export ─── stdout JSON envelope           │
│                                                                          │
└──────────────────────────────┬───────────────────────────────────────────┘
                               │ (piped across sibling-repo boundary)
                               ▼
┌──────────────────── lang_intelligence (sibling repo) ────────────────────┐
│                                                                          │
│  scripts/sync-plan-bug-graph.sh   ← NEW in §03                           │
│     │  (flock, log rotation, venv, fire-and-forget)                      │
│     │                                                                    │
│     ▼                                                                    │
│  neo4j/sync_plan_bug_graph.py     ← NEW in §02                           │
│     │  (reads JSON, MERGEs two phases, DETACH DELETE stale)              │
│     │                                                                    │
│     ▼                                                                    │
│  neo4j/import_plan_bug_graph.py   ← NEW in §02                           │
│     │  (node upsert + edge MERGE, reuses _resolve_target_py pattern)     │
│     │                                                                    │
│     ▼                                                                    │
│  Neo4j  ───────────────────────────────────────────────────────────────  │
│                                                                          │
│    :Plan ─[:HAS_SECTION]→ :PlanSection ─[:HAS_SUBSECTION]→ :Subsection   │
│    :Plan ─[:HAS_OVERVIEW]→ :Overview                                     │
│    :PlanSection ─[:DEPENDS_ON|BLOCKED_BY|SUPERSEDES|RESOLVES|...]→ ...   │
│    :BugTrackerSection ─[:HAS_BUG]→ :Bug                                  │
│    :Bug ─[:FIXED_BY]→ :FixSection ─[:RESOLVES]→ :Bug                     │
│                                                                          │
│    (any node) ─[:MENTIONS_CODE]→ :CodeReference ─[:RESOLVES_TO]→ :Symbol │
│                                                             └──→ :File   │
│                                                                          │
│    Code-symbol graph ───── ALREADY POPULATED (191K symbols, 505K CALLS)  │
│                                                                          │
└──────────────────────────────┬───────────────────────────────────────────┘
                               │
                               ▼
┌────────────────────── Query path (unchanged contract) ───────────────────┐
│                                                                          │
│  ori_lang/scripts/intel-query.sh                                         │
│     │  ─── status check → always exits 0                                 │
│     │  ─── dispatches to lang_intelligence/neo4j/query_graph.py          │
│     │                                                                    │
│     ▼                                                                    │
│  query_graph.py commands dict ← GAINS in §04:                            │
│     plan-status | blocks | bugs-for | symbol-plans | dag-ascii           │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘

Commit trigger:
  ori_lang/lefthook.yml post-commit:
    ├─ intel-sync       (existing — scoped to compiler/*.rs, library/*.{ori,rs})
    └─ intel-plan-sync  ← NEW in §03 (scoped to plans/**)
```

## Design Principles

**(1) dag.py is the DAG SSOT; the Neo4j projection is derived.**
`scripts/plan_corpus/dag.py` already builds the authoritative in-memory DAG (2160 lines, 8 classifiers, node types matching every `FileClass`). The Neo4j schema is a *projection* of that SSOT, not a parallel truth. `export_json.py` (§01) is a thin serialization adapter over `Dag.to_json()` + extensions; `import_plan_bug_graph.py` (§02) consumes exactly that envelope. Any new edge semantics must be modeled in `dag.py` first — forking a parallel DAG in the importer would be a `LEAK:scattered-knowledge` violation per `impl-hygiene.md §SSOT`. This is why §01 explicitly extends `dag.py` to promote `supersedes`/`references` frontmatter to first-class edges/references instead of hacking it in the exporter.

**(2) The existing `CodeReference` bridge is reused end-to-end for plan→code joins.**
The bridge layer (`:MENTIONS_CODE → :CodeReference → :RESOLVES_TO`) already exists (`schema.cypher:188-213`), is battle-tested by the issue-graph pipeline (`extract_code_refs.py`, `resolve_code_refs.py`), and handles resolution, staleness, ambiguity, and `--re-resolve` sweeps. Reusing it for plan→symbol joins gives us those features for free and avoids maintaining a parallel TOUCHES→Symbol resolver. Declarative `touches:` frontmatter produces `CodeReference` nodes with `mention_kind: "declared"`; scraped backtick mentions produce `mention_kind: "inferred"`. One join path; one staleness story.

**(3) Full rebuild on any `plans/**` change, not fine-grained incremental.**
The corpus is small (~30-40 plans, ~200-500 sections, ~100+ bugs). `dag.py`'s classifiers (cycle detection, transitive closure, subsystem clustering) are fundamentally whole-corpus operations — a fine-grained incremental that only touched "the changed file" would produce incorrect classifier output because classifier outputs depend on relationships between nodes that weren't just edited. Full rebuild produces correct classifier state every time and is simpler to reason about. This contrasts with the code-graph pipeline (`sync_ori_graph.py`) which CAN do true incremental because code-symbol relationships are locally computable.

## Section Dependency Graph

```
  §01  ────────────────────────────────────────────────┐
  │   (extend plan_corpus: schemas, types, dag,        │
  │    export_json; regenerate plan-schema-reference)   │
  ▼                                                     ▼
  §02  ─── (Neo4j schema + importer + bridge reuse) ─── consumers of JSON envelope
  │                                                     │
  ▼                                                     │
  §03  ─── (sync wrapper + lefthook wiring) ────────────┤
  │                                                     │
  ▼                                                     │
  §04  ─── (query subcommands) ─────────────────────────┤
  │                                                     │
  ▼                                                     │
  §05  ─── (doc sync + consumer wiring) ────────────────┤
  │                                                     │
  ▼                                                     │
  §06  ─── (verification + cross-plan invalidation) ────┘
```

- **§01 is strictly first**: §02 imports the JSON envelope whose shape §01 defines; §03 invokes §02's importer; §04 queries the schema §02 established; §05 documents the subcommands §04 delivers; §06 verifies all of the above.
- **No parallelizable pair**: every section strictly depends on the one before it. This is a sequential build order — §02 cannot start until §01's `touches:` field is landed, §03 cannot start until §02's importer exists, etc.

**Cross-section interactions (must be co-implemented):**

- **§01 + §02**: The JSON envelope is a contract between them. The envelope is pinned by §01's `test_export_json.py` fixture snapshot; §02's importer must consume exactly that shape. If either drifts without the other, round-trip breaks. Both sections' completion checklists explicitly reference the other's fixture to prevent drift.

- **§02 + §03**: The sync wrapper orchestrates the importer. The importer must accept `--full`, `--incremental` (stub, always forwards to full in Phase 1), and `--health` modes matching `sync-ori-graph.sh`'s flag surface so the wrapper contract is uniform.

## Implementation Sequence

```
Phase 0 — Prerequisites  (no behavioral change)
  └─ None — this plan is entirely additive to existing infrastructure.
     Code-graph sync, intel-query.sh, plan_corpus all exist and keep working.

Phase 1 — Foundation (§01)
  └─ §01.1: Add touches: field to PlanSectionSchema + FixBugSchema
  └─ §01.2: Add EXPLICIT_SUPERSEDES, EXPLICIT_REFERENCES to SourceKind
  └─ §01.3: Extend dag.py — relax Edge guard, add supersedes_sources loop,
            update classify_redundant_dependency filter, update severity map
  └─ §01.4: Write export_json.py + export subcommand in __main__.py
  └─ §01.5: Regenerate plan-schema-reference.md via docgen
  └─ §01.6: Fixture-corpus round-trip test
  Gate: python -m scripts.plan_corpus docgen --check returns 0;
        python -m scripts.plan_corpus export plans/plan-bug-dag-ingestion/
        produces valid JSON envelope.

Phase 2 — Neo4j projection (§02)
  └─ §02.1: Extend schema.cypher with plan/bug/fix-section labels + edges
  └─ §02.2: Write import_plan_bug_graph.py (two-phase MERGE + DETACH DELETE)
  └─ §02.3: Wire CodeReference bridge for touches + backtick scraping
  └─ §02.4: Unit tests with in-memory Neo4j mock
  Gate: end-to-end fixture corpus JSON → import → Cypher query returns
        expected node/edge counts.

Phase 3 — Sync wiring (§03)  [CRITICAL PATH]
  └─ §03.1: Write sync_plan_bug_graph.py (driver loop)
  └─ §03.2: Write sync-plan-bug-graph.sh wrapper (flock, log rotation, venv)
  └─ §03.3: Extend ori_lang/lefthook.yml post-commit with intel-plan-sync entry
  └─ §03.4: Full-rebuild-on-any-change semantics
  Gate: touch plans/test.md && git commit -m "test" — hook returns in <100ms;
        log shows sync completing in <10s.

Phase 4 — Query surface (§04)
  └─ §04.1: cmd_plan_status handler
  └─ §04.2: cmd_blocks handler (transitive BLOCKED_BY closure)
  └─ §04.3: cmd_bugs_for handler
  └─ §04.4: cmd_symbol_plans handler (joins via MENTIONS_CODE → Symbol)
  └─ §04.5: cmd_dag_ascii handler (Graphviz or ASCII tree render)
  └─ §04.6: intel-query.sh dispatch extension
  └─ §04.7: Unit tests for each subcommand
  Gate: each of 5 subcommands green in --json and --human modes on fixture.

Phase 5 — Doc sync (§05)
  └─ §05.1: Update .claude/rules/intelligence.md
  └─ §05.2: Update .claude/skills/query-intel/compose-intel-summary.md Step F
  └─ §05.3: Update CLAUDE.md §Commands
  └─ §05.4: Update ~/projects/lang_intelligence/CLAUDE.md
  Gate: /sync-claude clean; all three files mention the new subcommands.

Phase 6 — Verification (§06)
  └─ §06.1: Full vs incremental equivalence test
  └─ §06.2: Graceful degradation script
  └─ §06.3: Cross-plan review invalidation run
  Gate: tests pass; plan-invalidate produces empty stale list or triaged overlaps.
```

**Why this order:**
- Phase 0–1 are pure additions to `scripts/plan_corpus/` — no behavioral change to existing consumers (check/discover/docgen still work).
- Phase 2 must precede Phase 3 because the sync wrapper calls the importer.
- Phase 3 is the critical path — it makes the whole system useful (ingestion runs automatically).
- Phase 4 can precede Phase 3 in principle (queries work whenever data is in the graph), but is ordered after because without Phase 3 sync, the graph goes stale between manual imports.
- Phase 5 precedes Phase 6 because verification benefits from up-to-date docs.

**Known failing tests (expected until plan completion):**

None — this plan is strictly additive to existing infrastructure. No existing test should fail at any point during implementation. If a test fails, it indicates a regression introduced by this plan's changes, not a shared-infrastructure dependency.

## Metrics (Current State)

Baseline measurements before implementation begins.

| Surface | Current State | Where |
|---|---|---|
| `scripts/plan_corpus/` | 7 modules, ~4,550 LOC | `scripts/plan_corpus/` |
| `scripts/plan_corpus/dag.py` | ~2,160 LOC, 8 classifiers | `scripts/plan_corpus/dag.py` |
| Plan corpus size | ~30 active plans + bug-tracker + completed | `plans/` |
| lang_intelligence Neo4j pipeline | ~2,300 LOC Python | `~/projects/lang_intelligence/neo4j/` |
| Existing Neo4j nodes | 7 issue labels + 5 code labels + 5 bridge labels | `schema.cypher` |
| Existing intel-query.sh subcommands | 20 (search, compare, fixed, hot, …, similar, status, cypher) | `query_graph.py:1200-1219` |
| Commit-triggered sync | Scoped to compiler+library, not plans | `lefthook.yml` post-commit |
| Post-commit sync latency | <100ms hook return, <10s background completion | `sync-ori-graph.sh` |

## Estimated Effort

| Section | Est. Lines | Complexity | Depends On |
|---------|-----------|------------|------------|
| 01 plan_corpus schema + dag + exporter | ~400 new / ~50 modified | Medium — touches `dag.py` invariant | — |
| ↳ 01.1 `touches:` field | ~10 | Low | — |
| ↳ 01.2 SourceKind variants | ~10 | Low | — |
| ↳ 01.3 dag.py extension | ~80 modified | Medium — `Edge.__post_init__` guard | 01.1, 01.2 |
| ↳ 01.4 `export_json.py` | ~200 new | Medium | 01.3 |
| ↳ 01.5 docgen regen | auto-generated | Low | 01.1 |
| ↳ 01.6 round-trip test | ~100 new | Low | 01.4 |
| 02 Neo4j schema + importer | ~350 new | Medium — MERGE + DETACH DELETE correctness | 01 |
| ↳ 02.1 schema.cypher extension | ~50 new | Low | — |
| ↳ 02.2 `import_plan_bug_graph.py` | ~200 new | Medium | 02.1 |
| ↳ 02.3 CodeReference bridge reuse | ~50 new | Medium — resolver adaptation | 02.2 |
| ↳ 02.4 importer unit tests | ~50 new | Low | 02.2 |
| 03 sync wrapper + lefthook | ~250 new | Low — mirrors existing sync-ori-graph.sh | 02 |
| ↳ 03.1 `sync_plan_bug_graph.py` | ~80 new | Low | — |
| ↳ 03.2 `sync-plan-bug-graph.sh` | ~100 new | Low | 03.1 |
| ↳ 03.3 lefthook entry | ~10 new | Low | 03.2 |
| ↳ 03.4 smoke test | ~60 new | Low | 03.3 |
| 04 query subcommands | ~400 new | Medium | 02 |
| ↳ 04.1–04.5 query handlers | ~300 new | Medium — each Cypher query | 02 |
| ↳ 04.6 intel-query.sh dispatch | ~30 modified | Low | 04.1–04.5 |
| ↳ 04.7 handler unit tests | ~70 new | Low | 04.1–04.5 |
| 05 doc sync | ~80 modified across 4 files | Low | 04 |
| 06 verification | ~200 new | Medium | 01–05 |
| **Total new LOC** | **~1,680** | | |
| **Total modified LOC** | **~130** | | |

## Known Bugs (Pre-existing)

None discovered during research passes. The `scripts/plan_corpus/` module is mature; `lang_intelligence` pipeline is mature. Reviewer consensus found no blockers — only scope-expanding discoveries (missing `OverviewSchema.supersedes` edge promotion) which this plan absorbs.

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | plan_corpus schema + dag + exporter | `section-01-plan-corpus-extension.md` | Complete |
| 02 | Neo4j schema + importer + CodeReference bridge | `section-02-neo4j-schema-importer.md` | Complete |
| 03 | Commit-triggered sync wiring | `section-03-sync-wiring.md` | Complete |
| 04 | Plumbing query subcommands | `section-04-query-subcommands.md` | Not Started |
| 05 | Doc sync + consumer wiring | `section-05-doc-sync.md` | Not Started |
| 06 | Verification + cross-plan invalidation | `section-06-verification.md` | Not Started |
