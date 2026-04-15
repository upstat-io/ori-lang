---
section: "06"
title: "Plan schema — mandatory Intelligence Reconnaissance block + validator"
status: not-started
reviewed: false
goal: "Every new plan section carries an unnumbered `## Intelligence Reconnaissance` block — queries run + ≤500-char results summary + date — AND `python -m scripts.plan_corpus check` enforces it with a status-gated severity model: `status: not-started` missing recon → `Severity.HIGH`; `status: in-progress` → `Severity.MEDIUM`; `status: complete` → exempt. Scope is `FileClass.PLAN_SECTION` only; roadmap and bug-tracker sections keep their existing shape. Retrofit of `not-started` sections is handled by §09."
success_criteria:
  - "`.claude/skills/create-plan/plan-schema.md` Section File Template includes an unnumbered `## Intelligence Reconnaissance` block after the section framing (Goal / Context / Reference / Depends on) and BEFORE `## {NN}.1`; the block does NOT appear in the `sections:` frontmatter list"
  - "`.claude/skills/create-plan/SKILL.md:808` cites `plan-schema.md` as the single SSOT for section-level structural invariants — no re-assertion of the `{NN}.1, {NN}.2, ...` close-out structure"
  - "Explicit format-coupling contract: plan-resident recon summaries (§06) and `pre-review-intel.sh` hook-injected summaries (§07) share the §03 Step D composition grammar. The §03/§06/§07 shared contract covers three invariants: (1) ≤500-char bound, (2) `[ori]`/`[repo#N]`/`[repo:path]` citation vocabulary, (3) §03 SSOT helper (`.claude/skills/dual-tpr/compose-intel-summary.md`) as the source. Exact line-level formatting may vary per consumer's rendering context (plan-body text vs. hook additionalContext injection). Graceful degradation: block omitted entirely for §07 hook — or recorded as freeform prose `\"Graph was unavailable at YYYY-MM-DD when this section was authored\"` for the plan-resident artifact — when `scripts/intel-query.sh status` returns unavailable; NO sentinel string is matched by the validator. Drift in the ≤500-char bound or citation vocabulary among §03/§06/§07 is a DRIFT:scattered-knowledge finding."
  - "`python -m scripts.plan_corpus check` (the correct invocation per `scripts/plan_corpus/__main__.py`) uses a distinct `Outcome` axis (`WARNING` / `ERROR`) that is independent of `Severity`. Severity is set by the emitter (`LOW` / `MEDIUM` / `HIGH` / `CRITICAL` — impact classification). Outcome is set by the emitter per enforcement mode, NOT auto-derived from Severity. Exit-code policy: exit 0 when no finding has `Outcome.ERROR`; exit 1 when any finding has `Outcome.ERROR`. Default mode: a `status: not-started` missing-recon finding is `Severity.HIGH` + `Outcome.WARNING`. `--strict-recon` mode: the SAME finding is `Severity.HIGH` + `Outcome.ERROR` (Severity unchanged; Outcome rewritten at Finding-construction time)."
  - "Validator pipeline carries body text end-to-end: `scripts/plan_corpus/schema.py` `FILE_CLASS_META` entry for `PLAN_SECTION` declares a body-level validator phase in addition to the frontmatter validator; `scripts/plan_corpus/discovery.py:load_and_validate(path)` (per `ValidatedFile` — real field name is `body`, per `discovery.py:212`; real frontmatter access is `vf.data[...]`, per `discovery.py:210`) passes `body` into that phase. The refactor choice (new signature vs. parallel phase) is documented in §06.2 with rationale."
  - "Anti-performative-ritual detection: validator flags as `GAP:validation-bypass` any recon block that is (a) header-present-but-empty / whitespace-only, (b) body containing only placeholder tokens (`TBD`, `none`, `n/a`, `todo`, `(empty)`, ellipsis-only), OR fails ANY of the three concrete-content requirements: (c) no literal `scripts/intel-query.sh` command line (matched via regex `\\bscripts/intel-query\\.sh\\b` — must appear in the block body), (d) no date marker in ISO format `YYYY-MM-DD` within the block, (e) no concrete citation marker (`[ori]`, `[rust#N]`, `[swift#N]`, `[koka#N]`, etc.). Missing any ONE of (c), (d), (e) triggers `GAP:validation-bypass`."
  - "`python -m scripts.plan_corpus discover` reports per-plan recon coverage grouped by section `status` — the reporter §09 consumes to measure retrofit completion against `not-started` sections only."
  - "Matrix tests in `tests/plan-audit/test_recon_block.py` cover the full (FileClass × body-shape × severity-mode) matrix: (PLAN_SECTION / ROADMAP_SECTION / BUG_TRACKER_SECTION / FIX_BUG) × (present-with-content / present-with-placeholder / present-empty / absent / present-no-query / present-no-date / present-no-citation) × (default / `--strict-recon`) — positive pins AND negative pins. Exempt-class negative pins: ROADMAP_SECTION × (absent / present-empty / present-placeholder / present-no-citation) × (default / `--strict-recon`) → ALL zero findings; BUG_TRACKER_SECTION × same → ALL zero; FIX_BUG × same → ALL zero. These confirm exempt classes produce zero findings regardless of body shape or enforcement mode."
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
**Goal:** Every new plan section carries an unnumbered `## Intelligence Reconnaissance` block — placed after the section framing (Goal / Context / Reference / Depends on) and BEFORE `## {NN}.1` — that records the literal graph queries the author ran and a bounded ≤500-char results summary with date. `python -m scripts.plan_corpus check` enforces it with a WARNING/ERROR outcome model gated by section `status`. No on-edit escalation — in-progress sections stay at `Severity.MEDIUM` / `Outcome.WARNING` regardless of edits. Retrofit of `not-started` sections across the active corpus is §09's work.

**Context:** Plan sections today are the primary unit of compiler work but carry no required reconnaissance. Making recon schema-mandatory turns the graph from "something you remember" into "something the plan corpus enforces." §06 lands the template, the SSOT cite, and the validator architecture; §09 consumes §06 to backfill `not-started` sections. TPR codex-029 (high) is the root driver; gemini-005 concurs.

**Design decision 1 — scope is `FileClass.PLAN_SECTION` only.** `scripts/plan_corpus/schema.py:60`'s `FileClass` enum carries four section-like classes: `PLAN_SECTION`, `ROADMAP_SECTION`, `BUG_TRACKER_SECTION`, `FIX_BUG`. The recon mandate applies to `PLAN_SECTION` ONLY:
  - `ROADMAP_SECTION` files already use `## {NN}.0` for substantive content (e.g., `plans/roadmap/section-00-parser.md:67`). Overlaying an unnumbered recon block on top is possible, but roadmap sections are curated differently (many are in-flight, some are long-frozen process documents). Excluding them is both cheaper and correct.
  - `BUG_TRACKER_SECTION` and `FIX_BUG` use a separate template (`1. Root Cause / 2. TDD / 3. Implementation / ...` per `plan-schema.md:789`). The `{NN}.X` numbering does not apply, and `fix-BUG-*.md` files already run the full reconnaissance workflow through `/fix-bug` Phase 1 — duplicating it in a block is redundant.
  - Both the validator (§06.2) and the retrofit tool (§09.1) MUST filter on `file_class == FileClass.PLAN_SECTION`. Negative-pin tests ensure no false-positive findings fire on the exempt classes.
  - `reroute: true` and `parallel: true` are INDEX-level properties, not file classes — the ordinary section files under such indices are still `PLAN_SECTION` and IN scope.

**Design decision 2 — unnumbered block, NOT `{NN}.0` subsection.** An earlier draft proposed `## {NN}.0 Intelligence Reconnaissance` as a numbered subsection. 23 existing sections already use `## {NN}.0` for Prerequisites / Preflight / Goal content — mandating `{NN}.0 Intelligence Reconnaissance` would collide with those semantics or force mass renumbering. The unnumbered-block design — treating recon like the existing `Goal` / `Context` / `Reference implementations` unnumbered blocks — avoids the collision entirely and matches the structural-not-indexed nature of reconnaissance. `sections:` frontmatter lists only numbered `{NN}.X` subsections; the recon block does not appear there.

**Design decision 3 — methodology + results, NOT raw SSOT expansion.** The block stores: (1) the literal `scripts/intel-query.sh` commands the author ran, (2) a ≤500-char results summary, (3) the date. It does NOT `@`-include the SSOT protocol. `@`-includes are expanded by the harness at skill/command prompt-expansion time but NOT in plan-file markdown — `grep -rEn '^@\.' plans/` returns zero hits today. Embedding `@.claude/skills/dual-tpr/compose-intel-summary.md` in a plan body produces a dead literal; embedding the expanded SSOT protocol would be a `LEAK:algorithmic-duplication` — the exact pathology §03 just fixed for 18 consumers. The recon block records WHAT was done; the SSOT records HOW — one-way reference, no content copy.

**Design decision 4 — Severity and Outcome are INDEPENDENT axes.** `scripts/plan_corpus/__main__.py:37` currently does `return 1 if all_findings else 0`, so any finding fails `check` regardless of severity. And `scripts/plan_corpus/types.py:48` defines `Severity` as `LOW / MEDIUM / HIGH / CRITICAL` — not `WARNING / ERROR`. Severity alone is insufficient for a gate because the enforcement context matters independently of impact. §06.2 introduces a distinct `Outcome` axis:

  - **Severity** is set by the emitter — `LOW` / `MEDIUM` / `HIGH` / `CRITICAL` — per the impact classification of the finding. It reflects "how bad is this?"
  - **Outcome** is set by the emitter — `WARNING` / `ERROR` — per the enforcement mode. It reflects "does this gate the check?" Outcome is NOT auto-derived from Severity; both are set explicitly at `Finding`-construction time.
  - **Default mode:** `status: not-started` missing recon → `Severity.HIGH` + `Outcome.WARNING`. `status: in-progress` missing recon → `Severity.MEDIUM` + `Outcome.WARNING`.
  - **`--strict-recon` mode:** `status: not-started` missing recon → `Severity.HIGH` + `Outcome.ERROR` (Severity unchanged; Outcome rewritten). `status: in-progress` is unaffected by `--strict-recon`.
  - **Exit policy:** `exit 1 iff any finding has Outcome == ERROR`.

**Design decision 5 — status-gated severity, not A/B/C operator menu.** Combined with the corpus already carrying a `status` field, the validator reads each PLAN_SECTION's `data["status"]` (where `data` is `ValidatedFile.data`, the parsed frontmatter dict) and applies: `not-started` missing recon → `Severity.HIGH`, `Outcome.WARNING` (default) or `Outcome.ERROR` (`--strict-recon`); `in-progress` missing recon → `Severity.MEDIUM`, `Outcome.WARNING`; `complete` → exempt (0 findings). This is the objective, data-driven model that replaces §06's earlier A/B/C retrofit menu (now gone; retrofit is §09).

**Design decision 6 — `--strict-recon` as a CLI flag, not a corpus-wide frontmatter.** An earlier draft proposed a per-plan `strict_recon: bool` on `00-overview.md`. That model requires the `PlanSectionSchema` / `OverviewSchema` to accept a new frontmatter field (currently `schema.py:264` rejects unknown keys) and creates corpus-wide state that's hard to preview. Making it a CLI flag keeps policy at the invocation site: CI can pin `--strict-recon` for `not-started` sections; local runs default to warnings-only. No schema widening needed for policy — §06 does NOT add any frontmatter field to `OverviewSchema`.

**Design decision 7 — format coupling with §03 and §07 is a CONTRACT, not a convention.** `00-overview.md:144` already states "§07 hook output format MUST match §03's bounded summary template exactly." §06 extends this into a three-way coupling: §03 helper (source), §06 recon block (plan-body artifact), §07 hook injection (runtime prompt artifact) — all three use `.claude/skills/dual-tpr/compose-intel-summary.md` as the authoritative SSOT. The **§03/§06/§07 shared contract covers three invariants: (1) ≤500-char bound, (2) `[ori]`/`[repo#N]`/`[repo:path]` citation vocabulary, (3) §03 SSOT helper as the source. Exact line-level formatting may vary per consumer's rendering context** (§06 plan-resident blocks are static markdown artifacts; §07 hooks inject a bounded summary into a prompt payload; these rendering contexts differ legitimately). Graceful degradation: when `scripts/intel-query.sh status` returns unavailable, the §07 hook omits the summary entirely (per `compose-intel-summary.md` lines 222-227: "entire summary is OMITTED"), and the §06 plan-resident artifact records the graph-unavailable state as freeform prose (e.g. `"Graph was unavailable at YYYY-MM-DD when this section was authored"`) — NOT a sentinel string matched by the validator. Drift in the ≤500-char bound or citation vocabulary among the three surfaces is a `DRIFT:scattered-knowledge` finding; line-level formatting differences are not. §06.1's template text names this contract explicitly; §06.2's anti-stub detector enforces the citation-grammar half; §07's plan file cites §06's contract back.

  **NOTE — §07 skeleton drift:** The current `section-07-pre-review-intel-hook.md` hook skeleton uses a placeholder `- [$FILE] $RESULT` output format that does NOT yet match this citation-grammar contract. This is a known inconsistency: §07 is not yet implemented, and the skeleton is scaffolding only. §07's implementation MUST rewrite the hook output to emit the Step D citation grammar (≤500 chars, `[ori]` / `[repo#N]` / `[repo:path]` markers) before §07 close-out. The coupling contract stated here is the target; §07's current skeleton is the delta that implementation will close.

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

  Results summary (≤500 chars) [ori]: {bounded paragraph citing blast radius, cross-repo prior art, relevant symbols. Use `[ori]` for Ori-repo claims, `[rust#N]` / `[swift#N]` / `[koka#N]` / etc. for cross-repo issue citations, and `[repo:path]` for symbol results — the same grammar used by `compose-intel-summary.md` Step D (lines 64-82) and by §07's hook injection. Maximum 5 bullets, 500 characters. If the graph is unavailable, record the unavailability state as freeform prose (e.g. `"Graph was unavailable at YYYY-MM-DD when this section was authored"`) — do NOT silently omit the block; the block MUST still exist with the date and a note about unavailability so the validator recognizes it as intentional rather than forgotten.}

  See `.claude/skills/dual-tpr/compose-intel-summary.md` for the full query protocol (SSOT — do NOT `@`-include in plan files; plan markdown is not harness-expanded, so the include would be a dead literal).
  ```

  **Placement requirement:** AFTER all section framing (Goal, Success Criteria, Context, Reference implementations, Depends on) and BEFORE the first numbered subsection (`## {NN}.1`). The block is structurally parallel to the framing blocks — not a subsection.

  **Format-coupling contract:** The §03/§06/§07 shared contract covers three invariants: (1) ≤500-char bound, (2) `[ori]`/`[repo#N]`/`[repo:path]` citation vocabulary, (3) §03 SSOT helper (`.claude/skills/dual-tpr/compose-intel-summary.md`) as the source. Exact line-level formatting may vary per consumer's rendering context (plan-body text vs. hook additionalContext injection). Graceful degradation: §07 hook omits the summary entirely when graph is unavailable (per `compose-intel-summary.md` lines 222-227); §06 plan-resident artifact records the graph-unavailable state as freeform prose with a date (e.g. "Graph was unavailable at YYYY-MM-DD when this section was authored") — the validator recognizes this as `RECON_GRAPH_UNAVAILABLE` at `Severity.LOW` / `Outcome.WARNING` (intentional documentation, NOT a VALIDATION_BYPASS). Drift in the ≤500-char bound or citation vocabulary among the three surfaces is a `DRIFT:scattered-knowledge` finding (see §06 Design decision 7).

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
     Step D, lines 64-82), and the date. Coexists with §07's runtime hook:
     the hook omits the summary entirely when graph is unavailable; the
     plan-resident block records unavailability as freeform prose. Enforced
     by `python -m scripts.plan_corpus check` — the validator gates
     severity on the section's `status` field:
       - status: not-started → Severity.HIGH (ERROR under --strict-recon)
       - status: in-progress → Severity.MEDIUM (WARNING, no on-edit escalation)
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
  - [ ] Format-coupling contract text is present in both the template block and the MANDATORY SECTION STRUCTURE comment; the `[ori]` / `[repo#N]` / `[repo:path]` citation grammar and graceful-degradation behavior (block omitted for §07 hook; freeform prose for §06 plan-resident artifact) are named explicitly
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

1. **Body text does not reach validators.** `scripts/plan_corpus/schema.py:487` — `validate(fc, data, path)` takes only frontmatter. `scripts/plan_corpus/discovery.py:268 load_and_validate(path)` splits body text into `ValidatedFile.body` (real field name per `discovery.py:212`) but never passes it into the validator dispatch. Body-level recon detection requires plumbing or a parallel phase.
2. **No WARNING/ERROR outcome model.** `scripts/plan_corpus/__main__.py:37` does `return 1 if all_findings else 0`. Severity distinctions are not expressible as exit codes.
3. **No status-gated severity.** Validators receive frontmatter `data` but don't branch on `data.get("status")` when emitting recon-block findings.
4. **No anti-performative-ritual detection.** Nothing today rejects "header-present, body-empty" or "header-present, no citation" stubs.

All four must be fixed together; any one alone leaves the enforcement path broken.

- [ ] **Fix CLI entrypoint DRIFT.** Grep active/editable surfaces for legacy references to `scripts/plan_corpus.py` (with the `.py` suffix — the file does NOT exist; the package is invoked as `python -m scripts.plan_corpus`). Command: `grep -rn "scripts/plan_corpus\.py" CLAUDE.md .claude/ scripts/ && grep -rn "scripts/plan_corpus\.py" plans/*/section-*.md | grep -v 'status: complete'`. Replace every hit with `python -m scripts.plan_corpus`. Include CLAUDE.md §Commands and active (non-complete) plan section files. **Do NOT edit completed sections (§01-§05 of this plan and any other `status: complete` sections) — they are frozen artifacts and retain historical command strings as-is.** Capture the count of replaced occurrences in the close-out note.

  NOTE: Scrub scope is intentionally limited to active/editable surfaces only (CLAUDE.md, .claude/rules/*.md, .claude/skills/*, plans/*/section-*.md with status != complete, scripts/*.py). Completed sections (§01-§05 and any other `status: complete` sections across the corpus) retain historical command strings as-is — they are frozen artifacts, not active documentation. Scrubbing frozen sections would violate the freeze policy.

- [ ] **Add `MISSING_RECON_BLOCK`, `VALIDATION_BYPASS`, and `RECON_GRAPH_UNAVAILABLE` to the `FindingSubtype` enum** in `scripts/plan_corpus/types.py` (around line 85). Register all three under `FindingCategory.GAP` in the `_CATEGORY_SUBTYPES` dict (around line 153, within the `FindingCategory.GAP: frozenset({...})` block). The validator then emits:
  - `Finding(category=FindingCategory.GAP, subtype=FindingSubtype.MISSING_RECON_BLOCK, ...)` for entirely missing blocks
  - `Finding(category=FindingCategory.GAP, subtype=FindingSubtype.VALIDATION_BYPASS, ...)` for stub/ritual blocks (header present but content fails concrete-content checks)
  - `Finding(category=FindingCategory.GAP, subtype=FindingSubtype.RECON_GRAPH_UNAVAILABLE, ...)` for graph-unavailable documentation blocks (intentional, NOT a performative stub — see detection rules below)
  All three are type-safe via the enum; without this registration, `Finding` construction raises `ValueError` because `FindingSubtype` is an enum and `_CATEGORY_SUBTYPES` validation rejects unregistered members.

- [ ] **Add `Outcome` enum to `scripts/plan_corpus/types.py`.** Distinct axis from `Severity`:
  ```python
  class Outcome(enum.Enum):
      """Gate outcome — distinct from Severity. Gate behavior answers
      'does this fail the check?' independently of how severe it is."""
      WARNING = "warning"   # printed, does NOT affect exit code
      ERROR = "error"       # printed AND forces exit 1
  ```
  Add `outcome: Outcome = Outcome.ERROR` as a **default field** on the `Finding` dataclass. The default is `Outcome.ERROR` — this ensures ALL existing `Finding(...)` callsites (schema violations, parse errors, unknown frontmatter keys, etc.) continue to emit `Outcome.ERROR` and gate CI without requiring edits to every existing callsite. Only the new recon-block-specific findings explicitly use `Outcome.WARNING`. Outcome is set EXPLICITLY by each new emitter at `Finding`-construction time — it is NOT auto-derived from Severity. The two axes are independent: Severity answers "how bad?" and Outcome answers "does this gate?" For recon-block findings specifically: `status: not-started` missing recon → `Severity.HIGH` + `Outcome.WARNING` (default) or `Outcome.ERROR` (`--strict-recon`); `status: in-progress` → `Severity.MEDIUM` + `Outcome.WARNING`; `RECON_GRAPH_UNAVAILABLE` → `Severity.LOW` + `Outcome.WARNING`. Update `to_markdown` / `to_json` to render the outcome channel. Do NOT rename the `Severity` enum — `LOW`/`MEDIUM`/`HIGH`/`CRITICAL` is the established taxonomy and is consumed elsewhere in the package.

- [ ] **Rewrite the exit-code policy in `scripts/plan_corpus/__main__.py`.** Replace:
  ```python
  return 1 if all_findings else 0
  ```
  with:
  ```python
  errors = [f for f in all_findings if f.outcome == Outcome.ERROR]
  return 1 if errors else 0
  ```
  Keep print/JSON output for ALL findings including warnings. Update `check` help text: `'Validate a file or directory (exits 1 only on findings with Outcome.ERROR; WARNING findings are printed but non-gating)'`. Add a `--strict-recon` flag to the `check` subcommand parser. Plumbing path: `__main__.py` parses `args.strict_recon` → passes to `load_and_validate(path, strict_recon=args.strict_recon)` → `load_and_validate` passes to `validate(..., strict_recon=strict_recon)` → `validate` passes to the `body_validator` dispatch → `_check_intel_recon_block(data, body, path, strict_recon=strict_recon)`. When `strict_recon=True`, the function constructs `Finding(..., outcome=Outcome.ERROR)` directly for `status: not-started` missing/stub recon — it does NOT mutate an existing Finding (Finding is `frozen=True`).

- [ ] **Refactor `FILE_CLASS_META` to carry a body-level validator in addition to the frontmatter validator.** Two viable refactor shapes — §06.2 picks shape (a) with rationale documented inline:

  - (a) **Extend `FileClassMeta` with a `body_validator: Callable[[dict, str, Path, bool], list[Finding]] | None` field** (None for classes without body-level checks). The `bool` parameter is `strict_recon`. Update `validate()` signature to `validate(file_class, data, body, path, *, strict_recon: bool = False)` — calls both frontmatter and body validators sequentially and concatenates findings. `discovery.load_and_validate(path, *, strict_recon: bool = False)` already produces `ValidatedFile.body` (real field name per `discovery.py:212`); the call site passes it through along with `strict_recon`. Rationale: one dispatch mechanism, explicit per-class opt-in, no parallel phase. (Shape (b) — a post-schema body-check phase registered separately — is rejected because it duplicates the dispatch plumbing and would drift from the class-keyed registry that docgen already relies on.)
  - Update the `validate()` signature and EVERY call site (`discovery.py:267`, any direct callers). Use `rg 'schema\.validate\(' scripts/ tests/` to find all call sites BEFORE editing; list them in the commit message. This is a `- [ ]` item, not a deferral — signature propagation IS the work.
  - Update `load_and_validate()` signature to `load_and_validate(path: Path, *, strict_recon: bool = False)` and thread `strict_recon` through to `validate()`.
  - Thread `strict_recon` from CLI: `scripts/plan_corpus/__main__.py` parses the `--strict-recon` flag and passes it down through `load_and_validate(path, strict_recon=args.strict_recon)` → `validate(..., strict_recon=strict_recon)` → body_validator. Since `Finding` is `frozen=True`, the validator constructs Finding with the correct Outcome directly at creation time — NOT via mutation after construction.
  - For classes with `body_validator = None` (ROADMAP_SECTION, BUG_TRACKER_SECTION, FIX_BUG, the various overview / index classes), the extended dispatch is a no-op. Negative-pin tests (§06.2 matrix) confirm zero findings fire.

- [ ] **Implement `_check_intel_recon_block(data: dict, body: str, path: Path, *, strict_recon: bool = False) -> list[Finding]`** in `scripts/plan_corpus/schema.py`. Attach it as the `body_validator` for `FileClass.PLAN_SECTION` only. Detection rules:

  - **Missing block** — no `^## Intelligence Reconnaissance\s*$` header found via `re.search(..., re.MULTILINE)` on `body`.
    - `status: not-started` → `Severity.HIGH`, `Outcome.WARNING` by default; `Severity.HIGH`, `Outcome.ERROR` under `--strict-recon`
    - `status: in-progress` → `Severity.MEDIUM`, `Outcome.WARNING` (unaffected by `--strict-recon`)
    - `status: complete` → 0 findings (exempt)
    - `FindingCategory.GAP`, `FindingSubtype.MISSING_RECON_BLOCK`, message cites `.claude/skills/dual-tpr/compose-intel-summary.md` as the SSOT protocol

  - **Graph-unavailable documentation block** (header present AND body contains a date marker AND body contains one of: literal `"graph unavailable"`, `"graph was unavailable"`, `"intelligence graph unavailable"` — case-insensitive):
    - This is INTENTIONAL documentation, NOT a performative stub — the author ran the availability check and recorded that the graph was down
    - `Severity.LOW`, `Outcome.WARNING` (printed, never gates CI)
    - `FindingCategory.GAP`, `FindingSubtype.RECON_GRAPH_UNAVAILABLE`, message: `"Section records graph-unavailable state at <date>. Fill in full queries if/when graph becomes available."`
    - This shape PASSES (does NOT trigger VALIDATION_BYPASS); it is a distinct, lower-severity finding

  - **Stub / performative-ritual block** (header present but body fails one or more concrete-content checks AND does NOT qualify as a graph-unavailable documentation block):
    - Block body is empty / whitespace-only between the header and the next `^## ` (or end-of-file), OR
    - Block body contains only placeholder tokens. Tokens (case-insensitive, whole-token match): `TBD`, `none`, `n/a`, `todo`, `(empty)`, ellipsis-only (`...`, `…`), OR
    - Block body fails ANY of the three concrete-content requirements:
      - (a) No literal `scripts/intel-query.sh` command line (matched via regex `\bscripts/intel-query\.sh\b` — must appear in the block body)
      - (b) No date marker in ISO format `YYYY-MM-DD` within the block (matched via regex `\d{4}-\d{2}-\d{2}`)
      - (c) No concrete citation marker: no literal `[ori]`, no cross-repo citation marker matching `\[[a-z][a-z0-9-]*[#:][^\]]+\]` (generic pattern — matches both `[repo#123]` issue citations AND `[repo:path/to/symbol]` symbol citations; avoids DRIFT when reference repos are added or when symbol results use the `[repo:path]` form permitted by `compose-intel-summary.md` Step D lines 78-80)
    - Block body pastes the literal `@.claude/skills/dual-tpr/compose-intel-summary.md` directive verbatim without a condensed summary paragraph following it (the `@`-include is a SOURCE for Claude's prompt, NOT a substitute for the plan-resident snapshot)
    - Severity / outcome mapping identical to "missing block" above
    - `FindingCategory.GAP`, `FindingSubtype.VALIDATION_BYPASS`, message names which specific check(s) failed (missing query / missing date / missing citation / placeholder-only / empty)

  - **Complete block** — header present AND body passes all concrete-content checks (or qualifies as graph-unavailable) → 0 VALIDATION_BYPASS findings

  **Accepted body shapes (PASS / no VALIDATION_BYPASS):**
  1. Full recon with `scripts/intel-query.sh` command + ISO date + citation marker → 0 findings
  2. Graph-unavailable note with ISO date + one of the unavailability phrases → `RECON_GRAPH_UNAVAILABLE` at `Severity.LOW` / `Outcome.WARNING` (distinct, non-gating)

  **Rejected body shapes (FAIL / emit VALIDATION_BYPASS):**
  3. Empty — header present, body whitespace-only
  4. Placeholder — body contains only `TBD` / `none` / `n/a` / `todo` / `(empty)` / ellipsis
  5. Citation-free — has query and date, no `[ori]` / `[repo#N]` citation marker
  6. Query-free — has date and citation, no `scripts/intel-query.sh` literal
  7. Date-free — has query and citation, no ISO `YYYY-MM-DD` marker

  Block-body extraction: slurp from the line after the header to the next `^## ` or end-of-file. Strip whitespace and HTML comments (`<!-- ... -->`) before token / citation checks. HTML comments are metadata, not content.

- [ ] **Wire body through `scripts/plan_corpus/discovery.py:load_and_validate`.** `ValidatedFile` already carries `body` (real field name per `discovery.py:212`); the dispatch to `validate(...)` at `discovery.py:267` currently passes only frontmatter. Update the call site to pass `body` and `strict_recon` through per the new `validate()` signature. Use `rg 'schema\.validate\(' scripts/ tests/` to find all call sites before editing.

- [ ] **Add `discover` per-plan recon-coverage reporter.** After the existing per-plan summary, print a status-grouped table — §09 consumes this table to measure retrofit completeness:
  ```
  Per-plan recon coverage:
    plans/foo/                  — not-started: 3/5 PRESENCE   in-progress: 1/2 PRESENCE   complete: 4/4 exempt
    plans/bar/                  — not-started: 0/4 PRESENCE   in-progress: 0/0            complete: 0/0
    plans/query-intel-adoption/ — not-started: 4/4 PRESENCE   in-progress: 0/0            complete: 5/5 exempt
  ```

  **Refactor choice (data source):** The `discover` command currently uses `discover_corpus()` (`discovery.py`), which walks the tree and classifies files but does NOT parse bodies or run `load_and_validate` per file. For the recon-coverage reporter, the `discover` command MUST additionally call `load_and_validate(path)` on each `PLAN_SECTION` path in `corpus.plan_sections.keys()` — this provides body text + recon-block validation findings without a second filesystem walk (paths are already discovered). The `discover` reporter then READS `ValidatedFile.violations` (already populated by `body_validator` during `load_and_validate`) to count recon-block findings — it does NOT call `_check_intel_recon_block` directly. Alternative (b) — running a standalone regex or calling `_check_intel_recon_block` directly from `discover` — is rejected because `load_and_validate` / `body_validator` already ran the detection; calling it again would be duplicate work and would diverge if detection logic changes. There is no `--strict-recon` flag on `discover` — `discover` reports block PRESENCE (any shape including stubs counts as present), not block quality gating. Quality findings (stub vs. complete) live in `check`. Concrete implementation:
  1. In `__main__.py` `discover` subcommand handler, after building the `Corpus`, iterate `corpus.plan_sections.keys()` and call `load_and_validate(path)` on each
  2. For each successful `LoadResult.ok`, read `ValidatedFile.violations` to determine recon block presence and shape (missing = `MISSING_RECON_BLOCK` violation; stub = `VALIDATION_BYPASS` violation; graph-unavailable = `RECON_GRAPH_UNAVAILABLE` violation; complete = no recon violations)
  3. The coverage metric counts block PRESENCE: a block is "present" if no `MISSING_RECON_BLOCK` violation exists (stubs, graph-unavailable notes, and complete blocks all count as present). Quality issues are separate findings reported by `check`.
  4. Group results by plan directory and status; emit the table above

- [ ] **Write the representative matrix of body-level recon tests in `tests/plan-audit/test_recon_block.py`** (new file, sibling of existing `test_plan_corpus.py`). Reuse the existing fixture harness pattern.

  Matrix: (FileClass) × (body-shape) × (severity-mode). Every cell is a positive or negative pin. The exempt-class section covers representative body shapes; `present-no-query`, `present-no-date`, and `graph-unavailable` body shapes are not pinned for exempt classes (any such content is ignored — the class exemption is total).

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
  | PLAN_SECTION | stub-no-query (prose + date + citation but no `scripts/intel-query.sh` literal) | not-started | no | 1, GAP:VALIDATION_BYPASS |
  | PLAN_SECTION | stub-no-date (query + citation but no YYYY-MM-DD date marker) | not-started | no | 1, GAP:VALIDATION_BYPASS |
  | PLAN_SECTION | stub-no-citation (query + date but no `[ori]` / `[repo#N]` / `[repo:path]`) | not-started | no | 1, GAP:VALIDATION_BYPASS |
  | PLAN_SECTION | complete with `[repo:path]` citation (e.g. `[rust:compiler/rustc_errors/src/lib.rs]`) | not-started | no | 0 (symbol-path citation is valid per `\[[a-z][a-z0-9-]*[#:][^\]]+\]`) |
  | PLAN_SECTION | stub-only-@-include (directive pasted, no condensed paragraph) | not-started | no | 1, GAP:VALIDATION_BYPASS |
  | PLAN_SECTION | graph-unavailable (date + "graph unavailable" phrase, no query) | not-started | no | 1, GAP:RECON_GRAPH_UNAVAILABLE, Severity.LOW, Outcome.WARNING |
  | PLAN_SECTION | graph-unavailable (date + "graph unavailable" phrase, no query) | not-started | yes | 1, GAP:RECON_GRAPH_UNAVAILABLE, Severity.LOW, Outcome.WARNING (--strict-recon does NOT escalate graph-unavailable) |
  | PLAN_SECTION | graph-unavailable (date + "intelligence graph unavailable" phrase) | in-progress | no | 1, GAP:RECON_GRAPH_UNAVAILABLE, Severity.LOW, Outcome.WARNING |
  | PLAN_SECTION | stub-empty (header, whitespace body) | in-progress | no | 1, Severity.MEDIUM, Outcome.WARNING |
  | PLAN_SECTION | stub-placeholder ("TBD" / "none" / etc.) | in-progress | no | 1, Severity.MEDIUM, Outcome.WARNING |
  | PLAN_SECTION | stub-no-query (prose + date + citation but no `scripts/intel-query.sh` literal) | in-progress | no | 1, Severity.MEDIUM, Outcome.WARNING |
  | PLAN_SECTION | stub-no-date (query + citation but no YYYY-MM-DD date marker) | in-progress | no | 1, Severity.MEDIUM, Outcome.WARNING |
  | PLAN_SECTION | stub-no-citation (query + date but no `[ori]` / `[repo#N]` / `[repo:path]`) | in-progress | no | 1, Severity.MEDIUM, Outcome.WARNING |
  <!-- NOTE: in-progress anti-stub shapes share the same detection logic as not-started; severity differs (MEDIUM vs HIGH) and --strict-recon does NOT escalate in-progress findings. The 5 rows above pin the severity mapping explicitly; detection code is shared. -->
  | **Exempt-class negative pins — ROADMAP_SECTION** | | | | |
  | ROADMAP_SECTION | absent | not-started | no | 0 (exempt — out of scope) |
  | ROADMAP_SECTION | absent | not-started | yes | 0 (exempt — out of scope) |
  | ROADMAP_SECTION | present-empty | not-started | no | 0 (exempt) |
  | ROADMAP_SECTION | present-empty | not-started | yes | 0 (exempt) |
  | ROADMAP_SECTION | present-placeholder | not-started | no | 0 (exempt) |
  | ROADMAP_SECTION | present-no-citation | not-started | no | 0 (exempt) |
  | ROADMAP_SECTION | present-no-query | not-started | no | 0 (exempt) |
  | ROADMAP_SECTION | present-no-date | not-started | no | 0 (exempt) |
  | ROADMAP_SECTION | graph-unavailable | not-started | no | 0 (exempt) |
  | ROADMAP_SECTION | absent | not-started | yes (--strict-recon) | 0 (exempt; strict does not affect exempt classes) |
  | **Exempt-class negative pins — BUG_TRACKER_SECTION** | | | | |
  | BUG_TRACKER_SECTION | absent | not-started | no | 0 (exempt — out of scope) |
  | BUG_TRACKER_SECTION | absent | not-started | yes | 0 (exempt — out of scope) |
  | BUG_TRACKER_SECTION | present-empty | not-started | no | 0 (exempt) |
  | BUG_TRACKER_SECTION | present-empty | not-started | yes | 0 (exempt) |
  | BUG_TRACKER_SECTION | present-placeholder | not-started | no | 0 (exempt) |
  | BUG_TRACKER_SECTION | present-no-citation | not-started | no | 0 (exempt) |
  | BUG_TRACKER_SECTION | present-no-query | not-started | no | 0 (exempt) |
  | BUG_TRACKER_SECTION | present-no-date | not-started | no | 0 (exempt) |
  | BUG_TRACKER_SECTION | graph-unavailable | not-started | no | 0 (exempt) |
  | BUG_TRACKER_SECTION | absent | not-started | yes (--strict-recon) | 0 (exempt; strict does not affect exempt classes) |
  | **Exempt-class negative pins — FIX_BUG** | | | | |
  | FIX_BUG | absent | not-started | no | 0 (exempt — different template) |
  | FIX_BUG | absent | not-started | yes | 0 (exempt — different template) |
  | FIX_BUG | present-empty | not-started | no | 0 (exempt) |
  | FIX_BUG | present-empty | not-started | yes | 0 (exempt) |
  | FIX_BUG | present-placeholder | not-started | no | 0 (exempt) |
  | FIX_BUG | present-no-citation | not-started | no | 0 (exempt) |
  | FIX_BUG | present-no-query | not-started | no | 0 (exempt) |
  | FIX_BUG | present-no-date | not-started | no | 0 (exempt) |
  | FIX_BUG | graph-unavailable | not-started | no | 0 (exempt) |
  | FIX_BUG | absent | not-started | yes (--strict-recon) | 0 (exempt; strict does not affect exempt classes) |
  | **Exit-code tests** | | | | |
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
  - [ ] **Run `/improve-tooling` retrospectively on 06.2** — does the validator error message include a pointer to the SSOT and the format-coupling contract? Minimum text: `"Section lacks non-stub '## Intelligence Reconnaissance' block. See '.claude/skills/create-plan/plan-schema.md' MANDATORY SECTION STRUCTURE; run queries per '.claude/skills/dual-tpr/compose-intel-summary.md'; summary format must use [ori] / [repo#N] / [repo:path] citation grammar (Step D, lines 64-82). If the graph is unavailable, record the unavailability as freeform prose with the date — do NOT omit the block."` Commit via `build(tooling): improve plan_corpus recon-block error messages — surfaced by section-06.2 retrospective`.
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
