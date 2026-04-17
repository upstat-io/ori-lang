---
section: "01"
title: "plan_corpus schema + dag + exporter"
status: complete
reviewed: true
goal: "Extend scripts/plan_corpus with touches: frontmatter field, SourceKind variants for supersedes/references, Edge invariant relaxation, and a Neo4j-flavored JSON exporter — producing a deterministic, round-trippable envelope that §02 will consume."
success_criteria:
  - "scripts/plan_corpus/schemas.py:PlanSectionSchema and FixBugSchema have optional touches: list[str] | None = None field; added after inspired_by and depends_on respectively."
  - "scripts/plan_corpus/types.py:SourceKind has new variants EXPLICIT_SUPERSEDES and EXPLICIT_REFERENCES."
  - "scripts/plan_corpus/dag.py:Edge.__post_init__ accepts EXPLICIT_DEPENDS_ON and EXPLICIT_SUPERSEDES (frozenset _EDGE_KINDS); classify_redundant_dependency filters to EXPLICIT_DEPENDS_ON only; apply_source_kind_severity assigns HIGH for EXPLICIT_SUPERSEDES and MEDIUM for EXPLICIT_REFERENCES."
  - "scripts/plan_corpus/dag.py:build_dag has a new supersedes_sources loop (mirrors deps_sources loop at lines 840-913) that iterates corpus.indexes and corpus.overviews for supersedes frontmatter entries and emits Edge(source_kind=EXPLICIT_SUPERSEDES); a parallel references-only loop populates dag.references for PlanIndexSchema.references and OverviewSchema.references."
  - "scripts/plan_corpus/export_json.py exists, exports a deterministic Neo4j-flavored envelope {nodes: [...], relationships: [...]} with node labels derived from NodeKind, edge types derived from SourceKind, and full provenance (source_kind, source_line, raw_text)."
  - "scripts/plan_corpus/__main__.py exposes `export` subcommand with --output <path> flag alongside existing check/discover/docgen."
  - "docs/internal/plan-schema-reference.md regenerated; python -m scripts.plan_corpus docgen --check returns exit 0."
  - "tests/plan-audit/test_export_json.py: fixture corpus round-trip test asserts nodes count, relationships count, envelope schema stability; runs via `pytest tests/plan-audit/test_export_json.py`."
  - "Satisfies mission criteria: 'scripts/plan_corpus/schemas.py exposes optional touches:...' and 'scripts/plan_corpus/export_json.py serializes Corpus + Dag to a Neo4j-flavored JSON envelope...'."
inspired_by:
  - "Ori scripts/plan_corpus/dag.py — existing DAG SSOT (NodeKind, Edge, Reference, build_dag, 8 classifiers)"
  - "Ori scripts/plan_corpus/docgen.py — auto-generation + --check drift gate pattern"
  - "lang_intelligence neo4j/import_code_graph.py — two-phase MERGE pattern the exporter feeds"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Add touches: field to PlanSectionSchema and FixBugSchema"
    status: complete
  - id: "01.2"
    title: "Add EXPLICIT_SUPERSEDES, EXPLICIT_REFERENCES to SourceKind enum"
    status: complete
  - id: "01.3"
    title: "Extend dag.py — relax Edge guard, add supersedes/references source loops, fix classifiers"
    status: complete
  - id: "01.4"
    title: "Write export_json.py + export subcommand in __main__.py"
    status: complete
  - id: "01.5"
    title: "Regenerate docs/internal/plan-schema-reference.md"
    status: complete
  - id: "01.6"
    title: "Fixture-corpus round-trip test"
    status: complete
  - id: "01.7"
    title: "Referentially-closed envelope: Bug+Subsection nodes, label fix, placeholder filter"
    status: complete
  - id: "01.N"
    title: "Completion Checklist"
    status: complete
---

# Section 01: plan_corpus schema + dag + exporter

**Status:** Complete
**Goal:** Extend `scripts/plan_corpus` with a `touches:` frontmatter field on `PlanSectionSchema` and `FixBugSchema`, two new `SourceKind` variants (`EXPLICIT_SUPERSEDES`, `EXPLICIT_REFERENCES`), corresponding edge-promotion loops and classifier fixes in `dag.py`, and a new `export_json.py` module that serializes the `Corpus + Dag` to a deterministic Neo4j-flavored JSON envelope — producing a complete, round-trippable artifact that §02's importer will consume unchanged.

**Success Criteria:**

- [x] `scripts/plan_corpus/schemas.py`: `PlanSectionSchema` has `touches: list[str] | None = None` after `inspired_by`; `FixBugSchema` has `touches: list[str] | None = None` after `depends_on` — verified by `python -m scripts.plan_corpus docgen --check` returning exit 0
- [x] `scripts/plan_corpus/types.py`: `SourceKind` enum has `EXPLICIT_SUPERSEDES = "explicit_supersedes"` and `EXPLICIT_REFERENCES = "explicit_references"` variants; docstring updated to reflect that EXPLICIT_DEPENDS_ON and EXPLICIT_SUPERSEDES are the ONLY kinds that become `Dag.edges`
- [x] `scripts/plan_corpus/dag.py`: `Edge.__post_init__` enforces `source_kind in _EDGE_KINDS` where `_EDGE_KINDS = frozenset({EXPLICIT_DEPENDS_ON, EXPLICIT_SUPERSEDES})`; `classify_redundant_dependency` filters to `EXPLICIT_DEPENDS_ON` edges only; `apply_source_kind_severity` maps `EXPLICIT_SUPERSEDES → HIGH` and `EXPLICIT_REFERENCES → MEDIUM`
- [x] `scripts/plan_corpus/dag.py`: `build_dag` has a `supersedes_sources` loop emitting `Edge(source_kind=EXPLICIT_SUPERSEDES, ...)` from `corpus.indexes` and `corpus.overviews`; a `references_sources` loop emitting `Reference(source_kind=EXPLICIT_REFERENCES, ...)` into `dag.references` only (no edges)
- [x] `scripts/plan_corpus/export_json.py` exists and exports a deterministic `{"schema_version": "1.0", "generated_at": ..., "nodes": [...], "relationships": [...]}` envelope — verified by `pytest tests/plan-audit/test_export_json.py` green
- [x] `scripts/plan_corpus/__main__.py`: `export` subcommand with `--output <path>` flag works alongside `check`/`discover`/`docgen`
- [x] `docs/internal/plan-schema-reference.md` regenerated; `python -m scripts.plan_corpus docgen --check` returns exit 0
- [x] `pytest tests/plan-audit/test_export_json.py` green (fixture corpus round-trip, determinism, Neo4j envelope validation)
- [x] `pytest tests/plan-audit/test_dag.py` green — no regression from `Edge.__post_init__` guard relaxation; `Edge` invariant test updated to assert `source_kind in _EDGE_KINDS`
- [x] Satisfies mission criterion: "scripts/plan_corpus/schemas.py exposes optional touches:..." and "scripts/plan_corpus/export_json.py serializes Corpus + Dag to a Neo4j-flavored JSON envelope..."

**Context:** The `scripts/plan_corpus/` library currently models all intra-plan dependency relationships as `EXPLICIT_DEPENDS_ON` edges. However, the plan frontmatter schema already allows `supersedes:` (on `PlanIndexSchema` and `OverviewSchema`) and `references:` (on both) — neither of which are promoted to first-class typed edges today. The Neo4j importer in §02 needs these as distinct edge kinds to model the `:SUPERSEDES` and `:REFERENCES` relationship types. Additionally, every plan section may need a `touches:` list of code symbols so plan/code joins (`:MENTIONS_CODE`) can be populated declaratively. This section delivers the schema extension, the new DAG semantics, and the serialization layer — the minimal foundation that §02 needs to MERGE into Neo4j.

**Reference implementations:**
- **Ori** `scripts/plan_corpus/dag.py` (lines 840-913): `deps_sources` loop pattern — the `supersedes_sources` and `references_sources` loops mirror this exactly, with DRY extraction via `_emit_edges_from_frontmatter_list()` if near-identical
- **Ori** `scripts/plan_corpus/docgen.py`: `generate_schema_reference()` + `docgen --check` drift gate — the auto-generation mechanism that §01.5 triggers
- **lang_intelligence** `neo4j/import_code_graph.py` (lines 536-557): two-phase MERGE pattern — nodes first, then relationships; `DETACH DELETE` for stale removal; the JSON envelope shape feeds this importer

**Depends on:** None — §01 is the foundation; all other sections depend on it.

---

## Intelligence Reconnaissance

Queries run 2026-04-17:

- `scripts/intel-query.sh --human search "plan corpus DAG ingestion" --limit 5` — no relevant prior art; returned `std.testing.fuzzInput` (Zig corpus fuzz), `CHECK-DAG` (LLVM FileCheck in Swift/Zig), none related to plan-metadata DAG export. Plan-corpus DAG ingestion is a novel construct in this codebase.
- `scripts/intel-query.sh --human symbols "build_dag" --repo ori --limit 10` — zero matches. The intelligence graph indexes Rust symbols only; `plan_corpus/dag.py` is Python and its symbols are absent. Expected result — confirmed the graph is Rust-only for the `ori` repo.
- `scripts/intel-query.sh --human file-symbols "plan_corpus" --repo ori` — zero matches. Confirms the code-symbol index is Rust-only; Python `scripts/plan_corpus/` modules do not appear. Manual reading of `dag.py`, `schemas.py`, `types.py`, `docgen.py`, `__main__.py` was required to ground the implementation plan.
- `scripts/intel-query.sh --human similar "build_dag" --repo rust,swift,go --limit 5` — symbol not found / no embedding yet. Cross-repo semantic lookup not available for this symbol.

Results summary (≤500 chars) [ori]: Graph is available (Neo4j 5.26.24, 191K+ symbols, 505K+ CALLS). All four queries returned zero relevant results — confirmed the graph indexes Rust/compiled symbols only; Python `scripts/plan_corpus/` is absent. No cross-repo prior art for plan-metadata DAG export was found (search results were unrelated: fuzz corpora, LLVM FileCheck). Implementation is grounded entirely by manual reading of the five target source files. No blast-radius concerns from the intelligence graph perspective.

See `.claude/skills/query-intel/compose-intel-summary.md` for the full query protocol (SSOT — do NOT `@`-include in plan files; plan markdown is not harness-expanded, so the include would be a dead literal).

---

## 01.1 Add `touches:` field to PlanSectionSchema and FixBugSchema

**File(s):** `scripts/plan_corpus/schemas.py`

This subsection adds the optional `touches: list[str] | None = None` field to `PlanSectionSchema` and `FixBugSchema`. The field lists code symbols (crate paths, function names, type names, file paths) that a plan section or fix section declaratively touches — consumed downstream by §02's CodeReference bridge to produce `[:MENTIONS_CODE]` edges with `mention_kind: "declared"`. Scraping-based `mention_kind: "inferred"` edges are deferred to §02.3 where symbol resolution context is available.

The field is an unvalidated list of freeform strings at this schema layer. Consumers (the exporter in §01.4, the importer in §02.3) apply symbol resolution; the schema only enforces that if present, it is a list of strings. No `_validate_touches_format` helper is needed at this layer — the `_schema_allowed_fields` introspection in `schema.py` automatically accepts the new field, and the WRONG_TYPE check in `schema.py`'s `_validate_field_types` catches non-list values.

- [x] Edit `scripts/plan_corpus/schemas.py` line ~67: add `touches: list[str] | None = None` after `inspired_by: list[str] | None = None` in `PlanSectionSchema`:
  ```python
  @dataclass(frozen=True)
  class PlanSectionSchema:
      """Schema for `plans/*/section-*.md` (non-roadmap plan sections)."""
      section: str
      title: str
      status: str
      reviewed: bool
      goal: str
      success_criteria: list[str]
      sections: list[dict]
      third_party_review: dict
      depends_on: list[str] | None = None
      inspired_by: list[str] | None = None
      touches: list[str] | None = None  # ← NEW: declarative code symbol list
  ```

- [x] Edit `scripts/plan_corpus/schemas.py` line ~123: add `touches: list[str] | None = None` after `depends_on: list[str] | None = None` in `FixBugSchema`:
  ```python
  @dataclass(frozen=True)
  class FixBugSchema:
      """Schema for `plans/bug-tracker/fix-BUG-*.md`."""
      bug: str
      title: str
      severity: str
      status: str
      goal: str
      success_criteria: list[str]
      subsystem: str
      found: str
      source: str
      third_party_review: dict
      sections: list[dict] | None = None
      depends_on: list[str] | None = None
      touches: list[str] | None = None  # ← NEW: declarative code symbol list
  ```

- [x] Verify no existing plan files are broken by the addition: `python -m scripts.plan_corpus check plans/` — `touches` is optional with `None` default so all existing files continue to parse without error; `UNKNOWN_FIELD` would only fire if files already have `touches:` entries that previously triggered schema violations — check for any.

- [x] Run `python -m scripts.plan_corpus docgen` (§01.5 will commit the output, but run now to confirm the auto-generation succeeds with the new fields present and the output is sane).

- [x] **Subsection close-out (01.1)** — MANDATORY before starting 01.2:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check`; clean any temp files detected.

---

## 01.2 Add EXPLICIT_SUPERSEDES, EXPLICIT_REFERENCES to SourceKind enum

**File(s):** `scripts/plan_corpus/types.py`

This subsection adds two new `SourceKind` variants. `EXPLICIT_SUPERSEDES` represents frontmatter `supersedes:` list entries — analogous to `EXPLICIT_DEPENDS_ON` but carrying supersede semantics; it becomes a DAG edge. `EXPLICIT_REFERENCES` represents frontmatter `references:` list entries — it is a reference-only kind, populating `dag.references` but never `dag.edges`, exactly like `PROSE_VERB` and `HTML_COMMENT_CONVENTION`.

The `SourceKind` docstring currently states: "Only `EXPLICIT_DEPENDS_ON` references become DAG edges." This must be updated to: "Only `EXPLICIT_DEPENDS_ON` and `EXPLICIT_SUPERSEDES` references become DAG edges; `EXPLICIT_REFERENCES` and the body-inferred kinds feed `dag.references` only."

**SSOT invariant enforced by this change:** `Edge.__post_init__` in `dag.py` enforces which `SourceKind` values are permitted in edges. Adding `EXPLICIT_SUPERSEDES` to `_EDGE_KINDS` (§01.3a) is the mechanism; this subsection merely introduces the enum variant that §01.3 will use.

- [x] Edit `scripts/plan_corpus/types.py` line ~99: after `CODE_FENCE_EXAMPLE = "code_fence_example"` add the two new variants and update the docstring:
  ```python
  class SourceKind(enum.Enum):
      """Reference source taxonomy used by the §02 DAG builder.

      Lives in types.py (NOT dag.py) because Finding.source_kind is a typed
      field on the canonical Finding dataclass — homing SourceKind in dag.py
      would create a circular import (types.py -> dag.py -> types.py). See
      §02.0 File(s) note and TPR-02-001-gemini round 2.

      Only EXPLICIT_DEPENDS_ON and EXPLICIT_SUPERSEDES references become DAG
      edges (dag.edges); all other kinds feed dag.references only —
      MISSING_DEPENDENCY / DEAD_REFERENCE classifiers run on references, not
      on shadow edges.
      """
      EXPLICIT_DEPENDS_ON = "explicit_depends_on"
      EXPLICIT_SUPERSEDES = "explicit_supersedes"   # ← NEW: frontmatter supersedes:
      EXPLICIT_REFERENCES = "explicit_references"   # ← NEW: frontmatter references: (ref-only)
      HTML_COMMENT_CONVENTION = "html_comment_convention"
      YAML_COMMENT = "yaml_comment"
      PROSE_VERB = "prose_verb"
      CODE_FENCE_EXAMPLE = "code_fence_example"
  ```

- [x] Verify `python -c "from scripts.plan_corpus.types import SourceKind; print(SourceKind.EXPLICIT_SUPERSEDES, SourceKind.EXPLICIT_REFERENCES)"` succeeds.

- [x] Verify `python -m scripts.plan_corpus check plans/plan-bug-dag-ingestion/` — no regressions from new enum variants (all downstream match arms must handle them; §01.3 handles `Edge.__post_init__`; `apply_source_kind_severity` update is in §01.3e).

- [x] **Subsection close-out (01.2)** — MANDATORY before starting 01.3:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check`.

---

## 01.3 Extend dag.py — relax Edge guard, add supersedes/references source loops, fix classifiers

**File(s):** `scripts/plan_corpus/dag.py`

This subsection performs the core `dag.py` changes. The work decomposes into six parts (01.3a–01.3f), grouped in dependency order.

**System invariant being enforced:** `dag.py` is the DAG SSOT per the §01 Design Principle "(1) dag.py is the DAG SSOT; the Neo4j projection is derived." All frontmatter relationship semantics must be modeled here first; the exporter in §01.4 is a thin serialization adapter. Adding `supersedes`/`references` edge promotion in the exporter instead would be a `LEAK:scattered-knowledge` violation per `impl-hygiene.md §SSOT`.

**Downstream consumer of this invariant:** §02's `import_plan_bug_graph.py` consumes exactly the JSON envelope shape `export_json.py` produces. The importer must not re-parse frontmatter to reconstruct supersedes relationships — it trusts the envelope.

### 01.3a — Relax `Edge.__post_init__`

The current guard at `dag.py:135-141` hard-codes `is EXPLICIT_DEPENDS_ON`. Replace with a module-level `_EDGE_KINDS` frozenset that both the guard and external tests can import:

```python
# Module-level constant — defined once, used by Edge.__post_init__ and tests.
_EDGE_KINDS: frozenset[SourceKind] = frozenset({
    SourceKind.EXPLICIT_DEPENDS_ON,
    SourceKind.EXPLICIT_SUPERSEDES,
})


@dataclass(frozen=True)
class Edge:
    """A DAG edge.

    Only source_kinds in _EDGE_KINDS (EXPLICIT_DEPENDS_ON, EXPLICIT_SUPERSEDES)
    are promoted to edges. Body-inferred references (PROSE_VERB,
    HTML_COMMENT_CONVENTION, YAML_COMMENT) and reference-only frontmatter
    (EXPLICIT_REFERENCES) are collected as Reference records only — they feed
    MISSING_DEPENDENCY per the §02.0 SSOT rule.
    """
    from_node: NodeId
    to_node: NodeId
    source_kind: SourceKind
    reference: Reference

    def __post_init__(self) -> None:
        if self.source_kind not in _EDGE_KINDS:
            raise ValueError(
                f"Edge source_kind must be in _EDGE_KINDS "
                f"({', '.join(k.name for k in _EDGE_KINDS)}) — "
                f"body-inferred references and EXPLICIT_REFERENCES feed "
                f"dag.references, never shadow edges. Got {self.source_kind.name}."
            )
```

- [x] Add `_EDGE_KINDS` frozenset constant immediately before the `Edge` dataclass (after the `Reference` dataclass definition)
- [x] Update `Edge.__post_init__` to use `source_kind not in _EDGE_KINDS`
- [x] Update the class docstring to name both permitted kinds

### 01.3b — `supersedes_sources` loop in `build_dag`

Add a new loop immediately after the `deps_sources` loop (which ends at approximately line 913 in the current file). The loop mirrors the `deps_sources` loop structure — iterate sources, extract frontmatter list, resolve each, emit `Edge + Reference`. Extract a common helper to avoid algorithmic duplication (`impl-hygiene.md §Algorithmic DRY`):

```python
def _emit_edges_from_frontmatter_list(
    dag: "Dag",
    sources: list[tuple["NodeId", Path, Path, dict]],
    field_name: str,
    source_kind: "SourceKind",
    edges: bool,
    corpus,
) -> None:
    """Shared kernel for deps_sources / supersedes_sources / references_sources.

    If `edges=True`, emits both Reference and Edge for each resolved target.
    If `edges=False`, emits Reference only (references-only kinds like
    EXPLICIT_REFERENCES and the body-inferred kinds).
    """
    from .docgen import resolve_dep
    from .types import Finding, FindingCategory, FindingSubtype, Severity
    from .parser import read_text_strict

    for from_node, declaring_file, plan_dir, data in sources:
        entries = data.get(field_name) or []
        if not isinstance(entries, list):
            continue
        for entry in entries:
            if not isinstance(entry, str):
                continue
            try:
                raw_text = read_text_strict(declaring_file)
                yaml_line = _find_yaml_list_line(raw_text, field_name, entry)
            except Exception:
                yaml_line = None

            resolved = resolve_dep(entry, plan_dir, corpus)
            if isinstance(resolved, Path):
                target_nid = next(
                    (n for n in dag.nodes if n.path == resolved),
                    None,
                )
                if target_nid is None:
                    dag.resolution_findings.append(Finding(
                        category=FindingCategory.DEAD_REFERENCE,
                        subtype=FindingSubtype.SECTION_FILE_NOT_FOUND,
                        severity=Severity.HIGH,
                        source=declaring_file,
                        source_line=yaml_line,
                        description=f"{field_name} target not in corpus: {entry}",
                        recommended_fix="Ensure the target file exists and is classified",
                        evidence=(entry,),
                        source_kind=source_kind,
                        target_value=entry,
                    ))
                    continue
                ref = Reference(
                    from_node=from_node,
                    target=entry,
                    source_kind=source_kind,
                    source_line=yaml_line if yaml_line else 0,
                    source_column=None,
                    raw_text=entry,
                )
                dag.references.append(ref)
                if edges:
                    dag.edges.append(Edge(
                        from_node=from_node,
                        to_node=target_nid,
                        source_kind=source_kind,
                        reference=ref,
                    ))
            else:
                enriched = enrich_resolve_dep_finding(
                    resolved,
                    dep_id=entry,
                    yaml_line=yaml_line or 0,
                    declaring_file=declaring_file,
                )
                dag.resolution_findings.append(enriched)
```

- [x] Add `_emit_edges_from_frontmatter_list` helper function immediately before `build_dag`
- [x] **Refactor** the existing `deps_sources` inner loop in `build_dag` to call `_emit_edges_from_frontmatter_list(..., field_name="depends_on", source_kind=EXPLICIT_DEPENDS_ON, edges=True)` — this is the DRY refactor; existing behavior must be preserved exactly
- [x] After `deps_sources` block, add `supersedes_sources` using the same list structure but populating from `corpus.indexes` and `corpus.overviews` (both have `supersedes:` field per `PlanIndexSchema:50` and `OverviewSchema:95`):
  ```python
  # 2b. Parse supersedes edges.
  supersedes_sources: list[tuple[NodeId, Path, Path, dict]] = []
  for path, data in corpus.indexes.items():
      supersedes_sources.append((NodeId(NodeKind.PLAN_INDEX, path), path, path.parent, data))
  for path, data in corpus.overviews.items():
      supersedes_sources.append((NodeId(NodeKind.OVERVIEW, path), path, path.parent, data))

  _emit_edges_from_frontmatter_list(
      dag, supersedes_sources, "supersedes",
      SourceKind.EXPLICIT_SUPERSEDES, edges=True, corpus=corpus,
  )
  ```
- [x] After `supersedes_sources` block, add `references_sources` (references-only — both `corpus.indexes` and `corpus.overviews` have `references:` per the same schemas):
  ```python
  # 2c. Parse references (reference-only — no edges).
  references_sources: list[tuple[NodeId, Path, Path, dict]] = []
  for path, data in corpus.indexes.items():
      references_sources.append((NodeId(NodeKind.PLAN_INDEX, path), path, path.parent, data))
  for path, data in corpus.overviews.items():
      references_sources.append((NodeId(NodeKind.OVERVIEW, path), path, path.parent, data))

  _emit_edges_from_frontmatter_list(
      dag, references_sources, "references",
      SourceKind.EXPLICIT_REFERENCES, edges=False, corpus=corpus,
  )
  ```
- [x] Update `build_dag` docstring to mention steps 2b and 2c

### 01.3c — `classify_redundant_dependency` filter

At approximately line 1614, the `for e in dag.edges:` loop in `classify_redundant_dependency` iterates ALL edges including the newly-possible `EXPLICIT_SUPERSEDES` edges. A supersede edge A→B plus a depends_on edge A→B should NOT produce a false-positive REDUNDANT_DEPENDENCY finding. Add a guard at the top of the loop body:

```python
def classify_redundant_dependency(dag: Dag, corpus) -> list:
    """REDUNDANT_DEPENDENCY: A->C direct when A->B->...->C transitively exists.

    Filters to EXPLICIT_DEPENDS_ON edges only — supersedes edges have distinct
    semantics and must not be flagged as redundant even when a parallel
    depends_on edge exists.
    """
    from .types import Finding, FindingCategory, FindingSubtype, Severity
    findings: list = []

    # Build adjacency from EXPLICIT_DEPENDS_ON edges only.
    adj: dict[NodeId, set[NodeId]] = {}
    for e in dag.edges:
        if e.source_kind is not SourceKind.EXPLICIT_DEPENDS_ON:
            continue  # ← guard: supersedes edges have distinct semantics
        adj.setdefault(e.from_node, set()).add(e.to_node)

    # For each EXPLICIT_DEPENDS_ON edge A->C, check for depth >= 2 path.
    for e in dag.edges:
        if e.source_kind is not SourceKind.EXPLICIT_DEPENDS_ON:
            continue  # ← same guard for the outer iteration
        A, C = e.from_node, e.to_node
        ...  # rest of the existing BFS logic unchanged
```

- [x] Update `classify_redundant_dependency` to filter `e.source_kind is not SourceKind.EXPLICIT_DEPENDS_ON` in BOTH the adjacency-build loop and the outer edge-iteration loop
- [x] Update the function docstring to document this filter

### 01.3d — `apply_source_kind_severity` new ladder entries

At approximately line 1942, `apply_source_kind_severity` has a `if/elif/elif/else` chain. Add `EXPLICIT_SUPERSEDES → HIGH` and `EXPLICIT_REFERENCES → MEDIUM` entries before the final `else: out.append(f); continue` branch:

```python
if f.source_kind is SourceKind.EXPLICIT_DEPENDS_ON:
    target_sev = Severity.HIGH
elif f.source_kind is SourceKind.EXPLICIT_SUPERSEDES:
    target_sev = Severity.HIGH        # ← NEW: supersedes carries same weight as depends_on
elif f.source_kind in (SourceKind.HTML_COMMENT_CONVENTION, SourceKind.YAML_COMMENT):
    target_sev = Severity.MEDIUM
elif f.source_kind is SourceKind.EXPLICIT_REFERENCES:
    target_sev = Severity.MEDIUM      # ← NEW: references is medium (informational)
elif f.source_kind is SourceKind.PROSE_VERB:
    target_sev = Severity.LOW
else:
    out.append(f)
    continue
```

- [x] Update `apply_source_kind_severity` severity chain as shown above
- [x] Update the docstring comment (lines ~1908-1912) to mention `EXPLICIT_SUPERSEDES=HIGH`, `EXPLICIT_REFERENCES=MEDIUM`

### 01.3e — Existing test updates

The tests in `tests/plan-audit/test_dag.py` and `tests/plan-audit/test_dag_construction.py` include tests that verify the `Edge.__post_init__` invariant. After the guard relaxation, these tests must be updated to assert the new invariant (`source_kind in _EDGE_KINDS`) rather than the narrower predicate (`is EXPLICIT_DEPENDS_ON`).

- [x] Run `pytest tests/plan-audit/test_dag.py tests/plan-audit/test_dag_construction.py` before any changes — observe which tests fail with the old guard (expected: tests that construct `Edge(source_kind=EXPLICIT_SUPERSEDES)` now fail with the old `is EXPLICIT_DEPENDS_ON` guard, but none should exist yet since the variant is new)
- [x] After §01.3a–01.3d changes, run `pytest tests/plan-audit/test_dag.py tests/plan-audit/test_dag_construction.py` — any test that tests the `Edge.__post_init__` error message or asserts `EXPLICIT_DEPENDS_ON` exclusivity must be updated to import and use `_EDGE_KINDS`
- [x] Add a new test `test_edge_rejects_non_edge_kind_source_kind` that asserts `Edge(..., source_kind=SourceKind.EXPLICIT_REFERENCES, ...)` raises `ValueError`
- [x] Add a new test `test_edge_accepts_explicit_supersedes` that constructs `Edge(..., source_kind=SourceKind.EXPLICIT_SUPERSEDES, ...)` without error
- [x] Verify no classifier test regresses: `pytest tests/plan-audit/` green

- [x] **Subsection close-out (01.3)** — MANDATORY before starting 01.4:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check`.

---

## 01.4 Write export_json.py + export subcommand in __main__.py

**File(s):** `scripts/plan_corpus/export_json.py` (new, ~200 lines); `scripts/plan_corpus/__main__.py` (modified, ~20 lines)

This subsection delivers the core serialization layer: a new `export_json.py` module that converts `Corpus + Dag` to a deterministic Neo4j-flavored JSON envelope, and an `export` subcommand in `__main__.py` that runs `discover_corpus() + build_dag() + export_neo4j_json()` and emits to stdout or `--output <path>`.

**Design constraint — dag.py is the SSOT:** The exporter must NOT re-parse frontmatter to discover edges or references. It reads `Corpus` for node properties and `Dag` for edges and references. Any new relationship type must first be modeled in `dag.py` (as an edge or reference) before the exporter can serialize it. This is enforced by the architecture: `export_neo4j_json(corpus, dag)` takes both and has no access to raw frontmatter YAML.

**File size constraint:** `export_json.py` must stay under 500 lines. If it approaches the limit, split structural-edge generation into `_structural_edges(corpus, dag, node_id_map)` and reference-edge generation into `_reference_edges(dag, node_id_map)` helper functions.

### Node stable ID mapping

Each node kind maps to a stable string ID deterministically derived from its file path. §02's MERGE queries use these as uniqueness keys:

| NodeKind | Stable ID |
|---|---|
| `PLAN_INDEX` | plan directory name (e.g., `plan-bug-dag-ingestion`) |
| `PLAN_SECTION` | repo-relative path `plans/<plan-dir>/section-<NN>-<slug>.md` |
| `ROADMAP_SECTION` | `roadmap/section-<NN>-<slug>.md` |
| `OVERVIEW` | `plans/<plan-dir>/00-overview.md` (or repo-relative path) |
| `BUG_TRACKER_SECTION` | `bug-tracker/section-<NN>-<slug>.md` |
| `FIX_BUG` | `BUG-XX-NNN` extracted from the file's `bug:` frontmatter field |
| `COMPLETED_INDEX` | plan directory name under `completed/` |

All IDs are computed relative to `REPO_ROOT` (from `types.py`). `PLANS_DIR = REPO_ROOT / "plans"`.

### SourceKind → relationship type mapping

| SourceKind | Relationship type (`rel["type"]`) | Goes to `edges`? |
|---|---|---|
| `EXPLICIT_DEPENDS_ON` | `DEPENDS_ON` | Yes |
| `EXPLICIT_SUPERSEDES` | `SUPERSEDES` | Yes |
| `HTML_COMMENT_CONVENTION` (verb=`blocked-by`) | `BLOCKED_BY` | Reference only |
| `HTML_COMMENT_CONVENTION` (verb=`unblocks`) | `UNBLOCKS` | Reference only |
| `HTML_COMMENT_CONVENTION` (verb=`supersedes`) | `SUPERSEDES` | Reference only |
| `HTML_COMMENT_CONVENTION` (verb=`resolves`) | `RESOLVES` | Reference only |
| `HTML_COMMENT_CONVENTION` (verb=`rewrites`) | `REWRITES` | Reference only |
| `HTML_COMMENT_CONVENTION` (verb=`update-complete`) | `UPDATE_COMPLETE` | Reference only |
| `HTML_COMMENT_CONVENTION` (verb=`updated-by`) | `UPDATED_BY` | Reference only |
| `EXPLICIT_REFERENCES` | `REFERENCES` | Reference only |
| `PROSE_VERB` | `REFERENCES` | Reference only |
| `YAML_COMMENT` | `REFERENCES` | Reference only |

The HTML_COMMENT_CONVENTION verb is extracted from `reference.raw_text` via the same regex that `dag.py`'s body scanner uses. The exporter must not re-derive semantics — it reads `reference.source_kind` and `reference.raw_text` to decide the label.

### Structural edges (generated by exporter from corpus structure)

These are not in `dag.edges` or `dag.references` — they are structural containment edges derived from the corpus metadata:

| Edge type | Source | Target | Derivation |
|---|---|---|---|
| `HAS_SECTION` | PlanIndex node | PlanSection node | `corpus.plan_sections` keyed by `path.parent` |
| `HAS_OVERVIEW` | PlanIndex node | Overview node | `corpus.overviews` keyed by `path.parent` |
| `HAS_BUG` | BugTrackerSection node | Bug entry (synthetic node) | `bug_markers.parse_bug_entries()` on section body |
| `FIXED_BY` | Bug entry | FixSection node | `bug:` frontmatter field on `fix_bug_files` |

**Boundary with §02.3:** The `MENTIONS_CODE` edges (plan node → `CodeReference` → `Symbol`) are deferred to §02.3, where the symbol resolution context (`resolve_code_refs.py`) is available. The exporter emits a `touches_raw` property on each node containing the raw `touches:` list from frontmatter; the §02.3 importer scrapes this and resolves symbols. This boundary is documented in the exporter's module docstring.

### `export_neo4j_json` function signature and envelope shape

```python
def export_neo4j_json(
    corpus: "Corpus",
    dag: "Dag",
    *,
    include_references: bool = True,
) -> dict:
    """Serialize Corpus + Dag to a Neo4j-flavored JSON envelope.

    Returns:
        {
            "schema_version": "1.0",
            "generated_at": "<ISO-8601 UTC timestamp>",
            "nodes": [
                {
                    "id": "<stable string ID>",
                    "labels": ["<NodeKind label>"],
                    "properties": {
                        # All non-None frontmatter fields
                        # Plus: "touches_raw": [...] from touches: frontmatter
                        # Plus: "path": "<repo-relative path>"
                        # Plus: "repo": "ori"
                    }
                },
                ...
            ],
            "relationships": [
                {
                    "type": "<uppercase edge label>",
                    "start_id": "<source node stable ID>",
                    "end_id": "<target node stable ID>",
                    "properties": {
                        "source_kind": "<SourceKind.value>",
                        "source_line": <int or null>,
                        "raw_text": "<str>",
                        "mention_kind": "declared" | "inferred"
                    }
                },
                ...
            ]
        }

    Determinism: nodes sorted by id; relationships sorted by
    (start_id, type, end_id). json.dumps(..., sort_keys=True) for
    stable property ordering.

    MENTIONS_CODE edges are deferred to §02.3 (importer-side) where
    the Symbol resolution context from resolve_code_refs.py is available.
    The `touches_raw` property on each node carries the raw touches: list
    so §02.3 can resolve symbols without re-reading frontmatter.
    """
```

- [x] Create `scripts/plan_corpus/export_json.py` with:
  - [x] `_stable_id(node_id: NodeId, corpus: Corpus) -> str` helper that implements the stable ID mapping table above
  - [x] `_node_label(node_kind: NodeKind) -> str` helper that maps `NodeKind` to the Neo4j label string (e.g., `PLAN_INDEX → "Plan"`, `PLAN_SECTION → "PlanSection"`, etc.)
  - [x] `_source_kind_to_rel_type(source_kind: SourceKind, raw_text: str) -> str` helper that applies the SourceKind → relationship type mapping table, extracting verb from `raw_text` for `HTML_COMMENT_CONVENTION`
  - [x] `_structural_relationships(corpus: Corpus, node_id_map: dict) -> list[dict]` helper that produces `HAS_SECTION`, `HAS_OVERVIEW`, `FIXED_BY` relationships from corpus structure
  - [x] `export_neo4j_json(corpus, dag, *, include_references=True) -> dict` main function
  - [x] Determinism: `nodes.sort(key=lambda n: n["id"])`, `relationships.sort(key=lambda r: (r["start_id"], r["type"], r["end_id"]))`, `json.dumps(..., sort_keys=True)` when serializing

- [x] Edit `scripts/plan_corpus/__main__.py` to add the `export` subcommand:
  ```python
  export_p = sub.add_parser(
      "export",
      help="Export corpus + DAG as Neo4j-flavored JSON envelope (stdout or --output file)",
  )
  export_p.add_argument(
      "--output", type=Path, default=None,
      help="Write to file instead of stdout",
  )
  export_p.add_argument(
      "--no-references", action="store_true",
      help="Omit reference-only relationships (PROSE_VERB, YAML_COMMENT, etc.)",
  )
  ```
  And in the `main()` dispatch:
  ```python
  elif args.command == "export":
      from .dag import build_dag
      from .export_json import export_neo4j_json
      corpus = discover_corpus()
      dag = build_dag(corpus)
      envelope = export_neo4j_json(corpus, dag, include_references=not args.no_references)
      output = json.dumps(envelope, indent=2, sort_keys=True)
      if args.output:
          args.output.write_text(output)
          print(f"Wrote {len(envelope['nodes'])} nodes, {len(envelope['relationships'])} relationships to {args.output}")
      else:
          print(output)
      return 0
  ```

- [x] Verify `python -m scripts.plan_corpus export | python -c "import json,sys; d=json.load(sys.stdin); print(len(d['nodes']), len(d['relationships']))"` outputs plausible counts (expect 200–800 nodes from the active corpus)

- [x] **Subsection close-out (01.4)** — MANDATORY before starting 01.5:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check`.

---

## 01.5 Regenerate docs/internal/plan-schema-reference.md

**File(s):** `docs/internal/plan-schema-reference.md`

After §01.1 adds `touches:` to two schemas, the `plan-schema-reference.md` document will be out of date. The `docgen --check` drift gate enforces this. This subsection regenerates the file and verifies the drift gate passes.

This subsection is mostly automated — the schema reference is derived from `dataclasses.fields()` via `docgen.py`'s `generate_schema_reference()` function, which reads the `FILE_CLASS_META` registry in `schema.py`. No manual editing is needed.

- [x] Run `python -m scripts.plan_corpus docgen > docs/internal/plan-schema-reference.md` to regenerate
- [x] Run `python -m scripts.plan_corpus docgen --check` — must return exit 0; if it returns exit 1 with a diff, the regeneration step above was skipped or failed
- [x] Inspect the diff in `docs/internal/plan-schema-reference.md` — confirm it shows `touches` field added to `PlanSection` and `FixBug` sections and nothing else changed
- [x] Commit the regenerated file alongside the schema change (not separately — they must arrive atomically to avoid a window where `docgen --check` fails in CI)

- [x] **Subsection close-out (01.5)** — MANDATORY before starting 01.6:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check`.

---

## 01.6 Fixture-corpus round-trip test

**File(s):** `tests/plan-audit/test_export_json.py` (new, ~120 lines)

This subsection delivers the fixture-corpus round-trip test for `export_json.py`. The tests follow the pattern from `tests/plan-audit/test_dag_construction.py` — use `tmp_path` to write synthetic plan files, run `discover_corpus_from_root()` (or equivalent fixture utility), build the dag, export, and assert on the envelope.

**Test naming:** `<subject>_<scenario>_<expected>` per `impl-hygiene.md §Test Function Naming`. No ephemeral identifiers (section numbers, plan names) in function names.

**Test 1: `test_export_fixture_corpus_node_count_matches`**

Synthetic corpus: 1 plan with 2 sections + 1 bug-tracker section + 1 `fix-BUG` file. Expected node count: 4 (PlanIndex + 2 PlanSection + 1 BugTrackerSection + 1 FixBug = 5 if we include all, minus Overview if not created = varies). Assert exact node count based on what files are written. Assert required envelope keys are present.

**Test 2: `test_export_same_corpus_twice_produces_identical_json`**

Export the same corpus twice, assert the two JSON strings are byte-identical. Proves determinism of the sort + `sort_keys=True` contract.

**Test 3: `test_export_envelope_schema_has_required_keys`**

Parse the exported JSON. Assert top-level keys: `schema_version`, `generated_at`, `nodes`, `relationships`. Assert each node has `id`, `labels`, `properties`. Assert each relationship has `type`, `start_id`, `end_id`, `properties`. Assert `properties` on relationships has `source_kind`, `source_line`, `raw_text`.

**Test 4: `test_export_supersedes_edge_produces_supersedes_relationship`**

Create a fixture with a plan index having `supersedes: ["other-plan"]`. Verify the exported relationships list contains a relationship with `type: "SUPERSEDES"`.

**Test 5: `test_export_references_entry_produces_references_relationship_not_edge`**

Create a fixture with a plan index having `references: ["some-section"]`. Verify the exported relationships list contains a relationship with `type: "REFERENCES"` but that `dag.edges` does NOT contain an edge for this reference (confirms reference-only semantics).

```python
# Example structure
def test_export_same_corpus_twice_produces_identical_json(tmp_path):
    """Export determinism: same corpus produces identical JSON byte-for-byte."""
    # Write synthetic plan files to tmp_path
    # ... (follows test_dag_construction.py fixture pattern)
    from scripts.plan_corpus.export_json import export_neo4j_json
    from scripts.plan_corpus.dag import build_dag
    corpus = _build_fixture_corpus(tmp_path)
    dag = build_dag(corpus)
    out1 = json.dumps(export_neo4j_json(corpus, dag), sort_keys=True)
    out2 = json.dumps(export_neo4j_json(corpus, dag), sort_keys=True)
    assert out1 == out2
```

- [x] Create `tests/plan-audit/test_export_json.py` with tests 1–5 above
- [x] All 5 tests pass: `pytest tests/plan-audit/test_export_json.py -v`
- [x] No regression in `tests/plan-audit/test_dag.py`: `pytest tests/plan-audit/test_dag.py -v`
- [x] Full audit test suite green: `pytest tests/plan-audit/ -v`

- [x] **Subsection close-out (01.6)** — MANDATORY before starting 01.R:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check`.

---

## 01.7 Referentially-closed envelope: Bug + Subsection nodes, label fix, placeholder filter

**File(s):** `scripts/plan_corpus/export_json.py`, `tests/plan-audit/test_export_json.py`

**Scope addition discovered during §02.2 wire-up (2026-04-17):** a full-corpus export revealed four gaps that make the envelope referentially non-closed — edges point at IDs with no corresponding nodes, and one node label drifts from the §02 schema. Rather than make §02's importer tolerant of §01's gaps (which would scatter DAG knowledge into §02, violating `impl-hygiene.md §SSOT`), §01 is reopened to fix the envelope at its source per CLAUDE.md §The One Rule.

**Gaps addressed:**
1. **No `:Bug` nodes emitted.** `parse_bug_entries()` yields full bug metadata (bug_id, title, severity, status, subsystem, source_file, lineno, excluded_reason, repro), but `export_json.py` only emits `HAS_BUG` edges with `{status, severity, lineno}` on the edge — no Bug nodes. Result: 112 dangling `HAS_BUG.end_id` references in the real corpus export.
2. **No `:Subsection` nodes emitted.** `PlanSectionSchema` and `FixBugSchema` both have a `sections:` frontmatter list defining subsections, but the exporter's `_EXCLUDED_FRONTMATTER_KEYS` explicitly drops `sections` and never synthesizes Subsection nodes or `HAS_SUBSECTION` edges. Result: `BLOCKED_BY`/`DEPENDS_ON` edges that target subsection IDs like `"02.6"` or `"03.2"` have no matching node.
3. **Label mismatch.** `_NODE_LABEL` maps `NodeKind.COMPLETED_INDEX → "CompletedPlan"`, but §02's schema uses `:CompletedIndex`. Either direction is fine; the schema's choice wins (matches the `NodeKind` name). Rename to `"CompletedIndex"`.
4. **Template-placeholder edge targets.** Unfilled template tokens — `<source-ref>`, `<target-ref>`, literal `ID`, `...`, `resolves=<target-ref>` — leak into `rel.end_id` as dangling strings. The exporter should filter these.

**Success criteria:**
- [x] `export_json.py` emits one `:Bug` node per `BugEntry` with properties `{bug_id, title, severity, severity_raw, status, subsystem, source_file, lineno, excluded_reason, repro, repo}`; `None`-valued fields omitted per existing `_normalize_property_value` rule.
- [x] `export_json.py` emits one `:Subsection` node per subsection entry in a `PlanSection` or `FixSection` `sections:` frontmatter list, with stable id `<section-path>#<subsection-id>` and properties `{title, status}`.
- [x] `HAS_SUBSECTION` edges from the containing `PlanSection`/`FixSection` to each of its `Subsection` nodes, with `properties: {"structural": true}`.
- [x] `_NODE_LABEL[NodeKind.COMPLETED_INDEX] == "CompletedIndex"` (was `"CompletedPlan"`).
- [x] Template-placeholder edge targets are filtered: edges whose `end_id` matches `_PLACEHOLDER_TARGETS` (exact set of tokens) OR contains angle brackets are dropped from the envelope.
- [x] Matrix tests added — `test_export_emits_bug_nodes_with_full_metadata`, `test_export_emits_subsection_nodes_and_has_subsection_edges`, `test_export_uses_completed_index_label` (negative pin on `CompletedPlan` absence), `test_export_filters_placeholder_edge_targets`. Each is a semantic pin with a matching negative assertion.
- [x] Full-corpus export is referentially closed on the structural edge types: `HAS_SECTION`, `HAS_OVERVIEW`, `HAS_BUG`, `HAS_SUBSECTION`, `FIXED_BY` each point at nodes that exist in the envelope's `nodes` list. (Verified via `diff <(jq -r '.nodes[].id' envelope.json | sort) <(jq -r '.relationships[] | select(.type | IN("HAS_BUG","HAS_SUBSECTION","HAS_SECTION","HAS_OVERVIEW","FIXED_BY")) | .end_id' envelope.json | sort -u)` returning every containment target present.) `DEPENDS_ON`/`BLOCKED_BY`/`REFERENCES`/`UNBLOCKS`/`SUPERSEDES` may still contain non-node targets — those are provenance-tagged references handled by §02's importer.
- [x] `pytest tests/plan-audit/test_export_json.py -v` green (existing 5 tests still pass + 4 new matrix tests — 9 total).
- [x] `python -m scripts.plan_corpus export` on the full corpus produces an envelope (1926 nodes, 2386 relationships) with zero dangling `HAS_BUG.end_id` / `HAS_SUBSECTION.end_id` / `FIXED_BY.start_id`.

**Invariant-anchoring:** The system invariant §01.7 enforces is *envelope referential closure on containment edges*. Its downstream consumer is §02.2's importer — a closed envelope means Phase 2's `MATCH (s {id: r.start_id}), (t {id: r.end_id})` succeeds for every containment edge. A Classification B blocker during §02.2 testing (importer silently drops HAS_BUG because end_id has no matching node) was the discovery that §01's exporter fails this invariant. Per CLAUDE.md §Stabilization Discipline the fix is in §01 (the DAG SSOT), not in §02 (a thin projection).

**Banned resolutions** (any of these = Inverted TDD / SSOT violation):
- Making §02 create Bug nodes from `HAS_BUG` edge properties (§02 would duplicate `parse_bug_entries` logic).
- Suppressing the referential-closure invariant by skipping HAS_BUG/HAS_SUBSECTION edges with unresolved targets (losing the edge = losing the fact).
- Putting the `CompletedPlan` alias into §02's importer (§02 should consume canonical labels).

### Tasks

- [x] Extend `_structural_relationships` (or a new `_synth_bug_nodes` / `_synth_subsection_nodes` pair) to:
  - [x] Emit Bug nodes from `parse_bug_entries(section_text, section_path.name)` on every `corpus.bug_sections` entry. Node `id = bug_entry.bug_id`, label `["Bug"]`, properties derived from the `BugEntry` dataclass (exclude `body_text` and `body_lines` — too large and not queryable).
  - [x] Emit Subsection nodes from each `PlanSectionSchema.sections` list and each `FixBugSchema.sections` list. Node id `<section-path>#<sub.id>`, label `["Subsection"]`, properties `{title, status}` from the dict.
  - [x] Emit `HAS_SUBSECTION` structural edges from the parent section to each Subsection.
- [x] Rename `_NODE_LABEL[NodeKind.COMPLETED_INDEX]` from `"CompletedPlan"` to `"CompletedIndex"`.
- [x] Add `_PLACEHOLDER_TARGETS: frozenset[str]` containing at minimum `{"<source-ref>", "<target-ref>", "ID", "...", "resolves=<target-ref>"}`. Added `_is_placeholder_target(end_id)` helper — True if string is in set OR contains both `<` and `>`. Filter applied at end of `export_neo4j_json` before `rels.sort`.
- [x] Add tests to `tests/plan-audit/test_export_json.py` (4 new test classes, all green).
- [x] Run existing test suite to verify no regression: `pytest tests/plan-audit/test_export_json.py tests/plan-audit/test_dag.py -v` → 37 passed.
- [x] Run full-corpus export: `python -m scripts.plan_corpus export --output /tmp/envelope2.json`; verified zero dangling HAS_BUG / HAS_SUBSECTION / FIXED_BY end_ids — all containment edges closed.

- [x] **Subsection close-out (01.7)** — MANDATORY before flipping §01 back to complete:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check`; clean any temp files.

---

## 01.N Completion Checklist

- [x] `scripts/plan_corpus/schemas.py`: `PlanSectionSchema` has `touches: list[str] | None = None` after `inspired_by`; `FixBugSchema` has `touches: list[str] | None = None` after `depends_on`
- [x] `scripts/plan_corpus/types.py`: `SourceKind.EXPLICIT_SUPERSEDES` and `SourceKind.EXPLICIT_REFERENCES` exist; docstring updated to name both edge-forming kinds
- [x] `scripts/plan_corpus/dag.py`: `_EDGE_KINDS = frozenset({EXPLICIT_DEPENDS_ON, EXPLICIT_SUPERSEDES})` exists; `Edge.__post_init__` uses it; `classify_redundant_dependency` filters to `EXPLICIT_DEPENDS_ON` in both loop positions; `apply_source_kind_severity` maps `EXPLICIT_SUPERSEDES → HIGH` and `EXPLICIT_REFERENCES → MEDIUM`
- [x] `scripts/plan_corpus/dag.py`: `_emit_edges_from_frontmatter_list` helper extracted; `deps_sources` loop refactored to call it; `supersedes_sources` loop added; `references_sources` loop added
- [x] `scripts/plan_corpus/export_json.py` exists with `export_neo4j_json(corpus, dag, *, include_references=True) -> dict` function; file stays under 500 lines
- [x] `scripts/plan_corpus/__main__.py`: `export` subcommand with `--output <path>` and `--no-references` flags works
- [x] `docs/internal/plan-schema-reference.md` regenerated: `python -m scripts.plan_corpus docgen --check` returns exit 0
- [x] `pytest tests/plan-audit/test_export_json.py` green (5 tests: node count, determinism, envelope schema keys, supersedes edge, references-only semantics)
- [x] `pytest tests/plan-audit/test_dag.py` green — no regression from `_EDGE_KINDS` guard change
- [x] `pytest tests/plan-audit/` green — full audit suite
- [x] `python -m scripts.plan_corpus check plans/plan-bug-dag-ingestion/section-01-plan-corpus-extension.md` returns 0 recon findings (this section has its recon block present)
- [x] `python -m scripts.plan_corpus check plans/` — no new schema violations introduced by `touches:` field (all existing files parse cleanly; field is optional)
- [x] `python -m scripts.plan_corpus export` emits valid JSON envelope with plausible node/relationship counts
- [x] Satisfies mission criterion: "scripts/plan_corpus/schemas.py exposes optional touches:..." (§01.1) and "scripts/plan_corpus/export_json.py serializes Corpus + Dag to a Neo4j-flavored JSON envelope..." (§01.4)
- [x] **Plan sync** — update plan metadata to reflect this section's completion:
  - [x] This section's frontmatter `status` → `complete`, all subsection statuses → `complete`
  - [x] `00-overview.md` Quick Reference table: Section 01 status → `Complete`
  - [x] `00-overview.md` mission success criteria: check off criterion 2 ("scripts/plan_corpus/schemas.py exposes optional touches:...") and criterion 3 ("scripts/plan_corpus/export_json.py serializes Corpus + Dag...")
  - [x] `index.md` Section 01 status → `Complete`
  - [x] Section 02's `depends_on: []` is correct — no stale assumptions from §01's work (§02 references the JSON envelope shape, which is now fixed)
- [x] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check`; clean any temp files before final commit.

**Exit Criteria:** `python -m scripts.plan_corpus docgen --check` returns exit 0; `pytest tests/plan-audit/test_export_json.py` passes all 5 tests; `pytest tests/plan-audit/test_dag.py` passes with no regressions; `python -m scripts.plan_corpus export` produces a valid, deterministic JSON envelope with `schema_version: "1.0"` and both `nodes` and `relationships` arrays populated; `./test-all.sh` green with 0 failures across all Rust test suites (§01 is Python-only, no Rust impact expected).
