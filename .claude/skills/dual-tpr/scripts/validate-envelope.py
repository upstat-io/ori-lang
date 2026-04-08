#!/usr/bin/env python3
"""validate-envelope.py — validate a findings envelope against the schema.

Usage:
    validate-envelope.py --envelope PATH --schema PATH

Exits 0 on success, 1 on failure with error details on stderr.
"""

import argparse
import json
import sys


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

    print(f"OK: {args.envelope}")
    sys.exit(0)


if __name__ == "__main__":
    main()
