#!/usr/bin/env python3
"""parse-gemini.py — extract a findings envelope from gemini's stream-json output.

Usage:
    parse-gemini.py --jsonl PATH --schema PATH

Reads the gemini stream-json stream from PATH:
  1. Concatenates all delta:true assistant message fragments in arrival order
  2. Verifies the terminal {"type":"result","status":"success"} event is present
  3. Searches the concatenated text for the BEGIN sentinel
  4. Extracts the fenced JSON block between BEGIN and END sentinels
  5. Parses the JSON block, validates against the schema
  6. Prints the envelope to stdout on success

Outcome codes (stderr first line on failure):
    missing_envelope        — no assistant messages found
    missing_terminator      — content present but no result/success event
    missing_begin_sentinel  — content present but no BEGIN sentinel
    missing_end_sentinel    — BEGIN found but END missing (truncation)
    missing_json_block      — sentinels present but no fenced JSON block
    parse_fail              — fenced JSON block is not valid JSON
    schema_violation        — JSON validates against neither shape nor schema
    failed_partial          — envelope validates but status != "complete"
"""

import argparse
import json
import os
import re
import sys

# Import envelope_invariants from the same directory. The script is invoked via
# `.claude/skills/dual-tpr/scripts/parse-gemini.py` from the repo root, so the
# script's directory is NOT on sys.path by default — we add it explicitly so the
# import works regardless of caller cwd.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from envelope_invariants import validate_envelope_invariants  # noqa: E402

BEGIN_SENTINEL = "<!-- BEGIN-ORI-DUAL-TPR-V1 -->"
END_SENTINEL = "<!-- END-ORI-DUAL-TPR-V1 -->"
# Fenced JSON block: ```json ... ```
FENCE_RE = re.compile(r"```json\s*\n(.*?)\n```", re.DOTALL)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--jsonl", required=True)
    ap.add_argument("--schema", required=True)
    args = ap.parse_args()

    try:
        import jsonschema
    except ImportError:
        print("missing_dependency", file=sys.stderr)
        sys.exit(1)

    with open(args.schema) as f:
        schema = json.load(f)

    # Read the gemini stream-json events
    assistant_chunks = []
    saw_terminator = False
    with open(args.jsonl) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                continue
            etype = obj.get("type")
            if etype == "message" and obj.get("role") == "assistant":
                # Collect content fragment (delta:true chunks must be concatenated in order)
                chunk = obj.get("content", "")
                assistant_chunks.append(chunk)
            elif etype == "result" and obj.get("status") == "success":
                saw_terminator = True

    if not assistant_chunks:
        print("missing_envelope", file=sys.stderr)
        print("no assistant message events in stream", file=sys.stderr)
        sys.exit(1)

    if not saw_terminator:
        print("missing_terminator", file=sys.stderr)
        print("assistant content present but no result/success event", file=sys.stderr)
        sys.exit(1)

    # Concatenate all assistant fragments in arrival order
    full_text = "".join(assistant_chunks)

    # Search for the BEGIN sentinel
    begin_idx = full_text.find(BEGIN_SENTINEL)
    if begin_idx < 0:
        print("missing_begin_sentinel", file=sys.stderr)
        print(f"BEGIN sentinel not found in assistant text", file=sys.stderr)
        sys.exit(1)

    # Search for the END sentinel after BEGIN
    end_idx = full_text.find(END_SENTINEL, begin_idx + len(BEGIN_SENTINEL))
    if end_idx < 0:
        print("missing_end_sentinel", file=sys.stderr)
        print("BEGIN found but END missing (response may be truncated)", file=sys.stderr)
        sys.exit(1)

    # Extract the text between sentinels
    between = full_text[begin_idx + len(BEGIN_SENTINEL):end_idx]

    # Find the fenced JSON block
    m = FENCE_RE.search(between)
    if not m:
        print("missing_json_block", file=sys.stderr)
        print("sentinels present but no ```json...``` block between them", file=sys.stderr)
        sys.exit(1)

    json_text = m.group(1)

    try:
        envelope = json.loads(json_text)
    except json.JSONDecodeError as e:
        print("parse_fail", file=sys.stderr)
        print(f"fenced JSON block is not valid JSON: {e}", file=sys.stderr)
        sys.exit(1)

    try:
        jsonschema.validate(envelope, schema)
    except jsonschema.ValidationError as e:
        print("schema_violation", file=sys.stderr)
        print(f"{e.message}", file=sys.stderr)
        sys.exit(1)

    # Validate code-level invariants (regex patterns, length limits, conditional
    # requirements that can't be expressed in the OpenAI Structured Outputs subset).
    # See envelope_invariants.py and BUG-08-003 for the rationale.
    invariant_error = validate_envelope_invariants(envelope)
    if invariant_error is not None:
        print("schema_violation", file=sys.stderr)
        print(invariant_error, file=sys.stderr)
        sys.exit(1)

    if envelope.get("status") != "complete":
        print("failed_partial", file=sys.stderr)
        print(f"envelope status: {envelope.get('status')}", file=sys.stderr)
        sys.exit(1)

    json.dump(envelope, sys.stdout, indent=2)
    sys.stdout.write("\n")
    sys.exit(0)


if __name__ == "__main__":
    main()
