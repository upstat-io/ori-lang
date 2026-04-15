#!/usr/bin/env python3
"""§06.2 — body-level `## Intelligence Reconnaissance` block detection tests.

Matrix coverage: (FileClass × body-shape × status × --strict-recon), per the
§06.2 plan. Tests are REPRESENTATIVE, not exhaustive permutation — every
body-shape × FileClass cell has at least one default-mode pin and at least
one `--strict-recon` pin where the modes diverge.

Fixtures are synthesized in-memory via `tmp_path` and helper writers so the
matrix stays inline and readable. The pattern mirrors `TestFindingTypeSafety`
in `test_plan_corpus.py`.

TDD per CLAUDE.md §TDD: these tests fail against the current package shape
(no `Outcome` enum, no body-level validator dispatch, no anti-stub detector);
they pass once §06.2 implementation items 2–9 land.
"""

from __future__ import annotations

import subprocess
import sys
import textwrap
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT))

import pytest

from scripts.plan_corpus import (
    FileClass,
    Finding,
    FindingCategory,
    FindingSubtype,
    Severity,
    classify_file,
    load_and_validate,
)

# `Outcome` is introduced by §06.2 item 3. Fall back to `None` here so the
# test file can still be COLLECTED before implementation lands; individual
# tests call `_require_outcome()` and emit a clear skip/fail when it's
# missing.
try:
    from scripts.plan_corpus import Outcome  # type: ignore[attr-defined]
except ImportError:
    Outcome = None  # type: ignore[assignment,misc]


def _require_outcome() -> Any:
    if Outcome is None:
        pytest.fail(
            "scripts.plan_corpus.Outcome not yet exported — §06.2 item 3 "
            "(Outcome enum on types.py) is pending. Tests-first per CLAUDE.md §TDD."
        )
    return Outcome


# ---------------------------------------------------------------------------
# Body-shape constants
#
# Every body includes a leading blank line then the `## Intelligence
# Reconnaissance` header (or omits it, for the absent shape). Trailing `---\n`
# is included where appropriate so the extractor's "slurp until next `## ` or
# EOF" rule has a well-defined end-of-block.
# ---------------------------------------------------------------------------

BODY_ABSENT = "\n# Section body has no recon block at all\n"

BODY_COMPLETE = textwrap.dedent("""\

    ## Intelligence Reconnaissance

    Queries run 2026-04-15:

    - `scripts/intel-query.sh status` — graph available
    - `scripts/intel-query.sh --human search "plan validation" --limit 5`

    Summary (≤500 chars) [ori]: Package under scripts/plan_corpus/
    validates plan frontmatter. No cross-repo equivalents directly relevant.

    ---
""")

BODY_STUB_EMPTY = textwrap.dedent("""\

    ## Intelligence Reconnaissance



    ---
""")

BODY_STUB_PLACEHOLDER = textwrap.dedent("""\

    ## Intelligence Reconnaissance

    TBD

    ---
""")

BODY_STUB_NO_QUERY = textwrap.dedent("""\

    ## Intelligence Reconnaissance

    Looked at the code on 2026-04-15. Results summary [ori]: the package
    looks fine — no specific issues found.

    ---
""")

BODY_STUB_NO_DATE = textwrap.dedent("""\

    ## Intelligence Reconnaissance

    Ran `scripts/intel-query.sh --human search "foo"`. Results [ori]: works
    as expected, nothing to flag.

    ---
""")

BODY_STUB_NO_CITATION = textwrap.dedent("""\

    ## Intelligence Reconnaissance

    Ran `scripts/intel-query.sh --human search "foo"` on 2026-04-15. Works
    as expected, nothing to flag.

    ---
""")

BODY_MIXED_PLACEHOLDER = textwrap.dedent("""\

    ## Intelligence Reconnaissance

    Queries run 2026-04-15:
    - `scripts/intel-query.sh --human search "foo"`

    [ori] TBD — will fill in later

    ---
""")

BODY_REPO_PATH_CITATION = textwrap.dedent("""\

    ## Intelligence Reconnaissance

    Queries run 2026-04-15:

    - `scripts/intel-query.sh --human similar "compile_module" --repo rust`

    Summary [rust:compiler/rustc_errors/src/lib.rs]: Rust's diagnostic
    reporter shares the no-op failure propagation pattern. Direct
    equivalent.

    ---
""")

BODY_ONLY_AT_INCLUDE = textwrap.dedent("""\

    ## Intelligence Reconnaissance

    @.claude/skills/dual-tpr/compose-intel-summary.md

    ---
""")

BODY_GRAPH_UNAVAILABLE = textwrap.dedent("""\

    ## Intelligence Reconnaissance

    Graph was unavailable at 2026-04-15 when this section was authored.
    Retrofit when the graph comes back up.

    ---
""")

BODY_GRAPH_UNAVAILABLE_INTELLIGENCE = textwrap.dedent("""\

    ## Intelligence Reconnaissance

    Intelligence graph unavailable on 2026-04-15 — recon deferred; will
    revisit when graph is restored.

    ---
""")


# ---------------------------------------------------------------------------
# Fixture writers (synthesize minimal valid frontmatter per file_class)
# ---------------------------------------------------------------------------


def _plan_section_fm(status: str) -> str:
    return textwrap.dedent(f"""\
        ---
        section: "01"
        title: "Test section"
        status: {status}
        reviewed: true
        goal: "Test goal"
        success_criteria: []
        sections:
          - id: "01.1"
            title: "Sub"
            status: {status}
        third_party_review:
          status: none
          updated: null
        ---""") + "\n"


def _roadmap_section_fm(status: str) -> str:
    return textwrap.dedent(f"""\
        ---
        section: "01"
        title: "Test roadmap section"
        status: {status}
        reviewed: true
        goal: "Test goal"
        sections:
          - id: "01.1"
            title: "Sub"
            status: {status}
        ---""") + "\n"


def _bug_section_fm(status: str) -> str:
    return textwrap.dedent(f"""\
        ---
        section: "01"
        title: "Test bug section"
        status: {status}
        goal: "Test goal"
        ---""") + "\n"


def _fix_bug_fm(status: str) -> str:
    return textwrap.dedent(f"""\
        ---
        bug: "BUG-01-001"
        title: "Test bug"
        severity: "medium"
        status: {status}
        goal: "Test goal"
        success_criteria: []
        subsystem: "test"
        found: "2026-04-15"
        source: "test"
        third_party_review:
          status: none
          updated: null
        ---""") + "\n"


def write_plan_section(
    tmp_path: Path, body: str, *, status: str = "not-started",
    plan_dir: str = "testplan", name: str = "section-01-test.md",
) -> Path:
    """Create a PLAN_SECTION file under tmp_path/plans/<plan_dir>/<name>."""
    d = tmp_path / "plans" / plan_dir
    d.mkdir(parents=True, exist_ok=True)
    fp = d / name
    fp.write_text(_plan_section_fm(status) + body)
    return fp


def write_roadmap_section(
    tmp_path: Path, body: str, *, status: str = "not-started",
    name: str = "section-01-test.md",
) -> Path:
    """Create a ROADMAP_SECTION file under tmp_path/plans/roadmap/<name>."""
    d = tmp_path / "plans" / "roadmap"
    d.mkdir(parents=True, exist_ok=True)
    fp = d / name
    fp.write_text(_roadmap_section_fm(status) + body)
    return fp


def write_bug_section(
    tmp_path: Path, body: str, *, status: str = "not-started",
    name: str = "section-01-test.md",
) -> Path:
    """Create a BUG_TRACKER_SECTION file under tmp_path/plans/bug-tracker/<name>."""
    d = tmp_path / "plans" / "bug-tracker"
    d.mkdir(parents=True, exist_ok=True)
    fp = d / name
    fp.write_text(_bug_section_fm(status) + body)
    return fp


def write_fix_bug(
    tmp_path: Path, body: str, *, status: str = "not-started",
    name: str = "fix-BUG-01-001.md",
) -> Path:
    """Create a FIX_BUG file under tmp_path/plans/bug-tracker/<name>."""
    d = tmp_path / "plans" / "bug-tracker"
    d.mkdir(parents=True, exist_ok=True)
    fp = d / name
    fp.write_text(_fix_bug_fm(status) + body)
    return fp


# ---------------------------------------------------------------------------
# Helpers for finding inspection
# ---------------------------------------------------------------------------


def _recon_findings(violations: list[Finding]) -> list[Finding]:
    """Filter violations to only recon-block-related findings."""
    recon_subtypes = {
        getattr(FindingSubtype, name, None)
        for name in ("MISSING_RECON_BLOCK", "VALIDATION_BYPASS", "RECON_GRAPH_UNAVAILABLE")
    }
    recon_subtypes.discard(None)
    return [f for f in violations if f.subtype in recon_subtypes]


def _validate(path: Path, *, strict_recon: bool = False) -> list[Finding]:
    """Run load_and_validate with strict_recon and return recon-block findings only.

    Uses a keyword argument that §06.2 item 4/6 will add. When not yet
    implemented, falls back to the base signature; tests that specifically
    require strict escalation will naturally diverge.
    """
    try:
        result = load_and_validate(path, strict_recon=strict_recon)  # type: ignore[call-arg]
    except TypeError:
        # Pre-impl fallback — signature still takes only `path`.
        result = load_and_validate(path)
    assert result.is_ok, f"load_and_validate returned err: {result.err}"
    assert result.ok is not None
    return _recon_findings(result.ok.violations)


# ---------------------------------------------------------------------------
# PLAN_SECTION matrix — happy paths
# ---------------------------------------------------------------------------


class TestPlanSectionCompleteBlockPasses:
    """Complete blocks produce zero recon findings regardless of status."""

    def test_complete_block_not_started_zero_findings(self, tmp_path):
        p = write_plan_section(tmp_path, BODY_COMPLETE, status="not-started")
        assert _validate(p) == []

    def test_complete_block_in_progress_zero_findings(self, tmp_path):
        p = write_plan_section(tmp_path, BODY_COMPLETE, status="in-progress")
        assert _validate(p) == []

    def test_complete_block_complete_zero_findings(self, tmp_path):
        p = write_plan_section(tmp_path, BODY_COMPLETE, status="complete")
        assert _validate(p) == []

    def test_repo_path_citation_accepted_as_valid(self, tmp_path):
        """`[rust:compiler/rustc_errors/src/lib.rs]` is a valid citation per Step D."""
        p = write_plan_section(tmp_path, BODY_REPO_PATH_CITATION, status="not-started")
        assert _validate(p) == []


# ---------------------------------------------------------------------------
# PLAN_SECTION matrix — absent block
# ---------------------------------------------------------------------------


class TestPlanSectionAbsentBlock:
    """Missing recon block — status-gated severity; complete is exempt."""

    def test_absent_not_started_high_warning(self, tmp_path):
        p = write_plan_section(tmp_path, BODY_ABSENT, status="not-started")
        findings = _validate(p)
        assert len(findings) == 1
        f = findings[0]
        assert f.category == FindingCategory.GAP
        assert f.subtype == FindingSubtype.MISSING_RECON_BLOCK
        assert f.severity == Severity.HIGH
        Outcome_ = _require_outcome()
        assert f.outcome == Outcome_.WARNING

    def test_absent_not_started_strict_recon_high_error(self, tmp_path):
        p = write_plan_section(tmp_path, BODY_ABSENT, status="not-started")
        Outcome_ = _require_outcome()
        findings = _validate(p, strict_recon=True)
        assert len(findings) == 1
        f = findings[0]
        assert f.severity == Severity.HIGH
        assert f.outcome == Outcome_.ERROR

    def test_absent_in_progress_medium_warning(self, tmp_path):
        p = write_plan_section(tmp_path, BODY_ABSENT, status="in-progress")
        Outcome_ = _require_outcome()
        findings = _validate(p)
        assert len(findings) == 1
        f = findings[0]
        assert f.severity == Severity.MEDIUM
        assert f.outcome == Outcome_.WARNING

    def test_absent_in_progress_strict_recon_still_warning(self, tmp_path):
        """--strict-recon escalates ONLY not-started; in-progress unaffected."""
        p = write_plan_section(tmp_path, BODY_ABSENT, status="in-progress")
        Outcome_ = _require_outcome()
        findings = _validate(p, strict_recon=True)
        assert len(findings) == 1
        assert findings[0].severity == Severity.MEDIUM
        assert findings[0].outcome == Outcome_.WARNING

    def test_absent_complete_exempt_zero_findings(self, tmp_path):
        p = write_plan_section(tmp_path, BODY_ABSENT, status="complete")
        assert _validate(p) == []

    def test_absent_complete_strict_still_exempt(self, tmp_path):
        """--strict-recon does NOT override the complete exemption."""
        p = write_plan_section(tmp_path, BODY_ABSENT, status="complete")
        assert _validate(p, strict_recon=True) == []


# ---------------------------------------------------------------------------
# PLAN_SECTION matrix — stub / performative-ritual blocks
# ---------------------------------------------------------------------------


STUB_SHAPES = [
    ("stub_empty", BODY_STUB_EMPTY),
    ("stub_placeholder", BODY_STUB_PLACEHOLDER),
    ("stub_no_query", BODY_STUB_NO_QUERY),
    ("stub_no_date", BODY_STUB_NO_DATE),
    ("stub_no_citation", BODY_STUB_NO_CITATION),
    ("mixed_placeholder", BODY_MIXED_PLACEHOLDER),
    ("only_at_include", BODY_ONLY_AT_INCLUDE),
]


class TestPlanSectionStubBlocks:
    """Stub bodies trigger VALIDATION_BYPASS with status-gated severity."""

    @pytest.mark.parametrize("shape,body", STUB_SHAPES)
    def test_stub_not_started_high_warning_default(self, tmp_path, shape, body):
        p = write_plan_section(tmp_path, body, status="not-started")
        Outcome_ = _require_outcome()
        findings = _validate(p)
        assert len(findings) == 1, (
            f"shape={shape}: expected exactly 1 finding, got {len(findings)}"
        )
        f = findings[0]
        assert f.category == FindingCategory.GAP
        assert f.subtype == FindingSubtype.VALIDATION_BYPASS
        assert f.severity == Severity.HIGH
        assert f.outcome == Outcome_.WARNING

    @pytest.mark.parametrize("shape,body", STUB_SHAPES)
    def test_stub_not_started_strict_high_error(self, tmp_path, shape, body):
        """--strict-recon upgrades stub findings on not-started to ERROR."""
        p = write_plan_section(tmp_path, body, status="not-started")
        Outcome_ = _require_outcome()
        findings = _validate(p, strict_recon=True)
        assert len(findings) == 1, f"shape={shape}"
        assert findings[0].severity == Severity.HIGH
        assert findings[0].outcome == Outcome_.ERROR

    @pytest.mark.parametrize("shape,body", STUB_SHAPES)
    def test_stub_in_progress_medium_warning(self, tmp_path, shape, body):
        p = write_plan_section(tmp_path, body, status="in-progress")
        Outcome_ = _require_outcome()
        findings = _validate(p)
        assert len(findings) == 1, f"shape={shape}"
        assert findings[0].severity == Severity.MEDIUM
        assert findings[0].outcome == Outcome_.WARNING

    def test_mixed_placeholder_message_names_the_violation(self, tmp_path):
        """The finding's description must mention 'mixed-placeholder' or similar
        so consumers know WHICH concrete-content check failed."""
        p = write_plan_section(tmp_path, BODY_MIXED_PLACEHOLDER, status="not-started")
        findings = _validate(p)
        assert len(findings) == 1
        desc = findings[0].description.lower()
        assert any(tok in desc for tok in ("mixed", "placeholder", "tbd")), (
            f"description should name the violation; got: {findings[0].description!r}"
        )


# ---------------------------------------------------------------------------
# PLAN_SECTION matrix — graph-unavailable documentation
# ---------------------------------------------------------------------------


class TestPlanSectionGraphUnavailable:
    """Graph-unavailable docs are distinct from stubs; Severity.LOW, non-gating."""

    def test_graph_unavailable_not_started_low_warning(self, tmp_path):
        p = write_plan_section(tmp_path, BODY_GRAPH_UNAVAILABLE, status="not-started")
        Outcome_ = _require_outcome()
        findings = _validate(p)
        assert len(findings) == 1
        f = findings[0]
        assert f.category == FindingCategory.GAP
        assert f.subtype == FindingSubtype.RECON_GRAPH_UNAVAILABLE
        assert f.severity == Severity.LOW
        assert f.outcome == Outcome_.WARNING

    def test_graph_unavailable_strict_recon_not_escalated(self, tmp_path):
        """--strict-recon does NOT escalate RECON_GRAPH_UNAVAILABLE."""
        p = write_plan_section(tmp_path, BODY_GRAPH_UNAVAILABLE, status="not-started")
        Outcome_ = _require_outcome()
        findings = _validate(p, strict_recon=True)
        assert len(findings) == 1
        assert findings[0].severity == Severity.LOW
        assert findings[0].outcome == Outcome_.WARNING

    def test_graph_unavailable_intelligence_phrase_also_accepted(self, tmp_path):
        p = write_plan_section(
            tmp_path, BODY_GRAPH_UNAVAILABLE_INTELLIGENCE, status="in-progress",
        )
        findings = _validate(p)
        assert len(findings) == 1
        assert findings[0].subtype == FindingSubtype.RECON_GRAPH_UNAVAILABLE


# ---------------------------------------------------------------------------
# Exempt classes — ROADMAP_SECTION / BUG_TRACKER_SECTION / FIX_BUG
# ---------------------------------------------------------------------------


EXEMPT_BODY_SHAPES = [
    ("absent", BODY_ABSENT),
    ("stub_empty", BODY_STUB_EMPTY),
    ("stub_placeholder", BODY_STUB_PLACEHOLDER),
    ("stub_no_citation", BODY_STUB_NO_CITATION),
    ("stub_no_query", BODY_STUB_NO_QUERY),
    ("stub_no_date", BODY_STUB_NO_DATE),
    ("graph_unavailable", BODY_GRAPH_UNAVAILABLE),
    ("complete", BODY_COMPLETE),
]


class TestRoadmapSectionAlwaysExempt:
    """ROADMAP_SECTION produces zero recon findings for ANY body shape."""

    @pytest.mark.parametrize("shape,body", EXEMPT_BODY_SHAPES)
    def test_roadmap_section_no_recon_findings_default(self, tmp_path, shape, body):
        p = write_roadmap_section(tmp_path, body, status="not-started")
        assert classify_file(p) == FileClass.ROADMAP_SECTION
        assert _validate(p) == [], f"shape={shape} leaked a recon finding"

    @pytest.mark.parametrize("shape,body", EXEMPT_BODY_SHAPES[:3])
    def test_roadmap_section_no_recon_findings_strict(self, tmp_path, shape, body):
        """--strict-recon does NOT affect exempt classes."""
        p = write_roadmap_section(tmp_path, body, status="not-started")
        assert _validate(p, strict_recon=True) == [], (
            f"shape={shape} leaked a recon finding under --strict-recon"
        )


class TestBugTrackerSectionAlwaysExempt:
    """BUG_TRACKER_SECTION produces zero recon findings."""

    @pytest.mark.parametrize("shape,body", EXEMPT_BODY_SHAPES)
    def test_bug_section_no_recon_findings_default(self, tmp_path, shape, body):
        p = write_bug_section(tmp_path, body, status="not-started")
        assert classify_file(p) == FileClass.BUG_TRACKER_SECTION
        assert _validate(p) == [], f"shape={shape} leaked a recon finding"

    @pytest.mark.parametrize("shape,body", EXEMPT_BODY_SHAPES[:3])
    def test_bug_section_no_recon_findings_strict(self, tmp_path, shape, body):
        p = write_bug_section(tmp_path, body, status="not-started")
        assert _validate(p, strict_recon=True) == []


class TestFixBugAlwaysExempt:
    """FIX_BUG produces zero recon findings."""

    @pytest.mark.parametrize("shape,body", EXEMPT_BODY_SHAPES)
    def test_fix_bug_no_recon_findings_default(self, tmp_path, shape, body):
        p = write_fix_bug(tmp_path, body, status="not-started")
        assert classify_file(p) == FileClass.FIX_BUG
        assert _validate(p) == [], f"shape={shape} leaked a recon finding"

    @pytest.mark.parametrize("shape,body", EXEMPT_BODY_SHAPES[:3])
    def test_fix_bug_no_recon_findings_strict(self, tmp_path, shape, body):
        p = write_fix_bug(tmp_path, body, status="not-started")
        assert _validate(p, strict_recon=True) == []


# ---------------------------------------------------------------------------
# Outcome enum + Finding.outcome field shape
# ---------------------------------------------------------------------------


class TestOutcomeEnumShape:

    def test_outcome_enum_has_warning_and_error_members(self):
        Outcome_ = _require_outcome()
        assert hasattr(Outcome_, "WARNING")
        assert hasattr(Outcome_, "ERROR")
        assert Outcome_.WARNING != Outcome_.ERROR

    def test_outcome_values_are_stable_strings(self):
        Outcome_ = _require_outcome()
        assert Outcome_.WARNING.value == "warning"
        assert Outcome_.ERROR.value == "error"

    def test_finding_outcome_defaults_to_error_for_backcompat(self):
        """Existing Finding(...) call sites that don't set outcome must
        still gate CI (exit 1) — default=ERROR preserves existing behavior."""
        Outcome_ = _require_outcome()
        f = Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.UNKNOWN_FIELD,
            severity=Severity.HIGH,
            source=Path("test.md"),
            description="test",
            recommended_fix="fix",
        )
        assert f.outcome == Outcome_.ERROR

    def test_finding_outcome_is_independent_of_severity(self):
        """Severity and Outcome are orthogonal — a LOW + ERROR is legal, and
        a HIGH + WARNING is legal. Emitters set them independently."""
        Outcome_ = _require_outcome()
        f1 = Finding(
            category=FindingCategory.GAP,
            subtype=FindingSubtype.MISSING_INDEX_MD,
            severity=Severity.LOW,
            source=Path("test.md"),
            description="low+error",
            recommended_fix="fix",
            outcome=Outcome_.ERROR,  # type: ignore[call-arg]
        )
        assert f1.severity == Severity.LOW
        assert f1.outcome == Outcome_.ERROR

    def test_finding_to_json_includes_outcome(self):
        Outcome_ = _require_outcome()
        f = Finding(
            category=FindingCategory.GAP,
            subtype=FindingSubtype.MISSING_INDEX_MD,
            severity=Severity.HIGH,
            source=Path("test.md"),
            description="test",
            recommended_fix="fix",
            outcome=Outcome_.WARNING,  # type: ignore[call-arg]
        )
        payload = f.to_json()
        assert "outcome" in payload
        assert payload["outcome"] == "warning"

    def test_finding_outcome_does_not_change_id_hash(self):
        """Adding outcome as a field MUST NOT change existing Finding ids.
        Backward-compat with pre-§06.2 saved reports."""
        common = dict(
            category=FindingCategory.PARSE_ERROR,
            subtype=FindingSubtype.YAML_SYNTAX_ERROR,
            severity=Severity.HIGH,
            source=Path("test.md"),
            source_line=5,
            description="test",
            recommended_fix="fix",
        )
        Outcome_ = _require_outcome()
        f_err = Finding(**common, outcome=Outcome_.ERROR)  # type: ignore[call-arg]
        f_warn = Finding(**common, outcome=Outcome_.WARNING)  # type: ignore[call-arg]
        # Same id — id hash does not depend on outcome.
        assert f_err.id == f_warn.id


class TestFindingSubtypeRegistration:

    def test_missing_recon_block_registered_under_gap(self):
        assert hasattr(FindingSubtype, "MISSING_RECON_BLOCK")
        from scripts.plan_corpus import _CATEGORY_SUBTYPES
        assert FindingSubtype.MISSING_RECON_BLOCK in _CATEGORY_SUBTYPES[FindingCategory.GAP]

    def test_validation_bypass_registered_under_gap(self):
        assert hasattr(FindingSubtype, "VALIDATION_BYPASS")
        from scripts.plan_corpus import _CATEGORY_SUBTYPES
        assert FindingSubtype.VALIDATION_BYPASS in _CATEGORY_SUBTYPES[FindingCategory.GAP]

    def test_recon_graph_unavailable_registered_under_gap(self):
        assert hasattr(FindingSubtype, "RECON_GRAPH_UNAVAILABLE")
        from scripts.plan_corpus import _CATEGORY_SUBTYPES
        assert FindingSubtype.RECON_GRAPH_UNAVAILABLE in _CATEGORY_SUBTYPES[FindingCategory.GAP]


# ---------------------------------------------------------------------------
# Exit-code policy via subprocess
# ---------------------------------------------------------------------------


def _run_check(path: Path, *extra_args: str) -> subprocess.CompletedProcess[str]:
    """Invoke `python -m scripts.plan_corpus check <path> [args]` from REPO_ROOT."""
    return subprocess.run(
        [sys.executable, "-m", "scripts.plan_corpus", "check", str(path), *extra_args],
        cwd=str(REPO_ROOT),
        capture_output=True,
        text=True,
        timeout=60,
    )


class TestExitCodePolicy:
    """Exit 0 unless some finding has Outcome.ERROR; warnings are printed but non-gating."""

    def test_exit_0_when_only_warnings(self, tmp_path):
        """A not-started section with missing recon produces a WARNING only
        (default mode). Exit code must be 0."""
        _require_outcome()
        p = write_plan_section(tmp_path, BODY_ABSENT, status="not-started")
        result = _run_check(p)
        assert result.returncode == 0, (
            f"Expected exit 0 on warning-only, got {result.returncode}\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
        # Finding should still be printed.
        assert "missing_recon_block" in result.stdout.lower() or "recon" in result.stdout.lower()

    def test_exit_1_with_strict_recon_on_not_started_missing(self, tmp_path):
        """--strict-recon escalates a not-started missing-recon to ERROR, gating CI."""
        _require_outcome()
        p = write_plan_section(tmp_path, BODY_ABSENT, status="not-started")
        result = _run_check(p, "--strict-recon")
        assert result.returncode == 1, (
            f"Expected exit 1 under --strict-recon, got {result.returncode}\n"
            f"stdout:\n{result.stdout}"
        )

    def test_exit_0_with_strict_recon_on_in_progress_missing(self, tmp_path):
        """--strict-recon does NOT escalate in-progress findings."""
        _require_outcome()
        p = write_plan_section(tmp_path, BODY_ABSENT, status="in-progress")
        result = _run_check(p, "--strict-recon")
        assert result.returncode == 0, (
            f"Expected exit 0 (in-progress not escalated), got {result.returncode}\n"
            f"stdout:\n{result.stdout}"
        )

    def test_exit_1_on_schema_violation_regardless_of_recon(self, tmp_path):
        """Schema violations (missing required field) are Outcome.ERROR by
        default — existing behavior preserved. Verifies the new default
        doesn't silently demote schema errors to warnings."""
        _require_outcome()
        # Write a file missing the required `goal` field.
        d = tmp_path / "plans" / "testplan"
        d.mkdir(parents=True)
        fp = d / "section-01-test.md"
        fp.write_text(textwrap.dedent("""\
            ---
            section: "01"
            title: "Test"
            status: not-started
            reviewed: true
            success_criteria: []
            sections: []
            third_party_review:
              status: none
              updated: null
            ---
        """) + BODY_COMPLETE)
        result = _run_check(fp)
        assert result.returncode == 1, (
            f"Schema violations must gate CI by default, got exit {result.returncode}"
        )

    def test_check_help_mentions_strict_recon(self):
        """--help text must advertise the new flag."""
        result = subprocess.run(
            [sys.executable, "-m", "scripts.plan_corpus", "check", "--help"],
            cwd=str(REPO_ROOT),
            capture_output=True,
            text=True,
            timeout=30,
        )
        assert result.returncode == 0
        assert "--strict-recon" in result.stdout


# ---------------------------------------------------------------------------
# `discover` recon-coverage reporter
# ---------------------------------------------------------------------------


class TestDiscoverReconCoverageReporter:
    """`discover` subcommand reports per-plan recon presence grouped by status.

    Runs against the real repo corpus — the reporter's job is to produce a
    status-grouped table that §09 can consume for retrofit progress. We
    assert on the table's shape and one well-known plan entry, not on
    exact numbers (corpus contents evolve).
    """

    def test_discover_emits_recon_coverage_table(self):
        """Running `discover` emits a per-plan recon-coverage table."""
        _require_outcome()
        result = subprocess.run(
            [sys.executable, "-m", "scripts.plan_corpus", "discover"],
            cwd=str(REPO_ROOT),
            capture_output=True,
            text=True,
            timeout=120,
        )
        # Don't assert on returncode — discover emits gap warnings but is
        # informational, not gating.
        out = result.stdout.lower()
        assert "recon coverage" in out, (
            f"Discover output missing 'recon coverage' header.\n"
            f"stdout tail:\n{result.stdout[-2000:]}"
        )

    def test_discover_table_includes_known_plan(self):
        """The table should include `plans/query-intel-adoption/` (this plan)
        as a line with status-grouped counts."""
        _require_outcome()
        result = subprocess.run(
            [sys.executable, "-m", "scripts.plan_corpus", "discover"],
            cwd=str(REPO_ROOT),
            capture_output=True,
            text=True,
            timeout=120,
        )
        # The plan this §06.2 test lives in must show up with at least one
        # section (this one) counted.
        assert "query-intel-adoption" in result.stdout, (
            f"Discover output missing the query-intel-adoption plan line.\n"
            f"stdout tail:\n{result.stdout[-2000:]}"
        )


# ---------------------------------------------------------------------------
# Self-verifying matrix completeness (from tests.md)
# ---------------------------------------------------------------------------


class TestMatrixCompleteness:
    """Proves every body-shape / FileClass cell has at least one test pin.

    Per tests.md §Self-Verifying Matrix Completeness: a matrix loop that
    silently skips cells is worse than no matrix. These tests drive the
    expected cell counts from explicit cell tables and cross-check them
    against the actual parametrize data + collected test names. If someone
    removes a stub variant from `STUB_SHAPES` or a class from the exempt
    list without updating the parametrized tests, this catches it.
    """

    # Explicit cell table — NOT derived from STUB_SHAPES so drift is
    # visible. Each entry is `(shape_name, default_strict_divergence)` —
    # divergence=True means default and strict emit different Outcomes.
    _EXPECTED_STUB_SHAPES = [
        ("stub_empty", True),
        ("stub_placeholder", True),
        ("stub_no_query", True),
        ("stub_no_date", True),
        ("stub_no_citation", True),
        ("mixed_placeholder", True),
        ("only_at_include", True),
    ]

    def test_stub_shape_set_matches_expected(self):
        """The stub shapes covered by parametrize MUST equal the expected
        explicit table, exactly — no silent additions or removals."""
        expected_names = {n for n, _ in self._EXPECTED_STUB_SHAPES}
        actual_names = {s for s, _ in STUB_SHAPES}
        assert actual_names == expected_names, (
            f"STUB_SHAPES drift: expected {expected_names}, "
            f"got {actual_names}"
        )

    def test_stub_plan_section_cell_count_matches_expected(self):
        """Count of parametrized invocations in TestPlanSectionStubBlocks
        must equal len(STUB_SHAPES) * 3 test functions (default,
        strict, in-progress). A missing cell = a missing regression guard."""
        expected_cells = len(STUB_SHAPES) * 3
        # Count by inspecting the class's parametrized functions directly.
        import inspect
        cls = TestPlanSectionStubBlocks
        stub_test_methods = [
            m for name, m in inspect.getmembers(cls, predicate=inspect.isfunction)
            if name.startswith("test_stub_")
            and hasattr(m, "pytestmark")
        ]
        total_cells = 0
        for m in stub_test_methods:
            for mark in m.pytestmark:
                if mark.name == "parametrize":
                    # parametrize("shape,body", STUB_SHAPES) — the second arg
                    # is the data
                    total_cells += len(mark.args[1])
        assert total_cells == expected_cells, (
            f"TestPlanSectionStubBlocks cell count {total_cells} != "
            f"expected {expected_cells} (len(STUB_SHAPES)={len(STUB_SHAPES)} × 3 methods)"
        )

    def test_exempt_classes_cover_representative_body_shapes(self):
        """EXEMPT_BODY_SHAPES must include: absent, 3+ stub variants,
        graph-unavailable, complete — enough to demonstrate the class
        exemption is total regardless of body."""
        shapes = {s for s, _ in EXEMPT_BODY_SHAPES}
        assert "absent" in shapes
        assert "complete" in shapes
        assert "graph_unavailable" in shapes
        # At least 3 distinct stub variants.
        stub_count = sum(1 for s, _ in EXEMPT_BODY_SHAPES if s.startswith("stub_"))
        assert stub_count >= 3


# ---------------------------------------------------------------------------
# Round-1 TPR regression tests (codex findings TPR-06-001..003)
# ---------------------------------------------------------------------------


class TestNaturalNoneAcceptedAfterCitation:
    """Regression pin for TPR-06-001-codex: mixed-placeholder detection must
    NOT false-positive on natural-English `None`/`n/a` usage after a citation."""

    def test_ori_none_of_callers_prose_passes(self, tmp_path):
        """`[ori] None of the callers of validate live outside...` is a
        legitimate summary — `None` is a common English subject."""
        body = textwrap.dedent("""\

            ## Intelligence Reconnaissance

            Queries run 2026-04-15:

            - `scripts/intel-query.sh --human callers "validate" --repo ori`

            [ori] None of the callers of validate live outside the package.
            Verified by grepping scripts/ for the function.

            ---
        """)
        p = write_plan_section(tmp_path, body, status="not-started")
        assert _validate(p) == [], (
            "Natural-English 'None of the callers' must not trigger "
            "mixed-placeholder-after-citation (TPR-06-001-codex)"
        )

    def test_ori_n_a_in_natural_prose_passes(self, tmp_path):
        """Similar regression guard for natural `N/A` usage in prose."""
        body = textwrap.dedent("""\

            ## Intelligence Reconnaissance

            Queries run 2026-04-15:

            - `scripts/intel-query.sh --human similar "foo" --repo rust`

            [ori] Applied to foo; cross-repo N/A because there is no rust
            equivalent. Confirmed by reading the lowering pass.

            ---
        """)
        p = write_plan_section(tmp_path, body, status="not-started")
        assert _validate(p) == [], (
            "Natural-English 'N/A' in prose must not trigger "
            "mixed-placeholder-after-citation (TPR-06-001-codex)"
        )

    def test_strict_stub_tokens_still_detected(self, tmp_path):
        """Negative pin: `[ori] TBD` still fires — strict tokens are not
        exempt. This is the ORIGINAL mixed-placeholder case the validator
        was designed to catch."""
        body = BODY_MIXED_PLACEHOLDER  # `[ori] TBD — will fill in later`
        p = write_plan_section(tmp_path, body, status="not-started")
        findings = _validate(p)
        assert len(findings) == 1
        assert findings[0].subtype == FindingSubtype.VALIDATION_BYPASS


class TestRepoCitationGrammarStrict:
    """Regression pin for TPR-06-002-codex: citation regex must enforce
    Step D grammar exactly — `[repo#N]` requires integer N, `[repo:path]`
    requires non-empty path."""

    def test_malformed_issue_citation_rejected(self, tmp_path):
        """`[rust#abc]` is NOT a valid Step D issue citation — issue IDs
        are integers. A block with only this malformed citation must be
        flagged as missing-citation, not accepted."""
        body = textwrap.dedent("""\

            ## Intelligence Reconnaissance

            Queries run 2026-04-15:

            - `scripts/intel-query.sh --human search "foo" --repo rust`

            Summary [rust#abc]: Some prose. (Malformed — issue ID not numeric.)

            ---
        """)
        p = write_plan_section(tmp_path, body, status="not-started")
        findings = _validate(p)
        assert len(findings) == 1, (
            "[rust#abc] is not a valid Step D citation — block should "
            "be flagged VALIDATION_BYPASS for missing citation "
            "(TPR-06-002-codex)"
        )
        assert findings[0].subtype == FindingSubtype.VALIDATION_BYPASS
        assert "citation" in findings[0].description.lower()

    def test_empty_issue_citation_rejected(self, tmp_path):
        """`[rust#]` (empty number) is malformed; must not count as citation."""
        body = textwrap.dedent("""\

            ## Intelligence Reconnaissance

            Queries run 2026-04-15:

            - `scripts/intel-query.sh --human search "foo" --repo rust`

            Summary [rust#]: Bare hash, no issue id. Malformed.

            ---
        """)
        p = write_plan_section(tmp_path, body, status="not-started")
        findings = _validate(p)
        assert len(findings) == 1
        assert findings[0].subtype == FindingSubtype.VALIDATION_BYPASS

    def test_empty_path_citation_rejected(self, tmp_path):
        """`[rust:]` (empty path) is malformed; must not count as citation."""
        body = textwrap.dedent("""\

            ## Intelligence Reconnaissance

            Queries run 2026-04-15:

            - `scripts/intel-query.sh --human search "foo" --repo rust`

            Summary [rust:] with empty symbol path. Malformed.

            ---
        """)
        p = write_plan_section(tmp_path, body, status="not-started")
        findings = _validate(p)
        assert len(findings) == 1
        assert findings[0].subtype == FindingSubtype.VALIDATION_BYPASS

    def test_valid_numeric_issue_citation_accepted(self, tmp_path):
        """Positive pin: `[rust#12345]` is the canonical issue citation."""
        body = textwrap.dedent("""\

            ## Intelligence Reconnaissance

            Queries run 2026-04-15:

            - `scripts/intel-query.sh --human search "lifetime" --repo rust --limit 5`

            [rust#12345] Reference implementation — short phrase. Verified.

            ---
        """)
        p = write_plan_section(tmp_path, body, status="not-started")
        assert _validate(p) == []


class TestGraphUnavailablePrecedenceNarrowed:
    """Regression pin for TPR-06-003-codex: graph-unavailable must only fire
    when the block is SUBSTITUTING for full recon (no query), not when the
    phrase happens to appear in natural prose inside a complete block."""

    def test_complete_block_mentioning_phrase_still_complete(self, tmp_path):
        """A complete block (query + date + citation) that happens to use
        the phrase 'graph unavailable' in natural prose must NOT be
        downgraded to RECON_GRAPH_UNAVAILABLE."""
        body = textwrap.dedent("""\

            ## Intelligence Reconnaissance

            Queries run 2026-04-15:

            - `scripts/intel-query.sh --human search "retrofit" --limit 5`

            [ori] This section discusses how graph unavailable notes should
            be handled when authors run recon during a Neo4j outage.
            Verified behavior at scripts/plan_corpus/schema.py.

            ---
        """)
        p = write_plan_section(tmp_path, body, status="not-started")
        assert _validate(p) == [], (
            "Complete block mentioning 'graph unavailable' in prose must "
            "NOT be downgraded — it has real query + date + citation "
            "(TPR-06-003-codex)"
        )

    def test_genuine_graph_unavailable_still_detected(self, tmp_path):
        """Negative pin: a block with NO query AND the unavailability
        phrase still counts as RECON_GRAPH_UNAVAILABLE — the narrowing
        must not break the legitimate case."""
        body = BODY_GRAPH_UNAVAILABLE  # date + phrase, no query
        p = write_plan_section(tmp_path, body, status="not-started")
        findings = _validate(p)
        assert len(findings) == 1
        assert findings[0].subtype == FindingSubtype.RECON_GRAPH_UNAVAILABLE


# ---------------------------------------------------------------------------
# Extractor edge cases (TPR-06-004-codex):
# `_extract_recon_block` is load-bearing — test HTML comments, next-header
# truncation, multi-header bodies
# ---------------------------------------------------------------------------


class TestExtractorEdgeCases:
    """Regression coverage for `_extract_recon_block` — HTML comments
    stripped, next `## ` section truncates, multi-header keeps first only."""

    def test_html_comments_stripped_before_content_checks(self, tmp_path):
        """An HTML comment containing stub-like content must NOT influence
        the content check — the extractor strips comments before
        evaluation."""
        body = textwrap.dedent("""\

            ## Intelligence Reconnaissance

            <!-- TODO: this note is not part of the content per the
            extractor contract — comments are metadata. -->

            Queries run 2026-04-15:

            - `scripts/intel-query.sh --human search "foo"`

            [ori] Real summary text after the comment.

            ---
        """)
        p = write_plan_section(tmp_path, body, status="not-started")
        # If the comment was not stripped, `TODO` would trigger
        # mixed-placeholder or whole-body-placeholder detection. With the
        # stripping, the block passes.
        assert _validate(p) == []

    def test_next_section_header_truncates_block(self, tmp_path):
        """A `## NN.1` header after the recon block must terminate the
        extractor — content in the next subsection is NOT part of the
        recon block."""
        body = textwrap.dedent("""\

            ## Intelligence Reconnaissance

            Queries run 2026-04-15:

            - `scripts/intel-query.sh --human search "foo"`

            [ori] Real summary.

            ## 01.1 First Subsection

            TBD — this placeholder is in 01.1, not the recon block.
            If the extractor failed to truncate at the next `## `, the
            recon block would falsely include this TBD and trigger
            mixed-placeholder-after-citation.

            ---
        """)
        p = write_plan_section(tmp_path, body, status="not-started")
        assert _validate(p) == []

    def test_only_first_recon_header_used(self, tmp_path):
        """If the body contains two `## Intelligence Reconnaissance`
        headers (e.g., authoring mistake), the extractor takes the FIRST
        one. The validator then operates on the content between the first
        header and the next `## ` — which is the second header. The second
        header's content is NOT part of the block."""
        body = textwrap.dedent("""\

            ## Intelligence Reconnaissance

            Queries run 2026-04-15:

            - `scripts/intel-query.sh --human search "foo"`

            [ori] First block is complete and valid.

            ## Intelligence Reconnaissance

            TBD — this second block's content should NOT be seen by the
            validator because the first `## ` boundary terminated the
            extractor. If it were seen, mixed-placeholder would fire.

            ---
        """)
        p = write_plan_section(tmp_path, body, status="not-started")
        # First block is complete — no finding.
        assert _validate(p) == []


# ---------------------------------------------------------------------------
# Docgen byte-parity regression (TPR-06-008-codex)
# ---------------------------------------------------------------------------


class TestDocgenByteParityWithShellRedirect:
    """Regression pin: `python -m scripts.plan_corpus docgen > file` must
    produce bytes identical to `generate_schema_reference()`. Before the
    `print(ref, end="")` fix, shell redirect appended an extra newline and
    the next `docgen --check` run flagged self-drift."""

    def test_shell_redirect_bytes_match_generator_output(self, tmp_path):
        """Redirect docgen into a tmp file; compare bytes to the function's
        return value."""
        from scripts.plan_corpus.docgen import generate_schema_reference
        expected = generate_schema_reference()
        redirect_target = tmp_path / "out.md"
        result = subprocess.run(
            [sys.executable, "-m", "scripts.plan_corpus", "docgen"],
            cwd=str(REPO_ROOT),
            capture_output=True,
            text=True,
            timeout=30,
        )
        assert result.returncode == 0
        # The `print(ref, end="")` fix ensures stdout == expected bytes.
        redirect_target.write_text(result.stdout)
        actual = redirect_target.read_text()
        assert actual == expected, (
            f"docgen shell redirect drifted from generate_schema_reference(). "
            f"expected {len(expected)} chars; got {len(actual)} chars. "
            f"tail diff: expected ends with {expected[-20:]!r}, "
            f"got {actual[-20:]!r}"
        )

    def test_docgen_check_immediately_after_regeneration_passes(self, tmp_path):
        """End-to-end: regenerate via `docgen > file`, then run `docgen
        --check`. Must pass. Before the fix, this sequence failed."""
        gen = subprocess.run(
            [sys.executable, "-m", "scripts.plan_corpus", "docgen"],
            cwd=str(REPO_ROOT),
            capture_output=True,
            text=True,
            timeout=30,
        )
        assert gen.returncode == 0
        # Write via the exact shell-redirect pattern.
        target = REPO_ROOT / "docs" / "internal" / "plan-schema-reference.md"
        original = target.read_text()
        try:
            target.write_text(gen.stdout)
            check = subprocess.run(
                [sys.executable, "-m", "scripts.plan_corpus", "docgen", "--check"],
                cwd=str(REPO_ROOT),
                capture_output=True,
                text=True,
                timeout=30,
            )
            assert check.returncode == 0, (
                f"docgen --check failed after regeneration: {check.stdout}"
            )
        finally:
            # Restore the original committed file so this test is side-effect-free.
            target.write_text(original)
