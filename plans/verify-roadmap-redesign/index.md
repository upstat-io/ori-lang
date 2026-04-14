---
reroute: true
name: "Verify Roadmap"
full_name: "Verify Roadmap Redesign — Cross-Plan Coherence Auditor"
status: active
reviewed: false
order: 1
---

# Verify Roadmap Redesign Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Frontmatter Schema, Strict Parser & Shared Types
**File:** `section-01-frontmatter-schema.md` | **Status:** Not Started

```
frontmatter, schema, YAML, metadata, validation, strict parser
plan_corpus.py, scripts, SSOT, sole source of truth
seven schemas: plan index, plan section, roadmap section, overview, bug-tracker section, fix-BUG, completed index
RoadmapSectionSchema, tier, last_verified, spec, roadmap-specific fields
reroute, parallel, plan, status, order, reviewed, research, TprStatus, none, findings, resolved, clean
closed status enum, corpus-derived, whitelist enforcement
depends_on logical IDs, intra-plan NN, cross-plan plan-name#NN, name-based resolution, reject full paths, reject directory slug
PlanIndexSchema.name mandatory, Corpus.name_index, resolve_dep
Finding, Severity, FindingCategory, FindingSubtype, Corpus, shared types, two-level taxonomy
ITEM_VERIFICATION subtypes, MISSING_MATRIX_COVERAGE, MISSING_SEMANTIC_PIN, Section 04 reuse
canonical status normalizer, plain STATUS_CONTRADICTION findings, no safety_class in plan_corpus
SafeFix / ExposureReview OWNED BY Section 03, write-back policy, classify_safety at the edge
WriteBackContext, has_recent_commits, git queries at CLI edge only, pure library
load_and_validate boundary, Either[Finding, ValidatedFile], single try/except site
CorpusParseError, hard-fail, anti-swallow, PARSE_ERROR category
LEAK:swallowed-error, errors=replace, YAMLError, duplicate key, anchor, merge key
BOM, zero-width char, CRLF, invalid UTF-8, multi-document YAML
two-stage discovery classifier, plan candidate vs container vs unknown, MISSING_INDEX_MD, UNCLASSIFIED_DIRECTORY
fixture corpus, TDD, semantic pin, negative pin, fix-BUG complete+none/clean/resolved/findings matrix
docgen --check, CI drift gate, generated schema reference
pilot migration, seven schema classes, aot-perf, pkg_mgmt, project-reorganization, ori-ui-framework, roadmap section-00-parser, bug-tracker overview, fix-BUG-04-077, aims-10
index.md, 00-overview.md, section-*.md, fix-BUG-*.md, plans/completed/*/index.md, plans/roadmap/section-*.md
inconsistent fields, plan vs reroute vs parallel
frontmatter/body contradiction, status drift, date drift
```

---

### Section 02: DAG Builder & Conflict Classifier
**File:** `section-02-dag-builder.md` | **Status:** Not Started

```
DAG, dependency graph, topological sort, cycle detection
depends_on, cross-plan reference, shared subsystem
CONFLICT, SUPERSEDED, BLOCKED, STATUS_CONTRADICTION
MISSING_DEPENDENCY, DEAD_REFERENCE, priority inversion
repr-opt, locality-representation-unification, prerequisite
plans/ori_lsp, dead reference, nonexistent directory
test-suite-health, roadmap section-21A, supersession
BUG-04-039, iterator-element-ownership, status incoherence
```

---

### Section 03: Findings Report & Write-Back
**File:** `section-03-findings-report.md` | **Status:** Not Started

```
findings report, cross-plan report, write-back
fix mechanism, auto-fix, status reconciliation
SafeFix, ExposureReview, classify_safety, ClassifiedFinding, SafetyClass
WriteBackContext, has_recent_commits, git signals at the CLI edge
continue-roadmap integration, roadmap-scan.sh
report format, JSON, markdown, actionable findings
source plan, target plan, bilateral update
imports Finding/FindingCategory/FindingSubtype from plan_corpus (no shadow types)
```

---

### Section 04: Item-Level Verifier Preservation
**File:** `section-04-item-verifier.md` | **Status:** Not Started

```
item-level verification, matrix coverage, semantic pins
hygiene audit, test quality, verification criteria
verify-roadmap.md, 900 lines, refactor, extract
reusable phase, downstream phase, section-scoped
batch agents, review agents, update agents
ITEM_VERIFICATION category imported from plan_corpus (01.3 SSOT), no shadow enum
MISSING_MATRIX_COVERAGE, MISSING_SEMANTIC_PIN, MISSING_NEGATIVE_PIN, WEAK_TEST, HYGIENE_VIOLATION, INCOMPLETE_CHECKBOX, SCOPE_GAP
```

---

### Section 05: Validation & Known-Issue Sweep
**File:** `section-05-validation.md` | **Status:** Not Started

```
validation, test cases, known issues, sweep
repr-opt locality inversion, dead ori_lsp reference
test-suite-health roadmap drift, stale active plans
frontmatter body contradiction, inconsistent fields
BUG-04-039 incoherence, JIT exception handling stale
skill promotion, commands to skills migration
SKILL.md, .claude/skills/verify-roadmap/
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Frontmatter Schema, Strict Parser & Shared Types | `section-01-frontmatter-schema.md` |
| 02 | DAG Builder & Conflict Classifier | `section-02-dag-builder.md` |
| 03 | Findings Report & Write-Back | `section-03-findings-report.md` |
| 04 | Item-Level Verifier Preservation | `section-04-item-verifier.md` |
| 05 | Validation & Known-Issue Sweep | `section-05-validation.md` |
