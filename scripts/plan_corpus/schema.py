"""Schema definitions and validation for the seven plan file classes.

Contains FileClass, classify_file, status enums, dataclass-typed schema SSOTs,
all _check_* helpers, all _validate_* per-schema functions, and validate().

The seven `@dataclass(frozen=True)` schema classes (PlanIndexSchema etc.) are
the single source of truth for required/allowed field sets. Required fields
have no default; optional fields default to None. Validators derive the
required list and allowed set from `dataclasses.fields()` at call time.
"""

from __future__ import annotations

import enum
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .types import (
    Finding,
    FindingCategory,
    FindingSubtype,
    Severity,
    PLANS_DIR,
    ROADMAP_DIR,
    FIX_BUG_RE,
    SECTION_FILE_RE,
    OVERVIEW_RE,
    INTRA_PLAN_DEP_RE,
    CROSS_PLAN_DEP_RE,
    FULL_PATH_DEP_RE,
)
from .schemas import (
    PlanIndexSchema,
    PlanSectionSchema,
    RoadmapSectionSchema,
    OverviewSchema,
    BugTrackerSectionSchema,
    FixBugSchema,
    CompletedIndexSchema,
    _schema_required_fields,
    _schema_allowed_fields,
)


# ---------------------------------------------------------------------------
# File classification
# ---------------------------------------------------------------------------


class FileClass(enum.Enum):
    """The seven schema classes for plan files."""
    PLAN_INDEX = "plan_index"
    PLAN_SECTION = "plan_section"
    ROADMAP_SECTION = "roadmap_section"
    OVERVIEW = "overview"
    BUG_TRACKER_SECTION = "bug_tracker_section"
    FIX_BUG = "fix_bug"
    COMPLETED_INDEX = "completed_index"


def classify_file(path: Path) -> FileClass | None:
    """Classify a plan file into one of the seven schema classes."""
    path = path.resolve()
    parts = path.relative_to(PLANS_DIR).parts if path.is_relative_to(PLANS_DIR) else path.parts

    name = path.name

    # Completed plan indexes
    if len(parts) >= 2 and parts[0] == "completed" and name == "index.md":
        return FileClass.COMPLETED_INDEX

    # Bug-tracker fix files
    if len(parts) >= 2 and parts[0] == "bug-tracker" and FIX_BUG_RE.match(name):
        return FileClass.FIX_BUG

    # Bug-tracker section files
    if len(parts) >= 2 and parts[0] == "bug-tracker" and SECTION_FILE_RE.match(name):
        return FileClass.BUG_TRACKER_SECTION

    # Roadmap section files
    if len(parts) >= 2 and parts[0] == "roadmap" and SECTION_FILE_RE.match(name):
        return FileClass.ROADMAP_SECTION

    # Overview files
    if OVERVIEW_RE.match(name):
        return FileClass.OVERVIEW

    # Plan index files
    if name == "index.md":
        return FileClass.PLAN_INDEX

    # Plan section files
    if SECTION_FILE_RE.match(name):
        return FileClass.PLAN_SECTION

    return None


# ---------------------------------------------------------------------------
# Status enums (corpus-derived)
# ---------------------------------------------------------------------------

PLAN_STATUSES = frozenset({"active", "queued", "resolved", "not-started", "research"})
SECTION_STATUSES = frozenset({"not-started", "in-progress", "complete"})
OVERVIEW_STATUSES = frozenset({"not-started", "in-progress", "research", "complete"})
FIX_STATUSES = frozenset({"not-started", "in-progress", "complete"})
TPR_STATUSES = frozenset({"none", "findings", "resolved", "clean"})
SEVERITY_VALUES = frozenset({"critical", "high", "medium", "low"})
COMPLETED_STATUSES = frozenset({"resolved"})


# ---------------------------------------------------------------------------
# Schema helper types
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class SubsectionEntry:
    id: str
    title: str
    status: str


@dataclass(frozen=True)
class TprInfo:
    status: str
    updated: str | None


_SECTION_ENTRY_ALLOWED = frozenset({"id", "title", "status"})
_SECTION_ENTRY_REQUIRED = frozenset({"id", "title", "status"})


def _validate_sections(data: dict, path: Path) -> list[Finding]:
    """Validate each entry in the sections[] list matches SubsectionEntry shape.

    Emits one finding per malformed entry (unknown field, missing required field,
    or enum violation on status). Silently returns [] if sections is absent or
    empty — the required-fields check at the top level handles absence.
    """
    findings: list[Finding] = []
    sections = data.get("sections")
    if sections is None:
        return findings
    if not isinstance(sections, list):
        findings.append(Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.WRONG_TYPE,
            severity=Severity.HIGH,
            source=path,
            description="sections must be a list",
            recommended_fix="Use sections: [{id: NN.N, title: ..., status: ...}, ...]",
        ))
        return findings
    for idx, entry in enumerate(sections):
        if not isinstance(entry, dict):
            findings.append(Finding(
                category=FindingCategory.SCHEMA_VIOLATION,
                subtype=FindingSubtype.WRONG_TYPE,
                severity=Severity.HIGH,
                source=path,
                description=f"sections[{idx}] must be a mapping, got {type(entry).__name__}",
                recommended_fix="Use a mapping with id/title/status keys",
            ))
            continue
        for required_field in _SECTION_ENTRY_REQUIRED:
            if required_field not in entry:
                findings.append(Finding(
                    category=FindingCategory.SCHEMA_VIOLATION,
                    subtype=FindingSubtype.MISSING_REQUIRED_FIELD,
                    severity=Severity.HIGH,
                    source=path,
                    description=f"sections[{idx}] missing required field {required_field!r}",
                    recommended_fix=f"Add {required_field}: ... to the entry",
                ))
        for key in entry:
            if key not in _SECTION_ENTRY_ALLOWED:
                findings.append(Finding(
                    category=FindingCategory.SCHEMA_VIOLATION,
                    subtype=FindingSubtype.UNKNOWN_FIELD,
                    severity=Severity.MEDIUM,
                    source=path,
                    description=f"sections[{idx}] has unknown field {key!r}",
                    recommended_fix=f"Remove {key!r} or add it to the SubsectionEntry schema",
                ))
        status = entry.get("status")
        if status is not None and status not in SECTION_STATUSES:
            findings.append(Finding(
                category=FindingCategory.SCHEMA_VIOLATION,
                subtype=FindingSubtype.ENUM_OUT_OF_RANGE,
                severity=Severity.MEDIUM,
                source=path,
                description=(
                    f"sections[{idx}].status={status!r} not in {sorted(SECTION_STATUSES)}"
                ),
                recommended_fix=f"Use one of {sorted(SECTION_STATUSES)}",
            ))
    return findings


# ---------------------------------------------------------------------------
# Validation helpers
# ---------------------------------------------------------------------------


def _validate_tpr_info(raw: Any, path: Path) -> tuple[TprInfo | None, list[Finding]]:
    """Validate a third_party_review block."""
    findings: list[Finding] = []
    if raw is None:
        return None, findings
    if not isinstance(raw, dict):
        findings.append(Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.WRONG_TYPE,
            severity=Severity.HIGH,
            source=path,
            description="third_party_review must be a mapping",
            recommended_fix="Use third_party_review: {status: none, updated: null}",
        ))
        return None, findings
    status = raw.get("status")
    if status is not None:
        status = str(status)
    updated = raw.get("updated")
    if updated is not None:
        updated = str(updated)
    if status and status not in TPR_STATUSES:
        findings.append(Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.ENUM_OUT_OF_RANGE,
            severity=Severity.HIGH,
            source=path,
            description=f"third_party_review.status={status!r} not in {sorted(TPR_STATUSES)}",
            recommended_fix=f"Use one of: {', '.join(sorted(TPR_STATUSES))}",
        ))
    if status and status != "none" and updated is None:
        findings.append(Finding(
            category=FindingCategory.STATUS_CONTRADICTION,
            subtype=FindingSubtype.TPR_STATUS_WITHOUT_DATE,
            severity=Severity.MEDIUM,
            source=path,
            description=f"third_party_review.status={status!r} but updated is null",
            recommended_fix="Set third_party_review.updated to today's date",
        ))
    if status == "none" and updated is not None and updated != "null":
        findings.append(Finding(
            category=FindingCategory.STATUS_CONTRADICTION,
            subtype=FindingSubtype.TPR_STATUS_NONE_WITH_DATE,
            severity=Severity.LOW,
            source=path,
            description=f"third_party_review.status=none but updated={updated!r}",
            recommended_fix="Set updated to null when status is none",
        ))
    return TprInfo(status=status or "none", updated=updated), findings


def _check_required(
    data: dict, required: list[str], path: Path
) -> list[Finding]:
    findings = []
    for key in required:
        if key not in data:
            findings.append(Finding(
                category=FindingCategory.SCHEMA_VIOLATION,
                subtype=FindingSubtype.MISSING_REQUIRED_FIELD,
                severity=Severity.HIGH,
                source=path,
                description=f"missing required field: {key}",
                recommended_fix=f"Add '{key}:' to frontmatter",
            ))
    return findings


def _check_unknown_fields(
    data: dict, allowed: frozenset[str], path: Path
) -> list[Finding]:
    findings = []
    for key in data:
        if key not in allowed:
            findings.append(Finding(
                category=FindingCategory.SCHEMA_VIOLATION,
                subtype=FindingSubtype.UNKNOWN_FIELD,
                severity=Severity.MEDIUM,
                source=path,
                description=f"unknown field: {key!r}",
                recommended_fix=f"Remove '{key}' or check for typos. Allowed: {sorted(allowed)}",
            ))
    return findings


def _check_enum(
    data: dict, key: str, allowed: frozenset[str], path: Path
) -> list[Finding]:
    val = data.get(key)
    if val is None:
        return []
    val_str = str(val).strip()
    # Strip inline comments (e.g. "in-progress  # TPR done")
    if "#" in val_str:
        val_str = val_str.split("#")[0].strip()
    if val_str not in allowed:
        return [Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.ENUM_OUT_OF_RANGE,
            severity=Severity.HIGH,
            source=path,
            description=f"{key}={val!r} not in {sorted(allowed)}",
            recommended_fix=f"Use one of: {', '.join(sorted(allowed))}",
        )]
    return []


def _validate_dep_id(dep: Any, path: Path) -> list[Finding]:
    """Validate a single depends_on entry."""
    if not isinstance(dep, str):
        return [Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.DEP_ID_MALFORMED,
            severity=Severity.HIGH,
            source=path,
            description=f"depends_on entry must be a string, got {type(dep).__name__}",
            recommended_fix="Use depends_on: [\"01\", \"02\"]",
        )]
    if FULL_PATH_DEP_RE.match(dep):
        return [Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.DEP_ID_FULL_PATH,
            severity=Severity.HIGH,
            source=path,
            description=f"depends_on uses full path: {dep!r}",
            recommended_fix="Use logical ID (\"01\") or cross-plan (\"plan-name#01\")",
        )]
    if not INTRA_PLAN_DEP_RE.match(dep) and not CROSS_PLAN_DEP_RE.match(dep):
        return [Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.DEP_ID_MALFORMED,
            severity=Severity.HIGH,
            source=path,
            description=f"malformed depends_on ID: {dep!r}",
            recommended_fix="Use \"NN\" (intra-plan) or \"plan-name#NN\" (cross-plan)",
        )]
    return []


def _validate_depends_on(data: dict, path: Path) -> list[Finding]:
    """Validate the depends_on field if present."""
    raw = data.get("depends_on")
    if raw is None:
        return []
    if isinstance(raw, str):
        return [Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.DEP_ID_MALFORMED,
            severity=Severity.HIGH,
            source=path,
            description="depends_on must be a list, not a bare string (would iterate chars)",
            recommended_fix=f'Use depends_on: ["{raw}"]',
        )]
    if not isinstance(raw, list):
        return [Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.WRONG_TYPE,
            severity=Severity.HIGH,
            source=path,
            description=f"depends_on must be a list, got {type(raw).__name__}",
            recommended_fix="Use depends_on: [\"01\"]",
        )]
    findings = []
    for dep in raw:
        findings.extend(_validate_dep_id(dep, path))
    return findings


# ---------------------------------------------------------------------------
# Per-schema validators — required/allowed derived from dataclasses above
# ---------------------------------------------------------------------------


def _validate_plan_index(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _schema_required_fields(PlanIndexSchema), path))
    findings.extend(_check_unknown_fields(data, _schema_allowed_fields(PlanIndexSchema), path))
    findings.extend(_check_enum(data, "status", PLAN_STATUSES, path))
    if data.get("reroute") is False:
        findings.append(Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.CROSS_FIELD_INVARIANT,
            severity=Severity.MEDIUM,
            source=path,
            description="reroute: false is invalid; omit the field instead",
            recommended_fix="Remove the reroute field entirely",
        ))
    return findings


def _validate_plan_section(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _schema_required_fields(PlanSectionSchema), path))
    findings.extend(_check_unknown_fields(data, _schema_allowed_fields(PlanSectionSchema), path))
    findings.extend(_check_enum(data, "status", SECTION_STATUSES, path))
    findings.extend(_validate_depends_on(data, path))
    findings.extend(_validate_sections(data, path))
    _, tpr_findings = _validate_tpr_info(data.get("third_party_review"), path)
    findings.extend(tpr_findings)
    return findings


def _validate_roadmap_section(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _schema_required_fields(RoadmapSectionSchema), path))
    findings.extend(_check_unknown_fields(data, _schema_allowed_fields(RoadmapSectionSchema), path))
    findings.extend(_check_enum(data, "status", SECTION_STATUSES, path))
    findings.extend(_validate_depends_on(data, path))
    findings.extend(_validate_sections(data, path))
    if "third_party_review" in data:
        _, tpr_findings = _validate_tpr_info(data.get("third_party_review"), path)
        findings.extend(tpr_findings)
    return findings


def _validate_overview(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _schema_required_fields(OverviewSchema), path))
    findings.extend(_check_unknown_fields(data, _schema_allowed_fields(OverviewSchema), path))
    findings.extend(_check_enum(data, "status", OVERVIEW_STATUSES, path))
    return findings


def _validate_bug_section(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _schema_required_fields(BugTrackerSectionSchema), path))
    findings.extend(_check_unknown_fields(data, _schema_allowed_fields(BugTrackerSectionSchema), path))
    findings.extend(_check_enum(data, "status", SECTION_STATUSES, path))
    return findings


def _validate_fix_bug(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _schema_required_fields(FixBugSchema), path))
    findings.extend(_check_unknown_fields(data, _schema_allowed_fields(FixBugSchema), path))
    findings.extend(_check_enum(data, "status", FIX_STATUSES, path))
    findings.extend(_check_enum(data, "severity", SEVERITY_VALUES, path))
    _, tpr_findings = _validate_tpr_info(data.get("third_party_review"), path)
    findings.extend(tpr_findings)
    return findings


def _validate_completed_index(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _schema_required_fields(CompletedIndexSchema), path))
    findings.extend(_check_unknown_fields(data, _schema_allowed_fields(CompletedIndexSchema), path))
    findings.extend(_check_enum(data, "status", COMPLETED_STATUSES, path))
    return findings


def validate(file_class: FileClass, data: dict, path: Path) -> list[Finding]:
    """Validate parsed frontmatter against the schema for the given file class."""
    validators = {
        FileClass.PLAN_INDEX: _validate_plan_index,
        FileClass.PLAN_SECTION: _validate_plan_section,
        FileClass.ROADMAP_SECTION: _validate_roadmap_section,
        FileClass.OVERVIEW: _validate_overview,
        FileClass.BUG_TRACKER_SECTION: _validate_bug_section,
        FileClass.FIX_BUG: _validate_fix_bug,
        FileClass.COMPLETED_INDEX: _validate_completed_index,
    }
    validator = validators.get(file_class)
    if validator is None:
        return []
    return validator(data, path)
