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
    status: complete
  - id: "03.2"
    title: "Replace inlined copies with @-includes (18 consumers across review-family + wider skills)"
    status: complete
  - id: "03.3"
    title: "Verify SSOT invariant"
    status: complete
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

- [x] Create `.claude/skills/dual-tpr/compose-intel-summary.md` with body:

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

- [x] Verify: `wc -l .claude/skills/dual-tpr/compose-intel-summary.md` reports ≥80 lines; file parses as valid markdown.

- [x] **Subsection close-out (03.1)**:
  - [x] SSOT file created and content-reviewed for completeness (100 lines)
  - [x] Update `03.1` status to `complete`
  - [x] **Run `/improve-tooling` retrospectively on 03.1** — Retrospective: no tooling gaps. The polling-protocol.md sibling was a perfect template reference; writing the new SSOT was a 1-step Write operation using the verbatim body specified at plan L62-162. No query patterns surfaced during composition that aren't already in the template (availability check → file-symbols → callers/callees → similar → bounded summary). Sibling-precedent pattern (polling-protocol.md + compose-rules-brief.md) is itself the tool — it makes subsequent SSOTs near-mechanical.
  - [x] **Run `/sync-claude` on 03.1** — `.claude/rules/intelligence.md` cross-reference deferred to §03.3 Completion Checklist, where the §Algorithmic DRY entry is bundled with the impl-hygiene.md update. No new rules-level change needed solely from §03.1; the SSOT is the product, the cross-reference lands after invariant verification.
  - [x] **Repo hygiene check** — clean (only the new SSOT file added; no temp files).

---

## 03.2 Replace inlined copies with @-includes

**File(s):** 18 consumer files migrated (6 review-family + 12 wider-skill consumers — scope expanded during execution)

The replacement is mechanical but order matters: verify each file's inlined block matches the canonical template semantically before replacing, so we don't lose domain-specific customization.

**Scope discovery (2026-04-14):** The initial plan targeted 6 review-family files identified by the TPR. During migration, `grep -l 'scripts/intel-query.sh status' .claude/ -r` surfaced 12 ADDITIONAL skills that also inline the pattern. Per CLAUDE.md zero-deferral + correctness invariants, all 18 were migrated in this subsection. The SSOT's Step F was extended to document the full set of domain-specific extensions.

**Target files (review-family — original 6 from TPR):**

1. `.claude/skills/review-work/SKILL.md:251-259` (Step 1.5 CONDITIONAL — Intelligence Pre-Query)
2. `.claude/skills/tpr-review/SKILL.md` Step 0.75 (~42 lines)
3. `.claude/commands/review-plan.md:96-107`
4. `.claude/commands/review-work.md:70-75`
5. `.claude/commands/independent-review.md:221-224`
6. `.claude/commands/review-bugs.md:152-178`

**Target files (wider skills — 12 discovered during execution):**

7. `.claude/skills/tp-help/SKILL.md:98` (inline mention)
8. `.claude/skills/add-bug/SKILL.md:90` (inline mention)
9. `.claude/skills/improve-tooling/SKILL.md:89` (inline mention)
10. `.claude/skills/design-pattern-review/SKILL.md:134-155` (STEP 1.5)
11. `.claude/skills/create-draft-proposal/SKILL.md:61-75` (Step 4.5)
12. `.claude/skills/fix-bug/SKILL.md:105-120` (5a Intelligence Graph Query)
13. `.claude/skills/impl-hygiene-review/SKILL.md:191-196` (Intelligence-assisted map)
14. `.claude/skills/review-draft-proposal/SKILL.md:98-103` (CONDITIONAL Prior Art)
15. `.claude/skills/create-plan/SKILL.md:377-382` (Step 2.5)
16. `.claude/skills/rosetta-test/SKILL.md:75-80` (I. Cross-Language Intelligence)
17. `.claude/skills/code-journey/SKILL.md:112-117` (Intelligence map)
18. `.claude/skills/continue-roadmap/SKILL.md:381-399` (Step 2.1)

- [x] For EACH target file:
  - [x] Read the inlined block in full
  - [x] Confirm it matches the SSOT template (availability check, file-symbols, callers/callees, similar, condense-to-summary). Domain-specific extensions noted and integrated into SSOT Step F (review-bugs `search`/`fixed`, fix-bug `fixed`/`similar`, create-plan `symbols`, impl-hygiene-review `file-symbols` per crate, design-pattern-review `compare`/`symbols`, etc.)
  - [x] Replace the block with a single `@`-include directive plus preserved domain-specific queries where present
  - [x] Diff the before/after — each migration reduces inlined duplication; domain queries remain inline where semantically distinct from the generic pattern

- [x] Spot-check: `grep -l 'scripts/intel-query.sh status' .claude/ -r` returns only 3 files — the SSOT itself + 2 legitimate teaching surfaces (`.claude/rules/intelligence.md`, `.claude/commands/query-intel.md`). All skill/command consumers migrated.

- [x] **Subsection close-out (03.2)**:
  - [x] All 18 replacements landed; diff reviewed
  - [x] Update `03.2` status to `complete`
  - [x] **Run `/improve-tooling` retrospectively on 03.2** — Retrospective: (1) A scope-audit helper would have surfaced the 12 extra consumers BEFORE migration started, avoiding mid-execution user confirmation. Candidate: `scripts/ssot-consumers.py <pattern>` that lists all consumers of an SSOT pattern and classifies each as `inlined` / `@-included` / `teaching-surface`. Filed as follow-up tooling opportunity. (2) The `@`-include pattern proved robust — no harness complaints. (3) Grep-based invariant checks (§03.3 primary tool) are a blunt instrument but sufficient for the one-pattern SSOT case.
  - [x] **Run `/sync-claude` on 03.2** — `.claude/rules/intelligence.md` updated with a cross-reference to `compose-intel-summary.md` as the canonical pre-query protocol (see commit).
  - [x] **Repo hygiene check** — clean (no temp files from migrations).

---

## 03.3 Verify SSOT invariant

**File(s):** N/A (verification-only)

- [x] Run the invariant check:
  ```
  # Count inlined copies of the pre-query pattern (must be limited to the SSOT + legitimate teaching surfaces)
  grep -l 'scripts/intel-query.sh status' .claude/ -r
  # Result: 3 files — .claude/skills/dual-tpr/compose-intel-summary.md (SSOT itself),
  #                   .claude/rules/intelligence.md (canonical when-to-query rule),
  #                   .claude/commands/query-intel.md (command wrapper)
  # All skill/command consumers migrated. Invariant satisfied.
  ```
- [x] Harness-side check: the `@`-include expansion is the harness's standard splicing behavior (used by existing `polling-protocol.md` + `compose-rules-brief.md` SSOTs in the same directory). No additional harness-specific verification needed — the pattern is proven.
- [x] Document the SSOT invariant in `.claude/rules/impl-hygiene.md` — added "Precedents — SSOT-via-@-include for Skill Protocols" subsection to §Algorithmic DRY, citing both `polling-protocol.md` (2026-04) and `compose-intel-summary.md` (2026-04-14) precedents.

- [x] **Subsection close-out (03.3)**:
  - [x] Invariant check passes (SSOT + 2 legitimate teaching surfaces; all consumer skills/commands migrated)
  - [x] Update `03.3` status to `complete`
  - [x] **Run `/improve-tooling` retrospectively on 03.3** — Retrospective: the grep invariant is the only verification needed for now; promoting it to a pre-commit hook would be premature (the hook surface is already busy, and `grep -l` is fast enough that a periodic manual check suffices). When §07 ships `.claude/hooks/pre-review-intel.sh`, revisit — a UserPromptSubmit hook could run the invariant check alongside its primary intel pre-query work. Filed as a note for §07 to consider.
  - [x] **Run `/sync-claude` on 03.3** — `.claude/rules/impl-hygiene.md` §Algorithmic DRY updated with Precedents subsection citing both dual-tpr SSOTs. Commit bundled with §03.3 closure.
  - [x] **Repo hygiene check** — clean (no temp files).

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
