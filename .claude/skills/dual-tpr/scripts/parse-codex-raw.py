#!/usr/bin/env python3
"""parse-codex-raw.py — extract raw agent_message text from codex JSONL.

Usage:
    parse-codex-raw.py --jsonl PATH

Reads the codex JSONL stream from PATH, finds the LAST item.completed
agent_message event, and prints its raw text to stdout. On failure,
prints a category code to stderr and exits non-zero.

Unlike parse-codex.py, this script performs NO JSON parsing of the
agent_message contents, NO schema validation, NO envelope_invariants
checks. The agent_message text IS the final answer in raw prose — this
is the concatenation-mode sibling of parse-codex.py, used by /tp-help
(and any other concat-mode consumer).

Rationale for a sibling script instead of a --raw flag on parse-codex.py:
the envelope parser's validation logic (jsonschema + envelope_invariants)
is substantial and branching on a --raw flag inside it would halve the
test coverage for each mode. Keeping the raw parser as a tiny sibling
keeps the two code paths independently testable. See dual-tpr-gemini
plan §07.2 for the design decision.

Behavior contract:
- "Last agent_message wins": if the JSONL stream contains multiple
  item.completed agent_message events, the LAST one's text is printed.
  (Semantic pin C2 in transport-tests.sh raw_parsers matrix.)
- Malformed lines are skipped, not fatal: codex sometimes writes
  mid-stream noise, and tail truncation can produce half-written
  final lines. The parser tolerates both.
- If no agent_message is found in the stream (either empty file or
  a stream containing only non-agent_message events), the parser
  prints "missing_agent_message" to stderr and exits 1.

Outcome codes (stderr first line on failure):
    missing_agent_message — no item.completed/agent_message found
"""

import argparse
import json
import sys


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.strip().split("\n\n")[0])
    ap.add_argument("--jsonl", required=True, help="path to codex JSONL stream")
    args = ap.parse_args()

    last_text = None
    with open(args.jsonl) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                # Tolerate malformed lines — codex occasionally emits
                # mid-stream noise and tail truncation is common when the
                # stream is interrupted. The C4 (malformed) and C6
                # (truncated) cells in the raw_parsers matrix pin this.
                continue
            if (
                obj.get("type") == "item.completed"
                and obj.get("item", {}).get("type") == "agent_message"
            ):
                # Last agent_message wins (semantic pin C2).
                last_text = obj["item"].get("text")

    if last_text is None:
        print("missing_agent_message", file=sys.stderr)
        print(
            "no item.completed/agent_message found in JSONL stream",
            file=sys.stderr,
        )
        sys.exit(1)

    # Use sys.stdout.write (not print) for parity with parse-gemini-raw.py.
    # §07.3 TPR v2 DRIFT finding: the original print() appended a trailing
    # newline while the gemini raw parser did not, producing asymmetric
    # output between the two reviewer blocks when tp_help_emit_block
    # wrapped them. Aligning to bare sys.stdout.write gives both blocks
    # the same shape (no trailing newline — the caller adds structure).
    sys.stdout.write(last_text)
    sys.exit(0)


if __name__ == "__main__":
    main()
