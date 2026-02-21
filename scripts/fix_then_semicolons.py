#!/usr/bin/env python3
"""Fix incorrectly-placed semicolons after then-branches in if/else expressions.

The add_item_semicolons.py script incorrectly added `;` after then-branches
when the expression continues with `else` on the next line:

    if x then "A";    ← WRONG: `;` terminates the function body
    else "B"

This script removes those `;` so the expression stays intact.
The correct `;` at the end of the whole expression will be re-added by
running add_item_semicolons.py again after this fix.

Usage:
    python3 scripts/fix_then_semicolons.py tests/ library/
"""

import os
import sys


def fix_file(filepath):
    """Remove `;` from then-branches that are followed by `else` on the next line."""
    with open(filepath, "r") as f:
        lines = f.readlines()

    changes = 0
    result = []
    n = len(lines)

    for i in range(n):
        line = lines[i]
        stripped = line.rstrip()

        # Check if this line ends with `;` and the next non-blank line starts with `else`
        if stripped.endswith(";"):
            # Look ahead for `else` on next non-blank line
            j = i + 1
            while j < n and lines[j].strip() == "":
                j += 1

            if j < n and lines[j].lstrip().startswith("else"):
                # Remove the trailing `;` from this line
                result.append(stripped[:-1] + "\n")
                changes += 1
                continue

        result.append(line)

    if changes > 0:
        with open(filepath, "w") as f:
            f.writelines(result)
        print(f"  {filepath}: {changes} semicolons removed")

    return changes


def main():
    if len(sys.argv) < 2:
        print("Usage: python3 scripts/fix_then_semicolons.py <path>...")
        sys.exit(1)

    total = 0
    for path in sys.argv[1:]:
        if os.path.isfile(path) and (path.endswith(".ori") or path.endswith(".ori.expected")):
            total += fix_file(path)
        elif os.path.isdir(path):
            for root, _, files in sorted(os.walk(path)):
                for fname in sorted(files):
                    if fname.endswith(".ori") or fname.endswith(".ori.expected"):
                        total += fix_file(os.path.join(root, fname))

    print(f"\nTotal: {total} semicolons removed")


if __name__ == "__main__":
    main()
