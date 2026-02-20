#!/usr/bin/env python3
"""
Migrate Ori code examples from old run()/match()/try() syntax to block syntax.

Handles both .md files (inside ```ori code blocks) and .ori source files.

Transformations:
1. run(\n    a,\n    b\n) → {\n    a\n    b\n}
2. match(expr,\n    P -> e,\n) → match expr {\n    P -> e\n}
3. try(\n    ...\n) → try {\n    ...\n}
4. loop(run(\n    ...\n)) → loop {\n    ...\n}
5. loop(expr) → loop { expr }  (single-line)
6. unsafe(run(\n    ...\n)) → unsafe {\n    ...\n}
7. for ... do run(\n    ...\n) → for ... do {\n    ...\n}
8. Remove trailing commas from block statements
9. pre_check:/post_check: → flagged for manual review

Usage:
    python scripts/migrate_block_syntax.py [--dry-run] [--verbose] <path>...
    python scripts/migrate_block_syntax.py --dry-run docs/ori_lang/0.1-alpha/spec/
    python scripts/migrate_block_syntax.py .claude/rules/ori-syntax.md
"""

import argparse
import re
import sys
from pathlib import Path


class BlockSyntaxMigrator:
    """Migrates Ori code from run()/match()/try() to block syntax."""

    def __init__(self, verbose=False):
        self.verbose = verbose
        self.stats = {
            "files_processed": 0,
            "files_modified": 0,
            "run_converted": 0,
            "match_converted": 0,
            "try_converted": 0,
            "loop_run_converted": 0,
            "loop_single_converted": 0,
            "unsafe_run_converted": 0,
            "for_do_run_converted": 0,
            "commas_removed": 0,
            "contract_flags": [],
        }

    def migrate_file(self, path: Path) -> str | None:
        """Migrate a file. Returns new content if changed, None if unchanged."""
        content = path.read_text()

        if path.suffix == ".md":
            new_content = self._migrate_markdown(content, path)
        elif path.suffix == ".ori":
            new_content = self._migrate_ori(content, path)
        else:
            return None

        if new_content != content:
            self.stats["files_modified"] += 1
            return new_content
        return None

    def _migrate_markdown(self, content: str, path: Path) -> str:
        """Migrate code blocks inside markdown files."""
        # Find ```ori ... ``` blocks and transform their contents
        def replace_code_block(m):
            prefix = m.group(1)  # ```ori or ```
            code = m.group(2)
            suffix = m.group(3)  # ```
            new_code = self._migrate_ori(code, path)
            return f"{prefix}{new_code}{suffix}"

        # Match ```ori\n...\n``` blocks
        pattern = r'(```ori\n)(.*?)(```)'
        new_content = re.sub(pattern, replace_code_block, content, flags=re.DOTALL)

        # Also match ```\n... blocks that contain Ori-like code (run(, match(, etc.)
        # but only if they look like Ori code
        return new_content

    def _migrate_ori(self, code: str, path: Path) -> str:
        """Apply all transformations to Ori code."""
        # Order matters: compound forms first, then simple forms

        # 1. loop(run(...)) → loop { ... }
        code = self._convert_loop_run(code, path)

        # 2. unsafe(run(...)) → unsafe { ... }
        code = self._convert_unsafe_run(code, path)

        # 3. for ... do run(...) → for ... do { ... }
        code = self._convert_for_do_run(code, path)

        # 4. loop(single_expr) → loop { single_expr }
        # (Must come AFTER loop(run(...)) to avoid double-conversion)
        code = self._convert_loop_single(code, path)

        # 5. match(expr, arms...) → match expr { arms... }
        code = self._convert_match(code, path)

        # 6. try(...) → try { ... }
        code = self._convert_try(code, path)

        # 7. run(...) → { ... }
        # (Must come LAST — other forms contain run() that we handle specially)
        code = self._convert_run(code, path)

        # 8. Flag contracts for manual review
        self._flag_contracts(code, path)

        return code

    def _find_balanced_close(self, text: str, start: int) -> int | None:
        """Find the matching close paren for an open paren at `start`.

        Returns the index of the closing paren, or None if not found.
        Handles nested parens, brackets, braces, and string literals.
        """
        depth = 0
        i = start
        in_string = None  # None, '"', or '`'

        while i < len(text):
            ch = text[i]

            # Handle string literals
            if in_string:
                if ch == '\\' and i + 1 < len(text):
                    i += 2  # skip escape
                    continue
                if ch == in_string:
                    in_string = None
                i += 1
                continue

            if ch in ('"', '`'):
                in_string = ch
                i += 1
                continue

            # Handle nesting
            if ch in ('(', '[', '{'):
                depth += 1
            elif ch in (')', ']', '}'):
                depth -= 1
                if depth == 0:
                    return i

            i += 1

        return None

    def _extract_balanced(self, text: str, open_pos: int) -> tuple[str, int] | None:
        """Extract content between balanced parens starting at open_pos.

        Returns (inner_content, close_pos) or None.
        """
        close = self._find_balanced_close(text, open_pos)
        if close is None:
            return None
        inner = text[open_pos + 1:close]
        return (inner, close)

    def _remove_trailing_commas_from_block(self, block_content: str) -> str:
        """Remove trailing commas from statements in a block.

        Turns:
            let $x = 1,
            let $y = 2,
            x + y
        Into:
            let $x = 1
            let $y = 2
            x + y
        """
        lines = block_content.split('\n')
        new_lines = []
        for line in lines:
            stripped = line.rstrip()
            # Remove trailing comma, but NOT inside nested structures
            # Simple heuristic: remove comma at end of line if it's not inside parens/brackets
            if stripped.endswith(','):
                # Count open/close delimiters to check we're not inside a nested structure
                opens = stripped.count('(') + stripped.count('[') + stripped.count('{')
                closes = stripped.count(')') + stripped.count(']') + stripped.count('}')
                if opens <= closes:
                    # Safe to remove trailing comma
                    stripped = stripped[:-1]
                    self.stats["commas_removed"] += 1
            new_lines.append(stripped + line[len(line.rstrip()):])  # preserve trailing whitespace

        return '\n'.join(new_lines)

    def _convert_run(self, code: str, path: Path) -> str:
        """Convert run(...) → { ... }"""
        result = []
        i = 0

        while i < len(code):
            # Look for 'run(' not preceded by alphanumeric (avoid 'return(', 'rerun(', etc.)
            if code[i:i+4] == 'run(' and (i == 0 or not code[i-1].isalnum()):
                # Check it's not .run( (method call)
                if i > 0 and code[i-1] == '.':
                    result.append(code[i])
                    i += 1
                    continue

                extracted = self._extract_balanced(code, i + 3)
                if extracted:
                    inner, close_pos = extracted
                    # Recursively process nested constructs
                    inner = self._migrate_ori(inner, path)
                    inner = self._remove_trailing_commas_from_block(inner)
                    result.append('{')
                    result.append(inner)
                    result.append('}')
                    i = close_pos + 1
                    self.stats["run_converted"] += 1
                    continue

            result.append(code[i])
            i += 1

        return ''.join(result)

    def _convert_match(self, code: str, path: Path) -> str:
        """Convert match(expr, arm1, arm2) → match expr { arm1 \\n arm2 }"""
        result = []
        i = 0

        while i < len(code):
            # Look for 'match(' not preceded by . (not .match() guard)
            if code[i:i+6] == 'match(' and (i == 0 or code[i-1] not in ('.', '_')):
                # Check it's not already converted (match expr {)
                extracted = self._extract_balanced(code, i + 5)
                if extracted:
                    inner, close_pos = extracted
                    # Split on first comma to separate scrutinee from arms
                    # But must respect nesting
                    scrutinee, rest = self._split_first_arg(inner)
                    if scrutinee is not None and rest is not None:
                        # Recursively process nested constructs in arms
                        rest = self._migrate_ori(rest, path)
                        rest = self._remove_trailing_commas_from_block(rest)
                        result.append('match ')
                        result.append(scrutinee.strip())
                        result.append(' {')
                        result.append(rest)
                        result.append('}')
                        i = close_pos + 1
                        self.stats["match_converted"] += 1
                        continue

            result.append(code[i])
            i += 1

        return ''.join(result)

    def _convert_try(self, code: str, path: Path) -> str:
        """Convert try(...) → try { ... }"""
        result = []
        i = 0

        while i < len(code):
            if code[i:i+4] == 'try(' and (i == 0 or not code[i-1].isalnum()):
                extracted = self._extract_balanced(code, i + 3)
                if extracted:
                    inner, close_pos = extracted
                    # Recursively process nested constructs
                    inner = self._migrate_ori(inner, path)
                    inner = self._remove_trailing_commas_from_block(inner)
                    result.append('try {')
                    result.append(inner)
                    result.append('}')
                    i = close_pos + 1
                    self.stats["try_converted"] += 1
                    continue

            result.append(code[i])
            i += 1

        return ''.join(result)

    def _convert_loop_run(self, code: str, path: Path) -> str:
        """Convert loop(run(...)) → loop { ... }"""
        result = []
        i = 0

        while i < len(code):
            # Match loop(run( or loop( run( with optional whitespace/newline
            if code[i:i+5] == 'loop(' and (i == 0 or not code[i-1].isalnum()):
                # Find what's inside loop(...)
                extracted = self._extract_balanced(code, i + 4)
                if extracted:
                    inner, close_pos = extracted
                    inner_stripped = inner.strip()
                    # Check if inner content is run(...)
                    if inner_stripped.startswith('run(') and inner_stripped.endswith(')'):
                        # Extract the run() contents
                        run_inner = self._extract_balanced(inner_stripped, 3)
                        if run_inner:
                            run_content, _ = run_inner
                            # Recursively process nested constructs
                            run_content = self._migrate_ori(run_content, path)
                            run_content = self._remove_trailing_commas_from_block(run_content)
                            result.append('loop {')
                            result.append(run_content)
                            result.append('}')
                            i = close_pos + 1
                            self.stats["loop_run_converted"] += 1
                            continue

            result.append(code[i])
            i += 1

        return ''.join(result)

    def _convert_loop_single(self, code: str, path: Path) -> str:
        """Convert loop(expr) → loop { expr } for single expressions."""
        result = []
        i = 0

        while i < len(code):
            if code[i:i+5] == 'loop(' and (i == 0 or not code[i-1].isalnum()):
                extracted = self._extract_balanced(code, i + 4)
                if extracted:
                    inner, close_pos = extracted
                    # Recursively process nested constructs
                    inner = self._migrate_ori(inner, path)
                    inner = self._remove_trailing_commas_from_block(inner)
                    result.append('loop {')
                    result.append(inner)
                    result.append('}')
                    i = close_pos + 1
                    self.stats["loop_single_converted"] += 1
                    continue

            result.append(code[i])
            i += 1

        return ''.join(result)

    def _convert_unsafe_run(self, code: str, path: Path) -> str:
        """Convert unsafe(run(...)) → unsafe { ... }"""
        result = []
        i = 0

        while i < len(code):
            if code[i:i+7] == 'unsafe(' and (i == 0 or not code[i-1].isalnum()):
                extracted = self._extract_balanced(code, i + 6)
                if extracted:
                    inner, close_pos = extracted
                    inner_stripped = inner.strip()
                    if inner_stripped.startswith('run(') and inner_stripped.endswith(')'):
                        run_inner = self._extract_balanced(inner_stripped, 3)
                        if run_inner:
                            run_content, _ = run_inner
                            # Recursively process nested constructs
                            run_content = self._migrate_ori(run_content, path)
                            run_content = self._remove_trailing_commas_from_block(run_content)
                            result.append('unsafe {')
                            result.append(run_content)
                            result.append('}')
                            i = close_pos + 1
                            self.stats["unsafe_run_converted"] += 1
                            continue

            result.append(code[i])
            i += 1

        return ''.join(result)

    def _convert_for_do_run(self, code: str, path: Path) -> str:
        """Convert for ... do run(...) → for ... do { ... }"""
        # Match 'do run(' and convert to 'do {'
        result = []
        i = 0

        while i < len(code):
            # Look for 'do run(' pattern
            if code[i:i+7] == 'do run(' or (code[i:i+3] == 'do ' and i + 3 < len(code)
                                              and code[i+3:].lstrip().startswith('run(')):
                # Find the 'run(' part
                run_start = code.index('run(', i)
                extracted = self._extract_balanced(code, run_start + 3)
                if extracted:
                    inner, close_pos = extracted
                    # Recursively process nested constructs
                    inner = self._migrate_ori(inner, path)
                    inner = self._remove_trailing_commas_from_block(inner)
                    result.append('do {')
                    result.append(inner)
                    result.append('}')
                    i = close_pos + 1
                    self.stats["for_do_run_converted"] += 1
                    continue

            result.append(code[i])
            i += 1

        return ''.join(result)

    def _split_first_arg(self, text: str) -> tuple[str | None, str | None]:
        """Split text at the first top-level comma, respecting nesting.

        Returns (first_arg, rest) or (None, None) if no comma found.
        """
        depth = 0
        in_string = None
        i = 0

        while i < len(text):
            ch = text[i]

            if in_string:
                if ch == '\\' and i + 1 < len(text):
                    i += 2
                    continue
                if ch == in_string:
                    in_string = None
                i += 1
                continue

            if ch in ('"', '`'):
                in_string = ch
                i += 1
                continue

            if ch in ('(', '[', '{'):
                depth += 1
            elif ch in (')', ']', '}'):
                depth -= 1

            if ch == ',' and depth == 0:
                return (text[:i], text[i+1:])

            i += 1

        return (None, None)

    def _flag_contracts(self, code: str, path: Path):
        """Flag pre_check:/post_check: for manual review."""
        for i, line in enumerate(code.split('\n'), 1):
            if 'pre_check:' in line or 'post_check:' in line:
                self.stats["contract_flags"].append(
                    f"  {path}:{i}: {line.strip()}"
                )


def collect_files(paths: list[Path], extensions: set[str]) -> list[Path]:
    """Collect all files with given extensions from paths (recursive for dirs)."""
    files = []
    for p in paths:
        if p.is_file() and p.suffix in extensions:
            files.append(p)
        elif p.is_dir():
            for ext in extensions:
                files.extend(sorted(p.rglob(f"*{ext}")))
    return sorted(set(files))


def main():
    parser = argparse.ArgumentParser(
        description="Migrate Ori syntax from run()/match()/try() to block syntax"
    )
    parser.add_argument("paths", nargs="+", type=Path, help="Files or directories to process")
    parser.add_argument("--dry-run", action="store_true", help="Show what would change without writing")
    parser.add_argument("--verbose", "-v", action="store_true", help="Show detailed changes")
    parser.add_argument("--md-only", action="store_true", help="Only process .md files")
    parser.add_argument("--ori-only", action="store_true", help="Only process .ori files")
    args = parser.parse_args()

    extensions = set()
    if args.md_only:
        extensions = {".md"}
    elif args.ori_only:
        extensions = {".ori"}
    else:
        extensions = {".md", ".ori"}

    files = collect_files(args.paths, extensions)
    if not files:
        print("No matching files found.")
        return

    migrator = BlockSyntaxMigrator(verbose=args.verbose)

    for f in files:
        migrator.stats["files_processed"] += 1
        new_content = migrator.migrate_file(f)

        if new_content is not None:
            if args.dry_run:
                print(f"  WOULD MODIFY: {f}")
                if args.verbose:
                    # Show a compact diff
                    old_lines = f.read_text().splitlines()
                    new_lines = new_content.splitlines()
                    for i, (old, new) in enumerate(zip(old_lines, new_lines)):
                        if old != new:
                            print(f"    L{i+1}: {old.strip()}")
                            print(f"      → {new.strip()}")
            else:
                f.write_text(new_content)
                print(f"  MODIFIED: {f}")

    # Print summary
    s = migrator.stats
    print(f"\n--- Migration Summary ---")
    print(f"Files processed: {s['files_processed']}")
    print(f"Files modified:  {s['files_modified']}")
    print(f"Conversions:")
    print(f"  run() → {{}}:           {s['run_converted']}")
    print(f"  match() → match {{}}:   {s['match_converted']}")
    print(f"  try() → try {{}}:       {s['try_converted']}")
    print(f"  loop(run()) → loop {{}}: {s['loop_run_converted']}")
    print(f"  loop(e) → loop {{}}:    {s['loop_single_converted']}")
    print(f"  unsafe(run()) → unsafe {{}}: {s['unsafe_run_converted']}")
    print(f"  for..do run() → for..do {{}}: {s['for_do_run_converted']}")
    print(f"  Trailing commas removed: {s['commas_removed']}")

    if s["contract_flags"]:
        print(f"\n--- MANUAL REVIEW NEEDED: Contracts ({len(s['contract_flags'])} occurrences) ---")
        print("These pre_check:/post_check: references need manual migration to function-level pre()/post():")
        for flag in s["contract_flags"]:
            print(flag)


if __name__ == "__main__":
    main()
