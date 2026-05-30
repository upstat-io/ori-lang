#!/usr/bin/env bash
#
# check-proofs.sh — proof-checker regression gate.
#
# Recursively invokes the proof checker (aims-proof/checker/) over the
# aims-proof/proofs/ corpus and fails on any proof regression.
#
# Modes:
#   check-proofs.sh                  — full-tree: every theorem .proof under
#                                      aims-proof/proofs/.
#   check-proofs.sh <file> [<file>]  — affected-corpus: only the passed paths
#                                      that resolve under aims-proof/proofs/ and
#                                      end in .proof (lefthook {staged_files}).
#                                      A pure non-proof staged set yields zero
#                                      affected proofs -> exit 0.
#   ORI_RUN_PROOFS=1 check-proofs.sh <file>...  — widen an affected-set run to
#                                      the full corpus (local opt-in on a
#                                      non-proof commit). Never gates whether a
#                                      proof-touching commit is checked.
#
# Verdict rule per .proof:
#   - status: definition_only in-body  -> SKIP (formal-spec SSOT file, not a
#                                         dischargeable theorem; e.g.
#                                         proofs/11-coexistence/Handshake.proof).
#   - sibling <name>.expected present  -> emitted JSON MUST byte-match the
#                                         blessed .expected verdict (corpus SSOT).
#   - no .expected sibling             -> status MUST be valid OR
#                                         unimplemented_engine_shape.
#   any other verdict (fail / parse_failure / mismatch) -> regression -> exit 1.
#
# Locality shipped-conformance cross-walk: the locality cross-walk emits
# divergent-pending(blocked-by: the unified-locality dimension) while
# shipped Locality is 4-variant; divergent-pending is NON-FATAL. A fatal
# `divergent` verdict (manifest-free rule drift) or probe/manifest
# infrastructure failure DOES fail the gate.
#
# Exit codes:
#   0 — all affected proofs conform; locality cross-walk gate non-fatal-or-pass.
#   1 — a proof regression OR a fatal locality cross-walk divergence.
#   3 — infrastructure failure (checker build failed).
#
# Cwd contract: anchors to aims-proof/ (this script's parent dir's parent),
# then operates relative to aims-proof/.

set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AIMS_PROOF_DIR="$(dirname "$SCRIPT_DIR")"
REPO_ROOT="$(dirname "$AIMS_PROOF_DIR")"
PROOFS_DIR="${AIMS_PROOF_DIR}/proofs"
CHECKER="${AIMS_PROOF_DIR}/target/release/aims-proof-checker"

# --- Output dir: created here so a fresh checkout (CI) has it before any redirect. ---
mkdir -p "${AIMS_PROOF_DIR}/test-results"

# --- Phase (a): build the checker crate (release; mirrors section runners). ---
if ! ( cd "$AIMS_PROOF_DIR" && cargo build --release -p aims-proof-checker ) > "${AIMS_PROOF_DIR}/test-results/check-proofs-build.log" 2>&1; then
    echo "check-proofs: FATAL — cargo build --release -p aims-proof-checker failed; build log follows:" >&2
    cat "${AIMS_PROOF_DIR}/test-results/check-proofs-build.log" >&2 2>/dev/null || true
    exit 3
fi
if [[ ! -x "$CHECKER" ]]; then
    echo "check-proofs: FATAL — checker binary not present after build: ${CHECKER}" >&2
    exit 3
fi

# --- Resolve the affected proof set. ---
declare -a PROOF_SET=()
WIDEN="${ORI_RUN_PROOFS:-0}"

if [[ $# -eq 0 || "$WIDEN" == "1" ]]; then
    # Full-tree mode: every .proof under the corpus.
    while IFS= read -r p; do
        PROOF_SET+=("$p")
    done < <(find "$PROOFS_DIR" -name '*.proof' | sort)
else
    # Affected-corpus mode: filter passed paths to .proof files under proofs/.
    for arg in "$@"; do
        # Normalize to an absolute path (repo-root-relative or aims-proof-relative).
        case "$arg" in
            /*) abs="$arg" ;;
            aims-proof/*) abs="${REPO_ROOT}/${arg}" ;;
            *)  abs="${AIMS_PROOF_DIR}/${arg}" ;;
        esac
        case "$abs" in
            "${PROOFS_DIR}/"*.proof)
                if [[ -f "$abs" ]]; then
                    PROOF_SET+=("$abs")
                fi
                ;;
        esac
    done
fi

# --- Locality shipped-conformance cross-walk (non-fatal on divergent-pending). ---
# Always run on a full-tree invocation; on an affected-set run, run when any
# locality-cross-walk-relevant proof (08-realization / 05-decisions / 03-canonicalization /
# 06-interprocedural) is in scope. The runner is anchored to aims-proof/.
run_locality_gate() {
    local result="${AIMS_PROOF_DIR}/test-results/section-17-locality-result.json"
    local rc=0
    ( cd "$AIMS_PROOF_DIR" && bash scripts/run-locality-conformance.sh ) > /dev/null 2>&1 || rc=$?
    if [[ ! -f "$result" ]]; then
        echo "check-proofs: locality cross-walk produced no result envelope (rc=${rc})" >&2
        return 1
    fi
    local verdict
    verdict="$(python3 -c "import json,sys; print(json.load(open('${result}')).get('verdict','UNKNOWN'))" 2>/dev/null || echo "UNKNOWN")"
    case "$verdict" in
        pass|divergent_pending|divergent-pending)
            # pass: shipped lattice is 5-variant. divergent-pending: shipped
            # lattice still 4-variant — EXPECTED + NON-FATAL.
            echo "check-proofs: locality conformance verdict: ${verdict} (non-fatal)" >&2
            return 0
            ;;
        *)
            echo "check-proofs: locality conformance FATAL verdict: ${verdict} (rc=${rc})" >&2
            return 1
            ;;
    esac
}

locality_gate_relevant=0
if [[ $# -eq 0 || "$WIDEN" == "1" ]]; then
    locality_gate_relevant=1
else
    for p in "${PROOF_SET[@]+"${PROOF_SET[@]}"}"; do
        case "$p" in
            *"/08-realization/"*|*"/05-decisions/"*|*"/03-canonicalization/"*|*"/06-interprocedural/"*)
                locality_gate_relevant=1
                ;;
        esac
    done
fi

# --- Phase (b): per-proof discharge. ---
checked=0
skipped=0
declare -a regressions=()

for proof in "${PROOF_SET[@]+"${PROOF_SET[@]}"}"; do
    # Skip formal-spec SSOT files (definition_only) — not dischargeable theorems.
    if grep -qE '^\s*status:\s*definition_only\s*$' "$proof"; then
        skipped=$((skipped + 1))
        continue
    fi

    out="$("$CHECKER" check "$proof" --json 2>&1)"
    status="$(python3 -c "import sys,json; print(json.loads(sys.argv[1]).get('status','PARSE_ERR'))" "$out" 2>/dev/null || echo "PARSE_ERR")"
    rel="${proof#"${REPO_ROOT}/"}"
    expected_file="${proof%.proof}.expected"

    if [[ -f "$expected_file" ]]; then
        # Blessed-verdict mode: emitted JSON MUST match the .expected SSOT.
        expected_json="$(tr -d '[:space:]' < "$expected_file")"
        actual_json="$(printf '%s' "$out" | tr -d '[:space:]')"
        if [[ "$actual_json" == "$expected_json" ]]; then
            checked=$((checked + 1))
        else
            regressions+=("${rel}: expected ${expected_json} got ${actual_json}")
        fi
    else
        # Default mode: valid OR unimplemented_engine_shape is conformant.
        case "$status" in
            valid|unimplemented_engine_shape)
                checked=$((checked + 1))
                ;;
            *)
                regressions+=("${rel}: ${status}")
                ;;
        esac
    fi
done

# --- Locality cross-walk gate evaluation. ---
locality_fatal=0
if [[ "$locality_gate_relevant" -eq 1 ]]; then
    if ! run_locality_gate; then
        locality_fatal=1
    fi
fi

# --- Report. ---
echo "check-proofs: ${checked} proofs checked, ${skipped} definition_only skipped, ${#regressions[@]} regressions."
if [[ ${#regressions[@]} -gt 0 ]]; then
    echo "check-proofs: REGRESSIONS:" >&2
    for r in "${regressions[@]}"; do
        echo "  - ${r}" >&2
    done
fi

if [[ ${#regressions[@]} -gt 0 || "$locality_fatal" -eq 1 ]]; then
    exit 1
fi
exit 0
