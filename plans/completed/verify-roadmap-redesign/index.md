---
reroute: true
name: "Verify Roadmap"
full_name: "Verify Roadmap Redesign — Cross-Plan Coherence Auditor"
status: resolved
reviewed: true
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
**File:** `section-01-frontmatter-schema.md` | **Status:** Complete

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
**File:** `section-02-dag-builder.md` | **Status:** Complete

```
DAG, dependency graph, cycle detection, SCC, Tarjan
scripts/plan_corpus/dag.py, NodeKind, NodeId, Reference, Edge, Dag, DagReport
seven schema classes as nodes: PLAN_INDEX, PLAN_SECTION, ROADMAP_SECTION, OVERVIEW
BUG_TRACKER_SECTION, FIX_BUG, COMPLETED_INDEX
source-kind taxonomy, SourceKind enum
EXPLICIT_DEPENDS_ON, HTML_COMMENT_CONVENTION, YAML_COMMENT, PROSE_VERB, CODE_FENCE_EXAMPLE
depends_on SSOT, no shadow edges, MISSING_DEPENDENCY force-to-frontmatter
code_fence_exclusion, strip_code_blocks, indented_code_blocks
html_comment_grammar, blocked-by, unblocks, supersedes, resolves, rewrites, obsoletes
yaml_frontmatter_comment, extract_yaml_comments, raw-text post-YAML-parse
prose verb expansion: depends on, requires, blocked by, prerequisite, unblocks, supersedes, rewrites, obsoletes
informational verbs: see also, related, inspired by, cf.
subsystem normalization, normalize_subsystem, SUBSYSTEM_ALIASES, Cargo.toml workspace
8 classifiers: CONFLICT, SUPERSEDED (reroute + in-place-rewrite), BLOCKED, STATUS_CONTRADICTION
CROSS_EDGE_TEMPORAL_DRIFT (§02 only; TPR_STALE_VS_EDIT moved to §03)
MISSING_DEPENDENCY, DEAD_REFERENCE, REDUNDANT_DEPENDENCY, ORPHANED_PLAN
classifier precedence ladder, PRECEDENCE_RANK, deterministic ordering
source-kind severity ladder: EXPLICIT HIGH, HTML/YAML MEDIUM, PROSE LOW
priority inversion, transitive chains, root blocker identification, minimum unblock set
topological sort CUT (no consumer; Finding L)
handoff contract with §03, chain encoding Option A (typed Finding.dependency_chain + Finding.source_kind), source_column disambiguator
enrich_resolve_dep_finding, Finding.id collision mitigation
repr-opt, locality-representation-unification, prerequisite edge short-circuit
plans/ori_lsp, dead reference, plans/completed/ resolution
test-suite-health, roadmap section-21A, in-place-rewrite detection
BUG-04-039, iterator-element-ownership, YAML comment inference
```

---

### Section 03: Findings Report & Write-Back
**File:** `section-03-findings-report.md` | **Status:** Complete

```
findings report, cross-plan report, write-back
fix mechanism, auto-fix, status reconciliation
SafeFix, ExposureReview, classify_safety, ClassifiedFinding, SafetyClass
WriteBackContext, has_recent_commits, git signals at the CLI edge
PreimageRecord, concurrent session safety, preimage hash, atomic write, os.replace
frontmatter text patcher, targeted regex patching, PyYAML read-only
comment preservation, key ordering preservation, YAML comment DAG signal
plan: to name: rename collision guard, reviewed: false workflow gate
FM_DECLARED_VS_BODY_DERIVED always ExposureReview, normalizer aspirational marker
dead reference audit trail in fixes-applied.json, no inline HTML comments
--quick mode bypasses WriteBackContext, BLOCKED + DEAD_REFERENCE only
--full mode all classifiers + auto-fix + git signals
roadmap_scan.py shadow parser migration tracking
continue-roadmap integration, roadmap-scan.sh
report format, JSON, markdown, console, actionable findings
source plan, target plan, bilateral update
imports Finding/FindingCategory/FindingSubtype from plan_corpus (no shadow types)
```

---

### Section 04: Item-Level Verifier Preservation
**File:** `section-04-item-verifier.md` | **Status:** Complete

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
**File:** `section-05-validation.md` | **Status:** Complete

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
