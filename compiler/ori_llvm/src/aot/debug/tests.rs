use inkwell::context::Context;
use inkwell::debug_info::{AsDIScope, DWARFEmissionKind};
use inkwell::module::Module;

use super::*;

/// Test that `to_emission_kind` correctly maps `DebugLevel` variants.
/// This test must remain inline as it tests a private method.
#[test]
fn test_debug_level_emission_kind() {
    assert_eq!(DebugLevel::None.to_emission_kind(), DWARFEmissionKind::None);
    assert_eq!(
        DebugLevel::LineTablesOnly.to_emission_kind(),
        DWARFEmissionKind::LineTablesOnly
    );
    assert_eq!(DebugLevel::Full.to_emission_kind(), DWARFEmissionKind::Full);
}

/// Helper: create a DebugInfoBuilder with Full level for testing.
fn make_test_di<'ctx>(module: &Module<'ctx>, context: &'ctx Context) -> DebugInfoBuilder<'ctx> {
    DebugInfoBuilder::new(
        module,
        context,
        DebugInfoConfig::development(),
        "test.ori",
        "/tmp",
    )
    .expect("DebugInfoBuilder::new should succeed for Full level")
}

#[test]
fn create_auto_variable_produces_valid_metadata() {
    let ctx = Context::create();
    let module = ctx.create_module("test_auto_var");
    let di = make_test_di(&module, &ctx);

    let int_ty = di.int_type().unwrap().as_type();
    let scope = di.compile_unit().as_debug_info_scope();
    let var = di.create_auto_variable(scope, "x", 1, int_ty);

    // The variable should be non-null (valid metadata)
    assert!(!var.as_mut_ptr().is_null());

    di.finalize();
    assert!(
        module.verify().is_ok(),
        "module should verify after finalize"
    );
}

#[test]
fn create_parameter_variable_produces_valid_metadata() {
    let ctx = Context::create();
    let module = ctx.create_module("test_param_var");
    let di = make_test_di(&module, &ctx);

    let int_ty = di.int_type().unwrap().as_type();
    let scope = di.compile_unit().as_debug_info_scope();
    let var = di.create_parameter_variable(scope, "a", 1, 10, int_ty);

    assert!(!var.as_mut_ptr().is_null());

    di.finalize();
    assert!(
        module.verify().is_ok(),
        "module should verify after finalize"
    );
}

#[test]
fn emit_dbg_declare_on_alloca_passes_verify() {
    let ctx = Context::create();
    let module = ctx.create_module("test_dbg_declare");
    let di = make_test_di(&module, &ctx);
    let builder = ctx.create_builder();

    // Create a simple function with an alloca
    let void_ty = ctx.void_type();
    let fn_ty = void_ty.fn_type(&[], false);
    let func = module.add_function("test_fn", fn_ty, None);

    // Create and attach DISubprogram
    let subprogram = di.create_simple_function("test_fn", 1).unwrap();
    di.attach_function(func, subprogram);

    let entry = ctx.append_basic_block(func, "entry");
    builder.position_at_end(entry);

    // Alloca for a local variable
    let i64_ty = ctx.i64_type();
    let alloca = builder.build_alloca(i64_ty, "x").unwrap();

    // Emit dbg.declare
    let scope = subprogram.as_debug_info_scope();
    let int_di_ty = di.int_type().unwrap().as_type();
    let var = di.create_auto_variable(scope, "x", 2, int_di_ty);
    let loc = di.create_debug_location(2, 5, scope);
    di.emit_dbg_declare(alloca, var, loc, entry);

    // Set a location and return
    di.set_location(&builder, 3, 1, scope);
    builder.build_return(None).unwrap();

    di.finalize();
    assert!(
        module.verify().is_ok(),
        "module with dbg.declare should verify"
    );
}

#[test]
fn create_rc_heap_type_produces_two_field_struct() {
    let ctx = Context::create();
    let module = ctx.create_module("test_rc_type");
    let di = make_test_di(&module, &ctx);

    let int_ty = di.int_type().unwrap().as_type();
    let rc_type = di.create_rc_heap_type(int_ty, "int", 64).unwrap();

    // RC<int> = { strong_count: i64, data: int }
    // Total size should be 128 bits (64 + 64)
    assert!(!rc_type.as_type().as_mut_ptr().is_null());

    di.finalize();
    assert!(
        module.verify().is_ok(),
        "module should verify after finalize"
    );
}

#[test]
fn composite_type_cache_deduplicates() {
    let ctx = Context::create();
    let module = ctx.create_module("test_composite_cache");
    let di = make_test_di(&module, &ctx);

    let int_ty = di.int_type().unwrap().as_type();

    // Cache a composite type at index 42
    di.cache_composite_type(42, int_ty);
    assert!(di.get_cached_composite(42).is_some());
    assert!(di.get_cached_composite(99).is_none());

    di.finalize();
}

#[test]
fn debug_context_set_location_from_offset() {
    let ctx = Context::create();
    let module = ctx.create_module("test_dc_location");
    let builder = ctx.create_builder();

    // Source: "let x = 42\nlet y = 99\n"
    let source = "let x = 42\nlet y = 99\n";
    let dc = DebugContext::new(
        &module,
        &ctx,
        DebugInfoConfig::development(),
        std::path::Path::new("/tmp/test.ori"),
        source,
    )
    .expect("DebugContext::new should succeed for Full level");

    // Create a function so we have a scope and can build instructions
    let void_ty = ctx.void_type();
    let fn_ty = void_ty.fn_type(&[], false);
    let func = module.add_function("test_fn", fn_ty, None);
    let subprogram = dc.create_function_at_offset("test_fn", 0).unwrap();
    dc.di().attach_function(func, subprogram);

    let entry = ctx.append_basic_block(func, "entry");
    builder.position_at_end(entry);

    // Enter function scope and set location
    dc.enter_function(subprogram);
    dc.set_location_from_offset_in_current_scope(&builder, 0); // "let x" at offset 0
    dc.set_location_from_offset_in_current_scope(&builder, 12); // "let y" at offset 12
    dc.exit_function();

    // Set location for return
    dc.set_location_from_offset(&builder, 0, subprogram.as_debug_info_scope());
    builder.build_return(None).unwrap();

    dc.finalize();
    assert!(
        module.verify().is_ok(),
        "module with debug locations should verify"
    );
}

#[test]
fn debug_context_emit_declare_for_alloca_convenience() {
    let ctx = Context::create();
    let module = ctx.create_module("test_dc_declare");
    let builder = ctx.create_builder();

    let source = "let x = 42\n";
    let dc = DebugContext::new(
        &module,
        &ctx,
        DebugInfoConfig::development(),
        std::path::Path::new("/tmp/test.ori"),
        source,
    )
    .expect("DebugContext::new should succeed");

    let void_ty = ctx.void_type();
    let fn_ty = void_ty.fn_type(&[], false);
    let func = module.add_function("test_fn", fn_ty, None);
    let subprogram = dc.create_function_at_offset("test_fn", 0).unwrap();
    dc.di().attach_function(func, subprogram);

    let entry = ctx.append_basic_block(func, "entry");
    builder.position_at_end(entry);
    dc.enter_function(subprogram);

    // Alloca + convenience declare
    let i64_ty = ctx.i64_type();
    let alloca = builder.build_alloca(i64_ty, "x").unwrap();
    let int_di_ty = dc.di().int_type().unwrap().as_type();
    dc.emit_declare_for_alloca(alloca, "x", int_di_ty, 0, entry);

    // Return with location
    dc.set_location_from_offset_in_current_scope(&builder, 0);
    builder.build_return(None).unwrap();

    dc.exit_function();
    dc.finalize();
    assert!(
        module.verify().is_ok(),
        "module with convenience dbg.declare should verify"
    );
}

#[test]
fn line_map_offset_to_line_col() {
    let source = "let x = 42\nlet y = 99\nlet z = 0\n";
    let map = LineMap::new(source);

    // "let x = 42\n" = 11 chars (0..10), line 2 starts at offset 11
    // "let y = 99\n" = 11 chars (11..21), line 3 starts at offset 22
    assert_eq!(map.offset_to_line_col(0), (1, 1)); // 'l' in "let x"
    assert_eq!(map.offset_to_line_col(4), (1, 5)); // 'x' in "let x"
    assert_eq!(map.offset_to_line_col(11), (2, 1)); // start of line 2
    assert_eq!(map.offset_to_line_col(15), (2, 5)); // 'y' in "let y"
    assert_eq!(map.offset_to_line_col(22), (3, 1)); // start of line 3
    assert_eq!(map.line_count(), 4); // 3 newlines + initial line
}

#[test]
fn debug_none_level_returns_none_builder() {
    let ctx = Context::create();
    let module = ctx.create_module("test_none_level");
    let di = DebugInfoBuilder::new(
        &module,
        &ctx,
        DebugInfoConfig::new(DebugLevel::None),
        "test.ori",
        "/tmp",
    );
    assert!(
        di.is_none(),
        "DebugInfoBuilder::new should return None for DebugLevel::None"
    );
}
