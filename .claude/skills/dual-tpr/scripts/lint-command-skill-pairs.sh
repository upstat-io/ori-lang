#!/usr/bin/env bash
# lint-command-skill-pairs.sh — validate .claude/commands/ ↔ .claude/skills/ overlap pairs
#
# Enumerates all command-skill overlap pairs, classifies each as:
#   thin-pointer:      command is a small shim pointing to the canonical skill
#   parallel-workflow: both are substantive with different use cases
#   unknown:           drift — neither pattern detected (finding)
#
# Usage:
#   .claude/skills/dual-tpr/scripts/lint-command-skill-pairs.sh [--help]
#
# Exits 0 if all pairs classify cleanly; non-zero if any pair is "unknown".

set -euo pipefail

# --- Color support ---
if [[ "${NO_COLOR:-}" != "" || ! -t 1 ]]; then
  RED="" GREEN="" YELLOW="" RESET=""
else
  RED=$'\033[0;31m' GREEN=$'\033[0;32m' YELLOW=$'\033[0;33m' RESET=$'\033[0m'
fi

if [[ "${1:-}" == "--help" ]]; then
  echo "Usage: lint-command-skill-pairs.sh [--help]"
  echo ""
  echo "Validates .claude/commands/ ↔ .claude/skills/ overlap pairs."
  echo "Classifies each as thin-pointer, parallel-workflow, or unknown (drift)."
  echo "Exits 0 if all pairs classify; non-zero if any is unknown."
  exit 0
fi

CMD_DIR=".claude/commands"
SKILL_DIR=".claude/skills"
ERRORS=0
PAIRS=0

# Operational markers that indicate a command has substantive content
# (not just a thin pointer to a skill).
OPERATIONAL_MARKERS='codex exec|gemini --|python3 |Step [0-9]+:|run_in_background|Phase [0-9]+:|### Step [0-9]'

for cmd_file in "$CMD_DIR"/*.md; do
  name=$(basename "$cmd_file" .md)
  skill_file="$SKILL_DIR/$name/SKILL.md"

  [[ -f "$skill_file" ]] || continue
  PAIRS=$((PAIRS + 1))

  cmd_lines=$(wc -l < "$cmd_file")
  # Check for operational markers in the command file
  has_ops=$(grep -cE "$OPERATIONAL_MARKERS" "$cmd_file" 2>/dev/null || true)

  if [[ "$cmd_lines" -le 50 && "$has_ops" -eq 0 ]]; then
    echo "${GREEN}OK${RESET}  $name: thin-pointer (${cmd_lines} lines, 0 operational markers)"
  elif [[ "$cmd_lines" -gt 50 && "$has_ops" -gt 0 ]]; then
    echo "${GREEN}OK${RESET}  $name: parallel-workflow (${cmd_lines} lines, ${has_ops} operational markers)"
  elif [[ "$cmd_lines" -gt 50 && "$has_ops" -eq 0 ]]; then
    # Large command without operational markers — could be either pattern.
    # Check if it explicitly declares a relationship to the skill.
    if grep -qi "skill\|canonical\|thin.pointer\|parallel" "$cmd_file" 2>/dev/null; then
      echo "${GREEN}OK${RESET}  $name: parallel-workflow (${cmd_lines} lines, declarative)"
    else
      echo "${YELLOW}WARN${RESET} $name: large command (${cmd_lines} lines) without operational markers or relationship declaration"
      ERRORS=$((ERRORS + 1))
    fi
  else
    # Small with operational markers — likely a thin pointer that drifted
    echo "${RED}FAIL${RESET} $name: unknown pattern (${cmd_lines} lines but ${has_ops} operational markers — possible drift)"
    ERRORS=$((ERRORS + 1))
  fi
done

echo ""
echo "Pairs checked: $PAIRS"
if [[ "$ERRORS" -gt 0 ]]; then
  echo "${RED}$ERRORS pair(s) could not be classified — possible SSOT drift${RESET}"
  exit 1
else
  echo "${GREEN}All pairs classified cleanly${RESET}"
  exit 0
fi
