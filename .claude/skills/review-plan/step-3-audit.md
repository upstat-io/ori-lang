# Step 3 — plan-audit.py orchestration

Read by a Sonnet sub-agent dispatched from `/review-plan`. Not a registered skill.

## Input

Read `/tmp/review-plan-context.json` for `plan_dir`.

## Run the audit

```bash
python3 .claude/skills/plan-audit/plan-audit.py {plan_dir} --fix-safe --apply --json \
  > /tmp/plan-audit-output.json \
  2> /tmp/plan-audit-fixes.log
```

## Parse outputs

Read both files. Extract:

- **Counts**: `critical`, `major`, `minor` totals
- **Auto-fixed**: list of what was corrected (file + message per entry)
- **Remaining findings**: everything that couldn't be auto-fixed — file, line, message, severity

## Commit auto-fixes

If `plan-audit-fixes.log` shows ANY auto-fixes were applied, commit via `Skill: commit-push` with message `chore(plans): plan-audit auto-fix mechanical drift`.

## Output

Write `/tmp/review-plan-audit.json`:

```json
{
  "counts": {"critical": 0, "major": 0, "minor": 0},
  "auto_fixed": [
    {"file": "plans/foo/section-03.md", "message": "...", "severity": "major"}
  ],
  "remaining": [
    {"file": "plans/foo/section-04.md", "line": 42, "message": "...", "severity": "critical"}
  ],
  "summary": "Phase 1: N findings (X critical, Y major, Z minor), M auto-fixed, K remaining",
  "escalate": false
}
```

Never escalate — this phase is fully mechanical.

## Do NOT

- Manually edit plan files (plan-audit.py does the fixes)
- Touch files outside `{plan_dir}`
- Run git directly — use `/commit-push`
- Interpret the remaining findings; downstream phases do that
