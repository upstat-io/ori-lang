#!/usr/bin/env python3
"""Test-naming hygiene linter — ephemeral identifiers only.

Ephemeral identifiers (bug IDs, plan names, section/phase tokens, TPR
codes, dates, authors, commit hashes) belong in doc comments, never in
function names (target shape: `<subject>_<scenario>_<expected>`).

Weak-descriptor detection (`_basic`, `_default`, ...) is NOT this lint's
job: those need per-name domain-word adjudication (`_ok` as Result-Ok
subject, `_default` as Default-trait subject) and are surfaced
reviewer-time by the hygiene-lint `test-weak` check.

Usage:
  scripts/test-naming-lint.py [paths...]   # paths default to compiler workspace
  scripts/test-naming-lint.py --json       # machine-readable output
  scripts/test-naming-lint.py --exit-zero  # report findings without exit 1

Returns exit 1 when any violation is found (unless --exit-zero).
"""
from __future__ import annotations

import argparse
import json
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

BANNED_PATTERNS: list[tuple[str, re.Pattern[str]]] = [
    ("bug-id", re.compile(r"(?:^|_)bug[_-]?\d+", re.IGNORECASE)),
    ("bug-id", re.compile(r"BUG[_-]?\d{2,4}[_-]?\d{2,4}", re.IGNORECASE)),
    ("issue-id", re.compile(r"(?:^|_)issue_\d+", re.IGNORECASE)),
    ("issue-id", re.compile(r"(?:^|_)fix_\d+", re.IGNORECASE)),
    ("plan-section", re.compile(r"(?:^|_)section_\d+(?:_\d+)?", re.IGNORECASE)),
    ("plan-phase", re.compile(r"(?:^|_)phase_[a-z0-9](?:_|$)", re.IGNORECASE)),
    ("plan-section", re.compile(r"_\d+_\d+_(?:phase|step)_[a-z0-9]+", re.IGNORECASE)),
    ("plan-section", re.compile(r"_§\d+", re.IGNORECASE)),
    ("tpr-code", re.compile(r"(?:^|_)TPR[_-]?\d+", re.IGNORECASE)),
    ("tpr-code", re.compile(r"(?:^|_)CROSS[_-]?\d+", re.IGNORECASE)),
    ("plan-name", re.compile(r"(?:^|_)roadmap_\d+", re.IGNORECASE)),
    ("date", re.compile(r"_\d{4}_\d{2}_\d{2}")),
    ("date", re.compile(r"(?:^|_)q\d_regression", re.IGNORECASE)),
    ("commit-hash", re.compile(r"(?:^|_)[0-9a-f]{7,40}(?:_|$)")),
    ("author-initials", re.compile(r"_(?:eric|alice|bob|es|ab)_(?:fix|repro|patch)", re.IGNORECASE)),
]

RUST_TEST_RE = re.compile(r"^\s*fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(")
RUST_ATTR_TEST = re.compile(r"#\[test\]|#\[tokio::test\]|#\[rstest\]")

ORI_TEST_RE = re.compile(r"^\s*@([a-zA-Z_][a-zA-Z0-9_]*)\s+tests\s")


@dataclass
class Finding:
    file: str
    line: int
    name: str
    category: str
    suggestion: str

    def to_dict(self) -> dict[str, str | int]:
        return {
            "file": self.file,
            "line": self.line,
            "name": self.name,
            "category": self.category,
            "suggestion": self.suggestion,
        }


def classify(name: str) -> tuple[str, str] | None:
    for label, pat in BANNED_PATTERNS:
        if pat.search(name):
            cure = "move provenance to /// doc comment; rename per <subject>_<scenario>_<expected>"
            return (label, cure)
    return None


def scan_rust(path: Path) -> Iterable[Finding]:
    text = path.read_text(encoding="utf-8", errors="replace")
    lines = text.splitlines()
    pending_test = False
    for idx, line in enumerate(lines, start=1):
        if RUST_ATTR_TEST.search(line):
            pending_test = True
            continue
        if pending_test:
            m = RUST_TEST_RE.match(line)
            if m:
                name = m.group(1)
                hit = classify(name)
                if hit:
                    yield Finding(str(path), idx, name, hit[0], hit[1])
                pending_test = False
            elif line.strip().startswith("#["):
                continue
            elif line.strip() and not line.strip().startswith("//"):
                pending_test = False


def scan_ori(path: Path) -> Iterable[Finding]:
    text = path.read_text(encoding="utf-8", errors="replace")
    for idx, line in enumerate(text.splitlines(), start=1):
        m = ORI_TEST_RE.match(line)
        if m:
            name = m.group(1)
            hit = classify(name)
            if hit:
                yield Finding(str(path), idx, f"@{name}", hit[0], hit[1])


def discover(roots: list[Path]) -> Iterable[Path]:
    for root in roots:
        if root.is_file():
            yield root
            continue
        for dirpath, dirnames, filenames in os.walk(root):
            dirnames[:] = [d for d in dirnames if d not in {"target", "node_modules", ".git"}]
            for fn in filenames:
                if fn.endswith(".rs") or fn.endswith(".ori"):
                    yield Path(dirpath) / fn


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("paths", nargs="*", help="files or directories to scan")
    ap.add_argument("--json", action="store_true", help="emit JSON findings")
    ap.add_argument("--exit-zero", action="store_true", help="report findings without exit 1")
    args = ap.parse_args(argv)

    if args.paths:
        roots = [Path(p) for p in args.paths]
    else:
        repo = Path(__file__).resolve().parents[1]
        roots = [
            repo / "compiler",
            repo / "tests",
            repo / "library",
        ]
        roots = [r for r in roots if r.exists()]

    findings: list[Finding] = []
    for fp in discover(roots):
        if fp.suffix == ".rs":
            findings.extend(scan_rust(fp))
        elif fp.suffix == ".ori":
            findings.extend(scan_ori(fp))

    if args.json:
        json.dump([f.to_dict() for f in findings], sys.stdout, indent=2)
        sys.stdout.write("\n")
    else:
        for f in findings:
            print(f"{f.file}:{f.line}: {f.name} [{f.category}] — {f.suggestion}")
        if findings:
            print(f"\nTotal: {len(findings)} test-naming violation(s).")
            print("Rule: test function names use <subject>_<scenario>_<expected> shape; bug IDs / plan refs in /// doc comments only.")
        else:
            print("Clean: no test-naming violations found.")

    if findings and not args.exit_zero:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
