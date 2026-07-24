#!/usr/bin/env python3
"""Fix stale typed-lambda syntax in fixture .ori files.

Stale shape: `let X = (p: T) -> body` (no return type + `=` after `->`)
Current shape (per spec/grammar.ebnf typed_lambda + ori-syntax.md Lambdas):
    `let X = (p: T) -> R = body`

A prior parser change enforced this shape; the fixture corpus was never
updated, producing E1017 / E1005 at every test run that compiles a stale
fixture.

Heuristic for return type R:
  - top-level comparison/logical op (==, !=, <, >, <=, >=, &&, ||, !) → bool
  - method call .contains/.starts_with/.ends_with/.is_empty → bool
  - method call .length/.len → int
  - body starts with `if cond then X else Y` → recurse on X
  - body starts with `(` and contains top-level `,` → tuple, look at elements
  - body starts with `Identifier {` → Identifier (struct constructor)
  - body starts with `{` and ends with `}` → block, default int (manual review flag)
  - body is string literal `"..."` or contains string concat → str
  - default → int

Usage:
    python3 scripts/fix_stale_lambda_fixtures.py [--dry-run] [path...]

Default scans compiler/ori_llvm/tests/aot/fixtures/.
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

# Matches any `(<params with type annotation>) -> <rest>` anywhere on a line.
# The `:` inside parens forces typed-lambda interpretation; untyped lambdas are valid syntax.
# Stale shapes occur in three positions:
#   1. `let X = (p: T) -> body`           (binding)
#   2. `... -> (p: T) -> body`            (returned from outer fn body — function-returning-function)
#   3. `f(arg: (p: T) -> body)`           (callback / lambda as argument)
# Tightened: params must have non-whitespace non-paren content after `:` (excludes
# match-arm variant punning `(val:)` which has empty content after `:`).
STALE_LAMBDA_RE = re.compile(
    r"(?P<prefix>.*?)(?P<paren>\((?P<params>[^()]*?:\s*[^()\s][^()]*?)\)) -> (?P<rest>.+)$"
)

# PascalCase identifier ending the prefix indicates a variant pattern (`Some(x)`, `WithStr(val:)`).
# These appear in match arms, not lambdas. Skip.
PASCAL_PREFIX_RE = re.compile(r"[A-Z]\w*$")

# Function declarations start with `@`. Skip the entire line — function decls already
# require `R = body` syntax; if they were stale, they wouldn't compile (caught elsewhere).
FN_DECL_RE = re.compile(r"^\s*@\w+")


def split_top_level(s: str, delim: str) -> list[str]:
    """Split on a top-level delimiter, ignoring nesting in (), [], {}, strings."""
    out: list[str] = []
    depth = 0
    in_string = False
    in_template = False
    start = 0
    i = 0
    while i < len(s):
        c = s[i]
        if c == '"' and not in_template:
            in_string = not in_string
        elif c == '`':
            in_template = not in_template
        elif not in_string and not in_template:
            if c in "([{":
                depth += 1
            elif c in ")]}":
                depth -= 1
            elif depth == 0 and s[i:i + len(delim)] == delim:
                out.append(s[start:i])
                start = i + len(delim)
                i += len(delim)
                continue
        i += 1
    out.append(s[start:])
    return out


def is_already_typed(rest: str) -> bool:
    """Detect existing `-> R = body` shape: an `=` (not `==`) at top-level before any `;`."""
    depth = 0
    in_string = False
    in_template = False
    i = 0
    while i < len(rest):
        c = rest[i]
        if c == '"' and not in_template:
            in_string = not in_string
            i += 1
            continue
        if c == '`':
            in_template = not in_template
            i += 1
            continue
        if in_string or in_template:
            i += 1
            continue
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
        elif depth == 0 and c == ';':
            return False
        elif depth == 0 and c == '=':
            # Skip == and != and >= and <=
            if i + 1 < len(rest) and rest[i + 1] == '=':
                i += 2
                continue
            if i > 0 and rest[i - 1] in "!<>=":
                i += 1
                continue
            return True
        i += 1
    return False


COMPARISON_OPS = ['==', '!=', '<=', '>=', '<', '>']
LOGICAL_OPS = ['&&', '||']


def has_top_level_op(body: str, ops: list[str]) -> bool:
    """Check if any of ops appears at top level (outside nesting, strings)."""
    depth = 0
    in_string = False
    in_template = False
    i = 0
    while i < len(body):
        c = body[i]
        if c == '"' and not in_template:
            in_string = not in_string
            i += 1
            continue
        if c == '`':
            in_template = not in_template
            i += 1
            continue
        if in_string or in_template:
            i += 1
            continue
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
        elif depth == 0:
            for op in ops:
                if body[i:i + len(op)] == op:
                    return True
        i += 1
    return False


def infer_return_type(body: str) -> tuple[str, bool]:
    """Return (inferred_type, confident). confident=False means manual review recommended."""
    body = body.strip().rstrip(';').strip()
    if not body:
        return ('int', False)
    # Unary not → bool
    if body.startswith('!') and len(body) > 1 and body[1] != '=':
        return ('bool', True)
    # Top-level comparison or logical → bool
    if has_top_level_op(body, COMPARISON_OPS + LOGICAL_OPS):
        return ('bool', True)
    # Method calls that return bool
    if re.search(r'\.(contains|starts_with|ends_with|is_empty|is_some|is_none|is_ok|is_err)\b', body):
        return ('bool', True)
    # Method calls that return int
    if re.search(r'\.(length|len|count)\(\)', body) and not has_top_level_op(body, ['+', '-', '*', '/', '%']):
        return ('int', True)
    # Conditional → look at then-branch
    if body.startswith('if '):
        m = re.match(r'if .+? then (.+?) else .+', body, re.DOTALL)
        if m:
            return infer_return_type(m.group(1))
        return ('int', False)
    # Tuple constructor `(a, b)` at top level — multiple elements separated by top-level `,`
    if body.startswith('('):
        # Strip outer parens and check for top-level commas
        # Find matching paren
        depth = 0
        end = -1
        for i, c in enumerate(body):
            if c == '(':
                depth += 1
            elif c == ')':
                depth -= 1
                if depth == 0:
                    end = i
                    break
        if end == len(body) - 1:
            inner = body[1:end]
            parts = split_top_level(inner, ',')
            if len(parts) >= 2:
                # Tuple — recursively type each part
                sub_types = [infer_return_type(p.strip())[0] for p in parts]
                return (f'({", ".join(sub_types)})', False)  # Manual review: tuple types vary
    # Struct constructor `Name { ... }`
    m = re.match(r'^([A-Z]\w*)\s*\{', body)
    if m:
        return (m.group(1), True)
    # String literal or template
    if body.startswith('"') or body.startswith('`'):
        return ('str', True)
    # Top-level + or other binary on string?
    parts = split_top_level(body, '+')
    if len(parts) > 1:
        for p in parts:
            stripped = p.strip()
            if stripped.startswith('"') or stripped.startswith('`'):
                return ('str', True)
    # Block body — peek at last expression
    if body.startswith('{') and body.endswith('}'):
        return ('int', False)  # Default; manual review recommended
    # Default: assume int (most common for arithmetic / method results on int parameters)
    return ('int', True)


def find_stale_in_segment(segment: str) -> tuple[int, int, str, bool] | None:
    """Search `segment` for the FIRST stale typed lambda. Returns (start, end, replacement, confident)
    or None when no stale lambda exists. start/end are offsets into segment of the substring
    `(params) -> body` (without the body itself — we replace `\\) -> <rest_start>` portion)."""
    # Lambda params must start with a lowercase identifier or `$` or `_` followed by `:`.
    # This excludes:
    #   - Map type annotations: `({str: int})` starts with `{` — skip
    #   - Function type annotations: `((int) -> int)` — handled by `[^()]` exclusion
    #   - Variant punning patterns: `(val:)` — handled by post-match PascalCase prefix check + `:`-must-have-type
    paren_arrow_re = re.compile(r"\(\s*(?P<params>[a-z_$][\w$]*[^()]*?:\s*[^()\s][^()]*?)\) -> ")
    for pm in paren_arrow_re.finditer(segment):
        paren_start = pm.start()
        # Skip variant patterns (PascalCase identifier directly before paren).
        before = segment[:paren_start]
        if PASCAL_PREFIX_RE.search(before):
            continue
        rest_start = pm.end()
        rest = segment[rest_start:]
        if is_already_typed(rest):
            continue
        # Trait-method-declaration heuristic: if segment starts with `@` AND rest has
        # neither `=` nor `;` at any depth, it's a method signature without a body
        # (legal inside `trait X { ... }`). Skip.
        if re.match(r"^\s*@\w+", segment) and '=' not in rest and ';' not in rest:
            continue
        # Stale!
        rtype, confident = infer_return_type(rest)
        # Replacement substring covers `) -> ` segment; we insert ` -> <rtype> = ` there.
        # Specifically: keep `(params)` as-is, replace ` -> ` with ` -> <rtype> = `.
        return (pm.end() - len(" -> "), pm.end(), f" -> {rtype} = ", confident)
    return None


def fix_line(line: str) -> tuple[str | None, bool]:
    """Return (new_line, confident) or (None, True) if no change.

    Handles MULTIPLE stale lambdas per line — e.g., a function decl like
    `@make_adder (n: int) -> (int) -> int = (x: int) -> x + n;`
    where the outer lambda is correctly typed but the body lambda is stale.
    """
    result = line
    confident_all = True
    while True:
        found = find_stale_in_segment(result)
        if found is None:
            break
        start, end, replacement, confident = found
        result = result[:start] + replacement + result[end:]
        confident_all = confident_all and confident
    if result == line:
        return (None, True)
    return (result, confident_all)


def process_file(path: Path, dry_run: bool) -> tuple[int, list[tuple[int, str, bool]]]:
    """Returns (changes_made, [(line_no, new_line, confident_flag), ...])."""
    text = path.read_text()
    lines = text.split('\n')
    changes: list[tuple[int, str, bool]] = []
    new_lines = list(lines)
    for i, line in enumerate(lines):
        new_line, confident = fix_line(line)
        if new_line is not None and new_line != line:
            changes.append((i + 1, new_line, confident))
            new_lines[i] = new_line
    if changes and not dry_run:
        path.write_text('\n'.join(new_lines))
    return (len(changes), changes)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dry-run', action='store_true', help='Show changes without writing')
    parser.add_argument('paths', nargs='*', help='Files or directories (default: fixture dir)')
    args = parser.parse_args()

    if args.paths:
        targets = [Path(p) for p in args.paths]
    else:
        repo_root = Path(__file__).resolve().parent.parent
        targets = [repo_root / 'compiler/ori_llvm/tests/aot/fixtures']

    files_to_process: list[Path] = []
    for t in targets:
        if t.is_file():
            files_to_process.append(t)
        elif t.is_dir():
            files_to_process.extend(t.rglob('*.ori'))

    total_changes = 0
    total_files = 0
    needs_review: list[tuple[Path, int, str]] = []
    for f in sorted(files_to_process):
        n_changes, changes = process_file(f, args.dry_run)
        if n_changes > 0:
            total_files += 1
            total_changes += n_changes
            for line_no, new_line, confident in changes:
                if not confident:
                    needs_review.append((f, line_no, new_line))
            rel = f.relative_to(Path.cwd()) if f.is_absolute() else f
            print(f"[{'DRY' if args.dry_run else 'FIX'}] {rel}: {n_changes} stale lambda(s)")
            for line_no, new_line, confident in changes:
                marker = ' (?)' if not confident else ''
                print(f"    L{line_no}{marker}: {new_line.strip()}")

    print()
    print(f"{'Would update' if args.dry_run else 'Updated'} {total_files} files, {total_changes} stale lambdas.")
    if needs_review:
        print()
        print(f"WARNING: {len(needs_review)} rewrites flagged for manual review (type inference uncertain):")
        for f, line_no, new_line in needs_review:
            rel = f.relative_to(Path.cwd()) if f.is_absolute() else f
            print(f"  {rel}:{line_no}  {new_line.strip()}")
    return 0


if __name__ == '__main__':
    sys.exit(main())
