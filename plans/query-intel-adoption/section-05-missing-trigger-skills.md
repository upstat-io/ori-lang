---
section: "05"
title: "Missing-trigger skills & commands"
status: not-started
reviewed: false
goal: "Add a concrete graph-query workflow step to every review/investigation skill and command currently lacking one"
success_criteria:
  - "4 skills (verify-tpr, sync-claude, fix-next-bug, tp-help) each have a numbered workflow step that runs graph queries — not a token bullet"
  - "3 commands (sync-spec, sync-grammar, verify-roadmap) each have a Step N that runs graph queries before their primary work"
  - "Every new step @-includes `.claude/skills/dual-tpr/compose-intel-summary.md` instead of inlining the pre-query pattern"
  - "Satisfies mission criterion: 4 gap skills + 3 gap commands each include a concrete graph-query workflow step"
inspired_by:
  - "`.claude/skills/tpr-review/SKILL.md` Step 0.75 — gold-standard concrete-workflow integration (codex finding TPR-XX-006 informational)"
  - "`.claude/skills/dual-tpr/compose-intel-summary.md` — the SSOT consumers will @-include (§03)"
  - "TPR findings codex-010/011/012/013/016/017/018, gemini-006, gemini-007"
depends_on: ["03"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Skill edits — verify-tpr, sync-claude, fix-next-bug, tp-help"
    status: not-started
  - id: "05.2"
    title: "Command edits — sync-spec, sync-grammar, verify-roadmap"
    status: not-started
  - id: "05.3"
    title: "Cross-reference audit"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Missing-trigger skills & commands

**Status:** Not Started
**Goal:** Seven surfaces (4 skills + 3 commands) currently have NO concrete graph-query workflow step. Each gets one, using the SSOT helper from §03. Where a skill has a token bullet mentioning intel (e.g., `/tp-help` per codex finding TPR-XX-013), elevate it to a Step-N workflow with explicit query commands.

**Context:** Per the 2026-04-14 TPR, skill/command authorship has been uneven — the review-family gold standard (`tpr-review` Step 0.75, `review-work` Step 1.5) established a concrete workflow, but several peer skills never caught up. `verify-tpr` triages TPR findings without blast-radius context; `sync-claude` runs doc checks without checking intelligence-surface drift; `fix-next-bug` hands off to /fix-bug with no graph-derived symbol context; `/tp-help` mentions intel only as an optional bullet. On the command side, `sync-spec`, `sync-grammar`, and `verify-roadmap` have no mention at all. Each of these is a GAP:missing-trigger finding.

**Reference implementations:**
- **Ori** `.claude/skills/tpr-review/SKILL.md` Step 0.75 — concrete-workflow pattern: availability check → run query → parse → inject into prompt
- **Ori** `.claude/commands/review-work.md:71-74` — one of today's inlined patterns; after §03 lands, this becomes an `@`-include and the new §05 additions follow the same shape
- **TPR finding provenance**: codex-010 (verify-tpr), 011 (sync-claude), 012 (fix-next-bug), 013 (tp-help), 016 (sync-spec), 017 (sync-grammar), 018 (verify-roadmap); gemini-006/007

**Depends on:** Section 03 (the SSOT helper must exist before §05 can `@`-include it).

---

## 05.1 Skill edits — verify-tpr, sync-claude, fix-next-bug, tp-help

**File(s):** 4 SKILL.md files

Each skill gets a new Step N (numbered to fit its existing workflow) that runs the graph-first query via the SSOT helper. Step text is skill-specific (the QUESTION the graph is answering differs per skill) but the mechanism is the same.

- [ ] **`.claude/skills/verify-tpr/SKILL.md`** — Insert a Step before finding-triage:

  ```markdown
  ## Step N — Blast-radius query on each surfaced finding (MANDATORY)

  For each finding, before deciding accept/reject:

  @.claude/skills/dual-tpr/compose-intel-summary.md

  Query with the finding's cited symbol. Example: finding cites `resolve_fully`
  — run `scripts/intel-query.sh --human callers "resolve_fully" --repo ori` to
  see how many sites consume the behavior the finding questions. A finding
  against a symbol with 20+ callers deserves more scrutiny than one with 2
  callers. Use the result to CALIBRATE accept/reject — not as authority.
  ```

- [ ] **`.claude/skills/sync-claude/SKILL.md`** — Insert a Step after the standard diff analysis:

  ```markdown
  ## Step N — Intelligence-surface drift audit

  If the diff touches ANY rule file, skill file, or command file, audit whether
  that file's references to the intelligence graph are current:

  @.claude/skills/dual-tpr/compose-intel-summary.md

  Specifically check: does the changed file cite an `.claude/rules/intelligence.md`
  workflow that no longer exists? Does it @-include `compose-intel-summary.md`
  for the pre-query pattern (vs inlined)? Flag drift and fix as part of sync.
  ```

- [ ] **`.claude/skills/fix-next-bug/SKILL.md`** — Insert before the hand-off to /fix-bug:

  ```markdown
  ## Step N — Blast-radius reconnaissance on the selected bug

  Before handing off to /fix-bug, run:

  @.claude/skills/dual-tpr/compose-intel-summary.md

  Target the bug's repro symbol (from the bug entry's Repro or Subsystem field).
  The summary tells /fix-bug's Phase 1 (investigation) where to look — how many
  callers touch the buggy code path, whether a similar pattern was already fixed
  in Rust/Swift/Koka, and which modules share the buggy symbol's ownership.
  Pass the summary to /fix-bug as initial context.
  ```

- [ ] **`.claude/skills/tp-help/SKILL.md`** — Elevate the existing token bullet to Step 2:

  ```markdown
  ## Step 2 — Enrich context with intel summary (MANDATORY when relevant)

  Before writing the /tp-help prompt, if the question references any Ori symbol
  or reference-compiler pattern:

  @.claude/skills/dual-tpr/compose-intel-summary.md

  Inject the resulting Intelligence Summary into the context section of the
  tp-help prompt — it gives codex AND gemini the same symbol/blast-radius
  baseline, preventing them from discovering different code paths via different
  grep heuristics.
  ```

- [ ] Spot-check each edit: `grep -c '@.claude/skills/dual-tpr/compose-intel-summary.md' <FILE>` returns ≥1 for each of the 4 files.

- [ ] **Subsection close-out (05.1)**:
  - [ ] All 4 skill files have the new Step; grep verification passes
  - [ ] Update `05.1` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 05.1** — did editing 4 skill files reveal any inconsistency in how SKILL.md files structure their numbered steps? (E.g., some use `## Step N`, others `### Step N`, others bare `## N — Title`.) If normalization would help, file an improvement ticket for a Skill-style linter, commit via `build(tooling): ...`.
  - [ ] **Run `/sync-claude` on 05.1** — 4 skill files changed. `.claude/rules/intelligence.md`'s workflow inventory (refreshed in §04.3) already anticipates these additions — verify no drift between the inventory and the actual skill steps.
  - [ ] **Repo hygiene check**.

---

## 05.2 Command edits — sync-spec, sync-grammar, verify-roadmap

**File(s):** 3 command files

Commands are shorter than skills but follow the same step-insertion pattern.

- [ ] **`.claude/commands/sync-spec.md`** — Insert a Step 0 / pre-step before spec editing:

  ```markdown
  ## Step 0 — Graph-first reconnaissance before spec edits

  Spec edits alter normative language used across the compiler. Before writing:

  @.claude/skills/dual-tpr/compose-intel-summary.md

  Query `callers` on every symbol the spec-edit might affect. If the edit
  changes operator-rules.md §X, run `scripts/intel-query.sh --human callers
  "<relevant symbol>" --repo ori` to see every site that interprets the rule.
  This prevents silent behavior drift when a spec change ships without
  updating an implementation call site.
  ```

- [ ] **`.claude/commands/sync-grammar.md`** — Insert a pre-step before grammar.ebnf edits:

  ```markdown
  ## Step 0 — Symbol lookup for grammar-adjacent types

  Grammar changes affect parser and lexer types. Before editing grammar.ebnf:

  @.claude/skills/dual-tpr/compose-intel-summary.md

  Query `file-symbols "compiler/ori_parse/"` and `file-symbols
  "compiler/ori_lexer/"` to inventory parser/lexer types that consume the
  grammar. Flag any grammar production whose implementation symbol isn't
  covered — that's a parse-site gap.
  ```

- [ ] **`.claude/commands/verify-roadmap.md`** — Insert into the review-agent prompt generation step (before rule-file reading):

  ```markdown
  ## Step N — Review agents MUST query the graph before reading rules

  When generating prompts for roadmap-review agents, prepend the intel summary
  ahead of the rule-file reading instructions:

  @.claude/skills/dual-tpr/compose-intel-summary.md

  Target the roadmap section's declared scope (crates / subsystems named in the
  section's frontmatter). The agents get ambient blast-radius context before
  they start reading rules, which shortens their ramp-up.
  ```

- [ ] Spot-check: `grep -c '@.claude/skills/dual-tpr/compose-intel-summary.md' <FILE>` returns ≥1 for each of the 3 commands.

- [ ] **Subsection close-out (05.2)**:
  - [ ] All 3 command files edited; grep verification passes
  - [ ] Update `05.2` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 05.2** — were the 3 commands' Step insertions mechanically similar or did each need genuine customization? If a boilerplate-ish Step emerged, flag for consolidation (could be part of §03 or a sibling SSOT). Commit via `docs(skills): ...` or note negative.
  - [ ] **Run `/sync-claude` on 05.2** — command-file edits don't generally affect CLAUDE.md, but `.claude/rules/intelligence.md` workflow inventory should list sync-spec/sync-grammar/verify-roadmap (done in §04.3 — verify).
  - [ ] **Repo hygiene check**.

---

## 05.3 Cross-reference audit

**File(s):** N/A (verification-only)

Confirm that every file with a `@.claude/skills/dual-tpr/compose-intel-summary.md` include is discoverable via a single grep invocation, so future audits can enumerate consumers.

- [ ] Run:
  ```
  grep -rln '@.claude/skills/dual-tpr/compose-intel-summary.md' .claude/
  # Expect: at least 13 files (6 from §03 replacements + 4 skills from §05.1 + 3 commands from §05.2)
  ```
- [ ] Document the consumer list in `.claude/skills/dual-tpr/compose-intel-summary.md`'s `## Consumers` section (the SSOT should know who uses it; §03 left the section with a generic reference, §05.3 populates it with the actual consumer list).
- [ ] Verify no file with a graph-query reference is missing the `@`-include — a file that runs queries by hand without the SSOT helper is a new LEAK.

- [ ] **Subsection close-out (05.3)**:
  - [ ] Consumer list exhaustive; SSOT's Consumers section populated
  - [ ] Update `05.3` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 05.3** — the grep invariant is running twice now (§03.3 and §05.3). Should this be a lefthook pre-commit hook that fails the commit if any `.claude/` file contains the pre-query pattern without the SSOT include? Commit via `build(ci): ...` if matured.
  - [ ] **Run `/sync-claude` on 05.3** — the SSOT Consumers section is a living artifact; future sections (§06, §07) will add themselves. Confirm the update process is documented (the SSOT should explain "how to add yourself as a consumer").
  - [ ] **Repo hygiene check**.

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [ ] All 4 skill files have a numbered step with `@.claude/skills/dual-tpr/compose-intel-summary.md`
- [ ] All 3 command files have a Step with `@.claude/skills/dual-tpr/compose-intel-summary.md`
- [ ] No file in `.claude/` contains an inlined `scripts/intel-query.sh status` block (invariant check passes)
- [ ] SSOT's `## Consumers` section lists all 13+ consumers
- [ ] `./test-all.sh` green
- [ ] `python scripts/plan_corpus.py check plans/query-intel-adoption/section-05-missing-trigger-skills.md` returns 0 errors
- [ ] **Plan sync**:
  - [ ] Section frontmatter → `complete`
  - [ ] `00-overview.md` Quick Reference and mission criteria updated
  - [ ] `index.md` updated
- [ ] `/tpr-review` passed — reviewers confirm each added Step is actionable, not ceremonial
- [ ] `/impl-hygiene-review` passed — no new inlined patterns; @-includes only
- [ ] `/improve-tooling` section-close sweep
- [ ] `/sync-claude` section-close doc sync
- [ ] `diagnostics/repo-hygiene.sh --check`

**Exit Criteria:** Every review/investigation skill or command that benefits from the graph now has a concrete Step N that runs queries via the SSOT helper. No inlined query blocks outside the SSOT. `./test-all.sh` green.
