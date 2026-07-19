#!/usr/bin/env python3
"""Operation-name-without-operation linter (NAME-34 / NAMING:operation-name-without-operation).

Flags a binding NAMED for an operation whose own initializer never performs that
operation — `let cloned = export;` (a move, not a clone). The name asserts
behavior the code lost; it is the classic residue of a mechanical autofix
(clippy `redundant_clone`) that stripped the operation without renaming
(per impl-hygiene.md NAME-34 + TEST-34).

Mechanically gates ONLY the near-zero-false-positive subset (Rust `.rs` files):
  - a `let cloned` binding whose initializer (text from `=` to the terminating `;`)
    contains NO clone token. In Rust, cloning is ALWAYS explicit, so this is
    unambiguous residue.

Flagged:   let cloned = export;            (move named as a clone)
           let cloned: WasmExport = src;
Exempt:    let cloned = export.clone();    (initializer clones)
           let cloned = src.to_owned();
           let cloned = make_clone();      (initializer defers to a clone callee)
           // let cloned = x               (comment / doc line — prose, not code)

Deliberately NOT gated (reviewer-time per NAME-34 + TEST-34 + TEST-35 — FP-prone):
  - `copied` / `duplicated` bindings: a `Copy`-type assignment is legitimately
    "copied" with no call (`let copied = kind; // Copy`), and `duplicated` is often
    a bool flag.
  - `.ori` files: Ori value semantics make `let cloned = x` a legitimate copy.
  - a `*_clone`/`*_copy` test fn whose BODY performs no clone (needs a span scan).
  - the wider domain-mismatch surface (a name that says X while the body does any other Y).

Usage:
  scripts/operation-name-lint.py [paths...]   # default: compiler/ library/ tests/
  scripts/operation-name-lint.py --json       # machine-readable
  scripts/operation-name-lint.py --warn-only  # report without exit 1 (bed-in)
  scripts/operation-name-lint.py --exit-zero  # alias of --warn-only
  scripts/operation-name-lint.py --self-test  # run embedded positive/negative cases

Returns exit 1 when any violation is found (unless --warn-only/--exit-zero).
"""
from __future__ import annotations

import argparse
import json
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path

# A Rust binding named `cloned` — the ONLY near-zero-false-positive token. In
# Rust, cloning is ALWAYS explicit (`.clone()` / `Clone::clone` / `.to_owned()`),
# so a `cloned` binding with no clone in its initializer is unambiguous residue.
# `copied` / `duplicated` are deliberately NOT gated: a `Copy`-type assignment is
# legitimately "copied" with no call, and `duplicated` is often a bool flag — both
# are reviewer-time per NAME-34. Ori (`.ori`) is excluded: its value semantics make
# `let cloned = x` a legitimate copy, so the Rust-only invariant does not hold there.
BINDING_RE = re.compile(
    r"\blet\s+(?:mut\s+)?(cloned)\b(?:\s*:[^=;]+)?\s*=\s*(.*)",
)
# Comment / doc-comment lines never carry code residue — a `///` line quoting
# `let cloned = x` is prose, not a binding.
COMMENT_RE = re.compile(r"^\s*(?://|/\*|\*)")
# Tokens proving the initializer DOES perform a clone/copy (so it is not residue).
OP_TOKENS = re.compile(
    r"\.clone\(|\.clone_from\(|Clone::clone|\.to_owned\(|\.to_vec\(|\.copied\(|"
    r"\.cloned\(|clone|copy|copies|duplicat",
    re.IGNORECASE,
)
# How many lines a single `let` statement may span before we give up (FP guard).
MAX_STMT_LINES = 4


@dataclass
class Finding:
    file: str
    line: int
    name: str
    category: str

    def to_dict(self) -> dict[str, str | int]:
        return {"file": self.file, "line": self.line, "name": self.name, "category": self.category}


def _statement_text(lines: list[str], start: int) -> str:
    """Initializer text from the match line through the line ending in `;`."""
    chunk: list[str] = []
    for off in range(MAX_STMT_LINES):
        if start + off >= len(lines):
            break
        chunk.append(lines[start + off])
        if ";" in lines[start + off]:
            break
    return "\n".join(chunk)


def scan_text(content: str) -> list[tuple[int, str]]:
    """Return (1-based line, binding-name) for each operation-name-without-operation."""
    lines = content.splitlines()
    out: list[tuple[int, str]] = []
    for idx, line in enumerate(lines):
        if COMMENT_RE.match(line):
            continue
        m = BINDING_RE.search(line)
        if not m:
            continue
        name = m.group(1)
        remainder = m.group(2)
        # Initializer is scoped to THIS statement. If the `let ...;` terminates on
        # its own line, the initializer is the text before its `;` — never the
        # following lines (a later `cloned.field` use would falsely look like a
        # clone). Only gather continuation lines when the statement is unterminated.
        if ";" in remainder:
            init = remainder.split(";", 1)[0]
        else:
            init = remainder + "\n" + _statement_text(lines, idx + 1)
        if not OP_TOKENS.search(init):
            out.append((idx + 1, name))
    return out


# A finding requires a `cloned` binding, so a file without the substring can
# never produce one — skipped without decode or line-walking.
def scan_file(path: Path) -> list[Finding]:
    raw = path.read_bytes()
    if b"cloned" not in raw:
        return []
    content = raw.decode("utf-8", errors="replace")
    return [Finding(str(path), ln, name, "operation-name-without-operation")
            for ln, name in scan_text(content)]


def discover(roots: list[Path]):
    for root in roots:
        if root.is_file():
            yield root
            continue
        for dirpath, dirnames, filenames in os.walk(root):
            dirnames[:] = [d for d in dirnames if d not in {"target", "node_modules", ".git"}]
            for fn in filenames:
                if fn.endswith(".rs"):
                    yield Path(dirpath) / fn


def run(roots: list[Path]) -> list[Finding]:
    findings: list[Finding] = []
    for fp in discover(roots):
        findings.extend(scan_file(fp))
    return findings


def self_test() -> int:
    must_flag = [
        "let cloned = export;",
        "let cloned = foo.bar;",
        "let mut cloned = src;",
        "let cloned: WasmExport = export;",
        # The exact wasm.rs residue shape: a move named `cloned`, then a later
        # `cloned.field` use on the next line (must NOT be mistaken for a clone).
        'let cloned = export;\nassert_eq!(cloned.ori_name, "test");',
    ]
    must_pass = [
        "let cloned = export.clone();",
        "let cloned = src.to_owned();",
        "let cloned = nodes.to_vec();",
        "let cloned = make_clone();",
        "let cloned = Clone::clone(&x);",
        "let copied = kind;",              # `copied` not gated — Copy-by-assignment is legitimate
        "let duplicated = items.iter().any(|x| seen.contains(x));",  # `duplicated` not gated — bool flag
        "let url = clone_url;",            # clone is a noun; binding not named `cloned`
        "let export = export;",
        "let x = y;",
        "let cloned = foo\n    .clone();",  # multi-line clone init
        "    /// let cloned = x // a doc line quoting code",  # comment — never code residue
    ]
    failures: list[str] = []
    for snippet in must_flag:
        if not scan_text(snippet):
            failures.append(f"FALSE NEGATIVE: {snippet!r} should flag but did not")
    for snippet in must_pass:
        if scan_text(snippet):
            failures.append(f"FALSE POSITIVE: {snippet!r} flagged")
    if failures:
        for f in failures:
            print(f"  {f}")
        print(f"\nself-test FAILED: {len(failures)} case(s).")
        return 1
    print(f"self-test ok: {len(must_flag)} flagged, {len(must_pass)} exempt.")
    return 0


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("paths", nargs="*", help="files or directories to scan")
    ap.add_argument("--json", action="store_true", help="emit JSON findings")
    ap.add_argument("--warn-only", action="store_true", help="report findings without exit 1")
    ap.add_argument("--exit-zero", action="store_true", help="alias of --warn-only")
    ap.add_argument("--self-test", action="store_true", help="run embedded positive/negative cases")
    args = ap.parse_args(argv)

    if args.self_test:
        return self_test()

    if args.paths:
        roots = [Path(p) for p in args.paths]
    else:
        repo = Path(__file__).resolve().parents[1]
        roots = [repo / "compiler", repo / "library", repo / "tests"]
        roots = [r for r in roots if r.exists()]

    findings = run(roots)

    if args.json:
        json.dump([f.to_dict() for f in findings], sys.stdout, indent=2)
        sys.stdout.write("\n")
    else:
        for f in findings:
            print(f"{f.file}:{f.line}: {f.name} [NAMING:operation-name-without-operation]")
        if findings:
            print(f"\nTotal: {len(findings)} operation-name-without-operation violation(s).")
            print("Rule NAME-34: rename to what the code does, OR restore the operation "
                  "if it was load-bearing (TEST-34).")
        else:
            print("Clean: no operation-name-without-operation violations found.")

    if findings and not (args.warn_only or args.exit_zero):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
