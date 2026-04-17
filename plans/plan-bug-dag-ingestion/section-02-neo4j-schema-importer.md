---
section: "02"
title: "Neo4j schema + importer + CodeReference bridge"
status: not-started
reviewed: true
goal: "Extend ~/projects/lang_intelligence/neo4j/schema.cypher with typed labels/constraints/indexes for plan/bug/fix-section nodes + edges; write import_plan_bug_graph.py that consumes §01's JSON envelope via two-phase MERGE (nodes then edges) with DETACH DELETE stale-pruning; route plan→code symbol joins through the existing CodeReference bridge pattern."
success_criteria:
  - "schema.cypher has CREATE CONSTRAINT clauses for :Plan (by name), :PlanSection (by id), :Subsection (by id), :Bug (by bug_id), :FixSection (by bug_id), :BugTrackerSection (by id), :Overview (by plan); idempotent re-run produces no schema changes."
  - "schema.cypher adds fulltext index on (:Plan.name, :Plan.full_name) and (:Bug.title, :Bug.subsystem) matching existing fulltext pattern."
  - "~/projects/lang_intelligence/neo4j/import_plan_bug_graph.py consumes JSON envelope from stdin or --input <file>; Phase 1 MERGEs all nodes (batched UNWIND); Phase 2 MERGEs all non-MENTIONS_CODE edges; Phase 3 routes MENTIONS_CODE → CodeReference → RESOLVES_TO via resolve_code_refs.py pattern."
  - "Stale-pruning: nodes in Neo4j but NOT in the envelope are detected via all_envelope_ids - existing_db_ids diff and removed via DETACH DELETE; mirrors import_code_graph.py:536-557."
  - "TOUCHES: declared entries (from §01 touches: frontmatter) create CodeReference with mention_kind='declared'; inferred entries (backtick scrape from body markdown) create mention_kind='inferred'; ambiguous resolutions create :UnresolvedSymbol per _resolve_target_py pattern in import_code_graph.py:229-246."
  - "Environment variables: NEO4J_URI, NEO4J_USER, NEO4J_PASS read from os.environ with documented defaults; no hardcoded credentials."
  - "~/projects/lang_intelligence/tests/test_import_plan_bug_graph.py: fixture envelope → in-memory mock driver → assert Cypher operations (MERGEs, DELETEs, counts); pattern matches test_import_code_graph.py."
  - "End-to-end smoke: export §01's JSON envelope for plans/plan-bug-dag-ingestion/ → pipe to importer → Cypher verify expected node/edge counts via intel-query.sh cypher."
  - "Satisfies mission criteria: 'Neo4j schema.cypher declares typed node labels...', '~/projects/lang_intelligence/neo4j/import_plan_bug_graph.py consumes the JSON envelope...', 'Plan/bug nodes are joined to code symbols via the existing bridge...'."
inspired_by:
  - "lang_intelligence neo4j/import_code_graph.py — two-phase MERGE + UNWIND batching + stale DETACH DELETE + _resolve_target_py resolution"
  - "lang_intelligence neo4j/schema.cypher — existing constraint/index patterns for Symbol/File/CodeReference"
  - "lang_intelligence neo4j/resolve_code_refs.py — CodeReference resolution + stale tracking pipeline"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Extend schema.cypher with plan/bug/fix-section labels + edges + indexes"
    status: not-started
  - id: "02.2"
    title: "Write import_plan_bug_graph.py two-phase MERGE + stale pruning"
    status: not-started
  - id: "02.3"
    title: "Wire CodeReference bridge for touches + backtick scraping"
    status: not-started
  - id: "02.4"
    title: "Importer unit tests with in-memory mock driver"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Neo4j schema + importer + CodeReference bridge

**Status:** Not Started
**Goal:** Extend `~/projects/lang_intelligence/neo4j/schema.cypher` with typed node labels, uniqueness constraints, performance indexes, and fulltext indexes for the plan/bug graph; write `~/projects/lang_intelligence/neo4j/import_plan_bug_graph.py` that consumes the JSON envelope produced by §01 in three phases (node MERGE → structural edge MERGE → CodeReference bridge), with stale-node DETACH DELETE mirroring `import_code_graph.py:536-557`; and wire plan→code-symbol joins through the existing `:MENTIONS_CODE → :CodeReference → :RESOLVES_TO` bridge, reusing `_resolve_target_py` and `_build_symbol_index` from `import_code_graph.py` as the SSOT.

**Success Criteria:**

- [ ] `schema.cypher` has `CREATE CONSTRAINT ... IF NOT EXISTS` for every new label; `cypher-shell < schema.cypher` run twice produces zero schema diffs on second invocation
- [ ] Fulltext indexes `plan_text` (Plan.name, Plan.full_name) and `bug_text` (Bug.title, Bug.subsystem) added; match existing `issue_text` / `symbol_text` Lucene-backed pattern
- [ ] `import_plan_bug_graph.py` reads JSON envelope from `--input <path>` or `--input -` (stdin); validates `schema_version == "1.0"` and bails on unknown versions
- [ ] Phase 1 (node upsert): UNWIND-batched MERGE (~1000/batch) with `ON CREATE SET first_imported_at` / `ON MATCH SET last_imported_at`; APOC fallback documented
- [ ] Phase 2 (structural/dependency edges): UNWIND-batched MERGE over all non-MENTIONS_CODE relationship types; APOC fallback documented
- [ ] Phase 3 (CodeReference bridge): `touches_raw` property from each node → declared mentions; body markdown backtick scan → inferred mentions; both resolved via `_resolve_target_py` / `_build_symbol_index` (imported from `import_code_graph.py`, not forked)
- [ ] Stale-pruning gate mirrors `jsonl_clean` idiom: only runs when envelope parsed cleanly (no errors, node count > 0); removes nodes in DB but absent from incoming `id` set via `DETACH DELETE`
- [ ] `NEO4J_URI`, `NEO4J_USER`, `NEO4J_PASS` read from `os.environ.get(...)` with documented defaults; no hardcoded secrets in source
- [ ] `import_plan_bug_graph.py` stays under 500 lines (per CLAUDE.md §File size)
- [ ] `tests/test_import_plan_bug_graph.py` green using MagicMock driver; covers all node label dimensions and all edge type dimensions (see §02.4 matrix)
- [ ] End-to-end smoke: `python -m scripts.plan_corpus export plans/plan-bug-dag-ingestion/ | python ~/projects/lang_intelligence/neo4j/import_plan_bug_graph.py --input -` succeeds; `scripts/intel-query.sh cypher "MATCH (p:PlanSection)-[:MENTIONS_CODE]->(cr)-[:RESOLVES_TO]->(s:Symbol) RETURN count(DISTINCT s) > 0"` returns true
- [ ] Satisfies mission criterion: "Neo4j schema.cypher declares typed node labels... with uniqueness constraints..." (§02 criterion) and "Plan/bug nodes are joined to code symbols via the existing bridge..." (§02 criterion)

**Context:** §01 delivers a deterministic `{"schema_version": "1.0", "nodes": [...], "relationships": [...]}` JSON envelope via `python -m scripts.plan_corpus export`. This section projects that envelope into the Neo4j graph that already holds 191K+ code symbols and 505K+ CALLS edges. The key design principle is **zero forking** of the existing resolution infrastructure: `_resolve_target_py`, `_build_symbol_index`, and `_retry_tx` from `import_code_graph.py` are the canonical implementations — reusing them keeps the resolution story coherent (same ambiguity semantics, same `UnresolvedSymbol` stub pattern, same retry backoff). The importer must consume the envelope as a black box; it MUST NOT re-parse plan frontmatter to reconstruct edge semantics — all relationship semantics are in the envelope by §01's SSOT guarantee.

**Reference implementations:**
- **lang_intelligence** `neo4j/import_code_graph.py`: two-phase MERGE + UNWIND batching + stale DETACH DELETE (lines 536-557) + `_resolve_target_py` resolution (lines 229-246) + `_build_symbol_index` (lines 249-278) + `_retry_tx` exponential backoff (lines 283-295) — all reused, none forked
- **lang_intelligence** `neo4j/schema.cypher`: existing constraint/index/fulltext pattern (lines 95-178) that new Plan/Bug section extends
- **lang_intelligence** `neo4j/resolve_code_refs.py`: `CodeReference` node creation, `MENTIONS_CODE` + `RESOLVES_TO` edge pattern, staleness tracking, `UnresolvedSymbol` ambiguity handling
- **lang_intelligence** `tests/test_import_code_graph.py`: `importlib.util.spec_from_file_location` workaround for the `neo4j/` package-shadow problem; MagicMock driver pattern for unit tests without live Neo4j

**Depends on:** Section 01 (delivers the JSON envelope shape, `export_neo4j_json()`, and the `touches_raw` property on nodes).

---

## Intelligence Reconnaissance

Queries run 2026-04-17:

- `scripts/intel-query.sh --human symbols "MERGE" --repo ori --limit 10` — expected zero: Cypher keywords are not Rust/Python symbols; the intelligence graph indexes compiled-language symbols only. Confirmed: zero results. This is the expected outcome — Cypher is not indexed.
- `scripts/intel-query.sh --human search "Neo4j importer" --limit 5` — returned unrelated results (React, GraphQL, web tooling). No prior art for plan-metadata Neo4j importers in the indexed issue corpus.
- `scripts/intel-query.sh --human file-symbols "import_code_graph" --repo ori` — zero matches. Confirms the code-symbol index covers Rust/Ori `.rs`/`.ori` files only; Python scripts in `lang_intelligence/` are absent. Manual reading of `import_code_graph.py`, `resolve_code_refs.py`, `schema.cypher`, `test_import_code_graph.py` was required.
- `scripts/intel-query.sh --human similar "_resolve_target_py" --repo rust,swift,go --limit 5` — symbol not found in graph (Python function, not indexed). No cross-compiler embedding match available.

Results summary (≤500 chars) [ori]: Graph available (Neo4j 5.26.24, 191K+ symbols). All four queries returned zero relevant results — confirmed the graph indexes Rust/Ori compiled symbols only; Python `lang_intelligence/` scripts are absent. No prior art for plan-metadata importer patterns found via issue search. Implementation is grounded entirely by manual reading of the five `lang_intelligence/` target files. No blast-radius concerns from the graph perspective — §02 adds only new Python + Cypher files with no changes to existing Rust/Ori code.

See `.claude/skills/query-intel/compose-intel-summary.md` for the full query protocol (SSOT — do NOT `@`-include in plan files; plan markdown is not harness-expanded, so the include would be a dead literal).

---

## 02.1 Extend schema.cypher with plan/bug/fix-section labels + edges + indexes

**File(s):** `~/projects/lang_intelligence/neo4j/schema.cypher`

This subsection appends a "Plan & Bug Graph" section to `schema.cypher`, following the same structural pattern as the existing "Code Graph" (lines 95–178) and "Bridge Layer" (lines 180–243) sections. The section opens with a schematic header block documenting node types, properties, and relationship types — matching the comment style at lines 54–93 and 152–178 of the existing file.

All new statements use the `IF NOT EXISTS` guard, making the entire schema.cypher file idempotent: running `cypher-shell -u neo4j -p intelligence < neo4j/schema.cypher` a second time produces zero schema changes. This matches the behavior of every existing constraint and index in the file.

### Constraints

```cypher
// ═══════════════════════════════════════════════
// Plan & Bug Graph
// ═══════════════════════════════════════════════

// ─────────────────────────────────────────────
// Plan & Bug Graph: Constraints
// ─────────────────────────────────────────────

// Plan nodes keyed by directory name (e.g., "plan-bug-dag-ingestion").
// Stable across plan content changes; derived from plans/<dir>/index.md path.
CREATE CONSTRAINT plan_name IF NOT EXISTS FOR (p:Plan) REQUIRE p.name IS UNIQUE;

// PlanSection keyed by repo-relative path (e.g., "plans/<dir>/section-01-*.md").
CREATE CONSTRAINT plan_section_id IF NOT EXISTS FOR (s:PlanSection) REQUIRE s.id IS UNIQUE;

// Subsection keyed by "<section-id>#<subsection-id>" (e.g., "plans/.../section-02.md#02.1").
CREATE CONSTRAINT subsection_id IF NOT EXISTS FOR (sub:Subsection) REQUIRE sub.id IS UNIQUE;

// Bug entries keyed by BUG-XX-NNN identifier.
CREATE CONSTRAINT bug_id IF NOT EXISTS FOR (b:Bug) REQUIRE b.bug_id IS UNIQUE;

// FixSection nodes keyed by the bug they fix (BUG-XX-NNN from fix-BUG-*.md frontmatter).
CREATE CONSTRAINT fix_section_id IF NOT EXISTS FOR (f:FixSection) REQUIRE f.bug_id IS UNIQUE;

// BugTrackerSection keyed by "bug-tracker/section-<NN>-<slug>.md" (repo-relative path).
CREATE CONSTRAINT bug_tracker_section_id IF NOT EXISTS FOR (bt:BugTrackerSection) REQUIRE bt.id IS UNIQUE;

// Overview nodes keyed by plan directory name (same namespace as Plan.name).
CREATE CONSTRAINT overview_plan IF NOT EXISTS FOR (o:Overview) REQUIRE o.plan IS UNIQUE;

// RoadmapSection keyed by repo-relative path "roadmap/section-<NN>-<slug>.md".
CREATE CONSTRAINT roadmap_section_id IF NOT EXISTS FOR (rs:RoadmapSection) REQUIRE rs.id IS UNIQUE;

// CompletedIndex keyed by plan directory name under plans/completed/.
CREATE CONSTRAINT completed_index_name IF NOT EXISTS FOR (ci:CompletedIndex) REQUIRE ci.name IS UNIQUE;
```

### Indexes

```cypher
// ─────────────────────────────────────────────
// Plan & Bug Graph: Performance indexes
// ─────────────────────────────────────────────

CREATE INDEX plan_status IF NOT EXISTS FOR (p:Plan) ON (p.status);
CREATE INDEX plan_section_status IF NOT EXISTS FOR (s:PlanSection) ON (s.status);
CREATE INDEX bug_severity IF NOT EXISTS FOR (b:Bug) ON (b.severity);
CREATE INDEX bug_status IF NOT EXISTS FOR (b:Bug) ON (b.status);
CREATE INDEX fix_section_status IF NOT EXISTS FOR (f:FixSection) ON (f.status);

// ─────────────────────────────────────────────
// Plan & Bug Graph: Full-text search indexes
// ─────────────────────────────────────────────

CREATE FULLTEXT INDEX plan_text IF NOT EXISTS
  FOR (p:Plan) ON EACH [p.name, p.full_name];

CREATE FULLTEXT INDEX bug_text IF NOT EXISTS
  FOR (b:Bug) ON EACH [b.title, b.subsystem];
```

### Schematic header block (for the Node types comment at file end)

```cypher
// ─────────────────────────────────────────────
// Plan & Bug Graph: Node types and properties
//
// (:Plan {name, full_name, status, reroute, order, repo})
//   - name: plan directory name (stable ID)
//   - full_name: display title from index.md
//   - status: "not-started" | "in-progress" | "complete"
//   - reroute: bool (from index.md reroute: true)
//   - order: int (reroute queue priority; 999 = unset)
//
// (:PlanSection {id, title, status, reviewed, goal, repo, path, touches_raw})
//   - id: repo-relative path "plans/<dir>/section-<NN>-<slug>.md"
//   - touches_raw: list[str] from touches: frontmatter (raw, unresolved)
//
// (:Subsection {id, title, status})
//   - id: "<section-path>#<subsection-id>" (e.g., "plans/.../section-02.md#02.1")
//
// (:Bug {bug_id, title, severity, status, subsystem, found, source, repo})
//   - bug_id: "BUG-XX-NNN" (canonical identifier)
//   - severity: "critical" | "high" | "medium" | "low"
//
// (:FixSection {bug_id, title, severity, status, goal, subsystem, repo, path, touches_raw})
//   - bug_id: "BUG-XX-NNN" (matches Bug.bug_id — resolves to same bug)
//   - touches_raw: list[str] from touches: frontmatter
//
// (:BugTrackerSection {id, title, status, repo})
//   - id: repo-relative path "bug-tracker/section-<NN>-<slug>.md"
//
// (:Overview {plan, title, status, repo, path})
//   - plan: plan directory name (same as Plan.name)
//
// (:RoadmapSection {id, title, status, repo, path})
//   - id: "roadmap/section-<NN>-<slug>.md"
//
// (:CompletedIndex {name, full_name, status, repo})
//   - name: plan directory name under plans/completed/
//
// ─────────────────────────────────────────────
// Plan & Bug Graph: Relationship types
//
// (:Plan)-[:HAS_SECTION]->(:PlanSection)
// (:Plan)-[:HAS_OVERVIEW]->(:Overview)
// (:PlanSection)-[:HAS_SUBSECTION]->(:Subsection)
// (:BugTrackerSection)-[:HAS_BUG]->(:Bug)
// (:Bug)-[:FIXED_BY]->(:FixSection)
// (:FixSection)-[:RESOLVES]->(:Bug)
// (:PlanSection|:Plan|:FixSection)-[:DEPENDS_ON]->(:PlanSection|:Plan|:FixSection)
// (:Plan|:PlanSection|:FixSection|:Bug)-[:BLOCKED_BY]->(:PlanSection|:Plan|:Bug)
// (:Plan|:PlanSection)-[:SUPERSEDES]->(:Plan|:PlanSection)
// (:PlanSection|:Plan|:Overview)-[:REFERENCES]->(:PlanSection|:Plan)
// (:Plan|:PlanSection|:Bug|:FixSection)-[:MENTIONS_CODE]->(:CodeReference)
//   — reuses the existing Bridge Layer CodeReference pattern (schema lines 188-213)
// ─────────────────────────────────────────────
```

### Idempotency guarantee

Every `CREATE CONSTRAINT ... IF NOT EXISTS` and `CREATE INDEX ... IF NOT EXISTS` statement is idempotent per Neo4j 5.x semantics. Running the full `schema.cypher` a second time produces exactly zero schema changes — verified by running `cypher-shell -f schema.cypher` twice and comparing `:schema` output before and after the second run.

### Task breakdown

- [ ] Open `~/projects/lang_intelligence/neo4j/schema.cypher` and append the "Plan & Bug Graph" section after the final "Bridge Layer" block
- [ ] Add constraints block (9 constraints) with comments explaining each key choice
- [ ] Add performance indexes block (5 indexes)
- [ ] Add fulltext indexes block (2 indexes: `plan_text`, `bug_text`)
- [ ] Add schematic header comment block (node types + properties + relationship types)
- [ ] Verify idempotent run: `docker exec -i lang-intelligence cypher-shell -u neo4j -p intelligence < ~/projects/lang_intelligence/neo4j/schema.cypher` — first run outputs "Created N constraints, N indexes"; second run outputs "Added 0 constraints, 0 indexes"
- [ ] Verify via `:schema` command in Neo4j Browser that all 9 constraints and 7 new indexes appear

- [ ] **Subsection close-out (02.1)** — MANDATORY before starting 02.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check`; clean any temp files.

---

## 02.2 Write import_plan_bug_graph.py two-phase MERGE + stale pruning

**File(s):** `~/projects/lang_intelligence/neo4j/import_plan_bug_graph.py` (new, target ≤ 500 lines total across §02.2 + §02.3)

This subsection delivers the core importer: CLI surface, envelope ingestion and validation, Phase 1 (node MERGE), Phase 2 (structural/dependency edge MERGE), and stale-pruning. The CodeReference bridge (Phase 3) is deferred to §02.3 which has access to the symbol resolution context.

**File size constraint:** `import_plan_bug_graph.py` must stay under 500 lines total (§02.2 + §02.3 combined). The 500-line limit per CLAUDE.md §File size applies to production Python files the same as Rust files. If the importer approaches the limit, extract `_resolve_mentions_code()` and `_build_mentions_batch()` into a sibling helper `import_plan_bug_bridge.py` and `import` it — do NOT inline more than the limit allows.

**APOC dependency note:** APOC (Awesome Procedures On Cypher) provides dynamic-label and dynamic-relationship-type operations but is an optional plugin. The `schema.cypher` setup uses only built-in Neo4j 5.x Cypher (no APOC), so the graph is guaranteed to be available without APOC. For the importer, APOC is desirable for Phase 1 (dynamic labels via `apoc.create.addLabels`) and Phase 2 (dynamic relationship type via `apoc.merge.relationship`), but MUST NOT be required. Both phases must have documented APOC-optional fallbacks.

### CLI surface

```
python import_plan_bug_graph.py --input <path>     # read JSON envelope from file
python import_plan_bug_graph.py --input -          # read JSON envelope from stdin
python import_plan_bug_graph.py --input <path> --full        # explicit full rebuild (default)
python import_plan_bug_graph.py --input <path> --incremental # stub: same as --full in Phase 1
python import_plan_bug_graph.py --input <path> --dry-run     # print operation plan, no writes
python import_plan_bug_graph.py --health           # check Neo4j connectivity and exit
```

`--incremental` is a stub in Phase 1 (this section); §03.4 notes that full rebuild is always correct for the plan corpus (Design Principle 3 in `00-overview.md`). The flag is present now so the §03 sync wrapper can call it with a uniform interface matching `sync-ori-graph.sh`.

### Environment variables

```python
NEO4J_URI  = os.environ.get("NEO4J_URI",  "bolt://localhost:7687")
NEO4J_USER = os.environ.get("NEO4J_USER", "neo4j")
NEO4J_PASS = os.environ.get("NEO4J_PASS", "intelligence")
```

**Anti-pattern note:** `import_code_graph.py` uses module-level string literals for the connection (`NEO4J_URI = "bolt://localhost:7687"`). This new importer MUST NOT perpetuate that pattern — credentials and connection parameters belong in environment variables. The module docstring must explicitly document this as an improvement over the sibling file's approach.

### Envelope validation

```python
def _load_envelope(path_or_stdin: str) -> dict:
    """Load and validate the JSON envelope from §01's export_json.py.

    Raises ValueError if:
    - JSON is malformed
    - schema_version is missing
    - schema_version != "1.0" (unknown version — bail, not best-effort)
    Returns the parsed dict with nodes and relationships lists guaranteed present.
    """
    if path_or_stdin == "-":
        raw = sys.stdin.read()
    else:
        raw = Path(path_or_stdin).read_text()

    try:
        envelope = json.loads(raw)
    except json.JSONDecodeError as e:
        raise ValueError(f"Malformed JSON envelope: {e}") from e

    version = envelope.get("schema_version")
    if version != "1.0":
        raise ValueError(
            f"Unknown envelope schema_version: {version!r}. "
            f"Expected '1.0'. Re-run with an updated §01 exporter."
        )

    envelope.setdefault("nodes", [])
    envelope.setdefault("relationships", [])
    return envelope
```

### Phase 1 — Node MERGE (APOC path + fallback)

**Option A — APOC available (preferred):**

```cypher
UNWIND $nodes AS n
MERGE (x {id: n.id})
ON CREATE SET x += n.properties, x.first_imported_at = datetime()
ON MATCH  SET x += n.properties, x.last_imported_at  = datetime()
WITH x, n
CALL apoc.create.addLabels(x, n.labels) YIELD node
RETURN count(node)
```

**Option B — No APOC (fallback, generates per-label-combination Cypher):**

Since the label set is a finite closed vocabulary (9 labels: Plan, PlanSection, Subsection, Bug, FixSection, BugTrackerSection, Overview, RoadmapSection, CompletedIndex), group nodes by their `labels` list, then emit one batched MERGE per label. Each label maps to a distinct `id` property namespace (see §02.1 stable ID strategy).

```python
# Group nodes by label (each node has exactly one label from the finite set)
from collections import defaultdict
nodes_by_label: dict[str, list[dict]] = defaultdict(list)
for node in envelope["nodes"]:
    labels = node.get("labels", [])
    label = labels[0] if labels else "Unknown"
    nodes_by_label[label].append(node)

KNOWN_LABELS = [
    "Plan", "PlanSection", "Subsection", "Bug", "FixSection",
    "BugTrackerSection", "Overview", "RoadmapSection", "CompletedIndex",
]

for label in KNOWN_LABELS:
    batch = nodes_by_label.get(label, [])
    if not batch:
        continue
    for chunk in _chunks(batch, BATCH_SIZE):
        _retry_write(driver, lambda tx, c=chunk, lbl=label: tx.run(f"""
            UNWIND $nodes AS n
            MERGE (x:{lbl} {{id: n.id}})
            ON CREATE SET x += n.properties, x.first_imported_at = datetime()
            ON MATCH  SET x += n.properties, x.last_imported_at  = datetime()
        """, nodes=c))
```

The fallback is the default path — implement Option B first (no APOC dependency), then add Option A as an `_apoc_available(driver)` probe branch.

### Phase 2 — Structural/dependency edge MERGE (APOC path + fallback)

**Option A — APOC available:**

```cypher
UNWIND $rels AS r
MATCH (s {id: r.start_id}), (t {id: r.end_id})
CALL apoc.merge.relationship(s, r.type, {}, r.properties, t) YIELD rel
RETURN count(rel)
```

**Option B — No APOC (fallback, parametrized over the finite type set):**

```python
KNOWN_REL_TYPES = [
    "DEPENDS_ON", "SUPERSEDES", "BLOCKED_BY", "RESOLVES", "REFERENCES",
    "HAS_SECTION", "HAS_SUBSECTION", "HAS_BUG", "FIXED_BY", "HAS_OVERVIEW",
    "REWRITES", "UNBLOCKS", "UPDATE_COMPLETE", "UPDATED_BY",
]

# Group relationships by type
rels_by_type: dict[str, list[dict]] = defaultdict(list)
for rel in envelope["relationships"]:
    if rel["type"] != "MENTIONS_CODE":  # defer to Phase 3
        rels_by_type[rel["type"]].append(rel)

for rel_type in KNOWN_REL_TYPES:
    batch = rels_by_type.get(rel_type, [])
    if not batch:
        continue
    for chunk in _chunks(batch, BATCH_SIZE):
        _retry_write(driver, lambda tx, c=chunk, rt=rel_type: tx.run(f"""
            UNWIND $rels AS r
            MATCH (s {{id: r.start_id}}), (t {{id: r.end_id}})
            MERGE (s)-[rel:{rt}]->(t)
            SET rel += r.properties
        """, rels=c))
```

### Stale-node pruning

Mirrors `import_code_graph.py:536-557`. The guard is critical: if the envelope was parsed with errors or is empty, skip pruning to avoid mass-deleting valid data.

```python
def _prune_stale_nodes(driver, envelope: dict, dry_run: bool = False) -> int:
    """Remove plan/bug nodes in Neo4j that are absent from the incoming envelope.

    Only runs when the envelope was parsed cleanly (no errors, node count > 0).
    Mirrors import_code_graph.py:520-557 jsonl_clean gate idiom.

    Returns the count of deleted nodes.
    """
    PLAN_BUG_LABELS = [
        "Plan", "PlanSection", "Subsection", "Bug", "FixSection",
        "BugTrackerSection", "Overview", "RoadmapSection", "CompletedIndex",
    ]

    incoming_ids = {n["id"] for n in envelope["nodes"] if n.get("id")}
    if not incoming_ids:
        print("  Stale-pruning SKIPPED: empty envelope (safety gate)")
        return 0

    # Collect all existing IDs for the plan/bug label set
    label_filter = " OR ".join(
        f"'{lbl}' IN labels(n)" for lbl in PLAN_BUG_LABELS
    )
    with driver.session() as session:
        result = session.run(f"""
            MATCH (n)
            WHERE n.id IS NOT NULL
              AND ({label_filter})
            RETURN n.id AS node_id
        """)
        existing_ids = {rec["node_id"] for rec in result}

    stale_ids = existing_ids - incoming_ids
    if not stale_ids:
        return 0

    print(f"  Stale-pruning: {len(stale_ids)} nodes to remove")
    if dry_run:
        for sid in sorted(stale_ids)[:5]:
            print(f"    [dry-run] would DETACH DELETE node id={sid!r}")
        if len(stale_ids) > 5:
            print(f"    ... and {len(stale_ids) - 5} more")
        return len(stale_ids)

    stale_list = list(stale_ids)
    for chunk in _chunks(stale_list, BATCH_SIZE):
        _retry_write(driver, lambda tx, c=chunk: tx.run("""
            UNWIND $ids AS sid
            MATCH (n)
            WHERE n.id = sid
              AND any(lbl IN labels(n) WHERE lbl IN
                    ['Plan', 'PlanSection', 'Subsection', 'Bug', 'FixSection',
                     'BugTrackerSection', 'Overview', 'RoadmapSection', 'CompletedIndex'])
            DETACH DELETE n
        """, ids=c))

    return len(stale_ids)
```

### Retry helper (reuse, do NOT fork)

`_retry_tx` from `import_code_graph.py` is imported directly:

```python
# import_plan_bug_graph.py module preamble:
# NOTE: This script must be run from outside the neo4j/ directory
# or via the sync wrapper which handles the cwd automatically.
# The neo4j/ directory shadows the neo4j Python package — see test file
# for the importlib.util workaround when running tests.

# Import shared helpers from sibling importer (SSOT — do NOT fork):
import importlib.util as _ilu
import os as _os
_icg_path = _os.path.join(_os.path.dirname(__file__), "import_code_graph.py")
_icg_spec = _ilu.spec_from_file_location("import_code_graph", _icg_path)
_icg = _ilu.module_from_spec(_icg_spec)
_icg_spec.loader.exec_module(_icg)
_retry_tx        = _icg._retry_tx
_retry_write     = _icg._retry_write
_resolve_target_py   = _icg._resolve_target_py   # used in §02.3
_build_symbol_index  = _icg._build_symbol_index  # used in §02.3
```

This is the canonical import pattern for reusing helpers from a sibling script in the same `neo4j/` directory without triggering the package-shadow problem.

### Tasks

- [ ] Create `~/projects/lang_intelligence/neo4j/import_plan_bug_graph.py` with:
  - [ ] Module docstring documenting anti-APOC-dependency stance, env-var connection pattern (improvement over sibling file's hardcoded credentials), three-phase design, and §02.3 CodeReference deferral
  - [ ] `_load_envelope(path_or_stdin: str) -> dict` — validation with `schema_version` guard
  - [ ] `_chunks(lst, size)` — generic batching helper
  - [ ] `_apoc_available(driver) -> bool` — probe APOC via `CALL apoc.version() YIELD version RETURN version`; catches `ClientError` and returns False
  - [ ] `_merge_nodes(driver, envelope, *, dry_run=False) -> dict` — Phase 1 (APOC or fallback)
  - [ ] `_merge_structural_edges(driver, envelope, *, dry_run=False) -> dict` — Phase 2 (APOC or fallback), skips `MENTIONS_CODE`
  - [ ] `_prune_stale_nodes(driver, envelope, *, dry_run=False) -> int` — stale removal with safety gate
  - [ ] `main()` — CLI parse, load envelope, connect driver (env vars), run phases 1–3 (Phase 3 = stub `pass` until §02.3), report stats
- [ ] Verify `python import_plan_bug_graph.py --health` exits 0 when Neo4j is up, non-zero when down
- [ ] Verify `python import_plan_bug_graph.py --input - --dry-run < fixture.json` prints the operation plan without writing to Neo4j
- [ ] Verify file stays under 500 lines: `wc -l ~/projects/lang_intelligence/neo4j/import_plan_bug_graph.py`

- [ ] **Subsection close-out (02.2)** — MANDATORY before starting 02.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check`.

---

## 02.3 Wire CodeReference bridge for touches + backtick scraping

**File(s):** `~/projects/lang_intelligence/neo4j/import_plan_bug_graph.py` (extended in Phase 3 section)

This subsection fills in Phase 3 of the importer: resolving `touches_raw` properties (declarative mentions from `touches:` frontmatter, produced by §01's exporter) and scraping body markdown for backtick-fenced code tokens (inferred mentions). Both paths route through the existing `_resolve_target_py` / `_build_symbol_index` functions imported in §02.2 — NO fork of the resolution logic.

### CodeReference node shape

The `CodeReference` node created for plan/bug mentions reuses the existing schema (lines 218–228 of `schema.cypher`), with `source_type` set to `"plan_section"` / `"bug"` / `"fix_section"` / `"overview"` (extending the existing `"issue" | "comment" | "review"` vocabulary). The `source_key` is the node's stable `id` property. This is a deliberate extension of the bridge vocabulary — the schema comment already documents `source_type: "issue" | "comment" | "review"`, and adding `"plan_section"` etc. is backward-compatible (existing queries that filter on `source_type IN ['issue', 'comment', 'review']` continue to work; new queries can include the plan types).

```
CodeReference node properties (plan mentions):
  repo              = "ori"
  source_type       = "plan_section" | "bug" | "fix_section" | "overview"
  source_key        = <node.id>           (e.g., "plans/plan-bug-dag-ingestion/section-02-*.md")
  raw_text          = <the token>         (e.g., "_resolve_target_py" or "compiler/ori_types/src/check/mod.rs")
  mention_kind      = "declared" | "inferred"
  confidence        = 1.0 for declared; 0.7 for inferred (heuristic)
  resolved          = True | False
  stale             = False (set to True by --invalidate-stale sweeps in resolve_code_refs.py)
  ambiguous         = True | False
  ambiguous_count   = <int>  (set when len(matches) > 1 in _resolve_target_py)
```

### Declared mentions (`mention_kind = "declared"`)

Each node in the envelope has a `touches_raw` property (a list of strings from `touches:` frontmatter, or empty list). For each `touches_raw` entry:

1. Build the symbol index once per import run (`_build_symbol_index(driver, "ori")`).
2. For each `raw_text` in `touches_raw`, call `_resolve_target_py(idx, raw_text)`.
3. If resolved → one `CodeReference` node + `MENTIONS_CODE` edge from the plan node + `RESOLVES_TO` edge to the `Symbol` or `File`.
4. If unresolved (ambiguous or missing) → `UnresolvedSymbol` stub + `CodeReference` with `resolved=False, ambiguous=<bool>, ambiguous_count=<N>`.

```python
def _create_code_reference(tx, source_id: str, source_type: str,
                            raw_text: str, mention_kind: str,
                            resolved: bool, ambiguous: bool,
                            ambiguous_count: int,
                            target_qn: str | None = None,
                            target_sh: str | None = None) -> None:
    """MERGE a CodeReference node and link it to the source plan/bug node.

    Source node must already exist (Phase 1 guarantee).
    """
    cr_props = {
        "repo": "ori",
        "source_type": source_type,
        "source_key": source_id,
        "raw_text": raw_text,
        "mention_kind": mention_kind,
        "confidence": 1.0 if mention_kind == "declared" else 0.7,
        "resolved": resolved,
        "stale": False,
        "ambiguous": ambiguous,
        "ambiguous_count": ambiguous_count,
        "resolution_attempted_at": datetime.now(timezone.utc).isoformat(),
    }
    tx.run("""
        MERGE (cr:CodeReference {
            repo: $repo, source_type: $stype,
            source_key: $skey, raw_text: $raw
        })
        SET cr += $props
        WITH cr
        MATCH (src {id: $src_id})
        MERGE (src)-[:MENTIONS_CODE]->(cr)
    """, repo="ori", stype=source_type, skey=source_id,
         raw=raw_text, props=cr_props, src_id=source_id)

    if resolved and target_qn is not None:
        tx.run("""
            MATCH (cr:CodeReference {
                repo: $repo, source_type: $stype,
                source_key: $skey, raw_text: $raw
            })
            MATCH (sym:Symbol {repo: $repo,
                               qualified_name: $tgt_qn,
                               signature_hash: $tgt_sh})
            MERGE (cr)-[:RESOLVES_TO]->(sym)
        """, repo="ori", stype=source_type, skey=source_id,
             raw=raw_text, tgt_qn=target_qn, tgt_sh=target_sh)
    elif resolved and target_qn is None:
        # File path resolution (raw_text matched a File node, not a Symbol)
        tx.run("""
            MATCH (cr:CodeReference {
                repo: $repo, source_type: $stype,
                source_key: $skey, raw_text: $raw
            })
            MATCH (f:File {repo: $repo, path: $path})
            MERGE (cr)-[:RESOLVES_TO]->(f)
        """, repo="ori", stype=source_type, skey=source_id,
             raw=raw_text, path=raw_text)
    else:
        # Unresolved — create or reuse UnresolvedSymbol stub
        tx.run("""
            MERGE (u:UnresolvedSymbol {repo: $repo, target_identifier: $tid})
            WITH u
            MATCH (cr:CodeReference {
                repo: $repo, source_type: $stype,
                source_key: $skey, raw_text: $raw
            })
            MERGE (cr)-[:RESOLVES_TO]->(u)
        """, repo="ori", stype=source_type, skey=source_id,
             raw=raw_text, tid=raw_text)
```

### Inferred mentions (`mention_kind = "inferred"`)

For nodes that include a `body_preview` property (up to 4KB of body markdown from the envelope — see §01.4's boundary decision), scan for backtick-fenced tokens matching:

- **File path pattern**: `r'\`([a-zA-Z0-9_/.-]+\.(?:rs|ori|md|py|sh))\`'` — matches paths like `` `compiler/ori_types/src/check/mod.rs` ``
- **Symbol name pattern**: `r'\`([A-Za-z_][A-Za-z0-9_:]{2,})\`'` — matches identifiers like `` `_resolve_target_py` `` or `` `check_exhaustiveness` ``

**Decision on body source:** The envelope carries `body_preview` (first 4KB, set in §01.4's exporter). The scanner works on `body_preview`. For nodes where a full scan is needed (body > 4KB), the `path` property gives the repo-relative file path; the scanner can re-read the full file from disk when `_FULL_BODY_SCAN = os.environ.get("PLAN_FULL_BODY_SCAN", "0") == "1"`. Default is preview-only (bounded); full scan is opt-in for complete coverage.

**Ambiguity policy:** Per the existing `_resolve_target_py` contract (lines 229-246 of `import_code_graph.py`): if multiple symbols match a token (ambiguous), do NOT pick arbitrarily — create `UnresolvedSymbol` stub with `ambiguous=True, ambiguous_count=N`. This matches the issue-graph pipeline's behavior and keeps the "ambiguous" concept consistent across all bridge entries.

**Deduplication:** Multiple occurrences of the same token in one node's body produce one `CodeReference` node (MERGE on `(repo, source_type, source_key, raw_text)`) with `occurrence_count` incremented.

### Tasks

- [ ] Add `_build_mentions_for_node(node: dict, idx: dict) -> list[dict]` in `import_plan_bug_graph.py`:
  - Processes `node["properties"].get("touches_raw", [])` for declared mentions
  - Processes `node["properties"].get("body_preview", "")` with file-path and symbol regexes for inferred mentions
  - Returns list of `{"raw_text": str, "mention_kind": str, "resolved": bool, "ambiguous": bool, "ambiguous_count": int, "target_qn": str|None, "target_sh": str|None}` records
- [ ] Add `_merge_code_references(driver, envelope, idx, *, dry_run=False) -> dict` (Phase 3):
  - Calls `_build_mentions_for_node` for every node with non-empty `touches_raw` or `body_preview`
  - Batches `_create_code_reference` calls via `_retry_write`
  - Returns stats `{"declared": N, "inferred": N, "resolved": N, "unresolved": N, "ambiguous": N}`
- [ ] Update `main()` to:
  - Build symbol index once: `idx = _build_symbol_index(driver, "ori")` (after Phase 1+2 complete)
  - Call `_merge_code_references(driver, envelope, idx, dry_run=dry_run)`
  - Print Phase 3 stats line
- [ ] Verify `body_preview` is present on at least one node from the real corpus export (§01.4 must set it; if absent, add a `body_preview` stub to the test fixture in §02.4)
- [ ] Verify file stays under 500 lines after §02.3 additions

- [ ] **Subsection close-out (02.3)** — MANDATORY before starting 02.4:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check`.

---

## 02.4 Importer unit tests with in-memory mock driver

**File(s):** `~/projects/lang_intelligence/tests/test_import_plan_bug_graph.py` (new, ~60 lines)

This subsection delivers the unit test file for `import_plan_bug_graph.py`. The tests follow the exact pattern from `tests/test_import_code_graph.py`:

- `importlib.util.spec_from_file_location` to bypass the `neo4j/` directory package shadow
- MagicMock driver + session for all tests (no live Neo4j required)
- `_skip_if_no_module()` guard for CI environments where the neo4j Python package is not installed

**Package shadow problem:** The `~/projects/lang_intelligence/neo4j/` directory is named `neo4j`, which shadows the `neo4j` Python package. Any test run from the project directory with `neo4j/` on `sys.path` will fail with `ImportError: cannot import name 'GraphDatabase'` because it finds the directory instead of the package. The workaround (per `test_import_code_graph.py:27-47`) is to use `importlib.util.spec_from_file_location` with a filtered `sys.path` that strips entries ending in `/lang_intelligence` or `/lang_intelligence/neo4j`. The new test file must use the identical workaround — copy the `_import_module()` function from `test_import_code_graph.py`, adjusted for the new module path.

**Recommended test invocation:**

```bash
# From /tmp or any directory outside lang_intelligence to avoid the shadow:
cd /tmp && python3 -m pytest ~/projects/lang_intelligence/tests/test_import_plan_bug_graph.py -v
# Or via the project's test runner if it handles this automatically.
```

### Test matrix

The matrix covers all **node label dimensions** and all **edge type dimensions** per CLAUDE.md §MANDATORY Matrix Testing. Every dimension must appear in at least one test.

**Node label dimensions** (9 labels):

| Label | Must appear in |
|---|---|
| `Plan` | `test_importer_merges_plan_node` |
| `PlanSection` | `test_importer_merges_plan_section_node` |
| `Subsection` | `test_importer_multi_plan_envelope_includes_subsection` |
| `Bug` | `test_importer_merges_bug_node` |
| `FixSection` | `test_importer_merges_fix_section_node` |
| `BugTrackerSection` | `test_importer_merges_bug_tracker_section_node` |
| `Overview` | `test_importer_merges_overview_node` |
| `RoadmapSection` | `test_importer_multi_plan_envelope_includes_roadmap_section` |
| `CompletedIndex` | `test_importer_multi_plan_envelope_includes_completed_index` |

**Edge type dimensions** (10 types):

| Edge type | Must appear in |
|---|---|
| `DEPENDS_ON` | `test_importer_merges_depends_on_edge` |
| `SUPERSEDES` | `test_importer_multi_plan_with_supersedes_edges` |
| `BLOCKED_BY` | `test_importer_merges_structural_edges` |
| `RESOLVES` | `test_importer_merges_structural_edges` |
| `HAS_SECTION` | `test_importer_merges_structural_edges` |
| `HAS_SUBSECTION` | `test_importer_merges_structural_edges` |
| `HAS_BUG` | `test_importer_merges_structural_edges` |
| `FIXED_BY` | `test_importer_merges_structural_edges` |
| `REFERENCES` | `test_importer_merges_structural_edges` |
| `MENTIONS_CODE` | `test_importer_declared_mentions_code_creates_code_reference` |

### Test scenarios

**`test_importer_rejects_malformed_json`**
```python
def test_importer_rejects_malformed_json(tmp_path):
    """Malformed JSON raises ValueError with a clear message."""
    p = tmp_path / "bad.json"
    p.write_text("{not valid json")
    with pytest.raises(ValueError, match="Malformed JSON"):
        _mod._load_envelope(str(p))
```

**`test_importer_rejects_incompatible_schema_version`** ← semantic pin
```python
def test_importer_rejects_incompatible_schema_version(tmp_path):
    """Envelope with schema_version != '1.0' raises ValueError."""
    p = tmp_path / "v2.json"
    p.write_text(json.dumps({"schema_version": "2.0", "nodes": [], "relationships": []}))
    with pytest.raises(ValueError, match="Unknown envelope schema_version"):
        _mod._load_envelope(str(p))
```

**`test_importer_does_not_create_nodes_without_id`** ← negative pin
```python
def test_importer_does_not_create_nodes_without_id(tmp_path):
    """Nodes without 'id' field are skipped — malformed envelope causes validation error."""
    envelope = {
        "schema_version": "1.0",
        "nodes": [{"labels": ["Plan"], "properties": {}}],  # no id
        "relationships": [],
    }
    # The importer's node-merge uses n.id as the MERGE key;
    # nodes with no id in properties produce incorrect MERGE semantics.
    # _merge_nodes must validate and skip or raise.
    ...  # assert mock driver's run() was NOT called, or ValueError raised
```

**`test_importer_empty_envelope_skips_stale_pruning`**

Verify that `_prune_stale_nodes` returns 0 immediately when the envelope has no nodes (safety gate: empty envelope must not trigger mass DETACH DELETE of the entire plan/bug graph).

**`test_importer_stale_pruning_removes_absent_nodes`**

Fixture: envelope with node id `"plan-foo"`; mock DB returns `["plan-foo", "plan-bar"]`. Assert that `_prune_stale_nodes` issues a DETACH DELETE for `"plan-bar"` but not `"plan-foo"`.

**`test_importer_multi_plan_with_supersedes_edges`**

Fixture envelope: 2 Plan nodes + 1 SUPERSEDES edge. Assert Phase 2 issues a MERGE for the SUPERSEDES edge type.

**`test_importer_declared_mentions_code_creates_code_reference`**

Fixture: 1 PlanSection node with `touches_raw: ["_resolve_target_py"]`; mock `_resolve_target_py` returns `("import_code_graph::_resolve_target_py", "abc123")`. Assert Phase 3 creates a CodeReference node with `mention_kind="declared"` and `resolved=True`.

**`test_importer_inferred_mention_backtick_scrape`**

Fixture: 1 PlanSection node with `body_preview` containing `` `check_exhaustiveness` ``. Assert Phase 3 creates a CodeReference with `mention_kind="inferred"`.

**`test_importer_dry_run_produces_no_writes`**

Run `_merge_nodes(driver, envelope, dry_run=True)`. Assert mock driver's `session().execute_write()` was NOT called.

**`test_importer_merges_structural_edges`**

Fixture envelope with one each of HAS_SECTION, HAS_SUBSECTION, HAS_BUG, FIXED_BY, RESOLVES, BLOCKED_BY, REFERENCES edges. Assert Phase 2 issues 7 separate MERGE operations (one per type).

### Task breakdown

- [ ] Create `~/projects/lang_intelligence/tests/test_import_plan_bug_graph.py` with:
  - [ ] `_import_module()` with `importlib.util.spec_from_file_location` workaround (exact pattern from `test_import_code_graph.py:21-47`)
  - [ ] `_skip_if_no_module()` guard
  - [ ] All tests listed in the matrix above (node label dimensions + edge type dimensions)
- [ ] All tests pass: `cd /tmp && python3 -m pytest ~/projects/lang_intelligence/tests/test_import_plan_bug_graph.py -v`
- [ ] No regression in `test_import_code_graph.py`: `cd /tmp && python3 -m pytest ~/projects/lang_intelligence/tests/test_import_code_graph.py -v`

- [ ] **Subsection close-out (02.4)** — MANDATORY before starting 02.R:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check`.

---

## 02.N Completion Checklist

- [ ] `~/projects/lang_intelligence/neo4j/schema.cypher` has 9 new `CREATE CONSTRAINT ... IF NOT EXISTS` statements (plan_name, plan_section_id, subsection_id, bug_id, fix_section_id, bug_tracker_section_id, overview_plan, roadmap_section_id, completed_index_name)
- [ ] `schema.cypher` has 5 new `CREATE INDEX ... IF NOT EXISTS` statements (plan_status, plan_section_status, bug_severity, bug_status, fix_section_status)
- [ ] `schema.cypher` has 2 new `CREATE FULLTEXT INDEX ... IF NOT EXISTS` statements (plan_text, bug_text) matching existing Lucene-backed fulltext pattern
- [ ] `cypher-shell -u neo4j -p intelligence < ~/projects/lang_intelligence/neo4j/schema.cypher` run twice: second run produces zero schema changes (idempotent)
- [ ] `~/projects/lang_intelligence/neo4j/import_plan_bug_graph.py` exists, reads from `--input <path>` or `--input -`, validates `schema_version == "1.0"`, runs three phases
- [ ] Phase 1 (node MERGE) uses UNWIND batching (~1000/batch); APOC path and fallback both implemented and documented
- [ ] Phase 2 (structural edge MERGE) uses UNWIND batching over the finite `KNOWN_REL_TYPES` set; skips `MENTIONS_CODE`; APOC path and fallback both documented
- [ ] Stale-pruning: `_prune_stale_nodes` has the empty-envelope safety gate; issues DETACH DELETE only when incoming IDs > 0
- [ ] `NEO4J_URI`, `NEO4J_USER`, `NEO4J_PASS` read from `os.environ.get(...)` — no hardcoded credentials
- [ ] `_retry_tx`, `_retry_write`, `_resolve_target_py`, `_build_symbol_index` imported from `import_code_graph.py` via `importlib.util.spec_from_file_location`; not forked
- [ ] Phase 3 (CodeReference bridge) resolves `touches_raw` declared mentions and backtick-inferred mentions; creates `CodeReference` nodes with correct `mention_kind`, `confidence`, `resolved`, `ambiguous`, `ambiguous_count`
- [ ] `import_plan_bug_graph.py` stays under 500 lines: `wc -l ~/projects/lang_intelligence/neo4j/import_plan_bug_graph.py` ≤ 500
- [ ] `python ~/projects/lang_intelligence/neo4j/import_plan_bug_graph.py --input <fixture> --dry-run` prints expected operation plan without writes
- [ ] End-to-end smoke: `python -m scripts.plan_corpus export plans/plan-bug-dag-ingestion/ | python ~/projects/lang_intelligence/neo4j/import_plan_bug_graph.py --input -` succeeds without error
- [ ] `scripts/intel-query.sh cypher "MATCH (p:PlanSection)-[:MENTIONS_CODE]->(cr)-[:RESOLVES_TO]->(s:Symbol) RETURN count(DISTINCT s) > 0"` returns true after the smoke import
- [ ] `cd /tmp && python3 -m pytest ~/projects/lang_intelligence/tests/test_import_code_graph.py -v` green (no regression)
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] This section's frontmatter `status` → `complete`, all subsection statuses → `complete`
  - [ ] `00-overview.md` Quick Reference table: Section 02 status → `Complete`
  - [ ] `00-overview.md` mission success criteria: check off criterion 1 ("Neo4j schema.cypher declares typed node labels...") and criterion 4 ("~/projects/lang_intelligence/neo4j/import_plan_bug_graph.py consumes the JSON envelope...") and criterion 5 ("Plan/bug nodes are joined to code symbols...")
  - [ ] `index.md` Section 02 status → `Complete`
  - [ ] Section 03's `depends_on: ["02"]` is correct — §03 sync wrapper calls the §02 importer; no stale assumptions
- [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check`; clean any temp files before final commit.

**Exit Criteria:** `cypher-shell < schema.cypher` idempotent (second run: 0 schema changes); `pytest tests/test_import_plan_bug_graph.py` green (all 9 label dimensions + 10 edge type dimensions + semantic + negative pins covered); end-to-end smoke `python -m scripts.plan_corpus export plans/plan-bug-dag-ingestion/ | python import_plan_bug_graph.py --input -` succeeds; `intel-query.sh cypher "MATCH (p:PlanSection)-[:MENTIONS_CODE]->(cr)-[:RESOLVES_TO]->(s) RETURN count(DISTINCT s) > 0"` returns true; `import_plan_bug_graph.py` ≤ 500 lines; `./test-all.sh` green (§02 adds only Python + Cypher — zero Rust impact expected).
