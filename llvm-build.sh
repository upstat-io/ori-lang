#!/bin/bash
# Build the LLVM crate (ori_llvm) and the runtime staticlib (ori_rt) natively
# against the local LLVM 21 toolchain (LLVM_SYS_211_PREFIX in .cargo/config.toml).
# Usage: ./llvm-build.sh [additional cargo args...]
#
# Both ori_llvm and ori_rt are built to ensure libori_rt.a is available for AOT
# compilation. ori_rt is excluded from the workspace, so it is built via an
# explicit --manifest-path.
ARGS="${*:---release}"
exec sh -c "cargo build --manifest-path compiler/ori_rt/Cargo.toml ${ARGS} && cargo build --manifest-path compiler/ori_llvm/Cargo.toml ${ARGS}"
