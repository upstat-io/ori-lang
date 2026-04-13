---
section: "04"
title: "Skill Integration: Remaining Skills (Tier 2+3)"
status: not-started
reviewed: true
goal: "Complete the Claude ecosystem integration by adding intelligence queries to design-pattern-review, create-draft-proposal, continue-roadmap, and review-bugs — following the Tier 1 integration contract established in Section 03."
success_criteria:
  - "/design-pattern-review Agent B uses intelligence to rank its existing 2-3 repo shortlist before reading source code"
  - "/create-draft-proposal populates a DRAFT Prior Art section from intelligence, with mandatory verification step before finalizing"
  - "/continue-roadmap queries intelligence once per section focus within a session (not per invocation) for section-relevant cross-language context"
  - "/review-bugs Step 5 recommendations cross-reference Ori bug descriptions against reference compiler issues for pattern matching"
  - "All 4 skills follow the Tier 1 integration contract: status probe first, --human mode, --limit 5, silent skip on unavailable"
  - "All skills use scripts/intel-query.sh exclusively — no open-coded Neo4j logic"
depends_on: ["01", "02", "03"]
sections:
  - id: "04.0"
    title: "Cross-Cutting: Integration Contract & Conventions"
    status: not-started
  - id: "04.1"
    title: "Design Pattern Review"
    status: not-started
  - id: "04.2"
    title: "Draft Proposal"
    status: not-started
  - id: "04.3"
    title: "Continue Roadmap"
    status: not-started
  - id: "04.4"
    title: "Review Bugs"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
third_party_review:
  status: resolved
  updated: 2026-04-12
---

# 04 Skill Integration: Remaining Skills (Tier 2+3)

## 04.0 Cross-Cutting: Integration Contract & Conventions

All 4 skill integrations in this section MUST follow the **Tier 1 integration contract** established in Section 03 (TPR + Fix-Bug). This subsection codifies the contract so each skill implements it identically — no ad-hoc degradation patterns.

### Canonical Integration Contract

Every intelligence integration point follows this exact sequence:

1. **Availability probe**: Run `scripts/intel-query.sh status` and parse the JSON output. If the `status` field is not `"ok"`, skip intelligence silently — do not mention it in output, do not include empty sections.
2. **Query with `--human` mode**: Use `--human` flag for context-friendly text output (not JSON). Use `--limit 5` to bound context size (not `--limit 20` — raw JSON from large result sets wastes context tokens).
3. **Hold results in Claude's context**: Query output is visible in Claude's context window after the Bash tool call. Do NOT capture into shell variables — SKILL.md files are executed by Claude step-by-step, and single-quoted heredocs (`<<'PROMPT'`) suppress variable expansion.
4. **Silent degradation**: If unavailable OR if results are empty, skip silently. Never include "no results found" or empty intelligence sections.

**Reference implementation**: See Section 03 — `.claude/skills/tpr-review/SKILL.md` Step 0.75 is the canonical pattern.

### Query Term Extraction Convention

All 4 skills extract query terms from their domain-specific context using the SAME principle — no ad-hoc heuristics:

1. **Primary term**: Extract from the skill's natural domain input — the design domain name (04.1), the proposal topic (04.2), the section `title` field (04.3), or the bug title/description (04.4).
2. **Preset mapping**: Map to an intelligence preset using `.claude/rules/intelligence.md` §Subsystem Mapping when the primary term maps to a known compiler subsystem. Otherwise use `search "<primary term>"`.
3. **Query types per skill**: Each subsection defines its own query shapes (`compare`, `search`, `fixed`, etc.) appropriate to its use case. The shared contract governs availability probing, output mode (`--human --limit 5`), and degradation behavior — NOT which query subcommands to use. Section 03's Tier 1 precedent uses up to 3 queries (preset + search + fixed) where appropriate.

### Intelligence Results Are Discovery, Not Replacement

Per `.claude/rules/intelligence.md`: results mean "investigate this" — never cite without verifying against actual source code or issues. This rule applies uniformly across all 4 skills.

- [ ] Document the canonical integration contract above in all 4 target files: 3 SKILL.md files (design-pattern-review, create-draft-proposal, continue-roadmap) + `.claude/commands/review-bugs.md`
- [ ] Verify all 4 skills use `--human --limit 5` consistently
- [ ] Verify skills that query compiler subsystems reference `.claude/rules/intelligence.md` §Subsystem Mapping for opportunistic preset selection (04.3 always; 04.1/04.2/04.4 only when their domain input naturally maps to a subsystem)

- [ ] **Subsection close-out (04.0)** — MANDATORY before starting 04.1:
  - [ ] All 04.0 checklist items verified complete
  - [ ] Frontmatter `sections[04.0].status` → `complete`
  - [ ] `/improve-tooling` retrospective (zero deferral, commit via valid conventional-commit type per plan-schema.md): Did the shared contract reduce divergence across the 4 integrations? Any gaps in `intel-query.sh` flags (e.g., missing `--limit` support, `--human` formatting issues) surfaced during implementation?

---

## 04.1 Design Pattern Review

**File**: `.claude/skills/design-pattern-review/SKILL.md`
**Insertion**: Agent B template (lines 128-167) — before STEP 2 (reading reference repos).

**Current state of Agent B**: Agent B already receives a **domain-specific 2-3 repo shortlist** from the file map (e.g., `arc-optimization` maps to Swift, Lean 4, Koka). It does NOT scan all 10 repos. The intelligence integration RANKS the existing shortlist by relevance, and may surface specific issue/PR numbers worth reading first — it does NOT replace or broaden the curated shortlist.

**What to add to SKILL.md** (new step in Agent B template, between STEP 1 and STEP 2):

```markdown
STEP 1.5: CONDITIONAL — Intelligence Pre-Query

If the intelligence graph is available (check: scripts/intel-query.sh status,
parse JSON status field — skip silently if not "ok"):

Run: scripts/intel-query.sh --human compare "<DOMAIN>" --limit 5
Run: scripts/intel-query.sh --human search "<DOMAIN>" --limit 5

Use results to:
- Identify which of your assigned 2-3 repos have the most relevant prior art
  (read those FIRST in Step 2)
- Note specific issue/PR numbers worth examining alongside source code
- Flag cross-compiler patterns (2+ repos hit same design challenge)

If unavailable or empty, proceed to Step 2 normally — the repo shortlist
from the file map is sufficient without intelligence ranking.
```

- [ ] Add STEP 1.5 to Agent B template in SKILL.md between STEP 1 (reading prior-art-ref.md) and STEP 2 (reading reference repos)
- [ ] Follow the Tier 1 integration contract from 04.0 (status probe, `--human --limit 5`, silent skip)
- [ ] Intelligence RANKS the existing repo shortlist — it does NOT replace it or broaden it
- [ ] Agent B still reads actual source files (intelligence provides issue metadata, not source code)

- [ ] **Subsection close-out (04.1)** — MANDATORY before starting 04.2:
  - [ ] All 04.1 checklist items verified complete
  - [ ] Frontmatter `sections[04.1].status` → `complete`
  - [ ] `/improve-tooling` retrospective (zero deferral, commit via valid conventional-commit type per plan-schema.md): Did intelligence ranking change which repo Agent B read first? Any design domains where the `compare` query type was more useful than `search`, or vice versa?

---

## 04.2 Draft Proposal

**File**: `.claude/skills/create-draft-proposal/SKILL.md`
**Insertion**: New Step 4.5 between Step 4 (Purity Analysis gate) and Step 5 (Generate the Proposal).

**What to add to SKILL.md**:

```markdown
### Step 4.5: CONDITIONAL — Intelligence Prior Art Query

Query the intelligence graph for cross-language prior art relevant to
the proposed feature. Results populate a DRAFT Prior Art section that
must be verified before inclusion.

1. Check availability: run `scripts/intel-query.sh status` and parse the
   JSON `status` field. If not `"ok"`, skip to Step 5 — omit the Prior Art
   section or populate it manually.

2. Run queries (output visible in Claude's context):
   - `scripts/intel-query.sh --human search "<proposal topic>" --limit 5`
   - `scripts/intel-query.sh --human compare "<feature concept>" --limit 5`

3. From the results, draft a `## Prior Art` section with structured entries:
   - Which languages implemented this feature
   - What issues arose (link to specific issue numbers from results)
   - What approaches were rejected and why

4. **MANDATORY VERIFICATION** — The drafted Prior Art section is a STARTING
   POINT, not a finished product. Before including it in the proposal:
   - Verify each referenced issue/PR actually exists and says what the
     summary claims (intelligence results are for DISCOVERY, not replacement
     — per .claude/rules/intelligence.md)
   - Check referenced source files in ~/projects/reference_repos/lang_repos/
     to confirm implementation details
   - Remove any entries that cannot be verified
   - Add entries discovered through manual inspection that intelligence missed

If intelligence is unavailable or returns no results, skip silently. The
Prior Art section can be populated manually or omitted for small proposals.
```

- [ ] Add Step 4.5 to SKILL.md between Step 4 (Purity Analysis) and Step 5 (Generate Proposal)
- [ ] Follow the Tier 1 integration contract from 04.0
- [ ] Include MANDATORY verification step — intelligence populates a DRAFT, author verifies each entry against actual code/issues
- [ ] Unverified entries are removed, not published

- [ ] **Subsection close-out (04.2)** — MANDATORY before TPR checkpoint:
  - [ ] All 04.2 checklist items verified complete
  - [ ] Frontmatter `sections[04.2].status` → `complete`
  - [ ] `/improve-tooling` retrospective (zero deferral, commit via valid conventional-commit type per plan-schema.md): Did intelligence help populate Prior Art sections faster than manual research alone? Was the `compare` query useful for finding which languages implemented the feature? Any feature categories that need dedicated presets in `.claude/rules/intelligence.md`?

- [ ] **TPR checkpoint** — `/tpr-review` covering 04.0–04.2 implementation work. This intermediate checkpoint catches DRIFT before it compounds across 04.3 and 04.4. Run before continuing to 04.3.

---

## 04.3 Continue Roadmap

**File**: `.claude/skills/continue-roadmap/SKILL.md`
**Insertion**: After Step 2 (focus section determination), before Step 2.5 (blocker chain resolution).

**Key constraints**:
1. `/continue-roadmap` is a hot-loop skill invoked repeatedly within a single conversation session. Querying intelligence on every invocation re-derives static facts. The integration MUST query **once per section focus within the current session** — Claude holds the query results in its conversation context, so re-querying is unnecessary as long as the focus section hasn't changed. There is no cross-session persistence mechanism (and none is needed — a new session starts fresh).
2. Section frontmatter has `title` and `goal` fields but NO `subsystem` field. Query terms are extracted from `title` and `goal`, not from a nonexistent frontmatter field.
3. Infrastructure/tooling sections (e.g., "CLI", "Tooling", "Testing Framework") do not benefit from cross-compiler intelligence. Skip these explicitly.

**What to add to SKILL.md**:

```markdown
### Step 2.1: CONDITIONAL — Intelligence Context for Focus Section

After determining the focus section in Step 2, optionally query the
intelligence graph for cross-language context relevant to this section's
domain. This step runs at most ONCE per section focus within the current
conversation session — if the focus section hasn't changed since the last
query in this session, skip (the results are still in Claude's context).
No cross-session persistence is needed; a new session starts fresh.

1. Check availability: run `scripts/intel-query.sh status` and parse the
   JSON `status` field. If not `"ok"`, skip silently.

2. Extract query terms from the focus section's frontmatter:
   - Use the `title` field as the primary search term (always valid for `search` queries)
   - Use the `goal` field for additional context keywords
   - **Preset mapping is opportunistic**: `.claude/rules/intelligence.md` §Subsystem Mapping
     maps *file path patterns* (e.g., `compiler/ori_arc/`) to presets, not section titles.
     If the section title clearly maps to a subsystem (e.g., "ARC Optimization" → `ori-arc`,
     "Type Inference" → `ori-inference`), use the preset. Otherwise, `search "<title>"` is
     sufficient — do NOT force a preset mapping when the title doesn't naturally match one.

3. **Skip condition**: If the section title indicates infrastructure or
   tooling work (e.g., contains "CLI", "Tooling", "Testing Framework",
   "Build System", "Documentation") rather than a compiler/language
   feature, skip this step — cross-compiler intelligence has low relevance
   for project-specific infrastructure.

4. Run query (output visible in Claude's context):
   - `scripts/intel-query.sh --human search "<title keywords>" --limit 5`
   - If a preset was identified: `scripts/intel-query.sh --human <preset> --limit 5`

5. Hold results as "Cross-language context for Section {NN}:" — use when
   making design decisions within the section. Do NOT inject into every
   subsection prompt; reference as needed.

If unavailable or empty, skip silently. Section work proceeds normally.
```

- [ ] Add Step 2.1 to SKILL.md after Step 2 (focus section determination), before Step 2.5
- [ ] Follow the Tier 1 integration contract from 04.0
- [ ] Extract query terms from `title` and `goal` frontmatter fields (NOT a `subsystem` field — it does not exist)
- [ ] Query once per section focus within the current session, not on every `/continue-roadmap` invocation (no cross-session persistence needed)
- [ ] Explicit skip condition for infrastructure/tooling sections that do not benefit from cross-compiler intelligence
- [ ] Results held as ambient context, not injected into every subsection

- [ ] **Subsection close-out (04.3)** — MANDATORY before starting 04.4:
  - [ ] All 04.3 checklist items verified complete
  - [ ] Frontmatter `sections[04.3].status` → `complete`
  - [ ] `/improve-tooling` retrospective (zero deferral, commit via valid conventional-commit type per plan-schema.md): Was the `title`-based term extraction accurate enough for useful queries? Which section types produced useful intelligence vs. noise? Should the skip condition list be expanded? Was the once-per-session caching effective in practice?

---

## 04.4 Review Bugs

**File**: `.claude/commands/review-bugs.md`
**Insertion**: Step 5 (Recommendations) ONLY. NOT Step 2 (OBE Check).

**Why NOT Step 2 (OBE Check)**: The OBE check determines whether THIS Ori bug was fixed by recent Ori commits — it checks test results, recent git history, and completed plans. Reference compiler issues from the intelligence graph have zero relevance to whether an Ori bug was overtaken by events. Intelligence belongs in recommendations/prioritization, not OBE detection.

**How cross-referencing works**: The intelligence graph contains reference compiler issues (Rust, Go, Swift, etc.), NOT Ori's bug tracker. Cross-referencing works by matching Ori bug *descriptions and keywords* against reference compiler issue titles and labels — pattern matching on failure modes, not direct node clustering. Example: an Ori bug titled "iterator cleanup skipped on early break" matches against similar iterator/cleanup issues in Rust and Swift, suggesting the fix approach is well-understood.

**What to add to review-bugs.md** (in Step 5: Present Summary, inside the `### Recommended Actions` block):

```markdown
**CONDITIONAL — Intelligence Cross-Reference:**

If the intelligence graph is available (check: scripts/intel-query.sh status,
parse JSON status field — skip silently if not "ok"):

For each high-priority bug being recommended for fixing, run:
  scripts/intel-query.sh --human search "<bug title keywords>" --limit 5
  scripts/intel-query.sh --human fixed "<bug category>" --repo rust,swift,koka,lean4 --limit 5

Use results to enrich recommendations:
- Bugs where 2+ reference compilers hit the same failure mode → higher
  confidence the fix approach is known (mention in recommendation)
- Bugs matching "fixed" issues in reference compilers → note the fix
  approach for the implementer's benefit
- Bug clusters (multiple Ori bugs matching the same reference compiler
  issue class) → recommend fixing together via a single fix section

If unavailable or empty, present recommendations without intelligence
enrichment — the prioritization logic works without it.
```

- [ ] Add intelligence cross-reference to Step 5 (Recommended Actions) in `review-bugs.md`
- [ ] Do NOT add intelligence to Step 2 (OBE Check) — OBE checks Ori-local state, not reference compiler issues
- [ ] Follow the Tier 1 integration contract from 04.0
- [ ] Cross-reference works by keyword/description matching against reference compiler issues, not direct graph clustering
- [ ] Results enrich recommendations with fix-approach confidence, not replace the prioritization logic

- [ ] **Subsection close-out (04.4)** — MANDATORY before completion checklist:
  - [ ] All 04.4 checklist items verified complete
  - [ ] Frontmatter `sections[04.4].status` → `complete`
  - [ ] `/improve-tooling` retrospective (zero deferral, commit via valid conventional-commit type per plan-schema.md): Did cross-referencing bugs help with prioritization or fix-approach confidence? Any bug categories where intelligence was particularly valuable? Should `fixed` queries use different repo filters for different subsystems?

---

## 04.R Third Party Review Findings

- [x] `[TPR-04-001-codex][high]` `section-04:75` — GAP: Missing schema-mandated close-out checklist blocks and intermediate TPR checkpoints.
  Resolved: Fixed on 2026-04-12. Converted all 5 close-outs to checklist format, added TPR checkpoint after 04.2.
- [x] `[TPR-04-002-codex][high]` `section-04:185` — GAP: Once-per-focus query caching has no persistence mechanism.
  Resolved: Fixed on 2026-04-12. Narrowed to same-session ambient context (Claude's conversation context is the state carrier).
- [x] `[TPR-04-003-codex][medium]` `section-04:61` — DRIFT: 04.0 "at most 2 queries" contradicts per-skill query patterns and Tier 1 precedent.
  Resolved: Fixed on 2026-04-12. Removed prescriptive query count from 04.0; each subsection defines its own query shapes.
- [x] `[TPR-04-004-codex][medium]` `section-04:14` — DRIFT: depends_on missing "03" despite sequential dependency.
  Resolved: Fixed on 2026-04-12. Added "03" to depends_on in frontmatter, index.md, and 00-overview.md.
- [x] `[TPR-04-001-gemini][high]` `section-04:166` — GAP: Missing intermediate TPR checkpoint (5 subsections, 0 checkpoints).
  Resolved: Fixed on 2026-04-12. Added TPR checkpoint after 04.2 covering 04.0-04.2.
- [x] `[TPR-04-002-gemini][high]` `section-04:75` — GAP: Close-outs as text paragraphs, not checklist format per plan-schema.md.
  Resolved: Fixed on 2026-04-12. All 5 close-outs converted to `- [ ] **Subsection close-out (04.X)**` format.
- [x] `[TPR-04-003-gemini][medium]` `section-04:196` — DRIFT: Title-based preset mapping doesn't align with intelligence.md file-path-based mapping.
  Resolved: Fixed on 2026-04-12. Clarified that preset mapping is opportunistic; search by title is the always-valid primary query.
- [x] `[TPR-04-004-gemini][medium]` `section-04:288` — GAP: Missing plan-sync items (mission criteria, TPR resolution) in completion checklist.
  Resolved: Fixed on 2026-04-12. Added mission criteria checkbox update and TPR checkpoint resolution items.
- [x] `[TPR-04-001-codex][high]` `section-01:47` — DRIFT: Section 01 --human contract says "skills NEVER pass --human" but Tier 1+2 both use it.
  Resolved: Fixed on 2026-04-12. Updated Section 01 contract text to reflect implemented usage.
- [x] `[TPR-04-002-codex][medium]` `section-04:327` — DRIFT: sections[04.R].status still not-started despite all items checked.
  Resolved: Fixed on 2026-04-12. Set 04.R status to complete; added TPR status sync item to completion checklist.
- [x] `[TPR-04-003-codex][low]` `section-04:71` — GAP: "4 skill SKILL.md files" wording excludes review-bugs.md command file.
  Resolved: Fixed on 2026-04-12. Rewrote to explicitly list 3 SKILL.md files + review-bugs.md.
- [x] `[TPR-04-001-gemini][medium]` `section-04:58` — GAP: Close-out blocks abbreviate /improve-tooling requirements.
  Resolved: Fixed on 2026-04-12. Added zero-deferral and commit-type reference to all close-out retrospective items.
- [x] `[TPR-04-002-gemini][low]` `section-04:237` — GAP: Missing exact plan-annotations script command.
  Resolved: Fixed on 2026-04-12. Added exact bash command to completion checklist.
- [x] `[TPR-04-001-codex][medium]` `section-04:73` — DRIFT: Preset-mapping contract universally required but only 04.3 implements it.
  Resolved: Fixed on 2026-04-12. Relaxed 04.0 and 04.N to "where applicable" — preset mapping is opportunistic, not universal.
- [x] `[TPR-04-002-codex][low]` `section-03:223` — DRIFT: Section 03 completion note says section 04 depends on 01, 02 only.
  Resolved: Fixed on 2026-04-12. Updated Section 03 note to reflect 01, 02, 03 dependency.
- [x] `[TPR-04-001-codex][low]` `section-04:326` — DRIFT: "4 skills" wording in 04.N and exit criteria doesn't match the 3+1 scope.
  Resolved: Fixed on 2026-04-12. Changed to "4 integration points (3 SKILL.md files + review-bugs.md)" throughout.

---

## 04.N Completion Checklist

**Implementation verification:**
- [ ] All 4 integration points modified (3 SKILL.md files + `.claude/commands/review-bugs.md`) following the Tier 1 contract from 04.0
- [ ] `/design-pattern-review` SKILL.md has Agent B STEP 1.5 (ranks existing shortlist, does not replace it)
- [ ] `/create-draft-proposal` SKILL.md has Step 4.5 (DRAFT Prior Art with mandatory verification)
- [ ] `/continue-roadmap` SKILL.md has Step 2.1 (once-per-section, title/goal extraction, infrastructure skip)
- [ ] `/review-bugs` review-bugs.md has Step 5 intelligence cross-reference (NOT in Step 2 OBE check)
- [ ] All 4 integration points use `scripts/intel-query.sh` exclusively with `--human --limit 5`
- [ ] All 4 integration points degrade gracefully: unavailable OR empty results → skip silently, no errors, no empty sections
- [ ] Skills querying compiler subsystems use `.claude/rules/intelligence.md` §Subsystem Mapping for opportunistic preset selection where applicable (04.3 always; others only when domain input maps to a subsystem)

**Testing and review:**
- [ ] No test regressions: `timeout 150 ./test-all.sh`
- [ ] `/tpr-review` passed (final, full-section) — independent dual-source review (Codex + Gemini) found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — AFTER `/tpr-review` is clean. Markdown-only changes expected; verify SSOT (no duplicated mapping tables, no open-coded Neo4j logic)
- [ ] `/improve-tooling` **section-close sweep** — verify each subsection (04.0-04.4) has either an "improvements made" entry or documented "no gaps" from its per-subsection retrospective. Look for cross-subsection patterns: repeated query flag combinations, common failure modes across skills, `intel-query.sh` gaps surfaced during implementation.

**Plan sync (after section completion):**
- [ ] This section's frontmatter `status` → `complete`, all subsection statuses (including `04.R`) updated
- [ ] `third_party_review.status` and `updated` fields synced with 04.R block state
- [ ] `00-overview.md` Quick Reference table status updated for section 04
- [ ] `00-overview.md` Mission Success Criteria checkboxes updated for items delivered by this section
- [ ] `index.md` section 04 status updated
- [ ] All intermediate TPR checkpoint findings resolved (see checkpoint item after 04.2)
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan lang-intelligence` returns 0 stale annotations for section 04
- [ ] Verify section 05's `depends_on` is correct (05 does not depend on 04 — they are independent pillars)

**Exit Criteria:** All 4 integration points (3 SKILL.md files + `.claude/commands/review-bugs.md`) contain intelligence integration that follows the Tier 1 contract. Each integration uses `scripts/intel-query.sh` with `--human --limit 5`, probes availability via `status` first, and silently degrades when unavailable. Manual testing of each skill with intelligence available confirms queries run and results appear in context; testing with intelligence unavailable confirms silent skip with no errors. `./test-all.sh` is green with no regressions.
