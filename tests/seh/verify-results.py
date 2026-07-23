#!/usr/bin/env python3
"""Check recorded SEH witness results against the expected contract.

Usage:
    verify-results.py <results.jsonl> [--arm cured] [--expected expected.json]

Exit codes:
    0  every expected witness matched
    1  a mismatch, a missing witness, or an unreadable/empty results file
    2  a usage or contract-load error

Reads one JSON object per line as produced by record-witnesses.ps1. Absence of a
required record is a FAILURE, never a pass.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def load_jsonl(path: Path) -> list[dict]:
    """Return the parsed records in `path`. Raises ValueError when unreadable."""
    try:
        text = path.read_text(encoding="utf-8-sig")
    except OSError as exc:
        raise ValueError(f"cannot read results file {path}: {exc}") from exc
    rows: list[dict] = []
    for lineno, line in enumerate(text.splitlines(), start=1):
        line = line.strip()
        if not line:
            continue
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError as exc:
            raise ValueError(f"{path}:{lineno}: malformed JSON: {exc}") from exc
    return rows


def normalize(stream: str | None) -> str:
    """Return `stream` with CRLF collapsed to LF and None mapped to empty."""
    return (stream or "").replace("\r\n", "\n")


def check(rows: list[dict], contract: dict, arm: str) -> list[str]:
    """Return a list of failure strings; empty means every witness matched."""
    failures: list[str] = []
    by_name = {r.get("witness"): r for r in rows if r.get("arm") == arm}
    for witness in contract["witnesses"]:
        name = witness["name"]
        want = witness["expect"]
        got = by_name.get(name)
        if got is None:
            failures.append(f"{name}: MISSING from results for arm '{arm}'")
            continue
        if not got.get("build_ok", False):
            failures.append(f"{name}: build_ok is false")
            continue
        for field in ("exit_signed", "exit_hex"):
            if got.get(field) != want[field]:
                failures.append(
                    f"{name}: {field} expected {want[field]!r}, got {got.get(field)!r}"
                )
        for field in ("stdout", "stderr"):
            if normalize(got.get(field)) != normalize(want[field]):
                failures.append(
                    f"{name}: {field} expected {want[field]!r}, "
                    f"got {normalize(got.get(field))!r}"
                )
    return failures


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("results", type=Path, help="JSONL produced by the recorder")
    parser.add_argument("--arm", default="cured", help="arm to check (default: cured)")
    parser.add_argument(
        "--expected",
        type=Path,
        default=Path(__file__).with_name("expected.json"),
        help="contract file (default: expected.json beside this script)",
    )
    args = parser.parse_args(argv)

    try:
        contract = json.loads(args.expected.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"FAIL: cannot load contract {args.expected}: {exc}", file=sys.stderr)
        return 2

    try:
        rows = load_jsonl(args.results)
    except ValueError as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 1

    if not rows:
        print(f"FAIL: {args.results} contains no records", file=sys.stderr)
        return 1

    failures = check(rows, contract, args.arm)
    if failures:
        print(f"FAIL ({args.arm}): {len(failures)} problem(s)", file=sys.stderr)
        for line in failures:
            print(f"  - {line}", file=sys.stderr)
        defect = contract.get("defect_signature", {})
        arm_rows = [r for r in rows if r.get("arm") == args.arm]
        if any(r.get("exit_hex") == defect.get("exit_hex") for r in arm_rows):
            print(f"  note: {defect.get('exit_hex')} present - {defect.get('meaning')}",
                  file=sys.stderr)
        return 1

    print(f"PASS ({args.arm}): {len(contract['witnesses'])} witnesses matched")
    return 0


if __name__ == "__main__":
    sys.exit(main())
