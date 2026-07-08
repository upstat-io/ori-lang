#!/usr/bin/env python3
"""Refactor-narration comment linter (COMMENT-11 / COMMENT-26 self-referential-narration).

Flags a doc/inline comment that states WHY a file or function was
restructured (split / extracted / factored out for a line-cap or hygiene
reason) or narrates a "previously X; consolidated as Y" history, instead of
stating what the code IS/DOES now. History belongs in the commit message;
`impl-hygiene.md` COMMENT-11 (removal/change trail) + COMMENT-26
(self-referential-narration) already ban this in prose, but no mechanical
detector runs today (the graph-native `comment-*` detectors need the
not-yet-live `Comment {repo:'ori'}` CPG extraction per `intelligence.md`
"Capability added by graph-driven-hygiene"). This script is the ast-lint
companion until that lands.

Mechanically gates ONLY the near-zero-false-positive subset (Rust/Ori files):
  - `split out of` / `factored out of` / `extracted from` / `lives apart from`
    naming a sibling file/module AND citing a size/hygiene reason
    (500-line, line cap, line-budget, file size, WASTE-12, hygiene sweep,
    workspace file-size limit).
  - `within/under the ... line cap` as the stated reason for a helper split.
  - `previously ... consolidated as a(n) ... cure` history narration.

Flagged:   //! Split out of `dup_inc.rs` per the 500-line file cap.
           //! Extracted from `type_info/mod.rs` for file size hygiene.
           /// (the two previously duplicated this walk; consolidated as a
           /// `LEAK:algorithmic-duplication` cure).
Exempt:    //! Extracted from the shared SSOT constant in `ori_registry`.
           /// Replaces `Param` (which contains `MatchPattern`, ...).
           // This file is exempt from the 500-line limit: it is a pure
           // `declare_builtins!` macro invocation ... (states a CURRENT
           // constraint, not why the file was split).

Deliberately NOT gated (reviewer-time per COMMENT-11/26/45/46 — FP-prone):
  - bare `replaces` / `no longer` / `extracted from` without a size/hygiene
    citation — usually a legitimate present-tense behavior description.
  - past-tense narration with no refactor-provenance verb (COMMENT-45/46
    cover the wider temporal/geographical-framing surface, reviewer-time).

Usage:
  scripts/comment-refactor-narration-lint.py [paths...]   # default: compiler/ library/
  scripts/comment-refactor-narration-lint.py --json       # machine-readable
  scripts/comment-refactor-narration-lint.py --warn-only  # report without exit 1 (bed-in)
  scripts/comment-refactor-narration-lint.py --exit-zero  # alias of --warn-only
  scripts/comment-refactor-narration-lint.py --self-test  # run embedded positive/negative cases

Returns exit 1 when any violation is found (unless --warn-only/--exit-zero).
"""
from __future__ import annotations

import argparse
import json
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path

COMMENT_LINE_RE = re.compile(r"^\s*(?://[!/]?|\*)\s?(.*)$")

# Verb naming a refactor-provenance action, paired with a size/hygiene-cap
# citation elsewhere on the same comment block. Either side alone is FP-prone
# (bare "replaces" is usually a present-tense behavior description); the
# pairing is the near-zero-false-positive signal.
PROVENANCE_VERB_RE = re.compile(
    r"\b(split out of|factored out of|extracted from|lives apart from)\b",
    re.IGNORECASE,
)
SIZE_HYGIENE_RE = re.compile(
    r"(500-line|line[\s-]cap|line[\s-]budget|file[\s-]size (hygiene|limit)|"
    r"workspace file-size limit|WASTE-12|hygiene sweep|within the line cap|"
    r"under the (500-line|line cap|workspace file-size))",
    re.IGNORECASE,
)
# "previously X; consolidated as a cure" — history narration citing a fix as
# the reason for the current shape (COMMENT-11 removal/change trail).
HISTORY_CONSOLIDATION_RE = re.compile(
    r"\bpreviously\b.{0,80}\bconsolidated as\b",
    re.IGNORECASE | re.DOTALL,
)
# How many contiguous comment lines form one "block" for pairing purposes.
MAX_BLOCK_LINES = 6


@dataclass
class Finding:
    file: str
    line: int
    snippet: str
    category: str

    def to_dict(self) -> dict[str, str | int]:
        return {"file": self.file, "line": self.line, "snippet": self.snippet, "category": self.category}


def _comment_blocks(content: str) -> list[tuple[int, str]]:
    """Return (1-based start line, joined text) for each contiguous comment block."""
    lines = content.splitlines()
    blocks: list[tuple[int, str]] = []
    i = 0
    while i < len(lines):
        m = COMMENT_LINE_RE.match(lines[i])
        if not m:
            i += 1
            continue
        start = i
        texts = [m.group(1)]
        i += 1
        while i < len(lines) and len(texts) < MAX_BLOCK_LINES:
            m2 = COMMENT_LINE_RE.match(lines[i])
            if not m2:
                break
            texts.append(m2.group(1))
            i += 1
        blocks.append((start + 1, " ".join(texts)))
    return blocks


def scan_text(content: str) -> list[tuple[int, str, str]]:
    """Return (1-based line, snippet, category) for each refactor-narration hit."""
    out: list[tuple[int, str, str]] = []
    for start_line, text in _comment_blocks(content):
        if PROVENANCE_VERB_RE.search(text) and SIZE_HYGIENE_RE.search(text):
            out.append((start_line, text[:160], "refactor-provenance-cited-as-reason"))
            continue
        if HISTORY_CONSOLIDATION_RE.search(text):
            out.append((start_line, text[:160], "history-consolidation-narration"))
    return out


def scan_file(path: Path) -> list[Finding]:
    content = path.read_text(encoding="utf-8", errors="replace")
    return [Finding(str(path), ln, snippet, cat) for ln, snippet, cat in scan_text(content)]


def discover(roots: list[Path]):
    for root in roots:
        if root.is_file():
            yield root
            continue
        for dirpath, dirnames, filenames in os.walk(root):
            dirnames[:] = [d for d in dirnames if d not in {"target", "node_modules", ".git"}]
            for fn in filenames:
                if fn.endswith(".rs") or fn.endswith(".ori"):
                    yield Path(dirpath) / fn


def run(roots: list[Path]) -> list[Finding]:
    findings: list[Finding] = []
    for fp in discover(roots):
        findings.extend(scan_file(fp))
    return findings


def self_test() -> int:
    must_flag = [
        "//! Split out of `dup_inc.rs` per the 500-line file cap.",
        "//! Extracted from `type_info/mod.rs` for file size hygiene.",
        "//! Factored out of `define_phase.rs` as part of the hygiene sweep "
        "so `define_phase.rs` stays under the 500-line cap.",
        "//! Lives apart from `impl_lookup.rs` to keep that module under the "
        "500-line cap.",
        "/// the two previously duplicated this walk; consolidated as an\n"
        "/// `LEAK:algorithmic-duplication` cure.",
    ]
    must_pass = [
        "/// Extracted from the shared SSOT constant in `ori_registry::burden::table`.",
        "/// Replaces `Param` (which contains `MatchPattern`, `ParsedType`, `ExprId`).",
        "// This file is exempt from the 500-line limit: it is a pure "
        "`declare_builtins!` macro invocation generating a single `match` dispatch.",
        "/// Replaces the element at `index` with `elem`.",
        "// the slice no longer references it.",
        "/// Mirrors the Phase-6.66c `record_iter_consume_uses` SSOT.",
    ]
    failures: list[str] = []
    for snippet in must_flag:
        if not scan_text(snippet):
            failures.append(f"FALSE NEGATIVE: {snippet!r} should flag but did not")
    for snippet in must_pass:
        if scan_text(snippet):
            failures.append(f"FALSE POSITIVE: {snippet!r} flagged")
    if failures:
        for f in failures:
            print(f"  {f}")
        print(f"\nself-test FAILED: {len(failures)} case(s).")
        return 1
    print(f"self-test ok: {len(must_flag)} flagged, {len(must_pass)} exempt.")
    return 0


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("paths", nargs="*", help="files or directories to scan")
    ap.add_argument("--json", action="store_true", help="emit JSON findings")
    ap.add_argument("--warn-only", action="store_true", help="report findings without exit 1")
    ap.add_argument("--exit-zero", action="store_true", help="alias of --warn-only")
    ap.add_argument("--self-test", action="store_true", help="run embedded positive/negative cases")
    args = ap.parse_args(argv)

    if args.self_test:
        return self_test()

    if args.paths:
        roots = [Path(p) for p in args.paths]
    else:
        repo = Path(__file__).resolve().parents[1]
        roots = [repo / "compiler", repo / "library"]
        roots = [r for r in roots if r.exists()]

    findings = run(roots)

    if args.json:
        json.dump([f.to_dict() for f in findings], sys.stdout, indent=2)
        sys.stdout.write("\n")
    else:
        for f in findings:
            print(f"{f.file}:{f.line}: [{f.category}] {f.snippet}")
        if findings:
            print(f"\nTotal: {len(findings)} refactor-narration violation(s).")
            print("Rule COMMENT-11/COMMENT-26: delete the provenance narration; "
                  "state what the code IS/DOES. History belongs in the commit.")
        else:
            print("Clean: no refactor-narration violations found.")

    if findings and not (args.warn_only or args.exit_zero):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
