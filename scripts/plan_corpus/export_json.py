"""Neo4j-flavored JSON exporter for the plan corpus + DAG.

§01.4 deliverable — serializes `Corpus + Dag` to a deterministic envelope
that §02's Neo4j importer consumes. Architecturally:

  * `dag.py` is the DAG SSOT — frontmatter relationship semantics are
    modeled there. This module is a thin serialization adapter that reads
    `Corpus` for node properties and `Dag` for edges + references.
  * No frontmatter re-parsing happens here. If a new relationship kind is
    needed, it is added to `dag.py` first; this module picks it up via
    the `SourceKind → rel_type` table.
  * Determinism is the contract: same input corpus produces byte-identical
    JSON output across runs. Nodes sort by `id`; relationships sort by
    `(start_id, type, end_id)`; property dicts are emitted with sorted
    keys at serialization time.
  * `MENTIONS_CODE` edges (plan → CodeReference → Symbol) are deferred to
    §02.3 where `resolve_code_refs.py` is available. This module emits a
    `touches_raw` property on each node carrying the raw `touches:` list;
    §02.3 reads it without re-parsing frontmatter.
"""

from __future__ import annotations

import datetime as _dt
import json as _json
import re
from pathlib import Path
from typing import Any

from .bug_markers import parse_bug_entries
from .types import REPO_ROOT, SourceKind
from .dag import Dag, NodeId, NodeKind


SCHEMA_VERSION = "1.0"


# ---------------------------------------------------------------------------
# Node label + stable-ID tables
# ---------------------------------------------------------------------------


_NODE_LABEL: dict[NodeKind, str] = {
    NodeKind.PLAN_INDEX:          "Plan",
    NodeKind.PLAN_SECTION:        "PlanSection",
    NodeKind.ROADMAP_SECTION:     "RoadmapSection",
    NodeKind.OVERVIEW:            "Overview",
    NodeKind.BUG_TRACKER_SECTION: "BugTrackerSection",
    NodeKind.FIX_BUG:             "FixSection",
    NodeKind.COMPLETED_INDEX:     "CompletedIndex",
}


# Template placeholder tokens that leak into edge end_ids when plan authors
# fill out `<!-- blocked-by:<target-ref> -->` scaffolds without replacing the
# placeholder. Emitted edges pointing at these become permanently-dangling
# references in Neo4j; filter them at export time so §02's importer does not
# have to carry the filter list.
_PLACEHOLDER_TARGETS: frozenset[str] = frozenset({
    "<source-ref>",
    "<target-ref>",
    "ID",
    "...",
    "resolves=<target-ref>",
})


def _is_placeholder_target(end_id: str) -> bool:
    """Return True if `end_id` is an unfilled template placeholder, not a
    real node reference. Matches the exact `_PLACEHOLDER_TARGETS` tokens OR
    any string containing an angle-bracket fragment (the unfilled-scaffold
    signature)."""
    if end_id in _PLACEHOLDER_TARGETS:
        return True
    if "<" in end_id and ">" in end_id:
        return True
    return False


def _node_label(node_kind: NodeKind) -> str:
    """Neo4j label for a NodeKind. Single dict lookup — drift-free."""
    return _NODE_LABEL[node_kind]


def _rel_path(p: Path) -> str:
    """Render an absolute Path as repo-relative POSIX string. Never ~ or ///.

    Non-REPO_ROOT-relative paths (shouldn't occur in normal discovery)
    round-trip as POSIX absolute strings so the envelope stays valid JSON.
    """
    try:
        return p.resolve().relative_to(REPO_ROOT.resolve()).as_posix()
    except ValueError:
        return p.as_posix()


def _stable_id(node: NodeId, corpus) -> str:
    """Deterministic string ID used as Neo4j MERGE key. Derived from path
    per the §01.4 "Node stable ID mapping" table. FIX_BUG uses its file
    stem (`fix-BUG-XX-NNN`) — this must stay distinct from the Bug
    node's `bug_id` (`BUG-XX-NNN`), otherwise Phase 2's cross-label
    MERGE `MATCH (:PlanBugNode {id: ...})` matches both nodes and
    multiplies FIXED_BY / HAS_BUG edge counts."""
    if node.kind is NodeKind.FIX_BUG:
        return node.path.stem
    if node.kind is NodeKind.PLAN_INDEX:
        return node.path.parent.name
    if node.kind is NodeKind.COMPLETED_INDEX:
        return node.path.parent.name
    return _rel_path(node.path)


# ---------------------------------------------------------------------------
# SourceKind → relationship-type mapping
# ---------------------------------------------------------------------------


# Static table for kinds whose rel_type doesn't depend on raw_text content.
_STATIC_REL_TYPE: dict[SourceKind, str] = {
    SourceKind.EXPLICIT_DEPENDS_ON: "DEPENDS_ON",
    SourceKind.EXPLICIT_SUPERSEDES: "SUPERSEDES",
    SourceKind.EXPLICIT_REFERENCES: "REFERENCES",
    SourceKind.PROSE_VERB:          "REFERENCES",
    SourceKind.YAML_COMMENT:        "REFERENCES",
}

# HTML_COMMENT_CONVENTION verbs → rel_type. Matches the seven verbs the
# `_HTML_COMMENT_RE` scanner in dag.py emits; extend both tables together
# if a new verb is added. Missing verb falls back to REFERENCES.
_HTML_VERB_REL_TYPE: dict[str, str] = {
    "blocked-by":       "BLOCKED_BY",
    "unblocks":         "UNBLOCKS",
    "supersedes":       "SUPERSEDES",
    "resolves":         "RESOLVES",
    "rewrites":         "REWRITES",
    "update-complete":  "UPDATE_COMPLETE",
    "updated-by":       "UPDATED_BY",
}

_HTML_VERB_RE = re.compile(r"<!--\s*([A-Za-z-]+)\s*:")


def _source_kind_to_rel_type(source_kind: SourceKind, raw_text: str) -> str:
    """Map a reference's SourceKind + raw_text to an uppercase Neo4j
    relationship type. HTML_COMMENT_CONVENTION verbs are extracted from
    raw_text; every other kind has a static mapping."""
    if source_kind is SourceKind.HTML_COMMENT_CONVENTION:
        m = _HTML_VERB_RE.search(raw_text or "")
        if m:
            verb = m.group(1).lower()
            return _HTML_VERB_REL_TYPE.get(verb, "REFERENCES")
        return "REFERENCES"
    return _STATIC_REL_TYPE.get(source_kind, "REFERENCES")


# ---------------------------------------------------------------------------
# Node property extraction
# ---------------------------------------------------------------------------


# Keys that never travel into the envelope even when present in frontmatter.
# `sections:` is structural — lowered to HAS_SUBSECTION edges elsewhere;
# including it as a property would duplicate information.
_EXCLUDED_FRONTMATTER_KEYS: frozenset[str] = frozenset({"sections"})


def _normalize_property_value(value: Any) -> Any:
    """Render a frontmatter value into a Neo4j-safe shape.

    Neo4j property values may only be primitives or arrays of primitives
    (strings, numbers, booleans) — nested maps and arrays-of-maps are
    rejected at write time. This function flattens non-primitive
    containers into JSON strings so the raw structure is preserved but
    the value fits Neo4j's property model.

    * `None` stays None (filtered by caller before emission).
    * Scalars pass through.
    * Path → POSIX string.
    * Lists-of-primitives pass through (after recursive normalization).
    * Lists containing any non-primitive → JSON-encoded string.
    * Dicts → JSON-encoded string (sorted keys, stable output).
    * Anything else → str() fallback.
    """
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, Path):
        return value.as_posix()
    if isinstance(value, list):
        norm = [_normalize_property_value(v) for v in value]
        # Neo4j rejects collections containing null — strip them. The
        # serialized envelope is lossy for `[a, null, b]`, but null in
        # a list almost always indicates an absent frontmatter value
        # that the consumer should ignore.
        norm = [v for v in norm if v is not None]
        # Arrays of primitives (Neo4j-compatible) pass through as-is.
        if all(
            isinstance(v, (bool, int, float, str))
            for v in norm
        ):
            return norm
        # Array contains non-primitives (e.g. list-of-dicts from
        # `subsections:` / `tpr_findings:` frontmatter) — JSON-encode
        # the whole array so the structure is preserved as a string.
        return _json.dumps(norm, sort_keys=True, default=str)
    if isinstance(value, dict):
        # Nested map (e.g. `third_party_review:`, `keywords:`,
        # `verification_stats:`) — JSON-encode. Deterministic via
        # sort_keys. Consumers that need structure parse on read.
        return _json.dumps(value, sort_keys=True, default=str)
    return str(value)


def _data_for_node(node: NodeId, corpus) -> dict:
    """Look up the frontmatter dict for a node in the corpus buckets."""
    bucket_by_kind = {
        NodeKind.PLAN_INDEX:          corpus.indexes,
        NodeKind.PLAN_SECTION:        corpus.plan_sections,
        NodeKind.ROADMAP_SECTION:     corpus.roadmap_sections,
        NodeKind.OVERVIEW:            corpus.overviews,
        NodeKind.BUG_TRACKER_SECTION: corpus.bug_sections,
        NodeKind.FIX_BUG:             corpus.fix_bug_files,
        NodeKind.COMPLETED_INDEX:     corpus.completed_indexes,
    }
    return bucket_by_kind.get(node.kind, {}).get(node.path, {}) or {}


def _node_properties(node: NodeId, corpus) -> dict:
    """Materialize the property dict for a node — normalized frontmatter
    plus structural extras (`path`, `repo`, `touches_raw`). Omits
    properties whose value is None after normalization."""
    data = _data_for_node(node, corpus)
    props: dict = {}
    for k, v in data.items():
        if k in _EXCLUDED_FRONTMATTER_KEYS:
            continue
        nv = _normalize_property_value(v)
        if nv is None:
            continue
        props[k] = nv
    # Structural metadata.
    props["path"] = _rel_path(node.path)
    props["repo"] = "ori"
    # touches_raw carried as its own key so §02.3 can read it without
    # re-parsing frontmatter even if `touches` is omitted because it is None.
    touches = data.get("touches")
    if isinstance(touches, list):
        props["touches_raw"] = [str(t) for t in touches if isinstance(t, str)]
    return props


# ---------------------------------------------------------------------------
# Structural relationship synthesis
# ---------------------------------------------------------------------------


def _structural_relationships(corpus, node_id_map: dict[NodeId, str]) -> list[dict]:
    """Derive `HAS_SECTION`, `HAS_OVERVIEW`, `HAS_BUG`, `FIXED_BY` edges
    from corpus structure. These are not in `dag.edges` or
    `dag.references` — they encode containment / binding that the schema
    makes implicit."""
    rels: list[dict] = []

    # Index → section: group plan_sections by plan directory.
    for section_path in sorted(corpus.plan_sections.keys()):
        plan_dir = section_path.parent
        idx_path = plan_dir / "index.md"
        if idx_path not in corpus.indexes:
            continue
        start_id = node_id_map.get(NodeId(NodeKind.PLAN_INDEX, idx_path))
        end_id = node_id_map.get(NodeId(NodeKind.PLAN_SECTION, section_path))
        if start_id is None or end_id is None:
            continue
        rels.append({
            "type": "HAS_SECTION",
            "start_id": start_id,
            "end_id": end_id,
            "properties": {"structural": True},
        })

    # Index → overview.
    for overview_path in sorted(corpus.overviews.keys()):
        plan_dir = overview_path.parent
        idx_path = plan_dir / "index.md"
        if idx_path not in corpus.indexes:
            continue
        start_id = node_id_map.get(NodeId(NodeKind.PLAN_INDEX, idx_path))
        end_id = node_id_map.get(NodeId(NodeKind.OVERVIEW, overview_path))
        if start_id is None or end_id is None:
            continue
        rels.append({
            "type": "HAS_OVERVIEW",
            "start_id": start_id,
            "end_id": end_id,
            "properties": {"structural": True},
        })

    # BugTrackerSection → Bug entries (synthetic — bug entries aren't in
    # the corpus buckets; parse_bug_entries yields them from body text).
    # FixSection ← Bug via `bug:` frontmatter on fix-BUG files.
    fix_id_by_bug: dict[str, str] = {}
    for fix_path, fix_data in corpus.fix_bug_files.items():
        bug_id = fix_data.get("bug")
        if isinstance(bug_id, str):
            fix_id_by_bug[bug_id.strip()] = node_id_map.get(
                NodeId(NodeKind.FIX_BUG, fix_path), ""
            )

    for section_path in sorted(corpus.bug_sections.keys()):
        try:
            text = section_path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        section_stable = node_id_map.get(
            NodeId(NodeKind.BUG_TRACKER_SECTION, section_path)
        )
        if section_stable is None:
            continue
        for entry in parse_bug_entries(text, section_path.name):
            rels.append({
                "type": "HAS_BUG",
                "start_id": section_stable,
                "end_id": entry.bug_id,
                "properties": {
                    "status": entry.status,
                    "severity": entry.severity,
                    "lineno": entry.lineno,
                },
            })
            fix_stable = fix_id_by_bug.get(entry.bug_id)
            if fix_stable:
                rels.append({
                    "type": "FIXED_BY",
                    "start_id": entry.bug_id,
                    "end_id": fix_stable,
                    "properties": {"structural": True},
                })

    return rels


# ---------------------------------------------------------------------------
# Synthetic nodes — Bug entries and Subsections
# ---------------------------------------------------------------------------


# BugEntry fields that should NEVER land in Neo4j — body_text and body_lines
# carry full entry markdown (kilobytes each) and are not query-shaped.
_BUG_EXCLUDED_FIELDS: frozenset[str] = frozenset({"body_text", "body_lines"})


def _synth_bug_nodes(corpus) -> list[dict]:
    """Emit one :Bug node per BugEntry found in every bug-tracker section.

    Closes the dangling-end_id gap for HAS_BUG / FIXED_BY edges. `BugEntry`
    is the SSOT for bug metadata (bug_markers.parse_bug_entries); this
    function is a pure projection of its dataclass fields into node
    properties, so any future schema change (new field on BugEntry)
    propagates without editing this module.
    """
    nodes: list[dict] = []
    for section_path in sorted(corpus.bug_sections.keys()):
        try:
            text = section_path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for entry in parse_bug_entries(text, section_path.name):
            props: dict = {"repo": "ori"}
            for field_name, field_value in entry.__dict__.items():
                if field_name in _BUG_EXCLUDED_FIELDS:
                    continue
                nv = _normalize_property_value(field_value)
                if nv is None:
                    continue
                props[field_name] = nv
            nodes.append({
                "id": entry.bug_id,
                "labels": ["Bug"],
                "properties": props,
            })
    return nodes


def _synth_subsection_nodes_and_edges(
    corpus, node_id_map: dict[NodeId, str]
) -> tuple[list[dict], list[dict]]:
    """Emit :Subsection nodes from every section's `sections:` frontmatter
    list and HAS_SUBSECTION edges linking the parent section to each one.

    Stable id: `<parent-section-path>#<subsection-id>`. Both PlanSection
    and FixBug section frontmatter can carry `sections:`; both are
    handled here so the envelope covers every subsection node
    referenced by BLOCKED_BY / DEPENDS_ON edges (where authors target a
    subsection via its section-id alone)."""
    nodes: list[dict] = []
    edges: list[dict] = []

    def _process(bucket, parent_kind: NodeKind) -> None:
        for parent_path in sorted(bucket.keys()):
            data = bucket[parent_path]
            subs = data.get("sections") or []
            parent_stable = node_id_map.get(NodeId(parent_kind, parent_path))
            if not parent_stable or not isinstance(subs, list):
                continue
            for sub in subs:
                if not isinstance(sub, dict):
                    continue
                sub_id = sub.get("id")
                if not isinstance(sub_id, str) or not sub_id:
                    continue
                stable = f"{_rel_path(parent_path)}#{sub_id}"
                props: dict = {"subsection_id": sub_id}
                for key in ("title", "status"):
                    val = sub.get(key)
                    if isinstance(val, str) and val:
                        props[key] = val
                nodes.append({
                    "id": stable,
                    "labels": ["Subsection"],
                    "properties": props,
                })
                edges.append({
                    "type": "HAS_SUBSECTION",
                    "start_id": parent_stable,
                    "end_id": stable,
                    "properties": {"structural": True},
                })

    _process(corpus.plan_sections, NodeKind.PLAN_SECTION)
    _process(corpus.fix_bug_files, NodeKind.FIX_BUG)
    return nodes, edges


# ---------------------------------------------------------------------------
# Main entry point
# ---------------------------------------------------------------------------


def export_neo4j_json(
    corpus,
    dag: Dag,
    *,
    include_references: bool = True,
) -> dict:
    """Serialize `corpus + dag` into a deterministic Neo4j-flavored envelope.

    Shape:
        {
            "schema_version": "1.0",
            "generated_at": "<ISO-8601 UTC timestamp>",
            "nodes": [
                {"id": "<stable>", "labels": ["<NodeLabel>"], "properties": {...}},
                ...
            ],
            "relationships": [
                {"type": "<UPPER>", "start_id": "<stable>", "end_id": "<stable>",
                 "properties": {"source_kind": "...", "source_line": <int|null>,
                                "raw_text": "...", "mention_kind": "declared"|"inferred"}},
                ...
            ]
        }

    Determinism: nodes sort by id; relationships sort by
    `(start_id, type, end_id)`. JSON-serialization should use
    `sort_keys=True` for stable property ordering (the CLI caller does
    this already — `__main__.py` passes `sort_keys=True` to `json.dumps`).

    `include_references=False` omits reference-only relationships
    (kinds that don't become Edges: PROSE_VERB, YAML_COMMENT,
    HTML_COMMENT_CONVENTION, EXPLICIT_REFERENCES). Edge-backed
    relationships always emit. Structural relationships
    (HAS_SECTION / HAS_OVERVIEW / HAS_BUG / FIXED_BY) always emit.
    """
    # Nodes — one entry per DAG node.
    nodes: list[dict] = []
    node_id_map: dict[NodeId, str] = {}
    for node in sorted(dag.nodes):
        stable = _stable_id(node, corpus)
        node_id_map[node] = stable
        nodes.append({
            "id": stable,
            "labels": [_node_label(node.kind)],
            "properties": _node_properties(node, corpus),
        })

    # Synthetic nodes — Bug entries and Subsections (not in dag.nodes
    # because they aren't file-based; DAG SSOT stays file-node-shaped).
    nodes.extend(_synth_bug_nodes(corpus))
    sub_nodes, sub_edges = _synth_subsection_nodes_and_edges(corpus, node_id_map)
    nodes.extend(sub_nodes)

    nodes.sort(key=lambda n: n["id"])

    # Relationships — three sources: dag.edges, dag.references (optional),
    # structural synthesis.
    rels: list[dict] = []

    # Edge-backed relationships (edges always carry a Reference — source_line
    # / raw_text come from there).
    for edge in dag.edges:
        start = node_id_map.get(edge.from_node)
        end = node_id_map.get(edge.to_node)
        if start is None or end is None:
            continue
        rels.append({
            "type": _source_kind_to_rel_type(
                edge.source_kind, edge.reference.raw_text
            ),
            "start_id": start,
            "end_id": end,
            "properties": {
                "source_kind": edge.source_kind.value,
                "source_line": edge.reference.source_line or None,
                "raw_text": edge.reference.raw_text,
                "mention_kind": "declared",
            },
        })

    if include_references:
        for ref in dag.references:
            # Edge-forming kinds already emit via the Edge loop above when
            # the target resolves; the parallel Reference record is a
            # provenance twin (dag.py emits both). Skip the reference-side
            # emission unconditionally for these kinds to avoid duplicate
            # relationships in the envelope.
            if ref.source_kind in (
                SourceKind.EXPLICIT_DEPENDS_ON,
                SourceKind.EXPLICIT_SUPERSEDES,
            ):
                continue
            start = node_id_map.get(ref.from_node)
            if start is None:
                continue
            # Body-inferred references don't resolve to a NodeId target
            # here — that resolution lives in §02.3. We emit them against
            # the `target` string; §02.3 either resolves to a Symbol node
            # or creates an UnresolvedSymbol stub.
            end: str = ref.target
            rels.append({
                "type": _source_kind_to_rel_type(
                    ref.source_kind, ref.raw_text
                ),
                "start_id": start,
                "end_id": end,
                "properties": {
                    "source_kind": ref.source_kind.value,
                    "source_line": ref.source_line or None,
                    "raw_text": ref.raw_text,
                    "mention_kind": "declared"
                    if ref.source_kind is SourceKind.EXPLICIT_REFERENCES
                    else "inferred",
                },
            })

    rels.extend(_structural_relationships(corpus, node_id_map))
    rels.extend(sub_edges)
    # Filter edges whose target is an unfilled template placeholder.
    # These are authored-scaffold leaks (`<source-ref>`, `ID`, `...`), not
    # real node references, and cannot be MERGEd against anything.
    rels = [r for r in rels if not _is_placeholder_target(r["end_id"])]
    rels.sort(key=lambda r: (r["start_id"], r["type"], r["end_id"]))

    return {
        "schema_version": SCHEMA_VERSION,
        "generated_at": _dt.datetime.now(_dt.timezone.utc).isoformat(
            timespec="seconds"
        ),
        "nodes": nodes,
        "relationships": rels,
    }


__all__ = ["export_neo4j_json", "SCHEMA_VERSION"]
