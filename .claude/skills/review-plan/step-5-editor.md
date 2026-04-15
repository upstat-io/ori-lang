# Step 5 — 4-lens Editor

Read by a Sonnet sub-agent dispatched from `/review-plan`. Not a registered skill. Model policy: Sonnet by default; change the dispatch in `SKILL.md`'s Step 5 dispatch to `model: "opus"` if empirical convergence metrics show Sonnet produces vague edits.

You are the primary writer in the /review-plan pipeline. You have FULL AUTHORITY to restructure the plan: add sections, remove sections, merge, split, reorder, rewrite checklists, change boundaries — whatever serves the mission. **Never scope down.**

## Input

Read:

- `/tmp/review-plan-context.json` — `mode`, `plan_dir`, `target_section`
- `/tmp/review-plan-precheck.json` — flipped sections + any escalated ambiguities (for awareness)
- `/tmp/review-plan-audit.json` — remaining findings (treat as authoritative — plan-audit.py is deterministic)
- `/tmp/review-plan-blind-spots.json` — reviewer-surfaced blind spots, risks, cross-cutting concerns (treat as discovery pointers, not conclusions)

## Prerequisite reads

```
Read file: CLAUDE.md
Read file: .claude/rules/impl-hygiene.md
Read file: .claude/rules/compiler.md
Read file: .claude/rules/tests.md
```

Then read every file in `{plan_dir}` (single-section mode: read siblings for context, edit `{target_section}` only).

## Intelligence Reconnaissance (run AFTER plan read, BEFORE editing)

Once you have read the plan, you know exactly what files, types, functions, and subsystems it touches. That is when the intelligence graph pays off — targeted structural queries that tell you blast radius, cross-file references, and prior art before you decide what to restructure or rewrite.

Check graph availability first — skip silently if unavailable:

```bash
scripts/intel-query.sh status
```

If `status == "ok"`, run these categories of queries based on what the plan touches:

1. **Blast radius** — for each high-signal symbol the plan proposes to add, rename, remove, or significantly change, run BOTH directions:
   ```bash
   scripts/intel-query.sh --human callers "<symbol>" --repo ori
   scripts/intel-query.sh --human callees "<symbol>" --repo ori
   ```
   Use results to (a) catch unplanned cross-section touches — if a symbol the plan modifies is called by code in a different subsystem, either the plan needs a cross-link or the blast radius changes the structural decomposition; (b) size rules-weaving work — every caller site needs an embedded checklist item, not a vague "update callers".

2. **Module inventory** — for each crate the plan operates on, list existing symbols so you do not re-invent or collide:
   ```bash
   scripts/intel-query.sh --human file-symbols "<path-fragment>" --repo ori
   ```

3. **Prior art** — for each architectural decision the plan makes, check how other compilers solved the same problem:
   ```bash
   scripts/intel-query.sh --human similar "<concept or symbol>" --repo rust,swift,go,koka --limit 5
   ```
   Similarity uses vector embeddings and can return empty for Ori-specific terms — that is fine, the structural queries above still work. When it returns hits, use them as pointers to READ the reference repos under `~/projects/reference_repos/lang_repos/` for verification. Never cite a `similar` result as authoritative without verifying the actual source.

4. **Topic/symbol search** — if the plan introduces a new concept, check the ori repo for existing partial implementations:
   ```bash
   scripts/intel-query.sh --human symbols "<topic keyword>" --repo ori --limit 20
   ```

Keep reconnaissance bounded — typically 4–8 queries total. The queries inform your editing decisions; they are not a pre-written report. If a query returns nothing useful, move on.

Results are for DISCOVERY, not replacement — always verify against actual code before citing in a plan edit. Per `.claude/rules/intelligence.md`: "Never cite a Neo4j result without verifying against the actual code."

## Edit scope

- **Single-section mode** — edit `{target_section}` only. May touch `00-overview.md` / `index.md` if structural changes require it (e.g., you added success criteria that flow up to mission).
- **Whole-plan mode** — edit all files in `{plan_dir}`.

**NEVER touch `reviewed` fields** — `review-plan-verify` handles that.

## Part 1: Technical Accuracy & Feasibility

1. Cross-reference every technical claim against the codebase:
   - Do referenced files / types / functions / modules exist?
   - Are crate dependency assumptions correct? (`ori_lexer → ori_parse → ori_ir → ori_types → ori_eval → ori_llvm → oric`)
   - Are code patterns described accurately?
2. Check claims against the spec (`docs/ori_lang/v2026/spec/` — `grammar.ebnf`, `operator-rules.md`, clause files)
3. EDIT inaccuracies directly — don't flag, fix.
4. For each section, assess whether the implementation approach will actually work:
   - Can each checklist item be implemented as described?
   - Are there hidden prerequisites not mentioned?
   - Does the approach handle the full problem, or only a subset?
   - Are there architectural constraints (file size limits, phase boundaries, crate deps) that would block it?
5. Infeasible items: do NOT remove or defer. EXPAND — add prerequisite steps, restructure the section, or add a new section that addresses the blocker.
6. Structural assessment — step back and assess the plan as a whole:
   - Is this the right set of sections?
   - Are sections at the right granularity?
   - Does section ordering reflect implementation dependencies?
   - If you see a better structure, IMPLEMENT IT.

## Part 2: Mission Fulfillment & Strategic Cohesion

7. Identify the plan's stated mission/goal and mission success criteria (from `00-overview.md`)
8. Verify the success criteria hierarchy:
   - Does `00-overview.md` have a "Mission Success Criteria" section with concrete, testable criteria?
   - Does every section have `success_criteria` in frontmatter AND a "Success Criteria" block in body?
   - Does every mission criterion trace to ≥1 section that delivers it?
   - Does every section criterion connect upward to ≥1 mission criterion?
   - Are all criteria concrete and testable (not "X works" but "X produces Y when Z is run")?
   - If missing or vague, ADD them.
9. For each aspect of the mission, verify ≥1 section addresses it. If the plan covers 70% of the mission, add sections for the remaining 30%.
10. Blocker resolution — does the plan identify and resolve ALL blockers between the current codebase and the mission's goals?
    - Are there UNIDENTIFIED blockers? Search codebase, roadmap, bug-tracker, other plans.
    - If a blocker is tracked elsewhere, this plan must include resolving it with cross-links (`<!-- resolves: plans/... -->`)
11. Verify sections can be worked in order (N before N+1) — does each section's output provide what the next needs? Resolve circular dependencies by reordering or splitting.
12. Fix deferral traps: "bonus", "future", "lower priority", "nice to have", "stretch goal", "requires architectural change" → concrete mandatory tasks OR explicit `<!-- blocked-by:X -->`. Remove all soft-deferral language.

## Part 3: Section Executability & Codebase Hygiene

13. For each section, assess executability — could an implementer sit down and work through every checklist item in order?
    - Is each item a concrete, verifiable task (WHAT + WHERE)?
    - Are there hidden steps between items?
    - Would the implementer need to make design decisions not covered by the plan?
14. Vague items: EXPAND — break into specific sub-items with file paths and approach. Add "WHERE:" annotations when the location isn't obvious.
15. Too-thin sections (fewer than 3 substantive items): expand by researching the codebase for concrete items.
16. Too-large sections (20+ items, or mixing unrelated concerns): SPLIT IT.
17. Reorder items within sections if they violate crate dependency ordering.
18. **Rules weaving** — CLAUDE.md and `.claude/rules/*.md` constraints must be embedded organically in checklist items, not assumed:
    - "Add variant X, update match arms at `file.rs:123` and `other.rs:456`" NOT "Add variant (remember sync points)"
    - Key rules: TDD discipline, file size limits, crate ordering, registration sync, ARC invariants, phase boundaries, test conventions
    - If a section touches a subsystem but doesn't embed its rules, ADD the relevant constraints inline.
19. **Codebase scan** — extract every file path / crate / module the plan will touch. READ those files (up to 30 files; prioritize multi-section references). Look for:
    - **BLOAT**: Files >500 lines the plan will touch but doesn't plan to split
    - **WASTE**: Dead code, stale comments, unnecessary clones in touched files
    - **DRIFT**: Registration sync points already out of sync
    - **EXPOSURE/LEAK**: Phase bleeding in files the plan modifies
    - NOTE: Phase 1 (plan-audit.py) handled DEAD_PATH deterministically. Focus on architectural issues requiring judgment.
20. Weave "fix along the way" checklist items for real findings (file:line required — no fabrication):
    - `- [ ] **[BLOAT]** file:line — Split into submodules (currently N lines)`
    - `- [ ] **[DRIFT]** file:line — Sync missing variant at other_file:line`
    - Group under "Cleanup" sub-heading if 3+ findings per section.

## Part 4: Testing Rigor & Final Integration

21. For EVERY section modifying compiler code, verify a test strategy meeting CLAUDE.md requirements:
    - **Matrix tests**: type × pattern dimensions explicitly named
    - **Semantic pin**: ≥1 test that ONLY passes with the new semantics
    - **TDD ordering**: failing tests FIRST, debug+release verification LAST
    - Missing? ADD:
      - `- [ ] Write failing test matrix BEFORE implementation` (first)
      - `- [ ] Verify all tests pass in both debug and release` (last)
      - `- [ ] Add semantic pin test that only passes with new behavior`
22. Review for clarity and internal consistency:
    - Terminology consistent across sections?
    - Does the overview accurately reflect section contents?
    - Any contradictions between sections?
    - Update overview/index to reflect current structure.
23. **Plan-sync line items** — verify every section's completion checklist includes:
    - Section frontmatter status update
    - `00-overview.md` Quick Reference + mission success criteria checkboxes
    - `index.md` section status
    - Cross-links to other plans (if resolves external blockers)
    - Next section's `depends_on` verification
    - If missing → ADD from `plan-schema.md` template.
24. **Final coherence check**: read through the entire plan again. Does it tell a complete, sequential story? Is this the RIGHT plan for the mission?

## Critical Rules

- NEVER scope down — always expand. "Requires architectural change" IS the work.
- No deferral traps — every checkbox must be implementable.
- Be specific — every change needs evidence: a spec clause, a file:line, or concrete reasoning.
- Cross-reference, don't guess — read spec files and source code.
- Testing rigor is non-negotiable — matrix tests, semantic pins, TDD ordering, debug+release.
- Success criteria mandatory at both mission and section levels, bidirectional.
- Rules woven in, not assumed — plans are self-contained execution documents.
- **DO NOT touch `reviewed` fields** — `review-plan-verify` handles them.

## Commit

After editing, commit via `Skill: commit-push` with message like `feat(plans): apply /review-plan editor mutations to {plan name}`.

## Output

Write `/tmp/review-plan-editor.json`. The schema has two branches, keyed by `escalate`. Step 5 is an escalation-capable phase (listed in `review-plan/SKILL.md` §Escalation handling), so when `escalate: true` the payload MUST carry verbatim `question` + `options` fields the parent can feed directly into `AskUserQuestion` — prose-only escalations break the mechanical handoff contract.

### Branch A — `escalate: false` (normal exit)

```json
{
  "structural_changes": [
    {"type": "split", "from": "section-03", "into": ["section-03", "section-03B"], "reason": "..."},
    {"type": "add", "file": "section-07.md", "reason": "..."},
    {"type": "merge", "from": ["section-05", "section-06"], "into": "section-05"},
    {"type": "reorder", "detail": "moved section-04 before section-03"}
  ],
  "accuracy_fixes": 0,
  "cohesion_fixes": 0,
  "hygiene_items_woven": {"BLOAT": 0, "WASTE": 0, "DRIFT": 0, "EXPOSURE": 0},
  "test_strategy_gaps_filled": 0,
  "files_touched": ["plans/foo/section-03.md", "..."],
  "summary": "Phase 3: N structural changes, M accuracy fixes, K cohesion fixes, J hygiene items, H test gaps filled",
  "escalate": false
}
```

### Branch B — `escalate: true` (human judgment required)

Escalate ONLY when the editor hits a case requiring human judgment — e.g., two sections describe contradictory designs and the plan doesn't indicate which is authoritative, or the mission itself is self-contradictory and cannot be resolved by reading spec + code.

```json
{
  "structural_changes": [/* partial changes already applied, if any */],
  "accuracy_fixes": 0,
  "cohesion_fixes": 0,
  "hygiene_items_woven": {"BLOAT": 0, "WASTE": 0, "DRIFT": 0, "EXPOSURE": 0},
  "test_strategy_gaps_filled": 0,
  "files_touched": ["plans/foo/section-03.md", "..."],
  "summary": "Phase 3: N fixes applied; escalating ambiguity on <one-line description>",
  "escalate": true,
  "ambiguity": {
    "location": "plans/foo/section-03.md:120 vs plans/foo/section-05.md:45",
    "description": "Section 03 requires the lattice to be 7-dimensional; section 05 asserts a 5-dimensional lattice. Spec cites neither. Cannot pick canonically without user direction.",
    "evidence": ["<quote or line reference>", "..."]
  },
  "question": "The plan has two contradictory designs for <X>: <one-sentence framing>. How should I resolve it?",
  "options": [
    {"key": "prefer-section-03", "label": "Authoritative is section 03 (<summary>); rewrite section 05 to match"},
    {"key": "prefer-section-05", "label": "Authoritative is section 05 (<summary>); rewrite section 03 to match"},
    {"key": "split-scope", "label": "Both are valid for different scopes — split/rename the sections to avoid the conflict"},
    {"key": "abort-editor", "label": "Abort Step 5 and surface the ambiguity for manual plan work", "next_skill": null}
  ]
}
```

### Invariants

- `question` and `options` MUST live INSIDE the JSON handoff when `escalate: true`. Prose-only escalations break the parent's mechanical `AskUserQuestion` dispatch.
- When an option carries a follow-up skill dispatch, include a `next_skill` field (string skill name, or `null`). The parent invokes `Skill: <next_skill>` with any option-specific arguments when the user picks that option.
- When `escalate: false`, `question`/`options`/`ambiguity` MUST be absent.
