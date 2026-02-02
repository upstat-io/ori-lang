#!/bin/bash
# Run a command in the LLVM container
# Mounts workspace read-write, optionally uses host cargo cache (local dev only)
#
# Usage:
#   ./docker/llvm/run.sh cargo build       # builds ori_llvm
#   ./docker/llvm/run.sh cargo test        # tests ori_llvm
#   ./docker/llvm/run.sh cargo clippy      # clippy ori_llvm
#   ./docker/llvm/run.sh ori test          # run ALL spec tests with LLVM backend
#   ./docker/llvm/run.sh ori test <path>   # run specific spec tests with LLVM
#   ./docker/llvm/run.sh                   # interactive shell
#
# Resource limits (override via environment):
#   LLVM_MEMORY=8g ./docker/llvm/run.sh cargo build
#   LLVM_CPUS=4 ./docker/llvm/run.sh cargo test

set -e

WORKSPACE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# Resource limits - conservative defaults to protect host
MEMORY_LIMIT="${LLVM_MEMORY:-4g}"
CPU_LIMIT="${LLVM_CPUS:-2}"

# Separate target dir to avoid host/container binary conflicts
CONTAINER_TARGET="${WORKSPACE_ROOT}/target-llvm"
mkdir -p "${CONTAINER_TARGET}"

# Use -it only for interactive shell, otherwise just -i
if [ $# -eq 0 ]; then
    DOCKER_FLAGS="-it"
    CMD="/bin/bash"
elif [ "$1" = "cargo" ] && [ $# -ge 2 ]; then
    DOCKER_FLAGS=""
    # Auto-add manifest path for cargo commands on ori_llvm
    SUBCMD="$2"
    shift 2
    CMD="cargo ${SUBCMD} --manifest-path compiler/ori_llvm/Cargo.toml $*"
elif [ "$1" = "ori" ]; then
    DOCKER_FLAGS=""
    shift
    # Build oric with LLVM feature and run with --backend=llvm
    # Note: ori_rt must be built explicitly via --manifest-path (excluded from workspace)
    if [ "$1" = "test" ]; then
        shift
        # Build ori_rt staticlib, oric with llvm feature, then run tests
        CMD="cargo build --manifest-path compiler/ori_rt/Cargo.toml --release && cargo build -p oric --release --features llvm && ./target/release/ori test --backend=llvm $*"
    else
        CMD="cargo build --manifest-path compiler/ori_rt/Cargo.toml --release && cargo build -p oric --release --features llvm && ./target/release/ori $*"
    fi
else
    DOCKER_FLAGS=""
    CMD="$*"
fi

# Use a persistent Docker volume for cargo cache (faster rebuilds, avoids read-only issues)
# The volume persists between runs so downloaded crates are cached
CARGO_VOLUME="ori-llvm-cargo-cache"

# Create the volume if it doesn't exist (silent, idempotent)
docker volume create "${CARGO_VOLUME}" >/dev/null 2>&1 || true

CARGO_CACHE_MOUNT="-v ${CARGO_VOLUME}:/root/.cargo/registry"

exec docker run --rm ${DOCKER_FLAGS} \
    --memory="${MEMORY_LIMIT}" \
    --memory-swap="${MEMORY_LIMIT}" \
    --cpus="${CPU_LIMIT}" \
    --security-opt seccomp=unconfined \
    -v "${WORKSPACE_ROOT}:/workspace" \
    -v "${CONTAINER_TARGET}:/workspace/target" \
    ${CARGO_CACHE_MOUNT} \
    -w /workspace \
    ${ORI_DEBUG_LLVM:+-e ORI_DEBUG_LLVM="$ORI_DEBUG_LLVM"} \
    ori-llvm:latest \
    sh -c "${CMD}"
