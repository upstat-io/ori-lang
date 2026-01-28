#!/bin/bash
# Run a command in the LLVM container
# Mounts workspace read-write, uses cached cargo registry
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

# Use host's cargo registry cache for faster builds
CARGO_CACHE="${HOME}/.cargo/registry"

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
    if [ "$1" = "test" ]; then
        shift
        # Build oric with llvm feature, then run tests
        CMD="cargo build --release --features llvm && ./target/release/ori test --backend=llvm $*"
    else
        CMD="cargo build --release --features llvm && ./target/release/ori $*"
    fi
else
    DOCKER_FLAGS=""
    CMD="$*"
fi

exec docker run --rm ${DOCKER_FLAGS} \
    --memory="${MEMORY_LIMIT}" \
    --memory-swap="${MEMORY_LIMIT}" \
    --cpus="${CPU_LIMIT}" \
    --security-opt seccomp=unconfined \
    -v "${WORKSPACE_ROOT}:/workspace" \
    -v "${CONTAINER_TARGET}:/workspace/target" \
    -v "${CARGO_CACHE}:/root/.cargo/registry:ro" \
    -w /workspace \
    -e ORI_DEBUG_LLVM="${ORI_DEBUG_LLVM:-}" \
    ori-llvm:latest \
    sh -c "${CMD}"
