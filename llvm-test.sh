#!/bin/bash
# Run LLVM-related Rust tests
# Tests ori_llvm (which includes ori_rt as a dependency)
#
# Usage: ./llvm-test [additional args...]
#
# Note: Requires LLVM 17 installed. Path configured in .cargo/config.toml
# Uses --manifest-path since ori_llvm is excluded from workspace.
#
# AOT integration tests require an LLVM-enabled `ori` binary. This script
# builds one before running tests to prevent stale-binary failures (E5004).

set -e

# Build the LLVM-enabled ori binary + runtime library.
# AOT tests use ori_binary() to find target/debug/ori or target/release/ori —
# if the most recent binary lacks LLVM support, all AOT tests fail with E5004.
echo "Building LLVM-enabled ori binary..."
cargo build -p oric -p ori_rt --features llvm -q
echo "Running LLVM tests..."
cargo test --manifest-path compiler/ori_llvm/Cargo.toml "$@"
