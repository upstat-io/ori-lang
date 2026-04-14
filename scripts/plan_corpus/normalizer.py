"""Canonical status normalizer — pure, no git queries, no side effects.

Computes derived status from body signals and child statuses, and emits
STATUS_CONTRADICTION findings when declared != derived.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from .types import (
    Finding,
    FindingCategory,
    FindingSubtype,
    Severity,
)


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

    # Derive status from evidence.
    # Plan-level: if declared is active and all children not-started, derive "queued"
    # (per section-01.4: "PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED — derived should be queued").
    if child_statuses is not None:
        if declared == "active" and child_statuses and all(s == "not-started" for s in child_statuses):
            derived = "queued"
        else:
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
    # Explicit body markers override checkbox counts (per section-01.4:
    # "derived is computed from evidence: body COMPLETE / [done] markers,
    # checkbox density, sections[].status aggregation for plan-level").
    if signals.has_complete_marker or signals.has_done_marker:
        # If there are still unchecked boxes, the marker is aspirational —
        # caller should flag FM_DECLARED_VS_BODY_DERIVED. Return "complete"
        # to trigger that check when declared != complete.
        return "complete"
    if signals.has_todo_marker and signals.checked == 0:
        return "not-started"
    total = signals.checked + signals.unchecked
    if total == 0:
        return ""
    if signals.unchecked == 0:
        return "complete"
    if signals.checked == 0:
        return "not-started"
    return "in-progress"
