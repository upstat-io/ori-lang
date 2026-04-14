"""Shared types, constants, and regex patterns for the plan corpus library.

This module has NO imports from sibling modules — it is the leaf dependency
that all other submodules import from.
"""

from __future__ import annotations

import enum
import hashlib
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any


# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
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
# Severity & Finding Taxonomy
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


# Category -> allowed subtypes mapping (enforced at Finding construction)
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
