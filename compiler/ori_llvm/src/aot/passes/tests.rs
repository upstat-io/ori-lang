//! Tests for the optimization pipeline, including lint pass integration.

use inkwell::context::Context;

use crate::aot::target::TargetConfig;
use crate::aot::OptimizationConfig;

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
