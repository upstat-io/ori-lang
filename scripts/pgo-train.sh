#!/usr/bin/env bash
# PGO Training Script
#
# Runs representative workloads through an instrumented ori binary
# to generate profile data for profile-guided optimization.
#
# Usage: scripts/pgo-train.sh <path-to-instrumented-ori>
#
# The workload exercises:
#   - Spec tests (diverse syntax, edge cases, many small files)
#   - Standard library (real-world code, larger files)
#   - Benchmarks (compute-heavy, if available)
#
# Each file is run through `ori check` (lexer + parser + type checker),
# which is the primary hot path we're optimizing.

set -euo pipefail

if [[ $# -lt 1 ]]; then
    echo "Usage: $0 <path-to-instrumented-ori>"
    echo ""
    echo "Runs representative workloads to generate PGO training profiles."
    exit 1
fi

ORI="$1"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

if [[ ! -x "$ORI" ]]; then
    echo "ERROR: $ORI is not executable"
    exit 1
fi

echo "Training binary: $ORI"
echo ""

checked=0
errors=0

run_check() {
    local file="$1"
    if "$ORI" check "$file" >/dev/null 2>&1; then
        checked=$((checked + 1))
    else
        # Still generates profile data even on check failure
        checked=$((checked + 1))
        errors=$((errors + 1))
    fi
}

# === Phase 1: Spec tests (diverse syntax) ===
echo "Phase 1: Spec tests..."
while IFS= read -r -d '' file; do
    run_check "$file"
done < <(find "$ROOT_DIR/tests/spec" -name '*.ori' -print0)
echo "  Checked $checked files ($errors with errors)"

# === Phase 2: Standard library ===
echo "Phase 2: Standard library..."
phase2_start=$checked
while IFS= read -r -d '' file; do
    run_check "$file"
done < <(find "$ROOT_DIR/library/std" -name '*.ori' -print0)
echo "  Checked $((checked - phase2_start)) files"

# === Phase 3: Benchmarks (if available) ===
if [[ -d "$ROOT_DIR/tests/benchmarks" ]]; then
    echo "Phase 3: Benchmark programs..."
    phase3_start=$checked
    while IFS= read -r -d '' file; do
        run_check "$file"
    done < <(find "$ROOT_DIR/tests/benchmarks" -name '*.ori' -print0)
    echo "  Checked $((checked - phase3_start)) files"
fi

# === Phase 4: Multiple passes on large files (steady-state behavior) ===
echo "Phase 4: Repeated passes on large files..."
prelude="$ROOT_DIR/library/std/prelude.ori"
if [[ -f "$prelude" ]]; then
    for _ in {1..5}; do
        run_check "$prelude"
    done
    echo "  5 extra passes on prelude.ori"
fi

echo ""
echo "Training complete: $checked total checks ($errors with errors)"
