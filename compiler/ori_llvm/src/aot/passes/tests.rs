//! Tests for the optimization pipeline, including lint pass integration
//! and sanitizer mode configuration.

use inkwell::context::Context;

use crate::aot::target::TargetConfig;
use crate::aot::{OptimizationConfig, SanitizerMode};

use super::run_optimization_passes;

/// Create a minimal valid LLVM module with a single void function.
fn create_minimal_module(context: &Context) -> inkwell::module::Module<'_> {
    let module = context.create_module("lint_test");
    let void_type = context.void_type();
    let fn_type = void_type.fn_type(&[], false);
    let function = module.add_function("test_fn", fn_type, None);
    let entry = context.append_basic_block(function, "entry");
    let builder = context.create_builder();
    builder.position_at_end(entry);
    builder.build_return(None).unwrap();
    module
}

/// Lint pass runs successfully on valid IR without errors.
///
/// Verifies that `function(lint)` is appended to the pipeline when
/// `lint_enabled` is true, and that valid IR passes without errors.
#[test]
fn opt_lint_runs_on_valid_ir_without_error() {
    let target = TargetConfig::native().unwrap();
    let tm = target.create_target_machine().unwrap();
    let context = Context::create();
    let module = create_minimal_module(&context);

    let config = OptimizationConfig::new(crate::aot::OptimizationLevel::O0).with_lint(true);

    let result = run_optimization_passes(&module, &tm, &config);
    assert!(result.is_ok(), "lint pass should succeed on valid IR");
}

/// Lint pass integrates with audit pipeline: `ORI_AUDIT_CODEGEN=1` auto-enables lint.
///
/// The `lint_enabled` field is wired through the config builder, not the env
/// var directly (env var is in `oric`). This test verifies the config path.
#[test]
fn opt_lint_config_builder_enables_lint() {
    let config = OptimizationConfig::new(crate::aot::OptimizationLevel::O2).with_lint(true);
    assert!(
        config.lint_enabled,
        "with_lint(true) should set lint_enabled"
    );

    let default_config = OptimizationConfig::new(crate::aot::OptimizationLevel::O2);
    assert!(
        !default_config.lint_enabled,
        "lint should be off by default"
    );
}

/// Lint pass runs at multiple optimization levels without error.
#[test]
fn opt_lint_runs_at_all_optimization_levels() {
    let target = TargetConfig::native().unwrap();
    let tm = target.create_target_machine().unwrap();
    let context = Context::create();
    let module = create_minimal_module(&context);

    for level in [
        crate::aot::OptimizationLevel::O0,
        crate::aot::OptimizationLevel::O1,
        crate::aot::OptimizationLevel::O2,
        crate::aot::OptimizationLevel::O3,
    ] {
        let config = OptimizationConfig::new(level).with_lint(true);
        let result = run_optimization_passes(&module, &tm, &config);
        assert!(
            result.is_ok(),
            "lint pass should succeed at {level:?}: {result:?}"
        );
    }
}

// SanitizerMode parsing and configuration tests

#[test]
fn sanitizer_mode_from_env_address_only() {
    let mode = SanitizerMode::from_env_value("address");
    assert!(mode.address);
    assert!(!mode.undefined);
}

#[test]
fn sanitizer_mode_from_env_address_and_undefined() {
    let mode = SanitizerMode::from_env_value("address,undefined");
    assert!(mode.address);
    assert!(mode.undefined);
}

#[test]
fn sanitizer_mode_from_env_undefined_only() {
    let mode = SanitizerMode::from_env_value("undefined");
    assert!(!mode.address);
    assert!(mode.undefined);
}

#[test]
fn sanitizer_mode_from_env_empty_is_none() {
    let mode = SanitizerMode::from_env_value("");
    assert!(!mode.address);
    assert!(!mode.undefined);
    assert_eq!(mode, SanitizerMode::NONE);
}

#[test]
fn sanitizer_mode_from_env_whitespace_tolerant() {
    let mode = SanitizerMode::from_env_value(" address , undefined ");
    assert!(mode.address);
    assert!(mode.undefined);
}

#[test]
fn sanitizer_mode_from_env_unknown_ignored() {
    let mode = SanitizerMode::from_env_value("address,foo");
    assert!(mode.address);
    assert!(!mode.undefined);
}

#[test]
fn sanitizer_mode_clang_flag_value_both() {
    let mode = SanitizerMode {
        address: true,
        undefined: true,
    };
    assert_eq!(
        mode.clang_flag_value(),
        Some("address,undefined".to_string())
    );
}

#[test]
fn sanitizer_mode_clang_flag_value_address_only() {
    let mode = SanitizerMode {
        address: true,
        undefined: false,
    };
    assert_eq!(mode.clang_flag_value(), Some("address".to_string()));
}

#[test]
fn sanitizer_mode_clang_flag_value_none() {
    assert_eq!(SanitizerMode::NONE.clang_flag_value(), None);
}

#[test]
fn sanitizer_mode_any_enabled_true_for_address() {
    let mode = SanitizerMode {
        address: true,
        undefined: false,
    };
    assert!(mode.any_enabled());
}

#[test]
fn sanitizer_mode_any_enabled_true_for_undefined() {
    let mode = SanitizerMode {
        address: false,
        undefined: true,
    };
    assert!(mode.any_enabled());
}

#[test]
fn sanitizer_mode_any_enabled_false_for_none() {
    assert!(!SanitizerMode::NONE.any_enabled());
}

#[test]
fn optimization_config_default_has_no_sanitizer() {
    let config = OptimizationConfig::default();
    assert_eq!(config.sanitizer, SanitizerMode::NONE);
}

#[test]
fn optimization_config_with_sanitizer_builder() {
    let mode = SanitizerMode {
        address: true,
        undefined: true,
    };
    let config =
        OptimizationConfig::new(crate::aot::OptimizationLevel::O2).with_sanitizer(mode.clone());
    assert_eq!(config.sanitizer, mode);
}
