//! Optimization pipeline tests.
//!
//! Tests for the LLVM optimization pass management using the new pass manager.
//! These tests validate:
//! - OptimizationLevel pipeline strings and settings
//! - LtoMode pipeline generation
//! - OptimizationConfig builder pattern and effective settings
//! - Integration tests for running optimization passes

#[cfg(feature = "llvm")]
mod tests {
    use ori_llvm::aot::passes::{
        run_custom_pipeline, run_optimization_passes, LtoMode, OptimizationConfig,
        OptimizationError, OptimizationLevel,
    };

    // -- OptimizationLevel tests --

    #[test]
    fn test_optimization_level_pipeline_strings() {
        assert_eq!(OptimizationLevel::O0.pipeline_string(), "default<O0>");
        assert_eq!(OptimizationLevel::O1.pipeline_string(), "default<O1>");
        assert_eq!(OptimizationLevel::O2.pipeline_string(), "default<O2>");
        assert_eq!(OptimizationLevel::O3.pipeline_string(), "default<O3>");
        assert_eq!(OptimizationLevel::Os.pipeline_string(), "default<Os>");
        assert_eq!(OptimizationLevel::Oz.pipeline_string(), "default<Oz>");
    }

    #[test]
    fn test_optimization_level_vectorization() {
        assert!(!OptimizationLevel::O0.enables_loop_vectorization());
        assert!(!OptimizationLevel::O1.enables_loop_vectorization());
        assert!(OptimizationLevel::O2.enables_loop_vectorization());
        assert!(OptimizationLevel::O3.enables_loop_vectorization());
        assert!(!OptimizationLevel::Os.enables_loop_vectorization());
        assert!(!OptimizationLevel::Oz.enables_loop_vectorization());
    }

    #[test]
    fn test_optimization_level_unrolling() {
        assert!(!OptimizationLevel::O0.enables_loop_unrolling());
        assert!(OptimizationLevel::O1.enables_loop_unrolling());
        assert!(OptimizationLevel::O2.enables_loop_unrolling());
        assert!(OptimizationLevel::O3.enables_loop_unrolling());
        assert!(!OptimizationLevel::Os.enables_loop_unrolling());
        assert!(!OptimizationLevel::Oz.enables_loop_unrolling());
    }

    #[test]
    fn test_optimization_level_merge_functions() {
        assert!(!OptimizationLevel::O0.enables_merge_functions());
        assert!(!OptimizationLevel::O3.enables_merge_functions());
        assert!(OptimizationLevel::Os.enables_merge_functions());
        assert!(OptimizationLevel::Oz.enables_merge_functions());
    }

    #[test]
    fn test_optimization_level_display() {
        assert_eq!(format!("{}", OptimizationLevel::O0), "O0");
        assert_eq!(format!("{}", OptimizationLevel::O3), "O3");
        assert_eq!(format!("{}", OptimizationLevel::Os), "Os");
    }

    #[test]
    fn test_optimization_level_default() {
        assert_eq!(OptimizationLevel::default(), OptimizationLevel::O0);
    }

    #[test]
    fn test_optimization_level_display_all() {
        assert_eq!(format!("{}", OptimizationLevel::O0), "O0");
        assert_eq!(format!("{}", OptimizationLevel::O1), "O1");
        assert_eq!(format!("{}", OptimizationLevel::O2), "O2");
        assert_eq!(format!("{}", OptimizationLevel::O3), "O3");
        assert_eq!(format!("{}", OptimizationLevel::Os), "Os");
        assert_eq!(format!("{}", OptimizationLevel::Oz), "Oz");
    }

    // -- LtoMode tests --

    #[test]
    fn test_lto_mode_prelink_pipeline() {
        assert_eq!(
            LtoMode::Off.prelink_pipeline_string(OptimizationLevel::O2),
            None
        );
        assert_eq!(
            LtoMode::Thin.prelink_pipeline_string(OptimizationLevel::O2),
            Some("thinlto-pre-link<O2>".to_string())
        );
        assert_eq!(
            LtoMode::Full.prelink_pipeline_string(OptimizationLevel::O2),
            Some("lto-pre-link<O2>".to_string())
        );
    }

    #[test]
    fn test_lto_mode_lto_pipeline() {
        assert_eq!(
            LtoMode::Off.lto_pipeline_string(OptimizationLevel::O3),
            None
        );
        assert_eq!(
            LtoMode::Thin.lto_pipeline_string(OptimizationLevel::O3),
            Some("thinlto<O3>".to_string())
        );
        assert_eq!(
            LtoMode::Full.lto_pipeline_string(OptimizationLevel::O3),
            Some("lto<O3>".to_string())
        );
    }

    #[test]
    fn test_lto_mode_display() {
        assert_eq!(format!("{}", LtoMode::Off), "off");
        assert_eq!(format!("{}", LtoMode::Thin), "thin");
        assert_eq!(format!("{}", LtoMode::Full), "full");
    }

    #[test]
    fn test_lto_mode_default() {
        assert_eq!(LtoMode::default(), LtoMode::Off);
    }

    #[test]
    fn test_lto_mode_pipelines_all_opt_levels() {
        // Test all opt levels with Thin LTO
        assert_eq!(
            LtoMode::Thin.prelink_pipeline_string(OptimizationLevel::O0),
            Some("thinlto-pre-link<O0>".to_string())
        );
        assert_eq!(
            LtoMode::Thin.prelink_pipeline_string(OptimizationLevel::O1),
            Some("thinlto-pre-link<O1>".to_string())
        );
        assert_eq!(
            LtoMode::Thin.prelink_pipeline_string(OptimizationLevel::Os),
            Some("thinlto-pre-link<Os>".to_string())
        );
        assert_eq!(
            LtoMode::Thin.prelink_pipeline_string(OptimizationLevel::Oz),
            Some("thinlto-pre-link<Oz>".to_string())
        );

        // Test all opt levels with Full LTO
        assert_eq!(
            LtoMode::Full.lto_pipeline_string(OptimizationLevel::O0),
            Some("lto<O0>".to_string())
        );
        assert_eq!(
            LtoMode::Full.lto_pipeline_string(OptimizationLevel::O1),
            Some("lto<O1>".to_string())
        );
        assert_eq!(
            LtoMode::Full.lto_pipeline_string(OptimizationLevel::Os),
            Some("lto<Os>".to_string())
        );
        assert_eq!(
            LtoMode::Full.lto_pipeline_string(OptimizationLevel::Oz),
            Some("lto<Oz>".to_string())
        );
    }

    // -- OptimizationConfig tests --

    #[test]
    fn test_config_presets() {
        let debug = OptimizationConfig::debug();
        assert_eq!(debug.level, OptimizationLevel::O0);
        assert_eq!(debug.lto, LtoMode::Off);

        let release = OptimizationConfig::release();
        assert_eq!(release.level, OptimizationLevel::O2);

        let aggressive = OptimizationConfig::aggressive();
        assert_eq!(aggressive.level, OptimizationLevel::O3);

        let size = OptimizationConfig::size();
        assert_eq!(size.level, OptimizationLevel::Os);

        let min_size = OptimizationConfig::min_size();
        assert_eq!(min_size.level, OptimizationLevel::Oz);
    }

    #[test]
    fn test_config_builder_pattern() {
        let config = OptimizationConfig::new(OptimizationLevel::O2)
            .with_lto(LtoMode::Thin)
            .with_loop_vectorization(true)
            .with_slp_vectorization(false)
            .with_inliner_threshold(250)
            .with_verify_each(true);

        assert_eq!(config.level, OptimizationLevel::O2);
        assert_eq!(config.lto, LtoMode::Thin);
        assert_eq!(config.loop_vectorization, Some(true));
        assert_eq!(config.slp_vectorization, Some(false));
        assert_eq!(config.inliner_threshold, Some(250));
        assert!(config.verify_each);
    }

    #[test]
    fn test_config_pipeline_string_normal() {
        let config = OptimizationConfig::new(OptimizationLevel::O3);
        assert_eq!(config.pipeline_string(), "default<O3>");
    }

    #[test]
    fn test_config_pipeline_string_lto_prelink() {
        let config = OptimizationConfig::new(OptimizationLevel::O2).with_lto(LtoMode::Thin);
        assert_eq!(config.pipeline_string(), "thinlto-pre-link<O2>");
    }

    #[test]
    fn test_config_pipeline_string_lto_phase() {
        let config = OptimizationConfig::new(OptimizationLevel::O2)
            .with_lto(LtoMode::Full)
            .as_lto_phase();
        assert_eq!(config.pipeline_string(), "lto<O2>");
    }

    #[test]
    fn test_config_effective_settings() {
        // O2 should enable vectorization by default
        let o2 = OptimizationConfig::new(OptimizationLevel::O2);
        assert!(o2.effective_loop_vectorization());
        assert!(o2.effective_slp_vectorization());
        assert!(o2.effective_loop_unrolling());

        // O0 should disable vectorization by default
        let o0 = OptimizationConfig::new(OptimizationLevel::O0);
        assert!(!o0.effective_loop_vectorization());
        assert!(!o0.effective_loop_unrolling());

        // Override should work
        let overridden =
            OptimizationConfig::new(OptimizationLevel::O0).with_loop_vectorization(true);
        assert!(overridden.effective_loop_vectorization());
    }

    #[test]
    fn test_config_extra_passes() {
        let config =
            OptimizationConfig::new(OptimizationLevel::O2).with_extra_passes("instcombine,dce");
        assert_eq!(config.extra_passes, Some("instcombine,dce".to_string()));
    }

    #[test]
    fn test_config_effective_loop_interleaving() {
        // Loop interleaving follows loop unrolling by default
        let config = OptimizationConfig::new(OptimizationLevel::O2);
        assert!(config.effective_loop_interleaving());

        let config = OptimizationConfig::new(OptimizationLevel::O0);
        assert!(!config.effective_loop_interleaving());

        // Can be explicitly set
        let config = OptimizationConfig::new(OptimizationLevel::O0).with_loop_unrolling(false);
        let config = OptimizationConfig {
            loop_interleaving: Some(true),
            ..config
        };
        assert!(config.effective_loop_interleaving());
    }

    #[test]
    fn test_config_effective_merge_functions() {
        // Size levels enable merge_functions by default
        let config = OptimizationConfig::new(OptimizationLevel::Os);
        assert!(config.effective_merge_functions());

        let config = OptimizationConfig::new(OptimizationLevel::Oz);
        assert!(config.effective_merge_functions());

        // O3 doesn't enable merge_functions by default
        let config = OptimizationConfig::new(OptimizationLevel::O3);
        assert!(!config.effective_merge_functions());

        // Can be explicitly enabled
        let config = OptimizationConfig::new(OptimizationLevel::O3).with_merge_functions(true);
        assert!(config.effective_merge_functions());
    }

    #[test]
    fn test_config_with_debug_logging() {
        let config = OptimizationConfig::new(OptimizationLevel::O2).with_debug_logging(true);
        assert!(config.debug_logging);
    }

    // -- Error tests --

    #[test]
    fn test_optimization_error_display() {
        let err = OptimizationError::PassBuilderOptionsCreationFailed;
        assert!(format!("{err}").contains("pass builder"));

        let err = OptimizationError::PassesFailed {
            message: "test error".to_string(),
        };
        assert!(format!("{err}").contains("test error"));

        let err = OptimizationError::InvalidPipeline {
            pipeline: "bad".to_string(),
            message: "invalid".to_string(),
        };
        assert!(format!("{err}").contains("bad"));
    }

    // -- Integration tests (require LLVM) --

    #[test]
    fn test_run_optimization_passes_o0() {
        use ori_llvm::inkwell::context::Context;

        // Initialize native target
        if ori_llvm::inkwell::targets::Target::initialize_native(
            &ori_llvm::inkwell::targets::InitializationConfig::default(),
        )
        .is_err()
        {
            // Skip if native target unavailable
            return;
        }

        let context = Context::create();
        let module = context.create_module("test");

        // Create a simple function
        let i64_type = context.i64_type();
        let fn_type = i64_type.fn_type(&[i64_type.into()], false);
        let function = module.add_function("identity", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        let builder = context.create_builder();
        builder.position_at_end(entry);
        let param = function.get_first_param().unwrap().into_int_value();
        builder.build_return(Some(&param)).unwrap();

        // Create target machine
        let triple = ori_llvm::inkwell::targets::TargetMachine::get_default_triple();
        let target = ori_llvm::inkwell::targets::Target::from_triple(&triple).unwrap();
        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                ori_llvm::inkwell::OptimizationLevel::None,
                ori_llvm::inkwell::targets::RelocMode::Default,
                ori_llvm::inkwell::targets::CodeModel::Default,
            )
            .unwrap();

        // Configure module
        module.set_triple(&triple);
        module.set_data_layout(&target_machine.get_target_data().get_data_layout());

        // Run O0 optimization (should succeed even with minimal passes)
        let config = OptimizationConfig::debug();
        let result = run_optimization_passes(&module, &target_machine, &config);
        assert!(result.is_ok(), "O0 optimization failed: {result:?}");
    }

    #[test]
    fn test_run_optimization_passes_o2() {
        use ori_llvm::inkwell::context::Context;

        if ori_llvm::inkwell::targets::Target::initialize_native(
            &ori_llvm::inkwell::targets::InitializationConfig::default(),
        )
        .is_err()
        {
            return;
        }

        let context = Context::create();
        let module = context.create_module("test");

        // Create a function with some optimization opportunity
        let i64_type = context.i64_type();
        let fn_type = i64_type.fn_type(&[i64_type.into()], false);
        let function = module.add_function("add_zero", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        let builder = context.create_builder();
        builder.position_at_end(entry);
        let param = function.get_first_param().unwrap().into_int_value();
        let zero = i64_type.const_int(0, false);
        let result = builder.build_int_add(param, zero, "add").unwrap();
        builder.build_return(Some(&result)).unwrap();

        let triple = ori_llvm::inkwell::targets::TargetMachine::get_default_triple();
        let target = ori_llvm::inkwell::targets::Target::from_triple(&triple).unwrap();
        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                ori_llvm::inkwell::OptimizationLevel::Default,
                ori_llvm::inkwell::targets::RelocMode::Default,
                ori_llvm::inkwell::targets::CodeModel::Default,
            )
            .unwrap();

        module.set_triple(&triple);
        module.set_data_layout(&target_machine.get_target_data().get_data_layout());

        // Run O2 optimization
        let config = OptimizationConfig::release();
        let result = run_optimization_passes(&module, &target_machine, &config);
        assert!(result.is_ok(), "O2 optimization failed: {result:?}");
    }

    #[test]
    fn test_run_optimization_passes_o3() {
        use ori_llvm::inkwell::context::Context;

        if ori_llvm::inkwell::targets::Target::initialize_native(
            &ori_llvm::inkwell::targets::InitializationConfig::default(),
        )
        .is_err()
        {
            return;
        }

        let context = Context::create();
        let module = context.create_module("test_o3");

        let i64_type = context.i64_type();
        let fn_type = i64_type.fn_type(&[i64_type.into()], false);
        let function = module.add_function("square", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        let builder = context.create_builder();
        builder.position_at_end(entry);
        let param = function.get_first_param().unwrap().into_int_value();
        let result = builder.build_int_mul(param, param, "sq").unwrap();
        builder.build_return(Some(&result)).unwrap();

        let triple = ori_llvm::inkwell::targets::TargetMachine::get_default_triple();
        let target = ori_llvm::inkwell::targets::Target::from_triple(&triple).unwrap();
        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                ori_llvm::inkwell::OptimizationLevel::Aggressive,
                ori_llvm::inkwell::targets::RelocMode::Default,
                ori_llvm::inkwell::targets::CodeModel::Default,
            )
            .unwrap();

        module.set_triple(&triple);
        module.set_data_layout(&target_machine.get_target_data().get_data_layout());

        // Run O3 optimization
        let config = OptimizationConfig::aggressive();
        let result = run_optimization_passes(&module, &target_machine, &config);
        assert!(result.is_ok(), "O3 optimization failed: {result:?}");
    }

    #[test]
    fn test_run_optimization_passes_size() {
        use ori_llvm::inkwell::context::Context;

        if ori_llvm::inkwell::targets::Target::initialize_native(
            &ori_llvm::inkwell::targets::InitializationConfig::default(),
        )
        .is_err()
        {
            return;
        }

        let context = Context::create();
        let module = context.create_module("test_size");

        let i64_type = context.i64_type();
        let fn_type = i64_type.fn_type(&[], false);
        let function = module.add_function("const_val", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        let builder = context.create_builder();
        builder.position_at_end(entry);
        builder
            .build_return(Some(&i64_type.const_int(42, false)))
            .unwrap();

        let triple = ori_llvm::inkwell::targets::TargetMachine::get_default_triple();
        let target = ori_llvm::inkwell::targets::Target::from_triple(&triple).unwrap();
        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                ori_llvm::inkwell::OptimizationLevel::Default,
                ori_llvm::inkwell::targets::RelocMode::Default,
                ori_llvm::inkwell::targets::CodeModel::Default,
            )
            .unwrap();

        module.set_triple(&triple);
        module.set_data_layout(&target_machine.get_target_data().get_data_layout());

        // Run Os optimization
        let config = OptimizationConfig::size();
        let result = run_optimization_passes(&module, &target_machine, &config);
        assert!(result.is_ok(), "Os optimization failed: {result:?}");

        // Run Oz optimization
        let config = OptimizationConfig::min_size();
        let result = run_optimization_passes(&module, &target_machine, &config);
        assert!(result.is_ok(), "Oz optimization failed: {result:?}");
    }

    #[test]
    fn test_run_custom_pipeline() {
        use ori_llvm::inkwell::context::Context;

        if ori_llvm::inkwell::targets::Target::initialize_native(
            &ori_llvm::inkwell::targets::InitializationConfig::default(),
        )
        .is_err()
        {
            return;
        }

        let context = Context::create();
        let module = context.create_module("test");

        let void_type = context.void_type();
        let fn_type = void_type.fn_type(&[], false);
        let function = module.add_function("empty", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        let builder = context.create_builder();
        builder.position_at_end(entry);
        builder.build_return(None).unwrap();

        let triple = ori_llvm::inkwell::targets::TargetMachine::get_default_triple();
        let target = ori_llvm::inkwell::targets::Target::from_triple(&triple).unwrap();
        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                ori_llvm::inkwell::OptimizationLevel::None,
                ori_llvm::inkwell::targets::RelocMode::Default,
                ori_llvm::inkwell::targets::CodeModel::Default,
            )
            .unwrap();

        module.set_triple(&triple);
        module.set_data_layout(&target_machine.get_target_data().get_data_layout());

        // Run a custom minimal pipeline
        let result = run_custom_pipeline(&module, &target_machine, "function(verify)");
        assert!(result.is_ok(), "Custom pipeline failed: {result:?}");
    }

    #[test]
    fn test_config_with_extra_passes_integration() {
        use ori_llvm::inkwell::context::Context;

        if ori_llvm::inkwell::targets::Target::initialize_native(
            &ori_llvm::inkwell::targets::InitializationConfig::default(),
        )
        .is_err()
        {
            return;
        }

        let context = Context::create();
        let module = context.create_module("test");

        let i64_type = context.i64_type();
        let fn_type = i64_type.fn_type(&[], false);
        let function = module.add_function("const_42", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        let builder = context.create_builder();
        builder.position_at_end(entry);
        let val = i64_type.const_int(42, false);
        builder.build_return(Some(&val)).unwrap();

        let triple = ori_llvm::inkwell::targets::TargetMachine::get_default_triple();
        let target = ori_llvm::inkwell::targets::Target::from_triple(&triple).unwrap();
        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                ori_llvm::inkwell::OptimizationLevel::None,
                ori_llvm::inkwell::targets::RelocMode::Default,
                ori_llvm::inkwell::targets::CodeModel::Default,
            )
            .unwrap();

        module.set_triple(&triple);
        module.set_data_layout(&target_machine.get_target_data().get_data_layout());

        // Run with extra passes
        let config = OptimizationConfig::debug().with_extra_passes("function(verify)");
        let result = run_optimization_passes(&module, &target_machine, &config);
        assert!(
            result.is_ok(),
            "Optimization with extra passes failed: {result:?}"
        );
    }

    #[test]
    fn test_invalid_custom_pipeline() {
        use ori_llvm::inkwell::context::Context;

        if ori_llvm::inkwell::targets::Target::initialize_native(
            &ori_llvm::inkwell::targets::InitializationConfig::default(),
        )
        .is_err()
        {
            return;
        }

        let context = Context::create();
        let module = context.create_module("test_invalid");

        let void_type = context.void_type();
        let fn_type = void_type.fn_type(&[], false);
        let function = module.add_function("empty", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        let builder = context.create_builder();
        builder.position_at_end(entry);
        builder.build_return(None).unwrap();

        let triple = ori_llvm::inkwell::targets::TargetMachine::get_default_triple();
        let target = ori_llvm::inkwell::targets::Target::from_triple(&triple).unwrap();
        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                ori_llvm::inkwell::OptimizationLevel::None,
                ori_llvm::inkwell::targets::RelocMode::Default,
                ori_llvm::inkwell::targets::CodeModel::Default,
            )
            .unwrap();

        module.set_triple(&triple);
        module.set_data_layout(&target_machine.get_target_data().get_data_layout());

        // Run with invalid pipeline - should fail
        let result = run_custom_pipeline(&module, &target_machine, "not-a-real-pass");
        assert!(result.is_err());
        if let Err(OptimizationError::PassesFailed { message }) = result {
            assert!(!message.is_empty());
        }
    }
}
