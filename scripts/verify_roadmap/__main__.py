"""CLI entry point for verify-roadmap.

Section 03.5 of verify-roadmap-redesign plan.

Usage:
    python -m scripts.verify_roadmap --quick   # fast pre-check (BLOCKED + DEAD_REFERENCE only)
    python -m scripts.verify_roadmap --full    # full classifier sweep + auto-fix (TBD)

Exit codes (per §03.2):
    0 — clean (no findings, no unapplied fixes)
    1 — findings present (low/medium/high) or unapplied fixes
    2 — at least one critical finding

`--quick` mode is invoked by /continue-roadmap's roadmap-scan.sh as a
pre-check. `--full` is invoked manually for explicit verification +
auto-fix runs.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from .quick import run_quick
from .report import (
    exit_code_for_findings,
    render_console,
    write_reports,
)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="python -m scripts.verify_roadmap",
        description="Verify-roadmap cross-plan coherence checker.",
    )
    mode_group = parser.add_mutually_exclusive_group(required=True)
    mode_group.add_argument(
        "--quick",
        action="store_true",
        help="Fast pre-check: BLOCKED + DEAD_REFERENCE only, no auto-fix.",
    )
    mode_group.add_argument(
        "--full",
        action="store_true",
        help="Full classifier sweep with auto-fix (not yet implemented).",
    )
    parser.add_argument(
        "--no-auto-fix",
        action="store_true",
        help="Disable auto-fix in --full mode (report-only).",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("build/verify-roadmap"),
        help="Where to write findings.json + findings.md (default: build/verify-roadmap).",
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress console output (still writes JSON + markdown).",
    )
    parser.add_argument(
        "--no-color",
        action="store_true",
        help="Disable ANSI color codes in console output.",
    )

    args = parser.parse_args(argv)

    if args.quick:
        report = run_quick()
    else:
        # --full mode is the §03 plan's eventual full-mode runner.
        # For now, surface explicitly so callers don't get silent partial behavior.
        print(
            "ERROR: --full mode is not yet implemented. Use --quick for the "
            "pre-check surface (BLOCKED + DEAD_REFERENCE).",
            file=sys.stderr,
        )
        return 2

    if not args.quiet:
        print(render_console(report, color=not args.no_color))

    write_reports(report, output_dir=args.output_dir)
    return exit_code_for_findings(report)


if __name__ == "__main__":
    sys.exit(main())
