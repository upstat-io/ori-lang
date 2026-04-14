#!/usr/bin/env python3
"""Tests for §02.4 Classifier Precedence & Determinism.

Pins:
- DEAD_REFERENCE > MISSING_DEPENDENCY on the same (source, line, target)
- CYCLE > BLOCKED on cycle members
- Deterministic order: running the full pipeline twice yields list equality
- Source-kind severity ladder: EXPLICIT_DEPENDS_ON=HIGH, HTML/YAML=MEDIUM,
  PROSE_VERB=LOW, CODE_FENCE_EXAMPLE=(no finding)
"""

from __future__ import annotations

import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT))

import pytest

from scripts.plan_corpus import FindingSubtype, FindingCategory, Severity, SourceKind
from scripts.plan_corpus import Finding
from scripts.plan_corpus.discovery import discover_corpus
from scripts.plan_corpus.dag import (
    apply_precedence,
    apply_source_kind_severity,
    build_dag,
    run_classifiers_with_precedence,
)


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


def _section(sec: str, title: str, status="not-started",
             body_extra="", depends_on=None) -> str:
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
        "---\n\n# body\n" + body_extra
    )


class TestPrecedenceDedup:
    def test_dead_ref_beats_missing_dep_on_same_line(self):
        """Synthetic: two findings with the same (source, line, target)
        differing only in subtype — DEAD_REFERENCE kept, MISSING_DEPENDENCY
        suppressed under evidence."""
        src = Path("plans/x/section-01.md")
        target = Path("plans/nonexistent/section-01.md")
        f_dead = Finding(
            category=FindingCategory.DEAD_REFERENCE,
            subtype=FindingSubtype.PLAN_DIRECTORY_NOT_FOUND,
            severity=Severity.HIGH,
            source=src,
            source_line=10,
            target=target,
            description="dead",
            recommended_fix="fix dead",
        )
        f_miss = Finding(
            category=FindingCategory.DAG_CONFLICT,
            subtype=FindingSubtype.MISSING_DEPENDENCY,
            severity=Severity.MEDIUM,
            source=src,
            source_line=10,
            target=target,
            description="miss",
            recommended_fix="fix miss",
        )

        deduped = apply_precedence([f_miss, f_dead])
        assert len(deduped) == 1
        kept = deduped[0]
        assert kept.subtype is FindingSubtype.PLAN_DIRECTORY_NOT_FOUND
        suppressed = [e for e in kept.evidence
                      if isinstance(e, str) and e.startswith("suppressed_by_precedence:")]
        assert suppressed
        assert "missing_dependency" in suppressed[0]

    def test_cycle_beats_blocked(self):
        src, tgt = Path("a.md"), Path("b.md")
        f_cycle = Finding(
            category=FindingCategory.DAG_CONFLICT,
            subtype=FindingSubtype.CYCLE,
            severity=Severity.HIGH,
            source=src, source_line=5, target=tgt,
            description="cycle", recommended_fix="fix",
        )
        f_blocked = Finding(
            category=FindingCategory.DAG_CONFLICT,
            subtype=FindingSubtype.BLOCKED,
            severity=Severity.HIGH,
            source=src, source_line=5, target=tgt,
            description="blocked", recommended_fix="fix",
        )
        deduped = apply_precedence([f_blocked, f_cycle])
        assert len(deduped) == 1
        assert deduped[0].subtype is FindingSubtype.CYCLE


class TestSourceKindSeverity:
    def test_explicit_depends_on_forces_high(self):
        src = Path("x.md")
        f = Finding(
            category=FindingCategory.DAG_CONFLICT,
            subtype=FindingSubtype.MISSING_DEPENDENCY,
            severity=Severity.LOW,  # wrong — should be promoted to HIGH
            source=src, source_line=1, target=None,
            description="d", recommended_fix="f",
            source_kind=SourceKind.EXPLICIT_DEPENDS_ON,
        )
        adjusted = apply_source_kind_severity([f])
        assert adjusted[0].severity is Severity.HIGH

    def test_prose_verb_forces_low(self):
        src = Path("x.md")
        f = Finding(
            category=FindingCategory.DAG_CONFLICT,
            subtype=FindingSubtype.MISSING_DEPENDENCY,
            severity=Severity.HIGH,  # wrong — should be demoted to LOW
            source=src, source_line=1, target=None,
            description="d", recommended_fix="f",
            source_kind=SourceKind.PROSE_VERB,
        )
        adjusted = apply_source_kind_severity([f])
        assert adjusted[0].severity is Severity.LOW

    def test_html_comment_forces_medium(self):
        src = Path("x.md")
        f = Finding(
            category=FindingCategory.DAG_CONFLICT,
            subtype=FindingSubtype.MISSING_DEPENDENCY,
            severity=Severity.LOW,
            source=src, source_line=1, target=None,
            description="d", recommended_fix="f",
            source_kind=SourceKind.HTML_COMMENT_CONVENTION,
        )
        adjusted = apply_source_kind_severity([f])
        assert adjusted[0].severity is Severity.MEDIUM

    def test_null_source_kind_untouched(self):
        src = Path("x.md")
        f = Finding(
            category=FindingCategory.DAG_CONFLICT,
            subtype=FindingSubtype.CONFLICT,
            severity=Severity.HIGH,
            source=src, source_line=1,
            description="d", recommended_fix="f",
            source_kind=None,
        )
        adjusted = apply_source_kind_severity([f])
        assert adjusted[0].severity is Severity.HIGH

    def test_dead_reference_severity_preserved_through_post_pass(self):
        """TPR-02-002-codex r2 regression pin: DEAD_REFERENCE findings carry
        their own severity ladder (completed-plan LOW; explicit-edge HIGH).
        apply_source_kind_severity must NOT overwrite classify_dead_
        reference's choices — case (h) specifies LOW severity for a
        completed-plan target regardless of source_kind."""
        src = Path("plans/roadmap/section-21A-llvm.md")
        f = Finding(
            category=FindingCategory.DEAD_REFERENCE,
            subtype=FindingSubtype.PLAN_DIRECTORY_NOT_FOUND,
            severity=Severity.LOW,  # intentional: completed-plan target
            source=src, source_line=83,
            description="reference points at completed plan jit-exception-handling",
            recommended_fix="Update or remove the stale annotation",
            source_kind=SourceKind.HTML_COMMENT_CONVENTION,
        )
        adjusted = apply_source_kind_severity([f])
        # HTML_COMMENT_CONVENTION would normally force MEDIUM, but
        # DEAD_REFERENCE is excluded from the ladder.
        assert adjusted[0].severity is Severity.LOW, (
            "DEAD_REFERENCE severity must be preserved through the post-pass; "
            "classify_dead_reference owns its own ladder"
        )


class TestDeterministicOrder:
    def test_full_pipeline_idempotent(self, tmp_path: Path):
        plans = tmp_path / "plans"
        _write(plans / "a" / "index.md", _index("A"))
        _write(plans / "a" / "section-01.md",
               _section("01", "a", depends_on=["B#01"]))
        _write(plans / "b" / "index.md", _index("B"))
        _write(plans / "b" / "section-01.md", _section("01", "b"))

        corpus = discover_corpus(root=plans)
        dag = build_dag(corpus)
        f1 = run_classifiers_with_precedence(dag, corpus)
        f2 = run_classifiers_with_precedence(dag, corpus)
        # Same (subtype, source, source_line, target) tuples in same order.
        key = lambda f: (f.subtype.value, str(f.source), f.source_line, str(f.target) if f.target else None)
        assert [key(f) for f in f1] == [key(f) for f in f2]


class TestLiveCorpusPrecedence:
    def test_live_corpus_precedence_pipeline_runs(self):
        from scripts.plan_corpus import PLANS_DIR
        corpus = discover_corpus(PLANS_DIR)
        dag = build_dag(corpus)
        findings = run_classifiers_with_precedence(dag, corpus)
        # Every kept finding must have valid category/subtype.
        from scripts.plan_corpus.types import _CATEGORY_SUBTYPES
        for f in findings:
            assert f.subtype in _CATEGORY_SUBTYPES[f.category]
