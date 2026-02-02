#!/bin/bash
# Build LLVM crate and runtime library in Docker container
# Usage: ./llvm-build [additional args...]
#
# Note: Both ori_llvm and ori_rt are built to ensure libori_rt.a is available
# for AOT compilation. We use raw shell commands to control manifest paths.
ARGS="${*:---release}"
exec ./docker/llvm/run.sh sh -c "cargo build --manifest-path compiler/ori_rt/Cargo.toml ${ARGS} && cargo build --manifest-path compiler/ori_llvm/Cargo.toml ${ARGS}"
