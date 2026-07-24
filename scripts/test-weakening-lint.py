#!/usr/bin/env python3
"""test-weakening-lint — diff-driven detector for test-weakening edits.

Surfaces commits that simultaneously modify existing test files AND production
Rust code under `compiler/` when the test-side edit looks like a regression
of pinned spec-conformance surface. The detector is mechanical, not semantic;
it is intended to catch the canonical hook-failure-driven test-weakening
shapes that the project's testing discipline forbids.

Five patterns:

  P1: NET additions of attribute markers on existing test files —
      +#fail(           — runtime panic accepted as success oracle
      +#skip(           — existing test silently disabled
      +#compile_fail(   — negative-pin attribute added to an existing test
      Per-hunk pairing: a `+marker(` line paired with a `-marker(` of the
      same kind in the same hunk is a modification (e.g., reason-text edit),
      not an addition; only unpaired `+marker(` lines fire findings.
  P2: removed turbofish / generic args — `<$N>`, `<T>`, `<int>` etc. removed
      from method calls or type ascriptions in test bodies.
  P3: removed method-chain depth — characteristic richer-chain method calls
      (`.to_fixed`, `.map`, `.collect`, `.fold`, etc.) disappear without an
      equivalent replacement.
  P4: return-type narrowing in @t signatures — `-> [T, max N]`,
      `-> Result<T, E>`, `-> Map<K, V>` replaced with `-> int`, `-> bool`,
      `-> str`.
  P5: net-negative executable line count — existing test files lose more
      non-comment, non-blank lines than they gain
      (`removed - added` >= --threshold, default 5).

Diff-pair gate:
  P1-P5 fire ONLY when production code (`compiler/**/*.rs`) is ALSO modified
  in the same diff. A standalone test refactor with no production-code change
  is never flagged.

Existing-test-only:
  Triggers ONLY on MODIFICATIONS to existing test files (git status `M`),
  never on additions (status `A`). New test files are coverage growth.

Test-file scope:
  - tests/spec/**
  - any *.ori file containing #test / #fail / #skip / #compile_fail markers.

Modes (mutually exclusive; default --staged):
  --staged                Scan staged changes (pre-commit use)
  --commit REV            Scan a single commit (e.g., HEAD, abc123)
  --range REV..REV        Scan a commit range
  --diff-file FILE        Read unified diff from a file (testing)

Flags:
  --warn-only             Always exit 0; print findings to stderr.
                          Use this for lefthook bedding-in.
  --json                  Emit findings as JSON to stdout.
  --threshold N           P5 net-line threshold (default: 5). Does NOT
                          affect P1-P4 (presence checks have no threshold).

Exit codes:
  0  clean (or --warn-only)
  1  violations found
  2  error (bad args, git failure)
"""

import argparse
import fnmatch
import json
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

# Test-file path globs. Matched via fnmatch against the post-rename path.
TEST_PATH_GLOBS = (
    "tests/spec/*",
    "tests/spec/**",
    "compiler_repo/tests/spec/*",
    "compiler_repo/tests/spec/**",
    "bug-tracker/plans/*/_fixtures-staging/*",
    "bug-tracker/plans/*/_fixtures-staging/**",
)

# Production-code path globs. P1-P5 require AT LEAST ONE matching file in
# the same diff (diff-pair gate).
PROD_PATH_GLOBS = (
    "compiler/*.rs",
    "compiler/**/*.rs",
    "compiler_repo/compiler/*.rs",
    "compiler_repo/compiler/**/*.rs",
)

# Markers identifying an `.ori` file as a test fixture even when its path
# doesn't match TEST_PATH_GLOBS. Used as a fallback test-file detector.
ORI_TEST_MARKERS = (
    "#test",
    "#fail",
    "#skip",
    "#compile_fail",
)

# P1: marker patterns identifying test-weakening attributes. Anchored to the
# start of the diff-line CONTENT (after stripping the leading +/- and any
# whitespace), so the same regex matches both `+#skip(...)` and `-#skip(...)`
# lines. Per-hunk pairing in detect_p1 distinguishes a genuine addition
# (unpaired `+marker(`) from a modification of an existing marker
# (`+marker(` paired with `-marker(` in the same hunk — e.g., editing the
# reason text inside an existing `#skip(...)` annotation).
P1_MARKER_PATTERNS = (
    (re.compile(r"#fail\s*\("), "fabricated-assertion-output", "+#fail(", "high"),
    (re.compile(r"#skip\s*\("), "blocker-deferred-via-skip", "+#skip(", "high"),
    (re.compile(r"#compile_fail\s*\("), "disabled-negative-pin", "+#compile_fail(", "critical"),
)

# P2: turbofish / generic-arg removal patterns (matched on `-` lines).
# These regexes look for the removed token pattern in the deleted line.
P2_PATTERNS = (
    re.compile(r"\.\w+<\$?[A-Za-z_][A-Za-z0-9_,\s]*>\s*\("),  # method.<...>(
    re.compile(r"\b[A-Z]\w*<\$?[A-Za-z_][A-Za-z0-9_,\s]*>"),    # Type<...>
)

# P3: richer method-chain markers; presence on a `-` line in test body
# suggests removed method depth. Uses simple substring match for speed.
P3_TOKENS = (
    ".to_fixed",
    ".map(",
    ".filter(",
    ".fold(",
    ".collect(",
    ".flat_map(",
    ".zip(",
    ".chain(",
    ".take(",
    ".enumerate(",
)

# P4: return-type narrowing — match `-> RICH_TYPE` removed AND
# `-> NARROW_TYPE` added in the same diff hunk.
RETURN_TYPE_RE = re.compile(r"->\s*([^=;{]+?)\s*(?:=|\{|;)")
# Rich types are container/parametric; narrow types are primitives.
RICH_TYPE_RE = re.compile(
    r"\[[^\]]+,\s*max\s+[^\]]+\]"          # [T, max N]
    r"|Result<[^>]+>"                       # Result<T, E>
    r"|Option<[^>]+>"                       # Option<T>
    r"|Iterator<[^>]+>"                     # Iterator<T>
    r"|\{[^}]*:[^}]+\}"                     # {K: V}
    r"|Set<[^>]+>"                          # Set<T>
)
NARROW_TYPES = frozenset((
    "int", "bool", "str", "char", "byte", "float", "void", "()",
))

HUNK_RE = re.compile(r"^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@")
FILE_RE = re.compile(r"^diff --git a/(?P<a>[^\s]+) b/(?P<b>[^\s]+)")
NEW_FILE_RE = re.compile(r"^new file mode")
DELETED_FILE_RE = re.compile(r"^deleted file mode")


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
        return run_git(["diff", f"{args.commit}^!", "--unified=3"])
    if args.range:
        return run_git(["diff", args.range, "--unified=3"])
    return run_git(["diff", "--cached", "--unified=3"])


def matches_any(path, globs):
    """Return True if path matches any glob (fnmatch-style)."""
    return any(fnmatch.fnmatch(path, g) for g in globs)


def is_test_path(path, file_content_cache):
    """Return True if path is in test scope. Falls back to scanning .ori
    file content for #test/#fail/#skip/#compile_fail markers."""
    if matches_any(path, TEST_PATH_GLOBS):
        return True
    if path.endswith(".ori"):
        # Check the cached file content (lazy-loaded) for test markers.
        content = file_content_cache.get(path)
        if content is None:
            try:
                content = Path(path).read_text()
            except (FileNotFoundError, PermissionError):
                content = ""
            file_content_cache[path] = content
        return any(m in content for m in ORI_TEST_MARKERS)
    return False


def is_prod_path(path):
    """Return True if path is production Rust code (P1-P5 gate input)."""
    return matches_any(path, PROD_PATH_GLOBS)


def parse_diff(diff_text):
    """Walk the unified diff once. Yield per-file records with the file's
    new/deleted/modified status, added/removed line counts, and lists of
    `(line_no, hunk_id, raw_line)` for added and removed lines.

    `hunk_id` increments per `@@` header within a file (1-based, reset per
    file). detect_p1 uses it for per-hunk add/remove pairing so a
    `+#skip(...)` line paired with a `-#skip(...)` in the same hunk is
    correctly recognized as a modification, not an addition.

    Skips files with new/deleted status — only modifications produce records.
    """
    records = {}
    current_file = None
    current_status = None  # "M" (default), "A" (new), "D" (deleted)
    current_hunk_old = 0
    current_hunk_new = 0
    current_hunk_id = 0

    for raw in diff_text.splitlines():
        m_file = FILE_RE.match(raw)
        if m_file:
            # Flush previous file if it was a modification
            if current_file and current_status == "M":
                pass  # already in records
            current_file = m_file.group("b")
            current_status = "M"
            current_hunk_id = 0
            records[current_file] = {
                "path": current_file,
                "status": "M",
                "added": [],     # list of (line_no, hunk_id, raw_text)
                "removed": [],   # list of (line_no_in_old, hunk_id, raw_text)
                "added_count": 0,
                "removed_count": 0,
            }
            continue

        if current_file is None:
            continue

        if NEW_FILE_RE.match(raw):
            records[current_file]["status"] = "A"
            current_status = "A"
            continue
        if DELETED_FILE_RE.match(raw):
            records[current_file]["status"] = "D"
            current_status = "D"
            continue

        m_hunk = HUNK_RE.match(raw)
        if m_hunk:
            current_hunk_old = int(m_hunk.group(1))
            current_hunk_new = int(m_hunk.group(2))
            current_hunk_id += 1
            continue

        # Skip diff metadata lines we don't care about.
        if raw.startswith(("+++", "---", "diff ", "index ", "@@")):
            continue

        if current_status != "M":
            # Skip body of new/deleted files for P1-P5; we only flag mods.
            continue

        if raw.startswith("+"):
            records[current_file]["added"].append((current_hunk_new, current_hunk_id, raw))
            records[current_file]["added_count"] += 1
            current_hunk_new += 1
        elif raw.startswith("-"):
            records[current_file]["removed"].append((current_hunk_old, current_hunk_id, raw))
            records[current_file]["removed_count"] += 1
            current_hunk_old += 1
        else:
            # Context line.
            current_hunk_old += 1
            current_hunk_new += 1

    return records


def detect_p1(record):
    """P1: marker additions on existing test files, with per-hunk pairing.

    A `+#skip(...)` line paired with a `-#skip(...)` line in the same hunk
    is a modification of an existing skip (e.g., editing the reason text),
    not an addition of a new skip — suppress the finding. Each `-marker(`
    occurrence in a hunk consumes one pairing slot for that marker; only
    `+marker(` lines that exhaust all available pairings fire findings.
    Pairing is per-(hunk_id, marker_idx); different markers in the same
    hunk do not pair (e.g., a removed `#skip` does not absorb an added
    `#fail`). Same-marker, different-hunk does not pair either.

    Returns list of finding dicts.
    """
    findings = []
    pairings = defaultdict(int)  # (hunk_id, marker_idx) -> available pairings

    for _, hunk_id, raw in record["removed"]:
        content = raw[1:].lstrip() if raw and raw[0] == "-" else raw
        for i, (marker_re, _, _, _) in enumerate(P1_MARKER_PATTERNS):
            if marker_re.match(content):
                pairings[(hunk_id, i)] += 1
                break  # Only one marker per line.

    for line_no, hunk_id, raw in record["added"]:
        content = raw[1:].lstrip() if raw and raw[0] == "+" else raw
        for i, (marker_re, subcat, marker, severity) in enumerate(P1_MARKER_PATTERNS):
            if marker_re.match(content):
                if pairings[(hunk_id, i)] > 0:
                    pairings[(hunk_id, i)] -= 1  # Modification: pairs with a `-marker(` in same hunk.
                    break
                findings.append({
                    "tag": f"INVERTED-TDD:{subcat}",
                    "pattern": "P1",
                    "marker": marker,
                    "path": record["path"],
                    "line": line_no,
                    "evidence": raw[:200],
                    "severity": severity,
                })
                break  # Only one P1 match per line.
    return findings


def detect_p2(record):
    """P2: turbofish / generic-arg removal — heuristic. Counts the
    occurrences of P2 patterns on `-` lines vs `+` lines; if `-` count
    exceeds `+` count, generic surface was removed.

    Emits ONE finding per file (not per line) — the per-line evidence is
    too noisy for a presence-style check.
    """
    minus_hits = 0
    plus_hits = 0
    sample_minus = None
    for _, _, raw in record["removed"]:
        for pat in P2_PATTERNS:
            if pat.search(raw):
                minus_hits += 1
                if sample_minus is None:
                    sample_minus = raw[:200]
                break
    for _, _, raw in record["added"]:
        for pat in P2_PATTERNS:
            if pat.search(raw):
                plus_hits += 1
                break
    if minus_hits > plus_hits and sample_minus:
        return [{
            "tag": "INVERTED-TDD:positive-test-modification",
            "pattern": "P2",
            "marker": "removed turbofish / generic args",
            "path": record["path"],
            "line": record["removed"][0][0] if record["removed"] else 1,
            "evidence": sample_minus,
            "severity": "critical",
            "minus_hits": minus_hits,
            "plus_hits": plus_hits,
        }]
    return []


def detect_p3(record):
    """P3: removed method-chain depth — ONE finding per file when richer
    method calls disappear without equivalent replacements."""
    minus_tokens = defaultdict(int)
    plus_tokens = defaultdict(int)
    sample = None
    for _, _, raw in record["removed"]:
        for tok in P3_TOKENS:
            if tok in raw:
                minus_tokens[tok] += 1
                if sample is None:
                    sample = raw[:200]
    for _, _, raw in record["added"]:
        for tok in P3_TOKENS:
            if tok in raw:
                plus_tokens[tok] += 1
    lost = sum(
        max(0, minus_tokens[tok] - plus_tokens[tok])
        for tok in P3_TOKENS
    )
    if lost > 0 and sample:
        return [{
            "tag": "INVERTED-TDD:positive-test-modification",
            "pattern": "P3",
            "marker": f"removed method-chain depth ({lost} richer call(s) net)",
            "path": record["path"],
            "line": record["removed"][0][0] if record["removed"] else 1,
            "evidence": sample,
            "severity": "high",
            "lost_calls": lost,
        }]
    return []


def detect_p4(record):
    """P4: return-type narrowing — match a removed `-> RICH` against an
    added `-> NARROW` in the file."""
    removed_rich = []
    added_narrow = []
    for line_no, _, raw in record["removed"]:
        m = RETURN_TYPE_RE.search(raw)
        if m and RICH_TYPE_RE.search(m.group(1)):
            removed_rich.append((line_no, raw[:200], m.group(1).strip()))
    for line_no, _, raw in record["added"]:
        m = RETURN_TYPE_RE.search(raw)
        if m and m.group(1).strip() in NARROW_TYPES:
            added_narrow.append((line_no, raw[:200], m.group(1).strip()))

    if removed_rich and added_narrow:
        return [{
            "tag": "INVERTED-TDD:positive-test-modification",
            "pattern": "P4",
            "marker": f"return-type narrowed ({removed_rich[0][2]} → {added_narrow[0][2]})",
            "path": record["path"],
            "line": added_narrow[0][0],
            "evidence": f"removed: {removed_rich[0][1]}\nadded: {added_narrow[0][1]}",
            "severity": "critical",
            "from_type": removed_rich[0][2],
            "to_type": added_narrow[0][2],
        }]
    return []


def detect_p5(record, threshold):
    """P5: net-negative executable line count on existing tests."""
    removed_count = sum(is_test_surface_line(raw) for _, _, raw in record["removed"])
    added_count = sum(is_test_surface_line(raw) for _, _, raw in record["added"])
    net = removed_count - added_count
    if net >= threshold:
        return [{
            "tag": "INVERTED-TDD:positive-test-modification",
            "pattern": "P5",
            "marker": f"net-negative executable line count (-{net}, threshold {threshold})",
            "path": record["path"],
            "line": 1,
            "evidence": f"removed {removed_count} executable line(s); added {added_count} executable line(s)",
            "severity": "high",
            "net_removed": net,
            "threshold": threshold,
        }]
    return []


def is_test_surface_line(raw):
    """Return whether a diff line can affect executable test coverage."""
    content = raw[1:].strip()
    return bool(content) and not content.startswith("//")


def find_violations(records, threshold):
    """Apply P1-P5 to all modified test files in records.

    Diff-pair gate: P1-P5 fire ONLY when at least one production-code
    file is also modified in the same diff. If no prod-code mod, skip.
    """
    file_content_cache = {}

    has_prod_mod = any(
        record["status"] == "M" and is_prod_path(path)
        for path, record in records.items()
    )

    if not has_prod_mod:
        return [], 0  # diff-pair gate not satisfied

    test_records = [
        record for path, record in records.items()
        if record["status"] == "M" and is_test_path(path, file_content_cache)
    ]

    findings = []
    for record in test_records:
        findings.extend(detect_p1(record))
        findings.extend(detect_p2(record))
        findings.extend(detect_p3(record))
        findings.extend(detect_p4(record))
        findings.extend(detect_p5(record, threshold))

    return findings, len(test_records)


def format_text(violations, scanned_files, has_prod_mod):
    """Human-readable findings on stderr (mirrors prose-lint / sprawl-lint UX)."""
    if not has_prod_mod:
        return (
            "test-weakening-lint: clean - no production-code modifications "
            "in diff; diff-pair gate not satisfied (P1-P5 require "
            "compiler/**/*.rs change in same diff).\n"
        )
    if not violations:
        return (
            f"test-weakening-lint: clean - {scanned_files} modified test "
            f"file(s) scanned, 0 INVERTED-TDD violations.\n"
        )

    lines = [
        f"test-weakening-lint: {len(violations)} test-weakening "
        f"violation(s) across {scanned_files} modified test file(s)\n\n",
    ]
    for v in violations:
        lines.append(
            f"  [{v['tag']}][{v['severity']}] {v['path']}:{v['line']} "
            f"({v['pattern']}: {v['marker']})\n"
        )
        for ev_line in v["evidence"].splitlines()[:3]:
            lines.append(f"    {ev_line}\n")
        lines.append(
            "    Cure: fix the production code so the original test passes; "
            "OR add a Test-Change: justification to the commit body citing "
            "a spec clause / approved proposal / concrete plan item.\n\n"
        )
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
                        help="P5 net-line threshold (default: 5)")
    args = parser.parse_args(argv)

    try:
        diff_text = resolve_diff(args)
    except subprocess.CalledProcessError as e:
        msg = e.stderr.strip() if e.stderr else str(e)
        print(f"test-weakening-lint: git failed: {msg}", file=sys.stderr)
        return 2
    except FileNotFoundError as e:
        print(f"test-weakening-lint: {e}", file=sys.stderr)
        return 2

    records = parse_diff(diff_text)
    has_prod_mod = any(
        record["status"] == "M" and is_prod_path(path)
        for path, record in records.items()
    )
    violations, scanned = find_violations(records, threshold=args.threshold)

    if args.json:
        print(json.dumps({
            "violations": violations,
            "scanned_test_files": scanned,
            "has_prod_mod": has_prod_mod,
            "threshold": args.threshold,
        }, indent=2))
    else:
        sys.stderr.write(format_text(violations, scanned, has_prod_mod))

    if args.warn_only:
        return 0
    return 1 if violations else 0


if __name__ == "__main__":
    sys.exit(main())
