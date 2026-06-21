"""Tests for diagnostics/path_to_node_index.py — the .ori path -> node index.

Covers:
  - a .ori referenced in a plan section content body attributes to that section;
  - a bug-plan TDD .ori attributes to the bug;
  - an unreferenced .ori maps to None.
  - frontmatter touches:/owns_crates: prefix coverage attributes by prefix;
  - explicit content reference beats a competing prefix match.
Hermetic tmp_path fixtures only — never depends on the live repo plan tree.
"""

import sys
from pathlib import Path

import pytest

_DIAG = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(_DIAG))

import path_to_node_index  # noqa: E402


@pytest.fixture
def tree(tmp_path):
    plans = tmp_path / "plans"
    bug_plans = tmp_path / "bug-tracker" / "plans"

    # Plan section content referencing a .ori (section id encoded in filename).
    sec = plans / "feature-x" / "content"
    sec.mkdir(parents=True)
    (sec / "core--s-1234ab.md").write_text(
        "# s-01 core\n\nOwns `compiler_repo/tests/spec/featx/main.ori`.\n",
        encoding="utf-8",
    )

    # Plan section using frontmatter touches: prefix coverage (no explicit ref).
    sec2 = plans / "feature-y" / "content"
    sec2.mkdir(parents=True)
    (sec2 / "lower--s-99ff.md").write_text(
        "---\n"
        "touches:\n"
        "  - compiler_repo/tests/spec/featy/\n"
        "owns_crates: []\n"
        "---\n\n# s-02 lower\n\nNo explicit .ori reference here.\n",
        encoding="utf-8",
    )

    # Bug plan TDD .ori.
    bug = bug_plans / "BUG-05-077"
    bug.mkdir(parents=True)
    (bug / "section-03-tdd-matrix.md").write_text(
        "# matrix\n\ncompiler_repo/tests/spec/regress/bug05.ori\n",
        encoding="utf-8",
    )

    return {
        "plans_root": str(plans),
        "bug_plans_root": str(bug_plans),
    }


def _ptn(tree, ori):
    return path_to_node_index.path_to_node(
        ori, plans_root=tree["plans_root"], bug_plans_root=tree["bug_plans_root"])


def test_content_reference_attributes_to_section(tree):
    assert _ptn(tree, "compiler_repo/tests/spec/featx/main.ori") == "s-1234ab"


def test_bug_plan_tdd_attributes_to_bug(tree):
    assert _ptn(tree, "compiler_repo/tests/spec/regress/bug05.ori") == "BUG-05-077"


def test_unreferenced_ori_maps_to_none(tree):
    assert _ptn(tree, "compiler_repo/tests/spec/nowhere/lost.ori") is None


def test_empty_path_maps_to_none(tree):
    assert _ptn(tree, "") is None


def test_frontmatter_prefix_coverage(tree):
    # A .ori under a touches: prefix directory, with NO explicit reference,
    # attributes to that section by prefix coverage.
    assert _ptn(tree, "compiler_repo/tests/spec/featy/deep/case.ori") == "s-99ff"


def test_directory_reference_owns_children(tree):
    # The content body for featx names the exact file; a sibling under the same
    # directory is NOT owned by that body (it names the file, not the dir), so
    # it falls through to None here.
    assert _ptn(tree, "compiler_repo/tests/spec/featx/sibling.ori") is None


def test_explicit_reference_beats_prefix(tmp_path):
    # One plan references the .ori explicitly; another plan's touches: prefix
    # would also cover it. Explicit content reference wins.
    plans = tmp_path / "plans"
    explicit = plans / "explicit-owner" / "content"
    explicit.mkdir(parents=True)
    (explicit / "own--s-ab12cd.md").write_text(
        "Owns `compiler_repo/tests/spec/shared/x.ori`.\n", encoding="utf-8")
    prefix = plans / "prefix-owner" / "content"
    prefix.mkdir(parents=True)
    (prefix / "cover--s-99ee.md").write_text(
        "---\ntouches:\n  - compiler_repo/tests/spec/shared/\n---\n",
        encoding="utf-8")
    bug_plans = tmp_path / "bug-tracker" / "plans"
    bug_plans.mkdir(parents=True)
    node = path_to_node_index.path_to_node(
        "compiler_repo/tests/spec/shared/x.ori",
        plans_root=str(plans), bug_plans_root=str(bug_plans))
    assert node == "s-ab12cd"


def test_build_index_omits_unattributable(tree):
    paths = [
        "compiler_repo/tests/spec/featx/main.ori",
        "compiler_repo/tests/spec/regress/bug05.ori",
        "compiler_repo/tests/spec/nowhere/lost.ori",
    ]
    index = path_to_node_index.build_index(
        paths, plans_root=tree["plans_root"], bug_plans_root=tree["bug_plans_root"])
    assert index == {
        "compiler_repo/tests/spec/featx/main.ori": "s-1234ab",
        "compiler_repo/tests/spec/regress/bug05.ori": "BUG-05-077",
    }
    assert "compiler_repo/tests/spec/nowhere/lost.ori" not in index


def test_plan_level_node_when_no_section_id(tmp_path):
    # A content file whose name does NOT encode a section id attributes to the
    # plan directory name (plan-level node).
    plans = tmp_path / "plans"
    content = plans / "plain-plan" / "content"
    content.mkdir(parents=True)
    (content / "overview.md").write_text(
        "Owns compiler_repo/tests/spec/plain/y.ori\n", encoding="utf-8")
    bug_plans = tmp_path / "bug-tracker" / "plans"
    bug_plans.mkdir(parents=True)
    node = path_to_node_index.path_to_node(
        "compiler_repo/tests/spec/plain/y.ori",
        plans_root=str(plans), bug_plans_root=str(bug_plans))
    assert node == "plain-plan"
