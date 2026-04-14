---
section: "03"
title: "Findings Report & Write-Back"
status: not-started
reviewed: false
goal: "Design the findings report format and implement the write-back mechanism for auto-fixable issues and manual-review flagging"
success_criteria:
  - "Findings report format defined (JSON + markdown) with category, subtype, severity, source, target, recommended fix (reuses 01.3 two-level taxonomy; no shadow fields)"
  - "Auto-fix engine handles safe issues: frontmatter normalization, status reconciliation, dead reference removal"
  - "SafeFix / ExposureReview taxonomy OWNED here (relocated from 01.4); `classify_safety(finding, context)` is the single canonical classifier for both schema violations AND status contradictions"
  - "WriteBackContext carries caller-supplied git signals (has_recent_commits) — `plan_corpus.py` stays pure; git queries happen at the CLI edge"
  - "Manual-review flagging for issues requiring human decision: CONFLICT resolution, SUPERSEDED acknowledgment, ExposureReview-classified findings"
  - "Integration with /continue-roadmap's roadmap-scan.sh surfaces cross-plan conflicts during roadmap work"
  - "Report is human-readable and machine-parseable"
inspired_by: []
depends_on:
  - "01"
  - "02"
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Report Format"
    status: not-started
  - id: "03.2"
    title: "Auto-Fix Engine"
    status: not-started
  - id: "03.3"
    title: "Continue-Roadmap Integration"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Findings Report & Write-Back

**Status:** Not Started
**Goal:** Design the findings report format and implement the write-back mechanism that auto-fixes safe issues and flags issues requiring human decision. Connect the output to `/continue-roadmap` so cross-plan conflicts surface during active roadmap work.

**Success Criteria:**
- [ ] Findings report format defined and implemented (JSON + markdown)
- [ ] Auto-fix engine handles safe issues without human intervention
- [ ] Manual-review issues are flagged with clear context and recommended actions
- [ ] Integration with `/continue-roadmap` surfaces findings during active work

**Context:** Sections 01 and 02 produce raw findings (schema violations, DAG conflicts, priority inversions). This section turns those findings into actionable output: a structured report for review, an auto-fix engine for safe corrections, and integration with the existing `/continue-roadmap` workflow so findings surface at the right time. The distinction between auto-fixable and manual-review issues is critical -- auto-fixing frontmatter field renames is safe; auto-resolving goal conflicts between plans is not.

**Depends on:** Section 02 (DAG Builder) -- the report format depends on the classifier output structure.

---

## 03.1 Report Format

**File(s):** Report generation integrated into the verify-roadmap skill pipeline

Design and implement the findings report format. The report must be both human-readable (markdown) and machine-parseable (JSON) for downstream tool integration.

- [ ] Import the finding data model from `plan_corpus` (01.3 SSOT — do NOT redefine here):
  - `Finding` = `{id, category, subtype, severity, source, source_line, target, target_line, description, recommended_fix, evidence}`
  - `FindingCategory` and `FindingSubtype` enums are imported (see Section 01.3 for the complete taxonomy — seven categories with fine-grained subtypes including Phase 4 `ITEM_VERIFICATION` subtypes)
  - `Finding.to_json()` / `Finding.to_markdown()` are used as-is; Section 03 only wraps them into a report
- [ ] The report-format augmentation: wrap each `Finding` in `ClassifiedFinding(finding, safety_class, rationale)` at write-back time (see 03.2). The report serializes `ClassifiedFinding` records, not raw `Finding` — auto-fix readiness is a write-back annotation, not a parser-library fact.

- [ ] Implement JSON report output:
  - Array of finding objects matching the data model above
  - Written to `build/verify-roadmap/findings.json` (build directory, not committed)
  - Include metadata header: timestamp, corpus size, classifier versions

- [ ] Implement markdown report output:
  - Grouped by severity (critical first, then high, medium, low)
  - Within each severity, grouped by classifier type
  - Each finding shows: type badge, source -> target, description, recommended fix
  - Summary table at top: count by type and severity
  - Written to `build/verify-roadmap/findings.md` (build directory, not committed)

- [ ] Implement console summary output:
  - One-line-per-finding format for terminal display
  - Color-coded by severity (if terminal supports it)
  - Exit code reflects findings: 0 = clean, 1 = findings present, 2 = critical findings

- [ ] **Subsection close-out (03.1)** -- MANDATORY before starting 03.2:
  - [ ] All tasks above are `[x]` and report generates correctly on current corpus
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** -- check whether changes
        invalidated any CLAUDE.md, `.claude/rules/*.md`, or `canon.md`
        claims. If no changes, document briefly. Fix any drift NOW.

---

## 03.2 Auto-Fix Engine

**File(s):** Auto-fix logic integrated into verification pipeline (this section OWNS the `SafeFix` / `ExposureReview` taxonomy and `classify_safety`; they were relocated here from Section 01.4 on 2026-04-14 per CODEX-01-003 / GEMINI-01-004 / GEMINI-01-005 — write-back policy belongs at the write-back phase boundary)

Implement automatic fixes for findings that are safe to resolve without human review. Safety criterion: a fix is auto-fixable if it cannot change plan semantics -- only metadata normalization.

- [ ] Define `SafetyClass(Enum)`: `SafeFix | ExposureReview` — the auto-fix gating tag. `SafeFix` findings are applied automatically (with backup + log); `ExposureReview` findings are surfaced for human review (never auto-applied).

- [ ] Define `ClassifiedFinding` dataclass: `{finding: Finding, safety_class: SafetyClass, rationale: str}` — wraps a plain `Finding` (imported from `plan_corpus`; NO `safety_class` on the `Finding` itself per 01.3). Section 03 produces `ClassifiedFinding` records; 01.x never does.

- [ ] Define `WriteBackContext` dataclass: carries caller-supplied signals needed by the classifier — specifically `has_recent_commits: dict[Path, bool]` mapping plan directories → git signal. The CLI front-end populates this by running `git log --since=14d -- plans/<name>/` at the edge; `plan_corpus.py` stays pure.

- [ ] Implement `classify_safety(finding: Finding, context: WriteBackContext) -> ClassifiedFinding`:
  - Dispatches on `finding.category` + `finding.subtype`:
    - `SCHEMA_VIOLATION` subtypes (SafeFix): field rename `plan:` → `name:` (CODEX finding 8 flagging: only rename if value preservation is byte-safe); removing `reroute: false`; adding missing `reviewed: false` default; adding `third_party_review: {status: none, updated: null}` where missing
    - `SCHEMA_VIOLATION` subtypes (ExposureReview): `MISSING_REQUIRED_FIELD` when the missing field needs semantic inference from body content (e.g. missing frontmatter entirely — previously "generate canonical frontmatter from file content analysis", flagged ExposureReview per CODEX finding 8)
    - `STATUS_CONTRADICTION/PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED` — SafeFix IFF `context.has_recent_commits[plan_dir] == False` (no activity supports status=queued); else ExposureReview (recent commits suggest the plan IS actively being worked on but sections are stale — needs human)
    - `STATUS_CONTRADICTION/FM_DECLARED_VS_BODY_DERIVED` — ExposureReview (ambiguous intent; body markers vs frontmatter disagree — needs human validation)
    - All other `STATUS_CONTRADICTION` subtypes — ExposureReview by default (conservative)
    - `DEAD_REFERENCE` subtypes (SafeFix): `PLAN_DIRECTORY_NOT_FOUND` / `SECTION_FILE_NOT_FOUND` / `CROSS_PLAN_NAME_NOT_FOUND` / `SPEC_FILE_NOT_FOUND` when the dead-reference target is unambiguously gone (e.g. a `depends_on` entry pointing at `plans/ori_lsp/` that does not exist on disk — stripping the entry cannot change plan semantics because there is no target to depend on); removal is mechanical (frontmatter list entry only; prose body references are always ExposureReview per the `DEAD_REFERENCE` body-removal guard below)
    - `DEAD_REFERENCE` subtypes (ExposureReview): target is ambiguous — e.g. the dead reference appears in prose body text where a human-authored replacement may be needed; OR the target name is close enough to an existing plan that automatic stripping would likely be wrong (did-you-mean hint is non-empty)
    - All other categories (`PARSE_ERROR`, `DAG_CONFLICT`, `ITEM_VERIFICATION`, `GAP`) — ExposureReview by default (conservative; never auto-applied). Auto-fix is explicitly opt-in per category/subtype; absence of an explicit SafeFix rule means ExposureReview, and the default branch MUST record the rationale "no SafeFix rule declared for <category>/<subtype>"
  - Each `ClassifiedFinding` carries a `rationale` string explaining why it got its class
  - Pure function of `(finding, context)` — no I/O inside `classify_safety` itself; all git queries happen in the caller that builds `WriteBackContext`

- [ ] Implement auto-fix for SCHEMA_VIOLATION findings:
  - Field renames: `plan:` -> `name:` (preserving value; SafeFix only when value is byte-safe)
  - Field removal: `reroute: false` -> remove field (default-equivalent)
  - Default field insertion: add `reviewed: false`, `third_party_review: {status: none, updated: null}` where missing
  - Missing frontmatter: surfaced as ExposureReview (NEVER auto-applied) — reconstructing canonical frontmatter from body content is semantic inference, not normalization
  - **NOTE**: `parallel: true` is a VALID canonical `PlanIndexSchema` field (see 01.2 schema; 01.6 pilot covers `plans/pkg_mgmt/index.md` which uses it). Auto-fix MUST NOT remove `parallel: true` — it is permanent-plan metadata.

- [ ] Implement auto-fix for STATUS_CONTRADICTION findings (the `STALE_METADATA` classifier name retired on 2026-04-14; all status-drift findings now carry `FindingCategory.STATUS_CONTRADICTION` with subtypes owned in 01.3):
  - Frontmatter/body status reconciliation: when body clearly indicates completion (all checkboxes checked, `COMPLETE` marker), update frontmatter `status` to `complete`
  - When frontmatter says `complete` but body has unchecked items, flag as manual-review (ambiguous intent)
  - Active-but-empty reconciliation: when plan is `active` but all sections are `not-started`, change plan to `queued`

- [ ] Implement auto-fix for DEAD_REFERENCE findings (low severity only):
  - Remove references to nonexistent `plans/*/` directories from `depends_on` fields
  - Add a `<!-- Removed dead reference to plans/X/ (VR-NNN) -->` comment for audit trail
  - Do NOT auto-remove references from prose body text (might need human-authored replacement)

- [ ] Implement safe-fix guards:
  - All auto-fixes create a backup of the original file in `build/verify-roadmap/backups/`
  - All auto-fixes are logged to `build/verify-roadmap/fixes-applied.json`
  - `--dry-run` flag shows what would be fixed without modifying files
  - `--no-auto-fix` flag disables auto-fixing entirely (report-only mode)

- [ ] Define manual-review flagging for non-auto-fixable findings:
  - CONFLICT findings: always manual -- requires human decision on which plan's goals take precedence
  - SUPERSEDED findings: always manual -- requires acknowledgment that a reroute claim is stale or completion of the reroute
  - BLOCKED findings: always manual -- requires plan reordering or dependency acknowledgment
  - MISSING_DEPENDENCY findings: always manual -- requires explicit dependency declaration or acknowledgment of independence

- [ ] **Subsection close-out (03.2)** -- MANDATORY before starting 03.3:
  - [ ] All tasks above are `[x]` and auto-fix engine tested on known cases
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** -- check whether changes
        invalidated any CLAUDE.md, `.claude/rules/*.md`, or `canon.md`
        claims. If no changes, document briefly. Fix any drift NOW.

---

## 03.3 Continue-Roadmap Integration

**File(s):** `.claude/skills/verify-roadmap/SKILL.md`, integration with roadmap-scan.sh

Integrate the findings report with `/continue-roadmap` so cross-plan conflicts surface during active roadmap work, not only during explicit `/verify-roadmap` runs.

- [ ] Add a lightweight cross-plan check to roadmap-scan.sh:
  - Before `/continue-roadmap` selects the next section to work on, run a fast subset of the DAG analysis
  - Check whether the selected section has BLOCKED or CONFLICT findings
  - If findings exist, display them before proceeding and let the user decide whether to continue or switch to resolving the finding

- [ ] Design the integration interface:
  - The verify-roadmap skill exposes a `--quick` mode that runs only BLOCKED and DEAD_REFERENCE checks (fast, no shared-subsystem analysis)
  - The full mode (`--full`) runs all 6 classifiers plus auto-fix
  - `/continue-roadmap` calls `--quick` mode as a pre-check; users invoke `--full` explicitly

- [ ] Document the integration in SKILL.md:
  - How `/continue-roadmap` uses the quick check
  - When to run `/verify-roadmap --full` manually (after plan changes, before major milestones)
  - How to interpret and act on findings

- [ ] **Subsection close-out (03.3)** -- MANDATORY before marking section complete:
  - [ ] All tasks above are `[x]` and integration tested
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` on THIS subsection** -- check whether changes
        invalidated any CLAUDE.md, `.claude/rules/*.md`, or `canon.md`
        claims. If no changes, document briefly. Fix any drift NOW.
  - [ ] **Repo hygiene check** -- run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] Findings report format defined and implemented (JSON + markdown + console) — imports `Finding` / `FindingCategory` / `FindingSubtype` from `plan_corpus`, no shadow types
- [ ] `SafetyClass` enum (`SafeFix | ExposureReview`), `ClassifiedFinding` wrapper, and `classify_safety(finding, context)` all OWNED here (relocated from 01.4); `WriteBackContext` carries git signals
- [ ] Auto-fix engine handles safe issues: field renames, status reconciliation, dead reference removal; applies only `SafeFix` findings; logs `ExposureReview` without mutation
- [ ] Manual-review flagging for CONFLICT, SUPERSEDED, BLOCKED, MISSING_DEPENDENCY (intrinsically manual) + any ExposureReview-classified finding
- [ ] Safe-fix guards: backups, logging, `--dry-run`, `--no-auto-fix`
- [ ] `classify_safety` is pure (no I/O); git signal population lives at the CLI edge; `plan_corpus.py` is grep-verified to contain no `subprocess` or `git` calls
- [ ] Integration with `/continue-roadmap` via `--quick` mode pre-check
- [ ] `timeout 150 ./test-all.sh` green -- no regressions
- [ ] `/tpr-review` -- dual-source review of report format and auto-fix logic
- [ ] `/impl-hygiene-review` -- verify auto-fix safety (no semantic changes), report completeness
- [ ] `/improve-tooling` section-close sweep -- verify per-subsection retrospectives ran; add cross-subsection findings
- [ ] `/sync-claude` section-close sweep -- verify CLAUDE.md and rules reflect new verify-roadmap modes and integration points
