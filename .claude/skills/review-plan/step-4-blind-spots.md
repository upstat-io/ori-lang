# Step 4 — /tp-help blind-spot analysis

Read by a Sonnet sub-agent dispatched from `/review-plan`. Not a registered skill.

## Input

Read `/tmp/review-plan-context.json` for `mode`, `plan_dir`, `target_section`.

Read the plan to build context for /tp-help:

- `{plan_dir}/00-overview.md` — mission, section list with goals and statuses
- `{plan_dir}/index.md` (if exists) — full_name, description
- In single-section mode, also read `{target_section}` body

## Dispatch /tp-help

```
Skill: tp-help
```

Provide a prompt that contains:

- **The plan's mission/goal** (from `00-overview.md`)
- **The section list** (id, title, goal, status)
- **Plan scope** (which crates, which subsystems)
- **Review mode** (single-section vs whole-plan)
- **Specific questions:**
  - "What are the most likely failure modes this review should watch for?"
  - "What architectural risks or blind spots would you flag?"
  - "Are there cross-cutting concerns that might fall between section boundaries?"

Wait for /tp-help to complete.

## Distill the response

The /tp-help return contains BOTH reviewers' raw text concatenated. Extract:

- **Blind spots** — bullet list, max 10 items, merged across reviewers (drop duplicates)
- **Architectural risks** — ≤5 bullets
- **Cross-cutting concerns** — ≤5 bullets

Each bullet must be ≤200 characters and reference something specific (a file, a section, a named risk). Reject vague bullets like "consider more testing" — if both reviewers only produce vague output, note it in the `summary` field and return an empty list.

## Output

Write `/tmp/review-plan-blind-spots.json`:

```json
{
  "blind_spots": [
    "Section 04's Tag::Var propagation rule contradicts Section 02's validate_body_types behavior when …"
  ],
  "architectural_risks": [
    "Bodies-pass integration risks breaking #[ignore] tests that assumed silent passthrough"
  ],
  "cross_cutting": [
    "impl-hygiene tests.md matrix testing rules not woven into Section 03 checklist"
  ],
  "summary": "Phase 2: N blind spots, M risks, K cross-cutting concerns",
  "escalate": false
}
```

Never escalate — this phase is advisory. The editor phase uses these as discovery pointers, not authoritative claims.

## Do NOT

- Make plan edits in this phase (that's the editor's job)
- Cite /tp-help results as authoritative — they're discovery
- Truncate the tp-help context aggressively (reviewers need context to produce useful blind spots); only the distilled bullets are bounded
