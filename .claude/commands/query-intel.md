---
name: query-intel
description: "Query the intelligence graph — cross-language prior art (issues/PRs) AND code symbols (Ori + reference repos: functions, types, call graphs)."
allowed-tools: Bash, Read, Grep, Glob
argument-hint: "[search|compare|fixed|hot|sentiment|landscape|symbols|callers|callees|file-symbols|similar|ori-*|cypher|status] [args...] (symbol: symbols/callers/callees/file-symbols <name>; similarity: similar <name> --repo rust)"
---

# /query-intel

Run: `scripts/intel-query.sh $ARGUMENTS`

If `$ARGUMENTS` is empty, run: `scripts/intel-query.sh status`

Present results to the user with context.

## Issue/PR results
- Cross-repo patterns (same issue in 2+ languages)
- High-signal items (many reactions, MEMBER authors, completed state_reason)
- Ori-relevant items (features Ori is building or planning)

## Code symbol results
The graph indexes 32K+ symbols, 1.4K files, and 24K+ call edges from Ori and reference repos. Use these for codebase navigation:
- `symbols <name>` — find types/functions by name (case-insensitive), filter with `--kind` and `--repo`
- `callers <name>` — who calls this function? (reverse call graph)
- `callees <name>` — what does this function call? (forward call graph)
- `file-symbols <path>` — list all symbols declared in matching files
- `similar <name>` — find semantically equivalent symbols in other repos (vector embeddings)
