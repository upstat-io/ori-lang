"""shell_lex.py — character-level shell tokenizer and word-normalizer.

Extracted from classify-review-command.py (split for file-size hygiene
per `.claude/rules/impl-hygiene.md` §File Organization, surfaced by TPR
iteration 4). The classifier's command-position detection logic stays
in classify-review-command.py; this module holds the low-level shell
grammar helpers it relies on.

Public API:

    tokenize(cmd: str) -> list[tuple[str, str]]
        Split a bash command line into (kind, value) pairs where
        kind ∈ {'word', 'op'}. Handles single/double quotes, backtick
        substitution, $(...) substitution (with nesting), compound
        operators (| || ; & && ( )), newline as a command separator,
        and backslash-newline line continuation.

    normalize_word(token: str) -> str
        Apply bash's quote-removal and backslash-quote rules to a
        token and return the effective command name. Handles all
        quoting forms the tokenizer emits, including dollar-prefixed
        quotes ($'...' ANSI-C and $"..." locale-aware), line
        continuation inside quotes, and interspersed quotes
        (codex"" → codex, ""codex → codex).

    is_env_assign(token: str) -> bool
        Return True if the token is a valid bash env-var assignment
        (NAME=value or NAME+=value) that preserves command position.

Intentional non-goals (same as the parent classifier):
  - Variable expansion, glob expansion, tilde expansion
  - Heredoc body parsing
  - Process substitution <(...) and >(...)
  - Syntax validation

The tokenizer is deliberately permissive: malformed shell input is
tokenized best-effort rather than raising — downstream classifiers
can still make a conservative decision.
"""


def tokenize(cmd: str):
    """Tokenize cmd into a list of (kind, value) pairs.

    kind ∈ {'word', 'op'}. Operators are: | || ; & && ( ) and newline.
    Comments (#...) at command-position are skipped to end-of-line.
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

        # Backslash-newline is line continuation: bash erases both chars
        # and merges the surrounding content into a single word. Do NOT
        # flush — the surrounding word characters should concatenate.
        # Without this, `co\<newline>dex` would tokenize as `co` + `dex`
        # instead of `codex` and bypass the command-position check.
        if c == "\\" and i + 1 < n and cmd[i + 1] == "\n":
            i += 2
            continue

        # Backslash-anything inside an unquoted context: consume the next
        # char as a literal (preserves the escape in the token so
        # downstream normalize_word can see it).
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
            while i < n and cmd[i] != "\n":
                i += 1
            continue

        # Double-quoted string: consume to matching ", handling backslash escapes
        if c == '"':
            current.append(c)
            i += 1
            while i < n:
                if cmd[i] == "\\" and i + 1 < n and cmd[i + 1] == "\n":
                    # Line continuation inside double quotes: erase both
                    # chars (don't append). Bash merges surrounding content
                    # into the same quoted string.
                    i += 2
                elif cmd[i] == "\\" and i + 1 < n:
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


def normalize_word(token: str) -> str:
    """Strip quotes and backslash escapes from a shell token.

    Applies bash's quote-removal and backslash-quote rules to get the
    effective value of the token as bash would see it at invocation time.
    Handles:
      - Surrounding "..." double quotes (with \\-escapes for " \\ ` $)
      - Surrounding '...' single quotes (no escapes in POSIX)
      - Interspersed quotes (codex"" → codex)
      - Unquoted backslash escapes (\\codex → codex, co\\dex → codex)
      - Line continuation (\\<newline>) inside words and quotes
      - Dollar-prefixed quotes ($'...' ANSI-C, $"..." locale-aware)
    """
    result = []
    i = 0
    n = len(token)
    while i < n:
        c = token[i]
        # Dollar-prefixed quotes: $'...' (ANSI-C quoting) and $"..."
        # (locale-aware string). Both are bash-specific forms where the
        # $ is a sigil that affects how the quoted content is interpreted,
        # but for our purposes (extracting the literal command name) we
        # can strip the $ and let the regular quote handling take over.
        # $'codex' → codex, $"codex" → codex.
        if c == "$" and i + 1 < n and token[i + 1] in "\"'":
            i += 1
            continue
        if c == '"':
            i += 1
            while i < n and token[i] != '"':
                if token[i] == "\\" and i + 1 < n:
                    nxt = token[i + 1]
                    if nxt == "\n":
                        # Line continuation inside double quotes: bash
                        # erases both the backslash AND the newline,
                        # merging the surrounding content. Do the same
                        # here so `"co\<newline>dex"` → `codex`.
                        i += 2
                    elif nxt in '"\\`$':
                        # In double quotes, backslash escapes " \ ` $.
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
            i += 1
            while i < n and token[i] != "'":
                result.append(token[i])
                i += 1
            if i < n:
                i += 1  # closing '
            continue
        if c == "\\" and i + 1 < n:
            # Unquoted backslash: the next char is taken literally,
            # EXCEPT that backslash-newline is line continuation that
            # bash erases entirely (merges surrounding chars into one
            # word).
            if token[i + 1] == "\n":
                i += 2
            else:
                result.append(token[i + 1])
                i += 2
            continue
        result.append(c)
        i += 1
    return "".join(result)


def is_env_assign(token: str) -> bool:
    """Return True if token is an env-var assignment.

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
