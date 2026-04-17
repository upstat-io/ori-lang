---
section: "08"
title: "Tool UX & output shapes"
status: not-started
reviewed: false
goal: "Polish scripts/intel-query.sh UX (tty default, --help, blast-radius, --format md) and query_graph.py output shapes (ASCII call-trees, grouped file-symbols, clickable deep-links)"
success_criteria:
  - "`scripts/intel-query.sh --help` prints usage; `scripts/intel-query.sh` defaults to `--human` when stdout is a tty"
  - "`scripts/intel-query.sh blast-radius <symbol>` runs callers + callees + similar in one invocation and formats a unified report"
  - "`scripts/intel-query.sh --format md` emits markdown tables pasteable into plan sections and reviewer envelopes"
  - "`query_graph.py` callers/callees emit ASCII call-trees in `--human` mode; file-symbols groups by kind; deep-links are file:line format"
  - "Issue-search results include full `https://github.com/<owner>/<repo>/issues/N` URLs"
  - "Satisfies mission criterion: wrapper + query_graph.py improved per the findings"
inspired_by:
  - "Existing `scripts/intel-query.sh` availability-check flow (lines 75-112) — extended with tty detection"
  - "`../lang_intelligence/neo4j/query_graph.py` existing subcommand dispatch — extended with new formatters"
  - "TPR findings codex-031/032/033/034/035, gemini-008, gemini-009"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "08.1"
    title: "scripts/intel-query.sh UX improvements"
    status: not-started
  - id: "08.2"
    title: "query_graph.py output shape improvements"
    status: not-started
  - id: "08.3"
    title: "Session caching (stretch)"
    status: not-started
  - id: "08.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "08.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 08: Tool UX & output shapes

**Status:** Not Started
**Goal:** The operator's question — "Maybe we need to improve the output of content to be more useful as well?" — is an open invitation. This section polishes every rough edge in the tooling: wrapper UX (help, sensible defaults, composite subcommand, markdown mode), output shapes (call-trees, grouping, clickable links), and optional session caching. Ambitiously: what reaches the operator should look less like raw graph queries and more like pre-formatted reconnaissance reports.

**Context:** `scripts/intel-query.sh` is 206 lines, defaults to JSON, rejects `--help` with "Unknown command". `../lang_intelligence/neo4j/query_graph.py` is 1240 lines and emits flat lists for all result types. The command file `.claude/commands/query-intel.md` is 14 lines — barely a teaching surface. Each of these was a separate TPR finding (codex-031 through 035, plus gemini-008/009). Fixing them raises the "cost to use the graph" from "remember the right subcommand and flag and parse JSON" to "run it and read the output." This section is fully parallel with §01–§07 (touches only `scripts/` and `../lang_intelligence/`; no `.claude/` prompt surface interactions).

**Reference implementations:**
- **Ori** `scripts/intel-query.sh` itself — today's implementation is the baseline being improved
- **Ori** `../lang_intelligence/neo4j/query_graph.py` — today's output formatters
- **TPR finding provenance**: codex-031 (command file teaching surface), codex-032 (json-as-default BLOAT), codex-033 (output-shape for callers/callees/file-symbols), codex-034 (no markdown output mode), codex-035 (no blast-radius composite), gemini-008 (UX polish), gemini-009 (output enhancements)

**Depends on:** Nothing in this plan. Fully parallel with §01–§07.

---

## 08.1 scripts/intel-query.sh UX improvements

**File(s):** `scripts/intel-query.sh` (modify), `.claude/commands/query-intel.md` (expand)

Four UX improvements land together because they share the same flag-parsing path at the top of the script.

- [ ] **Add `--help` / `-h` support** at the top of the flag loop (before the existing `--human`/`--timeout` handling):

  ```bash
  case "$1" in
      --help|-h)
          cat <<EOF
  Usage: intel-query.sh [options] <subcommand> [args...]

  Code symbol queries (Ori + 10 reference repos):
    symbols <name> [--repo R] [--kind K]  — find symbols by name
    callers <name> --repo ori              — reverse call graph
    callees <name> --repo ori              — forward call graph
    file-symbols <path-fragment> --repo ori — module inventory
    similar <name> [--repo rust,...]       — semantic equivalents

  Composite workflows:
    blast-radius <symbol>                  — callers + callees + similar in one shot

  Issue/PR search:
    search <terms>                         — full-text search
    compare <terms>                        — cross-repo convergence
    fixed <terms> [--repo ...]             — closed-as-fixed issues
    hot [--repo R]                         — highly-reacted issues
    ori-arc | ori-inference | ori-codegen | ori-patterns | ori-diagnostics
    sentiment pain|controversy|excitement [--repo R]
    landscape [--repo R]                   — per-label sentiment aggregation
    ori-sentiment                          — highest-pain in ARC-relevant repos

  Administrative:
    status                                 — graph health + node/edge counts
    cypher "<query>"                       — raw Cypher

  Options:
    --human          human-readable output (default when stdout is a tty)
    --json           JSON output (default when stdout is piped)
    --format md      markdown output (tables, links)
    --timeout N      step timeout in seconds (default 5)

  See .claude/skills/query-intel/SKILL.md for the full capability reference.
  EOF
          exit 0
          ;;
  esac
  ```

- [ ] **tty-aware default**: change the existing `JSON_FLAG="--json"` initialization to tty-detect:

  ```bash
  # Default: human-readable on tty, JSON when piped
  if [[ -t 1 ]]; then
      JSON_FLAG=""
  else
      JSON_FLAG="--json"
  fi
  # --human and --json flags still work as explicit overrides
  ```

- [ ] **`blast-radius` composite subcommand**: add a dispatch case that runs callers + callees + similar in one invocation:

  ```bash
  if [[ "${PASS_ARGS[0]}" == "blast-radius" ]]; then
      SYMBOL="${PASS_ARGS[1]}"
      REPO="${PASS_ARGS[2]:-ori}"
      printf '=== BLAST RADIUS: %s (repo=%s) ===\n\n' "$SYMBOL" "$REPO"
      printf '--- Callers ---\n'
      "$VENV_PYTHON" "$QUERY_SCRIPT" callers "$SYMBOL" --repo "$REPO"
      printf '\n--- Callees ---\n'
      "$VENV_PYTHON" "$QUERY_SCRIPT" callees "$SYMBOL" --repo "$REPO"
      printf '\n--- Semantic equivalents (cross-repo) ---\n'
      "$VENV_PYTHON" "$QUERY_SCRIPT" similar "$SYMBOL" --repo rust,swift,go,koka --limit 5
      exit 0
  fi
  ```

- [ ] **`--format md` mode**: add flag parsing that routes to `query_graph.py --format md` (implemented in §08.2):

  ```bash
  case "$1" in
      --format)
          FORMAT_FLAG="--format $2"
          shift 2
          ;;
      ...
  esac
  ```

  And propagate `$FORMAT_FLAG` to the `$VENV_PYTHON $QUERY_SCRIPT` invocation.

- [ ] **Expand `.claude/commands/query-intel.md`** from 14 lines to a teaching surface (after §01 reduces it to an alias, §08.1 gives the alias richer example text while keeping it thin):

  ```markdown
  # /query-intel

  Thin alias — see `.claude/skills/query-intel/SKILL.md` for the canonical reference.

  Runs: `scripts/intel-query.sh $ARGUMENTS`

  ## Quick examples

  - Health: `/query-intel status`
  - Blast radius: `/query-intel blast-radius resolve_fully`
  - Module inventory: `/query-intel file-symbols iterator/consumers --repo ori`
  - Cross-repo prior art: `/query-intel similar emit_rc_inc --repo rust,swift,koka`
  - Paste into plan section: `/query-intel --format md callers resolve_fully --repo ori`
  - Issue search: `/query-intel fixed "memory leak" --repo rust,swift`

  ## Full reference
  - `.claude/skills/query-intel/SKILL.md` — subcommand catalog
  - `.claude/rules/intelligence.md` — when-to-query workflow inventory
  ```

- [ ] **Subsection close-out (08.1)**:
  - [ ] All 4 wrapper improvements land + command file expanded
  - [ ] Smoke test: `scripts/intel-query.sh --help | wc -l > 20`; `scripts/intel-query.sh status` produces human output on tty; `scripts/intel-query.sh status | cat` (piped) produces JSON; `blast-radius resolve_fully` works
  - [ ] Update `08.1` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 08.1** — the script is now 250+ lines. At what point does it need to split into modules? (Probably not yet, but flag the threshold.) Also: should the tty-detection be pushed into `query_graph.py` to centralize? File a follow-up if so.
  - [ ] **Run `/sync-claude` on 08.1** — CLAUDE.md §Commands mentions `/query-intel` after §02. Verify the description still matches post-08.1 expansion.
  - [ ] **Repo hygiene check**.

---

## 08.2 query_graph.py output shape improvements

**File(s):** `../lang_intelligence/neo4j/query_graph.py` (modify)

Output shape changes to four result types. The file is in a SEPARATE repo (`../lang_intelligence/`), so edits require a PR / commit in that repo, not this one. Coordinate accordingly (may need user direction on PR flow).

- [ ] **Callers/callees as ASCII call-trees** when `--human`:

  ```
  Found 3 callers of 'resolve_fully' (2 modules):

    compiler/ori_arc/src/aims/emit_rc/borrowed_defs.rs
      ├── is_take_project              (line 61)
      └── collect_inline_enum_projected_defs  (line 216)

    compiler/ori_arc/src/aims/emit_rc/drop_hints/mod.rs
      └── is_collection_var            (line 17)
  ```

- [ ] **file-symbols grouped by kind** with section headers:

  ```
  Symbols in compiler/ori_eval/src/interpreter/method_dispatch/iterator/consumers.rs (13 total)

  functions (11):
    eval_iter_fold       (line 18)
    eval_iter_count      (line 42)
    ...

  types (2):
    IterConsumerResult   (line 5)
    ...
  ```

- [ ] **Leading one-line summary** for every result block: "Found 3 callers of X across 2 modules", "Found 12 symbols in 1 file", etc.

- [ ] **Clickable `file:line` deep-links**: emit paths in `path:line:col` format compatible with VS Code / Cursor / JetBrains quick-open. Avoid emitting "line N" prose — use the terminal-clickable shorthand.

- [ ] **Issue-search URLs**: every issue result includes `https://github.com/<owner>/<repo>/issues/<N>` — constructed from the Repo node's GitHub coordinates.

- [ ] **`--format md` output mode**: when invoked, emit markdown tables instead of ASCII. Examples:

  ```markdown
  ## Callers of `resolve_fully` (3 across 2 modules)

  | Function | File | Line |
  |----------|------|------|
  | `is_take_project` | [borrowed_defs.rs](compiler/ori_arc/src/aims/emit_rc/borrowed_defs.rs#L61) | 61 |
  | `collect_inline_enum_projected_defs` | [borrowed_defs.rs](compiler/ori_arc/src/aims/emit_rc/borrowed_defs.rs#L216) | 216 |
  | `is_collection_var` | [drop_hints/mod.rs](compiler/ori_arc/src/aims/emit_rc/drop_hints/mod.rs#L17) | 17 |
  ```

- [ ] Unit-test each formatter against a known-good fixture (small graph snapshot) to prevent output drift.

- [ ] **Subsection close-out (08.2)**:
  - [ ] All 5 formatter improvements land + fixture tests pass
  - [ ] Update `08.2` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 08.2** — the formatters now span ASCII, markdown, and JSON. If the next ask is HTML or Slack-block format, is the refactor obvious? If not, flag a follow-up to extract a Formatter protocol.
  - [ ] **Run `/sync-claude` on 08.2** — `../lang_intelligence/neo4j/query_graph.py` is outside the ori_lang repo; CLAUDE.md Key Paths references `../lang_intelligence/` — check whether any new capability (e.g., `--format md`) should be surfaced in CLAUDE.md Commands. If §02 already covers this, no-op.
  - [ ] **Repo hygiene check**.

---

## 08.3 Session caching (stretch)

**File(s):** `scripts/intel-query.sh` (modify — cache layer)

- [ ] Cache layer design:
  - Cache directory: `${TMPDIR:-/tmp}/intel-cache/`
  - Cache key: SHA-1 of `(subcommand + args + flags)` — deterministic
  - TTL: 10 minutes (configurable via `INTEL_CACHE_TTL=600` env var)
  - Invalidation: on cache miss or TTL expiry, query and write atomically
  - Bypass: `--no-cache` flag skips read AND write
  - Cleanup: stale entries (>1 hour) pruned on startup of any query

- [ ] Implementation skeleton:

  ```bash
  if [[ -z "${NO_CACHE:-}" && "${PASS_ARGS[0]}" != "status" ]]; then
      CACHE_KEY=$(printf '%s' "${PASS_ARGS[*]} $JSON_FLAG $FORMAT_FLAG" | sha1sum | cut -d' ' -f1)
      CACHE_FILE="${TMPDIR:-/tmp}/intel-cache/$CACHE_KEY"
      if [[ -f "$CACHE_FILE" ]]; then
          AGE=$(( $(date +%s) - $(stat -c %Y "$CACHE_FILE") ))
          if [[ $AGE -lt "${INTEL_CACHE_TTL:-600}" ]]; then
              cat "$CACHE_FILE"
              exit 0
          fi
      fi
      mkdir -p "${TMPDIR:-/tmp}/intel-cache"
      # On query, tee to cache file
  fi
  ```

- [ ] Time the cache hit vs miss: a file-symbols query should drop from ~300ms to ~5ms on cache hit.

- [ ] **Subsection close-out (08.3)** — marked OPTIONAL; may skip per scope:
  - [ ] If implemented: smoke test hit vs miss, document in command file
  - [ ] If deferred: document in this subsection's body that cache is deferred to a follow-up, with a `<!-- deferred-to: <plan> -->` anchor per CLAUDE.md's deferral rules
  - [ ] Update `08.3` status to `complete` OR `deferred` (not the banned "not-started-indefinitely" state)
  - [ ] **Run `/improve-tooling` retrospectively on 08.3** — if the cache landed, note performance wins; if deferred, note the specific trigger that would make it worth implementing later.
  - [ ] **Run `/sync-claude` on 08.3** — no artifact changes if deferred.
  - [ ] **Repo hygiene check**.

---

## 08.R Third Party Review Findings

- None.

---

**Exit Criteria:** Every tool-UX finding from the 2026-04-14 TPR (codex-031 through 035, gemini-008, gemini-009) is either addressed or explicitly deferred with a concrete plan anchor. Wrapper + query_graph.py serve human operators and agent pipelines equally well. `./test-all.sh` green.
