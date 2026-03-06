#!/usr/bin/env python3
"""Extract LLVM IR from a journey results markdown file.

Parses the first ```llvm fenced code block that appears after the
`#### Generated LLVM IR` header. Ignores later ```llvm blocks
(ideal/comparison sections).

Usage:
    python3 extract_ir_from_results.py results.md > ir.txt
    python3 extract_ir_from_results.py results.md --output ir.txt
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


def extract_ir(markdown: str) -> str | None:
    """Extract the Generated LLVM IR block from results markdown.

    Returns the IR text (without fences), or None if not found.
    """
    lines = markdown.split("\n")
    # Find the header
    header_idx = None
    for i, line in enumerate(lines):
        if line.strip() == "#### Generated LLVM IR":
            header_idx = i
            break

    if header_idx is None:
        return None

    # Find the first ```llvm after the header
    fence_start = None
    for i in range(header_idx + 1, len(lines)):
        if lines[i].strip() == "```llvm":
            fence_start = i
            break

    if fence_start is None:
        return None

    # Find the closing ```
    fence_end = None
    for i in range(fence_start + 1, len(lines)):
        if lines[i].strip() == "```":
            fence_end = i
            break

    if fence_end is None:
        return None

    return "\n".join(lines[fence_start + 1 : fence_end])


def extract_frontmatter(markdown: str) -> dict[str, str]:
    """Extract YAML frontmatter key-value pairs (simple flat parsing)."""
    lines = markdown.split("\n")
    if not lines or lines[0].strip() != "---":
        return {}

    result = {}
    for line in lines[1:]:
        if line.strip() == "---":
            break
        m = re.match(r"^(\w[\w_]*)\s*:\s*(.+)$", line)
        if m:
            result[m.group(1)] = m.group(2).strip().strip('"')
    return result


def main() -> int:
    p = argparse.ArgumentParser(description="Extract LLVM IR from journey results markdown.")
    p.add_argument("results_file", type=Path, help="Path to *-results.md file")
    p.add_argument("--output", "-o", type=Path, default=None, help="Output file (default: stdout)")
    args = p.parse_args()

    if not args.results_file.exists():
        print(f"ERROR: File not found: {args.results_file}", file=sys.stderr)
        return 1

    markdown = args.results_file.read_text()
    ir = extract_ir(markdown)

    if ir is None:
        print(f"WARNING: No '#### Generated LLVM IR' block found in {args.results_file.name}",
              file=sys.stderr)
        return 1

    if args.output:
        args.output.write_text(ir + "\n")
        print(f"Extracted {len(ir.splitlines())} lines of IR to {args.output}", file=sys.stderr)
    else:
        print(ir)

    return 0


if __name__ == "__main__":
    sys.exit(main())
