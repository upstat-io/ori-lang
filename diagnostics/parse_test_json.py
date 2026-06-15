#!/usr/bin/env python3
"""Parse the `ori test --format json` summary into shell-consumable output.

Single source of truth for consuming the runner's machine-readable test
summary. Replaces the hand-rolled bash text-scrape (`grep "N passed, N failed"`)
and the bash JSON escaper (`json_escape_string`): json.load reads the runner's
serde-escaped object, and json.dumps re-emits failure entries with control
chars escaped by construction.

JSON contract (from compiler/oric/src/test/result/mod.rs render_json):
  top-level: passed, failed, skipped, skipped_unchanged, llvm_compile_fail,
             error_files, llvm_compile_fail_files, duration_ns, files[]
  each files[] entry: path, results[], passed, failed, skipped,
                      skipped_unchanged, llvm_compile_fail, duration_ns,
                      errors[], llvm_compile_error
  each results[] entry: name, targets[], outcome, duration_ns
    outcome is "Passed" | {"Failed": <msg>} | {"Skipped": <reason>}
            | "SkippedUnchanged" | {"LlvmCompileFail": <reason>}

Modes (exactly one):
  --counts        emit KEY=value lines: PASSED, FAILED, SKIPPED, LCFAIL,
                  CRASHED (always 0 here; crash detection keys on exit code,
                  not output). Mirrors the parse_ori_results prefix convention.
  --failures-json emit a JSON array of Ori failure objects via json.dumps. Each
                  object: test_id, test_id_kind, suite, failure_kind,
                  error_message. Requires --suite. Reads the runner JSON.
  --summary-line  emit the reconstructed human summary line (the console UX
                  the JSON consumption replaces the runner's text render with).
  --fail-lines    emit one human-readable `FAIL: <name> - <msg>` line per failed
                  test (the console failure-detail the runner's text render gave).
  --rust-failures emit a JSON array of Rust (cargo/libtest) failure objects via
                  json.dumps. Scrapes `test <name> ... FAILED` lines from the
                  cargo text log (libtest output, NOT the Ori runner JSON).
                  Requires --suite. The single JSON-emission home so no bash
                  string escaper survives; control chars escaped by json.dumps.

Input: a file path argument, or stdin when no path is given. For --rust-failures
the input is a cargo text log; for the other modes it is the runner JSON object.

Parse-error contract (per the JSON parse-error fallback): invalid JSON exits
NON-ZERO with a diagnostic on stderr. NEVER silent default-zero — default-zero
is exactly what masked the malformed-output bug the JSON consumption fixes.
"""

import argparse
import json
import re
import sys

EXIT_OK = 0
EXIT_USAGE = 2
EXIT_PARSE_ERROR = 3

# cargo libtest failure line: `test <name> ... FAILED`.
_RUST_FAIL_RE = re.compile(r"^test\s+(?P<name>.*?)\s+\.\.\.\s+FAILED$")
# cargo-nextest failure line: `<indent>FAIL [   0.038s] (7487/7887) <crate> <test::path>`.
# The `(n/total)` progress index is present in streamed runs, absent in some
# modes; the time field varies (`0.038s` / `1.2s ` / `12.34s`). Both optional.
_NEXTEST_FAIL_RE = re.compile(
    r"^\s*FAIL \[[^\]]*\]\s+(?:\(\d+/\d+\)\s+)?(?P<crate>\S+)\s+(?P<name>\S+)\s*$"
)


def _load(path):
    """Load and validate the runner JSON object. Raise ValueError on any
    shape that is not a JSON object carrying the aggregate count fields."""
    raw = sys.stdin.read() if path is None else open(path, encoding="utf-8").read()
    obj = json.loads(raw)  # raises json.JSONDecodeError on malformed input
    if not isinstance(obj, dict):
        raise ValueError(f"expected a JSON object, got {type(obj).__name__}")
    for key in ("passed", "failed", "skipped", "llvm_compile_fail"):
        if key not in obj:
            raise ValueError(f"missing required aggregate count field: {key!r}")
        if not isinstance(obj[key], int):
            raise ValueError(f"aggregate count {key!r} is not an integer")
    return obj


def _emit_counts(obj):
    print(f"PASSED={obj['passed']}")
    print(f"FAILED={obj['failed']}")
    print(f"SKIPPED={obj['skipped']}")
    print(f"LCFAIL={obj['llvm_compile_fail']}")
    print("CRASHED=0")


def _emit_summary_line(obj):
    """Reconstruct the runner's human summary line from the parsed object,
    matching print_summary_stats (compiler/oric/src/commands/test.rs)."""
    parts = [
        f"{obj['passed']} passed",
        f"{obj['failed']} failed",
        f"{obj['skipped']} skipped",
    ]
    if obj.get("skipped_unchanged", 0) > 0:
        parts.append(f"{obj['skipped_unchanged']} skipped (unchanged)")
    if obj.get("llvm_compile_fail", 0) > 0:
        parts.append(f"{obj['llvm_compile_fail']} llvm compile fail")
    if obj.get("error_files", 0) > 0:
        parts.append(f"{obj['error_files']} files with errors")
    print(", ".join(parts))


def _failure_entries(obj, suite):
    """Yield a failure object per failed / llvm-compile-fail test, plus one
    per file-level error. The runner already resolved interned names to
    strings and serde-escaped every message; json.dumps re-escapes on emit."""
    entries = []
    for file_summary in obj.get("files", []):
        for result in file_summary.get("results", []):
            outcome = result.get("outcome")
            name = result.get("name", "")
            if isinstance(outcome, dict) and "Failed" in outcome:
                msg = outcome["Failed"]
                entries.append({
                    "test_id": name,
                    "test_id_kind": "ori_spec",
                    "suite": suite,
                    "failure_kind": "assertion_failure",
                    "error_message": msg,
                })
            elif isinstance(outcome, dict) and "LlvmCompileFail" in outcome:
                reason = outcome["LlvmCompileFail"]
                entries.append({
                    "test_id": name,
                    "test_id_kind": "ori_spec",
                    "suite": suite,
                    "failure_kind": "llvm_compile_fail",
                    "error_message": reason,
                })
        for error in file_summary.get("errors", []):
            entries.append({
                "test_id": file_summary.get("path", ""),
                "test_id_kind": "ori_spec",
                "suite": suite,
                "failure_kind": "file_error",
                "error_message": error,
            })
    return entries


def _emit_fail_lines(obj):
    """Human-readable per-failure listing for the console (the detail the
    runner's text render emitted; reconstructed from the parsed JSON)."""
    for file_summary in obj.get("files", []):
        printed_path = False
        for result in file_summary.get("results", []):
            outcome = result.get("outcome")
            name = result.get("name", "")
            label = None
            if isinstance(outcome, dict) and "Failed" in outcome:
                label = f"  FAIL: {name} - {outcome['Failed']}"
            elif isinstance(outcome, dict) and "LlvmCompileFail" in outcome:
                label = f"  LLVM COMPILE FAIL: {name} - {outcome['LlvmCompileFail']}"
            if label is not None:
                if not printed_path:
                    print(file_summary.get("path", ""))
                    printed_path = True
                print(label)
        for error in file_summary.get("errors", []):
            if not printed_path:
                print(file_summary.get("path", ""))
                printed_path = True
            print(f"  ERROR: {error}")


def _emit_failures_json(obj, suite):
    # json.dumps escapes control chars (tab/newline/quote) by construction —
    # this is what retires the hand-rolled bash json_escape_string.
    print(json.dumps(_failure_entries(obj, suite), ensure_ascii=False))


def _rust_failure_entries(text, suite, failure_kind):
    """Yield a failure object per failed Rust test in a cargo OR nextest log.

    Matches both runner formats so failure-name attribution survives the
    cargo-test -> cargo-nextest runner switch:
    - cargo libtest:  `test <name> ... FAILED`
    - cargo-nextest:  `<indent>FAIL [ <time> ] (<n>/<total>) <crate> <test::path>`
    A test_id is emitted at most once even if both forms appear (dedup by id).
    """
    kind = "panic" if "panicked at" in text else failure_kind
    entries = []
    seen = set()
    for line in text.splitlines():
        cargo = _RUST_FAIL_RE.match(line)
        if cargo:
            name = cargo.group("name")
            if name in seen:
                continue
            seen.add(name)
            entries.append({
                "test_id": name,
                "test_id_kind": "rust",
                "suite": suite,
                "failure_kind": kind,
                "error_message": f"test {name} ... FAILED",
            })
            continue
        nextest = _NEXTEST_FAIL_RE.match(line)
        if nextest:
            crate = nextest.group("crate")
            name = nextest.group("name")
            if name in seen:
                continue
            seen.add(name)
            entries.append({
                "test_id": name,
                "test_id_kind": "rust",
                "suite": suite,
                "failure_kind": kind,
                "error_message": f"FAIL {crate} {name} (nextest)",
            })
    return entries


def _emit_rust_failures(text, suite, failure_kind):
    print(json.dumps(_rust_failure_entries(text, suite, failure_kind), ensure_ascii=False))


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--counts", action="store_true")
    mode.add_argument("--failures-json", action="store_true")
    mode.add_argument("--summary-line", action="store_true")
    mode.add_argument("--fail-lines", action="store_true")
    mode.add_argument("--rust-failures", action="store_true")
    parser.add_argument("--suite", help="suite id for failure entries")
    parser.add_argument("--failure-kind", default="panic",
                        help="default failure_kind for --rust-failures")
    parser.add_argument("path", nargs="?", help="input file (default: stdin)")
    args = parser.parse_args(argv)

    if (args.failures_json or args.rust_failures) and not args.suite:
        print("error: failure modes require --suite", file=sys.stderr)
        return EXIT_USAGE

    # --rust-failures reads cargo TEXT (no JSON parse, no parse-error contract:
    # a missing/empty log is an empty failure list, not a malformed runner JSON).
    if args.rust_failures:
        try:
            text = sys.stdin.read() if args.path is None else \
                open(args.path, encoding="utf-8").read()
        except OSError:
            text = ""
        _emit_rust_failures(text, args.suite, args.failure_kind)
        return EXIT_OK

    try:
        obj = _load(args.path)
    except (json.JSONDecodeError, ValueError, OSError) as exc:
        # Parse-error contract: non-zero exit, never silent default-zero.
        print(f"parse_test_json: invalid runner JSON: {exc}", file=sys.stderr)
        return EXIT_PARSE_ERROR

    if args.counts:
        _emit_counts(obj)
    elif args.failures_json:
        _emit_failures_json(obj, args.suite)
    elif args.summary_line:
        _emit_summary_line(obj)
    elif args.fail_lines:
        _emit_fail_lines(obj)
    return EXIT_OK


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
