#!/usr/bin/env python3
"""
retrofit-subsection-closeout.py

Retrofits existing plan section files to include /tpr-review and
/impl-hygiene-review checkboxes in every Subsection close-out block,
per the updated plan-schema.md.

Two close-out styles exist in the wild:

  OLD-STYLE — entire close-out is a single checklist item with
  /improve-tooling instructions embedded inline:

    - [ ] **Subsection close-out (10.1)** — MANDATORY ... Run `/improve-tooling` ...

  Injection: insert /tpr-review and /impl-hygiene-review as separate
  top-level checklist items immediately BEFORE this close-out line.

  NEW-STYLE — close-out header followed by indented sub-items:

    - [ ] **Subsection close-out ({NN}.1)** — MANDATORY ...:
      - [ ] All tasks above are `[x]` ...
      - [ ] Update this subsection's `status` ...
      - [ ] `/improve-tooling` retrospective ...

  Injection: insert /tpr-review and /impl-hygiene-review sub-items
  immediately BEFORE the /improve-tooling sub-item.

Both styles are idempotent: if the items already exist, no change is made.
Completed blocks ([x]) are also checked and retrofitted if missing.

Usage:
    python scripts/retrofit-subsection-closeout.py [--dry-run] [path]
    python scripts/retrofit-subsection-closeout.py --dry-run plans/
    python scripts/retrofit-subsection-closeout.py plans/roadmap/section-10.md

Options:
    --dry-run   Preview changes without writing files (exit 0 always)
    path        File or directory to scan (default: plans/)
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path

# ── Patterns ──────────────────────────────────────────────────────────────────

# Start of any Subsection close-out block (completed or not)
RE_CLOSEOUT = re.compile(r'^- \[[ x]\] \*\*Subsection close-out')

# Inline /improve-tooling in old-style single-line close-outs
RE_INLINE_IMPROVE = re.compile(r'`/improve-tooling`', re.IGNORECASE)

# Sub-item with /improve-tooling (new-style multi-item close-outs)
RE_SUBITEM_IMPROVE = re.compile(
    r'^\s{2,}- \[[ x]\] \*{0,2}(?:Run )?`/improve-tooling`',
    re.IGNORECASE,
)

# /tpr-review passed item (top-level or sub-item, any checkbox state)
RE_TPR = re.compile(
    r'^(?:\s+)?- \[[ x]\] [`]?/tpr-review[`]? passed',
    re.IGNORECASE,
)

# /impl-hygiene-review passed item (top-level or sub-item, any checkbox state)
RE_HYGIENE = re.compile(
    r'^(?:\s+)?- \[[ x]\] [`]?/impl-hygiene-review[`]? passed',
    re.IGNORECASE,
)

# Top-level markdown boundaries (section headers, rules, code fences)
RE_TOPLEVEL_END = re.compile(r'^(?:#{1,6} |---|```|> )')

# ── Injection text ────────────────────────────────────────────────────────────

def tpr_line(indent: str) -> str:
    return (
        f"{indent}- [ ] `/tpr-review` passed — independent review found no "
        f"critical or major issues (or all findings triaged)\n"
    )

def hygiene_line(indent: str) -> str:
    return (
        f"{indent}- [ ] `/impl-hygiene-review` passed — hygiene review clean. "
        f"MUST run AFTER `/tpr-review` is clean.\n"
    )

# ── Data ──────────────────────────────────────────────────────────────────────

@dataclass
class Injection:
    file: Path
    closeout_line: int   # 1-based line number of the Subsection close-out
    style: str           # "old" or "new"
    inject_before: int   # 0-based line index to insert before
    inject_indent: str   # indentation prefix for injected lines
    missing_tpr: bool
    missing_hygiene: bool

    @property
    def missing_labels(self) -> str:
        parts = []
        if self.missing_tpr:
            parts.append('/tpr-review')
        if self.missing_hygiene:
            parts.append('/impl-hygiene-review')
        return ' + '.join(parts)

# ── Core detection ────────────────────────────────────────────────────────────

def _scan_old_style(
    lines: list[str],
    closeout_idx: int,
    path: Path,
) -> Injection | None:
    """
    Old-style: single-line close-out with /improve-tooling inline.
    Inject /tpr-review + /impl-hygiene-review as top-level items before
    the close-out line, unless they already appear in the surrounding 6 lines.
    """
    # Check the 6 lines immediately before this close-out for existing items
    window_start = max(0, closeout_idx - 6)
    window = lines[window_start:closeout_idx]
    has_tpr = any(RE_TPR.match(l) for l in window)
    has_hygiene = any(RE_HYGIENE.match(l) for l in window)

    if has_tpr and has_hygiene:
        return None  # Already retrofitted

    return Injection(
        file=path,
        closeout_line=closeout_idx + 1,
        style='old',
        inject_before=closeout_idx,
        inject_indent='',           # top-level: no indentation
        missing_tpr=not has_tpr,
        missing_hygiene=not has_hygiene,
    )


def _scan_new_style(
    lines: list[str],
    closeout_idx: int,
    path: Path,
) -> Injection | None:
    """
    New-style: multi-item close-out with indented sub-items.
    Inject /tpr-review + /impl-hygiene-review before the /improve-tooling
    sub-item, unless they already appear in the block.
    """
    has_tpr = False
    has_hygiene = False
    improve_idx: int | None = None

    j = closeout_idx + 1
    while j < len(lines):
        sub = lines[j]

        # Block ends at a top-level boundary or another top-level checklist item
        if sub.strip() and RE_TOPLEVEL_END.match(sub):
            break
        if RE_CLOSEOUT.match(sub):
            break
        # Blank line followed by non-indented line also ends the block
        if not sub.strip() and j + 1 < len(lines):
            nxt = lines[j + 1]
            if nxt.strip() and nxt[:1] not in (' ', '\t'):
                break

        if RE_TPR.match(sub):
            has_tpr = True
        elif RE_HYGIENE.match(sub):
            has_hygiene = True
        elif RE_SUBITEM_IMPROVE.match(sub) and improve_idx is None:
            improve_idx = j

        j += 1

    if improve_idx is None:
        return None  # Couldn't find /improve-tooling sub-item; skip

    if has_tpr and has_hygiene:
        return None  # Already retrofitted

    # Derive indentation from the /improve-tooling sub-item
    improve_line = lines[improve_idx]
    indent_len = len(improve_line) - len(improve_line.lstrip())
    indent = ' ' * indent_len

    return Injection(
        file=path,
        closeout_line=closeout_idx + 1,
        style='new',
        inject_before=improve_idx,
        inject_indent=indent,
        missing_tpr=not has_tpr,
        missing_hygiene=not has_hygiene,
    )


def find_injections(path: Path) -> list[Injection]:
    """Scan a file for Subsection close-out blocks that need retrofitting."""
    try:
        text = path.read_text(encoding='utf-8')
    except (OSError, UnicodeDecodeError) as e:
        print(f'  warning: could not read {path}: {e}', file=sys.stderr)
        return []

    lines = text.splitlines(keepends=True)
    injections: list[Injection] = []
    i = 0

    while i < len(lines):
        if not RE_CLOSEOUT.match(lines[i]):
            i += 1
            continue

        closeout_idx = i

        if RE_INLINE_IMPROVE.search(lines[i]):
            # Old-style: /improve-tooling is embedded in the close-out line itself
            inj = _scan_old_style(lines, closeout_idx, path)
            if inj:
                injections.append(inj)
            i += 1
        else:
            # New-style: close-out header with sub-items following
            inj = _scan_new_style(lines, closeout_idx, path)
            if inj:
                injections.append(inj)
            # Skip past the whole block (advance to the /improve-tooling line or end)
            if inj:
                i = inj.inject_before + 1
            else:
                i += 1

    return injections

# ── Application ───────────────────────────────────────────────────────────────

def apply_injections(path: Path, injections: list[Injection]) -> None:
    """Write retrofitted file. Injections applied back-to-front to preserve indices."""
    lines = path.read_text(encoding='utf-8').splitlines(keepends=True)

    for inj in sorted(injections, key=lambda x: x.inject_before, reverse=True):
        idx = inj.inject_before
        to_insert: list[str] = []
        if inj.missing_tpr:
            to_insert.append(tpr_line(inj.inject_indent))
        if inj.missing_hygiene:
            to_insert.append(hygiene_line(inj.inject_indent))
        lines[idx:idx] = to_insert

    path.write_text(''.join(lines), encoding='utf-8')

# ── File collection ───────────────────────────────────────────────────────────

SKIP_NAMES = frozenset({'README.md', 'MEMORY.md', 'plan-schema.md', 'SKILL.md', 'index.md'})

def collect_files(root: Path) -> list[Path]:
    return sorted(p for p in root.rglob('*.md') if p.name not in SKIP_NAMES)

# ── CLI ───────────────────────────────────────────────────────────────────────

def main() -> int:
    parser = argparse.ArgumentParser(
        description='Add /tpr-review + /impl-hygiene-review to subsection close-out blocks',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument('--dry-run', action='store_true',
                        help='Preview changes without writing files')
    parser.add_argument('path', nargs='?',
                        help='File or directory to scan (default: plans/)')
    args = parser.parse_args()

    script_dir = Path(__file__).resolve().parent
    project_root = script_dir.parent
    default_path = project_root / 'plans'

    root = Path(args.path).resolve() if args.path else default_path
    if not root.exists():
        print(f'error: path not found: {root}', file=sys.stderr)
        return 1

    files = [root] if root.is_file() else collect_files(root)
    if not files:
        print('No .md files found.')
        return 0

    all_injections: dict[Path, list[Injection]] = {}
    for path in files:
        injs = find_injections(path)
        if injs:
            all_injections[path] = injs

    total = sum(len(v) for v in all_injections.values())

    if not all_injections:
        print('✓ All subsection close-out blocks already have /tpr-review + /impl-hygiene-review.')
        return 0

    mode = '[DRY RUN] ' if args.dry_run else ''
    print(f'{mode}{total} close-out block(s) in {len(all_injections)} file(s) need retrofitting:\n')

    for path in sorted(all_injections):
        injs = all_injections[path]
        try:
            rel = path.relative_to(project_root)
        except ValueError:
            rel = path
        print(f'  {rel}  ({len(injs)} block(s), style={injs[0].style})')
        for inj in injs:
            print(f'    line {inj.closeout_line} [{inj.style}]: insert {inj.missing_labels}')
            header = lines_preview(path, inj.closeout_line - 1)
            print(f'      {header}')
        print()

    if args.dry_run:
        print(f'Re-run without --dry-run to apply {total} injection(s).')
    else:
        for path, injs in all_injections.items():
            apply_injections(path, injs)
        print(f'Done — retrofitted {total} block(s) in {len(all_injections)} file(s).')

    return 0


def lines_preview(path: Path, idx: int, max_len: int = 80) -> str:
    """Return a truncated preview of line idx (0-based) from path."""
    try:
        lines = path.read_text(encoding='utf-8').splitlines()
        text = lines[idx].strip() if idx < len(lines) else ''
        return (text[:max_len] + '…') if len(text) > max_len else text
    except Exception:
        return '(unreadable)'


if __name__ == '__main__':
    sys.exit(main())
