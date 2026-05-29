"""Runner no-interactive-prompt assertion per §10.2 close-out item 6.

Greps aims-proof/scripts/run-section-10-counterexample.sh + every helper it
shells out to for banned interactive shapes:

  Shell builtins: `read`, `select`, `prompt`
  Python calls: `input()`, `getpass.getpass()`

Passes iff zero hits. the non-interactive runner contract
Mode: autopilot dispatch via /continue-roadmap MUST NOT hang on an interactive
prompt.

The runner shells out to:
  - `python3 - ...` heredocs (inline; scanned via the runner-body grep below)
  - `timeout(1)` (system binary; no interactive prompt surface)
  - `$SOLVER` (z3 / cvc5 binaries; no interactive prompt surface in batch mode)
"""

from __future__ import annotations

import re
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[4]
RUNNER = REPO_ROOT / "aims-proof" / "scripts" / "run-section-10-counterexample.sh"


# ---------------------------------------------------------------------------
# Banned-shape patterns. Each pattern targets a SHELL or PYTHON construct
# that would block on terminal input in autopilot dispatch.
#
# Word-boundary anchors avoid false positives on substrings (e.g.,
# `already` contains `read`).
# ---------------------------------------------------------------------------

BANNED_PATTERNS_SHELL = [
    # `read VAR` — interactive shell read builtin (consumes stdin)
    re.compile(r"(?:^|;|\&\&|\|\||\(|\s)read\s+(?!-)", re.MULTILINE),
    # `select VAR in ...` — interactive menu builtin
    re.compile(r"(?:^|;|\&\&|\|\||\(|\s)select\s+\w+\s+in\s", re.MULTILINE),
    # bash `prompt` (rare; surface only)
    re.compile(r"(?:^|;|\&\&|\|\||\(|\s)prompt\b", re.MULTILINE),
]
BANNED_PATTERNS_PYTHON = [
    # `input(` — Python interactive read
    re.compile(r"\binput\s*\("),
    # `getpass.getpass(` / `from getpass import getpass`
    re.compile(r"\bgetpass(?:\.getpass)?\s*\("),
    re.compile(r"from\s+getpass\s+import"),
]


# Permitted shapes that contain banned-pattern substrings as part of an
# unrelated construct. Each permitted form is a fixed-string fragment;
# matches against any permitted form remove the hit from the banned set.
PERMITTED_FRAGMENTS = [
    # `IFS= read -r` is permitted: the `-r` flag is a read-loop-from-FILE
    # pattern (heredoc / process substitution), not interactive stdin.
    # Already excluded by the regex's `(?!-)` negative lookahead.
    # Heredoc lines containing the literal word `read` in comments etc.
    # are not flagged because the regex requires the word to be at a
    # command-position (start-of-line / after ;/&&/||/( / whitespace).
]


def _scan_file_for_banned(path: Path) -> list[tuple[str, int, str]]:
    """Return list of (banned_pattern, line_no, line_text) hits.

    The runner is .sh — apply shell patterns. Python heredocs inside the
    runner are scanned with the python patterns separately.
    """
    content = path.read_text()
    hits: list[tuple[str, int, str]] = []

    # Walk line by line for accurate line numbers.
    lines = content.splitlines()

    # Determine whether we're inside a python heredoc block. Heuristic:
    # `python3 - "$..." <<'PYEOF'` opens; `^PYEOF$` closes. Within the
    # block, apply Python patterns; outside, apply shell patterns.
    in_heredoc = False
    heredoc_terminator = None

    for line_no, line in enumerate(lines, start=1):
        if not in_heredoc:
            # Detect heredoc open: `<<'TAG'` or `<<TAG`
            heredoc_match = re.search(r"<<\s*'?(\w+)'?\s*$", line)
            if heredoc_match:
                in_heredoc = True
                heredoc_terminator = heredoc_match.group(1)
                # Don't scan the line that opens the heredoc.
                continue
            # Skip shell comment lines (start with `#`, optionally indented).
            stripped = line.lstrip()
            if stripped.startswith("#"):
                continue
            # Skip `IFS=... read -r` (loop-from-file pattern; `-r` already
            # excluded by the regex but covered here for clarity).
            # Shell-pattern scan.
            for pat in BANNED_PATTERNS_SHELL:
                if pat.search(line):
                    hits.append((pat.pattern, line_no, line.strip()))
        else:
            # Inside heredoc.
            if heredoc_terminator and line.strip() == heredoc_terminator:
                in_heredoc = False
                heredoc_terminator = None
                continue
            # Python-pattern scan.
            for pat in BANNED_PATTERNS_PYTHON:
                if pat.search(line):
                    hits.append((pat.pattern, line_no, line.strip()))

    return hits


def test_runner_contains_no_interactive_shell_builtins() -> None:
    assert RUNNER.exists(), f"runner missing at: {RUNNER}"
    hits = _scan_file_for_banned(RUNNER)
    # Filter heredoc-FALSE-positives: the regex's `(?!-)` already excludes
    # `read -r`; remaining hits are real.
    if hits:
        report = "\n".join(f" line {n}: pattern={p!r} → {text}" for p, n, text in hits)
        pytest.fail(
            f"Runner {RUNNER} contains banned interactive shapes:\n{report}\n"
            "Per skill-control-contract.md §Autopilot Mode: autopilot dispatch "
            "MUST NOT hang on interactive prompts."
        )


def test_runner_helpers_no_interactive() -> None:
    """The runner shells out only to `timeout`, `$SOLVER`, and inline
    `python3 -` heredocs. The first two are system binaries with no
    interactive surface in batch mode; the heredocs are scanned in
    test_runner_contains_no_interactive_shell_builtins via the heredoc-
    aware walker.

    This test asserts the runner has no NEW helper scripts that bypass
    the heredoc scan — i.e., no `source <other.sh>` / `bash <other.sh>` /
    `./<other.sh>` invocations referencing helper paths.
    """
    assert RUNNER.exists(), f"runner missing at: {RUNNER}"
    content = RUNNER.read_text()
    # Look for invocations that would shell out to a non-system helper.
    suspicious_patterns = [
        re.compile(r"^\s*source\s+[^/\s]"), # `source ./helper.sh`
        re.compile(r"^\s*\.\s+\./"), # `. ./helper.sh`
        re.compile(r"^\s*bash\s+\./"), # `bash ./helper.sh`
    ]
    hits = []
    for line_no, line in enumerate(content.splitlines(), start=1):
        for pat in suspicious_patterns:
            if pat.search(line):
                hits.append((pat.pattern, line_no, line.strip()))
    if hits:
        report = "\n".join(f" line {n}: pattern={p!r} → {text}" for p, n, text in hits)
        pytest.fail(
            f"Runner {RUNNER} shells out to helper scripts that bypass the no-interactive scan:\n{report}\n"
            "Add the helper to test_runner_no_interactive.py's scan list."
        )
