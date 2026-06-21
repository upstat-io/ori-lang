#!/usr/bin/env python3
"""Map each `.ori` test path to its OWNING plan-node or bug.

Single source of truth for `.ori path -> owning node` attribution feeding the
per-node LLVM dual-exec parity + leak verdict block in
`build/test-all-summary.json`. Scans BOTH `plans/` AND `bug-tracker/plans/`,
NOT the roadmap alone.

Ownership signals (first match wins, in this priority):
  1. a plan section/content body that references the .ori path or its
     containing directory (substring match against the body text);
  2. section frontmatter `touches:` / `owns_crates:` covering a path the .ori
     lives under (the .ori path starts with a listed path prefix);
  3. bug-plan TDD `.ori` tests referenced in a bug plan's content body.

An unattributable path maps to None (recorded downstream as the `"unknown"`
sentinel bucket — fail-closed, NEVER dropped or silently attributed to the
roadmap).

Node identity:
  - a plan node id is `s-<hex>` when a content file name encodes it
    (`<slug>--s-<hex>.md`), else the plan directory name (a plan-level node);
  - a bug node id is the bug directory name `BUG-XX-NNN`.

Dependency-light + deterministic: stdlib only. Plan content is read by plain
file scan (`content/**/*.md` + top-level `*.md` bodies + frontmatter text) —
NOT via the plan_corpus route API and NOT by reading `plan.json` (the
route-access ban). Results sort deterministically so two runs over the same
tree produce byte-identical output.
"""

import argparse
import json
import os
import re
import sys

__all__ = ["path_to_node", "build_index"]

# Repo-root-relative default scan roots. Resolved against the wrapper root two
# levels up from this file (compiler_repo/diagnostics/path_to_node_index.py).
_REPO_ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
_DEFAULT_PLANS_ROOT = os.path.join(_REPO_ROOT, "plans")
_DEFAULT_BUG_PLANS_ROOT = os.path.join(_REPO_ROOT, "bug-tracker", "plans")

# A content file name `<slug>--s-<hex>.md` encodes its owning section id in the
# `--s-<hex>` suffix; capture it so attribution keys on the section node, not
# the plan-level node, when a content body carries the reference.
_SECTION_ID_RE = re.compile(r"--(?P<sid>s-[0-9a-f]+)\.md$")
_BUG_DIR_RE = re.compile(r"^BUG-[A-Z0-9]+-[0-9]+$")

# frontmatter list-bearing keys whose entries are path prefixes a node covers.
_PREFIX_KEYS = ("touches:", "owns_crates:")


# Delimiters that terminate a path token in a markdown body — a directory
# reference owns its children only when the `<dir>/` occurrence is followed by
# one of these (i.e. the body names the directory itself, NOT a deeper file
# path that merely shares the prefix).
_PATH_TERMINATORS = ("`", ")", " ", "\t", "\n", "\r", ",", ";", "'", '"', "*")


def _dir_referenced_terminally(text, directory):
    """True iff `text` names `directory` as a terminal path token (a `<dir>/`
    occurrence followed by a delimiter, OR the bare `<dir>` at end-of-line) —
    NOT as a prefix of a deeper file path. Prevents a body referencing
    `spec/featx/main.ori` from matching the ancestor `spec/featx`."""
    needle = directory + "/"
    start = 0
    while True:
        idx = text.find(needle, start)
        if idx == -1:
            break
        after = idx + len(needle)
        nxt = text[after] if after < len(text) else "\n"
        if nxt in _PATH_TERMINATORS:
            return True
        start = after
    # Bare directory token at end-of-line (no trailing slash).
    for line in text.splitlines():
        toks = re.split(r"[\s`)\,;'\"*]+", line.strip())
        if directory in toks:
            return True
    return False


def _is_ori_ref(text, ori_path):
    """True iff `text` references `ori_path` or its containing directory.

    Matches the full path as a substring, or any ancestor directory named as a
    terminal path token (a body naming the containing directory owns every .ori
    under it). Normalizes backslashes so a Windows-authored path still matches a
    forward-slash body reference."""
    norm = ori_path.replace("\\", "/")
    if norm and norm in text:
        return True
    # Walk ancestor directories: a body referencing `tests/spec/traits/` owns
    # `tests/spec/traits/iterator/collect.ori`. Terminal-token match only.
    parent = os.path.dirname(norm)
    while parent and parent not in (".", "/"):
        if _dir_referenced_terminally(text, parent):
            return True
        parent = os.path.dirname(parent)
    return False


def _prefix_covers(prefixes, ori_path):
    """True iff any frontmatter `touches:`/`owns_crates:` prefix covers the
    .ori path (the path starts with a listed prefix, on a path boundary)."""
    norm = ori_path.replace("\\", "/").lstrip("./")
    for pre in prefixes:
        p = pre.replace("\\", "/").strip().strip("\"'").lstrip("./").rstrip("/")
        if not p:
            continue
        if norm == p or norm.startswith(p + "/"):
            return True
    return False


def _extract_prefixes(text):
    """Pull `touches:` / `owns_crates:` list entries out of frontmatter-ish
    text. Handles inline `[a, b]` and YAML block (`- item`) forms; tolerant
    plain-text scan, never a YAML parse (frontmatter may be embedded in a body
    we read as raw text)."""
    prefixes = []
    lines = text.splitlines()
    for i, line in enumerate(lines):
        stripped = line.strip()
        for key in _PREFIX_KEYS:
            if not stripped.startswith(key):
                continue
            rest = stripped[len(key):].strip()
            if rest.startswith("[") and rest.endswith("]"):
                inner = rest[1:-1]
                prefixes.extend(tok.strip().strip("\"'") for tok in inner.split(",") if tok.strip())
            elif rest and rest != "[]":
                prefixes.append(rest.strip().strip("\"'"))
            else:
                # YAML block list: consume subsequent `- item` lines.
                for follow in lines[i + 1:]:
                    fs = follow.strip()
                    if fs.startswith("- "):
                        prefixes.append(fs[2:].strip().strip("\"'"))
                    elif fs == "" or fs.startswith("#"):
                        continue
                    else:
                        break
    return prefixes


def _node_id_for_content_file(plan_dir_name, file_path):
    """Node id for a content file: the `s-<hex>` section id if the file name
    encodes one, else the plan directory name (plan-level node)."""
    m = _SECTION_ID_RE.search(os.path.basename(file_path))
    if m:
        return m.group("sid")
    return plan_dir_name


def _scan_plan_bodies(plans_root):
    """Yield (node_id, body_text, prefixes) per scanned plan markdown body
    under `plans_root`. Reads `content/**/*.md` + top-level `*.md` (skips
    plan.json per the route-access ban). Deterministic directory walk order."""
    if not os.path.isdir(plans_root):
        return
    for plan_dir in sorted(os.listdir(plans_root)):
        plan_path = os.path.join(plans_root, plan_dir)
        if not os.path.isdir(plan_path):
            continue
        # content/**/*.md
        content_root = os.path.join(plan_path, "content")
        md_files = []
        if os.path.isdir(content_root):
            for root, _dirs, files in os.walk(content_root):
                for fn in sorted(files):
                    if fn.endswith(".md"):
                        md_files.append(os.path.join(root, fn))
        # top-level *.md (00-overview.md, section-*.md, status.md, index.md)
        for fn in sorted(os.listdir(plan_path)):
            if fn.endswith(".md") and os.path.isfile(os.path.join(plan_path, fn)):
                md_files.append(os.path.join(plan_path, fn))
        for md in sorted(md_files):
            try:
                with open(md, encoding="utf-8") as fh:
                    text = fh.read()
            except OSError:
                continue
            node_id = _node_id_for_content_file(plan_dir, md)
            yield node_id, text, _extract_prefixes(text)


def _scan_bug_bodies(bug_plans_root):
    """Yield (bug_id, body_text) per scanned bug-plan markdown body under
    `bug_plans_root`. Reads `content/**/*.md` + top-level `*.md`. The node id
    is always the bug directory name `BUG-XX-NNN`."""
    if not os.path.isdir(bug_plans_root):
        return
    for bug_dir in sorted(os.listdir(bug_plans_root)):
        bug_path = os.path.join(bug_plans_root, bug_dir)
        if not os.path.isdir(bug_path):
            continue
        if not _BUG_DIR_RE.match(bug_dir):
            continue
        md_files = []
        content_root = os.path.join(bug_path, "content")
        if os.path.isdir(content_root):
            for root, _dirs, files in os.walk(content_root):
                for fn in sorted(files):
                    if fn.endswith(".md"):
                        md_files.append(os.path.join(root, fn))
        for fn in sorted(os.listdir(bug_path)):
            if fn.endswith(".md") and os.path.isfile(os.path.join(bug_path, fn)):
                md_files.append(os.path.join(bug_path, fn))
        for md in sorted(md_files):
            try:
                with open(md, encoding="utf-8") as fh:
                    text = fh.read()
            except OSError:
                continue
            yield bug_dir, text


def path_to_node(ori_path, *, plans_root=_DEFAULT_PLANS_ROOT,
                 bug_plans_root=_DEFAULT_BUG_PLANS_ROOT):
    """Return the owning node id for `ori_path`, or None when unattributable.

    Signals, first match wins (per module docstring):
      1. plan content/section body references the .ori path or its directory;
      2. plan section frontmatter `touches:` / `owns_crates:` covers it;
      3. bug-plan content body references the .ori (bug TDD test ownership).

    An empty/falsey `ori_path` is unattributable (None) — fail-closed."""
    if not ori_path:
        return None

    # Signal 1: plan content reference (highest priority across all plan nodes).
    # Signal 2: plan frontmatter prefix coverage (lower priority — collected,
    # applied only if no signal-1 match). Both scanned in one pass; signal-1
    # wins globally over signal-2 to keep "explicit reference beats prefix".
    prefix_match = None
    for node_id, text, prefixes in _scan_plan_bodies(plans_root):
        if _is_ori_ref(text, ori_path):
            return node_id
        if prefix_match is None and prefixes and _prefix_covers(prefixes, ori_path):
            prefix_match = node_id
    if prefix_match is not None:
        return prefix_match

    # Signal 3: bug-plan TDD .ori reference.
    for bug_id, text in _scan_bug_bodies(bug_plans_root):
        if _is_ori_ref(text, ori_path):
            return bug_id

    return None


def build_index(ori_paths=None, *, plans_root=_DEFAULT_PLANS_ROOT,
                bug_plans_root=_DEFAULT_BUG_PLANS_ROOT):
    """Return a dict mapping each `.ori` path to its owning node id.

    When `ori_paths` is given, attributes exactly those paths. When None,
    discovers every `.ori` under `compiler_repo/tests/` relative to the repo
    root and attributes each. Unattributable paths are OMITTED from the map
    (callers treat a missing key as None — fail-closed). Deterministic key
    order (sorted)."""
    if ori_paths is None:
        ori_paths = _discover_ori_tests()
    index = {}
    for p in sorted(set(ori_paths)):
        node = path_to_node(p, plans_root=plans_root, bug_plans_root=bug_plans_root)
        if node is not None:
            index[p] = node
    return index


def _discover_ori_tests():
    """Discover every `.ori` test path under `compiler_repo/tests/`, repo-root
    relative (matching the `source_path` shape the runner emits)."""
    tests_root = os.path.join(_REPO_ROOT, "compiler_repo", "tests")
    found = []
    if not os.path.isdir(tests_root):
        return found
    for root, _dirs, files in os.walk(tests_root):
        for fn in files:
            if fn.endswith(".ori"):
                abs_p = os.path.join(root, fn)
                found.append(os.path.relpath(abs_p, _REPO_ROOT))
    return found


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--plans-root", default=_DEFAULT_PLANS_ROOT,
                        help="root of feature/roadmap plans (default: <repo>/plans)")
    parser.add_argument("--bug-plans-root", default=_DEFAULT_BUG_PLANS_ROOT,
                        help="root of bug plans (default: <repo>/bug-tracker/plans)")
    parser.add_argument("path", nargs="*",
                        help="specific .ori paths to attribute (default: discover all)")
    args = parser.parse_args(argv)

    index = build_index(args.path or None,
                        plans_root=args.plans_root,
                        bug_plans_root=args.bug_plans_root)
    print(json.dumps(index, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
