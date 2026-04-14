---
section: "03"
title: "SSOT: compose-intel-summary helper"
status: complete
reviewed: true
goal: "Create one canonical intel-summary injection template and replace every inlined copy across review-family skills, wider skill consumers, and command files with @-includes (18 consumers migrated, 3 legitimate teaching surfaces preserved)"
success_criteria:
  - "`.claude/skills/dual-tpr/compose-intel-summary.md` exists as the sole source of the availability-check → file-symbols → callers → callees → similar → bounded-summary template, with Step F as a maintained registry of per-consumer extensions"
  - "18 consumers (6 review-family + 12 wider-skill consumers) `@`-include the helper instead of inlining the pattern — see §03.2 for the enumerated list"
  - "`grep -l 'scripts/intel-query.sh status' .claude/ -r` returns exactly 3 files: the SSOT itself, `.claude/rules/intelligence.md` (canonical when-to-query rule), and `.claude/commands/query-intel.md` (command wrapper) — all legitimate teaching surfaces. Zero inlined copies remain."
  - "Satisfies mission criterion: exactly ONE canonical intel-summary helper; zero LEAK:algorithmic-duplication for the pre-query pattern. Precedent documented in `.claude/rules/impl-hygiene.md` §Algorithmic DRY §Precedents alongside `polling-protocol.md`."
inspired_by:
  - "`.claude/skills/dual-tpr/polling-protocol.md` — SSOT for polling-loop text across tpr-review, review-work, tp-help"
  - "`.claude/skills/dual-tpr/compose-rules-brief.md` — SSOT for rules-brief composition"
  - "TPR findings codex-007 [high] LEAK:algorithmic-duplication, gemini-003 [medium]"
depends_on: ["01"]
third_party_review:
  status: resolved
  updated: 2026-04-14
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
    status: complete
  - id: "03.N"
    title: "Completion Checklist"
    status: complete
---

# Section 03: SSOT — compose-intel-summary helper

**Status:** Complete (2026-04-14) — all subsections complete, 3-round TPR converged clean, impl-hygiene review clean, section-close sweep identified 1 cross-subsection tooling opportunity tracked in §07.4.
**Goal:** Establish `.claude/skills/dual-tpr/compose-intel-summary.md` as the ONE canonical source for the intel-pre-query template. Every current consumer (18 files: 6 review-family + 12 wider-skill consumers) `@`-includes it. Additional migrations are planned per §05 (`/verify-tpr`, `/sync-claude`, `/fix-next-bug` — these skills will add intel reconnaissance) and §07 (`.claude/hooks/pre-review-intel.sh` — a hook-family consumer that does not yet exist). The harness splices the included file into the prompt at expansion time — so updates to the template automatically propagate to all consumers without drift.

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

Dual-source TPR run on 2026-04-14 (scratch dir `/tmp/ori-tpr-a4vtapRS`): codex walltime 547s / gemini walltime 176s. `ASYMMETRY: HIGH` but both reviewers independently identified the same core DRIFT class (SSOT Step F vs actual consumer usage) — agreement in spirit, not in exact (location, title). Codex found 3 additional findings gemini missed. All findings verified against the actual code before acceptance.

- [x] `[TPR-03-001-codex][medium]` `.claude/skills/rosetta-test/SKILL.md:75` — Convert the four inline @-include mentions into real standalone splice sites.
  Resolved: Fixed on 2026-04-14. All four files (`tp-help/SKILL.md:98`, `add-bug/SKILL.md:90`, `improve-tooling/SKILL.md:89`, `rosetta-test/SKILL.md:75`) rewritten so the `@`-include appears on its own line with surrounding local context before/after. Invariant `grep -c '^[[:space:]]*@\.claude/skills/dual-tpr/compose-intel-summary\.md$' .claude/ -r` now returns 18 files (matching the 18 consumer count exactly).
  Evidence: `tp-help/SKILL.md:98`, `add-bug/SKILL.md:90`, `improve-tooling/SKILL.md:89`, and `rosetta-test/SKILL.md:75` embed `@.claude/skills/dual-tpr/compose-intel-summary.md` mid-sentence inside prose/bullets rather than on their own line. The other 14 consumer sites use standalone `@`-include directives. `polling-protocol.md:108` documents `@`-include as a full-content splice; mid-sentence placements are not valid include boundaries.
  Impact: GAP:ssot-splice-point — if the documented splice semantics apply strictly, these four consumers either expand into malformed prompt text or stop being true SSOT consumers, overstating the 18-consumer migration claim.
  Required plan update: Convert each of the four inline mentions into a standalone `@`-include line with surrounding local context before and after.
  Basis: inference. Confidence: medium.
- [x] `[TPR-03-002-codex][medium]` `.claude/skills/dual-tpr/compose-intel-summary.md:4` — Restrict the SSOT consumer roster to actual current adopters.
  Resolved: Fixed on 2026-04-14. SSOT header rewritten to enumerate the 18 current consumers (6 review-family + 12 wider-skill) explicitly and move the future-intended consumers (`/verify-tpr`, `/sync-claude`, `/fix-next-bug`, `.claude/hooks/pre-review-intel.sh`) into a distinct "Planned future consumers" section with pointers to §05 and §07 where their migrations are tracked.
  Evidence: The SSOT header claims coverage over `/verify-tpr`, `/sync-claude`, `/fix-next-bug`, and `.claude/hooks/pre-review-intel.sh`. Verified: `verify-tpr/SKILL.md`, `sync-claude/SKILL.md`, `fix-next-bug/SKILL.md` contain zero `compose-intel-summary` references, and the hook path does not exist in the repo.
  Impact: DRIFT:ssot-consumer-inventory — the canonical file overstates adoption, inviting LEAK:scattered-knowledge when readers assume nonexistent adopters are already migrated.
  Required plan update: Trim the SSOT Consumers section to the actual current adopters (18 files, enumerated). Move `/verify-tpr`, `/sync-claude`, `/fix-next-bug`, and the hook into a distinct "Future intended consumers" list with pointers to §05/§07 where their migrations are planned.
  Basis: direct_file_inspection. Confidence: high.
- [x] `[TPR-03-003-codex][low]` `.claude/skills/dual-tpr/compose-intel-summary.md:75` — Synchronize Step F with the migrated consumer extensions.
  Resolved: Fixed on 2026-04-14. Step F rewritten as the canonical "Consumer extension registry" with one entry per consumer, each listing the consumer's actual `scripts/intel-query.sh` invocations verbatim. Added `/design-pattern-review` entry (previously missing). Corrected `/fix-bug` entry (added `search`/`fixed`). Corrected `/rosetta-test` entry (added `file-symbols`/`search`). Added a Registry Contract paragraph stating that any consumer adding/removing queries MUST update Step F in the same commit — codifying this as an SSOT obligation to prevent future `DRIFT:intel-extension-registry`.
  Evidence: Step F says `/fix-bug` extends the base protocol with `callers` + `similar` only, but the migrated `fix-bug/SKILL.md:111-114` uses `search` + `fixed` + `similar`. Step F has no `/design-pattern-review` entry even though that consumer's text explicitly labels its queries "per SSOT Step F". The shared `code-journey`/`rosetta-test` bullet omits `rosetta-test`'s `search "<failure mode>"` query. `[TPR-03-001-gemini]` surfaced the same drift class independently.
  Impact: DRIFT:intel-extension-registry — Step F is no longer a faithful registry of consumer-specific deviations, so future migrations could silently lose behavior or copy the wrong extension set.
  Required plan update: Rewrite Step F as a maintained registry, reading each consumer's actual queries directly and recording them verbatim. Add missing entries (`/design-pattern-review`) and correct incomplete entries (`/fix-bug`, `/rosetta-test`).
  Basis: git_history. Confidence: high.
- [x] `[TPR-03-004-codex][medium]` `plans/query-intel-adoption/section-03-compose-intel-summary-ssot.md:4` — Refresh section 03 status and success criteria after the 18-consumer expansion.
  Resolved: Fixed on 2026-04-14. Frontmatter `status` → `in-progress` (§03.R/§03.N still pending). `goal` rewritten to reflect 18-consumer scope. `success_criteria` updated with true consumer count, new 3-file invariant (SSOT + 2 teaching surfaces), and Step F registry mention. Body intro updated to "**Status:** In Progress (§03.1 / §03.2 / §03.3 complete; §03.R triage in progress; §03.N pending)" with accurate file counts. Completion Checklist migrated from "6 consumers" to "18 consumers" with correct invariant; Exit Criteria rewritten to reflect the 3-file outcome.
  Evidence: Frontmatter and body still say `status: not-started` and "6 consumers" (success_criteria line 9, body line 40-41). §03.2 and §03.3 record 18 migrated consumers and a 3-file `intel-query.sh status` invariant. The Completion Checklist and Exit Criteria still reference the obsolete 6-consumer / exactly-one-file targets.
  Impact: DRIFT:plan-state and DRIFT:mission-criteria — `/continue-roadmap` and human readers will see the section as unstarted and scoped to the old 6-consumer target.
  Required plan update: Update frontmatter `status` → `in-progress` (§03.R/§03.N still pending), success_criteria to "18 consumers", body intro to reflect true scope, Completion Checklist consumer count, and Exit Criteria invariant text.
  Basis: direct_file_inspection. Confidence: high.
- [x] `[TPR-03-001-gemini][medium]` `.claude/skills/dual-tpr/compose-intel-summary.md:88` — Sync Step F domain extensions with actual consumer usage. (Semantic agreement with `[TPR-03-003-codex]` — different location/title, same root cause.)
  Resolved: Fixed on 2026-04-14. Same fix as `[TPR-03-003-codex]`: Step F rewritten as a maintained registry matching each consumer's verbatim query set.
  Evidence: Step F lists `/review-bugs` using `search` + `fixed`, but actual usage is `search` + `fixed` + `callers` + `file-symbols` + `similar`. `/rosetta-test` lists `symbols` + `callers`/`callees` + `similar`, actual usage adds `file-symbols` + `search`. `/fix-bug` lists `callers` + `similar`, actual usage is `search` + `fixed` + `similar`.
  Impact: DRIFT — Step F documentation is out of sync with consumer code.
  Required plan update: Rewrite Step F to accurately list the exact queries used by each consumer extension (bundled with `[TPR-03-003-codex]`).
  Basis: direct_file_inspection. Confidence: high.
- [x] `[TPR-03-002-gemini][informational]` — Positive confirmation of invariant check (3 files contain the pattern: SSOT + 2 teaching surfaces). Non-actionable, no fix needed.

### Round 2 findings (2026-04-14, `/tmp/ori-tpr-ZPNF2wWA`)

Re-review after fixes landed in commit `dc1086ce`. Dual-source run: codex walltime 308s (33 files read, thorough), gemini walltime 96s (15 files read — up from 8 in round 1, rules consulted `CLAUDE.md` + both rules, scope materially broader per the Thoroughness Re-review Directive). `ASYMMETRY: HIGH` but gemini's floor is now adequate per §6b (rules complete, scope covers the meat of the change). Gemini emitted a clean-pass informational finding confirming fixes are sound. Codex caught 1 final low-severity DRIFT detail:

- [x] `[TPR-03-005-codex][low]` `.claude/skills/dual-tpr/compose-intel-summary.md:132` — Align `/create-plan` Step F entry with consumer's verbatim query set.
  Evidence: Step F `/create-plan` entry said "Then Step C base queries on high-signal symbols", deferring to Step C's default `--repo rust,swift,go`. But the consumer (`create-plan/SKILL.md:383`) uses `similar "<symbol>" --repo rust,swift,go,koka --limit 5` — the `koka` override was dropped from Step F's documentation.
  Impact: DRIFT:intel-extension-registry (micro) — Step F under-documented `/create-plan`'s actual prior-art surface. Maintainers reading Step F as the registry could silently lose the Koka leg of reconnaissance.
  Resolved: Fixed on 2026-04-14. Step F `/create-plan` entry now lists the `callers`/`callees`/`similar` queries verbatim with the `--repo rust,swift,go,koka` override explicitly noted, plus a parenthetical explaining why `koka` is included (plan reconnaissance benefits from Koka's effect-system prior art).
  Basis: direct_file_inspection. Confidence: high.
- [x] `[TPR-03-003-gemini][informational]` — Round 2 clean-pass confirmation. Gemini verified all 5 round-1 fixes hold, confirmed the 3-file / 18-file invariants via shell queries, and recommended "§03 may proceed to closure." Non-actionable.

### Round 3 findings (2026-04-14, `/tmp/ori-tpr-cFZdEzZU`) — CLEAN PASS

Final re-review after round 2 fix landed in commit `a54af6b6`. Dual-source run: codex walltime 127s (7 files read, 3 rules, 5 diagnostics, `basis: fresh_verification`), gemini walltime 70s (confidence 100, needs_escalation: false). `ASYMMETRY: MODERATE` — acceptable for the narrow round-2-only scope. Merge summary: 0 actionable findings, 2 informational positive confirmations (one from each reviewer). Both reviewers independently confirm: `/create-plan` Step F entry now documents the koka override verbatim, consumer matches, invariants hold (3 teaching-surface files + 18 standalone `@`-include splice lines). TPR loop complete — both reviewers report full consensus that §03 is architecturally clean.

- [x] `[TPR-03-006-codex][informational]` — Round 3 clean-pass confirmation. Codex verified the round-2 Step F /create-plan fix is isolated and invariants still hold; "On the SSOT/consumer/invariant axis, §03 is clean; remaining work is close-out bookkeeping rather than another repair cycle." `basis: fresh_verification`. Non-actionable.
- [x] `[TPR-03-004-gemini][informational]` — Round 3 clean-pass confirmation. Gemini verified the `/create-plan` Step F entry matches consumer perfectly. Invariant counts match (3 files / 18 splice lines). "`plans/query-intel-adoption` is fully clean." Non-actionable.

**TPR verdict:** Full consensus clean across 2 rounds of fixing + 1 final confirmation round. `third_party_review.status` → `resolved`.

---

## 03.N Completion Checklist

- [x] `.claude/skills/dual-tpr/compose-intel-summary.md` exists and is ≥80 lines
- [x] All 18 consumer files (listed in §03.2) `@`-include the SSOT via standalone splice lines; inlined copies removed
- [x] Invariant check: `grep -l 'scripts/intel-query.sh status' .claude/ -r` returns exactly 3 files — the SSOT itself plus `.claude/rules/intelligence.md` and `.claude/commands/query-intel.md` (legitimate teaching surfaces; no consumer copies remain)
- [x] `./test-all.sh` green (modulo pre-existing BUG-04-030 LLVM backend crash, orthogonal to §03's doc-only scope)
- [x] `python -m scripts.plan_corpus check plans/query-intel-adoption/section-03-compose-intel-summary-ssot.md` returns 0 errors
- [x] **Plan sync**:
  - [x] Section frontmatter `status` → `complete` (triage clean, 3 TPR rounds converged, hygiene clean)
  - [x] `00-overview.md` Quick Reference and mission criterion updated (18-consumer scope, §03 status Complete)
  - [x] `index.md` updated (§03 status Complete)
  - [x] §05, §06, §07 `depends_on: ["03"]` verified — SSOT exists and is stable. §07.4 added as new close-out work surfaced by section-close sweep.
- [x] `/tpr-review` passed clean — dual-source review converged across 3 rounds (round 1: 5 findings fixed; round 2: 1 Step F drift fixed; round 3: 0 actionable findings, both reviewers confirmed clean). All 18 consumers point at the same SSOT, Step F matches consumer code verbatim, invariants hold.
- [x] `/impl-hygiene-review` passed — 1 cosmetic DRIFT:style-consistency finding fixed inline (`## Banned patterns` → `## Banned Patterns` to match polling-protocol.md precedent). All 4 hygiene passes (LEAK/SSOT, Algorithmic DRY, Boundary/Flow, Surface Hygiene) clean after the fix.
- [x] `/improve-tooling` section-close sweep — Per-subsection retrospectives covered tooling gaps within each subsection (§03.2 filed a "scope-audit helper" opportunity). Cross-subsection pattern identified at sweep scope: a **SSOT-registry drift detector tool** would have caught 4 of the 6 total TPR findings across 3 rounds (Step F vs consumer query-set mismatches) in a single automated pass. Concrete implementation anchor created: `plans/query-intel-adoption` §07.4 "SSOT registry drift detector (`scripts/ssot-registry-audit.py`)". The Registry Contract clause in §03's SSOT commits this contract in prose; §07.4 commits it in code. Zero-deferral rule satisfied — cross-subsection finding has a concrete `- [ ]` anchor in §07.4.
- [x] `/sync-claude` section-close doc sync — `.claude/rules/impl-hygiene.md` §Algorithmic DRY §Precedents subsection cites the compose-intel-summary SSOT alongside polling-protocol.md
- [x] `diagnostics/repo-hygiene.sh --check` (clean modulo user's `test.py` scratch file, tracked by user separately)

**Exit Criteria:** Exactly 3 files under `.claude/` contain the `scripts/intel-query.sh status` pattern — the SSOT itself (`.claude/skills/dual-tpr/compose-intel-summary.md`) plus two legitimate teaching surfaces (`.claude/rules/intelligence.md` canonical when-to-query rule and `.claude/commands/query-intel.md` command wrapper). All 18 current consumers reference the SSOT via standalone `@`-include splice lines. Step F registry in the SSOT matches each consumer's actual query set verbatim (registry contract). `.claude/rules/impl-hygiene.md` §Algorithmic DRY §Precedents subsection cites the consolidation as a second SSOT-via-`@`-include precedent alongside `polling-protocol.md`. `./test-all.sh` green modulo pre-existing unrelated failures.
