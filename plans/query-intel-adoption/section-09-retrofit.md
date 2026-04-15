---
section: "09"
title: "Retrofit active plans — status-gated recon coverage"
status: not-started
reviewed: false
goal: "Backfill the unnumbered `## Intelligence Reconnaissance` block across the active plan corpus using a status-gated severity model (not an A/B/C operator menu). Status `not-started` sections emit `Severity.HIGH` when missing recon; `in-progress` sections emit `HIGH` on next body edit; `complete` sections are exempt unless explicitly reopened. Meta-dogfood is scoped to `not-started` sections of this plan only (§06, §07, §08, §09 itself) — completed sections (§01-§05) are frozen and NOT rewritten."
success_criteria:
  - "`python -m scripts.plan_corpus discover` reports 100% recon-block coverage for every plan section with `status: not-started` across `plans/` (excluding `plans/completed/` and `plans/bug-tracker/`); the per-plan coverage table (added in §06.2) shows `N/N` for the `not-started` slice of every active plan"
  - "`scripts/plan_corpus/retrofit_recon.py` is a permanent tool (not throwaway) that enumerates targets via `ValidatedFile.frontmatter.status`, writes stub recon blocks into `not-started` sections, and refuses to touch `status: complete` sections without an explicit `--allow-reopen <path>` flag per target"
  - "§06, §07, §08, and §09 of this plan carry non-stub recon blocks (meta-dogfood); §01-§05 (already `status: complete`, `reviewed: true`, `third_party_review: resolved`) are NOT modified — no historical-fiction retrospective-recon injected into frozen sections"
  - "Validator's status-gated severity applies: `status: not-started` missing recon → `Severity.HIGH`; `status: in-progress` sections print `Severity.MEDIUM` and escalate to `Severity.HIGH` ONLY when the section body is edited on the next touch (enforced via a `check --on-edit` mode wired into lefthook, or documented as a follow-up without introducing a deferral); `status: complete` sections are exempt"
  - "Retrofit does NOT flip `reviewed: true` → `reviewed: false` on any section, because no `reviewed: true` section is touched (meta-dogfood scoped to `not-started`; `in-progress` retrofit is on-edit only)"
inspired_by:
  - "Phase 2 dual-source convergence (Codex + Gemini, 2026-04-14): A/B/C operator menu is the wrong decomposition — it forces a subjective scope choice when the corpus already carries an objective `status` field that encodes exactly which sections are live work vs. frozen artifacts"
  - "CLAUDE.md §Stabilization Discipline: 'Every fix becomes a permanent test' and 'Fix interference = reorder, don't skip' — retrofitting frozen `status: complete` artifacts with reconstructed-from-memory queries creates documented investigations that never happened (historical fiction) and forces meaningless re-review cycles"
  - "§06.2 per-plan coverage reporting — the validator already computes per-section recon presence; retrofit scope is the subset of that report where `status == 'not-started'`"
depends_on: ["06"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "09.1"
    title: "Build retrofit_recon.py using status-gated targeting"
    status: not-started
  - id: "09.2"
    title: "Meta-dogfood: fill recon blocks in §06, §07, §08, §09"
    status: not-started
  - id: "09.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "09.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 09: Retrofit active plans — status-gated recon coverage

**Status:** Not Started
**Goal:** Backfill the unnumbered `## Intelligence Reconnaissance` block across the active plan corpus using the objective `status` field, not a subjective A/B/C operator menu. Scope is defined by data (`ValidatedFile.frontmatter.status`), not by a human policy choice. `status: not-started` sections get a stub recon block the owner fills on next touch; `status: in-progress` sections emit a validator warning that escalates on edit; `status: complete` sections are exempt (no retrospective reconstruction).

**Context:** An earlier draft of this work lived inside §06.3 with an A/B/C policy menu (aggressive / moderate / passive) plus a mandate to inject retrospective recon blocks into frozen `reviewed: true` sections. Phase 2 dual-source review (both Codex and Gemini, 2026-04-14) converged on rejecting that model: Option A violates CLAUDE.md §Stabilization Discipline by mutating frozen-reviewed artifacts and producing historical fiction (queries reconstructed from git-log + memory for investigations that never happened); Option C leaves active work un-reconnoitered. The right decomposition is status-gated severity — the corpus already knows which sections are live (`not-started` / `in-progress`) vs. frozen (`complete`), so scope enumeration is mechanical, not judgmental.

**Design decision 1 — scope is `status: not-started` only for forced backfill.** `scripts/plan_corpus/retrofit_recon.py` enumerates target files by reading `ValidatedFile.frontmatter.status` (per `scripts/plan_corpus/discovery.py:load_and_validate`) and filters to `status == "not-started"`. `in-progress` sections are reported but NOT edited by the retrofit tool — they escalate on the next body edit via the validator's on-edit mode. `complete` sections are exempt entirely. This matches the §06.2 validator's status-gated severity exactly, so retrofit and validation share one data-driven scope.

**Design decision 2 — no `reviewed: true` invalidation.** Because retrofit only writes into `status: not-started` sections (which are by construction `reviewed: false` — completed sections become reviewed after implementation, not before), the retrofit never touches a `reviewed: true` section. The earlier §06.3 mandate to flip `reviewed: true` → `reviewed: false` with `<!-- retrofit:06.3 -->` annotation is therefore moot. If the on-edit-escalation path ever reaches a `reviewed: true` `in-progress` section (rare — such a section would have drifted between review and implementation), the tool refuses the edit and emits a `Severity.HIGH` finding demanding human resolution.

**Design decision 3 — meta-dogfood is `not-started` sections of this plan only.** §01-§05 are `status: complete`, `reviewed: true`, `third_party_review: resolved`. Mandating retrospective-recon blocks on those sections — as the prior §06.3 draft did — injects fabricated investigation records into sections that have already been independently reviewed and closed. That is historical fiction and is explicitly banned by CLAUDE.md §Correctness Above All (the correct record is "no recon was done at the time," not a reconstructed-from-memory narrative). Meta-dogfood is therefore scoped to `not-started` sections of this plan at §09 land time: §06, §07, §08, and §09 itself. Each gets a non-stub recon block written during the actual authoring / implementation window, not reconstructed.

**Design decision 4 — `retrofit_recon.py` is a permanent `python -m scripts.plan_corpus` subcommand, not a throwaway script.** Future plans may spawn new `status: not-started` sections after §09 lands but before the next corpus-wide sweep. The tool must remain runnable as `python -m scripts.plan_corpus retrofit-recon [--dry-run] [--plan <dir>]` so any plan author can backfill a newly-created batch of sections without bespoke scripting. Subcommand registration lives in `scripts/plan_corpus/__main__.py`.

**Reference implementations:**
- **Ori** `scripts/plan_corpus/discovery.py:load_and_validate` — yields `ValidatedFile` instances with `frontmatter` and `body_text`; retrofit iterates these and filters on `frontmatter.get("status")`
- **Ori** `scripts/plan_corpus/__main__.py` — subcommand registry (`sub.add_parser(...)` pattern); §09.1 adds `retrofit-recon` alongside `check`, `discover`, `docgen`
- **Ori** §06.2 validator (the dependency) — per-plan coverage reporter and status-gated severity model; §09 consumes this reporter to measure success

**Depends on:** Section 06 (the validator must exist and report per-plan status-gated coverage before retrofit can measure its own completion).

---

## Intelligence Reconnaissance

Queries run 2026-04-14 (during /review-plan authoring of §09):

- `scripts/intel-query.sh status` — graph available (191K Ori symbols, 505K CALLS edges, 298K issues)
- `scripts/intel-query.sh --human file-symbols "scripts/plan_corpus/discovery" --repo ori` — zero results (`scripts/plan_corpus/` is Python; the code-symbol index is Rust-only today). Confirmed via Read of `discovery.py` that `load_and_validate` yields `ValidatedFile(path, file_class, frontmatter, body_text, findings)` — the enumeration surface for retrofit.
- `scripts/intel-query.sh --human search "bulk rewrite frontmatter"` — external-repo hits on mass-edit tooling (rustc book, go module-graph rewriting); limited direct applicability to markdown plan corpus
- `grep -l 'status: not-started' plans/*/section-*.md | grep -v completed | grep -v bug-tracker | wc -l` — N `not-started` sections across the active corpus (count recomputed at §09.1 runtime; informs the retrofit target set)
- `grep -l 'reviewed: true' plans/*/section-*.md | grep -v completed | grep -v bug-tracker | wc -l` — M `reviewed: true` sections across the active corpus; retrofit MUST NOT write into any of these (verified by assertion in §09.1)

Results summary (≤500 chars): Scope is mechanical, not judgmental — `ValidatedFile.frontmatter["status"] == "not-started"` defines the target set. Graph does not index `scripts/plan_corpus/` (Python); surface walked directly. No `reviewed: true` section is touched under this design, so the earlier `<!-- retrofit:06.3 -->` reviewed-invalidation mandate is obsolete. Meta-dogfood is §06/§07/§08/§09 only; §01-§05 remain frozen.

See `.claude/skills/dual-tpr/compose-intel-summary.md` for the full query protocol (SSOT — do NOT `@`-include in plan bodies; markdown is not harness-expanded).

---

## 09.1 Build retrofit_recon.py using status-gated targeting

**File(s):** `scripts/plan_corpus/retrofit_recon.py` (new, ~200 lines), `scripts/plan_corpus/__main__.py`, `tests/plan-audit/test_retrofit_recon.py` (new)

- [ ] **Create `scripts/plan_corpus/retrofit_recon.py`** as a module exposing `run_retrofit(root: Path, *, dry_run: bool, plan_filter: str | None, allow_reopen: list[Path]) -> RetrofitReport`. The function:
  - Iterates `discovery.load_and_validate(root)` to get `ValidatedFile` instances
  - Filters to `file_class == FileClass.PLAN_SECTION` AND `frontmatter.get("status") == "not-started"` AND the file's parent is not `plans/completed/` or `plans/bug-tracker/`
  - If `plan_filter` is set, additionally filters to sections under that plan directory
  - For each target: checks whether the body already contains `^## Intelligence Reconnaissance\s*$` (via the same regex §06.2 uses); if present, skip; if absent, insert the stub block at the canonical position (after the last framing block `**Depends on:** ...` and before the first `## {NN}.` header)
  - Asserts every target has `frontmatter.get("reviewed") is False` before writing; if a target has `reviewed: true` AND its path is not in `allow_reopen`, abort with a `Severity.HIGH` report entry naming the file
  - `dry_run=True`: collects the planned edits into `RetrofitReport.planned_edits` and returns without writing
  - `dry_run=False`: writes each edit atomically (temp-file + rename per `impl-hygiene.md` §Cross-Platform Parity) and records applied edits in `RetrofitReport.applied_edits`

- [ ] **Inserted stub block shape** (identical for every target so the validator recognizes it uniformly as "stub" and emits a single predictable finding class):
  ```markdown
  ---

  ## Intelligence Reconnaissance

  <!-- retrofit:09 YYYY-MM-DD — stub inserted by retrofit_recon.py. -->
  <!-- Fill in before implementation begins. The validator emits Severity.MEDIUM for stubs; -->
  <!-- Severity.HIGH when the section transitions to status: in-progress with the stub intact. -->

  Queries run: (not yet filled in — author runs `scripts/intel-query.sh` commands per `.claude/skills/dual-tpr/compose-intel-summary.md` and records them here)

  Results summary (≤500 chars): (not yet filled in)

  See `.claude/skills/dual-tpr/compose-intel-summary.md` for the query protocol (SSOT).
  ```
  The stub is DELIBERATELY incomplete — header present but no `scripts/intel-query.sh` literal invocation in the body — so §06.2's anti-performative-ritual detection classifies it as `stub` (not `complete`, not `missing`) and emits the intended warning until the owner fills it.

- [ ] **Register the subcommand in `scripts/plan_corpus/__main__.py`** alongside `check`, `discover`, `docgen`:
  ```python
  p_retrofit = sub.add_parser(
      "retrofit-recon",
      help="Insert stub Intelligence Reconnaissance blocks into status: not-started plan sections.",
  )
  p_retrofit.add_argument("--dry-run", action="store_true",
      help="List targets and planned edits; write nothing.")
  p_retrofit.add_argument("--plan", metavar="DIR",
      help="Restrict targets to a single plan directory.")
  p_retrofit.add_argument("--allow-reopen", action="append", metavar="PATH", default=[],
      help="Permit retrofit to touch the named reviewed: true section. Repeatable.")
  ```
  Map to `run_retrofit(...)`; exit 0 on success, 1 on any `Severity.HIGH` report entry (reviewed-true guard violation).

- [ ] **Write `tests/plan-audit/test_retrofit_recon.py`** covering the full matrix — (section status: `not-started` / `in-progress` / `complete`) × (body shape: no-recon-block / stub-recon-block / complete-recon-block) × (reviewed flag: `false` / `true`) × (mode: dry-run / apply):
  - `not_started_no_block_reviewed_false_apply` → inserts stub; no reviewed flip; 0 HIGH findings
  - `not_started_no_block_reviewed_true_apply_without_allow` → ABORT with HIGH finding; file unchanged
  - `not_started_no_block_reviewed_true_apply_with_allow_reopen` → inserts stub; no reviewed flip (reviewed stays true and is flagged for re-review separately); 0 HIGH findings
  - `not_started_stub_block_apply` → skip (stub already present); 0 edits
  - `not_started_complete_block_apply` → skip (already satisfies validator); 0 edits
  - `in_progress_no_block_apply` → skip (not in retrofit target set); reporter notes "in-progress with missing recon — validator will escalate on next edit"
  - `complete_no_block_apply` → skip (exempt); reporter notes "complete — exempt"
  - `not_started_no_block_reviewed_false_dry_run` → no file writes; `RetrofitReport.planned_edits` populated
  - Plan filter: `plan_filter="plans/query-intel-adoption"` limits enumeration to this plan only
  - Exit-code tests: `main(["retrofit-recon", "--dry-run"])` → 0 on clean corpus; 1 on corpus containing a `reviewed: true not-started` target without `--allow-reopen`

  Follow the fixture conventions already used in `tests/plan-audit/test_plan_corpus.py`.

- [ ] **Subsection close-out (09.1)** — MANDATORY before starting 09.2:
  - [ ] `retrofit_recon.py` lands; subcommand registered; all 10 matrix tests pass via `pytest tests/plan-audit/test_retrofit_recon.py`
  - [ ] `./test-all.sh` green (no regressions in existing plan-audit or Rust test suites)
  - [ ] `python -m scripts.plan_corpus retrofit-recon --dry-run` on the full active corpus prints a report whose target count matches the manually-computed `grep -l 'status: not-started' plans/*/section-*.md | grep -v completed | grep -v bug-tracker | xargs grep -L '## Intelligence Reconnaissance' | wc -l`
  - [ ] Update `09.1` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 09.1** — did the atomic-write / path-enumeration paths surface any missing helper in `scripts/plan_corpus/discovery.py`? Was the `--allow-reopen` UX clear (does a single-flag path list scale or should it be a config file)? Commit improvements via `build(tooling): <change> — surfaced by query-intel-adoption/section-09.1 retrospective`. Document "no gaps" explicitly if none.
  - [ ] **Run `/sync-claude` on 09.1** — CLAUDE.md §Commands "Plan corpus" bullet (line ~167) must gain `retrofit-recon` alongside `check`, `discover`, `docgen`. Verify no `.claude/rules/*.md` file references a throwaway retrofit script.
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check` → clean.

---

## 09.2 Meta-dogfood: fill recon blocks in §06, §07, §08, §09

**File(s):** `plans/query-intel-adoption/section-06-plan-schema-recon.md`, `section-07-pre-review-intel-hook.md`, `section-08-tool-ux-and-output.md`, `section-09-retrofit.md`

All four target sections are `status: not-started` at §09 land time. Meta-dogfood writes non-stub recon blocks into each, documenting the queries actually run during their authoring — NOT reconstructed from memory for frozen sections (§01-§05 remain untouched).

- [ ] **Verify scope at start of 09.2** — run `python -m scripts.plan_corpus check plans/query-intel-adoption/` and confirm that the only `Severity.MEDIUM`-or-higher recon-block findings are on `§06`, `§07`, `§08`, `§09` (and NO findings on §01-§05, because they're `status: complete` and exempt). If §01-§05 fire findings, §06.2's status-gated severity gate is broken — fix §06.2 before proceeding. Do NOT write retrospective-recon into frozen sections.

- [ ] **§06 recon block** — already present in the restructured §06 body (written during §06 authoring on 2026-04-14). Run `python -m scripts.plan_corpus check plans/query-intel-adoption/section-06-plan-schema-recon.md` and confirm zero recon-related findings. If the block is a stub, fill it with the queries actually run during §06 implementation before marking 09.2 complete.

- [ ] **§07 recon block** — author runs the §07-relevant queries during §07 implementation and records them in the recon block. Example queries (tailored by §07's author to the actual implementation):
  - `scripts/intel-query.sh status`
  - `scripts/intel-query.sh --human file-symbols ".claude/hooks" --repo ori`
  - `scripts/intel-query.sh --human search "UserPromptSubmit hook additionalContext"`
  - Bounded ≤500-char results summary covering hook shape, matcher reuse (`classify-review-command.py`), and graceful-degradation precedent (`block-banned-commands.sh`).

- [ ] **§08 recon block** — author runs the §08-relevant queries during §08 implementation. Example queries:
  - `scripts/intel-query.sh status`
  - `scripts/intel-query.sh --human file-symbols "scripts/intel-query" --repo ori`
  - `scripts/intel-query.sh --human search "tty detection CLI tool --human default"`
  - Bounded ≤500-char summary covering tty-default precedent (rustc/cargo), ASCII call-tree prior art (lean4 #tree), and deep-link conventions.

- [ ] **§09 recon block** — already present in this file (authored 2026-04-14). Fill in any additional queries run during §09 implementation if the authoring-time block proves incomplete when §09 work actually begins.

- [ ] **Re-run validator after each fill** — `python -m scripts.plan_corpus check plans/query-intel-adoption/<section>.md` returns zero recon-related findings for each of §06/§07/§08/§09.

- [ ] **Subsection close-out (09.2)** — MANDATORY before section close:
  - [ ] `python -m scripts.plan_corpus check plans/query-intel-adoption/` returns zero `Severity.HIGH` or `Severity.MEDIUM` recon findings across §06-§09
  - [ ] `python -m scripts.plan_corpus discover` per-plan coverage report shows `4/4` recon-block coverage for the `not-started` slice of `plans/query-intel-adoption/` (the 4 `not-started` sections are §06, §07, §08, §09)
  - [ ] §01-§05 untouched: `git diff --stat plans/query-intel-adoption/section-0{1,2,3,4,5}-*.md` is empty
  - [ ] Update `09.2` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 09.2** — did hand-writing four recon blocks surface a gap in the intel query scripts (e.g., missing `blast-radius` composite — addressed in §08, cross-link here)? Document findings.
  - [ ] **Run `/sync-claude` on 09.2** — verify CLAUDE.md and rule files still match the final plan-corpus surface (status-gated severity, `retrofit-recon` subcommand, meta-dogfood scope).
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check` → clean.

---

## 09.R Third Party Review Findings

- None.

---

## 09.N Completion Checklist

- [ ] All 09.1 close-out items complete (retrofit_recon.py + subcommand + matrix tests + reviewed-flag guard + dry-run equivalence)
- [ ] All 09.2 close-out items complete (§06/§07/§08/§09 recon blocks filled; §01-§05 untouched; discover coverage 4/4 on this plan's not-started slice)
- [ ] `python -m scripts.plan_corpus discover` reports 100% recon-block coverage for the `not-started` slice of every active plan corpus-wide
- [ ] `./test-all.sh` green (including new plan-audit tests)
- [ ] `python -m scripts.plan_corpus check plans/` returns exit 0 (no Outcome.ERROR recon findings)
- [ ] **Plan sync**:
  - [ ] Section frontmatter `status: not-started` → `complete`
  - [ ] `00-overview.md` Mission Success Criteria §09 checkbox checked; Quick Reference §09 row → Complete
  - [ ] `index.md` §09 status updated
- [ ] `/tpr-review` passed — clean dual-source pass (watch-items: status-gated severity gates correctly; `reviewed: true` guard cannot be bypassed without `--allow-reopen`; meta-dogfood did not touch §01-§05; no historical-fiction narratives in any recon block)
- [ ] `/impl-hygiene-review` passed — verify `retrofit_recon.py` reuses `discovery.load_and_validate` (no parallel enumeration); verify the stub block shape is a single source of truth shared with §06.2's detector
- [ ] `/improve-tooling` section-close sweep — cross-subsection patterns only
- [ ] `/sync-claude` section-close doc sync
- [ ] `diagnostics/repo-hygiene.sh --check` → clean
