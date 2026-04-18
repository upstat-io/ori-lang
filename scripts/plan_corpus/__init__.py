"""Corpus SSOT for Ori compiler plan tooling.

This package is the **sole** home for:
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

# Types, constants, taxonomy
from .types import (
    REPO_ROOT,
    PLANS_DIR,
    COMPLETED_DIR,
    BUG_TRACKER_DIR,
    ROADMAP_DIR,
    FRONTMATTER_FENCE,
    YAML_MERGE_KEY_RE,
    ZERO_WIDTH_CHARS,
    INTRA_PLAN_DEP_RE,
    CROSS_PLAN_DEP_RE,
    FULL_PATH_DEP_RE,
    FIX_BUG_RE,
    SECTION_FILE_RE,
    OVERVIEW_RE,
    Severity,
    Outcome,
    FindingCategory,
    FindingSubtype,
    SourceKind,
    _CATEGORY_SUBTYPES,
    Finding,
    CorpusParseError,
)

# Parser
from .parser import (
    read_text_strict,
    split_frontmatter_strict,
)

# Dataclass schema SSOTs (shape + status enum constants)
from .schemas import (
    PLAN_STATUSES,
    SECTION_STATUSES,
    OVERVIEW_STATUSES,
    FIX_STATUSES,
    TPR_STATUSES,
    SEVERITY_VALUES,
    COMPLETED_STATUSES,
    SubsectionEntry,
    TprInfo,
    PlanIndexSchema,
    PlanSectionSchema,
    RoadmapSectionSchema,
    OverviewSchema,
    BugTrackerSectionSchema,
    FixBugSchema,
    CompletedIndexSchema,
)

# FileClass + classification + validation dispatch
from .schema import (
    FileClass,
    classify_file,
    FileClassMeta,
    FILE_CLASS_META,
    validate,
)

# Discovery + load
from .discovery import (
    DirClass,
    Corpus,
    discover_corpus,
    ValidatedFile,
    LoadResult,
    load_and_validate,
)

# Normalizer
from .normalizer import (
    BodySignals,
    scan_body_signals,
    NormalizedStatus,
    normalize_status,
)

# Bug-entry markers (lifecycle marker SSOT for bug-tracker bullets)
from .bug_markers import (
    BUG_SUPERSEDED_RE,
    BUG_ESCALATED_RE,
    BUG_BLOCKED_RE,
    BUG_BLOCKED_BY_COMMENT_RE,
    BUG_SUPERSEDED_TARGET_RE,
    BUG_HEADER_RE,
    BugEntry,
    classify_bug_exclusion,
    extract_supersede_target,
    extract_repro,
    extract_subsystem,
    normalize_severity,
    parse_bug_entries,
    parse_bug_tracker_dir,
)

# Bug-entry validators (bidirectional supersede drift + auto-fix planner)
from .bug_validators import (
    MissingSupersedeMarker,
    OrphanSupersedeMarker,
    SupersedeDriftReport,
    PlannedEdit,
    parse_plan_supersedes,
    find_supersede_drift,
    plan_auto_fixes,
    apply_planned_edits,
)

# Docgen / DepId resolution
from .docgen import (
    resolve_dep,
    generate_schema_reference,
)

__all__ = [
    # Constants
    "REPO_ROOT",
    "PLANS_DIR",
    "COMPLETED_DIR",
    "BUG_TRACKER_DIR",
    "ROADMAP_DIR",
    "FRONTMATTER_FENCE",
    "YAML_MERGE_KEY_RE",
    "ZERO_WIDTH_CHARS",
    "INTRA_PLAN_DEP_RE",
    "CROSS_PLAN_DEP_RE",
    "FULL_PATH_DEP_RE",
    "FIX_BUG_RE",
    "SECTION_FILE_RE",
    "OVERVIEW_RE",
    # Taxonomy
    "Severity",
    "Outcome",
    "FindingCategory",
    "FindingSubtype",
    "SourceKind",
    "Finding",
    "CorpusParseError",
    # Parser
    "read_text_strict",
    "split_frontmatter_strict",
    # Schema
    "FileClass",
    "classify_file",
    "FileClassMeta",
    "FILE_CLASS_META",
    "PLAN_STATUSES",
    "SECTION_STATUSES",
    "OVERVIEW_STATUSES",
    "FIX_STATUSES",
    "TPR_STATUSES",
    "SEVERITY_VALUES",
    "COMPLETED_STATUSES",
    "SubsectionEntry",
    "TprInfo",
    "PlanIndexSchema",
    "PlanSectionSchema",
    "RoadmapSectionSchema",
    "OverviewSchema",
    "BugTrackerSectionSchema",
    "FixBugSchema",
    "CompletedIndexSchema",
    "validate",
    # Discovery
    "DirClass",
    "Corpus",
    "discover_corpus",
    "ValidatedFile",
    "LoadResult",
    "load_and_validate",
    # Normalizer
    "BodySignals",
    "scan_body_signals",
    "NormalizedStatus",
    "normalize_status",
    # Bug-entry markers
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
    # Bug-entry validators
    "MissingSupersedeMarker",
    "OrphanSupersedeMarker",
    "SupersedeDriftReport",
    "PlannedEdit",
    "parse_plan_supersedes",
    "find_supersede_drift",
    "plan_auto_fixes",
    "apply_planned_edits",
    # Docgen
    "resolve_dep",
    "generate_schema_reference",
]
