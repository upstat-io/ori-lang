#!/usr/bin/env bash
set -euo pipefail

MSG_FILE="$1"
MSG=$(head -1 "$MSG_FILE")

# Allow merge commits
if [[ "$MSG" =~ ^Merge\  ]]; then
    exit 0
fi

# Allow revert commits
if [[ "$MSG" =~ ^Revert\  ]]; then
    exit 0
fi

# Conventional commit pattern
# Format: type(scope): description OR type: description
# Types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert
PATTERN="^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\([a-zA-Z0-9_-]+\))?: .+"

if [[ ! "$MSG" =~ $PATTERN ]]; then
    cat <<'EOF'

ERROR: Commit message does not follow Conventional Commits format.

Expected format: <type>(<scope>): <description>
             or: <type>: <description>

Valid types:
  feat:     A new feature
  fix:      A bug fix
  docs:     Documentation only changes
  style:    Code style changes (formatting, semicolons, etc)
  refactor: Code change that neither fixes a bug nor adds a feature
  perf:     Performance improvement
  test:     Adding or correcting tests
  build:    Changes to build system or dependencies
  ci:       Changes to CI configuration
  chore:    Other changes that don't modify src or test files
  revert:   Reverts a previous commit

Examples:
  feat: add user authentication
  fix(parser): handle empty input correctly
  docs(readme): update installation instructions

EOF
    echo "Your message was:"
    echo "  $MSG"
    echo ""
    exit 1
fi
