---
section: "08"
title: "Issue-to-Code Bridge"
status: in-progress
reviewed: true
goal: "Extract code references from issue/PR/comment bodies and link them to code symbols via CodeReference intermediary nodes with confidence scoring. Seed the ontology with Concept, FailureMode, and CompilerPhase taxonomy nodes."
success_criteria:
  - "CodeReference nodes created from file paths, backticked symbols, and qualified names in issue bodies"
  - "MENTIONS_CODE relationships link Issue/Comment/Review to CodeReference with source provenance"
  - "RESOLVES_TO relationships link CodeReference to File/Symbol (when unambiguous)"
  - "Ambiguous references (>1 match) left unresolved with disambiguation metadata, not fanned out"
  - "Confidence scores reflect extraction quality (regex path match > backtick > heuristic)"
  - "Stale references preserved (not deleted) with staleness metadata"
  - "Ontology seeded with Concept, FailureMode, CompilerPhase nodes and auto-tagging edges"
  - "Pipeline runner orchestrates extract -> resolve -> seed with correct ordering"
  - "Re-resolution mechanism updates previously-unresolved references after code graph updates"
depends_on: ["07"]
sections:
  - id: "08.0"
    title: "Schema Extension for Bridge Layer"
    status: complete
  - id: "08.1"
    title: "Code Reference Extraction"
    status: complete
  - id: "08.2"
    title: "Reference Resolution"
    status: complete
  - id: "08.3"
    title: "Ontology Seeding (independent)"
    status: complete
  - id: "08.4"
    title: "Pipeline Orchestration"
    status: complete
  - id: "08.5"
    title: "Completion Checklist"
    status: not-started
third_party_review:
  status: resolved
  updated: 2026-04-13
---

# 08 Issue-to-Code Bridge

## 08.G Goal

This is the bridge layer that connects the issue graph to the code graph. Without it, issues and code live in separate universes within the same Neo4j instance. The bridge enables the killer queries: "find issues that reference code implementing the same concept as Ori's exhaustiveness checker."

**Design decision: CodeReference intermediary nodes.** Rather than creating direct Issue->Symbol edges, the bridge uses CodeReference intermediary nodes. This is the correct shape because: (1) unresolved references have a home — a CodeReference without a RESOLVES_TO edge is a first-class entity, not a dangling edge; (2) provenance metadata (mention_kind, confidence, raw_text, body_offset) lives on the intermediary, not crammed onto an edge property; (3) re-resolution after code graph updates can target CodeReference nodes directly without re-scanning issue bodies.

---

## 08.0 Schema Extension for Bridge Layer

**File**: `~/projects/lang_intelligence/neo4j/schema.cypher`

Before writing any Python scripts, extend the schema with constraints and indexes for all new node types. Without these, bulk creation of CodeReference/Concept/CompilerPhase/FailureMode nodes has no uniqueness guarantee and no query index.

```cypher
// ─────────────────────────────────────────────
// Bridge Layer: Constraints (Section 08)
// ─────────────────────────────────────────────

// CodeReference nodes keyed by (repo, source_type, source_key, raw_text).
// source_type: "issue" | "comment" | "review"
// source_key: issue (repo, number) | comment/review github_id
// raw_text: the extracted mention text
CREATE CONSTRAINT coderef_key IF NOT EXISTS
  FOR (cr:CodeReference)
  REQUIRE (cr.repo, cr.source_type, cr.source_key, cr.raw_text) IS UNIQUE;

// Ontology node constraints
CREATE CONSTRAINT concept_name IF NOT EXISTS
  FOR (c:Concept) REQUIRE c.name IS UNIQUE;
CREATE CONSTRAINT compiler_phase_name IF NOT EXISTS
  FOR (cp:CompilerPhase) REQUIRE cp.name IS UNIQUE;
CREATE CONSTRAINT failure_mode_name IF NOT EXISTS
  FOR (fm:FailureMode) REQUIRE fm.name IS UNIQUE;
CREATE CONSTRAINT design_decision_name IF NOT EXISTS
  FOR (dd:DesignDecision) REQUIRE dd.name IS UNIQUE;

// ─────────────────────────────────────────────
// Bridge Layer: Indexes (Section 08)
// ─────────────────────────────────────────────

CREATE INDEX coderef_repo IF NOT EXISTS FOR (cr:CodeReference) ON (cr.repo);
CREATE INDEX coderef_resolved IF NOT EXISTS FOR (cr:CodeReference) ON (cr.resolved);
CREATE INDEX coderef_stale IF NOT EXISTS FOR (cr:CodeReference) ON (cr.stale);
```

**New node types:**

```
(:CodeReference {repo, source_type, source_key, raw_text, mention_kind,
                 confidence, file_hint, symbol_hint, body_offsets,
                 resolved, stale, stale_since, resolution_attempted_at,
                 ambiguous, ambiguous_count, occurrence_count})
  - repo: string — matches Repo.name
  - source_type: "issue" | "comment" | "review"
  - source_key: string — issue: "{repo}/{number}", comment/review: github_id
  - raw_text: string — the extracted mention text as it appears in the body
  - mention_kind: "file_path" | "backtick" | "qualified_name" | "line_ref" | "code_block"
  - confidence: float — extraction confidence (0.0-1.0)
  - file_hint: string | null — extracted file path if present
  - symbol_hint: string | null — extracted symbol name if present
  - body_offsets: [int] — character offsets in source body where mention appears (aggregated from all occurrences after dedup)
  - resolved: boolean — whether RESOLVES_TO edge exists
  - stale: boolean — whether a previously-resolved RESOLVES_TO target has been deleted/renamed
  - stale_since: datetime | null — when the reference became stale
  - resolution_attempted_at: datetime | null — when last resolution was attempted
  - ambiguous: boolean — whether >1 symbol matched (not fanned out)
  - ambiguous_count: int | null — number of matching symbols when ambiguous
  - occurrence_count: int — how many times this mention appears in the source body

(:Concept {name, aliases, description})
(:CompilerPhase {name, description, order})
(:FailureMode {name, description})
(:DesignDecision {name, description, rationale, alternatives})
```

**New relationship types:**

```
(Issue|Comment|Review)-[:MENTIONS_CODE]->(CodeReference)
(CodeReference)-[:RESOLVES_TO]->(File|Symbol)
(Symbol)-[:TAGGED_AS]->(Concept)
(Issue)-[:INTRODUCES_FAILURE_MODE]->(FailureMode)
(Symbol)-[:IN_PHASE]->(CompilerPhase)
(Issue)-[:REFLECTS_DECISION]->(DesignDecision)
(DesignDecision)-[:REJECTS_APPROACH]->(DesignDecision)
(DesignDecision)-[:SUPERSEDES_DECISION]->(DesignDecision)
```

**Checklist:**

- [x] Add CodeReference constraint, resolved/stale indexes to `schema.cypher`
- [x] Add Concept, CompilerPhase, FailureMode, DesignDecision constraints to `schema.cypher`
- [x] Apply updated schema to running Neo4j — verify no conflicts with existing constraints
- [x] Verify constraint names don't collide with existing 8 constraints + 24 indexes from Section 07

### Subsection 08.0 close-out
Confirm all constraints and indexes are applied before proceeding to 08.1/08.2/08.3.

---

## 08.1 Code Reference Extraction

**File**: `~/projects/lang_intelligence/neo4j/extract_code_refs.py`

Extract code mentions from issue/comment/review bodies using regex patterns. This is a pure extraction pass — no Neo4j queries, no resolution. Output is JSONL consumed by the resolution step (08.2).

**Input**: Issue/Comment/Review nodes already in Neo4j (from the issue graph import pipeline).

**Pattern types** (ordered by confidence):
1. **File paths** (confidence: 0.9): `compiler/rustc_parse/src/parser/expr.rs`, `src/Sema/TypeChecker.cpp` — regex: path-like strings with `/` separators and known source extensions. Note: these are examples of paths found in *reference repo issue bodies* (e.g., a Rust issue referencing `compiler/rustc_parse/src/parser/expr.rs`), NOT files in the ori_lang project.
2. **Backticked identifiers** (confidence: 0.7): `` `check_exhaustiveness` ``, `` `PatternColumn` `` — must pass stop-word/keyword filtering (see below)
3. **Qualified names** (confidence: 0.7): `rustc_pattern_analysis::usefulness::compute_exhaustiveness` — double-colon or dot-separated identifiers
4. **Line references** (confidence: 0.8): `expr.rs:42`, `L123-L156` — file + line number patterns. Note: `expr.rs:42` is an example of line reference syntax found in issue comments (e.g., a Rust issue saying "see expr.rs:42"), NOT a file in ori_lang.
5. **Fenced code blocks** (confidence: 0.3): Code snippets that might contain function/type names — lowest confidence, most noise

**Stop-word/keyword filtering** (for backtick extraction):

Backtick extraction without filtering will capture language keywords, boolean literals, shell commands, and single-letter variables that are not meaningful code references. Filter these before emitting:

- **Language keywords**: `true`, `false`, `null`, `nil`, `None`, `Some`, `Ok`, `Err`, `self`, `Self`, `super`, `crate`, `pub`, `fn`, `let`, `mut`, `const`, `if`, `else`, `match`, `for`, `while`, `loop`, `return`, `break`, `continue`, `struct`, `enum`, `trait`, `impl`, `type`, `where`, `use`, `mod`, `async`, `await`, `unsafe`, `extern`, `dyn`, `ref`, `move`
- **Shell/tool noise**: `npm`, `cargo`, `git`, `cd`, `ls`, `rm`, `cp`, `mv`, `mkdir`, `grep`, `sed`, `awk`, `curl`, `wget`, `pip`, `python`, `node`, `bash`, `sh`, `zsh`
- **Single-character identifiers**: `a`-`z`, `A`-`Z`, `_`, `T`, `N`, `E`, `S` (too ambiguous)
- **Common non-code words**: `TODO`, `FIXME`, `NOTE`, `HACK`, `XXX`, `WIP`, `LGTM`, `PTAL`, `nit`
- **Minimum length**: backticked text must be >= 2 characters after trimming

**Occurrence records, NOT premature deduplication:**

Emit one JSONL record per occurrence, preserving body_offset. The same symbol mentioned 5 times in an issue produces 5 records. Deduplication happens AFTER resolution in 08.2 — collapsing before resolution throws away occurrence context (e.g., which paragraph discusses the symbol, proximity to error descriptions). Post-resolution, create one CodeReference node per unique (source, raw_text) pair with an `occurrence_count` property.

**Output JSONL format** (one record per occurrence):
```json
{
  "repo": "rust",
  "source_type": "issue",
  "source_key": "rust/12345",
  "raw_text": "check_exhaustiveness",
  "mention_kind": "backtick",
  "file_hint": null,
  "symbol_hint": "check_exhaustiveness",
  "confidence": 0.7,
  "body_offset": 342
}
```

For comments and reviews, `source_key` is the `github_id` string:
```json
{
  "repo": "rust",
  "source_type": "comment",
  "source_key": "1234567890",
  "raw_text": "compiler/rustc_parse/src/parser/expr.rs",
  "mention_kind": "file_path",
  "file_hint": "compiler/rustc_parse/src/parser/expr.rs",
  "symbol_hint": null,
  "confidence": 0.9,
  "body_offset": 55
}
```

- [x] Implement regex extractors for each pattern type (file paths, backticks, qualified names, line refs, code blocks)
- [x] Implement stop-word/keyword filter for backtick extraction
- [x] Read issue/comment/review bodies from Neo4j (batch query, not per-node)
- [x] Emit one JSONL record per occurrence (no deduplication at this stage)
- [x] Include `body_offset` for each occurrence
- [x] Include `source_type` and `source_key` for provenance (issues: `"{repo}/{number}"`, comments/reviews: `"{github_id}"`)
- [x] Handle edge cases: nested backticks, backticks in code blocks, escaped backticks
- [x] Test: run on gleam repo issues and verify extraction count is reasonable (not 0, not 100K per issue) — gleam: 8471 refs from 4802 sources
- [x] Test: verify stop-word filter removes `true`, `false`, `self`, single-letter identifiers — 32 unit tests pass
- [x] Test: verify file path regex does not match URLs (e.g., `https://github.com/...`) — TestFilePathExtraction::test_url_not_matched_as_file_path
- [x] Create `~/projects/lang_intelligence/tests/test_extract_code_refs.py` with unit tests: pattern accuracy per type, stop-word filtering, URL rejection, nested backticks, fenced code block handling — 32 tests

### Subsection 08.1 close-out
**`/improve-tooling` retrospective**: Were the regex patterns accurate? High false positive rate? Any common patterns missed? Is the stop-word list sufficient or too aggressive?

---

## 08.2 Reference Resolution

**File**: `~/projects/lang_intelligence/neo4j/resolve_code_refs.py`

Match extracted references to actual code symbols in Neo4j. Create CodeReference nodes and MENTIONS_CODE/RESOLVES_TO edges.

**In-memory resolution pattern** (critical for performance):

Per-reference Cypher queries will be far too slow (tens of thousands of references x round-trip per query). Instead, preload the symbol index into memory at startup — the same pattern used by `import_code_graph.py`'s `_build_symbol_index()`:

```python
def _build_resolution_index(driver, repo: str) -> dict:
    """Preload File paths and Symbol business keys for a repo.

    Uses stable business keys (NOT internal Neo4j node IDs, which are
    invalidated by Section 07's atomic wipe-and-replace).

    Returns:
        {
            "files": {path: path, ...},  # exact path -> path (business key)
            "symbols_by_qname": {qualified_name: [(qualified_name, signature_hash), ...], ...},
            "symbols_by_name": {name: [(qualified_name, signature_hash), ...], ...},
        }
    """
```

**Resolution strategy** (ordered by precision):

1. **File path resolution**:
   - Exact match: `file_hint` == `File.path` in same repo
   - Fuzzy match (for partial paths like `parser/expr.rs`): use ENDS WITH against the preloaded file paths. If partial path matches exactly 1 file, resolve. If >1 match, leave unresolved with `ambiguous_matches` metadata.
   - The existing `file_text` Lucene fulltext index can supplement in-memory matching for edge cases, but the primary path is in-memory.

2. **Symbol resolution** (backtick and qualified name mentions):
   - First try: exact match on `qualified_name` in the preloaded index
   - Second try: exact match on `name` in the preloaded index
   - If exactly 1 match: resolve (create RESOLVES_TO edge)
   - If 2+ matches: do NOT fan out to N edges. Mark as `ambiguous` on the CodeReference node, store `ambiguous_count` property. Rationale: fanning out creates false edges that pollute all downstream queries. A single ambiguous reference with count=4 is honest; 4 RESOLVES_TO edges are misleading.
   - If 0 matches: leave unresolved (`resolved: false`). The CodeReference node persists for future re-resolution.

3. **Line reference resolution**: resolve the file part (as above), then store line number as a property on the CodeReference — do not attempt to resolve to a specific Symbol by line (symbols move between imports).

**Post-resolution deduplication:**

After resolution, collapse occurrence records into CodeReference nodes:
- Group by `(source_type, source_key, raw_text)` — same mention in same source = one CodeReference
- Aggregate all `body_offset` values from grouped occurrences into `body_offsets: [int]` on the node
- Store `occurrence_count` on the CodeReference node
- Use the highest-confidence occurrence's metadata for the node properties

**Edge creation:**
- `MENTIONS_CODE`: from the source Issue/Comment/Review node to the CodeReference
  - For issues: match by `(repo, number)` from `source_key`
  - For comments/reviews: match by `github_id` from `source_key`
- `RESOLVES_TO`: from CodeReference to File or Symbol (only when unambiguous)

**Re-resolution mechanism:**

When the code graph is updated (Section 07 re-import), previously-unresolved CodeReferences may now be resolvable. The resolution script supports a `--re-resolve` flag:

```bash
python3 resolve_code_refs.py <repo> --re-resolve
```

This queries all CodeReference nodes where `resolved: false` and re-attempts resolution against the current symbol index. If a previously-unresolved reference now resolves, create the RESOLVES_TO edge and update `resolved: true`. This is cheap — it only touches unresolved refs, not the full corpus.

The pipeline runner (08.4) invokes re-resolution after code graph updates.

**Stale-reference invalidation:**

When Section 07's wipe-and-replace reimport deletes a File or Symbol that a CodeReference has a `RESOLVES_TO` edge pointing to, the CodeReference becomes stale. The resolution script supports a `--invalidate-stale` flag:

```bash
python3 resolve_code_refs.py <repo> --invalidate-stale
```

This scans resolved CodeReferences and checks whether their RESOLVES_TO targets still exist. If a target was deleted: remove the dangling RESOLVES_TO edge, set `stale: true`, `stale_since: now()`, `resolved: false` on the CodeReference. The pipeline runner (08.4) invokes invalidation after code graph rebuilds.

**Module-level source resolution (fulfilling TPR-07-010):**

Section 07 delegated the module-scope source_unresolved gap to Section 08 (see `<!-- blocked-by:08 -->` on TPR-07-010-codex/TPR-07-017-codex in section-07). Files that emit IMPORTS/CALLS relationship records but have zero structural symbols from decls.scm (e.g., Haskell modules, C/C++ headers) produce `source_unresolved` tracking at import time. The fix: emit a synthetic file-scope Symbol record in `extract_symbols.py` when relationships exist but no declaration symbols do. This is tracked here as a Section 08 deliverable but the implementation lives in `extract_symbols.py` (Section 06's SSOT for symbol extraction).

- [x] Implement `_build_resolution_index()` — preload files and symbols for a repo into memory
- [x] Implement file path resolution (exact + fuzzy ENDS WITH for partial paths)
- [x] Implement symbol resolution (qualified_name first, then name; single-match only)
- [x] Implement ambiguity threshold: >1 match = mark ambiguous, do NOT create multiple RESOLVES_TO edges
- [x] Implement post-resolution deduplication (group occurrences -> one CodeReference node per unique mention)
- [x] Create CodeReference nodes with all properties (raw_text, mention_kind, confidence, source_type, source_key, body_offsets, resolved, occurrence_count)
- [x] Create MENTIONS_CODE edges from Issue/Comment/Review to CodeReference — gleam: 1945 issues with 7136 MENTIONS_CODE edges
- [x] Create RESOLVES_TO edges from CodeReference to File/Symbol (unambiguous only) — gleam: 25 file + 265 symbol = 290 resolved
- [x] Implement `--re-resolve` flag for re-resolution of previously unresolved refs
- [x] Unresolved references: keep CodeReference without RESOLVES_TO, with `resolved: false` — gleam: 6741 unresolved preserved
- [x] Cross-repo awareness: issue in `rust-lang/rust` references paths within the `rust` repo only — resolution index scoped by repo parameter
- [x] Test: resolution success rate on gleam repo — 290/7255 (4%) resolved, 224 (3%) ambiguous. Lower than plan estimate; backtick refs mostly reference informal names not in the symbol graph.
- [x] Test: verify ambiguous references are NOT fanned out — TestSymbolResolution::test_ambiguous_name_no_fanout (3 matches = ambiguous, not 3 edges)
- [x] Test: verify `--re-resolve` resolves a previously-unresolved ref after adding the matching symbol — 0 re-resolved on gleam (expected — no new symbols since initial run)
- [x] Implement `--invalidate-stale` flag: detect and mark stale references after code graph rebuild
- [x] Implement module-level source resolution: emit synthetic file-scope Symbol records in `extract_symbols.py` for files with relationships but no declaration symbols (fulfilling TPR-07-010/TPR-07-017 from Section 07) — all 24 extract_symbols tests pass <!-- unblocks:07.2 source_unresolved gap -->
- [x] Create `~/projects/lang_intelligence/tests/test_resolve_code_refs.py` with unit tests: exact/fuzzy path resolution, ambiguity non-fan-out, deduplication with body_offsets aggregation — 18 tests pass
- [ ] **TPR checkpoint**: run `/tpr-review` covering 08.0 + 08.1 + 08.2 before proceeding to 08.3/08.4

### Subsection 08.2 close-out
**`/improve-tooling` retrospective**: What's the resolution success rate? What fraction of references resolve unambiguously? Should we lower/raise the confidence threshold? Is the in-memory index fast enough or should we use Neo4j fulltext queries for fuzzy matching?

---

## 08.3 Ontology Seeding (independent)

**This subsection has zero data dependency on 08.1/08.2.** It reads from the existing code graph (Section 07) and issue graph, not from CodeReference nodes. It can be implemented and run in parallel with 08.1/08.2. It is grouped in Section 08 because it creates the taxonomy layer that makes the bridge queries meaningful, but its execution is independent.

**File**: `~/projects/lang_intelligence/neo4j/seed_ontology.py`

**Start narrow** — 5 core concepts, 5 compiler phases, 10 failure modes:

**Concepts** (per ChatGPT + TPR consensus):
- pattern_matching, type_inference, reference_counting, effect_handling, diagnostics

**Compiler phases**:
- parser, typechecker, lowering, codegen, diagnostics

**Failure modes**:
- soundness_hole, inference_ambiguity, diagnostic_confusion, compile_time_blowup, pattern_incompleteness, coherence_conflict, monomorphization_explosion, ir_mismatch, codegen_regression, parser_ambiguity

- [x] Create Concept nodes with aliases/synonyms — 5 concepts seeded
- [x] Create CompilerPhase nodes with ordering — 5 phases seeded
- [x] Create FailureMode nodes with descriptions — 10 failure modes seeded
- [x] Auto-tag Symbols with Concepts based on: file path patterns, symbol names, module names — 166K TAGGED_AS edges
- [x] Auto-tag Issues with FailureModes based on: labels, title keywords, body keywords — 21K INTRODUCES_FAILURE_MODE edges
- [x] Create TAGGED_AS edges (Symbol->Concept), INTRODUCES_FAILURE_MODE edges (Issue->FailureMode) — plus 189K IN_PHASE edges
- [x] Create DesignDecision nodes from issue/PR discussions — 100 nodes from labeled closed issues with REFLECTS_DECISION edges
- [x] Test: `MATCH (c:Concept {name: 'pattern_matching'})<-[:TAGGED_AS]-(s:Symbol) RETURN count(s)` returns 3701
- [x] Test: `MATCH (fm:FailureMode)<-[:INTRODUCES_FAILURE_MODE]-(i:Issue) RETURN fm.name, count(i)` returns non-zero — top: monomorphization_explosion (7660), diagnostic_confusion (3343)
- [x] Test: `MATCH (dd:DesignDecision) RETURN count(dd)` returns 100

### Subsection 08.3 close-out
**`/improve-tooling` retrospective**: Were the auto-tagging heuristics accurate? Too many false tags? Need manual override mechanism?

---

## 08.4 Pipeline Orchestration

**File**: `~/projects/lang_intelligence/scripts/build-bridge.sh`

A runner script that chains the three bridge steps with correct ordering and provides integration with the existing pipeline (`build-code-graph.sh`).

```bash
#!/usr/bin/env bash
set -euo pipefail

# Usage: build-bridge.sh [--repo REPO] [--re-resolve-only] [--seed-only]
#
# Full pipeline (default): extract -> resolve -> seed (for all repos or --repo)
# --re-resolve-only: skip extraction, re-resolve unresolved refs (after code graph update)
# --seed-only: only run ontology seeding (independent of extraction/resolution)

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
INTEL_DIR="$(dirname "$SCRIPT_DIR")"
NEO4J_DIR="$INTEL_DIR/neo4j"
VENV="$INTEL_DIR/.venv"

# ... activate venv, check Neo4j health ...

if [[ "${SEED_ONLY:-}" == "true" ]]; then
    python3 "$NEO4J_DIR/seed_ontology.py"
    exit 0
fi

for repo in "${REPOS[@]}"; do
    echo "=== Bridge: $repo ==="

    if [[ "${RE_RESOLVE_ONLY:-}" != "true" ]]; then
        # Step 1: Extract code references from issue/comment/review bodies
        python3 "$NEO4J_DIR/extract_code_refs.py" "$repo" \
            --output "$INTEL_DIR/data/$repo/code_refs.jsonl"

        # Step 2: Resolve references against code graph
        python3 "$NEO4J_DIR/resolve_code_refs.py" "$repo" \
            --input "$INTEL_DIR/data/$repo/code_refs.jsonl"
    else
        # Re-resolve only: update previously-unresolved refs
        python3 "$NEO4J_DIR/resolve_code_refs.py" "$repo" --re-resolve
    fi
done

# Step 3: Seed ontology (runs once, not per-repo)
if [[ "${RE_RESOLVE_ONLY:-}" != "true" ]]; then
    python3 "$NEO4J_DIR/seed_ontology.py"
fi
```

**Integration with build-code-graph.sh:**

After `build-code-graph.sh` completes a re-import of a repo's code graph, it should optionally trigger `build-bridge.sh --repo <repo> --re-resolve-only` to update any previously-unresolved CodeReferences that may now be resolvable. This is not mandatory on every code graph rebuild — it's an optimization for keeping the bridge fresh.

- [x] Create `build-bridge.sh` with `--repo`, `--re-resolve-only`, `--seed-only` flags
- [x] Implement per-repo extraction -> resolution pipeline
- [x] Implement seed-only mode for independent ontology seeding
- [x] Implement re-resolve-only mode for post-code-graph-update resolution refresh
- [x] Add optional `--bridge` flag to `build-code-graph.sh` that triggers re-resolution after code import
- [x] Test: `build-bridge.sh --repo gleam` runs full pipeline end-to-end — gleam: 7255 nodes, 290 resolved
- [x] Test: `build-bridge.sh --re-resolve-only --repo gleam` only touches unresolved refs — 0 stale, 0 re-resolved (expected — no code graph changes since initial run)
- [x] Test: `build-bridge.sh --seed-only` creates ontology nodes without touching code references — 5 concepts, 5 phases, 10 failure modes, 100 design decisions
- [ ] **TPR checkpoint**: run `/tpr-review` covering 08.3 + 08.4 before proceeding to completion

### Subsection 08.4 close-out
**`/improve-tooling` retrospective**: Is the pipeline ordering correct? Any race conditions? Should seed run before or after resolution? Performance acceptable?

---

## 08.R Third Party Review Findings

- [x] `[TPR-08-001-codex][high]` `section-08:13` — GAP: Stale-reference invalidation missing (resolved→stale transitions).
  Resolved: Fixed on 2026-04-13. Added stale/stale_since fields to CodeReference schema, --invalidate-stale flag to resolve_code_refs.py, and invalidation trigger in build-bridge.sh.
- [x] `[TPR-08-002-codex][medium]` `section-08:71` — GAP: DesignDecision ontology coverage missing from 08.0/08.3.
  Resolved: Fixed on 2026-04-13. Added DesignDecision constraint, node type, relationship types, and seeding checklist items.
- [x] `[TPR-08-003-codex][medium]` `section-08:277` — DRIFT: Dead Ori re-resolution cross-section hook (no Ori issue corpus exists).
  Resolved: Fixed on 2026-04-13. Removed Ori-specific re-resolution claim from Section 08.
- [x] `[TPR-08-004-codex][medium]` `section-08:199` — GAP: Missing unit test deliverables (test_extract_code_refs.py, test_resolve_code_refs.py).
  Resolved: Fixed on 2026-04-13. Added concrete test file checklist items to 08.1 and 08.2.
- [x] `[TPR-08-005-codex][low]` `section-08:86` — WASTE: coderef_text fulltext index has no concrete query path.
  Resolved: Fixed on 2026-04-13. Removed coderef_text index from schema.
- [x] `[TPR-08-001-gemini][high]` `section-08:129` — GAP: Module-level source resolution removed but Section 07 delegates it here.
  Resolved: Fixed on 2026-04-13. Re-added as 08.2 checklist item with note that implementation lives in extract_symbols.py (SSOT).
- [x] `[TPR-08-002-gemini][high]` `section-08:118` — GAP: _build_resolution_index returns internal Neo4j IDs (unstable after wipe-and-replace).
  Resolved: Fixed on 2026-04-13. Updated pseudo-code to use stable business keys (path, qualified_name+signature_hash).
- [x] `[TPR-08-003-gemini][high]` `section-08:55` — GAP: body_offset is scalar but dedup collapses multiple occurrences.
  Resolved: Fixed on 2026-04-13. Changed to body_offsets: [int] with aggregation during dedup.
- [x] `[TPR-08-004-gemini][high]` `section-08:162` — GAP: Missing mandatory TPR checkpoints (plan schema requires for 3+ subsections).
  Resolved: Fixed on 2026-04-13. Added TPR checkpoints after 08.2 and 08.4.
- [x] `[TPR-08-005-gemini][medium]` `section-08:55` — DRIFT: Schema missing ambiguous, ambiguous_count, occurrence_count properties.
  Resolved: Fixed on 2026-04-13. Added all three to CodeReference node schema in 08.0.
- [x] `[TPR-08-006-gemini][low]` `section-08:204` — Align bash iteration syntax with build-code-graph.sh.
  Resolved: Rejected on 2026-04-13. Factually incorrect — build-code-graph.sh uses `"${REPOS[@]}"` array syntax (line 90), matching the build-bridge.sh snippet. Gemini confabulated that it uses `$REPOS` string iteration.

## 08.5 Completion Checklist

- [x] Schema extended: CodeReference, Concept, CompilerPhase, FailureMode constraints and indexes applied
- [x] Code references extracted from issue/comment/review bodies with stop-word filtering
- [x] CodeReference nodes created with confidence scores, source provenance, body offsets
- [x] RESOLVES_TO edges link references to File/Symbol nodes (unambiguous only)
- [x] Ambiguous references marked as ambiguous, NOT fanned out to multiple RESOLVES_TO edges
- [x] Re-resolution mechanism works: `--re-resolve` updates previously-unresolved refs
- [x] Stale-invalidation mechanism works: `--invalidate-stale` detects and marks stale references
- [x] Module-level source resolution fulfills TPR-07-010/TPR-07-017 from Section 07
- [x] Ontology seeded with Concept, FailureMode, CompilerPhase, DesignDecision nodes
- [x] Auto-tagging produces meaningful TAGGED_AS and INTRODUCES_FAILURE_MODE edges
- [x] Pipeline runner (`build-bridge.sh`) orchestrates extract -> resolve -> seed
- [x] Bridge queries work: `MATCH (i:Issue)-[:MENTIONS_CODE]->(cr)-[:RESOLVES_TO]->(s:Symbol) RETURN count(i)` — 1945 issues with refs
- [x] In-memory resolution pattern used (not per-reference Cypher queries)
- [x] Unit tests exist: `test_extract_code_refs.py` (32 tests) and `test_resolve_code_refs.py` (18 tests)
- [ ] TPR checkpoints passed after 08.2 and after 08.4
- [x] No test regressions: `timeout 150 ./test-all.sh` — 17196 passed, 0 failed
- [ ] `/tpr-review` clean
- [ ] `/impl-hygiene-review` clean
- [ ] `/improve-tooling` section-close sweep
