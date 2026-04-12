---
section: "07"
title: "Code Graph: Neo4j Import Pipeline"
status: not-started
reviewed: false
goal: "Load extracted symbol and relationship records into Neo4j, extending the schema with code graph nodes and creating the structural graph alongside the existing issue graph."
success_criteria:
  - "Neo4j schema extended with Revision, File, Symbol, Occurrence, Implementation nodes"
  - "import_code_graph.py loads JSONL from extract_symbols.py into Neo4j"
  - "Batch imports: 5K-20K records per transaction for performance"
  - "Full-text indexes on Symbol.name, Symbol.qualified_name, File.path"
  - "All reference repos' code graphs loadable within 10 minutes total"
  - "Code graph queryable alongside issue graph (same Neo4j instance)"
depends_on: ["06"]
third_party_review:
  status: none
  updated: null
---

# 07 Code Graph: Neo4j Import Pipeline

## 07.0 Goal

Take the JSONL output from Section 06 and load it into the same Neo4j instance that already contains the issue graph. The result: one unified graph where code structure coexists with issue discussions, linked by shared repo identifiers and ready for the bridge layer (Section 08).

## 07.1 Schema Extension

**File**: `~/projects/lang_intelligence/neo4j/schema.cypher`

Add code graph constraints and indexes:

```cypher
// Code graph nodes
CREATE CONSTRAINT revision_key IF NOT EXISTS FOR (r:Revision) REQUIRE (r.repo, r.commit_sha) IS UNIQUE;
CREATE CONSTRAINT file_key IF NOT EXISTS FOR (f:File) REQUIRE (f.repo, f.path) IS UNIQUE;
CREATE CONSTRAINT symbol_key IF NOT EXISTS FOR (s:Symbol) REQUIRE (s.repo, s.qualified_name) IS UNIQUE;

// Full-text indexes for code search
CREATE FULLTEXT INDEX symbol_text IF NOT EXISTS FOR (s:Symbol) ON EACH [s.name, s.qualified_name];
CREATE FULLTEXT INDEX file_text IF NOT EXISTS FOR (f:File) ON EACH [f.path];

// Performance indexes
CREATE INDEX symbol_kind IF NOT EXISTS FOR (s:Symbol) ON (s.kind);
CREATE INDEX symbol_repo IF NOT EXISTS FOR (s:Symbol) ON (s.repo);
CREATE INDEX file_repo IF NOT EXISTS FOR (f:File) ON (f.repo);
```

- [ ] Extend `schema.cypher` with code graph constraints and indexes
- [ ] Apply to running Neo4j instance
- [ ] Verify constraints don't conflict with existing issue graph schema

### Subsection 07.1 close-out
**`/improve-tooling` retrospective**: Any schema issues? Index creation time on existing data?

---

## 07.2 Import Script

**File**: `~/projects/lang_intelligence/neo4j/import_code_graph.py`

**Contract**:
```
Usage: python3 neo4j/import_code_graph.py <repo_name> <symbols.jsonl>
Reads JSONL from extract_symbols.py
Loads into Neo4j with batch transactions
```

**Implementation**:
- [ ] Read JSONL line by line (streaming, not loading entire file into memory)
- [ ] Batch symbol records into transactions of 5K-20K records
- [ ] Create Revision node (keyed by repo + HEAD commit sha)
- [ ] Create File nodes (keyed by repo + path)
- [ ] Create Symbol nodes with all properties (kind, language, qualified_name, line, signature_hash)
- [ ] Create relationships: CALLS, IMPORTS, IMPLEMENTS, DECLARES (File→Symbol), HAS_FILE (Revision→File)
- [ ] Use MERGE for idempotency (re-importing updates, doesn't duplicate)
- [ ] Report: symbols imported, relationships created, time taken
- [ ] Performance target: <30 seconds per repo, <10 minutes total for all reference repos

### Subsection 07.2 close-out
**`/improve-tooling` retrospective**: Was batch sizing appropriate? Any performance bottlenecks in the import? Should we use `CALL { ... } IN TRANSACTIONS` for better memory management?

---

## 07.3 Full Pipeline Script

**File**: `~/projects/lang_intelligence/scripts/build-code-graph.sh`

End-to-end: parse → extract → import for all repos:
```bash
for repo in $(yq '.repos | keys | .[]' repos.yaml); do
    python3 neo4j/extract_symbols.py $repo --output /tmp/$repo-symbols.jsonl
    python3 neo4j/import_code_graph.py $repo /tmp/$repo-symbols.jsonl
done
```

- [ ] Create the script with progress reporting per repo
- [ ] Add `--repo` flag for single-repo rebuild
- [ ] Add `--dry-run` flag that runs extraction but skips import
- [ ] Test: full pipeline for Rust (largest repo) completes in <3 minutes
- [ ] Test: full pipeline for all repos completes in <10 minutes

### Subsection 07.3 close-out
**`/improve-tooling` retrospective**: Is the pipeline script robust to failures? Should it have resume capability like the fetch scripts?

---

## 07.R Third Party Review Findings

- None.

## Completion Checklist

- [ ] Neo4j schema extended with code graph nodes and indexes
- [ ] `import_code_graph.py` loads JSONL into Neo4j with batch transactions
- [ ] `build-code-graph.sh` runs end-to-end for all repos
- [ ] Full pipeline completes in <10 minutes
- [ ] Code graph queryable: `MATCH (s:Symbol {kind: 'function'}) RETURN count(s)`
- [ ] `/tpr-review` clean
- [ ] `/impl-hygiene-review` clean
- [ ] `/improve-tooling` section-close sweep
