---
plan: "verify-roadmap-redesign"
title: "Verify Roadmap Redesign: Cross-Plan Coherence Auditor"
status: in-progress
supersedes:
  - ".claude/commands/verify-roadmap.md"
  - ".claude/skills/plan-audit/planlib.py"
references:
  - "/tmp/ori-tpr-iiArmHok/merged.json"
  - "plans/roadmap/index.md"
---

# Verify Roadmap Redesign: Cross-Plan Coherence Auditor

## Mission

Redesign the `/verify-roadmap` skill from a section-local item verifier into a cross-plan coherence auditor that treats the entire planning corpus (master roadmap + 17 reroute plans + completed plans + bug tracker) as one interconnected system. The current skill audits individual checkbox items within `plans/roadmap/section-*.md` files but has zero awareness of reroute plans, cross-plan conflicts, or dependency inversions. The redesigned skill detects when goals of one plan conflict with another, when dependencies are inverted (active plan depending on queued prerequisite), when references point to nonexistent plans, and when metadata has drifted from reality.

## Mission Success Criteria

- [ ] `/verify-roadmap` catches all 8 known TPR test cases automatically (see Known Bugs below)
- [ ] `scripts/plan_corpus/` (Python package, invoked via `python -m scripts.plan_corpus`) exists as the single source of truth for corpus schema, parsing, discovery, and finding types — Sections 02-05 import it; no shadow parsers or types anywhere (Section 01)
- [ ] Strict parser hard-fails on malformed YAML: missing/unclosed `---`, YAML parse errors, non-mapping roots, duplicate keys, anchors/aliases/merge keys, multi-document YAML, UTF-8 BOM, zero-width chars, invalid UTF-8 bytes (directly inverts `planlib.py:250-270,350` LEAK:swallowed-error anti-patterns) (Section 01)
- [ ] Schema validation rejects inconsistent frontmatter (mixed `reroute`/`plan`/`parallel` fields) with whitelist enforcement (unknown fields rejected) across SEVEN file classes: plan index, plan section, roadmap section (distinct `tier`/`last_verified`/`spec` fields), overview, bug-tracker section, fix-BUG, completed-plan index (Section 01)
- [ ] `depends_on` convention standardized: intra-plan `"NN"`, cross-plan `"plan-name#NN"` resolving against the target plan's `name:` field (stable across directory renames); full paths AND directory-slug-style cross-plan IDs rejected (Section 01)
- [ ] Two-level Finding taxonomy (`FindingCategory × FindingSubtype`) OWNED by Section 01.3; Phase 4 (Section 04) `ITEM_VERIFICATION` subtypes live there, not in Section 04 (no shadow taxonomy)
- [ ] `plan_corpus.py` is a pure library — no git queries, no I/O outside explicit file reads; `SafeFix`/`ExposureReview` classification lives in Section 03 (write-back phase) consuming caller-supplied `has_recent_commits` signal
- [x] DAG construction detects priority inversions (active plan depending on queued prerequisite) using the logical-ID resolver from Section 01 and covers ALL seven §01.2 schema classes as nodes (plan index, plan section, roadmap section, overview, bug-tracker section, fix-BUG file, completed-plan index) — Section 02
- [x] Dead references to nonexistent `plans/*/` directories are flagged as DEAD_REFERENCE (Section 02), with code-fence examples and indented-code blocks excluded and HTML-comment + YAML-frontmatter-comment references scanned for reverse-edge signal (`unblocks`, `supersedes`, `rewrites`, `obsoletes`)
- [x] Classifier stack is deterministic: source-kind tagging (`EXPLICIT_DEPENDS_ON | HTML_COMMENT_CONVENTION | YAML_COMMENT | PROSE_VERB | CODE_FENCE_EXAMPLE`), documented precedence ladder (PARSE_ERROR > DEAD_REFERENCE > CYCLE > BLOCKED > CONFLICT > SUPERSEDED > STATUS_CONTRADICTION > MISSING_DEPENDENCY > REDUNDANT_DEPENDENCY > ORPHANED_PLAN), and TDD-enforced ordering (Section 02)
- [ ] Frontmatter/body status contradictions are detected via the canonical status normalizer (Section 01.4, pure fact-producer); Section 03 classifies findings into `SafeFix` / `ExposureReview` at write-back time and auto-fixes only the `SafeFix` class (Sections 01, 03)
- [ ] The existing item-level verification (matrix coverage, semantic pins, hygiene) still works for specific sections (Section 04)
- [ ] Full-corpus migration to the canonical schema is completed by the single-ownership sweep (Section 05.3)
- [ ] The skill is promoted to `.claude/skills/verify-roadmap/SKILL.md` with proper skill structure (Section 05)
- [ ] `./test-all.sh` green -- no regressions (all sections)

## Architecture

```
/verify-roadmap [scope]
        |
        v
Phase 1: CORPUS INVENTORY & SCHEMA VALIDATION
  - Scan ALL plans/*/index.md + plans/*/section-*.md
  - Validate frontmatter against canonical schema
  - Detect field mismatches, missing fields, contradictions
  - Output: validated corpus with normalized metadata
        |
        v
Phase 2: DAG CONSTRUCTION
  - Parse depends_on fields from section frontmatter
  - Parse cross-plan references from section bodies
  - Map shared subsystems (files, crates, symbols)
  - Identify supersession markers (reroute vs roadmap overlap)
  - Output: plan-graph (nodes = plans, edges = dependencies)
        |
        v
Phase 3: CONFLICT DETECTION & CLASSIFICATION
  - Walk the DAG for priority inversions (BLOCKED)
  - Check cross-plan goal contradictions (CONFLICT)
  - Check supersession drift (SUPERSEDED)
  - Validate all inter-plan path references exist (DEAD_REFERENCE)
  - Reconcile status fields across dependency chain (STATUS_CONTRADICTION)
  - Detect undocumented dependencies (MISSING_DEPENDENCY)
  - Output: classified findings list
        |
        v
Phase 4: ITEM-LEVEL VERIFICATION (optional, per scope)
  - Reuse existing section-item verifier on touched sections
  - Matrix coverage, semantic pins, hygiene audit
  - Output: per-section findings
        |
        v
Phase 5: WRITE-BACK & REPORT
  - Auto-fix safe issues (frontmatter normalization, status reconciliation)
  - File findings that require human decision
  - Generate cross-plan coherence report
```

## Design Principles

1. **Schema-first**: No semantic analysis runs on unstable metadata. Phase 1 normalizes frontmatter before Phase 2 builds the graph. This prevents the DAG builder from silently skipping plans with non-standard frontmatter (the exact failure mode that caused the current verify-roadmap to miss 17 reroute plans entirely).

2. **Single-package SSOT**: `scripts/plan_corpus/` is the sole home for schema types, parser, discovery walker, `Finding`/classifier types, and the status normalizer. Sections 02-05 import from it; re-implementing any of these elsewhere is a LEAK:scattered-knowledge violation. Markdown schema documentation is GENERATED from the Python types, not authored separately. (Section 01 originally shipped as a single-file module; refactored into a package after the initial SSOT landed.)

3. **Strict parsing, no swallowed errors**: The current `.claude/skills/plan-audit/planlib.py` is the cautionary example — it uses `errors="replace"` (line 250), returns `{}` on YAML parse errors (line 269), and silently drops sections with malformed frontmatter (line 350-351). The redesigned parser HARD-FAILS on every class of malformed input. Permissive parsing on unstable metadata is worse than no auditor — it reports "clean" on corrupt inputs.

4. **Graph-driven conflict detection**: Conflicts are found by graph analysis (cycle detection, reachability, topological sort), not by pairwise text comparison. Two plans conflict when they share a node (subsystem/file/crate) in the graph AND their goals for that node are contradictory. This scales to N plans without N^2 comparisons.

5. **Preserve what works**: The existing item-level verifier (matrix coverage, semantic pins, hygiene audit) is the skill's strongest feature. It becomes Phase 4, invoked selectively on sections that Phase 3 flags as affected. The new phases wrap it, they don't replace it.

## Conventions

### `depends_on` — logical IDs only

All `depends_on` entries use logical identifiers. Full repo-relative paths are REJECTED by the strict parser because file renames silently break every dependent.

- Intra-plan: bare `"NN"` (e.g. `"01"`, `"04B"`) — matches existing corpus usage in `plans/completed/iter-rc-contract/section-02-elem-dec-fn.md:10`, `plans/completed/jit-exception-handling/section-06-lcfail-resolution.md:11`
- Cross-plan: `"plan-name#NN"` where `plan-name` is the value of the target plan's `PlanIndexSchema.name` field (stable logical identifier, survives `git mv`) — e.g. `"Locality Representation Unification#02"`. Directory slugs MUST NOT be used; they are physical file layout (GEMINI-01-002).
- `plan_corpus.resolve_dep()` maps logical IDs to physical `Path` objects at DAG-build time via `Corpus.name_index`. Unknown `plan-name` → `DEAD_REFERENCE/CROSS_PLAN_NAME_NOT_FOUND`. Duplicate `name` across plans → `SCHEMA_VIOLATION/DUPLICATE_PLAN_NAME`.
- Every plan `index.md` MUST declare `name:` (enforced by `PlanIndexSchema`).

This plan's own sibling sections (02, 03, 04, 05) are migrated to logical IDs as part of Section 01.6 (pilot + cascade).

### Status enum — corpus-derived

Plan-level: `active | queued | resolved | not-started | research` — `research` is NOT an invention; it is the declared value on `plans/ori-ui-framework/index.md:5`. The original 01 draft invented a closed enum that excluded it; this would have REJECTED a legitimate live plan.

Section-level: `not-started | in-progress | complete`.

New values require `/create-draft-proposal`, not silent coercion.

## Schema Owners

Seven file classes have distinct schemas, each owned by a Python dataclass in `scripts/plan_corpus/schemas.py`. Live-corpus exemplars cited inline.

| File class | Path pattern | Schema dataclass | Exemplar |
|---|---|---|---|
| Plan index | `plans/*/index.md` (excluding `plans/completed/*/index.md`) | `PlanIndexSchema` | `plans/verify-roadmap-redesign/index.md` |
| Plan section | `plans/*/section-*.md` (excluding `plans/roadmap/section-*.md`) | `PlanSectionSchema` | `plans/verify-roadmap-redesign/section-01-*.md` |
| Roadmap section | `plans/roadmap/section-*.md` | `RoadmapSectionSchema` | `plans/roadmap/section-00-parser.md:1-16` (`tier`, `last_verified`, `spec` fields) |
| Overview | `plans/*/00-overview.md` | `OverviewSchema` | `plans/bug-tracker/00-overview.md` |
| Bug-tracker section | `plans/bug-tracker/section-*.md` | `BugTrackerSectionSchema` | `plans/bug-tracker/section-01-parser-lexer.md:1-6` |
| Fix-BUG file | `plans/bug-tracker/fix-BUG-*.md` | `FixBugSchema` | `plans/bug-tracker/fix-BUG-04-077.md:1-18` |
| Completed-plan index | `plans/completed/*/index.md` | `CompletedIndexSchema` | `plans/completed/aims-10/index.md:1-6` |

The prior 01 draft covered only the first two classes. The third through sixth classes were discovered via `/tp-help` dual-source blind-spot analysis (CODEX finding 7). The seventh class (`RoadmapSectionSchema`) was added on 2026-04-14 per GEMINI-01-001 after verifying that roadmap sections carry `tier: int`, `last_verified: date`, and `spec: list[str]` fields that `PlanSectionSchema` rejects as unknown.

## Section Dependency Graph

```
Section | Depends On
01      | --
02      | 01
03      | 01, 02
04      | 01, 02, 03
05      | 01, 02, 03, 04
```

- Section 01 (Schema) is the foundation -- all other sections depend on normalized metadata via `plan_corpus.py` SSOT.
- Section 02 (DAG) depends on 01 — consumes `plan_corpus.resolve_dep()` and `Corpus.name_index`; emits `STATUS_CONTRADICTION` findings using 01.3's two-level taxonomy.
- Section 03 (Report) depends on 01 AND 02 — imports `Finding`/`FindingCategory`/`FindingSubtype` from `plan_corpus` (01.3 SSOT), consumes 02's classifier output AND 01.4's `normalize_status()` facts; OWNS the SafeFix/ExposureReview taxonomy and `classify_safety()` (relocated from 01.4 on 2026-04-14).
- Section 04 (Item Verifier) depends on 01, 02, AND 03 — imports the `Finding` model + `ITEM_VERIFICATION` subtypes from 01.3 SSOT; emits findings into 03's report format.
- Section 05 (Validation) depends on all prior sections -- it runs the tool against known test cases.

## Implementation Sequence

```
Phase 1 - Foundation
  +-- 01: Strict parser, schema-as-types, shared Finding/classifier types,
          canonical status normalizer, fixture corpus, pilot migration (all seven schema classes)

Phase 2 - Core
  +-- 02.0: Node model (all seven §01.2 schema classes) + source-kind taxonomy
  +-- 02.1: Build DAG from depends_on (logical IDs) — EXPLICIT_DEPENDS_ON edges only;
            body-inferred references collected but NOT promoted to shadow edges
  +-- 02.2: Implement 8 conflict classifiers (CONFLICT, SUPERSEDED [two cases],
            BLOCKED, CROSS_EDGE_TEMPORAL_DRIFT, MISSING_DEPENDENCY, DEAD_REFERENCE,
            REDUNDANT_DEPENDENCY, ORPHANED_PLAN) importing types from 01.3
  +-- 02.3: Transitive priority inversion chains + root blocker identification
  +-- 02.4: Classifier precedence ladder + TDD-enforced determinism tests
  +-- 02.5: Handoff contract with §03 (Option A typed fields:
            Finding.dependency_chain + Finding.source_kind;
            Finding.id disambiguation via source_column; enriched
            resolve_dep findings with precise YAML line numbers)

Phase 3 - Integration
  +-- 03: Report format (Finding.to_json / Finding.to_markdown from 01)
  +-- 03: Auto-fix engine — SafeFix class only; ExposureReview flagged for human
  +-- 04: Extract item-level verifier from current command

Phase 4 - Delivery
  +-- 05: Promote to skill directory (.claude/skills/verify-roadmap/)
  +-- 05: Full-corpus migration sweep (SINGLE OWNERSHIP — 01.6 is pilot only)
  +-- 05: Run against all 8 known test cases
  +-- 05: Fix all known cleanup issues
  Gate: all 8 test cases pass, no regressions
```

**Why this order:**
- Phase 1 must come first because all subsequent phases depend on stable metadata AND shared types (`Finding`, `FindingCategory`, `FindingSubtype`, `Corpus`, `load_and_validate`). Sections 02-05 import from `plan_corpus.py`; they never re-implement.
- Phase 2 must precede Phase 3 because the report format depends on classifier output, but classifier *types* are defined in Phase 1 (Section 01.3).
- Full-corpus migration runs ONCE in Section 05.3, not in 01. Migrating before Sections 02-04 land would force re-migration if those sections discover missing fields. Section 01 does a PILOT (≥1 artifact per schema class, ten artifacts in total) to prove the pipeline end-to-end and surface schema gaps.
- Phase 4 (delivery) gates on all prior phases being complete.

## Known Bugs (Pre-existing)

These are the 8 test cases discovered during the dual-source TPR review (2026-04-14). The redesigned skill must catch all of them.

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| (a) repr-opt active, locality prerequisite queued | Priority inversion -- no DAG validation | Section 02 | Implemented (route A/B split documented) |
| (b) Roadmap 22.2 references `plans/ori_lsp/` (nonexistent) | Dead reference -- no path validation | Section 02 | Implemented |
| (c) test-suite-health 02 says rewrite roadmap 21A, not done | Supersession drift -- no cross-plan tracking | Section 02 | Implemented (structural SUPERSEDED case (ii)) |
| (d) 5+ plans marked active, all sections Not Started | Stale metadata -- no status reconciliation | Section 01 | Not Started |
| (e) section-01 frontmatter `in-progress`, body `COMPLETE` | Intra-file contradiction -- no validation | Section 01 | Not Started |
| (f) Plan indexes use `reroute`/`plan`/`parallel` inconsistently | Schema violation -- no canonical schema | Section 01 | Not Started |
| (g) BUG-04-039 in-progress but blocked by queued plan | Status incoherence across dependency chain | Section 02 | Implemented (route A/B split documented) |
| (h) Section 21A stale "unblocks JIT Exception Handling" ref | Dead reference to completed plan | Section 02 | Implemented (LOW-severity plans/completed/ resolution) |

## Metrics (Current State)

| Artifact | Lines | Notes |
|----------|-------|-------|
| `.claude/commands/verify-roadmap.md` | ~900 | Current skill (will be superseded) |
| `plans/*/index.md` (17 reroute plans) | ~2600 total | Frontmatter varies widely |
| `plans/roadmap/section-*.md` (24 sections) | ~12000 total | Frontmatter/body contradictions |
| `plans/completed/*/index.md` | ~200 total | Missing status/reviewed fields |

## Estimated Effort

| Section | Est. Lines | Complexity | Depends On (logical IDs) |
|---------|-----------|------------|--------------------------|
| 01 Frontmatter Schema, Strict Parser & Shared Types | ~500 | High | -- |
| 02 DAG Builder & Classifier | ~900 | High | `"01"` |
| 03 Findings Report | ~200 | Medium | `"01", "02"` |
| 04 Item Verifier Preservation | ~300 | Medium | `"01", "02", "03"` |
| 05 Validation, Sweep & Skill Promotion | ~300 | Medium | `"01", "02", "03", "04"` |
| **Total new** | **~1700** | | |

Section 01's scope grew because the original "Schema Definition" + "Validation Script" split was a LEAK:scattered-knowledge violation (schema re-authored in two places). The redesigned 01 collapses them into one SSOT Python module and adds the four missing boundary types (shared `Finding`, status normalizer, strict parser, fixture corpus) that Sections 02-05 consumed without owning. This is a net reduction in duplication across the plan.

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Frontmatter Schema, Strict Parser & Shared Types | `section-01-frontmatter-schema.md` | Complete |
| 02 | DAG Builder & Conflict Classifier | `section-02-dag-builder.md` | Complete |
| 03 | Findings Report & Write-Back | `section-03-findings-report.md` | Not Started |
| 04 | Item-Level Verifier Preservation | `section-04-item-verifier.md` | Not Started |
| 05 | Validation, Sweep & Skill Promotion | `section-05-validation.md` | Not Started |

### Section 01 subsection breakdown

| Sub | Title | Output |
|----|-------|--------|
| 01.1 | Strict Parser & Discovery | `scripts/plan_corpus/` package parser, `CorpusParseError`, directory walker |
| 01.2 | Schema as Python Types (Sole SSOT) | Seven dataclass schemas (incl. RoadmapSectionSchema), closed status enum, `DepId` validator (name-based cross-plan resolution), `--docgen --check` mode, generated docs |
| 01.3 | Shared Finding & Classifier Types | `Finding`, `Severity`, `FindingCategory × FindingSubtype` (two-level; includes `ITEM_VERIFICATION` subtypes for Section 04), `Corpus` (imported by Sections 02-05) |
| 01.4 | Canonical Status Normalizer (facts only) | `normalize_status()` emits plain `Finding(STATUS_CONTRADICTION, …)` records; `classify_safety()` is RELOCATED to Section 03.2 |
| 01.5 | Fixture Corpus & TDD Tests | `tests/plan-audit/fixtures/` + pytest harness; semantic and negative pins |
| 01.6 | Pilot Migration (all seven schema classes) | ≥1 artifact per schema class + this plan's sibling `depends_on` cascaded to logical IDs |
