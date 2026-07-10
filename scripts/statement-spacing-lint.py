#!/usr/bin/env python3
"""Statement-paragraph spacing linter (NAME-46 / CODE_STYLE:statement-paragraph-spacing).

Flags two consecutive multi-line `let` statements with no separating blank line
(per impl-hygiene.md NAME-46). Each multi-line statement is a code paragraph;
the blank line marks the concept boundary, exactly as multi-line match arms are
isolated (NAME-38).

Mechanically gates ONLY the near-zero-false-positive subset (Rust `.rs` files):
  - two consecutive `let` statements, each spanning >= 3 physical lines, at
    identical indentation, with the second starting on the line immediately
    after the first ends.

Flagged:   let ttr: Vec<usize> = contract
               .params
               .collect();
           let dims: Vec<String> = contract     <- no blank line above
               .params
               .collect();
Exempt:    the same pair separated by a blank line;
           a comment line between the pair (comment placement is COMMENT-51);
           a multi-line let followed by a single-line let (one paragraph tail);
           runs of single-line statements (one paragraph).

Deliberately NOT gated (reviewer-time per NAME-46 -- FP-prone):
  - non-`let` multi-line statements (expression statements, match/if blocks);
  - `.ori` files (`ori fmt` owns Ori spacing).

Usage:
  scripts/statement-spacing-lint.py [paths...]   # default: compiler/ library/ tests/
  scripts/statement-spacing-lint.py --json       # machine-readable
  scripts/statement-spacing-lint.py --warn-only  # report without exit 1 (bed-in)
  scripts/statement-spacing-lint.py --exit-zero  # alias of --warn-only
  scripts/statement-spacing-lint.py --self-test  # run embedded positive/negative cases

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

# Minimum physical lines for a statement to count as a paragraph.
MIN_PARAGRAPH_LINES = 3
# Statements longer than this are skipped (parse-confusion guard, FP guard).
MAX_STMT_LINES = 40

# A `let` statement starting a line (leading whitespace captured for the
# identical-indent gate). `if let` / `while let` / `else if let` never match:
# the line must START with `let`.
LET_RE = re.compile(r"^(\s*)let\b")
# Raw / byte-raw string opener at the current position: r"...", r#"..."#, br"...".
RAW_STR_RE = re.compile(r'b?r(#*)"')
# A char literal (escaped or single-char). A bare lifetime tick (`'a` in
# `<'a, 'b>`) has no closing quote and never matches.
CHAR_LIT_RE = re.compile(r"'(?:\\.[^']*|[^'\\])'")


@dataclass
class Finding:
    file: str
    line: int  # 1-based line where the SECOND statement starts
    prev_span: int
    span: int
    category: str = "statement-paragraph-spacing"

    def to_dict(self) -> dict[str, str | int]:
        return {
            "file": self.file,
            "line": self.line,
            "prev_span": self.prev_span,
            "span": self.span,
            "category": self.category,
        }


def mask_non_code(content: str) -> str:
    """Blank out comment and string/char-literal contents (newlines preserved)
    so delimiter counting and `;` detection see only real code structure."""
    out = list(content)
    n = len(content)

    def blank(a: int, b: int) -> None:
        for k in range(a, min(b, n)):
            if out[k] != "\n":
                out[k] = " "

    i = 0
    while i < n:
        c = content[i]
        two = content[i : i + 2]
        if two == "//":
            j = content.find("\n", i)
            j = n if j == -1 else j
            blank(i, j)
            i = j
        elif two == "/*":
            # Rust block comments nest.
            depth = 0
            j = i
            while j < n:
                if content[j : j + 2] == "/*":
                    depth += 1
                    j += 2
                elif content[j : j + 2] == "*/":
                    depth -= 1
                    j += 2
                    if depth == 0:
                        break
                else:
                    j += 1
            blank(i, j)
            i = j
        elif c == '"':
            j = i + 1
            while j < n:
                if content[j] == "\\":
                    j += 2
                elif content[j] == '"':
                    j += 1
                    break
                else:
                    j += 1
            blank(i, j)
            i = j
        elif c in "rb":
            m = RAW_STR_RE.match(content, i)
            if m:
                close = '"' + m.group(1)
                j = content.find(close, m.end())
                j = n if j == -1 else j + len(close)
                blank(i, j)
                i = j
            else:
                i += 1
        elif c == "'":
            m = CHAR_LIT_RE.match(content, i)
            if m:
                blank(i, m.end())
                i = m.end()
            else:
                i += 1  # lifetime tick / label -- not a literal
        else:
            i += 1
    return "".join(out)


def _let_statements(masked_lines: list[str]) -> list[tuple[int, int, str]]:
    """(start_idx, end_idx, indent) per line-starting `let` statement, where
    end_idx is the line carrying the terminating `;` at delimiter depth 0."""
    stmts: list[tuple[int, int, str]] = []
    i = 0
    n = len(masked_lines)
    while i < n:
        m = LET_RE.match(masked_lines[i])
        if not m:
            i += 1
            continue
        indent = m.group(1)
        depth = 0
        end: int | None = None
        for j in range(i, min(i + MAX_STMT_LINES, n)):
            for ch in masked_lines[j]:
                if ch in "([{":
                    depth += 1
                elif ch in ")]}":
                    depth -= 1
                elif ch == ";" and depth <= 0:
                    end = j
                    break
            if end is not None:
                break
        if end is None:
            i += 1
            continue
        stmts.append((i, end, indent))
        i = end + 1
    return stmts


def scan_text(content: str) -> list[tuple[int, int, int]]:
    """Return (1-based line of second stmt, prev_span, span) per violation."""
    lines = mask_non_code(content).splitlines()
    stmts = _let_statements(lines)
    out: list[tuple[int, int, int]] = []
    for (a_start, a_end, a_ind), (b_start, b_end, b_ind) in zip(stmts, stmts[1:]):
        a_span = a_end - a_start + 1
        b_span = b_end - b_start + 1
        if (
            b_start == a_end + 1
            and a_ind == b_ind
            and a_span >= MIN_PARAGRAPH_LINES
            and b_span >= MIN_PARAGRAPH_LINES
        ):
            out.append((b_start + 1, a_span, b_span))
    return out


def scan_file(path: Path) -> list[Finding]:
    content = path.read_text(encoding="utf-8", errors="replace")
    return [Finding(str(path), ln, prev, span) for ln, prev, span in scan_text(content)]


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
    flag_adjacent = (
        "    let ttr: Vec<usize> = contract\n"
        "        .params\n"
        "        .iter()\n"
        "        .collect();\n"
        "    let param_dims: Vec<String> = contract\n"
        "        .params\n"
        "        .iter()\n"
        "        .collect();\n"
    )
    flag_match_pair = (
        "    let a = match x {\n"
        "        Some(v) => v,\n"
        "        None => 0,\n"
        "    };\n"
        "    let b = match y {\n"
        "        Some(v) => v,\n"
        "        None => 0,\n"
        "    };\n"
    )
    # String contents must not confuse the span scanner: the `;` and `}` inside
    # the literal are masked, so both statements still span 4 lines each.
    flag_string_contents = (
        "    let a = foo(\n"
        '        "text with ; and } inside",\n'
        "        x,\n"
        "    );\n"
        "    let b = bar(\n"
        '        r#"raw with ; and " inside"#,\n'
        "        y,\n"
        "    );\n"
        "    let done = 1;\n"
    )
    must_flag = [flag_adjacent, flag_match_pair, flag_string_contents]

    pass_blank_line = flag_adjacent.replace(
        "    .collect();\n    let param_dims",
        "    .collect();\n\n    let param_dims",
    )
    pass_comment_between = flag_adjacent.replace(
        "    .collect();\n    let param_dims",
        "    .collect();\n    // dims per param\n    let param_dims",
    )
    pass_single_line_tail = (
        "    let ttr: Vec<usize> = contract\n"
        "        .params\n"
        "        .iter()\n"
        "        .collect();\n"
        "    let n = ttr.len();\n"
    )
    pass_single_line_run = "    let a = 1;\n    let b = 2;\n    let c = 3;\n"
    pass_two_line_pair = (
        "    let a = foo(\n        x);\n"
        "    let b = bar(\n        y);\n"
    )
    pass_differing_indent = (
        "    let a = foo(|| {\n"
        "        let inner = build(\n"
        "            x,\n"
        "            y,\n"
        "        );\n"
        "        inner\n"
        "    });\n"
    )
    pass_inner_semicolons = (
        "    let a = xs.iter().map(|x| {\n"
        "        let y = x + 1;\n"
        "        y * 2\n"
        "    });\n"
        "    let done = 1;\n"
    )
    must_pass = [
        pass_blank_line,
        pass_comment_between,
        pass_single_line_tail,
        pass_single_line_run,
        pass_two_line_pair,
        pass_differing_indent,
        pass_inner_semicolons,
        "    if let Some(x) = opt {\n        use_it(x);\n    }\n",
    ]

    failures: list[str] = []
    for idx, snippet in enumerate(must_flag):
        if not scan_text(snippet):
            failures.append(f"FALSE NEGATIVE: must_flag[{idx}] did not flag")
    for idx, snippet in enumerate(must_pass):
        hits = scan_text(snippet)
        if hits:
            failures.append(f"FALSE POSITIVE: must_pass[{idx}] flagged at {hits}")
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
            print(
                f"{f.file}:{f.line}: consecutive multi-line let statements with no "
                f"blank-line separation (prev {f.prev_span} lines, this {f.span} lines) "
                f"[CODE_STYLE:statement-paragraph-spacing]"
            )
        if findings:
            print(f"\nTotal: {len(findings)} statement-paragraph-spacing violation(s).")
            print(
                "Rule NAME-46: insert a blank line between statement paragraphs, or "
                "extract the unnamed computation into a named helper (SPEC-73)."
            )
        else:
            print("Clean: no statement-paragraph-spacing violations found.")

    if findings and not (args.warn_only or args.exit_zero):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
