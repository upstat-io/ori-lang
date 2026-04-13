//! Tests for Clang-delegated sanitizer pass integration.

use super::*;
use crate::aot::target::TargetConfig;

#[test]
fn clang_available_check_succeeds_when_clang_installed() {
    let result = check_clang_available();
    assert!(
        result.is_ok(),
        "clang should be available in test environment (tried: {}): {result:?}",
        CLANG_CANDIDATES.join(", ")
    );
}

#[test]
fn find_clang_returns_some_when_installed() {
    let clang = find_clang();
    assert!(
        clang.is_some(),
        "should find a clang binary (tried: {})",
        CLANG_CANDIDATES.join(", ")
    );
}

#[test]
fn clang_compile_with_sanitizers_produces_object_file() {
    use std::io::Write;

    let dir = std::env::temp_dir().join("ori_sanitizer_test_obj");
    let _ = std::fs::create_dir_all(&dir);

    // Write minimal LLVM IR
    let ir_path = dir.join("test.ll");
    let obj_path = dir.join("test.o");
    let mut f = std::fs::File::create(&ir_path).unwrap();
    writeln!(
        f,
        r"define void @test_fn() {{
  ret void
}}"
    )
    .unwrap();

    let sanitizer = SanitizerMode {
        address: true,
        undefined: false,
    };

    let target = TargetConfig::native().unwrap();
    let result =
        clang_compile_with_sanitizers(&ir_path, &obj_path, &sanitizer, "-O0", target.triple());
    assert!(
        result.is_ok(),
        "clang sanitizer compilation should succeed: {result:?}"
    );
    assert!(obj_path.exists(), "object file should be produced");

    // Verify the object contains ASan instrumentation symbols.
    // This pins the -fsanitize flag plumbing — without it, clang produces a
    // plain object that would still pass the exists() check above.
    let obj_bytes = std::fs::read(&obj_path).unwrap();
    let has_asan = obj_bytes
        .windows(7)
        .any(|w| w == b"__asan_" || w == b"asan_re");
    assert!(
        has_asan,
        "sanitized object should contain __asan symbols — \
         the -fsanitize flag may not be reaching clang"
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clang_compile_with_sanitizers_fails_on_invalid_ir() {
    use std::io::Write;

    let dir = std::env::temp_dir().join("ori_sanitizer_test_invalid");
    let _ = std::fs::create_dir_all(&dir);

    let ir_path = dir.join("bad.ll");
    let obj_path = dir.join("bad.o");
    let mut f = std::fs::File::create(&ir_path).unwrap();
    writeln!(f, "this is not valid LLVM IR").unwrap();

    let sanitizer = SanitizerMode {
        address: true,
        undefined: false,
    };

    let target = TargetConfig::native().unwrap();
    let result =
        clang_compile_with_sanitizers(&ir_path, &obj_path, &sanitizer, "-O0", target.triple());
    assert!(result.is_err(), "clang should fail on invalid IR");

    // Cleanup
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn optimization_pipeline_unchanged_when_sanitizers_disabled() {
    let config = crate::aot::OptimizationConfig::new(crate::aot::OptimizationLevel::O2);
    assert!(
        !config.sanitizer.any_enabled(),
        "default config should have no sanitizers"
    );
    assert_eq!(
        config.sanitizer.clang_flag_value(),
        None,
        "no Clang flag when sanitizers disabled"
    );
}
