#!/usr/bin/env bash
# lint-dual-tpr-docs.sh — verify dual-TPR documentation invariants.
#
# Lints the dual-TPR documentation files (transport.md + both gemini
# SKILL.md files + the tpr-review wrapper) for two classes of bug
# that both landed in the dual-tpr-gemini/section-03.3 work product
# before self-review caught them:
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
TPR_REVIEW_WRAPPER="$REPO_ROOT/.claude/skills/tpr-review/SKILL.md"
REVIEW_WORK_WRAPPER="$REPO_ROOT/.claude/skills/review-work/SKILL.md"

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
for f in "$TRANSPORT" "$GEMINI_REVIEW_WORK" "$GEMINI_REVIEW_PLAN" "$TPR_REVIEW_WRAPPER" "$REVIEW_WORK_WRAPPER"; do
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
for f in "$TRANSPORT" "$GEMINI_REVIEW_WORK" "$GEMINI_REVIEW_PLAN" "$TPR_REVIEW_WRAPPER" "$REVIEW_WORK_WRAPPER"; do
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
  # Required-field documentation checks (surfaced by §05.2 Scenario 2
  # round 1 — gemini omitted schema_version because the skill file did
  # not explicitly document it as required). These assertions pin the
  # fix: both gemini skills MUST explicitly name schema_version and
  # no_findings in their envelope contract section so gemini is
  # guided to emit them.
  "$GEMINI_REVIEW_WORK|||schema_version|||review-work gemini contract documents schema_version"
  "$GEMINI_REVIEW_WORK|||no_findings|||review-work gemini contract documents no_findings"
  "$GEMINI_REVIEW_PLAN|||schema_version|||review-plan gemini contract documents schema_version"
  "$GEMINI_REVIEW_PLAN|||no_findings|||review-plan gemini contract documents no_findings"
  # Minimal envelope template checks (surfaced by §05.2 Scenario 2
  # round 2 — gemini fixed schema_version but then omitted files_read.
  # Documenting required fields one at a time is whack-a-mole.
  # The durable fix is a complete envelope template that gemini can
  # pattern-match structurally). These assertions pin the presence
  # of the template example in both gemini skills.
  "$GEMINI_REVIEW_WORK|||Minimal envelope template|||review-work gemini has minimal envelope template"
  "$GEMINI_REVIEW_WORK|||files_read|||review-work gemini template lists files_read"
  "$GEMINI_REVIEW_WORK|||rules_consulted|||review-work gemini template lists rules_consulted"
  "$GEMINI_REVIEW_PLAN|||Minimal envelope template|||review-plan gemini has minimal envelope template"
  "$GEMINI_REVIEW_PLAN|||files_read|||review-plan gemini template lists files_read"
  "$GEMINI_REVIEW_PLAN|||rules_consulted|||review-plan gemini template lists rules_consulted"
  # Codex review-work skill — parser-layer contract wording (fix for
  # §05.2 Scenario 2 round 2 finding #2). The skill previously said
  # "--output-schema flag enforces schema conformance at the CLI
  # boundary", which became stale after BUG-08-003 (commit a5a2753f)
  # removed the --output-schema flag. The new wording explains that
  # codex is validated only at the parser layer, symmetrically with
  # gemini, via parse-codex.py. This assertion pins the corrected
  # wording so a future edit cannot reintroduce the stale rationale.
  "$REPO_ROOT/.codex/skills/review-work/SKILL.md|||parser layer|||codex review-work skill documents parser-layer contract"
  "$REPO_ROOT/.codex/skills/review-work/SKILL.md|||parse-codex.py|||codex review-work skill references parse-codex.py explicitly"
  # tpr-review wrapper (Section 04) — transport script references,
  # prompt preamble phrases, and the three preserved safety blocks.
  # These checks guard against copy-paste erasure when Sections 05/06/07
  # derive their wrappers from this one as a template.
  "$TPR_REVIEW_WRAPPER|||dual-invoke-with-retry.sh|||tpr-review transport launcher reference"
  "$TPR_REVIEW_WRAPPER|||merge-findings.py|||tpr-review merger reference"
  "$TPR_REVIEW_WRAPPER|||scratch-dir.sh|||tpr-review scratch helper reference"
  "$TPR_REVIEW_WRAPPER|||envelope-only|||tpr-review codex mode keyword in prompt example"
  "$TPR_REVIEW_WRAPPER|||Activate the review-work skill|||tpr-review gemini activation phrase in prompt example"
  "$TPR_REVIEW_WRAPPER|||Step 0 — MANDATORY: Re-read CLAUDE.md|||tpr-review preserved Step 0 header"
  "$TPR_REVIEW_WRAPPER|||ABSOLUTE: You May NEVER Reason Out of Findings|||tpr-review preserved 'never reason out' ABSOLUTE"
  "$TPR_REVIEW_WRAPPER|||ABSOLUTE: Correct Architectural Solutions Only|||tpr-review preserved 'architectural solutions' ABSOLUTE"
  # review-work wrapper (Section 05) — same 8 preservation assertions as
  # tpr-review. Section 05 rewrites review-work/SKILL.md from the
  # tpr-review template to adopt dual-source, so the same copy-paste
  # erasure risks apply. Also guards the Task #10 fix (the absence of
  # "ABSOLUTE: NEVER Background") via a dedicated negative check below.
  "$REVIEW_WORK_WRAPPER|||dual-invoke-with-retry.sh|||review-work transport launcher reference"
  "$REVIEW_WORK_WRAPPER|||merge-findings.py|||review-work merger reference"
  "$REVIEW_WORK_WRAPPER|||scratch-dir.sh|||review-work scratch helper reference"
  "$REVIEW_WORK_WRAPPER|||envelope-only|||review-work codex mode keyword in prompt example"
  "$REVIEW_WORK_WRAPPER|||Activate the review-work skill|||review-work gemini activation phrase in prompt example"
  "$REVIEW_WORK_WRAPPER|||Step 0 — MANDATORY: Re-read CLAUDE.md|||review-work preserved Step 0 header"
  "$REVIEW_WORK_WRAPPER|||ABSOLUTE: You May NEVER Reason Out of Findings|||review-work preserved 'never reason out' ABSOLUTE"
  "$REVIEW_WORK_WRAPPER|||ABSOLUTE: Correct Architectural Solutions Only|||review-work preserved 'architectural solutions' ABSOLUTE"
)

for entry in "${REQUIRED[@]}"; do
  file="${entry%%|||*}"
  rest="${entry#*|||}"
  phrase="${rest%%|||*}"
  desc="${rest#*|||}"
  grep -qF -- "$phrase" "$file"
  check "$desc" "$?" "0"
done

# --- Check: forbidden phrases must NOT be present ---
# Negative assertions: phrases that MUST NOT appear in specific docs.
# These are regression guards for known-bad patterns that were
# deliberately removed and must not creep back in.
echo ""
echo "=== forbidden phrases (negative checks) ==="

FORBIDDEN=(
  # Task #10 fix — the "ABSOLUTE: NEVER Background" block was the
  # self-contradicting directive at lines 78-80 of the pre-rewrite
  # review-work/SKILL.md. It contradicted the background-invocation
  # pattern in the same file. The rewrite removed it; this assertion
  # ensures it stays removed. Mirror the check on tpr-review as a
  # pure regression guard (it never had the block, but a copy-paste
  # from an old template could reintroduce it).
  "$REVIEW_WORK_WRAPPER|||ABSOLUTE: NEVER Background|||review-work Task #10 regression guard (block must be absent)"
  "$TPR_REVIEW_WRAPPER|||ABSOLUTE: NEVER Background|||tpr-review regression guard (never had it; prevent copy-paste regression)"
  # Stale CLI-boundary wording in codex review-work skill (fix for
  # §05.2 Scenario 2 round 2 finding #2). The skill used to document
  # envelope-only mode as enforced by "codex's --output-schema flag
  # enforces schema conformance at the CLI boundary". BUG-08-003
  # removed --output-schema from the codex invocation; codex is now
  # validated only at the parser layer. This negative assertion
  # prevents the stale wording from being re-added by a future edit.
  "$REPO_ROOT/.codex/skills/review-work/SKILL.md|||--output-schema\` flag enforces schema conformance|||codex review-work skill must not claim CLI-boundary schema enforcement"
)

for entry in "${FORBIDDEN[@]}"; do
  file="${entry%%|||*}"
  rest="${entry#*|||}"
  phrase="${rest%%|||*}"
  desc="${rest#*|||}"
  grep -qF -- "$phrase" "$file"
  # For forbidden phrases, grep exit 1 (not found) is success.
  # grep -q can also exit 2 on file-read errors, which should fail.
  rc=$?
  if [[ "$rc" == "1" ]]; then
    echo "  PASS: $desc"
    PASS=$((PASS + 1))
  elif [[ "$rc" == "0" ]]; then
    echo "  FAIL: $desc (phrase present; must be absent)"
    FAIL=$((FAIL + 1))
    FAILED_CHECKS+=("$desc")
  else
    echo "  FAIL: $desc (grep error, rc=$rc)"
    FAIL=$((FAIL + 1))
    FAILED_CHECKS+=("$desc (grep error)")
  fi
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
