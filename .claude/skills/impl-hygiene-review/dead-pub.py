#!/usr/bin/env python3
"""
dead-pub.py — Dead public item detector for Ori compiler.

Collects all `pub fn`, `pub struct`, `pub enum`, `pub trait`, `pub type`,
`pub const`, `pub static` declarations, then cross-references usage across
the codebase. Reports items that appear to be pub but are only used within
their own crate (could be pub(crate)) or not used at all.

This is a heuristic tool — it uses grep-based cross-referencing, not
full Rust name resolution. False positives are possible for:
- Items used only via re-exports
- Items used in macros
- Items used in doc tests
- Generic trait impls that are never called directly

Usage:

  dead-pub.py                          # Scan all compiler crates
  dead-pub.py --scope compiler/ori_types/  # Scan specific crate
  dead-pub.py --json                   # Machine-readable output
  dead-pub.py --summary                # Counts only

Exit codes: 0 = clean, 1 = dead pub items found
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
COMPILER_DIR = REPO_ROOT / "compiler"

_use_color = sys.stdout.isatty()
def _c(code: str, text: str) -> str:
    return f"\033[{code}m{text}\033[0m" if _use_color else text
def red(t: str) -> str: return _c("31", t)
def yellow(t: str) -> str: return _c("33", t)
def green(t: str) -> str: return _c("32", t)
def cyan(t: str) -> str: return _c("36", t)
def bold(t: str) -> str: return _c("1", t)
def dim(t: str) -> str: return _c("2", t)


@dataclass
class PubItem:
    kind: str       # fn, struct, enum, trait, type, const, static
    name: str
    file: str       # relative to REPO_ROOT
    line: int
    crate_name: str
    visibility: str  # "pub", "pub(crate)", "pub(super)"

@dataclass
class DeadPubFinding:
    item: PubItem
    usage_in_crate: int     # usage count within same crate
    usage_outside: int      # usage count in other crates
    recommendation: str     # "remove pub", "pub(crate)", "investigate"

    def to_dict(self) -> dict:
        return {
            "kind": self.item.kind,
            "name": self.item.name,
            "file": self.item.file,
            "line": self.item.line,
            "crate": self.item.crate_name,
            "usage_in_crate": self.usage_in_crate,
            "usage_outside": self.usage_outside,
            "recommendation": self.recommendation,
        }


def rel_path(path: Path) -> str:
    try:
        return str(path.relative_to(REPO_ROOT))
    except ValueError:
        return str(path)


def crate_from_path(file_path: str) -> str:
    parts = Path(file_path).parts
    for i, part in enumerate(parts):
        if part == "compiler" and i + 1 < len(parts):
            return parts[i + 1]
    return ""


# ─── Pub item extraction ────────────────────────────────────

PUB_ITEM_RE = re.compile(
    r"^\s*pub\s+"
    r"(?:(?:async|const|unsafe)\s+)*"
    r"(fn|struct|enum|trait|type|const|static)\s+"
    r"(\w+)"
)

# Skip pub(crate) and pub(super) — those are already narrowed
PUB_CRATE_RE = re.compile(r"^\s*pub\s*\(\s*(?:crate|super)\s*\)")


def extract_pub_items(files: list[Path]) -> list[PubItem]:
    """Extract all `pub` declarations from files."""
    items: list[PubItem] = []

    for path in files:
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue

        rp = rel_path(path)
        crate = crate_from_path(rp)

        for i, line in enumerate(text.splitlines()):
            # Skip pub(crate) and pub(super) — already narrowed
            if PUB_CRATE_RE.match(line):
                continue

            m = PUB_ITEM_RE.match(line)
            if m:
                kind = m.group(1)
                name = m.group(2)

                # Skip common false positives
                if name in ("self", "super", "crate", "Self"):
                    continue
                # Skip test-only items
                if "#[cfg(test)]" in line:
                    continue
                # Skip very short names that are too generic to grep
                if len(name) <= 2:
                    continue

                items.append(PubItem(
                    kind=kind,
                    name=name,
                    file=rp,
                    line=i + 1,
                    crate_name=crate,
                    visibility="pub",
                ))

    return items


# ─── Usage search ────────────────────────────────────────────

def count_usages(
    name: str,
    file_cache: dict[str, str],
    defining_crate: str,
    defining_file: str,
) -> tuple[int, int]:
    """Count usages of a name in-crate and outside-crate.

    Returns (in_crate_count, outside_crate_count).
    Excludes the defining line itself.
    """
    # Simple word-boundary search
    pattern = re.compile(rf"\b{re.escape(name)}\b")

    in_crate = 0
    outside = 0

    for filepath, text in file_cache.items():
        if name not in text:  # fast pre-filter
            continue

        crate = crate_from_path(filepath)
        count = len(pattern.findall(text))

        # Subtract the definition itself if same file
        if filepath == defining_file:
            count = max(0, count - 1)

        if count > 0:
            if crate == defining_crate:
                in_crate += count
            else:
                outside += count

    return in_crate, outside


# ─── Analysis ────────────────────────────────────────────────

def analyze_dead_pub(
    items: list[PubItem],
    file_cache: dict[str, str],
) -> list[DeadPubFinding]:
    """Find pub items that may not need to be pub."""
    findings: list[DeadPubFinding] = []

    for item in items:
        in_crate, outside = count_usages(
            item.name, file_cache, item.crate_name, item.file,
        )

        if outside == 0 and in_crate == 0:
            findings.append(DeadPubFinding(
                item=item,
                usage_in_crate=0,
                usage_outside=0,
                recommendation="dead code — remove or mark #[cfg(test)]",
            ))
        elif outside == 0 and in_crate > 0:
            findings.append(DeadPubFinding(
                item=item,
                usage_in_crate=in_crate,
                usage_outside=0,
                recommendation="pub(crate) — only used within crate",
            ))

    return findings


# ─── File collection ─────────────────────────────────────────

def collect_rs_files(scope_paths: list[Path]) -> list[Path]:
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


def read_file_cache(files: list[Path]) -> dict[str, str]:
    cache: dict[str, str] = {}
    for f in files:
        try:
            cache[rel_path(f)] = f.read_text(encoding="utf-8", errors="replace")
        except OSError:
            pass
    return cache


# ─── Reporters ───────────────────────────────────────────────

def report_text(findings: list[DeadPubFinding]) -> None:
    if not findings:
        print(green("✓ No dead pub items found."))
        return

    # Group by recommendation
    dead = [f for f in findings if f.usage_in_crate == 0]
    narrow = [f for f in findings if f.usage_in_crate > 0]

    if dead:
        print(f"\n{bold(red('Dead code (0 usages anywhere):'))}")
        for f in sorted(dead, key=lambda x: (x.item.file, x.item.line)):
            loc = f"{f.item.file}:{f.item.line}"
            print(f"  {cyan(loc)} — pub {f.item.kind} {bold(f.item.name)}")

    if narrow:
        print(f"\n{bold(yellow('Could be pub(crate) (used only within crate):'))}")
        for f in sorted(narrow, key=lambda x: (x.item.file, x.item.line)):
            loc = f"{f.item.file}:{f.item.line}"
            print(f"  {cyan(loc)} — pub {f.item.kind} {bold(f.item.name)} "
                  f"({f.usage_in_crate} in-crate uses)")

    print(f"\n{bold('─── Summary ───')}")
    print(f"  Dead code: {len(dead)}")
    print(f"  Narrowable to pub(crate): {len(narrow)}")
    print(f"  Total: {bold(str(len(findings)))}")


def report_json(findings: list[DeadPubFinding]) -> None:
    output = {
        "total": len(findings),
        "dead": sum(1 for f in findings if f.usage_in_crate == 0),
        "narrowable": sum(1 for f in findings if f.usage_in_crate > 0),
        "findings": [f.to_dict() for f in findings],
    }
    print(json.dumps(output, indent=2))


def report_summary(findings: list[DeadPubFinding]) -> None:
    dead = sum(1 for f in findings if f.usage_in_crate == 0)
    narrow = sum(1 for f in findings if f.usage_in_crate > 0)
    print(f"Dead pub items:       {red(str(dead)) if dead else green('0')}")
    print(f"Narrowable pub items: {yellow(str(narrow)) if narrow else green('0')}")
    print(f"Total:                {bold(str(len(findings)))}")


# ─── CLI ─────────────────────────────────────────────────────

def main() -> int:
    global _use_color

    p = argparse.ArgumentParser(
        prog="dead-pub",
        description="Dead public item detector for Ori compiler.",
    )
    p.add_argument("--scope", nargs="+", metavar="PATH",
                   help="Paths to scan for pub items (default: compiler/)")
    p.add_argument("--json", action="store_true", help="JSON output")
    p.add_argument("--summary", action="store_true", help="Summary counts only")
    p.add_argument("--no-color", action="store_true", help="Disable color")
    args = p.parse_args()

    if args.no_color:
        _use_color = False

    # Scope for pub item extraction
    if args.scope:
        scope_paths = [REPO_ROOT / s if not Path(s).is_absolute() else Path(s) for s in args.scope]
    else:
        scope_paths = [COMPILER_DIR]

    # Always search the full compiler tree for usages
    all_compiler_files = collect_rs_files([COMPILER_DIR])
    scope_files = collect_rs_files(scope_paths)

    # Read all files into cache
    file_cache = read_file_cache(all_compiler_files)

    # Extract pub items from scope
    pub_items = extract_pub_items(scope_files)

    # Analyze
    findings = analyze_dead_pub(pub_items, file_cache)

    # Report
    if args.json:
        report_json(findings)
    elif args.summary:
        report_summary(findings)
    else:
        report_text(findings)

    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
