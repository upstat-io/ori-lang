#!/usr/bin/env python3
"""classify-review-command.py — detect whether a bash command string
invokes `codex` or `gemini` at any top-level command position.

This helper exists because shell is not a regular language: the earlier
regex-based REVIEW_CMD_RE in block-banned-commands.sh kept leaking
bypasses (escaped quotes, command substitution `$(...)`, backticks,
heredocs, backslash-newline continuation, literal newlines) despite
multiple fix iterations. Each new regex alternation opened up two more
edge cases. The correct architectural fix is a shell-aware tokenizer.

Usage:
    echo "$COMMAND" | classify-review-command.py
    # exit 0 → codex/gemini invocation detected
    # exit 1 → no codex/gemini invocation
    # exit 2 → bad input (empty stdin)

The classifier walks the command string character-by-character with a
state machine that tracks:
  - double-quoted strings (with backslash escapes)
  - single-quoted strings (POSIX, no escapes)
  - backtick command substitution
  - `$(...)` command substitution (with nesting)
  - subshells `(...)`
  - backslash-newline continuation (treated as whitespace)
  - compound operators: | ; & && || ( )
  - newline as a command separator

At each "command position" (start of string, after |/;/&/&&/||/(/newline),
it:
  1. Skips leading env-var assignments (NAME=value) — the assignment's
     value may be quoted, command-substituted, or backticked, and the
     state machine handles all those while scanning the token.
  2. Checks if the next real token is `codex` or `gemini`.

Known limitations (documented but not blockers for our use case):
  - Process substitution `<(...)` / `>(...)` is treated as an unknown
    token (not a command) — this is a banned construct in our hooks
    anyway.
  - Here-docs `<<EOF ... EOF` are not specially handled; the classifier
    treats everything between `<<EOF` and `EOF` as part of the current
    command's arguments. For the "is this a codex/gemini invocation"
    question this is correct — a heredoc body is INSIDE an argument, not
    a new command position.
  - ANSI C-style quoting `$'...'` is treated as a regular `$` followed by
    a single-quoted string. The classifier still correctly skips the
    quoted content.
  - Tilde expansion, brace expansion, and glob patterns are not expanded
    — but since we only care about the literal token `codex` or `gemini`,
    expansion is irrelevant.
"""

import os
import sys

# Add this file's directory to sys.path so the sibling shell_lex module
# can be imported regardless of caller cwd. The hook is invoked from the
# Bash tool's working directory (the project root in our case), but the
# helper module lives next to this script.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from shell_lex import is_env_assign, normalize_word, tokenize  # noqa: E402

REVIEW_COMMANDS = {"codex", "gemini"}

# Wrapper commands that take another command as an argument. Each entry
# maps the wrapper name to a `positional_skip` count — the number of
# positional arguments (non-flag, non-env-assign) to skip after the
# wrapper before reaching the wrapped command position. Flags (tokens
# starting with `-`) are always skipped. Env-var assignments (NAME=value
# or NAME+=value) are skipped ONLY if `allow_env_prefixes` is true
# (currently only `env` itself).
#
# Examples:
#   env VAR=foo codex exec            → env allows env prefixes; codex is cmd
#   timeout 30 codex exec             → timeout skips 1 positional (duration); codex is cmd
#   ssh user@host codex exec          → ssh skips 1 positional (user@host); codex is cmd
#   nice -n 10 codex exec             → nice skips -n/10 flag+value; codex is cmd
#   sudo -u alice codex exec          → sudo skips -u/alice flag+value; codex is cmd
#   eval codex exec                   → eval runs its arg as a shell cmd; codex is cmd
#   time codex exec                   → time wraps the command directly; codex is cmd
#
# TPR-04-001-codex iteration 4 regression: the previous wrapper-skip
# scanned EVERY remaining token until the next operator and treated any
# bare `codex`/`gemini` as a wrapped command. This matched
# `timeout 30 echo codex` as a review invocation even though codex was
# an argument to echo, not the wrapped command. The new per-wrapper
# positional_skip logic locates the EXACT command position instead of
# scanning.
# Per-wrapper spec keys:
#   positional_skip            — number of POSITIONAL args (non-flag,
#                                non-env-assign) to skip after the wrapper
#                                before the wrapped command position
#                                (e.g. timeout DURATION, ssh user@host).
#   allow_env_prefixes         — if true, NAME=value tokens are skipped
#                                (used for `env`).
#   flags_with_values          — set of flags (short or long) that take
#                                a value in the NEXT token. Both flag and
#                                value get skipped together. Long-form
#                                flags with embedded value (--user=val)
#                                are a single token, skipped via the
#                                regular flag-skip path.
#   shell_string_flags         — set of flags whose value is a shell
#                                command string that should be re-parsed
#                                and recursively classified (e.g.
#                                `bash -c "codex exec"`, `su -c "..."`,
#                                `sh -c "..."`).
#   shell_string_first_positional — if true, the first non-flag, non-env-
#                                assign positional after the wrapper is a
#                                shell command string, recursively
#                                classified (e.g. eval "codex exec",
#                                ssh user@host "codex exec").
#
# Iteration 5 added long-form flags-with-values, profiler/sandbox/shell
# wrappers, and the shell_string_* recursion mechanism.
WRAPPER_SPECS = {
    # Standard wrappers
    "env":        {"positional_skip": 0, "allow_env_prefixes": True},
    "command":    {"positional_skip": 0},
    "exec":       {"positional_skip": 0, "flags_with_values": {"-a"}},
    "timeout":    {"positional_skip": 1, "flags_with_values": {"-k", "--kill-after", "-s", "--signal"}},
    "nice":       {"positional_skip": 0, "flags_with_values": {"-n", "--adjustment"}},
    "ionice":     {"positional_skip": 0, "flags_with_values": {"-c", "-n", "-p", "-P", "-u", "--class", "--classdata", "--pid", "--pgid", "--uid"}},
    "taskset":    {"positional_skip": 1, "flags_with_values": {"-p", "--pid"}},
    "stdbuf":     {"positional_skip": 0, "flags_with_values": {"-i", "-o", "-e", "--input", "--output", "--error"}},
    "unbuffer":   {"positional_skip": 0},
    "sudo":       {"positional_skip": 0, "flags_with_values": {"-u", "-g", "-h", "-U", "-C", "-c", "-D", "-r", "-R", "-t", "-T", "-p", "--user", "--group", "--host", "--other-user", "--close-from", "--login-class", "--chdir", "--role", "--type", "--prompt"}},
    "su":         {"positional_skip": 1, "flags_with_values": {"-s", "-m", "--shell"}, "shell_string_flags": {"-c", "--command"}},
    "ssh":        {"positional_skip": 1, "flags_with_values": {"-b", "-B", "-c", "-D", "-e", "-E", "-F", "-I", "-i", "-J", "-L", "-l", "-m", "-O", "-o", "-p", "-Q", "-R", "-S", "-W", "-w"}, "shell_string_first_positional": True},
    "xargs":      {"positional_skip": 0, "flags_with_values": {"-n", "-I", "-L", "-P", "-d", "-s", "-E", "-a", "--max-args", "--replace", "--max-lines", "--max-procs", "--delimiter", "--max-chars", "--eof", "--arg-file"}},
    "nohup":      {"positional_skip": 0},
    "setsid":     {"positional_skip": 0},
    "chrt":       {"positional_skip": 1},
    "eatmydata":  {"positional_skip": 0},
    "eval":       {"positional_skip": 0, "shell_string_first_positional": True},
    "time":       {"positional_skip": 0},
    # Profiler / sandbox wrappers (iter 5)
    "strace":     {"positional_skip": 0, "flags_with_values": {"-e", "-o", "-O", "-p", "-s", "-S", "-u", "-E", "-I", "-A", "-P", "-x", "-X", "-Z"}},
    "ltrace":     {"positional_skip": 0, "flags_with_values": {"-e", "-o", "-p", "-s", "-u", "-l", "-x", "-A", "-D", "-X"}},
    "gdb":        {"positional_skip": 0, "flags_with_values": {"-x", "-ex", "-iex", "-tty", "-ix", "-eix", "-cd", "-d", "-directory", "-r"}},
    "valgrind":   {"positional_skip": 0, "flags_with_values": {"--tool", "--log-file", "--xml-file", "--suppressions", "--db-command"}},
    "firejail":   {"positional_skip": 0, "flags_with_values": {"--profile", "--name", "--hostname", "--ip", "--dns", "--bind", "--whitelist", "--blacklist", "--read-only", "--rlimit", "--shell"}},
    "bwrap":      {"positional_skip": 0, "flags_with_values": {"--bind", "--ro-bind", "--dev-bind", "--proc", "--dev", "--tmpfs", "--symlink", "--chdir", "--setenv", "--unsetenv"}},
    "unshare":    {"positional_skip": 0, "flags_with_values": {"-S", "-G", "--map-user", "--map-group", "--setuid", "--setgid"}},
    "setpriv":    {"positional_skip": 0, "flags_with_values": {"--ruid", "--euid", "--rgid", "--egid", "--reuid", "--regid", "--groups", "--inh-caps", "--ambient-caps", "--bounding-set", "--securebits", "--pdeathsig"}},
    # Shell wrappers — recursively classify the -c arg as a shell string (iter 5)
    "bash":       {"positional_skip": 0, "shell_string_flags": {"-c", "--command"}, "flags_with_values": {"--rcfile", "--init-file"}},
    "sh":         {"positional_skip": 0, "shell_string_flags": {"-c"}},
    "zsh":        {"positional_skip": 0, "shell_string_flags": {"-c"}},
    "dash":       {"positional_skip": 0, "shell_string_flags": {"-c"}},
    "ksh":        {"positional_skip": 0, "shell_string_flags": {"-c"}},
    "tcsh":       {"positional_skip": 0, "shell_string_flags": {"-c"}},
    "csh":        {"positional_skip": 0, "shell_string_flags": {"-c"}},
    "fish":       {"positional_skip": 0, "shell_string_flags": {"-c", "--command"}},
}
WRAPPER_COMMANDS = set(WRAPPER_SPECS)


def is_review_invocation(cmd: str) -> bool:
    """Return True iff cmd contains a top-level codex or gemini invocation.

    Walks cmd with a character-level state machine that understands
    quotes, command substitution, subshells, and compound operators.
    Emits tokens tagged as 'word' or 'op'. After tokenization, walks
    the token list tracking command position, skipping leading env-var
    assignments, and recursively descending through wrapper commands
    like `env`, `timeout`, `sudo`, etc.

    Each word token is NORMALIZED before comparison: quotes are stripped,
    backslash escapes are removed. This handles bypasses like "codex",
    'codex', \\codex, co\\dex, codex"" that bash executes as `codex`.
    Normalization is independent of the state-machine tokenization
    because shell quoting can appear WITHIN a token (codex"" is one
    token as far as the tokenizer is concerned, but bash strips the
    trailing "" before invoking).
    """
    tokens = tokenize(cmd)
    i = 0
    at_cmd_pos = True
    while i < len(tokens):
        kind, value = tokens[i]
        if kind == "op":
            # Every operator resets command position — the next word-token
            # is the start of a new command.
            at_cmd_pos = True
            i += 1
            continue
        # kind == "word"
        if not at_cmd_pos:
            i += 1
            continue
        # Skip leading env-var assignments. Both NAME=value and NAME+=value
        # (bash's append-assignment syntax) are valid at command position
        # and preserve it.
        if is_env_assign(value):
            i += 1
            continue
        # Normalize the token: strip quotes and backslash escapes so
        # "codex", 'codex', \codex, co\dex, codex"", $'codex', $"codex"
        # all normalize to codex.
        normalized = normalize_word(value)
        if normalized in REVIEW_COMMANDS:
            return True
        # If the normalized command is a known wrapper, handle it via the
        # wrapper-detection helper. The helper handles three sub-modes:
        #   1. shell_string_flags — recursively classify the value of a
        #      flag like `bash -c "codex exec"` (iter 5)
        #   2. shell_string_first_positional — recursively classify the
        #      first positional after the wrapper as a shell string
        #      (e.g. `eval "codex exec"`, `ssh host "codex exec"`) (iter 5)
        #   3. positional command — the wrapped command is at a known
        #      position after the wrapper's flags + positional_skip
        #      args. Jump there and re-check with at_cmd_pos=True so
        #      nested wrappers (e.g. `sudo nice codex`) recurse via the
        #      main loop's normal flow.
        if normalized in WRAPPER_COMMANDS:
            spec = WRAPPER_SPECS[normalized]
            # Sub-mode 1+2: shell_string_* — find a shell-string arg and
            # recursively classify it. If true, this whole command line
            # invokes codex/gemini through the wrapper.
            if "shell_string_flags" in spec or spec.get("shell_string_first_positional"):
                if _check_wrapper_shell_string(tokens, i, spec):
                    return True
                # Fall through to positional check below in case the
                # wrapper takes BOTH a shell-string arg AND a wrapped
                # command (rare, but ssh user@host CMD ARGS works that
                # way when CMD is a separate token, not a single string).
            cmd_idx = _find_wrapper_cmd_position(tokens, i, spec)
            if cmd_idx is not None:
                i = cmd_idx
                at_cmd_pos = True
                continue
            # Wrapper had no wrapped command before the next operator —
            # treat it as a normal non-review command at this position.
        # Not a review command and not a wrapper with review args.
        # Consume this word and skip to the next operator.
        at_cmd_pos = False
        i += 1
    return False


def _check_wrapper_shell_string(tokens, wrapper_idx, spec) -> bool:
    """Check if a wrapper that takes a shell-string argument invokes a
    review command via that string.

    Two sub-modes:
      1. shell_string_flags — look for one of the listed flags (e.g.
         `bash -c VALUE`, `su -c VALUE`); the next token after the flag
         is the shell string. Normalize it (strip outer quotes) and
         recursively call is_review_invocation on the result.
      2. shell_string_first_positional — the first non-flag, non-env-
         assign token after the wrapper's positional_skip is the shell
         string (e.g. `eval VALUE`, `ssh user@host VALUE` where VALUE
         is a single quoted token). Same recursive classification.

    Returns True if either mode finds a review invocation in the inner
    string. Returns False if neither mode applies or neither finds one.
    """
    flags_with_values = spec.get("flags_with_values", set())
    shell_string_flags = spec.get("shell_string_flags", set())
    n = len(tokens)
    j = wrapper_idx + 1

    # Sub-mode 1: walk forward looking for a shell_string_flag
    if shell_string_flags:
        k = j
        while k < n:
            kkind, kvalue = tokens[k]
            if kkind == "op":
                break
            knorm = normalize_word(kvalue)
            if knorm in shell_string_flags:
                # Next token is the shell string
                if k + 1 < n and tokens[k + 1][0] != "op":
                    inner_raw = tokens[k + 1][1]
                    inner_normalized = normalize_word(inner_raw)
                    # Bash treats the -c arg as a shell string to re-parse.
                    # Recursively classify it.
                    if is_review_invocation(inner_normalized):
                        return True
                # Even if the value isn't a review invocation, we've
                # consumed the flag — continue walking in case there's
                # another -c (unusual but possible).
                k += 2
                continue
            # Skip flags with values (their value tokens shouldn't be
            # treated as shell strings)
            if knorm.startswith("-") and knorm in flags_with_values and k + 1 < n and tokens[k + 1][0] != "op":
                k += 2
                continue
            if knorm.startswith("-"):
                k += 1
                continue
            # Non-flag token — keep scanning, since the -c flag may come
            # later in the wrapper's args
            k += 1

    # Sub-mode 2: shell_string_first_positional — find the first non-flag,
    # non-env-assign positional after positional_skip
    if spec.get("shell_string_first_positional"):
        cmd_idx = _find_wrapper_cmd_position(tokens, wrapper_idx, spec)
        if cmd_idx is not None:
            inner_raw = tokens[cmd_idx][1]
            inner_normalized = normalize_word(inner_raw)
            # If the normalized form contains whitespace or shell
            # operators, it's almost certainly a shell string that needs
            # re-parsing. If it's a single bare word, it's the wrapped
            # command directly (handled by the regular wrapper-skip path
            # in the caller, so we still try the recursive classify here
            # — it's a no-op for single-word commands).
            if is_review_invocation(inner_normalized):
                return True

    return False


def _find_wrapper_cmd_position(tokens, wrapper_idx, spec):
    """Locate the wrapped-command position after a wrapper at tokens[wrapper_idx].

    Walks forward from wrapper_idx+1, skipping:
      - Flag tokens (start with `-`)
      - The token AFTER any flag listed in spec['flags_with_values']
        (handles short flags that take a value in the next token, e.g.
        `nice -n 10`, `sudo -u alice`, `xargs -I {}`). Without this,
        the next token after such a flag would be treated as the
        wrapped command, producing false positives like
        `sudo -u codex whoami` matching (codex is the USER, not the cmd).
      - Env-var assignments (only if spec allows env prefixes, i.e. `env`)
      - The first `spec['positional_skip']` non-flag, non-env-assign
        positional tokens (handles wrappers like timeout's DURATION,
        ssh's user@host, su's USERNAME, taskset's MASK, chrt's PRIORITY)

    Returns the index of the wrapped command token, or None if we hit an
    operator or end-of-tokens before finding it.
    """
    positional_skip = spec.get("positional_skip", 0)
    allow_env = spec.get("allow_env_prefixes", False)
    flags_with_values = spec.get("flags_with_values", set())
    positionals_skipped = 0
    j = wrapper_idx + 1
    n = len(tokens)
    while j < n:
        jkind, jvalue = tokens[j]
        if jkind == "op":
            return None
        jnorm = normalize_word(jvalue)
        # Flags always skipped. If the flag is known to take a value in
        # the next token (per spec['flags_with_values']), also skip the
        # next token as its value. The check `jnorm in flags_with_values`
        # matches the short form `-X`; the inline form `-X10` has the
        # value embedded in the flag token itself and doesn't need a
        # next-token skip. Long forms like `--user=foo` embed the value
        # too and don't need a next-token skip.
        if jnorm.startswith("-"):
            if jnorm in flags_with_values and j + 1 < n and tokens[j + 1][0] != "op":
                j += 2  # Skip flag AND its value token
            else:
                j += 1
            continue
        # Env-var assignments (env only)
        if allow_env and is_env_assign(jvalue):
            j += 1
            continue
        # Positional skip (timeout's duration, ssh's user@host, etc.)
        if positionals_skipped < positional_skip:
            positionals_skipped += 1
            j += 1
            continue
        # This is the wrapped command position
        return j
    return None



def main() -> int:
    cmd = sys.stdin.read()
    if not cmd:
        return 2
    return 0 if is_review_invocation(cmd) else 1


if __name__ == "__main__":
    sys.exit(main())
