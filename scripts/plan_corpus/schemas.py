"""Dataclass SSOTs for the seven plan file schemas.

Each `@dataclass(frozen=True)` class is the sole source of truth for the
required / allowed field sets of one file class. Required fields have no
default; optional fields default to `None`.

Consumers (`schema.py`, `docgen.py`) derive `required` / `allowed` from
these dataclasses via `dataclasses.fields()` — do NOT maintain parallel
allowlist constants.

Status enum frozensets (`PLAN_STATUSES`, `SECTION_STATUSES`, …) are also
homed here: they are schema constraints — the allowed values for enum
fields — so they belong with the dataclass shapes they constrain.
"""

from __future__ import annotations

import dataclasses
from dataclasses import dataclass


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
# Top-level file-class schemas
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class PlanIndexSchema:
    """Schema for `plans/*/index.md`."""
    name: str
    full_name: str
    status: str
    reviewed: bool | None = None
    reroute: bool | None = None
    parallel: bool | None = None
    order: int | None = None
    supersedes: list[str] | None = None
    references: list[str] | None = None
    inspired_by: list[str] | None = None


@dataclass(frozen=True)
class PlanSectionSchema:
    """Schema for `plans/*/section-*.md` (non-roadmap plan sections)."""
    section: str
    title: str
    status: str
    reviewed: bool
    goal: str
    success_criteria: list[str]
    sections: list[dict]
    third_party_review: dict
    depends_on: list[str] | None = None
    inspired_by: list[str] | None = None


@dataclass(frozen=True)
class RoadmapSectionSchema:
    """Schema for `plans/roadmap/section-*.md`."""
    section: str
    title: str
    status: str
    reviewed: bool
    goal: str
    sections: list[dict]
    tier: str | None = None
    last_verified: str | None = None
    spec: str | None = None
    depends_on: list[str] | None = None
    third_party_review: dict | None = None
    tpr_findings: list[dict] | None = None
    verification_summary: str | None = None


@dataclass(frozen=True)
class OverviewSchema:
    """Schema for `plans/*/00-overview.md`."""
    plan: str
    title: str
    status: str
    reviewed: bool | None = None
    supersedes: list[str] | None = None
    references: list[str] | None = None


@dataclass(frozen=True)
class BugTrackerSectionSchema:
    """Schema for `plans/bug-tracker/section-*.md`."""
    section: str
    title: str
    status: str
    goal: str
    sections: list[dict] | None = None


@dataclass(frozen=True)
class FixBugSchema:
    """Schema for `plans/bug-tracker/fix-BUG-*.md`."""
    bug: str
    title: str
    severity: str
    status: str
    goal: str
    success_criteria: list[str]
    subsystem: str
    found: str
    source: str
    third_party_review: dict
    sections: list[dict] | None = None
    depends_on: list[str] | None = None


@dataclass(frozen=True)
class CompletedIndexSchema:
    """Schema for `plans/completed/*/index.md`."""
    name: str
    full_name: str
    status: str
    reroute: bool | None = None
    parallel: bool | None = None
    order: int | None = None


# ---------------------------------------------------------------------------
# Shape helpers
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class SubsectionEntry:
    """Schema for each entry in a section's `sections: [...]` list.

    Used by `_validate_sections` in schema.py — its required/allowed field
    sets are derived from this dataclass via the same introspection helpers
    as the top-level file schemas, so there is ONE SSOT for the shape.
    """
    id: str
    title: str
    status: str


@dataclass(frozen=True)
class TprInfo:
    """Parsed shape of a `third_party_review` block."""
    status: str
    updated: str | None


def _schema_required_fields(cls) -> list[str]:
    """Fields without defaults are required.

    Preserves declaration order, so the resulting list is stable and
    matches the order used in generated documentation.
    """
    return [
        f.name for f in dataclasses.fields(cls)
        if f.default is dataclasses.MISSING
        and f.default_factory is dataclasses.MISSING
    ]


def _schema_allowed_fields(cls) -> frozenset[str]:
    """All dataclass field names are allowed frontmatter keys."""
    return frozenset(f.name for f in dataclasses.fields(cls))
