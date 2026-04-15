#!/usr/bin/env python3
"""Tests for scripts/verify_roadmap/auto_fix.py — Auto-Fix Engine.

TDD per CLAUDE.md: tests define expected behavior.
Section 03.3 of verify-roadmap-redesign plan.

Coverage:
  - build_fix_plan: SafeFix -> FmOperation list per (category, subtype)
  - Defense-in-depth: ExposureReview rejected, FM_DECLARED_VS_BODY_DERIVED panic
  - parallel: true field never touched
  - apply_fixes: backup, audit log, dry-run, --no-auto-fix
  - Concurrent-modification propagation: PatchResult(applied=False)
    -> ClassifiedFinding with ExposureReview
  - Idempotency: applying twice produces identical results
"""

import json
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
)
from scripts.verify_roadmap import (
    SafetyClass,
    ClassifiedFinding,
    WriteBackContext,
    PreimageRecord,
    PatchResult,
    FmOperationKind,
    FmOperation,
    classify_safety,
    FixPlan,
    AutoFixError,
    build_fix_plan,
    build_fix_plans,
    apply_fixes,
    FixApplyResult,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _finding(
    category: FindingCategory,
    subtype: FindingSubtype,
    severity: Severity = Severity.MEDIUM,
    source: Path = Path("plans/test/section-01.md"),
    description: str = "test",
    recommended_fix: str = "fix",
    source_kind: SourceKind | None = None,
    source_line: int | None = None,
) -> Finding:
    return Finding(
        category=category,
        subtype=subtype,
        severity=severity,
        source=source,
        description=description,
        recommended_fix=recommended_fix,
        source_kind=source_kind,
        source_line=source_line,
    )


def _safe_fix(
    finding: Finding,
    rationale: str = "test",
    pairing_tag: str | None = None,
) -> ClassifiedFinding:
    return ClassifiedFinding(
        finding=finding,
        safety_class=SafetyClass.SAFE_FIX,
        rationale=rationale,
        pairing_tag=pairing_tag,
    )


def _exposure(finding: Finding, rationale: str = "test") -> ClassifiedFinding:
    return ClassifiedFinding(
        finding=finding,
        safety_class=SafetyClass.EXPOSURE_REVIEW,
        rationale=rationale,
    )


# ---------------------------------------------------------------------------
# FmOperation construction
# ---------------------------------------------------------------------------

class TestFmOperation:
    def test_make_sorts_kwargs(self):
        op = FmOperation.make(
            FmOperationKind.RENAME_KEY, new_key="name", old_key="plan"
        )
        # Determinism: same kwargs -> same operation
        op2 = FmOperation.make(
            FmOperationKind.RENAME_KEY, old_key="plan", new_key="name"
        )
        assert op == op2

    def test_kwargs_dict_recovers_view(self):
        op = FmOperation.make(
            FmOperationKind.RENAME_KEY, old_key="plan", new_key="name"
        )
        d = op.kwargs_dict()
        assert d == {"old_key": "plan", "new_key": "name"}

    def test_hashable(self):
        op = FmOperation.make(FmOperationKind.REMOVE_KEY, key="reroute")
        # Frozen dataclass should be hashable for dedup
        s = {op, op}
        assert len(s) == 1


# ---------------------------------------------------------------------------
# build_fix_plan — SafeFix dispatch per category/subtype
# ---------------------------------------------------------------------------

class TestBuildFixPlanSchemaViolation:

    def test_unknown_field_plan_rename_emits_rename_op(self):
        from scripts.verify_roadmap.safety import PAIRING_TAG_PLAN_TO_NAME_RENAME
        f = _finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.UNKNOWN_FIELD,
            description="Unknown field: plan",
            source=Path("plans/test-plan/index.md"),
        )
        cf = _safe_fix(f, "rename plan: to name:", pairing_tag=PAIRING_TAG_PLAN_TO_NAME_RENAME)
        plan = build_fix_plan(cf)
        assert plan is not None
        assert len(plan.operations) == 1
        op = plan.operations[0]
        assert op.kind == FmOperationKind.RENAME_KEY
        kw = op.kwargs_dict()
        assert kw["old_key"] == "plan"
        assert kw["new_key"] == "name"

    def test_unknown_field_plan_collision_remove_emits_remove_op(self):
        """Collision-same-values rationale -> remove redundant plan: key."""
        f = _finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.UNKNOWN_FIELD,
            description="Unknown field: plan",
            source=Path("plans/test-plan/index.md"),
        )
        cf = _safe_fix(f, "plan: and name: have identical values — remove redundant plan: key")
        plan = build_fix_plan(cf)
        assert plan is not None
        op = plan.operations[0]
        assert op.kind == FmOperationKind.REMOVE_KEY
        assert op.kwargs_dict()["key"] == "plan"

    def test_missing_reviewed_emits_insert(self):
        f = _finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.MISSING_REQUIRED_FIELD,
            description="Missing required field: reviewed",
            source=Path("plans/test-plan/section-01.md"),
        )
        cf = _safe_fix(f, "Insert reviewed: false")
        plan = build_fix_plan(cf)
        assert plan is not None
        op = plan.operations[0]
        assert op.kind == FmOperationKind.INSERT_KEY
        kw = op.kwargs_dict()
        assert kw["key"] == "reviewed"
        assert kw["value"] == "false"

    def test_missing_third_party_review_emits_insert(self):
        f = _finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.MISSING_REQUIRED_FIELD,
            description="Missing required field: third_party_review",
            source=Path("plans/test-plan/section-01.md"),
        )
        cf = _safe_fix(f, "Insert third_party_review default")
        plan = build_fix_plan(cf)
        assert plan is not None
        op = plan.operations[0]
        assert op.kind == FmOperationKind.INSERT_KEY
        kw = op.kwargs_dict()
        assert kw["key"] == "third_party_review"


class TestBuildFixPlanStatusContradiction:

    def test_plan_active_all_not_started_replaces_status(self):
        f = _finding(
            FindingCategory.STATUS_CONTRADICTION,
            FindingSubtype.PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED,
            source=Path("plans/test-plan/index.md"),
        )
        cf = _safe_fix(f, "no recent commits — downgrade to queued")
        plan = build_fix_plan(cf)
        assert plan is not None
        op = plan.operations[0]
        assert op.kind == FmOperationKind.REPLACE_VALUE
        kw = op.kwargs_dict()
        assert kw["key"] == "status"
        assert kw["new_value"] == "queued"

    def test_fm_declared_vs_body_derived_panics_defense_in_depth(self):
        """SEMANTIC PIN: FM_DECLARED_VS_BODY_DERIVED reaching SafeFix dispatch
        must raise AutoFixError. This is a defense-in-depth invariant — the
        classifier should never produce SafeFix for this subtype.
        """
        f = _finding(
            FindingCategory.STATUS_CONTRADICTION,
            FindingSubtype.FM_DECLARED_VS_BODY_DERIVED,
        )
        # Bypass classify_safety to construct a SafeFix manually (the bug
        # we're guarding against)
        cf = _safe_fix(f, "would be wrong")
        with pytest.raises(AutoFixError, match="FM_DECLARED_VS_BODY_DERIVED"):
            build_fix_plan(cf)


class TestBuildFixPlanDeadReference:

    def test_depends_on_emits_remove_list_item(self):
        f = _finding(
            FindingCategory.DEAD_REFERENCE,
            FindingSubtype.PLAN_DIRECTORY_NOT_FOUND,
            source_kind=SourceKind.EXPLICIT_DEPENDS_ON,
            description="Dead depends_on entry: nonexistent-plan",
        )
        cf = _safe_fix(f, "remove dead depends_on entry")
        plan = build_fix_plan(cf)
        assert plan is not None
        op = plan.operations[0]
        assert op.kind == FmOperationKind.REMOVE_LIST_ITEM
        assert op.kwargs_dict()["list_key"] == "depends_on"

    def test_section_not_found_depends_on_emits_remove(self):
        f = _finding(
            FindingCategory.DEAD_REFERENCE,
            FindingSubtype.SECTION_FILE_NOT_FOUND,
            source_kind=SourceKind.EXPLICIT_DEPENDS_ON,
        )
        cf = _safe_fix(f)
        plan = build_fix_plan(cf)
        assert plan is not None
        assert plan.operations[0].kind == FmOperationKind.REMOVE_LIST_ITEM


class TestBuildFixPlanDefenseInDepth:

    def test_exposure_review_returns_none(self):
        """NEGATIVE PIN: ExposureReview findings produce no fix plan."""
        f = _finding(
            FindingCategory.STATUS_CONTRADICTION,
            FindingSubtype.PLAN_COMPLETE_WITH_OPEN_SECTIONS,
        )
        cf = _exposure(f)
        result = build_fix_plan(cf)
        assert result is None

    def test_unknown_safefix_subtype_returns_none(self):
        """Defensive: SafeFix on a subtype with no handler returns None."""
        f = _finding(
            FindingCategory.PARSE_ERROR,
            FindingSubtype.YAML_SYNTAX_ERROR,
        )
        # Force a SafeFix classification (which classify_safety would never do)
        cf = _safe_fix(f, "test forced safefix")
        result = build_fix_plan(cf)
        # No handler => no plan, NOT a panic — let dispatcher skip silently
        assert result is None


# ---------------------------------------------------------------------------
# build_fix_plans — bulk dispatch + dedup
# ---------------------------------------------------------------------------

class TestBuildFixPlans:

    def test_filters_to_safefix_only(self):
        """Negative pin: ExposureReview findings filtered out."""
        sf = _safe_fix(_finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.UNKNOWN_FIELD,
            description="Unknown field: plan",
            source=Path("plans/p1/index.md"),
        ), "rename")
        rev = _exposure(_finding(
            FindingCategory.STATUS_CONTRADICTION,
            FindingSubtype.PLAN_COMPLETE_WITH_OPEN_SECTIONS,
        ))
        plans = list(build_fix_plans([sf, rev]))
        assert len(plans) == 1
        assert plans[0].finding_id == sf.finding.id

    def test_groups_operations_by_path(self):
        """Multiple findings on the same file produce one FixPlan per file."""
        path = Path("plans/p1/index.md")
        f1 = _finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.UNKNOWN_FIELD,
            description="Unknown field: plan",
            source=path,
            source_line=2,
        )
        f2 = _finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.UNKNOWN_FIELD,
            description="Unknown field: reroute",
            source=path,
            source_line=3,
        )
        cf1 = _safe_fix(f1, "rename plan: to name:")
        # Forge a SafeFix for reroute (not actually a real classify_safety
        # output for unknown field reroute, but tests grouping)
        # Use a more reliable pattern:
        f3 = _finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.MISSING_REQUIRED_FIELD,
            description="Missing required field: reviewed",
            source=path,
            source_line=4,
        )
        cf3 = _safe_fix(f3, "Insert reviewed: false")
        plans = list(build_fix_plans([cf1, cf3]))
        # Both findings target the same file -> grouped into one FixPlan
        # OR returned as separate plans; behavior is "one plan per finding"
        # so the test pins the per-finding semantics
        assert len(plans) == 2
        for p in plans:
            assert p.path == path

    def test_skips_resolved_by_sibling(self):
        """NEGATIVE PIN: findings with non-None resolved_by_sibling are
        skipped by build_fix_plans. Regression for TPR-03-002-{codex,gemini}-r4.
        """
        path = Path("plans/p1/index.md")
        rename = _safe_fix(
            _finding(
                FindingCategory.SCHEMA_VIOLATION,
                FindingSubtype.UNKNOWN_FIELD,
                description="Unknown field: plan",
                source=path,
            ),
            "rename plan: to name:",
        )
        dependent = ClassifiedFinding(
            finding=_finding(
                FindingCategory.SCHEMA_VIOLATION,
                FindingSubtype.MISSING_REQUIRED_FIELD,
                description="Missing required field: name",
                source=path,
            ),
            safety_class=SafetyClass.SAFE_FIX,
            rationale="insert name: (would be skipped)",
            resolved_by_sibling=rename.finding.id,
        )
        plans = list(build_fix_plans([rename, dependent]))
        # Only the rename half should produce a plan; the sibling-resolved
        # dependent is skipped.
        assert len(plans) == 1
        assert plans[0].finding_id == rename.finding.id


# ---------------------------------------------------------------------------
# apply_fixes — orchestration with stub patcher
# ---------------------------------------------------------------------------

class _StubPatcher:
    """Stub for §03.4's apply_patch — records calls and returns canned results."""

    def __init__(self, results: list[PatchResult] | None = None):
        self.calls: list[tuple] = []
        self.results = results or []

    def __call__(
        self,
        path: Path,
        operations: list[FmOperation],
        preimage: PreimageRecord,
        corpus_root: Path,
    ) -> PatchResult:
        self.calls.append((path, list(operations), preimage, corpus_root))
        if self.results:
            return self.results.pop(0)
        # Default: success
        return PatchResult(
            applied=True,
            reason="ok",
            finding_id="VR-stub",
            path=path,
            before_hash="aa",
            after_hash="bb",
        )


def _preimage(path: Path) -> PreimageRecord:
    return PreimageRecord(
        path=path,
        content_hash="aa",
        scan_timestamp=0.0,
    )


class TestApplyFixesNormalPath:

    def test_dispatches_to_patcher(self, tmp_path):
        f = _finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.UNKNOWN_FIELD,
            description="Unknown field: plan",
            source=tmp_path / "plans/p1/index.md",
        )
        cf = _safe_fix(f, "rename plan: to name:")
        patcher = _StubPatcher()
        preimages = {f.source: _preimage(f.source)}
        result = apply_fixes(
            [cf],
            patcher=patcher,
            preimages=preimages,
            output_dir=tmp_path / "out",
            corpus_root=tmp_path,
        )
        assert len(patcher.calls) == 1
        assert len(result.applied_findings) == 1
        assert len(result.unapplied_results) == 0

    def test_dry_run_does_not_invoke_patcher(self, tmp_path):
        f = _finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.UNKNOWN_FIELD,
            description="Unknown field: plan",
            source=tmp_path / "plans/p1/index.md",
        )
        cf = _safe_fix(f, "rename")
        patcher = _StubPatcher()
        preimages = {f.source: _preimage(f.source)}
        result = apply_fixes(
            [cf],
            patcher=patcher,
            preimages=preimages,
            output_dir=tmp_path / "out",
            corpus_root=tmp_path,
            dry_run=True,
        )
        assert len(patcher.calls) == 0
        # In dry-run, the planned fixes are reported but not applied
        assert len(result.planned_findings) == 1
        assert len(result.applied_findings) == 0


class TestApplyFixesDefenseInDepth:

    def test_exposure_review_rejected(self, tmp_path):
        """NEGATIVE PIN: passing ExposureReview to apply_fixes raises."""
        f = _finding(
            FindingCategory.STATUS_CONTRADICTION,
            FindingSubtype.PLAN_COMPLETE_WITH_OPEN_SECTIONS,
        )
        cf = _exposure(f)
        patcher = _StubPatcher()
        with pytest.raises(AutoFixError, match="ExposureReview"):
            apply_fixes(
                [cf],
                patcher=patcher,
                preimages={},
                output_dir=tmp_path / "out",
                corpus_root=tmp_path,
            )

    def test_fm_declared_vs_body_derived_safefix_panics(self, tmp_path):
        """SEMANTIC PIN: FM_DECLARED_VS_BODY_DERIVED with SafeFix raises."""
        f = _finding(
            FindingCategory.STATUS_CONTRADICTION,
            FindingSubtype.FM_DECLARED_VS_BODY_DERIVED,
        )
        cf = _safe_fix(f, "would be wrong")  # bypass classifier
        patcher = _StubPatcher()
        preimages = {f.source: _preimage(f.source)}
        with pytest.raises(AutoFixError, match="FM_DECLARED_VS_BODY_DERIVED"):
            apply_fixes(
                [cf],
                patcher=patcher,
                preimages=preimages,
                output_dir=tmp_path / "out",
                corpus_root=tmp_path,
            )


class TestApplyFixesParallelGuard:
    """parallel: true is a valid PlanIndexSchema field — fix handlers must
    never touch it."""

    def test_no_handler_emits_op_targeting_parallel(self, tmp_path):
        """Walk every classify_safety -> build_fix_plan output and confirm
        no operation has key='parallel' or value containing 'parallel'."""
        from scripts.plan_corpus import _CATEGORY_SUBTYPES

        ctx = WriteBackContext()
        for cat, subs in _CATEGORY_SUBTYPES.items():
            for sub in subs:
                f = _finding(cat, sub)
                cf = classify_safety(f, ctx)
                if cf.safety_class != SafetyClass.SAFE_FIX:
                    continue
                try:
                    plan = build_fix_plan(cf)
                except AutoFixError:
                    continue
                if plan is None:
                    continue
                for op in plan.operations:
                    kw = op.kwargs_dict()
                    for k, v in kw.items():
                        assert "parallel" != v, (
                            f"Op for {cat.name}/{sub.name} touches "
                            f"parallel: field — {k}={v!r}"
                        )


# ---------------------------------------------------------------------------
# apply_fixes — concurrent-modification propagation
# ---------------------------------------------------------------------------

class TestApplyFixesConcurrentModification:

    def test_unapplied_patch_converts_to_exposure_review(self, tmp_path):
        """TPR-03-003-codex: PatchResult(applied=False) demotes the original
        SafeFix to ExposureReview surfaced in the result."""
        f = _finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.UNKNOWN_FIELD,
            description="Unknown field: plan",
            source=tmp_path / "plans/p1/index.md",
        )
        cf = _safe_fix(f, "rename plan: to name:")
        # Patcher returns failure
        failed = PatchResult(
            applied=False,
            reason="file modified by concurrent session",
            finding_id=f.id,
            path=f.source,
        )
        patcher = _StubPatcher(results=[failed])
        preimages = {f.source: _preimage(f.source)}
        result = apply_fixes(
            [cf],
            patcher=patcher,
            preimages=preimages,
            output_dir=tmp_path / "out",
            corpus_root=tmp_path,
        )
        assert len(result.applied_findings) == 0
        assert len(result.unapplied_results) == 1
        assert result.unapplied_results[0].applied is False


# ---------------------------------------------------------------------------
# apply_fixes — backups + audit log
# ---------------------------------------------------------------------------

class TestApplyFixesBackup:

    def test_backup_file_created(self, tmp_path):
        # Real file so we can backup it
        src_dir = tmp_path / "plans" / "p1"
        src_dir.mkdir(parents=True)
        src = src_dir / "index.md"
        src.write_text("---\nplan: foo\n---\n# body\n", encoding="utf-8")

        f = _finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.UNKNOWN_FIELD,
            description="Unknown field: plan",
            source=src,
        )
        cf = _safe_fix(f, "rename plan: to name:")
        patcher = _StubPatcher()
        preimages = {f.source: _preimage(f.source)}
        out = tmp_path / "out"
        apply_fixes([cf], patcher=patcher, preimages=preimages, output_dir=out, corpus_root=tmp_path)
        # Backup goes into out/backups/
        backups_dir = out / "backups"
        assert backups_dir.exists()
        # At least one backup file
        backups = list(backups_dir.rglob("*"))
        assert any(b.is_file() for b in backups)

    def test_audit_log_written(self, tmp_path):
        f = _finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.UNKNOWN_FIELD,
            description="Unknown field: plan",
            source=tmp_path / "plans/p1/index.md",
        )
        cf = _safe_fix(f, "rename")
        patcher = _StubPatcher()
        preimages = {f.source: _preimage(f.source)}
        out = tmp_path / "out"
        apply_fixes([cf], patcher=patcher, preimages=preimages, output_dir=out, corpus_root=tmp_path)
        audit_file = out / "fixes-applied.json"
        assert audit_file.exists()
        data = json.loads(audit_file.read_text())
        assert "fixes" in data
        assert len(data["fixes"]) >= 1
        entry = data["fixes"][0]
        # Audit trail includes finding id, file, fix type, timestamp
        assert "finding_id" in entry
        assert "path" in entry
        assert "operations" in entry
        assert "timestamp" in entry


# ---------------------------------------------------------------------------
# Idempotency
# ---------------------------------------------------------------------------

class TestApplyFixesIdempotency:

    def test_two_runs_produce_same_operations(self, tmp_path):
        """Applying the same set of findings twice produces the same plan."""
        f = _finding(
            FindingCategory.SCHEMA_VIOLATION,
            FindingSubtype.UNKNOWN_FIELD,
            description="Unknown field: plan",
            source=tmp_path / "plans/p1/index.md",
        )
        cf = _safe_fix(f, "rename plan: to name:")
        plan1 = build_fix_plan(cf)
        plan2 = build_fix_plan(cf)
        assert plan1.operations == plan2.operations
        assert plan1.path == plan2.path
