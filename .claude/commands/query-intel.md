---
name: query-intel
description: "Query the cross-language intelligence graph for prior art, similar bugs, and design patterns."
allowed-tools: Bash, Read, Grep, Glob
argument-hint: "[search|compare|fixed|hot|ori-arc|cypher|status] [args...]"
---

# /query-intel

Run: `scripts/intel-query.sh $ARGUMENTS`

If `$ARGUMENTS` is empty, run: `scripts/intel-query.sh status`

Present results to the user with context. For search results, highlight:
- Cross-repo patterns (same issue in 2+ languages)
- High-signal items (many reactions, MEMBER authors, completed state_reason)
- Ori-relevant items (features Ori is building or planning)
