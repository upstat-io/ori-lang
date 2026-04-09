#!/usr/bin/env python3
"""
enum-drift.py — Cross-file enum coverage analyzer for Ori compiler.

Finds all variants of key IR enums (CanExpr, ExprKind, TypeTag, etc.),
locates every match expression on those enums across the codebase, and
compares arm coverage. Flags missing variants, catch-all arms, and
arm-count mismatches between files.

This is the highest-value hygiene tool: Rust's exhaustive match only
catches missing arms within a single crate. When CanExpr is defined in
ori_ir but matched in ori_eval and ori_llvm, adding a variant to ori_ir
compiles fine — the other crates hit their `_ => unreachable!()` at
runtime.

Known Enums (auto-discovered or --enum flag):

  CanExpr       Canonical expression IR (ori_ir → ori_eval, ori_llvm)
  ExprKind      AST expression variants (ori_ir → ori_canon, ori_fmt)
  TypeTag       Type identity discriminant (ori_registry → ori_types,
                ori_eval, ori_llvm, ori_arc)
  DerivedTrait  Derivable traits (ori_ir → ori_types, ori_eval, ori_llvm)
  TokenKind     Lexer token types (ori_ir → ori_parse)
  CollectionMethod  Iterator/collection dispatch (ori_eval internal)
  IteratorValue Iterator state variants (ori_patterns → ori_eval)

Usage:

  enum-drift.py                      # Analyze all known enums
  enum-drift.py --enum TypeTag       # Analyze specific enum
  enum-drift.py --scope compiler/ori_eval/  # Restrict match search
  enum-drift.py --json               # Machine-readable output
  enum-drift.py --summary            # Counts only

Exit codes: 0 = no drift, 1 = drift found
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
COMPILER_DIR = REPO_ROOT / "compiler"

# ─── Color helpers ───────────────────────────────────────────

_use_color = sys.stdout.isatty()

def _c(code: str, text: str) -> str:
    return f"\033[{code}m{text}\033[0m" if _use_color else text

def red(t: str) -> str: return _c("31", t)
def yellow(t: str) -> str: return _c("33", t)
def green(t: str) -> str: return _c("32", t)
def cyan(t: str) -> str: return _c("36", t)
def bold(t: str) -> str: return _c("1", t)
def dim(t: str) -> str: return _c("2", t)


# ─── Known enum definitions ─────────────────────────────────

@dataclass
class EnumDef:
    """Definition of a tracked enum."""
    name: str
    file: str           # relative to REPO_ROOT
    defining_crate: str
    consumer_crates: list[str]
    description: str

KNOWN_ENUMS: dict[str, EnumDef] = {
    "CanExpr": EnumDef(
        "CanExpr",
        "compiler/ori_ir/src/canon/expr.rs",
        "ori_ir",
        ["ori_eval", "ori_llvm", "ori_arc", "ori_canon"],
        "Canonical expression IR — matched in evaluator and LLVM codegen",
    ),
    "ExprKind": EnumDef(
        "ExprKind",
        "compiler/ori_ir/src/ast/expr.rs",
        "ori_ir",
        ["ori_canon", "ori_fmt", "ori_parse"],
        "AST expression variants — matched in canonicalization and formatting",
    ),
    "TypeTag": EnumDef(
        "TypeTag",
        "compiler/ori_registry/src/tags/mod.rs",
        "ori_registry",
        ["ori_types", "ori_eval", "ori_llvm", "ori_arc"],
        "Type identity discriminant — matched in all downstream phases",
    ),
    "DerivedTrait": EnumDef(
        "DerivedTrait",
        "compiler/ori_ir/src/derives/mod.rs",
        "ori_ir",
        ["ori_types", "ori_eval", "ori_llvm", "ori_arc"],
        "Derivable trait list — 4 sync points must stay aligned",
    ),
    "TokenKind": EnumDef(
        "TokenKind",
        "compiler/ori_ir/src/token/kind.rs",
        "ori_ir",
        ["ori_parse", "ori_lexer", "ori_fmt"],
        "Lexer token types — matched in parser",
    ),
    "CollectionMethod": EnumDef(
        "CollectionMethod",
        "compiler/ori_eval/src/interpreter/resolvers/mod.rs",
        "ori_eval",
        [],
        "Collection/iterator method dispatch — internal to evaluator",
    ),
    "IteratorValue": EnumDef(
        "IteratorValue",
        "compiler/ori_patterns/src/value/iterator/mod.rs",
        "ori_patterns",
        ["ori_eval"],
        "Iterator state variants — matched in evaluator iterator dispatch",
    ),
}


# ─── Data structures ────────────────────────────────────────

@dataclass
class EnumVariant:
    """A variant of an enum."""
    name: str
    line: int
    has_fields: bool = False

@dataclass
class MatchSite:
    """A location where an enum is matched."""
    file: str
    line: int
    arms: list[str]         # variant names found in match arms
    has_catch_all: bool     # has `_ =>` or `.. =>` arm
    arm_count: int

@dataclass
class DriftFinding:
    """A drift finding between enum definition and match site."""
    enum_name: str
    match_file: str
    match_line: int
    missing_variants: list[str]
    has_catch_all: bool
    severity: str   # "critical" if cross-crate, "major" if same crate
    message: str

    def to_dict(self) -> dict:
        return {
            "enum": self.enum_name,
            "file": self.match_file,
            "line": self.match_line,
            "missing_variants": self.missing_variants,
            "has_catch_all": self.has_catch_all,
            "severity": self.severity,
            "message": self.message,
        }


def rel_path(path: Path) -> str:
    try:
        return str(path.relative_to(REPO_ROOT))
    except ValueError:
        return str(path)


# ─── Enum variant extraction ────────────────────────────────

# Matches: `VariantName` or `VariantName(...)` or `VariantName { ... }`
VARIANT_RE = re.compile(r"^\s+(\w+)(?:\s*[({]|\s*,\s*$|\s*$)")

def extract_variants(enum_file: Path, enum_name: str) -> list[EnumVariant]:
    """Parse an enum definition and extract all variant names."""
    try:
        text = enum_file.read_text(encoding="utf-8")
    except OSError:
        return []

    lines = text.splitlines()
    variants: list[EnumVariant] = []
    in_enum = False
    depth = 0

    # Find the enum definition
    enum_pat = re.compile(rf"^\s*pub\s+enum\s+{re.escape(enum_name)}\s*")

    for i, line in enumerate(lines):
        if not in_enum:
            if enum_pat.match(line):
                in_enum = True
                depth = 0
                for ch in line:
                    if ch == '{':
                        depth += 1
                    elif ch == '}':
                        depth -= 1
                continue
        else:
            for ch in line:
                if ch == '{':
                    depth += 1
                elif ch == '}':
                    depth -= 1

            if depth <= 0:
                break

            # Only look at depth 1 (top-level variants)
            if depth == 1:
                stripped = line.strip()
                # Skip comments, blank lines, attributes
                if not stripped or stripped.startswith("//") or stripped.startswith("#["):
                    continue
                # Extract variant name
                m = VARIANT_RE.match(line)
                if m:
                    vname = m.group(1)
                    # Skip Rust keywords that might appear
                    if vname in ("pub", "fn", "let", "type", "use", "mod", "impl",
                                 "struct", "enum", "trait", "const", "static", "where"):
                        continue
                    has_fields = "(" in line or "{" in stripped
                    variants.append(EnumVariant(vname, i + 1, has_fields))

    return variants


# ─── Match site discovery ────────────────────────────────────

def _collect_rs_files(scope_paths: list[Path]) -> list[Path]:
    """Walk scope directories once, collect all .rs files (excluding target/)."""
    files: list[Path] = []
    for scope in scope_paths:
        if scope.is_file() and scope.suffix == ".rs":
            files.append(scope)
        elif scope.is_dir():
            for root, dirs, filenames in os.walk(scope):
                dirs[:] = [d for d in dirs if d not in ("target", ".git")]
                for fn in filenames:
                    if fn.endswith(".rs"):
                        files.append(Path(root) / fn)
    return files


def find_match_sites(
    enum_name: str,
    variant_names: set[str],
    all_files: list[Path],
) -> list[MatchSite]:
    """Find match blocks on an enum using regex-only (no brace counting).

    Strategy: for each file, use a single compiled regex to find all
    `EnumName::VariantName` with their line numbers. Group consecutive
    hits (within 50 lines) into "match blocks". Check for catch-all
    `_ =>` near each block. No brace counting needed.
    """
    sites: list[MatchSite] = []
    qualified = f"{enum_name}::"

    # Pre-compile the variant extraction regex
    variant_re = re.compile(rf"{re.escape(enum_name)}::(\w+)")
    catch_all_re = re.compile(r"^\s*_\s*=>")

    for path in all_files:
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue

        # Fast skip: no reference at all
        if qualified not in text:
            continue

        lines = text.splitlines()
        fpath = rel_path(path)

        # Collect all (line_number, variant_name) hits
        hits: list[tuple[int, str]] = []
        catch_all_lines: set[int] = set()

        for i, line in enumerate(lines):
            if qualified in line:
                for m in variant_re.finditer(line):
                    vname = m.group(1)
                    if vname in variant_names:
                        hits.append((i, vname))
            if catch_all_re.match(line):
                catch_all_lines.add(i)

        if len(hits) < 3:
            continue

        # Group consecutive hits into match blocks (gap > 50 lines = new block)
        blocks: list[list[tuple[int, str]]] = []
        current: list[tuple[int, str]] = [hits[0]]

        for h in hits[1:]:
            if h[0] - current[-1][0] > 50:
                blocks.append(current)
                current = [h]
            else:
                current.append(h)
        blocks.append(current)

        # Convert blocks to MatchSites
        for block in blocks:
            if len(block) < 3:
                continue

            arms = list(dict.fromkeys(vname for _, vname in block))
            first_line = block[0][0]
            last_line = block[-1][0]

            # Check for catch-all near this block
            has_catch = any(
                cl >= first_line - 5 and cl <= last_line + 5
                for cl in catch_all_lines
            )

            sites.append(MatchSite(
                file=fpath,
                line=first_line + 1,
                arms=arms,
                has_catch_all=has_catch,
                arm_count=len(arms),
            ))

    return sites


# ─── Drift analysis ─────────────────────────────────────────

def analyze_drift(
    enum_name: str,
    enum_def: EnumDef,
    variants: list[EnumVariant],
    match_sites: list[MatchSite],
) -> list[DriftFinding]:
    """Compare enum variants against match sites, report drift."""
    findings: list[DriftFinding] = []
    variant_names = {v.name for v in variants}

    for site in match_sites:
        matched_names = set(site.arms)
        missing = variant_names - matched_names

        if not missing and not site.has_catch_all:
            continue  # Fully covered, no issues

        # Determine if cross-crate
        site_crate = _crate_from_path(site.file)
        is_cross_crate = site_crate != enum_def.defining_crate
        severity = "critical" if is_cross_crate else "major"

        if missing and not site.has_catch_all:
            # Missing variants WITHOUT catch-all — compile error would catch this
            # in same crate, but cross-crate it's silent
            findings.append(DriftFinding(
                enum_name=enum_name,
                match_file=site.file,
                match_line=site.line,
                missing_variants=sorted(missing),
                has_catch_all=False,
                severity=severity,
                message=(
                    f"match on {enum_name} missing {len(missing)} variant(s): "
                    f"{', '.join(sorted(missing)[:8])}"
                    f"{'...' if len(missing) > 8 else ''}"
                ),
            ))
        elif site.has_catch_all:
            # Has catch-all — variants may be implicitly handled but this hides drift
            unhandled = variant_names - matched_names
            if unhandled:
                findings.append(DriftFinding(
                    enum_name=enum_name,
                    match_file=site.file,
                    match_line=site.line,
                    missing_variants=sorted(unhandled),
                    has_catch_all=True,
                    severity="major" if is_cross_crate else "minor",
                    message=(
                        f"match on {enum_name} has catch-all `_ =>` hiding "
                        f"{len(unhandled)} variant(s): "
                        f"{', '.join(sorted(unhandled)[:8])}"
                        f"{'...' if len(unhandled) > 8 else ''}"
                    ),
                ))

    return findings


def _crate_from_path(file_path: str) -> str:
    """Extract crate name from a file path like compiler/ori_eval/src/..."""
    parts = Path(file_path).parts
    for i, part in enumerate(parts):
        if part == "compiler" and i + 1 < len(parts):
            return parts[i + 1]
    return ""


# ─── Reporters ───────────────────────────────────────────────

def report_text(
    results: dict[str, tuple[list[EnumVariant], list[MatchSite], list[DriftFinding]]],
) -> None:
    """Human-readable report."""
    total_findings = sum(len(f) for _, _, f in results.values())

    if total_findings == 0:
        print(green("✓ No enum drift found."))
        return

    for enum_name, (variants, sites, findings) in results.items():
        if not findings:
            print(f"\n{green('✓')} {bold(enum_name)}: {len(variants)} variants, "
                  f"{len(sites)} match sites — no drift")
            continue

        print(f"\n{bold(red(f'✗ {enum_name}'))}: {len(variants)} variants, "
              f"{len(sites)} match sites, {bold(str(len(findings)))} findings")

        for f in sorted(findings, key=lambda x: (x.severity, x.match_file)):
            sev_color = red if f.severity == "critical" else yellow
            loc = f"{f.match_file}:{f.match_line}"
            catch = " [catch-all]" if f.has_catch_all else ""
            print(f"  {sev_color(f'[{f.severity}]')} {cyan(loc)}{catch}")
            print(f"    {f.message}")

    # Summary
    print(f"\n{bold('─── Summary ───')}")
    print(f"  Enums analyzed: {len(results)}")
    total_sites = sum(len(s) for _, s, _ in results.values())
    print(f"  Match sites found: {total_sites}")
    print(f"  Drift findings: {bold(str(total_findings))}")

    by_sev: dict[str, int] = defaultdict(int)
    for _, _, findings in results.values():
        for f in findings:
            by_sev[f.severity] += 1
    for sev in ["critical", "major", "minor"]:
        if by_sev[sev]:
            print(f"  {sev}: {by_sev[sev]}")


def report_json(
    results: dict[str, tuple[list[EnumVariant], list[MatchSite], list[DriftFinding]]],
) -> None:
    """Machine-readable JSON output."""
    output = {"enums": {}}
    total = 0
    for enum_name, (variants, sites, findings) in results.items():
        total += len(findings)
        output["enums"][enum_name] = {
            "variant_count": len(variants),
            "variants": [v.name for v in variants],
            "match_sites": len(sites),
            "findings": [f.to_dict() for f in findings],
        }
    output["total_findings"] = total
    print(json.dumps(output, indent=2))


def report_summary(
    results: dict[str, tuple[list[EnumVariant], list[MatchSite], list[DriftFinding]]],
) -> None:
    """Summary counts only."""
    print(f"{'Enum':<22s} {'Variants':>8s} {'Sites':>6s} {'Drift':>6s}")
    print("─" * 50)
    total = 0
    for enum_name, (variants, sites, findings) in results.items():
        count = len(findings)
        total += count
        indicator = red(f"{count:6d}") if count else green(f"{count:6d}")
        print(f"{enum_name:<22s} {len(variants):8d} {len(sites):6d} {indicator}")
    print("─" * 50)
    print(f"{'Total':<22s} {'':>8s} {'':>6s} {bold(f'{total:6d}')}")


# ─── CLI ─────────────────────────────────────────────────────

def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="enum-drift",
        description="Cross-file enum coverage analyzer for Ori compiler.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=f"Known enums: {', '.join(sorted(KNOWN_ENUMS.keys()))}",
    )
    p.add_argument(
        "--enum", nargs="+", metavar="NAME",
        help="Specific enum(s) to analyze (default: all known)",
    )
    p.add_argument(
        "--scope", nargs="+", metavar="PATH",
        help="Paths to search for match sites (default: compiler/)",
    )
    p.add_argument("--json", action="store_true", help="JSON output")
    p.add_argument("--summary", action="store_true", help="Summary counts only")
    p.add_argument("--no-color", action="store_true", help="Disable color")
    p.add_argument(
        "--list-enums", action="store_true",
        help="List all known enums and exit",
    )
    return p


def main() -> int:
    global _use_color
    parser = build_parser()
    args = parser.parse_args()

    if args.no_color:
        _use_color = False

    if args.list_enums:
        print(f"{'Enum':<22s} {'Crate':<16s} {'Consumers':<40s} Description")
        print("─" * 100)
        for name, edef in sorted(KNOWN_ENUMS.items()):
            consumers = ", ".join(edef.consumer_crates) or "(internal)"
            print(f"{name:<22s} {edef.defining_crate:<16s} {consumers:<40s} {edef.description}")
        return 0

    # Determine which enums to analyze
    if args.enum:
        target_enums = {}
        for name in args.enum:
            if name not in KNOWN_ENUMS:
                print(f"Unknown enum: {name}", file=sys.stderr)
                print(f"Known: {', '.join(sorted(KNOWN_ENUMS.keys()))}", file=sys.stderr)
                return 1
            target_enums[name] = KNOWN_ENUMS[name]
    else:
        target_enums = KNOWN_ENUMS

    # Determine scope for match site search
    if args.scope:
        scope_paths = []
        for s in args.scope:
            p = Path(s)
            if not p.is_absolute():
                p = REPO_ROOT / p
            scope_paths.append(p)
    else:
        scope_paths = [COMPILER_DIR]

    # Collect all .rs files once — shared across all enum analyses
    all_files = _collect_rs_files(scope_paths)

    # Analyze each enum
    results: dict[str, tuple[list[EnumVariant], list[MatchSite], list[DriftFinding]]] = {}

    for enum_name, enum_def in target_enums.items():
        enum_file = REPO_ROOT / enum_def.file
        variants = extract_variants(enum_file, enum_name)
        if not variants:
            print(f"Warning: could not extract variants for {enum_name} from {enum_def.file}",
                  file=sys.stderr)
            continue

        variant_names = {v.name for v in variants}
        match_sites = find_match_sites(enum_name, variant_names, all_files)
        findings = analyze_drift(enum_name, enum_def, variants, match_sites)
        results[enum_name] = (variants, match_sites, findings)

    # Report
    if args.json:
        report_json(results)
    elif args.summary:
        report_summary(results)
    else:
        report_text(results)

    total_findings = sum(len(f) for _, _, f in results.values())
    return 1 if total_findings else 0


if __name__ == "__main__":
    sys.exit(main())
