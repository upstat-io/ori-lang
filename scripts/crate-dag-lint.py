#!/usr/bin/env python3
"""crate-dag-lint — architectural fitness function over the workspace crate DAG.

Two checks over the workspace PRODUCTION dependency graph read via `cargo
metadata --no-deps` (dev/build deps excluded, see `_graph_from_metadata`):

1. Forbidden-edge direction (FORBIDDEN_EDGES): a banned phase/crate dependency
   must NOT appear. Each forbidden edge cites the per-crate conflict rule.
2. Canon-consumer registration (single semantic authority): every crate listed
   in the `ori_ir::canon::consumers::CANON_CONSUMERS` Rust registry (the SSOT,
   read from source not duplicated here) MUST have a dependency edge on `ori_ir`.

`cargo metadata` reads Cargo.toml only — it succeeds on a non-compiling tree,
so the gate works on a tree a parallel session has broken.

Usage:
  crate-dag-lint.py                # check the actual workspace, print verdict
  crate-dag-lint.py --json         # machine-readable verdict to stdout
  crate-dag-lint.py --self-test    # synthetic-graph fixtures, exit 0 on pass

Exit codes:
  0  clean (no violation present) OR --self-test passed
  1  one or more violations present (names the edge/consumer + the rule)
  2  could not enumerate the workspace (cargo metadata failed OR the
     CANON_CONSUMERS registry source is unreadable) — caller SHALL fall back
     rather than skip the gate
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tempfile
from collections import deque
from dataclasses import dataclass
from pathlib import Path

# Rust SSOT for the canon-consumer registry, relative to the Cargo workspace
# root. Parsed (never copied) by `_read_canon_consumers`.
CANON_CONSUMERS_SRC = Path("compiler/ori_ir/src/canon/consumers.rs")
# The leaf IR crate every canon-meaning consumer must depend on.
CANON_IR_CRATE = "ori_ir"
# Extracts each `crate_name: "<name>"` entry from CANON_CONSUMERS.
_CRATE_NAME_RE = re.compile(r'crate_name:\s*"([^"]+)"')


@dataclass(frozen=True)
class ForbiddenEdge:
    """A banned dependency: `src` must not reach any crate in `targets`.

    `transitive` selects direct-only (False) vs reachability (True). `rule`
    cites the per-crate conflict rule. `targets=None` is the leaf-crate
    sentinel (`_resolve_targets`): every workspace member except `src` and
    `allowed`, so a newly added crate is banned automatically.
    """

    src: str
    targets: frozenset[str] | None
    transitive: bool
    rule: str
    allowed: frozenset[str] = frozenset()


# Forbidden-edge SSOT, each derived from a per-crate conflict rule in the
# project's mission inventory (cited via `rule`); see ForbiddenEdge docstring
# for the transitive/direct + targets=None semantics.
FORBIDDEN_EDGES: tuple[ForbiddenEdge, ...] = (
    # Direct-only: ori_llvm->ori_arc->ori_canon transitively is legitimate;
    # only a DIRECT ori_llvm->ori_canon edge is banned (codegen consumes ARC IR).
    ForbiddenEdge(
        src="ori_llvm",
        targets=frozenset({"ori_canon"}),
        transitive=False,
        rule="ori_llvm conflict rule: depends on ori_arc but NOT on ori_canon "
        "(codegen consumes realized ARC IR, not CanExpr)",
    ),
    # Leaf status: ori_ir depends on no pipeline crate. targets=None bans every
    # other workspace member, so a newly added crate is covered automatically.
    ForbiddenEdge(
        src="ori_ir",
        targets=None,
        transitive=True,
        rule="ori_ir conflict rule: leaf status is load-bearing; an upstream "
        "compiler dep forces every pipeline crate to rebuild on IR changes",
    ),
    # Leaf of the lexing front: imported only by ori_lexer; no compiler dep.
    ForbiddenEdge(
        src="ori_lexer_core",
        targets=None,
        transitive=True,
        rule="ori_lexer_core conflict rule: leaf of the lexing front; core "
        "stays pure (no names/types/scopes, no compiler dep)",
    ),
    # Zero semantic knowledge: lexer bans everything except its 2 declared
    # deps (`allowed`) — incl. ori_parse, the canonical backward-edge example.
    ForbiddenEdge(
        src="ori_lexer",
        targets=None,
        allowed=frozenset({"ori_ir", "ori_lexer_core"}),
        transitive=True,
        rule="ori_lexer conflict rule: zero semantic knowledge; names/types/"
        "scopes are phase-bleeding; one-way data flow bans ANY dep on a "
        "downstream/sibling phase",
    ),
    # Pure IO-free facade: legitimately depends on core pipeline crates, but
    # must not transitively reach oric (Salsa driver) or ori_llvm (LLVM backend).
    ForbiddenEdge(
        src="ori_compiler",
        targets=frozenset({"oric", "ori_llvm"}),
        transitive=True,
        rule="ori_compiler conflict rule: pure IO-free facade; any IO/Salsa/"
        "LLVM dependency (oric or ori_llvm) fractures embedding",
    ),
    # Parser owns output absolutely: must not transitively depend on any
    # downstream-phase consumer of its output.
    ForbiddenEdge(
        src="ori_parse",
        targets=frozenset({"ori_types", "ori_canon", "ori_eval", "ori_llvm"}),
        transitive=True,
        rule="ori_parse conflict rule: parser owns output absolutely; "
        "downstream phases never re-parse/re-infer (no dep on consumers)",
    ),
)


def _graph_from_metadata(meta: dict) -> dict[str, set[str]]:
    """Pure: {crate_name: {workspace-member PRODUCTION dep names}} from a
    parsed `cargo metadata --format-version 1` document.

    Excludes external (non-member) deps and non-normal (`kind is not None`)
    edges — dev-dependency edges are permitted to be cyclic by Cargo (a leaf
    crate's tests may dev-depend on a downstream crate for fixtures) and are
    excluded from the forbidden-edge graph.
    """
    members = {pkg["name"] for pkg in meta.get("packages", [])}
    graph: dict[str, set[str]] = {name: set() for name in members}
    for pkg in meta.get("packages", []):
        name = pkg["name"]
        for dep in pkg.get("dependencies", []):
            if dep.get("kind") is not None:
                continue
            dep_name = dep.get("name", "")
            if dep_name in members and dep_name != name:
                graph[name].add(dep_name)
    return graph


def _read_workspace_graph(root: Path) -> dict[str, set[str]]:
    """Return {crate_name: {workspace-member production dep names}} via cargo metadata.

    `--no-deps` restricts packages to workspace members. Raises RuntimeError on
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
    return _graph_from_metadata(json.loads(out.stdout))


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


def _resolve_targets(edge: ForbiddenEdge, graph: dict[str, set[str]]) -> frozenset[str]:
    """Resolve an edge's ban set against the live workspace graph.

    An explicit `targets` frozenset is returned unchanged. `targets=None`
    resolves to every workspace member except `edge.src` and `edge.allowed`
    — derived from `graph`, never a parallel hand-maintained copy.
    """
    if edge.targets is not None:
        return edge.targets
    return frozenset(graph) - {edge.src} - edge.allowed


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
        targets = _resolve_targets(edge, graph)
        if edge.transitive:
            hits = _reaches(graph, edge.src, targets)
        else:
            hits = targets & graph.get(edge.src, set())
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


def _read_canon_consumers(root: Path) -> list[str]:
    """Return the registered `crate_name`s from the CANON_CONSUMERS Rust SSOT.

    Reads `compiler/ori_ir/src/canon/consumers.rs` and extracts each
    `crate_name: "<name>"` entry. The Rust const is the single source of truth;
    this parse derives from it, never keeps a parallel copy. Raises RuntimeError
    when the source file is missing (caller maps to exit 2).
    """
    src = root / CANON_CONSUMERS_SRC
    try:
        text = src.read_text(encoding="utf-8")
    except OSError as exc:
        raise RuntimeError(f"cannot read CANON_CONSUMERS registry {src}: {exc}") from exc
    names = _CRATE_NAME_RE.findall(text)
    if not names:
        # Non-empty by construction; zero-entry parse = broken registry. Fail
        # closed (caller maps RuntimeError to exit 2), never a false clean.
        raise RuntimeError(
            f"CANON_CONSUMERS registry {src} parsed zero entries — "
            "the const is empty, renamed, or reformatted past the parser"
        )
    return names


def check_registered_consumers(
    graph: dict[str, set[str]], registry: list[str], ir_crate: str = CANON_IR_CRATE
) -> list[dict[str, str]]:
    """Pure check: every registered canon consumer must depend on `ir_crate`.

    One violation dict per registered `crate_name` that is either not a
    workspace member (`unknown-consumer-crate`) or lacks a direct dependency
    edge on `ir_crate` (`missing-ir-dep`). Empty list = clean.
    """
    rule = (
        "single semantic authority: every CANON_CONSUMERS entry depends on "
        f"ori_ir::canon and MUST carry a dependency edge on {ir_crate}"
    )
    violations: list[dict[str, str]] = []
    for crate_name in registry:
        if crate_name not in graph:
            kind = "unknown-consumer-crate"
        elif ir_crate not in graph[crate_name]:
            kind = "missing-ir-dep"
        else:
            continue
        violations.append(
            {"src": crate_name, "target": ir_crate, "kind": kind, "rule": rule}
        )
    return violations


def _self_test() -> int:
    """Synthetic-graph fixtures: forbidden-edge graph fails closed; clean passes.

    Covers direct + transitive violations, a direct non-violation through an
    allowed intermediate, a clean baseline, `targets=None` leaf-ban coverage
    of a new crate, the `allowed` exemption, and the canon-consumer +
    dev-dep-exclusion checks.
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

    # Fixture E — leaf ban (`targets=None`) auto-covers a brand-new crate
    # absent from any hand-maintained list (ori_format-class gap pin).
    ir_edge = tuple(e for e in FORBIDDEN_EDGES if e.src == "ori_ir")
    graph_new_crate = {"ori_ir": {"ori_brand_new"}, "ori_brand_new": set()}
    expect(
        "leaf-ban-covers-new-crate", check_graph(graph_new_crate, ir_edge), want_nonempty=True
    )

    # Fixture F — ori_lexer's ban exempts its 2 legit deps but bans ori_parse
    # (lexer never imports parser — zero semantic knowledge in the lexing front).
    graph_lexer_ok = {
        "ori_lexer": {"ori_ir", "ori_lexer_core"},
        "ori_ir": set(),
        "ori_lexer_core": set(),
    }
    expect(
        "lexer-allowed-deps-clean", check_graph(graph_lexer_ok, lexer_edge), want_nonempty=False
    )
    graph_lexer_bad = {"ori_lexer": {"ori_ir", "ori_parse"}, "ori_ir": set(), "ori_parse": set()}
    expect("lexer-bans-parse", check_graph(graph_lexer_bad, lexer_edge), want_nonempty=True)

    # Fixture G — registered consumers all depend on ori_ir (clean).
    reg_graph = {
        "ori_ir": set(),
        "ori_eval": {"ori_ir"},
        "oric": {"ori_ir", "ori_eval"},
    }
    expect(
        "registered-clean",
        check_registered_consumers(reg_graph, ["ori_eval", "oric"]),
        want_nonempty=False,
    )

    # Fixture H — a registered consumer lacks a dependency edge on ori_ir.
    reg_graph_missing = {"ori_ir": set(), "ori_eval": set()}
    expect(
        "registered-missing-ir-dep",
        check_registered_consumers(reg_graph_missing, ["ori_eval"]),
        want_nonempty=True,
    )

    # Fixture I — a registered crate_name is not a workspace member (typo).
    expect(
        "registered-unknown-crate",
        check_registered_consumers({"ori_ir": set()}, ["ori_evla"]),
        want_nonempty=True,
    )

    # Fixture J — _read_canon_consumers fails closed on a zero-entry parse
    # (registry present but const emptied/renamed) rather than reporting clean.
    with tempfile.TemporaryDirectory() as td:
        src = Path(td) / CANON_CONSUMERS_SRC
        src.parent.mkdir(parents=True, exist_ok=True)
        src.write_text("// no CANON_CONSUMERS entries here\n", encoding="utf-8")
        try:
            _read_canon_consumers(Path(td))
            failures += 1
            print("FAIL[read-empty-registry]: expected RuntimeError", file=sys.stderr)
        except RuntimeError:
            pass
        src.write_text('ConsumerEntry { crate_name: "ori_eval" }\n', encoding="utf-8")
        if _read_canon_consumers(Path(td)) != ["ori_eval"]:
            failures += 1
            print("FAIL[read-valid-registry]: expected ['ori_eval']", file=sys.stderr)

    # Fixture K — a dev-dependency must NOT become a graph edge (Cargo permits
    # a cyclic dev-dep for test fixtures; mirrors the real ori_types ->
    # ori_lexer dev-dep in the workspace).
    meta_with_dev_dep = {
        "packages": [
            {
                "name": "ori_ir",
                "dependencies": [{"name": "ori_parse", "kind": "dev"}],
            },
            {"name": "ori_parse", "dependencies": []},
        ]
    }
    graph_from_meta = _graph_from_metadata(meta_with_dev_dep)
    if graph_from_meta.get("ori_ir") != set():
        failures += 1
        print(
            f"FAIL[dev-dep-excluded]: expected empty ori_ir edge set, "
            f"got {graph_from_meta.get('ori_ir')}",
            file=sys.stderr,
        )

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
        registry = _read_canon_consumers(root)
    except RuntimeError as exc:
        print(f"crate-dag-lint: {exc}", file=sys.stderr)
        return 2

    violations = check_graph(graph) + check_registered_consumers(graph, registry)

    if args.json:
        print(json.dumps({"clean": not violations, "violations": violations}, indent=2))
    elif violations:
        print(f"crate-dag-lint: {len(violations)} violation(s) present:", file=sys.stderr)
        for v in violations:
            print(
                f"  VIOLATION: {v['src']} -> {v['target']} ({v['kind']})\n"
                f"    rule: {v['rule']}",
                file=sys.stderr,
            )
    else:
        print(
            "crate-dag-lint: clean — no forbidden crate-DAG edge; "
            "every registered canon consumer depends on ori_ir"
        )

    return 1 if violations else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
