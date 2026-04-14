"""CLI entry point for the plan corpus library.

Invoke via `python -m scripts.plan_corpus`.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

from .types import Finding, REPO_ROOT
from .discovery import discover_corpus, load_and_validate
from .docgen import generate_schema_reference


def main() -> int:
    import argparse

    parser = argparse.ArgumentParser(
        description="Ori plan corpus parser and validator"
    )
    sub = parser.add_subparsers(dest="command")

    check_p = sub.add_parser("check", help="Validate a file or directory")
    check_p.add_argument("paths", nargs="+", type=Path)
    check_p.add_argument("--json", action="store_true")

    sub.add_parser("discover", help="Discover and report corpus")

    docgen_p = sub.add_parser("docgen", help="Generate schema reference")
    docgen_p.add_argument("--check", action="store_true",
                          help="Compare against committed file, exit non-zero on diff")

    args = parser.parse_args()

    if args.command == "check":
        all_findings: list[Finding] = []
        for p in args.paths:
            if p.is_dir():
                for md in sorted(p.rglob("*.md")):
                    result = load_and_validate(md)
                    if result.err:
                        all_findings.append(result.err)
                    elif result.ok:
                        all_findings.extend(result.ok.violations)
            else:
                result = load_and_validate(p)
                if result.err:
                    all_findings.append(result.err)
                elif result.ok:
                    all_findings.extend(result.ok.violations)

        if args.json:
            print(json.dumps([f.to_json() for f in all_findings], indent=2))
        else:
            for f in sorted(all_findings, key=lambda f: (-f.severity.value, str(f.source))):
                print(f.to_markdown())

        return 1 if all_findings else 0

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
            print(ref)
            return 0

    else:
        parser.print_help()
        return 0


if __name__ == "__main__":
    sys.exit(main())
