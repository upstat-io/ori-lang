#!/usr/bin/env python3
"""Round-trip-equality check for the adversarial failure-message fixture.

Asserts a failure message survives the json.dumps -> file -> json.load round
trip byte-for-byte, not parse-success alone: control chars (tab/newline/quote)
and non-ASCII (unicode) in a failure message must reach the parsed summary
unchanged.

Reads a summary JSON file (the harness test-all-summary.json shape with
suites[].failures[].message, OR the runner --format json shape with
files[].results[].outcome), json.load (which IS the round-trip parse and the
`jq empty` validity gate at once), and collects every failure message. With
--expect-message, asserts the expected bytes appear among the collected
messages byte-for-byte; absent the flag, the round-trip parse alone is the
assertion.

Imports parse_test_json so the runner-JSON failure extraction stays single-
source (no second parser path, per the section's no-second-text-path principle).

Exit codes (parse-error contract shared with parse_test_json):
  0 ok; 2 usage; 3 invalid JSON; 4 round-trip mismatch (expected absent/altered).
"""

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import parse_test_json  # noqa: E402

EXIT_OK = 0
EXIT_USAGE = 2
EXIT_PARSE_ERROR = 3
EXIT_MISMATCH = 4


def _harness_failure_messages(obj):
    """Failure messages from the harness test-all-summary.json shape:
    suites[].failures[].message."""
    messages = []
    for suite in obj.get("suites", []):
        for failure in suite.get("failures", []):
            if "message" in failure:
                messages.append(failure["message"])
    return messages


def _runner_failure_messages(obj):
    """Failure messages from the runner --format json shape via the
    parse_test_json SSOT extractor (suite label irrelevant for comparison)."""
    return [e["error_message"] for e in parse_test_json._failure_entries(obj, "")]


def collect_messages(obj):
    """Every failure message in either supported summary shape."""
    return _harness_failure_messages(obj) + _runner_failure_messages(obj)


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("path", help="summary JSON file (harness or runner shape)")
    parser.add_argument(
        "--expect-message",
        help="the exact failure-message bytes that must survive the round trip",
    )
    args = parser.parse_args(argv)

    try:
        raw = open(args.path, encoding="utf-8").read()
        obj = json.loads(raw)  # round-trip parse == jq empty validity gate
    except (json.JSONDecodeError, OSError) as exc:
        print(
            f"parse_test_json_roundtrip_check: invalid summary JSON: {exc}",
            file=sys.stderr,
        )
        return EXIT_PARSE_ERROR
    if not isinstance(obj, dict):
        print(
            f"parse_test_json_roundtrip_check: expected a JSON object, got "
            f"{type(obj).__name__}",
            file=sys.stderr,
        )
        return EXIT_PARSE_ERROR

    messages = collect_messages(obj)

    if args.expect_message is None:
        # Round-trip parse alone is the assertion (jq-empty equivalent).
        return EXIT_OK

    if args.expect_message not in messages:
        print(
            "parse_test_json_roundtrip_check: round-trip MISMATCH — expected "
            "message not found byte-for-byte among parsed failures",
            file=sys.stderr,
        )
        print(f"  expected: {args.expect_message!r}", file=sys.stderr)
        for msg in messages:
            print(f"  parsed:   {msg!r}", file=sys.stderr)
        return EXIT_MISMATCH

    return EXIT_OK


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
