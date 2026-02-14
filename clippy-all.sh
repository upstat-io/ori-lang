#!/bin/bash
# Run clippy on ALL Rust code: workspace, runtime, and LLVM crate
# Usage: ./clippy-all

set -e

# Workspace clippy (target/) and Docker LLVM clippy (target-llvm/) use
# different target directories, so they can safely run in parallel.
echo "=== Running clippy on workspace + LLVM crate (parallel) ==="

cargo cl &
CL_PID=$!

./llvm-clippy.sh &
LLVM_PID=$!

# Wait for workspace clippy first — ori_rt may share its target/ dir
CL_EXIT=0
wait $CL_PID || CL_EXIT=$?

# ori_rt clippy runs after workspace clippy (same target/ dir)
echo ""
echo "=== Running clippy on runtime library ==="
RT_EXIT=0
cargo clippy --manifest-path compiler/ori_rt/Cargo.toml --all-targets -- -D warnings || RT_EXIT=$?

# Wait for Docker LLVM clippy
LLVM_EXIT=0
wait $LLVM_PID || LLVM_EXIT=$?

# Report results
if [ $CL_EXIT -ne 0 ] || [ $RT_EXIT -ne 0 ] || [ $LLVM_EXIT -ne 0 ]; then
    echo ""
    [ $CL_EXIT -ne 0 ] && echo "  ✗ Workspace clippy FAILED"
    [ $RT_EXIT -ne 0 ] && echo "  ✗ Runtime clippy FAILED"
    [ $LLVM_EXIT -ne 0 ] && echo "  ✗ LLVM clippy FAILED"
    exit 1
fi

echo ""
echo "=== All clippy checks passed ==="
