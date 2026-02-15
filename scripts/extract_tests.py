#!/usr/bin/env python3
"""Extract inline #[cfg(test)] mod tests { ... } blocks to sibling tests.rs files.

For each .rs file with an inline test module:
  - foo.rs       → foo/tests.rs  (mkdir foo/ if needed)
  - bar/mod.rs   → bar/tests.rs

The inline block is replaced with: #[cfg(test)]\nmod tests;

Usage:
    python3 scripts/extract_tests.py                  # dry-run (default)
    python3 scripts/extract_tests.py --apply           # actually write files
    python3 scripts/extract_tests.py --file path.rs    # process single file (dry-run)
    python3 scripts/extract_tests.py --file path.rs --apply
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path


@dataclass
class ExtractionResult:
    """Result of extracting a test module from a source file."""

    source_path: Path
    tests_path: Path
    # The new content of the source file (with inline block replaced)
    new_source: str
    # The content of the new tests.rs file
    tests_content: str
    # Line range of the extracted block (1-indexed, inclusive)
    block_start_line: int
    block_end_line: int
    # Total lines in original file
    total_lines: int


class BraceMatchError(Exception):
    pass


def find_matching_brace(source: str, open_pos: int) -> int:
    """Find the matching closing brace for an opening brace at open_pos.

    Properly handles:
    - Line comments (//)
    - Block comments (/* */ including nested)
    - String literals ("..." with escapes)
    - Raw string literals (r#"..."#, r##"..."##, etc.)
    - Byte string literals (b"...", br#"..."#)
    - Character literals ('...' with escapes)
    """
    assert source[open_pos] == "{", f"Expected '{{' at position {open_pos}, got {source[open_pos]!r}"

    depth = 1
    i = open_pos + 1
    length = len(source)

    while i < length and depth > 0:
        ch = source[i]

        # Line comment
        if ch == "/" and i + 1 < length and source[i + 1] == "/":
            # Skip to end of line
            newline = source.find("\n", i)
            if newline == -1:
                i = length
            else:
                i = newline + 1
            continue

        # Block comment (supports nesting)
        if ch == "/" and i + 1 < length and source[i + 1] == "*":
            i += 2
            comment_depth = 1
            while i < length and comment_depth > 0:
                if source[i] == "/" and i + 1 < length and source[i + 1] == "*":
                    comment_depth += 1
                    i += 2
                elif source[i] == "*" and i + 1 < length and source[i + 1] == "/":
                    comment_depth -= 1
                    i += 2
                else:
                    i += 1
            continue

        # Raw string literal: r#"..."#, r##"..."##, br#"..."#, etc.
        if ch in ("r", "b"):
            raw_start = i
            ri = i
            # Skip optional 'b' prefix
            if source[ri] == "b" and ri + 1 < length and source[ri + 1] == "r":
                ri += 1
            if source[ri] == "r":
                # Count hashes
                ri += 1
                num_hashes = 0
                while ri < length and source[ri] == "#":
                    num_hashes += 1
                    ri += 1
                if ri < length and source[ri] == '"':
                    # This is a raw string literal
                    ri += 1  # skip opening quote
                    # Find closing: "###
                    closing = '"' + "#" * num_hashes
                    end = source.find(closing, ri)
                    if end == -1:
                        raise BraceMatchError(
                            f"Unterminated raw string literal starting at position {raw_start}"
                        )
                    i = end + len(closing)
                    continue
            # Not a raw string, fall through

        # Regular string literal
        if ch == '"':
            i += 1
            while i < length and source[i] != '"':
                if source[i] == "\\":
                    i += 2  # skip escape sequence
                else:
                    i += 1
            i += 1  # skip closing quote
            continue

        # Character literal
        if ch == "'":
            # Distinguish char literals from lifetime annotations.
            # A char literal: 'x', '\\n', '\\x41', '\\u{1F600}'
            # A lifetime: 'a, 'static, '_ (followed by an identifier char)
            # We need to check if this looks like a char literal.
            if i + 1 < length:
                next_ch = source[i + 1]
                if next_ch == "\\":
                    # Escape sequence in char literal: '\n', '\x41', etc.
                    i += 2  # skip ' and \
                    # Skip the escaped char(s)
                    while i < length and source[i] != "'":
                        i += 1
                    i += 1  # skip closing '
                    continue
                elif i + 2 < length and source[i + 2] == "'":
                    # Simple char literal: 'x'
                    i += 3
                    continue
            # Otherwise it's a lifetime or label, just skip the apostrophe
            i += 1
            continue

        # Braces
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return i

        i += 1

    raise BraceMatchError(f"No matching closing brace found (started at position {open_pos})")


# Pattern to match the start of a cfg(test) module block.
# Captures: optional attributes between #[cfg(test)] and mod tests,
# and the mod tests { opening.
CFG_TEST_MOD_RE = re.compile(
    r"^(?P<attrs>"  # start capturing attribute group
    r"#\[cfg\(test\)\]\s*\n"  # #[cfg(test)]
    r"(?:#\[(?:allow|deny|warn|expect)\([^\]]*\)\]\s*\n)*"  # optional #[allow/deny/warn/expect(...)]
    r")"  # end attribute group
    r"mod\s+tests\s*\{",  # mod tests {
    re.MULTILINE,
)

# Pattern for already-external test module declaration
EXTERN_MOD_RE = re.compile(
    r"#\[cfg\(test\)\]\s*\n\s*mod\s+tests\s*;",
    re.MULTILINE,
)


def dedent_test_content(content: str) -> str:
    """Remove one level of indentation (4 spaces) from test content."""
    lines = content.split("\n")
    dedented = []
    for line in lines:
        if line.startswith("    "):
            dedented.append(line[4:])
        elif line.strip() == "":
            dedented.append("")
        else:
            # Line isn't indented by 4 — keep as-is
            dedented.append(line)
    return "\n".join(dedented)


def extract_test_module(source_path: Path) -> ExtractionResult | None:
    """Extract the inline test module from a source file.

    Returns None if:
    - File has no inline test module
    - File already uses external test module (mod tests;)
    - A tests.rs sibling already exists
    """
    source = source_path.read_text(encoding="utf-8")

    # Skip if already using external test module
    if EXTERN_MOD_RE.search(source):
        return None

    # Find inline test module
    match = CFG_TEST_MOD_RE.search(source)
    if match is None:
        return None

    # Determine where the tests.rs file should go.
    # Rust module resolution rules:
    #   lib.rs / main.rs (crate roots): mod tests; → src/tests.rs (same dir)
    #   mod.rs:                         mod tests; → parent_dir/tests.rs (same dir)
    #   foo.rs:                         mod tests; → foo/tests.rs (subdir)
    if source_path.name in ("mod.rs", "lib.rs", "main.rs"):
        tests_dir = source_path.parent
    else:
        tests_dir = source_path.parent / source_path.stem

    tests_path = tests_dir / "tests.rs"

    # Skip if tests.rs already exists
    if tests_path.exists():
        return None

    # Find the opening brace of mod tests {
    # The match ends right after the {
    open_brace_pos = match.end() - 1
    assert source[open_brace_pos] == "{"

    # Find matching closing brace
    try:
        close_brace_pos = find_matching_brace(source, open_brace_pos)
    except BraceMatchError as e:
        print(f"  WARNING: {source_path}: {e}", file=sys.stderr)
        return None

    # Calculate line numbers (1-indexed)
    block_start_line = source[:match.start()].count("\n") + 1
    block_end_line = source[: close_brace_pos + 1].count("\n") + 1
    total_lines = source.count("\n") + 1

    # Extract the inner content (between { and })
    inner_content = source[open_brace_pos + 1 : close_brace_pos]

    # Strip leading/trailing blank lines from inner content
    inner_content = inner_content.strip("\n")

    # Dedent by one level (4 spaces)
    tests_content = dedent_test_content(inner_content)

    # Ensure file ends with a single newline
    tests_content = tests_content.rstrip("\n") + "\n"

    # Build the attributes to preserve on the declaration.
    # We keep #[cfg(test)] but also any #[allow(...)] etc.
    attrs_text = match.group("attrs")

    # Build replacement: preserved attributes + mod tests;
    # Strip trailing whitespace/newlines from attrs, then add the declaration
    attrs_lines = attrs_text.rstrip().split("\n")
    replacement = "\n".join(attrs_lines) + "\nmod tests;\n"

    # Build new source content
    # Find the start of the block (including any blank line before #[cfg(test)])
    block_start = match.start()

    # Check if there's a blank line before the block and preserve it
    before = source[:block_start]
    after = source[close_brace_pos + 1 :]

    # Remove trailing whitespace from before, add single blank line, then replacement
    before = before.rstrip("\n")
    if before:
        before += "\n\n"

    # Remove leading whitespace from after
    after = after.lstrip("\n")
    if after:
        after = "\n" + after

    new_source = before + replacement + after

    # Ensure file ends with newline
    if not new_source.endswith("\n"):
        new_source += "\n"

    return ExtractionResult(
        source_path=source_path,
        tests_path=tests_path,
        new_source=new_source,
        tests_content=tests_content,
        block_start_line=block_start_line,
        block_end_line=block_end_line,
        total_lines=total_lines,
    )


def find_rust_files(root: Path) -> list[Path]:
    """Find all .rs files under root, excluding target/ and tests.rs files."""
    rs_files = []
    for dirpath, dirnames, filenames in os.walk(root):
        # Skip target directories and hidden directories
        dirnames[:] = [
            d for d in dirnames if d != "target" and not d.startswith(".")
        ]
        for f in filenames:
            if f.endswith(".rs") and f != "tests.rs":
                rs_files.append(Path(dirpath) / f)
    rs_files.sort()
    return rs_files


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Extract inline test modules to sibling tests.rs files"
    )
    parser.add_argument(
        "--apply",
        action="store_true",
        help="Actually write files (default is dry-run)",
    )
    parser.add_argument(
        "--file",
        type=Path,
        help="Process a single file instead of the whole codebase",
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path("compiler"),
        help="Root directory to search (default: compiler/)",
    )
    parser.add_argument(
        "--verbose",
        "-v",
        action="store_true",
        help="Show detailed output for each file",
    )
    args = parser.parse_args()

    mode = "APPLY" if args.apply else "DRY-RUN"
    print(f"[{mode}] Extract inline test modules to sibling files\n")

    if args.file:
        files = [args.file.resolve()]
    else:
        files = find_rust_files(args.root.resolve())

    results: list[ExtractionResult] = []
    skipped = 0
    errors = 0

    for path in files:
        try:
            result = extract_test_module(path)
        except Exception as e:
            print(f"  ERROR: {path}: {e}", file=sys.stderr)
            errors += 1
            continue

        if result is None:
            skipped += 1
            continue

        results.append(result)

        test_lines = result.block_end_line - result.block_start_line + 1
        pct = test_lines * 100 // result.total_lines
        rel_source = result.source_path.relative_to(Path.cwd()) if Path.cwd() in result.source_path.parents or Path.cwd() == result.source_path.parent else result.source_path
        rel_tests = result.tests_path.relative_to(Path.cwd()) if Path.cwd() in result.tests_path.parents or Path.cwd() == result.tests_path.parent else result.tests_path

        print(f"  {rel_source}")
        print(f"    → {rel_tests}")
        print(f"    lines {result.block_start_line}-{result.block_end_line} ({test_lines} lines, {pct}% of file)")

        if args.verbose:
            print(f"    first 3 lines of tests.rs:")
            for line in result.tests_content.split("\n")[:3]:
                print(f"      | {line}")
            print()

    print(f"\n{'─' * 50}")
    print(f"  Files to extract: {len(results)}")
    print(f"  Files skipped:    {skipped}")
    print(f"  Errors:           {errors}")
    print(f"{'─' * 50}")

    if not args.apply:
        print(f"\n  This was a dry run. Use --apply to write files.")
        return 0

    # Apply changes
    print(f"\n  Applying changes...")
    applied = 0
    for result in results:
        # Create directory if needed
        result.tests_path.parent.mkdir(parents=True, exist_ok=True)

        # Write the tests.rs file
        result.tests_path.write_text(result.tests_content, encoding="utf-8")

        # Rewrite the source file
        result.source_path.write_text(result.new_source, encoding="utf-8")

        applied += 1

    print(f"  Applied {applied} extractions successfully.")

    # Handle foo.rs → foo/mod.rs renames needed
    # When we extract from foo.rs (not mod.rs), we created foo/tests.rs.
    # Rust 2018+ supports foo.rs + foo/ directory, so mod tests; in foo.rs
    # will look for foo/tests.rs. No rename needed.

    return 0


if __name__ == "__main__":
    sys.exit(main())
