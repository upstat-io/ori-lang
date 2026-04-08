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
- Malformed JSON lines are FATAL — the parser exits with parse_fail
  on the first JSONDecodeError. This is stricter than parse-codex-raw.py
  because gemini stream-json is a line-oriented JSONL protocol where
  a malformed line usually indicates a serious framing error rather
  than mid-stream noise. (Cell G5 pins this.)

Outcome codes (stderr first line on failure):
    parse_fail                — invalid JSON on any line
    missing_terminator        — no result/success event in the stream
    missing_assistant_content — terminator present but no delta:true chunks
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
                # Stricter than parse-codex-raw.py: a malformed line in
                # stream-json is a framing error, not mid-stream noise.
                # Cell G5 pins this behavior.
                print("parse_fail", file=sys.stderr)
                print(f"invalid JSON on line: {line[:80]}", file=sys.stderr)
                sys.exit(1)

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
