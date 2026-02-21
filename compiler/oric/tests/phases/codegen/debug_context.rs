//! Debug context and line map tests.
//!
//! Tests for `DebugContext` (combined debug info + line mapping) and `LineMap`
//! (source offset to line/column conversion).

#[cfg(feature = "llvm")]
mod tests {
    use std::path::Path;

    use ori_llvm::aot::debug::{DebugContext, DebugInfoConfig, DebugLevel, LineMap};
    use ori_llvm::inkwell::context::Context;
    use ori_llvm::inkwell::debug_info::AsDIScope;

    // -- LineMap tests --

    #[test]
    fn test_line_map_simple() {
        let source = "line1\nline2\nline3";
        let map = LineMap::new(source);

        assert_eq!(map.line_count(), 3);

        // First character of each line
        assert_eq!(map.offset_to_line_col(0), (1, 1)); // 'l' in line1
        assert_eq!(map.offset_to_line_col(6), (2, 1)); // 'l' in line2
        assert_eq!(map.offset_to_line_col(12), (3, 1)); // 'l' in line3

        // Middle of lines
        assert_eq!(map.offset_to_line_col(2), (1, 3)); // 'n' in line1
        assert_eq!(map.offset_to_line_col(8), (2, 3)); // 'n' in line2
    }

    #[test]
    fn test_line_map_empty() {
        let source = "";
        let map = LineMap::new(source);
        assert_eq!(map.line_count(), 1);
        assert_eq!(map.offset_to_line_col(0), (1, 1));
    }

    #[test]
    fn test_line_map_single_line() {
        let source = "hello";
        let map = LineMap::new(source);
        assert_eq!(map.line_count(), 1);
        assert_eq!(map.offset_to_line_col(0), (1, 1));
        assert_eq!(map.offset_to_line_col(4), (1, 5));
    }

    #[test]
    fn test_line_map_trailing_newline() {
        let source = "line1\nline2\n";
        let map = LineMap::new(source);
        assert_eq!(map.line_count(), 3); // Empty line after trailing newline
    }

    #[test]
    fn test_line_map_offset_at_start() {
        let source = "first\nsecond\nthird";
        let map = LineMap::new(source);

        // Offset 0 should be line 1, column 1
        let (line, col) = map.offset_to_line_col(0);
        assert_eq!(line, 1);
        assert_eq!(col, 1);
    }

    #[test]
    fn test_line_map_mid_line() {
        let source = "hello world";
        let map = LineMap::new(source);

        // Offset 6 should be line 1, column 7 ("w" in "world")
        let (line, col) = map.offset_to_line_col(6);
        assert_eq!(line, 1);
        assert_eq!(col, 7);
    }

    #[test]
    fn test_line_map_multiline() {
        let source = "line1\nline2\nline3";
        let map = LineMap::new(source);

        // Start of line 2 (offset 6)
        let (line, col) = map.offset_to_line_col(6);
        assert_eq!(line, 2);
        assert_eq!(col, 1);

        // Middle of line 3 (offset 14, "ne3")
        let (line, col) = map.offset_to_line_col(14);
        assert_eq!(line, 3);
        assert_eq!(col, 3);
    }

    #[test]
    fn test_line_map_past_end() {
        let source = "short";
        let map = LineMap::new(source);

        // Offset past end should return last line
        let (line, _col) = map.offset_to_line_col(100);
        assert_eq!(line, 1);
        // Column will be clamped to line length
    }

    // -- DebugContext tests --

    #[test]
    fn test_debug_context_creation() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let source = "line1\nline2\nline3";
        let path = Path::new("/src/test.ori");

        let ctx = DebugContext::new(&module, &context, config, path, source);
        assert!(ctx.is_some());

        let ctx = ctx.unwrap();

        // Test offset to line/col
        assert_eq!(ctx.offset_to_line_col(0), (1, 1)); // Start of line 1
        assert_eq!(ctx.offset_to_line_col(6), (2, 1)); // Start of line 2
    }

    #[test]
    fn test_debug_context_function_at_offset() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let source = "// comment\n@main () -> void = {}";
        let path = Path::new("/src/main.ori");

        let ctx = DebugContext::new(&module, &context, config, path, source)
            .expect("debug info should be enabled");

        // Create function at offset 11 (start of @main on line 2)
        let _subprogram = ctx.create_function_at_offset("main", 11).unwrap();

        ctx.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_context_scope_management() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let source = "@outer () -> void = {\n  let x = 1\n}";
        let path = Path::new("/src/test.ori");

        let ctx = DebugContext::new(&module, &context, config, path, source)
            .expect("debug info should be enabled");

        // Create function at line 1
        let subprogram = ctx.create_function_at_offset("outer", 0).unwrap();

        // Enter function scope
        ctx.enter_function(subprogram);

        // Create lexical block for the body (line 2)
        let _block = ctx.create_lexical_block_at_offset(
            subprogram.as_debug_info_scope(),
            22, // Start of "let x = 1"
        );

        // Exit function scope
        ctx.exit_function();

        ctx.finalize();
    }

    #[test]
    fn test_debug_context_location_from_offset() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let source = "let x = 42\nlet y = x + 1";
        let path = Path::new("/src/test.ori");

        let ctx = DebugContext::new(&module, &context, config, path, source)
            .expect("debug info should be enabled");

        // Create a function
        let fn_type = context.void_type().fn_type(&[], false);
        let fn_val = module.add_function("test_func", fn_type, None);
        let subprogram = ctx.create_function_at_offset("test_func", 0).unwrap();
        fn_val.set_subprogram(subprogram);

        // Create entry block
        let entry = context.append_basic_block(fn_val, "entry");
        let ir_builder = context.create_builder();
        ir_builder.position_at_end(entry);

        // Enter function scope
        ctx.enter_function(subprogram);

        // Set location at offset 0 (line 1, col 1)
        ctx.set_location_from_offset_in_current_scope(&ir_builder, 0);

        // Build an instruction with this location
        ir_builder.build_return(None).unwrap();

        ctx.exit_function();
        ctx.finalize();

        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_context_disabled() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::None);

        let source = "let x = 1";
        let path = Path::new("/src/test.ori");

        let ctx = DebugContext::new(&module, &context, config, path, source);
        assert!(ctx.is_none());
    }

    #[test]
    fn test_debug_context_with_function_type() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let source = "@add (a: int, b: int) -> int = a + b;";
        let path = Path::new("/src/math.ori");

        let ctx = DebugContext::new(&module, &context, config, path, source)
            .expect("debug info should be enabled");

        // Create function with type info
        let int_ty = ctx.di().int_type().unwrap().as_type();
        let subroutine = ctx.di().create_subroutine_type(
            Some(int_ty),      // return type
            &[int_ty, int_ty], // parameters
        );
        let _subprogram = ctx
            .di()
            .create_function("add", None, 1, subroutine, false, true);

        ctx.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_context_clear_location() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let source = "let x = 1";
        let path = Path::new("/src/test.ori");

        let ctx = DebugContext::new(&module, &context, config, path, source)
            .expect("debug info should be enabled");

        // Create a function
        let fn_type = context.void_type().fn_type(&[], false);
        let fn_val = module.add_function("test", fn_type, None);
        let subprogram = ctx.create_function_at_offset("test", 0).unwrap();
        fn_val.set_subprogram(subprogram);

        // Create entry block
        let entry = context.append_basic_block(fn_val, "entry");
        let ir_builder = context.create_builder();
        ir_builder.position_at_end(entry);

        ctx.enter_function(subprogram);

        // Set and then clear location
        ctx.set_location_from_offset_in_current_scope(&ir_builder, 4);
        ctx.di().clear_location(&ir_builder);

        ir_builder.build_return(None).unwrap();

        ctx.finalize();
        assert!(module.verify().is_ok());
    }
}
