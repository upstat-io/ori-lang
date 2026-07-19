"""Tests for diagnostics/parse_test_json.py --rust-failures mode.

Sibling `test_parse_test_json.py` covers --counts / --summary-line /
--fail-lines; `test_parse_test_json_failures.py` covers --failures-json.

--rust-failures reads cargo/nextest TEXT logs (not the Ori runner JSON) and
scrapes both the cargo-libtest `FAILED` line shape and the cargo-nextest
`FAIL [time] (n/total) crate test::path` shape.
"""

import json


def test_rust_failures_scrapes_failed_lines(run_ptj):
    log = (
        "running 3 tests\n"
        "test foo::bar ... ok\n"
        "test foo::baz ... FAILED\n"
        "test qux ... FAILED\n"
    )
    result = run_ptj(["--rust-failures", "--suite", "rust_workspace"], stdin=log)
    assert result.returncode == 0
    entries = json.loads(result.stdout)
    names = sorted(e["test_id"] for e in entries)
    assert names == ["foo::baz", "qux"]
    assert all(e["test_id_kind"] == "rust" for e in entries)
    assert all(e["suite"] == "rust_workspace" for e in entries)


def test_rust_failures_classifies_panic(run_ptj):
    log = "test t ... FAILED\nthread 'main' panicked at src/x.rs:1\n"
    result = run_ptj(["--rust-failures", "--suite", "rust_rt", "--failure-kind", "assertion"], stdin=log)
    entries = json.loads(result.stdout)
    assert entries[0]["failure_kind"] == "panic"


def test_rust_failures_empty_log_is_empty_array(run_ptj):
    result = run_ptj(["--rust-failures", "--suite", "aot"], stdin="all passed\n")
    assert result.returncode == 0
    assert json.loads(result.stdout) == []


def test_rust_failures_missing_file_is_empty_array(run_ptj, tmp_path):
    missing = tmp_path / "nope.log"
    result = run_ptj(["--rust-failures", "--suite", "aot", str(missing)])
    assert result.returncode == 0
    assert json.loads(result.stdout) == []


def test_rust_failures_requires_suite(run_ptj):
    result = run_ptj(["--rust-failures"], stdin="test t ... FAILED\n")
    assert result.returncode == 2


# --- rust-failures: cargo-nextest FAIL-line format (runner-switch coverage) ---

def test_rust_failures_scrapes_nextest_fail_lines(run_ptj):
    # Real cargo-nextest streamed format: indent, FAIL [time] (n/total) crate test::path
    log = (
        "        PASS [   0.004s] (1/3) oric foo::tests::ok_one\n"
        "        FAIL [   0.038s] (2/3) oric eval::tests::dispatch::handler_missing\n"
        "        FAIL [   1.2s  ] (3/3) ori_arc lattice::tests::join_law\n"
        "     Summary [   3.393s] 3 tests run: 1 passed, 2 failed, 0 skipped\n"
    )
    result = run_ptj(["--rust-failures", "--suite", "rust_workspace"], stdin=log)
    assert result.returncode == 0
    entries = json.loads(result.stdout)
    names = sorted(e["test_id"] for e in entries)
    assert names == ["eval::tests::dispatch::handler_missing", "lattice::tests::join_law"]
    assert all(e["test_id_kind"] == "rust" for e in entries)


def test_rust_failures_nextest_without_progress_index(run_ptj):
    # Some nextest output modes omit the `(n/total)` progress index.
    log = "        FAIL [   0.038s] oric eval::tests::handler_missing\n"
    result = run_ptj(["--rust-failures", "--suite", "rust_workspace"], stdin=log)
    entries = json.loads(result.stdout)
    assert [e["test_id"] for e in entries] == ["eval::tests::handler_missing"]


def test_rust_failures_mixed_cargo_and_nextest_dedup(run_ptj):
    # Both formats present (e.g. nextest streamed + a cargo-style replay); the
    # same test id is emitted at most once.
    log = (
        "        FAIL [   0.038s] (1/1) oric eval::tests::dup\n"
        "test eval::tests::dup ... FAILED\n"
    )
    result = run_ptj(["--rust-failures", "--suite", "rust_workspace"], stdin=log)
    entries = json.loads(result.stdout)
    assert [e["test_id"] for e in entries] == ["eval::tests::dup"]


def test_rust_failures_multi_failure_panic_attributed_to_correct_test(run_ptj):
    # REGRESSION: a panic in ONE test's thread must not misclassify a
    # DIFFERENT failing test in the same log as a panic too (a whole-log
    # substring check would blanket-apply "panic" to every failure).
    log = (
        "test alpha::panics ... FAILED\n"
        "test beta::assertion_only ... FAILED\n"
        "\n"
        "failures:\n"
        "\n"
        "---- alpha::panics stdout ----\n"
        "thread 'alpha::panics' panicked at src/lib.rs:5:5:\n"
        "boom\n"
        "\n"
        "---- beta::assertion_only stdout ----\n"
        "assertion `left == right` failed\n"
        "  left: 1\n"
        " right: 2\n"
    )
    result = run_ptj(
        ["--rust-failures", "--suite", "rust_workspace", "--failure-kind", "assertion_failure"],
        stdin=log,
    )
    entries = json.loads(result.stdout)
    by_name = {e["test_id"]: e["failure_kind"] for e in entries}
    assert by_name["alpha::panics"] == "panic"
    assert by_name["beta::assertion_only"] == "assertion_failure"


def test_rust_failures_nextest_passing_run_is_empty(run_ptj):
    # NEGATIVE PIN: a clean nextest run (only PASS lines) yields no failures.
    log = (
        "        PASS [   0.004s] (1/2) oric foo::ok\n"
        "        PASS [   0.005s] (2/2) oric bar::ok\n"
        "     Summary [   0.090s] 2 tests run: 2 passed, 0 skipped\n"
    )
    result = run_ptj(["--rust-failures", "--suite", "rust_workspace"], stdin=log)
    assert json.loads(result.stdout) == []
