#!/usr/bin/env bash
#
# run-locality-conformance.sh — locality shipped-conformance runner.
#
# Per the locality shipped-conformance cross-walk:
# emits a per-manifest-rule verdict block in
# test-results/section-17-locality-result.json that the aims-rules revision /
# the CI gate / the global reframing / the locality cross-walk flip protocol
# consume. Invoked by run-section-08-proofs.sh
# --locality-section (sibling-extension shape).
#
# Phases:
# (a) build the aims-proof-checker crate (which builds the
# locality-conformance binary via the syn-AST probe in build.rs);
# failure -> locality_conformance_probe_compile_failure (exit 3).
# (b) validate aims-proof/checker/shipped-conformance-manifest.json
# against shipped-conformance-manifest.schema.json — python3 and
# jsonschema are HARD REQUIREMENTS per the locality cross-walk SC3 (codex-F3 round-2 cure);
# absence exits 3 with locality_conformance_probe_compile_failure;
# schema-validation failure -> locality_conformance_manifest_invalid (exit 1).
# (c) run the locality-conformance binary which reads probe consts
# + manifest + emits the structured envelope; pass-through exit.
#
# Cwd contract: anchors to aims-proof/ via cd "$(dirname "$0")/.." at entry.

set -e
cd "$(dirname "$0")/.."

mkdir -p test-results

readonly MANIFEST="checker/shipped-conformance-manifest.json"
readonly SCHEMA="checker/shipped-conformance-manifest.schema.json"
readonly RESULT_PATH="test-results/section-17-locality-result.json"
readonly BUILD_LOG="test-results/section-17-locality-build.log"

# Phase (a) — build.
if ! cargo build --release -p aims-proof-checker --bin locality-conformance > "$BUILD_LOG" 2>&1; then
    cat > "$RESULT_PATH" <<EOF
{"verdict": "probe_compile_failure", "exit_reason": "locality_conformance_probe_compile_failure", "autopilot_routable": false, "phase": "build", "reason": "cargo build --release -p aims-proof-checker --bin locality-conformance failed; see ${BUILD_LOG}"}
EOF
    exit 3
fi

# Phase (b) — manifest schema validation (HARD dependency per the locality cross-walk SC3 validator-
# dependency discipline). jsonschema MUST be available; silently skipping
# validation would let manifest drift ship undetected. If python3 or jsonschema
# is missing, exit non-zero with locality_conformance_probe_compile_failure and
# surface an installation hint to stderr.
if ! command -v python3 >/dev/null 2>&1; then
    cat > "$RESULT_PATH" <<EOF
{"verdict": "probe_compile_failure", "exit_reason": "locality_conformance_probe_compile_failure", "autopilot_routable": false, "phase": "schema_validate", "reason": "python3 not found on PATH; required for manifest schema validation per the locality cross-walk SC3. Install python3."}
EOF
    echo "run-locality-conformance.sh: python3 not found; install python3 to validate manifest schema per the locality cross-walk SC3" >&2
    exit 3
fi
if ! python3 -c "import jsonschema"; then
    cat > "$RESULT_PATH" <<EOF
{"verdict": "probe_compile_failure", "exit_reason": "locality_conformance_probe_compile_failure", "autopilot_routable": false, "phase": "schema_validate", "reason": "python3 jsonschema package not installed; required for manifest schema validation per the locality cross-walk SC3. Install with: pip install jsonschema"}
EOF
    echo "run-locality-conformance.sh: python3 jsonschema package not installed; install with: pip install jsonschema (required per the locality cross-walk SC3 validator-dependency discipline)" >&2
    exit 3
fi
if ! python3 - <<PY >> "$BUILD_LOG" 2>&1
import json
import sys
import jsonschema

with open("$MANIFEST") as f:
    manifest = json.load(f)
with open("$SCHEMA") as f:
    schema = json.load(f)
jsonschema.validate(instance=manifest, schema=schema)
print("manifest schema validation: ok")
PY
then
    cat > "$RESULT_PATH" <<EOF
{"verdict": "manifest_invalid", "exit_reason": "locality_conformance_manifest_invalid", "autopilot_routable": false, "phase": "schema_validate", "reason": "manifest failed JSON Schema validation; see ${BUILD_LOG}"}
EOF
    exit 1
fi

# Phase (c) — run cross-walk binary.
readonly BIN="./target/release/locality-conformance"
if [[ ! -x "$BIN" ]]; then
    cat > "$RESULT_PATH" <<EOF
{"verdict": "probe_compile_failure", "exit_reason": "locality_conformance_probe_compile_failure", "autopilot_routable": false, "phase": "binary_missing", "reason": "$BIN not present after build"}
EOF
    exit 3
fi

set +e
"$BIN" > "$RESULT_PATH" 2>>"$BUILD_LOG"
exit_code=$?
set -e

exit "$exit_code"
