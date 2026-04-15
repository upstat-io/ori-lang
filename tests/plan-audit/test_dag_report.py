#!/usr/bin/env python3
"""Tests for §02.5 DagReport — §03 handoff contract.

Pin the stable schema: §03 imports DagReport.to_json() and must find
exactly these top-level keys.
"""

from __future__ import annotations

import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT))

from scripts.plan_corpus.discovery import discover_corpus
from scripts.plan_corpus.dag import DagReport, build_dag_report


def _write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content)


def _index(name: str) -> str:
    return (
        "---\n"
        f'name: "{name}"\n'
        f'full_name: "{name}"\n'
        "reroute: true\n"
        "status: active\n"
        "order: 1\n"
        f'goal: "{name}"\n'
        "---\n\n# body\n"
    )


def _section(sec, title, status="not-started", depends_on=None):
    deps = ""
    if depends_on:
        deps = "depends_on:\n" + "\n".join(f'  - "{d}"' for d in depends_on) + "\n"
    return (
        "---\n"
        f'section: "{sec}"\n'
        f'title: "{title}"\n'
        f"status: {status}\n"
        "reviewed: false\n"
        f'goal: "g"\n'
        "sections: []\n"
        "third_party_review:\n"
        "  status: none\n"
        "  updated: null\n"
        f"{deps}"
        "---\n\n# body\n"
    )


class TestDagReportSchema:
    def test_to_json_has_stable_top_level_keys(self, tmp_path: Path):
        plans = tmp_path / "plans"
        _write(plans / "a" / "index.md", _index("A"))
        _write(plans / "a" / "section-01.md",
               _section("01", "a", depends_on=["B#01"]))
        _write(plans / "b" / "index.md", _index("B"))
        _write(plans / "b" / "section-01.md", _section("01", "b"))

        corpus = discover_corpus(root=plans)
        report = build_dag_report(corpus)
        assert isinstance(report, DagReport)
        j = report.to_json()
        assert set(j.keys()) == {
            "findings", "inversions", "unblock_sets", "dag", "stats",
        }

    def test_stats_contract(self, tmp_path: Path):
        plans = tmp_path / "plans"
        _write(plans / "a" / "index.md", _index("A"))
        _write(plans / "a" / "section-01.md", _section("01", "a"))

        corpus = discover_corpus(root=plans)
        report = build_dag_report(corpus)
        required_keys = {
            "nodes", "edges", "references", "subsystems",
            "resolution_findings", "findings_total",
            "inversions", "unblock_sets",
            "by_subtype", "by_source_kind",
        }
        assert required_keys <= set(report.stats.keys())

    def test_idempotent_on_live_corpus(self):
        from scripts.plan_corpus import PLANS_DIR
        corpus = discover_corpus(PLANS_DIR)
        r1 = build_dag_report(corpus)
        r2 = build_dag_report(corpus)
        # Stats + finding IDs are identical across runs
        assert r1.stats == r2.stats
        ids1 = [f.id for f in r1.findings]
        ids2 = [f.id for f in r2.findings]
        assert ids1 == ids2
