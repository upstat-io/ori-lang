#!/bin/bash
# Run clippy on LLVM crates in Docker container
# Usage: ./llvm-clippy [additional args...]
# Note: run.sh auto-adds --manifest-path for ori_llvm
exec ./docker/llvm/run.sh cargo clippy --all-targets -- -D warnings "$@"
