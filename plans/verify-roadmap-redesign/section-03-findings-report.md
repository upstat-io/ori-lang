---
section: "03"
title: "Findings Report & Write-Back"
status: in-progress
reviewed: true
goal: "Design the findings report format and implement the write-back mechanism for auto-fixable issues and manual-review flagging"
success_criteria:
  - "Findings report format defined (JSON + markdown) with category, subtype, severity, source, target, recommended fix (reuses 01.3 two-level taxonomy; no shadow fields)"
  - "Auto-fix engine handles safe issues: frontmatter normalization, status reconciliation, dead reference removal"
  - "SafeFix / ExposureReview taxonomy OWNED here (relocated from 01.4); `classify_safety(finding, context)` is the single canonical classifier for both schema violations AND status contradictions"
  - "WriteBackContext carries caller-supplied git signals (has_recent_commits) — `plan_corpus.py` stays pure; git queries happen at the CLI edge"
  - "Manual-review flagging for issues requiring human decision: CONFLICT resolution, SUPERSEDED acknowledgment, ExposureReview-classified findings"
  - "All frontmatter writes use targeted text patching (regex on the frontmatter slice), NEVER PyYAML dump/reload — PyYAML destroys comments, key ordering, and inline formatting"
  - "Concurrent-session safety: preimage hash on read, hash-compare before write, atomic temp-file + os.replace"
  - "Integration with /continue-roadmap's roadmap-scan.sh surfaces cross-plan conflicts during roadmap work"
  - "Report is human-readable and machine-parseable"
inspired_by: []
depends_on:
  - "01"
  - "02"
third_party_review:
  status: resolved
  updated: 2026-04-14
sections:
  - id: "03.1"
    title: "Safety Taxonomy & Data Types"
    status: complete
  - id: "03.2"
    title: "Report Format"
    status: complete
  - id: "03.3"
    title: "Auto-Fix Engine"
    status: complete
  - id: "03.4"
    title: "Frontmatter Text Patcher"
    status: complete
  - id: "03.5"
    title: "Continue-Roadmap Integration"
    status: complete
  - id: "03.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Findings Report & Write-Back

**Status:** Not Started
**Goal:** Design the findings report format and implement the write-back mechanism that auto-fixes safe issues and flags issues requiring human decision. Connect the output to `/continue-roadmap` so cross-plan conflicts surface during active roadmap work.

**Success Criteria:**
- [ ] Safety taxonomy (`SafetyClass`, `ClassifiedFinding`, `WriteBackContext`) defined and tested
- [ ] Findings report format defined and implemented (JSON + markdown + console)
- [ ] Frontmatter text patcher operates on raw text (regex), never PyYAML dump/reload
- [ ] Auto-fix engine handles safe issues without human intervention, with concurrent-session guards
- [ ] Manual-review issues are flagged with clear context and recommended actions
- [ ] Integration with `/continue-roadmap` surfaces findings during active work

**Context:** Sections 01 and 02 produce raw findings (schema violations, DAG conflicts, priority inversions). This section turns those findings into actionable output: a structured report for review, an auto-fix engine for safe corrections, and integration with the existing `/continue-roadmap` workflow so findings surface at the right time. The distinction between auto-fixable and manual-review issues is critical -- auto-fixing frontmatter field renames is safe; auto-resolving goal conflicts between plans is not.

**Depends on:** Section 02 (DAG Builder) -- the report format depends on the classifier output structure.

**Architectural decisions:**

1. **PyYAML is read-only.** PyYAML `safe_load` is used to PARSE frontmatter. It is NEVER used to WRITE frontmatter back. PyYAML `dump` destroys YAML comments (which are DAG signal per Section 02's HTML_COMMENT_CONVENTION and YAML_COMMENT source kinds), reorders keys, strips trailing whitespace, normalizes quoting style, and flattens multi-line strings. All frontmatter writes go through the targeted text patcher (03.4) which operates on the raw text slice between the `---` fences using line-level regex replacements. This is the ONLY safe write path. See also: `roadmap_scan.py:344` (`yaml.safe_load` for read) — the same constraint applies there.

2. **Safety taxonomy lives here, not in plan_corpus.** `SafetyClass`, `ClassifiedFinding`, and `classify_safety` are write-back policy. The `plan_corpus` library produces factual `Finding` records (no policy); this section consumes those findings and classifies them for write-back. `plan_corpus` must never import from this module.

3. **Concurrent-session safety is mandatory.** The user runs parallel agent sessions with uncommitted work (see MEMORY.md `feedback_never_destructive_git.md`). Any read-modify-write on plan files must: (a) record a preimage hash at scan time, (b) re-read and hash-compare before write, (c) write to a temp file and `os.replace` atomically. If the file changed between scan and write, refuse to apply and log the conflict.

---

## 03.1 Safety Taxonomy & Data Types

**File(s):** New module in the verify-roadmap skill (e.g. `scripts/verify_roadmap/safety.py` or inline in the skill's write-back logic)

Define the safety taxonomy data types that the report format (03.2) and auto-fix engine (03.3) both consume. This subsection exists to break the circular dependency identified by tp-help: the report format needs `ClassifiedFinding` to serialize, and the auto-fix engine needs `SafetyClass` to gate writes — both need the types before either can be implemented.

- [x] Define `SafetyClass(Enum)`: `SafeFix | ExposureReview` -- the auto-fix gating tag:
  - `SafeFix` findings are applied automatically (with backup + log)
  - `ExposureReview` findings are surfaced for human review (never auto-applied)

- [x] Define `ClassifiedFinding` dataclass:
  - Fields: `finding: Finding`, `safety_class: SafetyClass`, `rationale: str`
  - Wraps a plain `Finding` (imported from `plan_corpus`; NO `safety_class` on the `Finding` itself per 01.3)
  - Section 03 produces `ClassifiedFinding` records; Sections 01/02 never do

- [x] Define `WriteBackContext` dataclass:
  - Field: `has_recent_commits: dict[Path, bool]` -- maps plan directories to git activity signal
  - The CLI front-end populates this by running `git log --since=14d -- plans/<name>/` at the edge
  - `plan_corpus` stays pure -- grep-verify it contains no `subprocess` or `git` calls
  - **`--quick` mode optimization (blind spot #10):** `WriteBackContext` construction requires O(N) `git log` subprocess calls per plan. `--quick` mode runs only read-only DAG checks (BLOCKED, DEAD_REFERENCE) which do not need git signals. `--quick` MUST bypass `WriteBackContext` population entirely by passing `context=None` to the report generator. `classify_safety` in `--quick` mode skips classification and marks all findings as ExposureReview (report-only, no auto-fix). This is a correctness optimization, not just performance -- `--quick` is a pre-check, not a write-back trigger.

- [x] Define `PreimageRecord` dataclass (concurrent-session guard):
  - Fields: `path: Path`, `content_hash: str`, `scan_timestamp: float`
  - `content_hash` is `hashlib.sha256(path.read_bytes()).hexdigest()`
  - Captured at scan time for every file that might be modified
  - Used by the text patcher (03.4) to detect concurrent modifications before write

- [x] Implement `classify_safety(finding: Finding, context: WriteBackContext | None, frontmatter_data: dict | None = None) -> ClassifiedFinding`:
  - **Signature note (TPR-03-001-gemini):** the `frontmatter_data` parameter carries the parsed frontmatter dict for the finding's source file. This allows `classify_safety` to inspect sibling fields (e.g., checking whether both `plan:` and `name:` exist for the collision guard) WITHOUT performing I/O — the dict is pre-parsed by `plan_corpus.parser` at scan time. The function remains pure: `(finding, context, dict) -> ClassifiedFinding`.
  - **When `context is None` (--quick mode):** return `ClassifiedFinding(finding, ExposureReview, "quick mode — no write-back classification")`
  - **When `context` is provided:** dispatch on `finding.category` + `finding.subtype`:

  **SCHEMA_VIOLATION subtypes — SafeFix:**
  - Field rename `plan:` -> `name:` — **NOTE: `OverviewSchema` canonically uses `plan:` (see `schemas.py:89-91`); `PlanIndexSchema` canonically uses `name:` (see `schemas.py:39`). Renaming `plan:` to `name:` is ONLY valid on files where the schema expects `name:` but the file has `plan:` instead (i.e., the file is a `PlanIndexSchema` file misusing `plan:`).** SafeFix ONLY when:
    - The target file's schema class is `PlanIndexSchema` (the schema that requires `name:`) AND the file has `plan:` instead of `name:`
    - **NOT valid on `OverviewSchema` files** — those canonically use `plan:` as a required field; renaming it to `name:` would violate the schema
    - **Collision guard (blind spot #3):** if the file already has BOTH a `plan:` key AND a `name:` key with DIFFERENT values, this is ExposureReview (human must decide which value to keep). Check uses `frontmatter_data` parameter: `"plan" in frontmatter_data and "name" in frontmatter_data and frontmatter_data["plan"] != frontmatter_data["name"]` — no I/O needed, dict is pre-parsed
    - If `plan:` exists and `name:` does not (on a PlanIndexSchema file), SafeFix: rename key preserving value byte-for-byte
    - If both exist with identical values, SafeFix: remove the `plan:` key (redundant)
    - **Paired-finding deduplication (TPR-03-002-gemini):** When `plan:` is used instead of `name:`, `plan_corpus.schema` emits TWO findings: `UNKNOWN_FIELD: plan` AND `MISSING_REQUIRED_FIELD: name`. The rename SafeFix resolves BOTH. The auto-fix dispatcher (03.3) must deduplicate these: when a `plan:→name:` rename is applied, mark the paired `MISSING_REQUIRED_FIELD: name` finding as resolved-by-sibling (do NOT surface it as a separate ExposureReview). Add a `resolved_by_sibling: Finding.id | None` field to `ClassifiedFinding` for this case.
  - Removing `reroute: false` — SafeFix (default-equivalent value)
  - Adding missing `reviewed: false` default — SafeFix ONLY for `PlanSectionSchema` and `RoadmapSectionSchema` where `reviewed: bool` is a REQUIRED field with no default (see `schemas.py:62,76`). **Workflow behavior guard (blind spot #7):** for `PlanIndexSchema` where `reviewed: bool | None = None` is OPTIONAL, auto-inserting `reviewed: false` is ExposureReview because it triggers the `/continue-roadmap` Step 1.7 unreviewed-plan gate (see `SKILL.md:205-218`). The absence of the field means "no review state" (None), which does NOT trigger the gate; `false` actively triggers it. This is a semantic change, not normalization.
  - Adding missing `third_party_review: {status: none, updated: null}` — SafeFix where the field is required by schema (`PlanSectionSchema`, `FixBugSchema`)

  **SCHEMA_VIOLATION subtypes — ExposureReview:**
  - `MISSING_REQUIRED_FIELD` when the missing field needs semantic inference from body content (e.g. missing frontmatter entirely — reconstructing canonical frontmatter from body content is semantic inference, not normalization)

  **STATUS_CONTRADICTION subtypes:**
  - `PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED` — SafeFix IFF `context.has_recent_commits[plan_dir] == False` (no activity supports status=queued); else ExposureReview (recent commits suggest the plan IS actively being worked on but sections are stale — needs human)
  - `FM_DECLARED_VS_BODY_DERIVED` — **ALWAYS ExposureReview** (blind spot #4). The normalizer (`normalizer.py:155-159`) returns `derived="complete"` when `has_complete_marker is True` even when `unchecked > 0` (aspirational COMPLETE marker with remaining work). Auto-fixing status to `complete` based on this derivation is WRONG — it would mark plans as complete when they have unchecked checkboxes. The normalizer intentionally returns "complete" to trigger the `FM_DECLARED_VS_BODY_DERIVED` finding; the finding itself is the signal that human review is needed, not that auto-fix should proceed. The auto-fix engine MUST NOT override the ExposureReview classification for this subtype.
  - `PLAN_COMPLETE_WITH_OPEN_SECTIONS` — ExposureReview (semantic decision: complete open sections or downgrade plan status)
  - All other `STATUS_CONTRADICTION` subtypes — ExposureReview by default (conservative)

  **DEAD_REFERENCE subtypes — SafeFix (frontmatter only):**
  - `PLAN_DIRECTORY_NOT_FOUND` / `SECTION_FILE_NOT_FOUND` / `CROSS_PLAN_NAME_NOT_FOUND` when the dead reference is in a `depends_on` frontmatter list entry (mechanical removal from a YAML list). Prose body references are ALWAYS ExposureReview (human-authored replacement may be needed)
  - `SPEC_FILE_NOT_FOUND` — ExposureReview (NOT SafeFix). The `spec:` field lives on `RoadmapSectionSchema` (`schemas.py:81`) and references spec file paths. A dead spec reference may indicate a spec file was renamed or reorganized — the correct target needs human determination. Unlike `depends_on` entries where removal is mechanical, a missing spec file may need a replacement path, not deletion.
  - **Audit trail guard (blind spot #8):** dead-reference removal audit trail goes to `build/verify-roadmap/fixes-applied.json`, NOT as inline HTML comments. An inline `<!-- Removed dead reference to plans/X/ -->` comment would be re-scanned by Section 02's HTML_COMMENT_CONVENTION parser and produce false positive MISSING_DEPENDENCY findings in future runs. The `fixes-applied.json` log is the audit trail.

  **All other categories:**
  - `PARSE_ERROR`, `DAG_CONFLICT`, `ITEM_VERIFICATION`, `GAP` — ExposureReview by default (conservative; never auto-applied). The default branch MUST record the rationale `"no SafeFix rule declared for <category>/<subtype>"`

  - Each `ClassifiedFinding` carries a `rationale` string explaining why it got its class
  - Pure function of `(finding, context)` — no I/O inside `classify_safety` itself

- [x] **Tests (TDD — write before implementation):**
  - **Matrix:** every `(FindingCategory, FindingSubtype)` pair in `types.py:_CATEGORY_SUBTYPES` must have a test case asserting its safety classification
  - **Semantic pins:** `FM_DECLARED_VS_BODY_DERIVED` -> ExposureReview (pin: revert to SafeFix -> test fails)
  - **Semantic pins:** `PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED` with `has_recent_commits=True` -> ExposureReview
  - **Semantic pins:** `PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED` with `has_recent_commits=False` -> SafeFix
  - **Negative pins:** `classify_safety` with `context=None` MUST return ExposureReview for every finding
  - **Collision guard pin:** `plan:` -> `name:` rename when both keys exist with different values -> ExposureReview
  - **Collision guard pin:** `plan:` -> `name:` rename when both keys exist with same values -> SafeFix (remove `plan:`)
  - **Workflow behavior pin:** `reviewed: false` insertion on PlanIndexSchema -> ExposureReview
  - **Workflow behavior pin:** `reviewed: false` insertion on PlanSectionSchema -> SafeFix

- [x] **Subsection close-out (03.1)** -- MANDATORY before starting 03.2:
  - [x] All tasks above are `[x]` and types + classify_safety tested
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [x] **Run `/sync-claude` on THIS subsection** -- check whether changes
        invalidated any CLAUDE.md, `.claude/rules/*.md`, or `canon.md`
        claims. If no changes, document briefly. Fix any drift NOW.

---

## 03.2 Report Format

**File(s):** Report generation integrated into the verify-roadmap skill pipeline

Design and implement the findings report format. The report must be both human-readable (markdown) and machine-parseable (JSON) for downstream tool integration. This subsection CONSUMES the types defined in 03.1.

- [x] Import the finding data model from `plan_corpus` (01.3 SSOT — do NOT redefine here):
  - `Finding` = `{id, category, subtype, severity, source, source_line, source_column, target, target_line, description, recommended_fix, evidence, dependency_chain, source_kind}`
  - `FindingCategory` and `FindingSubtype` enums are imported (see Section 01.3 for the complete taxonomy)
  - `Finding.to_json()` / `Finding.to_markdown()` are used as-is; Section 03 only wraps them into a report

- [x] Import `ClassifiedFinding` and `SafetyClass` from 03.1 (local to this section's module; NOT from `plan_corpus`). The report serializes `ClassifiedFinding` records — each entry includes the finding data PLUS the safety classification, rationale, and sibling resolution state.

- [x] Implement JSON report output:
  - Array of `ClassifiedFinding` objects: each has `finding` (the `Finding.to_json()` dict), `safety_class` (`"safe_fix"` or `"exposure_review"`), `rationale` (string), `resolved_by_sibling` (Finding.id string or null — non-null when this finding was resolved as a side-effect of fixing a paired finding, e.g., `MISSING_REQUIRED_FIELD: name` resolved by the `plan:→name:` rename)
  - Written to `build/verify-roadmap/findings.json` (build directory, not committed)
  - Include metadata header: timestamp, corpus size, classifier versions, mode (`--full` / `--quick`)
  - When mode is `--quick`, omit `safety_class` and `rationale` fields (classification was not performed)

- [x] Implement markdown report output:
  - Grouped by severity (critical first, then high, medium, low)
  - Within each severity, grouped by safety class (ExposureReview first, then SafeFix)
  - Within each group, sorted by classifier type
  - Each finding shows: type badge, source -> target, description, recommended fix, safety classification
  - Summary table at top: count by type and severity, count by safety class
  - Written to `build/verify-roadmap/findings.md` (build directory, not committed)

- [x] Implement console summary output:
  - One-line-per-finding format for terminal display
  - Color-coded by severity (if terminal supports it)
  - SafeFix findings marked with `[auto]` prefix; ExposureReview with `[review]`; unapplied fixes marked with `[UNAPPLIED]` (concurrent-modification refusal from PatchResult(applied=False))
  - Exit code reflects findings: 0 = clean, 1 = findings present, 2 = critical findings

- [x] **Unapplied-fix report surface (TPR-03-003-codex / TPR-03-002-gemini):** The report format must surface `PatchResult(applied=False)` results from the auto-fix engine as a distinct group in both JSON and markdown output. In JSON: add an `unapplied_fixes` array alongside the main findings array. In markdown: add an "Unapplied Fixes" section after the main findings grouped by reason (concurrent modification, malformed file, etc.). These are NOT dropped — they represent intended work that could not safely complete.

- [x] **Tests (TDD):**
  - Round-trip test: `ClassifiedFinding` -> JSON -> parse -> verify all fields preserved
  - Markdown grouping test: verify severity ordering, safety class ordering
  - Exit code test: 0 for empty findings, 1 for low/medium, 2 for critical
  - `--quick` mode test: verify JSON output omits safety_class/rationale
  - **Unapplied-fix surface test:** verify that `PatchResult(applied=False)` entries appear in the `unapplied_fixes` group in both JSON and markdown reports (not silently dropped)

- [x] **Subsection close-out (03.2)** -- MANDATORY before starting 03.3:
  - [x] All tasks above are `[x]` and report generates correctly on current corpus
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [x] **Run `/sync-claude` on THIS subsection** -- check whether changes
        invalidated any CLAUDE.md, `.claude/rules/*.md`, or `canon.md`
        claims. If no changes, document briefly. Fix any drift NOW.

---

## 03.3 Auto-Fix Engine

**File(s):** Auto-fix logic integrated into verification pipeline

Implement automatic fixes for findings classified as `SafeFix` by 03.1's `classify_safety`. Safety criterion: a fix is auto-fixable if it cannot change plan semantics -- only metadata normalization.

- [x] Implement auto-fix dispatcher:
  - Input: list of `ClassifiedFinding` records
  - Filter to `safety_class == SafeFix` only
  - For each SafeFix finding, dispatch to the appropriate fix handler based on `finding.category` + `finding.subtype`
  - All fixes go through the text patcher (03.4) -- the auto-fix engine NEVER writes files directly

- [x] Implement auto-fix for SCHEMA_VIOLATION SafeFix findings:
  - Field rename `plan:` -> `name:` (via text patcher: regex replace `^plan:` with `name:` in frontmatter slice; preserving value byte-for-byte)
  - Field removal: `reroute: false` -> remove entire line from frontmatter slice
  - Default field insertion: add `reviewed: false` via text patcher (insert line in frontmatter slice) — only for PlanSectionSchema/RoadmapSectionSchema files (see 03.1 workflow behavior guard)
  - Default field insertion: add `third_party_review:` block — only for schemas where required

- [x] Implement auto-fix for STATUS_CONTRADICTION SafeFix findings:
  - `PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED` (when classified SafeFix by 03.1): change `status: active` to `status: queued` in frontmatter via text patcher
  - **NOTE: `FM_DECLARED_VS_BODY_DERIVED` is NEVER SafeFix** (see 03.1). The auto-fix engine MUST assert that no `FM_DECLARED_VS_BODY_DERIVED` finding reaches the SafeFix dispatch — this is a defense-in-depth invariant. If it fires, the classifier has a bug.
  - **`parallel: true` guard (from Section 01.2):** `parallel: true` is a VALID canonical `PlanIndexSchema` field. Auto-fix MUST NOT remove it. Verify no fix handler touches fields outside its explicit scope.

- [x] Implement auto-fix for DEAD_REFERENCE SafeFix findings:
  - Remove dead `depends_on` entries from frontmatter list via text patcher
  - **Audit trail in `fixes-applied.json` only (blind spot #8):** do NOT add inline HTML comments like `<!-- Removed dead reference to plans/X/ -->`. Section 02's HTML_COMMENT_CONVENTION parser scans for `blocked-by`, `unblocks`, `supersedes`, `resolves` patterns in HTML comments. While a "Removed dead reference" comment does not match those verbs today, any future verb expansion or fuzzy matching would produce false positive MISSING_DEPENDENCY findings. The `fixes-applied.json` log is the permanent audit trail.
  - Do NOT auto-remove references from prose body text (might need human-authored replacement)

- [x] Implement safe-fix guards:
  - All auto-fixes create a backup of the original file in `build/verify-roadmap/backups/`
  - All auto-fixes are logged to `build/verify-roadmap/fixes-applied.json` with: finding ID, file path, fix type, before/after snippet, timestamp
  - `--dry-run` flag shows what would be fixed without modifying files
  - `--no-auto-fix` flag disables auto-fixing entirely (report-only mode)
  - **Defense-in-depth:** auto-fix engine MUST reject any finding that is not `SafeFix` -- this is a hard assert, not a silent skip. If an `ExposureReview` finding leaks into the auto-fix path, it is a classifier bug and must fail loudly.
  - **Concurrent-modification propagation (TPR-03-003-codex / TPR-03-002-gemini):** when `apply_patch` returns `PatchResult(applied=False)` (preimage hash mismatch from concurrent session), the auto-fix dispatcher MUST convert the original `SafeFix` finding into an `ExposureReview` finding with the failure reason appended to the rationale (e.g., `"SafeFix reverted to ExposureReview: file modified by concurrent session"`) and append it to the final report as an unapplied fix. The report format (03.2) must surface these as a distinct "unapplied fixes" group — they represent work the tool intended to do but could not safely complete. They MUST NOT be silently dropped.

- [x] Define manual-review flagging for non-auto-fixable findings:
  - CONFLICT findings: always manual -- requires human decision on which plan's goals take precedence
  - SUPERSEDED findings: always manual -- requires acknowledgment that a reroute claim is stale or completion of the reroute. **§02 handoff note (TPR-03-005-codex):** Section 02 defines a git-aware SUPERSEDED specialization with two structural cases (`section-02-dag-builder.md:251-252`). `classify_safety` deliberately routes ALL SUPERSEDED findings to ExposureReview (never SafeFix) because SUPERSEDED resolution is inherently semantic — the user must decide whether the reroute claim is valid, stale, or in progress. `WriteBackContext.has_recent_commits` is available for future SafeFix graduation if a narrow, safe subcase is identified (e.g., "SUPERSEDED by a plan with `status: resolved`"), but no such subcase is implemented in this section. This is an explicit design decision, not an omission.
  - BLOCKED findings: always manual -- requires plan reordering or dependency acknowledgment
  - MISSING_DEPENDENCY findings: always manual -- requires explicit dependency declaration or acknowledgment of independence
  - All ExposureReview-classified findings: surfaced in the report with context and recommended actions

- [x] **Tests (TDD):**
  - **Semantic pin:** `FM_DECLARED_VS_BODY_DERIVED` reaching auto-fix dispatcher -> assert/panic (defense-in-depth)
  - **Semantic pin:** `parallel: true` field untouched by any fix handler
  - **Matrix:** each SafeFix subtype has a test case verifying the correct text transformation
  - **Negative pin:** ExposureReview finding passed to auto-fix dispatcher -> rejected
  - **Backup test:** verify backup file created before modification
  - **Dry-run test:** verify no file modifications in dry-run mode
  - **Idempotency test:** running auto-fix twice on the same corpus produces identical results

- [x] **Subsection close-out (03.3)** -- MANDATORY before starting 03.4:
  - [x] All tasks above are `[x]` and auto-fix engine tested on known cases
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [x] **Run `/sync-claude` on THIS subsection** -- check whether changes
        invalidated any CLAUDE.md, `.claude/rules/*.md`, or `canon.md`
        claims. If no changes, document briefly. Fix any drift NOW.

---

## 03.4 Frontmatter Text Patcher

**File(s):** New module for targeted text-level frontmatter manipulation

This subsection implements the ONLY write path for frontmatter modifications. PyYAML is read-only; all writes go through targeted text patching on the raw frontmatter slice. This subsection also implements the concurrent-session safety guards.

**Rationale (blind spot #1):** PyYAML `safe_load` parses YAML into Python dicts (losing comments, key order, quoting style, trailing whitespace). If we were to modify the dict and `yaml.dump` it back, every comment in the frontmatter — including YAML comments that are DAG signal for Section 02's `YAML_COMMENT` source kind — would be destroyed. Additionally, key ordering changes produce noisy git diffs. The text patcher operates on the raw text between the `---` fences, using line-level regex replacements that preserve everything the fix does not explicitly target.

- [x] Implement `extract_frontmatter_slice(text: str) -> tuple[str, int, int]`:
  - Returns `(frontmatter_text, start_offset, end_offset)` — the raw text between `---` fences (exclusive of fences)
  - Uses the same boundary detection as `plan_corpus.parser.split_frontmatter_strict` (exact fence regex from `types.py:FRONTMATTER_FENCE`) — note: the actual API name is `split_frontmatter_strict`, NOT `split_frontmatter`
  - Returns empty/zero on malformed files (no fences) -- caller handles

- [x] Implement per-fix-type text operations (all operate on the frontmatter slice string):
  - `rename_key(fm_text: str, old_key: str, new_key: str) -> str` — regex `^{old_key}(\s*:.*)$` -> `{new_key}\1` (preserves value, spacing, inline comments)
  - `remove_key(fm_text: str, key: str) -> str` — remove the entire line matching `^{key}\s*:.*$` (handles multi-line values by tracking indent)
  - `replace_value(fm_text: str, key: str, new_value: str) -> str` — regex `^({key}\s*:\s*).*$` -> `\1{new_value}` (preserves key formatting)
  - `insert_key(fm_text: str, key: str, value: str, after_key: str | None) -> str` — insert `{key}: {value}` on a new line after `after_key` (or at end of frontmatter if `after_key` is None)
  - `remove_list_item(fm_text: str, list_key: str, item_value: str) -> str` — remove a single `- "value"` entry from a YAML list under `list_key`, handling both inline `[a, b]` and block `- a\n- b` list styles

- [x] Implement `apply_patch(path: Path, fm_operations: list[FmOperation], preimage: PreimageRecord) -> PatchResult`:
  - `FmOperation` = `(operation_type, **kwargs)` matching the per-fix-type operations above
  - **Concurrent-session guard (blind spot #6):**
    1. Re-read `path` and compute `sha256(content)` 
    2. Compare against `preimage.content_hash`
    3. If hashes differ: refuse to write, return `PatchResult(applied=False, reason="file modified since scan by concurrent session")`
    4. If hashes match: apply all operations to the frontmatter slice, reassemble full text, write to temp file (`path.with_suffix('.tmp')`) via `os.replace` for atomicity
  - Returns `PatchResult(applied: bool, reason: str, before_hash: str, after_hash: str)`

- [x] Implement `reassemble_file(original_text: str, patched_fm: str, start_offset: int, end_offset: int) -> str`:
  - Splice the patched frontmatter back into the original text at the correct offsets
  - Preserve everything before `start_offset` and after `end_offset` (including the `---` fences)

- [ ] **Shadow parser note (blind spot #5):** `roadmap_scan.py` (1462 lines) has its own `split_frontmatter`, `parse_section_file`, `parse_index_file` (~600 lines of parsing logic). This is LEAK:algorithmic-duplication with `plan_corpus`. The text patcher MUST NOT introduce a third frontmatter parser. It uses `plan_corpus.types.FRONTMATTER_FENCE` for boundary detection. The full `roadmap_scan.py` parser refactoring to import `plan_corpus` is tracked separately (it is a prerequisite for `--quick` mode correctness in 03.5, since `/continue-roadmap` and `/verify-roadmap --quick` must agree on corpus parse results). Add a `- [ ]` item to Section 05 or the plan overview noting this migration.

- [x] **Tests (TDD):**
  - **Semantic pin:** `rename_key` preserves YAML comments on the same line (`name: foo  # this is important`)
  - **Semantic pin:** `rename_key` preserves YAML comments on adjacent lines
  - **Semantic pin:** `remove_key` handles multi-line YAML values (indented continuation lines)
  - **Negative pin:** `apply_patch` refuses write when preimage hash mismatches (concurrent modification)
  - **Negative pin:** `apply_patch` refuses write on malformed files (no frontmatter fences)
  - **Atomicity test:** interrupt during write -> original file intact (temp file may remain)
  - **Round-trip test (TPR-03-003-gemini):** `extract -> modify -> reassemble -> parse with plan_corpus.parser` (NOT `yaml.safe_load`) produces expected YAML dict — the strict parser must accept the patched output, not just a lenient YAML loader
  - **Key ordering test:** unmodified keys retain their original order after patch
  - **Comment preservation test:** YAML comments (`# ...`) and inline comments survive all operations
  - **`remove_list_item` test:** both inline `[a, b]` and block `- a\n- b` list styles handled
  - **Collision guard integration test:** `plan:` exists and `name:` exists with different values -> ExposureReview classification -> patcher never invoked

- [x] **Subsection close-out (03.4)** -- MANDATORY before starting 03.5:
  - [x] All tasks above are `[x]` and text patcher tested with comment-preserving round-trips
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [x] **Run `/sync-claude` on THIS subsection** -- check whether changes
        invalidated any CLAUDE.md, `.claude/rules/*.md`, or `canon.md`
        claims. If no changes, document briefly. Fix any drift NOW.

---

## 03.5 Continue-Roadmap Integration

**File(s):** `.claude/skills/verify-roadmap/SKILL.md`, integration with roadmap-scan.sh

Integrate the findings report with `/continue-roadmap` so cross-plan conflicts surface during active roadmap work, not only during explicit `/verify-roadmap` runs.

- [x] Add a lightweight cross-plan check to roadmap-scan.sh:
  - Before `/continue-roadmap` selects the next section to work on, run a fast subset of the DAG analysis
  - Check whether the selected section has BLOCKED or DEAD_REFERENCE findings (the two classifiers included in `--quick` mode — NOT CONFLICT, which requires O(N^2) shared-subsystem analysis)
  - If findings exist, display them before proceeding and let the user decide whether to continue or switch to resolving the finding

- [x] Design the integration interface — resolve scope contradiction (blind spot #9):
  - The verify-roadmap skill exposes a `--quick` mode that runs ONLY `BLOCKED` and `DEAD_REFERENCE` checks (fast, no shared-subsystem analysis, no git signal population per 03.1)
  - **Explicitly NOT included in `--quick`:** CONFLICT (requires shared-subsystem analysis which is O(N^2)), STATUS_CONTRADICTION (requires body scanning), SUPERSEDED (requires reroute resolution), MISSING_DEPENDENCY (requires full prose scan)
  - The full mode (`--full`) runs all classifiers from Sections 01-02, runs `classify_safety` with full `WriteBackContext`, and applies auto-fixes
  - `/continue-roadmap` calls `--quick` mode as a pre-check; users invoke `--full` explicitly
  - **`--quick` mode MUST NOT build WriteBackContext (blind spot #10):** quick mode only runs read-only DAG checks. It skips git signal population entirely (no `git log` subprocess calls). It passes `context=None` to `classify_safety` (see 03.1), which returns ExposureReview for all findings. Report is generated in report-only mode (no auto-fix).

- [x] Document the integration in SKILL.md:
  - How `/continue-roadmap` uses the quick check
  - When to run `/verify-roadmap --full` manually (after plan changes, before major milestones)
  - How to interpret and act on findings
  - Explicit list of what `--quick` checks vs what `--full` checks (no ambiguity)

- [x] **Shadow parser migration (blind spot #5, TPR-03-001-gemini mandate):** `roadmap_scan.py` has ~600 lines of parsing logic (`split_frontmatter`, `parse_section_file`, `parse_index_file`) that duplicates `plan_corpus`. `--quick` mode MUST use `plan_corpus` for parsing — two diverging corpus truths is a LEAK:algorithmic-duplication that violates SSOT-2. **Mandated approach (Option A):** refactor `roadmap_scan.py` to import `plan_corpus.load_and_validate` as the sole parsing entrypoint (per Section 01's SSOT boundary — downstream consumers MUST NOT call `split_frontmatter_strict` directly), keeping only the `/continue-roadmap`-specific logic (section selection, focus plan, health signals). This eliminates the `errors="replace"` + `{}` on YAMLError swallowed-error pattern (`roadmap_scan.py:327-348`) that Section 01 was designed to prevent. **Option B (shadow parser divergence) is explicitly rejected** — it would allow the known LEAK to survive with no committed follow-up, violating R-2 and R-3. The migration is tracked as a concrete `- [ ]` in Section 05.

- [x] **Tests (TDD):**
  - **Integration test:** `/verify-roadmap --quick` returns findings for a corpus with a known BLOCKED finding
  - **Negative test:** `/verify-roadmap --quick` does NOT return CONFLICT findings (not in --quick scope)
  - **Performance test:** `--quick` mode completes in < 5 seconds on the full corpus (no git log calls)
  - **Semantic pin:** `--quick` mode with `context=None` -> all findings classified as ExposureReview

- [x] **Subsection close-out (03.5)** -- MANDATORY before marking section complete:
  - [x] All tasks above are `[x]` and integration tested
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [x] **Run `/sync-claude` on THIS subsection** -- check whether changes
        invalidated any CLAUDE.md, `.claude/rules/*.md`, or `canon.md`
        claims. If no changes, document briefly. Fix any drift NOW.
  - [x] **Repo hygiene check** -- run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 03.R Third Party Review Findings

- [x] `[TPR-03-001-codex][high]` `section-03:107` — Align schema-driven SafeFix rules with the schema SSOT. OverviewSchema uses `plan:` canonically, not `name:`. SPEC_FILE_NOT_FOUND needs its own handling.
  Resolved: Fixed on 2026-04-14. Corrected SafeFix table: `plan:→name:` rename restricted to PlanIndexSchema files only; OverviewSchema explicitly excluded. SPEC_FILE_NOT_FOUND reclassified to ExposureReview.
- [x] `[TPR-03-002-codex][medium]` `section-03:330` — Make quick-mode continue-roadmap contract consistent. BLOCKED vs CONFLICT scope contradiction.
  Resolved: Fixed on 2026-04-14. Integration bullets now consistently specify BLOCKED + DEAD_REFERENCE only for --quick mode.
- [x] `[TPR-03-003-codex][medium]` `section-03:286` — Propagate unapplied patch results into the report.
  Resolved: Fixed on 2026-04-14. Added concurrent-modification propagation to auto-fix guards (SafeFix→ExposureReview on hash mismatch), unapplied-fix report surface in 03.2, and test pin.
- [x] `[TPR-03-004-codex][medium]` `section-03:348` — Replace roadmap_scan migration placeholder with concrete checkbox.
  Resolved: Fixed on 2026-04-14. Option B rejected. Concrete `- [ ]` added to Section 05.3 mandating Option A migration.
- [x] `[TPR-03-005-codex][medium]` `section-03:240` — Either consume Section 02 SUPERSEDED handoff or remove it.
  Resolved: Fixed on 2026-04-14. Added explicit design decision: all SUPERSEDED → ExposureReview; WriteBackContext available for future SafeFix graduation.
- [x] `[TPR-03-001-gemini][high]` `section-03:278` — Remove Option B and mandate shadow parser migration.
  Resolved: Fixed on 2026-04-14. Same fix as TPR-03-004-codex — Option B removed, Option A mandated, Section 05.3 item added.
- [x] `[TPR-03-002-gemini][high]` `section-03:192` — Propagate concurrent modification failures to findings report.
  Resolved: Fixed on 2026-04-14. Same fix as TPR-03-003-codex — auto-fix dispatcher converts to ExposureReview, report format surfaces unapplied fixes.
- [x] `[TPR-03-003-gemini][medium]` `section-03:253` — Explicitly require plan_corpus.parser in round-trip test.
  Resolved: Fixed on 2026-04-14. Test description updated to specify plan_corpus.parser, not yaml.safe_load.

**Round 2 findings (iteration 2, 2026-04-14):**
- [x] `[TPR-03-001-codex-r2][medium]` `section-05:187` — Replace roadmap_scan migration with real plan_corpus API surface. References to nonexistent `parse_section_file`/`parse_index_file`.
  Resolved: Fixed on 2026-04-14. Updated §03.4 and §05.3 to use actual API: `read_text_strict`, `split_frontmatter_strict`, `load_and_validate`.
- [x] `[TPR-03-002-codex-r2][high]` `section-02:251` — Align §02 SUPERSEDED handoff with §03's all-ExposureReview decision. §02 still said "to route SafeFix vs ExposureReview."
  Resolved: Fixed on 2026-04-14. Updated §02 handoff text: git_status enrichment is advisory/reporting, not SafeFix routing. All SUPERSEDED → ExposureReview.
- [x] `[TPR-03-001-gemini-r2][high]` `section-03:68` — classify_safety needs parsed frontmatter for collision guard purity.
  Resolved: Fixed on 2026-04-14. Added `frontmatter_data: dict | None = None` parameter to `classify_safety` signature. Pre-parsed at scan time; no I/O inside classifier.
- [x] `[TPR-03-002-gemini-r2][medium]` `section-03:71` — Paired UNKNOWN_FIELD/MISSING_REQUIRED_FIELD deduplication for plan:→name: rename.
  Resolved: Fixed on 2026-04-14. Added paired-finding deduplication with `resolved_by_sibling` field on ClassifiedFinding.
- [x] `[TPR-03-003-gemini-r2][medium]` `section-05:46` — --quick mode must include Phase 5 for report generation.
  Resolved: Fixed on 2026-04-14. Updated §05.1: --quick runs Phases 1-3 and 5 (report-only, no auto-fix). Phase 4 skipped.

**Round 3 findings (iteration 3, 2026-04-14):**
- [x] `[TPR-03-001-codex-r3][high]` `section-05:68` — Point §05 phase wiring at real plan_corpus entrypoints (python -m scripts.plan_corpus, not scripts/plan_corpus.py).
  Resolved: Fixed on 2026-04-14. Updated Phase 1/2 entrypoints to actual package API.
- [x] `[TPR-03-002-codex-r3][high]` `section-05:113` — Realign §05 validation cases (a)/(g) with live route A/B behavior. Current corpus = MISSING_DEPENDENCY, not BLOCKED.
  Resolved: Fixed on 2026-04-14. Updated both test cases to expect MISSING_DEPENDENCY (route B), with note about route A migration.
- [x] `[TPR-03-003-codex-r3][medium]` `section-05:187` — Route roadmap_scan migration through load_and_validate, not low-level split_frontmatter_strict.
  Resolved: Fixed on 2026-04-14. Updated migration item to use load_and_validate as sole entrypoint per §01 SSOT boundary.
- [x] `[TPR-03-004-codex-r3][medium]` `section-05:178` — Undefined --check mode; replaced with --full --no-auto-fix.
  Resolved: Fixed on 2026-04-14. Changed verification step to use existing --full --no-auto-fix mode.
- [x] `[TPR-03-005-codex-r3][medium]` `section-03:114` — Carry resolved_by_sibling through the report contract.
  Resolved: Fixed on 2026-04-14. Updated §03.2 JSON spec to include resolved_by_sibling field.

---

## 03.N Completion Checklist

- [ ] `SafetyClass` enum (`SafeFix | ExposureReview`), `ClassifiedFinding` wrapper, `PreimageRecord` guard, and `classify_safety(finding, context)` defined and tested -- all OWNED here (not in `plan_corpus`)
- [ ] `WriteBackContext` carries git signals; `--quick` mode bypasses its construction entirely
- [ ] `classify_safety` is pure (no I/O); git signal population lives at the CLI edge; `plan_corpus` grep-verified to contain no `subprocess` or `git` calls
- [ ] `plan:` -> `name:` rename guarded against collision (both keys present with different values -> ExposureReview)
- [ ] `reviewed: false` insertion differentiated by schema class (PlanSection/RoadmapSection: SafeFix; PlanIndex: ExposureReview per workflow gate)
- [ ] `FM_DECLARED_VS_BODY_DERIVED` is ALWAYS ExposureReview — defense-in-depth assert in auto-fix engine
- [ ] Findings report format defined and implemented (JSON + markdown + console) — imports `Finding` / `FindingCategory` / `FindingSubtype` from `plan_corpus`, no shadow types
- [ ] Frontmatter text patcher is the ONLY write path — PyYAML never used for output; comments, key ordering, and formatting preserved
- [ ] Concurrent-session guards: preimage hash check, atomic write via `os.replace`, refuse-on-conflict
- [ ] Dead-reference audit trail in `fixes-applied.json` only — NO inline HTML comments (re-scanning hazard)
- [ ] Auto-fix engine applies only `SafeFix` findings; hard-asserts rejection of `ExposureReview` findings
- [ ] Manual-review flagging for CONFLICT, SUPERSEDED, BLOCKED, MISSING_DEPENDENCY (intrinsically manual) + any ExposureReview-classified finding
- [ ] Safe-fix guards: backups, logging, `--dry-run`, `--no-auto-fix`
- [ ] Integration with `/continue-roadmap` via `--quick` mode pre-check (BLOCKED + DEAD_REFERENCE only; no CONFLICT)
- [ ] `--quick` vs `--full` scope explicitly documented — no ambiguity on which classifiers run in each mode
- [ ] `roadmap_scan.py` shadow parser migration mandated as Option A in Section 05.3 — Option B rejected per TPR
- [ ] `timeout 150 ./test-all.sh` green -- no regressions
- [ ] `/tpr-review` -- dual-source review of report format, auto-fix logic, text patcher safety, concurrent-session guards
- [ ] `/impl-hygiene-review` -- verify auto-fix safety (no semantic changes), report completeness, no shadow parsers introduced
- [ ] `/improve-tooling` section-close sweep -- verify per-subsection retrospectives ran; add cross-subsection findings
- [ ] `/sync-claude` section-close sweep -- verify CLAUDE.md and rules reflect new verify-roadmap modes and integration points
