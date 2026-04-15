"""Safety taxonomy and classification for verify-roadmap write-back.

Section 03.1 of verify-roadmap-redesign plan.

This module defines the safety taxonomy that gates auto-fix behavior:
  - SafetyClass: SafeFix (auto-applied) vs ExposureReview (human review)
  - ClassifiedFinding: wraps a Finding with its safety class + rationale
  - WriteBackContext: git activity signals for policy decisions
  - PreimageRecord: concurrent-session guard for file writes
  - classify_safety: pure classifier (Finding, context) -> ClassifiedFinding

Design invariants:
  1. classify_safety is PURE — no I/O, no subprocess calls
  2. plan_corpus stays pure — this module never modifies plan_corpus types
  3. classify_safety NEVER imports from plan_corpus.schema (FileClass) —
     it infers schema class from the file path to avoid a circular concern
  4. FM_DECLARED_VS_BODY_DERIVED is ALWAYS ExposureReview — defense-in-depth
  5. SPEC_FILE_NOT_FOUND is ALWAYS ExposureReview — needs human to find path
"""

from __future__ import annotations

import enum
from collections import defaultdict
from dataclasses import dataclass, field, replace
from pathlib import Path

from scripts.plan_corpus import (
    Finding,
    FindingCategory,
    FindingSubtype,
    SourceKind,
)


# ---------------------------------------------------------------------------
# Safety taxonomy types
# ---------------------------------------------------------------------------

class SafetyClass(enum.Enum):
    """Auto-fix gating tag.

    SAFE_FIX: applied automatically (with backup + log).
    EXPOSURE_REVIEW: surfaced for human review (never auto-applied).
    """
    SAFE_FIX = "safe_fix"
    EXPOSURE_REVIEW = "exposure_review"


PAIRING_TAG_PLAN_TO_NAME_RENAME = "plan_to_name_rename"


@dataclass(frozen=True)
class ClassifiedFinding:
    """A Finding wrapped with its safety classification.

    Section 03 produces ClassifiedFinding records; Sections 01/02 never do.
    The finding field is imported from plan_corpus; safety_class is write-back
    policy owned by this module.
    """
    finding: Finding
    safety_class: SafetyClass
    rationale: str
    resolved_by_sibling: str | None = None
    pairing_tag: str | None = None


@dataclass(frozen=True)
class WriteBackContext:
    """Caller-supplied context for policy decisions.

    The CLI front-end populates this by running git commands at the edge.
    plan_corpus stays pure — no subprocess or git calls inside.
    --quick mode passes context=None to bypass construction entirely.
    """
    has_recent_commits: dict[Path, bool] = field(default_factory=dict)


@dataclass(frozen=True)
class PreimageRecord:
    """Concurrent-session guard for file writes.

    Captured at scan time. Used by the text patcher (03.4) to detect
    concurrent modifications before write.
    """
    path: Path
    content_hash: str
    scan_timestamp: float


@dataclass(frozen=True)
class PatchResult:
    """Result of an attempted frontmatter patch (produced by §03.4).

    Forward-declared here so §03.2 (report format) and §03.3 (auto-fix
    engine) can consume `PatchResult(applied=False)` without a circular
    dependency on §03.4's text-patcher module.

    When applied=False, the auto-fix dispatcher converts the original
    SafeFix finding into an ExposureReview entry surfaced as an
    "unapplied fix" group in the report (TPR-03-003-codex).
    """
    applied: bool
    reason: str
    finding_id: str
    path: Path
    before_hash: str | None = None
    after_hash: str | None = None


class FmOperationKind(enum.Enum):
    """Frontmatter text patch operations (§03.4 contract).

    Forward-declared here so §03.3 (auto-fix dispatcher) can produce
    operation lists without depending on §03.4's patcher module.

    Operations are applied to the raw frontmatter slice between the
    `---` fences — preserving comments, key order, and formatting.
    """
    RENAME_KEY = "rename_key"            # kwargs: old_key, new_key
    REMOVE_KEY = "remove_key"            # kwargs: key
    REPLACE_VALUE = "replace_value"      # kwargs: key, new_value
    INSERT_KEY = "insert_key"            # kwargs: key, value, after_key
    REMOVE_LIST_ITEM = "remove_list_item"  # kwargs: list_key, item_value


@dataclass(frozen=True)
class FmOperation:
    """A single frontmatter patch operation.

    The kwargs dict carries operation-specific arguments per FmOperationKind.
    Frozen + hashable so operation lists can be deduplicated and logged
    deterministically.
    """
    kind: FmOperationKind
    kwargs: tuple[tuple[str, str], ...]  # frozen as sorted tuple-of-pairs

    @classmethod
    def make(cls, kind: FmOperationKind, **kwargs: str) -> "FmOperation":
        """Construct from keyword arguments — sorts for determinism."""
        return cls(kind=kind, kwargs=tuple(sorted(kwargs.items())))

    def kwargs_dict(self) -> dict[str, str]:
        """Return kwargs as a dict (recovering the kwargs view)."""
        return dict(self.kwargs)


# ---------------------------------------------------------------------------
# Path-based schema class inference
# ---------------------------------------------------------------------------

def _is_plan_index(source: Path) -> bool:
    """Check if source is a PlanIndexSchema file (plans/*/index.md)."""
    return source.name == "index.md" and "completed" not in source.parts


def _is_overview(source: Path) -> bool:
    """Check if source is an OverviewSchema file (plans/*/00-overview.md)."""
    return source.name == "00-overview.md"


def _is_plan_section(source: Path) -> bool:
    """Check if source is a PlanSectionSchema file (plans/*/section-*.md, non-roadmap)."""
    return (
        source.name.startswith("section-")
        and "roadmap" not in source.parts
    )


def _is_roadmap_section(source: Path) -> bool:
    """Check if source is a RoadmapSectionSchema file (plans/roadmap/section-*.md)."""
    return (
        source.name.startswith("section-")
        and "roadmap" in source.parts
    )


def _is_fix_bug(source: Path) -> bool:
    """Check if source is a FixBugSchema file (plans/bug-tracker/fix-BUG-*.md)."""
    return source.name.startswith("fix-BUG-")


# ---------------------------------------------------------------------------
# classify_safety — the single canonical classifier
# ---------------------------------------------------------------------------

def classify_safety(
    finding: Finding,
    context: WriteBackContext | None,
    frontmatter_data: dict | None = None,
) -> ClassifiedFinding:
    """Classify a Finding as SafeFix or ExposureReview.

    Pure function of (finding, context, frontmatter_data) — no I/O.

    Args:
        finding: The plan_corpus Finding to classify.
        context: WriteBackContext with git signals, or None for --quick mode.
        frontmatter_data: Pre-parsed frontmatter dict for the finding's source
            file. Allows inspecting sibling fields without I/O.

    Returns:
        ClassifiedFinding wrapping the finding with its safety class.
    """
    # Quick mode: no classification — all findings are ExposureReview
    if context is None:
        return ClassifiedFinding(
            finding=finding,
            safety_class=SafetyClass.EXPOSURE_REVIEW,
            rationale="Quick mode — no write-back classification",
        )

    # Dispatch on category
    cat = finding.category
    sub = finding.subtype

    if cat == FindingCategory.SCHEMA_VIOLATION:
        return _classify_schema_violation(finding, context, frontmatter_data)
    elif cat == FindingCategory.STATUS_CONTRADICTION:
        return _classify_status_contradiction(finding, context)
    elif cat == FindingCategory.DEAD_REFERENCE:
        return _classify_dead_reference(finding, context)
    else:
        # PARSE_ERROR, DAG_CONFLICT, ITEM_VERIFICATION, GAP —
        # all ExposureReview by default (conservative; never auto-applied)
        return ClassifiedFinding(
            finding=finding,
            safety_class=SafetyClass.EXPOSURE_REVIEW,
            rationale=f"No SafeFix rule declared for {cat.value}/{sub.value}",
        )


# ---------------------------------------------------------------------------
# Per-category classifiers
# ---------------------------------------------------------------------------

def _classify_schema_violation(
    finding: Finding,
    context: WriteBackContext,
    frontmatter_data: dict | None,
) -> ClassifiedFinding:
    """Classify SCHEMA_VIOLATION findings."""
    sub = finding.subtype
    source = finding.source

    if sub == FindingSubtype.UNKNOWN_FIELD:
        return _classify_unknown_field(finding, context, frontmatter_data)

    if sub == FindingSubtype.MISSING_REQUIRED_FIELD:
        return _classify_missing_required_field(finding, context, frontmatter_data)

    # All other SCHEMA_VIOLATION subtypes: ExposureReview
    # WRONG_TYPE, ENUM_OUT_OF_RANGE, CROSS_FIELD_INVARIANT,
    # DUPLICATE_PLAN_NAME, DEP_ID_MALFORMED, DEP_ID_FULL_PATH, DEP_ID_UNKNOWN_NAME
    return ClassifiedFinding(
        finding=finding,
        safety_class=SafetyClass.EXPOSURE_REVIEW,
        rationale=f"No SafeFix rule for SCHEMA_VIOLATION/{sub.value}",
    )


def _classify_unknown_field(
    finding: Finding,
    context: WriteBackContext,
    frontmatter_data: dict | None,
) -> ClassifiedFinding:
    """Classify UNKNOWN_FIELD — handles plan:->name: rename."""
    desc = finding.description.lower()
    source = finding.source

    # plan: key rename is only valid on PlanIndexSchema files
    if "plan" in desc and _is_plan_index(source):
        fm = frontmatter_data or {}

        has_plan = "plan" in fm
        has_name = "name" in fm

        if has_plan and has_name:
            # Collision guard: both keys exist
            if fm["plan"] != fm["name"]:
                return ClassifiedFinding(
                    finding=finding,
                    safety_class=SafetyClass.EXPOSURE_REVIEW,
                    rationale=(
                        "Collision: both plan: and name: exist with different values "
                        f"({fm['plan']!r} vs {fm['name']!r}) — human must decide"
                    ),
                )
            else:
                # Same values: remove redundant plan: key
                return ClassifiedFinding(
                    finding=finding,
                    safety_class=SafetyClass.SAFE_FIX,
                    rationale=(
                        "plan: and name: have identical values — "
                        "remove redundant plan: key"
                    ),
                )
        elif has_plan and not has_name:
            # Standard rename: plan: -> name:
            return ClassifiedFinding(
                finding=finding,
                safety_class=SafetyClass.SAFE_FIX,
                rationale="PlanIndexSchema: rename plan: to name: (preserving value)",
                pairing_tag=PAIRING_TAG_PLAN_TO_NAME_RENAME,
            )

    # plan: on OverviewSchema is canonical — NOT a rename candidate
    if "plan" in desc and _is_overview(source):
        return ClassifiedFinding(
            finding=finding,
            safety_class=SafetyClass.EXPOSURE_REVIEW,
            rationale=(
                "OverviewSchema uses plan: as a required field — "
                "not a rename candidate"
            ),
        )

    # Generic UNKNOWN_FIELD: no specific SafeFix rule
    return ClassifiedFinding(
        finding=finding,
        safety_class=SafetyClass.EXPOSURE_REVIEW,
        rationale="No SafeFix rule for UNKNOWN_FIELD on this file type",
    )


def _classify_missing_required_field(
    finding: Finding,
    context: WriteBackContext,
    frontmatter_data: dict | None,
) -> ClassifiedFinding:
    """Classify MISSING_REQUIRED_FIELD — handles safe defaults."""
    desc = finding.description.lower()
    source = finding.source

    # reviewed: false insertion
    if "reviewed" in desc:
        # SafeFix only for PlanSectionSchema and RoadmapSectionSchema
        # where reviewed: is a REQUIRED field with no default
        if _is_plan_section(source) or _is_roadmap_section(source):
            return ClassifiedFinding(
                finding=finding,
                safety_class=SafetyClass.SAFE_FIX,
                rationale=(
                    "Insert reviewed: false — required field with safe default "
                    "on PlanSectionSchema/RoadmapSectionSchema"
                ),
            )
        # PlanIndexSchema: reviewed is OPTIONAL (None = no gate trigger).
        # Inserting false triggers /continue-roadmap Step 1.7 gate.
        # This is a semantic change, not normalization.
        if _is_plan_index(source):
            return ClassifiedFinding(
                finding=finding,
                safety_class=SafetyClass.EXPOSURE_REVIEW,
                rationale=(
                    "PlanIndexSchema: reviewed is optional (None = no gate trigger); "
                    "inserting false would trigger /continue-roadmap Step 1.7 gate — "
                    "semantic change, not normalization"
                ),
            )

    # third_party_review: default insertion
    if "third_party_review" in desc:
        # SafeFix where required by schema (PlanSectionSchema, FixBugSchema)
        if _is_plan_section(source) or _is_fix_bug(source):
            return ClassifiedFinding(
                finding=finding,
                safety_class=SafetyClass.SAFE_FIX,
                rationale=(
                    "Insert third_party_review: {status: none, updated: null} — "
                    "required field with safe default"
                ),
            )

    # All other MISSING_REQUIRED_FIELD cases: needs semantic inference
    return ClassifiedFinding(
        finding=finding,
        safety_class=SafetyClass.EXPOSURE_REVIEW,
        rationale=(
            "Missing required field needs semantic inference from body content"
        ),
    )


def _classify_status_contradiction(
    finding: Finding,
    context: WriteBackContext,
) -> ClassifiedFinding:
    """Classify STATUS_CONTRADICTION findings."""
    sub = finding.subtype

    # FM_DECLARED_VS_BODY_DERIVED: ALWAYS ExposureReview (blind spot #4)
    if sub == FindingSubtype.FM_DECLARED_VS_BODY_DERIVED:
        return ClassifiedFinding(
            finding=finding,
            safety_class=SafetyClass.EXPOSURE_REVIEW,
            rationale=(
                "FM_DECLARED_VS_BODY_DERIVED is ALWAYS ExposureReview — "
                "normalizer may return derived='complete' with unchecked items "
                "(aspirational marker); auto-fix would be wrong"
            ),
        )

    # PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED: depends on git activity
    if sub == FindingSubtype.PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED:
        plan_dir = finding.source.parent
        has_activity = context.has_recent_commits.get(plan_dir)
        if has_activity is False:
            return ClassifiedFinding(
                finding=finding,
                safety_class=SafetyClass.SAFE_FIX,
                rationale=(
                    "Plan status=active but all sections not-started and "
                    "no recent git commits — safe to downgrade to queued"
                ),
            )
        else:
            # True or missing (conservative)
            reason = (
                "recent git activity" if has_activity is True
                else "no git data available (conservative)"
            )
            return ClassifiedFinding(
                finding=finding,
                safety_class=SafetyClass.EXPOSURE_REVIEW,
                rationale=(
                    f"Plan status=active, all sections not-started, but {reason} — "
                    "sections may be stale rather than plan inactive"
                ),
            )

    # All other STATUS_CONTRADICTION subtypes: ExposureReview
    return ClassifiedFinding(
        finding=finding,
        safety_class=SafetyClass.EXPOSURE_REVIEW,
        rationale=f"No SafeFix rule for STATUS_CONTRADICTION/{sub.value}",
    )


def _classify_dead_reference(
    finding: Finding,
    context: WriteBackContext,
) -> ClassifiedFinding:
    """Classify DEAD_REFERENCE findings."""
    sub = finding.subtype
    source_kind = finding.source_kind

    # SPEC_FILE_NOT_FOUND: ALWAYS ExposureReview
    if sub == FindingSubtype.SPEC_FILE_NOT_FOUND:
        return ClassifiedFinding(
            finding=finding,
            safety_class=SafetyClass.EXPOSURE_REVIEW,
            rationale=(
                "Dead spec reference may need a replacement path — "
                "human determination required"
            ),
        )

    # For PLAN_DIRECTORY_NOT_FOUND, SECTION_FILE_NOT_FOUND,
    # CROSS_PLAN_NAME_NOT_FOUND: SafeFix only when the reference is in
    # a depends_on frontmatter list (mechanical removal)
    if source_kind == SourceKind.EXPLICIT_DEPENDS_ON:
        return ClassifiedFinding(
            finding=finding,
            safety_class=SafetyClass.SAFE_FIX,
            rationale=(
                f"Dead depends_on entry ({sub.value}) — "
                "mechanical removal from frontmatter list"
            ),
        )

    # Prose body references, HTML comments, etc.: ExposureReview
    kind_desc = source_kind.value if source_kind else "unknown source kind"
    return ClassifiedFinding(
        finding=finding,
        safety_class=SafetyClass.EXPOSURE_REVIEW,
        rationale=(
            f"Dead reference ({sub.value}) from {kind_desc} — "
            "may need human-authored replacement"
        ),
    )
