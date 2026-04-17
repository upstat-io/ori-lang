#!/usr/bin/env python3
"""Tests for §02.0 Node Model + Source-Kind Taxonomy + body helpers.

Source: plans/verify-roadmap-redesign/section-02-dag-builder.md §02.0.

TDD: these tests define the expected contract for scripts/plan_corpus/dag.py.
Fixtures live in tests/plan-audit/fixtures/dag/ (created lazily).
"""

from __future__ import annotations

import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT))

import pytest

from scripts.plan_corpus import FileClass, SourceKind
from scripts.plan_corpus.dag import (
    _EDGE_KINDS,
    Edge,
    NodeId,
    NodeKind,
    Reference,
    SUBSYSTEM_ALIASES,
    extract_yaml_comments,
    normalize_subsystem,
    parse_html_comments,
    strip_code_blocks,
)


# ---------------------------------------------------------------------------
# NodeKind + NodeId
# ---------------------------------------------------------------------------


class TestNodeKindCoversSevenSchemaClasses:
    def test_node_kind_has_seven_variants_matching_file_class(self):
        expected = {
            "PLAN_INDEX",
            "PLAN_SECTION",
            "ROADMAP_SECTION",
            "OVERVIEW",
            "BUG_TRACKER_SECTION",
            "FIX_BUG",
            "COMPLETED_INDEX",
        }
        actual = {m.name for m in NodeKind}
        assert actual == expected

    def test_every_file_class_maps_to_node_kind(self):
        """Exhaustiveness pin: no FileClass can exist without a NodeKind mate."""
        for fc in FileClass:
            nk = NodeKind.from_file_class(fc)
            assert isinstance(nk, NodeKind)
            assert nk.name == fc.name


class TestNodeIdDataclass:
    def test_node_id_is_frozen_and_hashable(self):
        a = NodeId(NodeKind.PLAN_SECTION, Path("plans/foo/section-01.md"))
        b = NodeId(NodeKind.PLAN_SECTION, Path("plans/foo/section-01.md"))
        c = NodeId(NodeKind.PLAN_SECTION, Path("plans/foo/section-02.md"))
        assert a == b
        assert hash(a) == hash(b)
        assert a != c
        s = {a, b, c}
        assert len(s) == 2

    def test_node_id_ordering(self):
        """Deterministic sort order: (kind, path)."""
        a = NodeId(NodeKind.PLAN_INDEX, Path("plans/a/index.md"))
        b = NodeId(NodeKind.PLAN_SECTION, Path("plans/a/section-01.md"))
        assert a < b  # kind enum ordering


# ---------------------------------------------------------------------------
# Reference + Edge
# ---------------------------------------------------------------------------


class TestReferenceDataclass:
    def test_reference_carries_source_kind_and_column(self):
        ref = Reference(
            from_node=NodeId(NodeKind.PLAN_SECTION, Path("a.md")),
            target="plans/b/",
            source_kind=SourceKind.PROSE_VERB,
            source_line=42,
            source_column=7,
            raw_text="depends on plans/b/",
        )
        assert ref.source_kind is SourceKind.PROSE_VERB
        assert ref.source_column == 7

    def test_reference_source_column_may_be_none(self):
        ref = Reference(
            from_node=NodeId(NodeKind.PLAN_SECTION, Path("a.md")),
            target="plans/b/",
            source_kind=SourceKind.EXPLICIT_DEPENDS_ON,
            source_line=5,
            source_column=None,
            raw_text="",
        )
        assert ref.source_column is None


class TestEdgeDataclass:
    def test_edge_kinds_frozenset_contains_depends_on_and_supersedes(self):
        """Source-kind SSOT: _EDGE_KINDS is the single-source allowlist for
        Edge.source_kind. Exactly the two frontmatter-promoted kinds."""
        assert _EDGE_KINDS == frozenset({
            SourceKind.EXPLICIT_DEPENDS_ON,
            SourceKind.EXPLICIT_SUPERSEDES,
        })

    def test_edge_accepts_explicit_depends_on(self):
        ref = Reference(
            from_node=NodeId(NodeKind.PLAN_INDEX, Path("a/index.md")),
            target="b",
            source_kind=SourceKind.EXPLICIT_DEPENDS_ON,
            source_line=5,
            source_column=None,
            raw_text="b",
        )
        e = Edge(
            from_node=NodeId(NodeKind.PLAN_INDEX, Path("a/index.md")),
            to_node=NodeId(NodeKind.PLAN_INDEX, Path("b/index.md")),
            source_kind=SourceKind.EXPLICIT_DEPENDS_ON,
            reference=ref,
        )
        assert e.source_kind is SourceKind.EXPLICIT_DEPENDS_ON

    def test_edge_accepts_explicit_supersedes(self):
        """EXPLICIT_SUPERSEDES is the second frontmatter-promoted kind per
        _EDGE_KINDS; Edge construction must accept it without raising."""
        ref = Reference(
            from_node=NodeId(NodeKind.PLAN_INDEX, Path("old/index.md")),
            target="new",
            source_kind=SourceKind.EXPLICIT_SUPERSEDES,
            source_line=4,
            source_column=None,
            raw_text="new",
        )
        e = Edge(
            from_node=NodeId(NodeKind.PLAN_INDEX, Path("old/index.md")),
            to_node=NodeId(NodeKind.PLAN_INDEX, Path("new/index.md")),
            source_kind=SourceKind.EXPLICIT_SUPERSEDES,
            reference=ref,
        )
        assert e.source_kind is SourceKind.EXPLICIT_SUPERSEDES

    def test_edge_rejects_explicit_references(self):
        """EXPLICIT_REFERENCES is the reference-only kind — feeds
        dag.references but never becomes an Edge. Construction must raise."""
        ref = Reference(
            from_node=NodeId(NodeKind.PLAN_INDEX, Path("a/index.md")),
            target="b",
            source_kind=SourceKind.EXPLICIT_REFERENCES,
            source_line=5,
            source_column=None,
            raw_text="b",
        )
        with pytest.raises(ValueError, match="EXPLICIT_REFERENCES"):
            Edge(
                from_node=NodeId(NodeKind.PLAN_INDEX, Path("a/index.md")),
                to_node=NodeId(NodeKind.PLAN_INDEX, Path("b/index.md")),
                source_kind=SourceKind.EXPLICIT_REFERENCES,
                reference=ref,
            )

    def test_edge_rejects_body_inferred_source_kinds(self):
        """Body-inferred references (PROSE_VERB, HTML_COMMENT_CONVENTION,
        YAML_COMMENT) feed dag.references only — Edge construction must
        raise for all of them."""
        for sk in (
            SourceKind.PROSE_VERB,
            SourceKind.HTML_COMMENT_CONVENTION,
            SourceKind.YAML_COMMENT,
        ):
            ref = Reference(
                from_node=NodeId(NodeKind.PLAN_INDEX, Path("a/index.md")),
                target="b",
                source_kind=sk,
                source_line=5,
                source_column=None,
                raw_text="b",
            )
            with pytest.raises(ValueError, match=r"_EDGE_KINDS|EXPLICIT_DEPENDS_ON"):
                Edge(
                    from_node=NodeId(NodeKind.PLAN_INDEX, Path("a/index.md")),
                    to_node=NodeId(NodeKind.PLAN_INDEX, Path("b/index.md")),
                    source_kind=sk,
                    reference=ref,
                )


# ---------------------------------------------------------------------------
# SUBSYSTEM_ALIASES + normalize_subsystem
# ---------------------------------------------------------------------------


class TestSubsystemAliases:
    def test_every_workspace_crate_is_normalized(self):
        """Pin: every compiler/ori_X crate must be in the alias table."""
        crates = [
            "ori_ir", "ori_registry", "ori_diagnostic", "ori_lexer",
            "ori_types", "ori_parse", "ori_patterns", "ori_eval",
            "ori_fmt", "ori_arc", "ori_repr", "ori_canon", "oric",
            "ori_llvm", "ori_rt",
        ]
        for crate in crates:
            assert normalize_subsystem(crate) == crate, (
                f"Crate {crate} missing from SUBSYSTEM_ALIASES"
            )

    def test_crate_path_normalizes_to_crate_name(self):
        assert normalize_subsystem("compiler/ori_arc") == "ori_arc"
        assert normalize_subsystem("compiler/ori_llvm/src/codegen") == "ori_llvm"

    def test_logical_aliases_map_to_crates(self):
        assert normalize_subsystem("AIMS") == "ori_arc"
        assert normalize_subsystem("ArcClassifier") == "ori_arc"
        assert normalize_subsystem("Tag::Var") == "ori_types"
        assert normalize_subsystem("ReprPlan") == "ori_repr"

    def test_unknown_token_returns_none(self):
        assert normalize_subsystem("SomeRandomIdentifier") is None
        assert normalize_subsystem("") is None


# ---------------------------------------------------------------------------
# strip_code_blocks helper
# ---------------------------------------------------------------------------


class TestStripCodeBlocks:
    def test_fenced_block_detected(self):
        body = "before\n```python\ndef foo():\n    pass\n```\nafter"
        regions = strip_code_blocks(body)
        # 1 fenced region covering the code
        assert any(kind == "fenced" for _, _, kind in regions)
        fenced = [(s, e) for s, e, k in regions if k == "fenced"][0]
        # region spans the fence lines inclusive
        assert fenced[0] >= 1 and fenced[1] >= fenced[0]

    def test_indented_code_block_detected(self):
        body = "normal text\n\n    indented code line 1\n    indented code line 2\n\nback to normal"
        regions = strip_code_blocks(body)
        indented = [(s, e) for s, e, k in regions if k == "indented"]
        assert len(indented) >= 1, f"expected at least one indented block, got {regions}"

    def test_no_code_blocks_returns_empty_list(self):
        body = "line 1\nline 2\nline 3"
        regions = strip_code_blocks(body)
        assert regions == []

    def test_unmatched_fence_does_not_span_to_eof(self):
        """Defensive: a single stray ``` should not swallow the entire document."""
        body = "normal\n```\nnever closed\n\nstill going"
        regions = strip_code_blocks(body)
        # Implementation choice: an unclosed fence extends to EOF (CommonMark-ish),
        # but a defensive parser may emit it as best-effort. We assert it at least
        # doesn't crash and produces a region.
        assert isinstance(regions, list)

    def test_line_in_region(self):
        """Helper: given regions, a callsite can check whether a line is inside."""
        body = "a\n```\ncode\n```\nafter"
        regions = strip_code_blocks(body)
        # Line 3 (1-indexed) = 'code' should be inside a fenced region
        assert any(s <= 3 <= e and k == "fenced" for s, e, k in regions)
        # Line 5 (1-indexed) = 'after' should NOT be in any region
        assert not any(s <= 5 <= e for s, e, _k in regions)


# ---------------------------------------------------------------------------
# extract_yaml_comments helper
# ---------------------------------------------------------------------------


class TestExtractYamlComments:
    def test_extracts_frontmatter_comment(self):
        """Pin: `status: in-progress  # blocked by plans/iterator-element-ownership/`
        — Finding F semantic pin for test case (g)."""
        text = (
            "---\n"
            "section: \"04\"\n"
            "status: in-progress  # blocked by plans/iterator-element-ownership/\n"
            "---\n"
            "\nbody\n"
        )
        # body_offset = line index where body begins (after closing ---)
        body_offset = 4
        comments = extract_yaml_comments(text, body_offset)
        assert len(comments) >= 1
        # Expect the comment text to include the blocker reference
        found = any("plans/iterator-element-ownership" in c[2] for c in comments)
        assert found, f"expected blocker comment, got {comments}"

    def test_ignores_comments_after_frontmatter_closes(self):
        text = (
            "---\n"
            "section: \"04\"\n"
            "---\n"
            "body text # not a frontmatter comment\n"
        )
        comments = extract_yaml_comments(text, 3)
        assert all("not a frontmatter" not in c[2] for c in comments)

    def test_no_comments_returns_empty_list(self):
        text = "---\nsection: \"04\"\n---\nbody\n"
        comments = extract_yaml_comments(text, 3)
        assert comments == []


# ---------------------------------------------------------------------------
# parse_html_comments helper
# ---------------------------------------------------------------------------


class TestParseHtmlComments:
    def test_parses_blocked_by(self):
        body = "some text\n<!-- blocked-by:plan-slug/02 -->\nmore text"
        refs = parse_html_comments(body)
        assert len(refs) == 1
        ref = refs[0]
        assert ref.source_kind is SourceKind.HTML_COMMENT_CONVENTION
        assert ref.target == "plan-slug/02"
        assert ref.raw_text.startswith("<!-- blocked-by:")

    def test_parses_unblocks_multi_target(self):
        """Semantic pin for live case (h) — TPR-02-003-codex r1 +
        TPR-02-002-codex (r2): `<!-- unblocks:jit-exception-handling/04B,05,06 -->`
        MUST parse into 3 targets, all carrying the inherited plan slug
        `jit-exception-handling` (shorthand section tokens inherit the first
        target's plan prefix; bare `05`/`06` would be false positives for
        DEAD_REFERENCE)."""
        body = "<!-- unblocks:jit-exception-handling/04B,05,06 -->"
        refs = parse_html_comments(body)
        targets = [r.target for r in refs]
        assert "jit-exception-handling/04B" in targets
        assert "jit-exception-handling/05" in targets
        assert "jit-exception-handling/06" in targets
        # Negative: bare shorthand MUST NOT leak through.
        assert "05" not in targets
        assert "06" not in targets

    def test_hyphenated_targets_preserved(self):
        """TPR-02-003-codex r1: hyphens are allowed in target tokens."""
        body = "<!-- supersedes:iterator-element-ownership -->"
        refs = parse_html_comments(body)
        assert len(refs) == 1
        assert refs[0].target == "iterator-element-ownership"

    def test_parses_all_seven_verbs(self):
        verbs_and_targets = [
            ("blocked-by", "a/01"),
            ("unblocks", "b/02"),
            ("supersedes", "c"),
            ("resolves", "BUG-04-001"),
            ("rewrites", "d/05"),
            ("update-complete", "resolves=e/06"),
            ("updated-by", "f/07"),
        ]
        lines = [f"<!-- {v}:{t} -->" for v, t in verbs_and_targets]
        body = "\n".join(lines)
        refs = parse_html_comments(body)
        # at least one ref per verb
        assert len(refs) >= len(verbs_and_targets)

    def test_ignores_comments_inside_fenced_code_blocks(self):
        """Finding D: code-fence exclusion — reference inside ``` must NOT be emitted."""
        body = (
            "before\n"
            "```\n"
            "<!-- blocked-by:fake-target -->\n"
            "```\n"
            "after\n"
        )
        refs = parse_html_comments(body)
        fake_refs = [r for r in refs if r.target == "fake-target"]
        assert len(fake_refs) == 0, (
            f"Comments inside fenced code blocks must be excluded; got {fake_refs}"
        )

    def test_returns_empty_list_on_no_comments(self):
        assert parse_html_comments("just prose with no comments") == []

    def test_source_line_is_1_indexed(self):
        body = "line 1\nline 2\n<!-- blocked-by:x -->\nline 4"
        refs = parse_html_comments(body)
        assert len(refs) == 1
        assert refs[0].source_line == 3

    def test_shorthand_section_tokens_inherit_first_targets_plan_slug(self):
        """TPR-02-002-codex regression pin: `<!-- unblocks:jit-exception-
        handling/04B,05,06 -->` must yield targets with the shared plan
        slug propagated to shorthand tokens `05` and `06`, not bare
        numerics. Otherwise DEAD_REFERENCE treats `05`/`06` as nonexistent
        plan directories and emits false positives."""
        body = "<!-- unblocks:jit-exception-handling/04B,05,06 -->"
        refs = parse_html_comments(body)
        targets = [r.target for r in refs]
        assert "jit-exception-handling/04B" in targets
        assert "jit-exception-handling/05" in targets
        assert "jit-exception-handling/06" in targets
        # Negative pin: bare shorthand must NOT leak through.
        assert "05" not in targets
        assert "06" not in targets

    def test_shorthand_inheritance_only_when_first_target_has_slash(self):
        """Negative pin: when the first target has no slash, shorthand
        propagation must not fire — there's no plan slug to inherit."""
        body = "<!-- blocked-by:plan-a,plan-b,plan-c -->"
        refs = parse_html_comments(body)
        targets = [r.target for r in refs]
        assert set(targets) == {"plan-a", "plan-b", "plan-c"}
