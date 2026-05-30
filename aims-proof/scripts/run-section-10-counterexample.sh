#!/usr/bin/env bash
#
# run-section-10-counterexample.sh — §10 counterexample-search gate.
#
# Per the counterexample search §10.1
# checkbox 9 + §10.2 close-out items 2/3/4/6/7/10. Drives the SMT solver
# across every encoded shape in aims-proof/proofs/10-counterexample/ +
# the two encoder-validation fixtures. Mirrors §09 convention per
# aims-proof/scripts/run-section-09-proofs.sh:1 SSOT.
#
# Initial per-shape solver timeout: 300s wall-clock (default; overridable
# via --initial-timeout-seconds=<N>). Bounded-retry on TIMEOUT: 2× the
# prior budget per retry, max 3 retries (attempts at 300s / 600s / 1200s
# / 2400s; cap at attempt 4 fires TIMEOUT_EXHAUSTED). Each per-attempt
# wall-clock + retry count recorded in results.json.
#
# Result normalization closed enum:
# {UNSAT, SAT, TIMEOUT, TIMEOUT_EXHAUSTED, ENCODER_INVALID}
#
# Encoder-validation gate: BOTH fixtures
# (fixtures/encoder-validation.smt2 known-SAT + known-UNSAT blocks) run
# FIRST every invocation. Known-SAT non-SAT OR known-UNSAT non-UNSAT →
# ENCODER_INVALID halt; no real-shape verdict trusted.
#
# Exit codes + exit_reasons (per this checker's documented exit-code contract):
# exit 0 + counterexample_search_adequate — all UNSAT, fixtures green
# exit 2 + counterexample_witness_found — SAT, recovery cycle live
# exit 2 + counterexample_proof_gap_unprovable — SAT, unrecoverable
# exit 2 + counterexample_reformulation_required — SAT, recovered via reform
# exit 2 + encoder_invalid_halt — fixture failed
# exit 2 + smt_timeout_inconclusive — TIMEOUT_EXHAUSTED
# exit 3 + counterexample_infrastructure_failed — solver/scaffold/runner fail
#
# Runner the non-interactive runner contract
# - NEVER reads from stdin; NEVER invokes read/select/prompt/any interactive
# shell builtin; NEVER blocks on a terminal.
# - ALL configuration via command-line args OR environment variables.
# - Per-shape solver call respects --initial-timeout-seconds budget; no
# unbounded `wait`.
#
# Intel-query citations (per intelligence.md graph-first protocol; queries
# run 2026-05-28 at section entry):
# - similar "run_section_09" --repo ori → Symbol-not-found (shell-script
# symbols not indexed); fallback: aims-proof/scripts/run-section-09-proofs.sh
# read as SSOT for the conformance/discharge pattern this script mirrors.
# - file-symbols "aims-proof/scripts" --repo ori → 0 symbols (shell scripts
# not indexed); manual read of sibling runners (§02 / §03 / §04 / §05 /
# §06 / §08 / §09) confirms the cwd-anchor + test-results discipline.
#
# Cwd contract: anchors to aims-proof/ via cd "$(dirname "$0")/.." at entry.

set -e
cd "$(dirname "$0")/.."

mkdir -p test-results

# ---------------------------------------------------------------------------
# Configuration (all via CLI args or env vars per autopilot-no-interactive)
# ---------------------------------------------------------------------------

INITIAL_TIMEOUT_SECONDS="${ORI_S10_INITIAL_TIMEOUT_SECONDS:-300}"
MAX_RETRIES="${ORI_S10_MAX_RETRIES:-3}"
SOLVER="${ORI_S10_SOLVER:-}"
DRY_RUN=0
SAT_CLASSIFICATION="${COUNTEREXAMPLE_SAT_CLASSIFICATION:-counterexample_witness_found}"

# Parse CLI args. Only --flag=value or --flag value forms (no interactive
# prompting for missing values).
while [[ $# -gt 0 ]]; do
    case "$1" in
        --initial-timeout-seconds=*)
            INITIAL_TIMEOUT_SECONDS="${1#*=}"
            shift
            ;;
        --initial-timeout-seconds)
            INITIAL_TIMEOUT_SECONDS="$2"
            shift 2
            ;;
        --max-retries=*)
            MAX_RETRIES="${1#*=}"
            shift
            ;;
        --max-retries)
            MAX_RETRIES="$2"
            shift 2
            ;;
        --solver=*)
            SOLVER="${1#*=}"
            shift
            ;;
        --solver)
            SOLVER="$2"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        *)
            cat > test-results/section-10-result.json <<EOF
{"status": "fail", "exit_reason": "counterexample_infrastructure_failed", "phase": "argparse", "reason": "unknown CLI flag: $1"}
EOF
            exit 3
            ;;
    esac
done

# Validate numeric inputs without prompting on parse failure.
if ! [[ "$INITIAL_TIMEOUT_SECONDS" =~ ^[0-9]+$ ]]; then
    cat > test-results/section-10-result.json <<EOF
{"status": "fail", "exit_reason": "counterexample_infrastructure_failed", "phase": "argparse", "reason": "--initial-timeout-seconds must be a positive integer; got: ${INITIAL_TIMEOUT_SECONDS}"}
EOF
    exit 3
fi
if ! [[ "$MAX_RETRIES" =~ ^[0-9]+$ ]]; then
    cat > test-results/section-10-result.json <<EOF
{"status": "fail", "exit_reason": "counterexample_infrastructure_failed", "phase": "argparse", "reason": "--max-retries must be a positive integer; got: ${MAX_RETRIES}"}
EOF
    exit 3
fi

# Validate SAT-classification env var (used when ≥1 row records SAT to
# disambiguate witness_found / proof_gap_unprovable / reformulation_required).
readonly VALID_SAT_CLASSIFICATIONS=(
    "counterexample_witness_found"
    "counterexample_proof_gap_unprovable"
    "counterexample_reformulation_required"
)
sat_classification_valid=0
for valid in "${VALID_SAT_CLASSIFICATIONS[@]}"; do
    if [[ "$SAT_CLASSIFICATION" == "$valid" ]]; then
        sat_classification_valid=1
        break
    fi
done
if [[ "$sat_classification_valid" -ne 1 ]]; then
    cat > test-results/section-10-result.json <<EOF
{"status": "fail", "exit_reason": "counterexample_infrastructure_failed", "phase": "preflight", "reason": "COUNTEREXAMPLE_SAT_CLASSIFICATION='${SAT_CLASSIFICATION}' not in {${VALID_SAT_CLASSIFICATIONS[*]}}"}
EOF
    exit 3
fi

# Solver auto-detection (no prompting): prefer z3, fall back to cvc5.
if [[ -z "$SOLVER" ]]; then
    if command -v z3 > /dev/null 2>&1; then
        SOLVER="z3"
    elif command -v cvc5 > /dev/null 2>&1; then
        SOLVER="cvc5"
    fi
fi

# Fixture inventory (encoder-validation gates; run FIRST).
readonly FIXTURE_FILE="proofs/10-counterexample/fixtures/encoder-validation.smt2"
readonly FIXTURE_METADATA="proofs/10-counterexample/fixtures/encoder-validation.json"

# Real-shape corpus: every .smt2 in proofs/10-counterexample/ EXCLUDING
# fixtures/. Sorted for deterministic ordering.
shape_corpus=()
if [[ -d "proofs/10-counterexample" ]]; then
    while IFS= read -r f; do
        shape_corpus+=("$f")
    done < <(find proofs/10-counterexample -maxdepth 1 -name "*.smt2" -type f | sort)
fi

# Initial results.json schema (empty; runner populates per shape).
results_json="proofs/10-counterexample/results.json"
cat > "$results_json" <<EOF
{"schema_version": 1, "exit_reason": null, "runs": []}
EOF

# ---------------------------------------------------------------------------
# Dry-run early-exit (scaffolding verification; no solver invocation).
# ---------------------------------------------------------------------------

if [[ "$DRY_RUN" -eq 1 ]]; then
    fixture_present="no"
    if [[ -f "$FIXTURE_FILE" ]]; then
        fixture_present="yes"
    fi
    shape_count="${#shape_corpus[@]}"
    cat > test-results/section-10-result.json <<EOF
{"status": "dry-run", "exit_reason": null, "phase": "scaffold-only", "solver": "${SOLVER:-none}", "initial_timeout_seconds": ${INITIAL_TIMEOUT_SECONDS}, "max_retries": ${MAX_RETRIES}, "fixture_present": "${fixture_present}", "shape_count": ${shape_count}, "sat_classification": "${SAT_CLASSIFICATION}"}
EOF
    exit 0
fi

# ---------------------------------------------------------------------------
# Solver presence check (counterexample_infrastructure_failed on absence).
# ---------------------------------------------------------------------------

if [[ -z "$SOLVER" ]]; then
    cat > test-results/section-10-result.json <<EOF
{"status": "fail", "exit_reason": "counterexample_infrastructure_failed", "phase": "preflight", "reason": "no SMT solver found in PATH (tried z3, cvc5); set --solver=<path> or install z3/cvc5"}
EOF
    exit 3
fi

if ! command -v "$SOLVER" > /dev/null 2>&1; then
    cat > test-results/section-10-result.json <<EOF
{"status": "fail", "exit_reason": "counterexample_infrastructure_failed", "phase": "preflight", "reason": "solver '${SOLVER}' not executable or not in PATH"}
EOF
    exit 3
fi

# ---------------------------------------------------------------------------
# Helper: run-solver-with-bounded-retry
#
# Args: $1 = smt2 path, $2 = shape_id
# Emits to stdout one line: "result|retry_count|attempt_budgets_csv"
# result ∈ {UNSAT, SAT, TIMEOUT_EXHAUSTED, SOLVER_ERROR}
# (TIMEOUT is internal-only; the cap converts TIMEOUT → TIMEOUT_EXHAUSTED.)
# Per-attempt invocation: `timeout <N> $SOLVER <smt2>` — `timeout(1)` is the
# bounded wait per autopilot-no-unbounded-wait; no `wait` shell builtin.
# ---------------------------------------------------------------------------

run_solver_bounded_retry() {
    local smt_path="$1"
    local shape_id="$2"
    local attempt=0
    local budget="$INITIAL_TIMEOUT_SECONDS"
    local budgets_csv=""
    local solver_output=""
    local solver_exit=0

    while [[ "$attempt" -le "$MAX_RETRIES" ]]; do
        if [[ -z "$budgets_csv" ]]; then
            budgets_csv="$budget"
        else
            budgets_csv="${budgets_csv},${budget}"
        fi

        # Bounded wall-clock per attempt via timeout(1). No interactive wait.
        # timeout exit code 124 = wall-clock budget hit; pass through others.
        solver_output=$(timeout "$budget" "$SOLVER" "$smt_path" 2>&1) && solver_exit=0 || solver_exit=$?

        if [[ "$solver_exit" -eq 124 ]]; then
            # TIMEOUT: retry with doubled budget unless cap reached.
            attempt=$((attempt + 1))
            budget=$((budget * 2))
            if [[ "$attempt" -gt "$MAX_RETRIES" ]]; then
                echo "TIMEOUT_EXHAUSTED|${attempt}|${budgets_csv}"
                return 0
            fi
            continue
        fi

        if [[ "$solver_exit" -ne 0 ]]; then
            # Non-timeout non-zero exit = solver error / parse fail.
            echo "SOLVER_ERROR|${attempt}|${budgets_csv}"
            return 0
        fi

        # Parse solver output: each (check-sat) emits sat/unsat/unknown on its
        # own line. Multiple blocks (push/pop) emit multiple lines. The
        # load-bearing verdict is the FIRST sat/unsat token (the conjecture
        # block runs first; subsequent sentinel checks are discarded).
        local verdict
        verdict=$(echo "$solver_output" | grep -E '^(sat|unsat|unknown)$' | head -n 1)
        case "$verdict" in
            sat)
                echo "SAT|${attempt}|${budgets_csv}"
                return 0
                ;;
            unsat)
                echo "UNSAT|${attempt}|${budgets_csv}"
                return 0
                ;;
            unknown|"")
                # Solver returned `unknown` or no verdict line: treat as
                # SOLVER_ERROR to surface; do NOT retry indefinitely.
                echo "SOLVER_ERROR|${attempt}|${budgets_csv}"
                return 0
                ;;
            *)
                echo "SOLVER_ERROR|${attempt}|${budgets_csv}"
                return 0
                ;;
        esac
    done

    # Defensive: loop must terminate above; reaching here is a runner bug.
    echo "TIMEOUT_EXHAUSTED|${attempt}|${budgets_csv}"
    return 0
}

# ---------------------------------------------------------------------------
# Helper: append-run-row — append a row to results.json's runs[] array
# via Python (no inline jq; consistent with §09 pattern).
# ---------------------------------------------------------------------------

append_run_row() {
    local shape_id="$1"
    local smt_result="$2"
    local retry_count="$3"
    local budgets_csv="$4"
    local route_target="$5"
    local unsat_core_span="$6"

    python3 - "$results_json" "$shape_id" "$smt_result" "$retry_count" "$budgets_csv" "$route_target" "$unsat_core_span" <<'PYEOF'
import json, sys
results_path, shape_id, smt_result, retry_count, budgets_csv, route_target, unsat_core_span = sys.argv[1:8]
with open(results_path, "r") as f:
    data = json.load(f)
budgets = [int(x) for x in budgets_csv.split(",") if x]
data["runs"].append({
    "shape_id": shape_id,
    "smt_result": smt_result,
    "retry_count": int(retry_count),
    "attempt_budgets_seconds": budgets,
    "route_target": route_target,
    "unsat_core_span": unsat_core_span,
})
with open(results_path, "w") as f:
    json.dump(data, f, indent=2, sort_keys=True)
    f.write("\n")
PYEOF
}

# ---------------------------------------------------------------------------
# Helper: finalize-results — write exit_reason field + emit terminal envelope.
# ---------------------------------------------------------------------------

finalize_results() {
    local exit_reason="$1"
    python3 - "$results_json" "$exit_reason" <<'PYEOF'
import json, sys
results_path, exit_reason = sys.argv[1:3]
with open(results_path, "r") as f:
    data = json.load(f)
data["exit_reason"] = exit_reason
with open(results_path, "w") as f:
    json.dump(data, f, indent=2, sort_keys=True)
    f.write("\n")
PYEOF
}

# ---------------------------------------------------------------------------
# Phase (a) — encoder-validation gate (both fixtures FIRST).
# ---------------------------------------------------------------------------

if [[ ! -f "$FIXTURE_FILE" ]]; then
    finalize_results "counterexample_infrastructure_failed"
    cat > test-results/section-10-result.json <<EOF
{"status": "fail", "exit_reason": "counterexample_infrastructure_failed", "phase": "preflight", "reason": "encoder-validation fixture missing: ${FIXTURE_FILE}"}
EOF
    exit 3
fi

# Run the fixture file once and parse BOTH push/pop block verdicts in the
# emitted output. The known-SAT block is FIRST (per encoder-validation.smt2
# §2); known-UNSAT block is SECOND (per §3).
fixture_output=$(timeout "$INITIAL_TIMEOUT_SECONDS" "$SOLVER" "$FIXTURE_FILE" 2>&1) && fixture_exit=0 || fixture_exit=$?
if [[ "$fixture_exit" -ne 0 ]]; then
    finalize_results "counterexample_infrastructure_failed"
    cat > test-results/section-10-result.json <<EOF
{"status": "fail", "exit_reason": "counterexample_infrastructure_failed", "phase": "encoder_validation", "reason": "fixture solver exit nonzero: ${fixture_exit}"}
EOF
    exit 3
fi

# Collect every sat/unsat/unknown line emitted by the fixture's (check-sat)
# calls into a CSV. The fixture authors TWO load-bearing check-sat calls:
# index 0 = known-SAT block (expected sat), index 1 = known-UNSAT block
# (expected unsat).
fixture_verdicts=$(echo "$fixture_output" | grep -E '^(sat|unsat|unknown)$')
known_sat_verdict=$(echo "$fixture_verdicts" | sed -n '1p')
known_unsat_verdict=$(echo "$fixture_verdicts" | sed -n '2p')

known_sat_pass=0
known_unsat_pass=0
if [[ "$known_sat_verdict" == "sat" ]]; then
    known_sat_pass=1
fi
if [[ "$known_unsat_verdict" == "unsat" ]]; then
    known_unsat_pass=1
fi

# Record fixture rows in results.json.
if [[ "$known_sat_pass" -eq 1 ]]; then
    append_run_row "fixture-known-sat-vacuity-guard" "SAT" "0" "$INITIAL_TIMEOUT_SECONDS" "encoder_validated" ""
else
    append_run_row "fixture-known-sat-vacuity-guard" "ENCODER_INVALID" "0" "$INITIAL_TIMEOUT_SECONDS" "halt" "known-SAT returned: ${known_sat_verdict:-<none>}"
fi
if [[ "$known_unsat_pass" -eq 1 ]]; then
    append_run_row "fixture-known-unsat-soundness-sanity" "UNSAT" "0" "$INITIAL_TIMEOUT_SECONDS" "encoder_validated" ""
else
    append_run_row "fixture-known-unsat-soundness-sanity" "ENCODER_INVALID" "0" "$INITIAL_TIMEOUT_SECONDS" "halt" "known-UNSAT returned: ${known_unsat_verdict:-<none>}"
fi

if [[ "$known_sat_pass" -ne 1 ]] || [[ "$known_unsat_pass" -ne 1 ]]; then
    finalize_results "encoder_invalid_halt"
    cat > test-results/section-10-result.json <<EOF
{"status": "fail", "exit_reason": "encoder_invalid_halt", "phase": "encoder_validation", "known_sat_verdict": "${known_sat_verdict:-<none>}", "known_unsat_verdict": "${known_unsat_verdict:-<none>}", "reason": "encoder-validation fixture failed; vacuous-UNSAT hazard active OR soundness sanity rejected a sound shape"}
EOF
    exit 2
fi

# ---------------------------------------------------------------------------
# Phase (b) — real-shape corpus discharge.
# ---------------------------------------------------------------------------

shapes_unsat=0
shapes_sat=0
shapes_timeout_exhausted=0
shapes_solver_error=0

for smt_path in "${shape_corpus[@]}"; do
    shape_id=$(basename "$smt_path" .smt2)
    run_result=$(run_solver_bounded_retry "$smt_path" "$shape_id")
    IFS='|' read -r result retry_count budgets_csv <<< "$run_result"

    case "$result" in
        UNSAT)
            shapes_unsat=$((shapes_unsat + 1))
            append_run_row "$shape_id" "UNSAT" "$retry_count" "$budgets_csv" "feeds_section_12_adequacy" ""
            ;;
        SAT)
            shapes_sat=$((shapes_sat + 1))
            append_run_row "$shape_id" "SAT" "$retry_count" "$budgets_csv" "routes_section_02_to_09_plus_11_plus_12" "pending_classification"
            ;;
        TIMEOUT_EXHAUSTED)
            shapes_timeout_exhausted=$((shapes_timeout_exhausted + 1))
            append_run_row "$shape_id" "TIMEOUT_EXHAUSTED" "$retry_count" "$budgets_csv" "manual_review" "inconclusive"
            ;;
        SOLVER_ERROR)
            shapes_solver_error=$((shapes_solver_error + 1))
            append_run_row "$shape_id" "ENCODER_INVALID" "$retry_count" "$budgets_csv" "halt" "solver returned unknown / parse error"
            ;;
    esac
done

# ---------------------------------------------------------------------------
# Phase (c) — terminal exit_reason classification.
# ---------------------------------------------------------------------------

if [[ "$shapes_solver_error" -gt 0 ]]; then
    finalize_results "encoder_invalid_halt"
    cat > test-results/section-10-result.json <<EOF
{"status": "fail", "exit_reason": "encoder_invalid_halt", "phase": "real_shape_discharge", "unsat": ${shapes_unsat}, "sat": ${shapes_sat}, "timeout_exhausted": ${shapes_timeout_exhausted}, "solver_error": ${shapes_solver_error}, "reason": "≥1 real shape produced solver error / unknown verdict; encoder unsound"}
EOF
    exit 2
fi

if [[ "$shapes_sat" -gt 0 ]]; then
    # SAT verdict: classification disambiguates witness_found / proof_gap /
    # reformulation_required per COUNTEREXAMPLE_SAT_CLASSIFICATION env-var.
    finalize_results "$SAT_CLASSIFICATION"
    cat > test-results/section-10-result.json <<EOF
{"status": "fail", "exit_reason": "${SAT_CLASSIFICATION}", "phase": "real_shape_discharge", "unsat": ${shapes_unsat}, "sat": ${shapes_sat}, "timeout_exhausted": ${shapes_timeout_exhausted}}
EOF
    exit 2
fi

if [[ "$shapes_timeout_exhausted" -gt 0 ]]; then
    finalize_results "smt_timeout_inconclusive"
    cat > test-results/section-10-result.json <<EOF
{"status": "fail", "exit_reason": "smt_timeout_inconclusive", "phase": "real_shape_discharge", "unsat": ${shapes_unsat}, "sat": ${shapes_sat}, "timeout_exhausted": ${shapes_timeout_exhausted}}
EOF
    exit 2
fi

# All UNSAT, fixtures green: §10 PASS gate.
finalize_results "counterexample_search_adequate"
cat > test-results/section-10-result.json <<EOF
{"status": "pass", "exit_reason": "counterexample_search_adequate", "unsat": ${shapes_unsat}, "fixtures_passed": 2}
EOF
exit 0
