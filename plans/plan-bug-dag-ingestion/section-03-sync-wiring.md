---
section: "03"
title: "Commit-triggered sync wiring"
status: not-started
reviewed: true
goal: "Wire commit-triggered plan/bug graph ingestion: sync_plan_bug_graph.py driver + sync-plan-bug-graph.sh fire-and-forget wrapper + lefthook post-commit intel-plan-sync entry scoped to plans/**. Full-rebuild on any plans/** change. Graceful degradation when lang_intelligence is unreachable."
success_criteria:
  - "~/projects/lang_intelligence/neo4j/sync_plan_bug_graph.py: CLI surface --full | --incremental (stub, forwards to --full for Phase 1 per overview Design Principle 3) | --health | --bootstrap modes mirroring sync_ori_graph.py:436-472; invokes python -m scripts.plan_corpus export on ori_lang side + pipes to import_plan_bug_graph.py."
  - "~/projects/lang_intelligence/scripts/sync-plan-bug-graph.sh: flock-based lock on .plan-bug-sync.lock; log rotation at 10k lines (tail -n 10000 idiom); venv activation via $PROJECT_DIR/.venv/bin/activate; fire-and-forget with exit 0 on any path; env-var driven (NEO4J_*, ORI_INTEL_DIR, ORI_LANG_ROOT)."
  - "ori_lang/lefthook.yml post-commit gains a second commands: entry intel-plan-sync that invokes ../lang_intelligence/scripts/sync-plan-bug-graph.sh when any plans/** file changes; uses git diff-tree --name-only with glob filter; runs in background with >> log redirection mirroring existing intel-sync entry at lines 30-40."
  - "Commit latency budget: touch plans/<anything>.md + git commit returns in <100ms (hook does not block on sync); background sync completes in <10s (measured by comparing commit timestamp to log's 'sync-complete' line)."
  - "Graceful degradation: sync-plan-bug-graph.sh exits 0 when (a) lang_intelligence directory missing, (b) Neo4j container stopped, (c) venv missing, (d) export subcommand fails. Each path logs a reason to stderr and to plan-bug-sync.log; no commit fails due to sync failure."
  - "End-to-end smoke: git commit -m 'test' on a plans/plan-bug-dag-ingestion/ edit → ~/projects/lang_intelligence/logs/plan-bug-sync.log shows entry line within 100ms + 'sync-complete' line within 10s + Neo4j reflects the commit via intel-query.sh cypher."
  - "Concurrency: two rapid commits produce exactly one lock-holder + one lock-skipper; both exit 0; final Neo4j state matches second commit (flock -n 200 is the canonical pattern from sync-ori-graph.sh:89-93)."
  - "Satisfies mission criterion: '~/projects/lang_intelligence/scripts/sync-plan-bug-graph.sh and ~/projects/lang_intelligence/neo4j/sync_plan_bug_graph.py implement commit-triggered full corpus rebuild...'."
inspired_by:
  - "lang_intelligence scripts/sync-ori-graph.sh — canonical sync wrapper (flock + log rotation + venv + fire-and-forget)"
  - "lang_intelligence neo4j/sync_ori_graph.py — --full/--changed/--health/--bootstrap CLI surface"
  - "ori_lang lefthook.yml post-commit intel-sync — existing fire-and-forget pattern"
depends_on: ["02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Write sync_plan_bug_graph.py driver"
    status: not-started
  - id: "03.2"
    title: "Write sync-plan-bug-graph.sh wrapper"
    status: not-started
  - id: "03.3"
    title: "Extend ori_lang lefthook.yml post-commit with intel-plan-sync entry"
    status: not-started
  - id: "03.4"
    title: "End-to-end smoke test + concurrency test"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Commit-triggered sync wiring

**Status:** Not Started
**Goal:** Wire the full commit-triggered ingestion pipeline so that any `git commit` touching `plans/**` automatically triggers a fire-and-forget full corpus rebuild into Neo4j — with no commit latency, graceful degradation when `lang_intelligence` is unavailable, and flock-based concurrency safety. When this section is complete, the system is live: developers get plan/bug graph queries updated automatically on every commit, with the same "set it and forget it" reliability as the existing `intel-sync` hook for compiler code.

**Success Criteria:**

- [ ] `~/projects/lang_intelligence/neo4j/sync_plan_bug_graph.py` has `--full | --incremental | --health | --bootstrap` CLI modes; `--incremental` is a stub that logs the stub rationale and forwards to `--full` (Design Principle 3 from `00-overview.md`); `--full` invokes `python -m scripts.plan_corpus export` in the ori_lang root and pipes stdout to `import_plan_bug_graph.py --input -`
- [ ] `~/projects/lang_intelligence/scripts/sync-plan-bug-graph.sh` mirrors `sync-ori-graph.sh` structure: `set -euo pipefail`, SCRIPT_DIR resolution, `$LOG_DIR/plan-bug-sync.log`, 10k-line rotation, venv activation, argument parsing, flock non-blocking lock, `exit 0` on all degradation paths
- [ ] `ori_lang/lefthook.yml` `post-commit:` block has a second entry `intel-plan-sync` with `glob`-filtered trigger on `plans/**/*.md` and fire-and-forget `&` background invocation
- [ ] `time git commit` on a `plans/**` change completes in under 100ms (hook does not wait for sync)
- [ ] `~/projects/lang_intelligence/logs/plan-bug-sync.log` shows `sync-complete` within 10s of a triggering commit
- [ ] Two rapid commits: lock-skip message appears for the second commit; both commits succeed; Neo4j reflects second commit's state
- [ ] Graceful degradation verified: each of the four degradation paths (dir missing, container stopped, venv missing, export fail) exits 0 and logs its reason
- [ ] `bash ~/projects/lang_intelligence/tests/test_sync_plan_bug_integration.sh` green (all matrix cells pass)
- [ ] `shellcheck ~/projects/lang_intelligence/scripts/sync-plan-bug-graph.sh` exits 0 with no warnings
- [ ] Satisfies mission criterion: `grep -c "sync-complete" ~/projects/lang_intelligence/logs/plan-bug-sync.log` is non-zero after a trigger commit

**Context:** The existing `intel-sync` post-commit hook (`lefthook.yml:30-40`) fires `sync-ori-graph.sh --changed "$CHANGED"` for compiler and library file changes. That wrapper uses `flock -n` for non-blocking concurrency, `tail -n 10000` log rotation, venv activation, and unconditional `exit 0` for fire-and-forget semantics. The plan/bug sync is architecturally identical — same repo layout, same lefthook pattern, same graceful-degradation contract — but with one key difference: because `dag.py`'s classifiers are whole-corpus operations (cycle detection, transitive closure, subsystem clustering all depend on the global graph state), incremental sync is unsound. Every trigger is a full rebuild, regardless of which file changed. This is by design: see `00-overview.md` Design Principle 3.

**Reference implementations:**
- **lang_intelligence** `scripts/sync-ori-graph.sh`: flock pattern, log rotation idiom, venv activation, argument parsing — the exact template for `sync-plan-bug-graph.sh`. Lines 89-93 are the canonical `exec 200>$LOCK_FILE; flock -n 200` block. Lines 24-29 are the canonical 10k-line rotation. Lines 33-40 are the venv activation guard.
- **lang_intelligence** `neo4j/sync_ori_graph.py`: `argparse` with mutually exclusive group, `get_driver()`, `bootstrap_repo()`, `run_full()`, `run_incremental()`, `run_health()` — structure mirrors directly to `sync_plan_bug_graph.py` with corpus-export substituted for the tree-sitter extraction pipeline.
- **ori_lang** `lefthook.yml:27-40`: the `intel-sync` post-commit entry — `git diff-tree --name-only`, glob filter, `mkdir -p logs`, background `&` invocation. `intel-plan-sync` mirrors this entry with different globs and a different script path.

**Depends on:** Section 02 (§02) — `import_plan_bug_graph.py` must exist and accept `--input -` (stdin) before `sync_plan_bug_graph.py`'s `run_full()` can pipe to it. Section 01's `python -m scripts.plan_corpus export` subcommand must also be landed before `run_full()` can invoke it.

---

## Intelligence Reconnaissance

Queries run 2026-04-17:

- `scripts/intel-query.sh --human search "commit hook sync" --limit 5` — 5 results (TypeScript dprint pre-commit hook, Rust pre-commit fmt hook, Go codereview commit-msg hook, Rust pre-push hook). No direct fire-and-forget post-commit sync prior art in any repo. Graph is available (Neo4j 5.26.24, 191K+ symbols).
- `scripts/intel-query.sh --human symbols "flock" --repo ori` — 0 results. `flock` is a shell built-in, not a Rust/Ori symbol; the graph indexes compiled-language symbols. Expected absence.
- `scripts/intel-query.sh --human file-symbols "lefthook" --repo ori` — 0 results. `lefthook.yml` is a YAML config file, not indexed by the symbol extractor. Expected absence.
- `scripts/intel-query.sh --human similar "fire and forget hook" --repo rust,go,typescript --limit 5` — no embedding found for the phrase; vector similarity requires pre-indexed function-level embeddings. Expected for a natural-language query with no matching symbol.

Results summary [ori]: Graph available (32K+ Ori symbols, 505K+ CALLS edges). No cross-repo prior art for fire-and-forget post-commit sync found — the pattern is shell-idiomatic (flock + `&` background), not a language-level construct that would appear in compiler symbol graphs. All four queries confirm the implementation is in shell/Python plumbing with no Ori-codebase blast radius. The canonical reference is the existing `sync-ori-graph.sh` in the same repo — a direct template, not a design discovery.

---

## 03.1 Write sync_plan_bug_graph.py driver

**File:** `~/projects/lang_intelligence/neo4j/sync_plan_bug_graph.py` (~80 lines)

This driver is the Python backend for `sync-plan-bug-graph.sh`. It mirrors `sync_ori_graph.py`'s argparse CLI surface and module structure, substituting the tree-sitter extraction pipeline with a subprocess call to `python -m scripts.plan_corpus export`. The `--incremental` mode is a documented Phase 1 stub — it accepts the argument (so the shell wrapper can pass `--incremental` without breaking) but logs the forwarding rationale and delegates to `run_full()`. This stub-with-forwarding preserves the CLI contract for future Phase 2 without polluting the Phase 1 implementation with speculative incremental logic.

- [ ] Create `~/projects/lang_intelligence/neo4j/sync_plan_bug_graph.py` with the following structure:

  ```python
  #!/usr/bin/env python3
  """Commit-triggered sync for the plan/bug DAG in Neo4j.

  Full mode: invoke `python -m scripts.plan_corpus export` in ori_lang root,
  pipe stdout JSON envelope to import_plan_bug_graph.py --input -.

  Incremental mode (Phase 1 stub): forwards to run_full() — dag.py classifiers
  are whole-corpus operations; fine-grained incremental would produce incorrect
  classifier output. See plans/plan-bug-dag-ingestion/00-overview.md Design
  Principle 3.

  Bootstrap mode: create/verify :Repo{name:"ori"} node (idempotent MERGE).
  Health mode: Neo4j connection check + plan/bug node count.

  Python backend for scripts/sync-plan-bug-graph.sh (fire-and-forget).
  """
  import argparse
  import json
  import logging
  import os
  import subprocess
  import sys
  from pathlib import Path

  sys.path.insert(0, str(Path(__file__).parent))

  from neo4j import GraphDatabase
  from neo4j.exceptions import ServiceUnavailable

  from import_code_graph import NEO4J_URI, NEO4J_USER, NEO4J_PASS

  logger = logging.getLogger("sync_plan_bug_graph")

  ORI_LANG_ROOT = os.environ.get(
      "ORI_LANG_ROOT",
      str(Path(__file__).parent.parent.parent / "ori_lang"),
  )
  PLAN_BUG_SCHEMA_VERSION = "1.0"


  def get_driver():
      return GraphDatabase.driver(NEO4J_URI, auth=(NEO4J_USER, NEO4J_PASS))


  def run_full() -> int:
      """Full corpus rebuild: export + import."""
      ori_root = Path(ORI_LANG_ROOT)
      if not ori_root.exists():
          logger.error("ORI_LANG_ROOT not found: %s", ori_root)
          return 1

      neo4j_dir = Path(__file__).parent
      importer = neo4j_dir / "import_plan_bug_graph.py"
      if not importer.exists():
          logger.error("import_plan_bug_graph.py not found: %s", importer)
          return 1

      logger.info("Starting full plan/bug corpus rebuild from %s", ori_root)

      # Export corpus to JSON envelope (stdout) and pipe directly to importer
      export_proc = subprocess.Popen(
          [sys.executable, "-m", "scripts.plan_corpus", "export"],
          cwd=str(ori_root),
          stdout=subprocess.PIPE,
          stderr=subprocess.PIPE,
      )
      import_proc = subprocess.Popen(
          [sys.executable, str(importer), "--input", "-"],
          stdin=export_proc.stdout,
          stderr=subprocess.PIPE,
      )
      # Close exporter's stdout in parent so importer gets EOF when exporter exits
      export_proc.stdout.close()

      _import_stderr = import_proc.communicate()[0]
      export_proc.wait()

      if export_proc.returncode != 0:
          logger.error("plan_corpus export failed (exit %d)", export_proc.returncode)
          return 1
      if import_proc.returncode != 0:
          logger.error("import_plan_bug_graph failed (exit %d)", import_proc.returncode)
          return 1

      logger.info("sync-complete: full plan/bug rebuild succeeded")
      return 0


  def run_incremental(changed_paths: str) -> int:
      """Phase 1 stub: forwards to run_full().

      Rationale: dag.py classifiers (cycle detection, transitive closure,
      subsystem clustering) are whole-corpus operations. A fine-grained
      incremental that only processes changed files would produce incorrect
      classifier output because classifier outputs depend on relationships
      between nodes that weren't just edited. See Design Principle 3 in
      plans/plan-bug-dag-ingestion/00-overview.md.

      Phase 2 (future): replace this stub with a true incremental that
      re-runs affected classifiers only. The CLI shape is preserved here
      so the shell wrapper can pass --incremental without script changes.
      """
      logger.info(
          "Incremental mode (Phase 1 stub): forwarding to full rebuild "
          "(changed: %s)", changed_paths
      )
      return run_full()


  def run_health() -> int:
      """Quick Neo4j connection + plan/bug node count check."""
      try:
          driver = get_driver()
          with driver.session() as session:
              result = session.run(
                  "MATCH (n) WHERE n:Plan OR n:Bug OR n:PlanSection "
                  "RETURN count(n) AS total"
              )
              row = result.single()
              total = row["total"] if row else 0
          driver.close()
          print(json.dumps({"status": "ok", "details": {"plan_bug_nodes": total}}))
          return 0
      except ServiceUnavailable as e:
          print(json.dumps({"status": "degraded", "details": {"error": str(e)}}))
          return 1


  def run_bootstrap() -> int:
      """Create :Repo{name:'ori'} node if missing (idempotent MERGE)."""
      try:
          driver = get_driver()
          with driver.session() as session:
              session.run(
                  "MERGE (r:Repo {name: 'ori'}) "
                  "SET r.full_name = 'ori-lang/ori', r.is_custom = true",
              )
          driver.close()
          logger.info("Plan/bug graph bootstrap: :Repo{name:'ori'} ensured")
          return 0
      except ServiceUnavailable as e:
          logger.warning("Bootstrap skipped: Neo4j unavailable (%s)", e)
          return 0  # Degradation: skip, don't fail


  def main():
      parser = argparse.ArgumentParser(description="Plan/bug graph live sync")
      group = parser.add_mutually_exclusive_group(required=True)
      group.add_argument("--full", action="store_true", help="Full corpus rebuild")
      group.add_argument(
          "--incremental",
          metavar="CHANGED",
          help="Changed paths (Phase 1: forwards to --full)",
      )
      group.add_argument("--health", action="store_true", help="Connection + node count check")
      group.add_argument("--bootstrap", action="store_true", help="Ensure :Repo node exists")
      parser.add_argument(
          "--ori-lang-root",
          default=ORI_LANG_ROOT,
          help="Path to ori_lang checkout (default: $ORI_LANG_ROOT or sibling dir)",
      )
      args = parser.parse_args()

      # Allow --ori-lang-root to override env default
      global ORI_LANG_ROOT
      ORI_LANG_ROOT = args.ori_lang_root

      logging.basicConfig(
          level=logging.INFO,
          format="%(asctime)s %(levelname)s %(name)s: %(message)s",
          datefmt="%Y-%m-%dT%H:%M:%S",
      )

      if args.full:
          sys.exit(run_full())
      elif args.incremental is not None:
          sys.exit(run_incremental(args.incremental))
      elif args.health:
          sys.exit(run_health())
      elif args.bootstrap:
          sys.exit(run_bootstrap())


  if __name__ == "__main__":
      main()
  ```

- [ ] Verify that `run_full()` logs `sync-complete` on success — the end-to-end smoke test in §03.4 polls this line to confirm sync completion within the 10s budget
- [ ] Verify `run_incremental()` stub docstring cites Design Principle 3 by location (`00-overview.md`) so future implementers find the context without searching
- [ ] Verify `run_bootstrap()` returns 0 on `ServiceUnavailable` (degradation path — bootstrap is best-effort, never a blocker)
- [ ] Verify `run_health()` returns 0 on success and 1 on `ServiceUnavailable` — the shell wrapper checks this return code for the `--health` mode bypass path
- [ ] The `--ori-lang-root` flag must default to `$ORI_LANG_ROOT` env var or `../../ori_lang` relative to the script — mirrors `sync_ori_graph.py`'s `ORI_LANG_ROOT` resolution

- [ ] **Subsection close-out (03.1)** — MANDATORY before starting 03.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check`; clean any temp files accumulated during driver development.

---

## 03.2 Write sync-plan-bug-graph.sh wrapper

**File:** `~/projects/lang_intelligence/scripts/sync-plan-bug-graph.sh` (~100 lines)

This wrapper is the shell layer that `lefthook` invokes. It mirrors `sync-ori-graph.sh` exactly: same file layout, same safety patterns, same degradation contract. The only structural difference is the lock file name (`.plan-bug-sync.lock`), log file name (`plan-bug-sync.log`), and the Python driver it invokes (`sync_plan_bug_graph.py`). The `--changed` flag from `sync-ori-graph.sh` is replaced by `--full` (matching Phase 1 design), but `--incremental` is also wired to preserve future compatibility.

Graceful degradation is the highest-priority invariant: **every exit path exits 0**. This is the "additive, never blocking" rule from `.claude/rules/intelligence.md`. A commit that touches `plans/` must never fail because Neo4j is down.

The four degradation paths that must each log a reason and exit 0:
1. `$INTEL_DIR` missing — `lang_intelligence` repo not cloned
2. `$INTEL_DIR/.venv` missing — virtual environment not set up
3. `docker inspect lang-intelligence` fails — Neo4j container stopped
4. `python -m scripts.plan_corpus export` fails — corpus export error

**Critical code blocks (mirroring `sync-ori-graph.sh` line-for-line where possible):**

- [ ] Create `~/projects/lang_intelligence/scripts/sync-plan-bug-graph.sh` with the following canonical structure:

  ```bash
  #!/usr/bin/env bash
  # Sync the plan/bug DAG from ori_lang into Neo4j (full rebuild only in Phase 1).
  # Lives in ~/projects/lang_intelligence/scripts/
  #
  # Usage:
  #   sync-plan-bug-graph.sh --full       # full corpus rebuild (Phase 1 default)
  #   sync-plan-bug-graph.sh --incremental "f1.md f2.md"  # Phase 1: forwards to --full
  #   sync-plan-bug-graph.sh --bootstrap  # create :Repo node only (idempotent)
  #   sync-plan-bug-graph.sh --health     # report sync status
  #
  # Called by lefthook post-commit hook in ori_lang (fire-and-forget).
  # All output goes to logs/plan-bug-sync.log via the hook's redirection.
  set -euo pipefail

  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
  PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
  LOCK_FILE="$PROJECT_DIR/.plan-bug-sync.lock"
  LOG_DIR="$PROJECT_DIR/logs"
  NEO4J_DIR="$PROJECT_DIR/neo4j"
  ORI_LANG_ROOT="${ORI_LANG_ROOT:-$(dirname "$PROJECT_DIR")/ori_lang}"

  # ── Graceful degradation (all paths exit 0 — never fail the commit hook) ──

  if [[ ! -d "$PROJECT_DIR" ]]; then
      echo "$(date -Iseconds) SKIP: lang_intelligence not found at $PROJECT_DIR" >&2
      exit 0
  fi

  if [[ ! -f "$PROJECT_DIR/.venv/bin/activate" ]]; then
      echo "$(date -Iseconds) SKIP: .venv not found at $PROJECT_DIR/.venv" >&2
      exit 0
  fi

  # Check if Neo4j container is running (non-blocking — if docker isn't installed, skip)
  if command -v docker &>/dev/null; then
      if ! docker inspect lang-intelligence &>/dev/null 2>&1; then
          echo "$(date -Iseconds) SKIP: docker container 'lang-intelligence' not running" >&2
          exit 0
      fi
  fi

  mkdir -p "$LOG_DIR"

  # ── Log rotation: keep last 10,000 lines (mirrors sync-ori-graph.sh:24-29) ──
  LOG_FILE="$LOG_DIR/plan-bug-sync.log"
  if [[ -f "$LOG_FILE" ]]; then
      LINE_COUNT=$(wc -l < "$LOG_FILE")
      if (( LINE_COUNT > 10000 )); then
          tail -n 10000 "$LOG_FILE" > "$LOG_FILE.tmp" && mv "$LOG_FILE.tmp" "$LOG_FILE"
      fi
  fi

  # ── Auto-activate venv (mirrors sync-ori-graph.sh:33-40) ──
  if [[ -z "${VIRTUAL_ENV:-}" ]]; then
      # shellcheck disable=SC1091
      source "$PROJECT_DIR/.venv/bin/activate"
  fi

  # ── Parse arguments (mirrors sync-ori-graph.sh:43-75) ──
  MODE=""
  CHANGED_FILES=""
  while [[ $# -gt 0 ]]; do
      case "$1" in
          --incremental)
              MODE="incremental"
              CHANGED_FILES="${2:-}"
              shift 2 || shift
              ;;
          --full)
              MODE="full"
              shift
              ;;
          --bootstrap)
              MODE="bootstrap"
              shift
              ;;
          --health)
              MODE="health"
              shift
              ;;
          *)
              echo "$(date -Iseconds) ERROR: unknown argument: $1" >&2
              exit 0  # Degradation: unknown arg exits 0 (never fail hook)
              ;;
      esac
  done

  if [[ -z "$MODE" ]]; then
      echo "$(date -Iseconds) ERROR: no mode specified" >&2
      exit 0
  fi

  # ── Health + bootstrap bypass lock (mirrors sync-ori-graph.sh:77-87) ──
  if [[ "$MODE" == "health" ]]; then
      python3 "$NEO4J_DIR/sync_plan_bug_graph.py" --health || true
      exit 0
  fi

  if [[ "$MODE" == "bootstrap" ]]; then
      python3 "$NEO4J_DIR/sync_plan_bug_graph.py" --bootstrap || true
      exit 0
  fi

  # ── Acquire lock: non-blocking, skip if held (mirrors sync-ori-graph.sh:89-93) ──
  exec 200>"$LOCK_FILE"
  if ! flock -n 200; then
      echo "$(date -Iseconds) SKIP: another plan-bug sync is already running (lock held)" >&2
      exit 0
  fi

  # Ensure lock is released on exit (even on error)
  trap 'flock -u 200 2>/dev/null || true' EXIT

  # Auto-bootstrap on every sync run (idempotent; failure is non-fatal)
  python3 "$NEO4J_DIR/sync_plan_bug_graph.py" --bootstrap 2>/dev/null || true

  # ── Main sync ──
  if [[ "$MODE" == "incremental" ]]; then
      python3 "$NEO4J_DIR/sync_plan_bug_graph.py" --incremental "${CHANGED_FILES:-}" \
          --ori-lang-root "$ORI_LANG_ROOT" || {
          echo "$(date -Iseconds) ERROR: sync_plan_bug_graph.py --incremental failed" >&2
          exit 0  # Degradation: failure exits 0 (never fail hook)
      }
  elif [[ "$MODE" == "full" ]]; then
      python3 "$NEO4J_DIR/sync_plan_bug_graph.py" --full \
          --ori-lang-root "$ORI_LANG_ROOT" || {
          echo "$(date -Iseconds) ERROR: sync_plan_bug_graph.py --full failed" >&2
          exit 0  # Degradation: failure exits 0 (never fail hook)
      }
  fi

  exit 0
  ```

- [ ] All four degradation paths verified to exit 0 and log a reason to stderr: (a) `PROJECT_DIR` missing, (b) `.venv` missing, (c) docker container not running, (d) Python driver exits non-zero
- [ ] The trap `EXIT` handler releases the flock fd even on unexpected failures — prevents stale locks from blocking future syncs indefinitely
- [ ] `set -euo pipefail` is at the top; the `|| true` / `|| { ... exit 0; }` patterns are the correct way to handle expected-failures under `set -e` without turning off the safety net globally
- [ ] `${VAR:-default}` idiom used for all env vars with defaults (`ORI_LANG_ROOT`, `VIRTUAL_ENV`, `CHANGED_FILES`)
- [ ] `shellcheck` must pass: `exec 200>` is flagged safe by `SC1091`-exempt annotation on the `source` line; quote all variable expansions; `--` separators before glob arguments where applicable
- [ ] Lock file is `.plan-bug-sync.lock` (NOT `.ori-sync.lock` — separate lock namespace to allow both sync scripts to run concurrently for different pipelines)

- [ ] **Subsection close-out (03.2)** — MANDATORY before starting 03.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`.

---

## 03.3 Extend ori_lang lefthook.yml post-commit with intel-plan-sync entry

**File:** `ori_lang/lefthook.yml`

The existing `intel-sync` entry (lines 29-40) fires `sync-ori-graph.sh --changed "$CHANGED"` for commits touching `compiler/*.rs` or `library/*.ori`. The new `intel-plan-sync` entry is structurally identical but fires `sync-plan-bug-graph.sh --full` for commits touching `plans/**/*.md`.

**Why a second entry, not merged with intel-sync?** The two sync pipelines are architecturally orthogonal:
- `intel-sync` calls `sync-ori-graph.sh --changed` (true incremental: per-file symbol extraction with tree-sitter, atomic per-file upsert)
- `intel-plan-sync` calls `sync-plan-bug-graph.sh --full` (always full-rebuild: DAG classifiers are whole-corpus operations, incremental is unsound)

Merging them into one entry would conflate two different sync models behind a single hook invocation, making the logic harder to reason about and making future Phase 2 incremental-plan-sync impossible to isolate. Two independent entries are the correct decomposition: each fires independently, neither blocks the other, order doesn't matter (both are fire-and-forget).

- [ ] Edit `ori_lang/lefthook.yml` to add `intel-plan-sync` to the `post-commit: commands:` block, immediately after the existing `intel-sync` entry:

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
        # Scoped to compiler/ and library/ per repos.yaml include roots
      intel-plan-sync:
        run: |
          if [ -x ../lang_intelligence/scripts/sync-plan-bug-graph.sh ]; then
            CHANGED=$(git diff-tree --no-commit-id --name-only -r HEAD -- 'plans/*.md' 'plans/**/*.md')
            if [ -n "$CHANGED" ]; then
              mkdir -p ../lang_intelligence/logs
              ../lang_intelligence/scripts/sync-plan-bug-graph.sh --full >> ../lang_intelligence/logs/plan-bug-sync.log 2>&1 &
            fi
          fi
        # Fire-and-forget: returns immediately, full corpus rebuild runs in background
        # If lang_intelligence doesn't exist, the -x test fails silently
        # Scoped to plans/**.md — any plan file change triggers a full DAG rebuild
        # (dag.py classifiers are whole-corpus: incremental is unsound, see §03 context)
  ```

- [ ] Verify the glob patterns cover the actual file layout: `plans/*.md` catches top-level plan files (e.g. `plans/roadmap/index.md`); `plans/**/*.md` catches nested files (e.g. `plans/plan-bug-dag-ingestion/section-03-sync-wiring.md`). Both patterns are needed because `git diff-tree --name-only` on `plans/*.md` does NOT recurse into subdirectories — the `plans/**/*.md` second pattern is load-bearing.
- [ ] Verify the `2>&1 &` pattern: stderr is redirected to the log before backgrounding, so sync errors appear in `plan-bug-sync.log` even if the terminal has been closed. This matches the `intel-sync` pattern exactly.
- [ ] Verify that `intel-sync` is unchanged — the edit is additive only. Run `git diff lefthook.yml` after the edit to confirm no unintended modifications to the existing entry.
- [ ] Run a test commit touching a `plans/` file and verify `plan-bug-sync.log` receives an entry within 100ms of the commit returning (hook returns before the sync completes — only the entry timestamp matters here)

- [ ] **Subsection close-out (03.3)** — MANDATORY before starting 03.4:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`.

---

## 03.4 End-to-end smoke test + concurrency test

**File:** `~/projects/lang_intelligence/tests/test_sync_plan_bug_integration.sh` (~60 lines)

This integration test verifies the full commit-triggered pipeline end-to-end. It uses a matrix of **trigger types × graph states × degradation scenarios** per CLAUDE.md §Matrix Testing Rule.

**Matrix dimensions:**

| Axis | Values |
|------|--------|
| Trigger type | plan file edit, new plan directory, new section file, new bug entry, new fix-BUG file, compiler-only file edit (negative) |
| Graph state | empty (bootstrap path), populated (stale prune path) |
| Degradation scenario | docker stopped, venv missing, `ORI_INTEL_DIR` points to missing directory |

**Semantic pin:** `test_commit_on_plan_file_triggers_sync_within_10s` — actual `git commit` + poll `plan-bug-sync.log` for `sync-complete` line within 10s. This test only passes if the full pipeline is wired correctly: hook fires → wrapper runs → driver exports → importer ingests → `run_full()` logs `sync-complete`.

**Negative pin:** `test_commit_on_compiler_file_does_NOT_trigger_plan_sync` — edit `compiler/oric/src/main.rs`, commit, wait 1s, verify `plan-bug-sync.log` has no new entry (only `ori-sync.log` receives an entry from `intel-sync`). This pin fails if the glob patterns in `intel-plan-sync` are too broad.

- [ ] Create `~/projects/lang_intelligence/tests/test_sync_plan_bug_integration.sh` with the following structure and matrix:

  ```bash
  #!/usr/bin/env bash
  # Integration tests for sync-plan-bug-graph.sh + lefthook intel-plan-sync wiring.
  # Requires: a running Neo4j instance, lang_intelligence venv, ori_lang checkout.
  # Run: bash ~/projects/lang_intelligence/tests/test_sync_plan_bug_integration.sh
  set -euo pipefail

  PASS=0; FAIL=0
  INTEL_DIR="$(cd "$(dirname "$0")/.." && pwd)"
  ORI_LANG_ROOT="${ORI_LANG_ROOT:-$(dirname "$INTEL_DIR")/ori_lang}"
  LOG_FILE="$INTEL_DIR/logs/plan-bug-sync.log"
  SYNC_SCRIPT="$INTEL_DIR/scripts/sync-plan-bug-graph.sh"

  pass() { echo "PASS: $1"; ((PASS++)); }
  fail() { echo "FAIL: $1"; ((FAIL++)); }

  # ── Matrix cell: trigger = plan file edit, state = populated ──
  test_commit_on_plan_file_triggers_sync_within_10s() {
      local before_lines log_after deadline
      before_lines=$(wc -l < "$LOG_FILE" 2>/dev/null || echo 0)
      # Make a trivial change to a plans/ file and commit
      touch "$ORI_LANG_ROOT/plans/plan-bug-dag-ingestion/section-03-sync-wiring.md"
      git -C "$ORI_LANG_ROOT" add -- plans/plan-bug-dag-ingestion/section-03-sync-wiring.md
      git -C "$ORI_LANG_ROOT" commit --no-verify -m "test(sync): integration smoke trigger"
      # Poll for sync-complete within 10s
      deadline=$(( $(date +%s) + 10 ))
      while (( $(date +%s) < deadline )); do
          log_after=$(tail -n +$(( before_lines + 1 )) "$LOG_FILE" 2>/dev/null || echo "")
          if echo "$log_after" | grep -q "sync-complete"; then
              pass "plan file commit → sync-complete within 10s"
              return
          fi
          sleep 0.5
      done
      fail "plan file commit → sync-complete NOT seen within 10s"
  }

  # ── Matrix cell: trigger = compiler-only edit (negative pin) ──
  test_commit_on_compiler_file_does_NOT_trigger_plan_sync() {
      local before_lines
      before_lines=$(wc -l < "$LOG_FILE" 2>/dev/null || echo 0)
      # Touch a compiler file only
      touch "$ORI_LANG_ROOT/compiler/oric/src/main.rs"
      git -C "$ORI_LANG_ROOT" add -- compiler/oric/src/main.rs
      git -C "$ORI_LANG_ROOT" commit --no-verify -m "test(sync): compiler-only edit (negative)"
      sleep 1  # Give any spurious hook invocation time to appear
      local log_after
      log_after=$(tail -n +$(( before_lines + 1 )) "$LOG_FILE" 2>/dev/null || echo "")
      if echo "$log_after" | grep -q "sync-complete\|plan-bug-sync"; then
          fail "compiler-only commit should NOT trigger plan-bug sync — but did"
      else
          pass "compiler-only commit does not trigger plan-bug sync"
      fi
  }

  # ── Matrix cell: degradation = docker container stopped ──
  test_docker_stopped_exits_zero_and_logs_reason() {
      docker stop lang-intelligence 2>/dev/null || true
      local log_before log_after exit_code
      log_before=$(wc -l < "$LOG_FILE" 2>/dev/null || echo 0)
      "$SYNC_SCRIPT" --full >> "$LOG_FILE" 2>&1; exit_code=$?
      docker start lang-intelligence 2>/dev/null || true
      if [[ $exit_code -ne 0 ]]; then
          fail "wrapper should exit 0 when docker stopped (got $exit_code)"
          return
      fi
      log_after=$(tail -n +$(( log_before + 1 )) "$LOG_FILE" 2>/dev/null || echo "")
      if echo "$log_after" | grep -q "SKIP\|not running"; then
          pass "docker stopped: exits 0, logs SKIP reason"
      else
          fail "docker stopped: exit 0 but no SKIP reason logged"
      fi
  }

  # ── Matrix cell: degradation = venv missing ──
  test_venv_missing_exits_zero() {
      local venv_bak="$INTEL_DIR/.venv.bak_test"
      mv "$INTEL_DIR/.venv" "$venv_bak" 2>/dev/null || { pass "venv missing test skipped (venv not present)"; return; }
      local exit_code
      "$SYNC_SCRIPT" --full >> "$LOG_FILE" 2>&1; exit_code=$?
      mv "$venv_bak" "$INTEL_DIR/.venv"
      if [[ $exit_code -eq 0 ]]; then
          pass "venv missing: exits 0"
      else
          fail "venv missing: should exit 0 (got $exit_code)"
      fi
  }

  # ── Concurrency: two rapid commits → one lock-holder, one lock-skipper ──
  test_concurrent_commits_flock_skip() {
      local log_before
      log_before=$(wc -l < "$LOG_FILE" 2>/dev/null || echo 0)
      # Simulate two concurrent invocations
      "$SYNC_SCRIPT" --full >> "$LOG_FILE" 2>&1 &
      "$SYNC_SCRIPT" --full >> "$LOG_FILE" 2>&1 &
      wait
      local log_after
      log_after=$(tail -n +$(( log_before + 1 )) "$LOG_FILE" 2>/dev/null || echo "")
      if echo "$log_after" | grep -q "another plan-bug sync is already running"; then
          pass "concurrent invocations: second invocation logs lock-skip"
      else
          fail "concurrent invocations: expected lock-skip message not found"
      fi
  }

  # ── Run matrix ──
  test_docker_stopped_exits_zero_and_logs_reason
  test_venv_missing_exits_zero
  test_concurrent_commits_flock_skip
  test_commit_on_plan_file_triggers_sync_within_10s
  test_commit_on_compiler_file_does_NOT_trigger_plan_sync

  echo "---"
  echo "Results: $PASS passed, $FAIL failed"
  [[ $FAIL -eq 0 ]]
  ```

- [ ] **Concurrency test verified:** two simultaneous wrapper invocations — only one acquires the lock; the other logs the skip message and exits 0. Final Neo4j state reflects whichever invocation ran to completion (both produce the same output, so the final state is correct either way).
- [ ] **Degradation test verified:** wrapper exits 0 with logged reason for docker-stopped and venv-missing paths. The `ORI_INTEL_DIR` pointing to a missing directory is covered by the `[[ ! -d "$PROJECT_DIR" ]]` guard at the top of `sync-plan-bug-graph.sh`.
- [ ] Test is idempotent: multiple runs leave the repo and Neo4j in a consistent state; test commits made with `--no-verify` to avoid triggering hooks recursively
- [ ] `chmod +x ~/projects/lang_intelligence/tests/test_sync_plan_bug_integration.sh` applied

- [ ] **Subsection close-out (03.4)** — MANDATORY before starting §03.R:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`.

---

## 03.N Completion Checklist

- [ ] `~/projects/lang_intelligence/neo4j/sync_plan_bug_graph.py` exists and `python3 sync_plan_bug_graph.py --health` exits 0 with `{"status": "ok", ...}` JSON output
- [ ] `~/projects/lang_intelligence/scripts/sync-plan-bug-graph.sh --full` exits 0 when run manually with Neo4j running
- [ ] `~/projects/lang_intelligence/scripts/sync-plan-bug-graph.sh --full` exits 0 when Neo4j container is stopped (graceful degradation)
- [ ] `ori_lang/lefthook.yml` `post-commit:` block contains `intel-plan-sync` entry with glob patterns `'plans/*.md' 'plans/**/*.md'`
- [ ] `time git commit -m "test(sync): section-03 smoke"` on a `plans/` edit completes in under 100ms
- [ ] `tail -20 ~/projects/lang_intelligence/logs/plan-bug-sync.log` shows `sync-complete` line within 10s of the triggering commit
- [ ] `shellcheck ~/projects/lang_intelligence/scripts/sync-plan-bug-graph.sh` exits 0 with no warnings
- [ ] Concurrency verified: rapid double-invocation of the wrapper logs one lock-skip message; both exit 0
- [ ] Four degradation paths verified individually: (a) dir missing → `SKIP: lang_intelligence not found`, (b) docker stopped → `SKIP: docker container not running`, (c) venv missing → `SKIP: .venv not found`, (d) Python driver non-zero exit → `ERROR: ... failed` + exit 0
- [ ] `intel-sync` entry in `lefthook.yml` is UNCHANGED (additive edit only — verified with `git diff lefthook.yml`)
- [ ] Satisfies mission criterion: `grep -c "sync-complete" ~/projects/lang_intelligence/logs/plan-bug-sync.log` is non-zero after a real commit touching `plans/`
- [ ] **Plan sync** — update plan metadata:
  - [ ] This section's frontmatter `status` → `complete`, all subsection statuses updated
  - [ ] `00-overview.md` Quick Reference table status updated for Section 03
  - [ ] `00-overview.md` mission success criterion for §03 (`~/projects/lang_intelligence/scripts/sync-plan-bug-graph.sh and...`) checked off
  - [ ] `index.md` section status updated
- [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check` before final commit.

**Exit Criteria:** `bash ~/projects/lang_intelligence/tests/test_sync_plan_bug_integration.sh` exits 0 with all test cases passing (concurrency, docker-stopped degradation, venv-missing degradation); `time git commit` on a `plans/` change returns in under 100ms; `plan-bug-sync.log` shows `sync-complete` within 10s; `shellcheck` exits 0.
