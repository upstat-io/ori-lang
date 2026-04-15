---
reroute: true
name: "Query-Intel Adoption"
full_name: "Query-Intel Adoption"
status: queued
order: 999
---

# Query-Intel Adoption Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords — symbol names, rule filenames, skill names, hook names
2. Find the section ID
3. Open the section file

## Provenance

- Merged TPR findings: `plans/query-intel-adoption/tpr-2026-04-14-merged.json`
- Source TPR run: `/tmp/ori-tpr-JrlGnDIS` (codex + gemini, 2026-04-14)
- Related memory (user-facing rules): `feedback_intelligence_graph_is_full_codebase.md`, `reference_lang_intelligence_db.md`
- Completed predecessor (do not duplicate): `plans/completed/lang-intelligence/`

---

## Keyword Clusters by Section

### Section 01: Promote /query-intel to an auto-triggerable Skill
**File:** `section-01-promote-to-skill.md` | **Status:** Complete

```
/query-intel, query-intel SKILL.md, .claude/skills/query-intel/
auto-trigger, description frontmatter, harness activation
.claude/commands/query-intel.md alias, thin wrapper
scripts/intel-query.sh, discoverability
ori-syntax SKILL.md reference pattern
```

---

### Section 02: CLAUDE.md expansion
**File:** `section-02-claude-md-expansion.md` | **Status:** Complete

```
CLAUDE.md, Commands section (line 140), Key Paths (line 182)
Reference Repos section (line 186), graph-first paragraph
Ownership & Deferral (line 38), Fact-check rule strengthening
Compiler Coding Guidelines, graph-reconnaissance bullet
```

---

### Section 03: SSOT — compose-intel-summary helper
**File:** `section-03-compose-intel-summary-ssot.md` | **Status:** Complete

```
compose-intel-summary.md, .claude/skills/dual-tpr/
@-include pattern, SSOT, LEAK:algorithmic-duplication
polling-protocol.md sibling, compose-rules-brief.md sibling
Intelligence Summary template, bounded 500-char digest
review-work/SKILL.md:251, tpr-review/SKILL.md Step 0.75
review-plan.md:96, review-work.md:71, independent-review.md:221, review-bugs.md:156
availability check, file-symbols, callers, callees, similar
```

---

### Section 04: Rule files — graph-first guidance
**File:** `section-04-rules-graph-first.md` | **Status:** Complete

```
.claude/rules/arc.md, aims-rules.md, typeck.md, types.md
.claude/rules/tests.md, impl-hygiene.md, canonicalization.md, patterns.md
.claude/rules/intelligence.md workflow inventory refresh
LEAK:scattered-knowledge, cross-repo prior art, reference repos
graph-first paragraph template, Swift SILOptimizer, Lean4 IR, rustc, Koka, Gleam
```

---

### Section 05: Missing-trigger skills & commands
**File:** `section-05-missing-trigger-skills.md` | **Status:** In Progress

```
.claude/skills/verify-tpr/SKILL.md, blast-radius on findings
.claude/skills/sync-claude/SKILL.md, file-symbols crate-symbol inventory
.claude/skills/fix-next-bug/SKILL.md, callers-only lightweight blast-radius preview
.claude/commands/sync-spec.md, sync-grammar.md, verify-roadmap.md
GAP:missing-trigger, concrete workflow step, @-include §03
```

---

### Section 06: Plan schema — mandatory Intelligence Reconnaissance
**File:** `section-06-plan-schema-recon.md` | **Status:** Not Started

```
.claude/skills/create-plan/plan-schema.md, Section File Template
{NN}.0 Intelligence Reconnaissance subsection
scripts/plan_corpus.py check, transition-period WARNING
retrofit, plans/* audit, aggressive/moderate/passive retrofit policy
subsystem mapping from intelligence.md
availability check in plan section, preset query, blast-radius
```

---

### Section 07: Hook-heavy ambient automation
**File:** `section-07-pre-review-intel-hook.md` | **Status:** Not Started

```
.claude/hooks/pre-review-intel.sh, UserPromptSubmit hook
.claude/settings.json hooks registration
review-family slash-commands matcher
/tpr-review, /review-work, /review-plan, /independent-review, /review-bugs
/tp-help, /fix-bug, /fix-next-bug
hookSpecificOutput.additionalContext, bounded summary injection
--preview dry-run mode
reference: block-banned-commands.sh, classify-review-command.py
scripts/intel-query.sh status graceful degradation
```

---

### Section 08: Tool UX & output shapes
**File:** `section-08-tool-ux-and-output.md` | **Status:** Not Started

```
scripts/intel-query.sh, --help, -h, --human tty default, --json piped
blast-radius composite subcommand, --format md output mode
.claude/commands/query-intel.md teaching surface expansion
../lang_intelligence/neo4j/query_graph.py, 1240 lines
ASCII call-tree, callers/callees tree output
file-symbols grouped by kind (function, type, sum_type, trait)
clickable file:line deep-links, GitHub issue URL
session caching, /tmp/intel-cache/, 10-min TTL
BLOAT:json-as-default, GAP:output-shape
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Promote /query-intel to an auto-triggerable Skill | `section-01-promote-to-skill.md` |
| 02 | CLAUDE.md expansion | `section-02-claude-md-expansion.md` |
| 03 | SSOT: compose-intel-summary helper | `section-03-compose-intel-summary-ssot.md` |
| 04 | Rule files: graph-first guidance | `section-04-rules-graph-first.md` |
| 05 | Missing-trigger skills & commands | `section-05-missing-trigger-skills.md` |
| 06 | Plan schema: mandatory Intelligence Reconnaissance | `section-06-plan-schema-recon.md` |
| 07 | Hook-heavy ambient automation | `section-07-pre-review-intel-hook.md` |
| 08 | Tool UX & output shapes | `section-08-tool-ux-and-output.md` |
