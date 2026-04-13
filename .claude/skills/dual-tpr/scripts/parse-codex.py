#!/usr/bin/env python3
"""parse-codex.py — extract a findings envelope from codex's JSONL output.

Usage:
    parse-codex.py --jsonl PATH --schema PATH

Reads the codex JSONL stream from PATH, finds the final agent_message item,
parses its text as JSON, runs the repair layer (repair_envelope.py) to
normalize common schema violations, validates the (possibly repaired)
envelope against the schema, applies the code-level invariants in
envelope_invariants.py, and prints the envelope to stdout on success.
On failure, prints a failure category and details to stderr and exits
non-zero.

Codex is invoked WITHOUT `--output-schema` (BUG-08-003 phase 2, commit
a5a2753f), so the agent_message text is free-form JSON driven entirely by
the prompt template and the model's JSON-following ability. All structural
and semantic validation lives here and in envelope_invariants.py — there is
no API-level schema enforcement. This keeps the codex and gemini paths
symmetric (both validated only at the parser layer, both repaired by the
same repair_envelope module).

Outcome codes (stderr first line on failure):
    missing_envelope    — no agent_message item found in the JSONL
    parse_fail          — agent_message text is not valid JSON
    schema_violation    — [RESCUED: no longer causes exit(1)]
                          JSON parses but fails schema validation even after
                          repair. Now rescued — envelope written to stdout
                          with RESCUED warnings on stderr.
    failed_partial      — validates but status != "complete"
"""

import argparse
import json
import os
import sys

# Import envelope_invariants from the same directory. The script is invoked via
# `.claude/skills/dual-tpr/scripts/parse-codex.py` from the repo root, so the
# script's directory is NOT on sys.path by default — we add it explicitly so the
# import works regardless of caller cwd.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from envelope_invariants import validate_envelope_invariants  # noqa: E402
from repair_envelope import repair_envelope  # noqa: E402


def _flush_advisory(deferred):
    """Flush deferred advisory lines to stderr.

    REPAIR lines are emitted AFTER the primary output (envelope on stdout, or
    category line on stderr) so they can't corrupt `head -1 parse-error`
    extraction in dual-invoke-with-retry.sh. The stderr-first-line-is-category
    contract is load-bearing: the retry classifier uses the first stderr line
    to decide whether a failure is terminal. Prior to this indirection, a
    REPAIR that was followed by a schema violation corrupted the category to
    `codex_REPAIR: applied N auto-repair(s) to codex envelope:` which matched
    no classifier entry and was retried by accident. Kept symmetric with
    parse-gemini.py's _flush_advisory.
    """
    for msg in deferred:
        print(msg, file=sys.stderr)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--jsonl", required=True)
    ap.add_argument("--schema", required=True)
    args = ap.parse_args()

    # deferred_advisory accumulates REPAIR lines that would otherwise corrupt
    # the stderr-first-line-is-category contract (see _flush_advisory).
    deferred_advisory = []

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

    # Repair layer: normalize common schema violations before validation.
    # Symmetric with parse-gemini.py — both parsers use the same repair module.
    # REPAIR lines are deferred to `deferred_advisory` so they don't violate
    # the stderr-first-line-is-category contract (see _flush_advisory).
    envelope, repairs = repair_envelope(
        envelope, default_reviewer="codex", default_skill="review-work",
    )
    if repairs:
        deferred_advisory.append(
            f"REPAIR: applied {len(repairs)} auto-repair(s) to codex envelope:"
        )
        for r in repairs:
            deferred_advisory.append(f"  REPAIR: {r}")

    # Rescue mode: when schema or invariant validation fails AFTER repair,
    # accept the envelope rather than failing. Symmetric with parse-gemini.py.
    rescued = False

    # Validate against schema (structural — OpenAI-compatible subset)
    try:
        jsonschema.validate(envelope, schema)
    except jsonschema.ValidationError as e:
        rescued = True
        deferred_advisory.append(
            f"RESCUED: schema validation failed after repair — accepting "
            f"envelope as-is. Violation: {e.message}"
        )
        if repairs:
            deferred_advisory.append(
                f"RESCUED: repair layer had applied {len(repairs)} fix(es) but "
                f"envelope still fails validation"
            )

    # Validate code-level invariants (regex patterns, length limits, conditional
    # requirements that can't be expressed in the OpenAI Structured Outputs subset).
    # See envelope_invariants.py and BUG-08-003 for the rationale.
    invariant_error = validate_envelope_invariants(envelope)
    if invariant_error is not None:
        rescued = True
        deferred_advisory.append(
            f"RESCUED: invariant validation failed — {invariant_error}. "
            f"Accepting envelope to avoid review failure."
        )

    # Check status field
    if envelope.get("status") != "complete":
        print("failed_partial", file=sys.stderr)
        print(f"envelope status: {envelope.get('status')}", file=sys.stderr)
        _flush_advisory(deferred_advisory)
        sys.exit(1)

    # Success — print envelope to stdout
    if rescued:
        deferred_advisory.insert(0,
            "RESCUED: codex envelope accepted despite schema/invariant "
            "violations — content preserved to avoid review failure"
        )
    json.dump(envelope, sys.stdout, indent=2)
    sys.stdout.write("\n")
    _flush_advisory(deferred_advisory)
    sys.exit(0)


if __name__ == "__main__":
    main()
