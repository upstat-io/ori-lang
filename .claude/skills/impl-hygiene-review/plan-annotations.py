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
`--fix --dry-run`          Show what would be removed from source files.
                           (Not yet implemented — reserved.)
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

# Finding IDs: TPR-07-019, BUG-04-045, TPR-BUG-04-045-07, CROSS-04-014, etc.
# Intentionally loose — we capture and then look up against the plan index.
# Order matters: TPR-BUG must come before TPR to prefer the longer match.
FINDING_ID_RE = re.compile(
    r"\b((?:TPR-BUG|TPR|CROSS|BUG|FIND|TASK|ISSUE)-\d+-\d+(?:-\d+)?\w*)\b"
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

    # Checkbox pattern: `- [ ] ...[ID]...` or `- [x] ...[ID]...`
    # The backtick and leading code-fence are optional, and the entire
    # entry may span multiple lines (body follows) — we only need the
    # checkbox line.
    checkbox_re = re.compile(
        r"^\s*-\s+\[([ x])\]\s+.*?\b([A-Z]+(?:-[A-Z]+)*-\d+(?:-\d+)?)\b"
    )

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
                # The regex can capture sub-tokens of longer IDs; re-extract
                # to prefer the longest match at this position.
                longest = FINDING_ID_RE.search(line)
                if longest:
                    finding_id = longest.group(1)
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

    # 5. Output
    color = sys.stdout.isatty() and not args.no_color

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
