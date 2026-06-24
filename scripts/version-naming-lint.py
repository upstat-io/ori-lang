#!/usr/bin/env python3
"""Version/revision-marker naming linter (NAME-33 / NAMING:version-suffix).

Flags version/revision markers in declared identifiers (trait/type/function
/method/module/const). A version marker encodes WHEN an identifier was written,
not WHAT it does, and implies a parallel substitutable hierarchy the codebase
does not keep (per impl-hygiene.md NAME-33).

Mechanically gates ONLY the near-zero-false-positive subset:
  - V/v + digit: `LexerV2`, `parse_v2`, `v3_emit`, `_v2_`

Reviewer-time (NOT this lint) covers the fuzzier set per NAME-33: bare
ordinal-revision suffixes (`Resolver2`) and the revision WORDS
`Legacy`/`Deprecated`/`Old`/`New`/`Final`/`Original`. Those words collide with
legitimate domain use a gating lint must not fire on: a deprecated wasm target,
a legacy data format, the legacy RC path under migration, `new` constructor,
`old_value`/`new_value` swap pairs.

Domain tokens that denote a quantity / bit-width / standard / architecture are
NOT versions and never trip: `i64`/`u8`/`f64`/`utf8`/`sha256`/`crc32`/`fnv1a`
/`base64`/`simd128`/`x86`/`arm64`/`llvm21`, error codes `E0592` (no uppercase-V
+digit and no legacy/deprecated word appears in any of them).

Usage:
  scripts/version-naming-lint.py [paths...]   # default: compiler/ library/ tests/
  scripts/version-naming-lint.py --json       # machine-readable
  scripts/version-naming-lint.py --warn-only  # report without exit 1 (bed-in)
  scripts/version-naming-lint.py --exit-zero  # alias of --warn-only
  scripts/version-naming-lint.py --self-test  # run embedded positive/negative cases

Returns exit 1 when any violation is found (unless --warn-only/--exit-zero).
"""
from __future__ import annotations

import argparse
import json
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

# Version markers — the mechanically-gated subset (near-zero false-positive):
#   - camel V+digit suffix-or-component: LexerV2, ParserV10Fast
#   - snake v+digit component:           parse_v2, v3_emit, lexer_v2_helper
# Revision WORDS (Legacy/Deprecated/Old/New/Final/Original) and bare ordinal
# suffixes (Resolver2) are NAME-33 reviewer-time ONLY, never mechanically gated:
# they collide with legitimate domain use (a deprecated wasm target, a legacy
# data format, the legacy RC path under migration, old_value/new_value pairs).
MARKER_PATTERNS: list[tuple[str, re.Pattern[str]]] = [
    ("version-suffix", re.compile(r"[a-z0-9]V\d+(?:[A-Z]|$)")),
    ("version-suffix", re.compile(r"(?:^|_)v\d+(?:_|$)")),
]

# Rust declaration sites (the name an identifier is INTRODUCED at).
RUST_DECL_RE = re.compile(
    r"\b(?:fn|struct|enum|trait|type|mod|union|const|static)\s+([A-Za-z_][A-Za-z0-9_]*)"
)
# Ori declaration sites: `@name`, `trait Name`, `type Name`.
ORI_FN_RE = re.compile(r"(?:^|\s)@([a-zA-Z_][a-zA-Z0-9_]*)\s")
ORI_DECL_RE = re.compile(r"\b(?:trait|type)\s+([A-Za-z_][A-Za-z0-9_]*)")


@dataclass
class Finding:
    file: str
    line: int
    name: str
    category: str

    def to_dict(self) -> dict[str, str | int]:
        return {"file": self.file, "line": self.line, "name": self.name, "category": self.category}


def classify(name: str) -> str | None:
    for label, pat in MARKER_PATTERNS:
        if pat.search(name):
            return label
    return None


def _scan_names(line: str, patterns: Iterable[re.Pattern[str]]) -> Iterable[str]:
    for pat in patterns:
        for m in pat.finditer(line):
            yield m.group(1)


def scan_rust(path: Path) -> Iterable[Finding]:
    for idx, line in enumerate(path.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
        for name in _scan_names(line, (RUST_DECL_RE,)):
            cat = classify(name)
            if cat:
                yield Finding(str(path), idx, name, cat)


def scan_ori(path: Path) -> Iterable[Finding]:
    for idx, line in enumerate(path.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
        for name in _scan_names(line, (ORI_FN_RE, ORI_DECL_RE)):
            cat = classify(name)
            if cat:
                yield Finding(str(path), idx, f"@{name}" if line.lstrip().startswith("@") else name, cat)


def discover(roots: list[Path]) -> Iterable[Path]:
    for root in roots:
        if root.is_file():
            yield root
            continue
        for dirpath, dirnames, filenames in os.walk(root):
            dirnames[:] = [d for d in dirnames if d not in {"target", "node_modules", ".git"}]
            for fn in filenames:
                if fn.endswith((".rs", ".ori")):
                    yield Path(dirpath) / fn


def run(roots: list[Path]) -> list[Finding]:
    findings: list[Finding] = []
    for fp in discover(roots):
        if fp.suffix == ".rs":
            findings.extend(scan_rust(fp))
        elif fp.suffix == ".ori":
            findings.extend(scan_ori(fp))
    return findings


def self_test() -> int:
    must_flag = [
        "LexerV2", "ParserV10Fast", "parse_v2", "v3_emit", "lexer_v2_helper", "unescape_string_v2",
    ]
    # Exempt: domain tokens (bit-width / standard / arch) AND the revision WORDS
    # the lint deliberately leaves to NAME-33 reviewer-time (domain-collision-prone).
    must_pass = [
        "utf8", "sha256", "base64", "crc32", "fnv1a", "simd128", "x86_64", "arm64",
        "llvm21", "i64", "u8", "f64", "conv2d", "new", "with_capacity", "parse_expr",
        "from_utf8", "Vec2", "to_bytes", "resolver", "old_value", "new_value",
        "EmitterLegacy", "legacy_type_name", "deprecated_wasm32_wasi", "Resolver2",
    ]
    failures: list[str] = []
    for name in must_flag:
        if classify(name) is None:
            failures.append(f"FALSE NEGATIVE: {name!r} should flag but did not")
    for name in must_pass:
        cat = classify(name)
        if cat is not None:
            failures.append(f"FALSE POSITIVE: {name!r} flagged as {cat}")
    if failures:
        for f in failures:
            print(f"  {f}")
        print(f"\nself-test FAILED: {len(failures)} case(s).")
        return 1
    print(f"self-test ok: {len(must_flag)} flagged, {len(must_pass)} exempt.")
    return 0


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("paths", nargs="*", help="files or directories to scan")
    ap.add_argument("--json", action="store_true", help="emit JSON findings")
    ap.add_argument("--warn-only", action="store_true", help="report findings without exit 1")
    ap.add_argument("--exit-zero", action="store_true", help="alias of --warn-only")
    ap.add_argument("--self-test", action="store_true", help="run embedded positive/negative cases")
    args = ap.parse_args(argv)

    if args.self_test:
        return self_test()

    if args.paths:
        roots = [Path(p) for p in args.paths]
    else:
        repo = Path(__file__).resolve().parents[1]
        roots = [repo / "compiler", repo / "library", repo / "tests"]
        roots = [r for r in roots if r.exists()]

    findings = run(roots)

    if args.json:
        json.dump([f.to_dict() for f in findings], sys.stdout, indent=2)
        sys.stdout.write("\n")
    else:
        for f in findings:
            print(f"{f.file}:{f.line}: {f.name} [NAMING:version-suffix/{f.category}]")
        if findings:
            print(f"\nTotal: {len(findings)} version-naming violation(s).")
            print("Rule NAME-33: name the current item plainly; move version/provenance out of the identifier.")
        else:
            print("Clean: no version-naming violations found.")

    if findings and not (args.warn_only or args.exit_zero):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
