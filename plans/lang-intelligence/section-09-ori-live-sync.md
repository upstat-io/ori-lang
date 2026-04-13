---
section: "09"
title: "Ori Live Sync"
status: not-started
reviewed: false
goal: "Keep Ori's code graph in Neo4j continuously updated via lefthook post-commit async enqueue and tree-sitter incremental re-parse."
success_criteria:
  - "lefthook post-commit hook enqueues changed files and returns immediately (<100ms)"
  - "Background worker processes queued files via tree-sitter incremental parse"
  - "Single-file re-sync completes in <500ms"
  - "Dependency refresh (when imports change) completes in <2s"
  - "Broken ASTs during development handled gracefully (partial parse, retain last-good edges)"
  - "Manual sync command available: scripts/sync-ori-graph.sh"
  - "Full rebuild fallback: scripts/sync-ori-graph.sh --full"
depends_on: ["06"]
third_party_review:
  status: none
  updated: null
---

# 09 Ori Live Sync

## 09.0 Goal

Ori is the one repo where the code graph must stay current during active development. For the 10 reference repos, periodic batch rebuilds are fine. For Ori, the graph should reflect the latest save within 500ms.

The live sync lives entirely in `~/projects/lang_intelligence/` — per the architectural decision from TPR (Codex finding #6), ori_lang has NO dependency on or knowledge of the intelligence DB's sync mechanism. A lefthook hook in ori_lang provides the trigger; the sync logic is external.

## 09.1 Lefthook Post-Commit Hook

**File**: `lefthook.yml` (in ori_lang)

Add async post-commit enqueue:
```yaml
post-commit:
  commands:
    intel-sync:
      run: |
        if [ -x ../lang_intelligence/scripts/sync-ori-graph.sh ]; then
          ../lang_intelligence/scripts/sync-ori-graph.sh --changed "{staged_files}" &
        fi
      # Fire-and-forget: returns immediately, sync runs in background
      # If lang_intelligence doesn't exist, the -x test fails silently
```

- [ ] Add `intel-sync` to `lefthook.yml` post-commit section
- [ ] Verify it returns immediately (<100ms) — the `&` backgrounds the sync
- [ ] Verify it's a no-op when `../lang_intelligence/` doesn't exist
- [ ] Verify it doesn't interfere with existing pre-commit hooks
- [ ] Test: commit a file → sync runs in background → graph updated within 500ms

### Subsection 09.1 close-out
**`/improve-tooling` retrospective**: Is the lefthook hook reliable? Any timing issues with the background process?

---

## 09.2 Sync Script

**File**: `~/projects/lang_intelligence/scripts/sync-ori-graph.sh`

Two modes:
- **Incremental** (default): `sync-ori-graph.sh --changed "file1.rs file2.rs"` — re-parse only changed files
- **Full rebuild**: `sync-ori-graph.sh --full` — re-parse entire Ori codebase

**Incremental flow**:
1. For each changed file: tree-sitter incremental parse (14x faster than fresh parse)
2. Extract symbols from the new AST
3. Diff against current Neo4j state (compare signature_hash)
4. Update only changed symbols (MERGE with new properties)
5. If imports changed: re-extract IMPORTS edges for dependent files

**Performance targets** (from tree-sitter research):
- Single file parse: ~0.2ms (incremental on 10K-line file: 2.8ms)
- Symbol extraction + Neo4j update: ~200ms per file
- Total for single-file change: <500ms
- Full rebuild: <30s for entire Ori codebase

- [ ] Create `sync-ori-graph.sh` with both modes
- [ ] Implement incremental parse using tree-sitter `tree.edit()` + `parser.parse(new_source, old_tree)`
- [ ] Implement symbol diff: compare extracted symbols against Neo4j's current signature_hash for the file
- [ ] Only update Neo4j nodes that actually changed (avoid unnecessary writes)
- [ ] Handle parse errors gracefully: mark file as `parse_status: 'partial'`, keep last-good symbol edges
- [ ] Add debounce (250ms) for rapid successive saves (collapse multiple saves into one sync)
- [ ] Add lock file to prevent concurrent syncs from colliding
- [ ] Log sync operations to `~/projects/lang_intelligence/logs/ori-sync.log` for debugging

### Subsection 09.2 close-out
**`/improve-tooling` retrospective**: Is 500ms achievable in practice? Any Neo4j write bottlenecks? Should we batch multiple file changes into one transaction?

---

## 09.3 Ori Parser Integration

For Ori, we have a choice: tree-sitter or Ori's own Rust parser. The research found no tree-sitter grammar for Ori exists. Two options:

**Option A: Write a tree-sitter grammar.js for Ori**
- Pro: Consistent with the rest of the pipeline
- Con: Maintaining a separate grammar that must track the Rust parser

**Option B: Use Ori's own Rust parser via FFI**
- Pro: Always accurate, no grammar maintenance
- Con: Couples the sync to the Rust toolchain, slower than tree-sitter for incremental

**Decision**: Use Option B for now — accuracy matters more than speed for Ori's own codebase. tree-sitter grammar can be added later if incremental performance is insufficient.

- [ ] Create a thin Python wrapper that calls `cargo run -- check --dump-symbols <file>` (or equivalent) <!-- unblocks:05 -->
- [ ] If no symbol dump mode exists in the Ori CLI, add a `--dump-symbols` flag that outputs the same JSONL format as `extract_symbols.py`
- [ ] For incremental: re-run the parser on changed files only (Ori's Salsa-based incremental compilation helps here)
- [ ] Fallback: if the Rust parser fails (compilation error), mark file as `parse_status: 'error'` and retain last-good state

### Subsection 09.3 close-out
**`/improve-tooling` retrospective**: Is the Rust FFI approach fast enough? Should we reconsider a tree-sitter grammar?

---

## 09.R Third Party Review Findings

- None.

## Completion Checklist

- [ ] `lefthook.yml` has post-commit `intel-sync` hook
- [ ] `sync-ori-graph.sh` works in both incremental and full modes
- [ ] Single-file sync <500ms
- [ ] Full rebuild <30s
- [ ] Broken ASTs handled gracefully
- [ ] No interference with existing ori_lang hooks
- [ ] No test regressions: `timeout 150 ./test-all.sh`
- [ ] `/tpr-review` clean
- [ ] `/impl-hygiene-review` clean
- [ ] `/improve-tooling` section-close sweep
