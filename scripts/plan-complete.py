#!/usr/bin/env python3
"""Mark a plan section complete — flip checkboxes, sync statuses, update parent files.

Replaces the manual ceremony of:
  1. sed/python to flip `- [ ]` → `- [x]` on N checkboxes
  2. python to update each subsection status in frontmatter
  3. python to update section status in frontmatter
  4. python to update overview Quick Reference table
  5. python to update index.md status line

Usage:
  python scripts/plan-complete.py plans/foo/section-02-bar.md [--dry-run]
  python scripts/plan-complete.py plans/foo/section-02-bar.md --check-only
  python scripts/plan-complete.py plans/foo/section-02-bar.md --subsection 02.3

Options:
  --dry-run      Show what would change without writing files
  --check-only   Report unchecked items and status mismatches; exit 1 if any
  --subsection X Only complete subsection X (e.g., 02.3); leave others unchanged
  --no-parent    Skip overview/index updates
  --force        Complete even if unchecked items exist (checks them all off)
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


def _read(p: Path) -> str:
    return p.read_text(encoding="utf-8")


def _write(p: Path, content: str, dry_run: bool) -> bool:
    if dry_run:
        return True
    p.write_text(content, encoding="utf-8")
    return True


def flip_checkboxes(text: str, subsection_filter: str | None = None) -> tuple[str, int]:
    """Flip all `- [ ]` to `- [x]`. If subsection_filter is set, only flip
    within that subsection's body (between its ## header and the next ##)."""
    if subsection_filter is None:
        count = text.count("- [ ]")
        return text.replace("- [ ]", "- [x]"), count

    lines = text.split("\n")
    in_section = False
    section_re = re.compile(rf"^##\s+{re.escape(subsection_filter)}\b")
    next_section_re = re.compile(r"^##\s+")
    count = 0
    out = []
    for line in lines:
        if section_re.match(line):
            in_section = True
        elif in_section and next_section_re.match(line):
            in_section = False
        if in_section and "- [ ]" in line:
            line = line.replace("- [ ]", "- [x]", 1)
            count += 1
        out.append(line)
    return "\n".join(out), count


def sync_subsection_statuses(text: str) -> tuple[str, list[str]]:
    """Update each subsection's status in the YAML frontmatter sections list
    based on checkbox counts in the body."""
    changes: list[str] = []

    # Find all subsection IDs in frontmatter
    fm_match = re.search(r"^sections:\s*\n((?:\s+-\s+.*\n)*)", text, re.MULTILINE)
    if not fm_match:
        return text, changes

    section_ids: list[str] = []
    for m in re.finditer(r'id:\s*"([^"]+)"', fm_match.group(0)):
        section_ids.append(m.group(1))

    for sid in section_ids:
        # Count checkboxes under this subsection's body header
        header_re = re.compile(rf"^##\s+{re.escape(sid)}\b", re.MULTILINE)
        hm = header_re.search(text)
        if not hm:
            continue
        # Find next ## or end of file
        next_hm = re.search(r"^##\s+", text[hm.end():], re.MULTILINE)
        if next_hm:
            body = text[hm.end():hm.end() + next_hm.start()]
        else:
            body = text[hm.end():]
        checked = body.count("- [x]")
        unchecked = body.count("- [ ]")
        total = checked + unchecked
        if total == 0:
            continue
        if unchecked == 0:
            new_status = "complete"
        elif checked > 0:
            new_status = "in-progress"
        else:
            new_status = "not-started"

        # Update in frontmatter
        # Match the subsection entry block
        entry_re = re.compile(
            rf'(- id: "{re.escape(sid)}"\n\s+title: "[^"]*"\n\s+status: )(\S+)',
        )
        em = entry_re.search(text)
        if em and em.group(2) != new_status:
            text = text[:em.start(2)] + new_status + text[em.end(2):]
            changes.append(f"  {sid}: {em.group(2)} -> {new_status}")

    return text, changes


def sync_section_status(text: str) -> tuple[str, str | None]:
    """Update the top-level section status based on subsection statuses."""
    # Extract all subsection statuses from frontmatter
    statuses = re.findall(r'status:\s*(not-started|in-progress|complete)', text)
    if len(statuses) < 2:
        return text, None

    # First status is the section-level one; rest are subsections
    section_status = statuses[0]
    sub_statuses = statuses[1:]

    if all(s == "complete" for s in sub_statuses):
        new_status = "complete"
    elif any(s in ("in-progress", "complete") for s in sub_statuses):
        new_status = "in-progress"
    else:
        new_status = "not-started"

    if section_status == new_status:
        return text, None

    # Replace the FIRST status: line (which is the section-level one)
    text = re.sub(
        r"^(status:\s*)" + re.escape(section_status),
        rf"\g<1>{new_status}",
        text,
        count=1,
        flags=re.MULTILINE,
    )
    return text, f"section status: {section_status} -> {new_status}"


def update_parent_file(parent: Path, section_id: str, new_status: str, dry_run: bool) -> list[str]:
    """Update a parent file (overview or index) status for the given section."""
    if not parent.exists():
        return []
    text = _read(parent)
    changes: list[str] = []

    # Quick Reference table: | 02 | Title | file | Status |
    # Match row with the section ID
    row_re = re.compile(
        rf"(\|\s*{re.escape(section_id)}\s*\|[^|]*\|[^|]*\|\s*)(Not Started|In Progress|Complete)(\s*\|)",
        re.IGNORECASE,
    )
    m = row_re.search(text)
    if m and m.group(2).lower().replace(" ", "-") != new_status:
        display = {"complete": "Complete", "in-progress": "In Progress", "not-started": "Not Started"}[new_status]
        text = text[:m.start(2)] + display + text[m.end(2):]
        changes.append(f"  {parent.name}: table row {section_id} -> {display}")

    # index.md style: **Status:** Not Started
    idx_re = re.compile(
        rf"(section-{re.escape(section_id)}[^|]*\|\s*\*\*Status:\*\*\s*)(Not Started|In Progress|Complete)",
        re.IGNORECASE,
    )
    m = idx_re.search(text)
    if m and m.group(2).lower().replace(" ", "-") != new_status:
        display = {"complete": "Complete", "in-progress": "In Progress", "not-started": "Not Started"}[new_status]
        text = text[:m.start(2)] + display + text[m.end(2):]
        changes.append(f"  {parent.name}: status line -> {display}")

    if changes:
        _write(parent, text, dry_run)
    return changes


def force_all_complete(text: str) -> tuple[str, list[str]]:
    """Force every subsection status in frontmatter to 'complete'."""
    changes: list[str] = []
    for m in re.finditer(r'(- id: "([^"]+)"\n\s+title: "[^"]*"\n\s+status: )(\S+)', text):
        if m.group(3) != "complete":
            text = text[:m.start(3)] + "complete" + text[m.end(3):]
            changes.append(f"  {m.group(2)}: {m.group(3)} -> complete")
    return text, changes


def force_section_complete(text: str) -> tuple[str, str | None]:
    """Force the section-level status to 'complete'."""
    m = re.search(r"^(status:\s*)(\S+)", text, re.MULTILINE)
    if m and m.group(2) != "complete":
        old = m.group(2)
        text = text[:m.start(2)] + "complete" + text[m.end(2):]
        return text, f"section status: {old} -> complete"
    return text, None


def find_parent_files(section_file: Path) -> list[Path]:
    """Find 00-overview.md and index.md in the same plan directory."""
    plan_dir = section_file.parent
    parents = []
    for name in ("00-overview.md", "index.md"):
        p = plan_dir / name
        if p.exists():
            parents.append(p)
    return parents


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Mark a plan section complete — flip checkboxes, sync statuses, update parents.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument("section_file", type=Path, help="Path to the section .md file")
    parser.add_argument("--dry-run", action="store_true", help="Show changes without writing")
    parser.add_argument("--check-only", action="store_true", help="Report status; exit 1 if incomplete")
    parser.add_argument("--subsection", type=str, default=None, help="Only complete this subsection ID")
    parser.add_argument("--no-parent", action="store_true", help="Skip overview/index updates")
    parser.add_argument("--force", action="store_true", help="Check off all remaining boxes even if unchecked")
    parser.add_argument("--complete-all", action="store_true",
                        help="Force everything complete: check all boxes, set all subsection + section status to complete, update parents. The 'I finished, mark it done' workflow.")

    args = parser.parse_args()
    if args.complete_all:
        args.force = True

    if not args.section_file.exists():
        print(f"error: {args.section_file} not found", file=sys.stderr)
        return 1

    text = _read(args.section_file)
    unchecked = text.count("- [ ]")

    if args.check_only:
        checked = text.count("- [x]")
        total = checked + unchecked
        print(f"{args.section_file.name}: {checked}/{total} checked ({unchecked} remaining)")
        if unchecked > 0:
            # Show unchecked items
            for i, line in enumerate(text.split("\n"), 1):
                if "- [ ]" in line:
                    print(f"  L{i}: {line.strip()}")
        return 1 if unchecked > 0 else 0

    all_changes: list[str] = []

    # Step 1: Flip checkboxes
    if unchecked > 0:
        if not args.force and args.subsection is None:
            print(f"warning: {unchecked} unchecked items remain. Use --force to check them all off,")
            print(f"         or --subsection X to complete only one subsection.")
            print(f"         Use --check-only to see what's unchecked.")
            return 1
        text, flip_count = flip_checkboxes(text, args.subsection)
        if flip_count:
            all_changes.append(f"Checked off {flip_count} items" + (f" in {args.subsection}" if args.subsection else ""))

    # Step 2: Sync subsection statuses
    if args.complete_all:
        text, sub_changes = force_all_complete(text)
    else:
        text, sub_changes = sync_subsection_statuses(text)
    all_changes.extend(sub_changes)

    # Step 3: Sync section status
    if args.complete_all:
        text, sec_change = force_section_complete(text)
    else:
        text, sec_change = sync_section_status(text)
    if sec_change:
        all_changes.append(sec_change)

    # Step 4: Write section file
    if all_changes:
        _write(args.section_file, text, args.dry_run)

    # Step 5: Update parent files
    if not args.no_parent:
        # Extract section ID from filename (e.g., section-02-dag-builder.md -> 02)
        fname = args.section_file.name
        sid_match = re.match(r"section-(\d+[A-Za-z]?)", fname)
        if sid_match:
            section_id = sid_match.group(1)
            # Get the new section status
            new_status_match = re.search(r"^status:\s*(\S+)", text, re.MULTILINE)
            new_status = new_status_match.group(1) if new_status_match else "complete"
            for parent in find_parent_files(args.section_file):
                parent_changes = update_parent_file(parent, section_id, new_status, args.dry_run)
                all_changes.extend(parent_changes)

    # Report
    prefix = "[DRY RUN] " if args.dry_run else ""
    if all_changes:
        print(f"{prefix}Changes to {args.section_file.name}:")
        for c in all_changes:
            print(f"  {c}")
    else:
        print(f"{prefix}No changes needed — section already complete.")

    return 0


if __name__ == "__main__":
    sys.exit(main())
