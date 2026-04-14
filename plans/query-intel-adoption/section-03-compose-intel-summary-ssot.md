---
section: "03"
title: "SSOT: compose-intel-summary helper"
status: not-started
reviewed: false
goal: "Create one canonical intel-summary injection template and replace 6 inlined copies across review-family skills and commands with @-includes"
success_criteria:
  - "`.claude/skills/dual-tpr/compose-intel-summary.md` exists as the sole source of the availability-check → file-symbols → callers → callees → similar → bounded-summary template"
  - "6 consumers (`tpr-review`, `review-work` SKILL, `review-plan`, `review-work` command, `independent-review`, `review-bugs`) now @-include the helper instead of inlining the pattern"
  - "`grep -c 'intel-query.sh status' .claude/skills/ .claude/commands/ -r` returns 1 distinct source (the SSOT) plus some number of @-include references, but zero inlined copies"
  - "Satisfies mission criterion: exactly ONE canonical intel-summary helper; zero LEAK:algorithmic-duplication for the pre-query pattern"
inspired_by:
  - "`.claude/skills/dual-tpr/polling-protocol.md` — SSOT for polling-loop text across tpr-review, review-work, tp-help"
  - "`.claude/skills/dual-tpr/compose-rules-brief.md` — SSOT for rules-brief composition"
  - "TPR findings codex-007 [high] LEAK:algorithmic-duplication, gemini-003 [medium]"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Write compose-intel-summary.md"
    status: not-started
  - id: "03.2"
    title: "Replace 6 inlined copies with @-includes"
    status: not-started
  - id: "03.3"
    title: "Verify SSOT invariant"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: SSOT — compose-intel-summary helper

**Status:** Not Started
**Goal:** Establish `.claude/skills/dual-tpr/compose-intel-summary.md` as the ONE canonical source for the intel-pre-query template. Every consumer (6 files today, more as §05/§06/§07 land) `@`-includes it. The harness splices the included file into the prompt at expansion time — so updates to the template automatically propagate to all consumers without drift.

**Context:** The 2026-04-14 TPR (codex finding TPR-XX-007, severity high, category LEAK:algorithmic-duplication) confirmed that the availability-check → file-symbols → callers → callees → similar → bounded-summary pattern is inlined in 6 separate files today. This is a textbook SSOT violation per `.claude/rules/impl-hygiene.md` §Algorithmic DRY. The existing `polling-protocol.md` in the same directory is the canonical precedent — it consolidated three near-identical inlined polling blocks into one `@`-include after a similar drift-driven surfacing in early April 2026. This section repeats that consolidation for the intel-pre-query pattern.

**Reference implementations:**
- **Ori** `.claude/skills/dual-tpr/polling-protocol.md`: exact SSOT pattern — one canonical file, many `@`-include consumers, the rule file (`intelligence.md` or `impl-hygiene.md`) explains the drift violation resolved
- **Ori** `.claude/skills/dual-tpr/compose-rules-brief.md`: sibling in the same directory; shows the Agent(subagent_type=Sonnet) composition style that intel-summary may also adopt for longer summaries

**Depends on:** Section 01 (Skill dir structure for `.claude/skills/dual-tpr/` must be stable; §01's SKILL promotion work happens under `.claude/skills/query-intel/`, not here, but Skill-dir conventions must be settled before §03 adds a new sibling).

---

## 03.1 Write compose-intel-summary.md

**File(s):** `.claude/skills/dual-tpr/compose-intel-summary.md` (new)

The helper is a reference document, not an executable script. Consumers `@`-include it into their prompts. Its body is the canonical instructions that Claude (or a sub-agent) follows to produce a bounded Intelligence Summary.

- [ ] Create `.claude/skills/dual-tpr/compose-intel-summary.md` with body:

  ```markdown
  # Intelligence Summary Injection — Canonical SSOT

  **Single source of truth** for pre-query / summary-injection behavior across
  ALL consumers: /tpr-review Step 0.75, /review-work Step 1.5, /review-plan,
  /independent-review, /review-bugs, /tp-help, /verify-tpr, /sync-claude,
  /fix-next-bug, /fix-bug, and the .claude/hooks/pre-review-intel.sh hook.
  Every such consumer MUST @-include this file from its intel section rather
  than inlining the pattern.

  ## Protocol

  ### Step A — Availability check

  ```
  Bash (foreground):
    scripts/intel-query.sh status
  ```

  Parse the JSON. If `status != "ok"`, skip silently. Do NOT emit an empty
  section in the consumer's prompt — skipping means NO Intelligence Summary
  block appears at all.

  ### Step B — Subsystem and symbol identification

  For code/plan review modes, use the same scope the consumer is operating
  on (e.g., `git diff --name-only HEAD~5..HEAD`). For custom-objective mode,
  extract relevant file paths or symbol names from the objective text.

  Map subsystems to presets per `.claude/rules/intelligence.md` §Subsystem
  Mapping. DO NOT hardcode the mapping here.

  ### Step C — Run the queries

  Up to 5 queries total to keep the summary bounded. Output is visible in
  Claude's context — do NOT capture into a variable.

  1. Subsystem preset OR directed search:
     `scripts/intel-query.sh --human <preset-or-search> --limit 5`
  2. For top 3-5 changed files:
     `scripts/intel-query.sh --human file-symbols "<path-fragment>" --repo ori`
  3. For each high-signal symbol:
     `scripts/intel-query.sh --human callers "<symbol>" --repo ori`
     `scripts/intel-query.sh --human callees "<symbol>" --repo ori`
     `scripts/intel-query.sh --human similar "<symbol>" --repo rust,swift,go --limit 5`

  If any query returns empty, skip it silently in the summary.

  ### Step D — Condense into a bounded Intelligence Summary (≤500 chars)

  Format:

  ```
  **Intelligence Summary (from intelligence graph):**
  - [rust#12345] Similar bug / pattern — short phrase (N reactions)
  - [swift#6789] Reference implementation — short phrase
  - [ori] <symbol> called by N sites across M modules — blast radius note
  ```

  Rules:
  - Maximum 5 bullets.
  - Maximum 500 characters (hard cap; truncate with `…` if needed).
  - Reference-repo citations use `[repo#N]` issue shorthand or
    `[repo:path]` for symbol results.
  - Ori citations use `[ori]` prefix.
  - Do NOT cite a result as authoritative — this is DISCOVERY for the
    consumer, not conclusions.

  ### Step E — Inject into the consumer's prompt

  The consumer is responsible for placing the summary into its own prompt
  template (e.g., after the `## Scope:` header in a reviewer prompt,
  after the objective in a custom-objective prompt). This helper produces
  the summary text; the consumer chooses where to place it.

  ## Graceful degradation

  If `scripts/intel-query.sh status` returns unavailable, the entire
  summary is OMITTED. Do NOT emit an empty "Intelligence Summary: no
  results" block — that's noise. The consumer's prompt should be
  syntactically valid whether or not the summary appears.

  ## Banned patterns

  - Inlining this template in any consumer instead of `@`-including it
  - Open-coding Neo4j access (bypassing `scripts/intel-query.sh`)
  - Emitting a summary without the availability check
  - Citing a graph result without verifying against actual code

  ## Consumers

  Every consumer of this file references it via `@.claude/skills/dual-tpr/compose-intel-summary.md`
  at its intel section. Updates to this protocol propagate automatically.

  ## Related

  - `.claude/rules/intelligence.md` — when-to-query workflow inventory, subsystem mapping
  - `.claude/skills/query-intel/SKILL.md` — full capability surface
  - `scripts/intel-query.sh` — the canonical wrapper (206 lines; see §08 for planned UX improvements)
  - `.claude/skills/dual-tpr/polling-protocol.md` — sibling SSOT for dual-source polling
  - `.claude/skills/dual-tpr/compose-rules-brief.md` — sibling SSOT for rules-brief composition
  ```

- [ ] Verify: `wc -l .claude/skills/dual-tpr/compose-intel-summary.md` reports ≥80 lines; file parses as valid markdown.

- [ ] **Subsection close-out (03.1)**:
  - [ ] SSOT file created and content-reviewed for completeness
  - [ ] Update `03.1` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 03.1** — did the polling-protocol.md sibling make this easy, or were there gaps? Did we discover any query pattern during composition that should live in the SSOT but isn't surfaced by any current consumer? Commit via `docs(skills): ...`.
  - [ ] **Run `/sync-claude` on 03.1** — new SSOT file means `.claude/rules/intelligence.md` §Symbol-First Workflow may need a cross-reference. Commit via `docs(rules): ...` if applicable.
  - [ ] **Repo hygiene check**.

---

## 03.2 Replace 6 inlined copies with @-includes

**File(s):** 6 files to modify

The replacement is mechanical but order matters: verify each file's inlined block matches the canonical template semantically before replacing, so we don't lose domain-specific customization.

Target files with confirmed inlined pattern (line numbers from TPR verification):

1. `.claude/skills/review-work/SKILL.md:251-259` (Step 1.5 CONDITIONAL — Intelligence Pre-Query)
2. `.claude/skills/tpr-review/SKILL.md` Step 0.75 (~50 lines)
3. `.claude/commands/review-plan.md:96-103`
4. `.claude/commands/review-work.md:71-74`
5. `.claude/commands/independent-review.md:221-224`
6. `.claude/commands/review-bugs.md:156-174`

- [ ] For EACH target file:
  - [ ] Read the inlined block in full
  - [ ] Confirm it matches the SSOT template (availability check, file-symbols, callers/callees, similar, condense-to-summary). If a consumer has a domain-specific extension (e.g., `/review-bugs` queries `fixed` too), note the gap and EXTEND the SSOT rather than preserving the inlined copy
  - [ ] Replace the block with a single `@`-include directive:
    ```markdown
    ### {Step label as in original} — Intelligence Pre-Query
    
    @.claude/skills/dual-tpr/compose-intel-summary.md
    ```
  - [ ] Diff the before/after — the net change should be one block deleted, one `@`-include line added (plus optional section heading preservation).

- [ ] Spot-check: after all 6 replacements, `grep -l 'scripts/intel-query.sh status' .claude/skills/ .claude/commands/ -r` returns ONLY `.claude/skills/dual-tpr/compose-intel-summary.md` (the SSOT itself).

- [ ] **Subsection close-out (03.2)**:
  - [ ] All 6 replacements landed; diff reviewed
  - [ ] Update `03.2` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 03.2** — was finding all 6 call sites easy or did grep take several tries? Would a helper like `scripts/find-inlined-ssot.py <pattern>` be useful for future consolidations? Commit via `build(diagnostics): ...` if matured.
  - [ ] **Run `/sync-claude` on 03.2** — 6 skill/command files changed. `.claude/rules/intelligence.md` should mention the new SSOT under `## How to Query` or `## Symbol-First Workflow`. Commit via `docs(rules): ...`.
  - [ ] **Repo hygiene check**.

---

## 03.3 Verify SSOT invariant

**File(s):** N/A (verification-only)

- [ ] Run the invariant check:
  ```
  # Count inlined copies of the pre-query pattern (must be 1: the SSOT itself)
  grep -l 'scripts/intel-query.sh status' .claude/ -r | grep -v compose-intel-summary.md
  # Expect: 0 files
  ```
- [ ] Run a harness-side check: the `@`-include expansion works correctly by inspecting a consumer's expanded prompt (e.g., render `tpr-review/SKILL.md` and confirm the SSOT content appears inline at the include point).
- [ ] Document the SSOT invariant in `.claude/rules/impl-hygiene.md` if it's not already there (cross-reference the polling-protocol.md precedent).

- [ ] **Subsection close-out (03.3)**:
  - [ ] Invariant check passes (0 non-SSOT files contain the pattern)
  - [ ] Update `03.3` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 03.3** — should the invariant check be a pre-commit hook? (Lefthook config lives outside `.claude/` — flag for §07 to consider hook-family patterns.) Commit via `build(ci): ...` or note for §07 integration.
  - [ ] **Run `/sync-claude` on 03.3** — `.claude/rules/impl-hygiene.md` may need a new §Algorithmic DRY entry citing the compose-intel-summary SSOT as a second precedent (alongside polling-protocol.md). Commit via `docs(rules): ...`.
  - [ ] **Repo hygiene check**.

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] `.claude/skills/dual-tpr/compose-intel-summary.md` exists and is ≥80 lines
- [ ] All 6 consumer files (listed in 03.2) `@`-include the SSOT; inlined copies removed
- [ ] Invariant check: `grep -l 'scripts/intel-query.sh status' .claude/ -r | grep -v compose-intel-summary.md | wc -l` returns 0
- [ ] `./test-all.sh` green
- [ ] `python scripts/plan_corpus.py check plans/query-intel-adoption/section-03-compose-intel-summary-ssot.md` returns 0 errors
- [ ] **Plan sync**:
  - [ ] Section frontmatter `status` → `complete`
  - [ ] `00-overview.md` Quick Reference and mission criterion updated
  - [ ] `index.md` updated
  - [ ] §05, §06, §07 `depends_on` verified — the SSOT they rely on now exists
- [ ] `/tpr-review` passed — verify reviewers confirm all 6 consumer files now point at the same SSOT
- [ ] `/impl-hygiene-review` passed — the new file IS a canonical home, not a duplicate (validated against `.claude/rules/impl-hygiene.md` §SSOT)
- [ ] `/improve-tooling` section-close sweep
- [ ] `/sync-claude` section-close doc sync — `.claude/rules/impl-hygiene.md` §Algorithmic DRY updated to cite the compose-intel-summary SSOT alongside polling-protocol.md
- [ ] `diagnostics/repo-hygiene.sh --check`

**Exit Criteria:** Exactly one file under `.claude/` contains the `scripts/intel-query.sh status` pattern — the SSOT. All 6 review-family consumers reference it via `@`-include. `.claude/rules/impl-hygiene.md` cites the consolidation as a second precedent. `./test-all.sh` green.
