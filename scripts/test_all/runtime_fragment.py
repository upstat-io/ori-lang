#!/usr/bin/env python3
"""Inventory and result fragments for test-all runtime actions."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import time
from pathlib import Path

SUMMARY_RE = re.compile(r"Summary \[[^\]]+\].*$")
COUNT_RE = re.compile(r"(\d+)\s+(passed|failed|skipped)")
NEXTEST_STATUS_RE = re.compile(
    r"^\s*(PASS|FAIL|SKIP|LEAK|RETRY)\s+\[[^\]]*\]\s+(?:\(\d+/\d+\)\s+)?(?P<binary>\S+)\s+(?P<name>\S+)"
)
LIBTEST_FAILURE_RE = re.compile(r"^test\s+(?P<name>.*?)\s+\.\.\.\s+FAILED$")


def canonical_json(value) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()


def digest(value) -> str:
    return hashlib.sha256(canonical_json(value)).hexdigest()


def atomic_json(path: Path, value) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f".{path.name}.{os.getpid()}.{time.time_ns()}.tmp")
    with tmp.open("wb") as stream:
        stream.write(canonical_json(value) + b"\n")
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(tmp, path)
    fd = os.open(path.parent, os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def inventory_doc(items: list[dict]) -> dict:
    ordered = sorted(items, key=lambda item: item["identity"])
    identities = [item["identity"] for item in ordered]
    if len(identities) != len(set(identities)):
        raise ValueError("inventory contains duplicate identities")
    return {"items": ordered, "items_digest": digest(identities)}


def nextest_inventory(raw_path: Path, output_path: Path) -> None:
    doc = json.loads(raw_path.read_text(encoding="utf-8"))
    suites = doc.get("rust-suites")
    if not isinstance(suites, dict):
        raise ValueError("nextest inventory has no rust-suites object")
    items = []
    for binary_id, suite in suites.items():
        testcases = suite.get("testcases", {}) if isinstance(suite, dict) else {}
        if not isinstance(testcases, dict):
            raise ValueError(f"nextest suite {binary_id!r} has invalid testcases")
        package = str(suite.get("package-name") or suite.get("package_name") or "")
        for test_name, testcase in testcases.items():
            ignored = bool(isinstance(testcase, dict) and testcase.get("ignored", False))
            items.append(
                {
                    "identity": f"{binary_id}::{test_name}",
                    "binary_id": str(binary_id),
                    "package": package,
                    "test_name": str(test_name),
                    "ignored": ignored,
                }
            )
    atomic_json(output_path, inventory_doc(items))


def discover_ori(root: Path, output_path: Path) -> None:
    items = []
    tests_root = root / "tests"
    for path in tests_root.rglob("*.ori"):
        rel_parts = path.relative_to(root).parts
        if any(part.startswith(".") or part in {"target", "node_modules", "__pycache__"} for part in rel_parts):
            continue
        if Path(str(path) + ".expected").exists():
            continue
        rel = path.relative_to(root).as_posix()
        items.append({"identity": rel, "path": rel})
    atomic_json(output_path, inventory_doc(items))


def doctest_inventory(raw_path: Path, output_path: Path) -> None:
    doc = json.loads(raw_path.read_text(encoding="utf-8"))
    workspace = set(doc.get("workspace_members", []))
    items = []
    for package in doc.get("packages", []):
        if package.get("id") not in workspace:
            continue
        targets = package.get("targets", [])
        if not any("lib" in target.get("kind", []) for target in targets):
            continue
        name = str(package["name"])
        items.append({"identity": name, "package": name})
    atomic_json(output_path, inventory_doc(items))


def load_expected(path: Path) -> tuple[list[dict], list[str], str]:
    doc = json.loads(path.read_text(encoding="utf-8"))
    items = list(doc.get("items", []))
    identities = list(doc.get("identities", []))
    expected_digest = str(doc.get("items_digest") or "")
    if identities != sorted(identities) or len(identities) != len(set(identities)):
        raise ValueError("expected identities are not sorted and unique")
    if expected_digest != digest(identities):
        raise ValueError("expected identity digest mismatch")
    return items, identities, expected_digest


def _nextest_counts(text: str) -> dict[str, int]:
    summaries = [line for line in text.splitlines() if SUMMARY_RE.search(line)]
    if not summaries:
        raise ValueError("nextest emitted no parseable Summary line")
    counts = {"passed": 0, "failed": 0, "skipped": 0}
    for value, name in COUNT_RE.findall(summaries[-1]):
        counts[name] = int(value)
    return counts


def _nextest_failures(text: str, suite: str) -> list[dict]:
    failures = {}
    for line in text.splitlines():
        match = NEXTEST_STATUS_RE.match(line)
        if match and match.group(1) == "FAIL":
            test_id = match.group("name")
            failures[test_id] = {
                "test_id": test_id,
                "test_id_kind": "rust",
                "suite": suite,
                "failure_kind": "panic" if f"thread '{test_id}' panicked" in text else "test_failure",
                "error_message": line.strip(),
            }
        cargo = LIBTEST_FAILURE_RE.match(line)
        if cargo:
            test_id = cargo.group("name")
            failures.setdefault(
                test_id,
                {
                    "test_id": test_id,
                    "test_id_kind": "rust",
                    "suite": suite,
                    "failure_kind": "panic" if f"thread '{test_id}' panicked" in text else "test_failure",
                    "error_message": line.strip(),
                },
            )
    return [failures[key] for key in sorted(failures)]


def _nextest_leak_count(text: str) -> int:
    leak_ids = set()
    leak_lines = set()
    for line in text.splitlines():
        match = NEXTEST_STATUS_RE.match(line)
        if match and match.group(1) == "LEAK":
            leak_ids.add((match.group("binary"), match.group("name")))
        elif "leaked memory" in line.lower():
            leak_lines.add(line.strip())
    return len(leak_ids) if leak_ids else len(leak_lines)


def nextest_result(args) -> dict:
    items, identities, expected_digest = load_expected(args.expected)
    text = args.stdout.read_text(encoding="utf-8", errors="replace")
    counts = _nextest_counts(text)
    expected_ignored = sum(bool(item.get("ignored")) for item in items)
    observed = counts["passed"] + counts["failed"] + counts["skipped"]
    if observed != len(identities):
        if counts["skipped"] == 0 and observed + expected_ignored == len(identities):
            counts["skipped"] = expected_ignored
        else:
            raise ValueError(
                f"nextest observed {observed} outcomes for {len(identities)} expected tests"
            )
    if args.returncode not in (0, 100):
        raise ValueError(f"nextest infrastructure exit {args.returncode}")
    failures = _nextest_failures(text, args.suite)
    if len(failures) > counts["failed"]:
        raise ValueError("nextest failure identities exceed failed count")
    status = "failed" if counts["failed"] else "passed"
    payload = {
        "suite": args.suite,
        "display_name": args.display_name,
        **counts,
        "lcfail": 0,
        "aot_leaks": _nextest_leak_count(text),
        "status": status,
        "returncode": args.returncode,
        "failures": failures,
        "observed_test_ids": identities,
        "expected_test_set_digest": expected_digest,
    }
    if args.suite == "aot":
        payload["aot_gate"] = {
            "baseline_identity": args.baseline_identity,
            "post_identity": args.post_identity,
            "manifest_digest": args.manifest_digest,
            "snapshot_verdict": args.snapshot_verdict,
        }
    return payload


def ori_result(args) -> dict:
    _items, identities, expected_digest = load_expected(args.expected)
    doc = json.loads(args.stdout.read_text(encoding="utf-8"))
    for key in ("passed", "failed", "skipped", "llvm_compile_fail", "files"):
        if key not in doc:
            raise ValueError(f"Ori result missing {key}")
    observed_paths = sorted(str(entry.get("path", "")) for entry in doc["files"])
    if observed_paths != identities:
        raise ValueError(f"Ori result path coverage mismatch: expected={identities[:1]} observed={observed_paths[:1]}")
    if args.returncode > 128:
        raise ValueError(f"Ori runner crashed with exit {args.returncode}")
    failures = []
    for file_entry in doc["files"]:
        source_path = str(file_entry.get("path", ""))
        for result in file_entry.get("results", []):
            outcome = result.get("outcome")
            if isinstance(outcome, dict) and "Failed" in outcome:
                failures.append(
                    {
                        "test_id": str(result.get("name", "")),
                        "test_id_kind": "ori_spec",
                        "suite": args.suite,
                        "failure_kind": "assertion_failure",
                        "error_message": str(outcome["Failed"]),
                        "source_path": source_path,
                        "leak_positive": bool(re.search(r"not freed|memory leak|leaked memory|ARC leak", str(outcome["Failed"]), re.I)),
                    }
                )
            elif isinstance(outcome, dict) and "LlvmCompileFail" in outcome:
                failures.append(
                    {
                        "test_id": str(result.get("name", "")),
                        "test_id_kind": "ori_spec",
                        "suite": args.suite,
                        "failure_kind": "llvm_compile_fail",
                        "error_message": str(outcome["LlvmCompileFail"]),
                        "source_path": source_path,
                        "leak_positive": False,
                    }
                )
        for error in file_entry.get("errors", []):
            failures.append(
                {
                    "test_id": source_path,
                    "test_id_kind": "ori_spec",
                    "suite": args.suite,
                    "failure_kind": "file_error",
                    "error_message": str(error),
                    "source_path": source_path,
                    "leak_positive": False,
                }
            )
    failed = int(doc["failed"])
    lcfail = int(doc["llvm_compile_fail"])
    status = "failed" if failed or lcfail else "passed"
    return {
        "suite": args.suite,
        "display_name": args.display_name,
        "passed": int(doc["passed"]),
        "failed": failed,
        "skipped": int(doc["skipped"]),
        "lcfail": lcfail,
        "aot_leaks": 0,
        "status": status,
        "returncode": args.returncode,
        "failures": sorted(failures, key=lambda item: (item["suite"], item["test_id"])),
        "observed_test_ids": identities,
        "expected_test_set_digest": expected_digest,
    }


def doctest_result(args) -> dict:
    items, identities, expected_digest = load_expected(args.expected)
    text = args.stdout.read_text(encoding="utf-8", errors="replace")
    result_lines = [line for line in text.splitlines() if line.startswith("test result:")]
    counts = {"passed": 0, "failed": 0, "skipped": 0}
    for line in result_lines:
        for value, name in re.findall(r"(\d+)\s+(passed|failed|ignored)", line):
            key = "skipped" if name == "ignored" else name
            counts[key] += int(value)
    failures = _nextest_failures(text, args.suite)
    status = "failed" if counts["failed"] else "passed"
    if not result_lines and args.returncode != 0:
        package = str(items[0].get("package", "unknown"))
        counts["failed"] = 1
        status = "build_failed"
        failures = [
            {
                "test_id": package,
                "test_id_kind": "rust",
                "suite": args.suite,
                "failure_kind": "build_failure",
                "error_message": f"doctest package {package} failed before emitting test results",
            }
        ]
    return {
        "suite": args.suite,
        "display_name": args.display_name,
        **counts,
        "lcfail": 0,
        "aot_leaks": 0,
        "status": status,
        "returncode": args.returncode,
        "failures": failures,
        "observed_test_ids": identities,
        "expected_test_set_digest": expected_digest,
    }


def wasm_result(args) -> dict:
    _items, identities, expected_digest = load_expected(args.expected)
    skipped = args.skipped == "1"
    failed = 1 if args.returncode != 0 else 0
    failures = []
    if failed:
        failures.append(
            {
                "test_id": "wasm_playground",
                "test_id_kind": "build",
                "suite": args.suite,
                "failure_kind": "build_failure",
                "error_message": "external playground WASM build failed",
            }
        )
    return {
        "suite": args.suite,
        "display_name": args.display_name,
        "passed": 0 if skipped else 1,
        "failed": failed,
        "skipped": 1 if skipped else 0,
        "lcfail": 0,
        "aot_leaks": 0,
        "status": "failed" if failed else ("skipped" if skipped else "passed"),
        "returncode": args.returncode,
        "failures": failures,
        "observed_test_ids": identities,
        "expected_test_set_digest": expected_digest,
    }


def add_result_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--expected", type=Path, required=True)
    parser.add_argument("--stdout", type=Path, required=True)
    parser.add_argument("--returncode", type=int, required=True)
    parser.add_argument("--suite", required=True)
    parser.add_argument("--display-name", required=True)
    parser.add_argument("--output", type=Path, required=True)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    p = sub.add_parser("nextest-inventory")
    p.add_argument("--raw", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p = sub.add_parser("ori-inventory")
    p.add_argument("--root", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p = sub.add_parser("doctest-inventory")
    p.add_argument("--raw", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p = sub.add_parser("items0")
    p.add_argument("--input", type=Path, required=True)
    p.add_argument("--field", required=True)
    for name in ("nextest-result", "ori-result", "doctest-result", "wasm-result"):
        p = sub.add_parser(name)
        add_result_arguments(p)
        if name == "nextest-result":
            p.add_argument("--baseline-identity", default="")
            p.add_argument("--post-identity", default="")
            p.add_argument("--manifest-digest", default="")
            p.add_argument("--snapshot-verdict", default="")
        if name == "wasm-result":
            p.add_argument("--skipped", choices=("0", "1"), required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.command == "nextest-inventory":
            nextest_inventory(args.raw, args.output)
        elif args.command == "ori-inventory":
            discover_ori(args.root, args.output)
        elif args.command == "doctest-inventory":
            doctest_inventory(args.raw, args.output)
        elif args.command == "items0":
            doc = json.loads(args.input.read_text(encoding="utf-8"))
            for item in doc.get("items", []):
                value = item.get(args.field)
                if not isinstance(value, str):
                    raise ValueError(f"inventory item has no string field {args.field!r}")
                sys.stdout.buffer.write(value.encode() + b"\0")
        else:
            builders = {
                "nextest-result": nextest_result,
                "ori-result": ori_result,
                "doctest-result": doctest_result,
                "wasm-result": wasm_result,
            }
            atomic_json(args.output, builders[args.command](args))
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"runtime_fragment: {exc}", file=sys.stderr)
        return 3
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
