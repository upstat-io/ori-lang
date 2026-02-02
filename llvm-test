#!/bin/bash
# Run LLVM-related Rust tests
# Tests ori_llvm (which includes ori_rt as a dependency)
#
# Usage: ./llvm-test [additional args...]
#
# Note: Requires LLVM 17 installed. Path configured in .cargo/config.toml
# Uses --manifest-path since ori_llvm is excluded from workspace.

cargo test --manifest-path compiler/ori_llvm/Cargo.toml "$@"
