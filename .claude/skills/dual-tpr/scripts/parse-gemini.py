#!/usr/bin/env python3
"""parse-gemini.py — extract a findings envelope from gemini's stream-json output.

Usage:
    parse-gemini.py --jsonl PATH --schema PATH
    parse-gemini.py --jsonl PATH --recover-text

Default mode (`--schema`) reads the gemini stream-json stream from PATH:
  1. Concatenates all delta:true assistant message fragments in arrival order
  2. Verifies the terminal {"type":"result","status":"success"} event is present
  3. Searches the concatenated text for the BEGIN sentinel
  4. Extracts the fenced JSON block between BEGIN and END sentinels
  5. Parses the JSON block
  6. Runs the repair layer (repair_envelope.py) to normalize common schema
     violations — missing fields, enum aliases, location format, etc.
  7. Validates the (possibly repaired) envelope against the schema
  8. Prints the envelope to stdout on success

Recovery mode (`--recover-text`) dumps the raw concatenated assistant text
to stdout without any sentinel or schema validation. Use this when default
mode fails with `missing_begin_sentinel` / `missing_end_sentinel` and the
operator needs to see what the reviewer actually wrote. Stderr carries a
clearly-marked warning that the output is UNVALIDATED; callers must NOT
pass recovered text through findings merge tooling.

Outcome codes (stderr first line on default-mode failure):
    missing_envelope        — no assistant messages found
    missing_terminator      — content present but no result/success event
    missing_begin_sentinel  — content present, no BEGIN sentinel, AND no valid
                              fenced JSON envelope found as fallback
    missing_end_sentinel    — BEGIN found but END missing (truncation)
    missing_json_block      — sentinels present but no fenced JSON block
    parse_fail              — fenced JSON block is not valid JSON
    schema_violation        — JSON validates against neither shape nor schema
                              (even after repair attempt)
    failed_partial          — envelope validates but status != "complete"

Sentinel-less fallback:
    When gemini omits the BEGIN/END sentinels but produces a fenced JSON block
    (```json ... ```) that looks like a findings envelope (contains
    "schema_version" and "status" keys), the parser falls back to extracting
    that block directly. A WARNING is printed to stderr. This resilience
    measure exists because gemini inconsistently follows sentinel instructions
    despite them being clearly specified in the reviewer skill file. The
    sentinel requirement remains in the skill file; this is a parser-level
    safety net, not an endorsement of sentinel-less output.

Envelope repair:
    After JSON parsing succeeds, the repair layer (repair_envelope.py) runs
    BEFORE schema validation. It normalizes common LLM output violations:
    missing required fields (schema_version, no_findings, etc.), enum aliases
    ("info" → "informational"), location format (strip "./", add ":line"),
    and type coercions (string ordinals → int). All repairs are logged to
    stderr with a "REPAIR:" prefix so they're visible in postmortem. The
    repair layer is idempotent — valid envelopes pass through unchanged.

Recovery-mode failure (rare — only if the JSONL stream itself is unreadable):
    missing_envelope        — no assistant messages found in stream
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
from repair_envelope import repair_envelope  # noqa: E402

BEGIN_SENTINEL = "<!-- BEGIN-ORI-DUAL-TPR-V1 -->"
END_SENTINEL = "<!-- END-ORI-DUAL-TPR-V1 -->"
# Fenced JSON block: ```json ... ```
FENCE_RE = re.compile(r"```json\s*\n(.*?)\n```", re.DOTALL)


def read_assistant_text(jsonl_path):
    """Read all assistant message fragments from a gemini JSONL stream.

    Returns (full_text: str, saw_terminator: bool).
    """
    assistant_chunks = []
    saw_terminator = False
    with open(jsonl_path) as f:
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
                chunk = obj.get("content", "")
                assistant_chunks.append(chunk)
            elif etype == "result" and obj.get("status") == "success":
                saw_terminator = True
    return "".join(assistant_chunks), saw_terminator


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--jsonl", required=True)
    ap.add_argument(
        "--schema",
        help="Schema file for envelope validation (default mode)",
    )
    ap.add_argument(
        "--recover-text",
        action="store_true",
        help="Dump raw assistant text to stdout without sentinel/schema "
        "validation. Use when default mode fails with missing_begin_sentinel.",
    )
    args = ap.parse_args()

    if not args.schema and not args.recover_text:
        ap.error("one of --schema or --recover-text is required")

    # Recovery mode: just dump the text and exit
    if args.recover_text:
        full_text, _ = read_assistant_text(args.jsonl)
        if not full_text:
            print("missing_envelope", file=sys.stderr)
            print("no assistant message events in stream", file=sys.stderr)
            sys.exit(1)
        print(
            "WARNING: unvalidated recovery text — no sentinel or schema "
            "checks applied. Do NOT pass this through merge tooling.",
            file=sys.stderr,
        )
        sys.stdout.write(full_text)
        sys.exit(0)

    # Default mode: full sentinel + schema validation
    try:
        import jsonschema
    except ImportError:
        print("missing_dependency", file=sys.stderr)
        sys.exit(1)

    with open(args.schema) as f:
        schema = json.load(f)

    full_text, saw_terminator = read_assistant_text(args.jsonl)

    if not full_text:
        print("missing_envelope", file=sys.stderr)
        print("no assistant message events in stream", file=sys.stderr)
        sys.exit(1)

    if not saw_terminator:
        print("missing_terminator", file=sys.stderr)
        print("assistant content present but no result/success event", file=sys.stderr)
        sys.exit(1)

    # Search for the BEGIN sentinel
    begin_idx = full_text.find(BEGIN_SENTINEL)
    if begin_idx >= 0:
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
    else:
        # Sentinel-less fallback: gemini sometimes omits sentinels but still
        # produces a valid fenced JSON envelope. Try to extract a fenced JSON
        # block that looks like a findings envelope (has "schema_version" key).
        # This is a resilience measure — the sentinel requirement remains in
        # the gemini skill file and the transport still prefers sentinel-wrapped
        # envelopes. The fallback only fires when sentinels are truly absent.
        m = FENCE_RE.search(full_text)
        if not m:
            print("missing_begin_sentinel", file=sys.stderr)
            print("BEGIN sentinel not found in assistant text", file=sys.stderr)
            sys.exit(1)

        candidate = m.group(1)
        # Quick shape check: must look like a findings envelope
        if '"schema_version"' not in candidate or '"status"' not in candidate:
            print("missing_begin_sentinel", file=sys.stderr)
            print(
                "BEGIN sentinel not found; fenced JSON block present but does "
                "not appear to be a findings envelope",
                file=sys.stderr,
            )
            sys.exit(1)

        json_text = candidate
        print(
            "WARNING: sentinel-less fallback — gemini omitted BEGIN/END "
            "sentinels but produced a valid-looking fenced JSON envelope. "
            "Proceeding with schema validation.",
            file=sys.stderr,
        )

    try:
        envelope = json.loads(json_text)
    except json.JSONDecodeError as e:
        print("parse_fail", file=sys.stderr)
        print(f"fenced JSON block is not valid JSON: {e}", file=sys.stderr)
        sys.exit(1)

    # Repair layer: normalize common schema violations before validation.
    # Gemini frequently omits required fields (schema_version, no_findings),
    # uses enum aliases ("info" instead of "informational"), or produces
    # location formats the invariant regex rejects. The repair layer fixes
    # these in-place so a structurally-correct-but-sloppy envelope doesn't
    # kill a 20-minute review. All repairs are logged to stderr.
    envelope, repairs = repair_envelope(
        envelope, default_reviewer="gemini", default_skill="review-work",
    )
    if repairs:
        print(
            f"REPAIR: applied {len(repairs)} auto-repair(s) to gemini envelope:",
            file=sys.stderr,
        )
        for r in repairs:
            print(f"  REPAIR: {r}", file=sys.stderr)

    try:
        jsonschema.validate(envelope, schema)
    except jsonschema.ValidationError as e:
        print("schema_violation", file=sys.stderr)
        print(f"{e.message}", file=sys.stderr)
        if repairs:
            print(
                f"(repair layer applied {len(repairs)} fix(es) but envelope "
                f"still fails validation)",
                file=sys.stderr,
            )
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
