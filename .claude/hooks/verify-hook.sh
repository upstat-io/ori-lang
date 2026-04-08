#!/usr/bin/env bash
# verify-hook.sh — regression test suite for block-banned-commands.sh
#
# Runs the full timeout-gate matrix against the hook and reports
# PASS/FAIL per scenario. Exits non-zero on any failure. Designed to
# run in <1 second so it can be invoked freely after any hook edit.
#
# Usage:
#   bash .claude/hooks/verify-hook.sh
#   bash .claude/hooks/verify-hook.sh --quiet   # only report failures
#
# Why this exists:
#   The hook gates timeout windows on codex/gemini commands. Any change
#   to its threshold constants, conditional structure, or deny messages
#   risks breaking review-time guarantees. Reconstructing the 9-test
#   matrix from the section plan every time someone touches the hook
#   was a 10-15 minute manual chore; this script makes it a single
#   command. Surfaced by the dual-tpr-gemini section-01.4 retrospective.
#
# Test matrix dimensions:
#   command in {codex, gemini}  ×  timeout in {1min, 10min, 25min, 60min}
#   plus a control (echo, 1min) proving the gate is scoped to codex/gemini.
#
# Keep this matrix in sync with the section file's documented matrix —
# they describe the same contract.

set -uo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HOOK="$SCRIPT_DIR/block-banned-commands.sh"
QUIET=0

if [[ "${1:-}" == "--quiet" ]]; then
  QUIET=1
fi

if [[ ! -x "$HOOK" ]]; then
  printf 'error: hook not found or not executable: %s\n' "$HOOK" >&2
  exit 1
fi

# Color codes (no dependency on tput)
GREEN='\033[0;32m'
RED='\033[0;31m'
DIM='\033[0;2m'
RESET='\033[0m'
if [[ -n "${NO_COLOR:-}" ]] || [[ ! -t 1 ]]; then
  GREEN='' RED='' DIM='' RESET=''
fi

PASS=0
FAIL=0
FAILED_TESTS=()

# run_test NAME COMMAND TIMEOUT EXPECTED [DENY_SUBSTR]
#   EXPECTED: "deny" or "allow"
#   DENY_SUBSTR: optional substring that the deny message must contain
#                (only checked when EXPECTED=deny)
#
# The JSON input is encoded via python3 (a hook dependency) so that
# commands containing double quotes, backslashes, newlines, or other
# JSON-sensitive characters round-trip correctly. A printf-based encoder
# would corrupt any such command and silently produce the wrong input.
run_test() {
  local name="$1" cmd="$2" timeout="$3" expected="$4" deny_substr="${5:-}"

  local input
  input=$(CMD="$cmd" TIMEOUT="$timeout" python3 -c '
import json, os, sys
sys.stdout.write(json.dumps({
    "tool_name": "Bash",
    "tool_input": {
        "command": os.environ["CMD"],
        "timeout": int(os.environ["TIMEOUT"]),
    },
}))
')
  local output
  output=$(printf '%s' "$input" | bash "$HOOK" 2>&1)

  local actual="allow"
  if [[ -n "$output" ]] && printf '%s' "$output" | grep -q '"permissionDecision": *"deny"'; then
    actual="deny"
  fi

  local ok=0
  if [[ "$actual" == "$expected" ]]; then
    ok=1
    if [[ "$expected" == "deny" && -n "$deny_substr" ]]; then
      if ! printf '%s' "$output" | grep -q -- "$deny_substr"; then
        ok=0
      fi
    fi
  fi

  if (( ok )); then
    PASS=$((PASS + 1))
    if (( ! QUIET )); then
      printf '  %b[PASS]%b %s\n' "$GREEN" "$RESET" "$name"
    fi
  else
    FAIL=$((FAIL + 1))
    FAILED_TESTS+=("$name")
    printf '  %b[FAIL]%b %s\n' "$RED" "$RESET" "$name"
    printf '    %bcmd:%b      %s\n' "$DIM" "$RESET" "$cmd"
    printf '    %btimeout:%b  %s ms\n' "$DIM" "$RESET" "$timeout"
    printf '    %bexpected:%b %s' "$DIM" "$RESET" "$expected"
    if [[ -n "$deny_substr" ]]; then
      printf ' (containing "%s")' "$deny_substr"
    fi
    printf '\n'
    printf '    %bgot:%b      %s\n' "$DIM" "$RESET" "$actual"
    if [[ -n "$output" ]]; then
      printf '    %boutput:%b   %s\n' "$DIM" "$RESET" "$output"
    fi
  fi
}

if (( ! QUIET )); then
  printf '=== block-banned-commands.sh — timeout gate matrix ===\n'
fi

# Codex × {1min, 10min, 25min, 60min}
run_test "codex + 1 min   → DENY (under 20-min floor)"        "codex exec test" 60000   deny "20-35 minutes"
run_test "codex + 10 min  → DENY (negative pin: floor raised)" "codex exec test" 600000  deny "20-35 minutes"
run_test "codex + 25 min  → ALLOW (in sweet spot)"             "codex exec test" 1500000 allow
run_test "codex + 60 min  → DENY (over 35-min ceiling)"        "codex exec test" 3600000 deny "35-minute ceiling"

# Gemini × {1min, 10min, 25min, 60min}
run_test "gemini + 1 min  → DENY (under 20-min floor)"         "gemini -p test"  60000   deny "20-35 minutes"
run_test "gemini + 10 min → DENY (negative pin: floor raised)" "gemini -p test"  600000  deny "20-35 minutes"
run_test "gemini + 25 min → ALLOW (in sweet spot)"             "gemini -p test"  1500000 allow
run_test "gemini + 60 min → DENY (over 35-min ceiling)"        "gemini -p test"  3600000 deny "35-minute ceiling"

# Control: gate must NOT apply to non-codex/gemini commands
run_test "control: echo + 1 min → ALLOW (gate doesn't apply)"  "echo hello"      60000   allow

# ── False-positive suppression (BUG-08-001) ──────────────────────────
# The hook originally used a naive substring match — any command whose
# arguments, paths, or message body contained the literal strings
# "codex" or "gemini" was denied. The fix narrows the match to genuine
# invocations at shell-command position. These tests prove the false
# positives are gone. All use a short (60000 ms) timeout; ALL must
# return ALLOW because none of them actually invoke codex or gemini.

run_test "git commit with 'gemini' in message → ALLOW (message body, not invocation)" \
  'git commit -m "fix dual-tpr-gemini work"' 60000 allow
run_test "git commit with 'codex' in message → ALLOW (message body, not invocation)" \
  'git commit -m "refactor codex parser"' 60000 allow
run_test "grep codex as arg → ALLOW (codex is grep's argument, not a command)" \
  "grep codex .claude/" 60000 allow
run_test "grep gemini as arg → ALLOW (gemini is grep's argument, not a command)" \
  "grep gemini .claude/" 60000 allow
run_test "ls .codex/skills/ → ALLOW (.codex is a path component preceded by .)" \
  "ls .codex/skills/" 60000 allow
run_test "ls .gemini/skills/ → ALLOW (.gemini is a path component preceded by .)" \
  "ls .gemini/skills/" 60000 allow
run_test "cat dual-tpr-gemini section file → ALLOW (gemini in a path, preceded by -)" \
  "cat plans/dual-tpr-gemini/section-04-tpr-review.md" 60000 allow
run_test "bash dual-invoke.sh script → ALLOW (top-level command is bash, not codex/gemini)" \
  "bash .claude/skills/dual-tpr/scripts/dual-invoke.sh arg1" 60000 allow
run_test "./my-gemini-wrapper.sh → ALLOW (gemini inside a script name, preceded by -)" \
  "./my-gemini-wrapper.sh test" 60000 allow
run_test "echo 'fix codex and gemini' → ALLOW (both substrings in a quoted arg)" \
  "echo 'fix codex and gemini bugs'" 60000 allow

# ── Bypass closure — compound invocations (BUG-08-001) ───────────────
# A word-boundary-only fix would regress the gate on env-var prefixes,
# pipelines, sequences, &&, and subshells. These tests pin the gate
# shut: each command is a GENUINE codex/gemini invocation reached via
# a compound-command form, and all must DENY at 60000 ms.

run_test "ORI_LOG=debug codex exec → DENY (env-var prefix bypass closed)" \
  "ORI_LOG=debug codex exec test" 60000 deny "20-35 minutes"
run_test "multiple env-var prefixes + codex → DENY (multi-env bypass closed)" \
  "TARGET=x86_64 ORI_LOG=debug codex exec test" 60000 deny "20-35 minutes"
run_test "pipeline into codex exec → DENY (| bypass closed)" \
  "cat prompt.md | codex exec test" 60000 deny "20-35 minutes"
run_test "&& codex exec → DENY (logical AND bypass closed)" \
  "some-prep && codex exec test" 60000 deny "20-35 minutes"
run_test "; gemini -p → DENY (sequence bypass closed)" \
  "cleanup; gemini -p test" 60000 deny "20-35 minutes"
run_test "(codex exec) subshell → DENY (subshell bypass closed)" \
  "(codex exec test)" 60000 deny "20-35 minutes"
run_test "gemini --approval-mode → DENY (flag-form invocation still denied)" \
  "gemini --approval-mode yolo" 60000 deny "20-35 minutes"
run_test "gemini --output-format stream-json -p → DENY (flag-form invocation still denied)" \
  "gemini --output-format stream-json -p test" 60000 deny "20-35 minutes"

# ── Bypass closure — quoted env-var values (TPR-04-001-gemini) ───────
# Codex review (gemini reviewer) found that the prior env-var prefix
# pattern `[^[:space:]]*` stopped at the first space in a quoted value,
# letting `VAR="val with space" codex ...` bypass the timeout gate. The
# fix accepts quoted env-var values (double and single) as well as
# unquoted ones. These tests pin the closure: any form of env-var
# prefixing must still reach the codex/gemini command on the right.

run_test 'VAR="val with space" codex exec → DENY (double-quoted env-var value bypass closed)' \
  'VAR="val with space" codex exec test' 60000 deny "20-35 minutes"
run_test "VAR='single quoted' codex exec → DENY (single-quoted env-var value bypass closed)" \
  "VAR='single quoted' codex exec test" 60000 deny "20-35 minutes"
run_test 'A=1 B="two words" C=3 codex exec → DENY (mixed quoted and unquoted env-var values)' \
  'A=1 B="two words" C=3 codex exec test' 60000 deny "20-35 minutes"
run_test 'FOO="bar baz" gemini -p → DENY (quoted env-var + gemini)' \
  'FOO="bar baz" gemini -p test' 60000 deny "20-35 minutes"

# ── Bypass closure — shell-aware bypass forms (TPR-04-001-codex/gemini) ──
# Seven distinct bypass forms verified against the previous regex-based
# REVIEW_CMD_RE. The shell-tokenization classifier (classify-review-
# command.py) handles all of them correctly by walking the command string
# with a character-level state machine that tracks quote state, subshell
# depth, command substitution, and compound operators. Each test below
# pins one of the seven verified bypass forms so reverting the classifier
# breaks the matrix.

run_test 'VAR="a\" b" codex exec → DENY (escaped double quote inside env-var value)' \
  'VAR="a\" b" codex exec test' 60000 deny "20-35 minutes"
run_test 'VAR=$(printf "a b") codex exec → DENY ($(...) command substitution)' \
  'VAR=$(printf "a b") codex exec test' 60000 deny "20-35 minutes"
run_test 'VAR=`printf "a b"` codex exec → DENY (backtick command substitution)' \
  'VAR=`printf "a b"` codex exec test' 60000 deny "20-35 minutes"
run_test 'VAR=$(cat <<INNER ... INNER) codex exec → DENY (heredoc inside subshell)' \
  $'VAR=$(cat <<"INNER"\nhello\nINNER\n) codex exec test' 60000 deny "20-35 minutes"
run_test $'VAR=val \\\\\\ncodex exec → DENY (backslash-newline line continuation)' \
  $'VAR=val \\\ncodex exec test' 60000 deny "20-35 minutes"
run_test $'some_cmd\\ncodex exec → DENY (literal newline as command separator)' \
  $'some_cmd\ncodex exec test' 60000 deny "20-35 minutes"
run_test 'VAR=foo<TAB>codex exec → DENY (tab as word separator)' \
  $'VAR=foo\tcodex exec test' 60000 deny "20-35 minutes"

# ── Bypass closure — wrapper commands + token normalization (TPR-04-001-codex/gemini iteration 3) ──
# Iteration 3 of the dual-source review surfaced 13 additional bypass forms
# that the initial shell tokenizer missed: quoted/escaped command names,
# wrapper commands (env, timeout, sudo, etc.), and bash's += assignment-word
# syntax. The classifier now normalizes command-position tokens (strips
# quotes and backslash escapes) and recursively descends through known
# wrapper commands. Each test below pins one of the verified bypass forms.

run_test '"codex" exec → DENY (double-quoted command name)' \
  '"codex" exec test' 60000 deny "20-35 minutes"
run_test "'codex' exec → DENY (single-quoted command name)" \
  "'codex' exec test" 60000 deny "20-35 minutes"
run_test 'codex"" exec → DENY (trailing empty quotes stripped)' \
  'codex"" exec test' 60000 deny "20-35 minutes"
run_test '\codex exec → DENY (backslash-escaped command name)' \
  '\codex exec test' 60000 deny "20-35 minutes"
run_test 'co\dex exec → DENY (backslash escape inside command name)' \
  'co\dex exec test' 60000 deny "20-35 minutes"
run_test 'env FOO=bar codex exec → DENY (env wrapper)' \
  'env FOO=bar codex exec test' 60000 deny "20-35 minutes"
run_test 'command codex exec → DENY (command builtin wrapper)' \
  'command codex exec test' 60000 deny "20-35 minutes"
run_test 'exec codex exec → DENY (exec builtin wrapper)' \
  'exec codex exec test' 60000 deny "20-35 minutes"
run_test 'timeout 30 codex exec → DENY (timeout wrapper)' \
  'timeout 30 codex exec test' 60000 deny "20-35 minutes"
run_test 'nice -n 10 codex exec → DENY (nice wrapper with flag)' \
  'nice -n 10 codex exec test' 60000 deny "20-35 minutes"
run_test 'sudo codex exec → DENY (sudo wrapper)' \
  'sudo codex exec test' 60000 deny "20-35 minutes"
run_test 'ssh host codex exec → DENY (ssh wrapper)' \
  'ssh host codex exec' 60000 deny "20-35 minutes"
run_test 'echo args | xargs codex → DENY (xargs wrapper after pipe)' \
  'echo args | xargs codex exec' 60000 deny "20-35 minutes"
run_test 'PATH+=:/tmp codex exec → DENY (+= assignment-word form)' \
  'PATH+=:/tmp codex exec test' 60000 deny "20-35 minutes"

# Wrapper-with-non-review-command must NOT match (no false positives on
# wrapper commands that don't actually invoke codex/gemini).
run_test 'env ls (wrapper with non-review cmd) → ALLOW' \
  'env ls -la' 60000 allow
run_test 'timeout 30 sleep 10 (wrapper with non-review cmd) → ALLOW' \
  'timeout 30 sleep 10' 60000 allow
run_test 'env FOO=bar ls → ALLOW (env setting for non-review cmd)' \
  'env FOO=bar ls' 60000 allow

# ── Bypass closure — iteration-4 fixes (TPR-04-001-codex/gemini iter 4) ──
# Iteration 4 surfaced wrapper-skip false-positives (codex inside arg
# position of a wrapper's TARGET command), missing wrappers (eval, time),
# dollar-prefixed quotes ($'codex' / $"codex"), and line continuation
# inside words (co\<newline>dex). The classifier now uses per-wrapper
# positional skip counts + flags-with-values metadata, dollar-quote
# stripping in normalization, and proper line-continuation handling
# inside both unquoted and double-quoted contexts. Each test pins a
# verified bypass form.

# False positives that MUST allow (codex appears in arg, not as command)
run_test 'timeout 30 echo codex → ALLOW (codex is echo arg, not wrapped cmd)' \
  'timeout 30 echo codex' 60000 allow
run_test 'env printf codex → ALLOW (codex is printf arg, not wrapped cmd)' \
  'env printf codex' 60000 allow
run_test 'ssh codex uptime → ALLOW (codex is ssh user@host, not wrapped cmd)' \
  'ssh codex uptime' 60000 allow
run_test 'sudo -u codex whoami → ALLOW (codex is sudo USER, not wrapped cmd)' \
  'sudo -u codex whoami' 60000 allow
run_test 'nice -n 10 sleep 5 → ALLOW (no codex/gemini at all)' \
  'nice -n 10 sleep 5' 60000 allow

# Real bypasses that MUST deny
run_test '$"codex" exec → DENY (locale-aware dollar-quote)' \
  '$"codex" exec test' 60000 deny "20-35 minutes"
run_test "\$'codex' exec → DENY (ANSI-C dollar-quote)" \
  "\$'codex' exec test" 60000 deny "20-35 minutes"
run_test 'co\<newline>dex exec → DENY (line continuation inside word)' \
  $'co\\\ndex exec test' 60000 deny "20-35 minutes"
run_test '"co\<newline>dex" exec → DENY (line continuation inside double quotes)' \
  $'"co\\\ndex" exec test' 60000 deny "20-35 minutes"
run_test 'eval codex exec → DENY (eval wrapper)' \
  'eval codex exec test' 60000 deny "20-35 minutes"
run_test 'time codex exec → DENY (time wrapper)' \
  'time codex exec test' 60000 deny "20-35 minutes"

# Wrapper with flag+value still matches when codex is the wrapped command
run_test 'sudo -u alice codex → DENY (sudo flag value, codex IS wrapped cmd)' \
  'sudo -u alice codex exec' 60000 deny "20-35 minutes"
run_test 'nice -n 10 codex → DENY (nice flag value, codex IS wrapped cmd)' \
  'nice -n 10 codex exec' 60000 deny "20-35 minutes"
run_test 'xargs -n 1 codex → DENY (xargs flag value, codex IS wrapped cmd)' \
  'xargs -n 1 codex' 60000 deny "20-35 minutes"
run_test 'ssh user@host codex → DENY (ssh user@host as positional, codex IS wrapped cmd)' \
  'ssh user@host codex exec' 60000 deny "20-35 minutes"

# ── Bypass closure — iter 5: long-form flags + sandbox + shell wrappers + shell-string args ──
# Iteration 5 surfaced: long-form wrapper flag-value pairs (--user value),
# missing profiler/sandbox wrappers (strace, valgrind, gdb, firejail, bwrap),
# missing shell wrappers (bash, sh, zsh, su -c) with -c shell-string args,
# and quoted wrapper args (eval "codex exec"). The classifier now handles
# all four families via WRAPPER_SPECS extensions and a new
# `_check_wrapper_shell_string` helper that recursively classifies the
# shell-string arg of wrappers like bash -c, eval, ssh host CMD.

# Long-form flag-value bypasses
run_test 'sudo --user alice codex → DENY (long-form sudo flag with separate value)' \
  'sudo --user alice codex exec' 60000 deny "20-35 minutes"
run_test 'sudo --user=alice codex → DENY (long-form sudo flag with embedded value)' \
  'sudo --user=alice codex exec' 60000 deny "20-35 minutes"
run_test 'timeout --signal TERM 30 codex → DENY (long-form timeout flag)' \
  'timeout --signal TERM 30 codex exec' 60000 deny "20-35 minutes"
run_test 'timeout -k 1 30 codex → DENY (timeout -k flag-value before duration)' \
  'timeout -k 1 30 codex exec' 60000 deny "20-35 minutes"
run_test 'xargs --max-args 1 codex → DENY (long-form xargs flag)' \
  'xargs --max-args 1 codex' 60000 deny "20-35 minutes"
run_test 'nice --adjustment 10 codex → DENY (long-form nice flag)' \
  'nice --adjustment 10 codex exec' 60000 deny "20-35 minutes"

# Profiler / sandbox wrappers
run_test 'strace codex exec → DENY (strace wrapper)' \
  'strace codex exec test' 60000 deny "20-35 minutes"
run_test 'valgrind codex exec → DENY (valgrind wrapper)' \
  'valgrind codex exec test' 60000 deny "20-35 minutes"
run_test 'firejail codex exec → DENY (firejail sandbox wrapper)' \
  'firejail codex exec test' 60000 deny "20-35 minutes"
run_test 'gdb --args codex → DENY (gdb wrapper)' \
  'gdb --args codex exec test' 60000 deny "20-35 minutes"
run_test 'ltrace codex → DENY (ltrace wrapper)' \
  'ltrace codex exec test' 60000 deny "20-35 minutes"
run_test 'bwrap codex → DENY (bwrap sandbox wrapper)' \
  'bwrap codex exec test' 60000 deny "20-35 minutes"

# Shell wrappers with -c (recursive shell-string classification)
run_test 'bash -c "codex exec" → DENY (bash -c shell string)' \
  'bash -c "codex exec"' 60000 deny "20-35 minutes"
run_test 'sh -c "codex exec" → DENY (sh -c shell string)' \
  'sh -c "codex exec"' 60000 deny "20-35 minutes"
run_test 'zsh -c codex → DENY (zsh -c shell string)' \
  'zsh -c "codex exec"' 60000 deny "20-35 minutes"
run_test 'su -c codex root → DENY (su -c shell string with username)' \
  "su -c 'codex exec' root" 60000 deny "20-35 minutes"

# Quoted wrapper args (recursive classification)
run_test 'eval "codex exec" → DENY (eval shell-string positional)' \
  'eval "codex exec"' 60000 deny "20-35 minutes"
run_test 'ssh host "codex exec" → DENY (ssh quoted command string)' \
  'ssh host "codex exec"' 60000 deny "20-35 minutes"

# Wrapper false positives must still allow
run_test 'bash my_script.sh → ALLOW (bash with non-c arg, not codex)' \
  'bash my_script.sh' 60000 allow
run_test 'strace ls -la → ALLOW (strace with non-codex command)' \
  'strace ls -la' 60000 allow
run_test 'eval "ls -la" → ALLOW (eval with non-codex shell string)' \
  'eval "ls -la"' 60000 allow
run_test 'sh -c "ls" → ALLOW (sh with non-codex shell string)' \
  'sh -c "ls"' 60000 allow

# ── Bypass closure — iter 6: clustered short flags + su username false positive + embedded values ──
# Iteration 6 surfaced clustered short-flag forms (bash -lc, -ic, -xc),
# embedded-value forms (-cVALUE no space), and a su -c false positive
# regression where the username position was being treated as the
# wrapped command. Each test pins one form.

# HIGH: clustered shell short-options (bash -lc 'codex' is bash -l -c 'codex')
run_test 'bash -lc codex → DENY (clustered -lc form)' \
  "bash -lc 'codex exec'" 60000 deny "20-35 minutes"
run_test 'bash -ic codex → DENY (clustered -ic form)' \
  "bash -ic 'codex exec'" 60000 deny "20-35 minutes"
run_test 'zsh -lc codex → DENY (clustered zsh form)' \
  "zsh -lc 'codex exec'" 60000 deny "20-35 minutes"
run_test 'env FOO=1 bash -lc codex → DENY (env wrap with cluster)' \
  "env FOO=1 bash -lc 'codex exec'" 60000 deny "20-35 minutes"

# REGRESSION: su -c username false positive
run_test "su -c 'ls' codex → ALLOW (codex is the USERNAME, not command)" \
  "su -c 'ls -la' codex" 60000 allow
run_test "su --command 'ls' codex → ALLOW (long-form -c, codex is USER)" \
  "su --command 'ls -la' codex" 60000 allow
run_test "su -s /bin/sh -c 'ls' codex → ALLOW (su -s + -c, codex is USER)" \
  "su -s /bin/sh -c 'ls -la' codex" 60000 allow

# Embedded value forms (gemini iter 6)
run_test 'bash -c"codex" → DENY (embedded value, no space after -c)' \
  'bash -c"codex exec"' 60000 deny "20-35 minutes"
run_test 'sh -c"codex" → DENY (embedded value)' \
  'sh -c"codex exec"' 60000 deny "20-35 minutes"
run_test 'bash -lc"codex" → DENY (clustered + embedded value)' \
  'bash -lc"codex exec"' 60000 deny "20-35 minutes"

if (( ! QUIET )); then
  printf '\n'
fi

if (( FAIL == 0 )); then
  printf '%b[OK]%b %d/%d tests passed\n' "$GREEN" "$RESET" "$PASS" "$((PASS + FAIL))"
  exit 0
fi

printf '%b[FAIL]%b %d/%d tests passed (%d failed)\n' "$RED" "$RESET" "$PASS" "$((PASS + FAIL))" "$FAIL"
printf '\nFailed tests:\n'
for t in "${FAILED_TESTS[@]}"; do
  printf '  - %s\n' "$t"
done
exit 1
