#!/usr/bin/env python3
"""Banned clippy-suppression gate (SPEC-74).

Blocks `clippy::too_many_lines` and `clippy::cognitive_complexity` inside any
suppression attribute (`#[allow]`, `#[expect]`, `#![allow]`, `#![expect]`).
No suppression of either lint is valid (per .claude/rules/impl-hygiene.md
SPEC-74); the cure is splitting the function per SPEC-15, never a `reason=`.
The gate reads no reason text, so no reason can argue past it.

Modes:
  (default)        GATE staged additions: exit 1 when the staged diff ADDS
                   either banned token to a .rs file. Wired GATING (no
                   --warn-only) in lefthook pre-commit.
  --full [paths]   Inventory whole-tree suppression attributes carrying either
                   token (default root: compiler/). Exit 1 on any finding
                   unless --warn-only. Promote to the pre-commit gate in place
                   of the staged mode once the backlog reaches zero.
  --json           Machine-readable findings.
  --warn-only      Report without exit 1 (--full backlog inventory only; the
                   staged-additions gate ignores it by design).
  --self-test      Run embedded positive/negative cases.

Returns exit 1 on findings (staged mode always; --full unless --warn-only).
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path

BANNED_TOKEN_RE = re.compile(r"clippy\s*::\s*(too_many_lines|cognitive_complexity)")
SUPPRESSION_ATTR_RE = re.compile(r"#!?\[\s*(?:allow|expect)\b[^\]]*", re.DOTALL)

DIFF_FILE_RE = re.compile(r"^\+\+\+ b/(.*)$")
DIFF_HUNK_RE = re.compile(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@")


@dataclass
class Finding:
    file: str
    line: int
    lint: str

    def render(self) -> str:
        return (
            f"{self.file}:{self.line}: banned suppression of clippy::{self.lint} "
            "(SPEC-74: no reason= is valid; split the function per SPEC-15)"
        )


def scan_text(path: str, text: str) -> list[Finding]:
    """Attribute-aware scan: banned tokens inside suppression attributes."""
    findings: list[Finding] = []
    for attr in SUPPRESSION_ATTR_RE.finditer(text):
        for token in BANNED_TOKEN_RE.finditer(attr.group(0)):
            line = text.count("\n", 0, attr.start() + token.start()) + 1
            findings.append(Finding(path, line, token.group(1)))
    return findings


def scan_full(roots: list[Path]) -> list[Finding]:
    findings: list[Finding] = []
    for root in roots:
        paths = sorted(root.rglob("*.rs")) if root.is_dir() else [root]
        for path in paths:
            try:
                text = path.read_text(encoding="utf-8", errors="replace")
            except OSError:
                continue
            findings.extend(scan_text(str(path), text))
    return findings


def scan_staged_additions() -> list[Finding]:
    """Token scan over ADDED lines of the staged diff for .rs files."""
    diff = subprocess.run(
        ["git", "diff", "--cached", "-U0", "--", "*.rs"],
        capture_output=True,
        text=True,
        check=False,
    ).stdout
    findings: list[Finding] = []
    current_file = ""
    new_line = 0
    for raw in diff.splitlines():
        file_match = DIFF_FILE_RE.match(raw)
        if file_match:
            current_file = file_match.group(1)
            continue
        hunk_match = DIFF_HUNK_RE.match(raw)
        if hunk_match:
            new_line = int(hunk_match.group(1))
            continue
        if raw.startswith("+") and not raw.startswith("+++"):
            token = BANNED_TOKEN_RE.search(raw)
            if token:
                findings.append(Finding(current_file, new_line, token.group(1)))
            new_line += 1
        elif not raw.startswith("-"):
            new_line += 1
    return findings


SELF_TEST_CASES: list[tuple[str, str, int]] = [
    ("multi-line expect", '#[expect(\n    clippy::too_many_lines,\n    reason = "x"\n)]\nfn f() {}', 1),
    ("single-line allow", "#[allow(clippy::cognitive_complexity)]\nfn f() {}", 1),
    ("crate-level allow", "#![allow(clippy::too_many_lines)]", 1),
    ("both in one attr", "#[expect(clippy::too_many_lines, clippy::cognitive_complexity)]", 2),
    ("unrelated expect", '#[expect(clippy::needless_collect, reason = "borrow-break FP")]', 0),
    ("workspace deny mention outside attr", 'const DOC: &str = "too_many_lines is denied";', 0),
    ("no attrs", "fn f() { let x = 1; }", 0),
]


def run_self_test() -> int:
    failures = 0
    for name, text, expected in SELF_TEST_CASES:
        got = len(scan_text("case.rs", text))
        status = "ok" if got == expected else "FAIL"
        if got != expected:
            failures += 1
        print(f"[{status}] {name}: expected {expected}, got {got}")
    print(f"self-test: {len(SELF_TEST_CASES) - failures}/{len(SELF_TEST_CASES)} passed")
    return 1 if failures else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--full", action="store_true")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--warn-only", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("paths", nargs="*", type=Path)
    args = parser.parse_args()

    if args.self_test:
        return run_self_test()

    if args.full:
        roots = args.paths or [Path("compiler")]
        findings = scan_full(roots)
        gating = not args.warn_only
    else:
        findings = scan_staged_additions()
        gating = True

    if args.json:
        print(json.dumps([asdict(f) for f in findings], indent=2))
    else:
        for finding in findings:
            print(finding.render(), file=sys.stderr)
        if findings:
            mode = "staged addition(s)" if not args.full else "occurrence(s)"
            print(
                f"banned-suppression-lint: {len(findings)} banned {mode} of "
                "clippy::too_many_lines/cognitive_complexity (SPEC-74)",
                file=sys.stderr,
            )

    return 1 if findings and gating else 0


if __name__ == "__main__":
    sys.exit(main())
