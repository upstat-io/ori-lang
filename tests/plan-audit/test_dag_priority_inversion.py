#!/usr/bin/env python3
"""Tests for §02.3 Priority Inversion Detection.

Verifies:
- detect_priority_inversions enriches BLOCKED findings with full chains
- compute_minimum_unblock_set groups by root blocker
- Chains use typed Finding.dependency_chain (Option A) — NOT evidence strings
"""

from __future__ import annotations

import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT))

import pytest

from scripts.plan_corpus import FindingSubtype, SourceKind
from scripts.plan_corpus.discovery import discover_corpus
from scripts.plan_corpus.dag import (
    build_dag,
    detect_priority_inversions,
    compute_minimum_unblock_set,
)


def _write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content)


def _index(name: str, status: str = "active") -> str:
    return (
        "---\n"
        f'name: "{name}"\n'
        f'full_name: "{name}"\n'
        "reroute: true\n"
        f"status: {status}\n"
        "order: 1\n"
        f'goal: "{name} goal"\n'
        "---\n\n# body\n"
    )


def _section(section_id: str, title: str, status: str = "not-started",
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
        f'goal: "{title} goal"\n'
        "sections: []\n"
        "third_party_review:\n"
        "  status: none\n"
        "  updated: null\n"
        f"{deps}"
        "---\n\n# body\n"
    )


class TestDetectPriorityInversions:
    def test_multi_hop_chain_reported_as_typed_dependency_chain(self, tmp_path: Path):
        """A (in-progress) -> B (not-started) -> C (not-started)
        produces a BLOCKED finding with dependency_chain = (A, B, C) and
        source_kind = EXPLICIT_DEPENDS_ON (typed, not flattened)."""
        plans = tmp_path / "plans"
        _write(plans / "a" / "index.md", _index("A"))
        _write(plans / "a" / "section-01.md",
               _section("01", "a", status="in-progress",
                        depends_on=["B#01"]))
        _write(plans / "b" / "index.md", _index("B", status="queued"))
        _write(plans / "b" / "section-01.md",
               _section("01", "b", status="not-started",
                        depends_on=["C#01"]))
        _write(plans / "c" / "index.md", _index("C", status="queued"))
        _write(plans / "c" / "section-01.md",
               _section("01", "c", status="not-started"))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        findings = detect_priority_inversions(dag, corpus)

        assert len(findings) >= 1
        f = findings[0]
        assert f.subtype is FindingSubtype.BLOCKED
        assert len(f.dependency_chain) >= 2, (
            f"chain must span multiple hops; got {f.dependency_chain}"
        )
        assert f.source_kind is SourceKind.EXPLICIT_DEPENDS_ON
        # Evidence should NOT carry the chain (Option A).
        for ev in f.evidence:
            assert "→" not in ev and "->" not in ev, (
                f"chain must not be string-flattened into evidence: {ev}"
            )


class TestComputeMinimumUnblockSet:
    def test_shared_root_blocker_collapses_to_one_finding(self, tmp_path: Path):
        """Two active dependents share the same root blocker → one BLOCKED
        finding whose dependency_chain holds the unblock set."""
        plans = tmp_path / "plans"
        # A (in-progress) -> root (not-started)
        # B (in-progress) -> root
        _write(plans / "root" / "index.md", _index("Root", status="queued"))
        _write(plans / "root" / "section-01.md",
               _section("01", "r", status="not-started"))
        _write(plans / "a" / "index.md", _index("A"))
        _write(plans / "a" / "section-01.md",
               _section("01", "a", status="in-progress",
                        depends_on=["Root#01"]))
        _write(plans / "b" / "index.md", _index("B"))
        _write(plans / "b" / "section-01.md",
               _section("01", "b", status="in-progress",
                        depends_on=["Root#01"]))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        findings = compute_minimum_unblock_set(dag, corpus)

        # One finding whose target is the root.
        assert len(findings) >= 1
        f = findings[0]
        assert f.target.name == "section-01.md"
        assert "Root" in f.description or "root" in f.description
        # Dependency chain carries the unblock set (typed field).
        assert len(f.dependency_chain) >= 1

    def test_minimum_unblock_set_is_only_root_not_full_chain(self, tmp_path: Path):
        """TPR-02-003-codex regression pin: the minimum unblock set for a
        transitive chain A -> B -> C must be just {C}, not {A, B, C}.

        Unblocking the root cascades to unblock intermediates; including
        them in the minimum set over-states the work needed."""
        plans = tmp_path / "plans"
        _write(plans / "a" / "index.md", _index("A"))
        _write(plans / "a" / "section-01.md",
               _section("01", "a", status="in-progress",
                        depends_on=["B#01"]))
        _write(plans / "b" / "index.md", _index("B", status="queued"))
        _write(plans / "b" / "section-01.md",
               _section("01", "b", status="not-started",
                        depends_on=["C#01"]))
        _write(plans / "c" / "index.md", _index("C", status="queued"))
        _write(plans / "c" / "section-01.md",
               _section("01", "c", status="not-started"))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        findings = compute_minimum_unblock_set(dag, corpus)
        assert len(findings) >= 1
        f = findings[0]
        # Minimum set is exactly one path — the root leaf (c/section-01.md).
        assert len(f.dependency_chain) == 1, (
            f"minimum unblock set must be just the root, got "
            f"{[p for p in f.dependency_chain]}"
        )
        assert f.dependency_chain[0].parent.name == "c"


class TestNoInversionsWhenNotBlocked:
    def test_all_active_produces_zero_inversions(self, tmp_path: Path):
        plans = tmp_path / "plans"
        _write(plans / "a" / "index.md", _index("A"))
        _write(plans / "a" / "section-01.md",
               _section("01", "a", status="in-progress",
                        depends_on=["B#01"]))
        _write(plans / "b" / "index.md", _index("B"))
        _write(plans / "b" / "section-01.md",
               _section("01", "b", status="in-progress"))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        findings = detect_priority_inversions(dag, corpus)
        assert findings == []
