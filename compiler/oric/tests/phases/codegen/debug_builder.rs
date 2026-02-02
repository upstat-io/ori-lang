//! Debug info builder tests.
//!
//! Tests for `DebugInfoBuilder` creation, basic type generation, and function debug info.

#[cfg(feature = "llvm")]
mod tests {
    use std::path::Path;

    use ori_llvm::aot::debug::{DebugInfoBuilder, DebugInfoConfig, DebugLevel};
    use ori_llvm::inkwell::context::Context;
    use ori_llvm::inkwell::debug_info::AsDIScope;

    #[test]
    fn test_debug_info_builder_disabled() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::None);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".");
        assert!(builder.is_none());
    }

    #[test]
    fn test_debug_info_builder_creation() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", "src");
        assert!(builder.is_some());

        let builder = builder.unwrap();
        assert_eq!(builder.level(), DebugLevel::Full);
    }

    #[test]
    fn test_debug_info_builder_basic_types() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        // Create basic types (should not panic)
        let _int_ty = builder.int_type().unwrap();
        let _float_ty = builder.float_type().unwrap();
        let _bool_ty = builder.bool_type().unwrap();
        let _char_ty = builder.char_type().unwrap();
        let _byte_ty = builder.byte_type().unwrap();

        // Second call should return cached type
        let int_ty1 = builder.int_type().unwrap();
        let int_ty2 = builder.int_type().unwrap();
        // Types should be equal (same pointer) - use as_mut_ptr for comparison
        assert_eq!(int_ty1.as_mut_ptr(), int_ty2.as_mut_ptr());

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_info_builder_function() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        // Create a function
        let fn_type = context.void_type().fn_type(&[], false);
        let fn_val = module.add_function("test_func", fn_type, None);

        // Create function debug info
        let void_ty = builder.void_type().unwrap();
        let subroutine = builder.create_subroutine_type(Some(void_ty.as_type()), &[]);
        let subprogram = builder.create_function("test_func", None, 1, subroutine, false, true);

        // Attach to function
        builder.attach_function(fn_val, subprogram);

        // Add entry block
        let entry = context.append_basic_block(fn_val, "entry");
        let ir_builder = context.create_builder();
        ir_builder.position_at_end(entry);

        // Set debug location
        builder.set_location(&ir_builder, 2, 1, subprogram.as_debug_info_scope());

        ir_builder.build_return(None).unwrap();

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_info_builder_lexical_block() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        // Create function
        let subprogram = builder.create_simple_function("test_func", 1).unwrap();

        // Create lexical block
        let block = builder.create_lexical_block(subprogram.as_debug_info_scope(), 2, 1);

        // Use scope stack via public API
        builder.push_scope(subprogram.as_debug_info_scope());
        // Current scope should be the function
        assert_eq!(
            builder.current_scope().as_mut_ptr(),
            subprogram.as_debug_info_scope().as_mut_ptr()
        );

        builder.push_scope(block.as_debug_info_scope());
        // Current scope should be the lexical block
        assert_eq!(
            builder.current_scope().as_mut_ptr(),
            block.as_debug_info_scope().as_mut_ptr()
        );

        // Pop should restore to function scope
        builder.pop_scope();
        assert_eq!(
            builder.current_scope().as_mut_ptr(),
            subprogram.as_debug_info_scope().as_mut_ptr()
        );

        builder.finalize();
    }

    #[test]
    fn test_debug_info_builder_from_path() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let path = Path::new("/home/user/project/src/main.ori");
        let builder = DebugInfoBuilder::from_path(&module, &context, config, path);
        assert!(builder.is_some());
    }

    #[test]
    fn test_debug_info_builder_scope_management() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        // Initial scope should be compile unit
        let initial = builder.current_scope();
        assert_eq!(
            initial.as_mut_ptr(),
            builder.compile_unit().as_debug_info_scope().as_mut_ptr()
        );

        // Push function scope
        let subprogram = builder.create_simple_function("func", 1).unwrap();
        builder.push_scope(subprogram.as_debug_info_scope());

        // Current scope should be function
        let current = builder.current_scope();
        assert_eq!(
            current.as_mut_ptr(),
            subprogram.as_debug_info_scope().as_mut_ptr()
        );

        // Pop should restore to compile unit
        builder.pop_scope();
        let after_pop = builder.current_scope();
        assert_eq!(
            after_pop.as_mut_ptr(),
            builder.compile_unit().as_debug_info_scope().as_mut_ptr()
        );

        builder.finalize();
    }

    #[test]
    fn test_debug_builder_level_accessor() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::LineTablesOnly);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        assert_eq!(builder.level(), DebugLevel::LineTablesOnly);

        builder.finalize();
    }

    #[test]
    fn test_debug_builder_file_accessor() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "myfile.ori", "/home/user")
            .expect("debug info should be enabled");

        // Verify file() returns the file metadata
        let file = builder.file();
        assert!(!file.as_mut_ptr().is_null());

        builder.finalize();
    }

    #[test]
    fn test_debug_builder_compile_unit_accessor() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        // Verify compile_unit() returns the compile unit
        let cu = builder.compile_unit();
        assert!(!cu.as_mut_ptr().is_null());

        builder.finalize();
    }

    #[test]
    fn test_debug_subroutine_with_void_return() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        // Void function: () -> void
        let subroutine = builder.create_subroutine_type(None, &[]);
        let _subprogram = builder.create_function("void_func", None, 1, subroutine, false, true);

        // Function with linkage name
        let subroutine2 = builder.create_subroutine_type(None, &[]);
        let _subprogram2 = builder.create_function(
            "exported_func",
            Some("_ori_exported_func"),
            2,
            subroutine2,
            false,
            true,
        );

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_char_type() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        // Test char_type accessor
        let char_ty = builder.char_type().unwrap();
        assert!(!char_ty.as_mut_ptr().is_null());

        builder.finalize();
        assert!(module.verify().is_ok());
    }
}
