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


# ---------------------------------------------------------------------------
# §01.7 — envelope referential closure
# ---------------------------------------------------------------------------


def _section_with_subsections(
    section_id: str,
    title: str,
    subsections: list[dict],
) -> str:
    """Emit a PlanSection frontmatter with a populated `sections:` list."""
    sub_yaml = "\n".join(
        f'  - id: "{s["id"]}"\n'
        f'    title: "{s["title"]}"\n'
        f'    status: {s["status"]}'
        for s in subsections
    )
    return (
        "---\n"
        f'section: "{section_id}"\n'
        f'title: "{title}"\n'
        "status: not-started\n"
        "reviewed: false\n"
        'goal: "test goal"\n'
        "success_criteria: []\n"
        f"sections:\n{sub_yaml}\n"
        "third_party_review:\n"
        "  status: none\n"
        "  updated: null\n"
        "---\n\n"
        f"# Section {section_id}: {title}\n"
    )


class TestExportBugNodes:
    def test_export_emits_bug_nodes_with_full_metadata(self, tmp_path: Path):
        """BugTracker sections produce :Bug nodes carrying full BugEntry metadata.

        Semantic pin: without §01.7, HAS_BUG edges point at bug_id strings
        that never appear as nodes (dangling reference). With §01.7, the
        Bug node is present and carries title/severity/status.
        """
        plans = tmp_path / "plans"
        # Sane mini-plan so the corpus has an anchor.
        _write(plans / "alpha" / "index.md", _index("Alpha"))
        _write(plans / "alpha" / "section-01.md", _section("01", "A"))
        # Bug-tracker section with one BUG entry.
        _write(
            plans / "bug-tracker" / "section-01-demo.md",
            "---\n"
            'section: "01"\n'
            'title: "Demo"\n'
            "status: active\n"
            'goal: "t"\n'
            "sections: []\n"
            "---\n\n"
            "# Section 01: Demo\n\n"
            "## Open Bugs\n\n"
            "- [ ] `[BUG-01-001][high]` **Example bug title** — found by test.\n"
            "  Repro: example repro\n"
            "  Subsystem: demo\n"
            "  Found: 2026-04-17 | Source: user\n",
        )
        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        envelope = export_neo4j_json(corpus, dag)
        bug_nodes = [n for n in envelope["nodes"] if "Bug" in n["labels"]]
        assert len(bug_nodes) == 1
        node = bug_nodes[0]
        assert node["id"] == "BUG-01-001"
        assert node["properties"]["bug_id"] == "BUG-01-001"
        assert node["properties"]["severity"] == "high"
        assert node["properties"]["repo"] == "ori"
        assert "Example bug title" in node["properties"].get("title", "")
        # Negative pin: HAS_BUG.end_id must point at the Bug node — no dangle.
        node_ids = {n["id"] for n in envelope["nodes"]}
        has_bug_edges = [r for r in envelope["relationships"] if r["type"] == "HAS_BUG"]
        assert len(has_bug_edges) == 1
        assert has_bug_edges[0]["end_id"] in node_ids


class TestExportSubsectionNodes:
    def test_export_emits_subsection_nodes_and_has_subsection_edges(
        self, tmp_path: Path
    ):
        """PlanSections with a populated sections: frontmatter list get
        :Subsection nodes + HAS_SUBSECTION edges. Stable id derivation:
        `<section-path>#<sub.id>`."""
        plans = tmp_path / "plans"
        _write(plans / "alpha" / "index.md", _index("Alpha"))
        _write(
            plans / "alpha" / "section-01.md",
            _section_with_subsections(
                "01",
                "A",
                [
                    {"id": "01.1", "title": "first", "status": "complete"},
                    {"id": "01.2", "title": "second", "status": "not-started"},
                ],
            ),
        )
        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        envelope = export_neo4j_json(corpus, dag)
        sub_nodes = [n for n in envelope["nodes"] if "Subsection" in n["labels"]]
        assert len(sub_nodes) == 2
        ids = {n["id"] for n in sub_nodes}
        # Stable id is `<section-path>#<sub-id>`; in-tree paths are repo-relative,
        # test tmp_path falls back to absolute — match on the #<sub-id> suffix.
        assert any(i.endswith("section-01.md#01.1") for i in ids)
        assert any(i.endswith("section-01.md#01.2") for i in ids)
        # Properties carry title and status.
        first_id = next(i for i in ids if i.endswith("#01.1"))
        by_id = {n["id"]: n for n in sub_nodes}
        first = by_id[first_id]
        assert first["properties"]["title"] == "first"
        assert first["properties"]["status"] == "complete"
        assert first["properties"]["subsection_id"] == "01.1"
        # One HAS_SUBSECTION edge per subsection, from parent section to sub.
        has_sub = [r for r in envelope["relationships"] if r["type"] == "HAS_SUBSECTION"]
        assert len(has_sub) == 2
        section_nodes = [n for n in envelope["nodes"] if "PlanSection" in n["labels"]]
        section_id = next(n["id"] for n in section_nodes if n["id"].endswith("section-01.md"))
        for r in has_sub:
            assert r["start_id"] == section_id
            assert r["end_id"] in ids
            assert r["properties"]["structural"] is True


class TestExportCompletedIndexLabel:
    def test_export_uses_completed_index_label(self, tmp_path: Path):
        """Completed-plan index.md files emit `CompletedIndex` nodes, not
        `CompletedPlan`. Negative pin: `CompletedPlan` label must NOT
        appear anywhere in the envelope (would indicate the §02 schema
        drift has returned)."""
        plans = tmp_path / "plans"
        _write(plans / "alpha" / "index.md", _index("Alpha"))
        _write(plans / "alpha" / "section-01.md", _section("01", "A"))
        _write(plans / "completed" / "old-plan" / "index.md", _index("OldPlan", status="complete"))
        _write(plans / "completed" / "old-plan" / "section-01.md", _section("01", "O"))
        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        envelope = export_neo4j_json(corpus, dag)
        all_labels = {lbl for n in envelope["nodes"] for lbl in n["labels"]}
        assert "CompletedIndex" in all_labels
        # Negative pin: the old label is gone.
        assert "CompletedPlan" not in all_labels


class TestExportNeo4jSafeProperties:
    """Neo4j only accepts primitives or arrays-of-primitives as node
    properties. The §01.7 normalizer JSON-encodes nested dicts and
    arrays-of-dicts so the envelope is Neo4j-safe."""

    def test_export_flattens_nested_dict_property_to_json_string(self, tmp_path: Path):
        """A frontmatter dict (e.g., third_party_review) becomes a JSON string."""
        plans = tmp_path / "plans"
        _write(
            plans / "alpha" / "index.md",
            "---\n"
            'name: "Alpha"\n'
            'full_name: "Alpha"\n'
            "reroute: true\n"
            "status: active\n"
            "order: 1\n"
            "third_party_review:\n"
            "  status: none\n"
            "  updated: null\n"
            "---\n\n"
            "# Alpha\n",
        )
        _write(plans / "alpha" / "section-01.md", _section("01", "A"))
        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        envelope = export_neo4j_json(corpus, dag)
        plan_node = next(n for n in envelope["nodes"] if "Plan" in n["labels"])
        tpr = plan_node["properties"].get("third_party_review")
        # Must be a string (JSON-encoded), not a dict.
        assert isinstance(tpr, str), f"expected JSON string, got {type(tpr).__name__}"
        parsed = json.loads(tpr)
        assert parsed == {"status": "none", "updated": None}

    def test_export_flattens_list_of_dicts_to_array_of_json_strings(self, tmp_path: Path):
        """A frontmatter list-of-dicts becomes an array of JSON strings
        (each inner dict JSON-encoded). Neo4j rejects arrays-of-maps,
        but arrays-of-strings are primitive-compatible and richer than a
        single encoded string — consumers can index by position.

        Semantic pin: without the normalization fix, the envelope would
        carry `[{...}, {...}]` and Neo4j's driver raises
        CypherTypeError on MERGE."""
        plans = tmp_path / "plans"
        _write(
            plans / "alpha" / "index.md",
            "---\n"
            'name: "Alpha"\n'
            'full_name: "Alpha"\n'
            "reroute: true\n"
            "status: active\n"
            "order: 1\n"
            "subsections:\n"
            '  - id: "a"\n'
            '    name: "first"\n'
            '  - id: "b"\n'
            '    name: "second"\n'
            "---\n\n"
            "# Alpha\n",
        )
        _write(plans / "alpha" / "section-01.md", _section("01", "A"))
        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        envelope = export_neo4j_json(corpus, dag)
        plan_node = next(n for n in envelope["nodes"] if "Plan" in n["labels"])
        subs = plan_node["properties"].get("subsections")
        assert isinstance(subs, list), f"expected list of strings, got {type(subs).__name__}"
        assert len(subs) == 2
        assert all(isinstance(s, str) for s in subs), f"not all primitives: {subs}"
        # Each element round-trips through json.loads.
        parsed_items = [json.loads(s) for s in subs]
        assert parsed_items[0] == {"id": "a", "name": "first"}
        assert parsed_items[1] == {"id": "b", "name": "second"}

    def test_export_preserves_list_of_primitives_as_array(self, tmp_path: Path):
        """Negative pin for the above — arrays-of-primitives must NOT be
        stringified. Neo4j accepts them as-is, and consumers expect lists."""
        plans = tmp_path / "plans"
        _write(
            plans / "alpha" / "index.md",
            _index("Alpha", depends_on=["Beta"], references=["Gamma", "Delta"]),
        )
        _write(plans / "alpha" / "section-01.md", _section("01", "A"))
        _write(plans / "beta" / "index.md", _index("Beta"))
        _write(plans / "beta" / "section-01.md", _section("01", "B"))
        _write(plans / "gamma" / "index.md", _index("Gamma"))
        _write(plans / "gamma" / "section-01.md", _section("01", "G"))
        _write(plans / "delta" / "index.md", _index("Delta"))
        _write(plans / "delta" / "section-01.md", _section("01", "D"))
        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        envelope = export_neo4j_json(corpus, dag)
        alpha = next(n for n in envelope["nodes"] if n["id"] == "alpha")
        # references: [Gamma, Delta] → remains a list of strings.
        refs = alpha["properties"].get("references")
        if refs is not None:
            assert isinstance(refs, list)
            assert all(isinstance(x, str) for x in refs)


class TestExportPlaceholderFilter:
    def test_export_filters_placeholder_edge_targets(self, tmp_path: Path):
        """Edges whose end_id is an unfilled template placeholder
        (`<source-ref>`, `<target-ref>`, `ID`, `...`, or any `<...>` token)
        are filtered from the envelope."""
        plans = tmp_path / "plans"
        _write(
            plans / "alpha" / "index.md",
            _index(
                "Alpha",
                references=[
                    "<source-ref>",
                    "<target-ref>",
                    "ID",
                    "...",
                    "resolves=<target-ref>",
                ],
            ),
        )
        _write(plans / "alpha" / "section-01.md", _section("01", "A"))
        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        envelope = export_neo4j_json(corpus, dag)
        for r in envelope["relationships"]:
            assert r["end_id"] not in {
                "<source-ref>",
                "<target-ref>",
                "ID",
                "...",
                "resolves=<target-ref>",
            }, f"placeholder leaked: {r}"
            assert "<" not in r["end_id"] or ">" not in r["end_id"], (
                f"angle-bracket placeholder leaked: {r}"
            )
