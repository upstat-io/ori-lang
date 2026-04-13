---
plan: "lang-intelligence"
title: "Language Intelligence Graph"
reroute: false
---

# Language Intelligence Graph

## Quick Reference

| # | Section | Status | Depends On |
|---|---------|--------|------------|
| 01 | Infrastructure & Canonical Helper | complete | — |
| 02 | Claude Rules & Commands | complete | 01 |
| 03 | Skill Integration: TPR + Fix-Bug | complete | 01, 02 |
| 04 | Skill Integration: Remaining | complete | 01, 02, 03 |
| 05 | Code Graph: Parser Adapters | complete | — |
| 06 | Code Graph: Symbol Extraction | complete | 05 |
| 07 | Code Graph: Import Pipeline | complete | 06 |
| 08 | Issue-to-Code Bridge | complete | 07 |
| 09 | Ori Live Sync | complete | 06, 07 |
| 10 | Sentiment & Issue Signals | not-started | 01 |

## Keyword Clusters

### Infrastructure
neo4j, docker, intel-query.sh, health-probe, canonical-helper, graceful-degradation, availability-check, venv, bolt, cypher-shell

### Claude Integration
intelligence.md, query-intel, rules, skills, tpr-review, fix-bug, design-pattern-review, create-draft-proposal, continue-roadmap, review-bugs, evidence-packet, pre-query

### Ontology
concept, failure-mode, compiler-phase, design-decision, taxonomy, IMPLEMENTS_CONCEPT, INTRODUCES_FAILURE_MODE, REJECTS_APPROACH, SUPERSEDES_DECISION, code-reference, staleness

### Code Graph
tree-sitter, parser-adapter, query-families, decls.scm, calls.scm, imports.scm, impls.scm, symbol-extraction, structural-graph, Module, Function, Struct, Trait, Method, CALLS, IMPORTS, IMPLEMENTS, ParseResult, query-handles, languages.yaml

### Issue Bridge
code-reference, MENTIONS_CODE, RESOLVES_TO, confidence, provenance, backtick-extraction, file-path-extraction, qualified-name

### Live Sync
lefthook, post-commit, incremental-parse, file-watcher, debounce, sub-500ms, dependency-refresh, async-enqueue

### Sentiment & Issue Signals
reaction_plus1, reaction_minus1, reaction_confused, reaction_heart, reaction_rocket, reaction_eyes, reaction_laugh, reaction_hooray, pain_score, controversy_score, excitement_score, enrich_sentiment.py, sentiment, landscape, _print_issue, _print_landscape_row, percentile, threshold, bot-filtering, author_association, author_type, materialized-metrics, cross-repo-normalization

### Languages
rust, go, zig, typescript, haskell, swift, cpp, lean, koka, ori, tree-sitter-rust, tree-sitter-go, alex-pinkus
