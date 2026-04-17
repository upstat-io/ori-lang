#!/usr/bin/env bash
# verify-plan-bug-degraded.sh — verify that all ori_lang workflows continue
# functioning when the lang-intelligence Neo4j container is stopped.
#
# Directly satisfies the §06.3 mission invariant from
# plans/plan-bug-dag-ingestion/00-overview.md: "Graceful degradation
# preserved — when `docker stop lang-intelligence` is active, every
# downstream workflow continues working."
#
# Test design note — baseline comparison for test-all.sh:
# A naive "test-all.sh exits 0 with graph down" check conflates graph
# downtime with pre-existing compiler regressions. We instead capture a
# baseline exit code with graph UP and assert the exit code is IDENTICAL
# with graph DOWN. Equal-fail is a PASS for this test (graph isn't causing
# the failure). Baseline-0 with degraded-nonzero is the real regression.
#
# Usage:
#   verify-plan-bug-degraded.sh [--skip-test-all] [--skip-docker] [--help]
#     --skip-test-all  skip the test-all.sh baseline+degraded runs (fast mode)
#     --skip-docker    skip the docker stop/start cycle (for env without docker)
#
# Exit: 0 = all workflows continue; 1 = at least one graph-down regression.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SKIP_TEST_ALL=0
SKIP_DOCKER=0

for arg in "$@"; do
    case "$arg" in
        --help|-h)
            sed -n '3,22p' "$0" | sed 's/^# \?//'
            exit 0
            ;;
        --skip-test-all) SKIP_TEST_ALL=1 ;;
        --skip-docker)   SKIP_DOCKER=1 ;;
        *)
            echo "ERROR: unknown argument: $arg" >&2
            echo "Run with --help for usage." >&2
            exit 2
            ;;
    esac
done

PASS=0; FAIL=0
log() { echo "[$(date '+%H:%M:%S')] $*"; }
ok()   { log "PASS: $*"; PASS=$((PASS + 1)); }
fail() { log "FAIL: $*"; FAIL=$((FAIL + 1)); }

# ── Phase 0: capture baseline (graph UP) for exit-code comparison ────
BASELINE_EXIT="unset"
if [[ $SKIP_TEST_ALL -eq 0 ]]; then
    log "Capturing test-all.sh baseline with graph UP..."
    set +e
    timeout 150 bash "$REPO_ROOT/test-all.sh" >/dev/null 2>&1
    BASELINE_EXIT=$?
    set -e
    log "  baseline exit code: $BASELINE_EXIT"
fi

# ── Phase 1: bring graph DOWN ────────────────────────────────────────
if [[ $SKIP_DOCKER -eq 0 ]]; then
    log "Stopping lang-intelligence container..."
    if command -v docker >/dev/null 2>&1; then
        docker stop lang-intelligence 2>/dev/null || \
            log "  (container was already stopped or not found)"
    else
        log "  (docker CLI not available; skipping stop)"
    fi
fi

# ── Phase 2: run workflows that must be unaffected ───────────────────

# 2a: test-all.sh exit code unchanged vs baseline
if [[ $SKIP_TEST_ALL -eq 0 ]]; then
    log "Running ./test-all.sh with graph down (timeout 150s)..."
    set +e
    timeout 150 bash "$REPO_ROOT/test-all.sh" >/dev/null 2>&1
    DEGRADED_EXIT=$?
    set -e
    log "  degraded exit code: $DEGRADED_EXIT"
    if [[ "$BASELINE_EXIT" == "$DEGRADED_EXIT" ]]; then
        if [[ $BASELINE_EXIT -eq 0 ]]; then
            ok "test-all.sh passes with graph down (baseline=0, degraded=0)"
        else
            ok "test-all.sh exit code unchanged vs baseline ($BASELINE_EXIT) — graph is not the cause of the pre-existing failure"
        fi
    else
        fail "test-all.sh degraded in a NEW way: baseline=$BASELINE_EXIT degraded=$DEGRADED_EXIT — §03 regression"
    fi
fi

# 2b: intel-query.sh status with graph down → unavailable, still exits 0
log "Checking intel-query.sh status with graph down..."
set +e
STATUS_OUT=$(bash "$REPO_ROOT/scripts/intel-query.sh" status 2>&1)
STATUS_EXIT=$?
set -e
if [[ $STATUS_EXIT -eq 0 ]] && echo "$STATUS_OUT" | grep -q '"status"'; then
    if echo "$STATUS_OUT" | grep -q '"ok"'; then
        # Graph actually still up (--skip-docker mode or restart race) — acceptable
        ok "intel-query.sh status returns structured JSON (graph still reachable)"
    else
        ok "intel-query.sh status returns structured JSON with unavailable marker"
    fi
else
    fail "intel-query.sh status: exit=$STATUS_EXIT output=$STATUS_OUT"
fi

# 2c: plan_corpus check must pass (no Neo4j dependency)
log "Running python -m scripts.plan_corpus check..."
if ( cd "$REPO_ROOT" && python3 -m scripts.plan_corpus check plans/plan-bug-dag-ingestion/ >/dev/null 2>&1 ); then
    ok "plan_corpus check passes with graph down"
else
    fail "plan_corpus check failed with graph down (unexpected Neo4j dependency)"
fi

# 2d: git commit smoke — post-commit hook must not block even if graph down
log "Testing git commit smoke (empty commit, hook non-blocking)..."
set +e
( cd "$REPO_ROOT" && git commit -m "test(ci): degradation smoke" --allow-empty >/dev/null 2>&1 )
COMMIT_EXIT=$?
if [[ $COMMIT_EXIT -eq 0 ]]; then
    # Undo the smoke commit so we don't leave it in history
    ( cd "$REPO_ROOT" && git reset --soft HEAD~1 >/dev/null 2>&1 )
    ok "git commit succeeds with graph down (hook non-blocking)"
else
    fail "git commit failed with graph down — post-commit hook is blocking (§03 regression)"
fi
set -e

# ── Phase 3: bring graph back UP ─────────────────────────────────────
if [[ $SKIP_DOCKER -eq 0 ]]; then
    log "Starting lang-intelligence container..."
    if command -v docker >/dev/null 2>&1; then
        docker start lang-intelligence >/dev/null 2>&1 || \
            log "  (container not found — skipping restart)"
    fi

    RESTART_LATENCY=0
    for i in $(seq 1 30); do
        STATUS_UP=$(bash "$REPO_ROOT/scripts/intel-query.sh" status 2>/dev/null || true)
        if echo "$STATUS_UP" | grep -q '"status":"ok"'; then
            RESTART_LATENCY=$i
            break
        fi
        sleep 1
    done
    if [[ $RESTART_LATENCY -gt 0 ]]; then
        ok "lang-intelligence healthy after ${RESTART_LATENCY}s restart"
    else
        fail "lang-intelligence did not report healthy within 30s after restart"
    fi
fi

# ── Summary ──────────────────────────────────────────────────────────
echo ""
echo "============================="
echo "Degradation test: PASS=$PASS FAIL=$FAIL"
echo "Baseline exit (graph up):  $BASELINE_EXIT"
echo "============================="
[[ $FAIL -eq 0 ]] || exit 1
exit 0
