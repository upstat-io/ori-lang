#!/usr/bin/env python3
"""validate-envelope.py — validate a findings envelope against the schema.

Usage:
    validate-envelope.py --envelope PATH --schema PATH

Exits 0 on success, 1 on failure with error details on stderr.
"""

import argparse
import json
import os
import sys

# Import envelope_invariants from the same directory. The script is invoked via
# `.claude/skills/dual-tpr/scripts/validate-envelope.py` from the repo root, so
# the script's directory is NOT on sys.path by default — we add it explicitly so
# the import works regardless of caller cwd.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from envelope_invariants import validate_envelope_invariants  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--envelope", required=True)
    ap.add_argument("--schema", required=True)
    args = ap.parse_args()

    try:
        import jsonschema
    except ImportError:
        print("missing_dependency: pip install jsonschema", file=sys.stderr)
        sys.exit(1)

    with open(args.schema) as f:
        schema = json.load(f)
    with open(args.envelope) as f:
        envelope = json.load(f)

    try:
        jsonschema.validate(envelope, schema)
    except jsonschema.ValidationError as e:
        print(f"schema_violation: {e.message}", file=sys.stderr)
        print(f"  at path: {' / '.join(str(p) for p in e.absolute_path)}", file=sys.stderr)
        sys.exit(1)

    # Validate code-level invariants (regex patterns, length limits, conditional
    # requirements that can't be expressed in the OpenAI Structured Outputs subset).
    # See envelope_invariants.py and BUG-08-003 for the rationale.
    invariant_error = validate_envelope_invariants(envelope)
    if invariant_error is not None:
        print(f"schema_violation: {invariant_error}", file=sys.stderr)
        sys.exit(1)

    print(f"OK: {args.envelope}")
    sys.exit(0)


if __name__ == "__main__":
    main()
