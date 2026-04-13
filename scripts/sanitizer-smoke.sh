#!/usr/bin/env bash
# Run sanitizer smoke tests. Exit non-zero on any sanitizer error.
# Usage: ORI_SANITIZE=address,undefined ./scripts/sanitizer-smoke.sh
#
# Expected runtime: <=60s (within 150s timeout with margin).
# The semantic pin (semantic_pin_asan.ori) is expected to FAIL with sanitizers
# and PASS without — it validates that ASan catches a real heap-use-after-free.

set -euo pipefail

SANITIZE="${ORI_SANITIZE:-address,undefined}"
SMOKE_DIR="tests/sanitizer"
RELEASE="${ORI_RELEASE:-}"  # Set ORI_RELEASE=1 for O2 matrix coverage
FAIL_COUNT=0
PASS_COUNT=0
SKIP_COUNT=0
SEMANTIC_PIN_TESTED=false

if [ ! -d "$SMOKE_DIR" ]; then
    echo "ERROR: $SMOKE_DIR not found"
    exit 1
fi

# Check Clang availability
if ! command -v clang &>/dev/null; then
    echo "ERROR: Clang not found on PATH (required for sanitizer compilation)"
    exit 1
fi

export ORI_SANITIZE="$SANITIZE"

# Use pre-built binary (ORI_BIN env var or default to target/release/ori)
ORI="${ORI_BIN:-target/release/ori}"
if [ ! -x "$ORI" ]; then
    ORI="${ORI_BIN:-target/debug/ori}"
fi

if [ ! -x "$ORI" ]; then
    echo "ERROR: Ori binary not found at $ORI"
    echo "  Build with: cargo build --release"
    exit 1
fi

echo "=== Sanitizer smoke tests ==="
echo "  Binary:    $ORI"
echo "  Sanitizers: $SANITIZE"
echo "  Release:   ${RELEASE:-no (debug/O0)}"
echo ""

# Build the semantic pin C helper library if pin_helper.c exists
PIN_LIB=""
if [ -f "$SMOKE_DIR/pin_helper.c" ]; then
    PIN_TMPDIR=$(mktemp -d)
    echo "  Building semantic pin C helper..."
    SANITIZER_CFLAGS=""
    if [ -n "$SANITIZE" ]; then
        SANITIZER_CFLAGS="-fsanitize=$SANITIZE"
    fi
    if clang $SANITIZER_CFLAGS -c "$SMOKE_DIR/pin_helper.c" -o "$PIN_TMPDIR/pin_helper.o" 2>/dev/null; then
        if ar rcs "$PIN_TMPDIR/libpin_helper.a" "$PIN_TMPDIR/pin_helper.o" 2>/dev/null; then
            PIN_LIB="$PIN_TMPDIR"
            echo "  Built: $PIN_TMPDIR/libpin_helper.a"
        fi
    fi
    if [ -z "$PIN_LIB" ]; then
        echo "  WARNING: Failed to build pin_helper — semantic pin will be skipped"
    fi
fi

for ori_file in "$SMOKE_DIR"/*.ori; do
    [ -f "$ori_file" ] || continue
    name=$(basename "$ori_file" .ori)

    # Semantic pin gets special handling
    if [ "$name" = "semantic_pin_asan" ]; then
        if [ -z "$PIN_LIB" ]; then
            echo "  $name ... SKIP (no pin_helper library)"
            SKIP_COUNT=$((SKIP_COUNT + 1))
            continue
        fi

        echo -n "  $name ... "
        TMPDIR=$(mktemp -d)

        # Compile with sanitizers and link the C helper
        BUILD_FLAGS=""
        [ -n "$RELEASE" ] && BUILD_FLAGS="--release"
        if ! "$ORI" build $BUILD_FLAGS "$ori_file" -o "$TMPDIR/san_$name" \
            -L "$PIN_LIB" 2>"$TMPDIR/compile.log"; then
            echo "FAIL (compilation)"
            cat "$TMPDIR/compile.log" >&2
            FAIL_COUNT=$((FAIL_COUNT + 1))
            rm -rf "$TMPDIR"
            continue
        fi

        # Semantic pin SHOULD fail with sanitizers (ASan catches the UAF)
        if timeout 10 "$TMPDIR/san_$name" 2>"$TMPDIR/run.log"; then
            echo "FAIL (expected ASan to catch UAF, but program exited 0)"
            FAIL_COUNT=$((FAIL_COUNT + 1))
        else
            # Non-zero exit is expected — ASan detected the error
            if grep -q "heap-use-after-free\|AddressSanitizer" "$TMPDIR/run.log" 2>/dev/null; then
                echo "PASS (ASan correctly detected heap-use-after-free)"
                PASS_COUNT=$((PASS_COUNT + 1))
                SEMANTIC_PIN_TESTED=true
            else
                echo "FAIL (non-zero exit but no ASan report — unexpected crash)"
                cat "$TMPDIR/run.log" >&2
                FAIL_COUNT=$((FAIL_COUNT + 1))
            fi
        fi

        rm -rf "$TMPDIR"
        continue
    fi

    echo -n "  $name ... "
    TMPDIR=$(mktemp -d)

    # Compile with sanitizers (pass --release if ORI_RELEASE=1 for O2 matrix coverage)
    BUILD_FLAGS=""
    [ -n "$RELEASE" ] && BUILD_FLAGS="--release"
    if ! "$ORI" build $BUILD_FLAGS "$ori_file" -o "$TMPDIR/san_$name" 2>"$TMPDIR/compile.log"; then
        echo "FAIL (compilation)"
        cat "$TMPDIR/compile.log" >&2
        FAIL_COUNT=$((FAIL_COUNT + 1))
        rm -rf "$TMPDIR"
        continue
    fi

    # Run the sanitized binary (4s per-test timeout within 60s budget)
    if ! timeout 4 "$TMPDIR/san_$name" 2>"$TMPDIR/run.log"; then
        echo "FAIL (runtime/sanitizer)"
        cat "$TMPDIR/run.log" >&2
        FAIL_COUNT=$((FAIL_COUNT + 1))
        rm -rf "$TMPDIR"
        continue
    fi

    echo "PASS"
    PASS_COUNT=$((PASS_COUNT + 1))
    rm -rf "$TMPDIR"
done

# Clean up pin helper
[ -n "$PIN_LIB" ] && rm -rf "$PIN_LIB"

echo ""
echo "=== Sanitizer smoke: $PASS_COUNT passed, $FAIL_COUNT failed, $SKIP_COUNT skipped ==="

if [ "$SEMANTIC_PIN_TESTED" = true ]; then
    echo "  Semantic pin: VERIFIED (ASan detected deliberate UAF)"
else
    echo "  Semantic pin: NOT TESTED (pin_helper build failed or skipped)"
fi

if [ "$FAIL_COUNT" -gt 0 ]; then
    echo "ERROR: $FAIL_COUNT sanitizer smoke test(s) FAILED"
    echo "If failures are pre-existing memory bugs in generated code, file via /add-bug."
    exit 1
fi
