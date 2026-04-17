"""Bug-entry marker SSOT — schema for `- [ ] BUG-XX-NNN ...` entries.

This module is the **sole** home for parsing and classifying bug entries
in `plans/bug-tracker/section-*.md` files. It defines:

  - Lifecycle marker regex constants (Superseded, Escalated, Blocked, blocked-by)
  - `BugEntry` dataclass — the parsed shape of a bug entry
  - `parse_bug_entries(text, source_file)` — generator yielding entries
  - `classify_bug_exclusion(body_text)` — first-match precedence classifier
  - `extract_supersede_target(body_text)` — pull plan path from `Superseded by:`
  - `extract_repro(body_lines)` / `extract_subsystem(body_lines)` — field pullers

Re-implementing these regexes or the classifier elsewhere is a
`LEAK:algorithmic-duplication` violation. Both `bug_queue_scan.py`
(autopilot priority queue) and `roadmap_scan.py` (continue-roadmap
critical-bug gate) import from here. Adding a new lifecycle marker
(e.g., a future `Wontfix:` state) is a one-file change — extend the
PRECEDENCE list and add the regex.

## Lifecycle marker precedence

First match wins. The order encodes "what does this bug need next":

  1. **Superseded by:** — fix is owned by a multi-section plan; route via
     `/continue-roadmap <plan>`, NOT `/fix-bug`. The plan's frontmatter
     `supersedes:` field MUST point back to the fix-section file for
     bidirectional discoverability (validated by `bug_validators.py`).
  2. **Escalated to plan: / Escalated:** — bug requires a plan but none
     exists yet; user must run `/create-plan`.
  3. **Blocked: / **Blocked**:** — bug is waiting on a dependency
     (different from Superseded — no plan owns the fix yet).
  4. **<!-- blocked-by:** — cross-section blocker tag; the blocking work
     lives elsewhere in the corpus.

## `**BLOCKER**:` is NOT a lifecycle marker

Informational impact text describing what the bug blocks downstream
(e.g. "**BLOCKER**: This blocks ~800 spec tests until X lands") uses
the `**BLOCKER**:` prefix. This is NOT a lifecycle marker — it carries
no instruction about how to handle the bug. Only `**Blocked**:` (with
the lowercase `locked` substring + trailing colon + reason text) is.
The substring distinction is load-bearing; the regexes below enforce it.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path
from typing import Iterator


# ---------------------------------------------------------------------------
# Lifecycle marker regexes
# ---------------------------------------------------------------------------

# Each marker matches at start-of-line (multiline mode) with optional leading
# whitespace + optional `**` wrapping + the marker keyword + colon. The body
# text after the colon is what determines what action to take, but the regex
# only confirms the marker is present.

BUG_SUPERSEDED_RE = re.compile(r"(?im)^\s*superseded\s+by\s*:")
BUG_ESCALATED_RE = re.compile(r"(?im)^\s*escalated(?:\s+to\s+plan)?\s*:")
# BUG_BLOCKED_RE: matches `**Blocked**:`, `**Blocked:**`, `Blocked:`. The
# `\*{0,2}` allows 0 or 2 surrounding asterisks; the `\s*:` requires the
# trailing colon. Does NOT match `**BLOCKER**:` — the lowercased input
# `blocked` substring would not appear (BLOCKER lowercased is "blocker").
BUG_BLOCKED_RE = re.compile(r"(?im)^\s*\*{0,2}blocked\*{0,2}\s*:")
BUG_BLOCKED_BY_COMMENT_RE = re.compile(r"<!--\s*blocked-by:", re.IGNORECASE)

# Extracts the plan path from a `Superseded by:` line. Captures the first
# non-whitespace token on the right-hand side, stopping at backticks, commas,
# or end-of-line. Handles both `Superseded by: plans/foo/` (bare) and
# `Superseded by: ` + backtick-wrapped (`plans/foo/`) forms.
BUG_SUPERSEDED_TARGET_RE = re.compile(
    r"(?im)^\s*superseded\s+by\s*:\s*`?([^\s`,\n]+)`?",
)


# ---------------------------------------------------------------------------
# Entry header regex (canonical bug entry shape)
# ---------------------------------------------------------------------------

# Matches: `- [ ] [BUG-XX-NNN][severity] **Title** ...` (with `[x]` for fixed)
# Captures: section, ordinal, severity_raw, title.
# Tolerates: backticks around `[BUG-XX-NNN]`, optional `*` wrapping, severity
# reclassification like `[critical→medium]` or `[critical->medium]` (RHS is
# effective severity), AND trailing text after the closing `**` (e.g.
# `**Title** — found by continue-roadmap.` is a valid form in the corpus).
# The severity character class allows arrow/dash/word chars to admit
# reclassification syntax; `normalize_severity()` extracts the effective value.
BUG_HEADER_RE = re.compile(
    r"^\s*- \[(?P<checked>[ xX])\]\s+"
    r"`?\[BUG-(?P<section>\d{2})-(?P<ordinal>\d{3})\]"
    r"\[(?P<severity>[^\]]+)\]`?\s+"
    r"\*\*(?P<title>.+?)\*\*"
    r"(?:.*)?$"
)


# ---------------------------------------------------------------------------
# Severity normalization
# ---------------------------------------------------------------------------

_VALID_SEVERITIES = frozenset({"critical", "high", "medium", "low"})


def normalize_severity(severity_raw: str) -> str:
    """Normalize severity tag, handling reclassification syntax.

    `[critical→medium]` or `[critical->medium]` reclassifies — the
    target (RHS) severity is the effective one. Bare `critical`/`high`
    /`medium`/`low` is returned lowercased. Anything else is `unknown`.
    """
    raw = severity_raw.strip().lower()
    # Reclassification: take the right-hand side
    for sep in ("→", "->"):
        if sep in raw:
            raw = raw.split(sep, 1)[1].strip()
            break
    return raw if raw in _VALID_SEVERITIES else "unknown"


# ---------------------------------------------------------------------------
# Exclusion classifier (first-match precedence)
# ---------------------------------------------------------------------------

# Precedence: Superseded > Escalated > Blocked > blocked-by.
# Each entry is (regex, exclusion_reason). First match wins.
_EXCLUSION_PRECEDENCE: list[tuple[re.Pattern[str], str]] = [
    (BUG_SUPERSEDED_RE, "Superseded by plan"),
    (BUG_ESCALATED_RE, "Escalated to plan"),
    (BUG_BLOCKED_RE, "Blocked"),
    (BUG_BLOCKED_BY_COMMENT_RE, "Blocked (cross-section blocker tag)"),
]


def classify_bug_exclusion(body_text: str) -> str | None:
    """Return the exclusion reason for a bug body, or None if actionable.

    Applies the precedence list above. None means the bug is actionable
    by `/fix-bug` (no lifecycle marker fired). Any non-None string means
    the bug is owned by a different workflow:

    - "Superseded by plan" → route via `/continue-roadmap <plan>`
    - "Escalated to plan"  → user must `/create-plan`
    - "Blocked"            → waiting on a dependency
    - "Blocked (cross-section blocker tag)" → waiting on cross-section work
    """
    for regex, reason in _EXCLUSION_PRECEDENCE:
        if regex.search(body_text):
            return reason
    return None


def extract_supersede_target(body_text: str) -> str | None:
    """Return the plan path from a `Superseded by:` marker, or None.

    Trailing slash is preserved if present in the source (it's the
    convention for plan directories). Backticks are stripped.
    """
    m = BUG_SUPERSEDED_TARGET_RE.search(body_text)
    return m.group(1) if m else None


# ---------------------------------------------------------------------------
# Field pullers (Repro, Subsystem)
# ---------------------------------------------------------------------------


def extract_repro(body_lines: list[str]) -> str | None:
    """First `Repro:` line value, or None."""
    for line in body_lines:
        s = line.strip()
        if s.lower().startswith("repro:"):
            return s.split(":", 1)[1].strip()
    return None


def extract_subsystem(body_lines: list[str]) -> str | None:
    """First `Subsystem:` line value, or None."""
    for line in body_lines:
        s = line.strip()
        if s.lower().startswith("subsystem:"):
            return s.split(":", 1)[1].strip()
    return None


# ---------------------------------------------------------------------------
# BugEntry dataclass + parser
# ---------------------------------------------------------------------------


@dataclass
class BugEntry:
    """A parsed `- [ ]` / `- [x]` bug entry from a bug-tracker section file.

    All fields except the ID are best-effort; missing source data yields
    None (for optional fields) or empty strings.

    `excluded_reason` populated via `classify_bug_exclusion`. None means
    the bug is actionable by `/fix-bug`. Non-None means it's owned by
    another workflow — see `classify_bug_exclusion` docstring.

    `superseded_by` populated only when `excluded_reason == "Superseded by plan"`.
    Carries the plan path from the marker.
    """
    bug_id: str              # "BUG-04-074"
    section: int             # 4
    ordinal: int             # 74
    severity: str            # normalized: critical | high | medium | low | unknown
    severity_raw: str        # raw tag from source (may contain reclassification)
    status: str              # "open" | "fixed"
    title: str
    lineno: int              # 1-based line of the entry header
    source_file: str         # bug-tracker section filename (basename)
    body_text: str = ""      # joined body lines (for marker re-classification)
    body_lines: list[str] | None = None  # raw body lines (for field pullers)
    excluded_reason: str | None = None
    superseded_by: str | None = None
    repro: str | None = None
    subsystem: str | None = None


def parse_bug_entries(text: str, source_file: str) -> Iterator[BugEntry]:
    """Yield `BugEntry` objects from a bug-tracker section file's text.

    Body lines are collected as indented continuations until a blank line
    or the next `- [` header. This mirrors the conventional bug-entry
    layout in `plans/bug-tracker/section-*.md` files.

    Lifecycle marker classification + supersede-target extraction happen
    here so consumers don't have to re-parse the body.
    """
    lines = text.splitlines()
    i = 0
    while i < len(lines):
        line = lines[i]
        m = BUG_HEADER_RE.match(line)
        if not m:
            i += 1
            continue
        checked = m.group("checked").lower()
        section = int(m.group("section"))
        ordinal = int(m.group("ordinal"))
        severity_raw = m.group("severity")
        title = m.group("title").strip()
        # Collect body lines until blank or next `- [` header
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
        excluded_reason = classify_bug_exclusion(body_text)
        superseded_by = (
            extract_supersede_target(body_text)
            if excluded_reason == "Superseded by plan"
            else None
        )
        yield BugEntry(
            bug_id=f"BUG-{section:02d}-{ordinal:03d}",
            section=section,
            ordinal=ordinal,
            severity=normalize_severity(severity_raw),
            severity_raw=severity_raw,
            status="fixed" if checked == "x" else "open",
            title=title,
            lineno=i + 1,
            source_file=source_file,
            body_text=body_text,
            body_lines=body_lines,
            excluded_reason=excluded_reason,
            superseded_by=superseded_by,
            repro=extract_repro(body_lines),
            subsystem=extract_subsystem(body_lines),
        )
        i = j


def parse_bug_tracker_dir(bug_tracker_dir: Path) -> list[BugEntry]:
    """Convenience: parse all `section-*.md` files in a bug-tracker directory."""
    entries: list[BugEntry] = []
    for path in sorted(bug_tracker_dir.glob("section-*.md")):
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        entries.extend(parse_bug_entries(text, path.name))
    return entries


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------

__all__ = [
    "BUG_SUPERSEDED_RE",
    "BUG_ESCALATED_RE",
    "BUG_BLOCKED_RE",
    "BUG_BLOCKED_BY_COMMENT_RE",
    "BUG_SUPERSEDED_TARGET_RE",
    "BUG_HEADER_RE",
    "BugEntry",
    "classify_bug_exclusion",
    "extract_supersede_target",
    "extract_repro",
    "extract_subsystem",
    "normalize_severity",
    "parse_bug_entries",
    "parse_bug_tracker_dir",
]
