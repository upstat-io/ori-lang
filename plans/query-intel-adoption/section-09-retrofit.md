---
section: "09"
title: "Retrofit active plans — status-gated recon coverage"
status: not-started
reviewed: false
goal: "Backfill the unnumbered `## Intelligence Reconnaissance` block across the active plan corpus using a status-gated severity model (not an A/B/C operator menu). Status `not-started` sections emit `Severity.HIGH` when missing recon; `in-progress` sections emit `Severity.MEDIUM` / `Outcome.WARNING` (no on-edit escalation); `complete` sections are exempt unless explicitly reopened. Meta-dogfood is scoped to `not-started` sections of this plan only (§06, §07, §08, §09 itself) — completed sections (§01-§05) are frozen and NOT rewritten."
success_criteria:
  - "`python -m scripts.plan_corpus discover` reports 100% recon-block PRESENCE for every plan section with `status: not-started` across `plans/` (excluding `plans/completed/` and `plans/bug-tracker/`); the per-plan coverage table (added in §06.2) shows `N/N` for the `not-started` slice of every active plan. A retrofit stub counts as 'present' for this metric — the discover reporter counts block presence, not quality. Quality issues (stub vs. complete) are separate `VALIDATION_BYPASS` findings emitted by `check`, not `discover`."
  - "`scripts/plan_corpus/retrofit_recon.py` is a permanent tool (not throwaway) that enumerates targets via `ValidatedFile.data[\"status\"]` (where `data` is the parsed frontmatter dict per `discovery.py:210`), writes stub recon blocks into `not-started` sections, and refuses to touch `status: complete` sections without an explicit `--allow-reopen <path>` flag per target"
  - "§06, §07, §08, and §09 of this plan carry non-stub recon blocks (meta-dogfood); §01-§05 (all `status: complete`) are NOT modified — no historical-fiction retrospective-recon injected into frozen sections"
  - "Validator's status-gated severity applies per §06 Design Decision 5: `status: not-started` missing recon → `Severity.HIGH`, `Outcome.WARNING` (default) or `Outcome.ERROR` (`--strict-recon`); `status: in-progress` missing recon → `Severity.MEDIUM`, `Outcome.WARNING`; `status: complete` → exempt (0 findings). No `--on-edit` escalation mechanism — in-progress sections stay at `Severity.MEDIUM` / `Outcome.WARNING` regardless of edits; the simplicity of two fixed severity tiers (not-started=HIGH, in-progress=MEDIUM) is sufficient for this plan's scope"
  - "Retrofit preserves `reviewed: true` on all sections — the `reviewed` flag is never modified by retrofit. For existing `status: not-started` + `reviewed: true` sections in the corpus (the exact count is computed at runtime by `retrofit_recon.py`), retrofit adds the recon-block stub but does NOT change `reviewed: true` to `reviewed: false` (retrofit is additive; adding a recon block does not invalidate prior review). The `--allow-reopen` flag applies ONLY to `status: complete` sections (reopening review cycle); it does NOT affect the `reviewed` field on `not-started` sections"
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
**Goal:** Backfill the unnumbered `## Intelligence Reconnaissance` block across the active plan corpus using the objective `status` field, not a subjective A/B/C operator menu. Scope is defined by data (`ValidatedFile.data["status"]` — where `data` is the parsed frontmatter dict per `discovery.py:210`), not by a human policy choice. `status: not-started` sections get a stub recon block the owner fills on next touch; `status: in-progress` sections emit a validator warning (`Severity.MEDIUM`, `Outcome.WARNING`); `status: complete` sections are exempt (no retrospective reconstruction).

**Context:** An earlier draft of this work lived inside §06.3 with an A/B/C policy menu (aggressive / moderate / passive) plus a mandate to inject retrospective recon blocks into frozen `reviewed: true` sections. Phase 2 dual-source review (both Codex and Gemini, 2026-04-14) converged on rejecting that model: Option A violates CLAUDE.md §Stabilization Discipline by mutating frozen-reviewed artifacts and producing historical fiction (queries reconstructed from git-log + memory for investigations that never happened); Option C leaves active work un-reconnoitered. The right decomposition is status-gated severity — the corpus already knows which sections are live (`not-started` / `in-progress`) vs. frozen (`complete`), so scope enumeration is mechanical, not judgmental.

**Design decision 1 — scope is `status: not-started` only for forced backfill.** `scripts/plan_corpus/retrofit_recon.py` enumerates target files by iterating `discover_corpus(root).plan_sections.keys()` to get PLAN_SECTION paths, then calling `load_and_validate(path)` on each (since `load_and_validate` takes a single `Path` and returns a single `LoadResult` — per `discovery.py:227`). It reads `ValidatedFile.data["status"]` (where `data` is the parsed frontmatter dict per `discovery.py:210`) and filters to `status == "not-started"`. `in-progress` sections are reported but NOT edited by the retrofit tool — the validator emits `Severity.MEDIUM`, `Outcome.WARNING` on them. `complete` sections are exempt entirely. This matches the §06.2 validator's status-gated severity exactly, so retrofit and validation share one data-driven scope.

**Design decision 2 — retrofit preserves `reviewed: true`; no invalidation.** The corpus contains a non-trivial count of `status: not-started` sections that ALREADY carry `reviewed: true` (verified empirically at plan-authoring time; the exact count is computed at runtime by `retrofit_recon.py`). The "by construction reviewed:false" assumption from the prior draft is factually wrong. Retrofit handles this correctly: it adds the recon-block stub to ALL `not-started` sections regardless of their `reviewed` flag, and NEVER modifies the `reviewed` field. Adding a recon block is additive content — it does not invalidate prior review. The earlier §06.3 mandate to flip `reviewed: true` → `reviewed: false` with `<!-- retrofit:06.3 -->` annotation is therefore moot. The `--allow-reopen` flag applies ONLY to `status: complete` sections (reopening a completed review cycle to add recon); it does NOT interact with `reviewed` on `not-started` sections.

**Design decision 3 — meta-dogfood is `not-started` sections of this plan only.** §01-§05 are all `status: complete`. Mandating retrospective-recon blocks on those sections — as the prior §06.3 draft did — injects fabricated investigation records into sections that have already been independently reviewed and closed. That is historical fiction and is explicitly banned by CLAUDE.md §Correctness Above All (the correct record is "no recon was done at the time," not a reconstructed-from-memory narrative). Meta-dogfood is therefore scoped to `not-started` sections of this plan at §09 land time: §06, §07, §08, and §09 itself. Each gets a non-stub recon block written during the actual authoring / implementation window, not reconstructed.

**Design decision 4 — `retrofit_recon.py` is a permanent `python -m scripts.plan_corpus` subcommand, not a throwaway script.** Future plans may spawn new `status: not-started` sections after §09 lands but before the next corpus-wide sweep. The tool must remain runnable as `python -m scripts.plan_corpus retrofit-recon [--dry-run] [--plan <dir>]` so any plan author can backfill a newly-created batch of sections without bespoke scripting. Subcommand registration lives in `scripts/plan_corpus/__main__.py`.

**Reference implementations:**
- **Ori** `scripts/plan_corpus/discovery.py:load_and_validate(path)` — takes a single `Path`, returns a `LoadResult` with `.ok: ValidatedFile | None` and `.err: Finding | None`; `ValidatedFile` carries `data` (parsed frontmatter dict) and `body` (body text); retrofit calls this per-path and filters on `data.get("status")`
- **Ori** `scripts/plan_corpus/__main__.py` — subcommand registry (`sub.add_parser(...)` pattern); §09.1 adds `retrofit-recon` alongside `check`, `discover`, `docgen`
- **Ori** §06.2 validator (the dependency) — per-plan coverage reporter and status-gated severity model; §09 consumes this reporter to measure success

**Depends on:** Section 06 (the validator must exist and report per-plan status-gated coverage before retrofit can measure its own completion).

---

## Intelligence Reconnaissance

Queries run 2026-04-14 (during /review-plan authoring of §09):

- `scripts/intel-query.sh status` — graph available (191K Ori symbols, 505K CALLS edges, 298K issues)
- `scripts/intel-query.sh --human file-symbols "scripts/plan_corpus/discovery" --repo ori` — zero results (`scripts/plan_corpus/` is Python; the code-symbol index is Rust-only today). Confirmed via Read of `discovery.py` that `load_and_validate(path: Path) -> LoadResult` returns a `LoadResult` with `.ok: ValidatedFile | None`; `ValidatedFile` fields: `path`, `file_class`, `data` (frontmatter dict), `body_offset`, `body` (body text), `violations` — the enumeration surface for retrofit.
- `scripts/intel-query.sh --human search "bulk rewrite frontmatter"` — external-repo hits on mass-edit tooling (rustc book, go module-graph rewriting); limited direct applicability to markdown plan corpus
- `grep -l 'status: not-started' plans/*/section-*.md | grep -v completed | grep -v bug-tracker | wc -l` — N `not-started` sections across the active corpus (count recomputed at §09.1 runtime; informs the retrofit target set)
- `grep -l 'reviewed: true' plans/*/section-*.md | grep -v completed | grep -v bug-tracker | wc -l` — M `reviewed: true` sections across the active corpus; retrofit PRESERVES `reviewed: true` unchanged when writing stubs into `not-started` sections (additive content, not review-invalidation). The `--allow-reopen` flag applies ONLY to `status: complete` sections.

Results summary (≤500 chars): Scope is mechanical — `ValidatedFile.data["status"] == "not-started"` defines the target set. A non-trivial count of `not-started` sections already carry `reviewed: true` (exact count computed at runtime by `retrofit_recon.py`) — retrofit adds the recon-block stub to ALL targets and preserves `reviewed: true` unchanged (additive content, not review-invalidation). `--allow-reopen` scoped to `status: complete` only. Graph does not index `scripts/plan_corpus/` (Python); surface walked directly. Meta-dogfood is §06/§07/§08/§09 only; §01-§05 remain frozen.

See `.claude/skills/dual-tpr/compose-intel-summary.md` for the full query protocol (SSOT — do NOT `@`-include in plan bodies; markdown is not harness-expanded).

---

## 09.1 Build retrofit_recon.py using status-gated targeting

**File(s):** `scripts/plan_corpus/retrofit_recon.py` (new, ~200 lines), `scripts/plan_corpus/__main__.py`, `tests/plan-audit/test_retrofit_recon.py` (new)

- [ ] **Create `scripts/plan_corpus/retrofit_recon.py`** as a module exposing `run_retrofit(root: Path, *, dry_run: bool, plan_filter: str | None, allow_reopen: list[Path]) -> RetrofitReport`. The function:
  - Enumerates PLAN_SECTION paths via `discover_corpus(root).plan_sections.keys()`, then calls `load_and_validate(path)` on each (since `load_and_validate` takes a single `Path` and returns a single `LoadResult` — per `discovery.py:227`). Explicit iteration pattern:
    ```python
    from scripts.plan_corpus.discovery import discover_corpus, load_and_validate
    corpus = discover_corpus(root)
    errors = []
    for path in corpus.plan_sections.keys():
        result = load_and_validate(path)
        if not result.is_ok:
            errors.append((path, result.err)); continue  # accumulate parse errors, report at end
        vf = result.ok
        if vf.file_class != FileClass.PLAN_SECTION:
            continue  # safety — should always match
        status = vf.data.get("status")
        if status == "complete":
            if path in allow_reopen_paths:
                # process with HIGH finding noting reopen
                pass
            else:
                continue  # skip silently — complete sections need explicit --allow-reopen
        if status == "in-progress":
            continue  # not in forced-backfill target set; validator emits Severity.MEDIUM warning
        if status != "not-started":
            continue
        # ... insert stub
    ```
  - After the loop, all accumulated `errors` are reported in the `RetrofitReport.parse_errors` field and printed to stderr. Parse errors participate in exit-code: exit 1 if any parse errors exist, even if all stubs were injected successfully into the parseable files.
  - If `plan_filter` is set, additionally filters to sections under that plan directory
  - For each target: checks whether `vf.body` already contains `^## Intelligence Reconnaissance\s*$` (via the same regex §06.2 uses); if present, skip; if absent, insert the stub block at the canonical position (after the last framing block `**Depends on:** ...` and before the first `## {NN}.` header)
  - **Handle existing `status: not-started` + `reviewed: true` sections in the corpus** (exact count computed at runtime by `retrofit_recon.py`): retrofit adds the recon block stub and PRESERVES the `reviewed: true` flag unchanged — retrofit is additive content, not a review-invalidating change. The `--allow-reopen` flag applies ONLY to `status: complete` sections (reopening a completed review cycle); `status: complete` sections encountered during bulk enumeration without `--allow-reopen` are SILENTLY SKIPPED (via `continue`) — no abort, no HIGH finding. Only a `status: complete` section that is EXPLICITLY listed via `--allow-reopen` will be written. If `--allow-reopen` names a path that is NOT `status: complete`, that is a no-op (the flag only unlocks complete sections; `not-started` and `in-progress` sections are handled by their normal path)
  - `dry_run=True`: collects the planned edits into `RetrofitReport.planned_edits` and returns without writing
  - `dry_run=False`: writes each edit atomically (temp-file + rename per `impl-hygiene.md` §Cross-Platform Parity) and records applied edits in `RetrofitReport.applied_edits`

- [ ] **Inserted stub block shape** (identical for every target so the validator recognizes it uniformly as "stub" and emits a single predictable finding class):
  ```markdown
  ---

  ## Intelligence Reconnaissance

  <!-- retrofit:09 YYYY-MM-DD — stub inserted by retrofit_recon.py. -->
  <!-- Fill in before implementation begins. Per §06 Design Decision 5: -->
  <!-- status: not-started + stub → Severity.HIGH, Outcome.WARNING (default) or Outcome.ERROR (--strict-recon) -->
  <!-- status: in-progress + stub → Severity.MEDIUM, Outcome.WARNING -->

  Queries run: (not yet filled in — author runs `scripts/intel-query.sh` commands per `.claude/skills/dual-tpr/compose-intel-summary.md` and records them here)

  Results summary (≤500 chars): (not yet filled in)

  See `.claude/skills/dual-tpr/compose-intel-summary.md` for the query protocol (SSOT).
  ```
  The stub is DELIBERATELY incomplete — header present but no `scripts/intel-query.sh` literal invocation in the body — so §06.2's anti-performative-ritual detection classifies it as `stub` (not `complete`, not `missing`) and emits `FindingCategory.GAP` / `FindingSubtype.VALIDATION_BYPASS` with severity/outcome per §06 Design Decision 5 (`status: not-started` → `Severity.HIGH`, `Outcome.WARNING` by default) until the owner fills it.

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
  p_retrofit.add_argument("--allow-reopen", type=Path, action="append", default=[],
      help="Permits retrofit of the named status: complete section. Repeatable. Has no effect on not-started sections (those are always eligible regardless of reviewed flag).")
  ```
  Map to `run_retrofit(...)`; exit 0 on success, 1 on any `Severity.HIGH` report entry (e.g., a parse-error file that could not be loaded).

- [ ] **Write `tests/plan-audit/test_retrofit_recon.py`** covering the full matrix — (section status: `not-started` / `in-progress` / `complete`) × (body shape: no-recon-block / stub-recon-block / complete-recon-block) × (reviewed flag: `false` / `true`) × (mode: dry-run / apply):
  - `not_started_no_block_reviewed_false_apply` → inserts stub; reviewed stays false; 0 HIGH findings
  - `not_started_no_block_reviewed_true_apply` → inserts stub; reviewed stays true (PRESERVED — retrofit is additive, does NOT invalidate prior review); 0 HIGH findings
  - `not_started_stub_block_apply` → skip (stub already present); 0 edits
  - `not_started_complete_block_apply` → skip (already satisfies validator); 0 edits
  - `in_progress_no_block_apply` → skip (not in retrofit target set); reporter notes "in-progress with missing recon — validator emits Severity.MEDIUM, Outcome.WARNING"
  - `complete_no_block_apply` → SILENTLY SKIPPED (complete sections are exempt in bulk mode; `continue`); reporter notes "complete — exempt"; file unchanged; 0 HIGH findings
  - `complete_no_block_apply_with_allow_reopen` → inserts stub; reviewed preserved; 0 HIGH findings (--allow-reopen explicitly unlocks this complete section)
  - `bulk_mixed_statuses_silent_skip` → corpus with not-started, in-progress, and complete sections; only not-started sections receive stubs; complete and in-progress sections are silently skipped; 0 HIGH findings; no abort
  - `not_started_no_block_reviewed_false_dry_run` → no file writes; `RetrofitReport.planned_edits` populated
  - `not_started_no_block_reviewed_true_dry_run` → no file writes; planned_edits populated; reviewed flag noted as true but NOT flagged as error
  - Plan filter: `plan_filter="plans/query-intel-adoption"` limits enumeration to this plan only
  - Exit-code tests: `main(["retrofit-recon", "--dry-run"])` → 0 on clean corpus (complete sections silently skipped, not an error); 1 only when a parse-error file produces a `Severity.HIGH` report entry

  Follow the fixture conventions already used in `tests/plan-audit/test_plan_corpus.py`.

- [ ] **Subsection close-out (09.1)** — MANDATORY before starting 09.2:
  - [ ] `retrofit_recon.py` lands; subcommand registered; all 10 matrix tests pass via `pytest tests/plan-audit/test_retrofit_recon.py`
  - [ ] `./test-all.sh` green (no regressions in existing plan-audit or Rust test suites)
  - [ ] `python -m scripts.plan_corpus retrofit-recon --dry-run` on the full active corpus prints a report whose target count matches the count derived from `discover_corpus(root).plan_sections.keys()` filtered by `load_and_validate(path)` → `ValidatedFile.data.get('status') == 'not-started'` — frontmatter-aware, `FileClass.PLAN_SECTION`-scoped (not a grep-based comparator, which would pick up roadmap/bug-tracker sections)
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

- [ ] All 09.1 close-out items complete (retrofit_recon.py + subcommand + matrix tests + complete-section silent-skip + --allow-reopen opt-in unlock + dry-run equivalence)
- [ ] All 09.2 close-out items complete (§06/§07/§08/§09 recon blocks filled; §01-§05 untouched; discover coverage 4/4 on this plan's not-started slice)
- [ ] `python -m scripts.plan_corpus discover` reports 100% recon-block **PRESENCE** for the `not-started` slice of every active plan corpus-wide (stubs count as "present" — this is the scaffolding goal, not the quality goal; see NOTE below)
- [ ] `./test-all.sh` green (including new plan-audit tests)
- [ ] `python -m scripts.plan_corpus check plans/` returns exit 0 (no Outcome.ERROR recon findings — stubs emit `Outcome.WARNING` which does not gate the exit code in default mode; only `--strict-recon` would escalate `not-started` stubs to `Outcome.ERROR`)

  **NOTE — §09 is a scaffolding pass, not a quality gate.** `retrofit_recon.py` injects stub recon blocks so no section is silently missing reconnaissance infrastructure. A stub is classified by §06's validator as `GAP:VALIDATION_BYPASS` at `Severity.HIGH` / `Outcome.WARNING` (default) — it is printed but does NOT gate exit code. Stub elimination (filling stubs with real reconnaissance) happens organically as each section enters implementation via `/continue-roadmap`, which runs the §06 template and requires the author to fill the block before subsection close-out. The success criterion "100% PRESENCE" is explicitly the scaffolding goal; quality (stub vs. complete) is tracked per-section by `check`, not by this plan's §09.N gate.
- [ ] **Plan sync**:
  - [ ] Section frontmatter `status: not-started` → `complete`
  - [ ] `00-overview.md` Mission Success Criteria §09 checkbox checked; Quick Reference §09 row → Complete
  - [ ] `index.md` §09 status updated
- [ ] **Stub burn-down tracking**: Each injected stub is filled organically during `/continue-roadmap` execution of its owning section (the `## Intelligence Reconnaissance` template in `plan-schema.md` instructs the implementer to run queries). No separate burn-down section needed — the scaffolding resolves itself per-section as each section enters implementation. Verify: after §09 completes, run `python -m scripts.plan_corpus check plans/ 2>&1 | grep VALIDATION_BYPASS | wc -l` and log the count as the burn-down baseline.
- [ ] `/tpr-review` passed — clean dual-source pass (watch-items: status-gated severity gates correctly; `status: complete` sections are SILENTLY SKIPPED in bulk mode and only written when explicitly listed via `--allow-reopen`; `reviewed: true` on `not-started` sections is preserved unchanged; meta-dogfood did not touch §01-§05; no historical-fiction narratives in any recon block)
- [ ] `/impl-hygiene-review` passed — verify `retrofit_recon.py` reuses `discovery.load_and_validate` (no parallel enumeration); verify the stub block shape is a single source of truth shared with §06.2's detector
- [ ] `/improve-tooling` section-close sweep — cross-subsection patterns only
- [ ] `/sync-claude` section-close doc sync
- [ ] `diagnostics/repo-hygiene.sh --check` → clean
