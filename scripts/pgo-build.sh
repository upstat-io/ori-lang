#!/usr/bin/env bash
# PGO + BOLT Build Pipeline
#
# Builds an optimized ori binary using:
#   1. Profile-Guided Optimization (PGO) — branch layout from real workloads
#   2. BOLT binary optimization (optional) — post-link code layout
#
# Usage: scripts/pgo-build.sh [OPTIONS]
#
# Options:
#   --no-bolt       Skip BOLT optimization (PGO only)
#   --no-bench      Skip benchmark comparison at the end
#   --help          Show this help message
#
# Prerequisites:
#   - llvm-tools rustup component (for llvm-profdata)
#   - llvm-bolt-18 (for BOLT; optional — install: sudo apt install llvm-bolt)
#
# Output: target/release-pgo/ori

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$ROOT_DIR"

# --- Options ---
SKIP_BOLT=false
SKIP_BENCH=false
for arg in "$@"; do
    case "$arg" in
        --no-bolt)  SKIP_BOLT=true ;;
        --no-bench) SKIP_BENCH=true ;;
        --help)
            head -20 "$0" | tail -18
            exit 0
            ;;
        *)
            echo "Unknown option: $arg"
            exit 1
            ;;
    esac
done

# --- Tool discovery ---
SYSROOT="$(rustc --print sysroot)"
LLVM_PROFDATA="$SYSROOT/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-profdata"

if [[ ! -x "$LLVM_PROFDATA" ]]; then
    echo "ERROR: llvm-profdata not found at $LLVM_PROFDATA"
    echo "Install with: rustup component add llvm-tools"
    exit 1
fi

LLVM_BOLT=""
if [[ "$SKIP_BOLT" != true ]]; then
    for cmd in llvm-bolt-18 llvm-bolt; do
        if command -v "$cmd" &>/dev/null; then
            LLVM_BOLT="$cmd"
            break
        fi
    done
    if [[ -z "$LLVM_BOLT" ]]; then
        echo "WARNING: llvm-bolt not found. Skipping BOLT."
        echo "  Install with: sudo apt install llvm-bolt"
        SKIP_BOLT=true
    elif [[ ! -f /usr/lib/libbolt_rt_instr.a ]]; then
        echo "WARNING: /usr/lib/libbolt_rt_instr.a not found. Skipping BOLT."
        echo "  Fix with: sudo ln -s /usr/lib/llvm-18/lib/libbolt_rt_instr.a /usr/lib/"
        SKIP_BOLT=true
    fi
fi

TOTAL_PHASES=$([ "$SKIP_BOLT" = true ] && echo 4 || echo 7)
PROFILE_DIR="target/pgo-profiles"
MERGED_PROFILE="$PROFILE_DIR/merged.profdata"
BINARY="target/release-pgo/ori"
BOLT_PROFILE_DIR="target/bolt-profiles"

echo "╔══════════════════════════════════════════════╗"
echo "║         PGO + BOLT Build Pipeline            ║"
echo "╠══════════════════════════════════════════════╣"
echo "║  llvm-profdata: $(basename "$LLVM_PROFDATA")"
if [[ "$SKIP_BOLT" != true ]]; then
    echo "║  llvm-bolt:     $LLVM_BOLT"
else
    echo "║  BOLT:          skipped"
fi
echo "╚══════════════════════════════════════════════╝"
echo ""

# ============================================================
# Phase 1: Instrumented build (PGO profile generation)
# ============================================================
echo "=== Phase 1/$TOTAL_PHASES: Instrumented build (PGO) ==="
rm -rf "$PROFILE_DIR"
mkdir -p "$PROFILE_DIR"

RUSTFLAGS="-Cprofile-generate=$(pwd)/$PROFILE_DIR" \
    cargo build --profile release-pgo -p oric 2>&1

echo "  Built instrumented binary: $BINARY"
echo ""

# ============================================================
# Phase 2: PGO training
# ============================================================
echo "=== Phase 2/$TOTAL_PHASES: PGO training ==="
scripts/pgo-train.sh "$BINARY"
echo ""

# ============================================================
# Phase 3: Merge PGO profiles
# ============================================================
echo "=== Phase 3/$TOTAL_PHASES: Merge PGO profiles ==="
PROFRAW_COUNT=$(find "$PROFILE_DIR" -name '*.profraw' | wc -l)
echo "  Found $PROFRAW_COUNT profile files"

"$LLVM_PROFDATA" merge -o "$MERGED_PROFILE" "$PROFILE_DIR"/*.profraw
PROFILE_SIZE=$(du -h "$MERGED_PROFILE" | cut -f1)
echo "  Merged profile: $MERGED_PROFILE ($PROFILE_SIZE)"
echo ""

# ============================================================
# Phase 4: PGO-optimized build
# ============================================================
echo "=== Phase 4/$TOTAL_PHASES: PGO-optimized build ==="

# When BOLT is enabled, emit relocations so BOLT can read the binary.
PGO_RUSTFLAGS="-Cprofile-use=$(pwd)/$MERGED_PROFILE"
if [[ "$SKIP_BOLT" != true ]]; then
    PGO_RUSTFLAGS="$PGO_RUSTFLAGS -Clink-arg=-Wl,--emit-relocs"
fi

RUSTFLAGS="$PGO_RUSTFLAGS" \
    cargo build --profile release-pgo -p oric 2>&1

echo "  Built PGO-optimized binary: $BINARY"
echo ""

# ============================================================
# Phases 5-7: BOLT (optional)
# ============================================================
# BOLT uses software instrumentation (not perf LBR) so it works
# everywhere — VMs, WSL2, containers, CI — without hardware
# branch counter support.
if [[ "$SKIP_BOLT" != true ]]; then
    # --- Phase 5: Instrument binary with BOLT ---
    echo "=== Phase 5/$TOTAL_PHASES: BOLT instrumentation ==="

    BOLT_INST="${BINARY}.bolt-inst"
    rm -rf "$BOLT_PROFILE_DIR"
    mkdir -p "$BOLT_PROFILE_DIR"

    "$LLVM_BOLT" "$BINARY" \
        -instrument \
        -o "$BOLT_INST" \
        -instrumentation-file-append-pid \
        -instrumentation-file="$(pwd)/$BOLT_PROFILE_DIR/bolt" 2>&1

    echo "  Instrumented binary: $BOLT_INST"
    echo ""

    # --- Phase 6: BOLT training ---
    echo "=== Phase 6/$TOTAL_PHASES: BOLT training ==="

    # Run the instrumented binary through the same workloads.
    # Each run writes a .fdata profile to $BOLT_PROFILE_DIR.
    scripts/pgo-train.sh "$BOLT_INST"

    FDATA_COUNT=$(find "$BOLT_PROFILE_DIR" -name '*.fdata' 2>/dev/null | wc -l)
    echo "  Generated $FDATA_COUNT BOLT profile files"
    echo ""

    # --- Phase 7: BOLT optimization ---
    echo "=== Phase 7/$TOTAL_PHASES: BOLT optimization ==="

    # Merge all .fdata files
    BOLT_MERGED="$BOLT_PROFILE_DIR/merged.fdata"
    if [[ "$FDATA_COUNT" -gt 1 ]]; then
        merge-fdata-18 "$BOLT_PROFILE_DIR"/*.fdata > "$BOLT_MERGED" 2>/dev/null \
            || cat "$BOLT_PROFILE_DIR"/*.fdata > "$BOLT_MERGED"
    elif [[ "$FDATA_COUNT" -eq 1 ]]; then
        cp "$BOLT_PROFILE_DIR"/*.fdata "$BOLT_MERGED"
    else
        echo "  WARNING: No BOLT profile data generated. Skipping BOLT."
        rm -f "$BOLT_INST"
        SKIP_BOLT=true
    fi

    if [[ "$SKIP_BOLT" != true ]]; then
        "$LLVM_BOLT" "$BINARY" \
            -o "${BINARY}.bolt" \
            -data="$BOLT_MERGED" \
            -reorder-blocks=ext-tsp \
            -reorder-functions=hfsort \
            -split-functions \
            -split-all-cold \
            -dyno-stats 2>&1

        mv "${BINARY}.bolt" "$BINARY"
        rm -f "$BOLT_INST"
        echo "  BOLT-optimized binary: $BINARY"
    fi
    echo ""
fi

# ============================================================
# Verification
# ============================================================
echo "=== Verification ==="
FILE_SIZE=$(du -h "$BINARY" | cut -f1)
echo "  Binary size: $FILE_SIZE"
echo "  Quick smoke test..."
# Smoke test: verify the binary runs without crashing (segfault = 139, abort = 134).
# ori check returns non-zero for files with diagnostics, which is fine.
"$BINARY" --version >/dev/null 2>&1
SMOKE_EXIT=$?
if [[ $SMOKE_EXIT -eq 0 ]]; then
    echo "  Smoke test: PASSED ($("$BINARY" --version 2>&1 | head -1))"
else
    echo "  Smoke test: FAILED (exit $SMOKE_EXIT) — binary may be corrupted"
    exit 1
fi
echo ""

# ============================================================
# Benchmark comparison (optional)
# ============================================================
if [[ "$SKIP_BENCH" != true ]]; then
    echo "=== Benchmark ==="
    echo "  Run manually to compare against baseline:"
    echo "    cargo bench -p oric --bench lexer -- 'raw/throughput/5000'"
    echo ""
    echo "  Or compare PGO binary directly:"
    echo "    $BINARY check <file.ori>"
fi

echo ""
LABEL="PGO"
[[ "$SKIP_BOLT" != true ]] && LABEL="PGO+BOLT"
echo "╔══════════════════════════════════════════════╗"
echo "║  Done! $LABEL-optimized binary:              "
echo "║  $BINARY"
echo "╚══════════════════════════════════════════════╝"
