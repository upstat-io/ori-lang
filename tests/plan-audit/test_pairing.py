#!/usr/bin/env python3
"""Tests for scripts/verify_roadmap/pairing.py — resolved_by_sibling dedup.

Section 03.1 wiring. Regression for TPR-03-002-{codex,gemini}-r4 agreement
finding: the plan→name rename's paired MISSING_REQUIRED_FIELD(name) must be
linked to the UNKNOWN_FIELD(plan) rename so the auto-fix engine skips the
dependent half.
"""

import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT))

from scripts.plan_corpus import (
    Finding,
    FindingCategory,
    FindingSubtype,
    Severity,
)
from scripts.verify_roadmap.pairing import pair_resolved_by_sibling
from scripts.verify_roadmap.safety import (
    ClassifiedFinding,
    PAIRING_TAG_PLAN_TO_NAME_RENAME,
    SafetyClass,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _finding(
    category: FindingCategory,
    subtype: FindingSubtype,
    source: Path,
    description: str,
    target_key: str | None = None,
) -> Finding:
    return Finding(
        category=category,
        subtype=subtype,
        severity=Severity.MEDIUM,
        source=source,
        description=description,
        recommended_fix="fix it",
        target_key=target_key,
    )


def _classified(
    finding: Finding,
    safety: SafetyClass,
    rationale: str,
    pairing_tag: str | None = None,
) -> ClassifiedFinding:
    return ClassifiedFinding(
        finding=finding,
        safety_class=safety,
        rationale=rationale,
        pairing_tag=pairing_tag,
    )


INDEX = Path("plans/demo-plan/index.md")
RENAME_RATIONALE = "PlanIndexSchema: rename plan: to name: (preserving value)"


# ---------------------------------------------------------------------------
# Happy path: rename + missing-name pair on PlanIndex
# ---------------------------------------------------------------------------

class TestRenamePairResolved:

    def test_missing_name_gets_linked_to_rename(self):
        """SEMANTIC PIN: MISSING_REQUIRED_FIELD(name) on the same PlanIndex
        as a SafeFix UNKNOWN_FIELD(plan) rename is marked resolved_by_sibling."""
        rename = _classified(
            _finding(
                FindingCategory.SCHEMA_VIOLATION,
                FindingSubtype.UNKNOWN_FIELD,
                source=INDEX,
                description="Unknown field: plan",
                target_key="plan",
            ),
            SafetyClass.SAFE_FIX,
            RENAME_RATIONALE,
            pairing_tag=PAIRING_TAG_PLAN_TO_NAME_RENAME,
        )
        missing_name = _classified(
            _finding(
                FindingCategory.SCHEMA_VIOLATION,
                FindingSubtype.MISSING_REQUIRED_FIELD,
                source=INDEX,
                description="Missing required field: name",
                target_key="name",
            ),
            SafetyClass.EXPOSURE_REVIEW,
            "Missing required field needs semantic inference from body content",
        )

        out = pair_resolved_by_sibling([rename, missing_name])

        # rename half: unchanged
        [paired_rename] = [cf for cf in out if cf.finding.subtype == FindingSubtype.UNKNOWN_FIELD]
        assert paired_rename.resolved_by_sibling is None
        # missing_name half: resolved_by_sibling set to rename.finding.id
        [paired_missing] = [cf for cf in out if cf.finding.subtype == FindingSubtype.MISSING_REQUIRED_FIELD]
        assert paired_missing.resolved_by_sibling == rename.finding.id

    def test_out_is_new_list_not_mutation(self):
        """ClassifiedFinding is frozen; pairing returns new instances."""
        rename = _classified(
            _finding(
                FindingCategory.SCHEMA_VIOLATION,
                FindingSubtype.UNKNOWN_FIELD,
                source=INDEX,
                description="Unknown field: plan",
                target_key="plan",
            ),
            SafetyClass.SAFE_FIX,
            RENAME_RATIONALE,
            pairing_tag=PAIRING_TAG_PLAN_TO_NAME_RENAME,
        )
        missing_name = _classified(
            _finding(
                FindingCategory.SCHEMA_VIOLATION,
                FindingSubtype.MISSING_REQUIRED_FIELD,
                source=INDEX,
                description="Missing required field: name",
                target_key="name",
            ),
            SafetyClass.EXPOSURE_REVIEW,
            "needs inference",
        )
        inputs = [rename, missing_name]

        out = pair_resolved_by_sibling(inputs)

        # Inputs are unchanged
        assert inputs[0].resolved_by_sibling is None
        assert inputs[1].resolved_by_sibling is None
        # Output is a different list with new instances for the paired half
        assert out is not inputs
        assert out[1] is not missing_name


# ---------------------------------------------------------------------------
# Negative pins: conditions that do NOT pair
# ---------------------------------------------------------------------------

class TestNoSiblingFound:

    def test_missing_name_alone_is_unchanged(self):
        """NEGATIVE PIN: MISSING_REQUIRED_FIELD(name) with no rename sibling
        is NOT marked resolved_by_sibling (it must stay ExposureReview)."""
        missing_name = _classified(
            _finding(
                FindingCategory.SCHEMA_VIOLATION,
                FindingSubtype.MISSING_REQUIRED_FIELD,
                source=INDEX,
                description="Missing required field: name",
                target_key="name",
            ),
            SafetyClass.EXPOSURE_REVIEW,
            "needs inference",
        )

        [out] = pair_resolved_by_sibling([missing_name])

        assert out.resolved_by_sibling is None

    def test_rename_alone_is_unchanged(self):
        """NEGATIVE PIN: rename with no missing-name sibling is unchanged."""
        rename = _classified(
            _finding(
                FindingCategory.SCHEMA_VIOLATION,
                FindingSubtype.UNKNOWN_FIELD,
                source=INDEX,
                description="Unknown field: plan",
                target_key="plan",
            ),
            SafetyClass.SAFE_FIX,
            RENAME_RATIONALE,
            pairing_tag=PAIRING_TAG_PLAN_TO_NAME_RENAME,
        )

        [out] = pair_resolved_by_sibling([rename])

        assert out.resolved_by_sibling is None

    def test_pair_on_different_files_not_linked(self):
        """NEGATIVE PIN: rename on file A and missing-name on file B are NOT linked."""
        index_a = Path("plans/plan-a/index.md")
        index_b = Path("plans/plan-b/index.md")
        rename = _classified(
            _finding(
                FindingCategory.SCHEMA_VIOLATION,
                FindingSubtype.UNKNOWN_FIELD,
                source=index_a,
                description="Unknown field: plan",
                target_key="plan",
            ),
            SafetyClass.SAFE_FIX,
            RENAME_RATIONALE,
            pairing_tag=PAIRING_TAG_PLAN_TO_NAME_RENAME,
        )
        missing_name = _classified(
            _finding(
                FindingCategory.SCHEMA_VIOLATION,
                FindingSubtype.MISSING_REQUIRED_FIELD,
                source=index_b,
                description="Missing required field: name",
                target_key="name",
            ),
            SafetyClass.EXPOSURE_REVIEW,
            "needs inference",
        )

        out = pair_resolved_by_sibling([rename, missing_name])

        [m] = [cf for cf in out if cf.finding.subtype == FindingSubtype.MISSING_REQUIRED_FIELD]
        assert m.resolved_by_sibling is None

    def test_rename_classified_exposure_review_does_not_pair(self):
        """NEGATIVE PIN: collision-case rename (ExposureReview) does NOT
        resolve the sibling — the rename is not actually being auto-applied."""
        collision_rename = _classified(
            _finding(
                FindingCategory.SCHEMA_VIOLATION,
                FindingSubtype.UNKNOWN_FIELD,
                source=INDEX,
                description="Unknown field: plan",
                target_key="plan",
            ),
            SafetyClass.EXPOSURE_REVIEW,  # collision -> ExposureReview
            "Collision: both plan: and name: exist with different values (...)",
        )
        missing_name = _classified(
            _finding(
                FindingCategory.SCHEMA_VIOLATION,
                FindingSubtype.MISSING_REQUIRED_FIELD,
                source=INDEX,
                description="Missing required field: name",
                target_key="name",
            ),
            SafetyClass.EXPOSURE_REVIEW,
            "needs inference",
        )

        out = pair_resolved_by_sibling([collision_rename, missing_name])

        [m] = [cf for cf in out if cf.finding.subtype == FindingSubtype.MISSING_REQUIRED_FIELD]
        assert m.resolved_by_sibling is None

    def test_non_plan_index_file_does_not_pair(self):
        """NEGATIVE PIN: pairing only applies on PlanIndex files (where
        plan:→name: is the canonical rename direction)."""
        section_path = Path("plans/demo-plan/section-01.md")
        rename = _classified(
            _finding(
                FindingCategory.SCHEMA_VIOLATION,
                FindingSubtype.UNKNOWN_FIELD,
                source=section_path,
                description="Unknown field: plan",
                target_key="plan",
            ),
            SafetyClass.SAFE_FIX,
            RENAME_RATIONALE,
            pairing_tag=PAIRING_TAG_PLAN_TO_NAME_RENAME,
        )
        missing_name = _classified(
            _finding(
                FindingCategory.SCHEMA_VIOLATION,
                FindingSubtype.MISSING_REQUIRED_FIELD,
                source=section_path,
                description="Missing required field: name",
                target_key="name",
            ),
            SafetyClass.EXPOSURE_REVIEW,
            "needs inference",
        )

        out = pair_resolved_by_sibling([rename, missing_name])

        [m] = [cf for cf in out if cf.finding.subtype == FindingSubtype.MISSING_REQUIRED_FIELD]
        assert m.resolved_by_sibling is None
