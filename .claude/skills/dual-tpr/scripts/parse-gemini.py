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
    schema_violation        — [RESCUED: no longer causes exit(1)]
                              JSON validates against neither shape nor schema
                              even after repair attempt. Previously fatal;
                              now rescued — envelope written to stdout with
                              RESCUED warnings on stderr. See §Rescue mode.
    failed_partial          — envelope validates but status != "complete"

Sentinel-less fallback:
    When gemini omits the BEGIN/END sentinels but produces a fenced JSON block
    (```json ... ```) that looks like a findings envelope, the parser falls
    back to extracting that block directly. A WARNING is printed to stderr.
    This resilience measure exists because gemini inconsistently follows
    sentinel instructions despite them being clearly specified in the
    reviewer skill file.

    The fallback acceptance guard is composed with the repair layer
    downstream: repair_envelope.py auto-fills missing `schema_version`,
    `status`, `reviewer`, `skill`, and `scope_actually_reviewed`, so the
    guard here only needs a strong-marker check that distinguishes a review
    envelope from random fenced JSON (config blobs, package.json, schema
    examples, etc.) WITHOUT requiring fields the repair layer would
    synthesize. An earlier incarnation of this guard required literal
    "schema_version" AND "status" substring matches, which rejected minimal
    clean-pass envelopes like
        {"skill":"review-plan","reviewer":"gemini","no_findings":true,"findings":[]}
    that repair_envelope.py would have made fully valid — a composition
    failure between the two resilience layers.

    Strong markers accepted by the guard (any one is sufficient):
      - "no_findings"             — unique top-level bool, review-envelope only
      - "scope_actually_reviewed" — unique top-level nested object
      - "reviewer" ∈ {codex, gemini}
      - "findings" is a list AND "skill" ∈ {tpr-review, review-work,
        review-plan, tp-help}

    The parser collects ALL fenced JSON blocks in arrival order and picks
    the LAST one that parses as a dict and matches the guard. "Last wins"
    handles the case where gemini shows a schema example in an earlier
    fenced block before emitting the real envelope in a later one. The
    sentinel requirement remains in the skill file; this is a parser-level
    safety net, not an endorsement of sentinel-less output.

Rescue mode (2026-04-13):
    When schema or invariant validation fails AFTER the repair layer has run,
    the parser accepts the repaired envelope as-is instead of exiting with
    schema_violation. This prevents costly full-review retries (10+ minutes
    each) for cosmetic JSON structure issues when the findings content is
    intact. RESCUED envelopes are identified by stderr lines starting with
    "RESCUED:". The only remaining fatal exit for content-bearing envelopes
    is failed_partial (status != "complete").

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

# Envelope shape markers for sentinel-less fallback. A fenced JSON block is
# accepted as a review envelope if it parses to a dict and satisfies AT LEAST
# ONE of the markers below. All other fields (schema_version, status,
# scope_actually_reviewed, etc.) are filled in by repair_envelope.py
# downstream, so the guard does NOT check for them.
_ENVELOPE_SKILLS = frozenset(
    ("tpr-review", "review-work", "review-plan", "tp-help", "custom")
)
_ENVELOPE_REVIEWERS = frozenset(("codex", "gemini"))


def looks_like_review_envelope(parsed: object) -> bool:
    """Strong-marker shape check for sentinel-less fallback acceptance.

    Returns True if ``parsed`` is a dict containing any field that is
    virtually unique to review envelopes and would not be synthesized by
    repair_envelope.py. See the module docstring §Sentinel-less fallback
    for the full marker list and rationale.
    """
    if not isinstance(parsed, dict):
        return False
    # Strong markers — any one is sufficient.
    if "no_findings" in parsed:
        return True
    if "scope_actually_reviewed" in parsed:
        return True
    if parsed.get("reviewer") in _ENVELOPE_REVIEWERS:
        return True
    # Medium marker: findings-list + skill-enum combination. Either alone is
    # too weak (random JSON can have a "findings" key or a "skill" string),
    # but the combination is load-bearing.
    if isinstance(parsed.get("findings"), list) and parsed.get("skill") in _ENVELOPE_SKILLS:
        return True
    return False


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


def _flush_advisory(deferred):
    """Flush deferred advisory lines to stderr.

    Advisory lines (WARNING, REPAIR) are emitted AFTER the primary output
    (envelope on stdout, or category line on stderr) so they can't corrupt
    `head -1 parse-error` extraction in dual-invoke-with-retry.sh.

    The stderr-first-line-is-category contract is load-bearing: the retry
    classifier in dual-invoke-with-retry.sh uses it to decide whether a
    failure is terminal (don't retry) or retryable. Prior to this indirection,
    sentinel-less fallback + schema violation corrupted the category to
    `gemini_WARNING: sentinel-less fallback — gemini omitted BEGIN/END...`
    which matched no classifier entry and was retried by accident.
    """
    for msg in deferred:
        print(msg, file=sys.stderr)


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
    # --default-skill threads the transport's actual review mode into the
    # repair layer so a missing/invalid `skill` field in a custom or
    # review-plan run is NOT silently rewritten to "review-work" (which
    # would misclassify envelope provenance in downstream reports). The
    # transport (dual-invoke-with-retry.sh) passes the active --skill
    # verbatim; falls back to "review-work" only when the caller does not
    # supply one (backward-compatible with legacy invocations).
    ap.add_argument(
        "--default-skill",
        default="review-work",
        help="skill name used as fallback when the envelope omits/mangles "
        "the 'skill' field (matches the transport's active --skill)",
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

    # deferred_advisory accumulates WARNING/REPAIR lines that would otherwise
    # corrupt the stderr-first-line-is-category contract (see _flush_advisory
    # docstring). Initialized here so every failure path below can safely
    # reference it — even failures that happen before the sentinel-less
    # fallback branch executes.
    deferred_advisory = []

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
        # produces a valid fenced JSON envelope. Collect ALL fenced JSON
        # blocks in arrival order and pick the LAST one that parses as a
        # dict matching the review-envelope shape guard. See the module
        # docstring §Sentinel-less fallback for rationale; the guard
        # composes with repair_envelope.py so minimal envelopes like
        # {"skill":..., "reviewer":..., "no_findings":true, "findings":[]}
        # are accepted and completed downstream.
        all_blocks = FENCE_RE.findall(full_text)
        if not all_blocks:
            print("missing_begin_sentinel", file=sys.stderr)
            print(
                "BEGIN sentinel not found in assistant text and no fenced "
                "JSON block found as fallback",
                file=sys.stderr,
            )
            sys.exit(1)

        json_text = None
        for block in reversed(all_blocks):  # last-block-wins
            try:
                parsed = json.loads(block)
            except json.JSONDecodeError:
                continue
            if looks_like_review_envelope(parsed):
                json_text = block
                break

        if json_text is None:
            print("missing_begin_sentinel", file=sys.stderr)
            print(
                f"BEGIN sentinel not found; inspected {len(all_blocks)} fenced "
                f"JSON block(s) but none matched review-envelope shape "
                f"(need no_findings, scope_actually_reviewed, "
                f"reviewer in {{codex,gemini}}, or findings-list + skill in "
                f"{{tpr-review,review-work,review-plan,tp-help,custom}})",
                file=sys.stderr,
            )
            sys.exit(1)

        # Defer the sentinel-less fallback WARNING. Printing it NOW would
        # violate parse-gemini.py's own contract — see the docstring "Outcome
        # codes (stderr first line on default-mode failure)". If any downstream
        # check (parse, repair, validate, invariants, status) fails, the
        # failure category MUST be the first stderr line so that
        # dual-invoke-with-retry.sh's `head -1 parse-error` gets the real
        # category and not this advisory WARNING. `deferred_advisory` is
        # flushed AFTER the category emission on the failure path, or after
        # the primary output on the success path. (Observed 2026-04-11: the
        # mangled category `gemini_WARNING: sentinel-less fallback ...` was
        # tripping the retry classifier in round.log / status-check output.)
        deferred_advisory.append(
            "WARNING: sentinel-less fallback — gemini omitted BEGIN/END "
            "sentinels but produced a fenced JSON block matching the review "
            "envelope shape. Proceeding with repair + schema validation."
        )

    try:
        envelope = json.loads(json_text)
    except json.JSONDecodeError as e:
        print("parse_fail", file=sys.stderr)
        print(f"fenced JSON block is not valid JSON: {e}", file=sys.stderr)
        _flush_advisory(deferred_advisory)
        sys.exit(1)

    # Repair layer: normalize common schema violations before validation.
    # Gemini frequently omits required fields (schema_version, no_findings),
    # uses enum aliases ("info" instead of "informational"), or produces
    # location formats the invariant regex rejects. The repair layer fixes
    # these in-place so a structurally-correct-but-sloppy envelope doesn't
    # kill a 20-minute review. All repairs are deferred to `deferred_advisory`
    # so they don't violate the stderr-first-line-is-category contract — see
    # the sentinel-less WARNING comment above.
    envelope, repairs = repair_envelope(
        envelope, default_reviewer="gemini", default_skill=args.default_skill,
    )
    if repairs:
        deferred_advisory.append(
            f"REPAIR: applied {len(repairs)} auto-repair(s) to gemini envelope:"
        )
        for r in repairs:
            deferred_advisory.append(f"  REPAIR: {r}")

    # Rescue mode: when schema or invariant validation fails AFTER repair,
    # accept the envelope rather than exiting — a 10+ minute full-review retry
    # for a cosmetic JSON structure issue is unacceptable. The repair layer has
    # already normalized all structurally significant violations; anything that
    # slips through is an edge-case format issue (unusual location characters,
    # unexpected nested type) that doesn't affect downstream merge/display.
    rescued = False

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
            f"Accepting envelope to avoid costly retry."
        )

    if envelope.get("status") != "complete":
        print("failed_partial", file=sys.stderr)
        print(f"envelope status: {envelope.get('status')}", file=sys.stderr)
        _flush_advisory(deferred_advisory)
        sys.exit(1)

    if rescued:
        deferred_advisory.insert(0,
            "RESCUED: gemini envelope accepted despite schema/invariant "
            "violations — content preserved to avoid costly full-review retry"
        )
    json.dump(envelope, sys.stdout, indent=2)
    sys.stdout.write("\n")
    _flush_advisory(deferred_advisory)
    sys.exit(0)


if __name__ == "__main__":
    main()
