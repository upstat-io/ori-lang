#!/usr/bin/env python3
"""Tests for §02.1 DAG Construction — build_dag(corpus) -> Dag.

Uses pytest tmp_path to construct mini-corpora programmatically rather than
committing dozens of tiny fixture .md files — keeps the live corpus pristine
and makes fixtures self-documenting.

Each mini-corpus is a minimal plans/ directory with the specific shape needed
to exercise one code path:
  - fixture_dag_basic: two plans, one depends_on the other -> 1 edge
  - fixture_dag_code_fence: plans/X/ path inside ```...``` -> 0 references
  - fixture_dag_yaml_comment: fix-BUG frontmatter with blocker-in-comment
  - fixture_dag_html_unblocks: roadmap section with <!-- unblocks:x -->
  - fixture_dag_fix_bug_node: fix-BUG file appears as NodeKind.FIX_BUG
  - fixture_dag_completed_index: plans/completed/X/index.md as COMPLETED_INDEX
"""

from __future__ import annotations

import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT))

import pytest

from scripts.plan_corpus import SourceKind, FindingSubtype, FindingCategory
from scripts.plan_corpus.discovery import discover_corpus
from scripts.plan_corpus.dag import (
    Dag,
    NodeId,
    NodeKind,
    build_dag,
    enrich_resolve_dep_finding,
)


# ---------------------------------------------------------------------------
# Mini-corpus builders (pytest tmp_path fixtures)
# ---------------------------------------------------------------------------


def _write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content)


def _basic_index(name: str, depends_on: list[str] | None = None,
                 status: str = "active", reroute: bool = True) -> str:
    deps = ""
    if depends_on is not None:
        items = "\n".join(f'  - "{d}"' for d in depends_on)
        deps = f"depends_on:\n{items}\n"
    return (
        "---\n"
        f'name: "{name}"\n'
        f'full_name: "{name}"\n'
        f"reroute: {str(reroute).lower()}\n"
        f'status: {status}\n'
        "order: 1\n"
        f"{deps}"
        "---\n\n"
        f"# {name}\n"
    )


def _basic_section(section_id: str, title: str, status: str = "not-started",
                   body_extra: str = "", depends_on: list[str] | None = None) -> str:
    deps = ""
    if depends_on is not None:
        items = "\n".join(f'  - "{d}"' for d in depends_on)
        deps = f"depends_on:\n{items}\n"
    return (
        "---\n"
        f'section: "{section_id}"\n'
        f'title: "{title}"\n'
        f"status: {status}\n"
        "reviewed: false\n"
        f'goal: "test goal"\n'
        "sections: []\n"
        "third_party_review:\n"
        "  status: none\n"
        "  updated: null\n"
        f"{deps}"
        "---\n\n"
        f"# Section {section_id}: {title}\n"
        f"{body_extra}"
    )


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


class TestDagConstructionBasic:
    """fixture_dag_basic: two plans, one depends_on the other -> 1 edge."""

    def test_two_plans_with_depends_on_yields_one_edge(self, tmp_path: Path):
        plans = tmp_path / "plans"
        _write(plans / "alpha" / "index.md", _basic_index("Alpha", depends_on=["Beta#01"]))
        _write(plans / "alpha" / "section-01.md", _basic_section("01", "A-sec"))
        _write(plans / "beta" / "index.md", _basic_index("Beta"))
        _write(plans / "beta" / "section-01.md", _basic_section("01", "B-sec"))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)

        # One edge: Alpha/index.md -> Beta/section-01.md
        assert len(dag.edges) == 1
        e = dag.edges[0]
        assert e.source_kind is SourceKind.EXPLICIT_DEPENDS_ON
        assert e.from_node.path == plans / "alpha" / "index.md"
        assert e.to_node.path == plans / "beta" / "section-01.md"


class TestDagConstructionCodeFence:
    """Finding D semantic pin: plans/X/ inside fenced code block emits 0 references."""

    def test_code_fence_path_does_not_produce_reference(self, tmp_path: Path):
        plans = tmp_path / "plans"
        body_with_fence = (
            "Prose references plans/real/ inline.\n\n"
            "```\n"
            "Example: plans/fake_fenced_path/ shown inside a code fence\n"
            "```\n"
        )
        _write(plans / "alpha" / "index.md", _basic_index("Alpha"))
        _write(plans / "alpha" / "section-01.md", _basic_section("01", "A", body_extra=body_with_fence))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)

        # No reference should carry the fenced path.
        fenced_refs = [r for r in dag.references if "fake_fenced_path" in r.target]
        assert len(fenced_refs) == 0, (
            f"Finding D: fenced paths must not produce references; got {fenced_refs}"
        )


class TestDagConstructionYamlComment:
    """Finding F semantic pin for test case (g): fix-BUG frontmatter with
    YAML comment blocker emits a YAML_COMMENT reference."""

    def test_yaml_comment_in_frontmatter_emits_reference(self, tmp_path: Path):
        plans = tmp_path / "plans"
        # Minimal fix-BUG frontmatter with a YAML comment on the status line.
        # Need: plans/bug-tracker/fix-BUG-NN-NNN.md shape
        fix_bug_content = (
            "---\n"
            'bug: "BUG-04-039"\n'
            'title: "test bug"\n'
            "status: in-progress  # blocked by plans/iterator-element-ownership/\n"
            'severity: "high"\n'
            'subsystem: "ori_llvm"\n'
            'source_section: "04.1"\n'
            "sections: []\n"
            "---\n\nbody\n"
        )
        _write(plans / "bug-tracker" / "fix-BUG-04-039.md", fix_bug_content)
        # Also need a bug-tracker section so the directory classifies
        _write(plans / "bug-tracker" / "index.md", _basic_index("Bug Tracker", reroute=False))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)

        yaml_refs = [r for r in dag.references if r.source_kind is SourceKind.YAML_COMMENT]
        assert len(yaml_refs) >= 1
        assert any("iterator-element-ownership" in r.target for r in yaml_refs), (
            f"expected YAML_COMMENT reference with iterator-element-ownership target, "
            f"got {[(r.target, r.source_kind) for r in yaml_refs]}"
        )


class TestDagConstructionHtmlUnblocks:
    """Finding B semantic pin for case (h): <!-- unblocks:X -->."""

    def test_html_unblocks_comment_emits_html_reference(self, tmp_path: Path):
        plans = tmp_path / "plans"
        body = "Body text.\n\n<!-- unblocks:jit-exception-handling/04B,05,06 -->\n"
        _write(plans / "roadmap" / "section-21A-llvm.md", _basic_section("21A", "LLVM", body_extra=body))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)

        html_refs = [r for r in dag.references if r.source_kind is SourceKind.HTML_COMMENT_CONVENTION]
        targets = {r.target for r in html_refs}
        # Three targets from the multi-target unblocks comment.
        assert "jit-exception-handling/04B" in targets
        assert "05" in targets
        assert "06" in targets


class TestDagNodeCoverage:
    """fix-BUG and completed-index files appear as nodes with correct NodeKind."""

    def test_fix_bug_file_appears_as_fix_bug_node(self, tmp_path: Path):
        plans = tmp_path / "plans"
        _write(plans / "bug-tracker" / "fix-BUG-04-001.md",
               "---\n"
               'bug: "BUG-04-001"\n'
               'title: "t"\n'
               "status: in-progress\n"
               'severity: "low"\n'
               'subsystem: "oric"\n'
               'source_section: "04.1"\n'
               "sections: []\n"
               "---\n\n")
        _write(plans / "bug-tracker" / "index.md", _basic_index("BT", reroute=False))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)

        fix_bug_nodes = [n for n in dag.nodes if n.kind is NodeKind.FIX_BUG]
        assert len(fix_bug_nodes) == 1
        assert fix_bug_nodes[0].path.name == "fix-BUG-04-001.md"

    def test_completed_index_appears_as_completed_index_node(self, tmp_path: Path):
        plans = tmp_path / "plans"
        _write(plans / "completed" / "done-plan" / "index.md",
               "---\n"
               'name: "Done"\n'
               'full_name: "Done"\n'
               'status: "resolved"\n'
               "---\n\n")

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)

        ci_nodes = [n for n in dag.nodes if n.kind is NodeKind.COMPLETED_INDEX]
        assert len(ci_nodes) == 1


class TestDagAllSevenKinds:
    def test_every_corpus_bucket_contributes_nodes(self, tmp_path: Path):
        """Exhaustiveness: each of the 7 Corpus buckets populates nodes of
        the corresponding NodeKind."""
        plans = tmp_path / "plans"

        # PLAN_INDEX + PLAN_SECTION + OVERVIEW
        _write(plans / "foo" / "index.md", _basic_index("Foo"))
        _write(plans / "foo" / "section-01.md", _basic_section("01", "F"))
        _write(plans / "foo" / "00-overview.md",
               "---\n"
               'plan: "foo"\n'
               'title: "Foo"\n'
               "status: in-progress\n"
               "sections: []\n"
               "---\n\n")

        # ROADMAP_SECTION
        _write(plans / "roadmap" / "section-01.md", _basic_section("01", "R"))

        # BUG_TRACKER_SECTION + FIX_BUG
        _write(plans / "bug-tracker" / "index.md", _basic_index("BT", reroute=False))
        _write(plans / "bug-tracker" / "section-01.md",
               "---\n"
               'section: "01"\n'
               'title: "BT-sec"\n'
               "status: in-progress\n"
               "sections: []\n"
               "---\n\n")
        _write(plans / "bug-tracker" / "fix-BUG-01-001.md",
               "---\n"
               'bug: "BUG-01-001"\n'
               'title: "b"\n'
               "status: in-progress\n"
               'severity: "low"\n'
               'subsystem: "oric"\n'
               'source_section: "01.1"\n'
               "sections: []\n"
               "---\n\n")

        # COMPLETED_INDEX
        _write(plans / "completed" / "done" / "index.md",
               "---\n"
               'name: "Done"\n'
               'full_name: "Done"\n'
               'status: "resolved"\n'
               "---\n\n")

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)

        kinds_seen = {n.kind for n in dag.nodes}
        expected = {
            NodeKind.PLAN_INDEX, NodeKind.PLAN_SECTION, NodeKind.OVERVIEW,
            NodeKind.ROADMAP_SECTION, NodeKind.BUG_TRACKER_SECTION,
            NodeKind.FIX_BUG, NodeKind.COMPLETED_INDEX,
        }
        assert kinds_seen == expected, f"missing: {expected - kinds_seen}"


class TestDagCycleDetection:
    def test_two_plan_cycle_emits_cycle_finding(self, tmp_path: Path):
        plans = tmp_path / "plans"
        # Section-to-section cycle: a/01 -> b/01 -> a/01
        _write(plans / "a" / "index.md", _basic_index("A"))
        _write(plans / "a" / "section-01.md",
               _basic_section("01", "a-sec", depends_on=["B#01"]))
        _write(plans / "b" / "index.md", _basic_index("B"))
        _write(plans / "b" / "section-01.md",
               _basic_section("01", "b-sec", depends_on=["A#01"]))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)

        cycles = [f for f in dag.findings
                  if f.subtype is FindingSubtype.CYCLE]
        assert len(cycles) >= 1
        c = cycles[0]
        # Chain is typed (Option A), not string-flattened
        assert len(c.dependency_chain) >= 2
        assert c.source_kind is SourceKind.EXPLICIT_DEPENDS_ON


class TestDagSubsystemMap:
    def test_subsystem_to_nodes_populated(self, tmp_path: Path):
        plans = tmp_path / "plans"
        body = "This section touches compiler/ori_arc extensively.\n"
        _write(plans / "foo" / "index.md", _basic_index("Foo"))
        _write(plans / "foo" / "section-01.md",
               _basic_section("01", "F", body_extra=body))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)

        assert "ori_arc" in dag.subsystem_to_nodes
        nodes = dag.subsystem_to_nodes["ori_arc"]
        assert len(nodes) >= 1


class TestDagJsonRoundtrip:
    def test_to_json_is_stable(self, tmp_path: Path):
        plans = tmp_path / "plans"
        _write(plans / "a" / "index.md", _basic_index("A"))
        _write(plans / "a" / "section-01.md", _basic_section("01", "a"))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        j1 = dag.to_json()
        j2 = dag.to_json()
        assert j1 == j2
        assert "nodes" in j1
        assert "edges" in j1


class TestEnrichResolveDepFinding:
    """Finding K: enrich resolve_dep-produced Findings with exact YAML line
    of the offending depends_on entry + evidence=(dep_id,)."""

    def test_enrich_sets_source_and_line(self):
        from scripts.plan_corpus import Finding, FindingCategory, FindingSubtype, Severity

        f = Finding(
            category=FindingCategory.DEAD_REFERENCE,
            subtype=FindingSubtype.CROSS_PLAN_NAME_NOT_FOUND,
            severity=Severity.HIGH,
            source=Path("plans/foo/index.md"),
            description="unknown plan: Bar",
            recommended_fix="fix",
        )
        enriched = enrich_resolve_dep_finding(
            f,
            dep_id="Bar#01",
            yaml_line=17,
            declaring_file=Path("plans/foo/index.md"),
        )
        assert enriched.source_line == 17
        assert "Bar#01" in enriched.evidence
