#!/usr/bin/env python3
"""Tests for §01.4 — export_neo4j_json(corpus, dag) serialization.

Fixture-corpus round-trip: the exporter reads `Corpus + Dag` and emits a
deterministic Neo4j-flavored JSON envelope. These tests pin the envelope
shape, determinism, and the SourceKind → relationship-type mapping that
§02's importer will consume.

Pattern mirrors tests/plan-audit/test_dag_construction.py — programmatic
mini-corpora via pytest tmp_path rather than committed fixture files.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT))

import pytest

from scripts.plan_corpus import SourceKind
from scripts.plan_corpus.dag import build_dag
from scripts.plan_corpus.discovery import discover_corpus
from scripts.plan_corpus.export_json import SCHEMA_VERSION, export_neo4j_json


# ---------------------------------------------------------------------------
# Mini-corpus builders (shared shape with test_dag_construction.py)
# ---------------------------------------------------------------------------


def _write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content)


def _index(
    name: str,
    *,
    depends_on: list[str] | None = None,
    supersedes: list[str] | None = None,
    references: list[str] | None = None,
    status: str = "active",
    reroute: bool = True,
) -> str:
    extras = ""
    if depends_on is not None:
        items = "\n".join(f'  - "{d}"' for d in depends_on)
        extras += f"depends_on:\n{items}\n"
    if supersedes is not None:
        items = "\n".join(f'  - "{d}"' for d in supersedes)
        extras += f"supersedes:\n{items}\n"
    if references is not None:
        items = "\n".join(f'  - "{d}"' for d in references)
        extras += f"references:\n{items}\n"
    return (
        "---\n"
        f'name: "{name}"\n'
        f'full_name: "{name}"\n'
        f"reroute: {str(reroute).lower()}\n"
        f"status: {status}\n"
        "order: 1\n"
        f"{extras}"
        "---\n\n"
        f"# {name}\n"
    )


def _section(
    section_id: str,
    title: str,
    *,
    status: str = "not-started",
    depends_on: list[str] | None = None,
    touches: list[str] | None = None,
) -> str:
    deps = ""
    if depends_on is not None:
        items = "\n".join(f'  - "{d}"' for d in depends_on)
        deps = f"depends_on:\n{items}\n"
    touches_block = ""
    if touches is not None:
        items = "\n".join(f'  - "{t}"' for t in touches)
        touches_block = f"touches:\n{items}\n"
    return (
        "---\n"
        f'section: "{section_id}"\n'
        f'title: "{title}"\n'
        f"status: {status}\n"
        "reviewed: false\n"
        f'goal: "test goal"\n'
        "success_criteria: []\n"
        "sections: []\n"
        "third_party_review:\n"
        "  status: none\n"
        "  updated: null\n"
        f"{deps}"
        f"{touches_block}"
        "---\n\n"
        f"# Section {section_id}: {title}\n"
    )


def _build(tmp_path: Path) -> tuple:
    """Build the Corpus + Dag + envelope for the most common fixture."""
    plans = tmp_path / "plans"
    _write(plans / "alpha" / "index.md", _index("Alpha", depends_on=["Beta#01"]))
    _write(plans / "alpha" / "section-01.md", _section("01", "A-sec"))
    _write(plans / "beta" / "index.md", _index("Beta"))
    _write(plans / "beta" / "section-01.md", _section("01", "B-sec"))
    corpus = discover_corpus(root=plans)
    dag = build_dag(corpus)
    envelope = export_neo4j_json(corpus, dag)
    return corpus, dag, envelope


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


class TestExportEnvelopeShape:
    def test_export_fixture_corpus_produces_expected_node_count(self, tmp_path: Path):
        """Two plans, each with one index + one section → 4 nodes emitted."""
        corpus, dag, envelope = _build(tmp_path)
        # 2 plans × (1 index + 1 section) = 4 nodes.
        assert len(envelope["nodes"]) == 4
        # Exactly one DEPENDS_ON edge (alpha.index → beta.section-01).
        rel_types = [r["type"] for r in envelope["relationships"]]
        assert "DEPENDS_ON" in rel_types
        assert rel_types.count("DEPENDS_ON") == 1

    def test_export_envelope_schema_has_required_keys(self, tmp_path: Path):
        """Top-level, node, and relationship schemas are all pinned."""
        _, _, envelope = _build(tmp_path)
        # Top-level schema.
        assert set(envelope.keys()) == {
            "schema_version",
            "generated_at",
            "nodes",
            "relationships",
        }
        assert envelope["schema_version"] == SCHEMA_VERSION
        # Node schema.
        for node in envelope["nodes"]:
            assert set(node.keys()) >= {"id", "labels", "properties"}
            assert isinstance(node["labels"], list) and len(node["labels"]) == 1
            assert isinstance(node["properties"], dict)
        # Relationship schema — every edge-backed rel carries source_kind /
        # source_line / raw_text / mention_kind; structural rels carry only
        # `structural: True`. Assert the union schema.
        for rel in envelope["relationships"]:
            assert set(rel.keys()) >= {"type", "start_id", "end_id", "properties"}
            props = rel["properties"]
            if "source_kind" in props:
                assert "raw_text" in props
                assert "mention_kind" in props
                assert props["mention_kind"] in {"declared", "inferred"}


class TestExportDeterminism:
    def test_export_same_corpus_twice_produces_byte_identical_json(
        self, tmp_path: Path
    ):
        """Determinism pin: same input → byte-identical JSON output.

        `generated_at` carries a second-resolution UTC timestamp, so we
        compare everything EXCEPT that field. Within-call ordering is
        what matters for §02 importer idempotence — the timestamp is
        cosmetic provenance, not load-bearing.
        """
        corpus, dag, _ = _build(tmp_path)
        env1 = export_neo4j_json(corpus, dag)
        env2 = export_neo4j_json(corpus, dag)
        for env in (env1, env2):
            env.pop("generated_at", None)
        out1 = json.dumps(env1, sort_keys=True)
        out2 = json.dumps(env2, sort_keys=True)
        assert out1 == out2


class TestExportSupersedesEdge:
    def test_supersedes_frontmatter_produces_supersedes_relationship(
        self, tmp_path: Path
    ):
        """`supersedes:` on an index's frontmatter must become a SUPERSEDES
        relationship in the envelope (edge-backed — source_kind populated)."""
        plans = tmp_path / "plans"
        _write(plans / "old" / "index.md", _index("Old", supersedes=["New"]))
        _write(plans / "old" / "section-01.md", _section("01", "O"))
        _write(plans / "new" / "index.md", _index("New"))
        _write(plans / "new" / "section-01.md", _section("01", "N"))
        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        envelope = export_neo4j_json(corpus, dag)
        supersedes = [
            r for r in envelope["relationships"] if r["type"] == "SUPERSEDES"
        ]
        assert len(supersedes) >= 1, (
            f"SUPERSEDES relationship missing; relationships = "
            f"{[r['type'] for r in envelope['relationships']]}"
        )
        # Edge-backed SUPERSEDES must carry source_kind + raw_text provenance.
        assert supersedes[0]["properties"]["source_kind"] == "explicit_supersedes"
        assert supersedes[0]["properties"]["mention_kind"] == "declared"


class TestExportReferencesRelationshipNotEdge:
    def test_references_frontmatter_produces_references_relationship_and_no_edge(
        self, tmp_path: Path
    ):
        """`references:` on an index's frontmatter must emit a REFERENCES
        relationship in the envelope but MUST NOT appear in dag.edges —
        references-only kinds feed dag.references only per §01.3b."""
        plans = tmp_path / "plans"
        _write(plans / "foo" / "index.md", _index("Foo", references=["Bar"]))
        _write(plans / "foo" / "section-01.md", _section("01", "F"))
        _write(plans / "bar" / "index.md", _index("Bar"))
        _write(plans / "bar" / "section-01.md", _section("01", "B"))
        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        envelope = export_neo4j_json(corpus, dag)
        # No EXPLICIT_REFERENCES edge — the invariant this test pins.
        explicit_ref_edges = [
            e for e in dag.edges
            if e.source_kind is SourceKind.EXPLICIT_REFERENCES
        ]
        assert explicit_ref_edges == []
        # But the envelope DOES carry a REFERENCES relationship sourced
        # from dag.references.
        refs = [
            r for r in envelope["relationships"]
            if r["type"] == "REFERENCES"
            and r["properties"].get("source_kind") == "explicit_references"
        ]
        assert len(refs) >= 1, (
            f"REFERENCES relationship missing despite references: [bar]; "
            f"all rels = "
            f"{[(r['type'], r['properties'].get('source_kind')) for r in envelope['relationships']]}"
        )
