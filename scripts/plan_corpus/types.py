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


class Outcome(enum.Enum):
    """Gate outcome — distinct from Severity.

    Severity answers "how bad is this?" (impact classification), set by the
    emitter per the finding's real-world significance. Outcome answers
    "does this gate the check?" (enforcement mode), also set by the emitter.
    The two axes are INDEPENDENT: a `Severity.LOW` can be `Outcome.ERROR`
    and a `Severity.HIGH` can be `Outcome.WARNING` — the combination depends
    on the finding's kind and the invocation mode (default vs
    `--strict-recon`).

    Exit-code policy (§06.2 Design Decision 4): `check` returns 1 iff any
    finding has `outcome == ERROR`; WARNING findings are printed but
    non-gating. `Finding.outcome` defaults to ERROR so pre-existing
    callsites (schema violations, parse errors, etc.) gate CI unchanged;
    only the new recon-block emitters explicitly opt into `Outcome.WARNING`.
    """
    WARNING = "warning"   # printed, does NOT affect exit code
    ERROR = "error"       # printed AND forces exit 1


class FindingCategory(enum.Enum):
    """Top-level finding category (two-level hierarchy with FindingSubtype)."""
    PARSE_ERROR = "parse_error"
    SCHEMA_VIOLATION = "schema_violation"
    STATUS_CONTRADICTION = "status_contradiction"
    DAG_CONFLICT = "dag_conflict"
    DEAD_REFERENCE = "dead_reference"
    ITEM_VERIFICATION = "item_verification"
    GAP = "gap"


class SourceKind(enum.Enum):
    """Reference source taxonomy used by the §02 DAG builder.

    Lives in types.py (NOT dag.py) because Finding.source_kind is a typed field
    on the canonical Finding dataclass — homing SourceKind in dag.py would
    create a circular import (types.py -> dag.py -> types.py). See §02.0
    File(s) note and TPR-02-001-gemini round 2.

    Only EXPLICIT_DEPENDS_ON references become DAG edges; the other kinds feed
    MISSING_DEPENDENCY / DEAD_REFERENCE classifiers but never add shadow edges.
    """
    EXPLICIT_DEPENDS_ON = "explicit_depends_on"
    HTML_COMMENT_CONVENTION = "html_comment_convention"
    YAML_COMMENT = "yaml_comment"
    PROSE_VERB = "prose_verb"
    CODE_FENCE_EXAMPLE = "code_fence_example"


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
    REDUNDANT_DEPENDENCY = "redundant_dependency"
    ORPHANED_PLAN = "orphaned_plan"

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
    # §06.2 — `## Intelligence Reconnaissance` block detection
    MISSING_RECON_BLOCK = "missing_recon_block"
    VALIDATION_BYPASS = "validation_bypass"
    RECON_GRAPH_UNAVAILABLE = "recon_graph_unavailable"


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
        FindingSubtype.REDUNDANT_DEPENDENCY,
        FindingSubtype.ORPHANED_PLAN,
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
        FindingSubtype.MISSING_RECON_BLOCK,
        FindingSubtype.VALIDATION_BYPASS,
        FindingSubtype.RECON_GRAPH_UNAVAILABLE,
    }),
}


@dataclass(frozen=True)
class Finding:
    """A single diagnostic finding. Frozen and hashable for deduplication.

    §02 extensions (Option A, typed phase-boundary per TPR-02-006-codex):
      - `dependency_chain`: ordered path sequence for multi-hop findings
        (BLOCKED chains, CYCLE paths, REDUNDANT_DEPENDENCY triples).
      - `source_kind`: reference classification for §03 SafeFix/ExposureReview
        routing without string-parsing `evidence`.
      - `source_column`: disambiguates multiple findings on the same line
        (Concern J — Finding.id collision guard).

    Finding.id backward-compat: source_column and target are only hashed when
    non-None, so legacy findings retain their pre-extension IDs.
    """

    category: FindingCategory
    subtype: FindingSubtype
    severity: Severity
    source: Path
    description: str
    recommended_fix: str
    source_line: int | None = None
    source_column: int | None = None
    target: Path | None = None
    target_line: int | None = None
    evidence: tuple[str, ...] = ()
    dependency_chain: tuple[Path, ...] = ()
    source_kind: "SourceKind | None" = None
    target_key: str | None = None
    # Structural target-value carrier for list-item findings (e.g. DEAD_REFERENCE
    # on a depends_on list item). Populated at finding-construction time so
    # downstream consumers (auto_fix SafeFix dispatch, report rendering) read
    # the value structurally instead of parsing `description`. Parallels
    # `target_key` — hashed into `id` when non-None for backward-compat.
    target_value: str | None = None
    # §06.2 — enforcement channel independent of Severity. Defaults to ERROR
    # so pre-existing call sites gate CI unchanged; recon-block emitters opt
    # into WARNING explicitly. NOT included in `id` hash — backward-compat
    # with saved reports (id discriminates logical identity, not gate mode).
    outcome: Outcome = Outcome.ERROR

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
        # Conditional append preserves backward-compat: legacy findings that
        # never populate source_column, target, or target_key produce the
        # pre-extension hash.
        if self.source_column is not None:
            content += f":{self.source_column}"
        if self.target is not None:
            content += f":{self.target}"
        if self.target_key is not None:
            # target_key discriminates same-file same-subtype findings that
            # differ only by which schema field they target (e.g. two
            # MISSING_REQUIRED_FIELD findings on the same file with
            # target_key='name' vs target_key='full_name'). Without this
            # discriminator, distinct findings collide on the same id.
            content += f":{self.target_key}"
        if self.target_value is not None:
            # target_value discriminates same-file same-subtype findings on
            # the same line that differ only by which list item they target
            # (e.g. two DEAD_REFERENCE findings on the same depends_on list
            # line where one slug is dead and another is stale). Mirrors
            # target_key's backward-compat conditional-hash pattern.
            content += f":{self.target_value}"
        return "VR-" + hashlib.sha256(content.encode()).hexdigest()[:6]

    def to_json(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "category": self.category.value,
            "subtype": self.subtype.value,
            "severity": self.severity.name.lower(),
            "outcome": self.outcome.value,
            "source": str(self.source),
            "source_line": self.source_line,
            "source_column": self.source_column,
            "target": str(self.target) if self.target else None,
            "target_line": self.target_line,
            "target_key": self.target_key,
            "target_value": self.target_value,
            "description": self.description,
            "recommended_fix": self.recommended_fix,
            "evidence": list(self.evidence),
            "dependency_chain": [str(p) for p in self.dependency_chain],
            "source_kind": self.source_kind.value if self.source_kind else None,
        }

    def to_markdown(self) -> str:
        parts = [
            f"- **[{self.id}][{self.severity.name.lower()}]"
            f"[{self.outcome.value}]** `{self.source}`"
        ]
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
