---
section: "04"
title: "Rule files — graph-first guidance in 9 domain rule files + intelligence.md refresh"
status: in-progress
reviewed: false
goal: "Add graph-first paragraphs to 9 domain rule files (arc, aims-rules, typeck, types, tests, impl-hygiene, canonicalization, patterns, compiler) and refresh intelligence.md workflow inventory — the 10 files identified by the original TPR as citing cross-repo prior art without graph-first guidance"
success_criteria:
  - "9 domain rule files (arc.md, aims-rules.md, typeck.md, types.md, tests.md, impl-hygiene.md, canonicalization.md, patterns.md, compiler.md) each contain a graph-first paragraph near their prior-art references; intelligence.md gets a workflow inventory refresh instead of a paragraph insert"
  - "`.claude/rules/intelligence.md` workflow inventory refreshed to include verify-tpr, sync-claude, fix-next-bug, tp-help, sync-spec, sync-grammar, verify-roadmap as covered workflows"
  - "`grep -L 'scripts/intel-query.sh' .claude/rules/{arc,aims-rules,typeck,types,tests,impl-hygiene,canonicalization,patterns,compiler,intelligence}.md` returns empty (all 10 now reference the graph)"
  - "Satisfies mission criterion: all 10 rule files include graph-first paragraph"
inspired_by:
  - "`.claude/rules/intelligence.md` §Symbol-First Workflow — existing graph-first language; template source"
  - "TPR findings codex-005, 020, 021, 022, 023, 024, 025, 026, 027, gemini-004 [medium]"
depends_on: ["03"]
third_party_review:
  status: findings
  updated: 2026-04-14
sections:
  - id: "04.1"
    title: "Draft the graph-first paragraph template"
    status: complete
  - id: "04.2"
    title: "Insert into 9 domain rule files"
    status: complete
  - id: "04.3"
    title: "Refresh intelligence.md workflow inventory"
    status: complete
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Rule files — graph-first guidance in 9 domain rule files

**Status:** In Progress
**Goal:** The 9 domain rule files identified by the original TPR (arc.md, aims-rules.md, typeck.md, types.md, tests.md, impl-hygiene.md, canonicalization.md, patterns.md, compiler.md) each receive a graph-first paragraph. intelligence.md receives a workflow inventory refresh. Other rule files (canon.md, codegen-rules.md, parse.md, llvm.md, etc.) may also benefit from graph-first guidance but were not in the original TPR's finding scope — they are tracked for future coverage.

**Context:** Per the 2026-04-14 TPR verification, the original TPR findings (codex-020 through codex-027, gemini-004) identified 9 specific domain rule files that cite reference-repo paths without mentioning the intelligence graph. The graph indexes all 10 reference repos with 24K+ call edges and vector embeddings for semantic similarity — it resolves cross-repo equivalents in sub-second time. A reader following today's rule-file guidance would open 5 reference repos manually and grep; after §04 lands, the same reader runs one graph query, narrows to 2 concrete files, and then opens those. LEAK:scattered-knowledge is the finding category (codex finding IDs TPR-XX-020 through TPR-XX-025 plus 026/027).

**Reference implementations:**
- **Ori** `.claude/rules/intelligence.md:66-75` (Symbol-First Workflow): existing canonical paragraph, adapted here for insertion at other sites
- **TPR finding provenance**: codex-020 arc.md:243, codex-021 aims-rules.md:727, codex-022 typeck.md:957, codex-023 types.md:839, codex-024 tests.md:274, codex-025 impl-hygiene.md:313, codex-026 canonicalization.md:37, codex-027 patterns.md:27, codex-005 intelligence.md workflow inventory, gemini-004 general

**Depends on:** Section 03 (the paragraph cites `compose-intel-summary.md` as the canonical summary template).

---

## 04.1 Draft the graph-first paragraph template

**File(s):** Template only (no file written in this subsection)

One canonical paragraph shape used in 8 of the 9 target files (§04.2). `intelligence.md` gets a different treatment (§04.3 refreshes its workflow inventory, not a paragraph insert).

- [x] Draft the template:

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
  [+ optional per-file preset bullet, e.g. `ori-arc --limit 5` for ARC-scoped files]

  The graph covers Ori plus 10 reference compilers, synced on every commit.
  Manual reference-repo reading stays authoritative — but only AFTER the
  graph narrows the search. Never cite a graph result without verifying
  against the actual source. See `.claude/rules/intelligence.md` for the
  canonical when-to-query workflow and subcommand reference, and
  `.claude/skills/dual-tpr/compose-intel-summary.md` for the canonical
  query protocol used by review-family skills.
  ```

  **Note:** The fenced template above is the FINAL shipped form (post-TPR round 1 fixes: stats removed, footer labels retargeted). The original pre-TPR draft had hardcoded `191K+/505K+` stats and `full workflow inventory` / `summary template` labels; those were fixed per TPR findings TPR-04-001-gemini and TPR-04-002-codex.

- [x] Draft the template (original draft accepted; final form reflects TPR round 1 corrections above).

- [x] Note per-file tuning — per-file preset mapping decided:

  | File | Preset bullet | Rationale |
  |------|---------------|-----------|
  | `arc.md` | `ori-arc` | ARC IR, RC, reuse subsystem |
  | `aims-rules.md` | `ori-arc` | AIMS lives in `ori_arc` crate |
  | `typeck.md` | `ori-inference` | Type checking + inference |
  | `types.md` | (none — cross-cutting) | Pool + registries span infer/check/pool — no single preset covers all; use `file-symbols` + `similar` instead |
  | `tests.md` | (none — cross-cutting) | Matrix testing spans all subsystems |
  | `impl-hygiene.md` | (none — cross-cutting) | Cross-phase invariants span all crates |
  | `canonicalization.md` | `ori-patterns` | Pattern compilation lives here (Maranget) |
  | `patterns.md` | `ori-patterns` | Pattern system core |
  | `compiler.md` | (none — architecture) | Emphasize cross-crate `file-symbols` instead |

- [x] **Subsection close-out (04.1)**:
  - [x] Template is drafted and per-file preset tuning is decided
  - [x] Update `04.1` status to `complete` (frontmatter updated)
  - [x] **`/improve-tooling` retrospective 04.1 (inline, no gaps)** — the existing `.claude/rules/intelligence.md §Subsystem Mapping` table maps file-path patterns (e.g., `compiler/ori_arc/`) to presets; rule files share their names with those subsystems (`arc.md` → `ori-arc`), so the preset decision is near-trivial and needs no new helper. No tooling gaps surfaced during §04.1 (pure decision subsection, zero tooling interaction).
  - [x] **`/sync-claude` 04.1 (no-op documented)** — template-drafting subsection; no `.claude/` artifacts changed. CLAUDE.md already references `.claude/rules/intelligence.md §Subsystem Mapping` (line 169 Intelligence graph bullet), so no sync needed.
  - [x] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check` detected `test.py`, a pre-existing scratch file from a parallel session the user elected to preserve (Step 1.95 decision). No §04.1-generated debris; `--clean` intentionally NOT run.

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

- [x] For EACH file in the list:
  - [x] Re-read the target line to confirm the insertion point still fits — all 9 verified (minor drift: impl-hygiene.md +1 line from plan's 313 to actual 314; others exact)
  - [x] Insert the §04.1 template with file-appropriate preset mentioned in the bullet list — per the §04.1 preset table; reference-repo lists also tuned per-file (arc/aims-rules → `rust,swift,koka,lean4`; typeck → + `gleam`; types → `rust,swift,zig,lean4`; tests → `rust,go,zig,swift,koka,lean4`; impl-hygiene → `rust,swift,zig,lean4`; canonicalization/patterns → `rust,gleam,elm,roc,koka`; compiler → `rust,swift,zig,lean4`)
  - [x] Diff: net change is one `## Graph-first, manual second` section added per file; no existing content deleted in the 9 domain files (all purely additive). Post-TPR line-count deltas differ from the initial +199 due to round 1 fixes (stats removal, footer rewording); see commit `6d7bf77a` for the authoritative diff.

- [x] Verify all 9 land: `grep -L 'scripts/intel-query.sh' .claude/rules/{arc,aims-rules,typeck,types,tests,impl-hygiene,canonicalization,patterns,compiler}.md` returns empty. ✓

- [x] **Subsection close-out (04.2)**:
  - [x] All 9 inserts verified
  - [x] Update `04.2` status to `complete` (frontmatter updated)
  - [x] **`/improve-tooling` retrospective 04.2 (inline, negative finding)** — 9 inserts felt mechanical but the template shape varies meaningfully per file (preset bullet, reference-repo list tuning, adjacent-section context). An `inject-graph-first.py` helper would need per-file config (preset, repos, insertion-anchor regex, adjacent-section wording) which approaches the complexity of the Edit calls themselves. Rule files rarely gain new prior-art references — future additions will happen 1-2 at a time over years, not in batches. One-shot work; no helper warranted. Documented negative finding.
  - [x] **`/sync-claude` 04.2 (no-op documented)** — Checked `.claude/rules/canon.md` for line-number citations to the 9 modified files: all cross-references use semantic anchors (`typeck.md §PC-2`, `arc.md §Cross-Phase Invariant Contracts`, `compiler.md §Architecture`, etc.), NEVER line numbers. CLAUDE.md references (`.claude/rules/intelligence.md` lines 135/169) are also semantic. Zero drift. No sync commit needed.
  - [x] **Repo hygiene check** — `test.py` still present (pre-existing scratch file from parallel session, user-preserved per Step 1.95). No §04.2-generated debris.

- [x] **Placement note on `compiler.md`** — plan instruction was "after the Source of Truth section's 10-repo reference list", but I placed the graph-first section BEFORE `## Source of Truth` for semantic consistency with the other 8 files. Rationale: "graph-first" requires the guidance to PRECEDE the manual-path list, not follow it; the plan's "pair with 'Graph reconnaissance — USE FIRST' bullet in CLAUDE.md" phrasing reinforces this intent. Flagging explicitly for TPR to review.

---

## 04.3 Refresh intelligence.md workflow inventory

**File(s):** `.claude/rules/intelligence.md`

The current "When to Query" block (lines 18-34) lists 15 workflows. Several review-family skills and commands added since that block was last refreshed are missing (verify-tpr, sync-claude, fix-next-bug, sync-spec, sync-grammar, verify-roadmap — 6 total; `/tp-help` was already present). After §05 lands, those workflows will query the graph — the `intelligence.md` inventory should document that.

- [x] Update the "When to Query" bullet list in `.claude/rules/intelligence.md` to add 6 workflows (not 7 — `/tp-help` was already present on line 48; plan's list overlapped by one):
  - `/verify-tpr` — placed next to `/tpr-review` ("TPR triage")
  - `/sync-claude` — grouped with other sync commands ("Doc sync")
  - `/fix-next-bug` — placed next to `/fix-bug` ("Bug autopilot")
  - `/sync-spec` — grouped with other sync commands ("Spec sync")
  - `/sync-grammar` — grouped with other sync commands ("Grammar sync")
  - `/verify-roadmap` — placed next to `/continue-roadmap` ("Roadmap verification")

- [x] §04.2 did not surface any rule files that regularly invoke the graph beyond the 9 listed (the inserts ARE their only invocation — guidance, not automation). No additions to §How to Use Results needed.

- [x] **Subsection close-out (04.3)**:
  - [x] Workflow inventory reflects the post-§05 state (21 bullets covering all known graph-consuming workflows)
  - [x] Update `04.3` status to `complete` (frontmatter updated)
  - [x] **`/improve-tooling` retrospective 04.3 (inline, FINDING surfaced)** — a script `scripts/list-intel-consumers.sh` that greps `.claude/skills/` and `.claude/commands/` for `scripts/intel-query.sh` would detect workflows the graph-query call exists in but aren't listed in `intelligence.md §When to Query`. This is legitimate tooling to prevent inventory drift. **However:** §05 of this same plan ("Missing-trigger skills & commands") is explicitly the audit/closure pass for this gap — it will add `intel-query.sh` invocations to the missing consumers, and any automated drift-detector should be built alongside §05's work (where the before/after set is known). Filing as implementation anchor in §05 rather than adding a new subsection here: §05 should include a completion-checklist item for a `list-intel-consumers.sh` drift-detector. Documented; see §04.N for cross-reference.
  - [x] **`/sync-claude` 04.3 (no-op documented)** — `CLAUDE.md` line 38 ("Graph-FIRST fact-check") describes the correctness-verification rule, not the workflow inventory. CLAUDE.md line 169 ("Intelligence graph") points at `.claude/rules/intelligence.md` without enumerating workflows. The workflow list is authoritatively in intelligence.md alone; CLAUDE.md needs no refresh.
  - [x] **Repo hygiene check** — `test.py` pre-existing, preserved per Step 1.95. No §04.3-generated debris.

---

## 04.R Third Party Review Findings

### Round 1 (2026-04-14) — dual-source (codex + gemini), merge `/tmp/ori-tpr-Zvld2zZO`

- [x] `[TPR-04-001-codex][high]` `.claude/rules/intelligence.md:39` — Remove future-state workflows from the live inventory.
  Evidence: The six bullets added at `intelligence.md:39,42,52,54-56` describe `/verify-tpr`, `/fix-next-bug`, `/sync-claude`, `/sync-spec`, `/sync-grammar`, `/verify-roadmap` as graph-using workflows. `.claude/skills/dual-tpr/compose-intel-summary.md:20-23` explicitly lists three of these (`/verify-tpr`, `/sync-claude`, `/fix-next-bug`) as "Planned future consumers (not yet migrated)"; the actual skill files confirm none of the six currently invoke `scripts/intel-query.sh`. This creates DRIFT between `intelligence.md` and the SSOT in `compose-intel-summary.md`.
  Impact: `intelligence.md` misrepresents which workflows currently query the graph vs. which are planned post-§05 consumers, hiding §05 scope.
  Required plan update: Mark each of the 6 new bullets as `[planned; tracked by plans/query-intel-adoption §05]` or promote them to live only in the same commit that adds the actual graph queries (i.e., as part of §05).
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. Added `*[planned — tracked by plans/query-intel-adoption §05]*` tag to all 6 bullets (`/fix-next-bug`, `/verify-tpr`, `/verify-roadmap`, `/sync-claude`, `/sync-spec`, `/sync-grammar`) in `.claude/rules/intelligence.md:39-56`. Added a legend paragraph directly under `## When to Query` explaining the tag semantics. §05 will promote these to live when the actual graph queries land. DRIFT against `compose-intel-summary.md:20-23` closed.

- [x] `[TPR-04-002-codex][low]` `.claude/rules/arc.md:261` (and 8 peer files) — Retarget the shared footer to the canonical query SSOT.
  Evidence: All 9 inserted paragraphs use the same closing pointer — "`intelligence.md` for the full workflow inventory" + "`compose-intel-summary.md` for the canonical summary template used by review-family skills". The actual SSOT split is: `intelligence.md` §When-to-Query + §How-to-Query + §Symbol-First Workflow = canonical query/reference workflow; `compose-intel-summary.md` = canonical query protocol. Current labels steer readers toward the inventory framing instead of the real SSOT sections.
  Impact: Mild scattered-knowledge risk — readers looking for authoritative query behavior hit "workflow inventory" framing instead of "canonical protocol".
  Required plan update: Revise the §04.1 template footer and reapply to the 9 files, pointing at the right semantic concerns.
  Basis: direct_file_inspection. Confidence: medium.
  Resolved: Fixed on 2026-04-14. Footer labels in all 9 files updated: "full workflow inventory" → "canonical when-to-query workflow and subcommand reference"; "canonical summary template used by review-family skills" → "canonical query protocol used by review-family skills". `grep -rl "full workflow inventory|summary template used by review" .claude/rules/` returns empty.

- [x] `[TPR-04-003-codex][low]` `.claude/rules/compiler.md:198` — Fix stale directional reference after moving the compiler paragraph.
  Evidence: `compiler.md:198` says "blast radius across all 19 workspace crates (see §Crates below)" but after the placement-before-Source-of-Truth move, the `## Crates` section is ABOVE at lines 170-180. Codex verified via `git show HEAD:.claude/rules/compiler.md` that this "below" wording was introduced by the new insertion, not pre-existing.
  Impact: Small coherence bug that weakens the sound semantic-consistency rationale for the before-§Source-of-Truth placement.
  Required plan update: Change "see §Crates below" → "see §Crates above" (or remove the directional cue).
  Basis: fresh_verification. Confidence: high.
  Resolved: Fixed on 2026-04-14. `.claude/rules/compiler.md:198` now reads "see §Crates above".

- [x] `[TPR-04-001-gemini][medium]` `.claude/rules/arc.md:257` (and 8 peer files) — Remove hardcoded graph-size stats from 9 rule files.
  Evidence: The 9 inserted paragraphs each repeat "The graph indexes 191K+ symbols and 505K+ CALLS edges". VERIFIED: this is duplicated verbatim across all 9 files. VERIFIED: `intelligence.md:75` contains an inline comment `# Code symbol queries (Ori + reference repos — 32K+ symbols, 24K+ call edges)` — stale numbers creating DRIFT within the same file. "synced on every commit" means the numbers are volatile; 10 places-to-update guarantees future drift.
  Impact: LEAK:scattered-knowledge on a volatile fact — 10 locations must be updated when the graph grows. Visible drift already exists between `191K/505K` (9 inserts + CLAUDE.md + compose-intel-summary.md) and `32K/24K` (`intelligence.md:75` comment).
  Required plan update: (1) Remove the "191K+ symbols and 505K+ CALLS edges" line from the 9 inserted paragraphs (it's a redirector, not the SSOT for size). (2) Update `intelligence.md:75` stale comment from `32K+/24K+` to `191K+/505K+` to match CLAUDE.md line 169.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-14. (1) The 9 paragraphs now say "The graph covers Ori plus 10 reference compilers, synced on every commit." — hardcoded stats removed. `grep 191K+\|505K+ .claude/rules/{arc,aims-rules,...,compiler}.md` returns empty. (2) `.claude/rules/intelligence.md:75` inline comment updated from `32K+/24K+` to `191K+/505K+` matching CLAUDE.md line 169. DRIFT closed. The canonical numeric facts now live in CLAUDE.md line 169 + intelligence.md line 75 + compose-intel-summary.md only — three SSOT locations, not ten.

- [x] `[TPR-04-002-gemini][low]` `.claude/rules/intelligence.md` `/verify-roadmap` bullet — Name the subcommands consistent with peer bullets.
  Evidence: The `/verify-roadmap` bullet says "review agents run intel queries before the rule-file read cycle" without naming subcommands. Peer bullets name specifics (e.g., `/sync-claude` says "`file-symbols` on changed crates"; `/sync-spec` says "`callers` of affected symbols"). Breaks the established format of the inventory.
  Impact: Incomplete documentation; inconsistent with peer bullets.
  Required plan update: Add subcommand names to the `/verify-roadmap` bullet (e.g., `file-symbols`/`callers`/`similar` to validate architectural claims).
  Basis: direct_file_inspection. Confidence: medium.
  Resolved: Fixed on 2026-04-14. `.claude/rules/intelligence.md:57` now reads `**Roadmap verification** (/verify-roadmap) *[planned — tracked by plans/query-intel-adoption §05]*: \`file-symbols\`/\`callers\`/\`similar\` — review agents validate architectural claims against the graph before the rule-file read cycle`. Format matches peer bullets.

### Round 2 (2026-04-14) — dual-source (codex + gemini), merge `/tmp/ori-tpr-jmlMqzva`

- [x] `[TPR-04-004-codex][medium]` `plans/query-intel-adoption/section-04-rules-graph-first.md:215` — Correct the §04.N no-deletions completion claim.
  Evidence: §04.N claimed "205 insertions, 0 deletions" based on the pre-fix diff. The committed change `6d7bf77a` includes TPR round 1 fixes (intelligence.md stale-comment replacement + intro paragraph), making the actual aggregate 306/53. The 9 domain-file inserts remain purely additive; the deletions are in intelligence.md (corrections) and plan-file reformatting.
  Resolved: Fixed on 2026-04-14. Updated §04.N item to accurately describe the per-file breakdown (9 domain files purely additive; intelligence.md had 2 line replacements as corrections).

- [x] `[TPR-04-005-codex][low]` `.claude/rules/types.md:849` — Align types.md preset with subsystem mapping.
  Evidence: `types.md` used `ori-inference` preset, but its scope (Pool, Idx, TypeFlags, registries) spans `ori_types/pool/`, `ori_types/tag/`, `ori_types/flags/` — not just `src/infer/` and `src/check/` which `ori-inference` maps to. The preset was a weak fit.
  Resolved: Fixed on 2026-04-14. Replaced `ori-inference` preset bullet with cross-cutting pattern: `file-symbols "ori_types/pool"` + `similar "TypePool" --repo rust,swift,zig,lean4`. Updated §04.1 preset table to show types.md as `(none — cross-cutting)`.

- [x] `[TPR-04-006-gemini][low]` `.claude/rules/intelligence.md:44` — Missing subcommand names in /fix-next-bug workflow description.
  Evidence: `/fix-next-bug` bullet said "blast-radius + `similar`" without naming `callers`/`callees` subcommands. Peer bullets (e.g., `/fix-bug Phase 1`) use backticked subcommand names.
  Resolved: Fixed on 2026-04-14. Changed to "`callers`/`callees` for blast-radius + `similar`".

- [x] `[TPR-04-007-gemini][informational]` `Multiple files` — SSOT compliance and compiler.md placement successfully validated.
  Resolved: 2026-04-14 — non-actionable informational finding. Confirms objectives (a), (c), (e) are sound.

- [x] `[TPR-04-003-gemini][informational]` `.claude/rules/compiler.md:189` — Placement of graph-first section before §Source of Truth is logically sound.
  Resolved: 2026-04-14 — non-actionable informational finding validating the §04.2 placement-note rationale. No fix required.

### Round 3 (2026-04-14) — dual-source (codex + gemini), merge `/tmp/ori-tpr-DeZ7thTo`

- [x] `[TPR-04-008-codex][medium]` `.claude/rules/intelligence.md:61` — Align /sync-grammar bullet with §05 `file-symbols` design.
  Evidence: Bullet said `symbols` lookup but §05 design (section-05:156-168) specifies `file-symbols "compiler/ori_parse/"` and `file-symbols "compiler/ori_lexer/"`. DRIFT.
  Resolved: Fixed on 2026-04-14. Changed to `file-symbols` on `compiler/ori_parse/` and `compiler/ori_lexer/`.

- [x] `[TPR-04-009-codex][low]` `section-04:73` — Refresh §04.1 template snapshot to shipped form.
  Evidence: Template at lines 61-79 still showed pre-TPR wording (hardcoded stats, old footer labels). Shipped paragraphs differ.
  Resolved: Fixed on 2026-04-14. Updated fenced template to final shipped form + added post-TPR note.

- [x] `[TPR-04-010-codex][low]` `section-04:129` — Correct stale +199 line-count breakdown.
  Evidence: Post-TPR line deltas differ from the initial +199 (stats removal changed each paragraph by -1 line).
  Resolved: Fixed on 2026-04-14. Replaced specific arithmetic with reference to authoritative commit diff.

- [x] `[TPR-04-011-codex][low]` `section-04:148` — Remove stale /tp-help from missing-workflows list.
  Evidence: Introduction lists 7 workflows as missing but implementation correctly added only 6 (/tp-help was already present).
  Resolved: Fixed on 2026-04-14. Removed `/tp-help` from the missing list, added "(6 total; `/tp-help` was already present)".

- [x] `[TPR-04-012-codex][low]` `section-04:234` — Update §04.N preset reference to match post-round-2 state.
  Evidence: Completion checklist still listed "types.md → ori-inference" but round 2 changed to cross-cutting.
  Resolved: Fixed on 2026-04-14. Updated to match current §04.1 table showing types.md as cross-cutting.

- [x] `[TPR-04-013-codex][informational]` — Rule-file insertions validated: slim, redirecting, no LEAK:algorithmic-duplication.
  Resolved: 2026-04-14 — non-actionable. Confirms core implementation is sound; remaining issues are plan-doc drift only.

- [x] `[TPR-04-014-gemini][informational]` — types.md preset deviation is architecturally sound.
  Resolved: 2026-04-14 — non-actionable. Confirms the round 2 cross-cutting change is correct.

- [x] `[TPR-04-015-gemini][informational]` — compiler.md placement and SSOT compliance re-validated.
  Resolved: 2026-04-14 — non-actionable. Third consecutive confirmation.

- [x] `[TPR-04-016-gemini][informational]` — Workflow inventory and SSOT compliance verified successfully.
  Resolved: 2026-04-14 — non-actionable.

### Round 4 (2026-04-14) — dual-source (codex + gemini), merge `/tmp/ori-tpr-8heRkxq8`

- [x] `[TPR-04-017-codex][medium]` `section-04:3` — Narrow section scope to the 9+1 files actually covered.
  Evidence: Section title/goal claimed "every cross-repo prior-art reference" but only 9 domain files got paragraphs + 1 (intelligence.md) got a workflow refresh. At least 7 other rule files also cite reference repos without graph-first guidance (canon.md, codegen-rules.md, parse.md, llvm.md, etc.) — beyond the original TPR findings' scope.
  Resolved: Fixed on 2026-04-14. Rescoped title, goal, and success criteria to match the 9+1 file set identified by the original TPR. Other rule files noted as future coverage opportunity.

- [x] `[TPR-04-018-codex][low]` `.claude/rules/canonicalization.md:48` — Add `ori_canon/src/patterns/` to Subsystem Mapping.
  Evidence: `canonicalization.md` uses `ori-patterns` preset but the Subsystem Mapping table didn't cover `compiler/ori_canon/src/patterns/`.
  Resolved: Fixed on 2026-04-14. Added `compiler/ori_canon/src/patterns/` to the `ori-patterns` row in `.claude/rules/intelligence.md` §Subsystem Mapping.

- [x] `[TPR-04-019-codex][low]` `.claude/skills/dual-tpr/compose-intel-summary.md:20` — Add 3 missing planned consumers to SSOT registry.
  Evidence: SSOT listed only 3 planned consumers but intelligence.md §04.3 added 6. `/sync-spec`, `/sync-grammar`, `/verify-roadmap` were missing.
  Resolved: Fixed on 2026-04-14. Added all 3 to the planned-consumer list in `compose-intel-summary.md:20`.

- [x] `[TPR-04-020-codex][informational]` — Core paragraph implementation validated: slim, redirecting, no LEAK.
  Resolved: 2026-04-14 — non-actionable. Fourth consecutive informational confirming sound implementation.

- [x] `[TPR-04-021-gemini][informational]` — Zero actionable issues verified across all 5 review objectives.
  Resolved: 2026-04-14 — non-actionable.

---

## 04.N Completion Checklist

- [x] All 10 rule files (9 domain + intelligence.md refresh) reference the graph
- [x] `grep -L 'scripts/intel-query.sh' .claude/rules/{arc,aims-rules,typeck,types,tests,impl-hygiene,canonicalization,patterns,compiler,intelligence}.md` returns empty
- [x] Per-file preset tuning verified — matches the §04.1 table (post-TPR round 2 correction: types.md changed from `ori-inference` to cross-cutting pattern per TPR-04-005-codex). Current mapping: arc.md → `ori-arc`, aims-rules.md → `ori-arc`, typeck.md → `ori-inference`, types.md → cross-cutting (`file-symbols "ori_types/pool"` + `similar "TypePool"`), canonicalization.md → `ori-patterns`, patterns.md → `ori-patterns`, tests/impl-hygiene/compiler → cross-cutting (no preset).
- [x] No existing content was deleted in any rule file (diff spot-check on all 10) — the 9 domain-file inserts are purely additive (each +19 to +26 lines, 0 deletions). `intelligence.md` had 2 line replacements (stale `32K+/24K+` comment → `191K+/505K+`, and intro paragraph added under `## When to Query`) — these are corrections, not content removals. Aggregate for the 13-file commit `6d7bf77a`: 306 insertions(+), 53 deletions(-); the 53 deletions are from plan-file reformatting, not rule-file content removal.
- [x] `./test-all.sh` green — 15,314 passed / 0 failed / 139 skipped / 0 LCFail. LLVM-backend spec-test crash is BUG-04-030 (tracked in `fix-BUG-04-039`, `fix-BUG-04-074`); unrelated to §04 docs changes; test-all.sh reports green per its own verdict.
- [x] `python -m scripts.plan_corpus check plans/query-intel-adoption/section-04-rules-graph-first.md` returns 0 errors (ran silent = success)
- [ ] **Plan sync** (deferred to end of §04.N — after TPR + hygiene are clean; section cannot flip to `complete` before those gates pass):
  - [ ] Section frontmatter `status` → `complete`
  - [ ] `00-overview.md` Quick Reference and mission criterion updated
  - [ ] `index.md` updated
- [ ] `/tpr-review` passed — verify reviewers agree the insertions POINT at the graph rather than duplicate its subcommand reference
- [ ] `/impl-hygiene-review` passed — the 9 paragraphs are near-identical by design but each points at the SSOT (`intelligence.md` + `compose-intel-summary.md`); confirm this is not LEAK:algorithmic-duplication (the paragraphs are user-facing prose tuned per-file; the canonical query pattern itself lives in §03's SSOT)
- [ ] `/improve-tooling` section-close sweep
- [ ] `/sync-claude` section-close doc sync
- [ ] `diagnostics/repo-hygiene.sh --check`

**Exit Criteria:** All 9 target rule files contain a graph-first reference, with per-file subsystem-preset tuning. `intelligence.md` workflow inventory reflects the full set of post-§05 consumers. `./test-all.sh` green.
