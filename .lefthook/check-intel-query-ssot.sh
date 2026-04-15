#!/bin/bash
# Pre-commit gate: enforce compose-intel-summary.md SSOT for intel-query status checks.
#
# INVARIANT: only 3 files in .claude/ are permitted to contain the literal
# string `scripts/intel-query.sh status`:
#   - .claude/skills/dual-tpr/compose-intel-summary.md  (the SSOT itself)
#   - .claude/rules/intelligence.md                     (teaching surface)
#   - .claude/commands/query-intel.md                   (teaching surface)
#
# Every other consumer MUST acquire the availability check via
# @-include: `@.claude/skills/dual-tpr/compose-intel-summary.md`.
#
# This catches the Algorithmic DRY violation (impl-hygiene.md §SSOT) where
# a new consumer inlines the status-check block instead of @-including the
# SSOT — a LEAK:intel-pre-query-pattern that would propagate divergent
# availability-check semantics across the skill/command surface.
#
# See plans/query-intel-adoption/section-05-missing-trigger-skills.md §05.3
# retrospective for the motivation.
#
# Exit 0 if only the 3 approved files contain the status string.
# Exit 1 otherwise, printing the offending files.

set -e

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT"

# Allowed files — paths relative to repo root.
APPROVED=(
  ".claude/skills/dual-tpr/compose-intel-summary.md"
  ".claude/rules/intelligence.md"
  ".claude/commands/query-intel.md"
)

# All files under .claude/ that contain the literal status-check string.
# -F for fixed-string (no regex), --glob excludes worktree mirrors.
mapfile -t hits < <(
  rg -l -F 'scripts/intel-query.sh status' --glob '!worktrees/**' .claude/ 2>/dev/null | sort
)

# Filter out approved files.
offenders=()
for f in "${hits[@]}"; do
  approved=0
  for a in "${APPROVED[@]}"; do
    if [ "$f" = "$a" ]; then approved=1; break; fi
  done
  if [ "$approved" = "0" ]; then
    offenders+=("$f")
  fi
done

if [ "${#offenders[@]}" -gt 0 ]; then
  echo "LEAK: intel-query SSOT invariant violated"
  echo ""
  echo "Files containing 'scripts/intel-query.sh status' outside approved set:"
  for f in "${offenders[@]}"; do
    echo "  $f"
  done
  echo ""
  echo "Fix: replace the inlined status-check block with"
  echo "  @.claude/skills/dual-tpr/compose-intel-summary.md"
  echo "at the consumer's intel section."
  echo ""
  echo "Approved surfaces (may reference intel-query.sh status directly):"
  for a in "${APPROVED[@]}"; do
    echo "  $a"
  done
  exit 1
fi

exit 0
