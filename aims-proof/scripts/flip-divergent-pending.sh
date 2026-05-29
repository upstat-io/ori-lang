#!/usr/bin/env bash
#
# flip-divergent-pending.sh — §17 locality shipped-conformance flip protocol.
#
# Per the locality shipped-conformance gate
# SC6: separate top-level script mirroring §10's counterexample-search
# pattern. Re-runs run-section-08-proofs.sh --locality-section, asserts
# every manifest rule emits `pass`, exits non-zero if any still emits
# `divergent-pending` under --as-flip-trigger mode. Consumers (§12
# annotation-clearer, §15 nightly gate, §16 reframer) cite this script as the
# flip trigger.
#
# Emits a structured JSON envelope to stdout per outcome:
# {verdict, exit_reason, autopilot_routable, rule_verdicts[], probe_tuple}
#
# DUAL INVOCATION MODES (per §17 SC6):
# --as-conformance-probe (DEFAULT) — periodic conformance check;
# `divergent_pending` is non-fatal (exit 0,
# expected steady-state under Lazy-Migration Policy)
# --as-flip-trigger — post-locality-migration assertion; `divergent_pending`
# is FATAL (exit code 2 + locality_conformance_flip_failed
# emitted to stderr) so the orchestrator surfaces the
# failed flip rather than silently treating it as success
#
# Exit codes by mode:
# --as-conformance-probe:
# 0 — `pass` (every manifest rule flipped) — §12/§15/§16 close-outs unblock
# 0 — `divergent_pending` (locality work has not landed; flip pending) — non-fatal
# 2 — `fatal_divergent` (manifest-free divergence) — routes to /fix-bug
# 3 — `probe_compile_failure` (build.rs cannot parse shipped surface)
# 1 — `manifest_invalid` (manifest fails schema validation)
# --as-flip-trigger:
# 0 — `pass` (every manifest rule flipped) — flip succeeded
# 2 — `divergent_pending` (flip FAILED — expected pass post-locality-migration)
# 2 — `fatal_divergent` (manifest-free divergence) — routes to /fix-bug
# 3 — `probe_compile_failure` (build.rs cannot parse shipped surface)
# 1 — `manifest_invalid` (manifest fails schema validation)
#
# BANNED per §17 SC6: AskUserQuestion in this path; tool-role per
# the non-interactive runner contract.
#
# Cwd contract: anchors to aims-proof/ via cd "$(dirname "$0")/.." at entry.

set -e
cd "$(dirname "$0")/.."

mkdir -p test-results

readonly RESULT_PATH="test-results/section-17-locality-result.json"
readonly FLIP_LOG="test-results/section-17-flip-protocol.log"

# Flag parsing — dual invocation modes per §17 SC6.
mode="as-conformance-probe"
for arg in "$@"; do
    case "$arg" in
        --as-conformance-probe)
            mode="as-conformance-probe"
            ;;
        --as-flip-trigger)
            mode="as-flip-trigger"
            ;;
        *)
            echo "flip-divergent-pending.sh: unknown argument: $arg" >&2
            echo "usage: flip-divergent-pending.sh [--as-conformance-probe | --as-flip-trigger]" >&2
            exit 1
            ;;
    esac
done

# Re-run the locality-section cross-walk.
set +e
bash scripts/run-section-08-proofs.sh --locality-section > "$FLIP_LOG" 2>&1
cross_walk_exit=$?
set -e

# The cross-walk wrote $RESULT_PATH; emit it verbatim to stdout (structured
# JSON envelope per §17 SC6). Consumers (§12/§15/§16) read this envelope.
if [[ -f "$RESULT_PATH" ]]; then
    cat "$RESULT_PATH"
else
    cat <<EOF
{"verdict": "probe_compile_failure", "exit_reason": "locality_conformance_probe_compile_failure", "autopilot_routable": false, "phase": "flip_protocol", "reason": "cross-walk did not produce $RESULT_PATH; see $FLIP_LOG"}
EOF
    exit 3
fi

# Mode-specific exit semantics. In --as-flip-trigger mode, a divergent_pending
# verdict is FATAL (exit 2 + locality_conformance_flip_failed to stderr) — the
# post-locality-migration ceremony expects `pass` and any other verdict signals
# the flip failed. In --as-conformance-probe (default) mode the cross-walk's
# own exit code is passed through verbatim (divergent_pending stays exit 0).
if [[ "$mode" == "as-flip-trigger" ]]; then
    verdict=$(python3 -c "import json,sys; print(json.load(open('$RESULT_PATH')).get('verdict', ''))" 2>/dev/null || echo "")
    if [[ "$verdict" == "divergent_pending" ]]; then
        echo "locality_conformance_flip_failed: --as-flip-trigger expected verdict 'pass' but got 'divergent_pending'; locality work has not landed or did not flip every manifest rule" >&2
        exit 2
    fi
fi

# Pass through the cross-walk's exit code; §17 SC6 verdict classes already
# encode the routing decision (pass / divergent_pending = exit 0;
# fatal_divergent = exit 2; manifest_invalid = exit 1; probe_compile_failure
# = exit 3).
exit "$cross_walk_exit"
