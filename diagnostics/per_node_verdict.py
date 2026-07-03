#!/usr/bin/env python3
"""Build the per-node LLVM dual-exec parity + leak verdict block.

Consumes the assembled `failures[]` (each carrying `source_path` +
`leak_positive` from `parse_test_json.py:_failure_entries`) plus the
`path_to_node` index and produces a `per_node` block keyed by owning node:

  per_node: {
    <node_id>: {
      "interp_pass": bool,
      "llvm_pass": bool,
      "parity": true | false | "unknown",
      "leak_free": true | false | "unknown"
    }
  }

The completion gate reader fail-closes on red parity, leak-positive failures,
or either-leg-unattributed failures. The shape below matches that reader.

Semantics:
  - `llvm_pass` is False when ANY `ori_llvm`-suite failure attributes to the
    node; `interp_pass` False when any `ori_interp` failure attributes to it.
  - `parity` tri-state: `false` when the node's `llvm_pass` is False
    (dual-exec divergence — the LLVM leg is red); `true` when both legs are
    green for the node; `"unknown"` when the node has a failure whose
    `source_path` was empty/unattributable (fail-closed).
  - `leak_free`: `false` when any failure attributing to the node has
    `leak_positive: True`; `true` when the node has failures but none
    leak-positive AND all attributed; `"unknown"` when an attributing failure
    had no source signal (fail-closed). A node with ZERO failures is parity
    true + leak_free true (green) — but zero-failure nodes do not appear in the
    block (only nodes with attributed failures + the `"unknown"` sentinel do;
    an absent node is green by the consumer's reading).
  - An unattributable failure (empty `source_path`, or a `source_path` that
    `path_to_node` maps to None) contributes to a sentinel `"unknown"` node
    bucket with `parity="unknown"` AND `leak_free="unknown"` — NEVER silently
    green, NEVER dropped.

stdlib only; deterministic (sorted node keys).
"""

import argparse
import json
import sys

__all__ = ["build_per_node_verdict", "UNKNOWN_NODE"]

# Sentinel bucket for failures whose source could not be attributed to a node.
UNKNOWN_NODE = "unknown"

# Suite ids whose failures count against the LLVM leg vs the interpreter leg.
_LLVM_SUITES = {"ori_llvm"}
_INTERP_SUITES = {"ori_interp"}


def _lazy_default_index():
    """Return the default `path_to_node` callable, imported lazily so the
    module stays importable (and unit-testable with an injected `index_fn`)
    even if the repo plan tree is absent at import time."""
    from path_to_node_index import path_to_node  # local import keeps top-level light
    return path_to_node


def build_per_node_verdict(failures, index_fn=None):
    """Build the `per_node` verdict block from `failures[]`.

    `failures` is the assembled list of failure objects (each a dict carrying
    at least `suite`, `source_path`, `leak_positive`). `index_fn` maps a
    `.ori` path to its owning node id or None; defaults to
    `path_to_node_index.path_to_node`.

    Returns a dict `{ node_id: { interp_pass, llvm_pass, parity, leak_free } }`
    with deterministic (sorted) key order. Nodes with zero attributed failures
    do NOT appear (the consumer reads an absent node as green); the
    `"unknown"` sentinel appears iff >=1 failure was unattributable."""
    if index_fn is None:
        index_fn = _lazy_default_index()

    # Per-node accumulators. A node only materializes here once a failure
    # attributes (or fails to attribute) to it.
    # node -> {"interp_fail": bool, "llvm_fail": bool, "leak": bool,
    #          "unattributed": bool}
    acc = {}

    def _ensure(node):
        if node not in acc:
            acc[node] = {
                "interp_fail": False,
                "llvm_fail": False,
                "leak": False,
                "unattributed": False,
            }
        return acc[node]

    for failure in failures:
        suite = failure.get("suite", "")
        source_path = failure.get("source_path", "")
        leak_positive = bool(failure.get("leak_positive", False))

        node = None
        unattributed = False
        if not source_path:
            unattributed = True
        else:
            node = index_fn(source_path)
            if node is None:
                unattributed = True

        target = UNKNOWN_NODE if unattributed else node
        entry = _ensure(target)

        if unattributed:
            entry["unattributed"] = True
        if suite in _LLVM_SUITES:
            entry["llvm_fail"] = True
        elif suite in _INTERP_SUITES:
            entry["interp_fail"] = True
        # A non-spec suite (rust/aot/etc) failure that still attributes to a
        # node marks neither leg directly but, when leak-positive, drives
        # leak_free; when unattributed, drives the unknown sentinel.
        if leak_positive:
            entry["leak"] = True

    per_node = {}
    for node in sorted(acc):
        entry = acc[node]
        interp_pass = not entry["interp_fail"]
        llvm_pass = not entry["llvm_fail"]

        if node == UNKNOWN_NODE or entry["unattributed"]:
            # Either-leg-unattributed -> fail-closed on BOTH verdicts.
            parity = "unknown"
            leak_free = "unknown"
        else:
            # parity false when the LLVM leg is red (dual-exec divergence);
            # true when both legs green for the node.
            parity = False if not llvm_pass else True
            leak_free = False if entry["leak"] else True

        per_node[node] = {
            "interp_pass": interp_pass,
            "llvm_pass": llvm_pass,
            "parity": parity,
            "leak_free": leak_free,
        }
    return per_node


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--from-failures",
                        help="file holding a JSON array of failure objects "
                             "(default: read the array from stdin)")
    args = parser.parse_args(argv)

    try:
        if args.from_failures:
            with open(args.from_failures, encoding="utf-8") as fh:
                raw = fh.read()
        else:
            raw = sys.stdin.read()
    except OSError as exc:
        print(f"per_node_verdict: cannot read failures: {exc}", file=sys.stderr)
        return 3

    raw = raw.strip()
    if not raw:
        failures = []
    else:
        try:
            failures = json.loads(raw)
        except json.JSONDecodeError as exc:
            print(f"per_node_verdict: invalid failures JSON: {exc}", file=sys.stderr)
            return 3
    if not isinstance(failures, list):
        print("per_node_verdict: expected a JSON array of failure objects",
              file=sys.stderr)
        return 3

    per_node = build_per_node_verdict(failures)
    print(json.dumps(per_node, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
