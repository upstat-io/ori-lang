---
section: "06"
title: "Plan schema — mandatory Intelligence Reconnaissance block + validator"
status: not-started
reviewed: false
goal: "Every new plan section carries an unnumbered `## Intelligence Reconnaissance` block — queries run + ≤500-char results summary + date — AND `python -m scripts.plan_corpus check` enforces it with a status-gated severity model: `status: not-started` missing recon → `Severity.HIGH`; `status: in-progress` → `Severity.MEDIUM` (escalating on body edit); `status: complete` → exempt. Scope is `FileClass.PLAN_SECTION` only; roadmap and bug-tracker sections keep their existing shape. Retrofit of `not-started` sections is handled by §09."
success_criteria:
  - "`.claude/skills/create-plan/plan-schema.md` Section File Template includes an unnumbered `## Intelligence Reconnaissance` block after the section framing (Goal / Context / Reference / Depends on) and BEFORE `## {NN}.1`; the block does NOT appear in the `sections:` frontmatter list"
  - "`.claude/skills/create-plan/SKILL.md:808` cites `plan-schema.md` as the single SSOT for section-level structural invariants — no re-assertion of the `{NN}.1, {NN}.2, ...` close-out structure"
  - "Explicit format-coupling contract: plan-resident recon summaries (§06) and `pre-review-intel.sh` hook-injected summaries (§07) share the exact §03 composition grammar — same helper source (`.claude/skills/dual-tpr/compose-intel-summary.md`), same ≤500-char bound, same `[ori]` / `[repo#N]` citation markers, same fallback string `\"Intelligence reconnaissance skipped: graph unavailable\"`. Drift between §03/§06/§07 is a DRIFT:scattered-knowledge finding."
  - "`python -m scripts.plan_corpus check` (the correct invocation per `scripts/plan_corpus/__main__.py`) distinguishes WARNING-level from ERROR-level findings via a new exit-code policy: exit 0 when all findings map to WARNING (`Severity.LOW` / `Severity.MEDIUM`); exit 1 when any finding is ERROR (`Severity.HIGH` / `Severity.CRITICAL`). `--strict-recon` escalates missing-recon findings on `status: not-started` PLAN_SECTION files from WARNING to ERROR."
  - "Validator pipeline carries `body_text` end-to-end: `scripts/plan_corpus/schema.py` `FILE_CLASS_META` entry for `PLAN_SECTION` declares a body-level validator phase in addition to the frontmatter validator; `scripts/plan_corpus/discovery.py:load_and_validate` (per `ValidatedFile`) passes `body_text` into that phase. The refactor choice (new signature vs. parallel phase) is documented in §06.2 with rationale."
  - "Anti-performative-ritual detection: validator flags as `GAP:validation-bypass` any recon block that is (a) header-present-but-empty / whitespace-only, (b) body containing only placeholder tokens (`TBD`, `none`, `n/a`, `todo`, `(empty)`, ellipsis-only), or (c) body with NO concrete citation (no `[ori]` marker, no `[rust#` / `[swift#` / `[koka#` / similar cross-repo citation marker, no literal fallback string)."
  - "`python -m scripts.plan_corpus discover` reports per-plan recon coverage grouped by section `status` — the reporter §09 consumes to measure retrofit completion against `not-started` sections only."
  - "Matrix tests in `tests/plan-audit/test_recon_block.py` cover the full (FileClass × body-shape × severity-mode) matrix: (PLAN_SECTION / ROADMAP_SECTION / BUG_TRACKER_SECTION / FIX_BUG) × (present-with-content / present-with-placeholder / present-empty / absent) × (default / `--strict-recon`) — positive pins AND negative pins (ROADMAP_SECTION / BUG_TRACKER_SECTION / FIX_BUG are EXEMPT; missing recon on them produces zero findings)."
inspired_by:
  - "Phase 2 dual-source convergence (Codex + Gemini, 2026-04-14): §06.2's 'extend validator' wording understated the work — validator architecture changes are required (body_text propagation, severity→outcome model, anti-stub detection), not a minor extension"
  - "CLI entrypoint SSOT: `scripts/plan_corpus/__main__.py` per CLAUDE.md line 167; earlier draft called `scripts/plan_corpus.py check` — no such file exists"
  - "Existing UNNUMBERED structural blocks in the Section File Template (Goal, Context, Reference implementations, Depends on) — the recon block matches this pattern, NOT the numbered `{NN}.X` subsection pattern"
  - "23 existing plan sections already use `## {NN}.0` for Prerequisites / Preflight / Goal (grep `^## \\d+\\.0\\s` under plans/) — the unnumbered design avoids collision; additionally, roadmap sections ALREADY use `.0` for substantive content and are therefore EXPLICITLY EXEMPT from the recon mandate"
  - "`plan-schema.md` Fix-Bug template (`1. Root Cause / 2. TDD / ...`) does not match the `{NN}.X` numbering pattern; `FIX_BUG` and `BUG_TRACKER_SECTION` file classes are EXEMPT from the recon mandate"
  - "TPR findings codex-029 [high], 030 [medium], gemini-005 [medium]; 2026-04-14 /tp-help blind-spot analysis converged on: unnumbered block design, warning/error exit channel, status-gated severity, anti-performative-ritual detection, format coupling with §07"
depends_on: ["03"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "06.1"
    title: "Plan-schema + create-plan SKILL.md edits (unnumbered recon block; SSOT cite; §03/§07 format-coupling contract)"
    status: not-started
  - id: "06.2"
    title: "plan_corpus validator: body_text propagation, warning/error outcome model, status-gated severity, anti-stub detection, matrix tests"
    status: not-started
  - id: "06.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "06.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Plan schema — mandatory Intelligence Reconnaissance block + validator

**Status:** Not Started
**Goal:** Every new plan section carries an unnumbered `## Intelligence Reconnaissance` block — placed after the section framing (Goal / Context / Reference / Depends on) and BEFORE `## {NN}.1` — that records the literal graph queries the author ran and a bounded ≤500-char results summary with date. `python -m scripts.plan_corpus check` enforces it with a WARNING/ERROR outcome model gated by section `status`. Retrofit of `not-started` sections across the active corpus is §09's work.

**Context:** Plan sections today are the primary unit of compiler work but carry no required reconnaissance. Making recon schema-mandatory turns the graph from "something you remember" into "something the plan corpus enforces." §06 lands the template, the SSOT cite, and the validator architecture; §09 consumes §06 to backfill `not-started` sections. TPR codex-029 (high) is the root driver; gemini-005 concurs.

**Design decision 1 — scope is `FileClass.PLAN_SECTION` only.** `scripts/plan_corpus/schema.py:60`'s `FileClass` enum carries four section-like classes: `PLAN_SECTION`, `ROADMAP_SECTION`, `BUG_TRACKER_SECTION`, `FIX_BUG`. The recon mandate applies to `PLAN_SECTION` ONLY:
  - `ROADMAP_SECTION` files already use `## {NN}.0` for substantive content (e.g., `plans/roadmap/section-00-parser.md:67`). Overlaying an unnumbered recon block on top is possible, but roadmap sections are curated differently (many are in-flight, some are long-frozen process documents). Excluding them is both cheaper and correct.
  - `BUG_TRACKER_SECTION` and `FIX_BUG` use a separate template (`1. Root Cause / 2. TDD / 3. Implementation / ...` per `plan-schema.md:789`). The `{NN}.X` numbering does not apply, and `fix-BUG-*.md` files already run the full reconnaissance workflow through `/fix-bug` Phase 1 — duplicating it in a block is redundant.
  - Both the validator (§06.2) and the retrofit tool (§09.1) MUST filter on `file_class == FileClass.PLAN_SECTION`. Negative-pin tests ensure no false-positive findings fire on the exempt classes.
  - `reroute: true` and `parallel: true` are INDEX-level properties, not file classes — the ordinary section files under such indices are still `PLAN_SECTION` and IN scope.

**Design decision 2 — unnumbered block, NOT `{NN}.0` subsection.** An earlier draft proposed `## {NN}.0 Intelligence Reconnaissance` as a numbered subsection. 23 existing sections already use `## {NN}.0` for Prerequisites / Preflight / Goal content — mandating `{NN}.0 Intelligence Reconnaissance` would collide with those semantics or force mass renumbering. The unnumbered-block design — treating recon like the existing `Goal` / `Context` / `Reference implementations` unnumbered blocks — avoids the collision entirely and matches the structural-not-indexed nature of reconnaissance. `sections:` frontmatter lists only numbered `{NN}.X` subsections; the recon block does not appear there.

**Design decision 3 — methodology + results, NOT raw SSOT expansion.** The block stores: (1) the literal `scripts/intel-query.sh` commands the author ran, (2) a ≤500-char results summary, (3) the date. It does NOT `@`-include the SSOT protocol. `@`-includes are expanded by the harness at skill/command prompt-expansion time but NOT in plan-file markdown — `grep -rEn '^@\.' plans/` returns zero hits today. Embedding `@.claude/skills/dual-tpr/compose-intel-summary.md` in a plan body produces a dead literal; embedding the expanded SSOT protocol would be a `LEAK:algorithmic-duplication` — the exact pathology §03 just fixed for 18 consumers. The recon block records WHAT was done; the SSOT records HOW — one-way reference, no content copy.

**Design decision 4 — WARNING vs ERROR as a finding-outcome axis, not just severity.** `scripts/plan_corpus/__main__.py:37` currently does `return 1 if all_findings else 0`, so any finding fails `check` regardless of severity. And `scripts/plan_corpus/types.py:48` defines `Severity` as `LOW / MEDIUM / HIGH / CRITICAL` — not `WARNING / ERROR`. Severity alone is insufficient for a gate because a `MEDIUM` recon stub in a `not-started` section and a `MEDIUM` recon stub in an `in-progress` section have different operational meanings. §06.2 therefore introduces a distinct `Outcome` axis (`WARNING` / `ERROR`) derived from `Severity + context`: by default `HIGH` / `CRITICAL` → ERROR and `LOW` / `MEDIUM` → WARNING; `--strict-recon` escalates missing-recon warnings on `status: not-started` PLAN_SECTION files to ERROR. The exit policy is `exit 1 iff any finding has Outcome == ERROR`.

**Design decision 5 — status-gated severity, not A/B/C operator menu.** Combined with the corpus already carrying a `status` field, the validator reads each PLAN_SECTION's `frontmatter.status` and applies: `not-started` missing recon → `Severity.HIGH`; `in-progress` missing recon → `Severity.MEDIUM` (escalating to `HIGH` on the next body edit via a `check --on-edit` mode or lefthook wiring); `complete` → exempt. This is the objective, data-driven severity model that replaces §06's earlier A/B/C retrofit menu (now gone; retrofit is §09).

**Design decision 6 — `--strict-recon` as a CLI flag, not a corpus-wide frontmatter.** An earlier draft proposed a per-plan `strict_recon: bool` on `00-overview.md`. That model requires the `PlanSectionSchema` / `OverviewSchema` to accept a new frontmatter field (currently `schema.py:264` rejects unknown keys) and creates corpus-wide state that's hard to preview. Making it a CLI flag keeps policy at the invocation site: CI can pin `--strict-recon` for `not-started` sections; local runs default to warnings-only. No schema widening needed for policy — §06 does NOT add any frontmatter field to `OverviewSchema`.

**Design decision 7 — format coupling with §03 and §07 is a CONTRACT, not a convention.** `00-overview.md:144` already states "§07 hook output format MUST match §03's bounded summary template exactly." §06 extends this into a three-way coupling: §03 helper (source), §06 recon block (plan-body artifact), §07 hook injection (runtime prompt artifact) — all three share the same ≤500-char bound, the same `[ori]` / `[repo#N]` citation grammar, and the same fallback-string `"Intelligence reconnaissance skipped: graph unavailable"`. Drift among the three is a `DRIFT:scattered-knowledge` finding. §06.1's template text names this contract explicitly; §06.2's anti-stub detector enforces the citation-grammar half; §07's plan file cites §06's contract back.

**Reference implementations:**
- **Ori** `.claude/skills/create-plan/plan-schema.md` existing Section File Template (lines 236-508) — §06.1 edit target
- **Ori** `.claude/skills/create-plan/SKILL.md:808` — second SSOT-ish surface that re-asserts subsection structure; §06.1 cites plan-schema.md instead
- **Ori** `scripts/plan_corpus/` Python package — `types.py` (Severity enum), `parser.py` (frontmatter split + body_offset), `schema.py` (`FILE_CLASS_META` + `validate`), `schemas.py` (`PlanSectionSchema` strict allowlist), `discovery.py` (`load_and_validate` / `ValidatedFile`), `__main__.py` (`check` / `discover` / `docgen` subcommands) — §06.2 edit targets
- **Ori** `.claude/skills/dual-tpr/compose-intel-summary.md` — the SSOT helper the template references (no `@`-include in plan bodies)
- **Ori** 23 existing `## {NN}.0` headers in `plans/` — motivation for the unnumbered design
- **Ori** `plans/roadmap/section-00-parser.md:67` — roadmap-side `.0` collision source; evidence for the PLAN_SECTION-only scope decision

**Depends on:** Section 03 (the recon block describes running the §03 SSOT queries).

---

## Intelligence Reconnaissance

Queries run 2026-04-14 (during /review-plan of this section):

- `scripts/intel-query.sh status` — graph available (191K Ori symbols indexed, 10 reference compilers with 505K CALLS edges and 298K issues)
- `scripts/intel-query.sh --human search "plan schema validation" --limit 5` — surfaced 5 external-repo schema-notation issues (zig ZON, go JSON schema, lean4 lakefile toml schema); low direct relevance for in-repo meta-tooling
- `scripts/intel-query.sh --human file-symbols "scripts/plan_corpus/schema" --repo ori` — zero results (Python code is NOT indexed in the Ori code-symbol graph; the code graph is Rust-only). Walked the 9-module `scripts/plan_corpus/` package directly via Read.
- `scripts/intel-query.sh --human callers "validate" --repo ori` — surfaced Rust-side callers (`compiler/ori_types` / `ori_arc`); disambiguated via manual Read that `scripts/plan_corpus/schema.py:487 validate(fc, data, path)` takes only frontmatter — body_text is produced in `discovery.py:load_and_validate` and currently NEVER reaches validators.
- `grep -rEn '^## \d+\.0\s' plans/` — 23 existing plan sections use `## {NN}.0` for Prerequisites / Preflight / Goal (the decisive finding that forced the unnumbered design)
- `grep -rEn '^@\.' plans/` — zero hits; confirms plan files are NOT harness-expanded, so `@`-includes in plan bodies would be dead literals

Results summary (≤500 chars) [ori]: Graph indexes Rust+reference repos but NOT `scripts/plan_corpus/` Python. Direct Read confirmed: `validate(fc, data, path)` takes only frontmatter; `body_text` is split in `discovery.py:load_and_validate` into `ValidatedFile` but never propagated — body-level validation requires a new phase, not a parameter tweak. Grep surfaced the load-bearing finding: `{NN}.0` slot occupied 23x (PLAN_SECTION) and roadmap `.0` is substantive content — forcing unnumbered-block design and PLAN_SECTION-only scope. Severity enum is LOW/MEDIUM/HIGH/CRITICAL (not WARNING/ERROR) — outcome model is a new axis.

Subsystem-mapping note: no preset matches meta-tooling (`scripts/plan_corpus/`, `.claude/skills/create-plan/`). Used `search` fallback per `.claude/rules/intelligence.md` §Subsystem Mapping. The template text in §06.1 explicitly addresses this fallback case for non-compiler plans.

See `.claude/skills/dual-tpr/compose-intel-summary.md` for the full query protocol (SSOT — do NOT inline; this block records what was done, not the protocol itself).

---

## 06.1 Plan-schema + create-plan SKILL.md edits

**File(s):** `.claude/skills/create-plan/plan-schema.md`, `.claude/skills/create-plan/SKILL.md`

Two surfaces describe section-level structural invariants today: `plan-schema.md` (Section File Template + "MANDATORY SUBSECTION STRUCTURE" HTML comment at lines 315-326) and `create-plan/SKILL.md:808` (which independently hardcodes `"EVERY subsection ({NN}.1, {NN}.2, ...)"`). Updating only one creates `DRIFT:scattered-knowledge`. §06.1 edits both — plan-schema.md as authoritative SSOT, SKILL.md as pointer.

- [ ] **plan-schema.md — insert unnumbered recon block in the Section File Template.** After the `**Depends on:** Section {NN} ({why}).` line (currently line 311) and BEFORE the `---` separator preceding `## {NN}.1`, add a `---` separator and the following unnumbered block example:

  ```markdown
  ---

  ## Intelligence Reconnaissance

  Queries run {YYYY-MM-DD}:

  - `scripts/intel-query.sh --human <preset>` — {one-line outcome}. For compiler sections use the matching preset per `.claude/rules/intelligence.md` §Subsystem Mapping (`ori-arc`, `ori-inference`, `ori-codegen`, `ori-patterns`, `ori-diagnostics`). For non-compiler plans (meta-tooling, docs, build scripts) use `search "<key terms>"` — no preset applies.
  - `scripts/intel-query.sh --human file-symbols "<path-fragment>" --repo ori` — {one-line outcome} (skip for non-Rust targets; the Ori code-symbol index is Rust-only today)
  - `scripts/intel-query.sh --human callers "<symbol>" --repo ori` — {one-line outcome} (blast radius for every public API the section changes)
  - `scripts/intel-query.sh --human similar "<symbol>" --repo rust,swift,koka --limit 5` — {one-line outcome} (cross-repo prior art for design decisions)

  Results summary (≤500 chars) [ori]: {bounded paragraph citing blast radius, cross-repo prior art, relevant symbols. Use `[ori]` for Ori-repo claims and `[rust#N]` / `[swift#N]` / `[koka#N]` / etc. for cross-repo citations — the same grammar used by `compose-intel-summary.md` Step D and by §07's hook injection. If the graph is unavailable, use the exact fallback string `"Intelligence reconnaissance skipped: graph unavailable"` — do NOT silently skip the block.}

  See `.claude/skills/dual-tpr/compose-intel-summary.md` for the full query protocol (SSOT — do NOT `@`-include in plan files; plan markdown is not harness-expanded, so the include would be a dead literal).
  ```

  **Placement requirement:** AFTER all section framing (Goal, Success Criteria, Context, Reference implementations, Depends on) and BEFORE the first numbered subsection (`## {NN}.1`). The block is structurally parallel to the framing blocks — not a subsection.

  **Format-coupling contract:** the block's ≤500-char summary format, `[ori]` / `[repo#N]` citation grammar, and `"Intelligence reconnaissance skipped: graph unavailable"` fallback string are IDENTICAL to §03's `compose-intel-summary.md` output and §07's `pre-review-intel.sh` hook-injected output. Drift among the three is a `DRIFT:scattered-knowledge` finding (see §06 Design decision 7).

- [ ] **plan-schema.md — replace the "MANDATORY SUBSECTION STRUCTURE" comment (currently lines 315-326) with "MANDATORY SECTION STRUCTURE"** covering both load-bearing invariants:

  ```markdown
  <!-- == MANDATORY SECTION STRUCTURE ==
  Every PLAN_SECTION file has TWO mandatory structural features that are
  NOT captured by the numbered {NN}.X subsection sequence alone:

  1. **Unnumbered `## Intelligence Reconnaissance` block** — placed after
     the section framing (Goal / Success Criteria / Context / Reference
     implementations / Depends on) and BEFORE `## {NN}.1`. Records the
     literal `scripts/intel-query.sh` commands the author ran, a
     ≤500-char results summary (using the same `[ori]` / `[repo#N]`
     citation grammar as `.claude/skills/dual-tpr/compose-intel-summary.md`
     Step D), and the date. Coexists with §07's runtime hook: the hook
     skips injection when this block is present and non-stub. Enforced
     by `python -m scripts.plan_corpus check` — the validator gates
     severity on the section's `status` field:
       - status: not-started → Severity.HIGH (ERROR under --strict-recon)
       - status: in-progress → Severity.MEDIUM (escalates on body edit)
       - status: complete    → exempt

  2. **Per-subsection close-out blocks** — EVERY numbered subsection
     ({NN}.1, {NN}.2, ...) MUST end with a `**Subsection close-out**`
     block containing the per-subsection `/improve-tooling`
     retrospective and `/sync-claude` doc sync BEFORE the `---`
     separator. Pain memory decays within hours, so the look-back fires
     while the debugging journey is hot — NOT at section close.

  SCOPE: The recon-block mandate applies ONLY to FileClass.PLAN_SECTION
  (files matching `plans/*/section-*.md` excluding `plans/roadmap/` and
  `plans/bug-tracker/`). Roadmap sections already use `## {NN}.0` for
  substantive content; fix-BUG-*.md files use a separate `1. Root Cause
  / 2. TDD / ...` template that runs recon through /fix-bug Phase 1.

  Plans that omit either feature will fail `/continue-roadmap`
  validation. This comment is the only authoritative enumeration of
  section-level structural invariants; `create-plan/SKILL.md` cites
  this schema file and does NOT re-assert the invariants
  (per `impl-hygiene.md` §SSOT).
  -->
  ```

- [ ] **plan-schema.md — `sections:` frontmatter example stays unchanged.** The recon block is UNNUMBERED and does NOT appear in the `sections:` list. Add a one-line comment near the `sections:` example:

  ```yaml
  # Note: Intelligence Reconnaissance is an UNNUMBERED structural block
  # (like Goal, Context, Reference implementations, Depends on). It does
  # NOT appear in this `sections:` list — only numbered {NN}.X subsections do.
  sections:
    - id: "{NN}.1"
      ...
  ```

- [ ] **create-plan/SKILL.md:808 — replace re-assertion with citation.** Current text: `"**Per-subsection close-out blocks** — EVERY subsection ({NN}.1, {NN}.2, ...) MUST end with a 'Subsection close-out' block ..."`. New text:

  ```markdown
  - **Section-level structural invariants** — see `.claude/skills/create-plan/plan-schema.md` "MANDATORY SECTION STRUCTURE" HTML comment for the two authoritative invariants: (1) unnumbered `## Intelligence Reconnaissance` block placed between section framing and `## {NN}.1` (PLAN_SECTION only; roadmap and bug-tracker sections are exempt); (2) per-subsection close-out blocks containing `/improve-tooling` + `/sync-claude` calls. `plan-schema.md` is the SSOT per `impl-hygiene.md` §SSOT; SKILL.md does NOT re-state the invariants — any drift between the two surfaces is a `DRIFT:scattered-knowledge` finding.
  ```

- [ ] **Verify via `grep` that no other `.claude/` file independently re-asserts subsection structure.** Command: `grep -rn "EVERY subsection\|{NN}.1, {NN}.2" .claude/`. Expected post-edit: only `plan-schema.md` contains the authoritative assertion; SKILL.md contains only the citation. If additional re-assertion sites exist, update each to cite plan-schema.md. Document findings in the subsection close-out.

- [ ] **Subsection close-out (06.1)** — MANDATORY before starting 06.2:
  - [ ] Template changes land; `plan-schema.md` renders with the unnumbered `## Intelligence Reconnaissance` block in the canonical example AND the scope note (PLAN_SECTION only) in the MANDATORY SECTION STRUCTURE comment
  - [ ] `create-plan/SKILL.md:808` updated to cite plan-schema.md rather than re-state invariants, including the PLAN_SECTION-only scope note
  - [ ] Format-coupling contract text is present in both the template block and the MANDATORY SECTION STRUCTURE comment; the `[ori]` / `[repo#N]` citation grammar and fallback string are named verbatim
  - [ ] `grep -rn "EVERY subsection\|{NN}.1, {NN}.2" .claude/` shows only plan-schema.md as authoritative site
  - [ ] `python -m scripts.plan_corpus check plans/query-intel-adoption/section-06-plan-schema-recon.md` still returns 0 (this file's own recon block above is already non-stub; 06.1 changes do not falsely trigger the not-yet-landed 06.2 validation)
  - [ ] Update `06.1` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 06.1** — was editing two SSOT surfaces in lockstep painful enough to warrant a small `scripts/` helper that diff-greps for subsection-structure re-assertions? If yes, add it. Commit via `build(tooling): add X — surfaced by query-intel-adoption/section-06.1 retrospective`. If no gaps, document: `"Retrospective 06.1: no tooling gaps — plan-schema.md and SKILL.md edits were mechanical."`
  - [ ] **Run `/sync-claude` on 06.1** — `plan-schema.md` is the SSOT for plan shape. Verify CLAUDE.md §Commands "Plan corpus" bullet (line ~167) still matches the invocation form (`python -m scripts.plan_corpus check`). Verify no `.claude/rules/*.md` file references a pre-package `scripts/plan_corpus.py` path or the old `{NN}.0` proposal.
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check` → clean.

---

## 06.2 plan_corpus validator: body_text propagation, warning/error outcome model, status-gated severity, anti-stub detection, matrix tests

**File(s):** `scripts/plan_corpus/types.py`, `scripts/plan_corpus/__main__.py`, `scripts/plan_corpus/schema.py`, `scripts/plan_corpus/discovery.py`, `tests/plan-audit/test_recon_block.py` (new), `tests/plan-audit/fixtures/` (new fixtures)

Four implementation gaps block the enforcement contract:

1. **Body-text does not reach validators.** `scripts/plan_corpus/schema.py:487` — `validate(fc, data, path)` takes only frontmatter. `scripts/plan_corpus/discovery.py:268 load_and_validate` splits body text into `ValidatedFile.body_text` but never passes it into the validator dispatch. Body-level recon detection requires plumbing or a parallel phase.
2. **No WARNING/ERROR outcome model.** `scripts/plan_corpus/__main__.py:37` does `return 1 if all_findings else 0`. Severity distinctions are not expressible as exit codes.
3. **No status-gated severity.** Validators receive frontmatter `data` but don't branch on `data.get("status")` when emitting recon-block findings.
4. **No anti-performative-ritual detection.** Nothing today rejects "header-present, body-empty" or "header-present, no citation" stubs.

All four must be fixed together; any one alone leaves the enforcement path broken.

- [ ] **Fix CLI entrypoint DRIFT.** Grep the plan corpus and the `.claude/` tree for legacy references to `scripts/plan_corpus.py` (with the `.py` suffix — the file does NOT exist; the package is invoked as `python -m scripts.plan_corpus`). Command: `grep -rn "scripts/plan_corpus\.py" .claude/ plans/ docs/ scripts/`. Replace every hit with `python -m scripts.plan_corpus`. Include CLAUDE.md §Commands and this plan's own success-criteria lines. Capture the count of replaced occurrences in the close-out note.

- [ ] **Add `Outcome` enum to `scripts/plan_corpus/types.py`.** Distinct axis from `Severity`:
  ```python
  class Outcome(enum.Enum):
      """Gate outcome — distinct from Severity. Gate behavior answers
      'does this fail the check?' independently of how severe it is."""
      WARNING = "warning"   # printed, does NOT affect exit code
      ERROR = "error"       # printed AND forces exit 1
  ```
  Add `outcome: Outcome` to the `Finding` dataclass with a default derived from severity (`HIGH`/`CRITICAL` → `ERROR`; `LOW`/`MEDIUM` → `WARNING`). Update `to_markdown` / `to_json` to render the outcome channel. Do NOT rename the `Severity` enum — `LOW`/`MEDIUM`/`HIGH`/`CRITICAL` is the established taxonomy and is consumed elsewhere in the package.

- [ ] **Rewrite the exit-code policy in `scripts/plan_corpus/__main__.py`.** Replace:
  ```python
  return 1 if all_findings else 0
  ```
  with:
  ```python
  errors = [f for f in all_findings if f.outcome == Outcome.ERROR]
  return 1 if errors else 0
  ```
  Keep print/JSON output for ALL findings including warnings. Update `check` help text: `'Validate a file or directory (exits 1 only on findings with Outcome.ERROR; WARNING findings are printed but non-gating)'`. Add a `--strict-recon` flag: when set, any missing/stub recon-block finding on a `status: not-started` PLAN_SECTION is promoted from WARNING to ERROR (regardless of the Severity default).

- [ ] **Refactor `FILE_CLASS_META` to carry a body-level validator in addition to the frontmatter validator.** Two viable refactor shapes — §06.2 picks shape (a) with rationale documented inline:

  - (a) **Extend `FileClassMeta` with a `body_validator: Callable[[dict, str, Path], list[Finding]] | None` field** (None for classes without body-level checks). `validate(file_class, data, body_text, path)` calls both validators sequentially and concatenates findings. `discovery.load_and_validate` already produces `body_text`; the call site passes it through. Rationale: one dispatch mechanism, explicit per-class opt-in, no parallel phase. (Shape (b) — a post-schema body-check phase registered separately — is rejected because it duplicates the dispatch plumbing and would drift from the class-keyed registry that docgen already relies on.)
  - Update the `validate()` signature and EVERY call site (`discovery.py`, any direct callers). Use `rg` to find all call sites BEFORE editing; list them in the commit message. This is a `- [ ]` item, not a deferral — signature propagation IS the work.
  - For classes with `body_validator = None` (ROADMAP_SECTION, BUG_TRACKER_SECTION, FIX_BUG, the various overview / index classes), the extended dispatch is a no-op. Negative-pin tests (§06.2 matrix) confirm zero findings fire.

- [ ] **Implement `_check_intel_recon_block(data: dict, body_text: str, path: Path) -> list[Finding]`** in `scripts/plan_corpus/schema.py`. Attach it as the `body_validator` for `FileClass.PLAN_SECTION` only. Detection rules:

  - **Missing block** — no `^## Intelligence Reconnaissance\s*$` header found via `re.search(..., re.MULTILINE)` on `body_text`.
    - `status: not-started` → `Severity.HIGH`, `Outcome.WARNING` by default, `Outcome.ERROR` under `--strict-recon`
    - `status: in-progress` → `Severity.MEDIUM`, `Outcome.WARNING`
    - `status: complete` → 0 findings (exempt)
    - `FindingCategory.GAP`, message cites `.claude/skills/dual-tpr/compose-intel-summary.md` as the SSOT protocol

  - **Stub / performative-ritual block** (header present but body fails one or more concrete-content checks):
    - Block body is empty / whitespace-only between the header and the next `^## ` (or end-of-file), OR
    - Block body contains only placeholder tokens. Tokens (case-insensitive, whole-token match): `TBD`, `none`, `n/a`, `todo`, `(empty)`, ellipsis-only (`...`, `…`), OR
    - Block body contains NO concrete citation marker: no literal `[ori]`, no cross-repo citation marker matching `\[(?:rust|swift|go|koka|lean4|gleam|elm|roc|zig|ts|typescript)#\d+\]`, AND no literal fallback string `"Intelligence reconnaissance skipped: graph unavailable"`, OR
    - Block body pastes the literal `@.claude/skills/dual-tpr/compose-intel-summary.md` directive verbatim without a condensed summary paragraph following it (the `@`-include is a SOURCE for Claude's prompt, NOT a substitute for the plan-resident snapshot)
    - Severity / outcome mapping identical to "missing block" above
    - `FindingCategory.GAP`, subtype string `"validation-bypass"`, message names which check failed

  - **Complete block** — header present AND body passes all concrete-content checks → empty list (no findings)

  Block-body extraction: slurp from the line after the header to the next `^## ` or end-of-file. Strip whitespace and HTML comments (`<!-- ... -->`) before token / citation checks. HTML comments are metadata, not content.

- [ ] **Wire body_text through `scripts/plan_corpus/discovery.py:load_and_validate`.** `ValidatedFile` already carries `body_text`; the dispatch to `validate(...)` currently passes only frontmatter. Update the call site to pass `body_text` through per the new `validate()` signature. Use `rg 'schema\.validate\('` to find all call sites before editing.

- [ ] **Add `discover` per-plan recon-coverage reporter.** After the existing per-plan summary, print a status-grouped table — §09 consumes this table to measure retrofit completeness:
  ```
  Per-plan recon coverage:
    plans/foo/            (strict)    — not-started: 3/5 non-stub   in-progress: 1/2 non-stub   complete: 4/4 exempt
    plans/bar/                        — not-started: 0/4 non-stub   in-progress: 0/0            complete: 0/0
    plans/query-intel-adoption/       — not-started: 4/4 non-stub   in-progress: 0/0            complete: 5/5 exempt
  ```
  Source data comes from a single pass over `discovery.load_and_validate` — no second filesystem walk. `strict` annotation is printed when the `discover` command was invoked with `--strict-recon`.

- [ ] **Write the full matrix of body-level recon tests in `tests/plan-audit/test_recon_block.py`** (new file, sibling of existing `test_plan_corpus.py`). Reuse the existing fixture harness pattern.

  Matrix: (FileClass) × (body-shape) × (severity-mode). Every cell is a positive or negative pin.

  | FileClass | body-shape | status | `--strict-recon`? | Expected findings |
  |-----------|------------|--------|-------------------|-------------------|
  | PLAN_SECTION | complete (header + queries + `[ori]` citation + summary) | not-started | no | 0 |
  | PLAN_SECTION | complete | in-progress | no | 0 |
  | PLAN_SECTION | complete | complete | no | 0 |
  | PLAN_SECTION | absent | not-started | no | 1, Severity.HIGH, Outcome.WARNING |
  | PLAN_SECTION | absent | not-started | yes | 1, Severity.HIGH, Outcome.ERROR |
  | PLAN_SECTION | absent | in-progress | no | 1, Severity.MEDIUM, Outcome.WARNING |
  | PLAN_SECTION | absent | in-progress | yes | 1, Severity.MEDIUM, Outcome.WARNING (strict only escalates not-started) |
  | PLAN_SECTION | absent | complete | no | 0 (exempt) |
  | PLAN_SECTION | absent | complete | yes | 0 (exempt; strict does not override complete exemption) |
  | PLAN_SECTION | stub-empty (header, whitespace body) | not-started | no | 1, Severity.HIGH, Outcome.WARNING |
  | PLAN_SECTION | stub-placeholder ("TBD" / "none" / etc.) | not-started | no | 1 (per token) |
  | PLAN_SECTION | stub-no-citation (prose but no `[ori]` / `[repo#N]` / fallback-string) | not-started | no | 1, GAP:validation-bypass |
  | PLAN_SECTION | stub-only-@-include (directive pasted, no condensed paragraph) | not-started | no | 1, GAP:validation-bypass |
  | ROADMAP_SECTION | absent | not-started | yes | 0 (exempt — out of scope) |
  | BUG_TRACKER_SECTION | absent | not-started | yes | 0 (exempt — out of scope) |
  | FIX_BUG | absent | not-started | yes | 0 (exempt — different template) |
  | Exit code — warnings-only corpus, default mode | — | — | no | `main()` returns 0 |
  | Exit code — warnings-only corpus, `--strict-recon` with not-started missing-recon | — | not-started | yes | `main()` returns 1 |
  | Exit code — corpus with one Severity.HIGH compound finding | — | — | no | `main()` returns 1 |

  Each test asserts exact finding counts, severities, outcomes, category / subtype strings, and for exit-code tests, the `__main__.py main()` return value. Fixtures live under `tests/plan-audit/fixtures/recon_block/`; use `tests/plan-audit/test_plan_corpus.py`'s fixture conventions.

- [ ] **Subsection close-out (06.2)** — MANDATORY before section close:
  - [ ] Validator refactor + matrix tests land; all new tests pass via `pytest tests/plan-audit/test_recon_block.py`
  - [ ] `./test-all.sh` green (no regressions in existing plan-audit or Rust test suites)
  - [ ] `python -m scripts.plan_corpus check plans/query-intel-adoption/section-06-plan-schema-recon.md` returns exit 0 (this file has a non-stub recon block above)
  - [ ] `python -m scripts.plan_corpus check plans/query-intel-adoption/section-07-pre-review-intel-hook.md` returns exit 1 under `--strict-recon` with one Severity.HIGH / Outcome.ERROR finding (no recon block yet; status: not-started) — end-to-end demo of the strict path. Without `--strict-recon`: exit 0 with one WARNING finding printed.
  - [ ] `python -m scripts.plan_corpus check plans/roadmap/section-00-parser.md` returns exit 0 with ZERO recon-related findings — ROADMAP_SECTION is out of scope (negative-pin check).
  - [ ] `python -m scripts.plan_corpus check plans/bug-tracker/fix-BUG-*.md` returns exit 0 with ZERO recon-related findings across the directory — FIX_BUG is out of scope.
  - [ ] CLI-entrypoint DRIFT count from the grep step is zero post-edit
  - [ ] Update `06.2` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 06.2** — does the validator error message include a pointer to the SSOT and the format-coupling contract? Minimum text: `"Section lacks non-stub '## Intelligence Reconnaissance' block. See '.claude/skills/create-plan/plan-schema.md' MANDATORY SECTION STRUCTURE; run queries per '.claude/skills/dual-tpr/compose-intel-summary.md'; summary format must use [ori] / [repo#N] citation grammar and the '\"Intelligence reconnaissance skipped: graph unavailable\"' fallback if the graph is down."` Commit via `build(tooling): improve plan_corpus recon-block error messages — surfaced by section-06.2 retrospective`.
  - [ ] **Run `/sync-claude` on 06.2** — CLAUDE.md §Commands "Plan corpus" bullet (line ~167) describes `plan_corpus check`. Update to mention the WARNING/ERROR outcome model, status-gated severity, and `--strict-recon` flag. Verify no `.claude/rules/*.md` file contradicts the new exit-code policy.
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check` → clean.

---

## 06.R Third Party Review Findings

- None.

---

## 06.N Completion Checklist

- [ ] All 06.1 close-out items complete (plan-schema.md + SKILL.md edits; format-coupling contract; grep-clean SSOT)
- [ ] All 06.2 close-out items complete (Outcome enum; status-gated severity; body_validator dispatch; anti-stub detection; matrix tests; CLI-entrypoint DRIFT scrub)
- [ ] `./test-all.sh` green (including new plan-audit tests)
- [ ] `python -m scripts.plan_corpus check plans/query-intel-adoption/` returns exit 0 with no Outcome.ERROR findings
- [ ] **Plan sync**:
  - [ ] Section frontmatter `status: not-started` → `complete`
  - [ ] `00-overview.md` Quick Reference and mission success criteria updated (§06 checkbox checked; §09 cross-ref to the retrofit dependency)
  - [ ] `index.md` section status updated
- [ ] `/tpr-review` passed — clean dual-source pass (watch-items: SSOT drift between plan-schema.md and SKILL.md; §03/§06/§07 format coupling; PLAN_SECTION-only scope; no false-positives on ROADMAP_SECTION / BUG_TRACKER_SECTION / FIX_BUG)
- [ ] `/impl-hygiene-review` passed — verify `body_validator` is the single dispatch path for body-level checks (no shadow body-check registry); verify the recon block references the SSOT protocol rather than inlining it
- [ ] `/improve-tooling` section-close sweep — cross-subsection patterns only (per-subsection retrospectives already ran in 06.1 / 06.2)
- [ ] `/sync-claude` section-close doc sync
- [ ] `diagnostics/repo-hygiene.sh --check` → clean
