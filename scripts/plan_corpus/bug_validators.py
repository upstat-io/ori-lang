"""Bidirectional supersede-drift validator + auto-fix planner.

A plan that lists `supersedes: ["plans/bug-tracker/fix-BUG-XX-NNN.md"]` in
its frontmatter MUST have the corresponding bug entry carrying a
`Superseded by: plans/{plan-name}/` line. Conversely, a bug entry carrying
`Superseded by:` MUST point at a plan whose frontmatter declares it as a
supersede target. Either direction missing is `bug_marker_drift` —
detectable here, auto-fixable by inserting the missing marker line into
the bug entry body (the safe direction; never mutate plan frontmatter).

Used by `roadmap_scan.py` to fire the `bug_marker_drift` auto-fix gate
during /continue-roadmap Step 2 cleanup.

## What this module enforces (the §I5 invariant)

The `Superseded by:` lifecycle marker has two endpoints that must agree:

  * **Bug entry side**: `plans/bug-tracker/section-NN-*.md` body line
    `Superseded by: plans/{plan}/`.
  * **Plan side**: `plans/{plan}/00-overview.md` frontmatter
    `supersedes: ["plans/bug-tracker/fix-BUG-XX-NNN.md"]`.

Without bidirectional declaration, supersede routing leaks: bugs without
markers re-trigger `/fix-bug`'s ~170k-token Phase -1 waste; plans without
back-references can't be discovered from the bug tracker.

## What this module does NOT do

It does not auto-fix the plan-frontmatter side. Plan frontmatter is the
canonical SSOT (the supersede declaration originates with plan creation);
mutating it as a downstream auto-fix would invert authority. The auto-fix
direction is always: plan frontmatter (authoritative) → bug entry marker
(derived). If a bug entry has a marker pointing at a plan that doesn't
declare it, that's a `BUG_MARKER_ORPHAN` finding — surfaced for the user
to resolve manually, never auto-edited.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path

from .bug_markers import BugEntry, parse_bug_tracker_dir, BUG_HEADER_RE


# ---------------------------------------------------------------------------
# Drift finding shapes
# ---------------------------------------------------------------------------


@dataclass
class MissingSupersedeMarker:
    """A bug entry that should have a `Superseded by:` marker but doesn't.

    Detected when a plan's frontmatter `supersedes:` list contains the
    path to this bug's fix-section file (`plans/bug-tracker/fix-BUG-XX-NNN.md`),
    but the bug entry body has no `Superseded by:` line.

    Auto-fixable: insert a `  Superseded by: plans/{plan_name}/` line into
    the bug entry body, after the existing context lines but before the
    next blank line / next entry header.
    """
    bug_id: str
    bug_section_file: str       # e.g. "section-04-codegen-llvm.md"
    bug_lineno: int             # 1-based line of the entry header
    plan_name: str              # e.g. "empty-container-typeck-phase-contract"
    plan_overview_path: str     # e.g. "plans/empty-container-typeck-phase-contract/00-overview.md"

    @property
    def bug_section_path(self) -> str:
        return f"plans/bug-tracker/{self.bug_section_file}"

    @property
    def insert_text(self) -> str:
        """The line to insert into the bug entry body."""
        return (
            f"  Superseded by: `plans/{self.plan_name}/` — fix lands when "
            f"that plan completes; do NOT use `/fix-bug` (use "
            f"`/continue-roadmap {self.plan_name}` instead). Auto-inserted "
            f"by /continue-roadmap Step 2 from {self.plan_overview_path} "
            f"`supersedes:` declaration."
        )


@dataclass
class OrphanSupersedeMarker:
    """A bug entry's `Superseded by:` marker points at a plan that doesn't
    claim to supersede it.

    NOT auto-fixable — the marker MAY have been hand-written incorrectly,
    or the plan's frontmatter MAY have been recently changed. Surfaced
    for user review.
    """
    bug_id: str
    bug_section_file: str
    bug_lineno: int
    declared_target: str                # what the bug entry says
    plan_overview_supersedes: list[str] # what the plan actually claims (may be empty)


# ---------------------------------------------------------------------------
# Plan frontmatter scanning (lightweight — uses regex, not full YAML parse)
# ---------------------------------------------------------------------------

# Match a `supersedes:` block in plan frontmatter. The block can be:
#   supersedes: []
#   supersedes:
#     - "plans/bug-tracker/fix-BUG-XX-NNN.md"
#     - "another"
# We extract list entries; bare `[]` returns empty list.
_SUPERSEDES_BLOCK_RE = re.compile(
    r"^supersedes:\s*(?:\[\s*\]|\n((?:[ \t]+-\s+.*\n)+))",
    re.MULTILINE,
)
_SUPERSEDES_ITEM_RE = re.compile(r"^[ \t]+-\s+\"?([^\"\n]+?)\"?\s*$", re.MULTILINE)
# Reference to a fix-section file: captures BUG-XX-NNN
_FIX_BUG_REF_RE = re.compile(
    r"plans/bug-tracker/fix-(BUG-\d{2}-\d{3})\.md",
)


def parse_plan_supersedes(overview_path: Path) -> list[str]:
    """Extract the `supersedes:` list from a plan's `00-overview.md` frontmatter.

    Returns the raw string entries (e.g., `"plans/bug-tracker/fix-BUG-04-074.md"`).
    Empty list if no supersedes field, no items, or unreadable file.
    Lightweight regex-based — does NOT require full YAML parse.
    """
    try:
        text = overview_path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return []
    # Frontmatter is between the first two `---` fences
    parts = text.split("---", 2)
    if len(parts) < 3:
        return []
    fm = parts[1]
    block_m = _SUPERSEDES_BLOCK_RE.search(fm)
    if not block_m:
        return []
    block_body = block_m.group(1) or ""
    return [m.group(1).strip() for m in _SUPERSEDES_ITEM_RE.finditer(block_body)]


# ---------------------------------------------------------------------------
# Bidirectional drift detection
# ---------------------------------------------------------------------------


@dataclass
class SupersedeDriftReport:
    """Result of a corpus-wide supersede-drift scan."""
    missing_markers: list[MissingSupersedeMarker]
    orphan_markers: list[OrphanSupersedeMarker]

    @property
    def total(self) -> int:
        return len(self.missing_markers) + len(self.orphan_markers)


def find_supersede_drift(
    plans_dir: Path,
    bug_tracker_dir: Path | None = None,
) -> SupersedeDriftReport:
    """Scan all plans for `supersedes:` declarations vs bug-entry markers.

    Returns a report containing:
      - `missing_markers`: bug entries lacking a `Superseded by:` line
        even though a plan claims them. Auto-fixable.
      - `orphan_markers`: bug entries whose `Superseded by:` line points at
        a plan that doesn't claim them. Surfaced, not auto-fixed.

    Bug-tracker dir defaults to `plans_dir / "bug-tracker"`.
    """
    if bug_tracker_dir is None:
        bug_tracker_dir = plans_dir / "bug-tracker"

    # Build map: BUG-XX-NNN -> list of plans claiming to supersede it
    claims: dict[str, list[tuple[str, str]]] = {}  # bug_id -> [(plan_name, overview_path)]
    for overview_path in plans_dir.rglob("00-overview.md"):
        # Skip overviews under bug-tracker itself
        try:
            rel = overview_path.relative_to(plans_dir)
        except ValueError:
            continue
        if rel.parts and rel.parts[0] == "bug-tracker":
            continue
        plan_name = overview_path.parent.name
        for entry in parse_plan_supersedes(overview_path):
            for m in _FIX_BUG_REF_RE.finditer(entry):
                bug_id = m.group(1)
                claims.setdefault(bug_id, []).append(
                    (plan_name, str(overview_path.relative_to(plans_dir.parent))),
                )

    # Parse all bug entries
    entries = parse_bug_tracker_dir(bug_tracker_dir)
    by_id: dict[str, BugEntry] = {e.bug_id: e for e in entries}

    missing: list[MissingSupersedeMarker] = []
    orphan: list[OrphanSupersedeMarker] = []

    # Direction A: plan claims bug → bug should have marker
    for bug_id, plan_refs in claims.items():
        entry = by_id.get(bug_id)
        if entry is None:
            # Plan supersedes a bug that doesn't exist in the tracker.
            # That's a different finding (DEAD_REFERENCE) — not our job here.
            continue
        if entry.status == "fixed":
            # Resolved bugs don't need supersede markers; the plan already
            # closed them out.
            continue
        if entry.excluded_reason == "Superseded by plan":
            # Marker is present; check it points at a plan in the claim list
            target = entry.superseded_by or ""
            target_plan_name = target.rstrip("/").rsplit("/", 1)[-1] if target else ""
            if not any(plan_name == target_plan_name for plan_name, _ in plan_refs):
                # Marker points at a different plan than the one(s) claiming.
                # That's an orphan — surface for review.
                orphan.append(OrphanSupersedeMarker(
                    bug_id=bug_id,
                    bug_section_file=entry.source_file,
                    bug_lineno=entry.lineno,
                    declared_target=target,
                    plan_overview_supersedes=[
                        f"plans/{name}/" for name, _ in plan_refs
                    ],
                ))
            # else: marker matches one of the claiming plans — clean.
        else:
            # No marker; pick the first claiming plan (most plans claim
            # only one bug; multi-plan claims are rare and the user can
            # disambiguate manually after auto-fix).
            plan_name, overview_path = plan_refs[0]
            missing.append(MissingSupersedeMarker(
                bug_id=bug_id,
                bug_section_file=entry.source_file,
                bug_lineno=entry.lineno,
                plan_name=plan_name,
                plan_overview_path=overview_path,
            ))

    # Direction B: bug has marker → plan should claim it
    for entry in entries:
        if entry.excluded_reason != "Superseded by plan":
            continue
        if entry.bug_id in claims:
            # Already handled above (matched or orphan)
            continue
        # Bug claims to be superseded but no plan declares it
        target = entry.superseded_by or "(unparseable)"
        orphan.append(OrphanSupersedeMarker(
            bug_id=entry.bug_id,
            bug_section_file=entry.source_file,
            bug_lineno=entry.lineno,
            declared_target=target,
            plan_overview_supersedes=[],  # nobody claims it
        ))

    return SupersedeDriftReport(missing_markers=missing, orphan_markers=orphan)


# ---------------------------------------------------------------------------
# Auto-fix planner — turns missing-marker findings into file edits
# ---------------------------------------------------------------------------


@dataclass
class PlannedEdit:
    """A planned edit to a bug-tracker section file: insert a line after
    the bug entry's existing body lines.

    Consumers (workflow.md Step 2d) apply by:
      1. Read the file
      2. Find the bug entry header at `header_lineno` (1-based)
      3. Walk forward to the last non-blank, non-`- [` body line
      4. Insert `insert_line` immediately after that line

    The insertion is idempotent: if the line is already present (string
    match against the marker portion `Superseded by:`), skip.
    """
    file_path: str          # e.g. "plans/bug-tracker/section-04-codegen-llvm.md"
    bug_id: str
    header_lineno: int      # 1-based line of `- [ ] [BUG-...]` header
    insert_line: str        # the line to insert (without trailing newline)
    rationale: str          # human-readable explanation


def plan_auto_fixes(report: SupersedeDriftReport) -> list[PlannedEdit]:
    """Convert a drift report into a list of `PlannedEdit`s for the
    `missing_markers` half. Orphans are NOT auto-fixed.

    Each edit inserts a `Superseded by:` line into the bug entry body.
    The workflow.md Step 2d consumer applies via the Edit tool.
    """
    edits: list[PlannedEdit] = []
    for finding in report.missing_markers:
        edits.append(PlannedEdit(
            file_path=finding.bug_section_path,
            bug_id=finding.bug_id,
            header_lineno=finding.bug_lineno,
            insert_line=finding.insert_text,
            rationale=(
                f"Plan {finding.plan_overview_path} declares "
                f"`supersedes: [\"plans/bug-tracker/fix-{finding.bug_id}.md\"]` "
                f"but the bug entry has no `Superseded by:` marker. "
                f"Inserting marker for bidirectional discoverability."
            ),
        ))
    return edits


def apply_planned_edits(edits: list[PlannedEdit], repo_root: Path) -> list[str]:
    """Apply a list of `PlannedEdit`s to disk. Returns list of applied
    file paths (relative to repo_root).

    Idempotent: skips edits whose target line already contains
    `Superseded by:` for this bug entry (string-match, no regex).
    """
    applied: list[str] = []
    # Group edits by file to minimize read/writes
    by_file: dict[str, list[PlannedEdit]] = {}
    for e in edits:
        by_file.setdefault(e.file_path, []).append(e)
    for rel_path, file_edits in by_file.items():
        path = repo_root / rel_path
        try:
            text = path.read_text(encoding="utf-8")
        except OSError:
            continue
        lines = text.splitlines()
        # Sort edits by lineno descending so insertions don't shift later edits
        file_edits.sort(key=lambda e: e.header_lineno, reverse=True)
        modified = False
        for edit in file_edits:
            # Find end of body (last indented non-blank line)
            insert_at = edit.header_lineno  # 1-based; convert to 0-based index
            i = insert_at  # next line after header
            last_body = i - 1
            while i < len(lines):
                line = lines[i]
                if line.strip() == "":
                    break
                if line.startswith("- ["):
                    break
                last_body = i
                i += 1
            # Idempotency check: scan the body for an existing Superseded by line
            body_slice = lines[edit.header_lineno:i]
            if any("Superseded by:" in bl for bl in body_slice):
                continue
            # Insert after last_body (0-based index → insert at last_body+1)
            lines.insert(last_body + 1, edit.insert_line)
            modified = True
        if modified:
            path.write_text("\n".join(lines) + ("\n" if text.endswith("\n") else ""), encoding="utf-8")
            applied.append(rel_path)
    return applied


__all__ = [
    "MissingSupersedeMarker",
    "OrphanSupersedeMarker",
    "SupersedeDriftReport",
    "PlannedEdit",
    "parse_plan_supersedes",
    "find_supersede_drift",
    "plan_auto_fixes",
    "apply_planned_edits",
]
