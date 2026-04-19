#!/usr/bin/env python3
"""
prose-lint: scan authored .md files for prose-creep violations.

Enforces the rule defined in `.claude/skills/improve-tooling/SKILL.md`
section "No Prose in Authored .md Files - ABSOLUTE" (mirrored from global
CLAUDE.md). Prescriptive authored files (skills, commands, rules, design
logs) must be bullets/tables/imperative sentences; banned patterns include
dated narrative, rationale tails, history comparisons, and paragraphs
longer than 2 sentences.

Scope (default): .claude/skills, .claude/commands, .claude/rules.
Exit codes: 0 clean (or --exit-zero), 1 violations found, 2 usage error.
"""

import argparse
import json
import re
import sys
from pathlib import Path

# ---------------------------------------------------------------------------
# Ban patterns - high-signal keywords + dated refs
# ---------------------------------------------------------------------------

KEYWORD_PATTERNS = [
    (r"\b(previously|originally|restoring|defeating)\b", "history-keyword"),
    (r"\bas of 20\d{2}(-\d{2}(-\d{2})?)?\b", "dated-ref-as-of"),
    (r"\bsince 20\d{2}(-\d{2}(-\d{2})?)?\b", "dated-ref-since"),
    (r"\u2014 causes\b", "rationale-tail-em-dash-causes"),
    (r"\bwas (originally|previously)\b", "history-phrase-was-originally"),
]

# Suppressions (false-positive guards)
COMPOUND_ADJ_RE = re.compile(
    r"\bpreviously-(failing|valid|completed|written|verified|seen|working|broken|projected|existing|resolved)\b",
    re.IGNORECASE,
)
STATE_LABEL_RE = re.compile(
    r"\*\*(CONFIRMED|REGRESSED|FIXED|PASSED|FAILED|NEW|RESOLVED)\*\*[^\n]*\bpreviously\b",
    re.IGNORECASE,
)

# Directive markers
LINT_OFF_RE = re.compile(r"<!--\s*prose-lint:\s*off\s*-->")
LINT_ON_RE = re.compile(r"<!--\s*prose-lint:\s*on\s*-->")
LINT_ALLOW_LINE_RE = re.compile(r"<!--\s*prose-lint:\s*allow\s*-->")

# Design-log section headers where prose is allowed (see Exceptions table)
EXEMPT_HEADER_RE = re.compile(r"^##\s+\u00a7[46]\b")

# Markdown constructs (list/table/blockquote)
LIST_OR_TABLE_RE = re.compile(r"^\s*([-*+]|\d+\.|>|\|)")
INDENT_CONTINUATION_RE = re.compile(r"^\s{2,}\S")

DEFAULT_ROOTS = [".claude/skills", ".claude/commands", ".claude/rules"]
DEFAULT_MAX_SENTENCES = 2


# ---------------------------------------------------------------------------
# Per-file exemptions
# ---------------------------------------------------------------------------

def is_design_log(path: Path) -> bool:
    return path.name.endswith("-design.md")


def is_exempt_path(path: Path) -> bool:
    s = str(path)
    # Rule-definition file itself (self-reference)
    if path.name == "SKILL.md" and "improve-tooling" in s:
        return True
    # CHANGELOG / HISTORY
    if path.name in ("CHANGELOG.md", "HISTORY.md"):
        return True
    return False


# ---------------------------------------------------------------------------
# Region exemption - fences, design-log sections, lint-off blocks
# ---------------------------------------------------------------------------

def compute_exempt_lines(lines, path):
    exempt = set()
    in_fence = False
    lint_off = False
    in_design_exempt = False
    design = is_design_log(path)
    for idx, line in enumerate(lines, 1):
        stripped = line.strip()
        if stripped.startswith("```"):
            in_fence = not in_fence
            exempt.add(idx)
            continue
        if in_fence:
            exempt.add(idx)
            continue
        if LINT_OFF_RE.search(line):
            lint_off = True
        if design and line.startswith("## "):
            in_design_exempt = bool(EXEMPT_HEADER_RE.match(line))
        if lint_off or in_design_exempt:
            exempt.add(idx)
        if LINT_ON_RE.search(line):
            lint_off = False
    return exempt


# ---------------------------------------------------------------------------
# Sentence counting (rough but good enough)
# ---------------------------------------------------------------------------

SENTENCE_END_RE = re.compile(r"[.!?](?:\s|$)")
ABBREVIATIONS = ["e.g.", "i.e.", "etc.", "vs.", "Dr.", "Mr.", "Mrs.", "Ms.", "St.", "Ft."]


def count_sentences(text):
    t = re.sub(r"`[^`]*`", " ", text)
    t = re.sub(r"https?://\S+", " ", t)
    for abbr in ABBREVIATIONS:
        t = t.replace(abbr, abbr.replace(".", ""))
    count = len(SENTENCE_END_RE.findall(t))
    stripped = t.rstrip()
    if stripped and stripped[-1] not in ".!?":
        count += 1
    return count


# ---------------------------------------------------------------------------
# Paragraph detection
# ---------------------------------------------------------------------------

def find_long_paragraphs(lines, exempt, max_sentences):
    findings = []
    n = len(lines)
    i = 0
    while i < n:
        lineno = i + 1
        line = lines[i]
        stripped = line.strip()
        if (
            not stripped
            or stripped.startswith("#")
            or stripped.startswith("```")
            or LIST_OR_TABLE_RE.match(line)
            or INDENT_CONTINUATION_RE.match(line)
            or lineno in exempt
        ):
            i += 1
            continue
        start = lineno
        buf = [stripped]
        j = i + 1
        while j < n:
            nxt = lines[j]
            ns = nxt.strip()
            if (
                not ns
                or ns.startswith("#")
                or ns.startswith("```")
                or LIST_OR_TABLE_RE.match(nxt)
                or (j + 1) in exempt
            ):
                break
            buf.append(ns)
            j += 1
        text = " ".join(buf)
        count = count_sentences(text)
        if count > max_sentences:
            excerpt = (text[:180] + "\u2026") if len(text) > 180 else text
            findings.append({
                "line": start,
                "end_line": j,
                "type": "paragraph-too-long",
                "sentence_count": count,
                "excerpt": excerpt,
            })
        i = j if j > i else i + 1
    return findings


# ---------------------------------------------------------------------------
# Keyword scan
# ---------------------------------------------------------------------------

def keyword_scan(lines, exempt):
    findings = []
    for idx, line in enumerate(lines, 1):
        if idx in exempt:
            continue
        if LINT_ALLOW_LINE_RE.search(line):
            continue
        if STATE_LABEL_RE.search(line):
            continue
        masked = COMPOUND_ADJ_RE.sub("<compound>", line)
        for pat, label in KEYWORD_PATTERNS:
            m = re.search(pat, masked, re.IGNORECASE)
            if m:
                findings.append({
                    "line": idx,
                    "type": "keyword",
                    "pattern": label,
                    "match": m.group(0),
                    "excerpt": line.strip()[:180],
                })
                break
    return findings


# ---------------------------------------------------------------------------
# File + path handling
# ---------------------------------------------------------------------------

def scan_file(path: Path, max_sentences: int):
    if is_exempt_path(path):
        return []
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as e:
        return [{"file": str(path), "line": 0, "type": "read-error", "message": str(e)}]
    lines = text.splitlines()
    exempt = compute_exempt_lines(lines, path)
    items = keyword_scan(lines, exempt) + find_long_paragraphs(lines, exempt, max_sentences)
    return [{"file": str(path), **f} for f in items]


def collect_paths(roots):
    out = []
    for r in roots:
        p = Path(r)
        if p.is_file() and p.suffix == ".md":
            out.append(p)
        elif p.is_dir():
            out.extend(p.rglob("*.md"))
    return sorted(set(out))


# ---------------------------------------------------------------------------
# Output
# ---------------------------------------------------------------------------

def format_human(findings, scanned):
    if not findings:
        return f"prose-lint: clean - {scanned} file(s) scanned, 0 violations.\n"
    by_file = {}
    for f in findings:
        by_file.setdefault(f["file"], []).append(f)
    out = [f"prose-lint: {len(findings)} violation(s) across {len(by_file)} file(s) (of {scanned} scanned)"]
    for file, fs in sorted(by_file.items()):
        out.append("")
        out.append(file)
        for f in sorted(fs, key=lambda x: x["line"]):
            t = f["type"]
            if t == "keyword":
                match = f.get("match", "")
                out.append(f"  :{f['line']}  [{f['pattern']}] match={match!r}")
                out.append(f"      {f['excerpt']}")
            elif t == "paragraph-too-long":
                out.append(
                    f"  :{f['line']}-{f['end_line']}  [paragraph: {f['sentence_count']} sentences > threshold]"
                )
                out.append(f"      {f['excerpt']}")
            else:
                out.append(f"  :{f['line']}  [{t}] {f.get('message','')}")
    out.append("")
    return "\n".join(out)


def main():
    parser = argparse.ArgumentParser(
        prog="prose-lint",
        description="Scan authored .md files for prose-creep violations.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""\
Scope (default): .claude/skills .claude/commands .claude/rules

Exempts:
  - Fenced code blocks.
  - Design-log sections: ## \u00a74 Lessons and ## \u00a76 Improvement Log.
  - Rule-definition self-reference file: .claude/skills/improve-tooling/SKILL.md.
  - CHANGELOG.md / HISTORY.md files.
  - Regions between <!-- prose-lint: off --> and <!-- prose-lint: on -->.
  - Individual lines with <!-- prose-lint: allow -->.
  - State-label definitions (**CONFIRMED|REGRESSED|FIXED** - previously ...).
  - Hyphenated compound adjectives (previously-failing/valid/completed/...).

Exit codes: 0 clean, 1 violations found, 2 usage error.
""",
    )
    parser.add_argument(
        "paths", nargs="*",
        help=f"Files or dirs to scan (default: {' '.join(DEFAULT_ROOTS)})",
    )
    parser.add_argument("--json", action="store_true", help="Emit JSON findings")
    parser.add_argument(
        "--max-paragraph-sentences", type=int, default=DEFAULT_MAX_SENTENCES,
        help=f"Paragraph length threshold (default: {DEFAULT_MAX_SENTENCES})",
    )
    parser.add_argument(
        "--exit-zero", action="store_true",
        help="Always exit 0 (even with findings)",
    )
    args = parser.parse_args()

    roots = args.paths or DEFAULT_ROOTS
    paths = collect_paths(roots)
    findings = []
    for p in paths:
        findings.extend(scan_file(p, args.max_paragraph_sentences))

    if args.json:
        print(json.dumps(
            {
                "scanned_files": len(paths),
                "finding_count": len(findings),
                "findings": findings,
            },
            indent=2,
        ))
    else:
        sys.stdout.write(format_human(findings, len(paths)))

    sys.exit(0 if (not findings or args.exit_zero) else 1)


if __name__ == "__main__":
    main()
