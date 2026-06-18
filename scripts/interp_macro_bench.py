#!/usr/bin/env python3
"""Interpreter macro wall-time tier: `ori run` vs `python3` via hyperfine.

The macro acceptance oracle for the interpreter perf grind: Ori `ori run` must stay
within Python3 * 1.5 (match-or-beat the 1.5x bar). Wall-time is noisy, so this is the
weekly/on-demand tier, NEVER the per-commit regression gate (that tier is deterministic
callgrind instruction count -- see interp_callgrind_gate.py).

For each program in the corpus (matched `<name>.ori` + `<name>.py`), runs hyperfine on
both, takes the medians, applies the match-or-beat gate, checks value parity (ori exit
code == python printed value mod 256), and appends a per-program wall-time row to the
trend log.

Subcommands:
  run [--ori PATH] [--corpus DIR] [--programs a,b] [--runs N] [--tolerance T]
      [--trend-log PATH] [--no-trend]
      Bench each program; exit 1 if any program loses the gate or fails value parity.
  self-test
      Exercise the pure gate + parity logic with synthetic data (no hyperfine).

Requires: hyperfine, python3, and a release `ori` binary (default target/release/ori).
"""

import argparse
import datetime
import json
import os
import subprocess
import sys
import tempfile

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT_ORI = os.path.join(REPO_ROOT, "target/release/ori")
DEFAULT_CORPUS = os.path.join(REPO_ROOT, "tests/benchmarks/macro")
DEFAULT_TREND_LOG = os.path.join(REPO_ROOT, "tests/benchmarks/macro/trend.jsonl")
# Ori `ori run` must stay within Python3 * (1 + tolerance). Macro tier only.
INTERP_MACRO_TOLERANCE = 0.5
DEFAULT_RUNS = 10


def gate(ori_ms, py_ms, tolerance):
    """Pure match-or-beat verdict: ori_ms <= py_ms * (1 + tolerance)."""
    if ori_ms <= 0 or py_ms <= 0:
        raise ValueError("medians must be positive (ms)")
    if tolerance < 0:
        raise ValueError("tolerance must be non-negative")
    threshold = py_ms * (1.0 + tolerance)
    passed = ori_ms <= threshold
    return {
        "verdict": "match-or-beat" if passed else "loss",
        "passed": passed,
        "ori_median_ms": float(ori_ms),
        "python_median_ms": float(py_ms),
        "ratio": float(ori_ms) / float(py_ms),
        "tolerance": float(tolerance),
        "threshold_ms": float(threshold),
    }


def _hyperfine_median_ms(cmd, runs):
    """Run hyperfine on `cmd`; return the median wall time in ms.

    `--ignore-failure`: `ori run` exits with main's int return value (a non-zero
    "success" value), which hyperfine would otherwise treat as a command failure.
    Real crashes/miscompiles are caught by the separate value-parity guard, not here.
    """
    with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as tf:
        out = tf.name
    try:
        subprocess.run(
            ["hyperfine", "--ignore-failure", "--warmup", "2", "--runs", str(runs),
             "--export-json", out, cmd],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        with open(out, encoding="utf-8") as fh:
            data = json.load(fh)
        return data["results"][0]["median"] * 1000.0
    finally:
        if os.path.exists(out):
            os.unlink(out)


def _ori_value(ori, ori_file):
    """Run `ori run` once; return its exit code (main's int return value mod 256)."""
    r = subprocess.run([ori, "run", ori_file], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return r.returncode


def _python_value(py_file):
    """Run python3 once; return the printed int value (or None)."""
    r = subprocess.run([sys.executable, py_file], capture_output=True, text=True)
    try:
        return int(r.stdout.strip())
    except (ValueError, AttributeError):
        return None


def _discover_programs(corpus, programs):
    if programs:
        names = programs.split(",")
    else:
        names = sorted(
            os.path.splitext(f)[0]
            for f in os.listdir(corpus)
            if f.endswith(".ori")
        )
    pairs = []
    for n in names:
        ori_file = os.path.join(corpus, f"{n}.ori")
        py_file = os.path.join(corpus, f"{n}.py")
        if os.path.exists(ori_file) and os.path.exists(py_file):
            pairs.append((n, ori_file, py_file))
        else:
            print(f"SKIP {n}: missing .ori/.py pair", file=sys.stderr)
    return pairs


def cmd_run(args):
    if not os.access(args.ori, os.X_OK):
        print(f"ERROR: ori binary not found/executable: {args.ori} (build with cargo build --release -p oric)")
        return 2
    pairs = _discover_programs(args.corpus, args.programs)
    if not pairs:
        print("ERROR: no .ori/.py program pairs found in corpus")
        return 2

    stamp = datetime.datetime.now(datetime.timezone.utc).isoformat()
    rows = []
    all_ok = True
    for name, ori_file, py_file in pairs:
        # Value parity guard: a miscompile must not silently pass the perf gate.
        ori_rc = _ori_value(args.ori, ori_file)
        py_val = _python_value(py_file)
        parity_ok = py_val is not None and ori_rc == (py_val % 256)

        ori_ms = _hyperfine_median_ms(f"{args.ori} run {ori_file}", args.runs)
        py_ms = _hyperfine_median_ms(f"{sys.executable} {py_file}", args.runs)
        g = gate(ori_ms, py_ms, args.tolerance)
        row = {
            "captured_at": stamp,
            "program": name,
            "ori_median_ms": round(ori_ms, 3),
            "python_median_ms": round(py_ms, 3),
            "ratio": round(g["ratio"], 4),
            "threshold_ms": round(g["threshold_ms"], 3),
            "verdict": g["verdict"],
            "parity_ok": parity_ok,
        }
        rows.append(row)
        status = g["verdict"].upper() if parity_ok else "PARITY-FAIL"
        print(
            f"{name}: ori {ori_ms:.1f}ms vs python {py_ms:.1f}ms "
            f"(ratio {g['ratio']:.2f}x, bar {1 + args.tolerance:.1f}x) -> {status}"
        )
        if not g["passed"] or not parity_ok:
            all_ok = False

    if not args.no_trend:
        os.makedirs(os.path.dirname(args.trend_log), exist_ok=True)
        with open(args.trend_log, "a", encoding="utf-8") as fh:
            for row in rows:
                fh.write(json.dumps(row, sort_keys=True) + "\n")
        print(f"trend: appended {len(rows)} rows -> {args.trend_log}")

    print("OK" if all_ok else "GATE FAILED")
    return 0 if all_ok else 1


def cmd_self_test(_args):
    # match-or-beat: ori within 1.5x of python -> pass.
    assert gate(140.0, 100.0, 0.5)["passed"], "1.4x must pass at 1.5x bar"
    assert not gate(160.0, 100.0, 0.5)["passed"], "1.6x must fail at 1.5x bar"
    assert gate(100.0, 100.0, 0.5)["passed"], "1.0x must pass"
    assert gate(50.0, 100.0, 0.5)["passed"], "0.5x (beat) must pass"
    try:
        gate(0, 100, 0.5)
        print("SELF-TEST FAIL: non-positive median did not raise")
        return 1
    except ValueError:
        pass
    print("self-test PASSED")
    return 0


def main():
    p = argparse.ArgumentParser(description="Interpreter macro wall-time tier (ori run vs python3)")
    sub = p.add_subparsers(dest="cmd", required=True)

    pr = sub.add_parser("run", help="bench the corpus and gate Ori <= Python * 1.5")
    pr.add_argument("--ori", default=DEFAULT_ORI)
    pr.add_argument("--corpus", default=DEFAULT_CORPUS)
    pr.add_argument("--programs", default=None, help="comma-separated subset")
    pr.add_argument("--runs", type=int, default=DEFAULT_RUNS)
    pr.add_argument("--tolerance", type=float, default=INTERP_MACRO_TOLERANCE)
    pr.add_argument("--trend-log", default=DEFAULT_TREND_LOG)
    pr.add_argument("--no-trend", action="store_true")
    pr.set_defaults(fn=cmd_run)

    ps = sub.add_parser("self-test", help="exercise the pure gate logic")
    ps.set_defaults(fn=cmd_self_test)

    args = p.parse_args()
    return args.fn(args)


if __name__ == "__main__":
    sys.exit(main())
