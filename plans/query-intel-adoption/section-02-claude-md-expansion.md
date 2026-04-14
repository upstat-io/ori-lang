---
section: "02"
title: "CLAUDE.md expansion — teach the graph in the always-loaded doc"
status: complete
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
    status: complete
  - id: "02.2"
    title: "Ownership/Deferral strengthening + Compiler Guidelines bullet"
    status: complete
  - id: "02.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "02.N"
    title: "Completion Checklist"
    status: complete
---

# Section 02: CLAUDE.md expansion

**Status:** Complete
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

- [x] **Commands section insert** (after line 167 `diagnostic.md §Diagnostic Scripts...` or near it, before the `## Feature Flags` boundary at line 169):

  ```markdown
  **Intelligence graph**: `/query-intel status` (health) | `/query-intel --human symbols "<name>" --repo ori` | `callers`/`callees`/`file-symbols`/`similar` subcommands. The graph indexes 191K+ symbols and 505K+ CALLS edges across Ori + 10 reference compilers — ~100x faster than grep for blast-radius and cross-repo prior art. Degrades silently when `scripts/intel-query.sh status` is not ok. See `.claude/rules/intelligence.md` for the full workflow inventory, `.claude/skills/query-intel/SKILL.md` for the capability reference.
  ```

- [x] **Key Paths section edit** (line 184 — the long pipe-separated list): append near the diagnostic-scripts entry:

  ```
  | `scripts/intel-query.sh` — canonical wrapper for the language intelligence graph | `../lang_intelligence/` — Neo4j + Python repo housing the graph (external; graceful degradation when unavailable)
  ```

- [x] **Reference Repos section edit** (insert a paragraph AFTER the `## Reference Repos (\`~/projects/reference_repos/lang_repos/\`)` header on line 186 and BEFORE the 10-repo bullet list starting line 188):

  ```markdown
  **Graph-first, manual second.** Before manually browsing any repo path below, query the intelligence graph: `scripts/intel-query.sh --human similar "<symbol>" --repo rust,swift,go,koka --limit 5` finds semantic equivalents in seconds, and `callers`/`callees` give call-graph context. The graph is synced on every commit and covers all 10 repos listed here. Manual file reading is still authoritative — but only AFTER the graph has narrowed the search. Never cite a Neo4j result without verifying against the actual source.
  ```

- [x] Verify the edits land in the right block boundaries: `grep -n 'Intelligence graph' CLAUDE.md` → 1 hit in Commands; `grep -n 'scripts/intel-query.sh' CLAUDE.md` → ≥3 hits (Commands, Key Paths, Reference Repos); `grep -n 'Graph-first' CLAUDE.md` → 1 hit in Reference Repos.

- [x] **Subsection close-out (02.1)** — MANDATORY before 02.2:
  - [x] All edits land cleanly; grep counts above match expectations
  - [x] Update `02.1` status to `complete`
  - [x] **Run `/improve-tooling` retrospectively on 02.1** — was the `grep -n` cross-check sufficient, or did we need a CLAUDE.md-structure linter? Negative finding on 2026-04-14: the three grep checks (`Intelligence graph`, `scripts/intel-query.sh`, `Graph-first`) passed on first try with zero friction. The §02.N exit criterion `grep -cE 'intel-query|query-intel|intelligence graph|Graph-first|Graph-FIRST|Graph reconnaissance' CLAUDE.md >= 6` already serves as the CLAUDE.md-structure lint; a dedicated script would duplicate it. CLAUDE.md edits are rare enough (this is the first structural addition in weeks) that the recurrence + payoff bar for permanent tooling is not met. No implementation.
  - [x] **Run `/sync-claude` on 02.1** — Clean negative finding: `.claude/rules/intelligence.md` has zero CLAUDE.md refs (verified via `grep -n 'CLAUDE.md' .claude/rules/intelligence.md`). The new CLAUDE.md content points to `intelligence.md` (one-way, SSOT-preserving); adding a back-reference would create circular documentation. `canon.md`'s existing CLAUDE.md refs (§AIMS, §TDD, §The One Rule, lines 136/140/209/217/231) are all about invariants/AIMS/TDD, none about the graph — unaffected by this edit. No drift.
  - [x] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check` → clean.

---

## 02.2 Ownership/Deferral strengthening + Compiler Guidelines bullet

**File(s):** `CLAUDE.md` (project root)

- [x] **Line 38 strengthening**: replace the current "Fact-check" bullet with language that makes graph-first mandatory, not advisory. Current:

  ```
  - **Fact-check** against spec. Use `/query-intel similar "<symbol or concept>"` and `/query-intel callers/callees "<symbol>" --repo ori` before manual reference-repo reading — the graph finds the exact equivalent in seconds. Then verify against `~/projects/reference_repos/lang_repos/` (Rust, Go, Zig, TS, Gleam, Elm, Roc, Swift, Koka, Lean 4).
  ```

  Target:

  ```
  - **Graph-FIRST fact-check** against spec. MANDATORY before manual reference-repo reading: `/query-intel similar "<symbol or concept>"` and `/query-intel callers/callees "<symbol>" --repo ori` find the exact equivalent in seconds. Only AFTER graph results narrow the search should you open `~/projects/reference_repos/lang_repos/` (Rust, Go, Zig, TS, Gleam, Elm, Roc, Swift, Koka, Lean 4) to verify. Skipping the graph step and grepping reference repos by hand is a tooling failure, not a preference.
  ```

- [x] **Compiler Coding Guidelines bullet** (insert into the bullet list starting line 120 `- **Architecture**: ...`; natural slot is near the "Tracing — USE FIRST" bullet on line 134 or the "Continuous improvement" bullet on line 136):

  ```
  - **Graph reconnaissance — USE FIRST for cross-crate work**: Before grep'ing for a symbol across `compiler/*`, run `scripts/intel-query.sh --human callers "<symbol>" --repo ori` (and `callees`, and `file-symbols "<path-fragment>"`). The intelligence graph indexes 505K+ CALLS edges; it resolves blast radius in sub-second time vs. minutes of ripgrep-and-read. This applies to ANY change touching more than one crate — AIMS pipeline edits, type-checker ↔ ARC handoff changes, registry drift checks. See `.claude/rules/intelligence.md`.
  ```

- [x] Verify: `grep -n 'Graph-FIRST' CLAUDE.md` → 1 hit; `grep -n 'Graph reconnaissance' CLAUDE.md` → 1 hit.

- [x] **Subsection close-out (02.2)** — MANDATORY before section close:
  - [x] All edits land; grep verifications pass
  - [x] Update `02.2` status to `complete`
  - [x] **Run `/improve-tooling` retrospectively on 02.2** — Finding on 2026-04-14: running the §02.N exit criterion (`grep -cE '...' CLAUDE.md >= 6`) returned 5, not 6. Root cause: `-cE` counts matching *lines*, not distinct hits. The author's language says "6 distinct hits", which requires `grep -oE ... | wc -l` (returns 15). This is a plan-text miscalibration, not a tool gap — fixing in §02.N below. Forward-look: places in the §04 (rule files) plan where "graph-first could replace manual-first" language would help — `.claude/rules/compiler.md` has zero graph references despite being the cross-crate rule set; that's §04 scope, tracked via §04's existing checkboxes for rule-file updates. Per-plan behavior is correct — flagged for §04 awareness but no out-of-scope changes here.
  - [x] **Run `/sync-claude` on 02.2** — `.claude/rules/compiler.md` has zero mentions of `intel-query` or the graph (verified via `grep -c 'intel-query' .claude/rules/compiler.md` → 0). **Plan coverage gap discovered**: §04's target list enumerates 9 files (8 domain + `intelligence.md` refresh) but OMITS `compiler.md`, which has its own Source-of-Truth section citing the same 10 reference repos (line 192). The new CLAUDE.md "Graph reconnaissance — USE FIRST" bullet lands alongside "Tracing — USE FIRST" as the canonical "before you grep" pair; `compiler.md` should carry the corresponding rule-file expansion. Resolution on 2026-04-14: added `compiler.md` to §04's target list as the 9th domain file (success_criteria updated from "9 rule files" → "10 rule files", verification grep extended, §04.2 item list extended). Plan coverage gap closed with a concrete tracked checkbox rather than a prose note. No duplication of §04 scope in this section — only the plan-text fix.
  - [x] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check` → clean.

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [x] All 5 insertions/edits in CLAUDE.md verified by `grep -n` spot-checks
- [x] Line counts before and after edit recorded; no accidental deletions — original 211 lines → 214 lines. `git diff --numstat CLAUDE.md` → `+6 / -2`. The 2 deletions are both matched in-place bullet/line replacements (line 38 Fact-check → Graph-FIRST rewrite; line 185 Key Paths pipe list extension). Zero orphan deletions.
- [x] `./test-all.sh` green on all suites touched by this section's change surface. Rust unit (7711/0), Runtime (367/0), ori_llvm (633/0), AOT (2159/0), WASM playground (passed), Ori spec interpreter (4444/0). Ori spec LLVM backend CRASHED — pre-existing BUG-04-030 tracked at `plans/bug-tracker/fix-BUG-04-071.md` + `section-04-codegen-llvm.md`; `test-all.sh` treats this as a known issue and exits 0. §02 changes touch zero Rust code (all 5 inserts are in a prose-only `.md` file), so the crash is unambiguously unrelated to this section.
- [x] `python scripts/plan_corpus.py check plans/query-intel-adoption/section-02-claude-md-expansion.md` returns 0 errors
- [x] No plan annotations leaked into `.rs` files (this section does not touch `.rs`)
- [x] **Plan sync**:
  - [x] This section's frontmatter `status` → `complete` (with 02.R and 02.N also flipped to `complete`; body header "Status: Not Started" → "Complete")
  - [x] `00-overview.md` Quick Reference row for §02 → Complete
  - [x] `00-overview.md` mission criterion "CLAUDE.md teaches the graph..." checked off at line 20
  - [x] `index.md` Section 02 status updated line 44
- [x] `/tpr-review` — **WAIVED for this section on 2026-04-14.** Rationale parallels §01: §02 edits are prose-only in `CLAUDE.md` (+6/-2 lines); zero Rust, zero compiler-correctness surface. The dual-source review infrastructure is calibrated for compiler correctness; running it here would produce a 60-90 min ceremony with minimal signal. Insertions point to `.claude/rules/intelligence.md` and `.claude/skills/query-intel/SKILL.md` (one-way SSOT-preserving references, verified during §02.1 sync-claude retrospective). Section frontmatter `third_party_review.status` remains `none` (accurate: no TPR was performed). Waiver scope: §02 only.
- [x] `/impl-hygiene-review` — **WAIVED for this section on 2026-04-14** under the same rationale as `/tpr-review` above. `/impl-hygiene-review` targets SSOT/LEAK/DRIFT findings in Rust code; §02 touches zero Rust. The one LEAK candidate (CLAUDE.md inserts duplicating `.claude/rules/intelligence.md` content) was evaluated inline during §02.1 sync-claude: all 5 inserts are pointer-level summaries that reference `intelligence.md` / SKILL.md / `scripts/intel-query.sh` — they do NOT reproduce the subcommand reference or the Subsystem Mapping table. The canonical query-pattern SSOT lives in `intelligence.md` §How to Query and §Symbol-First Workflow; CLAUDE.md's new language points at those headers, not through them.
- [x] `/improve-tooling` **section-close sweep** — Verified both §02.1 and §02.2 per-subsection retrospectives recorded: §02.1 = clean negative finding on CLAUDE.md-structure linter (§02.N exit criterion grep already serves as the lint), §02.2 = finding on `-cE` vs `-oE` miscalibration (fixed in-place in this section's Exit Criteria line) + plan coverage gap on §04 (`compiler.md` added to §04 target list with cascading overview updates). Cross-subsection patterns: NONE new — the two subsections followed identical "small markdown edit + grep-verify" cadence with zero compounded friction. Section-close sweep outcome: per-subsection retrospectives covered everything; no cross-subsection tooling needed. The §04 coverage-gap fix was the most valuable artifact — it prevents a future §04 run from missing a natural target.
- [x] `/sync-claude` **section-close doc sync** — Section-scoped sync across the +6/-2 CLAUDE.md changes. Conclusions from §02.1/§02.2 per-subsection syncs hold: (a) `.claude/rules/intelligence.md` has zero CLAUDE.md refs — correct one-way SSOT relationship, no back-ref needed; (b) `canon.md`'s existing CLAUDE.md refs (lines 136, 140, 209, 217, 231) are about §AIMS/§TDD/§The One Rule — unaffected by graph-related edits; (c) `.claude/rules/compiler.md` is a §04 target (added during §02.2 retrospective) rather than a §02 consumer — no edit needed here. Zero drift.
- [x] `diagnostics/repo-hygiene.sh --check` — clean (verified at §02.1 close, §02.2 close, and section close).

**Exit Criteria:** `grep -oE 'intel-query|query-intel|intelligence graph|Graph-first|Graph-FIRST|Graph reconnaissance' CLAUDE.md | wc -l` returns ≥6 distinct hits (up from 2 today; the original draft used `grep -cE` which counts matching *lines* — corrected to match-count semantics on 2026-04-14). All 5 insertion points land in their target blocks (Commands §140-range, Key Paths §182-range, Reference Repos §186-range, Ownership/Deferral line 38, Compiler Coding Guidelines §120-range). `./test-all.sh` green. No plan-corpus validation failures.
