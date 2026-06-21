"""Tests for diagnostics/per_node_verdict.py — the per-node verdict builder.

Covers the split the consumer (verify_and_complete_v7) engine-reads:
  (a) LLVM-red .ori -> owning node carries parity=false;
  (b) leaking-but-parity-green .ori -> leak_free=false, never silently green;
  (c) non-roadmap plan .ori AND bug-plan TDD .ori each attribute to their
      owning node (plans/ + bug-tracker/plans/), hermetic via tmp_path;
  (d) unattributable failure -> parity="unknown" AND leak_free="unknown",
      fail-closed, never silently green.
"""

import sys
from pathlib import Path

import pytest

_DIAG = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(_DIAG))

import per_node_verdict  # noqa: E402
import path_to_node_index  # noqa: E402


def _failure(suite, source_path="", leak_positive=False):
    return {
        "test_id": "t",
        "test_id_kind": "ori_spec",
        "suite": suite,
        "failure_kind": "assertion_failure",
        "error_message": "boom",
        "source_path": source_path,
        "leak_positive": leak_positive,
    }


# --- (a) LLVM-red .ori -> parity=false on the owning node ------------------

def test_llvm_red_node_parity_false():
    ori = "compiler_repo/tests/spec/foo.ori"
    index = {ori: "s-deadbeef"}
    failures = [_failure("ori_llvm", source_path=ori)]
    block = per_node_verdict.build_per_node_verdict(
        failures, index_fn=lambda p: index.get(p))
    assert block["s-deadbeef"]["parity"] is False
    assert block["s-deadbeef"]["llvm_pass"] is False
    assert block["s-deadbeef"]["interp_pass"] is True
    # No leak signal on a pure-LLVM-compile/assertion failure -> leak_free true.
    assert block["s-deadbeef"]["leak_free"] is True


def test_both_legs_green_for_other_nodes_absent():
    # A node with zero attributed failures does not appear (consumer reads
    # absent == green). Only the red node materializes.
    ori = "compiler_repo/tests/spec/foo.ori"
    index = {ori: "s-deadbeef"}
    block = per_node_verdict.build_per_node_verdict(
        [_failure("ori_llvm", source_path=ori)], index_fn=lambda p: index.get(p))
    assert list(block.keys()) == ["s-deadbeef"]


def test_interp_red_node_parity_still_keys_on_llvm():
    # An interp-only failure leaves llvm_pass True -> parity true (the LLVM
    # leg is the parity driver per the documented tri-state).
    ori = "compiler_repo/tests/spec/bar.ori"
    index = {ori: "s-aa11"}
    block = per_node_verdict.build_per_node_verdict(
        [_failure("ori_interp", source_path=ori)], index_fn=lambda p: index.get(p))
    assert block["s-aa11"]["interp_pass"] is False
    assert block["s-aa11"]["llvm_pass"] is True
    assert block["s-aa11"]["parity"] is True


# --- (b) leaking-but-parity-green .ori -> leak_free=false ------------------

def test_leak_positive_parity_green_surfaces_leak_free_false():
    # A leak-positive failure with no LLVM-leg divergence: parity stays green
    # (llvm_pass True) but leak_free MUST be false, never silently green.
    ori = "compiler_repo/tests/spec/leaky.ori"
    index = {ori: "s-leak01"}
    # Leak surfaced on the interp leg (parity not divergent), leak_positive set.
    failures = [_failure("ori_interp", source_path=ori, leak_positive=True)]
    block = per_node_verdict.build_per_node_verdict(
        failures, index_fn=lambda p: index.get(p))
    node = block["s-leak01"]
    assert node["leak_free"] is False
    # parity is driven by the LLVM leg, which is green here.
    assert node["llvm_pass"] is True
    assert node["parity"] is True
    # The crucial property: leak surfaces, node is NOT silently all-green.
    assert node["leak_free"] is not True


def test_aot_leak_attributed_node_leak_free_false():
    # A leak surfacing on a non-spec suite (aot) that still attributes to a
    # node drives leak_free false while leaving both spec legs green.
    ori = "compiler_repo/tests/valgrind/x.ori"
    index = {ori: "BUG-04-194"}
    failures = [_failure("aot", source_path=ori, leak_positive=True)]
    block = per_node_verdict.build_per_node_verdict(
        failures, index_fn=lambda p: index.get(p))
    node = block["BUG-04-194"]
    assert node["leak_free"] is False
    assert node["parity"] is True  # neither spec leg red


# --- (c) plan + bug-plan attribution, hermetic ----------------------------

@pytest.fixture
def fake_tree(tmp_path):
    """Build a tiny hermetic plans/ + bug-tracker/plans/ tree:
      - a NON-roadmap plan whose section content references a .ori;
      - a bug plan whose content references a TDD .ori.
    """
    plans = tmp_path / "plans"
    bug_plans = tmp_path / "bug-tracker" / "plans"

    # Non-roadmap plan with a content file encoding section id s-abc123.
    feat_content = plans / "my-feature" / "content"
    feat_content.mkdir(parents=True)
    (feat_content / "impl--s-abc123.md").write_text(
        "# s-01 impl\n\nThis section owns "
        "`compiler_repo/tests/spec/myfeature/case.ori` and its sibling.\n",
        encoding="utf-8",
    )

    # Bug plan with a TDD .ori referenced in section content.
    bug_dir = bug_plans / "BUG-09-001"
    bug_dir.mkdir(parents=True)
    (bug_dir / "section-03-tdd-matrix.md").write_text(
        "# TDD matrix\n\nRegression test: "
        "compiler_repo/tests/spec/regress/bug09.ori pins the fix.\n",
        encoding="utf-8",
    )

    return {
        "plans_root": str(plans),
        "bug_plans_root": str(bug_plans),
        "feat_ori": "compiler_repo/tests/spec/myfeature/case.ori",
        "bug_ori": "compiler_repo/tests/spec/regress/bug09.ori",
    }


def test_non_roadmap_plan_ori_attributes_to_section(fake_tree):
    node = path_to_node_index.path_to_node(
        fake_tree["feat_ori"],
        plans_root=fake_tree["plans_root"],
        bug_plans_root=fake_tree["bug_plans_root"],
    )
    assert node == "s-abc123"


def test_bug_plan_tdd_ori_attributes_to_bug(fake_tree):
    node = path_to_node_index.path_to_node(
        fake_tree["bug_ori"],
        plans_root=fake_tree["plans_root"],
        bug_plans_root=fake_tree["bug_plans_root"],
    )
    assert node == "BUG-09-001"


def test_per_node_verdict_through_real_index(fake_tree):
    # End-to-end: failures attribute through the real path_to_node index over
    # the hermetic tree, NOT to the roadmap.
    def index_fn(p):
        return path_to_node_index.path_to_node(
            p,
            plans_root=fake_tree["plans_root"],
            bug_plans_root=fake_tree["bug_plans_root"],
        )

    failures = [
        _failure("ori_llvm", source_path=fake_tree["feat_ori"]),
        _failure("ori_interp", source_path=fake_tree["bug_ori"],
                 leak_positive=True),
    ]
    block = per_node_verdict.build_per_node_verdict(failures, index_fn=index_fn)
    assert block["s-abc123"]["parity"] is False  # LLVM-red feature section
    assert block["BUG-09-001"]["leak_free"] is False  # leaking bug TDD test
    assert "unknown" not in block  # both attributed


# --- (d) unattributable failure -> fail-closed unknown --------------------

def test_empty_source_path_fails_closed():
    block = per_node_verdict.build_per_node_verdict(
        [_failure("ori_llvm", source_path="")], index_fn=lambda p: None)
    assert block["unknown"]["parity"] == "unknown"
    assert block["unknown"]["leak_free"] == "unknown"


def test_index_miss_fails_closed():
    # source_path present but the index maps it to None -> unknown sentinel.
    block = per_node_verdict.build_per_node_verdict(
        [_failure("ori_interp", source_path="compiler_repo/tests/spec/orphan.ori")],
        index_fn=lambda p: None)
    assert block["unknown"]["parity"] == "unknown"
    assert block["unknown"]["leak_free"] == "unknown"
    # Never silently green.
    assert block["unknown"]["leak_free"] is not True
    assert block["unknown"]["parity"] is not True


def test_unattributed_leak_does_not_taint_attributed_node():
    # A mix: one attributed LLVM-red node + one unattributed failure. The
    # attributed node keeps its concrete verdict; the unknown sentinel is
    # separate and fail-closed.
    ori = "compiler_repo/tests/spec/foo.ori"

    def index_fn(p):
        return "s-real" if p == ori else None

    failures = [
        _failure("ori_llvm", source_path=ori),
        _failure("ori_interp", source_path=""),
    ]
    block = per_node_verdict.build_per_node_verdict(failures, index_fn=index_fn)
    assert block["s-real"]["parity"] is False
    assert block["unknown"]["parity"] == "unknown"
    assert block["unknown"]["leak_free"] == "unknown"


def test_empty_failures_is_empty_block():
    block = per_node_verdict.build_per_node_verdict([], index_fn=lambda p: None)
    assert block == {}
