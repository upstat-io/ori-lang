#!/usr/bin/env python3
"""
hygiene-fix.py — Batch auto-fixer orchestrator for Ori compiler hygiene.

Runs the auto-fixable checks from hygiene-lint.py and applies fixes.
This is the "fix everything mechanical" command that runs before AI
review, cleaning up the surface so AI context is spent on architecture.

Fixable checks:
  banners         Remove decorative comment banners (// ===, // ---)
  plan-stale      Remove stale plan annotations (via plan-annotations.py)

Usage:

  hygiene-fix.py                        # Dry-run all fixable checks
  hygiene-fix.py --apply                # Apply all fixes
  hygiene-fix.py --checks banners       # Only fix banners
  hygiene-fix.py --scope compiler/ori_types/  # Restrict scope

Exit codes: 0 = nothing to fix, 1 = fixes applied, 2 = dry-run preview
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
SCRIPT_DIR = Path(__file__).resolve().parent

_use_color = sys.stdout.isatty()
def _c(code: str, text: str) -> str:
    return f"\033[{code}m{text}\033[0m" if _use_color else text
def bold(t: str) -> str: return _c("1", t)
def green(t: str) -> str: return _c("32", t)
def dim(t: str) -> str: return _c("2", t)

FIXABLE_CHECKS = {
    "banners":        "Remove decorative comment banners",
    "plan-stale":     "Remove stale plan annotations",
}


def run_hygiene_lint_fix(checks: list[str], scope: list[str], apply: bool) -> int:
    """Run hygiene-lint.py --fix for the given checks."""
    cmd = [
        sys.executable, str(SCRIPT_DIR / "hygiene-lint.py"),
        "--check", ",".join(checks),
        "--fix",
    ]
    if apply:
        cmd.append("--apply")
    if scope:
        cmd.extend(["--scope"] + scope)
    if not _use_color:
        cmd.append("--no-color")

    result = subprocess.run(cmd)
    return result.returncode


def run_plan_annotations_fix(scope: list[str], apply: bool) -> int:
    """Run plan-annotations.py --fix for stale annotations."""
    cmd = [
        sys.executable, str(SCRIPT_DIR / "plan-annotations.py"),
        "--fix",
    ]
    if apply:
        cmd.append("--apply")
    if scope:
        cmd.extend(["--scope"] + scope)

    result = subprocess.run(cmd)
    return result.returncode


def main() -> int:
    global _use_color

    p = argparse.ArgumentParser(
        prog="hygiene-fix",
        description="Batch auto-fixer for Ori compiler hygiene.",
        epilog=f"Fixable checks: {', '.join(FIXABLE_CHECKS.keys())}",
    )
    p.add_argument("--checks", metavar="CHECKS",
                   help="Comma-separated checks to fix (default: all)")
    p.add_argument("--scope", nargs="+", metavar="PATH",
                   help="Paths to fix (default: compiler/)")
    p.add_argument("--apply", action="store_true",
                   help="Apply fixes (default: dry-run preview)")
    p.add_argument("--no-color", action="store_true", help="Disable color")
    args = p.parse_args()

    if args.no_color:
        _use_color = False

    if args.checks:
        active = [c.strip() for c in args.checks.split(",")]
        for c in active:
            if c not in FIXABLE_CHECKS:
                print(f"Unknown fixable check: {c}", file=sys.stderr)
                print(f"Available: {', '.join(FIXABLE_CHECKS.keys())}", file=sys.stderr)
                return 1
    else:
        active = list(FIXABLE_CHECKS.keys())

    scope = args.scope or []
    mode = "Applying" if args.apply else "Preview"
    print(f"{bold(f'hygiene-fix — {mode}')}")
    print(f"Checks: {', '.join(active)}")
    if scope:
        print(f"Scope: {', '.join(scope)}")
    print()

    any_fixes = False

    # Run lint-based fixes (banners only — commented-code is not auto-fixable)
    lint_checks = [c for c in active if c == "banners"]
    if lint_checks:
        print(f"{bold('── hygiene-lint fixes ──')}")
        rc = run_hygiene_lint_fix(lint_checks, scope, args.apply)
        if rc == 2:  # dry-run with fixes available
            any_fixes = True
        elif rc == 0 and args.apply:
            # rc=0 with --apply means fixes were applied successfully
            any_fixes = True

    # Run plan annotation fixes
    if "plan-stale" in active:
        print(f"\n{bold('── plan-annotations fixes ──')}")
        rc = run_plan_annotations_fix(scope, args.apply)
        if rc == 2:
            any_fixes = True
        elif rc == 0 and args.apply:
            any_fixes = True

    if not any_fixes:
        print(f"\n{green('✓ Nothing to fix.')}")
        return 0

    if not args.apply:
        print(f"\n{dim('Run with --apply to write changes.')}")
        return 2

    return 0


if __name__ == "__main__":
    sys.exit(main())
