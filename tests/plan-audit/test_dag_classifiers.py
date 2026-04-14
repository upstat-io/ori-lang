#!/usr/bin/env python3
"""Tests for §02.2 Conflict Classifiers — semantic pins per §02.2 close-out
fixture list.

Each TestClass covers one classifier with at least one positive semantic
pin and (where applicable) a negative pin proving the guard holds.
"""

from __future__ import annotations

import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT))

import pytest

from scripts.plan_corpus import FindingSubtype, FindingCategory, Severity, SourceKind
from scripts.plan_corpus.discovery import discover_corpus
from scripts.plan_corpus.dag import (
    build_dag,
    run_classifiers,
    classify_conflict,
    classify_superseded,
    classify_blocked,
    classify_cross_edge_temporal_drift,
    classify_missing_dependency,
    classify_dead_reference,
    classify_redundant_dependency,
    classify_orphaned_plan,
)


def _write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content)


def _index(name: str, goal: str = "test", status: str = "active",
           reroute: bool = True, supersedes: list[str] | None = None,
           depends_on: list[str] | None = None) -> str:
    deps = ""
    if depends_on:
        items = "\n".join(f'  - "{d}"' for d in depends_on)
        deps = f"depends_on:\n{items}\n"
    sups = ""
    if supersedes is not None:
        items = "\n".join(f'  - "{s}"' for s in supersedes)
        sups = f"supersedes:\n{items}\n" if supersedes else "supersedes: []\n"
    return (
        "---\n"
        f'name: "{name}"\n'
        f'full_name: "{name}"\n'
        f"reroute: {str(reroute).lower()}\n"
        f"status: {status}\n"
        "order: 1\n"
        f'goal: "{goal}"\n'
        f"{sups}"
        f"{deps}"
        "---\n\n# body\n"
    )


def _section(section_id: str, title: str, goal: str = "t",
             status: str = "not-started", body_extra: str = "",
             depends_on: list[str] | None = None) -> str:
    deps = ""
    if depends_on:
        items = "\n".join(f'  - "{d}"' for d in depends_on)
        deps = f"depends_on:\n{items}\n"
    return (
        "---\n"
        f'section: "{section_id}"\n'
        f'title: "{title}"\n'
        f"status: {status}\n"
        "reviewed: false\n"
        f'goal: "{goal}"\n'
        "sections: []\n"
        "third_party_review:\n"
        "  status: none\n"
        "  updated: null\n"
        f"{deps}"
        "---\n\n"
        f"# Section {section_id}\n{body_extra}"
    )


# ---------------------------------------------------------------------------
# CONFLICT
# ---------------------------------------------------------------------------


class TestClassifyConflict:
    def test_shared_subsystem_contradictory_goals_emits_conflict(self, tmp_path: Path):
        plans = tmp_path / "plans"
        # Sections carry both the subsystem mention AND the conflicting goals,
        # because subsystem_to_nodes maps body tokens (on sections).
        _write(plans / "a" / "index.md", _index("A"))
        _write(plans / "a" / "section-01.md",
               _section("01", "a",
                        goal="remove the ori_arc refcount pipeline entirely",
                        body_extra="This plan modifies compiler/ori_arc extensively.\n"))
        _write(plans / "b" / "index.md", _index("B"))
        _write(plans / "b" / "section-01.md",
               _section("01", "b",
                        goal="keep the ori_arc refcount pipeline stable",
                        body_extra="This also touches compiler/ori_arc.\n"))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        findings = classify_conflict(dag, corpus)
        assert any(f.subtype is FindingSubtype.CONFLICT for f in findings), (
            f"expected CONFLICT, got {[f.subtype.value for f in findings]}"
        )

    def test_dependency_edge_short_circuits_conflict(self, tmp_path: Path):
        """Finding G: if A depends_on B, no CONFLICT — it's prerequisite
        coordination."""
        plans = tmp_path / "plans"
        # A depends on B; both touch ori_arc with opposing goals.
        _write(plans / "a" / "index.md",
               _index("A", goal="remove pipeline",
                      depends_on=["B#01"]))
        _write(plans / "a" / "section-01.md",
               _section("01", "a", body_extra="compiler/ori_arc\n"))
        _write(plans / "b" / "index.md", _index("B", goal="keep pipeline"))
        _write(plans / "b" / "section-01.md",
               _section("01", "b", body_extra="compiler/ori_arc\n"))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        findings = classify_conflict(dag, corpus)
        assert not any(f.subtype is FindingSubtype.CONFLICT for f in findings), (
            f"CONFLICT must short-circuit when edges connect the plans"
        )


# ---------------------------------------------------------------------------
# SUPERSEDED
# ---------------------------------------------------------------------------


class TestClassifySupersededReroute:
    """Case (i): reroute plan with non-empty supersedes and no rewrite section."""

    def test_reroute_with_supersedes_and_no_rewrite_section_emits_finding(
        self, tmp_path: Path
    ):
        plans = tmp_path / "plans"
        _write(plans / "rewriter" / "index.md",
               _index("Rewriter", supersedes=["Legacy"]))
        _write(plans / "rewriter" / "section-01.md",
               _section("01", "stuff",
                        body_extra="This is unrelated content.\n"))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        findings = classify_superseded(dag, corpus)
        assert any(f.subtype is FindingSubtype.SUPERSEDED for f in findings)


# ---------------------------------------------------------------------------
# BLOCKED
# ---------------------------------------------------------------------------


class TestClassifyBlocked:
    def test_active_depends_on_not_started_emits_blocked(self, tmp_path: Path):
        plans = tmp_path / "plans"
        _write(plans / "active" / "index.md", _index("Active", status="active"))
        _write(plans / "active" / "section-01.md",
               _section("01", "a", status="in-progress",
                        depends_on=["Prereq#01"]))
        _write(plans / "prereq" / "index.md", _index("Prereq", status="queued"))
        _write(plans / "prereq" / "section-01.md",
               _section("01", "p", status="not-started"))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        findings = classify_blocked(dag, corpus)
        assert any(f.subtype is FindingSubtype.BLOCKED for f in findings), (
            f"expected BLOCKED, got {[(f.subtype.value, f.source, f.target) for f in findings]}"
        )

    def test_both_active_no_blocked(self, tmp_path: Path):
        plans = tmp_path / "plans"
        _write(plans / "a" / "index.md", _index("A", status="active"))
        _write(plans / "a" / "section-01.md",
               _section("01", "a", status="in-progress",
                        depends_on=["B#01"]))
        _write(plans / "b" / "index.md", _index("B", status="active"))
        _write(plans / "b" / "section-01.md",
               _section("01", "b", status="in-progress"))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        findings = classify_blocked(dag, corpus)
        assert not any(f.subtype is FindingSubtype.BLOCKED for f in findings)


# ---------------------------------------------------------------------------
# CROSS_EDGE_TEMPORAL_DRIFT
# ---------------------------------------------------------------------------


class TestClassifyCrossEdgeTemporalDrift:
    def test_complete_dependent_on_not_started_prerequisite_emits_drift(
        self, tmp_path: Path
    ):
        plans = tmp_path / "plans"
        _write(plans / "a" / "index.md", _index("A"))
        _write(plans / "a" / "section-01.md",
               _section("01", "a", status="complete",
                        depends_on=["B#01"]))
        _write(plans / "b" / "index.md", _index("B"))
        _write(plans / "b" / "section-01.md",
               _section("01", "b", status="not-started"))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        findings = classify_cross_edge_temporal_drift(dag, corpus)
        assert any(
            f.subtype is FindingSubtype.CROSS_EDGE_TEMPORAL_DRIFT for f in findings
        )


# ---------------------------------------------------------------------------
# MISSING_DEPENDENCY
# ---------------------------------------------------------------------------


class TestClassifyMissingDependency:
    def test_prose_reference_without_edge_emits_missing_dep(self, tmp_path: Path):
        plans = tmp_path / "plans"
        body = "This section depends on plans/other/ without a declared edge.\n"
        _write(plans / "me" / "index.md", _index("Me"))
        _write(plans / "me" / "section-01.md",
               _section("01", "m", body_extra=body))
        _write(plans / "other" / "index.md", _index("Other"))
        _write(plans / "other" / "section-01.md", _section("01", "o"))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        findings = classify_missing_dependency(dag, corpus)
        assert any(
            f.subtype is FindingSubtype.MISSING_DEPENDENCY for f in findings
        ), f"got {[(f.subtype.value, f.source, f.target) for f in findings]}"


# ---------------------------------------------------------------------------
# DEAD_REFERENCE
# ---------------------------------------------------------------------------


class TestClassifyDeadReference:
    def test_prose_reference_to_missing_plan_emits_dead_ref(self, tmp_path: Path):
        plans = tmp_path / "plans"
        body = "This section depends on plans/nonexistent/ in prose.\n"
        _write(plans / "roadmap" / "section-22-tooling.md",
               _section("22", "tooling", body_extra=body))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        findings = classify_dead_reference(dag, corpus)
        assert any(
            f.subtype is FindingSubtype.PLAN_DIRECTORY_NOT_FOUND for f in findings
        )

    def test_code_fence_reference_is_not_dead_ref(self, tmp_path: Path):
        """Finding D: paths inside code fences must NOT emit DEAD_REFERENCE."""
        plans = tmp_path / "plans"
        body = (
            "Normal prose.\n\n"
            "```\n"
            "Example path: plans/fake_fenced_path/ inside a code fence\n"
            "```\n"
        )
        _write(plans / "bug-tracker" / "index.md",
               _index("BT", reroute=False))
        _write(plans / "bug-tracker" / "00-overview.md",
               "---\n"
               'plan: "bug-tracker"\n'
               'title: "Bugs"\n'
               "status: in-progress\n"
               "sections: []\n"
               "---\n\n" + body)

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        findings = classify_dead_reference(dag, corpus)
        fake = [f for f in findings if "fake_fenced_path" in f.description]
        assert len(fake) == 0

    def test_roadmap_section_shorthand_is_not_dead_reference(self, tmp_path: Path):
        """TPR-02-001-codex r2 regression pin: `plans/roadmap/section-21A`
        PROSE_VERB targets (produced by _ROADMAP_SECTION_RE in §02.1) must
        NOT emit DEAD_REFERENCE just because `roadmap` has no index.md.
        The roadmap directory is a real conventional home."""
        plans = tmp_path / "plans"
        body = "This section will reorder Section 21A subsections.\n"
        _write(plans / "roadmap" / "section-21A-llvm.md",
               _section("21A", "LLVM"))
        _write(plans / "rewriter" / "index.md", _index("Rewriter"))
        _write(plans / "rewriter" / "section-02.md",
               _section("02", "reprior", body_extra=body))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        findings = classify_dead_reference(dag, corpus)
        roadmap_dead_refs = [
            f for f in findings
            if "roadmap" in str(f.source) + str(f.description)
            and "section-21A" in f.description
        ]
        assert len(roadmap_dead_refs) == 0, (
            f"roadmap shorthand must not produce DEAD_REFERENCE; got {roadmap_dead_refs}"
        )

    def test_completed_plan_reference_is_low_severity_dead_ref(self, tmp_path: Path):
        plans = tmp_path / "plans"
        body = "<!-- unblocks:done-plan/01 -->\n"
        _write(plans / "roadmap" / "section-21A.md",
               _section("21A", "llvm", body_extra=body))
        _write(plans / "completed" / "done-plan" / "index.md",
               "---\n"
               'name: "Done"\n'
               'full_name: "Done"\n'
               'status: "resolved"\n'
               "---\n")

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        findings = classify_dead_reference(dag, corpus)
        low_refs = [
            f for f in findings
            if f.severity is Severity.LOW and "done-plan" in f.description
        ]
        assert len(low_refs) >= 1


# ---------------------------------------------------------------------------
# REDUNDANT_DEPENDENCY
# ---------------------------------------------------------------------------


class TestClassifyRedundantDependency:
    def test_depth3_transitive_emits_redundant(self, tmp_path: Path):
        """A -> B -> C + A -> C. The direct A->C is redundant."""
        plans = tmp_path / "plans"
        _write(plans / "a" / "index.md", _index("A"))
        _write(plans / "a" / "section-01.md",
               _section("01", "a", depends_on=["B#01", "C#01"]))
        _write(plans / "b" / "index.md", _index("B"))
        _write(plans / "b" / "section-01.md",
               _section("01", "b", depends_on=["C#01"]))
        _write(plans / "c" / "index.md", _index("C"))
        _write(plans / "c" / "section-01.md", _section("01", "c"))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        findings = classify_redundant_dependency(dag, corpus)
        assert any(
            f.subtype is FindingSubtype.REDUNDANT_DEPENDENCY for f in findings
        )


# ---------------------------------------------------------------------------
# ORPHANED_PLAN
# ---------------------------------------------------------------------------


class TestClassifyOrphanedPlan:
    def test_isolated_plan_emits_orphaned(self, tmp_path: Path):
        plans = tmp_path / "plans"
        _write(plans / "isolated" / "index.md", _index("Isolated", reroute=False))
        _write(plans / "isolated" / "section-01.md", _section("01", "x"))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        findings = classify_orphaned_plan(dag, corpus)
        assert any(f.subtype is FindingSubtype.ORPHANED_PLAN for f in findings)

    def test_plan_with_edge_is_not_orphaned(self, tmp_path: Path):
        plans = tmp_path / "plans"
        _write(plans / "a" / "index.md", _index("A"))
        _write(plans / "a" / "section-01.md",
               _section("01", "a", depends_on=["B#01"]))
        _write(plans / "b" / "index.md", _index("B"))
        _write(plans / "b" / "section-01.md", _section("01", "b"))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        findings = classify_orphaned_plan(dag, corpus)
        assert not any(f.subtype is FindingSubtype.ORPHANED_PLAN for f in findings)


# ---------------------------------------------------------------------------
# Live corpus smoke
# ---------------------------------------------------------------------------


class TestRunClassifiersLiveCorpus:
    def test_run_classifiers_produces_findings_without_exceptions(self):
        """Smoke: every classifier runs cleanly against the live corpus."""
        from scripts.plan_corpus import PLANS_DIR
        corpus = discover_corpus(PLANS_DIR)
        dag = build_dag(corpus)
        findings = run_classifiers(dag, corpus)
        # Don't assert counts — those change as the corpus evolves. Assert
        # only that every finding has a valid category/subtype pair.
        from scripts.plan_corpus.types import _CATEGORY_SUBTYPES
        for f in findings:
            assert f.subtype in _CATEGORY_SUBTYPES[f.category]
