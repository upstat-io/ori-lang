#!/usr/bin/env python3
"""Add trailing semicolons to expression-body items in .ori files.

Items (functions, tests, type declarations) with expression bodies that
don't end with `}` need a trailing `;` per the grammar enforcement:

    @f () -> int = 42       ->  @f () -> int = 42;
    type Id = int           ->  type Id = int;
    type Color = Red | Blue ->  type Color = Red | Blue;

Items with block bodies (ending with `}`) are left unchanged:

    @f () -> int = { 42 }   ->  (no change)
    type Point = { x: int } ->  (no change)

Usage:
    python3 scripts/add_item_semicolons.py tests/
    python3 scripts/add_item_semicolons.py tests/spec/types/primitives.ori
"""

import os
import sys

# Keywords/operators that indicate the expression continues on the next line
CONTINUATION_ENDINGS = (
    "yield", "do", "then", "else", "in", "->", "=>",
    "+", "-", "*", "/", "%", "&&", "||", "|", "&", "^",
    "==", "!=", "<", ">", "<=", ">=", "..", "..=",
    "??", "=", ",", "(",
)


def is_comment_line(line: str) -> bool:
    """Check if a line is a comment (ignoring leading whitespace)."""
    return line.lstrip().startswith("//")


def is_declaration_start(line: str) -> bool:
    """Check if a line starts a new top-level declaration."""
    stripped = line.lstrip()
    return (
        stripped.startswith("@")
        or stripped.startswith("type ")
        or stripped.startswith("trait ")
        or stripped.startswith("impl ")
        or stripped.startswith("def impl ")
        or stripped.startswith("extend ")
        or stripped.startswith("use ")
        or stripped.startswith("pub ")
        or stripped.startswith("extension ")
        or stripped.startswith("extern ")
        or stripped.startswith("let $")
        or stripped.startswith("$")
        or stripped.startswith("#")  # attributes
        or stripped.startswith("capset ")
    )


def line_continues(line: str) -> bool:
    """Check if a line's expression continues on the next line.

    True if the line ends with an operator/keyword that expects a right-hand side.
    """
    stripped = line.rstrip()
    if not stripped:
        return False
    for ending in CONTINUATION_ENDINGS:
        if stripped.endswith(ending):
            return True
    return False


def next_line_is_continuation(lines: list, idx: int) -> bool:
    """Check if the line at `idx` is a continuation of the previous expression.

    True if the line starts with `.` (method chain) or is indented
    and starts with an operator.
    """
    if idx >= len(lines):
        return False
    stripped = lines[idx].lstrip()
    if not stripped or stripped.startswith("//"):
        return False
    # Method chain continuation: .method(...)
    if stripped.startswith("."):
        return True
    # Operator continuation at start of indented line
    if lines[idx][0] in (" ", "\t") and stripped[0] in ("|", "&", "+", "-"):
        return True
    # else continuation (if/then/else chains)
    if stripped.startswith("else"):
        return True
    return False


def find_eq_in_decl(line: str) -> int:
    """Find the `=` that introduces a declaration body.

    Returns the index of `=` in the line, or -1 if not found.
    Skips `=` inside comparison/compound operators.
    """
    i = len(line) - 1
    depth = 0
    while i >= 0:
        ch = line[i]
        if ch in (")", "]", "}"):
            depth += 1
        elif ch in ("(", "[", "{"):
            depth = max(0, depth - 1)
        elif ch == "=" and depth == 0:
            if i > 0 and line[i - 1] in ("=", "!", "<", ">", "."):
                i -= 1
                continue
            if i + 1 < len(line) and line[i + 1] in ("=", ">"):
                i -= 1
                continue
            return i
        i -= 1
    return -1


def find_expression_end(lines: list, start_idx: int, after_eq: str) -> int:
    """Find the index of the LAST line of a declaration's expression body.

    Starts from `start_idx` (the declaration line) and walks forward,
    tracking:
    - Delimiter depth (parens, brackets, braces)
    - Method chain continuations (next line starts with `.`)
    - Operator continuations (line ends with operator/keyword)
    - Blank lines / next declarations as terminators

    Returns the index of the last line of the expression body.
    """
    n = len(lines)

    # Track delimiter depth in the expression
    depth = 0
    for ch in after_eq:
        if ch in ("(", "[", "{"):
            depth += 1
        elif ch in (")", "]", "}"):
            depth -= 1

    current = start_idx

    while True:
        # If delimiters are unbalanced, expression continues
        if depth > 0:
            current += 1
            if current >= n:
                break
            for ch in lines[current]:
                if ch in ("(", "[", "{"):
                    depth += 1
                elif ch in (")", "]", "}"):
                    depth -= 1
            continue

        # Delimiters are balanced. Check if expression continues:

        # 1. Current line ends with a continuation operator/keyword
        if line_continues(lines[current]):
            current += 1
            if current >= n:
                break
            for ch in lines[current]:
                if ch in ("(", "[", "{"):
                    depth += 1
                elif ch in (")", "]", "}"):
                    depth -= 1
            continue

        # 2. Next non-blank line is a method chain or operator continuation
        peek = current + 1
        while peek < n and lines[peek].strip() == "":
            peek += 1
        if peek < n and next_line_is_continuation(lines, peek):
            current = peek
            for ch in lines[current]:
                if ch in ("(", "[", "{"):
                    depth += 1
                elif ch in (")", "]", "}"):
                    depth -= 1
            continue

        # Expression is complete
        break

    return current


def needs_semicolon(line: str) -> bool:
    """Check if a line ends an expression body that needs `;`.

    Returns True if the line doesn't already end with `;`, `}`, `,`, or `{`.
    """
    stripped = line.rstrip()
    if not stripped:
        return False
    return stripped[-1] not in (";", "}", "{", ",", "(", "[")


def process_file(filepath: str, dry_run: bool = False) -> int:
    """Process a single .ori file, adding semicolons where needed.

    Returns the number of semicolons added.
    """
    with open(filepath, "r") as f:
        lines = f.readlines()

    changes = 0
    result = []
    i = 0
    n = len(lines)
    # Track which line indices get `;` appended (computed in first pass)
    semicolon_lines = set()

    # --- First pass: identify which lines need `;` ---
    while i < n:
        line = lines[i]

        if is_comment_line(line):
            i += 1
            continue

        stripped = line.lstrip()
        is_decl = (
            stripped.startswith("@")
            or stripped.startswith("pub @")
            or stripped.startswith("type ")
            or stripped.startswith("pub type ")
        )

        if not is_decl:
            i += 1
            continue

        eq_idx = find_eq_in_decl(line)
        if eq_idx == -1:
            i += 1
            continue

        after_eq = line[eq_idx + 1:].strip()

        if after_eq.startswith("{"):
            # Block body — no `;` needed
            i += 1
            continue

        if not after_eq:
            # `=` at end of line — body starts on next line
            i += 1
            if i < n:
                # Check if next non-blank line starts with `{`
                peek = i
                while peek < n and lines[peek].strip() == "":
                    peek += 1
                if peek < n and lines[peek].lstrip().startswith("{"):
                    i = peek + 1
                    continue
                # Find the end of the multi-line expression body
                end_idx = find_expression_end(lines, i, lines[i] if i < n else "")
                if needs_semicolon(lines[end_idx]):
                    semicolon_lines.add(end_idx)
                i = end_idx + 1
            continue

        # Expression body on same line as `=`
        end_idx = find_expression_end(lines, i, after_eq)

        if end_idx == i:
            # Single-line expression
            if needs_semicolon(line):
                semicolon_lines.add(i)
        else:
            # Multi-line expression
            if needs_semicolon(lines[end_idx]):
                semicolon_lines.add(end_idx)

        i = end_idx + 1

    # --- Second pass: apply changes ---
    for i, line in enumerate(lines):
        if i in semicolon_lines:
            result.append(line.rstrip("\n") + ";\n")
            changes += 1
        else:
            result.append(line)

    if changes > 0 and not dry_run:
        with open(filepath, "w") as f:
            f.writelines(result)

    return changes


def process_directory(dirpath: str, dry_run: bool = False) -> int:
    """Process all .ori files in a directory tree."""
    total = 0
    for root, _, files in sorted(os.walk(dirpath)):
        for fname in sorted(files):
            if fname.endswith(".ori") or fname.endswith(".ori.expected"):
                filepath = os.path.join(root, fname)
                changes = process_file(filepath, dry_run=dry_run)
                if changes > 0:
                    print(f"  {filepath}: {changes} semicolons added")
                    total += changes
    return total


def main():
    if len(sys.argv) < 2:
        print("Usage: python3 scripts/add_item_semicolons.py [--dry-run] <path>...")
        sys.exit(1)

    dry_run = "--dry-run" in sys.argv
    paths = [p for p in sys.argv[1:] if p != "--dry-run"]

    total = 0
    for path in paths:
        if os.path.isfile(path):
            changes = process_file(path, dry_run=dry_run)
            if changes > 0:
                print(f"  {path}: {changes} semicolons added")
                total += changes
        elif os.path.isdir(path):
            total += process_directory(path, dry_run=dry_run)
        else:
            print(f"Warning: {path} not found", file=sys.stderr)

    action = "would add" if dry_run else "added"
    print(f"\nTotal: {action} {total} semicolons")


if __name__ == "__main__":
    main()
