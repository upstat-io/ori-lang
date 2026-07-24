#[cfg(feature = "llvm")]
mod llvm_tests {
    use super::super::get_cache_dir;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    #[test]
    fn test_cache_dir_exists_or_creatable() {
        let cache_dir = get_cache_dir();
        // Should be a valid path
        assert!(!cache_dir.as_os_str().is_empty());
        // Should contain "ori" somewhere in the path
        let path_str = cache_dir.to_string_lossy();
        assert!(path_str.contains("ori"), "cache dir should contain 'ori'");
    }

    #[test]
    fn test_cache_dir_is_absolute_or_temp() {
        let cache_dir = get_cache_dir();
        // Should be either absolute or in temp
        let is_absolute = cache_dir.is_absolute();
        let is_in_temp = cache_dir.starts_with(std::env::temp_dir());
        assert!(
            is_absolute || is_in_temp,
            "cache dir should be absolute or in temp: {cache_dir:?}"
        );
    }

    #[test]
    fn test_content_hash_deterministic() {
        let content = "let x = 42";
        let version = env!("CARGO_PKG_VERSION");

        let hash1 = {
            let mut hasher = DefaultHasher::new();
            content.hash(&mut hasher);
            version.hash(&mut hasher);
            hasher.finish()
        };

        let hash2 = {
            let mut hasher = DefaultHasher::new();
            content.hash(&mut hasher);
            version.hash(&mut hasher);
            hasher.finish()
        };

        assert_eq!(hash1, hash2, "same content should produce same hash");
    }

    #[test]
    fn test_content_hash_differs_for_different_content() {
        let version = env!("CARGO_PKG_VERSION");

        let hash1 = {
            let mut hasher = DefaultHasher::new();
            "let x = 42".hash(&mut hasher);
            version.hash(&mut hasher);
            hasher.finish()
        };

        let hash2 = {
            let mut hasher = DefaultHasher::new();
            "let x = 43".hash(&mut hasher);
            version.hash(&mut hasher);
            hasher.finish()
        };

        assert_ne!(
            hash1, hash2,
            "different content should produce different hash"
        );
    }

    /// Regression: compiled-run path must thread `sanitizer_env` into `BuildOptions`
    /// so that `build_optimization_config()` produces a `SanitizerMode` with the
    /// correct flags. Previously, `..Default::default()` left `sanitizer_env: None`,
    /// silently dropping sanitizer instrumentation.
    /// See:.R iter 13
    #[test]
    fn test_build_optimization_config_reads_sanitizer_env() {
        use crate::commands::build::{build_optimization_config, BuildOptions, OptLevel};

        // When sanitizer_env is None, sanitizer should be disabled
        let options_none = BuildOptions {
            opt_level: OptLevel::O2,
            ..Default::default()
        };
        let config_none = build_optimization_config(&options_none);
        assert!(
            !config_none.sanitizer.any_enabled(),
            "sanitizer should be disabled when sanitizer_env is None"
        );

        // When sanitizer_env is set to "address", ASan should be enabled
        let options_asan = BuildOptions {
            opt_level: OptLevel::O2,
            sanitizer_env: Some("address".to_string()),
            ..Default::default()
        };
        let config_asan = build_optimization_config(&options_asan);
        assert!(
            config_asan.sanitizer.address,
            "ASan should be enabled when sanitizer_env contains 'address'"
        );
        assert!(
            !config_asan.sanitizer.undefined,
            "UBSan should NOT be enabled when sanitizer_env is only 'address'"
        );

        // When sanitizer_env is "address,undefined", both should be enabled
        let options_both = BuildOptions {
            opt_level: OptLevel::O2,
            sanitizer_env: Some("address,undefined".to_string()),
            ..Default::default()
        };
        let config_both = build_optimization_config(&options_both);
        assert!(config_both.sanitizer.address, "ASan should be enabled");
        assert!(config_both.sanitizer.undefined, "UBSan should be enabled");
    }

    #[test]
    fn test_binary_name_format() {
        let source_name = "hello";
        let content_hash: u64 = 0x1234_5678_90AB_CDEF;
        let binary_name = format!("{source_name}-{content_hash:016x}");

        assert_eq!(binary_name, "hello-1234567890abcdef");
        assert!(binary_name.contains(source_name));
        // Hash should be exactly 16 hex characters
        let parts: Vec<&str> = binary_name.split('-').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[1].len(), 16);
    }
}

#[test]
fn constant_failures_render_as_actionable_e2058_not_runtime_fallback() {
    let interner = oric::ir::StringInterner::new();
    let name = interner.intern("broken");
    let problem = ori_ir::canon::ConstEvalProblem {
        name,
        span: ori_ir::Span::new(4, 12),
        kind: ori_ir::canon::ConstEvalProblemKind::DivisionByZero,
    };
    let result = oric::eval::ModuleEvalResult::constant_errors(
        "error[E2058]: constant evaluation failed".to_string(),
        vec![problem],
    );

    let diagnostics = super::const_eval_diagnostics(&result, &interner);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, ori_diagnostic::ErrorCode::E2058);
    assert!(diagnostics[0].message.contains("divides by zero"));
    assert!(diagnostics[0]
        .suggestions
        .iter()
        .any(|suggestion| suggestion.contains("non-zero divisor")));
}
