#!/usr/bin/env python3
"""parse-codex.py — extract a findings envelope from codex's JSONL output.

Usage:
    parse-codex.py --jsonl PATH --schema PATH

Reads the codex JSONL stream from PATH, finds the final agent_message item,
parses its text as JSON (codex emits schema-conformant JSON when invoked with
--output-schema), validates the envelope against the schema, and prints the
envelope to stdout on success. On failure, prints a failure category and
details to stderr and exits non-zero.

Outcome codes (stderr first line on failure):
    missing_envelope    — no agent_message item found in the JSONL
    parse_fail          — agent_message text is not valid JSON
    schema_violation    — JSON parses but fails schema validation
    failed_partial      — validates but status != "complete"
"""

import argparse
import json
import sys


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--jsonl", required=True)
    ap.add_argument("--schema", required=True)
    args = ap.parse_args()

    try:
        import jsonschema
    except ImportError:
        print("missing_dependency", file=sys.stderr)
        print("install jsonschema: pip install jsonschema", file=sys.stderr)
        sys.exit(1)

    # Load schema
    with open(args.schema) as f:
        schema = json.load(f)

    # Find the final agent_message item in the JSONL stream
    messages = []
    with open(args.jsonl) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                continue  # ignore malformed lines (codex sometimes writes mid-stream noise)
            if obj.get("type") == "item.completed" and obj.get("item", {}).get("type") == "agent_message":
                messages.append(obj["item"].get("text", ""))

    if not messages:
        print("missing_envelope", file=sys.stderr)
        print("no item.completed/agent_message found in JSONL", file=sys.stderr)
        sys.exit(1)

    final_text = messages[-1]

    # Parse the final agent_message as JSON
    try:
        envelope = json.loads(final_text)
    except json.JSONDecodeError as e:
        print("parse_fail", file=sys.stderr)
        print(f"agent_message is not valid JSON: {e}", file=sys.stderr)
        sys.exit(1)

    # Validate against schema
    try:
        jsonschema.validate(envelope, schema)
    except jsonschema.ValidationError as e:
        print("schema_violation", file=sys.stderr)
        print(f"{e.message}", file=sys.stderr)
        sys.exit(1)

    # Check status field
    if envelope.get("status") != "complete":
        print("failed_partial", file=sys.stderr)
        print(f"envelope status: {envelope.get('status')}", file=sys.stderr)
        sys.exit(1)

    # Success — print envelope to stdout
    json.dump(envelope, sys.stdout, indent=2)
    sys.stdout.write("\n")
    sys.exit(0)


if __name__ == "__main__":
    main()
