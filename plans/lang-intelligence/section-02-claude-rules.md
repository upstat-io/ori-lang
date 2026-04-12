---
section: "02"
title: "Claude Rules & Commands"
status: in-progress
reviewed: false
goal: "Create the intelligence rule file (auto-loaded every conversation) and the /query-intel slash command for direct access."
success_criteria:
  - ".claude/rules/intelligence.md exists with paths trigger and graceful degradation"
  - ".claude/commands/query-intel.md exists and works as a slash command"
  - "Rule triggers intelligence queries during design decisions, bug fixes, and reviews"
  - "Command supports search, compare, fixed, hot, ori-* presets, and raw cypher"
depends_on: ["01"]
sections:
  - id: "02.1"
    title: "Intelligence Rule File"
    status: complete
  - id: "02.2"
    title: "Query Intel Command"
    status: complete
third_party_review:
  status: none
  updated: null
---

# 02 Claude Rules & Commands

## 02.0 Goal

Two new files that wire the intelligence graph into Claude's automatic behavior:
1. `.claude/rules/intelligence.md` — auto-loaded rule that tells Claude when and how to query
2. `.claude/commands/query-intel.md` — slash command for direct access

## 02.1 Intelligence Rule File

**File**: `.claude/rules/intelligence.md`

**Paths trigger**: `"**"` — always loaded, like `ori-syntax.md`. Intelligence is relevant in any context.

**Content design** (follows existing rule file patterns from `.claude/rules/compiler.md` etc.):

```markdown
---
paths: "**"
---

# Intelligence Graph

## Availability

The intelligence graph at `../lang_intelligence/` is OPTIONAL. Check before querying:
  scripts/intel-query.sh status
If unavailable, proceed normally — intelligence is additive, never blocking.

## When to Query

Query the intelligence graph proactively in these workflows:

- **Design decisions**: Before choosing an approach, query for how reference compilers handled it
- **Bug investigation** (/fix-bug Phase 1): Query for similar bugs across languages
- **TPR reviews** (/tpr-review Step 2): Pre-query relevant prior art for the evidence packet
- **Proposals** (/create-draft-proposal): Query cross-language precedent for the Prior Art section
- **Pattern review** (/design-pattern-review): Query for equivalent implementations
- **Roadmap** (/continue-roadmap): After focus resolution, query for section-relevant intelligence

## How to Query

Always use the canonical helper — never open-code Neo4j access:
  scripts/intel-query.sh search "pattern matching exhaustiveness"
  scripts/intel-query.sh compare "type inference"
  scripts/intel-query.sh fixed "memory leak" --repo rust,swift
  scripts/intel-query.sh ori-arc
  scripts/intel-query.sh cypher "MATCH (i:Issue)-[:FIXES]->(b) RETURN count(i)"

## How to Use Results

Results are for DISCOVERY, not replacement:
- A hit means "investigate this" — read the actual source code and issue
- Weight by: state_reason (completed > not_planned), reactions, author_association (MEMBER > NONE)
- Cross-language convergence is highest signal (3+ repos hit same issue class)
- Never cite a Neo4j result without verifying against the actual code/issue
```

**Implementation checklist**:
- [x] Create `.claude/rules/intelligence.md` with the content above
- [x] Verify paths trigger loads in all contexts (test by reading the rule from a compiler directory)
- [x] Verify the rule doesn't conflict with existing rules (check for duplicate advice)

### Subsection 02.1 close-out

**`/improve-tooling` retrospective**: Is the rule text clear enough? Would example queries for specific subsystems (ARC, type inference, pattern matching) be useful inline?

---

## 02.2 Query Intel Command

**File**: `.claude/commands/query-intel.md`

**Design**: Slash command that wraps `scripts/intel-query.sh` with convenience. Usage: `/query-intel <subcommand> [args]`

```markdown
---
description: "Query the cross-language intelligence graph for prior art, similar bugs, and design patterns."
---

# /query-intel

Run: scripts/intel-query.sh $ARGUMENTS

If $ARGUMENTS is empty, run: scripts/intel-query.sh status

Present results to the user with context. For search results, highlight:
- Cross-repo patterns (same issue in 2+ languages)
- High-signal items (many reactions, MEMBER authors, completed state_reason)
- Ori-relevant items (features Ori is building or planning)
```

**Implementation checklist**:
- [x] Create `.claude/commands/query-intel.md`
- [x] Test: `/query-intel search "exhaustiveness"` returns results
- [x] Test: `/query-intel` with no args shows graph stats
- [x] Test: `/query-intel ori-arc` runs the ARC preset
- [x] Test: `/query-intel cypher "MATCH (r:Repo) RETURN r.name"` runs raw Cypher

### Subsection 02.2 close-out

**`/improve-tooling` retrospective**: Is the command discoverable enough? Should it be listed in CLAUDE.md §Commands?

---

## 02.R Third Party Review Findings

- None.

## Completion Checklist

- [x] `.claude/rules/intelligence.md` exists and auto-loads
- [x] `.claude/commands/query-intel.md` exists and works as slash command
- [x] Both files follow existing patterns (rule file format, command file format)
- [x] No test regressions: `timeout 150 ./test-all.sh`
- [ ] `/tpr-review` clean
- [ ] `/impl-hygiene-review` clean
- [ ] `/improve-tooling` section-close sweep
