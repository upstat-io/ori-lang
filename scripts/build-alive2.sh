#!/usr/bin/env bash
# Build alive-tv (Alive2 translation validator) from a pinned commit.
# Requires: LLVM 21, Z3 (libz3-dev), re2c, cmake, make or ninja.
#
# Output: prints the path to the alive-tv binary on success.
#
# Usage: ./scripts/build-alive2.sh [--cached] [--help]
#
# The pinned commit is tested against LLVM 21. Update ALIVE2_COMMIT when
# the project's LLVM version changes. The local reference repo is used
# when available; otherwise clones from GitHub.

set -euo pipefail

# --- Configuration ---
ALIVE2_REPO="https://github.com/AliveToolkit/alive2.git"
# Local reference repo (dev machines). Preferred over network clone.
ALIVE2_LOCAL="$HOME/projects/reference_repos/verification_tools/alive2"
# Pinned commit: last LLVM 21-compatible commit (v21.0 + 6 bugfixes).
# Breaks at 324a4b3f ("add support for ptr2addr") which requires LLVM 22+.
ALIVE2_COMMIT="f9ad3cb48a926f456b79e22e73b91cff03fc7b81"
EXPECTED_LLVM_MAJOR=21

# Build output directory (relative to repo root)
BUILD_DIR="build/alive2"
BINARY="$BUILD_DIR/alive-tv"

# --- Flags ---
CACHED=false

usage() {
    cat <<'USAGE'
Usage: ./scripts/build-alive2.sh [OPTIONS]

Build alive-tv (Alive2 translation validator) from a pinned source commit.

Options:
  --cached    Skip rebuild if alive-tv binary exists and is up-to-date
  --help      Show this help message

Prerequisites:
  LLVM 21     System LLVM 21 installation (llvm-config-21 on PATH or
              /usr/lib/llvm-21/bin/llvm-config)
  Z3          SMT solver development files
                Ubuntu/Debian: sudo apt-get install libz3-dev
                macOS:         brew install z3
  re2c        Lexer generator
                Ubuntu/Debian: sudo apt-get install re2c
                macOS:         brew install re2c
  cmake       Build system generator (>= 3.10)
  make/ninja  Build backend (ninja preferred if available)

The alive-tv binary is written to build/alive2/alive-tv.
USAGE
}

for arg in "$@"; do
    case "$arg" in
        --cached) CACHED=true ;;
        --help|-h) usage; exit 0 ;;
        *) echo "ERROR: Unknown argument: $arg" >&2; usage >&2; exit 1 ;;
    esac
done

# --- Find llvm-config ---
find_llvm_config() {
    # Try versioned name first, then unversioned, then known paths
    for candidate in \
        "llvm-config-${EXPECTED_LLVM_MAJOR}" \
        "llvm-config" \
        "/usr/lib/llvm-${EXPECTED_LLVM_MAJOR}/bin/llvm-config"; do
        if command -v "$candidate" &>/dev/null || [ -x "$candidate" ]; then
            echo "$candidate"
            return 0
        fi
    done
    return 1
}

LLVM_CONFIG=$(find_llvm_config) || {
    echo "ERROR: llvm-config not found for LLVM ${EXPECTED_LLVM_MAJOR}" >&2
    echo "  Tried: llvm-config-${EXPECTED_LLVM_MAJOR}, llvm-config, /usr/lib/llvm-${EXPECTED_LLVM_MAJOR}/bin/llvm-config" >&2
    echo "  Install LLVM ${EXPECTED_LLVM_MAJOR}: https://apt.llvm.org/" >&2
    exit 1
}

# Validate LLVM version matches expected
ACTUAL_LLVM=$("$LLVM_CONFIG" --version | cut -d. -f1)
if [ "$ACTUAL_LLVM" != "$EXPECTED_LLVM_MAJOR" ]; then
    echo "ERROR: Alive2 pinned to LLVM $EXPECTED_LLVM_MAJOR but found LLVM $ACTUAL_LLVM (via $LLVM_CONFIG)" >&2
    exit 1
fi

LLVM_PREFIX=$("$LLVM_CONFIG" --prefix)
echo "LLVM: $LLVM_PREFIX (version $("$LLVM_CONFIG" --version))" >&2

# --- Check RTTI ---
RTTI=$("$LLVM_CONFIG" --has-rtti)
if [ "$RTTI" != "YES" ]; then
    echo "ERROR: LLVM must be built with RTTI enabled (--has-rtti reports: $RTTI)" >&2
    echo "  Alive2 requires -DLLVM_ENABLE_RTTI=ON" >&2
    exit 1
fi

# --- Check Z3 ---
# Use pkg-config for portable Z3 discovery (no hardcoded paths).
# CMake's FindZ3 handles the actual build-time discovery; this is a
# pre-flight check to give a clear error before cloning Alive2.
if pkg-config --exists z3 2>/dev/null; then
    Z3_VERSION=$(pkg-config --modversion z3)
    echo "Z3: $Z3_VERSION (via pkg-config)" >&2
elif command -v z3 &>/dev/null; then
    Z3_VERSION=$(z3 --version 2>/dev/null | head -1)
    echo "Z3: $Z3_VERSION (via z3 binary on PATH)" >&2
else
    echo "ERROR: Z3 development files not found" >&2
    echo "  pkg-config could not find z3, and z3 is not on PATH." >&2
    echo "  Ubuntu/Debian: sudo apt-get install libz3-dev" >&2
    echo "  macOS:         brew install z3" >&2
    echo "  From source:   https://github.com/Z3Prover/z3" >&2
    exit 1
fi

# --- Check re2c ---
if ! command -v re2c &>/dev/null; then
    echo "ERROR: re2c not found" >&2
    echo "  Ubuntu/Debian: sudo apt-get install re2c" >&2
    echo "  macOS:         brew install re2c" >&2
    exit 1
fi
echo "re2c: $(re2c --version 2>&1 | head -1)" >&2

# --- Check cmake ---
if ! command -v cmake &>/dev/null; then
    echo "ERROR: cmake not found" >&2
    echo "  Ubuntu/Debian: sudo apt-get install cmake" >&2
    echo "  macOS:         brew install cmake" >&2
    exit 1
fi

# --- Detect build backend ---
if command -v ninja &>/dev/null; then
    CMAKE_GENERATOR="Ninja"
    BUILD_CMD="ninja"
elif command -v ninja-build &>/dev/null; then
    CMAKE_GENERATOR="Ninja"
    BUILD_CMD="ninja-build"
else
    CMAKE_GENERATOR="Unix Makefiles"
    BUILD_CMD="make"
fi
echo "Build backend: $BUILD_CMD ($CMAKE_GENERATOR)" >&2

# --- Cached check ---
if [ "$CACHED" = true ] && [ -x "$BINARY" ]; then
    echo "alive-tv binary exists and --cached specified, skipping rebuild" >&2
    echo "$(pwd)/$BINARY"
    exit 0
fi

# --- Prepare source ---
CHECKOUT_DIR="$BUILD_DIR/src"
mkdir -p "$BUILD_DIR"

if [ -d "$CHECKOUT_DIR/.git" ]; then
    echo "Reusing existing checkout..." >&2
    cd "$CHECKOUT_DIR"
    # Ensure we have the pinned commit — fetch from GitHub if needed
    if ! git cat-file -e "$ALIVE2_COMMIT" 2>/dev/null; then
        echo "Pinned commit not found locally, fetching from GitHub..." >&2
        git remote add github "$ALIVE2_REPO" 2>/dev/null || true
        git fetch github 2>&1 | tail -1 >&2
    fi
else
    echo "Cloning Alive2..." >&2
    if [ -d "$ALIVE2_LOCAL/.git" ]; then
        # Try local reference for speed (may fail if shallow)
        git clone --reference "$ALIVE2_LOCAL" "$ALIVE2_REPO" "$CHECKOUT_DIR" 2>&1 | tail -1 >&2 || {
            echo "Reference clone failed, cloning from GitHub directly..." >&2
            rm -rf "$CHECKOUT_DIR"
            git clone "$ALIVE2_REPO" "$CHECKOUT_DIR" 2>&1 | tail -1 >&2
        }
    else
        git clone "$ALIVE2_REPO" "$CHECKOUT_DIR" 2>&1 | tail -1 >&2
    fi
    cd "$CHECKOUT_DIR"
fi

# Ensure we have the pinned commit (shallow clones may lack it)
if ! git cat-file -e "$ALIVE2_COMMIT" 2>/dev/null; then
    echo "Fetching pinned commit..." >&2
    git fetch origin "$ALIVE2_COMMIT" 2>/dev/null || {
        git remote add github "$ALIVE2_REPO" 2>/dev/null || true
        git fetch github 2>&1 | tail -1 >&2
    }
fi

# Checkout pinned commit
echo "Checking out pinned commit: ${ALIVE2_COMMIT:0:12}" >&2
git checkout "$ALIVE2_COMMIT" --quiet 2>&1 >&2

# --- Build ---
echo "Configuring Alive2 (Release, BUILD_TV=ON)..." >&2
cmake -B build -S . -G "$CMAKE_GENERATOR" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_PREFIX_PATH="$LLVM_PREFIX" \
    -DBUILD_TV=ON \
    2>&1 | grep -E "^--|ERROR|FATAL|WARNING|Found LLVM|Z3|RE2C|Build type" >&2

echo "Building alive-tv..." >&2
"$BUILD_CMD" -C build alive-tv 2>&1 | tail -5 >&2

# Return to repo root
cd - >/dev/null

# --- Verify ---
BUILT_BINARY="$CHECKOUT_DIR/build/alive-tv"
if [ ! -x "$BUILT_BINARY" ]; then
    echo "ERROR: alive-tv binary not found at $BUILT_BINARY" >&2
    exit 1
fi

# Copy to canonical location
cp "$BUILT_BINARY" "$BINARY"
chmod +x "$BINARY"

echo "alive-tv built successfully" >&2
echo "$(pwd)/$BINARY"
