#!/usr/bin/env python3
"""
plan-annotations.py — Classify plan annotations in source code.

Plan annotations (TPR-04-005, BUG-04-019, §04.3, Phase A, section-04-name,
etc.) are allowed as temporary scaffolding during active development but
MUST be removed when the plan or specific finding completes. This script
reads every plan's markdown content to build a map of finding IDs to their
status, then scans source code and classifies each annotation against that
map.

Classifications
---------------

PERMANENT
    Spec citations (`Spec: Clause N.M`) and architecture-internal
    section numbering (`AIMS Section N`, `eval_v2 Section N`). Never
    flagged — these are load-bearing design references.

ACTIVE_SCAFFOLDING
    The finding ID is marked `[ ]` (open) in an active plan. Acceptable
    as scaffolding for now; will need cleanup when the finding closes.

STALE_RESOLVED
    The finding ID is marked `[x]` (resolved) in an active plan. The
    work is done — the annotation in source code is now stale and must
    be removed. These are cleanup candidates NOW.

STALE_COMPLETED_PLAN
    The finding ID is referenced in a plan under `plans/completed/`, or
    in a plan whose `00-overview.md` status is `complete`. The plan is
    archived — every annotation referencing it is stale.

ORPHAN
    The finding ID does not appear in any plan's markdown. Either the
    plan was deleted without cleanup, or the annotation was never
    backed by a real finding. Investigate each.

SECTION_REF
    Generic section reference (`§04.3`, `Section 04.2`, `Phase A`,
    `section-04-name`) that doesn't include a specific finding ID.
    Classified by resolving the section to its owning plan, if possible.
    These are harder to classify than finding IDs — a `§04.2` could
    belong to any plan with a section 04.2.

Modes
-----

Default mode shows stale annotations (STALE_RESOLVED + STALE_COMPLETED_PLAN
+ ORPHAN) — things you should clean up now. ACTIVE_SCAFFOLDING and
PERMANENT are filtered out as "OK for now." Counts are always honest:
the summary reports raw total, per-classification breakdown, and what
was shown vs. hidden.

`--scope PATH [PATH ...]`  Restrict scanning to the given paths. Recommended
                           for hygiene reviews of a specific work arc.
`--all`                    Show every classification including ACTIVE and
                           PERMANENT. Useful for audits.
`--cleanup-only`           Show only STALE_RESOLVED and STALE_COMPLETED_PLAN.
`--orphans-only`           Show only ORPHAN — unresolvable references.
`--active-only`            Show only ACTIVE_SCAFFOLDING (what's being
                           tracked as in-progress work).
`--plan NAME`              Filter to annotations referencing a specific plan
                           by name (e.g., `--plan bug-tracker` or `--plan
                           repr-opt`). Also accepts numeric plan-section
                           (`--plan 04`) for legacy callers.
`--json`                   Machine-readable JSON output.
`--count`                  Summary counts per classification, no details.
`--fix`                    Remove STALE_RESOLVED + STALE_COMPLETED_PLAN +
                           ORPHAN annotations from source files. Conservative:
                           only edits inside `//`, `///`, `//!`, `/* */`
                           comment regions; never touches code or strings.
                           Strips the bare ID plus immediately adjacent
                           punctuation (`[ID]`, `(ID)`, `ID:`, `ID.`). If a
                           line becomes empty after stripping (e.g. just
                           `//`), the line is deleted. Default is dry-run
                           (preview only); pair with `--apply` to write.
`--apply`                  With `--fix`, actually write the modified files.
                           Without `--apply`, `--fix` prints a unified diff
                           preview and exits with code 2.
`--include-ori`            Also scan .ori files (default: .rs only).
`--pattern`                Print the master annotation regex and exit.
`--help`                   This message.

Examples
--------

    # Hygiene review of a specific arc:
    plan-annotations.py --scope compiler/ori_llvm/src/aot compiler/oric/src/commands

    # Full audit:
    plan-annotations.py --all

    # Cleanup pass after a plan completes:
    plan-annotations.py --cleanup-only

    # Find orphan IDs that reference nothing:
    plan-annotations.py --orphans-only

    # Filter to one plan:
    plan-annotations.py --plan bug-tracker --cleanup-only
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Iterable


REPO_ROOT = Path(__file__).resolve().parents[3]
PLANS_DIR = REPO_ROOT / "plans"
COMPLETED_DIR = PLANS_DIR / "completed"


# ─────────────────────────────────────────────────────────────
# Regex patterns — annotation kinds found in source code
# ─────────────────────────────────────────────────────────────

# Finding IDs: TPR-07-019, BUG-04-045, TPR-BUG-04-045-07, CROSS-04-014,
# TPR-05-001-codex, TPR-05-006-gemini, etc.
# Intentionally loose — we capture and then look up against the plan index.
# Order matters: TPR-BUG must come before TPR to prefer the longer match.
# The trailing `(?:-\w+)*` segment accommodates (a) numeric sub-ordinals like
# `-07` on TPR-BUG-04-045-07, and (b) reviewer suffixes like `-codex` /
# `-gemini` used by the dual-source TPR workflow.
FINDING_ID_RE = re.compile(
    r"\b((?:TPR-BUG|TPR|CROSS|BUG|FIND|TASK|ISSUE)-\d+-\d+(?:-\w+)*)\b"
)

# Canonical "subject-form" checkbox regex. Matches ONLY lines where the
# finding ID appears IMMEDIATELY after the checkbox marker, wrapped in
# brackets (with optional backticks or bold markers). Recognized shapes:
#   - [x] `[BUG-04-045]` ...                              (bug-tracker)
#   - [x] `[BUG-04-045][high]` **Title** ...              (bug-tracker w/ severity)
#   - [ ] `[TPR-05-001-codex][medium]` `path.rs:42` — ... (dual-tpr wrappers)
#   - [x] **[TPR-01-005]** Fix ...                        (repr-opt bold form)
#
# Must NOT match lines where the ID appears only in body prose, e.g.
#   - [x] At least 1 /review-work scenario ... BUG-04-045 dogfood
# Those are incidental references, not resolution markers. Matching them
# indexes the wrong entry as "resolved" and produces stale-resolved false
# positives. See test_plan_annotations_fix.py § CHECKBOX_SUBJECT_CASES for
# the regression matrix that guards this distinction.
CHECKBOX_SUBJECT_RE = re.compile(
    r"^\s*-\s+\[([ x])\]\s+"
    r"(?:\*\*)?`?\["
    r"((?:TPR-BUG|TPR|CROSS|BUG|FIND|TASK|ISSUE)-\d+-\d+(?:-\w+)*)"
    r"\]"
)

# Section symbol refs: §04.3, §07.R, §04.2 Phase A (context-dependent)
SECTION_SYMBOL_RE = re.compile(r"§(\d+[\d.]*[A-Z]?)")

# Spelled section refs: "Section 04.2", "Section 04"
SECTION_SPELLED_RE = re.compile(r"\bSection\s+(\d+[\d.]*)")

# Section file references: "section-04-codegen-llvm", "section-07-enum-repr"
SECTION_FILE_RE = re.compile(r"\b(section-\d+-[a-z][a-z0-9_-]*)")

# Phase refs: Phase A, Phase B, Phase C, Phase 0a, Phase 0b
PHASE_LETTER_RE = re.compile(r"\bPhase\s+([A-C])\b")
PHASE_SUB_RE = re.compile(r"\bPhase\s+(\d+[a-z])\b")

# Plan path references: plans/repr-opt/section-07-enum-repr
PLAN_PATH_RE = re.compile(r"\b(plans/[a-z_-]+/section-\d+-[a-z][a-z0-9_-]*)")

# Master search regex (for the initial grep pass — we then re-classify each
# match using the specialized regexes above).
MASTER_GREP_PATTERN = (
    r"(TPR|CROSS|BUG|FIND|TASK|ISSUE)-\d+-\d+"
    r"|§\d+[\d.]*"
    r"|\bSection\s+\d+[\d.]+"
    r"|\bsection-\d+-[a-z]"
    r"|\bPhase\s+[A-C]\b"
    r"|\bPhase\s+\d+[a-z]\b"
    r"|\bplans/[a-z_-]+/section-"
)

# Permanent (never flag) patterns
SPEC_CLAUSE_RE = re.compile(r"Spec:\s*Clause\s+\d+", re.IGNORECASE)
AIMS_SECTION_RE = re.compile(r"AIMS\s+Section\s+\d+", re.IGNORECASE)
EVAL_V2_SECTION_RE = re.compile(r"eval_v2\s+Section\s+\d+", re.IGNORECASE)
FIPS_PHASE_RE = re.compile(r"\bfip(?:s)?\s+Phase\s+[A-C]\b", re.IGNORECASE)

# Architecture-internal directories — section references inside these are
# ALWAYS internal design docs, never plan refs.
ARCH_DOC_DIRS = [
    "compiler/ori_arc/src/aims",
    "compiler/ori_canon",
]

# Source scanning: exclude these top-level dirs regardless of mode.
SCAN_EXCLUDE_DIRS = [
    "plans",
    "docs",
    ".claude",
    "target",
    "target-llvm",
    ".git",
    "tests/spec",  # reference data, not source
    "_repos",
]


# ─────────────────────────────────────────────────────────────
# Data model
# ─────────────────────────────────────────────────────────────


class PlanStatus(Enum):
    IN_PROGRESS = "in-progress"
    NOT_STARTED = "not-started"
    COMPLETE = "complete"
    RESEARCH = "research"
    UNKNOWN = "unknown"


class Classification(Enum):
    PERMANENT = "permanent"
    ACTIVE_SCAFFOLDING = "active-scaffolding"
    STALE_RESOLVED = "stale-resolved"
    STALE_COMPLETED_PLAN = "stale-completed-plan"
    ORPHAN = "orphan"
    SECTION_REF = "section-ref"
    ARCH_INTERNAL = "arch-internal"


@dataclass
class Plan:
    name: str  # "bug-tracker", "repr-opt", "completed/closure-ownership"
    path: Path  # absolute path to the plan dir
    status: PlanStatus
    is_completed_archive: bool  # True if anywhere under plans/completed/


@dataclass
class PlanEntry:
    finding_id: str  # "BUG-04-045", "TPR-07-019", "TPR-BUG-04-045-07"
    is_resolved: bool  # True if `[x]`, False if `[ ]`
    plan: Plan
    source_file: Path
    source_line: int


@dataclass
class Annotation:
    """A single annotation occurrence in a source file."""

    raw_text: str  # The literal matched text (e.g., "TPR-07-019", "§04.3")
    file: Path  # Absolute path
    line: int
    finding_id: str | None = None  # Parsed ID, if this annotation is an ID
    section_num: str | None = None  # Parsed section number, if a section ref
    plan_path_ref: str | None = None  # plans/repr-opt/section-07-... if a path ref
    classification: Classification | None = None
    plan_entry: PlanEntry | None = None  # Resolved entry, if any


# ─────────────────────────────────────────────────────────────
# Plan indexing
# ─────────────────────────────────────────────────────────────


def read_plan_status(overview_path: Path) -> PlanStatus:
    """Parse the YAML frontmatter of a plan's 00-overview.md for the status field."""
    try:
        text = overview_path.read_text(encoding="utf-8")
    except OSError:
        return PlanStatus.UNKNOWN

    # Minimal frontmatter parser: between --- and --- at file start
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return PlanStatus.UNKNOWN
    for line in lines[1:]:
        if line.strip() == "---":
            break
        m = re.match(r"^status:\s*\"?([a-z-]+)\"?\s*$", line)
        if m:
            raw = m.group(1)
            try:
                return PlanStatus(raw)
            except ValueError:
                return PlanStatus.UNKNOWN
    return PlanStatus.UNKNOWN


def discover_plans() -> dict[str, Plan]:
    """
    Walk plans/ and collect every plan directory with its status.

    Plans under plans/completed/ are all treated as COMPLETE regardless of
    their individual 00-overview.md status (some archives don't have one).

    Returns a dict keyed by plan name. For nested plans under
    plans/completed/, the name includes the "completed/" prefix so they
    don't collide with active plans of the same name.
    """
    plans: dict[str, Plan] = {}
    if not PLANS_DIR.is_dir():
        return plans

    # Top-level active plans
    for entry in sorted(PLANS_DIR.iterdir()):
        if not entry.is_dir():
            continue
        if entry.name in ("code-journeys", "completed"):
            continue
        overview = entry / "00-overview.md"
        status = read_plan_status(overview) if overview.is_file() else PlanStatus.UNKNOWN
        plans[entry.name] = Plan(
            name=entry.name,
            path=entry,
            status=status,
            is_completed_archive=False,
        )

    # Completed archive: every subdir under plans/completed/ is a complete plan
    if COMPLETED_DIR.is_dir():
        for entry in sorted(COMPLETED_DIR.iterdir()):
            if not entry.is_dir():
                continue
            qualified_name = f"completed/{entry.name}"
            plans[qualified_name] = Plan(
                name=qualified_name,
                path=entry,
                status=PlanStatus.COMPLETE,
                is_completed_archive=True,
            )

    return plans


def index_plan_entries(plans: dict[str, Plan]) -> dict[str, list[PlanEntry]]:
    """
    Walk every plan's markdown files and extract `- [ ]` / `- [x]` checkbox
    items whose text contains a finding ID.

    Returns a dict keyed by finding ID. Multiple entries per ID are possible
    (e.g., a bug is referenced in both the section file and a fix section
    file); the classifier picks the most-resolved one.
    """
    entries: dict[str, list[PlanEntry]] = defaultdict(list)

    # Subject-form checkbox regex — see CHECKBOX_SUBJECT_RE module constant
    # for the full pattern definition and rationale. We bind a local alias
    # here so the inner hot loop doesn't repeat the attribute lookup.
    checkbox_re = CHECKBOX_SUBJECT_RE

    for plan in plans.values():
        if not plan.path.is_dir():
            continue
        # Walk markdown files in this plan. Include section-*.md, fix-*.md,
        # 00-overview.md, any .md at all.
        for md_file in plan.path.rglob("*.md"):
            # Skip nested `completed/` inside a non-completed plan (if any)
            if plan.is_completed_archive:
                pass  # already counted as completed
            else:
                # If this md file is itself under plans/completed/, skip
                # (it'll be handled by the completed-archive entry instead)
                try:
                    rel = md_file.relative_to(PLANS_DIR)
                    if rel.parts and rel.parts[0] == "completed":
                        continue
                except ValueError:
                    pass

            try:
                lines = md_file.read_text(encoding="utf-8").splitlines()
            except OSError:
                continue
            for lineno, line in enumerate(lines, start=1):
                m = checkbox_re.match(line)
                if not m:
                    continue
                box_state, finding_id = m.group(1), m.group(2)
                # The checkbox regex now captures the complete canonical ID
                # (including alpha suffixes like -codex/-gemini). No fallback
                # search is needed — and would actively hurt, since it could
                # re-introduce the body-mention false positive the subject-
                # form regex is designed to prevent.
                entries[finding_id].append(
                    PlanEntry(
                        finding_id=finding_id,
                        is_resolved=(box_state == "x"),
                        plan=plan,
                        source_file=md_file,
                        source_line=lineno,
                    )
                )

    return entries


# ─────────────────────────────────────────────────────────────
# Source scanning
# ─────────────────────────────────────────────────────────────


def run_grep(paths: list[Path], include_ori: bool) -> list[tuple[Path, int, str]]:
    """
    Scan source code for the master annotation pattern.

    Shells out to `grep -rPn` because Python's own walk + re is ~5× slower
    on this repo. Uses `--exclude-dir` to skip plans/docs/.claude/target/
    and `--include=*.rs` (+ optional `*.ori`) to scope by language.
    """
    results: list[tuple[Path, int, str]] = []
    if not paths:
        paths = [REPO_ROOT]

    cmd: list[str] = ["grep", "-rPn", "--include=*.rs"]
    if include_ori:
        cmd.append("--include=*.ori")
    for d in SCAN_EXCLUDE_DIRS:
        cmd.append(f"--exclude-dir={Path(d).name}")
    cmd.append(MASTER_GREP_PATTERN)
    cmd.extend(str(p) for p in paths)

    try:
        proc = subprocess.run(
            cmd, capture_output=True, text=True, check=False, cwd=REPO_ROOT
        )
    except (subprocess.SubprocessError, FileNotFoundError) as e:
        print(f"error: failed to scan source: {e}", file=sys.stderr)
        return []

    for raw_line in proc.stdout.splitlines():
        # Format: path:lineno:text
        parts = raw_line.split(":", 2)
        if len(parts) < 3:
            continue
        path_str, lineno_str, text = parts
        try:
            lineno = int(lineno_str)
        except ValueError:
            continue
        results.append((Path(path_str), lineno, text))

    return results


def extract_annotations(
    grep_hits: list[tuple[Path, int, str]],
) -> list[Annotation]:
    """
    For each grep hit, extract ONE annotation occurrence. If the line has
    multiple annotation kinds, emit multiple Annotation objects — the
    classifier handles each independently.
    """
    annotations: list[Annotation] = []
    for file, line, text in grep_hits:
        seen_in_line: set[tuple[str, str]] = set()  # dedupe within a single line

        # Try finding IDs first (most specific)
        for m in FINDING_ID_RE.finditer(text):
            key = ("id", m.group(1))
            if key in seen_in_line:
                continue
            seen_in_line.add(key)
            annotations.append(
                Annotation(
                    raw_text=m.group(0),
                    file=file,
                    line=line,
                    finding_id=m.group(1),
                )
            )

        # Plan path refs next
        for m in PLAN_PATH_RE.finditer(text):
            key = ("path", m.group(1))
            if key in seen_in_line:
                continue
            seen_in_line.add(key)
            annotations.append(
                Annotation(
                    raw_text=m.group(0),
                    file=file,
                    line=line,
                    plan_path_ref=m.group(1),
                )
            )

        # Section symbol refs (§04.3)
        for m in SECTION_SYMBOL_RE.finditer(text):
            key = ("section", m.group(1))
            if key in seen_in_line:
                continue
            seen_in_line.add(key)
            annotations.append(
                Annotation(
                    raw_text=m.group(0),
                    file=file,
                    line=line,
                    section_num=m.group(1),
                )
            )

        # Section file refs (section-04-name)
        for m in SECTION_FILE_RE.finditer(text):
            key = ("sectionfile", m.group(1))
            if key in seen_in_line:
                continue
            seen_in_line.add(key)
            annotations.append(
                Annotation(
                    raw_text=m.group(0),
                    file=file,
                    line=line,
                    section_num=m.group(1),
                )
            )

        # Phase refs (Phase A, Phase 0b) — least-specific, only flag if not
        # already covered by a more-specific match on the same line
        if not seen_in_line:
            for m in PHASE_LETTER_RE.finditer(text):
                annotations.append(
                    Annotation(
                        raw_text=m.group(0),
                        file=file,
                        line=line,
                        section_num=f"Phase {m.group(1)}",
                    )
                )
            for m in PHASE_SUB_RE.finditer(text):
                annotations.append(
                    Annotation(
                        raw_text=m.group(0),
                        file=file,
                        line=line,
                        section_num=f"Phase {m.group(1)}",
                    )
                )

    return annotations


# ─────────────────────────────────────────────────────────────
# Classification
# ─────────────────────────────────────────────────────────────


def is_permanent_line(file: Path, line: int, text: str) -> bool:
    """Return True if the line is a permanent citation (spec, arch-internal)."""
    if SPEC_CLAUSE_RE.search(text):
        return True
    if AIMS_SECTION_RE.search(text):
        return True
    if EVAL_V2_SECTION_RE.search(text):
        return True
    if FIPS_PHASE_RE.search(text):
        return True
    return False


def is_arch_internal_path(file: Path) -> bool:
    """Return True if the file is inside an architecture-internal doc dir."""
    try:
        rel = file.resolve().relative_to(REPO_ROOT)
    except ValueError:
        rel = file
    rel_str = str(rel).replace("\\", "/")
    for d in ARCH_DOC_DIRS:
        if rel_str.startswith(d):
            return True
    return False


def classify_annotation(
    ann: Annotation,
    plan_entries: dict[str, list[PlanEntry]],
    plans: dict[str, Plan],
    source_cache: dict[Path, list[str]],
) -> None:
    """
    Set ann.classification and ann.plan_entry (if any) by cross-referencing
    the annotation against the plan index.
    """
    # Permanent check: read the line text and match against spec/arch patterns
    try:
        lines = source_cache.get(ann.file)
        if lines is None:
            lines = ann.file.read_text(encoding="utf-8").splitlines()
            source_cache[ann.file] = lines
    except OSError:
        lines = []
    line_text = lines[ann.line - 1] if 0 < ann.line <= len(lines) else ""

    if is_permanent_line(ann.file, ann.line, line_text):
        ann.classification = Classification.PERMANENT
        return

    # Architecture-internal dirs: any section-number reference is internal
    # design doc, not a plan ref.
    if is_arch_internal_path(ann.file) and ann.section_num is not None:
        ann.classification = Classification.ARCH_INTERNAL
        return

    # Finding ID classification
    if ann.finding_id is not None:
        matches = plan_entries.get(ann.finding_id, [])
        if not matches:
            ann.classification = Classification.ORPHAN
            return
        # Prefer the resolved-in-active-plan match (most actionable)
        for entry in matches:
            if entry.is_resolved and not entry.plan.is_completed_archive:
                ann.classification = Classification.STALE_RESOLVED
                ann.plan_entry = entry
                return
        for entry in matches:
            if entry.plan.is_completed_archive or entry.plan.status == PlanStatus.COMPLETE:
                ann.classification = Classification.STALE_COMPLETED_PLAN
                ann.plan_entry = entry
                return
        # Otherwise it's still an open scaffolding ref
        ann.classification = Classification.ACTIVE_SCAFFOLDING
        ann.plan_entry = matches[0]
        return

    # Plan path reference: resolve the plan and its section file
    if ann.plan_path_ref is not None:
        # e.g. "plans/repr-opt/section-07-enum-repr"
        parts = ann.plan_path_ref.split("/")
        if len(parts) >= 3:
            plan_name = parts[1]
            plan = plans.get(plan_name)
            if plan is None:
                ann.classification = Classification.ORPHAN
                return
            if plan.status == PlanStatus.COMPLETE or plan.is_completed_archive:
                ann.classification = Classification.STALE_COMPLETED_PLAN
                return
            # Active plan — informational
            ann.classification = Classification.SECTION_REF
            return
        ann.classification = Classification.ORPHAN
        return

    # Section number reference (§04.3, Section 04, section-04-name, Phase A)
    # These are too loose to resolve to a specific finding. Mark as
    # SECTION_REF (informational) — reviewer decides whether to clean up.
    ann.classification = Classification.SECTION_REF


def classify_all(
    annotations: list[Annotation],
    plan_entries: dict[str, list[PlanEntry]],
    plans: dict[str, Plan],
) -> None:
    source_cache: dict[Path, list[str]] = {}
    for ann in annotations:
        classify_annotation(ann, plan_entries, plans, source_cache)


# ─────────────────────────────────────────────────────────────
# Line-number reference stripping — separate ephemeral category
# ─────────────────────────────────────────────────────────────
#
# References like `evaluator/compile.rs:383`, `imports.rs:210`, or
# `int.rs:42-52` cite a specific line in another file. The path is
# permanent (the file still exists) but the line number rots the
# moment that file gets a single edit above the cited line. We strip
# just the `:NNN` (or `:NNN-NNN` range) part, leaving the file path.

LINE_NUMBER_REF_RE = re.compile(
    r"\b([a-zA-Z_][a-zA-Z0-9_/]*\.rs)(:\d+(?:-\d+)?)\b"
)


# ─────────────────────────────────────────────────────────────
# Fix mode — conservative annotation removal
# ─────────────────────────────────────────────────────────────


def is_in_comment_region(line: str, col: int) -> bool:
    """
    Return True if `col` (0-based byte index into `line`) is inside a
    `//`, `///`, `//!`, or `/* */` comment region.

    Conservative: a `//` inside a string literal does NOT start a comment
    region. We track string state by walking the line left-to-right.

    Multi-line `/* */` comments are NOT supported (we'd need cross-line
    state) — if the annotation is inside a `/* */` block, this returns
    False, preventing removal. That's the safe default.
    """
    i = 0
    n = len(line)
    in_str_dq = False
    in_str_sq = False
    while i < n and i <= col:
        ch = line[i]
        # Skip escapes inside string literals
        if (in_str_dq or in_str_sq) and ch == "\\" and i + 1 < n:
            i += 2
            continue
        if in_str_dq:
            if ch == '"':
                in_str_dq = False
            i += 1
            continue
        if in_str_sq:
            if ch == "'":
                in_str_sq = False
            i += 1
            continue
        if ch == '"':
            in_str_dq = True
            i += 1
            continue
        if ch == "'":
            in_str_sq = True
            i += 1
            continue
        # Not in a string — check for `//` start
        if ch == "/" and i + 1 < n and line[i + 1] == "/":
            return col >= i  # everything from `//` onward is a comment
        # Inline `/* ... */` on the same line
        if ch == "/" and i + 1 < n and line[i + 1] == "*":
            # Find the matching `*/`
            close = line.find("*/", i + 2)
            if close < 0:
                # Multi-line — col is inside if past the opening
                return col >= i
            if i <= col <= close + 1:
                return True
            i = close + 2
            continue
        i += 1
    return False


# Removal patterns, in order of preference (longest match wins per pattern)
def _make_removal_patterns(ann_id: str) -> list[re.Pattern]:
    """
    Build the set of regexes that try to strip the annotation plus
    adjacent punctuation. Each is anchored on the bare ID plus surrounding
    delimiters; the FIRST one that matches is used. Order matters: more
    specific (longer-context) patterns must come before shorter ones.

    Multi-ID lines (e.g. `// ID1 / ID2 / ID3: text`) are supported when
    ALL IDs in the list are stale: each ID-with-trailing-slash group is
    stripped one by one, leaving the trailing text. When only SOME IDs in
    a list are stale, the slash patterns leave the active IDs in place.
    """
    eid = re.escape(ann_id)
    # Em-dash, en-dash and ASCII hyphen-minus, with surrounding spaces
    DASH = r"\s+[—–\-]\s+"
    return [
        # `[ID]: ` and `[ID] ` — bracketed form with trailing punctuation
        # Mid-sentence variant consumes the leading space.
        re.compile(r"(?<=[^/\s])\s*\[" + eid + r"\][\s:.\-—–]*"),
        # Start-of-comment variant — no leading-space consume
        re.compile(r"\[" + eid + r"\][\s:.\-—–]*"),
        # ` (ID)` mid-sentence — consume the leading space too so the
        # remaining text doesn't get a stranded space before any
        # following period or punctuation.
        re.compile(r"(?<=[^/\s])\s*\(" + eid + r"\)"),
        # `(ID)` at start of comment — no leading-space consume.
        # If the ENTIRE comment was `(ID).`, the trailing `.` becomes a
        # stranded period and the empty-line detection deletes the line.
        re.compile(r"\(" + eid + r"\)\s*"),
        # ` — ID fix.` / ` - ID fix.` — dash-prefixed phrase ending in "fix"
        # (e.g. "tuples — TPR-04-003 fix." → "tuples")
        re.compile(DASH + eid + r"\s+fix\.?"),
        # ` — ID` / ` - ID` — dash-prefixed bare ID at end of phrase
        # (e.g. "tuples — TPR-04-003" → "tuples")
        re.compile(DASH + eid + r"\b"),
        # `ID — ` / `ID - ` — bare ID followed by dash separator
        # (e.g. "Semantic pin: TPR-04-006 — splitting" → "Semantic pin: splitting")
        re.compile(r"\b" + eid + DASH),
        # `ID / ` — slash-list separator (first item in a multi-ID list)
        # (e.g. "// TPR-A / TPR-B / TPR-C: text" with TPR-A stale →
        #  "// TPR-B / TPR-C: text"). Combined with the next pattern,
        # repeated stripping handles lists where ALL items are stale.
        re.compile(r"\b" + eid + r"\s+/\s+"),
        # ` / ID` — slash-list separator (non-first item in a multi-ID list)
        # Used when the previous item was already stripped, leaving us as
        # the new "first" item adjacent to the slash residue from the prior
        # iteration.
        re.compile(r"\s+/\s+" + eid + r"\b"),
        # `ID,` — comma-suffixed (e.g. "After ID, the rest...").
        # Do NOT consume the leading space — the comma is a sentence
        # separator, so we want the prior word's trailing space to remain
        # (otherwise `per TPR-XX-YY, and` becomes `perand`).
        re.compile(r"\b" + eid + r",\s?"),
        # `ID:` plus single space
        re.compile(r"\b" + eid + r":\s?"),
        # `ID.` (sentence-final form)
        re.compile(r"\b" + eid + r"\.\s?"),
        # Bare `ID` (last resort) plus optional trailing space
        re.compile(r"\b" + eid + r"\s?"),
    ]


def is_safe_id_context(line: str, idx: int, ann_id: str) -> bool:
    """
    Return False if the ID at `idx` is in a context where naive removal
    would damage the surrounding text. Specifically reject:

    - Possessive form: `TPR-XX-YY's function` — strip leaves `'s function`
    - Compound prefix: `post-TPR-XX-YY`, `pre-TPR-XX-YY` — strip leaves the
      prefix mashed against the next word
    - Slash continuation: `TPR-02-007/008` — strip leaves orphan `/008`
    - Word continuation: `TPR-XX-YYa`, `TPR-XX-YY1` — strip leaves orphan
      letter/digit suffix (the regex's `\\w*` already permits these in the
      ID, but if there's an unexpected suffix we play it safe)

    Decorative dash runs (`// ---- TPR-XX-YY`) are explicitly allowed
    because they're a recognised section-header pattern.
    """
    end = idx + len(ann_id)
    # Char immediately after the ID
    if end < len(line):
        ch = line[end]
        if ch == "'":  # possessive: TPR-XX-YY's
            return False
        if ch == "/":  # slash-continuation: TPR-02-007/008
            return False
        # word continuation (letter/digit immediately after)
        if ch.isalnum() or ch == "_":
            return False
    # Char immediately before the ID
    if idx > 0:
        ch = line[idx - 1]
        if ch == "-":
            # Compound prefix like `post-TPR-XX-YY` — but allow decorative
            # `// ---- TPR-XX-YY` section headers (3+ dashes preceded by
            # whitespace).
            preceding = line[max(0, idx - 6):idx]
            if "----" in preceding:
                return True  # decorative header, safe
            return False  # genuine compound prefix
        # Word continuation (letter/digit immediately before — shouldn't
        # happen because the ID starts with an uppercase letter, but be
        # defensive).
        if ch.isalnum() or ch == "_":
            return False
    return True


def remove_annotations_from_line(
    line: str,
    ann_ids_in_line: list[str],
) -> tuple[str | None, bool]:
    """
    Remove every annotation ID in `ann_ids_in_line` from `line`.

    Returns `(new_line, changed)`:
    - `new_line` is the modified line, or `None` if the line should be
      deleted entirely (e.g., it became an empty comment marker).
    - `changed` is True iff at least one annotation was actually stripped.

    Safety: we only edit IDs that are inside a comment region AND in a
    safe textual context (see `is_safe_id_context`). IDs in strings,
    identifiers, possessives, or compound words are left in place.
    """
    new_line = line
    changed = False
    for ann_id in ann_ids_in_line:
        idx = new_line.find(ann_id)
        if idx < 0:
            continue
        if not is_in_comment_region(new_line, idx):
            continue
        if not is_safe_id_context(new_line, idx, ann_id):
            continue
        for pat in _make_removal_patterns(ann_id):
            stripped = pat.sub("", new_line, count=1)
            if stripped != new_line:
                new_line = stripped
                changed = True
                break

    if not changed:
        return new_line, False

    # Strip trailing whitespace only — do NOT collapse internal runs of
    # spaces, because doc comments routinely use indented continuations
    # like `///   first` / `///   second` for list formatting, and
    # collapsing those would be a destructive reformat.
    new_line = new_line.rstrip()

    # If a strip left stranded leading punctuation immediately after the
    # comment marker (e.g. `// . text` produced by `(ID).` strip), remove
    # it. The marker stays put; only the orphan punctuation goes away.
    new_line = re.sub(
        r"^(\s*//[!/]*\s)[.,;:]\s",
        r"\1",
        new_line,
    )

    # If the line is now an empty comment marker, delete it
    stripped = new_line.lstrip()
    empty_markers = {
        "//",
        "///",
        "//!",
        "/*",
        "*/",
        "*",
    }
    # Also consider decorative lines that are JUST a comment marker plus
    # one or more runs of `-`/`=`/`*` (e.g., `// ----`, `// ==== ====`,
    # `// ---- ----`) left over from header strips.
    if stripped in empty_markers:
        return None, True
    if re.fullmatch(r"//[!/]*(?:\s*[-=*]+)+\s*", stripped):
        return None, True
    # Also delete lines that are just a comment marker plus a stranded
    # punctuation character (e.g., `// .`, `/// ,`) — these are leftover
    # sentence terminators from comment paragraphs that were entirely an
    # ID and a period.
    if re.fullmatch(r"//[!/]*\s*[.,;:]\s*", stripped):
        return None, True
    # Stranded reference words: a comment that, after stripping, contains
    # only a "See" or "See:" reference with nothing to refer to
    # (e.g., `// See TPR-XX-YY.` → `// See`). The reference is meaningless
    # without its target, so delete the entire line.
    if re.fullmatch(r"//[!/]*\s*[Ss]ee[.:;]?\s*", stripped):
        return None, True

    return new_line, True


@dataclass
class FileEdit:
    """A planned edit for a single file."""

    file: Path
    original_lines: list[str]
    new_lines: list[str]  # may be shorter than original (deletions)
    changed_line_indices: list[int]  # 0-based indices into original_lines
    deleted_line_indices: list[int]  # 0-based indices into original_lines


def plan_file_edits(
    annotations: list[Annotation],
) -> list[FileEdit]:
    """
    Group annotations by file and produce a FileEdit per file.

    Skips annotations that we cannot safely remove (not in a comment).
    """
    by_file: dict[Path, list[Annotation]] = defaultdict(list)
    for ann in annotations:
        if ann.finding_id is None:
            continue  # only ID-based annotations are removable
        by_file[ann.file].append(ann)

    edits: list[FileEdit] = []
    for file, anns in sorted(by_file.items(), key=lambda kv: str(kv[0])):
        try:
            text = file.read_text(encoding="utf-8")
        except OSError:
            continue
        # Preserve trailing newline state
        lines = text.splitlines(keepends=False)

        # Group IDs per line
        ids_by_line: dict[int, list[str]] = defaultdict(list)
        for ann in anns:
            ids_by_line[ann.line].append(ann.finding_id)  # type: ignore[arg-type]

        new_lines: list[str] = []
        changed_indices: list[int] = []
        deleted_indices: list[int] = []
        for idx, original in enumerate(lines):
            lineno = idx + 1
            ids_here = ids_by_line.get(lineno)
            if not ids_here:
                new_lines.append(original)
                continue
            new_line, changed = remove_annotations_from_line(original, ids_here)
            if not changed:
                new_lines.append(original)
                continue
            if new_line is None:
                deleted_indices.append(idx)
                continue
            new_lines.append(new_line)
            changed_indices.append(idx)

        if changed_indices or deleted_indices:
            edits.append(
                FileEdit(
                    file=file,
                    original_lines=lines,
                    new_lines=new_lines,
                    changed_line_indices=changed_indices,
                    deleted_line_indices=deleted_indices,
                )
            )

    return edits


def format_edit_diff(edit: FileEdit, color: bool = True) -> str:
    """Render a file edit as a unified-diff-style preview."""
    import difflib

    rel = _rel(edit.file)
    diff = difflib.unified_diff(
        [l + "\n" for l in edit.original_lines],
        [l + "\n" for l in edit.new_lines],
        fromfile=f"a/{rel}",
        tofile=f"b/{rel}",
        n=1,
        lineterm="",
    )
    out: list[str] = []
    for line in diff:
        line = line.rstrip("\n")
        if not color:
            out.append(line)
            continue
        if line.startswith("+++") or line.startswith("---"):
            out.append(f"\033[1m{line}\033[0m")
        elif line.startswith("@@"):
            out.append(f"\033[36m{line}\033[0m")
        elif line.startswith("+"):
            out.append(f"\033[32m{line}\033[0m")
        elif line.startswith("-"):
            out.append(f"\033[31m{line}\033[0m")
        else:
            out.append(line)
    return "\n".join(out)


def strip_line_numbers_in_line(line: str) -> tuple[str | None, bool]:
    """
    Strip ephemeral `:NNN` / `:NNN-NNN` line-number references from
    `path/to/file.rs:NNN` patterns inside comments. The file path is
    preserved; only the colon-and-line-number suffix is removed.

    Returns `(new_line, changed)` matching `remove_annotations_from_line`.
    Lines that became empty after stripping are returned as `None`
    (deletion request).
    """
    new_line = line
    changed = False
    # Walk all matches; only edit those inside comment regions
    # `finditer` is fine — we don't modify positions until after we've
    # collected the matches we want.
    matches = list(LINE_NUMBER_REF_RE.finditer(line))
    if not matches:
        return new_line, False
    # Build replacements as (start, end, replacement) tuples in source
    # order, then apply them right-to-left so earlier offsets stay valid.
    replacements: list[tuple[int, int, str]] = []
    for m in matches:
        path = m.group(1)
        suffix = m.group(2)  # `:NNN` or `:NNN-NNN`
        start = m.start()
        end = m.end()
        # Only strip if the match is inside a comment region
        if not is_in_comment_region(line, start):
            continue
        replacements.append((start, end, path))
    if not replacements:
        return new_line, False
    for start, end, replacement in reversed(replacements):
        new_line = new_line[:start] + replacement + new_line[end:]
        changed = True

    if not changed:
        return new_line, False

    # Strip trailing whitespace; preserve internal indentation.
    new_line = new_line.rstrip()
    return new_line, True


def plan_line_number_edits(
    scope_paths: list[Path],
    include_ori: bool = False,
) -> list[FileEdit]:
    """
    Scan source files for stale `path.rs:NNN` references inside comments
    and produce a FileEdit per file that needs changes. Reuses the file
    walk + comment detection from the annotation classifier.
    """
    # Find candidate files via grep — fast and respects SCAN_EXCLUDE_DIRS
    if not scope_paths:
        scope_paths = [REPO_ROOT]
    cmd: list[str] = ["grep", "-rPln", "--include=*.rs"]
    if include_ori:
        cmd.append("--include=*.ori")
    for d in SCAN_EXCLUDE_DIRS:
        cmd.append(f"--exclude-dir={Path(d).name}")
    cmd.append(LINE_NUMBER_REF_RE.pattern)
    cmd.extend(str(p) for p in scope_paths)
    try:
        proc = subprocess.run(
            cmd, capture_output=True, text=True, check=False, cwd=REPO_ROOT
        )
    except (subprocess.SubprocessError, FileNotFoundError) as e:
        print(f"error: failed to scan source: {e}", file=sys.stderr)
        return []
    candidate_files = [
        Path(p) for p in proc.stdout.splitlines() if p
    ]

    edits: list[FileEdit] = []
    for file in sorted(set(candidate_files), key=str):
        try:
            text = file.read_text(encoding="utf-8")
        except OSError:
            continue
        lines = text.splitlines(keepends=False)
        new_lines: list[str] = []
        changed_indices: list[int] = []
        deleted_indices: list[int] = []
        for idx, original in enumerate(lines):
            new_line, changed = strip_line_numbers_in_line(original)
            if not changed:
                new_lines.append(original)
                continue
            if new_line is None:
                deleted_indices.append(idx)
                continue
            new_lines.append(new_line)
            changed_indices.append(idx)
        if changed_indices or deleted_indices:
            edits.append(
                FileEdit(
                    file=file,
                    original_lines=lines,
                    new_lines=new_lines,
                    changed_line_indices=changed_indices,
                    deleted_line_indices=deleted_indices,
                )
            )
    return edits


def apply_file_edits(edits: list[FileEdit]) -> int:
    """Write each edit to disk. Returns the number of files written."""
    written = 0
    for edit in edits:
        # Preserve trailing newline if the original had one
        try:
            original_text = edit.file.read_text(encoding="utf-8")
        except OSError:
            continue
        had_trailing_nl = original_text.endswith("\n")
        new_text = "\n".join(edit.new_lines)
        if had_trailing_nl:
            new_text += "\n"
        try:
            edit.file.write_text(new_text, encoding="utf-8")
            written += 1
        except OSError as e:
            print(f"error: failed to write {_rel(edit.file)}: {e}", file=sys.stderr)
    return written


# ─────────────────────────────────────────────────────────────
# Output
# ─────────────────────────────────────────────────────────────


SEVERITY_ORDER = [
    Classification.STALE_RESOLVED,
    Classification.STALE_COMPLETED_PLAN,
    Classification.ORPHAN,
    Classification.ACTIVE_SCAFFOLDING,
    Classification.SECTION_REF,
    Classification.PERMANENT,
    Classification.ARCH_INTERNAL,
]

CLEANUP_CLASSIFICATIONS = {
    Classification.STALE_RESOLVED,
    Classification.STALE_COMPLETED_PLAN,
    Classification.ORPHAN,
}

# ANSI color codes (no dependency on `colorama`)
_COLORS = {
    Classification.STALE_RESOLVED: "\033[1;31m",  # red bold
    Classification.STALE_COMPLETED_PLAN: "\033[0;31m",  # red
    Classification.ORPHAN: "\033[1;33m",  # yellow bold
    Classification.ACTIVE_SCAFFOLDING: "\033[0;32m",  # green
    Classification.SECTION_REF: "\033[0;36m",  # cyan
    Classification.PERMANENT: "\033[0;90m",  # gray
    Classification.ARCH_INTERNAL: "\033[0;90m",  # gray
}
_RESET = "\033[0m"


def _label(cls: Classification, color: bool) -> str:
    name = cls.value.upper().replace("-", "_")
    if color:
        return f"{_COLORS.get(cls, '')}{name}{_RESET}"
    return name


def _rel(file: Path) -> str:
    try:
        return str(file.resolve().relative_to(REPO_ROOT))
    except ValueError:
        return str(file)


def format_human(
    annotations: list[Annotation],
    shown_classes: set[Classification],
    color: bool = True,
) -> str:
    """Human-readable grouped output."""
    # Group by (classification, finding_id_or_section)
    groups: dict[tuple[Classification, str], list[Annotation]] = defaultdict(list)
    for ann in annotations:
        if ann.classification not in shown_classes:
            continue
        key_id = ann.finding_id or ann.plan_path_ref or ann.section_num or "?"
        groups[(ann.classification, key_id)].append(ann)

    out: list[str] = []
    for cls in SEVERITY_ORDER:
        cls_groups = [
            (k, v) for k, v in groups.items() if k[0] == cls
        ]
        if not cls_groups:
            continue
        total = sum(len(v) for _, v in cls_groups)
        header = f"\n=== {_label(cls, color)} ({total} annotations, {len(cls_groups)} distinct IDs) ==="
        out.append(header)
        for (_, key_id), anns in sorted(cls_groups, key=lambda kv: kv[0][1]):
            # Group header line with resolution info if available
            first = anns[0]
            plan_info = ""
            if first.plan_entry is not None:
                pe = first.plan_entry
                state = "[x]" if pe.is_resolved else "[ ]"
                plan_info = f"  {state} in {_rel(pe.source_file)}:{pe.source_line}"
            out.append(f"  {key_id}{plan_info}")
            for ann in anns:
                out.append(f"    {_rel(ann.file)}:{ann.line}")
    return "\n".join(out) + "\n" if out else ""


def format_counts(annotations: list[Annotation]) -> str:
    """Per-classification counts."""
    counts: dict[Classification, int] = defaultdict(int)
    distinct_ids: dict[Classification, set[str]] = defaultdict(set)
    for ann in annotations:
        if ann.classification is None:
            continue
        counts[ann.classification] += 1
        if ann.finding_id:
            distinct_ids[ann.classification].add(ann.finding_id)

    out: list[str] = []
    out.append(f"{'Classification':<28}  {'Count':>7}  {'Distinct IDs':>13}")
    out.append("─" * 56)
    for cls in SEVERITY_ORDER:
        n = counts.get(cls, 0)
        ids = len(distinct_ids.get(cls, set()))
        if n == 0:
            continue
        out.append(f"{cls.value:<28}  {n:>7}  {ids:>13}")
    total = sum(counts.values())
    out.append("─" * 56)
    out.append(f"{'TOTAL':<28}  {total:>7}")
    return "\n".join(out) + "\n"


def format_json(annotations: list[Annotation]) -> str:
    payload: list[dict] = []
    for ann in annotations:
        row = {
            "file": _rel(ann.file),
            "line": ann.line,
            "raw_text": ann.raw_text,
            "classification": ann.classification.value if ann.classification else None,
            "finding_id": ann.finding_id,
            "section_num": ann.section_num,
            "plan_path_ref": ann.plan_path_ref,
        }
        if ann.plan_entry is not None:
            row["plan_entry"] = {
                "plan": ann.plan_entry.plan.name,
                "plan_status": ann.plan_entry.plan.status.value,
                "plan_source": _rel(ann.plan_entry.source_file),
                "plan_line": ann.plan_entry.source_line,
                "is_resolved": ann.plan_entry.is_resolved,
            }
        payload.append(row)
    return json.dumps(payload, indent=2)


# ─────────────────────────────────────────────────────────────
# Main
# ─────────────────────────────────────────────────────────────


def build_argparser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="plan-annotations.py",
        description=(
            "Classify plan annotations in source code against plan status. "
            "See the module docstring for classification definitions."
        ),
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "EXAMPLES:\n"
            "  plan-annotations.py --scope compiler/ori_llvm/src/aot\n"
            "  plan-annotations.py --all --count\n"
            "  plan-annotations.py --cleanup-only\n"
            "  plan-annotations.py --orphans-only\n"
            "  plan-annotations.py --plan bug-tracker --cleanup-only\n"
            "  plan-annotations.py --json --scope compiler/ori_arc\n"
        ),
    )
    mode = p.add_mutually_exclusive_group()
    mode.add_argument(
        "--all",
        action="store_true",
        help="Show every classification including ACTIVE and PERMANENT.",
    )
    mode.add_argument(
        "--cleanup-only",
        action="store_true",
        help="Show only STALE_RESOLVED and STALE_COMPLETED_PLAN.",
    )
    mode.add_argument(
        "--orphans-only",
        action="store_true",
        help="Show only ORPHAN annotations (IDs not found in any plan).",
    )
    mode.add_argument(
        "--active-only",
        action="store_true",
        help="Show only ACTIVE_SCAFFOLDING annotations (in-progress work).",
    )
    p.add_argument(
        "--scope",
        nargs="+",
        metavar="PATH",
        help="Restrict scanning to the given paths. Recommended for reviews.",
    )
    p.add_argument(
        "--plan",
        metavar="NAME",
        help="Filter to annotations referencing a specific plan (by name).",
    )
    p.add_argument(
        "--json",
        action="store_true",
        help="Machine-readable JSON output.",
    )
    p.add_argument(
        "--count",
        action="store_true",
        help="Per-classification count summary only.",
    )
    p.add_argument(
        "--include-ori",
        action="store_true",
        help="Also scan .ori files (default: .rs only).",
    )
    p.add_argument(
        "--no-color",
        action="store_true",
        help="Disable ANSI color in the output.",
    )
    p.add_argument(
        "--pattern",
        action="store_true",
        help="Print the master annotation regex and exit.",
    )
    p.add_argument(
        "--fix",
        action="store_true",
        help=(
            "Remove STALE_RESOLVED + STALE_COMPLETED_PLAN + ORPHAN annotations "
            "from source files. Conservative: only edits inside //, ///, //!, "
            "/* */ comment regions; never touches code or strings. Default is "
            "dry-run (preview only); pair with --apply to write."
        ),
    )
    p.add_argument(
        "--apply",
        action="store_true",
        help=(
            "With --fix or --strip-line-numbers, actually write the "
            "modified files. Without --apply, the change is previewed as "
            "a unified diff and exit code 2 is returned."
        ),
    )
    p.add_argument(
        "--strip-line-numbers",
        action="store_true",
        help=(
            "Strip ephemeral `:NNN` line-number suffixes from "
            "`path/to/file.rs:NNN` references inside source comments. "
            "The file path is preserved; only the colon-and-line-number "
            "part is removed. Default is dry-run; pair with --apply to "
            "write."
        ),
    )
    return p


def default_shown_classes() -> set[Classification]:
    """Default mode shows stale + orphan (cleanup candidates)."""
    return {
        Classification.STALE_RESOLVED,
        Classification.STALE_COMPLETED_PLAN,
        Classification.ORPHAN,
    }


def select_shown_classes(args: argparse.Namespace) -> set[Classification]:
    if args.all:
        return set(Classification)
    if args.cleanup_only:
        return {Classification.STALE_RESOLVED, Classification.STALE_COMPLETED_PLAN}
    if args.orphans_only:
        return {Classification.ORPHAN}
    if args.active_only:
        return {Classification.ACTIVE_SCAFFOLDING}
    return default_shown_classes()


def filter_by_plan(
    annotations: list[Annotation], plan_name: str
) -> list[Annotation]:
    """Filter to annotations whose resolved plan entry matches the given name."""
    result: list[Annotation] = []
    # Allow numeric suffix (`--plan 04`) to match plans with that section number.
    numeric = plan_name.lstrip("0") if plan_name.isdigit() else None
    for ann in annotations:
        if ann.plan_entry is not None:
            # Match by plan name (substring) or exact
            if plan_name in ann.plan_entry.plan.name:
                result.append(ann)
                continue
        if numeric is not None and ann.finding_id:
            # BUG-04-045, TPR-07-019 — match on the first numeric chunk
            m = re.match(r"^[A-Z-]+-(\d+)-", ann.finding_id)
            if m and m.group(1).lstrip("0") == numeric:
                result.append(ann)
                continue
        if numeric is not None and ann.section_num:
            first_num = ann.section_num.split(".")[0].lstrip("0")
            if first_num == numeric:
                result.append(ann)
                continue
        if numeric is not None and ann.plan_path_ref:
            m = re.search(r"section-0*(\d+)", ann.plan_path_ref)
            if m and m.group(1).lstrip("0") == numeric:
                result.append(ann)
                continue
    return result


def main(argv: list[str] | None = None) -> int:
    parser = build_argparser()
    args = parser.parse_args(argv)

    if args.pattern:
        print(MASTER_GREP_PATTERN)
        return 0

    # 1. Index plans
    plans = discover_plans()
    plan_entries = index_plan_entries(plans)

    # 2. Scan source code
    scope_paths: list[Path] = []
    if args.scope:
        scope_paths = [REPO_ROOT / p if not Path(p).is_absolute() else Path(p)
                       for p in args.scope]
    grep_hits = run_grep(scope_paths, include_ori=args.include_ori)

    # 3. Extract and classify
    annotations = extract_annotations(grep_hits)
    classify_all(annotations, plan_entries, plans)

    # 4. Filter
    if args.plan:
        annotations = filter_by_plan(annotations, args.plan)

    color = sys.stdout.isatty() and not args.no_color

    # 4.4. Line-number stripping (independent of --fix)
    if args.strip_line_numbers:
        edits = plan_line_number_edits(scope_paths, include_ori=args.include_ori)
        if not edits:
            print("No stale line-number references found.")
            return 0
        if not args.apply:
            for edit in edits:
                print(format_edit_diff(edit, color=color))
                print()
            total_changed = sum(len(e.changed_line_indices) for e in edits)
            total_deleted = sum(len(e.deleted_line_indices) for e in edits)
            print("─" * 56, file=sys.stderr)
            print(
                f"DRY-RUN: would edit {len(edits)} files "
                f"({total_changed} lines modified, {total_deleted} lines deleted)",
                file=sys.stderr,
            )
            print("Pass --apply to write the changes.", file=sys.stderr)
            return 2
        written = apply_file_edits(edits)
        total_changed = sum(len(e.changed_line_indices) for e in edits)
        total_deleted = sum(len(e.deleted_line_indices) for e in edits)
        print(
            f"applied {written} files: "
            f"{total_changed} lines modified, {total_deleted} lines deleted"
        )
        return 0

    # 4.5. Fix mode (dry-run preview or apply)
    if args.fix:
        # Always operate on the cleanup set: STALE_RESOLVED + STALE_COMPLETED_PLAN + ORPHAN
        cleanup_classes = {
            Classification.STALE_RESOLVED,
            Classification.STALE_COMPLETED_PLAN,
            Classification.ORPHAN,
        }
        to_fix = [a for a in annotations if a.classification in cleanup_classes]
        edits = plan_file_edits(to_fix)

        # Account for annotations that are NOT in safe comment regions and were skipped
        edited_locs: set[tuple[Path, int]] = set()
        for e in edits:
            for idx in e.changed_line_indices:
                edited_locs.add((e.file, idx + 1))
            for idx in e.deleted_line_indices:
                edited_locs.add((e.file, idx + 1))
        skipped: list[Annotation] = []
        for ann in to_fix:
            if (ann.file, ann.line) not in edited_locs:
                skipped.append(ann)

        if not edits:
            print("No removable annotations found.")
            if skipped:
                print(
                    f"Note: {len(skipped)} annotation(s) were classified for "
                    "cleanup but could not be safely removed (not inside a "
                    "comment region or no removal pattern matched).",
                    file=sys.stderr,
                )
            return 0

        if not args.apply:
            # Dry-run: print unified diff preview
            for edit in edits:
                print(format_edit_diff(edit, color=color))
                print()
            total_changed = sum(len(e.changed_line_indices) for e in edits)
            total_deleted = sum(len(e.deleted_line_indices) for e in edits)
            print("─" * 56, file=sys.stderr)
            print(
                f"DRY-RUN: would edit {len(edits)} files "
                f"({total_changed} lines modified, {total_deleted} lines deleted)",
                file=sys.stderr,
            )
            if skipped:
                print(
                    f"  skipped {len(skipped)} annotation(s) — not in a comment "
                    "region or no removal pattern matched",
                    file=sys.stderr,
                )
            print("Pass --apply to write the changes.", file=sys.stderr)
            return 2  # non-zero so callers know preview-only

        # Apply
        written = apply_file_edits(edits)
        total_changed = sum(len(e.changed_line_indices) for e in edits)
        total_deleted = sum(len(e.deleted_line_indices) for e in edits)
        print(
            f"applied {written} files: "
            f"{total_changed} lines modified, {total_deleted} lines deleted"
        )
        if skipped:
            print(
                f"skipped {len(skipped)} annotation(s) — not in a comment "
                "region or no removal pattern matched",
                file=sys.stderr,
            )
        return 0

    # 5. Output

    if args.json:
        shown_classes = select_shown_classes(args)
        filtered = [a for a in annotations if a.classification in shown_classes]
        print(format_json(filtered))
        return 0

    if args.count:
        print(format_counts(annotations))
        return 0

    shown_classes = select_shown_classes(args)
    body = format_human(annotations, shown_classes, color=color)

    # Summary always shows counts regardless of filter — honest accounting
    summary_lines: list[str] = []
    summary_lines.append("")
    summary_lines.append("─" * 56)
    summary_lines.append("Plan-annotation classification summary")
    summary_lines.append("─" * 56)
    total_counts: dict[Classification, int] = defaultdict(int)
    for ann in annotations:
        if ann.classification is not None:
            total_counts[ann.classification] += 1
    total = sum(total_counts.values())
    shown_total = sum(
        n for cls, n in total_counts.items() if cls in shown_classes
    )
    hidden_total = total - shown_total
    for cls in SEVERITY_ORDER:
        n = total_counts.get(cls, 0)
        if n == 0:
            continue
        marker = " (shown)" if cls in shown_classes else " (hidden)"
        summary_lines.append(f"  {cls.value:<25} {n:>5}{marker}")
    summary_lines.append("─" * 56)
    summary_lines.append(f"  TOTAL                   {total:>5}")
    summary_lines.append(f"  Shown in this report:   {shown_total:>5}")
    summary_lines.append(f"  Hidden by current mode: {hidden_total:>5}")
    summary_lines.append("─" * 56)

    if shown_total == 0:
        print("No annotations match the current mode.")
    else:
        print(body, end="")
    print("\n".join(summary_lines))

    # Exit code: non-zero if stale/orphan exist and default mode
    if not (args.all or args.active_only) and shown_total > 0:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
