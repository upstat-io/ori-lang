#!/usr/bin/env bash
# Build ori_rt with AddressSanitizer instrumentation.
# Requires nightly Rust toolchain.
#
# Output: target/debug/libori_rt_asan.a (or target/release/libori_rt_asan.a)
#
# Usage: ./scripts/build-rt-asan.sh [--release]

set -euo pipefail

PROFILE="debug"
PROFILE_DIR="debug"
if [[ "${1:-}" == "--release" ]]; then
    PROFILE="release"
    PROFILE_DIR="release"
fi

# Check for nightly
if ! rustup run nightly rustc --version &>/dev/null; then
    echo "ERROR: nightly Rust required for sanitizer-instrumented ori_rt"
    echo "Install with: rustup toolchain install nightly"
    exit 1
fi

# Detect host target triple
HOST_TARGET=$(rustc -vV | sed -n 's/^host: //p')

# ori_rt's build.rs compiles C/asm sources via cc::Build (ori_eh).
# Set CFLAGS BEFORE the cargo build to ensure those native objects are
# also ASan-instrumented.
export CFLAGS="-fsanitize=address"
echo "CFLAGS=-fsanitize=address (for C/asm sources in ori_rt build.rs / ori_eh)"

echo "Building ori_rt with ASan instrumentation (nightly, $PROFILE, target=$HOST_TARGET)..."
# -Zbuild-std ensures std itself is ASan-instrumented (Vec, String allocations)
# --target is required with -Zbuild-std
RUSTFLAGS="-Zsanitizer=address" \
    cargo +nightly build -p ori_rt \
    -Zbuild-std \
    --target "$HOST_TARGET" \
    $([ "$PROFILE" = "release" ] && echo "--release") \
    --target-dir target/sanitizer

# With -Zbuild-std + --target, output goes to target/sanitizer/<target-triple>/<profile>/
SRC="target/sanitizer/$HOST_TARGET/$PROFILE_DIR/libori_rt.a"
DEST="target/$PROFILE_DIR/libori_rt_asan.a"

if [ ! -f "$SRC" ]; then
    echo "ERROR: Expected $SRC not found"
    exit 1
fi

cp "$SRC" "$DEST"
echo "ASan-instrumented runtime: $DEST"
