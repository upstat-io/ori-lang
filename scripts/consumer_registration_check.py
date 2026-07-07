"""Canon-consumer registration checks — the single semantic authority gate.

The `ori_ir::canon` module (`CanExpr` / `DecisionTreePool` / the resolved type
pool) is the sole frontend meaning surface. Every crate that consumes that
meaning must be registered in the `ori_ir::canon::consumers::CANON_CONSUMERS`
Rust registry (the SSOT). Two directions are checked, one per direction:

- check_registered_consumers  — every REGISTERED entry has an `ori_ir` dep edge
  (stale-entry direction; `missing-ir-dep` / `unknown-consumer-crate`).
- check_unregistered_consumers — every crate whose source REFERENCES the module
  is registered (unregistered-consumer direction), with the PRODUCER `ori_canon`
  and the module-home crate `ori_ir` exempt.

Both consume `cargo metadata` (Cargo.toml only — succeeds on a non-compiling
tree) plus the parsed registry. `crate-dag-lint.py` imports these for its CLI.

Pool-subset mismatch (a registered `ConsumerEntry.pool_access` read-set that
disagrees with which `CanonPool` variants a crate's source actually tree-walks)
is NOT checked here — see the module-level decision comment above
`check_unregistered_consumers`.
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

# Rust SSOT for the canon-consumer registry, relative to the Cargo workspace
# root. Parsed (never copied) by `_read_canon_consumers`.
CANON_CONSUMERS_SRC = Path("compiler/ori_ir/src/canon/consumers.rs")
# The leaf IR crate every canon-meaning consumer must depend on.
CANON_IR_CRATE = "ori_ir"
# The Rust module path a canon-meaning consumer imports.
CANON_MODULE_PATH = "ori_ir::canon"
# Producer crates that hold an `ori_ir::canon` edge but consume no meaning, so
# they are correctly ABSENT from the consumer registry. `ori_canon` constructs
# the pools; the module-home crate (`ir_module`'s first path segment) owns the
# module itself. Both are exempt from the unregistered-consumer check.
CANON_PRODUCER_CRATES = frozenset({"ori_canon"})
# Extracts each `crate_name: "<name>"` entry from CANON_CONSUMERS.
_CRATE_NAME_RE = re.compile(r'crate_name:\s*"([^"]+)"')


def load_cargo_metadata(root: Path) -> dict:
    """Parsed `cargo metadata --no-deps --format-version 1` for the workspace.

    `--no-deps` restricts packages to workspace members. Raises RuntimeError on
    cargo failure (caller maps to exit 2). Single fetch shared by the graph
    reader and `_crate_source_dirs` so the subprocess skeleton has one home.
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
    return json.loads(out.stdout)


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


def _crate_source_dirs(root: Path) -> dict[str, Path]:
    """Return {crate_name: manifest_dir} for every workspace member.

    Derived from `cargo metadata` package `manifest_path`s — the SSOT for the
    package -> source-dir mapping, never a hardcoded layout walk.
    """
    meta = load_cargo_metadata(root)
    dirs: dict[str, Path] = {}
    for pkg in meta.get("packages", []):
        name = pkg.get("name", "")
        manifest = pkg.get("manifest_path")
        if name and manifest:
            dirs[name] = Path(manifest).parent
    return dirs


def _references_module(rs_text: str, ir_module: str) -> bool:
    """True iff a non-line-comment portion of `rs_text` contains `ir_module`.

    Strips each line at its first `//` before matching, so a doc-comment (`//!`
    / `///`) or trailing-comment mention of the module path is NOT counted as a
    real import/usage — only code references qualify.
    """
    for line in rs_text.splitlines():
        code = line.split("//", 1)[0]
        if ir_module in code:
            return True
    return False


def _scan_module_references(
    src_dirs: dict[str, Path], ir_module: str
) -> set[str]:
    """Return the set of crate names whose `.rs` source references `ir_module`.

    Walks each crate's manifest dir for `*.rs` files (skipping any `target`
    build-artifact dir) and applies `_references_module`. Pure over the injected
    {crate: dir} map — the cargo fetch lives in `_crate_source_dirs`.
    """
    hits: set[str] = set()
    for crate, src_dir in src_dirs.items():
        for rs in src_dir.rglob("*.rs"):
            if "target" in rs.parts:
                continue
            try:
                text = rs.read_text(encoding="utf-8")
            except OSError:
                continue
            if _references_module(text, ir_module):
                hits.add(crate)
                break
    return hits


def _unregistered_violations(
    referencing: set[str],
    registry: list[str],
    ir_module: str,
    exempt: frozenset[str],
) -> list[dict[str, str]]:
    """Pure diff: a crate that references `ir_module`, is not registered, and is
    not exempt is an unregistered consumer. One violation dict per such crate.
    """
    rule = (
        f"single semantic authority: a crate whose source references {ir_module} "
        "consumes canon meaning and MUST be registered — add a ConsumerEntry "
        "for it to ori_ir::canon::consumers::CANON_CONSUMERS (the producer "
        "ori_canon holds the edge but constructs the pools, so it is exempt)"
    )
    registered = set(registry)
    violations: list[dict[str, str]] = []
    for crate in sorted(referencing):
        if crate in registered or crate in exempt:
            continue
        violations.append(
            {
                "src": crate,
                "target": ir_module,
                "kind": "unregistered-consumer",
                "rule": rule,
            }
        )
    return violations


# Failure mode (c) — pool-subset mismatch — is intentionally NOT implemented as a
# mechanical check here, and this absence is recorded rather than silently left
# out. `cargo metadata` and a source grep resolve only crate-level dependency
# edges; neither can determine WHICH `CanonPool` variant (CanExpr /
# DecisionTreePool / ResolvedTypePool) a crate's source actually tree-walks.
# Verifying that a registered `ConsumerEntry.pool_access` read-set matches a
# crate's real pool usage requires semantic analysis of the Rust source, out of
# reach of this metadata/grep lint. Under single semantic authority that
# correctness is pinned where the registry is DEFINED — by the unit tests on the
# `ori_ir::canon::consumers` module (the `pool_access` closed-classification is a
# property of the registry, asserted at its home, not re-derived here).
def check_unregistered_consumers(
    root: Path, registry: list[str], ir_module: str = CANON_MODULE_PATH
) -> list[dict[str, str]]:
    """Failure mode (a): every crate whose source references `ir_module` must be
    a registered CANON_CONSUMERS entry (or the exempt producer / module home).

    Scans each workspace member's source for a real (non-comment) reference to
    `ir_module`, diffs against `registry`, and reports every unregistered
    dependent crate. The producer `ori_canon` and the module-home crate (the
    first path segment of `ir_module`, e.g. `ori_ir`) are exempt — they hold the
    edge but consume no meaning. Empty list = clean.
    """
    exempt = frozenset({ir_module.split("::", 1)[0]}) | CANON_PRODUCER_CRATES
    referencing = _scan_module_references(_crate_source_dirs(root), ir_module)
    return _unregistered_violations(referencing, registry, ir_module, exempt)


def self_test_consumer_checks() -> int:
    """Self-test for the consumer-registration checks. Returns the failure count
    and prints one FAIL line per failing fixture to stderr; folded into
    `crate-dag-lint.py --self-test`.
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

    # Registered direction — every registered consumer depends on ori_ir.
    reg_graph = {"ori_ir": set(), "ori_eval": {"ori_ir"}, "oric": {"ori_ir", "ori_eval"}}
    expect(
        "registered-clean",
        check_registered_consumers(reg_graph, ["ori_eval", "oric"]),
        want_nonempty=False,
    )
    expect(
        "registered-missing-ir-dep",
        check_registered_consumers({"ori_ir": set(), "ori_eval": set()}, ["ori_eval"]),
        want_nonempty=True,
    )
    expect(
        "registered-unknown-crate",
        check_registered_consumers({"ori_ir": set()}, ["ori_evla"]),
        want_nonempty=True,
    )

    # _read_canon_consumers fails closed on a zero-entry parse (const emptied /
    # renamed) rather than reporting a false clean.
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

    # Unregistered direction (mode a) — pure diff. A referencing crate absent
    # from the registry AND not exempt is a violation; a registered or exempt
    # (producer / module-home) referencing crate is clean.
    exempt = frozenset({"ori_ir", "ori_canon"})
    expect(
        "unregistered-clean",
        _unregistered_violations(
            {"ori_eval", "ori_canon", "ori_ir"}, ["ori_eval"], CANON_MODULE_PATH, exempt
        ),
        want_nonempty=False,
    )
    expect(
        "unregistered-flags-unlisted-crate",
        _unregistered_violations({"ori_newbackend"}, ["ori_eval"], CANON_MODULE_PATH, exempt),
        want_nonempty=True,
    )
    expect(
        "unregistered-exempts-producer",
        _unregistered_violations({"ori_canon"}, [], CANON_MODULE_PATH, exempt),
        want_nonempty=False,
    )

    # Comment-stripping in `_references_module`: real code matches; a `//!` doc
    # comment or a trailing `//` mention does not.
    if not _references_module("use ori_ir::canon::CanId;", CANON_MODULE_PATH):
        failures += 1
        print("FAIL[references-code]: expected code use to match", file=sys.stderr)
    if _references_module("//! docs about ori_ir::canon", CANON_MODULE_PATH):
        failures += 1
        print("FAIL[references-doc-comment]: doc mention must not match", file=sys.stderr)
    if _references_module("foo(); // ori_ir::canon note", CANON_MODULE_PATH):
        failures += 1
        print("FAIL[references-trailing]: trailing mention must not match", file=sys.stderr)

    # Temp-dir source scan (`_scan_module_references`): a crate whose `.rs` code
    # references the module is a hit; a doc-only crate is not; a `target` build
    # dir is skipped.
    with tempfile.TemporaryDirectory() as td:
        base = Path(td)
        (base / "consumer" / "src").mkdir(parents=True)
        (base / "consumer" / "src" / "lib.rs").write_text(
            "use ori_ir::canon::CanExpr;\n", encoding="utf-8"
        )
        (base / "docs_only" / "src").mkdir(parents=True)
        (base / "docs_only" / "src" / "lib.rs").write_text(
            "//! mentions ori_ir::canon in a doc comment only\n", encoding="utf-8"
        )
        (base / "consumer" / "target").mkdir(parents=True)
        (base / "consumer" / "target" / "gen.rs").write_text(
            "use ori_ir::canon::Never;\n", encoding="utf-8"
        )
        hits = _scan_module_references(
            {"consumer": base / "consumer", "docs_only": base / "docs_only"},
            CANON_MODULE_PATH,
        )
        if hits != {"consumer"}:
            failures += 1
            print(
                f"FAIL[scan-references]: expected {{'consumer'}}, got {hits}",
                file=sys.stderr,
            )

    # Composed negative fixture (the falsifiable core): a synthetic workspace
    # whose crate imports the module but is ABSENT from the registry is rejected
    # end-to-end (scan -> diff -> violation); registering it clears the
    # violation (positive control).
    with tempfile.TemporaryDirectory() as td:
        base = Path(td)
        (base / "rogue" / "src").mkdir(parents=True)
        (base / "rogue" / "src" / "lib.rs").write_text(
            "use ori_ir::canon::DecisionTreePool;\n", encoding="utf-8"
        )
        rogue_hits = _scan_module_references({"rogue": base / "rogue"}, CANON_MODULE_PATH)
        expect(
            "unregistered-synthetic-rejected",
            _unregistered_violations(rogue_hits, ["ori_eval"], CANON_MODULE_PATH, exempt),
            want_nonempty=True,
        )
        expect(
            "unregistered-synthetic-registered-clears",
            _unregistered_violations(
                rogue_hits, ["ori_eval", "rogue"], CANON_MODULE_PATH, exempt
            ),
            want_nonempty=False,
        )

    return failures
