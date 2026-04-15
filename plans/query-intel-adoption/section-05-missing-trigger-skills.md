---
section: "05"
title: "Missing-trigger skills & commands"
status: in-progress
reviewed: false
goal: "Add a concrete graph-query workflow step to every review/investigation skill and command currently lacking one"
success_criteria:
  - "3 skills (verify-tpr, sync-claude, fix-next-bug) each have a numbered workflow step that runs graph queries — not a token bullet"
  - "3 commands (sync-spec, sync-grammar, verify-roadmap) each have a Step/item that runs graph queries before their primary work"
  - "Every new step @-includes `.claude/skills/dual-tpr/compose-intel-summary.md` instead of inlining the pre-query pattern"
  - "Satisfies mission criterion: 3 gap skills + 3 gap commands each include a concrete graph-query workflow step"
inspired_by:
  - "`.claude/skills/tpr-review/SKILL.md` Step 0.75 — gold-standard concrete-workflow integration (codex finding TPR-XX-006 informational)"
  - "`.claude/skills/dual-tpr/compose-intel-summary.md` — the SSOT consumers will @-include (§03)"
  - "TPR findings codex-010/011/012/016/017/018, gemini-006, gemini-007"
depends_on: ["03"]
third_party_review:
  status: resolved
  updated: 2026-04-14
sections:
  - id: "05.1"
    title: "Skill edits — verify-tpr, sync-claude, fix-next-bug"
    status: not-started
  - id: "05.2"
    title: "Command edits — sync-spec, sync-grammar, verify-roadmap"
    status: not-started
  - id: "05.3"
    title: "Cross-reference audit and inventory updates"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Missing-trigger skills & commands

**Status:** Not Started
**Goal:** Six surfaces (3 skills + 3 commands) currently have NO concrete graph-query workflow step. Each gets one, using the SSOT helper from §03. `/tp-help` was already migrated in §03 (it has `@.claude/skills/dual-tpr/compose-intel-summary.md` at `.claude/skills/tp-help/SKILL.md:100` with a Step F extension registered in `compose-intel-summary.md:176-178`).

**Context:** Per the 2026-04-14 TPR, skill/command authorship has been uneven — the review-family gold standard (`tpr-review` Step 0.75, `review-work` Step 1.5) established a concrete workflow, but several peer skills never caught up. `verify-tpr` triages TPR findings without blast-radius context; `sync-claude` runs doc checks without querying `file-symbols` on changed crates to confirm docs still match (per `.claude/rules/intelligence.md:59`); `fix-next-bug` hands off to /fix-bug with no graph-derived symbol context for priority/scope assessment. On the command side, `sync-spec`, `sync-grammar`, and `verify-roadmap` have no mention at all. Each of these is a GAP:missing-trigger finding.

**Reference implementations:**
- **Ori** `.claude/skills/tpr-review/SKILL.md` Step 0.75 — concrete-workflow pattern: availability check -> run query -> parse -> inject into prompt
- **Ori** `.claude/commands/review-work.md:71-74` — one of today's inlined patterns; after §03 lands, this becomes an `@`-include and the new §05 additions follow the same shape
- **Ori** `.claude/skills/tp-help/SKILL.md:98-102` — already migrated in §03; uses `@`-include + Step F extension for `callers`/`callees`/`similar`
- **TPR finding provenance**: codex-010 (verify-tpr), 011 (sync-claude), 012 (fix-next-bug), 016 (sync-spec), 017 (sync-grammar), 018 (verify-roadmap); gemini-006/007

**Depends on:** Section 03 (the SSOT helper must exist before §05 can `@`-include it).

---

## 05.1 Skill edits — verify-tpr, sync-claude, fix-next-bug

**File(s):** 3 SKILL.md files

Each skill gets a new Step N (numbered to fit its existing workflow) that runs the graph-first query via the SSOT helper. Step text is skill-specific (the QUESTION the graph is answering differs per skill) but the mechanism is the same.

- [ ] **`.claude/skills/verify-tpr/SKILL.md`** — Insert a step between Step 2 (Read the Section File) and Step 3 (Triage Each Finding). The step runs blast-radius queries only for HIGH-SEVERITY findings or findings where the blast radius is ambiguous (not for every single finding, which would cause timeout/bloat). The step informs triage decisions, not replaces them:

  ```markdown
  ### Step 2.5: Blast-Radius Query on High-Signal Findings (MANDATORY)

  Before triaging findings in Step 3, identify which findings are high-severity
  or cite symbols whose blast radius is ambiguous (i.e., you cannot tell from
  the finding text alone whether the cited symbol is called by 2 or 200 sites).

  For each such finding, run the graph-first protocol:

  @.claude/skills/dual-tpr/compose-intel-summary.md

  Query with the finding's cited symbol. Example: finding cites `resolve_fully`
  — run `scripts/intel-query.sh --human callers "resolve_fully" --repo ori` to
  see how many sites consume the behavior the finding questions. A finding
  against a symbol with 20+ callers deserves more scrutiny than one with 2
  callers. Use the result to CALIBRATE accept/reject decisions in Step 3 —
  not as authority.

  Skip this step for [low]-severity findings with clearly-scoped symbols
  (e.g., a test helper function, a local formatting issue). The goal is
  informed triage, not query exhaustion.
  ```

- [ ] **`.claude/skills/sync-claude/SKILL.md`** — Insert between Step 1 (Identify What Changed) and Step 2 (Map Changes to Artifacts). The step implements the intelligence.md workflow at line 59: run `file-symbols` on changed crates to confirm docs still match. This is NOT a meta-audit of intel references in docs — it is a crate-symbol inventory to detect doc drift:

  ```markdown
  ### Step 1.5: Intelligence-Graph Symbol Inventory (MANDATORY when graph available)

  After identifying changed crates in Step 1, query the intelligence graph to
  get a complete symbol inventory for each changed crate. This reveals symbols
  that may have been added, renamed, or removed — and that doc artifacts might
  not reflect.

  @.claude/skills/dual-tpr/compose-intel-summary.md

  Specifically: for each crate touched in the diff, run
  `scripts/intel-query.sh --human file-symbols "<crate-path-fragment>" --repo ori`
  to get the current symbol surface. Compare against the rules file's documented
  symbols (Step 2's mapping). Symbols present in the graph but absent from the
  rules file are likely new additions that need documentation. Symbols in the
  rules file but absent from the graph may have been removed or renamed.

  This step does NOT check whether a file's references to the intelligence
  graph are current — that is a different concern. This step checks whether
  the CODE SYMBOLS documented in rules files match the actual codebase.
  ```

- [ ] **`.claude/skills/fix-next-bug/SKILL.md`** — Insert as Step 4.5 between Step 4 (Present the Selected Bug) and Step 5 (Choose Mode). The step provides a lightweight blast-radius preview to help the user assess priority and scope BEFORE choosing mode. It does NOT duplicate /fix-bug Phase 1's full investigation — /fix-bug already has its own intel queries (documented in compose-intel-summary.md Step F, lines 122-126). The Skill tool only accepts an args string (the bug ID), so this summary is for the presentation display, not passed to /fix-bug:

  ```markdown
  ### Step 4.5: Lightweight Blast-Radius Preview

  Before presenting the mode choice (Step 5), add blast-radius context to the
  bug presentation (Step 4's output). This helps the user gauge whether the bug
  is a localized fix or a cross-cutting concern:

  @.claude/skills/dual-tpr/compose-intel-summary.md

  Target the bug's repro symbol (from the bug entry's Repro or Subsystem field).
  Run `callers` to see how many sites touch the buggy code path. Append a
  one-line blast-radius note to the Step 4 presentation:

    Blast radius: <symbol> called by N sites across M modules

  This is a PREVIEW only — /fix-bug Phase 1 (investigation) runs its own
  full intelligence queries. The preview here helps the user decide between
  interactive vs. autopilot mode with better scope awareness.
  ```

  **Implementation note:** Step 4.5 is inserted between existing Steps 4 and 5. Internal references at lines 106, 127, 165 that say "Step 6" / "Step 7" remain valid because Step 4.5 does not renumber existing steps — it uses a fractional number consistent with the file's existing Step 5.5 pattern.

- [ ] Spot-check each edit: `grep -c '@.claude/skills/dual-tpr/compose-intel-summary.md' <FILE>` returns >=1 for each of the 3 files.

- [ ] **Subsection close-out (05.1)**:
  - [ ] All 3 skill files have the new Step; grep verification passes
  - [ ] Update `05.1` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 05.1** — did editing 3 skill files reveal any inconsistency in how SKILL.md files structure their numbered steps? (E.g., some use `## Step N`, others `### Step N`, others bare `## N — Title`.) If normalization would help, file an improvement ticket for a Skill-style linter, commit via `build(tooling): ...`.
  - [ ] **Run `/sync-claude` on 05.1** — 3 skill files changed. `.claude/rules/intelligence.md`'s workflow inventory (refreshed in §04.3) already anticipates these additions — verify no drift between the inventory and the actual skill steps.
  - [ ] **Repo hygiene check**.

---

## 05.2 Command edits — sync-spec, sync-grammar, verify-roadmap

**File(s):** 3 command files

Commands are shorter than skills and have DIFFERENT structural formats than skills. Each insertion must match the target command's actual document structure:
- `sync-spec.md` and `sync-grammar.md` use numbered lists under `## Update Process` (NOT `## Step N` H2 headers)
- `verify-roadmap.md` has a 2-phase architecture with review agents (Phase 1) and update agents (Phase 2). Only Phase 1 review agents need intel — it should be injected into the Phase 1 Step 2 review-agent prompt block (lines ~93-124), not as a standalone top-level step.

- [ ] **`.claude/commands/sync-spec.md`** — Insert as a new numbered item in the `## Update Process` section (currently items 1-6). Add as item 1.5 or renumber to insert before current item 1 ("Identify affected spec files"). The format MUST be a numbered list item matching the existing structure, NOT an H2 `## Step 0` header that would clash with the document's format:

  ```markdown
  1. **Query the intelligence graph for affected symbols** — before identifying
     spec files, run a blast-radius check on every symbol the spec-edit might
     affect:

     @.claude/skills/dual-tpr/compose-intel-summary.md

     Query `callers` on symbols referenced in the spec change. If the edit
     changes operator-rules.md section X, run
     `scripts/intel-query.sh --human callers "<relevant symbol>" --repo ori`
     to see every site that interprets the rule. This prevents silent behavior
     drift when a spec change ships without updating an implementation call site.
  ```

  Renumber subsequent items (current 1->2, 2->3, etc.) to maintain sequential numbering.

- [ ] **`.claude/commands/sync-grammar.md`** — Insert as a new numbered item in the `## Update Process` section (currently items 1-5). Add before current item 1 ("Read the current grammar.ebnf"). The format MUST be a numbered list item matching the existing structure, NOT an H2 header:

  ```markdown
  1. **Inventory parser/lexer types via the intelligence graph** — grammar
     changes affect parser and lexer types. Before reading grammar.ebnf:

     @.claude/skills/dual-tpr/compose-intel-summary.md

     Query `file-symbols "compiler/ori_parse/"` and `file-symbols
     "compiler/ori_lexer/"` to inventory parser/lexer types that consume the
     grammar. Flag any grammar production whose implementation symbol is not
     covered — that is a parse-site gap.
  ```

  Renumber subsequent items (current 1->2, 2->3, etc.) to maintain sequential numbering.

- [ ] **`.claude/commands/verify-roadmap.md`** — Inject into the Phase 1 review-agent prompt block ONLY. The file is 903 lines with a 2-phase architecture: Phase 1 (review agents, read-only) and Phase 2 (update agents, write section files). Only Phase 1 review agents need the intel summary. Insert as a standalone block AFTER the "These files contain CRITICAL context..." paragraph (which closes the file-reading list) and BEFORE `#### Step 3: Supervisor Monitoring`. Do NOT insert it as a numbered item inside the "Read these files" list — that list is for static file reading, not dynamic queries:

  ```markdown
  **Intelligence Reconnaissance (MANDATORY when graph available):**

  Before verifying items, run the intelligence graph protocol for the
  section's scope:

  @.claude/skills/dual-tpr/compose-intel-summary.md

  Target the roadmap section's declared scope (crates / subsystems named in
  the section's title or description). Run `file-symbols` on those crates
  and `callers`/`callees` on high-signal symbols. This gives the review
  agent ambient blast-radius context before it starts verifying items, which
  shortens ramp-up and reveals cross-crate dependencies the section file
  may not list.
  ```

  This is a review-agent instruction placed AFTER the static file-reading block but BEFORE the agent begins verification work. It goes INSIDE the agent prompt template, not alongside the Phase 1/Phase 2 architecture.

  **While editing:** also fix the stale rules-corpus instruction in the same prompt block — line 108 says "ALL 20 rules files" and enumerates a bulleted list of specific filenames below it. Replace the hardcoded count AND the bulleted list with a generic instruction: `"ALL rules files in /home/eric/projects/ori_lang/.claude/rules/ — read every file"` (no hardcoded count, no hardcoded list). This prevents the instruction from going stale again as rules are added.

- [ ] Spot-check: `grep -c '@.claude/skills/dual-tpr/compose-intel-summary.md' <FILE>` returns >=1 for each of the 3 commands.

- [ ] **Subsection close-out (05.2)**:
  - [ ] All 3 command files edited; grep verification passes
  - [ ] Update `05.2` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 05.2** — each command needed genuinely different insertion because their document structures differ (numbered list vs. agent prompt injection). Confirm no boilerplate emerged that should be consolidated.
  - [ ] **Run `/sync-claude` on 05.2** — command-file edits don't generally affect CLAUDE.md, but `.claude/rules/intelligence.md` workflow inventory should list sync-spec/sync-grammar/verify-roadmap (done in §04.3 — verify).
  - [ ] **Repo hygiene check**.

---

## 05.3 Cross-reference audit and inventory updates

**File(s):** 4 files need inventory updates (with multiple edits to compose-intel-summary.md) + grep verification

After §05.1 and §05.2 land, multiple inventory/tracking files become stale. This subsection updates them all.

**Sequencing note:** The intelligence.md workflow description updates and index.md keyword cluster updates SHOULD be committed in the SAME commit as the corresponding consumer edits in §05.1/§05.2 (not after). This prevents a window where intelligence.md describes one query shape while the actual consumer uses a different shape. During §05.1 implementation, update the intelligence.md entries for the 3 skills being edited; during §05.2 implementation, update the entries for the 3 commands. The grep audit and SSOT updates below can happen after both land.

### Grep consumer enumeration

- [ ] Run (excludes worktree mirrors to avoid false positives):
  ```
  rg -l --glob '!worktrees/**' '@\.claude/skills/dual-tpr/compose-intel-summary\.md' .claude/
  # Expect: 25 files total (24 consumers + the SSOT itself)
  # Breakdown: 18 existing (from §03) + 3 skills (§05.1) + 3 commands (§05.2) + 1 SSOT
  ```

### Inventory updates (same-commit as the skill/command edits)

- [ ] **`.claude/rules/intelligence.md`** — Remove `[planned -- tracked by plans/query-intel-adoption §05]` markers AND update the workflow descriptions to match the refined §05 designs:
  - `verify-tpr` — remove `*[planned]*` tag; update description to match §05.1: blast-radius via `callers` on high-severity findings or findings where the blast radius is ambiguous (not all findings, not `callees`/`similar`)
  - `sync-claude` — remove `*[planned]*` tag; update description to match §05.1: `file-symbols` on changed crates for doc-symbol drift detection (not "intelligence-surface drift audit")
  - `fix-next-bug` — remove `*[planned]*` tag; update description to match §05.1: lightweight `callers`-only blast-radius preview for scope assessment (not `callees`/`similar`)
  - `sync-spec` — remove `*[planned]*` tag; update to match §05.2: `callers` on affected symbols before spec edits
  - `sync-grammar` — remove `*[planned]*` tag; update to match §05.2: `file-symbols` on `ori_parse/`/`ori_lexer/` for parser/lexer type inventory
  - `verify-roadmap` — remove `*[planned]*` tag; update to match §05.2: `file-symbols`/`callers`/`callees` on section scope crates (Phase 1 review agents only)

- [ ] **`.claude/skills/dual-tpr/compose-intel-summary.md`** — Update the "Planned future consumers" list (lines 18-28):
  - Move `verify-tpr`, `sync-claude`, `fix-next-bug` from "Planned future consumers" to the "Wider skill consumers" list (or "Current consumers" header)
  - Move `sync-spec`, `sync-grammar`, `verify-roadmap` from "Planned future consumers" to the "Current consumers" list under a new "Command consumers" group
  - After migration, the "Planned future consumers" section should list ONLY `.claude/hooks/pre-review-intel.sh` (tracked by §07)
  - Update the consumer count header from "Current consumers (18" to "Current consumers (24"

- [ ] **`.claude/skills/dual-tpr/compose-intel-summary.md` Step F** — Add consumer extension registry entries for the 6 new consumers. Each consumer's extension entry documents which specific `scripts/intel-query.sh` subcommands it uses beyond the base protocol. Per the Step F registry contract (lines 187-195), this MUST happen in the same commit as the skill/command edits:

  **Entries to add:**

  ```markdown
  **TPR/verification consumers:**

  - **`/verify-tpr`** (Step 2.5) — per-finding blast-radius:
    - `callers "<finding symbol>" --repo ori` (high-severity or ambiguous-blast-radius findings)

  **Doc-sync consumers:**

  - **`/sync-claude`** (Step 1.5) — crate symbol inventory:
    - `file-symbols "<crate-path-fragment>" --repo ori` (per changed crate)

  **Bug-workflow consumers (additional):**

  - **`/fix-next-bug`** (Step 4.5) — lightweight blast-radius preview:
    - `callers "<repro symbol>" --repo ori`

  **Spec/grammar consumers:**

  - **`/sync-spec`** (Update Process item 1) — blast-radius before spec edits:
    - `callers "<affected symbol>" --repo ori`

  - **`/sync-grammar`** (Update Process item 1) — parser/lexer type inventory:
    - `file-symbols "compiler/ori_parse/" --repo ori`
    - `file-symbols "compiler/ori_lexer/" --repo ori`

  **Roadmap consumers:**

  - **`/verify-roadmap`** (Phase 1, Step 2 agent prompt) — review-agent context:
    - `file-symbols "<section scope crate>" --repo ori`
    - `callers`/`callees` on high-signal symbols
  ```

- [ ] **`plans/query-intel-adoption/00-overview.md`** — Update the mission criterion text at line 23:
  - Change "4 gap skills" to "3 gap skills" (tp-help was completed in §03)
  - Update the consumer fan-out diagram: MOVE `tp-help` from the `[§05]` group to the `[§03]` group (it was migrated in §03, but the diagram still lists it under §05). Do NOT simply delete it — that would under-report consumers.

- [ ] **`plans/query-intel-adoption/index.md`** — Update the Section 05 keyword cluster:
  - Remove line 90 (`tp-help/SKILL.md, elevate token bullet to Step 2 workflow`) — this work was completed in §03
  - Update line 88: change `intelligence-surface drift` to `file-symbols crate-symbol inventory` (matches refined §05.1 sync-claude design)
  - Update line 89: change `blast-radius + similar on repro symbol` to `callers-only lightweight blast-radius preview` (matches refined §05.1 fix-next-bug design)
  - The keyword cluster should reflect 3 skills + 3 commands, not 4 skills

- [ ] **`.claude/skills/dual-tpr/compose-intel-summary.md` `## Consumers` section** — Document the full consumer list (the SSOT should know who uses it). Enumerate all 24 consumers by source section (§03 original 18 + §05's 6 additions).

- [ ] Verify no CONSUMER file contains open-coded pre-query/injection blocks or unauthorized `scripts/intel-query.sh status` strings without the `@`-include. Note: `.claude/rules/intelligence.md` and `.claude/commands/query-intel.md` are legitimate non-consumer teaching surfaces that may reference `intel-query.sh` directly (per SSOT lines 28-31) — do NOT flag these as LEAKs.

- [ ] **Subsection close-out (05.3)**:
  - [ ] Consumer list exhaustive; SSOT's Consumers section populated with all 24+ consumers
  - [ ] All 4 inventory files updated with multiple edits (intelligence.md workflow descriptions, compose-intel-summary.md header + Step F + Consumers section, 00-overview.md diagram + mission criterion, index.md keyword cluster)
  - [ ] Update `05.3` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 05.3** — the grep invariant is running twice now (§03.3 and §05.3). Should this be a lefthook pre-commit hook that fails the commit if any `.claude/` file contains the pre-query pattern without the SSOT include? Commit via `build(ci): ...` if matured.
  - [ ] **Run `/sync-claude` on 05.3** — the SSOT Consumers section is a living artifact; future sections (§06, §07) will add themselves. Confirm the update process is documented (the SSOT should explain "how to add yourself as a consumer").
  - [ ] **Repo hygiene check**.

---

## 05.R Third Party Review Findings

- [x] `[TPR-05-001-codex][medium]` `section-05:217` — Add index.md to mandatory 05.3 inventory update set.
  Resolved: Fixed on 2026-04-14. Added explicit index.md update item in §05.3 to remove stale tp-help keyword cluster.
- [x] `[TPR-05-002-codex][medium]` `section-05:223` — Scope consumer-count grep to primary .claude tree.
  Resolved: Fixed on 2026-04-14. Replaced `grep -rln` with `rg -l --glob '!worktrees/**'` to avoid worktree noise.
- [x] `[TPR-05-003-codex][medium]` `section-05:316` — Replace stale plan_corpus.py checklist command.
  Resolved: Fixed on 2026-04-14. Changed to `python -m scripts.plan_corpus check`.
- [x] `[TPR-05-004-codex][low]` `section-05:288` — Narrow @-include audit to actual consumers.
  Resolved: Fixed on 2026-04-14. Excluded intelligence.md and query-intel.md as legitimate teaching surfaces.
- [x] `[TPR-05-001-gemini][medium]` `section-05:206` — Fix inaccurate 00-overview diagram update instruction.
  Resolved: Fixed on 2026-04-14. Changed instruction to MOVE tp-help from §05 to §03 group, not just remove.
- [x] `[TPR-05-002-gemini][medium]` `section-05:145` — Fix verify-roadmap prompt injection to avoid list semantic collision.
  Resolved: Fixed on 2026-04-14. Changed to standalone block after the static file-reading list.
- [x] `[TPR-05-001-codex-r2][medium]` `section-05:235` — Expand intelligence.md update to rewrite workflow descriptions, not just remove tags.
  Resolved: Fixed on 2026-04-14. Task now updates workflow bullets to match refined §05 query designs.
- [x] `[TPR-05-002-codex-r2][medium]` `section-05:289` — Refresh index.md keyword cluster descriptions and add to close-out checklist.
  Resolved: Fixed on 2026-04-14. Task broadened + index.md added to close-out file count.
- [x] `[TPR-05-001-codex-r3][medium]` `intelligence.md:44` — Sync intelligence.md workflow bullets to refined §05 designs.
  Resolved: Fixed on 2026-04-14. Added sequencing note: intelligence.md updates co-committed with consumer edits.
- [x] `[TPR-05-002-codex-r3][medium]` `index.md:87` — Refresh §05 keyword cluster to current 3-skill scope.
  Resolved: Fixed on 2026-04-14. Same sequencing note covers index.md updates.
- [x] `[TPR-05-003-codex-r3][medium]` `section-05:187` — Fix stale "20 rules files" in verify-roadmap prompt block.
  Resolved: Fixed on 2026-04-14. Added task to replace hardcoded count with generic instruction.
- `[TPR-05-001-gemini-r3][informational]` Clean round 3 re-review verification — all fixes confirmed.
- [x] `[TPR-05-001-codex-r4][medium]` `section-05:239` — verify-tpr scope mismatch: §05.3 said "high-severity only" but §05.1 says "high-severity or ambiguous".
  Resolved: Fixed on 2026-04-14. Updated both intelligence.md update and Step F entry to "high-severity or ambiguous".
- [x] `[TPR-05-002-codex-r4][medium]` `section-05:345` — Completion checklist status-string invariant impossible as written.
  Resolved: Fixed on 2026-04-14. Scoped to consumer files; listed 3 approved surfaces.
- [x] `[TPR-05-003-codex-r4][low]` `section-05:222` — File count says 5 but only 4 distinct files.
  Resolved: Fixed on 2026-04-14. Corrected to "4 files with multiple edits".
- `[TPR-05-001-gemini-r4][informational]` verify-roadmap bulleted list should also be removed — addressed.
- `[TPR-05-002-gemini-r4][informational]` verify-tpr Step F entry scope — same as TPR-05-001-codex-r4.

---

## 05.N Completion Checklist

- [ ] All 3 skill files have a numbered step with `@.claude/skills/dual-tpr/compose-intel-summary.md`
- [ ] All 3 command files have an insertion with `@.claude/skills/dual-tpr/compose-intel-summary.md` (matching each command's structural format — numbered list items for sync-spec/sync-grammar, agent prompt injection for verify-roadmap)
- [ ] No CONSUMER file in `.claude/` contains an inlined `scripts/intel-query.sh status` block — the 3 approved surfaces (SSOT `compose-intel-summary.md`, `intelligence.md`, `query-intel.md`) are legitimate and expected
- [ ] SSOT's `## Consumers` section lists all 24+ consumers
- [ ] SSOT Step F registry has entries for all 6 new consumers
- [ ] `.claude/rules/intelligence.md` has no remaining `[planned]` markers for the 6 migrated surfaces
- [ ] `plans/query-intel-adoption/00-overview.md` mission criterion updated ("3 gap skills" not "4")
- [ ] `./test-all.sh` green
- [ ] `python -m scripts.plan_corpus check plans/query-intel-adoption/section-05-missing-trigger-skills.md` returns 0 errors
- [ ] **Plan sync**:
  - [ ] Section frontmatter -> `complete`
  - [ ] `00-overview.md` Quick Reference and mission criteria updated
  - [ ] `index.md` updated
- [ ] `/tpr-review` passed — reviewers confirm each added Step is actionable, not ceremonial
- [ ] `/impl-hygiene-review` passed — no new inlined patterns; @-includes only
- [ ] `/improve-tooling` section-close sweep
- [ ] `/sync-claude` section-close doc sync
- [ ] `diagnostics/repo-hygiene.sh --check`

**Exit Criteria:** Every review/investigation skill or command that benefits from the graph now has a concrete Step N that runs queries via the SSOT helper. No inlined query blocks outside the SSOT. `./test-all.sh` green.
