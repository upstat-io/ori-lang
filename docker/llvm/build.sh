#!/bin/bash
# Build the LLVM container image
# Run once, then use run.sh for fast execution

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
docker build -t ori-llvm:latest "$SCRIPT_DIR"

echo "Built ori-llvm:latest"
echo "Run with: ./docker/llvm/run.sh <command>"
