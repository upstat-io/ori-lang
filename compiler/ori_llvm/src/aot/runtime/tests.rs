//! Tests for runtime library configuration and sanitizer variant discovery.

use super::*;

/// Try to detect runtime, skip test if not found (common in unit test context
/// where the binary is in `target/debug/deps/` not `target/debug/`).
fn detect_or_skip() -> Option<RuntimeConfig> {
    RuntimeConfig::detect().ok()
}

#[test]
fn runtime_config_lib_name_returns_platform_name() {
    let name = RuntimeConfig::lib_name();
    assert!(
        name == "libori_rt.a" || name == "ori_rt.lib",
        "unexpected lib name: {name}"
    );
}

#[test]
fn runtime_config_lib_name_asan_returns_asan_variant() {
    let name = RuntimeConfig::lib_name_asan();
    assert!(
        name == "libori_rt_asan.a" || name == "ori_rt_asan.lib",
        "unexpected asan lib name: {name}"
    );
}

#[test]
fn runtime_validate_sanitizer_ok_when_no_address() {
    let Some(config) = detect_or_skip() else {
        return;
    };
    let sanitizer = SanitizerMode {
        address: false,
        undefined: true,
    };
    // UBSan doesn't require ASan-instrumented runtime
    assert!(config.validate_sanitizer(&sanitizer).is_ok());
}

#[test]
fn runtime_validate_sanitizer_ok_when_none() {
    let Some(config) = detect_or_skip() else {
        return;
    };
    assert!(config.validate_sanitizer(&SanitizerMode::NONE).is_ok());
}

#[test]
fn runtime_configure_link_uses_standard_rt_when_no_sanitizer() {
    let Some(config) = detect_or_skip() else {
        return;
    };
    let mut input = LinkInput::default();
    config.configure_link(&mut input);

    let has_ori_rt = input.libraries.iter().any(|lib| lib.name == "ori_rt");
    assert!(has_ori_rt, "should link ori_rt when no sanitizer");

    let has_asan = input.libraries.iter().any(|lib| lib.name == "ori_rt_asan");
    assert!(!has_asan, "should NOT link ori_rt_asan when no sanitizer");
}

#[test]
fn runtime_config_new_creates_with_path() {
    let path = std::path::PathBuf::from("/tmp/test_rt");
    let config = RuntimeConfig::new(path.clone());
    assert_eq!(config.library_path, path);
    assert!(config.static_link);
}

#[test]
fn runtime_validate_sanitizer_address_fails_without_asan_lib() {
    // Create a config pointing to a directory that definitely doesn't have
    // libori_rt_asan.a — validates the hard error behavior
    let config = RuntimeConfig::new(std::env::temp_dir());
    let sanitizer = SanitizerMode {
        address: true,
        undefined: false,
    };
    let result = config.validate_sanitizer(&sanitizer);
    assert!(
        result.is_err(),
        "should fail when ASan requested but libori_rt_asan.a missing"
    );
}
