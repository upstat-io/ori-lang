# Plan Template

Use this template when creating new plans in `plans/`.

## Structure

New plans should include:
1. **An index file** for keyword-based discovery (see `plans/roadmap/index.md` as reference)
2. **Individual section files** for detailed task tracking
3. **Clear status tracking** via YAML frontmatter and checkboxes

---

## Directory Layout

```
plans/{plan-name}/
├── index.md           # Keyword clusters for quick finding
├── 00-overview.md     # High-level goals, tiers, dependencies
├── section-01-*.md    # First section
├── section-02-*.md    # Second section
└── ...
```

---

## Index File Template

Create `index.md` with keyword clusters for each section:

```markdown
# {Plan Name} Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: {Title}
**File:** `section-01-{name}.md` | **Status:** Not Started

\`\`\`
keyword1, keyword2, keyword3
formal term, common alias
feature, related concept
\`\`\`

---

### Section 02: {Title}
**File:** `section-02-{name}.md` | **Status:** Not Started

\`\`\`
keywords here
\`\`\`

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | {Title} | `section-01-{name}.md` |
| 02 | {Title} | `section-02-{name}.md` |
```

---

## Section File Template

Each section file follows this structure:

```markdown
---
section: "{ID}"
title: {Title}
status: not-started
goal: {One-line goal}
sections:
  - id: "{ID}.1"
    title: {Subsection}
    status: not-started
---

# Section {ID}: {Title}

**Status:** 📋 Planned
**Goal:** {Description}

---

## {ID}.1 {Subsection Title}

- [ ] {Task description}
  - [ ] {Sub-task}
  - [ ] {Sub-task}

- [ ] {Another task}

---

## {ID}.N Completion Checklist

- [ ] {Final checklist item}
- [ ] {Final checklist item}

**Exit Criteria:** {What must be true for completion}
```

---

## Status Conventions

| YAML Status | Meaning | Header Emoji |
|-------------|---------|--------------|
| `not-started` | No work done | 📋 Planned |
| `in-progress` | Partial completion | 🔶 Partial |
| `complete` | All done | ✅ Complete |

---

## Performance-Sensitive Plans

For plans touching **performance-critical components** (lexer, parser, type checker, evaluator, codegen), include benchmark checkpoints:

### When to Benchmark

| Component | Benchmark? | Skill |
|-----------|------------|-------|
| Lexer (`ori_lexer`) | ✅ Yes | `/benchmark short` |
| Parser (`ori_parse`) | ✅ Yes | `/benchmark short` |
| Type checker (`ori_typeck`) | ✅ Yes | `/benchmark short` |
| Evaluator (`ori_eval`) | ⚠️ Maybe | Manual profiling |
| Codegen (`ori_llvm`) | ⚠️ Maybe | Manual profiling |
| CLI, formatting, LSP | ❌ No | Not perf-critical |

### Adding Benchmark Checkpoints

In sections that modify hot paths, add:

```markdown
## X.N Performance Validation

- [ ] Run `/benchmark short` before changes (record baseline)
- [ ] Run `/benchmark short` after changes
- [ ] No regressions >5% vs baseline
- [ ] Document any intentional tradeoffs
```

Only add this for sections that:
1. Modify hot code paths (token processing, expression parsing, type unification)
2. Change data structures (token storage, AST nodes, type representations)
3. Add new algorithmic complexity

**Do NOT add benchmarks for**: error messages, CLI flags, documentation, tests, non-hot-path features.

---

## Reference

See `plans/roadmap/` for a complete example:
- `plans/roadmap/index.md` — Keyword index with 26 sections
- `plans/roadmap/section-*.md` — Individual section files
- `plans/roadmap/00-overview.md` — High-level overview
