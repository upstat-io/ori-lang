#!/usr/bin/env python3
"""Regenerate .ori.expected files by running the Ori formatter.

For each .ori.expected file found, runs the formatter on the corresponding
.ori input file and writes the result as the new expected output.

Uses `cargo run -p oric -- fmt --check` internally, but since we need
the raw formatter output (not check mode), we use a small Rust helper.

Actually, since we just need to add `;` at the end of expression-body
declarations in .ori.expected files, this script does a targeted fix:
finds lines ending with a non-`;` non-`}` character that are the last
line of a declaration body, and adds `;`.

Usage:
    python3 scripts/regen_expected.py tests/fmt/
"""

import os
import sys
import re


def needs_semicolon_at_end(lines, idx):
    """Check if line at idx is the end of an expression body needing `;`.

    Heuristic: a line that ends the body of a function/test/type declaration
    (i.e., is followed by a blank line or end-of-file or next declaration)
    and doesn't end with `}`, `;`, `{`, or `,`.
    """
    line = lines[idx].rstrip()
    if not line:
        return False

    last_char = line[-1]
    if last_char in (';', '}', '{', ',', '(', '[', '|'):
        return False

    # Check if this line is within a declaration context
    # Walk backward to find if there's an `= ` that started this body
    for back in range(idx, max(idx - 30, -1), -1):
        back_line = lines[back].rstrip()
        if re.search(r'\)\s*->\s*\S+\s*=\s*\S', back_line):
            return True
        if re.search(r'\)\s*=\s*\S', back_line):
            return True
        # Multiline: `) -> Type = ` at end (body on next line)
        if re.search(r'\)\s*->\s*\S+\s*=\s*$', back_line):
            return True
        if re.search(r'\)\s*=\s*$', back_line):
            return True
        # Sum type or newtype: `type X = expr`
        if re.search(r'^(?:pub\s+)?type\s+\w+.*=\s+\S', back_line):
            return True
    return False


def is_declaration_line(line):
    """Check if a line starts a new declaration."""
    stripped = line.lstrip()
    return (
        stripped.startswith("@")
        or stripped.startswith("pub @")
        or stripped.startswith("type ")
        or stripped.startswith("pub type ")
        or stripped.startswith("trait ")
        or stripped.startswith("impl ")
        or stripped.startswith("use ")
        or stripped.startswith("#")
    )


def process_expected_file(filepath):
    """Add missing `;` to expression bodies in a .expected file."""
    with open(filepath, "r") as f:
        content = f.read()

    lines = content.split("\n")
    changes = 0

    for i in range(len(lines)):
        line = lines[i].rstrip()
        if not line:
            continue

        last_char = line[-1]
        # Skip lines already terminated
        if last_char in (';', '}', '{', ',', '(', '[', '|', ':'):
            continue

        # Skip comments
        if line.lstrip().startswith("//"):
            continue

        # Check if next non-empty line is a new declaration or end-of-file
        next_significant = None
        for j in range(i + 1, len(lines)):
            if lines[j].strip():
                next_significant = lines[j]
                break

        is_body_end = (
            next_significant is None  # end of file
            or next_significant.strip() == ""  # shouldn't happen (we skip blank)
            or is_declaration_line(next_significant)
        )

        if not is_body_end:
            continue

        # This line ends a body. Check if it's part of a declaration with `=`
        if needs_semicolon_at_end(lines, i):
            lines[i] = line + ";"
            changes += 1

    if changes > 0:
        with open(filepath, "w") as f:
            f.write("\n".join(lines))
        print(f"  {filepath}: {changes} semicolons added")

    return changes


def main():
    if len(sys.argv) < 2:
        print("Usage: python3 scripts/regen_expected.py <path>...")
        sys.exit(1)

    total = 0
    for path in sys.argv[1:]:
        if os.path.isfile(path) and path.endswith(".ori.expected"):
            total += process_expected_file(path)
        elif os.path.isdir(path):
            for root, _, files in sorted(os.walk(path)):
                for fname in sorted(files):
                    if fname.endswith(".ori.expected"):
                        total += process_expected_file(os.path.join(root, fname))

    print(f"\nTotal: {total} semicolons added")


if __name__ == "__main__":
    main()
