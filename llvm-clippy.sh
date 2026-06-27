#!/bin/bash
# Run clippy on the LLVM crate (ori_llvm) natively against the local LLVM 21
# toolchain (LLVM_SYS_211_PREFIX in .cargo/config.toml).
# Usage: ./llvm-clippy.sh [additional args...]
# The whole-workspace ./clippy-all.sh already lints ori_llvm; this is the
# scoped ori_llvm-only form.
exec cargo clippy --manifest-path compiler/ori_llvm/Cargo.toml --all-targets "$@" -- -D warnings
