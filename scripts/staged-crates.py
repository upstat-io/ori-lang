#!/usr/bin/env python3
"""staged-crates — map the staged index to the workspace crates it touches.

Emits one cargo package name per line for every workspace member that owns at
least one staged file (`git diff --cached`). Consumed by `full-check.sh` when
`ORI_COMMIT_SCOPE=staged` to scope the blocking clippy gate to the committing
diff's own crates (so a parallel session's breakage in unrelated crates does
not block an independent check-in, while the committing diff's own breakage in
its crates still fails the gate).

`cargo metadata` reads Cargo.toml only — it succeeds even when the workspace
source does not compile, which is the whole point: scoping must work on a tree
a parallel session has broken.

Usage:
  staged-crates.py                 # print touched crate names, one per line
  staged-crates.py --self-test     # run internal mapping tests, exit 0 on pass

Exit codes:
  0  success (zero or more crate names printed)
  2  could not enumerate the workspace (cargo metadata failed) — caller SHALL
     fall back to a full-workspace check rather than skip the gate
"""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


def _git_root() -> Path:
    out = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        capture_output=True, text=True, check=False, timeout=10,
    )
    if out.returncode != 0:
        raise RuntimeError(f"git rev-parse failed: {out.stderr.strip()}")
    return Path(out.stdout.strip())


def _staged_files(root: Path) -> list[Path]:
    out = subprocess.run(
        ["git", "diff", "--cached", "--name-only", "--diff-filter=ACMR"],
        capture_output=True, text=True, check=False, timeout=30, cwd=root,
    )
    if out.returncode != 0:
        raise RuntimeError(f"git diff --cached failed: {out.stderr.strip()}")
    return [root / line for line in out.stdout.splitlines() if line.strip()]


def _workspace_packages(root: Path) -> list[tuple[Path, str]]:
    """Return [(package_dir, package_name), ...] for every workspace member."""
    out = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        capture_output=True, text=True, check=False, timeout=120, cwd=root,
    )
    if out.returncode != 0:
        raise RuntimeError(f"cargo metadata failed: {out.stderr.strip()[:400]}")
    meta = json.loads(out.stdout)
    pkgs: list[tuple[Path, str]] = []
    for pkg in meta.get("packages", []):
        manifest = Path(pkg["manifest_path"]).resolve()
        pkgs.append((manifest.parent, pkg["name"]))
    return pkgs


def map_files_to_crates(
    staged: list[Path], packages: list[tuple[Path, str]]
) -> list[str]:
    """Pure mapping: each staged file -> the longest-prefix package dir's name.

    Files under no package (root manifest, docs, scripts, tests/) contribute no
    crate. Returns crate names sorted + de-duplicated.
    """
    # Longest package dir first so nested crates win the prefix match.
    ordered = sorted(packages, key=lambda pd: len(str(pd[0])), reverse=True)
    hit: set[str] = set()
    for f in staged:
        fr = f.resolve()
        for pkg_dir, name in ordered:
            try:
                fr.relative_to(pkg_dir)
            except ValueError:
                continue
            hit.add(name)
            break
    return sorted(hit)


def _self_test() -> int:
    root = Path("/ws")
    packages = [
        (root / "compiler" / "ori_arc", "ori_arc"),
        (root / "compiler" / "oric", "oric"),
        (root / "compiler" / "ori_arc" / "subcrate", "ori_arc_sub"),
    ]
    cases: list[tuple[list[Path], list[str]]] = [
        # single crate touched
        ([root / "compiler/ori_arc/src/fbip/mod.rs"], ["ori_arc"]),
        # docs/tests outside any crate -> no crate
        ([root / "docs/x.md", root / "tests/spec/y.ori"], []),
        # mixed: crate file + doc -> just the crate
        ([root / "compiler/ori_arc/src/a.rs", root / "README.md"], ["ori_arc"]),
        # two distinct crates
        (
            [root / "compiler/ori_arc/src/a.rs", root / "compiler/oric/src/b.rs"],
            ["ori_arc", "oric"],
        ),
        # nested crate wins longest-prefix over parent
        ([root / "compiler/ori_arc/subcrate/src/c.rs"], ["ori_arc_sub"]),
        # empty staged set
        ([], []),
    ]
    failures = 0
    for staged, expected in cases:
        got = map_files_to_crates(staged, packages)
        if got != expected:
            failures += 1
            print(f"FAIL: {[str(p) for p in staged]} -> {got} (expected {expected})",
                  file=sys.stderr)
    if failures:
        print(f"staged-crates self-test: {failures} failure(s)", file=sys.stderr)
        return 1
    print("staged-crates self-test: OK")
    return 0


def main(argv: list[str]) -> int:
    if "--self-test" in argv:
        return _self_test()
    try:
        root = _git_root()
        staged = _staged_files(root)
        packages = _workspace_packages(root)
    except RuntimeError as exc:
        print(f"staged-crates: {exc}", file=sys.stderr)
        return 2
    for name in map_files_to_crates(staged, packages):
        print(name)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
