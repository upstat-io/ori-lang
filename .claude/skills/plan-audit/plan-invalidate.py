#!/usr/bin/env python3
"""Cross-plan review invalidation detector.

When a new plan is created or an existing plan is significantly changed,
sections in OTHER active plans with `reviewed: true` may be stale if their
scope (files, symbols, crates) overlaps with the changed plan's scope.

This script detects those stale reviews and optionally flips them to
`reviewed: false` so that `/continue-roadmap` will re-review them before
implementation begins.

Usage:
    plan-invalidate.py <changed-plan-dir>               # detect and report
    plan-invalidate.py <changed-plan-dir> --apply        # detect and apply
    plan-invalidate.py <changed-plan-dir> --json         # JSON output
    plan-invalidate.py <changed-plan-dir> --apply --json # apply + JSON output
    plan-invalidate.py --check-all                       # scan all plans against all plans
    plan-invalidate.py --check-all --json                # scan all plans, JSON output

Options:
    --apply         Flip reviewed: true → false on affected sections
    --json          Machine-readable JSON output
    --min-weight N  Minimum overlap weight to report (default: 2 = 1 file ref)
    --check-all     Check ALL active plans against each other for stale reviews
    --help          Show this help

Exit codes:
    0  No stale reviews found (or all applied successfully)
    1  Stale reviews found (when not using --apply)
    2  Usage error
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

# Add parent paths for planlib import
sys.path.insert(0, str(Path(__file__).resolve().parent))

from planlib import (
    StaleReviewEntry,
    extract_plan_footprint,
    find_stale_reviews,
    invalidate_section_review,
    load_all_plans,
    load_plan,
)


def format_entry_human(entry: StaleReviewEntry) -> str:
    """Format a stale review entry for human-readable output."""
    section_path = entry.section.path.relative_to(Path.cwd())
    plan_name = entry.plan.name
    section_id = entry.section.number
    section_title = entry.section.title
    overlap = entry.overlap

    lines = [
        f"  {section_path}",
        f"    Plan: {plan_name} | Section {section_id}: {section_title}",
        f"    Status: {entry.section.status} | Overlap weight: {overlap.weight}",
    ]
    if overlap.file_refs:
        refs = sorted(overlap.file_refs)
        display = refs[:5]
        lines.append(f"    Files: {', '.join(display)}")
        if len(refs) > 5:
            lines.append(f"           (+{len(refs) - 5} more)")
    if overlap.symbols:
        syms = sorted(overlap.symbols)
        display = syms[:5]
        lines.append(f"    Symbols: {', '.join(display)}")
        if len(syms) > 5:
            lines.append(f"             (+{len(syms) - 5} more)")
    lines.append(f"    Reason: {entry.reason}")
    return "\n".join(lines)


def entry_to_dict(entry: StaleReviewEntry) -> dict:
    """Convert a stale review entry to a JSON-serializable dict."""
    return {
        "plan": entry.plan.name,
        "plan_dir": str(entry.plan.dir),
        "section_file": str(entry.section.path),
        "section_number": entry.section.number,
        "section_title": entry.section.title,
        "section_status": entry.section.status,
        "overlap_weight": entry.overlap.weight,
        "overlapping_files": sorted(entry.overlap.file_refs),
        "overlapping_symbols": sorted(entry.overlap.symbols),
        "overlapping_crates": sorted(entry.overlap.crates),
        "reason": entry.reason,
    }


def run_single_plan(
    changed_plan_dir: Path,
    *,
    apply: bool,
    use_json: bool,
    min_weight: int,
) -> int:
    """Run invalidation check for a single changed plan against all others."""
    if not changed_plan_dir.is_dir():
        print(f"Error: not a directory: {changed_plan_dir}", file=sys.stderr)
        return 2

    changed_plan = load_plan(changed_plan_dir)
    all_plans = load_all_plans()

    # Show changed plan's footprint for context
    changed_fp = extract_plan_footprint(changed_plan)
    if not use_json:
        print(f"Changed plan: {changed_plan.name}")
        print(f"  Scope: {len(changed_fp.file_refs)} files, "
              f"{len(changed_fp.symbols)} symbols, "
              f"{len(changed_fp.crates)} crates")
        if changed_fp.crates:
            print(f"  Crates: {', '.join(sorted(changed_fp.crates))}")
        print()

    stale = find_stale_reviews(changed_plan, all_plans, min_weight=min_weight)

    if not stale:
        if use_json:
            print(json.dumps({"status": "clean", "stale_sections": []}, indent=2))
        else:
            print("No cross-plan review invalidation needed.")
            print("No other plan sections' reviewed scopes overlap with this plan.")
        return 0

    if use_json:
        result = {
            "status": "applied" if apply else "stale_found",
            "changed_plan": changed_plan.name,
            "stale_sections": [entry_to_dict(e) for e in stale],
            "total_affected": len(stale),
            "plans_affected": len(set(e.plan.name for e in stale)),
        }
        if apply:
            applied = []
            for entry in stale:
                if invalidate_section_review(entry.section):
                    applied.append(str(entry.section.path))
            result["applied_to"] = applied
        print(json.dumps(result, indent=2))
    else:
        print(f"Found {len(stale)} section(s) with potentially stale reviews:")
        print()
        for entry in stale:
            print(format_entry_human(entry))
            if apply:
                if invalidate_section_review(entry.section):
                    print(f"    → APPLIED: reviewed: true → false")
                else:
                    print(f"    → SKIPPED: already reviewed: false")
            print()

        affected_plans = set(e.plan.name for e in stale)
        print(f"Summary: {len(stale)} sections across {len(affected_plans)} plan(s)")
        if not apply:
            print("\nRe-run with --apply to flip reviewed: true → false on these sections.")

    return 0 if apply else 1


def run_check_all(*, use_json: bool, min_weight: int) -> int:
    """Check all active plans against each other for stale cross-references."""
    all_plans = load_all_plans()
    all_stale: list[tuple[str, StaleReviewEntry]] = []

    for plan in all_plans:
        if plan.dir.parent.name == "completed":
            continue
        stale = find_stale_reviews(plan, all_plans, min_weight=min_weight)
        for entry in stale:
            all_stale.append((plan.name, entry))

    # Deduplicate: same section can be flagged by multiple changed plans
    seen: set[str] = set()
    unique: list[tuple[str, StaleReviewEntry]] = []
    for trigger, entry in all_stale:
        key = str(entry.section.path)
        if key not in seen:
            seen.add(key)
            unique.append((trigger, entry))

    if not unique:
        if use_json:
            print(json.dumps({"status": "clean", "stale_sections": []}, indent=2))
        else:
            print("No cross-plan review staleness detected across any plans.")
        return 0

    if use_json:
        result = {
            "status": "stale_found",
            "stale_sections": [
                {**entry_to_dict(entry), "triggered_by_plan": trigger}
                for trigger, entry in unique
            ],
            "total_affected": len(unique),
        }
        print(json.dumps(result, indent=2))
    else:
        print(f"Found {len(unique)} section(s) with potentially stale reviews:\n")
        for trigger, entry in unique:
            print(f"  Triggered by plan: {trigger}")
            print(format_entry_human(entry))
            print()

    return 1 if unique else 0


def main() -> int:
    args = sys.argv[1:]

    if not args or "--help" in args or "-h" in args:
        print(__doc__)
        return 0

    apply = "--apply" in args
    use_json = "--json" in args
    check_all = "--check-all" in args

    min_weight = 2
    if "--min-weight" in args:
        idx = args.index("--min-weight")
        if idx + 1 < len(args):
            try:
                min_weight = int(args[idx + 1])
            except ValueError:
                print("Error: --min-weight requires an integer", file=sys.stderr)
                return 2

    if check_all:
        return run_check_all(use_json=use_json, min_weight=min_weight)

    # Find the plan directory argument (first non-flag arg)
    plan_dir_str = None
    skip_next = False
    for arg in args:
        if skip_next:
            skip_next = False
            continue
        if arg == "--min-weight":
            skip_next = True
            continue
        if arg.startswith("--"):
            continue
        plan_dir_str = arg
        break

    if not plan_dir_str:
        print("Error: must provide a plan directory path or --check-all", file=sys.stderr)
        return 2

    plan_dir = Path(plan_dir_str).resolve()
    return run_single_plan(plan_dir, apply=apply, use_json=use_json, min_weight=min_weight)


if __name__ == "__main__":
    sys.exit(main())
