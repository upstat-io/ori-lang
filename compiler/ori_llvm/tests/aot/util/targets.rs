//! Target configurations shared by AOT integration tests.

use ori_llvm::aot::{TargetConfig, TargetTripleComponents};

fn target(triple: &str) -> TargetConfig {
    let components = TargetTripleComponents::parse(triple).unwrap();
    TargetConfig::from_components(components)
}

#[must_use]
pub fn linux_target() -> TargetConfig {
    target("x86_64-unknown-linux-gnu")
}

#[must_use]
pub fn macos_target() -> TargetConfig {
    target("x86_64-apple-darwin")
}

#[must_use]
pub fn macos_arm_target() -> TargetConfig {
    target("aarch64-apple-darwin")
}

#[must_use]
pub fn windows_msvc_target() -> TargetConfig {
    target("x86_64-pc-windows-msvc")
}

#[must_use]
pub fn windows_gnu_target() -> TargetConfig {
    target("x86_64-pc-windows-gnu")
}

#[must_use]
pub fn wasm32_target() -> TargetConfig {
    target("wasm32-unknown-unknown")
}

/// Uses the canonical WASI Preview 1 spelling.
#[must_use]
pub fn wasm32_wasi_target() -> TargetConfig {
    target("wasm32-unknown-wasip1")
}
