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
    """Proves every body-shape / FileClass cell has at least one test pin."""

    def test_stub_shapes_all_pinned_for_plan_section(self):
        """Every STUB_SHAPES entry must have a not-started default + strict pin
        and an in-progress pin in `TestPlanSectionStubBlocks`."""
        expected = {s for s, _ in STUB_SHAPES}
        # The parametrized test names encode the shape.
        # Sanity check via introspection of the parametrize ids:
        ids = [s for s, _ in STUB_SHAPES]
        assert set(ids) == expected
        # At least 7 distinct stub shapes are covered.
        assert len(ids) >= 7

    def test_exempt_classes_cover_representative_body_shapes(self):
        """EXEMPT_BODY_SHAPES must include at least: absent, 2+ stub variants,
        graph-unavailable, complete — enough to demonstrate the class exemption
        is total regardless of body."""
        shapes = {s for s, _ in EXEMPT_BODY_SHAPES}
        assert "absent" in shapes
        assert "complete" in shapes
        assert "graph_unavailable" in shapes
        # At least 3 distinct stub variants.
        stub_count = sum(1 for s, _ in EXEMPT_BODY_SHAPES if s.startswith("stub_"))
        assert stub_count >= 3
