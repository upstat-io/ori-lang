#!/usr/bin/env python3
"""Corpus SSOT for Ori compiler plan tooling.

This module is the **sole** home for:
  - Strict YAML frontmatter parsing (hard-fail, no silent coercion)
  - Plan directory discovery with two-stage classification
  - Seven frontmatter schema definitions as Python dataclasses
  - Shared Finding / Severity / FindingCategory / FindingSubtype types
  - Canonical status normalizer (facts only, no policy)
  - load_and_validate boundary function

Sections 02-05 of the verify-roadmap-redesign plan MUST import from here.
Re-implementing these types elsewhere is a LEAK:algorithmic-duplication violation.

Supersedes: .claude/skills/plan-audit/planlib.py (permissive parser with
errors="replace" and silent YAML error swallowing).
"""

from __future__ import annotations

import enum
import hashlib
import json
import os
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Literal

import yaml

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

REPO_ROOT = Path(__file__).resolve().parent.parent
PLANS_DIR = REPO_ROOT / "plans"
COMPLETED_DIR = PLANS_DIR / "completed"
BUG_TRACKER_DIR = PLANS_DIR / "bug-tracker"
ROADMAP_DIR = PLANS_DIR / "roadmap"

FRONTMATTER_FENCE = re.compile(r"^---\s*$")
YAML_ANCHOR_RE = re.compile(r"[&*]\w")
YAML_MERGE_KEY_RE = re.compile(r"<<\s*:")
ZERO_WIDTH_CHARS = frozenset("\u200b\ufeff\u200e\u200f")

# DepId patterns
INTRA_PLAN_DEP_RE = re.compile(r"^[0-9]+[A-Za-z]*$")
CROSS_PLAN_DEP_RE = re.compile(r"^(.+)#([0-9]+[A-Za-z]*)$")
FULL_PATH_DEP_RE = re.compile(r"^plans/")

# File classification patterns
FIX_BUG_RE = re.compile(r"fix-BUG-\d{2}-\d{3}\.md$")
SECTION_FILE_RE = re.compile(r"section-\d{2}.*\.md$")
OVERVIEW_RE = re.compile(r"00-overview\.md$")

# ---------------------------------------------------------------------------
# §01.3 — Severity & Finding Taxonomy
# ---------------------------------------------------------------------------


class Severity(enum.IntEnum):
    """Finding severity, ordered for comparison."""
    LOW = 0
    MEDIUM = 1
    HIGH = 2
    CRITICAL = 3


class FindingCategory(enum.Enum):
    """Top-level finding category (two-level hierarchy with FindingSubtype)."""
    PARSE_ERROR = "parse_error"
    SCHEMA_VIOLATION = "schema_violation"
    STATUS_CONTRADICTION = "status_contradiction"
    DAG_CONFLICT = "dag_conflict"
    DEAD_REFERENCE = "dead_reference"
    ITEM_VERIFICATION = "item_verification"
    GAP = "gap"


class FindingSubtype(enum.Enum):
    """Fine-grained subtype scoped per FindingCategory."""

    # PARSE_ERROR subtypes
    MISSING_OPENING_DASHES = "missing_opening_dashes"
    UNCLOSED_FRONTMATTER = "unclosed_frontmatter"
    YAML_SYNTAX_ERROR = "yaml_syntax_error"
    NON_MAPPING_ROOT = "non_mapping_root"
    DUPLICATE_KEY = "duplicate_key"
    YAML_ANCHOR = "yaml_anchor"
    YAML_MERGE_KEY = "yaml_merge_key"
    MULTI_DOCUMENT = "multi_document"
    UTF8_BOM = "utf8_bom"
    ZERO_WIDTH_BEFORE_FM = "zero_width_before_fm"
    INVALID_UTF8_BYTES = "invalid_utf8_bytes"
    CRLF_BOUNDARY_DRIFT = "crlf_boundary_drift"

    # SCHEMA_VIOLATION subtypes
    UNKNOWN_FIELD = "unknown_field"
    MISSING_REQUIRED_FIELD = "missing_required_field"
    WRONG_TYPE = "wrong_type"
    ENUM_OUT_OF_RANGE = "enum_out_of_range"
    CROSS_FIELD_INVARIANT = "cross_field_invariant"
    DUPLICATE_PLAN_NAME = "duplicate_plan_name"
    DEP_ID_MALFORMED = "dep_id_malformed"
    DEP_ID_FULL_PATH = "dep_id_full_path"
    DEP_ID_UNKNOWN_NAME = "dep_id_unknown_name"

    # STATUS_CONTRADICTION subtypes
    FM_DECLARED_VS_BODY_DERIVED = "fm_declared_vs_body_derived"
    PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED = "plan_active_all_sections_not_started"
    PLAN_COMPLETE_WITH_OPEN_SECTIONS = "plan_complete_with_open_sections"
    TPR_STATUS_WITHOUT_DATE = "tpr_status_without_date"
    TPR_STATUS_NONE_WITH_DATE = "tpr_status_none_with_date"
    CROSS_EDGE_TEMPORAL_DRIFT = "cross_edge_temporal_drift"
    TPR_STALE_VS_EDIT = "tpr_stale_vs_edit"

    # DAG_CONFLICT subtypes
    CONFLICT = "conflict"
    SUPERSEDED = "superseded"
    BLOCKED = "blocked"
    MISSING_DEPENDENCY = "missing_dependency"
    CYCLE = "cycle"

    # DEAD_REFERENCE subtypes
    PLAN_DIRECTORY_NOT_FOUND = "plan_directory_not_found"
    SECTION_FILE_NOT_FOUND = "section_file_not_found"
    CROSS_PLAN_NAME_NOT_FOUND = "cross_plan_name_not_found"
    SPEC_FILE_NOT_FOUND = "spec_file_not_found"

    # ITEM_VERIFICATION subtypes (Section 04 imports these, does NOT redefine)
    MISSING_MATRIX_COVERAGE = "missing_matrix_coverage"
    MISSING_SEMANTIC_PIN = "missing_semantic_pin"
    MISSING_NEGATIVE_PIN = "missing_negative_pin"
    WEAK_TEST = "weak_test"
    HYGIENE_VIOLATION = "hygiene_violation"
    INCOMPLETE_CHECKBOX = "incomplete_checkbox"
    SCOPE_GAP = "scope_gap"

    # GAP subtypes
    MISSING_INDEX_MD = "missing_index_md"
    UNCLASSIFIED_DIRECTORY = "unclassified_directory"
    LEAK_SWALLOWED_ERROR = "leak_swallowed_error"


# Category → allowed subtypes mapping (enforced at Finding construction)
_CATEGORY_SUBTYPES: dict[FindingCategory, frozenset[FindingSubtype]] = {
    FindingCategory.PARSE_ERROR: frozenset({
        FindingSubtype.MISSING_OPENING_DASHES,
        FindingSubtype.UNCLOSED_FRONTMATTER,
        FindingSubtype.YAML_SYNTAX_ERROR,
        FindingSubtype.NON_MAPPING_ROOT,
        FindingSubtype.DUPLICATE_KEY,
        FindingSubtype.YAML_ANCHOR,
        FindingSubtype.YAML_MERGE_KEY,
        FindingSubtype.MULTI_DOCUMENT,
        FindingSubtype.UTF8_BOM,
        FindingSubtype.ZERO_WIDTH_BEFORE_FM,
        FindingSubtype.INVALID_UTF8_BYTES,
        FindingSubtype.CRLF_BOUNDARY_DRIFT,
    }),
    FindingCategory.SCHEMA_VIOLATION: frozenset({
        FindingSubtype.UNKNOWN_FIELD,
        FindingSubtype.MISSING_REQUIRED_FIELD,
        FindingSubtype.WRONG_TYPE,
        FindingSubtype.ENUM_OUT_OF_RANGE,
        FindingSubtype.CROSS_FIELD_INVARIANT,
        FindingSubtype.DUPLICATE_PLAN_NAME,
        FindingSubtype.DEP_ID_MALFORMED,
        FindingSubtype.DEP_ID_FULL_PATH,
        FindingSubtype.DEP_ID_UNKNOWN_NAME,
    }),
    FindingCategory.STATUS_CONTRADICTION: frozenset({
        FindingSubtype.FM_DECLARED_VS_BODY_DERIVED,
        FindingSubtype.PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED,
        FindingSubtype.PLAN_COMPLETE_WITH_OPEN_SECTIONS,
        FindingSubtype.TPR_STATUS_WITHOUT_DATE,
        FindingSubtype.TPR_STATUS_NONE_WITH_DATE,
        FindingSubtype.CROSS_EDGE_TEMPORAL_DRIFT,
        FindingSubtype.TPR_STALE_VS_EDIT,
    }),
    FindingCategory.DAG_CONFLICT: frozenset({
        FindingSubtype.CONFLICT,
        FindingSubtype.SUPERSEDED,
        FindingSubtype.BLOCKED,
        FindingSubtype.MISSING_DEPENDENCY,
        FindingSubtype.CYCLE,
    }),
    FindingCategory.DEAD_REFERENCE: frozenset({
        FindingSubtype.PLAN_DIRECTORY_NOT_FOUND,
        FindingSubtype.SECTION_FILE_NOT_FOUND,
        FindingSubtype.CROSS_PLAN_NAME_NOT_FOUND,
        FindingSubtype.SPEC_FILE_NOT_FOUND,
    }),
    FindingCategory.ITEM_VERIFICATION: frozenset({
        FindingSubtype.MISSING_MATRIX_COVERAGE,
        FindingSubtype.MISSING_SEMANTIC_PIN,
        FindingSubtype.MISSING_NEGATIVE_PIN,
        FindingSubtype.WEAK_TEST,
        FindingSubtype.HYGIENE_VIOLATION,
        FindingSubtype.INCOMPLETE_CHECKBOX,
        FindingSubtype.SCOPE_GAP,
    }),
    FindingCategory.GAP: frozenset({
        FindingSubtype.MISSING_INDEX_MD,
        FindingSubtype.UNCLASSIFIED_DIRECTORY,
        FindingSubtype.LEAK_SWALLOWED_ERROR,
    }),
}


@dataclass(frozen=True)
class Finding:
    """A single diagnostic finding. Frozen and hashable for deduplication."""

    category: FindingCategory
    subtype: FindingSubtype
    severity: Severity
    source: Path
    description: str
    recommended_fix: str
    source_line: int | None = None
    target: Path | None = None
    target_line: int | None = None
    evidence: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        allowed = _CATEGORY_SUBTYPES.get(self.category, frozenset())
        if self.subtype not in allowed:
            raise ValueError(
                f"FindingSubtype.{self.subtype.name} does not belong to "
                f"FindingCategory.{self.category.name}"
            )

    @property
    def id(self) -> str:
        content = f"{self.category.value}:{self.subtype.value}:{self.source}:{self.source_line}"
        return "VR-" + hashlib.sha256(content.encode()).hexdigest()[:6]

    def to_json(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "category": self.category.value,
            "subtype": self.subtype.value,
            "severity": self.severity.name.lower(),
            "source": str(self.source),
            "source_line": self.source_line,
            "target": str(self.target) if self.target else None,
            "target_line": self.target_line,
            "description": self.description,
            "recommended_fix": self.recommended_fix,
            "evidence": list(self.evidence),
        }

    def to_markdown(self) -> str:
        parts = [f"- **[{self.id}][{self.severity.name.lower()}]** `{self.source}`"]
        if self.source_line is not None:
            parts[0] += f":{self.source_line}"
        parts[0] += f" — {self.description}"
        if self.recommended_fix:
            parts.append(f"  Fix: {self.recommended_fix}")
        for ev in self.evidence:
            parts.append(f"  Evidence: {ev}")
        return "\n".join(parts)


# ---------------------------------------------------------------------------
# §01.1 — Strict Parser & Discovery
# ---------------------------------------------------------------------------


class CorpusParseError(Exception):
    """Hard-fail error from strict YAML parsing. Never silently swallowed."""

    def __init__(
        self,
        message: str,
        path: Path,
        line: int | None = None,
        column: int | None = None,
        yaml_error: Exception | None = None,
    ):
        self.path = path
        self.line = line
        self.column = column
        self.yaml_error = yaml_error
        loc = f"{path}"
        if line is not None:
            loc += f":{line}"
            if column is not None:
                loc += f":{column}"
        super().__init__(f"{loc}: {message}")


class _NoDuplicateKeyLoader(yaml.SafeLoader):
    """SafeLoader subclass that raises on duplicate mapping keys."""
    pass


def _no_duplicate_key_constructor(loader, node):
    keys_seen: set[str] = set()
    for key_node, _ in node.value:
        key = loader.construct_object(key_node)
        if key in keys_seen:
            mark = key_node.start_mark
            raise yaml.YAMLError(
                f"duplicate key: {key!r}",
                mark,
            )
        keys_seen.add(key)
    return loader.construct_mapping(node)


_NoDuplicateKeyLoader.add_constructor(
    yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG,
    _no_duplicate_key_constructor,
)


def read_text_strict(path: Path) -> str:
    """Read a file with strict UTF-8 decoding. Never replaces invalid bytes."""
    try:
        raw = path.read_bytes()
    except OSError as e:
        raise CorpusParseError(f"cannot read file: {e}", path) from e

    if raw[:3] == b"\xef\xbb\xbf":
        raise CorpusParseError(
            "UTF-8 BOM detected at byte 0; remove the BOM",
            path,
            line=1,
        )

    try:
        text = raw.decode("utf-8", errors="strict")
    except UnicodeDecodeError as e:
        raise CorpusParseError(
            f"invalid UTF-8 bytes at position {e.start}",
            path,
            yaml_error=e,
        ) from e

    first_line_end = text.find("\n")
    prefix = text[:first_line_end] if first_line_end >= 0 else text
    for i, ch in enumerate(prefix):
        if ch in ZERO_WIDTH_CHARS:
            raise CorpusParseError(
                f"zero-width character U+{ord(ch):04X} before opening ---",
                path,
                line=1,
                column=i + 1,
            )

    return text


def split_frontmatter_strict(
    text: str, path: Path
) -> tuple[dict[str, Any], int]:
    """Parse YAML frontmatter strictly. Returns (parsed_dict, body_line_offset).

    Raises CorpusParseError on any parse problem — never returns {} on error.
    """
    lines = text.split("\n")

    if not lines or lines[0].rstrip("\r") != "---":
        raise CorpusParseError(
            "missing opening --- on line 1",
            path,
            line=1,
        )

    end_idx = -1
    for i in range(1, len(lines)):
        stripped = lines[i].rstrip("\r")
        if stripped == "---":
            end_idx = i
            break
        if stripped == "...":
            end_idx = i
            break

    if end_idx < 0:
        raise CorpusParseError("unclosed frontmatter (no closing ---)", path, line=1)

    fm_lines = lines[1:end_idx]
    fm_text = "\n".join(fm_lines)

    # Pre-scan for anchors, aliases, merge keys
    for line_no, line in enumerate(fm_lines, start=2):
        if YAML_ANCHOR_RE.search(line):
            raise CorpusParseError(
                "YAML anchor/alias detected (schema-bypass vector)",
                path,
                line=line_no,
            )
        if YAML_MERGE_KEY_RE.search(line):
            raise CorpusParseError(
                "YAML merge key (<<:) detected",
                path,
                line=line_no,
            )

    # Check for multi-document markers inside frontmatter
    for line_no, line in enumerate(fm_lines, start=2):
        stripped = line.rstrip("\r")
        if stripped == "---" or stripped == "...":
            raise CorpusParseError(
                "multi-document YAML marker inside frontmatter",
                path,
                line=line_no,
            )

    # Check for CRLF inside frontmatter region
    has_crlf = any("\r" in line for line in fm_lines)
    if has_crlf:
        fm_text = fm_text.replace("\r\n", "\n").replace("\r", "\n")

    try:
        data = yaml.load(fm_text, Loader=_NoDuplicateKeyLoader)
    except yaml.YAMLError as e:
        line_no = None
        col = None
        if hasattr(e, "problem_mark") and e.problem_mark is not None:
            line_no = e.problem_mark.line + 2  # +1 for 0-index, +1 for opening ---
            col = e.problem_mark.column + 1
        raise CorpusParseError(
            f"YAML parse error: {e}",
            path,
            line=line_no,
            column=col,
            yaml_error=e,
        ) from e

    if data is None:
        raise CorpusParseError("frontmatter is empty (null)", path, line=1)

    if not isinstance(data, dict):
        raise CorpusParseError(
            f"frontmatter root must be a mapping, got {type(data).__name__}",
            path,
            line=1,
        )

    body_offset = end_idx + 1
    return data, body_offset


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
# Directory classification for discovery
# ---------------------------------------------------------------------------


class DirClass(enum.Enum):
    PLAN_CANDIDATE = "plan_candidate"
    CONTAINER = "container"
    UNKNOWN = "unknown"


_CONTAINER_DIRS = frozenset({"plans", "completed"})


def _classify_directory(dirpath: Path) -> DirClass:
    """Stage 1: classify a directory under plans/."""
    dirpath = dirpath.resolve()
    rel = dirpath.relative_to(PLANS_DIR) if dirpath.is_relative_to(PLANS_DIR) else dirpath
    dirname = rel.parts[-1] if rel.parts else dirpath.name

    if dirname in _CONTAINER_DIRS and len(rel.parts) <= 1:
        return DirClass.CONTAINER

    if (dirpath / "index.md").exists():
        return DirClass.PLAN_CANDIDATE

    if dirpath == ROADMAP_DIR:
        return DirClass.PLAN_CANDIDATE

    has_md = any(dirpath.glob("*.md"))
    if has_md:
        return DirClass.UNKNOWN

    return DirClass.CONTAINER


# ---------------------------------------------------------------------------
# §01.2 — Schema Definitions (Seven File Classes)
# ---------------------------------------------------------------------------

# Status enums (corpus-derived)
PLAN_STATUSES = frozenset({"active", "queued", "resolved", "not-started", "research"})
SECTION_STATUSES = frozenset({"not-started", "in-progress", "complete"})
OVERVIEW_STATUSES = frozenset({"not-started", "in-progress", "research", "complete"})
FIX_STATUSES = frozenset({"not-started", "in-progress", "complete"})
TPR_STATUSES = frozenset({"none", "findings", "resolved", "clean"})
SEVERITY_VALUES = frozenset({"critical", "high", "medium", "low"})
COMPLETED_STATUSES = frozenset({"resolved"})


@dataclass(frozen=True)
class SubsectionEntry:
    id: str
    title: str
    status: str


@dataclass(frozen=True)
class TprInfo:
    status: str
    updated: str | None


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


# --- Per-schema validators ---

_PLAN_INDEX_REQUIRED = ["name", "full_name", "status"]
_PLAN_INDEX_ALLOWED = frozenset({
    "name", "full_name", "status", "reviewed", "reroute", "parallel",
    "order", "supersedes", "references", "inspired_by",
})


def _validate_plan_index(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _PLAN_INDEX_REQUIRED, path))
    findings.extend(_check_unknown_fields(data, _PLAN_INDEX_ALLOWED, path))
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


_PLAN_SECTION_REQUIRED = [
    "section", "title", "status", "reviewed", "goal",
    "success_criteria", "sections", "third_party_review",
]
_PLAN_SECTION_ALLOWED = frozenset({
    "section", "title", "status", "reviewed", "goal",
    "success_criteria", "sections", "third_party_review",
    "depends_on", "inspired_by",
})


def _validate_plan_section(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _PLAN_SECTION_REQUIRED, path))
    findings.extend(_check_unknown_fields(data, _PLAN_SECTION_ALLOWED, path))
    findings.extend(_check_enum(data, "status", SECTION_STATUSES, path))
    findings.extend(_validate_depends_on(data, path))
    _, tpr_findings = _validate_tpr_info(data.get("third_party_review"), path)
    findings.extend(tpr_findings)
    return findings


_ROADMAP_SECTION_REQUIRED = [
    "section", "title", "status", "reviewed", "goal", "sections",
]
_ROADMAP_SECTION_ALLOWED = frozenset({
    "section", "title", "status", "reviewed", "goal", "sections",
    "tier", "last_verified", "spec", "depends_on", "third_party_review",
})


def _validate_roadmap_section(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _ROADMAP_SECTION_REQUIRED, path))
    findings.extend(_check_unknown_fields(data, _ROADMAP_SECTION_ALLOWED, path))
    findings.extend(_check_enum(data, "status", SECTION_STATUSES, path))
    findings.extend(_validate_depends_on(data, path))
    if "third_party_review" in data:
        _, tpr_findings = _validate_tpr_info(data.get("third_party_review"), path)
        findings.extend(tpr_findings)
    return findings


_OVERVIEW_REQUIRED = ["plan", "title", "status"]
_OVERVIEW_ALLOWED = frozenset({
    "plan", "title", "status", "supersedes", "references",
})


def _validate_overview(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _OVERVIEW_REQUIRED, path))
    findings.extend(_check_unknown_fields(data, _OVERVIEW_ALLOWED, path))
    findings.extend(_check_enum(data, "status", OVERVIEW_STATUSES, path))
    return findings


_BUG_SECTION_REQUIRED = ["section", "title", "status", "goal"]
_BUG_SECTION_ALLOWED = frozenset({
    "section", "title", "status", "goal", "sections",
})


def _validate_bug_section(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _BUG_SECTION_REQUIRED, path))
    findings.extend(_check_unknown_fields(data, _BUG_SECTION_ALLOWED, path))
    findings.extend(_check_enum(data, "status", SECTION_STATUSES, path))
    return findings


_FIX_BUG_REQUIRED = [
    "bug", "title", "severity", "status", "goal",
    "success_criteria", "subsystem", "found", "source",
    "third_party_review",
]
_FIX_BUG_ALLOWED = frozenset({
    "bug", "title", "severity", "status", "goal",
    "success_criteria", "subsystem", "found", "source",
    "third_party_review", "sections", "depends_on",
})


def _validate_fix_bug(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _FIX_BUG_REQUIRED, path))
    findings.extend(_check_unknown_fields(data, _FIX_BUG_ALLOWED, path))
    findings.extend(_check_enum(data, "status", FIX_STATUSES, path))
    findings.extend(_check_enum(data, "severity", SEVERITY_VALUES, path))
    _, tpr_findings = _validate_tpr_info(data.get("third_party_review"), path)
    findings.extend(tpr_findings)
    return findings


_COMPLETED_INDEX_REQUIRED = ["name", "full_name", "status"]
_COMPLETED_INDEX_ALLOWED = frozenset({
    "name", "full_name", "status", "reroute", "parallel", "order",
})


def _validate_completed_index(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _COMPLETED_INDEX_REQUIRED, path))
    findings.extend(_check_unknown_fields(data, _COMPLETED_INDEX_ALLOWED, path))
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


# ---------------------------------------------------------------------------
# §01.1 — Corpus Discovery
# ---------------------------------------------------------------------------


@dataclass
class Corpus:
    """The full discovered corpus with classified files and findings."""

    indexes: dict[Path, dict] = field(default_factory=dict)
    plan_sections: dict[Path, dict] = field(default_factory=dict)
    roadmap_sections: dict[Path, dict] = field(default_factory=dict)
    overviews: dict[Path, dict] = field(default_factory=dict)
    bug_sections: dict[Path, dict] = field(default_factory=dict)
    fix_bug_files: dict[Path, dict] = field(default_factory=dict)
    completed_indexes: dict[Path, dict] = field(default_factory=dict)
    name_index: dict[str, Path] = field(default_factory=dict)
    gaps: list[Finding] = field(default_factory=list)


def discover_corpus(
    root: Path | None = None,
    include: list[Path] | None = None,
) -> Corpus:
    """Walk plan directories and classify every .md file.

    Args:
        root: Plans root directory. Defaults to PLANS_DIR.
        include: If provided, only process these paths (for fast pilot runs).
    """
    if root is None:
        root = PLANS_DIR

    corpus = Corpus()

    if include is not None:
        for p in include:
            if p.is_file() and p.suffix == ".md":
                _classify_and_store(p, corpus)
        return corpus

    # Stage 1 + 2: walk directories
    for dirpath_str, dirnames, filenames in os.walk(root):
        dirpath = Path(dirpath_str)
        dir_class = _classify_directory(dirpath)

        if dir_class == DirClass.UNKNOWN:
            has_md = any(f.endswith(".md") for f in filenames)
            if has_md:
                corpus.gaps.append(Finding(
                    category=FindingCategory.GAP,
                    subtype=FindingSubtype.UNCLASSIFIED_DIRECTORY,
                    severity=Severity.LOW,
                    source=dirpath,
                    description=f"directory has .md files but no index.md: {dirpath}",
                    recommended_fix="Add index.md or move files to a plan directory",
                ))

        if dir_class == DirClass.PLAN_CANDIDATE:
            if not (dirpath / "index.md").exists() and dirpath != ROADMAP_DIR:
                corpus.gaps.append(Finding(
                    category=FindingCategory.GAP,
                    subtype=FindingSubtype.MISSING_INDEX_MD,
                    severity=Severity.HIGH,
                    source=dirpath,
                    description=f"plan candidate directory without index.md: {dirpath}",
                    recommended_fix="Add index.md to this plan directory",
                ))

        # Classify individual files
        for fname in filenames:
            if not fname.endswith(".md"):
                continue
            fpath = dirpath / fname
            _classify_and_store(fpath, corpus)

    # Build name_index from plan indexes
    for path, data in corpus.indexes.items():
        name = data.get("name")
        if name:
            if name in corpus.name_index:
                corpus.gaps.append(Finding(
                    category=FindingCategory.SCHEMA_VIOLATION,
                    subtype=FindingSubtype.DUPLICATE_PLAN_NAME,
                    severity=Severity.HIGH,
                    source=path,
                    target=corpus.name_index[name],
                    description=f"duplicate plan name: {name!r}",
                    recommended_fix="Each plan's name: field must be unique",
                ))
            else:
                corpus.name_index[name] = path.parent

    for path, data in corpus.completed_indexes.items():
        name = data.get("name")
        if name:
            if name in corpus.name_index:
                corpus.gaps.append(Finding(
                    category=FindingCategory.SCHEMA_VIOLATION,
                    subtype=FindingSubtype.DUPLICATE_PLAN_NAME,
                    severity=Severity.HIGH,
                    source=path,
                    target=corpus.name_index[name],
                    description=f"duplicate plan name: {name!r}",
                    recommended_fix="Each plan's name: field must be unique",
                ))
            else:
                corpus.name_index[name] = path.parent

    return corpus


def _classify_and_store(path: Path, corpus: Corpus) -> None:
    """Classify a file and store its raw frontmatter in the corpus."""
    fc = classify_file(path)
    if fc is None:
        return

    try:
        text = read_text_strict(path)
        data, _ = split_frontmatter_strict(text, path)
    except CorpusParseError:
        data = {}

    bucket = {
        FileClass.PLAN_INDEX: corpus.indexes,
        FileClass.PLAN_SECTION: corpus.plan_sections,
        FileClass.ROADMAP_SECTION: corpus.roadmap_sections,
        FileClass.OVERVIEW: corpus.overviews,
        FileClass.BUG_TRACKER_SECTION: corpus.bug_sections,
        FileClass.FIX_BUG: corpus.fix_bug_files,
        FileClass.COMPLETED_INDEX: corpus.completed_indexes,
    }.get(fc)

    if bucket is not None:
        bucket[path] = data


# ---------------------------------------------------------------------------
# §01.1 — load_and_validate boundary
# ---------------------------------------------------------------------------


@dataclass
class ValidatedFile:
    """A successfully parsed and classified file with optional schema violations."""
    path: Path
    file_class: FileClass
    data: dict[str, Any]
    body_offset: int
    body: str
    violations: list[Finding] = field(default_factory=list)


@dataclass
class LoadResult:
    """Tagged union: Ok(ValidatedFile) or Err(Finding)."""
    ok: ValidatedFile | None = None
    err: Finding | None = None

    @property
    def is_ok(self) -> bool:
        return self.ok is not None


def load_and_validate(path: Path) -> LoadResult:
    """The single CorpusParseError -> Finding boundary.

    Callers MUST check .is_ok — no try/except needed at call sites.
    """
    try:
        text = read_text_strict(path)
        data, body_offset = split_frontmatter_strict(text, path)
    except CorpusParseError as e:
        subtype = _parse_error_to_subtype(e)
        return LoadResult(err=Finding(
            category=FindingCategory.PARSE_ERROR,
            subtype=subtype,
            severity=Severity.HIGH,
            source=path,
            source_line=e.line,
            description=str(e),
            recommended_fix="Fix the parse error and retry",
        ))

    fc = classify_file(path)
    if fc is None:
        return LoadResult(ok=ValidatedFile(
            path=path,
            file_class=FileClass.PLAN_INDEX,  # fallback
            data=data,
            body_offset=body_offset,
            body="\n".join(text.split("\n")[body_offset:]),
        ))

    violations = validate(fc, data, path)
    body_text = "\n".join(text.split("\n")[body_offset:])

    return LoadResult(ok=ValidatedFile(
        path=path,
        file_class=fc,
        data=data,
        body_offset=body_offset,
        body=body_text,
        violations=violations,
    ))


def _parse_error_to_subtype(e: CorpusParseError) -> FindingSubtype:
    """Map a CorpusParseError to the most specific FindingSubtype."""
    msg = str(e).lower()
    if "bom" in msg:
        return FindingSubtype.UTF8_BOM
    if "zero-width" in msg:
        return FindingSubtype.ZERO_WIDTH_BEFORE_FM
    if "invalid utf-8" in msg or "invalid utf8" in msg:
        return FindingSubtype.INVALID_UTF8_BYTES
    if "missing opening" in msg:
        return FindingSubtype.MISSING_OPENING_DASHES
    if "unclosed" in msg:
        return FindingSubtype.UNCLOSED_FRONTMATTER
    if "anchor" in msg or "alias" in msg:
        return FindingSubtype.YAML_ANCHOR
    if "merge key" in msg:
        return FindingSubtype.YAML_MERGE_KEY
    if "multi-document" in msg:
        return FindingSubtype.MULTI_DOCUMENT
    if "non_mapping" in msg or "must be a mapping" in msg:
        return FindingSubtype.NON_MAPPING_ROOT
    if "duplicate key" in msg:
        return FindingSubtype.DUPLICATE_KEY
    return FindingSubtype.YAML_SYNTAX_ERROR


# ---------------------------------------------------------------------------
# §01.4 — Canonical Status Normalizer
# ---------------------------------------------------------------------------


@dataclass
class BodySignals:
    """Signals extracted from the body text for status derivation."""
    checked: int = 0
    unchecked: int = 0
    has_complete_marker: bool = False
    has_done_marker: bool = False
    has_todo_marker: bool = False


def scan_body_signals(body: str) -> BodySignals:
    """Scan body text for status-relevant markers and checkbox counts."""
    signals = BodySignals()
    in_fence = False

    for line in body.split("\n"):
        stripped = line.strip()

        # Skip fenced code blocks
        if stripped.startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue

        if re.search(r"\bCOMPLETE\b", stripped, re.IGNORECASE):
            signals.has_complete_marker = True
        if "[done]" in stripped.lower():
            signals.has_done_marker = True
        if "[todo]" in stripped.lower():
            signals.has_todo_marker = True

        if re.match(r"\s*- \[x\]", line):
            signals.checked += 1
        elif re.match(r"\s*- \[ \]", line):
            signals.unchecked += 1

    return signals


@dataclass
class NormalizedStatus:
    """Result of status normalization: declared vs derived, plus contradictions."""
    declared: str
    derived: str
    contradictions: list[Finding] = field(default_factory=list)


def normalize_status(
    data: dict[str, Any],
    body: str,
    path: Path,
    child_statuses: list[str] | None = None,
) -> NormalizedStatus:
    """Compute derived status from body signals and child statuses.

    Pure function: no git queries, no side effects.
    Emits STATUS_CONTRADICTION findings when declared != derived.
    """
    declared = str(data.get("status", "")).strip()
    if "#" in declared:
        declared = declared.split("#")[0].strip()

    signals = scan_body_signals(body)

    # Derive status from evidence
    if child_statuses is not None:
        derived = _derive_from_children(child_statuses)
    else:
        derived = _derive_from_body(signals)

    contradictions: list[Finding] = []

    if declared and derived and declared != derived:
        contradictions.append(Finding(
            category=FindingCategory.STATUS_CONTRADICTION,
            subtype=FindingSubtype.FM_DECLARED_VS_BODY_DERIVED,
            severity=Severity.MEDIUM,
            source=path,
            description=f"declared status={declared!r} but derived={derived!r}",
            recommended_fix=f"Update status to {derived!r} or fix body content",
            evidence=(
                f"checked={signals.checked}, unchecked={signals.unchecked}",
            ),
        ))

    # Plan-level: active but all sections not-started
    if child_statuses is not None:
        if declared == "active" and all(s == "not-started" for s in child_statuses) and child_statuses:
            contradictions.append(Finding(
                category=FindingCategory.STATUS_CONTRADICTION,
                subtype=FindingSubtype.PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED,
                severity=Severity.MEDIUM,
                source=path,
                description="plan is active but all sections are not-started",
                recommended_fix="Change plan status to queued, or start a section",
            ))
        if declared in ("complete", "resolved") and any(s != "complete" for s in child_statuses):
            contradictions.append(Finding(
                category=FindingCategory.STATUS_CONTRADICTION,
                subtype=FindingSubtype.PLAN_COMPLETE_WITH_OPEN_SECTIONS,
                severity=Severity.HIGH,
                source=path,
                description="plan marked complete but has non-complete sections",
                recommended_fix="Complete all sections first, or change plan status",
            ))

    return NormalizedStatus(
        declared=declared,
        derived=derived,
        contradictions=contradictions,
    )


def _derive_from_children(child_statuses: list[str]) -> str:
    if not child_statuses:
        return "not-started"
    if all(s == "complete" for s in child_statuses):
        return "complete"
    if all(s == "not-started" for s in child_statuses):
        return "not-started"
    return "in-progress"


def _derive_from_body(signals: BodySignals) -> str:
    total = signals.checked + signals.unchecked
    if total == 0:
        return ""
    if signals.unchecked == 0:
        return "complete"
    if signals.checked == 0:
        return "not-started"
    return "in-progress"


# ---------------------------------------------------------------------------
# §01.2 — DepId resolution
# ---------------------------------------------------------------------------


def resolve_dep(
    dep_id: str, current_plan_dir: Path, corpus: Corpus
) -> Path | Finding:
    """Resolve a depends_on ID to a concrete section file path."""
    m = CROSS_PLAN_DEP_RE.match(dep_id)
    if m:
        plan_name, section_id = m.group(1), m.group(2)
        target_dir = corpus.name_index.get(plan_name)
        if target_dir is None:
            return Finding(
                category=FindingCategory.DEAD_REFERENCE,
                subtype=FindingSubtype.CROSS_PLAN_NAME_NOT_FOUND,
                severity=Severity.HIGH,
                source=current_plan_dir / "index.md",
                description=f"cross-plan dep references unknown plan name: {plan_name!r}",
                recommended_fix=f"Known plan names: {sorted(corpus.name_index.keys())}",
            )
        return _find_section_file(target_dir, section_id)

    if INTRA_PLAN_DEP_RE.match(dep_id):
        return _find_section_file(current_plan_dir, dep_id)

    return Finding(
        category=FindingCategory.SCHEMA_VIOLATION,
        subtype=FindingSubtype.DEP_ID_MALFORMED,
        severity=Severity.HIGH,
        source=current_plan_dir / "index.md",
        description=f"malformed dep ID: {dep_id!r}",
        recommended_fix="Use \"NN\" or \"plan-name#NN\"",
    )


def _find_section_file(plan_dir: Path, section_id: str) -> Path | Finding:
    """Find a section file by ID within a plan directory."""
    candidates = list(plan_dir.glob(f"section-{section_id}*.md"))
    if not candidates:
        padded = section_id.zfill(2)
        candidates = list(plan_dir.glob(f"section-{padded}*.md"))
    if candidates:
        return candidates[0]
    return Finding(
        category=FindingCategory.DEAD_REFERENCE,
        subtype=FindingSubtype.SECTION_FILE_NOT_FOUND,
        severity=Severity.HIGH,
        source=plan_dir,
        description=f"no section file matching section-{section_id}*.md in {plan_dir}",
        recommended_fix="Check the section ID or create the missing file",
    )


# ---------------------------------------------------------------------------
# §01.2 — Schema Reference Generation
# ---------------------------------------------------------------------------


def generate_schema_reference() -> str:
    """Generate markdown documentation from the schema definitions."""
    lines = [
        "<!-- GENERATED from scripts/plan_corpus.py — do not edit -->",
        "",
        "# Plan Schema Reference",
        "",
        "Auto-generated from Python dataclass definitions in `scripts/plan_corpus.py`.",
        "",
        "## Status Enums",
        "",
        f"- **Plan statuses**: {', '.join(sorted(PLAN_STATUSES))}",
        f"- **Section statuses**: {', '.join(sorted(SECTION_STATUSES))}",
        f"- **Overview statuses**: {', '.join(sorted(OVERVIEW_STATUSES))}",
        f"- **Fix statuses**: {', '.join(sorted(FIX_STATUSES))}",
        f"- **TPR statuses**: {', '.join(sorted(TPR_STATUSES))}",
        f"- **Completed plan statuses**: {', '.join(sorted(COMPLETED_STATUSES))}",
        "",
        "## File Classes",
        "",
    ]

    schemas = [
        ("Plan Index", "plans/*/index.md", _PLAN_INDEX_REQUIRED, _PLAN_INDEX_ALLOWED),
        ("Plan Section", "plans/*/section-*.md", _PLAN_SECTION_REQUIRED, _PLAN_SECTION_ALLOWED),
        ("Roadmap Section", "plans/roadmap/section-*.md", _ROADMAP_SECTION_REQUIRED, _ROADMAP_SECTION_ALLOWED),
        ("Overview", "plans/*/00-overview.md", _OVERVIEW_REQUIRED, _OVERVIEW_ALLOWED),
        ("Bug Tracker Section", "plans/bug-tracker/section-*.md", _BUG_SECTION_REQUIRED, _BUG_SECTION_ALLOWED),
        ("Fix Bug", "plans/bug-tracker/fix-BUG-*.md", _FIX_BUG_REQUIRED, _FIX_BUG_ALLOWED),
        ("Completed Index", "plans/completed/*/index.md", _COMPLETED_INDEX_REQUIRED, _COMPLETED_INDEX_ALLOWED),
    ]

    for name, pattern, required, allowed in schemas:
        optional = sorted(allowed - frozenset(required))
        lines.append(f"### {name}")
        lines.append(f"**Pattern**: `{pattern}`")
        lines.append(f"**Required**: {', '.join(f'`{r}`' for r in required)}")
        if optional:
            lines.append(f"**Optional**: {', '.join(f'`{o}`' for o in optional)}")
        lines.append("")

    lines.append("## Finding Categories")
    lines.append("")
    for cat in FindingCategory:
        subtypes = sorted(s.value for s in _CATEGORY_SUBTYPES.get(cat, []))
        lines.append(f"### {cat.value}")
        for st in subtypes:
            lines.append(f"- `{st}`")
        lines.append("")

    return "\n".join(lines)


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def main() -> int:
    import argparse

    parser = argparse.ArgumentParser(
        description="Ori plan corpus parser and validator"
    )
    sub = parser.add_subparsers(dest="command")

    check_p = sub.add_parser("check", help="Validate a file or directory")
    check_p.add_argument("paths", nargs="+", type=Path)
    check_p.add_argument("--json", action="store_true")

    sub.add_parser("discover", help="Discover and report corpus")

    docgen_p = sub.add_parser("docgen", help="Generate schema reference")
    docgen_p.add_argument("--check", action="store_true",
                          help="Compare against committed file, exit non-zero on diff")

    args = parser.parse_args()

    if args.command == "check":
        all_findings: list[Finding] = []
        for p in args.paths:
            if p.is_dir():
                for md in sorted(p.rglob("*.md")):
                    result = load_and_validate(md)
                    if result.err:
                        all_findings.append(result.err)
                    elif result.ok:
                        all_findings.extend(result.ok.violations)
            else:
                result = load_and_validate(p)
                if result.err:
                    all_findings.append(result.err)
                elif result.ok:
                    all_findings.extend(result.ok.violations)

        if args.json:
            print(json.dumps([f.to_json() for f in all_findings], indent=2))
        else:
            for f in sorted(all_findings, key=lambda f: (-f.severity.value, str(f.source))):
                print(f.to_markdown())

        return 1 if all_findings else 0

    elif args.command == "discover":
        corpus = discover_corpus()
        print(f"Plan indexes: {len(corpus.indexes)}")
        print(f"Completed indexes: {len(corpus.completed_indexes)}")
        print(f"Plan sections: {len(corpus.plan_sections)}")
        print(f"Roadmap sections: {len(corpus.roadmap_sections)}")
        print(f"Overviews: {len(corpus.overviews)}")
        print(f"Bug sections: {len(corpus.bug_sections)}")
        print(f"Fix-BUG files: {len(corpus.fix_bug_files)}")
        print(f"Name index: {len(corpus.name_index)} plans")
        print(f"Gaps: {len(corpus.gaps)}")
        for gap in corpus.gaps:
            print(f"  {gap.to_markdown()}")
        return 0

    elif args.command == "docgen":
        ref = generate_schema_reference()
        target = REPO_ROOT / "docs" / "internal" / "plan-schema-reference.md"
        if args.check:
            if target.exists():
                committed = target.read_text().replace("\r\n", "\n")
                if committed == ref:
                    print("Schema reference is up to date.")
                    return 0
                else:
                    print("Schema reference is OUT OF DATE. Regenerate with:")
                    print(f"  python {__file__} docgen > {target}")
                    return 1
            else:
                print(f"Schema reference not found at {target}. Generate with:")
                print(f"  python {__file__} docgen > {target}")
                return 1
        else:
            print(ref)
            return 0

    else:
        parser.print_help()
        return 0


if __name__ == "__main__":
    sys.exit(main())
