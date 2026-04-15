---
name: verify-roadmap-item-verifier
description: DESIGN NOTE (not a currently-wired phase) — the target shape for Phase 4 item-level verification in the verify-roadmap 5-phase pipeline. The pipeline itself is not yet implemented in `scripts/verify_roadmap/`; today's item-level verifier lives in `.claude/commands/verify-roadmap.md` (agent-driven slash command).
---

# Item-Level Verifier — Design Note (NOT IMPLEMENTED)

**Status: design target, not wired.** This document describes how Phase 4
item verification *would* integrate with the 5-phase pipeline envisioned
by `plans/completed/verify-roadmap-redesign/`. **No code in
`scripts/verify_roadmap/` reads this file, and no Phase 4 exists today.**
The item-level verifier is still the agent-driven slash command at
`.claude/commands/verify-roadmap.md` (~914 lines), invoked directly via
`/verify-roadmap`.

Treat this file as:

- **A future-state anchor** describing what the implemented Phase 4
  contract should look like.
- **Documentation of the canonical prompt-template home** — when Phase 4
  does ship, it should reuse the prompts in `.claude/commands/verify-roadmap.md`
  rather than duplicate them.
- **NOT a wiring contract.** Nothing today reads or executes the protocol
  described below.

## Canonical prompt sources (target SSOT)

When Phase 4 is implemented, the review-agent and update-agent prompt
templates should live in `.claude/commands/verify-roadmap.md` — they are
there today and should stay there. The extracted module should point at
them rather than duplicating — inlining the ~900-line prompt templates
would create a `LEAK:algorithmic-duplication` that would drift from the
canonical command file as reviewers evolve their standards.

Relevant sections in `.claude/commands/verify-roadmap.md`:

- **Review agent protocol**: §"Phase 1: Review" + §"Phase 1: Review Agent
  Protocol" (lines 76–202+).
- **Update agent protocol**: §"Phase 2: Update Section Files" +
  §"Frontmatter Updates" (lines 159–636).
- **Verification criteria**: §"Verification Criteria" (lines 649+).

## Six target verification criteria (preserved from the original command)

Whichever implementation ships, it must retain every criterion the original
`/verify-roadmap` command assesses:

1. **Matrix coverage** — does the section have type × pattern × feature
   test coverage per `.claude/rules/tests.md` §Matrix Testing Rule?
2. **Semantic pin presence** — does the section have at least one test
   that would fail if the fix were reverted?
3. **Test quality** — are tests testing behavior (not implementation)?
4. **Hygiene audit** — dead code, stale references, missing docs,
   leftover plan annotations on completed work.
5. **Gap analysis** — what is missing from the section's claimed scope
   vs. what the checkbox list declares?
6. **Checkbox item verification** — are `[x]` items actually complete?

When the Phase 4 implementation lands, it should either reuse the
`FindingCategory.ITEM_VERIFICATION` enum defined in
`scripts/plan_corpus/types.py` (if that subtype set ever lands in §01.3)
or emit freestanding findings the pipeline can aggregate.

## Target invocation contract (future state — not implemented)

**Input (proposed)**:
- `section_path: Path` — path to the section file to verify.
- `scope: Literal["full", "quick"]` — `full` runs all six criteria;
  `quick` runs matrix + checkbox only (fast pre-check).

**Output (proposed)**:
- `list[Finding]` — findings using the `Finding` dataclass from
  `scripts/plan_corpus/types.py`.

**Pipeline placement (proposed)**: Phase 4 of the 5-phase verify-roadmap
pipeline would run between Phase 3 (cross-plan conflict classification)
and Phase 5 (write-back + report). The pipeline itself is not yet
implemented — see SKILL.md for the implemented CLI surface.

## What to use TODAY

If you need item-level verification right now, invoke the slash command
directly:

```
/verify-roadmap
```

The slash command dispatches review + update agents per section against
the plan-corpus indexes it discovers. It does NOT go through the
`scripts/verify_roadmap/` programmatic pipeline — those are separate
surfaces that will merge when Phase 4 ships.

## Related

- `.claude/commands/verify-roadmap.md` — canonical agent prompt templates
  (SSOT). Invoke this directly today.
- `.claude/skills/verify-roadmap/SKILL.md` — the programmatic
  `scripts/verify_roadmap/` surface (only `--quick` and `--full`-stub
  are implemented today).
- `scripts/plan_corpus/types.py` — `Finding` / `FindingCategory` /
  `FindingSubtype` SSOT.
- `plans/completed/verify-roadmap-redesign/` — the plan that proposed
  the 5-phase pipeline. Closed with the programmatic pipeline NOT yet
  implemented (see §05.3 follow-up anchors).
