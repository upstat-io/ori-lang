---
section: "02"
title: "CLAUDE.md expansion — teach the graph in the always-loaded doc"
status: not-started
reviewed: false
goal: "Insert graph awareness into 5 verified locations in CLAUDE.md so every future session starts with the graph in context"
success_criteria:
  - "CLAUDE.md Commands section (line 140) lists `/query-intel` with a one-line recipe"
  - "CLAUDE.md Key Paths section (line 182) names `scripts/intel-query.sh` and `../lang_intelligence/`"
  - "CLAUDE.md Reference Repos section (line 186) has a graph-first paragraph BEFORE the 10-repo list"
  - "CLAUDE.md line 38 Fact-check rule is strengthened to 'graph-FIRST' language"
  - "Compiler Coding Guidelines has a graph-reconnaissance bullet for cross-crate changes"
  - "Satisfies mission criterion: CLAUDE.md teaches the graph in 5 verified insertion points"
inspired_by:
  - "Existing CLAUDE.md line 38 Fact-check rule — the only current graph mention"
  - "TPR findings codex-002/003/004 [high], gemini-002 [high]"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Commands + Key Paths + Reference Repos edits"
    status: not-started
  - id: "02.2"
    title: "Ownership/Deferral strengthening + Compiler Guidelines bullet"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: CLAUDE.md expansion

**Status:** Not Started
**Goal:** Every session loads CLAUDE.md. Teaching the graph there is the highest-leverage change for passive discovery. Five verified insertion points lift the graph from "one mention on line 38" to "present in every ambient mental model Claude builds at session start."

**Context:** `CLAUDE.md` (root file, 211 lines) is the always-loaded project instruction set. Today it references `/query-intel` exactly once (line 38 in the Ownership & Deferral block), and ONLY as a fact-check suggestion after manual reference-repo reading. The natural lookup surfaces — Commands section (line 140), Key Paths (line 182), Reference Repos (line 186) — have no mention. This section inserts concise guidance at each of those surfaces.

**Reference implementations:**
- **Ori** `CLAUDE.md` itself: existing Commands block (line 140) format — `**Primary**: ...` / `**Tests**: ...` / `**Tracing**: ...` bullet shape dictates how `/query-intel` is introduced
- **TPR finding provenance**: codex-002 [high] (Commands gap), codex-003 [high] (Key Paths gap), codex-004 [high] (Reference Repos gap), gemini-002 [high] (general integration)

**Depends on:** Nothing. Parallel with §01.

---

## 02.1 Commands + Key Paths + Reference Repos edits

**File(s):** `CLAUDE.md` (project root)

Three mechanical inserts into already-established sections. Each is factual and non-duplicative of existing content.

- [ ] **Commands section insert** (after line 167 `diagnostic.md §Diagnostic Scripts...` or near it, before the `## Feature Flags` boundary at line 169):

  ```markdown
  **Intelligence graph**: `/query-intel status` (health) | `/query-intel --human symbols "<name>" --repo ori` | `callers`/`callees`/`file-symbols`/`similar` subcommands. The graph indexes 191K+ symbols and 505K+ CALLS edges across Ori + 10 reference compilers — ~100x faster than grep for blast-radius and cross-repo prior art. Degrades silently when `scripts/intel-query.sh status` is not ok. See `.claude/rules/intelligence.md` for the full workflow inventory, `.claude/skills/query-intel/SKILL.md` for the capability reference.
  ```

- [ ] **Key Paths section edit** (line 184 — the long pipe-separated list): append near the diagnostic-scripts entry:

  ```
  | `scripts/intel-query.sh` — canonical wrapper for the language intelligence graph | `../lang_intelligence/` — Neo4j + Python repo housing the graph (external; graceful degradation when unavailable)
  ```

- [ ] **Reference Repos section edit** (insert a paragraph AFTER the `## Reference Repos (\`~/projects/reference_repos/lang_repos/\`)` header on line 186 and BEFORE the 10-repo bullet list starting line 188):

  ```markdown
  **Graph-first, manual second.** Before manually browsing any repo path below, query the intelligence graph: `scripts/intel-query.sh --human similar "<symbol>" --repo rust,swift,go,koka --limit 5` finds semantic equivalents in seconds, and `callers`/`callees` give call-graph context. The graph is synced on every commit and covers all 10 repos listed here. Manual file reading is still authoritative — but only AFTER the graph has narrowed the search. Never cite a Neo4j result without verifying against the actual source.
  ```

- [ ] Verify the edits land in the right block boundaries: `grep -n 'Intelligence graph' CLAUDE.md` → 1 hit in Commands; `grep -n 'scripts/intel-query.sh' CLAUDE.md` → ≥3 hits (Commands, Key Paths, Reference Repos); `grep -n 'Graph-first' CLAUDE.md` → 1 hit in Reference Repos.

- [ ] **Subsection close-out (02.1)** — MANDATORY before 02.2:
  - [ ] All edits land cleanly; grep counts above match expectations
  - [ ] Update `02.1` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 02.1** — was the `grep -n` cross-check sufficient, or did we need a CLAUDE.md-structure linter? If a single script could validate that the 5 target insertion points are all present, that'd catch future regressions. Implement + commit via `build(diagnostics): ...` if the idea matures. Otherwise document negative finding.
  - [ ] **Run `/sync-claude` on 02.1** — CLAUDE.md is the subject of this edit. Does `.claude/rules/intelligence.md` now need a back-reference to CLAUDE.md's new language? Does `canon.md` cite CLAUDE.md and need a line-number refresh? Commit any drift via `docs(rules): ...`.
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`.

---

## 02.2 Ownership/Deferral strengthening + Compiler Guidelines bullet

**File(s):** `CLAUDE.md` (project root)

- [ ] **Line 38 strengthening**: replace the current "Fact-check" bullet with language that makes graph-first mandatory, not advisory. Current:

  ```
  - **Fact-check** against spec. Use `/query-intel similar "<symbol or concept>"` and `/query-intel callers/callees "<symbol>" --repo ori` before manual reference-repo reading — the graph finds the exact equivalent in seconds. Then verify against `~/projects/reference_repos/lang_repos/` (Rust, Go, Zig, TS, Gleam, Elm, Roc, Swift, Koka, Lean 4).
  ```

  Target:

  ```
  - **Graph-FIRST fact-check** against spec. MANDATORY before manual reference-repo reading: `/query-intel similar "<symbol or concept>"` and `/query-intel callers/callees "<symbol>" --repo ori` find the exact equivalent in seconds. Only AFTER graph results narrow the search should you open `~/projects/reference_repos/lang_repos/` (Rust, Go, Zig, TS, Gleam, Elm, Roc, Swift, Koka, Lean 4) to verify. Skipping the graph step and grepping reference repos by hand is a tooling failure, not a preference.
  ```

- [ ] **Compiler Coding Guidelines bullet** (insert into the bullet list starting line 120 `- **Architecture**: ...`; natural slot is near the "Tracing — USE FIRST" bullet on line 134 or the "Continuous improvement" bullet on line 136):

  ```
  - **Graph reconnaissance — USE FIRST for cross-crate work**: Before grep'ing for a symbol across `compiler/*`, run `scripts/intel-query.sh --human callers "<symbol>" --repo ori` (and `callees`, and `file-symbols "<path-fragment>"`). The intelligence graph indexes 505K+ CALLS edges; it resolves blast radius in sub-second time vs. minutes of ripgrep-and-read. This applies to ANY change touching more than one crate — AIMS pipeline edits, type-checker ↔ ARC handoff changes, registry drift checks. See `.claude/rules/intelligence.md`.
  ```

- [ ] Verify: `grep -n 'Graph-FIRST' CLAUDE.md` → 1 hit; `grep -n 'Graph reconnaissance' CLAUDE.md` → 1 hit.

- [ ] **Subsection close-out (02.2)** — MANDATORY before section close:
  - [ ] All edits land; grep verifications pass
  - [ ] Update `02.2` status to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on 02.2** — same cadence as 02.1. Did strengthening the Fact-check language expose any other places where graph-first could replace manual-first? If yes, note for §04 (rule files). Commit improvements separately.
  - [ ] **Run `/sync-claude` on 02.2** — Compiler Coding Guidelines change: does `.claude/rules/compiler.md` need a cross-reference? Does any rule file stand to benefit? (This is exactly §04's scope — cross-reference §04 in the sync note if so.)
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`.

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] All 5 insertions/edits in CLAUDE.md verified by `grep -n` spot-checks
- [ ] Line counts before and after edit recorded; no accidental deletions
- [ ] `./test-all.sh` green
- [ ] `python scripts/plan_corpus.py check plans/query-intel-adoption/section-02-claude-md-expansion.md` returns 0 errors
- [ ] No plan annotations leaked into `.rs` files (this section does not touch `.rs`)
- [ ] **Plan sync**:
  - [ ] This section's frontmatter `status` → `complete`
  - [ ] `00-overview.md` Quick Reference status updated for 02
  - [ ] `00-overview.md` mission criterion "CLAUDE.md teaches the graph..." checked off
  - [ ] `index.md` section status updated
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed — verify the CLAUDE.md insertions don't duplicate `.claude/rules/intelligence.md` content (they must POINT to it, not copy it)
- [ ] `/improve-tooling` section-close sweep
- [ ] `/sync-claude` section-close doc sync — CLAUDE.md just changed; check that `.claude/rules/*.md` back-references are still valid
- [ ] `diagnostics/repo-hygiene.sh --check`

**Exit Criteria:** `grep -cE 'intel-query|query-intel|intelligence graph|Graph-first|Graph-FIRST|Graph reconnaissance' CLAUDE.md` returns ≥6 distinct hits (up from 1 today). `./test-all.sh` green. No plan-corpus validation failures.
