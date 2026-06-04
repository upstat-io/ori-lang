#!/usr/bin/env python3
"""crate-dag-lint — architectural fitness function over the workspace crate DAG.

Asserts the intended phase/crate dependency direction: a forbidden-edge set
encoded as data (FORBIDDEN_EDGES, the SSOT) must NOT appear in the actual
workspace dependency graph read via `cargo metadata --no-deps`. Each forbidden
edge cites the per-crate conflict rule it enforces.

`cargo metadata` reads Cargo.toml only — it succeeds on a non-compiling tree,
so the gate works on a tree a parallel session has broken.

Usage:
  crate-dag-lint.py                # check the actual workspace, print verdict
  crate-dag-lint.py --json         # machine-readable verdict to stdout
  crate-dag-lint.py --self-test    # synthetic-graph fixtures, exit 0 on pass

Exit codes:
  0  clean (no forbidden edge present) OR --self-test passed
  1  one or more forbidden edges present (names the edge + the rule)
  2  could not enumerate the workspace (cargo metadata failed) — caller SHALL
     fall back rather than skip the gate
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections import deque
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class ForbiddenEdge:
    """A banned dependency: `src` must not reach any crate in `targets`.

    `transitive` selects direct-only (False) vs reachability (True) checking.
    `rule` names the per-crate conflict rule the edge enforces.
    """

    src: str
    targets: frozenset[str]
    transitive: bool
    rule: str


# Forbidden-edge SSOT. Each entry is derived from a per-crate conflict rule
# in the project's mission inventory; the `rule` string cites it. Targets are
# crate-name sets so one entry covers a "must not depend on ANY of these".
# Transitive entries assert REACHABILITY (src must not REACH target through
# the workspace-dep graph); direct entries assert only a direct dependency is
# absent (the source rule constrains the immediate edge, not the closure).
FORBIDDEN_EDGES: tuple[ForbiddenEdge, ...] = (
    # Codegen consumes realized ARC IR, not CanExpr. Direct-only: ori_llvm
    # depends on ori_arc, which legitimately depends on ori_canon, so the
    # transitive reach ori_llvm -> ori_canon exists through ori_arc and is
    # NOT a violation; the banned shape is a DIRECT ori_llvm -> ori_canon edge.
    ForbiddenEdge(
        src="ori_llvm",
        targets=frozenset({"ori_canon"}),
        transitive=False,
        rule="ori_llvm conflict rule: depends on ori_arc but NOT on ori_canon "
        "(codegen consumes realized ARC IR, not CanExpr)",
    ),
    # Leaf status: every pipeline crate depends on ori_ir, never vice versa.
    # Transitive: a dep on ANY workspace crate (direct or through the closure)
    # breaks leaf status.
    ForbiddenEdge(
        src="ori_ir",
        targets=frozenset(
            {
                "ori_arc",
                "ori_canon",
                "ori_compiler",
                "ori_diagnostic",
                "ori_eval",
                "ori_fmt",
                "ori_lexer",
                "ori_lexer_core",
                "ori_llvm",
                "ori_lsp",
                "ori_parse",
                "ori_patterns",
                "ori_registry",
                "ori_repr",
                "ori_rt",
                "ori_stack",
                "ori_test_harness",
                "ori_types",
                "oric",
            }
        ),
        transitive=True,
        rule="ori_ir conflict rule: leaf status is load-bearing; an upstream "
        "compiler dep forces every pipeline crate to rebuild on IR changes",
    ),
    # Leaf of the lexing front: imported only by ori_lexer; no compiler dep.
    ForbiddenEdge(
        src="ori_lexer_core",
        targets=frozenset(
            {
                "ori_arc",
                "ori_canon",
                "ori_compiler",
                "ori_diagnostic",
                "ori_eval",
                "ori_fmt",
                "ori_ir",
                "ori_lexer",
                "ori_llvm",
                "ori_lsp",
                "ori_parse",
                "ori_patterns",
                "ori_registry",
                "ori_repr",
                "ori_rt",
                "ori_stack",
                "ori_test_harness",
                "ori_types",
                "oric",
            }
        ),
        transitive=True,
        rule="ori_lexer_core conflict rule: leaf of the lexing front; core "
        "stays pure (no names/types/scopes, no compiler dep)",
    ),
    # Zero semantic knowledge: lexer must not reach the semantic crates.
    # Transitive: any reach into a semantic crate is phase-bleeding.
    ForbiddenEdge(
        src="ori_lexer",
        targets=frozenset(
            {
                "ori_types",
                "ori_canon",
                "ori_eval",
                "ori_arc",
                "ori_patterns",
                "ori_repr",
                "ori_llvm",
            }
        ),
        transitive=True,
        rule="ori_lexer conflict rule: zero semantic knowledge; names/types/"
        "scopes are phase-bleeding (parser or type checker owns that)",
    ),
    # Pure IO-free facade: no Salsa/LLVM. ori_compiler legitimately depends on
    # core pipeline crates (ori_canon/ori_eval/ori_patterns are in its dep
    # set), so the ban is narrow: it must not reach oric (Salsa driver) or
    # ori_llvm (LLVM backend). Transitive: reaching either through any path
    # fractures embedding.
    ForbiddenEdge(
        src="ori_compiler",
        targets=frozenset({"oric", "ori_llvm"}),
        transitive=True,
        rule="ori_compiler conflict rule: pure IO-free facade; any IO/Salsa/"
        "LLVM dependency (oric or ori_llvm) fractures embedding",
    ),
    # Downstream phases never re-parse/re-infer: parser must not depend on the
    # phases that consume its output. Transitive: reaching a downstream phase
    # through any path inverts the pipeline direction.
    ForbiddenEdge(
        src="ori_parse",
        targets=frozenset({"ori_types", "ori_canon", "ori_eval", "ori_llvm"}),
        transitive=True,
        rule="ori_parse conflict rule: parser owns output absolutely; "
        "downstream phases never re-parse/re-infer (no dep on consumers)",
    ),
)


def _read_workspace_graph(root: Path) -> dict[str, set[str]]:
    """Return {crate_name: {workspace-member dep names}} via cargo metadata.

    `--no-deps` restricts packages to workspace members; each member's
    `dependencies` are filtered to names that are themselves workspace
    members (external crates are not graph nodes). Raises RuntimeError on
    cargo failure (caller maps to exit 2).
    """
    out = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        capture_output=True,
        text=True,
        check=False,
        timeout=120,
        cwd=root,
    )
    if out.returncode != 0:
        raise RuntimeError(f"cargo metadata failed: {out.stderr.strip()[:400]}")
    meta = json.loads(out.stdout)
    members = {pkg["name"] for pkg in meta.get("packages", [])}
    graph: dict[str, set[str]] = {name: set() for name in members}
    for pkg in meta.get("packages", []):
        name = pkg["name"]
        for dep in pkg.get("dependencies", []):
            dep_name = dep.get("name", "")
            if dep_name in members and dep_name != name:
                graph[name].add(dep_name)
    return graph


def _reaches(graph: dict[str, set[str]], src: str, targets: frozenset[str]) -> set[str]:
    """Return the subset of `targets` reachable from `src` via graph edges.

    BFS over workspace-member edges; `src` itself is excluded from the result
    (a crate does not "depend on" itself). Returns empty set when src is not a
    graph node.
    """
    if src not in graph:
        return set()
    seen: set[str] = set()
    queue: deque[str] = deque(graph[src])
    while queue:
        node = queue.popleft()
        if node in seen:
            continue
        seen.add(node)
        queue.extend(graph.get(node, set()))
    return targets & seen


def check_graph(
    graph: dict[str, set[str]], edges: tuple[ForbiddenEdge, ...] = FORBIDDEN_EDGES
) -> list[dict[str, str]]:
    """Pure check: return one violation dict per present forbidden edge.

    Direct edges check the immediate dependency set; transitive edges check
    reachability. Each violation names src -> hit_target, the kind
    (direct/transitive), and the cited rule. Empty list = clean.
    """
    violations: list[dict[str, str]] = []
    for edge in edges:
        if edge.transitive:
            hits = _reaches(graph, edge.src, edge.targets)
        else:
            hits = edge.targets & graph.get(edge.src, set())
        for hit in sorted(hits):
            violations.append(
                {
                    "src": edge.src,
                    "target": hit,
                    "kind": "transitive" if edge.transitive else "direct",
                    "rule": edge.rule,
                }
            )
    return violations


def _self_test() -> int:
    """Synthetic-graph fixtures: forbidden-edge graph fails closed; clean passes.

    Exercises a direct-only fixture (ori_llvm -> ori_canon must fire), a
    transitive fixture (ori_lexer reaching a semantic crate through one hop),
    a direct-only NON-violation (ori_llvm reaching ori_canon ONLY through
    ori_arc must NOT fire), and a clean baseline.
    """
    failures = 0

    def expect(label: str, got: list[dict[str, str]], want_nonempty: bool) -> None:
        nonlocal failures
        if bool(got) != want_nonempty:
            failures += 1
            verdict = "violations" if got else "clean"
            wanted = "violations" if want_nonempty else "clean"
            print(
                f"FAIL[{label}]: got {verdict} (expected {wanted}): {got}",
                file=sys.stderr,
            )

    # Fixture A — direct forbidden edge present (ori_llvm -> ori_canon).
    graph_direct = {
        "ori_llvm": {"ori_arc", "ori_canon"},
        "ori_arc": {"ori_canon"},
        "ori_canon": set(),
    }
    expect("direct-violation", check_graph(graph_direct), want_nonempty=True)

    # Fixture B — ori_llvm reaches ori_canon ONLY through ori_arc (legitimate).
    # The direct-only ori_llvm rule must NOT fire here. Restrict to that edge
    # so ori_arc -> ori_canon (allowed) does not muddy the assertion.
    llvm_edge = tuple(e for e in FORBIDDEN_EDGES if e.src == "ori_llvm")
    graph_indirect = {
        "ori_llvm": {"ori_arc"},
        "ori_arc": {"ori_canon"},
        "ori_canon": set(),
    }
    expect(
        "direct-nonviolation",
        check_graph(graph_indirect, llvm_edge),
        want_nonempty=False,
    )

    # Fixture C — transitive forbidden edge (ori_lexer -> ori_types via a hop).
    lexer_edge = tuple(e for e in FORBIDDEN_EDGES if e.src == "ori_lexer")
    graph_transitive = {
        "ori_lexer": {"ori_lexer_core"},
        "ori_lexer_core": {"ori_types"},  # phase-bleed two hops out
        "ori_types": set(),
    }
    expect(
        "transitive-violation",
        check_graph(graph_transitive, lexer_edge),
        want_nonempty=True,
    )

    # Fixture D — clean baseline honoring every rule direction.
    graph_clean = {
        "ori_ir": set(),
        "ori_lexer_core": set(),
        "ori_lexer": {"ori_ir", "ori_lexer_core"},
        "ori_parse": {"ori_ir", "ori_lexer"},
        "ori_types": {"ori_ir", "ori_parse"},
        "ori_canon": {"ori_ir", "ori_types"},
        "ori_arc": {"ori_ir", "ori_canon"},
        "ori_llvm": {"ori_ir", "ori_arc"},
        "ori_compiler": {"ori_ir", "ori_canon", "ori_eval"},
        "ori_eval": {"ori_ir", "ori_canon"},
        "oric": {"ori_llvm", "ori_compiler"},
    }
    expect("clean-baseline", check_graph(graph_clean), want_nonempty=False)

    if failures:
        print(f"crate-dag-lint self-test: {failures} failure(s)", file=sys.stderr)
        return 1
    print("crate-dag-lint self-test: OK")
    return 0


def _cargo_workspace_root() -> Path:
    """Return the nearest ancestor of this script that contains Cargo.toml.

    Walks up from `__file__` so the gate runs cargo metadata from the Cargo
    workspace root regardless of the invoking cwd. Raises RuntimeError when no
    ancestor holds a Cargo.toml.
    """
    here = Path(__file__).resolve()
    for parent in here.parents:
        if (parent / "Cargo.toml").is_file():
            return parent
    raise RuntimeError(f"no Cargo.toml in any ancestor of {here}")


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="crate-dependency-DAG fitness function")
    parser.add_argument("--json", action="store_true", help="emit verdict as JSON")
    parser.add_argument("--self-test", action="store_true", help="run synthetic fixtures")
    args = parser.parse_args(argv)

    if args.self_test:
        return _self_test()

    try:
        root = _cargo_workspace_root()
        graph = _read_workspace_graph(root)
    except RuntimeError as exc:
        print(f"crate-dag-lint: {exc}", file=sys.stderr)
        return 2

    violations = check_graph(graph)

    if args.json:
        print(json.dumps({"clean": not violations, "violations": violations}, indent=2))
    elif violations:
        print(f"crate-dag-lint: {len(violations)} forbidden edge(s) present:", file=sys.stderr)
        for v in violations:
            print(
                f"  VIOLATION: {v['src']} -> {v['target']} ({v['kind']})\n"
                f"    rule: {v['rule']}",
                file=sys.stderr,
            )
    else:
        print("crate-dag-lint: clean — no forbidden crate-DAG edge present")

    return 1 if violations else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
