---
section: "04"
title: "Rule files — graph-first guidance at every cross-repo prior-art reference"
status: not-started
reviewed: false
goal: "Add a graph-first paragraph to every .claude/rules/*.md file that cites cross-repo prior art without pointing at the intelligence graph first"
success_criteria:
  - "10 rule files (arc.md, aims-rules.md, typeck.md, types.md, tests.md, impl-hygiene.md, canonicalization.md, patterns.md, compiler.md, intelligence.md) each contain a graph-first paragraph near their prior-art references"
  - "`.claude/rules/intelligence.md` workflow inventory refreshed to include verify-tpr, sync-claude, fix-next-bug, tp-help, sync-spec, sync-grammar, verify-roadmap as covered workflows"
  - "`grep -L 'scripts/intel-query.sh' .claude/rules/{arc,aims-rules,typeck,types,tests,impl-hygiene,canonicalization,patterns,compiler,intelligence}.md` returns empty (all 10 now reference the graph)"
  - "Satisfies mission criterion: all 10 rule files include graph-first paragraph"
inspired_by:
  - "`.claude/rules/intelligence.md` §Symbol-First Workflow — existing graph-first language; template source"
  - "TPR findings codex-005, 020, 021, 022, 023, 024, 025, 026, 027, gemini-004 [medium]"
depends_on: ["03"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Draft the graph-first paragraph template"
    status: not-started
  - id: "04.2"
    title: "Insert into 9 domain rule files"
    status: not-started
  - id: "04.3"
    title: "Refresh intelligence.md workflow inventory"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Rule files — graph-first guidance

**Status:** Not Started
**Goal:** Every rule file that cites cross-repo prior art (Rust, Swift, Koka, Lean4, Gleam, Elm, Roc, Zig, Go, TS) must point the reader at the intelligence graph BEFORE naming the manual file paths. This turns 27 domain rule files from "manual-first" into "graph-first, manual-second."

**Context:** Per the 2026-04-14 TPR verification, 27 of 28 rule files cite reference-repo paths and implementation patterns without a single mention of the intelligence graph. The graph indexes all 10 reference repos with 24K+ call edges and vector embeddings for semantic similarity — it resolves cross-repo equivalents in sub-second time. A reader following today's rule-file guidance would open 5 reference repos manually and grep; after §04 lands, the same reader runs one graph query, narrows to 2 concrete files, and then opens those. LEAK:scattered-knowledge is the finding category (codex finding IDs TPR-XX-020 through TPR-XX-025 plus 026/027).

**Reference implementations:**
- **Ori** `.claude/rules/intelligence.md:66-75` (Symbol-First Workflow): existing canonical paragraph, adapted here for insertion at other sites
- **TPR finding provenance**: codex-020 arc.md:243, codex-021 aims-rules.md:727, codex-022 typeck.md:957, codex-023 types.md:839, codex-024 tests.md:274, codex-025 impl-hygiene.md:313, codex-026 canonicalization.md:37, codex-027 patterns.md:27, codex-005 intelligence.md workflow inventory, gemini-004 general

**Depends on:** Section 03 (the paragraph cites `compose-intel-summary.md` as the canonical summary template).

---

## 04.1 Draft the graph-first paragraph template

**File(s):** Template only (no file written in this subsection)

One canonical paragraph shape used in 8 of the 9 target files (§04.2). `intelligence.md` gets a different treatment (§04.3 refreshes its workflow inventory, not a paragraph insert).

- [ ] Draft the template:

  ```markdown
  ## Graph-first, manual second

  Before reading the reference-repo paths cited in this rule file, query the
  intelligence graph:

  - `scripts/intel-query.sh --human similar "<symbol>" --repo rust,swift,koka,lean4 --limit 5`
    — semantic equivalents across reference compilers in sub-second time
  - `scripts/intel-query.sh --human callers "<symbol>" --repo ori` — blast radius
    for changes in this domain
  - `scripts/intel-query.sh --human file-symbols "<path-fragment>" --repo ori` — the
    module inventory before editing

  The graph indexes 191K+ symbols and 505K+ CALLS edges across Ori plus 10
  reference compilers, synced on every commit. Manual reference-repo reading
  stays authoritative — but only AFTER the graph narrows the search. Never
  cite a graph result without verifying against the actual source. See
  `.claude/rules/intelligence.md` for the full workflow inventory and
  `.claude/skills/dual-tpr/compose-intel-summary.md` for the canonical
  summary template used by review-family skills.
  ```

- [ ] Note per-file tuning: some files (e.g., `arc.md`, `aims-rules.md`) benefit from subsystem-preset shortcuts (`ori-arc`, `ori-inference`, etc.) in the bullet list. Note which presets match each file's scope before inserting.

- [ ] **Subsection close-out (04.1)**:
  - [ ] Template is drafted and per-file preset tuning is decided
  - [ ] Update `04.1` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 04.1** — was there a helper to surface "which preset matches which rule file's domain"? `.claude/rules/intelligence.md §Subsystem Mapping` already does this at the file-path level; flag if rule-file-level presets need a new helper.
  - [ ] **Run `/sync-claude` on 04.1** — no file writes in this subsection; sync is a no-op. Document: "Claude artifact sync 04.1: template-drafting subsection; no artifacts changed."
  - [ ] **Repo hygiene check**.

---

## 04.2 Insert into 9 domain rule files

**File(s):** 9 files to modify with the template from §04.1

Per-file insertion point (verified line numbers from TPR; `compiler.md` added 2026-04-14 during §02.2 sync-claude retrospective):

1. `.claude/rules/arc.md` (268 lines) — after the cross-backend mirrors discussion near line 243
2. `.claude/rules/aims-rules.md` (918 lines) — after the reference compilers block near line 727
3. `.claude/rules/typeck.md` (1125 lines) — after the rustc/Koka/Gleam/Swift/Zig/Lean citations near line 957
4. `.claude/rules/types.md` (933 lines) — after the prior-art table near line 839
5. `.claude/rules/tests.md` (286 lines) — after the production compiler test-strategy references near line 274
6. `.claude/rules/impl-hygiene.md` (720 lines) — after the aspirational patterns block citing Rust/Zig/Roc near line 313
7. `.claude/rules/canonicalization.md` — after the Maranget / DecisionTree consumer citations near line 37
8. `.claude/rules/patterns.md` — after the registry + dispatch prior-art references near line 27
9. `.claude/rules/compiler.md` (192 lines) — after the `## Source of Truth` section's 10-repo reference list near line 192 (pair with the "Graph reconnaissance — USE FIRST" bullet in CLAUDE.md Compiler Coding Guidelines added in §02.2)

- [ ] For EACH file in the list:
  - [ ] Re-read the target line to confirm the insertion point still fits (files may have drifted; the TPR was 2026-04-14)
  - [ ] Insert the §04.1 template with file-appropriate preset mentioned in the bullet list (e.g., `arc.md` → add `ori-arc` preset; `typeck.md` → `ori-inference`; `canonicalization.md` → `ori-patterns`; etc. per the mapping in `intelligence.md` §Subsystem Mapping)
  - [ ] Diff: net change is one `## Graph-first, manual second` section added; no existing content deleted

- [ ] Verify all 9 land: `grep -L 'scripts/intel-query.sh' .claude/rules/{arc,aims-rules,typeck,types,tests,impl-hygiene,canonicalization,patterns,compiler}.md` returns empty.

- [ ] **Subsection close-out (04.2)**:
  - [ ] All 9 inserts verified
  - [ ] Update `04.2` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 04.2** — did inserting 8 nearly-identical paragraphs feel mechanical enough that a `scripts/inject-graph-first.py <file>` helper could automate future rule-file additions? If matured, commit via `build(tooling): ...`. If the 8-file set is one-shot and unlikely to repeat, document negative finding.
  - [ ] **Run `/sync-claude` on 04.2** — 8 rule files changed. `canon.md` indexes rule files; verify no line numbers referenced in `canon.md` shifted. Commit any drift via `docs(rules): ...`.
  - [ ] **Repo hygiene check**.

---

## 04.3 Refresh intelligence.md workflow inventory

**File(s):** `.claude/rules/intelligence.md`

The current "When to Query" block (lines 18-34) lists 15 workflows. Several review-family skills and commands added since that block was last refreshed are missing (verify-tpr, sync-claude, fix-next-bug, tp-help, sync-spec, sync-grammar, verify-roadmap). After §05 lands, those workflows will query the graph — the `intelligence.md` inventory should document that.

- [ ] Update the "When to Query" bullet list in `.claude/rules/intelligence.md` to add:
  - `/verify-tpr` — callers/callees for blast-radius on each finding before accept/reject decision
  - `/sync-claude` — `file-symbols` on changed crates to confirm rules/canonical docs still match
  - `/fix-next-bug` — blast-radius + similar on the selected bug's repro symbol before handing to /fix-bug
  - `/tp-help` — callers/callees/similar to enrich the context package given to reviewers
  - `/sync-spec` — callers of affected symbols before spec edits
  - `/sync-grammar` — symbol lookup for grammar-adjacent types
  - `/verify-roadmap` — review agents run intel queries before the rule-file read cycle

- [ ] If §04.2 surfaces any rule files that regularly invoke the graph beyond the 8 listed, note them in the §How to Use Results paragraph for discoverability.

- [ ] **Subsection close-out (04.3)**:
  - [ ] Workflow inventory reflects the post-§05 state
  - [ ] Update `04.3` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 04.3** — is there a way to keep the inventory auto-in-sync with actual skill/command references? (E.g., a script that greps for `scripts/intel-query.sh` across `.claude/skills/` and `.claude/commands/` and produces the list.) If matured, commit via `build(tooling): ...`.
  - [ ] **Run `/sync-claude` on 04.3** — `intelligence.md` was the subject of the edit. Check whether CLAUDE.md's line-38 rule needs a refresh to match the new inventory.
  - [ ] **Repo hygiene check**.

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] All 10 rule files (9 domain + intelligence.md refresh) reference the graph
- [ ] `grep -L 'scripts/intel-query.sh' .claude/rules/{arc,aims-rules,typeck,types,tests,impl-hygiene,canonicalization,patterns,compiler,intelligence}.md` returns empty
- [ ] Per-file preset tuning verified (e.g., arc.md cites `ori-arc`, typeck.md cites `ori-inference`, compiler.md cites general-purpose + cross-crate guidance, etc.)
- [ ] No existing content was deleted in any rule file (diff spot-check on all 10)
- [ ] `./test-all.sh` green
- [ ] `python scripts/plan_corpus.py check plans/query-intel-adoption/section-04-rules-graph-first.md` returns 0 errors
- [ ] **Plan sync**:
  - [ ] Section frontmatter `status` → `complete`
  - [ ] `00-overview.md` Quick Reference and mission criterion updated
  - [ ] `index.md` updated
- [ ] `/tpr-review` passed — verify reviewers agree the insertions POINT at the graph rather than duplicate its subcommand reference
- [ ] `/impl-hygiene-review` passed — the 9 paragraphs are near-identical by design but each points at the SSOT (`intelligence.md` + `compose-intel-summary.md`); confirm this is not LEAK:algorithmic-duplication (the paragraphs are user-facing prose tuned per-file; the canonical query pattern itself lives in §03's SSOT)
- [ ] `/improve-tooling` section-close sweep
- [ ] `/sync-claude` section-close doc sync
- [ ] `diagnostics/repo-hygiene.sh --check`

**Exit Criteria:** All 9 target rule files contain a graph-first reference, with per-file subsystem-preset tuning. `intelligence.md` workflow inventory reflects the full set of post-§05 consumers. `./test-all.sh` green.
