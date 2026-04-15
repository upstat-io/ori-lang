#!/usr/bin/env python3
"""Tests for §02.0 types.py extensions — SourceKind, new subtypes, new Finding fields.

TDD per CLAUDE.md §TDD and plans/verify-roadmap-redesign/section-02-dag-builder.md
§02.N "Cross-section edits to §01.3's scripts/plan_corpus/types.py" (all authorized
extensions must land together).

These tests define the expected post-extension contract:
  - SourceKind(Enum) lives in scripts.plan_corpus.types alongside Finding (TPR-02-001-gemini-r2)
  - FindingSubtype.REDUNDANT_DEPENDENCY and ORPHANED_PLAN live in DAG_CONFLICT
  - Finding.dependency_chain: tuple[Path, ...] = () (Option A — TPR-02-006-codex)
  - Finding.source_kind: SourceKind | None = None (Option A — TPR-02-006-codex)
  - Finding.source_column: int | None = None (Concern J disambiguator)
  - Finding.id hash rebased to include source_column + target CONDITIONALLY
    (backward-compat: unchanged when defaults are None)
"""

from __future__ import annotations

import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT))

import pytest

from scripts.plan_corpus import (
    Finding,
    FindingCategory,
    FindingSubtype,
    Severity,
    SourceKind,
    _CATEGORY_SUBTYPES,
)


class TestSourceKindEnum:
    """SourceKind lives in types.py to avoid circular import with dag.py."""

    def test_source_kind_has_five_canonical_variants(self):
        expected = {
            "EXPLICIT_DEPENDS_ON",
            "HTML_COMMENT_CONVENTION",
            "YAML_COMMENT",
            "PROSE_VERB",
            "CODE_FENCE_EXAMPLE",
        }
        actual = {member.name for member in SourceKind}
        assert actual == expected, (
            f"SourceKind variants drift from §02.0 spec. "
            f"Expected {expected}, got {actual}"
        )

    def test_source_kind_is_importable_from_package_root(self):
        from scripts.plan_corpus import SourceKind as Sk
        assert Sk is SourceKind


class TestDagConflictNewSubtypes:
    """REDUNDANT_DEPENDENCY and ORPHANED_PLAN are authorized §02.2 additions."""

    def test_redundant_dependency_subtype_registered_in_dag_conflict(self):
        assert FindingSubtype.REDUNDANT_DEPENDENCY in _CATEGORY_SUBTYPES[
            FindingCategory.DAG_CONFLICT
        ]

    def test_orphaned_plan_subtype_registered_in_dag_conflict(self):
        assert FindingSubtype.ORPHANED_PLAN in _CATEGORY_SUBTYPES[
            FindingCategory.DAG_CONFLICT
        ]

    def test_new_subtypes_reject_other_categories(self):
        """Negative pin: cannot attach REDUNDANT_DEPENDENCY to PARSE_ERROR."""
        with pytest.raises(ValueError, match="does not belong"):
            Finding(
                category=FindingCategory.PARSE_ERROR,
                subtype=FindingSubtype.REDUNDANT_DEPENDENCY,
                severity=Severity.LOW,
                source=Path("test.md"),
                description="test",
                recommended_fix="fix",
            )


class TestFindingNewFields:
    """Option A: typed dependency_chain + source_kind + source_column."""

    def test_finding_accepts_dependency_chain_tuple_of_paths(self):
        f = Finding(
            category=FindingCategory.DAG_CONFLICT,
            subtype=FindingSubtype.CYCLE,
            severity=Severity.HIGH,
            source=Path("a.md"),
            description="cycle",
            recommended_fix="break cycle",
            dependency_chain=(Path("a.md"), Path("b.md"), Path("a.md")),
            source_kind=SourceKind.EXPLICIT_DEPENDS_ON,
        )
        assert f.dependency_chain == (Path("a.md"), Path("b.md"), Path("a.md"))
        assert f.source_kind is SourceKind.EXPLICIT_DEPENDS_ON

    def test_finding_dependency_chain_defaults_empty_tuple(self):
        f = Finding(
            category=FindingCategory.PARSE_ERROR,
            subtype=FindingSubtype.YAML_SYNTAX_ERROR,
            severity=Severity.HIGH,
            source=Path("t.md"),
            description="t",
            recommended_fix="f",
        )
        assert f.dependency_chain == ()
        assert f.source_kind is None

    def test_finding_source_column_defaults_none(self):
        f = Finding(
            category=FindingCategory.PARSE_ERROR,
            subtype=FindingSubtype.YAML_SYNTAX_ERROR,
            severity=Severity.HIGH,
            source=Path("t.md"),
            description="t",
            recommended_fix="f",
        )
        assert f.source_column is None

    def test_finding_source_column_accepts_int(self):
        f = Finding(
            category=FindingCategory.DAG_CONFLICT,
            subtype=FindingSubtype.MISSING_DEPENDENCY,
            severity=Severity.MEDIUM,
            source=Path("x.md"),
            description="x",
            recommended_fix="y",
            source_line=42,
            source_column=7,
        )
        assert f.source_column == 7


class TestFindingIdBackwardCompatibility:
    """Finding.id rebased to source_column + target ONLY when non-None (TPR-02-003-codex-r2)."""

    def test_id_unchanged_when_new_fields_are_defaults(self):
        """Pin: Finding.id with default source_column=None and target=None must
        match the pre-extension hash (category:subtype:source:source_line)."""
        import hashlib
        f = Finding(
            category=FindingCategory.PARSE_ERROR,
            subtype=FindingSubtype.YAML_SYNTAX_ERROR,
            severity=Severity.HIGH,
            source=Path("legacy.md"),
            description="desc",
            recommended_fix="fix",
            source_line=10,
        )
        legacy_content = f"parse_error:yaml_syntax_error:legacy.md:10"
        expected = "VR-" + hashlib.sha256(legacy_content.encode()).hexdigest()[:6]
        assert f.id == expected, (
            "Finding.id must be backward-compatible when source_column/target "
            "are at their defaults (None)"
        )

    def test_id_distinct_when_source_column_differs(self):
        """Collision guard (Concern J): two findings on the same line but
        different columns must have distinct ids."""
        f1 = Finding(
            category=FindingCategory.DAG_CONFLICT,
            subtype=FindingSubtype.MISSING_DEPENDENCY,
            severity=Severity.MEDIUM,
            source=Path("x.md"),
            description="a",
            recommended_fix="a",
            source_line=5,
            source_column=1,
        )
        f2 = Finding(
            category=FindingCategory.DAG_CONFLICT,
            subtype=FindingSubtype.MISSING_DEPENDENCY,
            severity=Severity.MEDIUM,
            source=Path("x.md"),
            description="b",
            recommended_fix="b",
            source_line=5,
            source_column=42,
        )
        assert f1.id != f2.id

    def test_id_distinct_when_target_differs(self):
        """Finding.id incorporates target so multiple dep edges on same line
        get distinct ids."""
        f1 = Finding(
            category=FindingCategory.DAG_CONFLICT,
            subtype=FindingSubtype.BLOCKED,
            severity=Severity.HIGH,
            source=Path("A.md"),
            description="a",
            recommended_fix="a",
            source_line=5,
            target=Path("B.md"),
        )
        f2 = Finding(
            category=FindingCategory.DAG_CONFLICT,
            subtype=FindingSubtype.BLOCKED,
            severity=Severity.HIGH,
            source=Path("A.md"),
            description="a",
            recommended_fix="a",
            source_line=5,
            target=Path("C.md"),
        )
        assert f1.id != f2.id


class TestExhaustivenessPinsStillHold:
    """§01.5 exhaustiveness pins must still pass after adding new subtypes."""

    def test_every_subtype_registered_in_category_map(self):
        all_registered = set()
        for subtypes in _CATEGORY_SUBTYPES.values():
            all_registered |= subtypes
        for member in FindingSubtype:
            assert member in all_registered, (
                f"FindingSubtype.{member.name} not registered"
            )

    def test_every_category_has_subtypes(self):
        for member in FindingCategory:
            assert member in _CATEGORY_SUBTYPES
