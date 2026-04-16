---
name: review-work
description: "Review actual implementation work via dual-source (Codex + Gemini) third-party review — TRIGGER proactively after completing ANY non-trivial work: bug fixes, new features, refactors, multi-file changes, compiler changes, codegen changes, test additions, plan implementations, or anything touching correctness-sensitive code. When in doubt, run it. The cost of an unnecessary review is near zero; the cost of a missed bug is high."
allowed-tools: Skill
---

# Review Work — delegates to `/tpr-review`

`/review-work` is a thin alias for `/tpr-review` with the default work mode. The substantive implementation (parallel Agent dispatch, verification-against-code, iterative fix-and-re-run loop, plan-TPR integration) lives in `.claude/skills/tpr-review/SKILL.md`.

## §1 How to invoke

Dispatch the Skill tool with the `review-work` mode flag:

```
Skill({ skill: "tpr-review", args: "--skill review-work" })
```

That single call delegates the entire workflow to `/tpr-review`:

- Parallel Agent dispatch (codex + gemini) via `.claude/skills/tpr-review/tp_agent_prompt.md`.
- Verification-against-code for every finding (Codex HIGH trust / Gemini LOWER trust).
- Round loop with stop gates (iteration cap 5, meta-only streak cap 2).
- Finding-handling policy (`CLAUDE.md §The One Rule`, banned-response list).
- Spec/grammar gate, plan-TPR frontmatter integration, coordinator render contract.

## §2 Relationship to the `/review-work` slash-command file

Typing `/review-work` at the prompt invokes `.claude/commands/review-work.md`, which is a parallel **"Claude self-reviews directly in-context"** workflow — no external CLIs. The slash-command file is deliberately unchanged and is not a duplicate of this skill.

- **`.claude/commands/review-work.md`** — in-context review, fast, Claude alone.
- **`.claude/skills/review-work/SKILL.md`** (this file) — dual-source review via external Codex + Gemini CLIs, delegates to `/tpr-review`.

Both paths coexist by design. Callers who invoke the Skill tool get the dual-source path; callers who type `/review-work` get the in-context path.

## §3 Why this skill is a delegator

Prior to 2026-04-16, this file held ~800 lines duplicating the dual-source review loop. Every change to the review protocol had to be made twice. The rewrite consolidates the loop into `/tpr-review` and reduces this file to a single Skill-tool invocation — a canonical single source of truth per `.claude/rules/impl-hygiene.md §SSOT`.

See `.claude/skills/tpr-review/SKILL.md` for:

- The full 13-section orchestrator contract (invocation surface, grounding, spec gate, trust tiers, round loop, meta-only classification, finding-handling, parallel dispatch template, failure handling, plan-TPR integration, rendering contract, model policy, and "what this skill does NOT do").
- The reviewer prompt template at `.claude/skills/tpr-review/tp_agent_prompt.md`.

## §4 When to trigger

Same trigger conditions as `/tpr-review` default mode — bug fixes, features, refactors, multi-file changes, compiler crate changes, test matrix additions, plan section implementations. When in doubt, run it.

Skip only for single-line typos, comment edits, or formatting-only changes.
