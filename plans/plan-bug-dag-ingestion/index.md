---
name: "plan-bug-dag-ingestion"
full_name: "Plan & Bug DAG Ingestion"
status: active
---

# Plan & Bug DAG Ingestion Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: plan_corpus schema + dag + exporter
**File:** `section-01-plan-corpus-extension.md` | **Status:** Complete

```
plan_corpus, schemas.py, types.py, dag.py, export_json.py, docgen
PlanSectionSchema, FixBugSchema, touches field
SourceKind, EXPLICIT_SUPERSEDES, EXPLICIT_REFERENCES
Edge, __post_init__, _EDGE_KINDS, build_dag, deps_sources
classify_redundant_dependency, apply_source_kind_severity
Dag.to_json, Corpus, NodeKind, Reference
plan-schema-reference.md, docgen --check drift gate
tests/plan-audit/test_export_json.py, fixture corpus round-trip
python -m scripts.plan_corpus export
```

---

### Section 02: Neo4j schema + importer + CodeReference bridge
**File:** `section-02-neo4j-schema-importer.md` | **Status:** Complete

```
Neo4j, schema.cypher, labels, uniqueness constraints, indexes
:Plan, :PlanSection, :Subsection, :Bug, :FixSection, :BugTrackerSection, :Overview
:HAS_SECTION, :HAS_SUBSECTION, :HAS_BUG, :HAS_OVERVIEW, :HAS_FIX_SECTION
:DEPENDS_ON, :BLOCKED_BY, :SUPERSEDES, :RESOLVES, :UNBLOCKS, :REWRITES, :REFERENCES
:MENTIONS_CODE, :CodeReference, :RESOLVES_TO, :Symbol, :UnresolvedSymbol
import_plan_bug_graph.py, two-phase MERGE, DETACH DELETE stale
_resolve_target_py, resolve_code_refs.py, mention_kind declared/inferred
signature_hash, qualified_name, stable IDs, plan directory name, BUG-XX-NNN
UNWIND batching, RELATIONSHIP_BATCH_SIZE, UnresolvedSymbol orphan cleanup
tests/test_import_plan_bug_graph.py, in-memory mock driver
```

---

### Section 03: Commit-triggered sync wiring
**File:** `section-03-sync-wiring.md` | **Status:** Complete

```
sync_plan_bug_graph.py, sync-plan-bug-graph.sh, lefthook.yml
post-commit, intel-plan-sync, fire-and-forget, plans/** glob
flock, .plan-bug-sync.lock, concurrency, lock discipline
log rotation, tail -n 10000, logs/plan-bug-sync.log
venv activation, $PROJECT_DIR/.venv/bin/python
full rebuild, --full mode, --incremental stub, --health mode
graceful degradation, exit 0 always, env-var connection
NEO4J_URI, NEO4J_USER, NEO4J_PASS, ORI_INTEL_DIR, ORI_LANG_ROOT
commit latency, <100ms hook return, <10s background sync
```

---

### Section 04: Plumbing query subcommands
**File:** `section-04-query-subcommands.md` | **Status:** Complete

```
intel-query.sh, query_graph.py, commands dict
plan-status, blocks, bugs-for, symbol-plans, dag-ascii
cmd_plan_status, cmd_blocks, cmd_bugs_for, cmd_symbol_plans, cmd_dag_ascii
--json, --human, output envelope, JSON mode
transitive closure, BLOCKED_BY path query, Cypher patterns
MENTIONS_CODE reverse join, Plan sections by symbol
ASCII tree rendering, Graphviz DOT output
_parse_flags, --repo, --limit flags
tests/test_query_plan_bug.py, fixture graph state
```

---

### Section 05: Doc sync + consumer wiring
**File:** `section-05-doc-sync.md` | **Status:** Not Started

```
.claude/rules/intelligence.md, When to Query, How to Query
.claude/skills/query-intel/compose-intel-summary.md, Step F registry
plan-family consumers, new subcommand documentation
CLAUDE.md §Commands, §Intelligence graph paragraph
~/projects/lang_intelligence/CLAUDE.md, pipeline documentation
/sync-claude, doc drift detection
cross-reference links, intelligence graph consumers
```

---

### Section 06: Verification + cross-plan invalidation
**File:** `section-06-verification.md` | **Status:** Not Started

```
full vs incremental equivalence, round-trip test, diff assertion
graceful degradation, docker stop lang-intelligence, CI-runnable
diagnostics/verify-plan-bug-degraded.sh, workflow preservation
python3 .claude/skills/plan-audit/plan-invalidate.py, stale overlaps
reviewed: true → reviewed: false flip, cross-plan review invalidation
test matrix, fixture corpora, snapshot stability
./test-all.sh, plan-corpus check, repo-hygiene
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | plan_corpus schema + dag + exporter | `section-01-plan-corpus-extension.md` |
| 02 | Neo4j schema + importer + CodeReference bridge | `section-02-neo4j-schema-importer.md` |
| 03 | Commit-triggered sync wiring | `section-03-sync-wiring.md` |
| 04 | Plumbing query subcommands | `section-04-query-subcommands.md` |
| 05 | Doc sync + consumer wiring | `section-05-doc-sync.md` |
| 06 | Verification + cross-plan invalidation | `section-06-verification.md` |
