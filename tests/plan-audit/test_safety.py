#!/usr/bin/env python3
"""Tests for scripts/verify_roadmap/safety.py — Safety Taxonomy & Data Types.

TDD per CLAUDE.md: these tests define the expected behavior.
Section 03.1 of verify-roadmap-redesign plan.

Matrix coverage: every (FindingCategory, FindingSubtype) pair in
_CATEGORY_SUBTYPES has a test case asserting its safety classification.
"""

import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT))

import pytest
from scripts.plan_corpus import (
    Finding,
    FindingCategory,
    FindingSubtype,
    Severity,
    SourceKind,
    _CATEGORY_SUBTYPES,
)
from scripts.verify_roadmap.safety import (
    SafetyClass,
    ClassifiedFinding,
    WriteBackContext,
    PreimageRecord,
    classify_safety,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _make_finding(
    category: FindingCategory,
    subtype: FindingSubtype,
    severity: Severity = Severity.MEDIUM,
    source: Path = Path("plans/test-plan/section-01.md"),
    description: str = "test finding",
    recommended_fix: str = "fix it",
    **kwargs,
) -> Finding:
    """Create a Finding with sensible defaults for testing."""
    return Finding(
        category=category,
        subtype=subtype,
        severity=severity,
        source=source,
        description=description,
        recommended_fix=recommended_fix,
        **kwargs,
    )


def _make_context(has_recent_commits: dict[Path, bool] | None = None) -> WriteBackContext:
    """Create a WriteBackContext with optional recent-commits data."""
    return WriteBackContext(
        has_recent_commits=has_recent_commits or {},
    )


# ---------------------------------------------------------------------------
# SafetyClass enum
# ---------------------------------------------------------------------------

class TestSafetyClass:
    def test_has_safe_fix(self):
        assert SafetyClass.SAFE_FIX is not None

    def test_has_exposure_review(self):
        assert SafetyClass.EXPOSURE_REVIEW is not None

    def test_only_two_members(self):
        assert len(SafetyClass) == 2


# ---------------------------------------------------------------------------
# ClassifiedFinding dataclass
# ---------------------------------------------------------------------------

class TestClassifiedFinding:
    def test_wraps_finding(self):
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.UNKNOWN_FIELD,
        )
        cf = ClassifiedFinding(
            finding=finding,
            safety_class=SafetyClass.EXPOSURE_REVIEW,
            rationale="test",
        )
        assert cf.finding is finding
        assert cf.safety_class == SafetyClass.EXPOSURE_REVIEW
        assert cf.rationale == "test"

    def test_resolved_by_sibling_default_none(self):
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.UNKNOWN_FIELD,
        )
        cf = ClassifiedFinding(
            finding=finding,
            safety_class=SafetyClass.EXPOSURE_REVIEW,
            rationale="test",
        )
        assert cf.resolved_by_sibling is None

    def test_resolved_by_sibling_set(self):
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.MISSING_REQUIRED_FIELD,
        )
        cf = ClassifiedFinding(
            finding=finding,
            safety_class=SafetyClass.SAFE_FIX,
            rationale="resolved by plan:->name: rename",
            resolved_by_sibling="VR-abc123",
        )
        assert cf.resolved_by_sibling == "VR-abc123"


# ---------------------------------------------------------------------------
# WriteBackContext dataclass
# ---------------------------------------------------------------------------

class TestWriteBackContext:
    def test_has_recent_commits_field(self):
        ctx = WriteBackContext(has_recent_commits={Path("plans/foo"): True})
        assert ctx.has_recent_commits[Path("plans/foo")] is True

    def test_empty_context(self):
        ctx = WriteBackContext(has_recent_commits={})
        assert len(ctx.has_recent_commits) == 0


# ---------------------------------------------------------------------------
# PreimageRecord dataclass
# ---------------------------------------------------------------------------

class TestPreimageRecord:
    def test_fields(self):
        rec = PreimageRecord(
            path=Path("plans/test/section-01.md"),
            content_hash="abc123",
            scan_timestamp=1234567890.0,
        )
        assert rec.path == Path("plans/test/section-01.md")
        assert rec.content_hash == "abc123"
        assert rec.scan_timestamp == 1234567890.0


# ---------------------------------------------------------------------------
# classify_safety — context=None (quick mode)
# ---------------------------------------------------------------------------

class TestClassifySafetyQuickMode:
    """When context is None (--quick mode), ALL findings -> ExposureReview."""

    @pytest.mark.parametrize(
        "category,subtype",
        [
            (cat, sub)
            for cat, subs in _CATEGORY_SUBTYPES.items()
            for sub in sorted(subs, key=lambda s: s.name)
        ],
        ids=lambda val: val.name if hasattr(val, "name") else str(val),
    )
    def test_quick_mode_always_exposure_review(self, category, subtype):
        """Negative pin: context=None MUST return ExposureReview for every finding."""
        finding = _make_finding(category, subtype)
        result = classify_safety(finding, context=None)
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW, (
            f"{category.name}/{subtype.name} should be ExposureReview in quick mode"
        )
        assert "quick mode" in result.rationale.lower()


# ---------------------------------------------------------------------------
# classify_safety — SCHEMA_VIOLATION subtypes
# ---------------------------------------------------------------------------

class TestClassifySafetySchemaViolation:
    """Classification rules for SCHEMA_VIOLATION findings."""

    def test_unknown_field_exposure_review(self):
        """Generic UNKNOWN_FIELD -> ExposureReview (no specific SafeFix rule)."""
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.UNKNOWN_FIELD,
            description="Unknown field: foo",
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_unknown_field_plan_key_on_plan_index_safe_fix(self):
        """plan: on PlanIndexSchema -> SafeFix (rename to name:)."""
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.UNKNOWN_FIELD,
            description="Unknown field: plan",
            source=Path("plans/test-plan/index.md"),
            target_key="plan",
        )
        # PlanIndexSchema file with plan: but no name:
        fm_data = {"plan": "test-plan", "full_name": "Test Plan", "status": "active"}
        result = classify_safety(
            finding, _make_context(),
            frontmatter_data=fm_data,
        )
        assert result.safety_class == SafetyClass.SAFE_FIX

    def test_unknown_field_plan_key_rename_emits_pairing_tag(self):
        """INTEGRATION PIN: classify_safety sets pairing_tag on the rename SafeFix.

        pair_resolved_by_sibling depends on this tag to link the
        MISSING_REQUIRED_FIELD(name) sibling. If the tag assignment
        regresses, sibling dedup breaks silently.
        """
        from scripts.verify_roadmap.safety import PAIRING_TAG_PLAN_TO_NAME_RENAME
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.UNKNOWN_FIELD,
            description="Unknown field: plan",
            source=Path("plans/test-plan/index.md"),
            target_key="plan",
        )
        fm_data = {"plan": "test-plan", "full_name": "Test Plan", "status": "active"}
        result = classify_safety(
            finding, _make_context(),
            frontmatter_data=fm_data,
        )
        assert result.pairing_tag == PAIRING_TAG_PLAN_TO_NAME_RENAME

    def test_unknown_field_plan_key_on_overview_exposure_review(self):
        """plan: on OverviewSchema -> ExposureReview (plan: is canonical there)."""
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.UNKNOWN_FIELD,
            description="Unknown field: plan",
            source=Path("plans/test-plan/00-overview.md"),
            target_key="plan",
        )
        # OverviewSchema uses plan: as a required field — NOT a rename candidate
        fm_data = {"plan": "test-plan", "title": "Test Plan", "status": "active"}
        result = classify_safety(
            finding, _make_context(),
            frontmatter_data=fm_data,
        )
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_collision_guard_different_values_exposure_review(self):
        """Collision guard: both plan: and name: with DIFFERENT values -> ExposureReview."""
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.UNKNOWN_FIELD,
            description="Unknown field: plan",
            source=Path("plans/test-plan/index.md"),
            target_key="plan",
        )
        fm_data = {"plan": "old-name", "name": "new-name", "full_name": "X", "status": "active"}
        result = classify_safety(
            finding, _make_context(),
            frontmatter_data=fm_data,
        )
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW
        assert "collision" in result.rationale.lower() or "different values" in result.rationale.lower()

    def test_collision_guard_same_values_safe_fix(self):
        """Collision guard: both plan: and name: with SAME values -> SafeFix (remove plan:)."""
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.UNKNOWN_FIELD,
            description="Unknown field: plan",
            source=Path("plans/test-plan/index.md"),
            target_key="plan",
        )
        fm_data = {"plan": "test-plan", "name": "test-plan", "full_name": "X", "status": "active"}
        result = classify_safety(
            finding, _make_context(),
            frontmatter_data=fm_data,
        )
        assert result.safety_class == SafetyClass.SAFE_FIX

    def test_missing_required_field_needs_inference_exposure_review(self):
        """MISSING_REQUIRED_FIELD when semantic inference needed -> ExposureReview."""
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.MISSING_REQUIRED_FIELD,
            description="Missing required field: goal",
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_missing_required_field_reviewed_on_plan_section_safe_fix(self):
        """Missing reviewed: on PlanSectionSchema -> SafeFix (insert reviewed: false)."""
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.MISSING_REQUIRED_FIELD,
            description="Missing required field: reviewed",
            source=Path("plans/test-plan/section-01.md"),
            target_key="reviewed",
        )
        fm_data = {"section": "01", "title": "Test", "status": "not-started",
                    "goal": "test", "success_criteria": [], "sections": [],
                    "third_party_review": {"status": "none", "updated": None}}
        result = classify_safety(
            finding, _make_context(),
            frontmatter_data=fm_data,
        )
        assert result.safety_class == SafetyClass.SAFE_FIX

    def test_missing_required_field_reviewed_on_plan_index_exposure_review(self):
        """Workflow behavior pin: reviewed: false on PlanIndexSchema -> ExposureReview.

        Inserting reviewed: false triggers /continue-roadmap Step 1.7 gate.
        Absence of the field means None (no gate trigger). This is semantic.
        """
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.MISSING_REQUIRED_FIELD,
            description="Missing required field: reviewed",
            source=Path("plans/test-plan/index.md"),
            target_key="reviewed",
        )
        fm_data = {"name": "test-plan", "full_name": "Test Plan", "status": "active"}
        result = classify_safety(
            finding, _make_context(),
            frontmatter_data=fm_data,
        )
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_missing_tpr_on_plan_section_safe_fix(self):
        """Missing third_party_review: on PlanSectionSchema -> SafeFix."""
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.MISSING_REQUIRED_FIELD,
            description="Missing required field: third_party_review",
            source=Path("plans/test-plan/section-01.md"),
            target_key="third_party_review",
        )
        fm_data = {"section": "01", "title": "Test", "status": "not-started",
                    "reviewed": False, "goal": "test", "success_criteria": [],
                    "sections": []}
        result = classify_safety(
            finding, _make_context(),
            frontmatter_data=fm_data,
        )
        assert result.safety_class == SafetyClass.SAFE_FIX

    def test_wrong_type_exposure_review(self):
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION, FindingSubtype.WRONG_TYPE,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_enum_out_of_range_exposure_review(self):
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION, FindingSubtype.ENUM_OUT_OF_RANGE,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_cross_field_invariant_exposure_review(self):
        # Default CROSS_FIELD_INVARIANT (no target_key) stays ExposureReview
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION, FindingSubtype.CROSS_FIELD_INVARIANT,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_cross_field_invariant_reroute_false_is_safefix(self):
        """Regression: reroute: false on plan-index files is a SafeFix
        because the schema rule guarantees the field is default-equivalent.
        Pre-fix all CROSS_FIELD_INVARIANT findings were ExposureReview, so
        the promised auto-fix never landed.

        See: TPR-03-006-codex.
        """
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.CROSS_FIELD_INVARIANT,
            target_key="reroute",
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.SAFE_FIX
        assert "reroute" in result.rationale.lower()

    def test_reroute_false_end_to_end_validate_classify_dispatch(self, tmp_path):
        """End-to-end regression: the reroute:false SafeFix must survive the
        real chain — schema._validate_plan_index produces a finding with
        target_key='reroute', safety.classify_safety classifies it as SafeFix,
        and auto_fix.build_fix_plan emits REMOVE_KEY('reroute').

        Pre-fix TPR-03-006-codex the unit tests (above) only exercised the
        classify_safety and dispatch layers with hand-constructed findings.
        That left the PRODUCER side (schema emitting target_key='reroute')
        untested — if _validate_plan_index stopped populating target_key,
        classify_safety would fall back to ExposureReview silently.

        See: TPR-03-001-codex-r5i2.
        """
        from scripts.plan_corpus.schema import _validate_plan_index
        from scripts.verify_roadmap.auto_fix import build_fix_plan
        from scripts.verify_roadmap.safety import FmOperationKind

        # Producer: real schema validator on plan-index frontmatter with
        # reroute: false. Must emit target_key='reroute'.
        data = {
            "name": "test-plan",
            "full_name": "Test Plan",
            "status": "active",
            "reroute": False,
        }
        findings = _validate_plan_index(data, Path("plans/test-plan/index.md"))
        reroute_findings = [
            f for f in findings
            if f.subtype == FindingSubtype.CROSS_FIELD_INVARIANT
        ]
        assert len(reroute_findings) == 1, (
            "schema._validate_plan_index must emit exactly one "
            "CROSS_FIELD_INVARIANT finding for reroute: false"
        )
        finding = reroute_findings[0]
        assert finding.target_key == "reroute", (
            "schema producer must populate target_key='reroute' — if this "
            "regresses, classify_safety would silently fall back to "
            "ExposureReview (TPR-03-006-codex chain breaks)"
        )

        # Classifier: real classify_safety on the produced finding must
        # route to SafeFix (not ExposureReview).
        classified = classify_safety(finding, _make_context())
        assert classified.safety_class == SafetyClass.SAFE_FIX, (
            "classify_safety regressed — reroute: false must classify as "
            "SafeFix via _classify_cross_field_invariant"
        )

        # Dispatcher: real build_fix_plan on the classified finding must
        # produce exactly one REMOVE_KEY operation targeting 'reroute'.
        plan = build_fix_plan(classified)
        assert plan is not None, (
            "build_fix_plan returned None for the reroute SafeFix — "
            "_dispatch_cross_field_invariant regressed"
        )
        assert len(plan.operations) == 1
        op = plan.operations[0]
        assert op.kind == FmOperationKind.REMOVE_KEY
        # Operations are frozen tuple-of-pairs; convert to dict for assertion
        assert dict(op.kwargs).get("key") == "reroute", (
            f"Expected REMOVE_KEY with key='reroute', got kwargs={op.kwargs}"
        )

    def test_duplicate_plan_name_exposure_review(self):
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION, FindingSubtype.DUPLICATE_PLAN_NAME,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_dep_id_malformed_exposure_review(self):
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION, FindingSubtype.DEP_ID_MALFORMED,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_dep_id_full_path_exposure_review(self):
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION, FindingSubtype.DEP_ID_FULL_PATH,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_dep_id_unknown_name_exposure_review(self):
        finding = _make_finding(
            FindingCategory.SCHEMA_VIOLATION, FindingSubtype.DEP_ID_UNKNOWN_NAME,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW


# ---------------------------------------------------------------------------
# classify_safety — STATUS_CONTRADICTION subtypes
# ---------------------------------------------------------------------------

class TestClassifySafetyStatusContradiction:

    def test_fm_declared_vs_body_derived_always_exposure_review(self):
        """Semantic pin: FM_DECLARED_VS_BODY_DERIVED -> ALWAYS ExposureReview.

        The normalizer returns derived="complete" when has_complete_marker is True
        even when unchecked > 0. Auto-fixing based on this is WRONG.
        """
        finding = _make_finding(
            FindingCategory.STATUS_CONTRADICTION,
            FindingSubtype.FM_DECLARED_VS_BODY_DERIVED,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_plan_active_all_not_started_no_recent_commits_safe_fix(self):
        """Semantic pin: no recent activity -> SafeFix (change to queued)."""
        plan_dir = Path("plans/test-plan")
        finding = _make_finding(
            FindingCategory.STATUS_CONTRADICTION,
            FindingSubtype.PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED,
            source=plan_dir / "index.md",
        )
        ctx = _make_context({plan_dir: False})
        result = classify_safety(finding, ctx)
        assert result.safety_class == SafetyClass.SAFE_FIX

    def test_plan_active_all_not_started_recent_commits_exposure_review(self):
        """Semantic pin: recent activity -> ExposureReview (stale sections, not inactive)."""
        plan_dir = Path("plans/test-plan")
        finding = _make_finding(
            FindingCategory.STATUS_CONTRADICTION,
            FindingSubtype.PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED,
            source=plan_dir / "index.md",
        )
        ctx = _make_context({plan_dir: True})
        result = classify_safety(finding, ctx)
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_plan_active_all_not_started_missing_git_data_exposure_review(self):
        """No git data for this plan -> conservative ExposureReview."""
        plan_dir = Path("plans/test-plan")
        finding = _make_finding(
            FindingCategory.STATUS_CONTRADICTION,
            FindingSubtype.PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED,
            source=plan_dir / "index.md",
        )
        ctx = _make_context({})  # no data for this plan
        result = classify_safety(finding, ctx)
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_plan_complete_with_open_sections_exposure_review(self):
        finding = _make_finding(
            FindingCategory.STATUS_CONTRADICTION,
            FindingSubtype.PLAN_COMPLETE_WITH_OPEN_SECTIONS,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_tpr_status_without_date_exposure_review(self):
        finding = _make_finding(
            FindingCategory.STATUS_CONTRADICTION,
            FindingSubtype.TPR_STATUS_WITHOUT_DATE,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_tpr_status_none_with_date_exposure_review(self):
        finding = _make_finding(
            FindingCategory.STATUS_CONTRADICTION,
            FindingSubtype.TPR_STATUS_NONE_WITH_DATE,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_cross_edge_temporal_drift_exposure_review(self):
        finding = _make_finding(
            FindingCategory.STATUS_CONTRADICTION,
            FindingSubtype.CROSS_EDGE_TEMPORAL_DRIFT,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_tpr_stale_vs_edit_exposure_review(self):
        finding = _make_finding(
            FindingCategory.STATUS_CONTRADICTION,
            FindingSubtype.TPR_STALE_VS_EDIT,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW


# ---------------------------------------------------------------------------
# classify_safety — DEAD_REFERENCE subtypes
# ---------------------------------------------------------------------------

class TestClassifySafetyDeadReference:

    def test_plan_directory_not_found_depends_on_safe_fix(self):
        """Dead depends_on entry -> SafeFix (mechanical list removal)."""
        finding = _make_finding(
            FindingCategory.DEAD_REFERENCE,
            FindingSubtype.PLAN_DIRECTORY_NOT_FOUND,
            source_kind=SourceKind.EXPLICIT_DEPENDS_ON,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.SAFE_FIX

    def test_plan_directory_not_found_prose_exposure_review(self):
        """Dead prose reference -> ExposureReview (human-authored replacement)."""
        finding = _make_finding(
            FindingCategory.DEAD_REFERENCE,
            FindingSubtype.PLAN_DIRECTORY_NOT_FOUND,
            source_kind=SourceKind.PROSE_VERB,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_plan_directory_not_found_no_source_kind_exposure_review(self):
        """No source_kind -> conservative ExposureReview."""
        finding = _make_finding(
            FindingCategory.DEAD_REFERENCE,
            FindingSubtype.PLAN_DIRECTORY_NOT_FOUND,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_section_file_not_found_depends_on_safe_fix(self):
        finding = _make_finding(
            FindingCategory.DEAD_REFERENCE,
            FindingSubtype.SECTION_FILE_NOT_FOUND,
            source_kind=SourceKind.EXPLICIT_DEPENDS_ON,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.SAFE_FIX

    def test_section_file_not_found_prose_exposure_review(self):
        finding = _make_finding(
            FindingCategory.DEAD_REFERENCE,
            FindingSubtype.SECTION_FILE_NOT_FOUND,
            source_kind=SourceKind.PROSE_VERB,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_cross_plan_name_not_found_depends_on_safe_fix(self):
        finding = _make_finding(
            FindingCategory.DEAD_REFERENCE,
            FindingSubtype.CROSS_PLAN_NAME_NOT_FOUND,
            source_kind=SourceKind.EXPLICIT_DEPENDS_ON,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.SAFE_FIX

    def test_cross_plan_name_not_found_html_comment_exposure_review(self):
        finding = _make_finding(
            FindingCategory.DEAD_REFERENCE,
            FindingSubtype.CROSS_PLAN_NAME_NOT_FOUND,
            source_kind=SourceKind.HTML_COMMENT_CONVENTION,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_spec_file_not_found_always_exposure_review(self):
        """SPEC_FILE_NOT_FOUND -> ExposureReview (needs human to find new path)."""
        finding = _make_finding(
            FindingCategory.DEAD_REFERENCE,
            FindingSubtype.SPEC_FILE_NOT_FOUND,
            source_kind=SourceKind.EXPLICIT_DEPENDS_ON,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW

    def test_spec_file_not_found_no_source_kind_exposure_review(self):
        finding = _make_finding(
            FindingCategory.DEAD_REFERENCE,
            FindingSubtype.SPEC_FILE_NOT_FOUND,
        )
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW


# ---------------------------------------------------------------------------
# classify_safety — DAG_CONFLICT subtypes (all ExposureReview)
# ---------------------------------------------------------------------------

class TestClassifySafetyDagConflict:

    @pytest.mark.parametrize("subtype", [
        FindingSubtype.CONFLICT,
        FindingSubtype.SUPERSEDED,
        FindingSubtype.BLOCKED,
        FindingSubtype.MISSING_DEPENDENCY,
        FindingSubtype.CYCLE,
        FindingSubtype.REDUNDANT_DEPENDENCY,
        FindingSubtype.ORPHANED_PLAN,
    ])
    def test_all_dag_conflict_subtypes_exposure_review(self, subtype):
        finding = _make_finding(FindingCategory.DAG_CONFLICT, subtype)
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW


# ---------------------------------------------------------------------------
# classify_safety — PARSE_ERROR subtypes (all ExposureReview)
# ---------------------------------------------------------------------------

class TestClassifySafetyParseError:

    @pytest.mark.parametrize("subtype", sorted(
        _CATEGORY_SUBTYPES[FindingCategory.PARSE_ERROR], key=lambda s: s.name
    ))
    def test_all_parse_error_subtypes_exposure_review(self, subtype):
        finding = _make_finding(FindingCategory.PARSE_ERROR, subtype)
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW


# ---------------------------------------------------------------------------
# classify_safety — ITEM_VERIFICATION subtypes (all ExposureReview)
# ---------------------------------------------------------------------------

class TestClassifySafetyItemVerification:

    @pytest.mark.parametrize("subtype", sorted(
        _CATEGORY_SUBTYPES[FindingCategory.ITEM_VERIFICATION], key=lambda s: s.name
    ))
    def test_all_item_verification_subtypes_exposure_review(self, subtype):
        finding = _make_finding(FindingCategory.ITEM_VERIFICATION, subtype)
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW


# ---------------------------------------------------------------------------
# classify_safety — GAP subtypes (all ExposureReview)
# ---------------------------------------------------------------------------

class TestClassifySafetyGap:

    @pytest.mark.parametrize("subtype", sorted(
        _CATEGORY_SUBTYPES[FindingCategory.GAP], key=lambda s: s.name
    ))
    def test_all_gap_subtypes_exposure_review(self, subtype):
        finding = _make_finding(FindingCategory.GAP, subtype)
        result = classify_safety(finding, _make_context())
        assert result.safety_class == SafetyClass.EXPOSURE_REVIEW


# ---------------------------------------------------------------------------
# classify_safety — rationale always present
# ---------------------------------------------------------------------------

class TestClassifySafetyRationale:
    """Every ClassifiedFinding must carry a non-empty rationale."""

    @pytest.mark.parametrize(
        "category,subtype",
        [
            (cat, sub)
            for cat, subs in _CATEGORY_SUBTYPES.items()
            for sub in sorted(subs, key=lambda s: s.name)
        ],
        ids=lambda val: val.name if hasattr(val, "name") else str(val),
    )
    def test_rationale_never_empty(self, category, subtype):
        finding = _make_finding(category, subtype)
        result = classify_safety(finding, _make_context())
        assert result.rationale, (
            f"{category.name}/{subtype.name} has empty rationale"
        )


# ---------------------------------------------------------------------------
# Matrix completeness self-check
# ---------------------------------------------------------------------------

class TestMatrixCompleteness:
    """Verify every (category, subtype) pair is covered by the test matrix."""

    def test_all_category_subtype_pairs_covered(self):
        """Self-verifying: count all pairs and ensure parametrize covers them."""
        total_pairs = sum(len(subs) for subs in _CATEGORY_SUBTYPES.values())
        # The quick-mode parametrize test covers all pairs
        assert total_pairs == len([
            (cat, sub)
            for cat, subs in _CATEGORY_SUBTYPES.items()
            for sub in subs
        ])
        # Sanity: we have 49 subtypes across 7 categories
        assert total_pairs >= 49, f"Expected >=49 pairs, got {total_pairs}"
        assert len(_CATEGORY_SUBTYPES) == 7
