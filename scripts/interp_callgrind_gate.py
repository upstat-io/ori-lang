#!/usr/bin/env python3
"""Interpreter micro-benchmark callgrind instruction-count regression gate.

Captures deterministic per-kernel instruction counts (Ir) for the
`compiler/oric/benches/interpreter.rs` single-shot mode under callgrind, freezes
them to a baseline JSON (the regression SSOT), and gates a current capture against
that baseline. Instruction count is deterministic run-to-run (unlike wall time), so
this is the per-commit regression tier; the hyperfine wall-time tier is macro-only.

Subcommands:
  capture --out <json> [--iters N] [--kernels a,b,...]
      Run each kernel under callgrind, write {kernel: ir, ...} + metadata to <json>.
  check --baseline <json> [--iters N] [--threshold T] [--kernels a,b,...]
      Re-capture and compare; exit 1 if any kernel's Ir exceeds baseline*(1+T).
  self-test
      Run the negative pin (a pessimized kernel trips the gate) and the positive pin
      (a known-good kernel passes). Exercises compare_readings without callgrind.

Requires: valgrind (callgrind). Builds the bench binary via `cargo bench --no-run`.
"""

import argparse
import datetime
import glob
import json
import os
import subprocess
import sys
import tempfile

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# Mirror of the KERNELS slice in compiler/oric/benches/interpreter.rs.
KERNELS = [
    "arithmetic_loop",
    "function_call_loop",
    "method_call_loop",
    "for_yield",
    "list_index_assign",
    "list_append_pop",
    "map_insert_lookup",
    "string_concat",
    "closure_capture",
    "pattern_match_dispatch",
    "nested_collections",
]

DEFAULT_ITERS = 20
DEFAULT_THRESHOLD = 0.03  # +3% Ir trips the gate


def _build_bench_binary():
    """Build the interpreter bench binary; return its path."""
    subprocess.run(
        ["cargo", "bench", "-p", "oric", "--bench", "interpreter", "--no-run"],
        cwd=REPO_ROOT,
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    cands = [
        p
        for p in glob.glob(os.path.join(REPO_ROOT, "target/release/deps/interpreter-*"))
        if not p.endswith(".d") and os.access(p, os.X_OK)
    ]
    if not cands:
        raise RuntimeError("interpreter bench binary not found after build")
    return max(cands, key=os.path.getmtime)


def _callgrind_ir(binary, kernel, iters):
    """Run one kernel single-shot under callgrind; return its total Ir (int)."""
    with tempfile.NamedTemporaryFile(suffix=".callgrind", delete=False) as tf:
        out_file = tf.name
    try:
        env = dict(os.environ)
        env["ORI_INTERP_CALLGRIND_KERNEL"] = kernel
        env["ORI_INTERP_CALLGRIND_ITERS"] = str(iters)
        subprocess.run(
            [
                "valgrind",
                "--tool=callgrind",
                f"--callgrind-out-file={out_file}",
                "--cache-sim=no",
                "--branch-sim=no",
                binary,
            ],
            env=env,
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        with open(out_file, encoding="utf-8") as fh:
            for line in fh:
                if line.startswith("summary:"):
                    return int(line.split()[1])
        raise RuntimeError(f"no summary line in callgrind output for {kernel}")
    finally:
        if os.path.exists(out_file):
            os.unlink(out_file)


def capture(kernels, iters):
    """Capture per-kernel Ir; return the baseline dict."""
    binary = _build_bench_binary()
    readings = {}
    for k in kernels:
        ir = _callgrind_ir(binary, k, iters)
        readings[k] = ir
        print(f"  {k}: {ir} Ir", file=sys.stderr)
    return {
        "tool": "callgrind",
        "metric": "Ir",
        "iters": iters,
        "captured_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "kernels": readings,
    }


def compare_readings(baseline_kernels, current_kernels, threshold):
    """Pure comparison: return (ok, regressions, missing).

    regressions: list of (kernel, baseline_ir, current_ir, pct) over threshold.
    missing: kernels in baseline absent from current (treated as a gate failure).
    """
    regressions = []
    missing = []
    for k, base_ir in baseline_kernels.items():
        if k not in current_kernels:
            missing.append(k)
            continue
        cur_ir = current_kernels[k]
        if base_ir <= 0:
            continue
        pct = (cur_ir - base_ir) / base_ir
        if pct > threshold:
            regressions.append((k, base_ir, cur_ir, pct))
    ok = not regressions and not missing
    return ok, regressions, missing


def cmd_capture(args):
    kernels = args.kernels.split(",") if args.kernels else KERNELS
    baseline = capture(kernels, args.iters)
    with open(args.out, "w", encoding="utf-8") as fh:
        json.dump(baseline, fh, indent=2, sort_keys=True)
        fh.write("\n")
    print(f"captured {len(kernels)} kernels -> {args.out}")
    return 0


def cmd_check(args):
    with open(args.baseline, encoding="utf-8") as fh:
        baseline = json.load(fh)
    iters = args.iters if args.iters is not None else baseline.get("iters", DEFAULT_ITERS)
    kernels = args.kernels.split(",") if args.kernels else list(baseline["kernels"].keys())
    current = capture(kernels, iters)
    # When checking an explicit subset, compare ONLY that subset against the baseline;
    # the missing-kernel guard applies to the requested set, not the full baseline.
    baseline_subset = {k: v for k, v in baseline["kernels"].items() if k in kernels}
    ok, regressions, missing = compare_readings(
        baseline_subset, current["kernels"], args.threshold
    )
    for k, b, c, pct in regressions:
        print(f"REGRESSION {k}: {b} -> {c} Ir (+{pct * 100:.2f}% > {args.threshold * 100:.1f}%)")
    for k in missing:
        print(f"MISSING {k}: present in baseline, absent in current capture")
    if ok:
        print(f"OK: all {len(kernels)} kernels within +{args.threshold * 100:.1f}% of baseline")
        return 0
    return 1


def cmd_self_test(_args):
    """Negative pin: a pessimized kernel trips the gate. Positive pin: known-good passes."""
    baseline = {"arithmetic_loop": 1_000_000, "function_call_loop": 5_000_000}

    # Negative pin: arithmetic_loop pessimized to +50% -> MUST trip.
    pessimized = {"arithmetic_loop": 1_500_000, "function_call_loop": 5_000_000}
    neg_ok, neg_regs, _ = compare_readings(baseline, pessimized, DEFAULT_THRESHOLD)
    if neg_ok or not neg_regs:
        print("SELF-TEST FAIL: negative pin did NOT trip on a +50% pessimized kernel")
        return 1
    print(f"negative pin OK: gate tripped on pessimized kernel ({neg_regs[0][0]} +50%)")

    # Positive pin: known-good (identical + a benign -1% improvement) -> MUST pass.
    known_good = {"arithmetic_loop": 1_000_000, "function_call_loop": 4_950_000}
    pos_ok, pos_regs, pos_missing = compare_readings(baseline, known_good, DEFAULT_THRESHOLD)
    if not pos_ok:
        print(f"SELF-TEST FAIL: positive pin tripped on known-good ({pos_regs or pos_missing})")
        return 1
    print("positive pin OK: gate passed on known-good (within threshold + an improvement)")

    # Missing-kernel guard: an absent kernel is a gate failure.
    miss_ok, _, miss = compare_readings(baseline, {"arithmetic_loop": 1_000_000}, DEFAULT_THRESHOLD)
    if miss_ok or "function_call_loop" not in miss:
        print("SELF-TEST FAIL: missing-kernel guard did not flag an absent kernel")
        return 1
    print("missing-kernel guard OK: absent kernel flagged")
    print("self-test PASSED")
    return 0


def main():
    p = argparse.ArgumentParser(description="Interpreter callgrind instruction-count regression gate")
    sub = p.add_subparsers(dest="cmd", required=True)

    pc = sub.add_parser("capture", help="capture per-kernel Ir to a baseline JSON")
    pc.add_argument("--out", required=True)
    pc.add_argument("--iters", type=int, default=DEFAULT_ITERS)
    pc.add_argument("--kernels", default=None, help="comma-separated subset")
    pc.set_defaults(fn=cmd_capture)

    pk = sub.add_parser("check", help="check a current capture against a baseline")
    pk.add_argument("--baseline", required=True)
    pk.add_argument("--iters", type=int, default=None)
    pk.add_argument("--threshold", type=float, default=DEFAULT_THRESHOLD)
    pk.add_argument("--kernels", default=None, help="comma-separated subset")
    pk.set_defaults(fn=cmd_check)

    ps = sub.add_parser("self-test", help="run the negative + positive gate pins")
    ps.set_defaults(fn=cmd_self_test)

    args = p.parse_args()
    return args.fn(args)


if __name__ == "__main__":
    sys.exit(main())
