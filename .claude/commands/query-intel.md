---
name: query-intel
description: "Query the intelligence graph. See .claude/skills/query-intel/SKILL.md for the full capability surface."
allowed-tools: Bash, Read, Grep, Glob
argument-hint: "[subcommand] [args...] — see .claude/skills/query-intel/SKILL.md for the full list"
---

# /query-intel

This command is a thin alias. The canonical capability surface lives in
`.claude/skills/query-intel/SKILL.md` — read that for the subcommand reference,
output formats, and workflow guidance.

Run: `scripts/intel-query.sh $ARGUMENTS`

If `$ARGUMENTS` is empty, run: `scripts/intel-query.sh status`
