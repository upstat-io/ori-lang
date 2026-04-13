---
section: "10"
title: "Sentiment & Issue Signals"
status: not-started
reviewed: true
goal: "Enrich the intelligence graph with per-emoji reaction data and materialized sentiment metrics so that design decisions, bug fixes, and reviews are informed by cross-language user pain, excitement, and controversy signals."
success_criteria:
  - "All 10 repos have per-emoji reaction data on Issue nodes — verified by Cypher: MATCH (r:Repo) OPTIONAL MATCH (i:Issue)-[:IN_REPO]->(r) WHERE i.reaction_plus1 IS NOT NULL RETURN r.name, count(i) — all repos have count > 0"
  - "Comment nodes store per-emoji reactions, author_association, and author_type for repos with fetched comments — verified per-repo with comment_coverage metadata on Repo nodes"
  - "Materialized sentiment metrics (pain_score, controversy_score, excitement_score) exist as indexed properties on Issue nodes across all 10 repos — verified by: MATCH (r:Repo) OPTIONAL MATCH (i:Issue)-[:IN_REPO]->(r) WHERE i.pain_score IS NOT NULL RETURN r.name, count(i)"
  - "query_graph.py sentiment command returns issues ranked by sentiment type — verified by: python3 neo4j/query_graph.py sentiment pain --repo rust --limit 5 (returns non-empty results)"
  - "query_graph.py landscape command returns per-label sentiment aggregations — verified by: python3 neo4j/query_graph.py landscape --repo rust (returns non-empty results)"
  - "_print_issue() shows percentile-based sentiment tags (e.g., [Pain], [Controversial]) for issues above the 95th percentile — verified by visual inspection of human output"
  - "All existing skills automatically inherit sentiment via --human output — no SKILL.md files modified"
  - "scripts/intel-query.sh ori-sentiment returns results via canonical helper — verified by: scripts/intel-query.sh --human ori-sentiment --limit 3"
  - "Satisfies mission criterion: all section success criteria met"
inspired_by:
  - "GitHub API reactions model (per-emoji breakdown on issues and comments)"
  - "Reddit-style controversy ranking (custom absolute-magnitude formula, not Wilson score interval)"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "10.1"
    title: "Schema Extension & Reaction Import"
    status: not-started
  - id: "10.2"
    title: "Materialized Sentiment Metrics"
    status: not-started
  - id: "10.3"
    title: "Query Commands & Output Formatting"
    status: not-started
  - id: "10.4"
    title: "Ori Presets & Integration Verification"
    status: not-started
  - id: "10.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "10.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 10: Sentiment & Issue Signals

**Status:** Not Started
**Goal:** Enrich the intelligence graph with per-emoji reaction data (already present in raw JSON but currently discarded at import time) and materialized sentiment metrics, so that every design decision, bug fix, and review is informed by cross-language user pain, excitement, and controversy signals — automatically, via the existing `--human` output path.

**Success Criteria:**

- [ ] Issue nodes store 8 per-emoji reaction properties — backfilled across all 10 repos (~295K issues)
- [ ] Comment nodes store per-emoji reactions, `author_association`, and `author_type` — enabling bot filtering (bots have `author_type = "Bot"`, not a distinct `author_association` value)
- [ ] Materialized pain, controversy, and excitement scores exist as indexed properties on Issue nodes
- [ ] `sentiment` and `landscape` commands work in `query_graph.py` with JSON and human output
- [ ] `_print_issue()` shows threshold-based sentiment tags — all skills auto-inherit
- [ ] `enrich_sentiment.py` is idempotent and re-runnable without full re-import

**Context:** The language intelligence graph currently stores only `Issue.reactions = total_count` (a single integer) and discards the per-emoji breakdown (`+1`, `-1`, `confused`, `heart`, `rocket`, `eyes`, `laugh`, `hooray`) that GitHub provides for every issue and comment. This means a Go generics proposal with +1=1997 and -1=152 looks identical to a universally-loved feature with total_count=2149 — the design tension is invisible. Similarly, comment-level reactions are completely dropped by `import_comments_batch()`, and comments lack both `author_association` and `author_type` (making it impossible to filter bot noise — bots are identified by `user.type == "Bot"` in the GitHub API, not by `author_association`). This section fixes these gaps and builds a sentiment query surface on top.

**Reference implementations:**
- **GitHub API** reactions model: per-emoji counts on issues, comments, and reviews
- **Reddit-inspired** controversy metric: `min(up, down) * log2(up + down + 1)` — high when both up and down votes are high, scale-aware (not the Wilson score interval, which is a confidence bound on a proportion; this is a custom absolute-magnitude formula)
- **Hacker News** ranking: engagement = reactions + comments, time-decayed

**Depends on:** Section 01 (canonical helper infrastructure). Soft dependency on Section 08: if Section 08.3's ontology seeder later introduces a label taxonomy (e.g., CanonicalLabel nodes), the `landscape` command can leverage it. Until then, the `landscape` command queries raw `(:Label)` nodes directly — these already exist in the graph via `(:Issue)-[:HAS_LABEL]->(:Label)` relationships created by `import_graph.py`.

**Scope boundary with Section 08:** This section owns *reaction-derived signals* (pain, controversy, excitement computed from emoji reactions) and uses existing raw `Label` nodes for aggregation. Section 08 owns *ontology classification* (FailureMode, Concept, CompilerPhase taxonomy). There is no hard dependency — Section 10 works with raw Labels; if Section 08 later normalizes them, Section 10's queries benefit automatically.

**Scope exclusions:** NLP body-text sentiment analysis, ML classification, real-time monitoring, Review-node sentiment (PR reviews have `state` which is already a signal — APPROVED vs CHANGES_REQUESTED — and adding reaction enrichment to reviews is a separate concern).

---

## 10.1 Schema Extension & Reaction Import

**File(s):** `~/projects/lang_intelligence/neo4j/schema.cypher`, `~/projects/lang_intelligence/neo4j/import_graph.py`

The raw GitHub JSON already contains per-emoji reaction data for every issue and every comment. The current importer (`import_graph.py:68-69`) extracts only `reactions.get("total_count", 0)` and stores it as `Issue.reactions`. Comment reactions are dropped entirely (`import_comments_batch()` at lines 154-179 stores only `body`, `created_at`, `updated_at`, `position`). This subsection fixes both.

### Schema changes

- [ ] Add 8 per-emoji reaction properties to Issue node documentation in `schema.cypher`:
  ```cypher
  // Issue reactions (per-emoji breakdown from GitHub API)
  // reaction_plus1, reaction_minus1, reaction_laugh, reaction_hooray,
  // reaction_confused, reaction_heart, reaction_rocket, reaction_eyes
  // Original total_count preserved as 'reactions' for backward compat
  ```

- [ ] Add reaction properties, `author_association`, and `author_type` to Comment node documentation:
  ```cypher
  // Comment: {github_id, body, created_at, updated_at, position,
  //           author_association, author_type,
  //           reaction_plus1, reaction_minus1, reaction_laugh, reaction_hooray,
  //           reaction_confused, reaction_heart, reaction_rocket, reaction_eyes}
  // author_type: "User" | "Bot" | "Organization" | "" (from GitHub user.type)
  ```

- [ ] Add performance indexes for sentiment queries:
  ```cypher
  CREATE INDEX IF NOT EXISTS issue_reaction_minus1 FOR (i:Issue) ON (i.reaction_minus1);
  CREATE INDEX IF NOT EXISTS issue_reaction_confused FOR (i:Issue) ON (i.reaction_confused);
  CREATE INDEX IF NOT EXISTS comment_author_assoc FOR (c:Comment) ON (c.author_association);
  CREATE INDEX IF NOT EXISTS comment_author_type FOR (c:Comment) ON (c.author_type);
  ```

### Import pipeline changes

- [ ] Modify `import_issue_batch()` (import_graph.py:48-151) to store per-emoji reaction data:
  ```python
  # Current (L68-69):
  reactions = item.get("reactions", {})
  reactions_total = reactions.get("total_count", 0) if isinstance(reactions, dict) else 0

  # New: extract per-emoji + keep total for backward compat
  reactions = item.get("reactions", {}) if isinstance(item.get("reactions"), dict) else {}
  reactions_total = reactions.get("total_count", 0)
  reaction_plus1 = reactions.get("+1", 0)
  reaction_minus1 = reactions.get("-1", 0)
  reaction_laugh = reactions.get("laugh", 0)
  reaction_hooray = reactions.get("hooray", 0)
  reaction_confused = reactions.get("confused", 0)
  reaction_heart = reactions.get("heart", 0)
  reaction_rocket = reactions.get("rocket", 0)
  reaction_eyes = reactions.get("eyes", 0)
  ```

- [ ] Update the Issue MERGE Cypher (import_graph.py:76-98) to SET all 8 reaction properties plus keep existing `reactions` field:
  ```cypher
  SET i.reactions = $reactions,
      i.reaction_plus1 = $reaction_plus1,
      i.reaction_minus1 = $reaction_minus1,
      i.reaction_laugh = $reaction_laugh,
      i.reaction_hooray = $reaction_hooray,
      i.reaction_confused = $reaction_confused,
      i.reaction_heart = $reaction_heart,
      i.reaction_rocket = $reaction_rocket,
      i.reaction_eyes = $reaction_eyes
  ```

- [ ] Modify `import_comments_batch()` (import_graph.py:154-179) to store per-emoji reactions, `author_association`, and `author_type`:
  ```python
  # Current: only body, created_at, updated_at, position
  # New: add author_association, author_type + 8 reaction fields
  author_assoc = comment.get("author_association", "")
  author_type = (comment.get("user") or {}).get("type", "") if isinstance(comment.get("user"), dict) else ""
  c_reactions = comment.get("reactions", {}) if isinstance(comment.get("reactions"), dict) else {}
  ```
  Update Comment MERGE Cypher to SET `c.author_association`, `c.author_type`, `c.reaction_plus1` through `c.reaction_eyes`.

  **Bot identification:** GitHub bots have `user.type == "Bot"` but `author_association` is typically `"NONE"` (not `"BOT"`). The `author_type` field is required for reliable bot filtering in 10.2's sentiment aggregation. Store it as `c.author_type` on Comment nodes.

- [ ] Verify: existing `Issue.reactions` (total_count) field preserved for backward compatibility — `cmd_hot()` at query_graph.py:451 uses `i.reactions` and must continue to work unchanged.

### Backfill

- [ ] **Prerequisite: Verify comment corpus completeness.** The comment fetch in `fetch_repo.sh` stops when GitHub rate limit drops below 50, so the local comment corpus is knowingly partial for large repos (e.g., Go has ~74K issues with comments but only ~1K comment JSON files on disk). Before backfilling:
  1. Run a per-repo coverage check: for each repo, count issues with `comments > 0` vs. comment JSON files on disk
  2. If coverage is below 80% for any repo, run `fetch_repo.sh <repo> <github_org/repo>` to complete the comment fetch (may require multiple runs due to rate limits)
  3. Document actual per-repo coverage in a verification artifact — thread-level aggregation in 10.2 is only meaningful when comment data is substantially complete
  4. If full comment fetch is impractical (rate limits), the plan must still proceed with issue-level-only sentiment for under-fetched repos. Add a `comment_coverage` property to `:Repo` nodes so queries can report which repos have reliable thread-level metrics.

- [ ] Re-import all 10 repos to backfill per-emoji data. The raw JSON files already contain the breakdown — re-running `import_graph.py` with the updated code will populate the new properties via MERGE+SET (idempotent — existing non-reaction properties are preserved).
  ```bash
  # Run from ~/projects/lang_intelligence/
  # Expected duration: ~30-60 min for all 10 repos (~295K issues total).
  # The MERGE pattern re-matches existing Issue nodes by (repo, number)
  # and SETs the new reaction properties without touching other fields.
  for repo in rust go zig typescript gleam elm roc swift koka lean4; do
      python3 neo4j/import_graph.py "$repo" \
          ~/projects/reference_repos/lang_repos/"$repo"/issue_tracker
  done
  ```

- [ ] Verify backfill with spot-check queries:
  ```cypher
  // Go generics proposal — should show reaction_plus1=1997, reaction_minus1=152
  MATCH (i:Issue {repo: 'go', number: 15292})
  RETURN i.reaction_plus1, i.reaction_minus1, i.reaction_confused,
         i.reaction_heart, i.reaction_rocket

  // Comment reactions populated
  MATCH (c:Comment)-[:ON_ISSUE]->(i:Issue {repo: 'rust'})
  WHERE c.reaction_plus1 > 0
  RETURN count(c)
  ```

### Test matrix

| Dimension | Values |
|-----------|--------|
| Node type | Issue, Comment |
| Reaction type | plus1, minus1, laugh, hooray, confused, heart, rocket, eyes |
| Data state | Zero reactions, high reactions, missing reactions dict |
| Repo | At least rust, go, typescript (different sizes) |
| Backward compat | cmd_hot still works with i.reactions (total_count) |

- [ ] **Semantic pin**: After backfill, Go #15292 must have `reaction_plus1 = 1997`, `reaction_minus1 = 152` (verified against raw JSON).
- [ ] **Negative pin**: An issue whose raw JSON has no `reactions` dict (malformed/missing) must get all 8 reaction fields set to 0, not crash or leave them NULL.
- [ ] **Negative pin**: A comment whose `user` is null or missing must get `author_type = ""`, not crash.

- [ ] **Subsection close-out (10.1)** — MANDATORY before starting 10.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect on the debugging journey for 10.1 specifically: which diagnostic scripts you ran, where you added debugging, where output was hard to interpret, where test failures gave unhelpful messages. Forward-look: what tool/log/diagnostic would shorten the next regression in 10.1's code path by 10 minutes? Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics)`, `test(...)`, `chore(...)`, etc.). Mandatory even when nothing felt painful.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any temp/scratch files that accumulated during this subsection.

---

## 10.2 Materialized Sentiment Metrics

**File(s):** `~/projects/lang_intelligence/neo4j/enrich_sentiment.py` (new), `~/projects/lang_intelligence/neo4j/schema.cypher`

Sentiment metrics are *derived* from raw reaction data — they are policy, not raw facts. Computing them at import time inside `import_graph.py` would mean any formula tweak requires a full re-import of ~295K issues from disk. A separate, idempotent `enrich_sentiment.py` runs post-import and can be re-executed independently whenever formulas change.

Additionally, `import_graph.py` processes Issues (Phase 1) before Comments (Phase 2), so thread-level aggregation (rolling comment reactions up to Issue) cannot happen during the initial issue import — it requires a post-import pass.

### Schema changes

- [ ] Add materialized sentiment indexes to `schema.cypher`:
  ```cypher
  CREATE INDEX IF NOT EXISTS issue_pain_score FOR (i:Issue) ON (i.pain_score);
  CREATE INDEX IF NOT EXISTS issue_controversy_score FOR (i:Issue) ON (i.controversy_score);
  CREATE INDEX IF NOT EXISTS issue_excitement_score FOR (i:Issue) ON (i.excitement_score);
  CREATE INDEX IF NOT EXISTS issue_pain_pctile FOR (i:Issue) ON (i.pain_pctile);
  CREATE INDEX IF NOT EXISTS issue_controversy_pctile FOR (i:Issue) ON (i.controversy_pctile);
  CREATE INDEX IF NOT EXISTS issue_excitement_pctile FOR (i:Issue) ON (i.excitement_pctile);
  ```

### Enrichment script

- [ ] Create `~/projects/lang_intelligence/neo4j/enrich_sentiment.py` with the following responsibilities:
  1. **Issue-level sentiment**: Compute scores from Issue's own per-emoji reactions
  2. **Thread-level aggregation**: Sum comment reactions per issue (excluding bots: `WHERE c.author_type <> 'Bot'`), add to issue's own reactions. Note: GitHub bots have `user.type == "Bot"` but `author_association` is typically `"NONE"`, so `author_type` (stored from `user.type` in 10.1) is the reliable bot discriminator.
  3. **Score formulas** (absolute magnitude, not ratios):
     ```python
     import math

     def controversy(plus1, minus1):
         """High when both up and down votes are high. Scale-aware."""
         return min(plus1, minus1) * math.log2(plus1 + minus1 + 1)

     def pain(minus1, confused):
         """Weighted negative signal. confused=1.0, minus1=2.0"""
         return minus1 * 2 + confused

     def excitement(heart, rocket):
         """Positive enthusiasm beyond simple agreement (plus1)."""
         return heart + rocket * 2
     ```
  4. **Within-repo percentile bands**: After computing raw scores, compute percentile rank within each repo. Store as `i.pain_pctile`, `i.controversy_pctile`, `i.excitement_pctile` (integer 0-100). This enables cross-repo comparison: "top 5% most painful issues in Go" is comparable to "top 5% in Rust" even though Go has 76K issues and Rust has 15K.
  5. **Idempotency**: Script must be re-runnable. Use `SET` (not `+=`) on all properties.

- [ ] Wire enrichment into ALL import entrypoints to prevent DRIFT between raw reactions and materialized scores:
  1. **`fetch-all.sh`** — after all repos are imported, run enrichment:
     ```bash
     # At end of fetch-all.sh, after all imports complete:
     python3 neo4j/enrich_sentiment.py
     ```
  2. **`fetch_repo.sh`** — after single-repo import completes (including `--since` incremental mode), run enrichment for that repo:
     ```bash
     python3 neo4j/enrich_sentiment.py --repo "$NAME"
     ```
     This requires `enrich_sentiment.py` to accept an optional `--repo` flag that limits enrichment to one repo's issues.
  3. **Documentation** — add a note to `~/projects/lang_intelligence/CLAUDE.md` that direct `python3 import_graph.py` runs must be followed by `python3 enrich_sentiment.py` to keep scores in sync.

- [ ] Verification queries:
  ```cypher
  // Top 5 most painful issues in Go
  MATCH (i:Issue {repo: 'go'})
  WHERE i.pain_score > 0
  RETURN i.number, i.title, i.pain_score, i.pain_pctile
  ORDER BY i.pain_score DESC LIMIT 5

  // Most controversial across all repos
  MATCH (i:Issue)
  WHERE i.controversy_score > 0
  RETURN i.repo, i.number, i.title, i.controversy_score
  ORDER BY i.controversy_score DESC LIMIT 10
  ```

### Test matrix

| Dimension | Values |
|-----------|--------|
| Score type | pain, controversy, excitement |
| Edge case | All zeros (no reactions), one-sided (only +1, no -1), balanced (equal +1/-1) |
| Aggregation | Issue-only reactions, issue+comment reactions, issue with bot comments filtered |
| Percentile | Repo with 1 issue, repo with 76K issues |
| Idempotency | Run twice — same results |

- [ ] Create `~/projects/lang_intelligence/tests/test_enrich_sentiment.py` with unit tests:
  - Pure formula tests: `controversy(0, 0) == 0`, `controversy(100, 100) > controversy(100, 1)`, `pain(0, 0) == 0`, `excitement(0, 0) == 0`
  - Edge cases: all-zero reactions, single-emoji reactions, large values
  - Idempotency: running enrichment twice produces identical scores
  - Bot filtering: verify bot comments excluded from thread aggregation
  - Percentile computation: repo with 1 issue gets pctile=100; repo with 100 issues distributes correctly

- [ ] **Semantic pin**: Go error handling (#32825) must have pain_score > 0 (has 223 thumbs-down, 22 confused => pain = 223*2 + 22 = 468).
- [ ] **Semantic pin**: Go error handling (#32825) must have controversy_score > Go generics (#15292). Calculation: #32825 has min(2010, 223) * log2(2233) = 223 * 11.13 ≈ 2482; #15292 has min(1997, 152) * log2(2150) = 152 * 11.07 ≈ 1683. Pin: `controversy_score(#32825) > controversy_score(#15292) > 1000` (both highly controversial, but #32825 has more absolute disagreement).
- [ ] **Negative pin**: An issue with zero reactions across all emoji types must have pain_score = 0, controversy_score = 0, excitement_score = 0.

- [ ] **Subsection close-out (10.2)** — MANDATORY before starting 10.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — same protocol as 10.1's close-out, scoped to 10.2's debugging journey. Commit improvements separately using a valid conventional-commit type.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 10.3 Query Commands & Output Formatting

**File(s):** `~/projects/lang_intelligence/neo4j/query_graph.py`

Add two query commands and modify the output formatter to show sentiment signals. Two commands (not five) to avoid BLOAT — `sentiment` handles ranked queries by type, `landscape` handles aggregated overview.

### New commands

- [ ] Add `cmd_sentiment(args, json_mode=False)` to `query_graph.py`:
  ```python
  # Usage: query_graph.py sentiment <type> [--repo X] [--limit N]
  # <type> is one of: pain, controversy, excitement
  # Returns issues ranked by the specified sentiment score (descending)
  # Filters to issues with score > 0
  ```
  Cypher template:
  ```cypher
  MATCH (i:Issue)-[:IN_REPO]->(r:Repo)
  WHERE i.{score_field} > 0 {repo_filter}
  RETURN r.name as repo, i.number as number, i.title as title,
         i.state as state, i.state_reason as state_reason,
         i.is_pr as is_pr, i.reactions as reactions,
         i.comment_count as comment_count,
         i.pain_score as pain_score,
         i.controversy_score as controversy_score,
         i.excitement_score as excitement_score,
         i.pain_pctile as pain_pctile,
         i.controversy_pctile as controversy_pctile,
         i.excitement_pctile as excitement_pctile
  ORDER BY i.{score_field} DESC LIMIT $limit
  ```
  Where `{score_field}` maps: pain→`pain_score`, controversy→`controversy_score`, excitement→`excitement_score`. **Input validation:** the `<type>` argument must be one of {pain, controversy, excitement}. Any other value must produce a clear error message (not a Cypher injection or silent empty result). Use a hardcoded allowlist dict, not string interpolation of raw user input.

- [ ] Add `cmd_landscape(args, json_mode=False)` to `query_graph.py`:
  ```python
  # Usage: query_graph.py landscape [--repo X] [--limit N]
  # Returns per-label aggregated sentiment statistics
  # Groups by Label, shows: issue count, avg pain, avg controversy, avg excitement
  # Sorted by avg pain descending (most painful areas first)
  # Uses _parse_flags(args) for --repo/--limit extraction (same pattern as other commands)
  ```
  Cypher template:
  ```cypher
  MATCH (i:Issue)-[:HAS_LABEL]->(l:Label)
  MATCH (i)-[:IN_REPO]->(r:Repo)
  WHERE i.pain_score IS NOT NULL {repo_filter}
  RETURN l.name as label, count(i) as issues,
         avg(i.pain_score) as avg_pain,
         avg(i.controversy_score) as avg_controversy,
         avg(i.excitement_score) as avg_excitement
  ORDER BY avg_pain DESC LIMIT $limit
  ```

- [ ] Register both commands in the `commands` dict (query_graph.py:714-727):
  ```python
  "sentiment": cmd_sentiment,
  "landscape": cmd_landscape,
  ```

### Output formatting

- [ ] Modify `_print_issue()` (query_graph.py:105-121) to show threshold-based sentiment tags:
  ```python
  def _print_issue(rec):
      kind = "PR" if rec.get("is_pr") else "issue"
      state = rec["state"]
      reason = rec.get("state_reason")
      if reason:
          state = f"{state}:{reason}"
      reactions = rec.get("reactions", 0) or 0
      comments = rec.get("comment_count", 0) or 0
      signal = ""
      if reactions:
          signal += f"+{reactions} "
      if comments:
          signal += f"{comments}c"

      # Sentiment tags — use percentile thresholds (not absolute scores)
      # to ensure cross-repo fairness (Go's 76K issues vs Koka's 677).
      # Tags only appear when percentile fields are returned by the query.
      tags = []
      pain_pct = rec.get("pain_pctile", 0) or 0
      controversy_pct = rec.get("controversy_pctile", 0) or 0
      excitement_pct = rec.get("excitement_pctile", 0) or 0
      if pain_pct >= 95:
          tags.append("[Pain]")
      if controversy_pct >= 95:
          tags.append("[Controversial]")
      if excitement_pct >= 95:
          tags.append("[Excitement]")
      tag_str = " ".join(tags)

      print(f"  {rec['repo']}#{rec['number']} ({kind} {state}) {rec['title']}")
      line2_parts = [s for s in [signal.strip(), tag_str] if s]
      if line2_parts:
          print(f"    {' '.join(line2_parts)}")
  ```
  **Key design**: tags only appear when scores are returned by the query. Existing queries (search, fixed, hot) don't return `pain_score` etc., so their output is unchanged. The `sentiment` and `landscape` commands DO return these fields, so tags appear naturally. No `--sentiment` flag needed — the query itself controls what data is available.

- [ ] Add `_print_landscape_row()` formatter for the `landscape` command. Unlike `_print_issue()` which formats individual issues, `landscape` returns per-label aggregations. Format:
  ```
    label-name (42 issues)
      avg pain: 12.3  avg controversy: 5.7  avg excitement: 8.1
  ```
  This is a separate formatter because `landscape` results have a different shape (label + aggregates) than individual issue results.

- [ ] Update JSON output for sentiment commands to include all score fields.

- [ ] Verify backward compatibility: existing `cmd_search`, `cmd_fixed`, `cmd_hot` output unchanged (they don't return sentiment fields, so tags never appear).

### Test matrix

| Dimension | Values |
|-----------|--------|
| Command | sentiment pain, sentiment controversy, sentiment excitement, landscape |
| Output mode | human (--human), JSON (default) |
| Repo filter | single repo, all repos, invalid repo |
| Backward compat | search, fixed, hot output unchanged |
| Threshold | Issue below threshold (no tags), above threshold (tags shown) |
| Edge case | No enriched data yet (graceful: no tags shown) |
| Negative | Invalid sentiment type (e.g., "anger") → clear error message |
| Negative | `landscape` on repo with no labeled issues → empty result, no crash |

- [ ] **Negative pin**: `python3 neo4j/query_graph.py sentiment anger` must produce a clear error (not a Cypher injection or KeyError).
- [ ] **Semantic pin**: `python3 neo4j/query_graph.py sentiment pain --repo go --limit 1` must return Go #32825 or another issue with the highest pain_score in Go.

- [ ] **TPR checkpoint** — `/tpr-review` covering 10.1–10.3 implementation work

- [ ] **Subsection close-out (10.3)** — MANDATORY before starting 10.4:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — same protocol as 10.1's close-out, scoped to 10.3's debugging journey. Commit improvements separately using a valid conventional-commit type.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 10.4 Ori Presets & Integration Verification

**File(s):** `~/projects/lang_intelligence/neo4j/query_graph.py`, `~/projects/ori_lang/scripts/intel-query.sh`

Add sentiment-specific Ori presets and verify that all existing skill integrations automatically inherit sentiment data through the `--human` output path.

### Preset

- [ ] Add `ori-sentiment` to `ORI_PRESETS` (query_graph.py:661-675). Unlike the existing presets (which use `cmd_search` with curated terms), this preset routes to `cmd_sentiment` with preset type + repo filter:
  ```python
  # In the ori-preset handler (cmd_ori_preset, L678-685):
  # Special case: ori-sentiment routes to cmd_sentiment, not cmd_search
  if preset == "ori-sentiment":
      return cmd_sentiment(["pain", "--repo", "rust,swift,koka,lean4,roc"] + args, json_mode)
  ```
  This shows the highest-pain issues across Ori-relevant repos (rust, swift, koka, lean4, roc — the ARC/memory-focused reference compilers). The results are NOT filtered to ARC/memory topics specifically — they show all high-pain issues in those repos, which is still useful because the repo selection is itself a topical filter.

- [ ] Update `intel-query.sh` help text (if the `status` subcommand or no-args help lists available commands) to mention `sentiment` and `landscape`.

### Integration verification

- [ ] Verify Tier 1 integration contract compliance (established in Sections 03-04):
  - `intel-query.sh` proxies `sentiment` and `landscape` commands without changes (it already passes all args to `query_graph.py`)
  - `--human` output includes sentiment tags via `_print_issue()` — no skill modifications needed
  - `--json` output includes sentiment score fields — no skill modifications needed
  - Graceful degradation: when Neo4j is unavailable, `intel-query.sh` returns `{"status":"unavailable"}` as before

- [ ] Smoke test the full pipeline end-to-end:
  ```bash
  # From ori_lang project root
  scripts/intel-query.sh --human sentiment pain --repo rust --limit 5
  scripts/intel-query.sh --human landscape --repo go --limit 10
  scripts/intel-query.sh --human ori-sentiment --limit 5
  ```

- [ ] Verify NO SKILL.md files were modified — this is a hard constraint from the dual-source consensus:
  ```bash
  git diff --name-only | grep -c 'SKILL.md'  # must be 0
  ```

- [ ] **Subsection close-out (10.4)** — MANDATORY before starting 10.R:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — same protocol as 10.1's close-out, scoped to 10.4's debugging journey. Commit improvements separately using a valid conventional-commit type.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 10.R Third Party Review Findings

<!-- Reserved for the dual-source `/tpr-review` (Codex + Gemini) and other external reviewers. Findings may be tagged `-codex`, `-gemini`, or carry `agreement: true` when both reviewers flagged the same location/title.
If unresolved findings exist here:
- section frontmatter `status` must be `in-progress`
- `third_party_review.status` must be `findings`

When all findings are triaged:
- accepted findings are integrated into the relevant implementation subsection(s)
- rejected findings are closed with rationale
- all items in this block are marked resolved
- `third_party_review.status` becomes `resolved` or `none`
-->

- None.

---

## 10.N Completion Checklist

- [ ] Schema extended: 8 per-emoji reaction properties on Issue and Comment nodes
- [ ] Comment nodes include `author_association` and `author_type` for bot/role filtering
- [ ] All 10 repos backfilled with per-emoji reaction data
- [ ] `enrich_sentiment.py` computes pain, controversy, excitement scores with absolute-magnitude formulas
- [ ] Percentile bands computed within each repo for cross-language comparison
- [ ] `cmd_sentiment` and `cmd_landscape` commands registered in `query_graph.py`
- [ ] `_print_issue()` shows threshold-based sentiment tags — no `--sentiment` flag
- [ ] `_print_landscape_row()` formats per-label aggregations for `landscape` command
- [ ] `ori-sentiment` preset works via `intel-query.sh`
- [ ] Backward compatibility verified: `cmd_hot`, `cmd_search`, `cmd_fixed` output unchanged
- [ ] No SKILL.md files modified — sentiment flows through existing `--human` output
- [ ] Unit tests exist: `tests/test_enrich_sentiment.py` with formula, edge case, idempotency, bot filtering, and percentile tests
- [ ] Spot-check queries verified: Go #15292 shows correct per-emoji breakdown, Go #32825 has high pain_score
- [ ] `./test-all.sh` green — no regressions
- [ ] Plan annotation cleanup: no temporary scaffolding left in source files
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] This section's frontmatter `status` → `complete`, subsection statuses updated
  - [ ] `00-overview.md` Quick Reference table status updated for this section
  - [ ] `00-overview.md` mission success criteria checkboxes updated
  - [ ] `index.md` section status updated
  - [ ] Next section's `depends_on` verified — no stale assumptions
- [ ] `/tpr-review` passed (final, full-section) — independent dual-source review (Codex + Gemini) found no critical or major issues
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean. MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` **section-close sweep** — verify every subsection's per-subsection retrospective actually ran, look for cross-subsection patterns. Most sweeps produce zero new findings when per-subsection captures are thorough.
- [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check`. Clean any detected temp files before final commit.

**Exit Criteria:** `python3 neo4j/query_graph.py sentiment pain --repo go --limit 3` returns Go issues ranked by pain score with `[Pain]` tags visible in human output. `python3 neo4j/query_graph.py landscape --repo rust` returns per-label sentiment aggregations. `scripts/intel-query.sh --human ori-sentiment` returns results via the canonical helper. All existing queries produce identical output to before this section. `enrich_sentiment.py` can be re-run and produces identical results (idempotent).
