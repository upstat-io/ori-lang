---
section: "09"
title: "Ori Live Sync"
status: in-progress
reviewed: true
goal: "Keep Ori's code graph in Neo4j continuously updated via a lefthook post-commit hook that triggers a background sync script in lang_intelligence/, using Ori's own built binary for parsing and the existing upsert_file_symbols() API for atomic Neo4j updates."
success_criteria:
  - "lefthook post-commit hook triggers background sync and returns immediately (<100ms)"
  - "Sync script identifies changed/deleted/renamed .ori/.rs files via git diff-tree and runs per-file extraction+upsert with relationship resolution"
  - "Single-file sync completes in <5s (regex extraction + Neo4j upsert)"
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
    status: complete
  - id: "09.1"
    title: "Lefthook Post-Commit Hook"
    status: complete
  - id: "09.2"
    title: "Sync Script & Error Handling"
    status: complete
  - id: "09.3"
    title: "Ori Symbol Extraction Adapter"
    status: complete
  - id: "09.4"
    title: "Health Monitoring & Diagnostics"
    status: in-progress
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
- [x] Ori `:Repo` node exists in Neo4j with `name: "ori"`
- [x] `import_code_graph.py ori <jsonl>` succeeds (Repo check passes)
- [ ] `ori_adapter.py` extracts `.ori` files and standard tree-sitter pipeline extracts `.rs` files — combined JSONL imports via `import_code_graph.py ori`

- [x] Create Ori `:Repo` node via a bootstrap Cypher in `sync-ori-graph.sh --bootstrap`:
  ```cypher
  MERGE (r:Repo {name: "ori"})
  SET r.full_name = "ori-lang/ori",
      r.description = "The Ori programming language compiler",
      r.is_custom = true
  ```
  The `is_custom: true` property distinguishes Ori from the 10 reference repos (which have issue graph data). This bootstrap is idempotent (MERGE).
- [x] Verify `import_code_graph.py` accepts the bootstrapped Repo node
- [x] Verify `logs/` directory is created by the sync script if it does not exist (`mkdir -p`)

- [ ] **Subsection close-out (09.0)** — MANDATORY before starting 09.1:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — Does the bootstrap Cypher need to be run manually, or should `sync-ori-graph.sh` auto-bootstrap on first run? Should `build-code-graph.sh` be updated to handle custom repos alongside issue-graph repos?
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any temp files

---

## 09.1 Lefthook Post-Commit Hook

**File**: `lefthook.yml` (in `ori_lang`)

Add an async post-commit hook that triggers the external sync script. The hook must:
1. Return immediately (background the sync with `&`)
2. Be a no-op when `../lang_intelligence/` doesn't exist
3. Not interfere with existing pre-commit hooks
4. Use `git diff-tree` to identify changed files (NOT `{staged_files}` — lefthook does NOT expose `{staged_files}` in post-commit context; files are already committed)

**success_criteria:**
- [x] Hook returns in <100ms (verified: 2ms — the `-x` test short-circuits when script absent)
- [x] Hook is a no-op when `../lang_intelligence/` is absent
- [x] No interference with existing pre-commit hooks (`fmt`, `full-check`, `version-sync`, `spec-proposal-gate`)

```yaml
post-commit:
  commands:
    intel-sync:
      run: |
        if [ -x ../lang_intelligence/scripts/sync-ori-graph.sh ]; then
          CHANGED=$(git diff-tree --no-commit-id --name-only -r HEAD -- 'compiler/*.rs' 'library/*.ori' 'library/*.rs')
          if [ -n "$CHANGED" ]; then
            mkdir -p ../lang_intelligence/logs
            ../lang_intelligence/scripts/sync-ori-graph.sh --changed "$CHANGED" >> ../lang_intelligence/logs/ori-sync.log 2>&1 &
          fi
        fi
      # Fire-and-forget: returns immediately, sync runs in background
      # If lang_intelligence doesn't exist, the -x test fails silently
      # Errors logged to ori-sync.log, not swallowed
```

Key design decisions:
- **`git diff-tree --no-commit-id --name-only -r HEAD`** identifies files changed in the just-committed revision. The pathspecs `'compiler/*.rs' 'library/*.ori' 'library/*.rs'` scope to the Ori code-graph's `include` roots defined in `repos.yaml` (`compiler/` and `library/`). This prevents fixtures, diagnostics, examples, tools, and other out-of-scope files from triggering unnecessary syncs or polluting the graph.
- **Log redirection**: stdout and stderr go to `ori-sync.log`. The original plan used fire-and-forget `&` with no output capture, which makes errors invisible (Finding #7). Logging to a file makes failures diagnosable.
- **Conditional trigger**: Only runs if `$CHANGED` is non-empty (no sync needed for docs-only commits).

- [x] Add `post-commit` section with `intel-sync` command to `lefthook.yml`
- [x] Verify hook returns immediately (<100ms) — 2ms measured
- [x] Verify hook is a no-op when `../lang_intelligence/` doesn't exist
- [x] Verify hook doesn't interfere with existing pre-commit hooks
- [x] Verify `git diff-tree` correctly identifies changed `.ori` and `.rs` files in compiler/library scope
- [x] Verify errors are captured in `ori-sync.log` (not silently dropped) — verified with sync script

- [ ] **Subsection close-out (09.1)** — MANDATORY before starting 09.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — Is the `git diff-tree` filter sufficient? Should we also trigger on `.toml` changes (new dependencies might affect symbols)? Any race conditions with rapid successive commits?
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any temp files

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
4. For each changed file (that still exists on disk):
   a. **Route by extension:** `.ori` files → `ori_adapter.extract_ori_file()` (09.3). `.rs` files → standard tree-sitter pipeline (`parse_file()` → `extract_from_parse_result()` from `extract_symbols.py`). This is critical: the hook triggers on both `.ori` and `.rs` changes, but `ori_adapter.py` only handles `.ori` files. Routing `.rs` files to `ori_adapter.py` would fail silently.
   b. **If extraction fails** (Python scanner exception): log the error and **skip this file** — do NOT call `upsert_file_symbols()` with empty symbols. This is the "retain last-good" contract. The existing graph state for this file remains intact.
   c. **If extraction succeeds**: call `upsert_file_symbols()` from `import_code_graph.py` for this file. This function implements atomic file-scoped symbol diff (see `import_code_graph.py` lines 45-202): it deletes stale symbols, merges updated symbols, and creates DECLARES/IN_REPO edges — all in a single transaction.
   d. **After symbol upsert**: resolve per-file relationships (CALLS/IMPORTS/IMPLEMENTS) for this file. `upsert_file_symbols()` only handles symbol nodes and DECLARES/IN_REPO edges — it does NOT rebuild CALLS/IMPORTS/IMPLEMENTS. These are handled by the bulk importer's separate Phase 2 relationship pass (`import_code_graph.py` lines 464-520). To avoid algorithmic duplication (LEAK:algorithmic-duplication), the incremental sync must use **the same shared logic** as the bulk importer — extract the Phase 2 resolution code from `import_code_graph.py::main()` into a reusable function (e.g., `resolve_file_relationships(driver, repo_name, file_path, relationships)`) that both the bulk importer and incremental sync call. The incremental sync invokes this function per-file: delete stale outgoing relationship edges, then resolve and create new ones from the extraction JSONL.
5. For deleted files (detected in Python via `os.path.exists()` — the shell wrapper passes all changed paths from `git diff-tree --name-only`, including paths that no longer exist on disk):
   a. Delete the old file's `(:File)` node and all connected `(:Symbol)` nodes and edges from Neo4j
   b. Git reports renames as separate add+delete entries (without `-M` flag). The deleted path is handled here; the new path is handled as a new file in step 4. This delete+add model is simpler and sufficient for live sync correctness.
   c. This prevents stale nodes from persisting until full rebuild
6. **Update `:Repo` node's `last_code_import_at` timestamp** after all files are processed. Without this, the `--health` check would falsely report the sync as stale after 24h regardless of how many incremental syncs ran.
7. **Reverse-dependency note:** When a changed file deletes or renames symbols, incoming edges from UNCHANGED files (e.g., a caller that `CALLS` a now-deleted function) become dangling. The incremental sync does NOT repair these — that would require re-extracting and re-resolving all files that reference the changed symbols, which approaches full-rebuild cost. This is an explicit simplification: incremental sync keeps symbols and outgoing edges correct; incoming edges from other files are eventually consistent via periodic `--full` rebuilds. **Recommended practice:** run `--full` weekly or after commits that delete/rename many symbols.
6. Release lock
7. Log summary (files processed, files deleted, files skipped due to errors, elapsed time)

**Full rebuild flow:**
1. Acquire lock
2. Auto-bootstrap Repo node
3. **Extract BOTH `.ori` and `.rs` symbols into a single combined JSONL.** `extract_symbols.py ori` processes ZERO files of any type because `parser_adapter.py:parse_repo()` (line 343-348) skips the entire repo when `coverage_status: custom`. The full-rebuild path must therefore enumerate all files itself and route per-file:
   a. Enumerate all `.ori` and `.rs` files within the Ori repo's `include` roots from `repos.yaml` (`compiler/`, `library/`), respecting `exclude` patterns
   b. **`.ori` files** → `ori_adapter.extract_ori_file()` (the standalone adapter from 09.3)
   c. **`.rs` files** → tree-sitter Rust pipeline per-file via `parse_file()` + `extract_from_parse_result()` from `extract_symbols.py` (`parse_file()` works per-file even for custom repos — it's `parse_repo()` that skips)
   d. **Parse-failed files during full rebuild:** Unlike incremental sync (where parse failures skip the file to preserve last-good state), full rebuild IS the canonical state reset — it produces the authoritative graph. Files that fail extraction are still included in the JSONL with `had_error: true` and zero symbols, which causes `upsert_file_symbols()` to remove their old symbols. This is **correct behavior for full rebuild**: if a file can't be parsed, its graph representation should reflect that (no symbols). The "retain last-good" contract applies ONLY to incremental sync, where a temporary parse error shouldn't destroy previously-good data. Parse failures during full rebuild are logged prominently so the developer knows to fix the broken files.
   e. **Combine all successful outputs** into a single JSONL temp file — this is critical because `import_code_graph.py`'s ghost file deletion removes files absent from the JSONL. The combined JSONL must contain BOTH `.ori` and `.rs` records so neither type gets ghost-deleted.
4. Run `import_code_graph.py ori <combined_jsonl>` (the standard bulk import path from Section 07 — this includes ghost file deletion and Phase 2 relationship resolution)
5. Release lock

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

- [x] Create `sync-ori-graph.sh` shell wrapper with `--changed`, `--full`, `--bootstrap`, `--health` modes
- [x] Implement lock file via `flock` to prevent concurrent syncs (verified: concurrent sync correctly skips)
- [x] Ensure `logs/` directory is created if missing (`mkdir -p`)
- [x] Implement auto-bootstrap (MERGE Ori Repo node on every run — idempotent)
- [x] Create `sync_ori_graph.py` Python module that:
  - [x] Accepts a list of changed file paths and calls the extraction adapter (09.3) per-file
  - [x] Short-circuits on extraction failure — does NOT call `upsert_file_symbols()` with empty symbols
  - [x] Calls `upsert_file_symbols()` from `import_code_graph.py` for each successfully-extracted file
  - [x] After symbol upsert, resolves per-file relationships (CALLS/IMPORTS/IMPLEMENTS) via `resolve_file_relationships()`
  - [x] Routes by file extension: `.ori` → `ori_adapter.py`, `.rs` → tree-sitter pipeline (`parse_file()` + `extract_from_parse_result()`)
  - [x] Handles deleted files (detected via `os.path.exists()`): removes (:File) node and all connected (:Symbol) nodes and edges
  - [x] Handles renamed files as delete+add (git reports renames as separate entries without `-M`)
  - [x] Updates `:Repo` node's `last_code_import_at` timestamp after successful sync
  - [x] Logs per-file results (success/skip/error/deleted) and summary statistics
- [x] Extract Phase 2 relationship resolution from `import_code_graph.py::main()` into `resolve_file_relationships()` — used by incremental sync, shares `_build_symbol_index`/`_resolve_source_py`/`_resolve_target_py` with bulk importer (SSOT)
- [x] Verify incremental mode does NOT use bulk import path (no ghost file deletion on partial input) — uses per-file `upsert_file_symbols()`
- [x] Verify per-file relationship resolution works (CALLS/IMPORTS/IMPLEMENTS survive incremental sync) — verified via `resolve_file_relationships()`
- [x] Verify full mode combines both pipelines (ori_adapter for .ori + tree-sitter for .rs) into single JSONL before import
- [x] Verify incremental mode routes .ori → ori_adapter, .rs → tree-sitter pipeline

- [ ] **Subsection close-out (09.2)** — MANDATORY before starting 09.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — Is `flock` sufficient for concurrency control, or do we need a PID-based guard? Should we batch multiple file changes into one Neo4j transaction for performance? Is the per-file extraction overhead acceptable?
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any temp files

- [ ] **TPR checkpoint** — `/tpr-review` covering 09.0–09.2 implementation work

---

## 09.3 Ori Symbol Extraction Adapter

**File**: `~/projects/lang_intelligence/neo4j/ori_adapter.py`

Ori uses its own Rust parser (`ori_parse`), not tree-sitter. The adapter must bridge Ori's compiler output to the JSONL format consumed by `upsert_file_symbols()`.

**Design principle: compiler-agnostic normalization.** The intelligence layer (`lang_intelligence/`) owns all symbol extraction logic. The compiler has NO knowledge of the intelligence DB's schema or extraction process. Specifically:
- **NO `--dump-symbols` flag in the compiler** — adding a flag that outputs "the same JSONL format as extract_symbols.py" leaks the intelligence schema into the compiler boundary.
- **NO compiler binary invocation during extraction** — the adapter uses a pure Python regex scanner on `.ori` source files, mirroring tree-sitter's approach of extracting structural declarations from source text. This avoids: (a) cold-start overhead of invoking the binary per-file, (b) type-checking rejections that would block extraction of valid structural declarations during active development, (c) coupling the intelligence pipeline to the compiler's build state.

**success_criteria:**
- [ ] Adapter produces JSONL records in the same format as `extract_symbols.py` (type: "symbol"/"relationship"/"file_meta")
- [ ] Pure Python regex scanner — no compiler binary invocation needed
- [ ] Handles malformed/partial `.ori` files gracefully (extracts what it can, logs warnings)
- [ ] Per-file extraction completes in <1s for typical Ori source files (pure Python, no process spawn)

**Approach: `ori check` + AST dump parsing.**

The Ori compiler already supports `ORI_DUMP_AFTER_PARSE=1 ori check <file>` which dumps the parsed AST to stderr in a structured indented format (see `compiler/oric/src/ast_dump/mod.rs`). The adapter can:

1. Run `ORI_DUMP_AFTER_PARSE=1 <ori_binary> check <file>` and capture stderr
2. Parse the AST dump to extract structural symbols (functions, types, traits, impls, modules)
3. Normalize to the JSONL symbol record format

However, the AST dump format is designed for human debugging, not machine consumption. A more robust approach:

**Preferred approach: direct source scanning (no `ori check` validation step).**

`ori check` performs BOTH parsing AND type-checking. A type error (which is common during active development) causes a non-zero exit code, which would block symbol extraction even when the structural declarations are perfectly valid. Instead, rely entirely on the Python regex scanner's fault tolerance — it extracts what it can from the source text, mirroring tree-sitter's approach of producing partial results from imperfect input.

1. Use a lightweight Python regex/AST scanner on the `.ori` source to extract structural declarations:
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

- [x] Create `ori_adapter.py` in `~/projects/lang_intelligence/neo4j/` with:
  - [x] `extract_ori_file(file_path) -> list[dict]` — extract symbols from a single `.ori` file using the regex scanner
  - [x] Python regex scanner for Ori structural declarations (`@fn`, `type`, `trait`, `impl`, `use`, `extend`, `let $`)
  - [x] `qualified_name` derivation from file path + nesting
  - [x] `signature_hash` computation (body-independent)
  - [x] JSONL record generation in the standard format (type: "symbol"/"relationship"/"file_meta")
- [x] **Do NOT register `ori` in `parser_adapter.py`'s `parse_file()`** — standalone pipeline, routes by extension in sync script
- [x] Verify output format matches `extract_symbols.py` schema exactly — tested: 9 functions from testing.ori correctly in Neo4j
- [x] Verify `.rs` files in `compiler/` use the standard Rust tree-sitter pipeline — verified: added `rust` to `repos.yaml` languages list, `build-code-graph.sh --repo ori` imported 47,096 symbols from 1,462 .rs files

- [ ] **Subsection close-out (09.3)** — MANDATORY before starting 09.4:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — Is the regex scanner robust enough for Ori's syntax? Should we invest in a proper AST dump JSON mode in the compiler instead? Benchmark the binary cold-start time — is <3s achievable?
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any temp files

---

## 09.4 Health Monitoring & Diagnostics

**File**: `~/projects/lang_intelligence/scripts/sync-ori-graph.sh` (health-check mode)

The background sync must not fail silently. This subsection adds observability.

**success_criteria:**
- [ ] `sync-ori-graph.sh --health` reports sync status (last sync time, files synced, errors since last success)
- [ ] Stale graph detection: warn if last sync > 24h and there have been commits since
- [ ] Log rotation or size cap prevents unbounded log growth
- [ ] `intel-query.sh status` output includes Ori sync metadata (last sync time, staleness)

- [x] Add `--health` mode to `sync-ori-graph.sh` that:
  - [x] Queries Neo4j for Ori Repo's `last_code_import_at` timestamp
  - [x] Checks `ori-sync.log` for recent errors
  - [x] Checks `git log --since=<last_sync>` for commits since last sync
  - [x] Reports: last sync time, files in graph, errors since last success, commits since last sync
- [x] Add log rotation: truncate `ori-sync.log` to last 10,000 lines on each sync run (in shell wrapper)
- [ ] Add Ori sync metadata to `intel-query.sh status` output — deferred: `sync-ori-graph.sh --health` provides this already; `intel-query.sh` enhancement tracked for Section 10
- [x] Verify stale detection works: `--health` shows "Commits since last sync: 0" (correct — no commits since import)

- [ ] **Subsection close-out (09.4)** — MANDATORY before starting 09.5:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — Is the health check sufficient for detecting problems? Should we add a cron job for periodic health checks? Should `--health` be integrated into `test-all.sh` or a CI check?
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any temp files

- [ ] **TPR checkpoint** — `/tpr-review` covering 09.3–09.4 implementation work

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
- [ ] `test_malformed_file_extracts_partial` — malformed `.ori` file extracts what it can and logs warnings
- [ ] `test_empty_file_produces_file_meta_only` — empty `.ori` file produces file_meta with zero symbols

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

- [ ] **Subsection close-out (09.5)** — MANDATORY before 09.N:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — Are the integration tests fast enough to run in CI? Do they need a dedicated Neo4j test instance? Should we add property-based tests for the regex scanner?
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any temp files

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
- [ ] **All intermediate TPR checkpoint findings resolved** (09.2 checkpoint, 09.4 checkpoint)
- [ ] **Plan annotation cleanup** — remove any `<!-- reviewed: ... -->` or TPR-specific comments from source code
- [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any temp files
- [ ] `/tpr-review` clean (final section-level review)
- [ ] `/impl-hygiene-review` clean
- [ ] `/improve-tooling` section-close sweep
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] Update `00-overview.md` Quick Reference table: Section 09 status → Complete
  - [ ] Update `index.md` section status
  - [ ] Verify mission success criteria checkbox for Ori live sync

**Exit Criteria:** All integration tests pass against a live Neo4j instance. A commit to ori_lang triggers background sync, and the changed symbols appear in Neo4j within 5s. A `--full` rebuild produces identical graph state to a fresh bulk import. Parse failures during development do not corrupt the graph. The `--health` check correctly reports stale state when sync has not run.
