#!/usr/bin/env bash
# validate-envelopes.sh — validate dual-tpr envelope JSON files against findings-schema.json
#
# Usage:
#   validate-envelopes.sh                        # validate all fixtures in fixtures/
#   validate-envelopes.sh path/to/file.json ...  # validate one or more specific envelope files
#   validate-envelopes.sh --help                 # show this help
#
# Negative fixtures are auto-detected by filename: any file whose basename starts with
# 'invalid-' is expected to be REJECTED by the schema (PASS = correct rejection,
# FAIL = the validator incorrectly accepted a malformed envelope).
#
# Exit codes:
#   0 — all expectations met (positive fixtures passed; negative fixtures correctly rejected)
#   1 — one or more validation mismatches
#   2 — usage error (missing schema, missing fixtures dir, no targets)
#
# Surfaced by dual-tpr-gemini/section-01.1 retrospective. The schema literal also
# lives in plans/dual-tpr-gemini/section-01-contracts-foundation.md — keep both in sync.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCHEMA="$SCRIPT_DIR/findings-schema.json"
FIXTURES_DIR="$SCRIPT_DIR/fixtures"

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    # Print the leading comment block (skip the shebang on line 1, stop at the
    # first non-comment line). Robust to comment-block edits — no hardcoded ranges.
    awk 'NR == 1 { next } /^#/ { sub(/^# ?/, ""); print; next } { exit }' "$0"
    exit 0
fi

if [[ ! -f "$SCHEMA" ]]; then
    echo "ERROR: schema file not found: $SCHEMA" >&2
    exit 2
fi

if [[ $# -eq 0 ]]; then
    if [[ ! -d "$FIXTURES_DIR" ]]; then
        echo "ERROR: fixtures directory not found: $FIXTURES_DIR" >&2
        exit 2
    fi
    # shellcheck disable=SC2207
    targets=( $(find "$FIXTURES_DIR" -maxdepth 1 -type f -name '*.json' | sort) )
else
    targets=( "$@" )
fi

if [[ ${#targets[@]} -eq 0 ]]; then
    echo "ERROR: no envelope files to validate" >&2
    exit 2
fi

python3 - "$SCHEMA" "${targets[@]}" <<'PY'
import json
import sys
from jsonschema import validate, ValidationError

schema_path = sys.argv[1]
targets = sys.argv[2:]

try:
    schema = json.load(open(schema_path))
except (OSError, json.JSONDecodeError) as exc:
    print(f"ERROR: cannot load schema {schema_path}: {exc}", file=sys.stderr)
    sys.exit(2)

mismatches = 0
for path in targets:
    basename = path.rsplit("/", 1)[-1]
    is_negative = basename.startswith("invalid-")
    try:
        envelope = json.load(open(path))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"FAIL: {path}: cannot parse JSON ({exc})")
        mismatches += 1
        continue
    try:
        validate(envelope, schema)
    except ValidationError as exc:
        if is_negative:
            # Negative fixture correctly rejected.
            print(f"PASS: {path} correctly rejected: {exc.message}")
        else:
            print(f"FAIL: {path}: {exc.message}")
            mismatches += 1
        continue
    if is_negative:
        print(f"FAIL: {path}: NEGATIVE fixture validated (schema should have rejected it)")
        mismatches += 1
    else:
        print(f"PASS: {path}")

print()
if mismatches:
    print(f"RESULT: {mismatches} mismatch(es) across {len(targets)} envelope(s)")
    sys.exit(1)
print(f"RESULT: all {len(targets)} envelope(s) validated correctly")
PY
