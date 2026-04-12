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
  status: resolved
  updated: 2026-04-12
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
- **Code reviews** (/review-work): Query for prior art relevant to the reviewed changes
- **Plan reviews** (/review-plan): Query for cross-language precedent on plan assumptions
- **Proposals** (/create-draft-proposal): Query cross-language precedent for the Prior Art section
- **Pattern review** (/design-pattern-review): Query for equivalent implementations
- **Roadmap** (/continue-roadmap): After focus resolution, query for section-relevant intelligence

## How to Query

Always use the canonical helper — never open-code Neo4j access:
  scripts/intel-query.sh search "pattern matching exhaustiveness"
  scripts/intel-query.sh compare "type inference"
  scripts/intel-query.sh fixed "memory leak" --repo rust,swift
  scripts/intel-query.sh hot --repo rust
  scripts/intel-query.sh ori-arc                          # also: ori-inference, ori-codegen, ori-patterns, ori-diagnostics
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
name: query-intel
description: "Query the cross-language intelligence graph for prior art, similar bugs, and design patterns."
allowed-tools: Bash, Read, Grep, Glob
argument-hint: "[search|compare|fixed|hot|ori-*|cypher|status] [args...] (ori presets: ori-arc, ori-inference, ori-codegen, ori-patterns, ori-diagnostics)"
---

# /query-intel

Run: `scripts/intel-query.sh $ARGUMENTS`

If `$ARGUMENTS` is empty, run: `scripts/intel-query.sh status`

Present results to the user with context. For search results, highlight:
- Cross-repo patterns (same issue in 2+ languages)
- High-signal items (many reactions, MEMBER authors, completed state_reason)
- Ori-relevant items (features Ori is building or planning)
```

**Implementation checklist**:
- [x] Create `.claude/commands/query-intel.md`
- [x] Test: `/query-intel search "exhaustiveness"` executes successfully (verified 2026-04-12 against live Neo4j — `status: ok`, 0 hits for this term; confirms plumbing works end-to-end)
- [x] Test: `/query-intel` with no args shows graph stats (verified 2026-04-12 — 10 repos, 298K issues)
- [x] Test: `/query-intel ori-arc` runs Ori presets (verified 2026-04-12 — `status: ok`; 5 presets: ori-arc, ori-inference, ori-codegen, ori-patterns, ori-diagnostics)
- [x] Test: `/query-intel cypher "MATCH (r:Repo) RETURN r.name"` runs raw Cypher (verified 2026-04-12 — returns all 10 repo names)

### Subsection 02.2 close-out

**`/improve-tooling` retrospective**: Is the command discoverable enough? Should it be listed in CLAUDE.md §Commands?

---

## 02.R Third Party Review Findings

- [x] `[TPR-02-001-codex][medium]` `plans/lang-intelligence/index.md:14` — DRIFT: index.md and 00-overview.md showed Section 02 as `not-started` when section file had `in-progress`.
  Resolved: Fixed on 2026-04-12. Updated both rollup tables to `in-progress`.
- [x] `[TPR-02-002-codex][low]` `plans/lang-intelligence/section-02-claude-rules.md:119` — GAP: Live-graph verification items lacked context (Codex re-ran with Neo4j container stopped).
  Resolved: Fixed on 2026-04-12. Added verification timestamps and specific results to each test checkbox.
- [x] `[TPR-02-003-codex][low]` `plans/lang-intelligence/section-02-claude-rules.md:119` — Wording: "returns results" contradicts "0 results" verification note.
  Resolved: Fixed on 2026-04-12. Changed to "executes successfully" — the test verifies plumbing, not specific hits.
- [x] `[TPR-02-004-codex][medium]` `.claude/rules/intelligence.md:22` — GAP: Rule omits `/review-work` and `/review-plan` from "When to Query" despite success criteria saying "reviews" broadly.
  Resolved: Fixed on 2026-04-12. Added `/review-work` and `/review-plan` entries to the "When to Query" section.
- [x] `[TPR-02-005-codex][low]` `plans/lang-intelligence/section-02-claude-rules.md:58` — DRIFT: Embedded "Content design" snippet in 02.1 was stale after TPR-02-004 fix.
  Resolved: Fixed on 2026-04-12. Synced the snippet with the actual implemented rule (added /review-work, /review-plan, hot subcommand).
- [x] `[TPR-02-006-codex][low]` `.claude/commands/query-intel.md:5` — GAP: Command argument-hint only advertised `ori-arc` but 5 ori presets exist.
  Resolved: Fixed on 2026-04-12. Updated argument-hint to `ori-*` with full preset list. Updated rule and plan snippets to note the full family.
- [x] `[TPR-02-007-codex][low]` `plans/lang-intelligence/section-02-claude-rules.md:104` — DRIFT: Plan's embedded query-intel snippet missing name/allowed-tools/argument-hint frontmatter.
  Resolved: Fixed on 2026-04-12. Synced snippet with actual command file frontmatter.
- [x] `[TPR-02-008-gemini][medium]` `plans/lang-intelligence/section-02-claude-rules.md:104` — Same as TPR-02-007-codex (agreement on missing frontmatter in snippet).
  Resolved: Same fix as TPR-02-007-codex.
- [x] `[TPR-02-009-gemini][low]` `plans/lang-intelligence/00-overview.md:20` — DRIFT: Overview success criteria said "search, compare, and Ori preset queries" but actual command supports more subcommands.
  Resolved: Fixed on 2026-04-12. Updated to "search, compare, fixed, hot, ori-* presets, and raw cypher".
- [x] `[TPR-02-010-gemini][low]` `plans/lang-intelligence/section-02-claude-rules.md:124` — GAP: Test checkbox "runs the ARC preset" didn't acknowledge the full preset family.
  Resolved: Fixed on 2026-04-12. Updated to note all 5 presets.

## Completion Checklist

- [x] `.claude/rules/intelligence.md` exists and auto-loads
- [x] `.claude/commands/query-intel.md` exists and works as slash command
- [x] Both files follow existing patterns (rule file format, command file format)
- [x] No test regressions: `timeout 150 ./test-all.sh`
- [ ] `/tpr-review` clean
- [ ] `/impl-hygiene-review` clean
- [ ] `/improve-tooling` section-close sweep
