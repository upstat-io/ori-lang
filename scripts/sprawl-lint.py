#!/usr/bin/env python3
"""sprawl-lint — PARAM_SPRAWL detector for Rust code in diffs.

Detects PARAM_SPRAWL:zero-default-proliferation per impl-hygiene.md
§Finding Categories: a single commit/PR touching N>=threshold construction
sites with the same zero-default value for the same newly-added field.

This is the diff-based detection trigger from impl-hygiene.md. Domain-fragment
and pass-through-tunnel subcategories require structural cross-file analysis
and remain reviewer-eye for now (whole-codebase mode is a v2 upgrade).

Modes (mutually exclusive; default --staged):
  --staged                Scan staged changes (pre-commit use)
  --commit REV            Scan a single commit (e.g., HEAD, abc123)
  --range REV..REV        Scan a commit range
  --diff-file FILE        Read unified diff from a file (testing)

Flags:
  --warn-only             Always exit 0; print findings to stderr.
                          Use this for lefthook bedding-in.
  --json                  Emit findings as JSON to stdout.
  --threshold N           Site-count threshold (default: 5).

Exit codes:
  0  clean (or --warn-only)
  1  violations found
  2  error (bad args, git failure)
"""

import argparse
import json
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

# Zero-default value expressions that count as "rarely populated" defaults.
# A field initialized to one of these at >=N sites in one commit is a strong
# signal the field belongs in a side-table rather than the struct.
DEFAULT_EXPRS = frozenset([
    "Vec::new()",
    "Vec::default()",
    "vec![]",
    "None",
    "false",
    "0",
    "0u8", "0u16", "0u32", "0u64", "0usize",
    "0i8", "0i16", "0i32", "0i64", "0isize",
    "Default::default()",
    "FxHashMap::default()",
    "HashMap::new()",
    "HashMap::default()",
    "FxHashSet::default()",
    "HashSet::new()",
    "HashSet::default()",
    "String::new()",
    'String::from("")',
    '""',
    '"".to_string()',
    "BTreeMap::new()",
    "BTreeSet::new()",
])

# Match an added field initializer line in a struct literal:
# "+    field_name: <expr>,"  or  "+    field_name: <expr>"
# Only matches lines that look like single-field initializers; multi-line
# expressions are intentionally skipped (acceptable miss for v1; the dominant
# PARAM_SPRAWL signal is single-line zero-default fanout).
FIELD_INIT_RE = re.compile(
    r'^\+\s+(?P<field>[a-z_][a-z0-9_]*)\s*:\s*(?P<value>[^\n]+?),?\s*$'
)

# Match diff hunk header to track current line number.
HUNK_RE = re.compile(r'^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@')

# Match diff file header.
FILE_RE = re.compile(r'^diff --git a/(?P<a>[^\s]+) b/(?P<b>[^\s]+)')


def run_git(args, cwd=None):
    """Run a git command, returning stdout text. Raises on failure."""
    return subprocess.run(
        ["git"] + args,
        capture_output=True, text=True, check=True, cwd=cwd,
    ).stdout


def resolve_diff(args):
    """Resolve the unified diff text per CLI mode."""
    if args.diff_file:
        return Path(args.diff_file).read_text()
    if args.commit:
        # `git diff REV^!` is `git diff REV^ REV` — single-commit changes.
        return run_git(["diff", f"{args.commit}^!", "--unified=0"])
    if args.range:
        return run_git(["diff", args.range, "--unified=0"])
    # Default: staged changes (pre-commit mode).
    return run_git(["diff", "--cached", "--unified=0"])


def find_field_inits(diff_text):
    """Walk a unified diff; return list of (file, field, value, line) tuples
    for each added single-line field initializer in a .rs file.
    """
    findings = []
    current_file = None
    current_line = 0

    for raw in diff_text.splitlines():
        m_file = FILE_RE.match(raw)
        if m_file:
            current_file = m_file.group("b")
            continue

        m_hunk = HUNK_RE.match(raw)
        if m_hunk:
            current_line = int(m_hunk.group(1))
            continue

        # Skip diff metadata lines.
        if raw.startswith(("+++", "---", "diff ", "index ", "@@")):
            continue

        # Only Rust files are subject to PARAM_SPRAWL detection.
        if not current_file or not current_file.endswith(".rs"):
            continue

        if raw.startswith("+"):
            m = FIELD_INIT_RE.match(raw)
            if m:
                findings.append((
                    current_file,
                    m.group("field"),
                    m.group("value").strip(),
                    current_line,
                ))
            current_line += 1
        elif raw.startswith("-"):
            # Removed line — line counter does not advance for the new file.
            pass
        else:
            # Context line (rare with --unified=0; possible with hunk headers).
            current_line += 1

    return findings


def detect_violations(field_inits, threshold):
    """Group field-inits by (field_name, default_value); flag groups whose
    value is a known zero-default AND group size >= threshold.
    """
    grouped = defaultdict(list)
    for path, field, value, line in field_inits:
        if value in DEFAULT_EXPRS:
            grouped[(field, value)].append((path, line))

    violations = []
    for (field, value), sites in sorted(grouped.items()):
        if len(sites) >= threshold:
            violations.append({
                "tag": "PARAM_SPRAWL:zero-default-proliferation",
                "field": field,
                "default_value": value,
                "site_count": len(sites),
                "threshold": threshold,
                "sites": [{"path": p, "line": l} for p, l in sites],
            })
    return violations


def format_text(violations, total_inits):
    """Produce human-readable findings on stderr (mirrors prose-lint.py UX)."""
    if not violations:
        return f"sprawl-lint: clean - {total_inits} field-init addition(s) scanned, 0 violations.\n"

    lines = [f"sprawl-lint: {len(violations)} PARAM_SPRAWL violation(s)\n"]
    for v in violations:
        lines.append(
            f"  [{v['tag']}] field=`{v['field']}` default=`{v['default_value']}` "
            f"sites={v['site_count']} (threshold {v['threshold']})\n"
        )
        lines.append(
            "    Cure hierarchy (impl-hygiene.md §Finding Categories):\n"
            "      1. Query the canonical owner (registry / pool / Salsa)\n"
            "      2. Lift the consumer toward the source\n"
            "      3. Introduce a domain newtype at the source\n"
            "      4. Side-table by existing key\n"
        )
        lines.append("    Sites:\n")
        for site in v['sites'][:10]:
            lines.append(f"      {site['path']}:{site['line']}\n")
        if len(v['sites']) > 10:
            lines.append(f"      ... and {len(v['sites']) - 10} more\n")
    return "".join(lines)


def main(argv=None):
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--staged", action="store_true",
                      help="Scan staged changes (default)")
    mode.add_argument("--commit", metavar="REV",
                      help="Scan a single commit")
    mode.add_argument("--range", metavar="REV..REV",
                      help="Scan a commit range")
    mode.add_argument("--diff-file", metavar="FILE",
                      help="Read unified diff from a file")
    parser.add_argument("--warn-only", action="store_true",
                        help="Always exit 0 (lefthook bedding-in mode)")
    parser.add_argument("--json", action="store_true",
                        help="Emit findings as JSON to stdout")
    parser.add_argument("--threshold", type=int, default=5, metavar="N",
                        help="Site-count threshold (default: 5)")
    args = parser.parse_args(argv)

    try:
        diff_text = resolve_diff(args)
    except subprocess.CalledProcessError as e:
        msg = e.stderr.strip() if e.stderr else str(e)
        print(f"sprawl-lint: git failed: {msg}", file=sys.stderr)
        return 2
    except FileNotFoundError as e:
        print(f"sprawl-lint: {e}", file=sys.stderr)
        return 2

    field_inits = find_field_inits(diff_text)
    violations = detect_violations(field_inits, threshold=args.threshold)

    if args.json:
        print(json.dumps({
            "violations": violations,
            "scanned_inits": len(field_inits),
            "threshold": args.threshold,
        }, indent=2))
    else:
        sys.stderr.write(format_text(violations, len(field_inits)))

    if args.warn_only:
        return 0
    return 1 if violations else 0


if __name__ == "__main__":
    sys.exit(main())
