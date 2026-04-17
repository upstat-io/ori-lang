"""CLI entry point for the plan corpus library.

Invoke via `python -m scripts.plan_corpus`.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

from collections import defaultdict

from .types import Finding, FindingSubtype, Outcome, REPO_ROOT
from .discovery import discover_corpus, load_and_validate
from .docgen import generate_schema_reference


def main() -> int:
    import argparse

    parser = argparse.ArgumentParser(
        description="Ori plan corpus parser and validator"
    )
    sub = parser.add_subparsers(dest="command")

    check_p = sub.add_parser(
        "check",
        help=(
            "Validate a file or directory. Exits 1 only on findings with "
            "Outcome.ERROR; WARNING findings are printed but non-gating."
        ),
    )
    check_p.add_argument("paths", nargs="+", type=Path)
    check_p.add_argument("--json", action="store_true")
    check_p.add_argument(
        "--strict-recon",
        action="store_true",
        help=(
            "Escalate `status: not-started` PLAN_SECTION missing/stub "
            "Intelligence Reconnaissance findings from Outcome.WARNING to "
            "Outcome.ERROR (gates CI). `status: in-progress` findings are "
            "NOT escalated; graph-unavailable documentation is NOT "
            "escalated. See §06 in plans/query-intel-adoption for the "
            "full enforcement contract."
        ),
    )

    sub.add_parser("discover", help="Discover and report corpus")

    docgen_p = sub.add_parser("docgen", help="Generate schema reference")
    docgen_p.add_argument("--check", action="store_true",
                          help="Compare against committed file, exit non-zero on diff")

    export_p = sub.add_parser(
        "export",
        help=(
            "Export corpus + DAG as a deterministic Neo4j-flavored JSON "
            "envelope (to stdout or --output file)."
        ),
    )
    export_p.add_argument(
        "--output", type=Path, default=None,
        help="Write to file instead of stdout",
    )
    export_p.add_argument(
        "--no-references", action="store_true",
        help=(
            "Omit reference-only relationships (PROSE_VERB, YAML_COMMENT, "
            "HTML_COMMENT_CONVENTION, EXPLICIT_REFERENCES). Edge-backed "
            "relationships and structural edges still emit."
        ),
    )

    args = parser.parse_args()

    if args.command == "check":
        all_findings: list[Finding] = []
        for p in args.paths:
            if p.is_dir():
                for md in sorted(p.rglob("*.md")):
                    result = load_and_validate(md, strict_recon=args.strict_recon)
                    if result.err:
                        all_findings.append(result.err)
                    elif result.ok:
                        all_findings.extend(result.ok.violations)
            else:
                result = load_and_validate(p, strict_recon=args.strict_recon)
                if result.err:
                    all_findings.append(result.err)
                elif result.ok:
                    all_findings.extend(result.ok.violations)

        if args.json:
            print(json.dumps([f.to_json() for f in all_findings], indent=2))
        else:
            for f in sorted(all_findings, key=lambda f: (-f.severity.value, str(f.source))):
                print(f.to_markdown())

        # Exit-code policy (§06.2 Design Decision 4): gate on Outcome.ERROR
        # only. WARNING findings are printed for visibility but do not fail
        # the check. This keeps the warnings-as-signal channel useful
        # without breaking CI on every `not-started` section that hasn't
        # yet done its recon.
        errors = [f for f in all_findings if f.outcome == Outcome.ERROR]
        return 1 if errors else 0

    elif args.command == "discover":
        corpus = discover_corpus()
        print(f"Plan indexes: {len(corpus.indexes)}")
        print(f"Completed indexes: {len(corpus.completed_indexes)}")
        print(f"Plan sections: {len(corpus.plan_sections)}")
        print(f"Roadmap sections: {len(corpus.roadmap_sections)}")
        print(f"Overviews: {len(corpus.overviews)}")
        print(f"Bug sections: {len(corpus.bug_sections)}")
        print(f"Fix-BUG files: {len(corpus.fix_bug_files)}")
        print(f"Name index: {len(corpus.name_index)} plans")
        print(f"Gaps: {len(corpus.gaps)}")
        for gap in corpus.gaps:
            print(f"  {gap.to_markdown()}")
        _print_recon_coverage(corpus)
        return 0

    elif args.command == "export":
        from .dag import build_dag
        from .export_json import export_neo4j_json
        corpus = discover_corpus()
        dag = build_dag(corpus)
        envelope = export_neo4j_json(
            corpus, dag, include_references=not args.no_references
        )
        rendered = json.dumps(envelope, indent=2, sort_keys=True)
        if args.output:
            args.output.write_text(rendered, encoding="utf-8")
            print(
                f"Wrote {len(envelope['nodes'])} nodes, "
                f"{len(envelope['relationships'])} relationships to {args.output}"
            )
        else:
            print(rendered)
        return 0

    elif args.command == "docgen":
        ref = generate_schema_reference()
        target = REPO_ROOT / "docs" / "internal" / "plan-schema-reference.md"
        if args.check:
            if target.exists():
                committed = target.read_text().replace("\r\n", "\n")
                if committed == ref:
                    print("Schema reference is up to date.")
                    return 0
                else:
                    print("Schema reference is OUT OF DATE. Regenerate with:")
                    print(f"  python -m scripts.plan_corpus docgen > {target}")
                    return 1
            else:
                print(f"Schema reference not found at {target}. Generate with:")
                print(f"  python -m scripts.plan_corpus docgen > {target}")
                return 1
        else:
            # `end=""` so shell redirect (`docgen > file.md`) produces bytes
            # identical to `generate_schema_reference()` output — otherwise
            # `print(ref)` appends a second `\n` and the next `docgen --check`
            # run flags the file as drifted against its own generator.
            print(ref, end="")
            return 0

    else:
        parser.print_help()
        return 0


def _print_recon_coverage(corpus) -> None:
    """Print a per-plan Intelligence Reconnaissance coverage table.

    Reads each PLAN_SECTION's `ValidatedFile.violations` (already populated
    by `body_validator` during `load_and_validate`) to classify coverage:

    * Missing   — any `MISSING_RECON_BLOCK` violation present
    * Present   — no missing violation (stub / graph-unavailable / complete
                  all count as PRESENT; quality issues surface in `check`)

    Grouped by plan directory and section `status`. §09 consumes this
    table to measure retrofit completeness against `not-started` sections.
    """
    # plan_dir -> status -> (present, total)
    buckets: dict[Path, dict[str, list[int]]] = defaultdict(
        lambda: {
            "not-started": [0, 0],
            "in-progress": [0, 0],
            "complete":    [0, 0],
        }
    )
    for path in sorted(corpus.plan_sections.keys()):
        result = load_and_validate(path)
        if not result.is_ok or result.ok is None:
            continue
        vf = result.ok
        status = str(vf.data.get("status", "")).strip()
        if status not in ("not-started", "in-progress", "complete"):
            # Unknown statuses are excluded — schema validation surfaces
            # these separately via ENUM_OUT_OF_RANGE.
            continue
        plan_dir = path.parent
        buckets[plan_dir][status][1] += 1
        if status == "complete":
            # `complete` sections are exempt — present-count is pinned
            # equal to total so the table reads as "N/N exempt".
            buckets[plan_dir][status][0] += 1
            continue
        # Present = no MISSING_RECON_BLOCK violation. Stub / graph-unavailable
        # still count as present (the block exists; quality is a `check`
        # concern).
        is_missing = any(
            f.subtype == FindingSubtype.MISSING_RECON_BLOCK
            for f in vf.violations
        )
        if not is_missing:
            buckets[plan_dir][status][0] += 1

    if not buckets:
        print("\nPer-plan recon coverage: (no plan sections found)")
        return

    print("\nPer-plan recon coverage:")
    # Format plan_dir as a relative path under `plans/` so output is stable
    # across checkout locations.
    from .types import PLANS_DIR
    width = max(
        len(_rel_plan_dir(d, PLANS_DIR)) for d in buckets.keys()
    )
    for plan_dir in sorted(buckets.keys()):
        counts = buckets[plan_dir]
        ns = counts["not-started"]
        ip = counts["in-progress"]
        co = counts["complete"]
        label = _rel_plan_dir(plan_dir, PLANS_DIR).ljust(width)
        ns_cell = f"not-started: {ns[0]}/{ns[1]} PRESENCE"
        ip_cell = f"in-progress: {ip[0]}/{ip[1]} PRESENCE"
        co_cell = f"complete: {co[0]}/{co[1]} exempt"
        print(f"  {label} — {ns_cell}   {ip_cell}   {co_cell}")


def _rel_plan_dir(plan_dir: Path, plans_dir: Path) -> str:
    """Render plan_dir as `plans/<plan-name>/` (stable across checkouts)."""
    try:
        rel = plan_dir.resolve().relative_to(plans_dir.parent.resolve())
        return str(rel) + "/"
    except ValueError:
        return str(plan_dir) + "/"


if __name__ == "__main__":
    sys.exit(main())
