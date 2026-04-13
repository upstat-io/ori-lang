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

# Check Clang availability — try versioned names (matching compiler/ori_llvm/src/aot/passes/sanitizer.rs)
find_clang() {
    for candidate in clang clang-21 clang-20 clang-19 clang-18 clang-17; do
        if command -v "$candidate" &>/dev/null; then
            echo "$candidate"
            return 0
        fi
    done
    return 1
}
CLANG=$(find_clang) || {
    echo "ERROR: Clang not found on PATH (tried: clang, clang-21..clang-17)"
    echo "  Required for sanitizer compilation. Install clang or a versioned variant."
    exit 1
}

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

# Semantic pin: compile pin_helper.c directly with ASan and run it.
# This validates that ASan catches a real heap-use-after-free on this host.
# An Ori-native FFI semantic pin is tracked in roadmap §11.12.
if [ -f "$SMOKE_DIR/pin_helper.c" ]; then
    echo -n "  semantic_pin (C) ... "
    PIN_TMPDIR=$(mktemp -d)
    if "$CLANG" -fsanitize=address "$SMOKE_DIR/pin_helper.c" -o "$PIN_TMPDIR/semantic_pin" 2>/dev/null; then
        if timeout 10 "$PIN_TMPDIR/semantic_pin" 2>"$PIN_TMPDIR/run.log"; then
            echo "FAIL (expected ASan to catch UAF, but program exited 0)"
            FAIL_COUNT=$((FAIL_COUNT + 1))
        else
            if grep -q "heap-use-after-free\|AddressSanitizer" "$PIN_TMPDIR/run.log" 2>/dev/null; then
                echo "PASS (ASan correctly detected heap-use-after-free)"
                PASS_COUNT=$((PASS_COUNT + 1))
                SEMANTIC_PIN_TESTED=true
            else
                echo "FAIL (non-zero exit but no ASan report)"
                head -5 "$PIN_TMPDIR/run.log" >&2
                FAIL_COUNT=$((FAIL_COUNT + 1))
            fi
        fi
    else
        echo "SKIP (failed to compile pin_helper.c)"
        SKIP_COUNT=$((SKIP_COUNT + 1))
    fi
    rm -rf "$PIN_TMPDIR"
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
    echo "  WARNING: Semantic pin NOT TESTED (pin_helper build failed or skipped)"
    echo "  The smoke suite passed but the ASan detection capability was not validated."
fi

if [ "$FAIL_COUNT" -gt 0 ]; then
    echo "ERROR: $FAIL_COUNT sanitizer smoke test(s) FAILED"
    echo "If failures are pre-existing memory bugs in generated code, file via /add-bug."
    exit 1
fi
