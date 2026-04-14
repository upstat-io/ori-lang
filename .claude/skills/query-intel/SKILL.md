---
name: query-intel
description: >-
  Query the Neo4j-backed intelligence graph for symbol lookup, call graphs,
  cross-repo prior art, and issue/PR search. TRIGGER proactively when:
  (1) looking up who calls / is called by a function or type in Ori or a
  reference repo (rust, swift, go, koka, lean4, gleam, elm, roc, zig, ts);
  (2) finding the Rust/Swift/Koka equivalent of an Ori function before manual
  browsing; (3) inventorying symbols in a module before editing; (4) checking
  prior art on a compiler design question (exhaustiveness, inference, ARC,
  RC elision, etc.); (5) assessing blast radius before a refactor. The graph
  houses 191K symbols, 505K CALLS edges, and 298K issues across 11 repos,
  synced on every commit — ~100x faster than grep. Uses scripts/intel-query.sh
  which degrades gracefully when the graph is unavailable.
paths:
  - "**"
---

# /query-intel

Canonical surface for the intelligence graph. See `.claude/rules/intelligence.md`
for the full workflow inventory (when to query, how to interpret results,
subsystem presets).

## Invocation

```
scripts/intel-query.sh <subcommand> [args...]
```

Always use the wrapper — never open-code Neo4j access.

## Subcommand reference

### Code symbol queries (Ori + reference repos)
- `symbols <name> [--repo R] [--kind K]` — find symbols by name
- `callers <name> --repo ori` — who calls this function?
- `callees <name> --repo ori` — what does it call?
- `file-symbols <path-fragment> --repo ori` — all symbols in matching files
- `similar <name> [--repo rust,swift,go,koka]` — semantic equivalents

### Issue/PR search (external repos)
- `search <terms>` — full-text search
- `compare <terms>` — cross-repo convergence (same issue in 2+ languages)
- `fixed <terms> [--repo ...]` — closed-as-fixed issues
- `hot [--repo R]` — highly-reacted issues
- `ori-arc` / `ori-inference` / `ori-codegen` / `ori-patterns` / `ori-diagnostics` — subsystem presets
- `sentiment pain|controversy|excitement [--repo R]` — ranked by emotional weight
- `landscape [--repo R]` / `ori-sentiment` — aggregated sentiment maps

### Administrative
- `status` — graph health + node/edge counts
- `cypher "<query>"` — raw Cypher escape hatch

## Output mode
- Default: JSON (for agent pipelines). (§08 changes this to tty-aware.)
- `--human`: human-readable text.

## Related canonical docs
- `.claude/rules/intelligence.md` — workflow inventory, subsystem mapping
- `.claude/skills/dual-tpr/compose-intel-summary.md` — SSOT summary template (§03)
