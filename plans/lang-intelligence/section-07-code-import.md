---
section: "07"
title: "Code Graph: Neo4j Import Pipeline"
status: in-progress
reviewed: true
goal: "Load extracted symbol and relationship records into Neo4j, extending the schema with code graph nodes and creating a unified structural graph connected to the existing issue graph via shared Repo nodes."
success_criteria:
  - "Neo4j schema extended with File, Symbol nodes and code graph indexes"
  - "import_code_graph.py loads JSONL from extract_symbols.py into Neo4j"
  - "Code graph linked to existing issue graph via (File)-[:IN_REPO]->(Repo) and (Symbol)-[:IN_REPO]->(Repo)"
  - "File-scoped wipe-and-replace prevents ghost symbols from accumulating"
  - "Unresolved relationship targets stored as UnresolvedSymbol stubs for later resolution"
  - "Batch imports with separate sizing for symbol vs relationship transactions"
  - "Full-text indexes on Symbol.name, Symbol.qualified_name, File.path"
  - "Exact composite index on (Symbol.repo, Symbol.name) for Section 08 resolution"
  - "All reference repos' code graphs loadable within 10 minutes total"
  - "Code graph queryable alongside issue graph (same Neo4j instance, shared Repo nodes)"
depends_on: ["06"]
sections:
  - id: "07.PRE"
    title: "Prerequisites"
    status: complete
  - id: "07.1"
    title: "Schema Extension"
    status: complete
  - id: "07.2"
    title: "Import Script"
    status: complete
  - id: "07.3"
    title: "Full Pipeline Script"
    status: complete
  - id: "07.4"
    title: "Verification Queries"
    status: complete
third_party_review:
  status: resolved
  updated: 2026-04-13
---

# 07 Code Graph: Neo4j Import Pipeline

## 07.0 Goal

Take the JSONL output from Section 06 and load it into the same Neo4j instance that already contains the issue graph. The result: one **unified** graph where code structure coexists with issue discussions, linked by shared `(:Repo)` nodes and ready for the bridge layer (Section 08).

**Design model: current-state graph.** The code graph represents the latest state of each repository, not a revisioned history. Each import replaces the previous state for that repo's files. This is sufficient for the intelligence use case (cross-language design queries, bug investigation, code reference resolution) and avoids the semantic incoherence of a revisioned model where globally-keyed File and Symbol nodes silently point at stale state.

**Rationale for dropping Revision nodes.** The original plan included `(:Revision)` keyed by `(repo, commit_sha)`. This creates a semantic contradiction: File and Symbol nodes are keyed by `(repo, path)` and `(repo, qualified_name)` respectively (global, not revision-scoped), but Revision links them to a specific point in time. On re-import, File and Symbol properties are mutated via MERGE, so old Revision nodes silently point at the new state. A correct revisioned model would require per-revision copies of all File and Symbol nodes -- quadratic storage, no query benefit for our use case. Instead, we store `last_imported_at` and `last_commit_sha` as properties on Repo, giving "when was this data from?" without phantom revision nodes.

**Downstream note for Section 09:** Section 09 (Ori Live Sync) depends on the import schema defined here, not just on Section 06's extraction format. Section 09's `depends_on` should include `"07"`. This file documents the contract; the Section 09 frontmatter update will happen when that section is reviewed.

---

## 07.PRE Prerequisites

- [x] Verify `neo4j` Python driver is in `requirements.txt` at `~/projects/lang_intelligence/requirements.txt` -- add `neo4j>=5.0` if missing
- [x] Verify Neo4j container is running and healthy: `docker compose -f ~/projects/lang_intelligence/docker-compose.yml ps`
- [x] Verify Section 06 is complete and `extract_symbols.py` produces valid JSONL for at least one repo (gleam: 6885 symbols, 29351 relationships)
- [x] Verify existing issue graph data is present (10 Repo nodes: elm, gleam, go, koka, lean4, roc, rust, swift, typescript, zig)

### Subsection 07.PRE close-out
Confirm all prerequisites pass before proceeding to 07.1.

---

## 07.1 Schema Extension

**File**: `~/projects/lang_intelligence/neo4j/schema.cypher`

Extend the existing issue graph schema with code graph constraints and indexes. The code graph must share the existing `(:Repo)` nodes -- no parallel repo tracking.

```cypher
// ─────────────────────────────────────────────
// Code Graph: Constraints
// ─────────────────────────────────────────────

// File nodes keyed by (repo, path). Current-state: one File per repo+path.
CREATE CONSTRAINT file_key IF NOT EXISTS FOR (f:File) REQUIRE (f.repo, f.path) IS UNIQUE;

// Symbol nodes keyed by (repo, qualified_name, signature_hash).
// signature_hash in the key handles function overloads (C++, Swift) that
// produce identical qualified_names but different signatures.
CREATE CONSTRAINT symbol_key IF NOT EXISTS FOR (s:Symbol) REQUIRE (s.repo, s.qualified_name, s.signature_hash) IS UNIQUE;

// UnresolvedSymbol stubs: raw target identifiers from relationship records
// that could not be resolved to a qualified_name at import time.
CREATE CONSTRAINT unresolved_symbol_key IF NOT EXISTS FOR (u:UnresolvedSymbol) REQUIRE (u.repo, u.target_identifier) IS UNIQUE;

// ─────────────────────────────────────────────
// Code Graph: Full-text indexes (Lucene-backed)
// ─────────────────────────────────────────────

CREATE FULLTEXT INDEX symbol_text IF NOT EXISTS FOR (s:Symbol) ON EACH [s.name, s.qualified_name];
CREATE FULLTEXT INDEX file_text IF NOT EXISTS FOR (f:File) ON EACH [f.path];

// ─────────────────────────────────────────────
// Code Graph: Performance indexes
// ─────────────────────────────────────────────

CREATE INDEX symbol_kind IF NOT EXISTS FOR (s:Symbol) ON (s.kind);
CREATE INDEX symbol_repo IF NOT EXISTS FOR (s:Symbol) ON (s.repo);
CREATE INDEX file_repo IF NOT EXISTS FOR (f:File) ON (f.repo);

// Exact composite index for Section 08 code reference resolution.
// Section 08 resolves backticked identifiers like `check_exhaustiveness`
// by exact-matching Symbol.name within a repo. The fulltext index is
// Lucene-backed (tokenized, stemmed) and not suitable for exact matching.
CREATE INDEX symbol_repo_name IF NOT EXISTS FOR (s:Symbol) ON (s.repo, s.name);
```

**Node types and properties:**

```
(:File {repo, path, language, had_error, coverage_status, content_hash, last_imported_at})
  - repo: string — matches Repo.name
  - path: string — relative path within repo (e.g., "compiler/rustc_parse/src/parser/expr.rs")
  - language: string — language_id from extraction
  - had_error: boolean — whether tree-sitter had parse errors
  - coverage_status: string — "full" | "partial" | "error"
  - content_hash: string — SHA-256 of entire file content (from ParseResult); enables Section 09 fast skip
  - last_imported_at: datetime — when this file was last imported

(:Symbol {repo, name, qualified_name, kind, language, language_kind,
          file, line, end_line, visibility, signature_hash, content_hash,
          had_error, coverage_status})
  - repo: string — matches Repo.name
  - name: string — unqualified symbol name
  - qualified_name: string — fully qualified name from Section 06
  - kind: string — normalized kind (function, method, type, trait_like, etc.)
  - language: string — language_id
  - language_kind: string — tree-sitter node type (function_item, etc.)
  - file: string — relative path to containing file
  - line: int — 1-based start line
  - end_line: int — 1-based end line
  - visibility: string — "pub" or ""
  - signature_hash: string — 16-char hex body-independent hash from Section 06
  - content_hash: string — SHA-256 of node's byte range (body-inclusive); enables Section 09 per-symbol incremental diffing
  - had_error: boolean — parse error flag from source file
  - coverage_status: string — parse coverage status

(:UnresolvedSymbol {repo, target_identifier})
  - repo: string — matches Repo.name
  - target_identifier: string — raw unresolved name from relationship records
```

**Relationship types:**

```
(File)-[:IN_REPO]->(Repo)         — links code graph to existing issue graph
(Symbol)-[:IN_REPO]->(Repo)       — links code graph to existing issue graph
(File)-[:DECLARES]->(Symbol)      — file declares symbol
(Symbol)-[:CALLS]->(Symbol|UnresolvedSymbol)
(Symbol)-[:IMPORTS]->(Symbol|UnresolvedSymbol)
(Symbol)-[:IMPLEMENTS]->(Symbol|UnresolvedSymbol)
```

**Checklist:**

- [x] Add code graph constraints to `schema.cypher` (File, Symbol, UnresolvedSymbol)
- [x] Add fulltext indexes (symbol_text, file_text)
- [x] Add performance indexes (symbol_kind, symbol_repo, file_repo, symbol_repo_name)
- [x] Add `neo4j>=5.0` to `~/projects/lang_intelligence/requirements.txt`
- [x] Apply updated schema to running Neo4j (22 statements, all OK)
- [x] Verify constraints don't conflict with existing issue graph schema (8 constraints, 24 indexes, no collisions)
- [x] Verify existing `(:Repo)` nodes are present (10 repos)

### Subsection 07.1 close-out
**`/improve-tooling` retrospective**: Any schema issues? Index creation time on existing data? Constraint name conflicts?

---

## 07.2 Import Script

**File**: `~/projects/lang_intelligence/neo4j/import_code_graph.py`

**Contract**:
```
Usage: python3 neo4j/import_code_graph.py <repo_name> <symbols.jsonl>
Reads JSONL from extract_symbols.py (Section 06 output)
Loads into Neo4j with batch transactions
Links to existing (:Repo) nodes from the issue graph
```

**Architecture decisions:**

### 07.2.1 Current-State Import Strategy (Declarative File-Scoped Diff)

MERGE-only import is insufficient: when files are refactored and symbols removed, MERGE inserts new symbols but leaves old ones orphaned ("ghost symbols" / graph rot). However, naive `DETACH DELETE` on all symbols in a file would destroy valid **incoming** edges from other files (e.g., file B's `CALLS` edge pointing at a function in file A). The import uses a **declarative diff** that preserves incoming edges:

1. Group incoming JSONL records by `file` path
2. For each file, within a single transaction:
   a. Delete **outgoing** edges from this file's current symbols: `MATCH (f:File {repo: $repo, path: $path})-[:DECLARES]->(s)-[r]->() DELETE r` (preserves incoming edges from other files)
   b. Compute the set of symbols **no longer present** in the incoming records (by qualified_name + signature_hash)
   c. `DETACH DELETE` only those stale symbols (safe: they have no outgoing edges after step a, and incoming edges to removed symbols are legitimately stale)
   d. MERGE remaining symbols with updated properties (in-place update, no edge loss)
   e. CREATE new symbols that didn't exist before
   f. Recreate DECLARES (File -> Symbol) and IN_REPO (File -> Repo, Symbol -> Repo) edges
3. Relationship edges (CALLS, IMPORTS, IMPLEMENTS) are created in a separate pass after all symbols are loaded

This ensures ghost symbols are cleaned up while preserving valid cross-file edges. The diff approach is compatible with Section 09's incremental live sync.

### 07.2.2 Relationship Resolution Strategy (Source and Target)

Section 06 produces relationship records with `source_qualified_name` and `target_identifier`. With the overload-safe key `(repo, qualified_name, signature_hash)`, both sides need resolution rules.

**Source resolution** (finding the source Symbol):
- Match by `repo + source_qualified_name + file + line range` — the relationship record includes `file` and `line`, so we can narrow to the symbol declared at that location
- If the file+line lookup yields exactly one symbol: use it
- If ambiguous (shouldn't happen in practice — source symbols are at known locations): pick the one whose line range contains the relationship's line

**Target resolution** (finding or stubbing the target Symbol):
1. First try: exact match `Symbol {repo: $repo, qualified_name: $target}` (works when target is already fully qualified)
2. Second try: exact match on `name` property using the `(repo, name)` composite index: `Symbol {repo: $repo, name: $target}` (O(1) lookup, handles unqualified references)
3. If **exactly one match**: use it
4. If **multiple matches** (ambiguous): create an `(:UnresolvedSymbol)` stub — do NOT pick arbitrarily. Ambiguous resolution writes false edges into the canonical graph, and nothing downstream re-resolves them. Preserve ambiguity for a dedicated resolution pass.
5. If **no match**: MERGE an `(:UnresolvedSymbol {repo: $repo, target_identifier: $target})` stub node
6. UnresolvedSymbol nodes carry a distinct label so they are never confused with real symbols in queries
7. Future resolution: Section 08 or a later pass can merge UnresolvedSymbol stubs into real Symbol nodes when cross-file analysis resolves the target

### 07.2.3 content_hash: File-Level and Symbol-Level

Section 09 needs incremental diffing at two granularities: **file-level** (has anything in this file changed?) and **symbol-level** (which specific symbols changed?).

**File-level content_hash** — `ParseResult.content_hash` from `parser_adapter.py` is a SHA-256 of the entire file content. Store this on the `(:File)` node as `content_hash`. Section 09 uses it for fast "skip this file entirely" checks. This requires adding `content_hash` to the JSONL output — `extract_symbols.py` has access to it via the `ParseResult` but doesn't currently emit it. Add a file-level record or include it on each symbol record and deduplicate at import time.

**Symbol-level content_hash** — `signature_hash` (already emitted by Section 06) is body-independent. For Section 09's fine-grained incremental diffing, we also need a body-inclusive hash. Add `content_hash` to each symbol record in `extract_symbols.py` by hashing the node's byte range: `hashlib.sha256(source_bytes[node.start_byte:node.end_byte]).hexdigest()[:16]`. This lets the importer detect which specific symbols changed content, even when the file-level hash changed.

**Action during 07.2 implementation**:
1. Add file-level `content_hash` to JSONL output (pass through from `ParseResult`)
2. Add symbol-level `content_hash` to each symbol record (hash the node's byte range)
3. Store file-level hash on `(:File)` node, symbol-level hash on `(:Symbol)` node
4. The File node's `content_hash` property enables Section 09's fast skip; the Symbol node's `content_hash` enables fine-grained diff

### 07.2.4 Batch Sizing and Transaction Management

The Neo4j instance has a 1GB heap cap (from `docker-compose.yml`). Batch sizing must account for this:

- **Symbol batches**: 5,000 records per transaction (symbols are property-heavy: ~15 properties each)
- **Relationship batches**: 10,000 records per transaction (relationships are lightweight: source + target + type)
- **File wipe-and-replace**: 1 transaction per file (atomic: wipe + re-insert must be in the same tx)
- **Retry with exponential backoff**: on transaction failure (OOM, deadlock, transient), retry up to 3 times with 1s/2s/4s delays
- **Progress reporting**: log every 100 files and every 10K relationships

### 07.2.5 Atomic File-Scoped Upsert Function

Section 09 (Ori Live Sync) needs sub-500ms incremental updates for single files. Rather than designing a separate incremental path, extract the file-scoped declarative diff logic into a reusable, **transaction-scoped** function that both the bulk importer and live sync can call:

```python
def upsert_file_symbols(
    driver,
    repo_name: str,
    file_path: str,
    symbols: list[dict],
) -> dict:
    """Atomic file-scoped upsert: diff old vs new symbols in a single transaction.

    Manages its own transaction internally (not session-based) for true atomicity.
    Does NOT handle outgoing CALLS/IMPORTS/IMPLEMENTS edges — those are created
    in a separate Phase 2 pass after all symbols are loaded (bulk import) or
    after the file upsert completes (live sync).

    Used by both bulk import (07.2) and live sync (09.2).
    Returns stats dict: {"symbols_created": N, "symbols_updated": N, "symbols_removed": N}.
    """
```

**Key design decisions:**
- Takes `driver` not `session` — manages its own transaction for true atomicity (no partial commits)
- Does NOT take `relationships` — relationship creation is always a separate pass. The bulk importer calls this with symbols only in Phase 1, then handles relationships in Phase 2. Section 09 calls this for a single file, then resolves that file's outgoing relationships separately.
- Uses the declarative diff from 07.2.1 (not naive DETACH DELETE) to preserve incoming edges from other files

The bulk importer calls it per-file in a loop; the live sync calls it for a single changed file.

### 07.2.6 Implementation Checklist

**Connection and setup:**
- [x] Use same connection pattern as `import_graph.py`: `GraphDatabase.driver("bolt://localhost:7687", auth=("neo4j", "intelligence"))`
- [x] Verify `(:Repo {name: $repo_name})` exists before importing; abort with clear error if not
- [x] Store `last_imported_at` on the Repo node after successful import

**File and symbol import (Phase 1 — nodes only, no relationships):**
- [x] Read JSONL line by line (streaming, not loading entire file into memory)
- [x] Group symbol records by `file` path (buffer in memory — one file at a time)
- [x] For each file group, call `upsert_file_symbols(driver, repo, path, symbols)` — declarative diff per 07.2.1, no relationships parameter
- [x] Include ALL Section 06 fields on Symbol nodes: `name`, `qualified_name`, `kind`, `language`, `language_kind`, `file`, `line`, `end_line`, `visibility`, `signature_hash`, `content_hash`, `had_error`, `coverage_status`
- [x] Store file-level `content_hash` (from ParseResult) on File nodes — added `file_meta` record to extract_symbols.py, consumed by import_code_graph.py
- [x] Set `last_imported_at` on File nodes

**Relationship import (Phase 2 — after ALL symbols are loaded):**
- [x] Collect relationship records during Phase 1 (buffer in memory)
- [x] Batch relationship creation: 10K per transaction
- [x] For each relationship: resolve source symbol by `repo + source_qualified_name + file + line range` (per 07.2.2)
- [x] For each relationship: resolve target by exact qualified_name match, then exact name match via (repo, name) index; keep ambiguous (multi-match) as UnresolvedSymbol (per 07.2.2)
- [x] Create CALLS/IMPORTS/IMPLEMENTS edges from source symbol to target (Symbol or UnresolvedSymbol)
- [x] For IMPLEMENTS records with `implementing_type`: include as a relationship property

**Error handling:**
- [x] Retry failed transactions up to 3 times with exponential backoff (1s, 2s, 4s)
- [x] Log and skip individual records that cause constraint violations (don't abort the whole import)
- [x] Report summary at end: files imported, symbols created, relationships created, unresolved targets, errors skipped

**Performance targets:**
- [x] <30 seconds per repo (9/10 repos pass; rust 41.2s exceeds due to 1,893 files × atomic upsert) — Optimized: pre-loaded Python symbol index for client-side resolution (eliminates per-record Cypher round-trips). Results: gleam 3.8s, elm 2.2s, koka 3.8s, lean4 1.4s, zig 10.3s, typescript 7.5s, roc 12.5s, swift 21s, go 26.2s, rust 41.2s. 45x speedup over original (gleam: 240s → 3.8s). Rust is inherently slower due to file count; further optimization possible via multi-file transaction batching but would break Section 09's atomic file-scoped reuse contract.
- [x] <10 minutes total for all reference repos — Full pipeline: 218s (3.6 minutes) for 10 repos. Well under 10-minute target.
- [x] Memory: streaming JSONL + per-file buffering keeps memory bounded regardless of repo size

### Subsection 07.2 close-out
**`/improve-tooling` retrospective**: Was batch sizing appropriate? Any OOM or deadlock issues? Is the wipe-and-replace fast enough? Should we use Neo4j's `CALL { ... } IN TRANSACTIONS` for better memory management? Is the `upsert_file_symbols()` function clean enough for Section 09 to reuse directly?

---

## 07.3 Full Pipeline Script

**File**: `~/projects/lang_intelligence/scripts/build-code-graph.sh`

End-to-end: extract -> import for all repos:
```bash
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Activate venv (lang_intelligence uses .venv, not venv)
source "$PROJECT_DIR/.venv/bin/activate"

# Derive repo names via Python to avoid yq dependency
REPOS=$(python3 -c "import yaml; print('\n'.join(yaml.safe_load(open('$PROJECT_DIR/repos.yaml')).keys()))")
TOTAL_START=$(date +%s)

for repo in $REPOS; do
    echo "=== $repo ==="
    REPO_START=$(date +%s)

    # Extract symbols to temp file
    JSONL="/tmp/${repo}-symbols.jsonl"
    python3 "$PROJECT_DIR/neo4j/extract_symbols.py" "$repo" --output "$JSONL" --stats

    # Import into Neo4j
    python3 "$PROJECT_DIR/neo4j/import_code_graph.py" "$repo" "$JSONL"

    # Cleanup temp file
    rm -f "$JSONL"

    REPO_END=$(date +%s)
    echo "  -> $repo completed in $((REPO_END - REPO_START))s"
done

TOTAL_END=$(date +%s)
echo "=== All repos completed in $((TOTAL_END - TOTAL_START))s ==="
```

- [x] Create the script with progress reporting per repo
- [x] Add `--repo <name>` flag for single-repo rebuild
- [x] Add `--dry-run` flag that runs extraction but skips import
- [x] Add `--skip-extract` flag that imports from existing JSONL (for re-import after schema changes)
- [x] Handle extraction failures gracefully: log error, continue to next repo
- [x] Test: full pipeline for Rust (largest repo) completes in <3 minutes — Verified: 57s total (16s extract + 41s import). Well under 3 minutes.
- [x] Test: full pipeline for all repos completes in <10 minutes — Verified: 218s (3.6 min) for 10 repos. Also fixed repos.yaml parsing bug (flat dict, not nested under `repos` key).

### Subsection 07.3 close-out
**`/improve-tooling` retrospective**: Is the pipeline script robust to failures? Should it have resume capability like the fetch scripts? Is the temp file approach clean enough or should we pipe directly?

---

## 07.4 Verification Queries

After the full pipeline completes, run these sanity-check queries to verify the imported graph is correct and connected:

```cypher
// 1. Node counts per repo
MATCH (s:Symbol) RETURN s.repo AS repo, count(s) AS symbols ORDER BY symbols DESC;
MATCH (f:File) RETURN f.repo AS repo, count(f) AS files ORDER BY files DESC;
MATCH (u:UnresolvedSymbol) RETURN u.repo AS repo, count(u) AS unresolved ORDER BY unresolved DESC;

// 2. Relationship counts
MATCH ()-[r:CALLS]->() RETURN count(r) AS calls;
MATCH ()-[r:IMPORTS]->() RETURN count(r) AS imports;
MATCH ()-[r:IMPLEMENTS]->() RETURN count(r) AS implements;
MATCH ()-[r:DECLARES]->() RETURN count(r) AS declares;

// 3. Connectivity: code graph linked to issue graph via Repo nodes
MATCH (f:File)-[:IN_REPO]->(r:Repo)<-[:IN_REPO]-(i:Issue)
RETURN r.name AS repo, count(DISTINCT f) AS files, count(DISTINCT i) AS issues
ORDER BY repo;

// 4. No orphan files (every File links to a Repo)
MATCH (f:File) WHERE NOT (f)-[:IN_REPO]->(:Repo) RETURN count(f) AS orphan_files;
// Expected: 0

// 5. No orphan symbols (every Symbol links to a File via DECLARES)
MATCH (s:Symbol) WHERE NOT (:File)-[:DECLARES]->(s) RETURN count(s) AS orphan_symbols;
// Expected: 0

// 6. Sample symbol query (proves fulltext search works)
CALL db.index.fulltext.queryNodes("symbol_text", "parse_expr") YIELD node
RETURN node.repo, node.qualified_name, node.kind LIMIT 10;

// 7. Sample cross-graph query (the killer query Section 08 will build on)
MATCH (s:Symbol {kind: 'function'})-[:IN_REPO]->(r:Repo)<-[:IN_REPO]-(i:Issue)
WHERE s.name CONTAINS 'exhaustive' AND i.title CONTAINS 'exhaustive'
RETURN r.name AS repo, s.qualified_name AS symbol, i.number AS issue, i.title
LIMIT 20;
```

- [x] Run all verification queries after full pipeline import — all 7 queries pass (2026-04-13)
- [x] Assert: zero orphan files, zero orphan symbols — verified: 0 orphan files, 0 orphan symbols
- [x] Assert: every repo with issue data also has code graph data (Repo nodes are shared) — verified: all 10 repos have both File and Issue nodes linked to shared Repo nodes
- [x] Assert: fulltext search returns results for known function names — verified: `parse_expr` found in rust (2 hits) and roc (1 hit)
- [x] Document expected node/relationship counts per repo as a baseline for regression detection:
  - **Symbols**: rust 34,439 | go 29,135 | swift 28,619 | zig 24,618 | roc 13,976 | ts 13,760 | gleam 6,844 | koka 4,083 | lean4 1,708 | elm 1,684 — **total: 158,866**
  - **Files**: rust 1,893 | go 1,165 | swift 1,117 | roc 479 | ts 206 | gleam 188 | zig 166 | koka 119 | elm 95 | lean4 76 — **total: 5,504**
  - **Relationships**: CALLS 417,295 | DECLARES 160,243 | IMPORTS 13,442 | IMPLEMENTS 2,313
  - **UnresolvedSymbols**: rust 12,956 | roc 5,526 | koka 3,620 | zig 2,068 | go 1,839 | gleam 1,505 | elm 1,483 | ts 1,002 — **total: 29,999**
  - **Note**: swift and lean4 have 0 CALLS relationships due to Section 06 extraction quality mismatch (calls.scm source_qualified_names use different path format than decls.scm symbol qualified_names in mixed C++/Swift repos)

### Subsection 07.4 close-out
**`/improve-tooling` retrospective**: Should these queries be automated into a `verify-code-graph.sh` script?

---

## 07.R Third Party Review Findings

- [x] `[TPR-07-001-codex][high]` `section-07-code-import.md:269` — GAP: Define source-symbol lookup for overloaded relationship records.
  Resolved: Fixed on 2026-04-13. Added source-resolution rule using `repo + file + line range` to 07.2.2.
- [x] `[TPR-07-002-codex][high]` `section-07-code-import.md:207` — LEAK: Keep ambiguous targets unresolved instead of picking any match.
  Resolved: Fixed on 2026-04-13. Changed 07.2.2 to keep multi-match targets as UnresolvedSymbol, not "pick any".
- [x] `[TPR-07-003-codex][high]` `section-07-code-import.md:235` — DRIFT: Make the file refresh contract truly atomic for Section 09 reuse.
  Resolved: Fixed on 2026-04-13. Rewrote 07.2.5 to take `driver` not `session`, manage own transaction, exclude relationships parameter.
- [x] `[TPR-07-004-codex][medium]` `section-07-code-import.md:214` — DRIFT: content_hash is file-level in ParseResult, not per-symbol.
  Resolved: Fixed on 2026-04-13. Rewrote 07.2.3 with two-level hashing: file-level on File node, per-symbol content_hash on Symbol node.
- [x] `[TPR-07-005-codex][medium]` `section-07-code-import.md:301` — DRIFT: venv vs .venv, yq dependency.
  Resolved: Fixed on 2026-04-13. Updated script to use `.venv`, replaced `yq` with Python yaml parsing.
- [x] `[TPR-07-001-gemini][high]` `section-07-code-import.md:83` — DETACH DELETE destroys incoming edges from other files.
  Resolved: Fixed on 2026-04-13. Rewrote 07.2.1 to use declarative diff — delete outgoing edges first, then only stale symbols.
- [x] `[TPR-07-002-gemini][high]` `section-07-code-import.md:126` — File-level content_hash defeats per-symbol incremental diffing.
  Resolved: Fixed on 2026-04-13. Same fix as TPR-07-004-codex — two-level hashing.
- [x] `[TPR-07-003-gemini][high]` `section-07-code-import.md:111` — O(N) suffix match for target resolution.
  Resolved: Fixed on 2026-04-13. Changed 07.2.2 second-try from suffix match to exact name match using (repo, name) index.
- [x] `[TPR-07-004-gemini][medium]` `section-07-code-import.md:175` — Phase 1 should not pass relationships to upsert.
  Resolved: Fixed on 2026-04-13. Clarified 07.2.5 and 07.2.6 Phase 1 — upsert takes symbols only, no relationships parameter.
- [x] `[TPR-07-006-codex][medium]` `import_code_graph.py:329` — GAP: Files with file_meta but no symbols don't get File nodes.
  Resolved: Fixed on 2026-04-13. Added symbolless file upsert pass after main loop; upsert_file_symbols now accepts file_meta param for language/coverage metadata.
- [x] `[TPR-07-007-codex][medium]` `build-code-graph.sh:45` — DRIFT: Script includes custom-only repos (ori) that lack Repo nodes in Neo4j.
  Resolved: Fixed on 2026-04-13. Added Neo4j Repo node filtering — all-repos mode now queries Neo4j for existing Repo nodes and skips repos not in the issue graph.
- [x] `[TPR-07-008-codex][medium]` `import_code_graph.py:423` — DRIFT: Phase 2 writes lack retry wrapper.
  Resolved: Fixed on 2026-04-13. Added _retry_write() helper; all Phase 2 Neo4j writes (UnresolvedSymbol stubs, relationship batches) now use retry with exponential backoff.
- [x] `[TPR-07-009-codex][low]` `section-07-code-import.md:303` — DRIFT: Per-repo <30s target checked off but Rust exceeds it.
  Resolved: Fixed on 2026-04-13. Reworded target to honestly note 9/10 repos pass; Rust exceeds due to file count with explanation.

## 07.C Completion Checklist

- [x] Neo4j schema extended with code graph nodes, constraints, and indexes (including exact `(repo, name)` index for Section 08) — 3 uniqueness constraints, 7 range indexes, 2 fulltext indexes
- [x] `import_code_graph.py` loads JSONL into Neo4j with file-scoped wipe-and-replace — optimized with UNWIND batching and pre-loaded Python symbol index
- [x] `upsert_file_symbols()` function extracted and documented for Section 09 reuse — manages own transaction, takes driver not session, no relationships param
- [x] Unresolved relationship targets handled via UnresolvedSymbol stub nodes — 29,999 across 8 repos
- [x] Code graph connected to issue graph via shared `(:Repo)` nodes — all 10 repos verified
- [x] `content_hash` propagated from ParseResult through extract_symbols.py into Symbol nodes — 158,866 symbols and 5,504 files with content_hash
- [x] `build-code-graph.sh` runs end-to-end for all repos — 10/10 repos succeed (ori skipped: no Repo node)
- [x] Full pipeline completes in <10 minutes — 218s (3.6 min) for 10 repos
- [x] Verification queries pass: zero orphans, cross-graph connectivity confirmed — all 7 queries pass
- [x] Code graph queryable: `MATCH (s:Symbol {kind: 'function'}) RETURN count(s)` returns expected counts — 98,499 function symbols
- [x] **Plan sync**: verify Section 09 `depends_on` includes `"07"` (not just `"06"`) — updated section-09 frontmatter
- [x] **Plan sync**: verify `requirements.txt` includes `neo4j>=5.0` — confirmed present
- [ ] `/tpr-review` clean
- [ ] `/impl-hygiene-review` clean
- [ ] `/improve-tooling` section-close sweep
