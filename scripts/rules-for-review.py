#!/usr/bin/env python3
"""
rules-for-review.py — Classify changed files into subsystems and resolve
which rule files a reviewer needs to read.

This script is the CLASSIFIER layer. It does NOT compose the reviewer brief —
that is done by a Sonnet subagent that reads the classified rule files and the
diff, then dynamically composes a focused brief via prompt engineering.

Outputs:
  --mode json    (default) JSON for programmatic consumption by the subagent
  --mode list    Human-readable numbered file list
  --mode paths   Bare file paths, one per line (for shell piping)

Usage:
    # Default scope (HEAD~5..HEAD + staged + unstaged)
    scripts/rules-for-review.py

    # Specific git range
    scripts/rules-for-review.py --diff HEAD~3..HEAD

    # Explicit files
    scripts/rules-for-review.py --files compiler/ori_arc/src/lower/mod.rs

    # Just the file paths for shell piping
    scripts/rules-for-review.py --mode paths
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from typing import Optional

# ---------------------------------------------------------------------------
# §1 — Subsystem Classification (path patterns → subsystem tags)
# ---------------------------------------------------------------------------

# Each pattern is tried in order; a file can match multiple subsystems.
# Patterns are prefix-matched against repo-relative paths.

SUBSYSTEM_PATTERNS: list[tuple[str, str]] = [
    # Type checker
    ("compiler/ori_types/src/infer/", "typeck"),
    ("compiler/ori_types/src/check/", "typeck"),
    ("compiler/ori_types/src/unify", "typeck"),
    ("compiler/ori_types/src/generalize", "typeck"),
    ("compiler/ori_types/src/engine", "typeck"),
    ("compiler/ori_types/src/bidirectional", "typeck"),
    ("compiler/ori_types/src/trait", "typeck"),
    ("compiler/ori_types/src/capability", "typeck"),
    ("compiler/ori_types/src/signature", "typeck"),
    ("compiler/ori_types/", "types"),

    # AIMS / ARC
    ("compiler/ori_arc/", "aims"),
    ("compiler/ori_rt/src/rc/", "aims"),
    ("compiler/ori_rt/src/rc.", "aims"),

    # LLVM codegen
    ("compiler/ori_llvm/", "codegen"),

    # Parser / lexer
    ("compiler/ori_parse/", "parser"),
    ("compiler/ori_lexer/", "parser"),

    # Evaluator
    ("compiler/ori_eval/", "eval"),

    # Pattern system
    ("compiler/ori_patterns/", "patterns"),

    # Canonicalization
    ("compiler/ori_canon/", "canon"),

    # Registry
    ("compiler/ori_registry/", "registry"),

    # IR
    ("compiler/ori_ir/", "ir"),

    # Diagnostics
    ("compiler/ori_diagnostic/", "diagnostic"),
    ("compiler/oric/src/diagnostic/", "diagnostic"),

    # Runtime
    ("compiler/ori_rt/", "runtime"),

    # Representation / layout
    ("compiler/ori_repr/", "repr"),
    ("compiler/ori_llvm/src/codegen/narrowing", "repr"),
    ("compiler/ori_llvm/src/codegen/type_", "repr"),

    # AOT pipeline
    ("compiler/oric/src/aot", "aot"),
    ("compiler/oric/src/compile", "aot"),
    ("compiler/oric/src/link", "aot"),

    # Stdlib
    ("library/std/", "stdlib"),

    # Tests
    ("tests/spec/", "tests"),
    ("tests/valgrind/", "tests"),
    ("tests/alive2/", "tests"),
    ("tests/benchmarks/", "tests"),
    ("compiler/ori_llvm/tests/aot/", "tests"),
    ("compiler/oric/tests/", "tests"),

    # Formatter
    ("compiler/ori_fmt/", "fmt"),

    # Plans / docs
    ("plans/", "plans"),
    ("docs/ori_lang/v2026/spec/", "spec"),
    ("docs/ori_lang/proposals/", "proposals"),

    # Skills / tooling
    (".claude/", "skills"),
    (".codex/", "skills"),
    (".gemini/", "skills"),
    ("scripts/", "tooling"),
    ("diagnostics/", "tooling"),
]


def classify_files(files: list[str]) -> set[str]:
    """Map a list of repo-relative file paths to subsystem tags."""
    subsystems: set[str] = set()
    for f in files:
        matched = False
        for pattern, subsystem in SUBSYSTEM_PATTERNS:
            if f.startswith(pattern):
                subsystems.add(subsystem)
                matched = True
        if not matched:
            if f.startswith("compiler/"):
                subsystems.add("compiler-general")
    return subsystems


# ---------------------------------------------------------------------------
# §2 — Subsystem → Rule File Mapping (file-level only, stable)
# ---------------------------------------------------------------------------

# "always" files are included in every review.
# "subsystem" files are included only when that subsystem is detected.
# Priority: "critical" = reviewer MUST read, "reference" = consult if needed.

ALWAYS_FILES: list[dict] = [
    {"file": "impl-hygiene.md", "priority": "critical",
     "why": "Finding categories (LEAK/DRIFT/GAP/WASTE), SSOT, No Side Logic, phase boundaries"},
    {"file": "tests.md", "priority": "critical",
     "why": "Matrix testing, TDD for bugs, negative pin protocol, regression discipline"},
    {"file": "compiler.md", "priority": "critical",
     "why": "Architecture, phase purity, tracing, crate structure"},
]

SUBSYSTEM_FILES: dict[str, list[dict]] = {
    "typeck": [
        {"file": "typeck.md", "priority": "critical",
         "why": "Type checker rules — check pipeline, inference engine, unification, bidirectional checking"},
        {"file": "types.md", "priority": "reference",
         "why": "Type pool architecture, tag catalog, interning, type flags"},
    ],
    "types": [
        {"file": "types.md", "priority": "critical",
         "why": "Type pool architecture, tag catalog, interning, type flags"},
    ],
    "aims": [
        {"file": "aims-rules.md", "priority": "critical",
         "why": "AIMS lattice dimensions, canonicalization, decision predicates, pipeline ordering, realization"},
        {"file": "arc.md", "priority": "critical",
         "why": "ARC mission, non-negotiable invariants, IR instructions, protocol builtins"},
    ],
    "codegen": [
        {"file": "codegen-rules.md", "priority": "critical",
         "why": "Type resolution, ABI computation, narrowing boundaries, trampolines"},
        {"file": "llvm.md", "priority": "critical",
         "why": "FastISel bug, loop latch pattern, ARM portability, Inkwell pitfalls"},
    ],
    "parser": [
        {"file": "parse.md", "priority": "critical",
         "why": "Cursor primitives, context flags, parse outcomes, error recovery, Pratt parsing"},
    ],
    "eval": [
        {"file": "eval.md", "priority": "critical",
         "why": "Evaluator architecture, method dispatch chain, value types, derived methods"},
    ],
    "patterns": [
        {"file": "patterns.md", "priority": "critical",
         "why": "PatternDefinition trait, pattern registry, PatternExecutor"},
    ],
    "canon": [
        {"file": "canonicalization.md", "priority": "critical",
         "why": "Pipeline position, 7 canonical-IR desugars, pattern compilation, output invariants"},
    ],
    "registry": [
        {"file": "registry.md", "priority": "critical",
         "why": "Key types, query API, adding types/methods, sync points, invariants"},
    ],
    "ir": [
        {"file": "ir.md", "priority": "critical",
         "why": "Arena allocation, ID newtypes, TypeId layout, DerivedTrait"},
    ],
    "repr": [
        {"file": "repr.md", "priority": "critical",
         "why": "ReprPlan, composite type representations, integer narrowing scope policy"},
    ],
    "runtime": [
        {"file": "runtime.md", "priority": "critical",
         "why": "FFI conventions, type representations, runtime functions"},
    ],
    "aot": [
        {"file": "aot.md", "priority": "critical",
         "why": "AOT pipeline, runtime discovery, symbol mangling, optimization"},
    ],
    "diagnostic": [
        {"file": "diagnostic.md", "priority": "critical",
         "why": "Error codes, diagnostic structure, message style, error code stability"},
    ],
    "fmt": [
        {"file": "fmt.md", "priority": "reference",
         "why": "Formatter rules"},
    ],
    "stdlib": [
        {"file": "registry.md", "priority": "critical",
         "why": "Adding methods, sync points (stdlib changes touch the registry)"},
    ],
    "tests": [
        # tests.md is already in ALWAYS_FILES — no additional files needed
    ],
    "spec": [
        {"file": "spec.md", "priority": "reference",
         "why": "Spec structure and clause references"},
    ],
    "proposals": [
        {"file": "proposals.md", "priority": "reference",
         "why": "Proposal format and structure"},
    ],
    "plans": [],
    "skills": [],
    "tooling": [
        {"file": "diagnostic.md", "priority": "reference",
         "why": "Diagnostic scripts reference table"},
    ],
    "compiler-general": [],
}


# ---------------------------------------------------------------------------
# §3 — Output Composition
# ---------------------------------------------------------------------------

def build_result(
    changed_files: list[str],
    subsystems: set[str],
) -> dict:
    """Build the structured result for all output modes."""
    # Collect rule files, deduplicated, preserving priority
    seen: dict[str, dict] = {}

    for entry in ALWAYS_FILES:
        key = entry["file"]
        seen[key] = {**entry, "source": "always"}

    for subsys in sorted(subsystems):
        for entry in SUBSYSTEM_FILES.get(subsys, []):
            key = entry["file"]
            if key not in seen:
                seen[key] = {**entry, "source": subsys}
            elif seen[key]["priority"] == "reference" and entry["priority"] == "critical":
                # Upgrade priority if a subsystem needs it as critical
                seen[key]["priority"] = "critical"
                seen[key]["source"] = subsys

    critical = [v for v in seen.values() if v["priority"] == "critical"]
    reference = [v for v in seen.values() if v["priority"] == "reference"]

    return {
        "changed_files": changed_files,
        "subsystems": sorted(subsystems),
        "rules": {
            "critical": [
                {"file": f".claude/rules/{e['file']}", "why": e["why"]}
                for e in sorted(critical, key=lambda x: x["file"])
            ],
            "reference": [
                {"file": f".claude/rules/{e['file']}", "why": e["why"]}
                for e in sorted(reference, key=lambda x: x["file"])
            ],
        },
    }


def format_list(result: dict) -> str:
    """Human-readable numbered list."""
    parts = ["## Rules for this review\n"]
    parts.append("**Critical (reviewer MUST read):**")
    for i, entry in enumerate(result["rules"]["critical"], 1):
        parts.append(f"  {i}. `{entry['file']}` — {entry['why']}")

    if result["rules"]["reference"]:
        parts.append("\n**Reference (consult for deeper context):**")
        for entry in result["rules"]["reference"]:
            parts.append(f"  - `{entry['file']}` — {entry['why']}")

    parts.append(f"\nSubsystems: {', '.join(result['subsystems'])}")
    return "\n".join(parts)


def format_paths(result: dict) -> str:
    """Bare file paths, one per line."""
    paths = [e["file"] for e in result["rules"]["critical"]]
    paths += [e["file"] for e in result["rules"]["reference"]]
    return "\n".join(paths)


# ---------------------------------------------------------------------------
# §4 — Git Integration
# ---------------------------------------------------------------------------

def get_changed_files(diff_range: Optional[str] = None) -> list[str]:
    """Get changed files from git."""
    files: set[str] = set()

    if diff_range:
        result = subprocess.run(
            ["git", "diff", "--name-only", diff_range],
            capture_output=True, text=True,
        )
        if result.returncode == 0:
            files.update(f.strip() for f in result.stdout.splitlines() if f.strip())
    else:
        for cmd in [
            ["git", "diff", "--name-only", "HEAD~5..HEAD"],
            ["git", "diff", "--name-only", "--cached"],
            ["git", "diff", "--name-only"],
        ]:
            result = subprocess.run(cmd, capture_output=True, text=True)
            if result.returncode == 0:
                files.update(f.strip() for f in result.stdout.splitlines() if f.strip())

    return sorted(files)


# ---------------------------------------------------------------------------
# §5 — CLI
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(
        description="Classify changed files into subsystems and resolve relevant rule files.",
        epilog="Examples:\n"
               "  scripts/rules-for-review.py                              # JSON, default scope\n"
               "  scripts/rules-for-review.py --diff HEAD~3..HEAD          # specific range\n"
               "  scripts/rules-for-review.py --files compiler/ori_arc/src/lower/mod.rs\n"
               "  scripts/rules-for-review.py --mode list                  # human-readable\n"
               "  scripts/rules-for-review.py --mode paths                 # bare paths\n",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--diff", metavar="RANGE",
        help="Git diff range (e.g., HEAD~3..HEAD). Default: HEAD~5..HEAD + staged + unstaged",
    )
    parser.add_argument(
        "--files", nargs="+", metavar="FILE",
        help="Explicit list of repo-relative file paths",
    )
    parser.add_argument(
        "--mode", choices=["json", "list", "paths"], default="json",
        help="Output mode (default: json)",
    )

    args = parser.parse_args()

    if args.files:
        changed_files = args.files
    else:
        changed_files = get_changed_files(args.diff)

    if not changed_files:
        print("No changed files detected.", file=sys.stderr)
        sys.exit(1)

    subsystems = classify_files(changed_files)
    if not subsystems:
        subsystems = {"compiler-general"}

    result = build_result(changed_files, subsystems)

    if args.mode == "json":
        print(json.dumps(result, indent=2))
    elif args.mode == "list":
        print(format_list(result))
    elif args.mode == "paths":
        print(format_paths(result))


if __name__ == "__main__":
    main()
