---
name: verify-roadmap-item-verifier
description: Item-level verifier module for the verify-roadmap skill. Wraps the review + update agent protocol from `.claude/commands/verify-roadmap.md` so it can be invoked as Phase 4 of the 5-phase pipeline or directly against a single section via `--section <path>`.
---

# Item-Level Verifier

Extracted from `.claude/commands/verify-roadmap.md` (914-line command file) per
plans/verify-roadmap-redesign §04. This module preserves the existing verifier
capability while making it composable with the new cross-plan analysis.

## Canonical prompt sources (SSOT)

The review-agent and update-agent prompt templates live in
`.claude/commands/verify-roadmap.md`. This module points at them rather than
duplicating — inlining the ~900-line prompt templates here would create a
`LEAK:algorithmic-duplication` that would drift from the canonical command file
as reviewers evolve their standards.

- **Review agent protocol**: `.claude/commands/verify-roadmap.md` §"Phase 1:
  Review" + §"Phase 1: Review Agent Protocol" (lines 76–202+).
- **Update agent protocol**: `.claude/commands/verify-roadmap.md` §"Phase 2:
  Update Section Files" + §"Frontmatter Updates" (lines 159–636).
- **Verification criteria**: `.claude/commands/verify-roadmap.md`
  §"Verification Criteria" (lines 649+).

When the 5-phase pipeline (or a direct `--section` invocation) needs to run
item-level verification, it **reads these sections** from the command file and
builds agent prompts that match the canonical templates, parameterized by the
section file path.

## Six preserved verification criteria

The item-level verifier retains every criterion the original `/verify-roadmap`
command assessed:

1. **Matrix coverage** — does the section have type × pattern × feature test
   coverage per `.claude/rules/tests.md` §Matrix Testing Rule?
2. **Semantic pin presence** — does the section have at least one test that
   would fail if the fix were reverted?
3. **Test quality** — are tests testing behavior (not implementation)?
4. **Hygiene audit** — dead code, stale references, missing docs, leftover
   plan annotations on completed work.
5. **Gap analysis** — what is missing from the section's claimed scope vs.
   what the checkbox list declares?
6. **Checkbox item verification** — are `[x]` items actually complete?

Severity mapping and finding categorization are defined in
`scripts/plan_corpus/types.py` (`FindingCategory.ITEM_VERIFICATION` + its
subtypes) per §01.3 SSOT. This module does NOT redefine categories; it emits
findings against the canonical `Finding` dataclass.

## Invocation contract

**Input**:
- `section_path: Path` — path to the section file to verify
  (e.g. `plans/roadmap/section-01-type-system.md` OR
  `plans/repr-opt/section-03-trampoline-contracts.md` — any plan, not just the
  master roadmap)
- `scope: Literal["full", "quick"]` — `full` runs all six criteria;
  `quick` runs matrix + checkbox only (fast pre-check)

**Output**:
- `list[Finding]` — findings using the `Finding` dataclass from
  `scripts/plan_corpus/types.py`. Each finding carries:
  - `category: FindingCategory.ITEM_VERIFICATION`
  - `subtype`: one of `MISSING_MATRIX_COVERAGE`, `MISSING_SEMANTIC_PIN`,
    `MISSING_NEGATIVE_PIN`, `WEAK_TEST`, `HYGIENE_VIOLATION`,
    `INCOMPLETE_CHECKBOX`, `SCOPE_GAP`
  - severity mapping per §04.2: semantic/negative pin + checkbox = high,
    matrix coverage + weak test + scope gap = medium, hygiene = low
  - `source` = the section file path
  - `source_line` = the checkbox / header line the finding references

## Pipeline integration (Phase 4 of the 5-phase pipeline)

- **`--full` mode**: Phase 4 runs item-verification on sections flagged by
  Phase 3 classifiers (BLOCKED / CONFLICT / SUPERSEDED findings).
- **`--quick` mode**: Phase 4 is **skipped** — cross-plan check only.
- **`--deep-all` mode**: Phase 4 runs on every section in the corpus
  (original command behavior).
- **`--section <path>` mode**: Phases 1–3 skipped; Phase 4 runs on the
  specified section only; Phase 5 reports findings from the single section.
- **`--plan <name>` mode**: Phases 1–3 scoped to the named plan directory;
  Phase 4 runs on all sections of that plan.

## Non-regression contract

This module preserves every capability of the original command. Specifically:

- The review-agent prompt structure is unchanged — same criteria, same
  reporting format.
- The update-agent's write-back format is unchanged — findings land in each
  section's `## {NN}.R Third Party Review Findings` block using the canonical
  reviewer-tagged `[TPR-NN-NNN-{reviewer}][severity]` shape.
- The update agent respects the §01 frontmatter schema — `third_party_review.
  status`, `updated`, subsection `status` fields all maintained per canonical
  shape.

If any of the six criteria or the write-back format ever diverge from the
`.claude/commands/verify-roadmap.md` source, the divergence is a
`LEAK:scattered-knowledge` finding against this module.

## Related

- `.claude/commands/verify-roadmap.md` — canonical agent prompt templates
  (SSOT)
- `.claude/skills/verify-roadmap/SKILL.md` — outer skill that orchestrates
  Phases 1–5 and delegates Phase 4 here
- `scripts/plan_corpus/types.py` — `Finding` / `FindingCategory` /
  `FindingSubtype` SSOT (§01.3)
- `scripts/verify_roadmap/` — programmatic cross-plan classifiers (Phases 1–3)
  that feed this module via the Phase 3 report
