---
plan: "lang-intelligence"
title: "Language Intelligence Graph: Exhaustive Implementation Plan"
status: in-progress
references:
  - "~/projects/lang_intelligence/CLAUDE.md"
  - "~/projects/lang_intelligence/neo4j/schema.cypher"
---

# Language Intelligence Graph: Exhaustive Implementation Plan

## Mission

Build a cross-language design-memory system that gives the Ori compiler proactive intelligence from 10 reference compilers. The system integrates a Neo4j graph database (issues, PRs, code structure, ontology) into the Ori Claude ecosystem so that every design decision, bug fix, and review is informed by the collective failure modes and design rationale of Rust, Go, Zig, TypeScript, Gleam, Elm, Roc, Swift, Koka, and Lean 4.

## Mission Success Criteria

- [x] `scripts/intel-query.sh` returns JSON by default: `{"status":"ok","data":...}` when Neo4j is available, `{"status":"unavailable","reason":"..."}` when not (exit 0 in both cases)
- [ ] `.claude/rules/intelligence.md` auto-loads and triggers intelligence queries during design decisions, bug fixes, and reviews
- [ ] `/query-intel` command works from any conversation with search, compare, fixed, hot, ori-* presets, and raw cypher
- [x] `/tpr-review` evidence packets include cross-language prior art from the intelligence graph
- [x] `/fix-bug` Phase 1 queries for similar bugs in reference compilers
- [ ] Ontology contains Concept, FailureMode, CompilerPhase, DesignDecision nodes with rich typed edges
- [ ] tree-sitter parses all 9 supported languages and extracts structural symbols (Module, Function, Struct, Trait, Method)
- [ ] Code graph contains CALLS, IMPORTS, IMPLEMENTS relationships for all reference repos
- [ ] Issue-to-code bridge links GitHub issues to code symbols via CodeReference nodes with confidence scores
- [ ] Ori live sync updates the code graph within 5s of a commit via lefthook post-commit async background sync
- [ ] Per-emoji reaction data stored on Issue and Comment nodes across all 10 repos (with `author_type` for bot filtering); materialized sentiment metrics (pain, controversy, excitement) computed with within-repo percentile normalization
- [ ] `sentiment` and `landscape` query commands surface cross-language user pain, controversy, and excitement signals via percentile-based thresholds
- [ ] `./test-all.sh` green — no regressions from any integration changes
- [ ] All section success criteria met

## Architecture

```
                    ┌─────────────────────────────────────────────┐
                    │         Neo4j Intelligence Graph            │
                    │                                             │
                    │  Issue Layer    Code Layer    Bridge Layer  │
                    │  ┌──────────┐  ┌──────────┐  ┌──────────┐ │
                    │  │ Issue    │  │ Symbol   │  │ CodeRef  │ │
                    │  │ PR      │  │ File     │  │ Concept  │ │
                    │  │ Comment │  │ Module   │  │ FailMode │ │
                    │  │ Review  │  │ Impl     │  │ Phase    │ │
                    │  │ Author  │  │ Occur.   │  │ Decision │ │
                    │  │ Label   │  │ Revision │  │          │ │
                    │  └──────────┘  └──────────┘  └──────────┘ │
                    │       │              │              │       │
                    │  REFERENCES    CALLS/IMPORTS   RESOLVES_TO │
                    │  FIXES         IMPLEMENTS      TAGGED_AS   │
                    │  HAS_LABEL     DECLARES        MENTIONS    │
                    └─────────────────────────────────────────────┘
                         ▲                ▲                ▲
                         │                │                │
                    ┌────┴────┐     ┌────┴────┐     ┌────┴────┐
                    │ GitHub  │     │  tree-  │     │ Regex   │
                    │ Fetcher │     │ sitter  │     │ Extract │
                    │ (REST)  │     │ Parsers │     │ + NLP   │
                    └─────────┘     └─────────┘     └─────────┘
                         ▲                ▲                
                         │                │                
                    ┌────┴────┐     ┌────┴────┐
                    │ 10 Ref  │     │ 11 Ref  │
                    │ Repos   │     │ Repos   │
                    │ Issues  │     │ Source  │
                    └─────────┘     └─────────┘

    Claude Integration:
    ┌──────────────────────────────────────────────────────┐
    │  scripts/intel-query.sh (canonical helper)           │
    │       ▲         ▲         ▲         ▲               │
    │  rules/     /tpr-review  /fix-bug  /query-intel     │
    │  intel.md   evidence    Phase 1   slash command      │
    │  (auto)     packets     research                     │
    └──────────────────────────────────────────────────────┘
```

## Design Principles

1. **Graceful degradation** — Every integration point checks availability via the canonical helper. If Neo4j is down or `../lang_intelligence/` doesn't exist, all workflows continue normally with zero errors. Intelligence is additive, never blocking.

2. **Discovery, not replacement** — Intelligence results shortlist repos and issues to examine. They do NOT replace reading actual source code. A Neo4j hit saying "Rust had this bug" is a pointer to investigate, not a conclusion to act on.

3. **One canonical helper, zero duplication** — All availability checks, venv activation, health probes, and query execution flow through `scripts/intel-query.sh`. Skills and rules call the helper, never open-code their own Neo4j logic. This is the SSOT principle applied to the intelligence surface.

4. **Ontology over data volume** — Rich, typed edges (IMPLEMENTS_CONCEPT, INTRODUCES_FAILURE_MODE) matter more than millions of generic nodes. Start with 5 concepts x 5 repos, prove query quality, then scale.

## Section Dependency Graph

```
01 (Helper) ──> 02 (Rules/Cmds) ──> 03 (TPR+Fix-Bug) ──> 04 (Other Skills)

05 (Parsers) ──> 06 (Extraction) ──> 07 (Import) ──> 08 (Bridge)
                       │
                       └──> 09 (Ori Sync)

01 (Helper) ──> 10 (Sentiment)   [soft dep on 08 for label taxonomy]
```

- Sections 01-04 (Claude Integration pillar) and 05-09 (Code Graph pillar) are independent pillars that can proceed in parallel after Section 01
- Within each pillar, sections are strictly sequential
- Section 08 (Bridge) depends on both pillars (needs code symbols from 07 AND issue data from existing graph)
- Section 10 (Sentiment) depends on Section 01 (canonical helper); soft dependency on Section 08 for label taxonomy — works with raw Labels until 08.3 normalizes them

## Implementation Sequence

1. **Foundation**: Section 01 (canonical helper) — sections 02-04 depend on this
2. **Claude Integration**: Sections 02 → 03 → 04 (rules, high-value skills, remaining skills)
3. **Code Graph**: Sections 05 → 06 → 07 (parsers, extraction, import)
4. **Bridge + Sync**: Sections 08, 09 (connect the two pillars, add live sync)
5. **Sentiment**: Section 10 (per-emoji reactions, materialized metrics, query surface) — depends on 01; soft dep on 08 for label taxonomy

## Quick Reference

| # | Section | Status | Files Touched | Depends On |
|---|---------|--------|---------------|------------|
| 01 | Infrastructure & Canonical Helper | complete | `scripts/intel-query.sh`, `~/projects/lang_intelligence/` | — |
| 02 | Claude Rules & Commands | complete | `.claude/rules/intelligence.md`, `.claude/commands/query-intel.md` | 01 |
| 03 | Skill Integration: TPR + Fix-Bug | complete | `.claude/skills/tpr-review/SKILL.md`, `.claude/skills/fix-bug/SKILL.md` | 01, 02 |
| 04 | Skill Integration: Remaining | complete | 4 skill files + `review-bugs.md` | 01, 02, 03 |
| 05 | Code Graph: Parser Adapters | complete | `~/projects/lang_intelligence/neo4j/`, `languages.yaml` | — |
| 06 | Code Graph: Symbol Extraction | complete | `~/projects/lang_intelligence/neo4j/extract_symbols.py` | 05 |
| 07 | Code Graph: Import Pipeline | complete | `~/projects/lang_intelligence/neo4j/import_code_graph.py`, `schema.cypher`, `scripts/build-code-graph.sh` | 06 |
| 08 | Issue-to-Code Bridge | complete | `~/projects/lang_intelligence/neo4j/extract_code_refs.py` | 07 |
| 09 | Ori Live Sync | not-started | `lefthook.yml`, `~/projects/lang_intelligence/scripts/sync-ori-graph.sh`, `~/projects/lang_intelligence/neo4j/ori_adapter.py`, `~/projects/lang_intelligence/neo4j/sync_ori_graph.py` | 06, 07 |
| 10 | Sentiment & Issue Signals | not-started | `schema.cypher`, `import_graph.py`, `enrich_sentiment.py`, `query_graph.py`, `fetch-all.sh`, `tests/test_enrich_sentiment.py` | 01 |

## Estimated Effort

| Section | Complexity | Key Risk |
|---------|-----------|----------|
| 01 | Low | Neo4j health probe edge cases (container up but DB not ready) |
| 02 | Low | Rule file trigger scope (too broad = noise, too narrow = missed) |
| 03 | Medium | Evidence packet size (too much intel = noisy reviewers) |
| 04 | Medium | 5 different skills with different insertion patterns |
| 05 | High | Swift (build from source), Lean (86% error rate), Ori (no grammar) |
| 06 | High | Per-language query family coverage (decls/calls/imports/impls) via adapter query handles |
| 07 | Medium | Neo4j batch import performance at scale (~500K nodes) |
| 08 | Medium | Code reference extraction accuracy (false positives vs coverage) |
| 09 | Medium | Incremental dependency invalidation (what to re-parse when imports change) |
| 10 | Medium | Sentiment formula calibration (thresholds, cross-repo normalization) |

## Tree-Sitter Language Support Matrix

| Language | pip Package | Maturity | tags.scm | Error Rate | Strategy |
|----------|-----------|----------|----------|------------|----------|
| Rust | tree-sitter-rust | Stable | YES | 9% | Standard pip |
| Go | tree-sitter-go | Stable | YES | 1% | Standard pip |
| Zig | tree-sitter-zig | Stable | NO | 6% | pip + custom queries |
| TypeScript | tree-sitter-typescript | Stable | YES | 5% | Standard pip |
| Haskell | tree-sitter-haskell | Stable | NO | 1-2% | pip + custom queries (for Elm, Koka .hs) |
| Swift | tree-sitter-swift (PyPI) | Stable | YES | Low | Try PyPI first; fall back to alex-pinkus source build |
| C++ | tree-sitter-cpp | Stable | YES | Low | Standard pip (for Lean4 runtime) |
| Lean | tree-sitter-cpp (C++ only) | Partial | NO | 86% (.lean) | Skip .lean files (86% error rate); parse C++ runtime only |
| Koka .kk | tree-sitter-koka | Beta | NO | Unknown | Build from source + custom queries |
| Ori | None | N/A | N/A | N/A | Use Ori's own Rust parser via FFI |

## Known Issues

- Rust fetch used v1 script (filtered PRs, no reviews) — needs re-fetch with v2 to pick up PR data
- `--paginate` on `gh api` produces concatenated JSON arrays — needs `jq -s 'add'` post-processing
- Fetch pipeline lacks incremental comment/review re-fetch for updated issues
- ~~`query_graph.py` issues~~ — All 10 `query_graph.py` issues resolved in Section 01.2 (driver error handling, driver.close() leak, label-graph, --json mode, env vars, _parse_flags validation, connection timeout, emptiness detection, cmd_compare/cmd_pattern flag bypass, health-check command)
