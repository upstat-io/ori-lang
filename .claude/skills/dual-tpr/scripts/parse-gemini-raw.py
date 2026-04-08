#!/usr/bin/env python3
"""parse-gemini-raw.py — extract raw assistant text from gemini stream-json.

Usage:
    parse-gemini-raw.py --jsonl PATH

Reads the gemini stream-json stream from PATH, concatenates all
delta:true assistant content chunks in arrival order, waits for a
terminal {"type":"result","status":"success"} event, and prints the
concatenated text to stdout on success.

Unlike parse-gemini.py, this script performs NO sentinel extraction
(no BEGIN/END markers), NO fenced JSON block extraction, NO schema
validation. The concatenated assistant content IS the final answer
in raw prose — this is the concatenation-mode sibling of
parse-gemini.py, used by /tp-help (and any other concat-mode consumer).

Rationale for a sibling script instead of a --raw flag on parse-gemini.py:
the envelope parser's sentinel extraction + jsonschema validation is
substantial and branching on a --raw flag inside it would halve the
test coverage for each mode. Keeping the raw parser as a tiny sibling
keeps the two code paths independently testable. See dual-tpr-gemini
plan §07.2 for the design decision.

Behavior contract:
- Only delta:true assistant messages are concatenated. Non-delta
  messages (delta:false or delta field absent) are IGNORED. This
  distinguishes the raw parser from parse-gemini.py, which is more
  permissive because the envelope is extracted from sentinels in the
  full stream regardless of delta status. (Semantic pin G1 + cell G6
  pin this in the raw_parsers matrix.)
- The stream MUST terminate with {"type":"result","status":"success"}.
  Any other terminal (failure, cancelled, truncation) produces a
  missing_terminator error. (Negative pins G3 + G7 enforce this.)
- Malformed JSON lines are SKIPPED, not fatal. This matches the
  tolerance behavior of parse-codex-raw.py AND the envelope-mode
  parse-gemini.py sibling, both of which use `except JSONDecodeError:
  continue` to skip mid-stream noise and tail truncation. §07.3 TPR
  finding: the original raw-gemini parser was stricter than both its
  codex peer and its envelope sibling (a DRIFT violation per
  impl-hygiene.md SSOT rule), which would silently drop the entire
  gemini contribution whenever a single line in a 100-line stream was
  malformed. Aligning to skip-malformed preserves the dual-source
  parity contract: one bad line should not silence an entire reviewer.
  (Cell G5 pins this aligned behavior.)

Outcome codes (stderr first line on failure):
    missing_terminator        — no result/success event in the stream
    missing_assistant_content — terminator present but no delta:true chunks

BLOAT NOTE (§07.3 TPR v2, gemini severity=NOTE): this parser accumulates
all delta:true chunks into an in-memory list before joining, which
produces a transient memory spike for multi-megabyte responses. The
memory cost is bounded by gemini's upstream API output cap (~1-2MB in
practice) so this is not a current concern. Restructuring to streaming
is tempting but would break the error-ordering semantics — we'd have
to write partial content to stdout BEFORE discovering a missing
terminator, leaking garbage to consumers on the error path. The
list-based approach preserves atomic "all-or-nothing" output: either
the full concatenation is written to stdout and we exit 0, or nothing
is written and we exit non-zero with a category on stderr. If the
memory cost ever matters (e.g., gemini's cap increases 100x), revisit
with a streaming-plus-atomic-commit approach (e.g., write to a temp
file then print its contents, or use a tee-to-memory pattern). For
now, documented acknowledgment is the correct response.
"""

import argparse
import json
import sys


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.strip().split("\n\n")[0])
    ap.add_argument("--jsonl", required=True, help="path to gemini stream-json")
    args = ap.parse_args()

    chunks: list[str] = []
    terminated = False

    with open(args.jsonl) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                # Skip malformed lines — symmetric with parse-codex-raw.py
                # and parse-gemini.py (the envelope-mode sibling). §07.3
                # TPR finding: the original raw-gemini behavior of
                # exit-on-first-JSONDecodeError was stricter than both
                # siblings and would silently drop an entire reviewer's
                # contribution when a single line was malformed — a
                # DRIFT violation per impl-hygiene.md §SSOT.
                continue

            etype = obj.get("type")
            if (
                etype == "message"
                and obj.get("role") == "assistant"
                and obj.get("delta", False)
            ):
                # Only delta:true chunks are concatenated (cell G6 pins this).
                chunks.append(obj.get("content", ""))
            elif etype == "result" and obj.get("status") == "success":
                terminated = True
                break

    if not terminated:
        # Negative pins G3 (no result event) + G7 (result status=failure).
        print("missing_terminator", file=sys.stderr)
        print(
            "no terminal result/success event found in stream",
            file=sys.stderr,
        )
        sys.exit(1)

    if not chunks:
        # Terminator present but no delta:true content (cell G4).
        print("missing_assistant_content", file=sys.stderr)
        print(
            "terminator present but no delta:true assistant chunks",
            file=sys.stderr,
        )
        sys.exit(1)

    sys.stdout.write("".join(chunks))
    sys.exit(0)


if __name__ == "__main__":
    main()
