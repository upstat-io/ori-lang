#!/usr/bin/env bash
# prose-lint.sh — pre-commit gate for compiler-source internal-vocabulary leaks.
#
# Runs scripts/prose-lint.py against staged *.rs / *.ori files. The script
# detects internal-vocabulary leaks that must not appear in the public OSS
# compiler repo: bug IDs (BUG-XX-NNN), methodology vocab (TPR Round,
# semantic/negative pin, INVERTED-TDD, TDD matrix, PIN-N + IR-/DERIVE-/MIXED-
# prefixed forms), reviewer-tool names (codex/gemini/opencode), internal-doc
# paths (CLAUDE.md, .claude/..., impl-hygiene.md, etc.).
#
# SSOT: scripts/prose-lint.py lives in the wrapper repo at ../scripts/. This
# script is the cross-repo invocation shim. Pattern matches aims-spec-drift
# in compiler_repo/lefthook.yml which also calls a wrapper-side helper via
# ../.claude/...
#
# Bedding-in mode (--exit-zero): always exits 0; surfaces findings to stderr.
# Promote to gating by dropping --exit-zero once existing source-side leaks
# are cleaned.
#
# Graceful-skip: if the wrapper's prose-lint.py is not present (public OSS
# standalone clone — no wrapper checkout), exit 0 silently. The gate is a
# team-workflow hygiene check, not an OSS-consumer obligation.

set -euo pipefail

readonly PROSE_LINT="../scripts/prose-lint.py"

if [[ ! -f "$PROSE_LINT" ]]; then
  exit 0
fi

# Lefthook substitutes {staged_files} as space-separated paths. With no args
# (manual run) prose-lint defaults to its built-in scope, which excludes
# compiler source — guard against that by requiring at least one arg.
if [[ $# -eq 0 ]]; then
  exit 0
fi

exec python3 "$PROSE_LINT" --exit-zero "$@"
