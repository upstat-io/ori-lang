---
section: "03"
title: "Skill Integration: TPR + Fix-Bug (Tier 1)"
status: complete
reviewed: true
goal: "Insert intelligence pre-queries into the two highest-value skills: /tpr-review (evidence packets for reviewers) and /fix-bug (investigation phase). Intelligence AUGMENTS existing workflows — it does not replace manual reference repo inspection."
success_criteria:
  - "/tpr-review Step 2 evidence packets include an Intelligence Summary section with cross-language prior art"
  - "/fix-bug Phase 1 Step 5 is split into 5a (intelligence query) and 5b (manual reference repos) — both remain under the existing design-question gate"
  - "Both skills use scripts/intel-query.sh — no open-coded Neo4j logic"
  - "Both skills check availability via `scripts/intel-query.sh status` (JSON status field) — never rely on exit codes"
  - "Both skills degrade gracefully when intelligence is unavailable"
  - "Evidence packet intelligence summary bounded at 500 chars max"
  - ".claude/rules/intelligence.md contains the canonical subsystem-to-preset mapping table — skill files reference it, never duplicate it"
depends_on: ["01", "02"]
sections:
  - id: "03.1"
    title: "TPR Review Integration"
    status: complete
  - id: "03.2"
    title: "Fix-Bug Integration"
    status: complete
  - id: "03.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "03.N"
    title: "Completion Checklist"
    status: complete
third_party_review:
  status: resolved
  updated: 2026-04-12
---

# 03 Skill Integration: TPR + Fix-Bug (Tier 1)

## 03.0 Goal

Insert intelligence queries into the two highest-value Claude skills. These are Tier 1 because:
- `/tpr-review` runs after every non-trivial change — highest frequency
- `/fix-bug` runs for every bug — highest impact per invocation

The key insight from the TPR (Codex finding #1): **Claude must pre-query and inject results into the reviewer evidence packet.** Codex and Gemini run in sandboxed environments and cannot query Neo4j themselves. So Claude queries before launch and includes the results.

**Backend**: The intelligence graph is a Neo4j graph database (Docker container, Bolt protocol). All queries flow through `scripts/intel-query.sh`, which wraps `~/projects/lang_intelligence/neo4j/query_graph.py`. The script always exits 0 — unavailability is reported via a JSON `status` field (`"ok"` or `"unavailable"`), never via exit codes.

**Execution model**: The SKILL.md files are markdown instructions that Claude (the AI agent) follows step-by-step. Each "step" is a tool invocation (Bash, Write, Read). When the plan describes running a query and inserting results into a prompt, Claude runs the query via the Bash tool, reads the output in its context window, then writes the prompt content using the Write tool (or Bash with heredoc). There is no persistent shell state between steps — Claude's context IS the state. This means intelligence summaries are held in Claude's context and written directly into prompt text, not passed via shell variables.

**SSOT for presets**: The canonical subsystem-to-preset mapping lives in `.claude/rules/intelligence.md` §Subsystem Mapping. This mapping must be added as a prerequisite before implementing this section (see 03.1 checklist). Skill files MUST reference that file for the mapping, not duplicate it.

## 03.1 TPR Review Integration

**File**: `.claude/skills/tpr-review/SKILL.md`

**Insertion point**: Between Step 0.5 (spec/grammar gate) and Step 1 (scratch dir creation).

**Prerequisite**: Add a subsystem-to-preset mapping table to `.claude/rules/intelligence.md` (currently only lists preset names without a mapping from file paths/subsystems):

```markdown
## Subsystem Mapping

Map diff file paths to intelligence presets:

| File path pattern | Preset |
|---|---|
| `compiler/ori_arc/`, `compiler/ori_rt/src/rc/` | `ori-arc` |
| `compiler/ori_types/src/infer/`, `compiler/ori_types/src/check/` | `ori-inference` |
| `compiler/ori_llvm/` | `ori-codegen` |
| `compiler/ori_patterns/`, `compiler/ori_eval/src/methods/` | `ori-patterns` |
| `compiler/ori_diagnostic/`, `compiler/oric/src/diagnostic/` | `ori-diagnostics` |
| Other / mixed | `search "<key terms from diff>"` |
```

**What to add to SKILL.md**:

```markdown
## Step 0.75 — CONDITIONAL: Intelligence Pre-Query

Query the intelligence graph for cross-language prior art relevant to the code under review.
This step runs when the graph is available and produces results; it is skipped silently
when the graph is unavailable or returns no hits.

1. Check availability via the `status` subcommand (returns JSON):
   Run: scripts/intel-query.sh status
   Parse the JSON output: if the `status` field is not `"ok"`, skip this step silently.
   Do not mention intelligence in prompts.

2. Identify the subsystem(s) from the diff (use file paths from `git diff --name-only`).
   Map subsystems to presets per .claude/rules/intelligence.md §Subsystem Mapping.

3. Run the query (output is visible in Claude's context — do NOT capture into a variable):
   Run: scripts/intel-query.sh --human <preset-or-search> --limit 5
   Read the output. If empty or only unavailability messages, skip silently.

4. Condense the query results into a bounded Intelligence Summary (max 500 chars):
   **Intelligence Summary (from cross-language graph):**
   - [rust#12345] Similar ARC bug in iterator early-exit (fixed, 45 comments)
   - [swift#6789] Protocol witness table leak on break (fixed, 12 comments)
   - Pattern appears in 3/10 reference compilers

5. Hold this condensed summary in context. In Step 2 (Write both reviewer prompts),
   write the summary directly into BOTH codex.prompt.md and gemini.prompt.md,
   after the "## Scope:" header. Do NOT use shell variable interpolation — the
   prompts use single-quoted heredocs (<<'PROMPT') which suppress expansion.
   Instead, write the intelligence summary as literal text in the prompt content.

If intelligence is unavailable or returns no results, skip silently — do not include
an empty intelligence section or "no results found" in the prompts.
```

**Implementation checklist**:
- [x] Add subsystem-to-preset mapping table to `.claude/rules/intelligence.md` §Subsystem Mapping (prerequisite — creates the SSOT the skill references)
- [x] Add Step 0.75 to `.claude/skills/tpr-review/SKILL.md` after Step 0.5, titled "CONDITIONAL"
- [x] Use `status` subcommand (JSON) for availability check, then `--human --limit 5` for actual queries — two distinct output modes
- [x] Instruct Claude to hold summary in context and write directly into prompt text (no shell variable interpolation — heredocs are single-quoted)
- [x] Note in Step 2 that when intelligence summary exists, it goes after `## Scope:` as literal text in both prompts
- [x] Intelligence summary bounded at 500 chars
- [x] Verify: available + results → summary in prompts; unavailable → silent skip; available + empty → silent skip

### Subsection 03.1 close-out

**`/improve-tooling` retrospective**: Was the 500-char bound appropriate? Too much context makes reviewers noisy; too little wastes the query. Was `--human` mode with `--limit 5` the right balance? Adjust based on actual TPR round results.

---

## 03.2 Fix-Bug Integration

**File**: `.claude/skills/fix-bug/SKILL.md`

**Insertion point**: Phase 1 (Investigation), splitting the existing Step 5 ("Check reference compilers") into Step 5a (intelligence query) and Step 5b (manual reference repo inspection). Both steps remain under the existing **design-question gate** — the current Step 5 is conditional on "if the bug involves a design question", and this condition is preserved on both 5a and 5b. This section does NOT broaden fix-bug to require cross-compiler inspection for all bugs.

Intelligence AUGMENTS manual inspection — it does NOT replace it. The intelligence DB contains GitHub issues and metadata, but NOT source code. The local reference repos at `~/projects/reference_repos/lang_repos/` contain actual source code that must still be inspected for design questions.

**What to add**:

```markdown
5. **Check reference compilers** (if the bug involves a design question):

   **5a. Intelligence Graph Query** — If the intelligence graph is available:
   1. Check availability: run `scripts/intel-query.sh status` and parse the JSON
      `status` field. If not `"ok"`, skip to 5b.
   2. Map the bug's subsystem to a preset per `.claude/rules/intelligence.md` §Subsystem Mapping.
   3. Run preset and search queries (output visible in Claude's context — do NOT
      capture into variables):
      - `scripts/intel-query.sh --human <preset> --limit 5`
      - `scripts/intel-query.sh --human search "<bug description keywords>" --limit 5`
      - `scripts/intel-query.sh --human fixed "<bug category>" --repo rust,swift,koka,lean4 --limit 5`
   4. Look for: same failure mode in 2+ compilers, how they fixed it, what regressions it caused.
   5. Record relevant findings in the fix section's investigation notes.
   If intelligence is unavailable, skip 5a entirely and proceed to 5b.

   **5b. Manual Reference Compiler Inspection** — Consult `~/projects/reference_repos/lang_repos/`
   for prior art. Intelligence results from 5a narrow the search — check the repos and issues
   it flagged first — but always inspect the actual source code.
   This sub-step is MANDATORY for design-question bugs regardless of whether 5a produced results.

   If the bug is NOT a design question, skip Steps 5a and 5b entirely.
```

**Implementation checklist**:
- [x] Split Phase 1 Step 5 in `.claude/skills/fix-bug/SKILL.md` into Step 5a (intelligence) + Step 5b (manual repos), both under the existing design-question gate
- [x] Use `status` subcommand (JSON) for availability, `--human --limit 5` for queries — same conventions as 03.1
- [x] Instruct Claude to run queries directly (visible in context), not capture into variables
- [x] Reference `.claude/rules/intelligence.md` §Subsystem Mapping for preset selection
- [x] Verify: design-question + available → 5a then 5b; design-question + unavailable → 5b only; non-design bug → skip both

### Subsection 03.2 close-out

**`/improve-tooling` retrospective**: Did the intelligence query actually help find relevant prior art? Was `--limit 5` sufficient? Was the keyword extraction from bug descriptions effective? Any query patterns that should be added as presets to `.claude/rules/intelligence.md`?

---

## 03.R Third Party Review Findings

- [x] `[TPR-03-001-codex][high]` `section-03:43` — Establish missing SSOT: subsystem-to-preset mapping not actually in intelligence.md.
  Resolved: Fixed on 2026-04-12. Added prerequisite checklist item to create §Subsystem Mapping table in intelligence.md before implementation. Success criteria updated.
- [x] `[TPR-03-002-codex][high]` `section-03:83` — Shell variable injection won't work with single-quoted heredocs in tpr-review Step 2.
  Resolved: Fixed on 2026-04-12. Rewrote to "Claude holds summary in context and writes directly into prompt text." Added execution model explanation in §03.0.
- [x] `[TPR-03-003-codex][medium]` `section-03:69` — DRIFT between JSON-status success criteria and --human query path.
  Resolved: Fixed on 2026-04-12. Separated: `status` subcommand (JSON) for availability check, `--human` for actual queries. Success criteria updated to reflect this two-mode approach.
- [x] `[TPR-03-004-codex][medium]` `section-03:135` — Removed design-question gate from Step 5b, broadening fix-bug without justification.
  Resolved: Fixed on 2026-04-12. Preserved existing "if design question" gate on both 5a and 5b. Explicit note that this section does NOT broaden fix-bug.
- [x] `[TPR-03-001-gemini][high]` `section-03:69` — Swallowed search results: capturing into variables hides output from Claude's context.
  Resolved: Fixed on 2026-04-12. Changed to "run queries directly (visible in context — do NOT capture into variables)" in both 03.1 and 03.2.
- [x] `[TPR-03-002-gemini][high]` `section-03:45` — Shell variable passing for heredoc injection is broken.
  Resolved: Fixed on 2026-04-12. Same fix as TPR-03-002-codex (agreement on core issue). Added execution model section in §03.0 explaining Claude-as-agent pattern.
- [x] `[TPR-03-001-codex-r2][medium]` `section-03:65` — Dead path `compiler/ori_eval/src/pattern/` in subsystem mapping table.
  Resolved: Fixed on 2026-04-12. Replaced with `compiler/ori_eval/src/method_dispatch/` (verified path exists).
- [x] `[TPR-03-002-codex-r2][medium]` `section-03:150` — Step 5a runs queries before mapping subsystem to preset; mapping never consumed.
  Resolved: Fixed on 2026-04-12. Reordered: map subsystem first (step 2), then run preset query (step 3) alongside search/fixed queries.
- [x] `[TPR-03-001-gemini-r2][low]` `section-03:121` — `###` headers in numbered list break markdown continuity.
  Resolved: Fixed on 2026-04-12. Changed to bold sub-items (`**5a.**`, `**5b.**`) under item 5.
- [x] `[TPR-03-002-gemini-r2][low]` `section-03:133` — "Skip to Step 5b" confusing when 5b has its own design-question gate.
  Resolved: Fixed on 2026-04-12. Clarified: non-design bugs skip both 5a and 5b entirely.
- [x] `[TPR-03-001-codex-r3][medium]` `section-03:65` — Dead path `method_dispatch/` (already fixed mid-round to `methods/`).
  Resolved: Fixed on 2026-04-12 (mid-round). Path corrected to `compiler/ori_eval/src/methods/`.
- [x] `[TPR-03-001-gemini-r3][medium]` `section-03:65` — Same dead path finding as codex-r3.
  Resolved: Same fix as TPR-03-001-codex-r3.
- [x] `[TPR-03-002-codex-r3][medium]` `section-03:16` — Frontmatter missing 03.R entry, 03.C should be 03.N, status drift.
  Resolved: Fixed on 2026-04-12. Added 03.R entry, renamed 03.C → 03.N, fixed status to not-started.
- [x] `[TPR-03-003-codex-r3][medium]` `section-03:192` — Completion checklist missing canonical close-out items.
  Resolved: Fixed on 2026-04-12. Expanded 03.N with plan-sync items per plan-schema.md.

## 03.N Completion Checklist

**Implementation verification:**
- [x] `.claude/rules/intelligence.md` has §Subsystem Mapping table (prerequisite for both skills)
- [x] `/tpr-review` SKILL.md has Step 0.75 (conditional, queries run visibly, summary written directly into prompts)
- [x] `/fix-bug` SKILL.md has Phase 1 Step 5a/5b under design-question gate, queries run visibly
- [x] Both skills use `scripts/intel-query.sh` exclusively — `status` for availability (JSON), `--human` for queries
- [x] Both degrade gracefully: unavailable → skip silently, no errors, no empty sections

**Testing and review:**
- [x] No test regressions: `timeout 150 ./test-all.sh`
- [x] `/tpr-review` clean — 1 iteration, 3 findings fixed (scope resolution, prompt template, flowchart)
- [x] `/impl-hygiene-review` clean — markdown-only changes, SSOT verified
- [x] `/improve-tooling` section-close sweep — per-subsection retrospectives covered everything; no cross-subsection patterns

**Plan sync (after section completion):**
- [x] Update section 03 frontmatter `status: complete`
- [x] Update `00-overview.md` Quick Reference table for section 03
- [x] Update `index.md` section 03 status
- [x] Clean up any plan annotations from section 03 in source code (none exist — markdown-only changes)
- [x] Verify section 04 `depends_on` is correct (updated to 01, 02, 03 — section 04 references Tier 1 contract from section 03)
