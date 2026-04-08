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


def is_review_invocation(cmd: str) -> bool:
    """Return True iff cmd contains a top-level codex or gemini invocation.

    Walks cmd with a character-level state machine that understands
    quotes, command substitution, subshells, and compound operators.
    Emits tokens tagged as 'word' or 'op'. After tokenization, walks
    the token list tracking command position and skipping leading
    env-var assignments at each command position.
    """
    tokens = _tokenize(cmd)
    env_assign_pattern = _make_env_assign_checker()
    at_cmd_pos = True
    for kind, value in tokens:
        if kind == "op":
            # Every operator resets command position — the next word-token
            # is the start of a new command.
            at_cmd_pos = True
            continue
        # kind == "word"
        if at_cmd_pos:
            # Skip leading env-var assignments, e.g. VAR=foo or VAR="bar baz"
            # or VAR=$(cmd). The _tokenize step has already collapsed the
            # RHS into a single token regardless of quoting.
            if env_assign_pattern(value):
                continue
            # This is the command position token. Strip any leading prefix
            # (env var exports are handled above; path prefixes like ./foo
            # don't match the literal 'codex' or 'gemini' anyway).
            if value in REVIEW_COMMANDS:
                return True
            # Not a review command — consume this word and everything that
            # follows until the next operator.
            at_cmd_pos = False
    return False


def _make_env_assign_checker():
    """Return a function that checks if a token is an env-var assignment.

    An env-var assignment has the form NAME=... where NAME matches
    [A-Za-z_][A-Za-z0-9_]*. The RHS can be anything; the tokenizer has
    already absorbed quoting/substitution into a single token.
    """
    def is_env_assign(token: str) -> bool:
        if "=" not in token:
            return False
        name, _sep, _rest = token.partition("=")
        if not name:
            return False
        if not (name[0].isalpha() or name[0] == "_"):
            return False
        return all(ch.isalnum() or ch == "_" for ch in name)
    return is_env_assign


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
