#!/usr/bin/env python3
"""validate-embedded-templates.py — schema-validate embedded envelope templates.

Extracts fenced ```json``` blocks from markdown files that appear inside a
section whose heading matches a given pattern (default: "Minimal envelope
template"), and validates each block against
`.claude/skills/dual-tpr/findings-schema.json` plus the code-level invariants
in `envelope_invariants.py`. Used by `lint-dual-tpr-docs.sh` as a semantic
validator on top of the existing phrase-presence checks.

Surfaced by §05.2 Scenario 2 round 3 — codex finding TPR-05-006 flagged that
the lint only grepped for marker phrases (`Minimal envelope template`,
`files_read`, `rules_consulted`) but never actually parsed the embedded JSON
and validated it against the schema. Round 2's fix introduced new drift
(invalid `layer` enum values) that the phrase-presence lint could not catch.
This script closes that GAP by parsing the fenced blocks directly.

Scoping rule: only blocks INSIDE sections whose heading matches the scope
pattern are validated. This excludes illustrative blocks (citation examples,
placeholder snippets like `{ ...complete envelope per schema... }`) from
validation, because those are not intended to be schema-conformant full
envelopes. A "section" starts at a `###`/`##`/`#` heading matching the pattern
and ends at the next heading of the same-or-higher level.

Usage:
    validate-embedded-templates.py FILE [FILE ...]
    validate-embedded-templates.py --scope 'Envelope Examples' FILE

Arguments:
    FILE          Markdown file(s) to validate.
    --scope       Regex pattern matched against heading text (case-insensitive).
                  Default: "Minimal envelope template".
    --schema      Path to findings-schema.json (default: repo-relative).

Exit codes:
    0 — all in-scope templates parse and validate
    1 — one or more in-scope templates failed validation
    2 — usage error or missing dependency

Output: human-readable PASS/FAIL lines per template plus a summary tail.
"""

import argparse
import json
import os
import re
import sys

# Import envelope_invariants from the same directory.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from envelope_invariants import validate_envelope_invariants  # noqa: E402

FENCE_RE = re.compile(r"```json\s*\n(.*?)\n```", re.DOTALL)
# Heading line: one or more # followed by text. Indented headings are not
# matched (markdown treats leading 4+ spaces as code blocks, not headings).
HEADING_RE = re.compile(r"^(#+)\s+(.*?)\s*$", re.MULTILINE)


def load_schema(schema_path: str) -> dict:
    with open(schema_path) as f:
        return json.load(f)


def in_scope_sections(text: str, scope_pattern: re.Pattern) -> list[tuple[int, int]]:
    """Return a list of (start_offset, end_offset) byte ranges covering the
    in-scope sections — each from a matching heading to the next same-or-higher-
    level heading (or EOF).
    """
    headings = [(m.start(), m.end(), len(m.group(1)), m.group(2)) for m in HEADING_RE.finditer(text)]
    scopes = []
    for i, (start, end, level, title) in enumerate(headings):
        if not scope_pattern.search(title):
            continue
        # Find the next heading of the same or higher level (lower or equal level number)
        scope_end = len(text)
        for j in range(i + 1, len(headings)):
            next_start, _, next_level, _ = headings[j]
            if next_level <= level:
                scope_end = next_start
                break
        scopes.append((end, scope_end))
    return scopes


def extract_json_blocks_in_scope(
    text: str, scopes: list[tuple[int, int]]
) -> list[tuple[int, str]]:
    """Return (line_number, json_text) pairs for fenced blocks inside any scope range."""
    blocks = []
    for match in FENCE_RE.finditer(text):
        match_start = match.start()
        if not any(s <= match_start < e for s, e in scopes):
            continue
        line_number = text[:match_start].count("\n") + 1
        blocks.append((line_number, match.group(1)))
    return blocks


def validate_block(
    json_text: str,
    schema: dict,
    jsonschema_module,
) -> tuple[bool, str]:
    """Return (ok, error_message). ok=True if the block validates."""
    try:
        envelope = json.loads(json_text)
    except json.JSONDecodeError as e:
        return (False, f"not valid JSON: {e}")

    try:
        jsonschema_module.validate(envelope, schema)
    except jsonschema_module.ValidationError as e:
        return (False, f"schema violation: {e.message}")

    invariant_error = validate_envelope_invariants(envelope)
    if invariant_error is not None:
        return (False, f"invariant violation: {invariant_error}")

    return (True, "")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("files", nargs="+")
    ap.add_argument(
        "--schema",
        default=".claude/skills/dual-tpr/findings-schema.json",
        help="Path to findings-schema.json",
    )
    ap.add_argument(
        "--scope",
        default="Minimal envelope template",
        help="Heading pattern (regex) identifying in-scope sections",
    )
    args = ap.parse_args()

    try:
        import jsonschema
    except ImportError:
        print("missing_dependency: jsonschema", file=sys.stderr)
        print("install with: pip install jsonschema", file=sys.stderr)
        return 2

    schema = load_schema(args.schema)
    scope_pattern = re.compile(args.scope, re.IGNORECASE)

    total_blocks = 0
    total_fail = 0
    for path in args.files:
        with open(path) as f:
            text = f.read()
        scopes = in_scope_sections(text, scope_pattern)
        if not scopes:
            print(f"  SKIP: {path} (no heading matches scope '{args.scope}')")
            continue
        blocks = extract_json_blocks_in_scope(text, scopes)
        if not blocks:
            print(f"  WARN: {path} (scope matched but no fenced json blocks inside)")
            continue
        for line_num, json_text in blocks:
            total_blocks += 1
            ok, err = validate_block(json_text, schema, jsonschema)
            rel = path
            if ok:
                print(f"  PASS: {rel}:{line_num} (template valid)")
            else:
                print(f"  FAIL: {rel}:{line_num} — {err}")
                total_fail += 1

    print()
    print(f"  scope pattern:   {args.scope!r}")
    print(f"  total templates: {total_blocks}")
    print(f"  total failures:  {total_fail}")
    return 0 if total_fail == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
