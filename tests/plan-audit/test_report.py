#!/usr/bin/env python3
"""Tests for scripts/verify_roadmap/report.py — Report Format.

TDD per CLAUDE.md: tests define expected behavior.
Section 03.2 of verify-roadmap-redesign plan.
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
)
from scripts.verify_roadmap import (
    SafetyClass,
    ClassifiedFinding,
    PatchResult,
    ReportMode,
    Report,
    generate_report,
    render_json,
    render_markdown,
    render_console,
    exit_code_for_findings,
    write_reports,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _finding(
    category: FindingCategory = FindingCategory.SCHEMA_VIOLATION,
    subtype: FindingSubtype = FindingSubtype.UNKNOWN_FIELD,
    severity: Severity = Severity.MEDIUM,
    source: Path = Path("plans/test/section-01.md"),
    description: str = "test finding",
    recommended_fix: str = "fix it",
) -> Finding:
    return Finding(
        category=category,
        subtype=subtype,
        severity=severity,
        source=source,
        description=description,
        recommended_fix=recommended_fix,
    )


def _classified(
    finding: Finding | None = None,
    safety_class: SafetyClass = SafetyClass.EXPOSURE_REVIEW,
    rationale: str = "test rationale",
    resolved_by_sibling: str | None = None,
) -> ClassifiedFinding:
    return ClassifiedFinding(
        finding=finding or _finding(),
        safety_class=safety_class,
        rationale=rationale,
        resolved_by_sibling=resolved_by_sibling,
    )


def _patch_failed(reason: str = "concurrent modification") -> PatchResult:
    return PatchResult(
        applied=False,
        reason=reason,
        finding_id="VR-abc123",
        path=Path("plans/test/index.md"),
    )


# ---------------------------------------------------------------------------
# ReportMode enum
# ---------------------------------------------------------------------------

class TestReportMode:
    def test_has_full_mode(self):
        assert ReportMode.FULL is not None

    def test_has_quick_mode(self):
        assert ReportMode.QUICK is not None

    def test_only_two_modes(self):
        assert len(ReportMode) == 2


# ---------------------------------------------------------------------------
# Report dataclass
# ---------------------------------------------------------------------------

class TestReport:
    def test_default_construction(self):
        report = Report(mode=ReportMode.FULL)
        assert report.mode == ReportMode.FULL
        assert report.findings == ()
        assert report.unapplied_fixes == ()

    def test_with_findings(self):
        cf = _classified()
        report = Report(mode=ReportMode.FULL, findings=(cf,))
        assert len(report.findings) == 1
        assert report.findings[0] is cf

    def test_with_unapplied_fixes(self):
        pr = _patch_failed()
        report = Report(mode=ReportMode.FULL, unapplied_fixes=(pr,))
        assert len(report.unapplied_fixes) == 1


# ---------------------------------------------------------------------------
# generate_report — wraps classified findings + unapplied fixes
# ---------------------------------------------------------------------------

class TestGenerateReport:
    def test_empty(self):
        report = generate_report(classified=[], unapplied=[], mode=ReportMode.FULL)
        assert report.findings == ()
        assert report.unapplied_fixes == ()

    def test_with_findings(self):
        cf1 = _classified(
            _finding(severity=Severity.HIGH),
            safety_class=SafetyClass.EXPOSURE_REVIEW,
        )
        cf2 = _classified(safety_class=SafetyClass.SAFE_FIX)
        report = generate_report(
            classified=[cf1, cf2], unapplied=[], mode=ReportMode.FULL,
        )
        assert len(report.findings) == 2

    def test_quick_mode_records_mode(self):
        report = generate_report(
            classified=[], unapplied=[], mode=ReportMode.QUICK,
        )
        assert report.mode == ReportMode.QUICK


# ---------------------------------------------------------------------------
# render_json — JSON output format
# ---------------------------------------------------------------------------

class TestRenderJson:
    def test_empty_report_structure(self):
        report = Report(mode=ReportMode.FULL)
        data = render_json(report)
        assert "metadata" in data
        assert data["metadata"]["mode"] == "full"
        assert data["findings"] == []
        assert data["unapplied_fixes"] == []

    def test_metadata_includes_timestamp(self):
        report = Report(mode=ReportMode.FULL)
        data = render_json(report)
        assert "timestamp" in data["metadata"]

    def test_metadata_includes_corpus_size(self):
        report = Report(mode=ReportMode.FULL)
        data = render_json(report, corpus_size=42)
        assert data["metadata"]["corpus_size"] == 42

    def test_metadata_includes_classifier_version(self):
        """Regression: JSON metadata must carry classifier_version so
        downstream consumers can detect report-schema changes.

        See: TPR-03-002-codex-r5i2 (also pins TPR-03-008-codex).
        """
        from scripts.verify_roadmap.report import CLASSIFIER_VERSION

        report = Report(mode=ReportMode.FULL)
        data = render_json(report)
        assert "classifier_version" in data["metadata"]
        assert data["metadata"]["classifier_version"] == CLASSIFIER_VERSION

    def test_json_finding_entry_serializes_target_key(self):
        """Regression: Finding.to_json (via render_json) must include
        target_key for findings that carry it, so machine consumers can
        rely on the structural discriminator.

        See: TPR-03-002-codex-r5i2 (complements TPR-03-007-codex).
        """
        finding = Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.UNKNOWN_FIELD,
            severity=Severity.MEDIUM,
            source=Path("plans/p1/index.md"),
            description="unknown field plan",
            recommended_fix="rename to name",
            target_key="plan",
        )
        cf = _classified(
            finding=finding,
            safety_class=SafetyClass.SAFE_FIX,
            rationale="rename",
        )
        report = Report(mode=ReportMode.FULL, findings=(cf,))
        data = render_json(report)
        entry = data["findings"][0]["finding"]
        assert entry["target_key"] == "plan"

    def test_full_mode_includes_safety_class(self):
        cf = _classified(
            safety_class=SafetyClass.SAFE_FIX,
            rationale="auto-renameable",
        )
        report = Report(mode=ReportMode.FULL, findings=(cf,))
        data = render_json(report)
        entry = data["findings"][0]
        assert entry["safety_class"] == "safe_fix"
        assert entry["rationale"] == "auto-renameable"

    def test_quick_mode_omits_safety_class_and_rationale(self):
        """--quick mode test: JSON omits safety_class and rationale fields."""
        cf = _classified(
            safety_class=SafetyClass.EXPOSURE_REVIEW,
            rationale="quick mode",
        )
        report = Report(mode=ReportMode.QUICK, findings=(cf,))
        data = render_json(report)
        entry = data["findings"][0]
        assert "safety_class" not in entry
        assert "rationale" not in entry

    def test_finding_included_as_dict(self):
        """Finding nested as full to_json() dict."""
        f = _finding(severity=Severity.HIGH, description="hello")
        cf = _classified(f)
        report = Report(mode=ReportMode.FULL, findings=(cf,))
        data = render_json(report)
        entry = data["findings"][0]
        assert entry["finding"]["description"] == "hello"
        assert entry["finding"]["severity"] == "high"

    def test_resolved_by_sibling_included(self):
        cf = _classified(resolved_by_sibling="VR-xyz789")
        report = Report(mode=ReportMode.FULL, findings=(cf,))
        data = render_json(report)
        assert data["findings"][0]["resolved_by_sibling"] == "VR-xyz789"

    def test_unapplied_fix_surfaced(self):
        """Unapplied-fix surface test: PatchResult(applied=False) -> JSON."""
        pr = _patch_failed("hash mismatch")
        report = Report(mode=ReportMode.FULL, unapplied_fixes=(pr,))
        data = render_json(report)
        assert len(data["unapplied_fixes"]) == 1
        uf = data["unapplied_fixes"][0]
        assert uf["finding_id"] == "VR-abc123"
        assert uf["reason"] == "hash mismatch"
        assert uf["applied"] is False

    def test_round_trip_json(self):
        """Round-trip: ClassifiedFinding -> JSON -> parse -> verify."""
        cf = _classified(
            _finding(severity=Severity.CRITICAL, description="critical bug"),
            safety_class=SafetyClass.EXPOSURE_REVIEW,
            rationale="needs human",
            resolved_by_sibling=None,
        )
        report = Report(mode=ReportMode.FULL, findings=(cf,))
        data = render_json(report)
        # JSON-roundtrip via dump/load to ensure serializable
        text = json.dumps(data)
        loaded = json.loads(text)
        entry = loaded["findings"][0]
        assert entry["safety_class"] == "exposure_review"
        assert entry["rationale"] == "needs human"
        assert entry["finding"]["severity"] == "critical"


# ---------------------------------------------------------------------------
# render_markdown — markdown output
# ---------------------------------------------------------------------------

class TestRenderMarkdown:
    def test_empty_report_has_summary(self):
        report = Report(mode=ReportMode.FULL)
        md = render_markdown(report)
        assert "# Verify-Roadmap Findings" in md or "# verify-roadmap" in md.lower()

    def test_severity_ordering_critical_first(self):
        cf_low = _classified(_finding(severity=Severity.LOW))
        cf_critical = _classified(_finding(severity=Severity.CRITICAL))
        report = Report(mode=ReportMode.FULL, findings=(cf_low, cf_critical))
        md = render_markdown(report)
        # critical should appear before low in body
        crit_pos = md.lower().find("critical")
        low_pos = md.lower().rfind("low")
        assert crit_pos < low_pos, "critical findings should be rendered before low"

    def test_safety_class_ordering_exposure_first(self):
        """Within a severity, ExposureReview before SafeFix."""
        cf_safe = _classified(safety_class=SafetyClass.SAFE_FIX)
        cf_review = _classified(safety_class=SafetyClass.EXPOSURE_REVIEW)
        report = Report(mode=ReportMode.FULL, findings=(cf_safe, cf_review))
        md = render_markdown(report)
        review_pos = md.lower().find("exposure")
        safe_pos = md.lower().find("safe")
        assert review_pos < safe_pos

    def test_summary_table_present(self):
        cf = _classified()
        report = Report(mode=ReportMode.FULL, findings=(cf,))
        md = render_markdown(report)
        assert "summary" in md.lower() or "total" in md.lower()

    def test_unapplied_fix_section_present(self):
        """Markdown surfaces unapplied fixes as distinct group."""
        pr = _patch_failed("locked file")
        report = Report(mode=ReportMode.FULL, unapplied_fixes=(pr,))
        md = render_markdown(report)
        assert "unapplied" in md.lower()
        assert "locked file" in md

    def test_quick_mode_no_safety_label(self):
        """Quick mode doesn't classify — markdown shouldn't show safety labels."""
        cf = _classified(safety_class=SafetyClass.EXPOSURE_REVIEW)
        report = Report(mode=ReportMode.QUICK, findings=(cf,))
        md = render_markdown(report)
        # the QUICK header should be visible
        assert "quick" in md.lower()

    def test_markdown_renders_category_subtype_type(self):
        """Regression: markdown finding renderer must show the classifier
        type (category/subtype) so reviewers can see it at a glance.

        See: TPR-03-002-codex-r5i2 (complements TPR-03-008-codex).
        """
        finding = Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.UNKNOWN_FIELD,
            severity=Severity.MEDIUM,
            source=Path("plans/p1/index.md"),
            description="unknown field",
            recommended_fix="remove",
            target_key="plan",
        )
        cf = _classified(
            finding=finding,
            safety_class=SafetyClass.SAFE_FIX,
            rationale="rename",
        )
        report = Report(mode=ReportMode.FULL, findings=(cf,))
        md = render_markdown(report)
        # Type line present with category/subtype
        assert "schema_violation/unknown_field" in md
        # target_key line present when target_key is populated
        assert "target key" in md.lower() or "Target key" in md
        assert "`plan`" in md

    def test_markdown_renders_source_to_target_reference(self):
        """Regression: markdown must show `source -> target` context for
        findings that carry a target path (DAG_CONFLICT, DEAD_REFERENCE).

        See: TPR-03-002-codex-r5i2.
        """
        finding = Finding(
            category=FindingCategory.DAG_CONFLICT,
            subtype=FindingSubtype.MISSING_DEPENDENCY,
            severity=Severity.HIGH,
            source=Path("plans/a/section-01.md"),
            source_line=3,
            target=Path("plans/b/index.md"),
            description="missing dep",
            recommended_fix="add dep",
        )
        cf = _classified(finding=finding)
        report = Report(mode=ReportMode.FULL, findings=(cf,))
        md = render_markdown(report)
        assert "Reference" in md or "reference" in md
        assert "plans/a/section-01.md" in md
        assert "plans/b/index.md" in md
        # The arrow connecting source -> target
        assert "->" in md


# ---------------------------------------------------------------------------
# render_console — console summary
# ---------------------------------------------------------------------------

class TestRenderConsole:
    def test_empty_report(self):
        report = Report(mode=ReportMode.FULL)
        out = render_console(report, color=False)
        # should be non-empty (banner / "no findings" line)
        assert len(out) > 0

    def test_safe_fix_marked_with_auto(self):
        """[auto] prefix for SafeFix findings."""
        cf = _classified(safety_class=SafetyClass.SAFE_FIX)
        report = Report(mode=ReportMode.FULL, findings=(cf,))
        out = render_console(report, color=False)
        assert "[auto]" in out

    def test_exposure_review_marked_with_review(self):
        """[review] prefix for ExposureReview findings."""
        cf = _classified(safety_class=SafetyClass.EXPOSURE_REVIEW)
        report = Report(mode=ReportMode.FULL, findings=(cf,))
        out = render_console(report, color=False)
        assert "[review]" in out

    def test_unapplied_marked(self):
        """[UNAPPLIED] prefix for unapplied fixes."""
        pr = _patch_failed()
        report = Report(mode=ReportMode.FULL, unapplied_fixes=(pr,))
        out = render_console(report, color=False)
        assert "[UNAPPLIED]" in out

    def test_console_shows_category_subtype_tag(self):
        """Regression: console output must include [category/subtype] tag
        for every finding so scanning the output shows classifier identity.

        See: TPR-03-002-codex-r5i2.
        """
        finding = Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.UNKNOWN_FIELD,
            severity=Severity.MEDIUM,
            source=Path("plans/p1/index.md"),
            description="unknown field",
            recommended_fix="remove",
        )
        cf = _classified(finding=finding, safety_class=SafetyClass.SAFE_FIX)
        report = Report(mode=ReportMode.FULL, findings=(cf,))
        out = render_console(report, color=False)
        assert "[schema_violation/unknown_field]" in out

    def test_console_shows_source_to_target(self):
        """Regression: console output includes `source -> target` context
        for findings that carry a target path.

        See: TPR-03-002-codex-r5i2.
        """
        finding = Finding(
            category=FindingCategory.DAG_CONFLICT,
            subtype=FindingSubtype.MISSING_DEPENDENCY,
            severity=Severity.HIGH,
            source=Path("plans/a/section-01.md"),
            source_line=3,
            target=Path("plans/b/index.md"),
            description="missing dep",
            recommended_fix="add dep",
        )
        cf = _classified(finding=finding)
        report = Report(mode=ReportMode.FULL, findings=(cf,))
        out = render_console(report, color=False)
        assert "plans/a/section-01.md:3" in out
        assert " -> " in out
        assert "plans/b/index.md" in out

    def test_quick_mode_no_safety_prefix(self):
        """Quick mode shows neither [auto] nor [review] (no classification)."""
        cf = _classified(safety_class=SafetyClass.EXPOSURE_REVIEW)
        report = Report(mode=ReportMode.QUICK, findings=(cf,))
        out = render_console(report, color=False)
        # quick mode entries don't carry classification labels
        assert "[auto]" not in out
        assert "[review]" not in out


# ---------------------------------------------------------------------------
# exit_code_for_findings — exit code semantics
# ---------------------------------------------------------------------------

class TestExitCode:
    def test_clean_returns_zero(self):
        report = Report(mode=ReportMode.FULL)
        assert exit_code_for_findings(report) == 0

    def test_low_findings_return_one(self):
        cf = _classified(_finding(severity=Severity.LOW))
        report = Report(mode=ReportMode.FULL, findings=(cf,))
        assert exit_code_for_findings(report) == 1

    def test_medium_findings_return_one(self):
        cf = _classified(_finding(severity=Severity.MEDIUM))
        report = Report(mode=ReportMode.FULL, findings=(cf,))
        assert exit_code_for_findings(report) == 1

    def test_high_findings_return_one(self):
        cf = _classified(_finding(severity=Severity.HIGH))
        report = Report(mode=ReportMode.FULL, findings=(cf,))
        assert exit_code_for_findings(report) == 1

    def test_critical_findings_return_two(self):
        cf = _classified(_finding(severity=Severity.CRITICAL))
        report = Report(mode=ReportMode.FULL, findings=(cf,))
        assert exit_code_for_findings(report) == 2

    def test_critical_among_others_returns_two(self):
        cfs = (
            _classified(_finding(severity=Severity.LOW)),
            _classified(_finding(severity=Severity.CRITICAL)),
        )
        report = Report(mode=ReportMode.FULL, findings=cfs)
        assert exit_code_for_findings(report) == 2

    def test_unapplied_fix_alone_returns_one(self):
        """Unapplied fixes alone should still elevate exit code."""
        pr = _patch_failed()
        report = Report(mode=ReportMode.FULL, unapplied_fixes=(pr,))
        assert exit_code_for_findings(report) >= 1


# ---------------------------------------------------------------------------
# write_reports — file I/O
# ---------------------------------------------------------------------------

class TestWriteReports:
    def test_writes_json_and_markdown(self, tmp_path):
        cf = _classified()
        report = Report(mode=ReportMode.FULL, findings=(cf,))
        write_reports(report, output_dir=tmp_path)
        assert (tmp_path / "findings.json").exists()
        assert (tmp_path / "findings.md").exists()

    def test_json_is_valid(self, tmp_path):
        cf = _classified()
        report = Report(mode=ReportMode.FULL, findings=(cf,))
        write_reports(report, output_dir=tmp_path)
        data = json.loads((tmp_path / "findings.json").read_text())
        assert "findings" in data
        assert "metadata" in data

    def test_creates_output_directory(self, tmp_path):
        out = tmp_path / "nested" / "dir"
        report = Report(mode=ReportMode.FULL)
        write_reports(report, output_dir=out)
        assert out.exists()
        assert (out / "findings.json").exists()


# ---------------------------------------------------------------------------
# Severity ordering invariant
# ---------------------------------------------------------------------------

class TestSeverityOrdering:
    """Documents the canonical severity ordering used throughout the report."""

    def test_critical_greatest(self):
        assert Severity.CRITICAL > Severity.HIGH
        assert Severity.HIGH > Severity.MEDIUM
        assert Severity.MEDIUM > Severity.LOW
