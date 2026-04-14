---
section: "02"
title: "DAG Builder & Conflict Classifier"
status: not-started
reviewed: false
goal: "Build the dependency DAG across all plans and implement the 6 conflict classifiers that detect cross-plan coherence issues"
success_criteria:
  - "DAG construction parses depends_on fields and cross-plan references from all plan files"
  - "Shared subsystem mapping identifies plans touching the same crates/files"
  - "CONFLICT classifier detects contradictory goals for the same subsystem"
  - "SUPERSEDED classifier detects reroute claims with incomplete rewrites"
  - "BLOCKED classifier detects active plans depending on queued prerequisites"
  - "STATUS_CONTRADICTION classifier detects status/date drift across dependency chains"
  - "MISSING_DEPENDENCY classifier discovers undocumented dependency edges"
  - "DEAD_REFERENCE classifier flags pointers to nonexistent plans/directories"
  - "Known test cases (a), (b), (c), (g), (h) from overview are caught"
inspired_by: []
depends_on:
  - "01"
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "DAG Construction"
    status: not-started
  - id: "02.2"
    title: "Conflict Classifiers"
    status: not-started
  - id: "02.3"
    title: "Priority Inversion Detection"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: DAG Builder & Conflict Classifier

**Status:** Not Started
**Goal:** Build a dependency DAG across ALL plans in the corpus and implement 6 conflict classifiers that detect cross-plan coherence issues. This is the core analysis engine of the redesigned verify-roadmap skill.

**Success Criteria:**
- [ ] DAG construction covers all plan files (indexes + sections)
- [ ] 6 conflict classifiers implemented and tested against known bugs
- [ ] Priority inversion detection catches active-depends-on-queued patterns
- [ ] Satisfies overview test cases (a), (b), (c), (g), (h)

**Context:** The current `/verify-roadmap` command operates on individual roadmap sections in isolation. It has zero awareness of reroute plans, cross-plan dependencies, or shared subsystem ownership. As a result, it misses entire classes of bugs: priority inversions (repr-opt active but its locality prerequisite is queued), dead references (roadmap section 22.2 references nonexistent `plans/ori_lsp/`), and supersession drift (test-suite-health section 02 claims to rewrite roadmap 21A but the rewrite never happened). The DAG builder creates the graph structure that makes these detectable.

**Depends on:** Section 01 (Frontmatter Schema) -- the DAG builder relies on normalized frontmatter to parse `depends_on` fields and `status` values reliably.

---

## 02.1 DAG Construction

**File(s):** `scripts/plan-dag-build.py` (new)

Build a directed acyclic graph where nodes are plan sections and edges represent dependencies. Three edge sources feed the graph.

- [ ] Parse explicit dependencies from `depends_on` frontmatter fields (consumes Section 01 SSOT — no path-based resolution):
  - Section 01's parser has already validated `depends_on` as logical-ID values (`DepId`): intra-plan `"NN"` or cross-plan `"plan-name#NN"`. Full paths are rejected at parse time and never reach Section 02.
  - Resolve every `DepId` through `plan_corpus.resolve_dep(dep_id, source_plan)` which uses `Corpus.name_index` to map `plan-name#NN` → target plan → target section. Intra-plan `"NN"` resolves within the source plan.
  - Unresolvable `DepId` values surface as `DEAD_REFERENCE` findings from `plan_corpus` — Section 02 consumes these facts rather than re-validating.
  - Create a directed edge from the dependent section to its resolved dependency.

- [ ] Parse implicit cross-plan references from section bodies:
  - Scan section body text for references to other `plans/*/` directories
  - Distinguish informational references from dependency references using heuristics:
    - "depends on", "requires", "blocked by", "prerequisite" = dependency edge
    - "see also", "related", "inspired by", "cf." = informational (no edge)
  - Create edges for dependency references; annotate informational references separately

- [ ] Map shared subsystems by scanning which plans touch the same crates/files:
  - Extract crate/file references from section bodies and success criteria
  - Build a subsystem-to-plans mapping (e.g., `ori_arc` -> [repr-opt, locality-representation-unification, ...])
  - Plans sharing a subsystem get a "shared subsystem" annotation (not a dependency edge, but input to CONFLICT classifier)

- [ ] Build the DAG data structure:
  - Nodes: plan sections (identified by `plans/<plan>/section-<NN>-<slug>.md`)
  - Node metadata: status, reviewed, goal, plan-level status
  - Edges: dependency relationships (explicit + implicit)
  - Annotations: shared subsystem overlaps, informational references
  - Output format: JSON serializable for use by classifiers and report generator

- [ ] Detect and report cycles in the dependency graph:
  - A cycle indicates a mutual dependency that cannot be resolved by execution order
  - Report all nodes participating in cycles with the full cycle path

- [ ] **Subsection close-out (02.1)** -- MANDATORY before starting 02.2:
  - [ ] All tasks above are `[x]` and DAG builds successfully on current corpus
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** -- check whether changes
        invalidated any CLAUDE.md, `.claude/rules/*.md`, or `canon.md`
        claims. If no changes, document briefly. Fix any drift NOW.

---

## 02.2 Conflict Classifiers

**File(s):** `scripts/plan-dag-build.py` (classifiers integrated into DAG script)

Implement 6 classifiers that walk the DAG and classify findings. Each classifier produces structured findings with type, severity, source, target, and recommended fix.

- [ ] **CONFLICT** -- contradictory goals for the same subsystem:
  - Walk shared-subsystem annotations from 02.1
  - For each pair of plans sharing a subsystem, compare their goals (from frontmatter `goal` fields)
  - Flag when goals are contradictory (e.g., one plan restructures a module, another refactors it differently)
  - Severity: high (requires human decision to resolve)

- [ ] **SUPERSEDED** -- reroute claims with incomplete rewrites:
  - For each plan with `reroute: true`, check its `supersedes` list
  - For each superseded target, verify that the reroute plan's sections actually cover the superseded work
  - Flag when a reroute plan claims to supersede a roadmap section but the reroute's sections do not address the superseded content
  - Known case: test-suite-health section 02 says it rewrites roadmap 21A, but the rewrite has not happened
  - Severity: medium (stale claim, needs acknowledgment or completion)

- [ ] **BLOCKED** -- active plan depends on queued prerequisite:
  - Walk the DAG edges looking for active-status nodes that depend on queued or not-started nodes
  - A plan section with `status: in-progress` or parent plan `status: active` depending on a section/plan with `status: queued` or `not-started` is a priority inversion
  - Known case: repr-opt is active but its locality prerequisite is queued
  - Severity: high (work cannot proceed correctly without prerequisite)

- [ ] **STATUS_CONTRADICTION** (category; subtypes defined in 01.3 SSOT) -- DAG-level status/date drift across dependency chains:
  - Section 01.4's `normalize_status()` already produces per-file `STATUS_CONTRADICTION` findings (e.g. `FM_DECLARED_VS_BODY_DERIVED`, `PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED`). Section 02 consumes those facts; the DAG classifier's job is to find DRIFT that is only visible across dependency edges.
  - Walk the DAG and check temporal consistency: a dependent section's declared status presupposes a state its prerequisite has not reached — drift visible only across the dependency edge, never emitted by 01.4's intra-file normalizer — emit `STATUS_CONTRADICTION / CROSS_EDGE_TEMPORAL_DRIFT` (subtype owned in 01.3).
  - Check for stale TPR snapshots: a section's `third_party_review.updated` predates mtime on files it depends on — the reviewed snapshot is stale relative to the upstream edits — emit `STATUS_CONTRADICTION / TPR_STALE_VS_EDIT` (subtype owned in 01.3).
  - **NOTE**: the pre-2026-04-14 flat `STALE_METADATA` classifier name is retired — all status-drift findings now flow through the `STATUS_CONTRADICTION` category in Section 01.3's two-level taxonomy. Write-back safety classification (SafeFix vs ExposureReview) happens in Section 03.2, not here.
  - Severity: medium (DRIFT; some subtypes auto-fixable, classified by Section 03.2).

- [ ] **MISSING_DEPENDENCY** -- undocumented dependency edge discovered via shared subsystem:
  - For each pair of plans sharing a subsystem, check whether either has a `depends_on` reference to the other
  - If plans share a subsystem and modify it concurrently (both active/in-progress) without a documented dependency, flag it
  - This catches interference risk: two active plans modifying the same crate without coordination
  - Severity: medium (potential interference, needs explicit ordering or acknowledgment)

- [ ] **DEAD_REFERENCE** -- pointer to nonexistent plan/directory:
  - From the DAG edge validation in 02.1, collect all references that failed to resolve
  - Also scan section bodies for `plans/*/` paths that do not exist on disk
  - Known case: Roadmap section 22.2 references `plans/ori_lsp/` which does not exist
  - Known case: Section 21A references stale "unblocks JIT Exception Handling" to a completed plan
  - Severity: low (stale reference, auto-fixable by removal or update)

- [ ] **Subsection close-out (02.2)** -- MANDATORY before starting 02.3:
  - [ ] All tasks above are `[x]` and all 6 classifiers produce findings on current corpus
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** -- check whether changes
        invalidated any CLAUDE.md, `.claude/rules/*.md`, or `canon.md`
        claims. If no changes, document briefly. Fix any drift NOW.

---

## 02.3 Priority Inversion Detection

**File(s):** `scripts/plan-dag-build.py` (integrated)

Specialized analysis for priority inversions -- the most impactful class of cross-plan bugs. This subsection extends the BLOCKED classifier with transitive analysis and actionable recommendations.

- [ ] Implement transitive priority inversion detection:
  - A depends on B, B depends on C: if A is active and C is queued, report the full chain A -> B -> C
  - Report the minimum set of plans that must be unblocked to resolve the inversion
  - Identify the "root blocker" -- the deepest queued dependency in the chain

- [ ] Implement execution order recommendation:
  - Given the DAG, compute a topological sort that respects all dependency edges
  - Compare the computed order against actual plan statuses (active/queued)
  - Report plans that are active out of order (should be queued until dependencies complete)
  - Report plans that are queued but could be activated (all dependencies complete)

- [ ] Validate against known test cases:
  - Test case (a): repr-opt active, locality prerequisite queued -- BLOCKED finding
  - Test case (g): BUG-04-039 in-progress but blocked by queued plan -- BLOCKED finding
  - Test case (b): Roadmap 22.2 references nonexistent `plans/ori_lsp/` -- DEAD_REFERENCE finding
  - Test case (c): test-suite-health 02 claims to rewrite roadmap 21A -- SUPERSEDED finding
  - Test case (h): Section 21A stale "unblocks JIT Exception Handling" ref -- DEAD_REFERENCE finding

- [ ] **Subsection close-out (02.3)** -- MANDATORY before marking section complete:
  - [ ] All tasks above are `[x]` and known test cases validated
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** -- check whether changes
        invalidated any CLAUDE.md, `.claude/rules/*.md`, or `canon.md`
        claims. If no changes, document briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** -- run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] DAG construction parses explicit and implicit dependencies from all plan files
- [ ] Shared subsystem mapping identifies overlapping crate/file ownership
- [ ] All 6 classifiers implemented: CONFLICT, SUPERSEDED, BLOCKED, STATUS_CONTRADICTION, MISSING_DEPENDENCY, DEAD_REFERENCE
- [ ] Transitive priority inversion detection with root blocker identification
- [ ] Execution order recommendation via topological sort
- [ ] Known test cases (a), (b), (c), (g), (h) validated
- [ ] `timeout 150 ./test-all.sh` green -- no regressions
- [ ] `/tpr-review` -- dual-source review of DAG builder and classifiers
- [ ] `/impl-hygiene-review` -- verify classifier logic is correct, no false negatives on known cases
- [ ] `/improve-tooling` section-close sweep -- verify per-subsection retrospectives ran; add cross-subsection findings
- [ ] `/sync-claude` section-close sweep -- verify CLAUDE.md and rules reflect any new scripts or conventions
