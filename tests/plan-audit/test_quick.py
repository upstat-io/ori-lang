#!/usr/bin/env python3
"""Tests for scripts/verify_roadmap/quick.py — --quick mode runner.

TDD per CLAUDE.md: tests define expected behavior.
Section 03.5 of verify-roadmap-redesign plan.

Coverage:
  - quick mode runs against the real corpus without crashing
  - All findings are ExposureReview (context=None semantic pin)
  - Performance: < 5 seconds on full corpus (no git log calls)
  - Negative pin: no CONFLICT / STATUS_CONTRADICTION / SUPERSEDED /
    MISSING_DEPENDENCY findings (--quick scope explicitly excludes them)
"""

import sys
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT))

import pytest
from scripts.plan_corpus import FindingCategory, FindingSubtype
from scripts.verify_roadmap import SafetyClass, ReportMode
from scripts.verify_roadmap.quick import run_quick


# ---------------------------------------------------------------------------
# --quick mode runs against the real corpus
# ---------------------------------------------------------------------------

class TestRunQuickIntegration:

    def test_runs_on_real_corpus_without_crashing(self):
        report = run_quick(plans_root=REPO_ROOT / "plans")
        assert report is not None
        assert report.mode == ReportMode.QUICK

    def test_quick_mode_findings_all_exposure_review(self):
        """SEMANTIC PIN: --quick passes context=None -> all ExposureReview."""
        report = run_quick(plans_root=REPO_ROOT / "plans")
        for cf in report.findings:
            assert cf.safety_class == SafetyClass.EXPOSURE_REVIEW, (
                f"Quick mode should never produce SafeFix. "
                f"Got {cf.safety_class.name} for {cf.finding.id}"
            )

    def test_quick_mode_no_unapplied_fixes(self):
        """--quick mode never invokes the auto-fix path."""
        report = run_quick(plans_root=REPO_ROOT / "plans")
        assert report.unapplied_fixes == ()

    def test_quick_mode_excludes_unsupported_classifiers(self):
        """NEGATIVE PIN: --quick MUST NOT return findings from excluded classifiers.

        Excluded per §03.5 blind spot #9:
          - CONFLICT
          - SUPERSEDED
          - MISSING_DEPENDENCY (DAG_CONFLICT subtype)
          - STATUS_CONTRADICTION subtypes (any)
        """
        report = run_quick(plans_root=REPO_ROOT / "plans")
        for cf in report.findings:
            sub = cf.finding.subtype
            cat = cf.finding.category
            assert sub != FindingSubtype.CONFLICT, (
                f"--quick must not run CONFLICT classifier (got {cf.finding.id})"
            )
            assert sub != FindingSubtype.SUPERSEDED, (
                f"--quick must not run SUPERSEDED classifier"
            )
            assert sub != FindingSubtype.MISSING_DEPENDENCY, (
                f"--quick must not run MISSING_DEPENDENCY classifier"
            )
            assert cat != FindingCategory.STATUS_CONTRADICTION, (
                f"--quick must not run STATUS_CONTRADICTION classifiers"
            )

    def test_quick_mode_findings_are_blocked_or_dead_ref_or_parse_only(self):
        """Allowed categories in --quick: DAG_CONFLICT (only BLOCKED subtype),
        DEAD_REFERENCE (any subtype), PARSE_ERROR (from load_and_validate)."""
        report = run_quick(plans_root=REPO_ROOT / "plans")
        allowed_categories = {
            FindingCategory.DAG_CONFLICT,
            FindingCategory.DEAD_REFERENCE,
            FindingCategory.PARSE_ERROR,
            FindingCategory.SCHEMA_VIOLATION,  # parse + schema validation findings
            FindingCategory.GAP,  # discovery gaps (missing index.md, etc)
        }
        for cf in report.findings:
            assert cf.finding.category in allowed_categories, (
                f"--quick produced unexpected category "
                f"{cf.finding.category.name} for {cf.finding.id}"
            )


class TestRunQuickPerformance:

    def test_completes_under_5_seconds(self):
        """PERFORMANCE: --quick on the full corpus must finish in < 5s.

        No git log subprocess calls, no shared-subsystem analysis.
        """
        start = time.time()
        run_quick(plans_root=REPO_ROOT / "plans")
        elapsed = time.time() - start
        assert elapsed < 5.0, (
            f"--quick mode took {elapsed:.2f}s; budget is 5.0s. "
            "Check for accidental git/subprocess/shared-subsystem calls."
        )


# ---------------------------------------------------------------------------
# CLI smoke test
# ---------------------------------------------------------------------------

class TestCLI:

    def test_cli_quick_returns_valid_exit_code(self):
        """Smoke: `python -m scripts.verify_roadmap --quick` returns 0/1/2."""
        from scripts.verify_roadmap.__main__ import main
        # Use a tmp dir so we don't pollute build/
        import tempfile
        with tempfile.TemporaryDirectory() as td:
            code = main([
                "--quick",
                "--quiet",
                "--output-dir", td,
            ])
            assert code in (0, 1, 2)
            # JSON + markdown reports written
            assert (Path(td) / "findings.json").exists()
            assert (Path(td) / "findings.md").exists()

    def test_cli_full_returns_2_not_implemented(self):
        """--full mode is not yet implemented; returns 2."""
        from scripts.verify_roadmap.__main__ import main
        import tempfile, io, contextlib
        with tempfile.TemporaryDirectory() as td:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                code = main(["--full", "--output-dir", td])
            assert code == 2
            assert "not yet implemented" in stderr.getvalue().lower()
