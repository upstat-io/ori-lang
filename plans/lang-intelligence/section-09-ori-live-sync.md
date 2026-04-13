---
section: "09"
title: "Ori Live Sync"
status: not-started
reviewed: false
goal: "Keep Ori's code graph in Neo4j continuously updated via a lefthook post-commit hook that triggers a background sync script in lang_intelligence/, using Ori's own built binary for parsing and the existing upsert_file_symbols() API for atomic Neo4j updates."
success_criteria:
  - "lefthook post-commit hook triggers background sync and returns immediately (<100ms)"
  - "Sync script identifies changed/deleted/renamed .ori/.rs files via git diff-tree and runs per-file extraction+upsert with relationship resolution"
  - "Single-file sync completes in <5s (built binary cold invocation + extraction + Neo4j upsert)"
  - "Parse failures short-circuit before upsert — last-good graph state preserved (zero data loss on broken AST)"
  - "Manual sync available: ~/projects/lang_intelligence/scripts/sync-ori-graph.sh [--full]"
  - "Errors logged to ~/projects/lang_intelligence/logs/ori-sync.log — no silent failures"
  - "Health check detects stale graph state (last sync > 24h with commits since)"
  - "Ori :Repo node created in Neo4j (prerequisite — ori has no issue graph data)"
  - "No test regressions: timeout 150 ./test-all.sh"
depends_on: ["06", "07"]
sections:
  - id: "09.0"
    title: "Prerequisites & Repo Bootstrap"
    status: not-started
  - id: "09.1"
    title: "Lefthook Post-Commit Hook"
    status: not-started
  - id: "09.2"
    title: "Sync Script & Error Handling"
    status: not-started
  - id: "09.3"
    title: "Ori Symbol Extraction Adapter"
    status: not-started
  - id: "09.4"
    title: "Health Monitoring & Diagnostics"
    status: not-started
  - id: "09.5"
    title: "Tests"
    status: not-started
  - id: "09.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "09.N"
    title: "Completion Checklist"
    status: not-started
third_party_review:
  status: none
  updated: null
---

# 09 Ori Live Sync

## 09.0 Prerequisites & Repo Bootstrap

Ori is the one repo where the code graph must stay current during active development. For the 10 reference repos, periodic batch rebuilds via `build-code-graph.sh` are sufficient. For Ori, the graph should be updated after every commit.

**Architectural boundary:** The live sync lives entirely in `~/projects/lang_intelligence/` — per the architectural decision from Section 07 TPR (Codex finding #6), `ori_lang` has NO dependency on or knowledge of the intelligence DB's schema, sync logic, or JSONL format. A lefthook hook in `ori_lang` provides the trigger (a shell one-liner that calls an external script); all sync logic is external. The compiler exposes compiler-native data via existing phase dump flags (`ORI_DUMP_AFTER_PARSE=1` etc.); the intelligence layer owns the normalization from compiler output to JSONL/Neo4j.

**Performance model:** The original plan targeted <500ms per-file sync using tree-sitter incremental parsing. This is infeasible for Ori because: (1) Ori has no tree-sitter grammar (`grammar: native` in `languages.yaml`), (2) `cargo run` has multi-second cold-start overhead and discards Salsa incrementality between invocations, and (3) each sync invokes the built Ori binary which includes process startup + parser init. The realistic target is **<5s per file** for the common case (built binary already exists, Neo4j is warm). This is still fast enough for a post-commit hook that runs in the background — the developer never waits for it.

**Why not a long-lived daemon?** The `ori watch` command (`compiler/oric/src/commands/watch.rs`) demonstrates persistent `CompilerDb` + Salsa incrementality + debounce, and could theoretically provide sub-100ms re-parse. However, a daemon adds operational complexity (lifecycle management, crash recovery, stale state) that is not warranted for a developer tool where commits happen at most a few times per minute. The background-process-per-commit model is simpler, more reliable, and sufficient. A daemon upgrade can be revisited if the <5s target proves insufficient in practice.

**Prerequisite: Ori `:Repo` node.** The `build-code-graph.sh` pipeline skips repos without a `:Repo` node in Neo4j (see line 74: `if r in neo4j_repos`). The 10 reference repos get their `:Repo` nodes from `import_graph.py` (the issue graph import). Ori has no issue graph data, so its `:Repo` node must be created explicitly. `import_code_graph.py` checks for the Repo node at line 328-334 and exits with an error if missing.

**success_criteria:**
- [ ] Ori `:Repo` node exists in Neo4j with `name: "ori"`
- [ ] `import_code_graph.py ori <jsonl>` succeeds (Repo check passes)
- [ ] `extract_symbols.py ori` can run against Ori source (via the native adapter from 09.3)

- [ ] Create Ori `:Repo` node via a bootstrap Cypher in `sync-ori-graph.sh --bootstrap`:
  ```cypher
  MERGE (r:Repo {name: "ori"})
  SET r.full_name = "ori-lang/ori",
      r.description = "The Ori programming language compiler",
      r.is_custom = true
  ```
  The `is_custom: true` property distinguishes Ori from the 10 reference repos (which have issue graph data). This bootstrap is idempotent (MERGE).
- [ ] Verify `import_code_graph.py` accepts the bootstrapped Repo node
- [ ] Verify `logs/` directory is created by the sync script if it does not exist (`mkdir -p`)

### Subsection 09.0 close-out
**`/improve-tooling` retrospective**: Does the bootstrap Cypher need to be run manually, or should `sync-ori-graph.sh` auto-bootstrap on first run? Should `build-code-graph.sh` be updated to handle custom repos alongside issue-graph repos?

---

## 09.1 Lefthook Post-Commit Hook

**File**: `lefthook.yml` (in `ori_lang`)

Add an async post-commit hook that triggers the external sync script. The hook must:
1. Return immediately (background the sync with `&`)
2. Be a no-op when `../lang_intelligence/` doesn't exist
3. Not interfere with existing pre-commit hooks
4. Use `git diff-tree` to identify changed files (NOT `{staged_files}` — lefthook does NOT expose `{staged_files}` in post-commit context; files are already committed)

**success_criteria:**
- [ ] Hook returns in <100ms (verified by timing `git commit` with and without hook)
- [ ] Hook is a no-op when `../lang_intelligence/` is absent
- [ ] No interference with existing pre-commit hooks (`fmt`, `full-check`, `version-sync`, `spec-proposal-gate`)

```yaml
post-commit:
  commands:
    intel-sync:
      run: |
        if [ -x ../lang_intelligence/scripts/sync-ori-graph.sh ]; then
          CHANGED=$(git diff-tree --no-commit-id --name-only -r HEAD -- '*.ori' '*.rs' 'library/')
          if [ -n "$CHANGED" ]; then
            ../lang_intelligence/scripts/sync-ori-graph.sh --changed "$CHANGED" >> ../lang_intelligence/logs/ori-sync.log 2>&1 &
          fi
        fi
      # Fire-and-forget: returns immediately, sync runs in background
      # If lang_intelligence doesn't exist, the -x test fails silently
      # Errors logged to ori-sync.log, not swallowed
```

Key design decisions:
- **`git diff-tree --no-commit-id --name-only -r HEAD`** identifies files changed in the just-committed revision. The `-- '*.ori' '*.rs' 'library/'` suffix filters to relevant file types.
- **Log redirection**: stdout and stderr go to `ori-sync.log`. The original plan used fire-and-forget `&` with no output capture, which makes errors invisible (Finding #7). Logging to a file makes failures diagnosable.
- **Conditional trigger**: Only runs if `$CHANGED` is non-empty (no sync needed for docs-only commits).

- [ ] Add `post-commit` section with `intel-sync` command to `lefthook.yml`
- [ ] Verify hook returns immediately (<100ms) — the `&` backgrounds the sync
- [ ] Verify hook is a no-op when `../lang_intelligence/` doesn't exist
- [ ] Verify hook doesn't interfere with existing pre-commit hooks
- [ ] Verify `git diff-tree` correctly identifies changed `.ori` and `.rs` files
- [ ] Verify errors are captured in `ori-sync.log` (not silently dropped)

### Subsection 09.1 close-out
**`/improve-tooling` retrospective**: Is the `git diff-tree` filter sufficient? Should we also trigger on `.toml` changes (new dependencies might affect symbols)? Any race conditions with rapid successive commits?

---

## 09.2 Sync Script & Error Handling

**File**: `~/projects/lang_intelligence/scripts/sync-ori-graph.sh`

Two modes:
- **Incremental** (default): `sync-ori-graph.sh --changed "file1.ori file2.rs ..."` — extract+upsert only changed files
- **Full rebuild**: `sync-ori-graph.sh --full` — re-extract entire Ori codebase
- **Bootstrap**: `sync-ori-graph.sh --bootstrap` — create the Ori `:Repo` node (idempotent, runs before first sync)

**success_criteria:**
- [ ] Incremental mode processes only the listed files
- [ ] Full mode re-extracts and upserts all Ori source files
- [ ] Parse failures short-circuit before `upsert_file_symbols()` — last-good state preserved
- [ ] Lock file prevents concurrent syncs from colliding
- [ ] All operations logged to `logs/ori-sync.log`
- [ ] Exit code 0 on success, non-zero on failure (for health monitoring)

**Incremental flow:**
1. Acquire lock (`flock` on `~/projects/lang_intelligence/.ori-sync.lock`)
2. Auto-bootstrap: ensure Ori `:Repo` node exists (idempotent MERGE)
3. Ensure `logs/` directory exists (`mkdir -p ~/projects/lang_intelligence/logs`)
4. For each changed file:
   a. Run Ori symbol extraction (see 09.3) to produce JSONL records
   b. **If extraction fails** (compiler error, non-zero exit): log the error and **skip this file** — do NOT call `upsert_file_symbols()` with empty symbols. This is the "retain last-good" contract. The existing graph state for this file remains intact.
   c. **If extraction succeeds**: call `upsert_file_symbols()` from `import_code_graph.py` for this file. This function implements atomic file-scoped symbol diff (see `import_code_graph.py` lines 45-202): it deletes stale symbols, merges updated symbols, and creates DECLARES/IN_REPO edges — all in a single transaction.
   d. **After symbol upsert**: resolve per-file relationships (CALLS/IMPORTS/IMPLEMENTS) for this file. `upsert_file_symbols()` only handles symbol nodes and DECLARES/IN_REPO edges — it does NOT rebuild CALLS/IMPORTS/IMPLEMENTS. These are handled by the bulk importer's separate Phase 2 relationship pass (`import_code_graph.py` lines 464-520). The incremental sync must run a file-scoped version of this relationship resolution: delete stale outgoing relationship edges for the changed file, then resolve and create new ones from the extraction JSONL. Without this, incremental sync would gradually strip the relationship graph.
5. For deleted/renamed files (detected via `git diff-tree --diff-filter=DR`):
   a. Delete the old file's `(:File)` node and all connected `(:Symbol)` nodes and edges from Neo4j
   b. For renames, the new path will be handled as a new file in step 4 above
   c. This prevents stale nodes from persisting until full rebuild
6. Release lock
7. Log summary (files processed, files deleted, files skipped due to errors, elapsed time)

**Full rebuild flow:**
1. Acquire lock
2. Auto-bootstrap Repo node
3. **Extract all Ori symbols via `ori_adapter.py` directly** (NOT `extract_symbols.py ori` — `parse_repo()` in `extract_symbols.py` skips `coverage_status: custom` languages, so `extract_symbols.py ori` would process zero files). The full-rebuild path must: enumerate all `.ori` files in the Ori source tree, call `ori_adapter.extract_ori_file()` for each, and write the combined JSONL to a temp file.
4. Run `import_code_graph.py ori <temp_jsonl>` (the standard bulk import path from Section 07 — this includes ghost file deletion and Phase 2 relationship resolution)
5. Release lock

**Note on `extract_symbols.py` integration:** The plan originally assumed `extract_symbols.py ori` would work. This is a GAP — `parser_adapter.py:parse_repo()` (line 343-348) explicitly skips `coverage_status: custom` languages, and `extract_symbols.py` only iterates `parse_repo()`. The fix is to have the full-rebuild path call `ori_adapter.py` directly rather than routing through the tree-sitter pipeline. A future enhancement could extend `parse_repo()` to dispatch custom adapters, but that is not required for Section 09.

**Critical: `upsert_file_symbols()` already does the diff.** The original plan (09.2) described implementing a "symbol diff: compare extracted symbols against Neo4j's current signature_hash." This is algorithmic duplication — `upsert_file_symbols()` already performs file-scoped declarative diff (steps 1-5 in the function: get existing keys, compute incoming keys, delete outgoing edges, delete stale symbols, merge new symbols). The sync script must NOT re-implement this logic. It feeds file-level symbol records to `upsert_file_symbols()` and lets it handle the diff.

**Critical: ghost file deletion is NOT used in incremental mode.** The bulk import path in `import_code_graph.py`'s `main()` runs ghost file deletion (lines 397-419) which removes files present in Neo4j but absent from the JSONL. The incremental sync MUST NOT use this bulk path — it would delete all files not in the current commit's change list. The incremental sync calls `upsert_file_symbols()` per-file, which only touches the symbols for that specific file.

```bash
#!/usr/bin/env bash
# Sync Ori's code graph into Neo4j (incremental or full).
# Lives in ~/projects/lang_intelligence/scripts/
#
# Usage:
#   sync-ori-graph.sh --changed "file1.ori file2.rs"  # incremental
#   sync-ori-graph.sh --full                           # full rebuild
#   sync-ori-graph.sh --bootstrap                      # create Repo node only
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
LOCK_FILE="$PROJECT_DIR/.ori-sync.lock"
LOG_DIR="$PROJECT_DIR/logs"
LOG_FILE="$LOG_DIR/ori-sync.log"

mkdir -p "$LOG_DIR"

# Auto-activate venv
if [[ -z "${VIRTUAL_ENV:-}" ]]; then
    if [[ -f "$PROJECT_DIR/.venv/bin/activate" ]]; then
        source "$PROJECT_DIR/.venv/bin/activate"
    else
        echo "$(date -Iseconds) ERROR: .venv not found" >> "$LOG_FILE"
        exit 1
    fi
fi

# Parse args...
# Implementation delegates to sync_ori_graph.py for the Python parts
```

- [ ] Create `sync-ori-graph.sh` shell wrapper with `--changed`, `--full`, `--bootstrap` modes
- [ ] Implement lock file via `flock` to prevent concurrent syncs
- [ ] Ensure `logs/` directory is created if missing (`mkdir -p`)
- [ ] Implement auto-bootstrap (MERGE Ori Repo node on every run — idempotent)
- [ ] Create `sync_ori_graph.py` Python module that:
  - [ ] Accepts a list of changed file paths and calls the extraction adapter (09.3) per-file
  - [ ] Short-circuits on extraction failure — does NOT call `upsert_file_symbols()` with empty symbols
  - [ ] Calls `upsert_file_symbols()` from `import_code_graph.py` for each successfully-extracted file
  - [ ] After symbol upsert, resolves per-file relationships (CALLS/IMPORTS/IMPLEMENTS) — deletes stale outgoing relationship edges for the changed file, then creates new ones from extraction JSONL
  - [ ] Handles deleted files: removes (:File) node and all connected (:Symbol) nodes and edges
  - [ ] Handles renamed files: deletes old path, processes new path as a new file
  - [ ] Detects deletions/renames via `git diff-tree --diff-filter=DR HEAD` in the shell wrapper
  - [ ] Logs per-file results (success/skip/error/deleted) and summary statistics
- [ ] Verify incremental mode does NOT use bulk import path (no ghost file deletion on partial input)
- [ ] Verify per-file relationship resolution works (CALLS/IMPORTS/IMPLEMENTS survive incremental sync)
- [ ] Verify full mode uses the dedicated full-rebuild path (see below)

### Subsection 09.2 close-out
**`/improve-tooling` retrospective**: Is `flock` sufficient for concurrency control, or do we need a PID-based guard? Should we batch multiple file changes into one Neo4j transaction for performance? Is the per-file extraction overhead acceptable?

---

## 09.3 Ori Symbol Extraction Adapter

**File**: `~/projects/lang_intelligence/neo4j/ori_adapter.py`

Ori uses its own Rust parser (`ori_parse`), not tree-sitter. The adapter must bridge Ori's compiler output to the JSONL format consumed by `upsert_file_symbols()`.

**Design principle: compiler-agnostic normalization.** The compiler exposes compiler-native data via existing phase dump flags (e.g., `ORI_DUMP_AFTER_PARSE=1`). The intelligence layer (`lang_intelligence/`) owns the normalization from compiler output to JSONL. The compiler has NO knowledge of the intelligence DB's schema. Specifically:
- **NO `--dump-symbols` flag in the compiler** — adding a flag that outputs "the same JSONL format as extract_symbols.py" leaks the intelligence schema into the compiler (Finding #4). The compiler's job is to parse and type-check Ori code; the intelligence layer's job is to extract symbols from that output.
- The adapter calls the **built binary** (`~/projects/ori_lang/target/release/ori` or `target/debug/ori`), NOT `cargo run`. The `cargo run` path has multi-second cold-start overhead (compiles if needed, creates fresh `CompilerDb`, discards Salsa incrementality). The built binary starts in ~50ms.

**success_criteria:**
- [ ] Adapter produces JSONL records in the same format as `extract_symbols.py` (type: "symbol"/"relationship"/"file_meta")
- [ ] Uses the built Ori binary, not `cargo run`
- [ ] Falls back gracefully if binary doesn't exist (logs error, returns empty result)
- [ ] Parse errors produce `had_error: true` file_meta record and zero symbol records
- [ ] Per-file extraction completes in <3s for typical Ori source files

**Approach: `ori check` + AST dump parsing.**

The Ori compiler already supports `ORI_DUMP_AFTER_PARSE=1 ori check <file>` which dumps the parsed AST to stderr in a structured indented format (see `compiler/oric/src/ast_dump/mod.rs`). The adapter can:

1. Run `ORI_DUMP_AFTER_PARSE=1 <ori_binary> check <file>` and capture stderr
2. Parse the AST dump to extract structural symbols (functions, types, traits, impls, modules)
3. Normalize to the JSONL symbol record format

However, the AST dump format is designed for human debugging, not machine consumption. A more robust approach:

**Preferred approach: `ori check` exit code + direct source scanning.**

1. Run `<ori_binary> check <file>` to validate parseability (exit 0 = parseable, non-zero = error)
2. Use a lightweight Python regex/AST scanner on the `.ori` source to extract structural declarations:
   - `@name (...) -> T` — function declarations
   - `type Name = { ... }` — struct/sum type declarations
   - `trait Name { ... }` — trait declarations
   - `impl Type: Trait { ... }` — impl blocks
   - `use "..." { ... }` — imports
3. Compute `qualified_name` from file path + declaration nesting (same algorithm as Section 06.2)
4. Compute `signature_hash` from the declaration signature (body-independent, same algorithm as Section 06.3)
5. Produce JSONL records in the standard format

This approach is the most correct because:
- It does not require compiler changes (no schema leakage)
- It uses the compiler for validation (parse success/failure) but not for structured output
- The Python scanner can be tested independently
- It follows the same data-driven pattern as `extract_symbols.py` for tree-sitter languages

**For `.rs` files in `compiler/` and `library/`:** Use the existing tree-sitter Rust parser (`languages.yaml: rust: grammar: tree-sitter-rust`). The Ori adapter only handles `.ori` files; Rust files go through the standard `extract_symbols.py` pipeline.

- [ ] Create `ori_adapter.py` in `~/projects/lang_intelligence/neo4j/` with:
  - [ ] `extract_ori_file(file_path, ori_binary) -> list[dict]` — extract symbols from a single `.ori` file
  - [ ] `find_ori_binary() -> str` — locate the Ori binary (prefer release, fall back to debug, error if neither exists)
  - [ ] `validate_parseable(file_path, ori_binary) -> bool` — run `ori check` and check exit code
  - [ ] Python regex scanner for Ori structural declarations (`@fn`, `type`, `trait`, `impl`, `use`)
  - [ ] `qualified_name` derivation from file path + nesting
  - [ ] `signature_hash` computation (body-independent)
  - [ ] JSONL record generation in the standard format (type: "symbol"/"relationship"/"file_meta")
- [ ] Register `ori` in `parser_adapter.py`'s `parse_file()` to delegate to `ori_adapter.py` for `coverage_status: custom` languages (currently `parse_file()` raises `ValueError` for native parsers — add a dispatch path)
- [ ] Verify output format matches `extract_symbols.py` schema exactly (same fields, same types)
- [ ] Verify `.rs` files in `compiler/` use the standard Rust tree-sitter pipeline (no adapter needed)

### Subsection 09.3 close-out
**`/improve-tooling` retrospective**: Is the regex scanner robust enough for Ori's syntax? Should we invest in a proper AST dump JSON mode in the compiler instead? Benchmark the binary cold-start time — is <3s achievable?

---

## 09.4 Health Monitoring & Diagnostics

**File**: `~/projects/lang_intelligence/scripts/sync-ori-graph.sh` (health-check mode)

The background sync must not fail silently. This subsection adds observability.

**success_criteria:**
- [ ] `sync-ori-graph.sh --health` reports sync status (last sync time, files synced, errors since last success)
- [ ] Stale graph detection: warn if last sync > 24h and there have been commits since
- [ ] Log rotation or size cap prevents unbounded log growth
- [ ] `intel-query.sh status` output includes Ori sync metadata (last sync time, staleness)

- [ ] Add `--health` mode to `sync-ori-graph.sh` that:
  - [ ] Queries Neo4j for Ori Repo's `last_code_import_at` timestamp
  - [ ] Checks `ori-sync.log` for recent errors
  - [ ] Checks `git log --since=<last_sync>` for commits since last sync
  - [ ] Reports: last sync time, files in graph, errors since last success, commits since last sync
- [ ] Add log rotation: truncate `ori-sync.log` to last 10,000 lines on each run (or use `logrotate` config)
- [ ] Add Ori sync metadata to `intel-query.sh status` output (query Repo node's `last_code_import_at` — `status` is the canonical public surface per Section 01, not a separate `health` command)
- [ ] Verify stale detection works: commit a file, wait, check `--health` reports stale

### Subsection 09.4 close-out
**`/improve-tooling` retrospective**: Is the health check sufficient for detecting problems? Should we add a cron job for periodic health checks? Should `--health` be integrated into `test-all.sh` or a CI check?

---

## 09.5 Tests

Zero tests in the original plan is a violation of CLAUDE.md testing requirements. This subsection adds comprehensive testing for all sync components.

**success_criteria:**
- [ ] Unit tests for `ori_adapter.py` (regex scanner, JSONL output, error handling)
- [ ] Integration tests for `sync_ori_graph.py` (end-to-end sync with test Neo4j instance)
- [ ] Lefthook hook contract tests (shell-level)

**Unit tests** (`~/projects/lang_intelligence/tests/test_ori_adapter.py`):

- [ ] `test_extract_function_declaration` — `@name (p: T) -> R = expr` produces correct symbol record
- [ ] `test_extract_type_declaration` — `type Name = { ... }` produces correct symbol record
- [ ] `test_extract_trait_declaration` — `trait Name { ... }` produces correct symbol record
- [ ] `test_extract_impl_block` — `impl Type: Trait { ... }` produces correct symbol+relationship records
- [ ] `test_extract_import` — `use "..." { ... }` produces correct relationship record
- [ ] `test_qualified_name_derivation` — file path + nesting produces correct qualified_name
- [ ] `test_signature_hash_body_independent` — changing function body does not change signature_hash
- [ ] `test_parse_failure_produces_error_meta` — broken `.ori` file produces file_meta with `had_error: true` and zero symbols
- [ ] `test_find_ori_binary_prefers_release` — release binary preferred over debug
- [ ] `test_find_ori_binary_fallback_to_debug` — debug binary used when release absent
- [ ] `test_find_ori_binary_error_when_neither` — clear error when no binary exists

**Integration tests** (`~/projects/lang_intelligence/tests/test_sync_ori_graph.py`):

- [ ] `test_incremental_sync_creates_symbols` — sync a single file, verify symbols in Neo4j
- [ ] `test_incremental_sync_updates_on_change` — modify a file, re-sync, verify updated symbols
- [ ] `test_incremental_sync_preserves_on_parse_failure` — break a file, sync, verify old symbols remain
- [ ] `test_incremental_sync_preserves_relationships` — sync a file, verify CALLS/IMPORTS/IMPLEMENTS edges survive and update
- [ ] `test_incremental_sync_handles_file_deletion` — delete a file, sync, verify (:File) and (:Symbol) nodes removed
- [ ] `test_incremental_sync_handles_file_rename` — rename a file, sync, verify old path removed and new path present
- [ ] `test_full_sync_creates_repo_node` — full sync bootstraps Repo node
- [ ] `test_full_sync_idempotent` — running full sync twice produces same graph state
- [ ] `test_full_sync_processes_all_ori_files` — full sync processes custom-language files (not skipped by parse_repo)
- [ ] `test_lock_prevents_concurrent_sync` — two concurrent syncs don't corrupt state

**Lefthook contract tests** (shell):

- [ ] `test_hook_noop_without_lang_intelligence` — verify hook exits cleanly when `../lang_intelligence/` is absent
- [ ] `test_hook_captures_changed_files` — verify `git diff-tree` output matches committed files
- [ ] `test_hook_skips_non_ori_commits` — docs-only commit produces no sync trigger

### Subsection 09.5 close-out
**`/improve-tooling` retrospective**: Are the integration tests fast enough to run in CI? Do they need a dedicated Neo4j test instance? Should we add property-based tests for the regex scanner?

---

## 09.R Third Party Review Findings

- None.

---

## 09.N Completion Checklist

- [ ] Ori `:Repo` node exists in Neo4j (09.0)
- [ ] `sync-ori-graph.sh` works in incremental, full, and bootstrap modes (09.2)
- [ ] Lefthook post-commit hook triggers sync on `.ori`/`.rs` changes (09.1)
- [ ] `ori_adapter.py` extracts symbols from `.ori` files via regex scanner (09.3)
- [ ] Per-file relationship resolution (CALLS/IMPORTS/IMPLEMENTS) works in incremental mode (09.2)
- [ ] Deleted/renamed files handled correctly — stale nodes removed (09.2)
- [ ] Parse failures short-circuit before `upsert_file_symbols()` — last-good preserved (09.2)
- [ ] Errors logged to `ori-sync.log` — no silent failures (09.1, 09.2)
- [ ] Health check detects stale graph state (09.4)
- [ ] `logs/` directory auto-created (09.0, 09.2)
- [ ] Lock file prevents concurrent sync corruption (09.2)
- [ ] Unit tests pass for `ori_adapter.py` (09.5)
- [ ] Integration tests pass for sync pipeline (09.5)
- [ ] No interference with existing ori_lang hooks (09.1)
- [ ] No test regressions: `timeout 150 ./test-all.sh`
- [ ] `/tpr-review` clean
- [ ] `/impl-hygiene-review` clean
- [ ] `/improve-tooling` section-close sweep

### Subsection 09.N close-out
Confirm all checklist items pass. Strip any plan annotations from code. Archive sync log.
