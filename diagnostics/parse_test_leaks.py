#!/usr/bin/env python3
"""Extract exact RC- and process-leak metrics from test-all leg artifacts."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

RC_ALLOCATION_RE = re.compile(
    r"^\s*(?:"
    r"ori:\s*(?P<ori_count>\d+)\s+RC\s+allocation\(s\)\s+not freed"
    r"|ARC\s+leak:\s*(?P<arc_count>\d+)\s+allocation\(s\)\s+not freed"
    r")",
    re.IGNORECASE | re.MULTILINE,
)
RC_LEAK_EVIDENCE_RE = re.compile(
    r"\b(?:RC\s+allocation|ARC\s+leak|memory\s+leak|leaked\s+memory)\b",
    re.IGNORECASE,
)
NEXTEST_STATUS_RE = re.compile(
    r"^\s*(?P<status>PASS|FAIL|SKIP|LEAK|RETRY|SLOW)\s+"
    r"\[\s*[^\]]*\]\s+(?:\([^)]*\)\s+)?(?P<binary>\S+)\s+(?P<name>\S+)"
)
NEXTEST_SUMMARY_RE = re.compile(r"^\s*Summary\s+\[", re.MULTILINE)


def rc_allocation_count(text: str) -> int:
    return sum(
        int(match.group("ori_count") or match.group("arc_count"))
        for match in RC_ALLOCATION_RE.finditer(text)
    )


def nextest_metrics(path: Path) -> tuple[int, int, int, int, int]:
    allocations: dict[str, int] = {}
    process_leaks: set[str] = set()
    current_test = ""
    current_allocations = 0
    current_rc_evidence = False
    current_canonical_rc_diagnostic = False
    text = path.read_text(encoding="utf-8", errors="replace")

    def flush_test() -> None:
        nonlocal current_test, current_allocations
        nonlocal current_rc_evidence, current_canonical_rc_diagnostic
        if current_allocations and not current_test:
            raise ValueError("RC leak diagnostic is not attributed to a nextest test")
        if (
            current_rc_evidence
            and not current_canonical_rc_diagnostic
        ):
            raise ValueError(
                f"RC leak evidence for {current_test or 'unattributed output'} "
                "has no parseable allocation count"
            )
        if current_test and current_allocations:
            allocations[current_test] = max(
                allocations.get(current_test, 0), current_allocations
            )
        current_test = ""
        current_allocations = 0
        current_rc_evidence = False
        current_canonical_rc_diagnostic = False

    for line in text.splitlines():
        status = NEXTEST_STATUS_RE.match(line)
        if status:
            flush_test()
            identity = f"{status.group('binary')}::{status.group('name')}"
            current_test = identity
            if status.group("status") == "LEAK":
                process_leaks.add(identity)
            continue
        current_rc_evidence |= bool(RC_LEAK_EVIDENCE_RE.search(line))
        current_canonical_rc_diagnostic |= bool(RC_ALLOCATION_RE.search(line))
        current_allocations += rc_allocation_count(line)
    flush_test()
    complete = bool(NEXTEST_SUMMARY_RE.search(text))
    return (
        len(allocations),
        sum(allocations.values()),
        int(complete or bool(allocations)),
        len(process_leaks),
        int(complete or bool(process_leaks)),
    )


def ori_metrics(path: Path) -> tuple[int, int, int, int, int]:
    document = json.loads(path.read_text(encoding="utf-8"))
    allocations: dict[tuple[str, str], int] = {}
    observed_results = 0
    for file_entry in document.get("files", []):
        source_path = str(file_entry.get("path") or "")
        for result in file_entry.get("results", []):
            observed_results += 1
            outcome = result.get("outcome")
            if not isinstance(outcome, dict):
                continue
            diagnostic = outcome.get("Failed") or outcome.get("LlvmCompileFail") or ""
            diagnostic_text = str(diagnostic)
            if RC_LEAK_EVIDENCE_RE.search(diagnostic_text) and not RC_ALLOCATION_RE.search(
                diagnostic_text
            ):
                raise ValueError(
                    f"RC leak evidence for {source_path}:{result.get('name') or ''} "
                    "has no parseable allocation count"
                )
            count = rc_allocation_count(diagnostic_text)
            if count:
                identity = (source_path, str(result.get("name") or ""))
                allocations[identity] = max(allocations.get(identity, 0), count)
        for index, error in enumerate(file_entry.get("errors", [])):
            error_text = str(error)
            if RC_LEAK_EVIDENCE_RE.search(error_text) and not RC_ALLOCATION_RE.search(
                error_text
            ):
                raise ValueError(
                    f"RC leak evidence for {source_path}:file-error-{index} "
                    "has no parseable allocation count"
                )
            count = rc_allocation_count(error_text)
            if count:
                identity = (source_path, f"file-error-{index}")
                allocations[identity] = max(allocations.get(identity, 0), count)
    return (
        len(allocations),
        sum(allocations.values()),
        int(bool(observed_results or allocations)),
        0,
        0,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--nextest", type=Path)
    source.add_argument("--ori-json", type=Path)
    args = parser.parse_args()

    try:
        metrics = (
            nextest_metrics(args.nextest)
            if args.nextest is not None
            else ori_metrics(args.ori_json)
        )
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        parser.error(str(exc))
    print(*metrics)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
