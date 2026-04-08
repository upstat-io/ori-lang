#!/usr/bin/env bash
# lint-dual-tpr-docs.sh — verify dual-TPR documentation invariants.
#
# Lints the dual-TPR documentation files (transport.md + both gemini
# SKILL.md files) for two classes of bug that both landed in the
# dual-tpr-gemini/section-03.3 work product before self-review caught
# them:
#
#   (1) Internal path references must resolve. Any `.claude/...` or
#       `.gemini/...` path mentioned in one of the linted docs MUST
#       point to a file that actually exists on disk. This catches
#       typos like `.claee/skills/dual-tpr/findings-schema.json`
#       (single-letter drop in "claude") that would propagate to
#       every future wrapper copying the template.
#
#   (2) Required literal phrases must be present. Some checks in the
#       plan require specific strings to appear verbatim in the docs
#       (e.g., both `Activate the review-work skill` and
#       `Activate the review-plan skill` must appear in transport.md).
#       Prose instructions like "substitute as appropriate" do NOT
#       satisfy these checks. This rule catches the gap where a
#       template author wrote substitution instructions in English
#       instead of both literal strings.
#
# This lint is a peer to `lint-command-file.sh`, not a replacement
# for it. command-file.md has a tightly-scoped contract
# (reviewer-agnostic + 6 methodology concepts) that deserves its own
# single-purpose lint. This umbrella lint owns the remaining dual-TPR
# surface: transport.md + both gemini SKILL.md files.
#
# Usage:
#   lint-dual-tpr-docs.sh              # lint the canonical dual-TPR docs
#
# Exits 0 if all checks pass, 1 if any lint fails, 2 on usage error.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"

TRANSPORT="$REPO_ROOT/.claude/skills/dual-tpr/transport.md"
GEMINI_REVIEW_WORK="$REPO_ROOT/.gemini/skills/review-work/SKILL.md"
GEMINI_REVIEW_PLAN="$REPO_ROOT/.gemini/skills/review-plan/SKILL.md"

PASS=0
FAIL=0
FAILED_CHECKS=()

check() {
  local name="$1"
  local actual_exit="$2"
  local expected_exit="$3"
  if [[ "$actual_exit" == "$expected_exit" ]]; then
    echo "  PASS: $name"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $name (expected exit=$expected_exit, got $actual_exit)"
    FAIL=$((FAIL + 1))
    FAILED_CHECKS+=("$name")
  fi
}

# --- Check: every linted file exists ---
echo "=== file inventory ==="
for f in "$TRANSPORT" "$GEMINI_REVIEW_WORK" "$GEMINI_REVIEW_PLAN"; do
  rel="${f#$REPO_ROOT/}"
  if [[ -f "$f" ]]; then
    echo "  PASS: $rel exists"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $rel does not exist"
    FAIL=$((FAIL + 1))
    FAILED_CHECKS+=("file inventory: $rel")
  fi
done

# If any target is missing, further checks are meaningless.
if [[ $FAIL -gt 0 ]]; then
  echo ""
  echo "=== summary ==="
  echo "PASS: $PASS"
  echo "FAIL: $FAIL"
  echo "Failed checks:"
  for c in "${FAILED_CHECKS[@]}"; do
    echo "  - $c"
  done
  exit 1
fi

# --- Check: internal path references resolve ---
# Extract every `.claude/...` and `.gemini/...` path reference from
# each linted doc, strip trailing punctuation that is unambiguously
# not part of the path (backtick, comma, period-at-end, close-paren),
# and verify the resulting path exists on disk. Paths inside fenced
# code blocks are included; they are the most common source of typos.
echo ""
echo "=== internal path resolution ==="
for f in "$TRANSPORT" "$GEMINI_REVIEW_WORK" "$GEMINI_REVIEW_PLAN"; do
  rel="${f#$REPO_ROOT/}"
  # grep -oE emits one match per line; sort -u deduplicates; while-read
  # loop processes one cleaned path per iteration (never parses multi-path
  # loop variables, which is the fragility that bit the 03.3 retrospective
  # harness).
  missing_for_file=0
  while IFS= read -r raw; do
    # Strip trailing punctuation: backtick, comma, period, close-paren, colon.
    clean=$(printf '%s' "$raw" | sed 's/[,`.)]*$//' | sed 's/:$//')
    if [[ -z "$clean" ]]; then
      continue
    fi
    # Also strip a trailing slash if present (directory reference).
    clean_noslash="${clean%/}"
    if [[ -e "$REPO_ROOT/$clean_noslash" ]]; then
      : # resolves
    else
      echo "  FAIL: $rel references $clean (does not exist)"
      missing_for_file=$((missing_for_file + 1))
    fi
  done < <(grep -oE '\.(claude|gemini)/[A-Za-z0-9_./-]+' "$f" | sort -u)
  if [[ $missing_for_file -eq 0 ]]; then
    total=$(grep -oE '\.(claude|gemini)/[A-Za-z0-9_./-]+' "$f" | sort -u | wc -l)
    echo "  PASS: $rel (all $total unique internal paths resolve)"
    PASS=$((PASS + 1))
  else
    FAIL=$((FAIL + missing_for_file))
    FAILED_CHECKS+=("internal path resolution: $rel ($missing_for_file missing)")
  fi
done

# --- Check: required literal phrases present ---
# These are phrases that MUST appear verbatim in specific docs. Each
# entry is "FILE|||PHRASE|||DESCRIPTION" using ||| as a safe delimiter
# (newlines and pipes can appear in phrases; ||| does not).
echo ""
echo "=== required literal phrases ==="

REQUIRED=(
  "$TRANSPORT|||Activate the review-work skill|||transport.md review-work activation phrase"
  "$TRANSPORT|||Activate the review-plan skill|||transport.md review-plan activation phrase"
  "$TRANSPORT|||envelope-only|||transport.md codex mode keyword"
  "$GEMINI_REVIEW_WORK|||google_web_search|||review-work gemini grounding directive"
  "$GEMINI_REVIEW_PLAN|||google_web_search|||review-plan gemini grounding directive"
  "$GEMINI_REVIEW_WORK|||Critical envelope contract points|||review-work envelope contract header"
  "$GEMINI_REVIEW_PLAN|||Critical envelope contract points|||review-plan envelope contract header"
  "$GEMINI_REVIEW_WORK|||BEGIN-ORI-DUAL-TPR-V1|||review-work sentinel begin"
  "$GEMINI_REVIEW_WORK|||END-ORI-DUAL-TPR-V1|||review-work sentinel end"
  "$GEMINI_REVIEW_PLAN|||BEGIN-ORI-DUAL-TPR-V1|||review-plan sentinel begin"
  "$GEMINI_REVIEW_PLAN|||END-ORI-DUAL-TPR-V1|||review-plan sentinel end"
)

for entry in "${REQUIRED[@]}"; do
  file="${entry%%|||*}"
  rest="${entry#*|||}"
  phrase="${rest%%|||*}"
  desc="${rest#*|||}"
  grep -qF "$phrase" "$file"
  check "$desc" "$?" "0"
done

echo ""
echo "=== summary ==="
echo "PASS: $PASS"
echo "FAIL: $FAIL"
if [[ $FAIL -gt 0 ]]; then
  echo "Failed checks:"
  for c in "${FAILED_CHECKS[@]}"; do
    echo "  - $c"
  done
  exit 1
fi
exit 0
