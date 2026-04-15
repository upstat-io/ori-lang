# Step 2 — Pre-check (effectively-complete detection)

Read by a Sonnet sub-agent dispatched from `/review-plan`. Not a registered skill.

## Input

Read `/tmp/review-plan-context.json` for `mode`, `plan_dir`, `target_section`.

## What "effectively complete" means (mechanical rule)

A section is effectively complete when ALL of the following hold:

1. Frontmatter `status` is `in-progress` (not `complete`, not `not-started`)
2. Every unchecked `- [ ]` item in the section has a `<!-- blocked-by:X -->` or `<!-- blocked-by:X.Y -->` annotation
3. Every blocker target (`X` or `X.Y`) references a section IN ANOTHER PLAN (not the current plan being reviewed — self-blockers are rework, not effective-completion)
4. Every referenced blocker target exists as a concrete `- [ ]` item in the target plan/section

If ALL four hold → flip `status: complete` and append a blocker note at the top of the body:

```markdown
> **Status: effectively complete.** All remaining implementation work is blocked by external plans: {list blocker refs}. This section is done for its own scope.
```

## Scope

- **Whole-plan mode**: scan every section file in `{plan_dir}`
- **Single-section mode**: scan only `{target_section}`

## Ambiguous cases — escalate, don't guess

If ANY of these conditions hold for a section, do NOT auto-flip; add the section to the escalations list:

- A `- [ ]` item has no `blocked-by` annotation at all (could be genuinely incomplete)
- A `blocked-by` target points to a plan/section that doesn't exist (broken reference — needs human attention)
- A `blocked-by` target points to the SAME plan being reviewed (self-blocker — design issue)
- A `blocked-by` target exists but its referenced item is already `[x]` complete (stale annotation — needs cleanup)

## Output

Write `/tmp/review-plan-precheck.json`:

```json
{
  "flipped_sections": [
    {
      "section_file": "plans/foo/section-03.md",
      "old_status": "in-progress",
      "new_status": "complete",
      "blockers": ["plans/bar/section-05#07.3", "..."]
    }
  ],
  "escalations": [
    {
      "section_file": "plans/foo/section-04.md",
      "reason": "stale-blocker | broken-reference | self-blocker | missing-annotation",
      "details": "short explanation, including the offending line(s)"
    }
  ],
  "summary": "Precheck: N sections flipped, M escalations",
  "escalate": true | false
}
```

Set `"escalate": true` if the escalations list is non-empty. The parent will present them to the user via `AskUserQuestion` with the `options` array below:

```json
"options": [
  {"key": "fix-individually", "label": "Walk through each ambiguous section and decide"},
  {"key": "leave-as-is", "label": "Leave all ambiguous sections in-progress"},
  {"key": "abort", "label": "Abort review and fix manually"}
]
```

## Commit

If any sections were flipped, commit via `Skill: commit-push` with message `chore(plans): precheck — flip effectively-complete sections to complete`. Do NOT combine with later phase commits.

## Do NOT

- Write to files outside `{plan_dir}` (or `{target_section}` in single-section mode)
- Touch `reviewed` fields (not this sub-skill's job)
- Edit `.rs` / `.ori` / anything under `compiler/`, `library/`, `tests/`
- Run git commits directly — always through `/commit-push`
