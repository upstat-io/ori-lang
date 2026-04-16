#!/usr/bin/env python3
"""bug_queue_scan.py — fix-next-bug queue scanner.

Replaces the Sonnet sub-agent's mechanical queue scan for /fix-next-bug.
Scans every `plans/bug-tracker/section-NN-*.md` file, collects open
(`- [ ]`) bug entries, filters out lifecycle-excluded entries
(Escalated / Blocked / blocked-by), sorts by priority, and emits JSON
or a human-readable queue display.

Prior behavior (before this scanner): Sonnet sub-agent read all 8
section files (~1,200 lines), did marker filtering by hand, and sorted
bugs through 34 tool calls / 143k tokens / ~3 minutes. This scanner
runs the same work deterministically in ~200 ms.

Priority order:
    severity: critical > high > medium > low
    section:  01 parser/lexer < 02 typeck < 03 eval < 04 codegen/llvm
              < 05 runtime/arc < 06 stdlib < 07 tooling/cli < 08 spec/docs
    ordinal:  lower ordinal first within same section + severity

Lifecycle exclusions (case-insensitive, matched against the entry body):
    - "Escalated to plan:" / "Escalated:"
    - "Blocked:" / "**Blocked**:" / "**Blocked:**"
    - "<!-- blocked-by:"

Resolved bugs (`- [x]`) are skipped entirely; they never enter the
candidate set.

CLI:
    bug_queue_scan.py                     # human-readable queue display
    bug_queue_scan.py --json              # structured JSON for parent
    bug_queue_scan.py --skip-ids ID,ID    # session skip list
    bug_queue_scan.py --bug-tracker DIR   # override default
    bug_queue_scan.py --help              # show this

Exit codes:
    0 — scan completed (queue may be empty or populated)
    1 — bug-tracker directory not found or unreadable
    2 — invalid arguments
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass, asdict, field
from pathlib import Path
from typing import Iterator


SEVERITY_RANK = {"critical": 0, "high": 1, "medium": 2, "low": 3}

SECTION_NAMES = {
    1: "parser-lexer",
    2: "typeck",
    3: "eval",
    4: "codegen-llvm",
    5: "runtime-arc",
    6: "stdlib",
    7: "tooling-cli",
    8: "spec-docs",
}


@dataclass
class Bug:
    bug_id: str
    section: int
    ordinal: int
    severity: str
    severity_raw: str
    title: str
    repro: str | None = None
    subsystem: str | None = None
    excluded_reason: str | None = None
    source_file: str = ""

    def sort_key(self) -> tuple[int, int, int]:
        return (
            SEVERITY_RANK.get(self.severity, 99),
            self.section,
            self.ordinal,
        )


def find_repo_root(start: Path) -> Path:
    p = start.resolve()
    for parent in [p, *p.parents]:
        if (parent / ".git").exists() or (parent / "CLAUDE.md").exists():
            return parent
    return start.resolve()


# Matches the checkbox header line of a bug entry. Captures:
#   checked   : " " or "x"
#   section   : numeric section ("04")
#   ordinal   : numeric ordinal ("074")
#   severity  : raw severity token (may include Unicode arrow "→")
#   title     : title text between the first `**` and the matching `**`
#
# We deliberately allow arbitrary trailing content after the closing
# `**` — entries frequently have attribution like `— found by tpr-review`
# following the bold title, and dropping those bugs (as an over-strict
# `\s*$` anchor did previously) would silently hide high-severity work.
BUG_HEADER_RE = re.compile(
    r"^- \[(?P<checked>[ xX])\] `\[BUG-(?P<section>\d+)-(?P<ordinal>\d+)\]"
    r"\[(?P<severity>[^\]]+)\]` \*\*(?P<title>.+?)\*\*"
)

# Lifecycle marker patterns (case-insensitive, matched against body text).
# We match on line boundaries where possible to avoid accidental substring
# matches like "reblocked:" or narrative prose that mentions "escalated to"
# without the colon form.
ESCALATED_RE = re.compile(r"(?im)^\s*escalated(?:\s+to\s+plan)?\s*:")
BLOCKED_RE = re.compile(r"(?im)^\s*\*{0,2}blocked\*{0,2}\s*:")
BLOCKED_BY_COMMENT_RE = re.compile(r"<!--\s*blocked-by:", re.IGNORECASE)


def normalize_severity(raw: str) -> str:
    """Handle reclassification notation like `critical→medium` — use the
    reclassified (current) severity for sort purposes."""
    s = raw.strip().lower()
    if "→" in s:
        s = s.split("→")[-1].strip()
    # Defensive: strip trailing annotations like "critical (reclassified)"
    s = re.split(r"[\s(]", s, maxsplit=1)[0]
    return s


def parse_section_file(path: Path) -> Iterator[Bug]:
    """Yield Bug entries from one section file, including excluded ones.

    The caller is responsible for filtering by `excluded_reason`.
    """
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as e:
        print(f"warning: cannot read {path}: {e}", file=sys.stderr)
        return

    i = 0
    while i < len(lines):
        line = lines[i]
        m = BUG_HEADER_RE.match(line)
        if not m:
            i += 1
            continue

        checked = m.group("checked").strip().lower() == "x"
        if checked:
            # Resolved bug — skip entirely.
            i += 1
            continue

        section = int(m.group("section"))
        ordinal = int(m.group("ordinal"))
        severity_raw = m.group("severity")
        title = m.group("title")

        # Collect body lines: indented continuations until blank line or next
        # `- [` header. Blank line is the canonical separator between bugs.
        body_lines: list[str] = []
        j = i + 1
        while j < len(lines):
            bl = lines[j]
            if bl.strip() == "":
                break
            if bl.startswith("- ["):
                break
            body_lines.append(bl)
            j += 1

        body_text = "\n".join(body_lines)

        excluded_reason: str | None = None
        if ESCALATED_RE.search(body_text):
            excluded_reason = "Escalated to plan"
        elif BLOCKED_RE.search(body_text):
            excluded_reason = "Blocked"
        elif BLOCKED_BY_COMMENT_RE.search(body_text):
            excluded_reason = "Blocked (cross-section blocker tag)"

        # Extract Repro and Subsystem (first occurrence of each).
        repro: str | None = None
        subsystem: str | None = None
        for bl in body_lines:
            s = bl.strip()
            if repro is None and s.lower().startswith("repro:"):
                repro = s.split(":", 1)[1].strip()
            elif subsystem is None and s.lower().startswith("subsystem:"):
                subsystem = s.split(":", 1)[1].strip()

        yield Bug(
            bug_id=f"BUG-{section:02d}-{ordinal:03d}",
            section=section,
            ordinal=ordinal,
            severity=normalize_severity(severity_raw),
            severity_raw=severity_raw,
            title=title,
            repro=repro,
            subsystem=subsystem,
            excluded_reason=excluded_reason,
            source_file=str(path.name),
        )
        i = j


def scan(bug_tracker_dir: Path, skip_ids: set[str]) -> tuple[list[Bug], list[Bug]]:
    """Scan all section files. Returns (candidates, excluded).

    `candidates` are open bugs eligible for fixing (sorted by priority).
    `excluded` are open bugs filtered out by lifecycle markers or skip list.
    """
    section_files = sorted(bug_tracker_dir.glob("section-*.md"))
    if not section_files:
        print(
            f"error: no section-*.md files found in {bug_tracker_dir}",
            file=sys.stderr,
        )
        sys.exit(1)

    all_bugs: list[Bug] = []
    for f in section_files:
        all_bugs.extend(parse_section_file(f))

    candidates: list[Bug] = []
    excluded: list[Bug] = []
    for b in all_bugs:
        if b.bug_id in skip_ids:
            # Copy with a skip-specific reason so the caller can distinguish.
            b2 = Bug(**{**asdict(b), "excluded_reason": "Session skip"})
            excluded.append(b2)
        elif b.excluded_reason is not None:
            excluded.append(b)
        else:
            candidates.append(b)

    candidates.sort(key=Bug.sort_key)
    return candidates, excluded


def render_text(candidates: list[Bug], excluded: list[Bug]) -> str:
    out: list[str] = []
    if not candidates:
        out.append("Queue empty — no open, fixable bugs in plans/bug-tracker/.")
        if excluded:
            out.append("")
            out.append(f"Excluded ({len(excluded)}):")
            for b in excluded[:10]:
                out.append(
                    f"  - [{b.bug_id}][{b.severity}] {b.title[:60]}... "
                    f"({b.excluded_reason})"
                )
            if len(excluded) > 10:
                out.append(f"  ... and {len(excluded) - 10} more excluded")
        return "\n".join(out) + "\n"

    selected = candidates[0]
    remaining = candidates[1:]

    out.append(f"Selected: [{selected.bug_id}][{selected.severity}] {selected.title}")
    if selected.repro:
        out.append(f"  Repro: {selected.repro}")
    if selected.subsystem:
        out.append(f"  Subsystem: {selected.subsystem}")
    out.append("")

    if not remaining:
        out.append("Remaining queue: (none — this is the last bug)")
    else:
        out.append(f"Remaining queue ({len(remaining)} bugs):")
        preview = remaining[:8]
        for idx, b in enumerate(preview, 1):
            out.append(f"  {idx}. [{b.bug_id}][{b.severity}] {b.title}")
        if len(remaining) > 8:
            out.append(f"  ... and {len(remaining) - 8} more")

    if excluded:
        out.append("")
        out.append(
            f"({len(excluded)} entries excluded by lifecycle markers: "
            "escalated-to-plan, blocked, or session skip)"
        )
    return "\n".join(out) + "\n"


def render_json(candidates: list[Bug], excluded: list[Bug]) -> str:
    selected = candidates[0] if candidates else None
    remaining = candidates[1:] if candidates else []

    payload = {
        "queue_empty": len(candidates) == 0,
        "total_open_candidates": len(candidates),
        "total_excluded": len(excluded),
        "selected": asdict(selected) if selected else None,
        "remaining": [
            {
                "bug_id": b.bug_id,
                "severity": b.severity,
                "title": b.title,
            }
            for b in remaining
        ],
        "sorted_queue_ids": [b.bug_id for b in candidates],
        "excluded": [
            {
                "bug_id": b.bug_id,
                "severity": b.severity,
                "title": b.title,
                "reason": b.excluded_reason,
            }
            for b in excluded
        ],
    }
    return json.dumps(payload, indent=2) + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="bug_queue_scan.py",
        description=(
            "Scan plans/bug-tracker/ for open bugs, filter lifecycle-"
            "excluded entries, sort by priority, and emit the queue."
        ),
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="emit structured JSON instead of human-readable text",
    )
    parser.add_argument(
        "--skip-ids",
        default="",
        metavar="ID,ID,...",
        help="comma-separated bug IDs to skip this session (re-scan)",
    )
    parser.add_argument(
        "--bug-tracker",
        default=None,
        metavar="DIR",
        help="override default plans/bug-tracker/ directory",
    )
    args = parser.parse_args(argv)

    repo_root = find_repo_root(Path.cwd())
    if args.bug_tracker:
        bug_tracker_dir = Path(args.bug_tracker).resolve()
    else:
        bug_tracker_dir = (repo_root / "plans" / "bug-tracker").resolve()

    if not bug_tracker_dir.is_dir():
        print(
            f"error: bug-tracker directory not found: {bug_tracker_dir}",
            file=sys.stderr,
        )
        return 1

    skip_ids = {
        s.strip() for s in args.skip_ids.split(",") if s.strip()
    } if args.skip_ids else set()

    candidates, excluded = scan(bug_tracker_dir, skip_ids)

    if args.json:
        sys.stdout.write(render_json(candidates, excluded))
    else:
        sys.stdout.write(render_text(candidates, excluded))
    return 0


if __name__ == "__main__":
    sys.exit(main())
