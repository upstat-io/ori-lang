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

import sys

REVIEW_COMMANDS = {"codex", "gemini"}

# Wrapper commands that take another command as an argument. When the
# command-position token matches one of these, the classifier skips
# forward through remaining tokens (flags, positional args) looking for
# a codex/gemini token before the next operator. This catches bypasses
# like `env codex exec`, `timeout 60 codex exec`, `nice codex exec`,
# `sudo codex exec`, `ssh host codex exec`, `xargs codex`, etc.
#
# The list is intentionally broad — a false positive (blocking `env ls`
# because it contains `env`) would be caught by the normalization step
# (env alone doesn't match REVIEW_COMMANDS). A false NEGATIVE here means
# a real bypass, which is strictly worse than a false positive.
#
# TPR-04-001-codex iteration 3 verified these wrappers all bypassed the
# previous classifier:
#   env, command, exec, timeout, nice, PATH+=:/tmp (assignment-word form)
WRAPPER_COMMANDS = {
    "env",
    "command",
    "exec",
    "timeout",
    "nice",
    "ionice",
    "taskset",
    "stdbuf",
    "unbuffer",
    "sudo",
    "su",
    "ssh",
    "xargs",
    "nohup",
    "setsid",
    "chrt",
    "eatmydata",
}


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
    tokens = _tokenize(cmd)
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
        if _is_env_assign(value):
            i += 1
            continue
        # Normalize the token: strip quotes and backslash escapes so
        # "codex", 'codex', \codex, co\dex, codex"" all normalize to codex.
        normalized = _normalize_word(value)
        if normalized in REVIEW_COMMANDS:
            return True
        # If the normalized command is a known wrapper, enter wrapper-skip
        # mode: scan forward through the current command's remaining tokens
        # looking for a codex/gemini match. Stop at the next operator
        # (which starts a new command).
        if normalized in WRAPPER_COMMANDS:
            j = i + 1
            while j < len(tokens):
                jkind, jvalue = tokens[j]
                if jkind == "op":
                    break
                # Normalize the scanned token too — wrappers can take
                # quoted/escaped command names just like top-level.
                jnorm = _normalize_word(jvalue)
                if jnorm in REVIEW_COMMANDS:
                    return True
                j += 1
            # Whether or not we found a match inside the wrapper's args,
            # we advance past the wrapper itself. If the wrapper's
            # command-position arg was NOT codex/gemini, we fall through
            # to the regular "not a review command" path.
        # Not a review command and not a wrapper with review args.
        # Consume this word and skip to the next operator.
        at_cmd_pos = False
        i += 1
    return False


def _is_env_assign(token: str) -> bool:
    """Return True if token is an env-var assignment (NAME=value or NAME+=value).

    Accepts both the standard `FOO=bar` form and bash's append-assignment
    `FOO+=bar` form (used for appending to arrays or strings at the start
    of a command). Both are valid at command position and preserve it.
    """
    # Try += first so we correctly identify `FOO+=bar` (otherwise the
    # plain `=` check would see `FOO+` as the name, which isn't a valid
    # identifier, and reject it).
    for sep in ("+=", "="):
        if sep in token:
            name = token.partition(sep)[0]
            if not name:
                return False
            if not (name[0].isalpha() or name[0] == "_"):
                continue
            if all(ch.isalnum() or ch == "_" for ch in name):
                return True
    return False


def _normalize_word(token: str) -> str:
    """Strip quotes and backslash escapes from a shell token.

    Applies bash's quote-removal and backslash-quote rules to get the
    effective value of the token as bash would see it at invocation time.
    Handles:
      - Surrounding "..." double quotes (with \\-escapes for " \\ ` $)
      - Surrounding '...' single quotes (no escapes in POSIX)
      - Interspersed quotes (codex"" → codex)
      - Unquoted backslash escapes (\\codex → codex, co\\dex → codex)

    This is what bash does before executing a command. Without this step,
    the classifier treats `"codex"` as a 7-char literal that doesn't equal
    `codex`, leaving a trivial bypass.
    """
    result = []
    i = 0
    n = len(token)
    while i < n:
        c = token[i]
        if c == '"':
            # Consume double-quoted content with backslash escape handling.
            i += 1
            while i < n and token[i] != '"':
                if token[i] == "\\" and i + 1 < n:
                    nxt = token[i + 1]
                    # In double quotes, backslash only escapes " \ ` $ \n.
                    # Other backslashes are preserved literally.
                    if nxt in '"\\`$\n':
                        result.append(nxt)
                        i += 2
                    else:
                        result.append(token[i])
                        result.append(nxt)
                        i += 2
                else:
                    result.append(token[i])
                    i += 1
            if i < n:
                i += 1  # closing "
            continue
        if c == "'":
            # Consume single-quoted content (no escapes in POSIX).
            i += 1
            while i < n and token[i] != "'":
                result.append(token[i])
                i += 1
            if i < n:
                i += 1  # closing '
            continue
        if c == "\\" and i + 1 < n:
            # Unquoted backslash: the next char is taken literally.
            result.append(token[i + 1])
            i += 2
            continue
        result.append(c)
        i += 1
    return "".join(result)


def _tokenize(cmd: str):
    """Tokenize cmd into a list of (kind, value) pairs.

    kind ∈ {'word', 'op'}. Operators are: | || ; & && ( ) and newline.

    The tokenizer is intentionally minimal — it only needs enough shell
    grammar to answer "at each command position, is the first token
    codex/gemini?". It does NOT expand variables, globs, or tilde; it
    does NOT parse heredocs; it does NOT validate syntax.
    """
    tokens = []
    current: list[str] = []
    i = 0
    n = len(cmd)

    def flush():
        if current:
            tokens.append(("word", "".join(current)))
            current.clear()

    while i < n:
        c = cmd[i]

        # Backslash-newline is line continuation: treat as whitespace
        if c == "\\" and i + 1 < n and cmd[i + 1] == "\n":
            flush()
            i += 2
            continue

        # Backslash-anything inside an unquoted context: consume the next
        # char as a literal (preserves the escape in the token so downstream
        # env-var matching still sees the quote/space). For our purposes
        # this just means "the next char is part of the current word".
        if c == "\\" and i + 1 < n:
            current.append(c)
            current.append(cmd[i + 1])
            i += 2
            continue

        # Whitespace (not newline) ends a word
        if c in (" ", "\t", "\r"):
            flush()
            i += 1
            continue

        # Newline is a command separator (like ;)
        if c == "\n":
            flush()
            tokens.append(("op", "\n"))
            i += 1
            continue

        # Comments: # at the start of a word runs to end of line
        if c == "#" and (not current) and (not tokens or tokens[-1][0] == "op"):
            # Skip to newline
            while i < n and cmd[i] != "\n":
                i += 1
            continue

        # Double-quoted string: consume to matching ", handling backslash escapes
        if c == '"':
            current.append(c)
            i += 1
            while i < n:
                if cmd[i] == "\\" and i + 1 < n:
                    # Escaped char inside double quotes (e.g. \" \\)
                    current.append(cmd[i])
                    current.append(cmd[i + 1])
                    i += 2
                elif cmd[i] == "`":
                    # Backtick substitution inside double quotes
                    i = _consume_backtick(cmd, i, current)
                elif cmd[i] == "$" and i + 1 < n and cmd[i + 1] == "(":
                    # $(...) substitution inside double quotes
                    i = _consume_paren_subst(cmd, i, current)
                elif cmd[i] == '"':
                    current.append(cmd[i])
                    i += 1
                    break
                else:
                    current.append(cmd[i])
                    i += 1
            continue

        # Single-quoted string: consume to matching ', no escapes
        if c == "'":
            current.append(c)
            i += 1
            while i < n and cmd[i] != "'":
                current.append(cmd[i])
                i += 1
            if i < n:
                current.append(cmd[i])
                i += 1
            continue

        # Backtick command substitution
        if c == "`":
            i = _consume_backtick(cmd, i, current)
            continue

        # $(...) command substitution
        if c == "$" and i + 1 < n and cmd[i + 1] == "(":
            i = _consume_paren_subst(cmd, i, current)
            continue

        # Compound operators: | || & && ;
        if c in "|&":
            flush()
            op = c
            i += 1
            if i < n and cmd[i] == c:
                op += cmd[i]
                i += 1
            tokens.append(("op", op))
            continue

        if c == ";":
            flush()
            tokens.append(("op", ";"))
            i += 1
            continue

        # Subshell / grouping
        if c in "()":
            flush()
            tokens.append(("op", c))
            i += 1
            continue

        # Regular character — part of the current word
        current.append(c)
        i += 1

    flush()
    return tokens


def _consume_backtick(cmd: str, start: int, out: list) -> int:
    """Consume a backtick substitution starting at cmd[start] == '`'.

    Returns the index of the character after the closing backtick.
    Appends the raw text (including the backticks) to out.
    """
    out.append(cmd[start])
    i = start + 1
    n = len(cmd)
    while i < n:
        if cmd[i] == "\\" and i + 1 < n:
            out.append(cmd[i])
            out.append(cmd[i + 1])
            i += 2
        elif cmd[i] == "`":
            out.append(cmd[i])
            return i + 1
        else:
            out.append(cmd[i])
            i += 1
    return i


def _consume_paren_subst(cmd: str, start: int, out: list) -> int:
    """Consume a `$(...)` command substitution starting at cmd[start] == '$'.

    Returns the index of the character after the closing paren.
    Handles nested parens, quoted strings, and nested substitutions.
    Appends the raw text to out.
    """
    assert cmd[start] == "$" and cmd[start + 1] == "("
    out.append(cmd[start])      # $
    out.append(cmd[start + 1])  # (
    i = start + 2
    n = len(cmd)
    depth = 1
    while i < n and depth > 0:
        c = cmd[i]
        if c == "\\" and i + 1 < n:
            out.append(c)
            out.append(cmd[i + 1])
            i += 2
            continue
        if c == '"':
            # Nested double-quoted string
            out.append(c)
            i += 1
            while i < n:
                if cmd[i] == "\\" and i + 1 < n:
                    out.append(cmd[i])
                    out.append(cmd[i + 1])
                    i += 2
                elif cmd[i] == '"':
                    out.append(cmd[i])
                    i += 1
                    break
                else:
                    out.append(cmd[i])
                    i += 1
            continue
        if c == "'":
            # Nested single-quoted string (no escapes)
            out.append(c)
            i += 1
            while i < n and cmd[i] != "'":
                out.append(cmd[i])
                i += 1
            if i < n:
                out.append(cmd[i])
                i += 1
            continue
        if c == "`":
            i = _consume_backtick(cmd, i, out)
            continue
        if c == "$" and i + 1 < n and cmd[i + 1] == "(":
            i = _consume_paren_subst(cmd, i, out)
            continue
        if c == "(":
            depth += 1
            out.append(c)
            i += 1
            continue
        if c == ")":
            depth -= 1
            out.append(c)
            i += 1
            if depth == 0:
                return i
            continue
        out.append(c)
        i += 1
    return i


def main() -> int:
    cmd = sys.stdin.read()
    if not cmd:
        return 2
    return 0 if is_review_invocation(cmd) else 1


if __name__ == "__main__":
    sys.exit(main())
